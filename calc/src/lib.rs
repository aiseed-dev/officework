//! calc — xlsx互換の表計算。writer とは**別のソフト**。
//!
//! Office を一つのソフトにしない。文書は writer、表は calc。
//! 共有するのは書式(docx/xlsx)だけ。
//!
//! **マクロは無い。** 表の中に実行コードを置かない設計で、
//! 「開く=実行」という攻撃経路を最初から持たない。
//!
//!   calc            空で開く
//!   calc 表.xlsx    その表を開く

pub(crate) use std::ops::Range;
pub(crate) use std::path::PathBuf;
pub(crate) use std::collections::HashMap;
pub(crate) use std::rc::Rc;

pub(crate) use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, SharedString, UTF16Selection, Window,
    WindowBounds, WindowOptions,
};
pub(crate) use gpui_platform::application;
pub(crate) use kumihan::Editor;

pub(crate) use sheet::model::{Borders, CellFormat, HAlign};
pub(crate) use sheet::{recalc, recalc_book, Book, Cell, Pos, Value};
pub(crate) use ui::{handler, ribbon, HasEditor};

// **関数の表は face へ移した**(2026-08-15)。名前も分類も説明も
// 絵を描かない物で、Kotlin / Swift の殻も同じ表を読む。ここで
// 再公開しているので crate::funcs の呼び出しは今までどおり
// **calc が使うのは funcs だけ。** 14言語の表は face の中で
// funcs_tables が引くので、並べると使われない再公開になる
// (clippy が -D warnings で落とす)
pub(crate) use face::funcs;
// 14言語の突き合わせの試験だけが直に引く。**この試験は本当は face 側に
// 引っ越すべき**(表がそちらへ移ったので)— 別便で
#[cfg(test)]
pub(crate) use face::funcs_tables;
mod util;
pub(crate) use util::*;
mod py;
pub(crate) use py::*;
mod io;
pub(crate) use io::*;
mod picks;
// RPC は**ユニックスソケットが設計**(この機械の中だけ・ネイティブファースト)。
// Windows ではこの受け口ごと開かない — ops が cfg(unix) で学んだのと同じ線
// (0.2.0 で Windows の wheel を壊した教訓)
#[cfg(unix)]
mod rpc;
mod cmds;
mod panels;
mod view;
mod input;
mod objects;
mod state;
#[cfg(test)]
mod tests;

pub struct Calc {
    focus: FocusHandle,
    book: Book,
    active: usize,
    cursor: Pos,
    /// 範囲選択の起点(Shift+矢印/クリックで伸ばす)。無ければ1セル
    anchor: Option<Pos>,
    /// ドラッグ選択の始点(マウスの左を押した位置。離すと終わる)
    drag: Option<Pos>,
    /// 見出しの境界を掴んだドラッグ(列幅・行高)。セル選択の drag とは別
    size_drag: Option<SizeDrag>,
    /// 見出しを掴んだ選択ドラッグ(列か, 始まりの番号)。B→D と撫でて複数列
    head_drag: Option<(bool, u32)>,
    /// 画像の復号の控え(実体のアドレス → GPUI の画像)。
    /// 毎フレーム作り直すと復号と転送をやり直すことになる
    img_cache: std::cell::RefCell<std::collections::HashMap<usize, std::sync::Arc<gpui::Image>>>,
    /// 検索と置換の検索語(パネルを2枚続けて使う間の控え。次回の初期値にもなる)
    find_term: Option<String>,
    /// ゴールシークの途中の控え(目標セル, 目標値)
    goal: Option<(Pos, f64)>,
    /// ピボットの聞き取りの途中経過(元の範囲・見出し・決めた欄)
    pivot_pend: Option<PivotPend>,
    /// 小計の聞き取りの途中経過(同じ形の控えを使い回す)
    sub_pend: Option<PivotPend>,
    /// 並べ替えの「拡張しますか」の聞き取り中(昇順か)。Esc でやめる
    sort_pend: Option<bool>,
    /// 一覧(pick)の題 — いま何を選んでいるか(ピボットの段など)。
    /// 一覧の上に太字で出す。閉じるときに消す
    pick_note: Option<SharedString>,
    /// ピボットの絞り込みの聞き取り中: (指図の番号, 見出し, 隠す値の作業用)
    pivot_flt: Option<(usize, String, std::collections::BTreeSet<String>)>,
    /// 重複の削除の下ごしらえ: (列番号, 見せる名前, 比べるか) の列と「先頭行は見出し」
    pub(crate) dedup_pend: Option<(Vec<(u32, String, bool)>, bool)>,
    /// 条件付き書式のルールの管理で選んだ規則(sheet.cond の添字)
    pub(crate) cond_pend: Option<usize>,
    /// テキスト取り込みの下ごしらえ(ウィザードのパネルが持つ)
    pub(crate) import_pend: Option<crate::py::ImportPend>,
    /// 既定の書体(実在する家族に解決済み)。「Noto Sans JP」の名指しは
    /// 入っていない機械で**素通りして太字も効かなくなる**(発注者報告)
    pub(crate) font_name: gpui::SharedString,
    /// 罫線のアイコンの格子パレット(開いている位置)。掛けても閉じない
    pub(crate) border_pal: Option<(f32, f32)>,
    /// **操作の記録**(発注者 2026-08-15)。始めると、手でやった操作が
    /// Python の行として溜まる。止めると .py になり、編集の道具で開く。
    /// 主従が逆転した今、これが「何を書けばいいか」を教える唯一の道具。
    /// **書くのは記録だけ** — ブックを開く行は足さない(2026-08-16)。
    /// 走らせる物にするのは人の手で、置き場も別(records)
    pub(crate) rec: Option<Vec<String>>,
    /// **書式の差分を全部は言い表せなかった**(罫線・字下げ・回転など、
    /// Python の口がまだ無い項目が動いた)。立っていれば、行が書けていても
    /// 穴の註を残す — 半分だけ走るマクロは、走らないマクロより悪い
    pub(crate) rec_fmt_partial: bool,
    /// 記録に**最後に書いた選択の範囲**。次の行を書くときにここと違って
    /// いたら、先に `select()` の行を入れる(矢印1回ごとに書くと洪水になる)
    pub(crate) rec_sel: Option<String>,
    /// フィルハンドル(選択枠の右下の角)のドラッグ中。
    /// (元の選択の左上, 右下, いまの伸ばし先, Ctrl)。離した時に写す。
    /// Ctrl は本家と同じ「既定の裏返し」— 写す↔連続データ
    pub(crate) fill_drag: Option<(Pos, Pos, Pos, bool)>,
    /// 格子の面の窓の中での位置と大きさ(x, y, 幅, 高さ px)。描くたびに書く。
    /// リボンを押した窓の座標を、格子の面の座標に直すのに要る
    pub(crate) pane_box: std::cell::Cell<(f32, f32, f32, f32)>,
    /// **いま開こうとしている一覧を出す場所。** リボンのボタンを押した
    /// ときだけ入り、run_cmd を抜けたら消える。None ならセルの下に出す
    pub(crate) pop_at: Option<(f32, f32)>,
    /// この品書きは**子から直に開いた**(リボンの「条件付き書式」など)。
    /// 親を通っていないので、Esc は子と親をまとめて閉じる — でないと
    /// 押した覚えのない親の品書きが出てくる(2026-08-08 一巡点検で発見)
    pub(crate) menu_direct: bool,
    /// 中身を変えた回数(控えを取るたびに1つ増える)。**画面の一巡点検が
    /// 「押して何か起きたか」を見るのに使う**(tools/ribbon_sweep.py)
    pub(crate) edits: u64,
    /// リボンのボタンの場所(命令の名前 → 窓の中の x, y, 幅, 高さ)。
    /// 描くたびに書く。一覧を**押したボタンの真下**に出すのに要る
    pub(crate) btn_box: Rc<std::cell::RefCell<HashMap<&'static str, (f32, f32, f32, f32)>>>,
    /// いま開いている一覧を開いたリボンのボタンの幅。**幅の決め方が変わる** —
    /// セルから開いた一覧(0.0)は列の幅に合わせるが、リボンからのものは
    /// 中身に合わせ、ボタンの幅を下限・POP_W を上限にする
    pub(crate) pop_btn_w: std::cell::Cell<f32>,
    /// **開く元の上辺**(格子の面を基準にした y)。一覧が下に入らないとき
    /// 上へ開くのに要る(発注者 2026-08-15「場所によっては上に出さないと
    /// いけなかったり、上下に出す場合もある」)。pop_anchor が入れる
    pub(crate) pop_top: std::cell::Cell<f32>,
    /// 罫線のペン(線種と色)。罫線の一覧から掛けるときに使う
    pen_style: sheet::model::BStyle,
    pen_color: Option<u32>,
    /// ヘッダー/フッターの聞き取り中: (フッターか, 0=左 1=中 2=右)
    hf_pend: Option<(bool, u8)>,
    /// 名前マネージャーで選んだ名前(移動/打ち直し/削除の相手)
    name_pend: Option<String>,
    /// 名前を定義する途中(名前, 参照)。**適用範囲を訊く2段目まで持つ** —
    /// 範囲まで選んで初めて名前が入る(途中でやめたら何も残らない)
    name_new: Option<(String, String)>,
    /// 結合の確認待ちの種類(中央/横方向/結合だけ)
    /// 書式のコピー(刷毛)で持っている書式。次のクリックで塗って手放す
    brush: Option<CellFormat>,
    /// 右クリックメニューの出どころ(Some(true)=列見出し / Some(false)=行見出し)
    menu_head: Option<bool>,
    /// ソルバーの小窓(開いている間、打鍵は選んだ欄へ)
    solver: Option<Solver>,
    /// SmartArt の選択中の分類(2段の pick の1段目の答え)
    sa_cat: usize,
    /// スライサー(列の値を押して絞る板)。**見え方だけ** —
    /// 絞り込みと同じで、保存される中身は変わらない。
    ///
    /// **何枚でも開ける**(2026-08-13。前は `Option<Slicer>` の1枚だった)。
    /// 全部の板が同時に効く=**かつ**で絞る(Excel と同じ)
    slicers: Vec<Slicer>,
    /// いま触っている板(`slicers` の番号)。Alt+S / Alt+C と Esc が
    /// どの板に効くかを決める。板を押すたびに移る
    slicer_sel: usize,
    /// 設定の板を開いているか(大きさ・列数・スタイル・位置)。
    /// **いま触っている板のもの**を出す
    slicer_cfg: bool,
    /// 板の移動(番号, つかんだ点, つかんだときの左上)
    slicer_drag: Option<(usize, (f32, f32), (f32, f32))>,
    /// コメントを見せるか(共同編集タブで切替。隠しても付いたまま)
    show_comments: bool,
    /// 数学オートコレクト(`\alpha ` → `α `)を掛けるか。器は settings.toml。
    /// **既定は入** — 打っても何も起きないより、Backspace で戻せる形で
    /// 働くほうが分かる(戻せることが要件。台帳の札)
    autocorrect: bool,
    /// コメントの一覧の板(開いていれば並べ方を持つ)。**ブック全体**を並べる
    comment_list: Option<CommentList>,
    /// **Alt のキーヒント。** `Some(打った途中の札)` で出ている。
    /// 空なら段の札、段を選んだあとはその段のボタンの札
    key_hint: Option<String>,
    /// Alt を押してから、まだ他の鍵を使っていない。
    /// **Alt を単独で離した時だけ**ヒントを出すための見張り —
    /// Alt+S のような組み合わせのあとに出ては邪魔になる
    alt_armed: bool,
    /// 暗号化のパスワード(次の保存から効く。開いた暗号化ブックからも入る)
    encrypt_pw: Option<String>,
    /// 「開くために聞いている」パスワード待ちのファイル
    pw_pending: Option<PathBuf>,
    /// pick の一覧が指す実体(バージョン履歴・プラグインの表示名 → パス)
    pick_paths: Vec<(String, PathBuf)>,
    /// PY のスピルの台帳(シート番号, アンカー → 行×列)。次の計算で前の面を消す
    py_spills: std::collections::HashMap<(usize, Pos), (u32, u32)>,
    /// UDF を計算し終えたときの**セルごとの**指紋
    /// (シート番号, セル) → 引数の指紋。これと今の指紋が食い違うセルだけを
    /// 計算し直す。**シート全体で1つの指紋だと駄目**だった(2026-08-14):
    /// 走っている最中に増えたセルまで「計算済み」と控えてしまい、二度と
    /// 走らない(表の3行目だけ #PY? が残る)。差分にすると重さも消える —
    /// 変わっていないセルは python に投げない(発注者「UDF の呼び出しは重い」)
    udf_stamp: std::collections::HashMap<(usize, Pos), u64>,
    /// UDF の計算が走っている最中(二重に走らせない)
    udf_busy: bool,
    /// plugins の .py を編集している面(zed 側の半分。pyedit.rs)
    py_edit: Option<ui::pyedit::PyEdit>,
    /// 書きかけのまま閉じようとした = 一度断った(もう一度 Esc で捨てる)
    py_edit_ask: bool,
    /// plugins の手続きが走っている最中。この間 rpc の書き込みは undo の節目を
    /// 作らない — 手続きが何回セルを書いても **Ctrl+Z 一回で戻る**
    rpc_batch: bool,
    /// トレースの光り(参照元=青緑 / 参照先=橙)。見え方だけ、保存されない
    trace: Vec<(Pos, bool)>,
    /// 自分が置いた排他ロック(閉じるとき・別のファイルを開くときに外す)
    my_lock: Option<PathBuf>,
    /// 先客の名乗り(このファイルは誰かが開いている)。上書き保存を止める
    locked_by: Option<String>,
    /// 選択中の図形(shapes_new の番号)。Esc/他クリックで解除、Del で削除
    shape_sel: Option<usize>,
    /// 図形のドラッグ(番号, 掴んだ格子px, 掴んだ時のアンカーの格子px, 大きさ変更か)
    shape_drag: Option<(usize, (f32, f32), (f32, f32), bool)>,
    /// 図形の回転ドラッグ(枠の上の丸を掴んでいる間だけ Some)
    shape_rot: Option<usize>,
    /// **ポイントの編集**(頂点をつまむモード)。図形の番号。
    /// このモードの間は、枠のドラッグではなく点のドラッグになる
    point_edit: Option<usize>,
    /// いまつまんでいる取っ手(点の番号, 種類)
    pt_drag: Option<(usize, PtHandle)>,
    /// Ctrl+クリックで足した図形(shape_sel が主、こちらは控え。整列が使う)
    shape_multi: Vec<usize>,
    /// いま出ているメニューは図形の専用メニューか(右クリックが図形の上)
    menu_shape: bool,
    /// 図形の切り取り/コピーの控え(セルのクリップボードとは別の器)
    shape_clip: Option<sheet::model::SheetShape>,
    /// データテーブルのパネルの途中(列の入力セル)。行のパネルの確定まで持つ
    dt_col: Option<Pos>,
    /// 変更履歴の記録中: 開始時点の「打った姿」の写し(シート名 → セル)。
    /// **writer と同じ型** — 操作を拾わず、止めたときに差分を数える
    #[allow(clippy::type_complexity)]
    track_from: Option<Vec<(String, std::collections::BTreeMap<Pos, String>)>>,
    /// 選んでいる画像(images_new の番号)。グラフもここ
    img_sel: Option<usize>,
    img_drag: Option<(usize, (f32, f32), (f32, f32), bool)>,
    /// ホイールの端数(触パネルの細かい送りを捨てずに貯める)
    wheel: (f32, f32),
    /// 窓の大きさ(px)。描画のたびに実測 — **見える範囲**の計算に使う。
    /// セルの大きさは設定どおり固定で、窓に合わせて伸縮させない
    view_w_px: f32,
    view_h_px: f32,
    /// このセルで**編集を始めた**(F2・ダブルクリック・打ち始め)。
    /// 立っていない間の最初の打鍵は、既存の中身を消して置き換える
    /// (Excel の作法)。セルを移ると降りる(sync_input)
    edit_armed: bool,
    /// 名前ボックスの打ちかけ(数式バーの左端)。番地・範囲・名前で飛び、
    /// 知らない名前なら**いまの選択に付ける**(Excel の名前ボックスと同じ)
    name_edit: Option<Editor>,
    /// 「関数を挿入」の小窓(検索・分類・一覧・説明)
    fn_dlg: Option<FnDlg>,
    /// 「関数の引数」の画面(次へ、で進む第2段)
    fn_args: Option<FnArgs>,
    /// 式の直入力中のセル掴み(起点, 入れた参照の文字の範囲)。
    /// クリックで参照がカーソルに入り、ドラッグで範囲(A1:C9)に伸びる
    ref_pick: Option<(Pos, std::ops::Range<usize>)>,
    /// 終了確認のパネル(未保存の変更があるときに出る。窓の中の中央)
    quit_ask: bool,
    /// 右クリックのメニュー(出ている場所。格子領域の px)
    menu_at: Option<(f32, f32)>,
    /// 開いている子メニュー(挿入▸ など)
    menu_sub: Option<&'static str>,
    /// 「ドロップダウンリストから選択」などの一覧(候補, 出す場所)。
    ///
    /// 候補は**(鍵, 見出し)の組**。鍵は日本語のまま — `apply_pick` の照合も
    /// 色見本の引き当ても鍵で行う。見出しだけが画面の言語に訳される。
    /// 中身が値そのもの(書体名・ファイル名・シート名など)のときは
    /// [`plain`] で鍵と見出しを同じにする。
    pick: Option<(Vec<(String, String)>, (f32, f32))>,
    /// pick の中身の意味: "value"=セルに入れる / "font"=書体 / "size"=文字の大きさ
    pick_kind: &'static str,
    /// 絞り込みつきの一覧のときの検索欄。**Some の間はここへ打鍵が流れる**。
    /// 打つほど一覧が絞られる(書体・入力規則)。素の一覧では None
    pick_filter: Option<Editor>,
    /// 一覧の中で今どれを選んでいるか(↑↓で動く。**絞り込み後の並びの添字**)。
    /// 開くとき今の値の位置へ送る。Enter でここを確定する
    pick_sel: usize,
    /// 耳(シートのタブ)のメニューが指しているシート(右クリックで開く)。
    /// 改名・色の2段目のパネルが閉じるまで持ち越す
    sheet_menu_at: Option<usize>,
    /// 書式の小窓(セルをフォーマットする)。範囲を選び直しながら使える
    fmt_panel: Option<(f32, f32)>,
    /// 小さな入力のパネル(種類, 入力欄)。"name"=名前の定義。開いている間は打鍵がここへ
    prompt: Option<(&'static str, Editor)>,
    /// カスタムプロパティを足す途中(名前, 型)。名前 → 型 → 値 の3段を
    /// 小窓の連なりで訊く(小計・ゴールシークと同じ運び)。**値まで打って
    /// 初めて足す** — 途中でやめたら何も残らない
    prop_add: Option<(String, PropKind)>,
    /// パスワードを伏せずに出すか。**小窓を開くたびに false へ戻す** —
    /// 前に見せたまま次の人が開く、を起こさない(2026-08-13、台帳
    /// 「パスワード表示/非表示アイコン」)
    pw_show: bool,
    /// 数式を値の代わりに出す(数式の表示)
    show_formulas: bool,
    /// 画面の窓の左上(スクロール)。**表は画面より大きい**
    view: Pos,
    /// 固定する行数・列数(見出しを置き去りにしないため)。カーソル位置で決める
    frozen: Option<Pos>,
    /// 固定した枠に影を付ける(本家の viewtab:freezeshadow。見た目だけ)
    freeze_shadow: bool,
    /// オートフィルタ(Excel の▼)。**見え方だけ** — 保存される中身は
    /// 変わらず、閉じれば消える。範囲の1行目が見出しで、列ごとに
    /// 「隠す値」を持つ(map に無い列は素通し)
    auto_filter: Option<AutoFilter>,
    /// 開いている▼のパネル(列, 値の検索)。Esc で閉じる
    filter_panel: Option<(u32, Editor)>,
    /// 「データの入力規則」のパネル(本家の3タブのダイアログの形)
    dv_dlg: Option<DvDlg>,
    /// 画面の文字の大きさ(リボン・メニュー・状態行まで全部に掛かる倍率。
    /// 格子のズームとは別。設定に覚える — 次回も同じ大きさで開く)
    ui_scale: f32,
    /// 表の操作(書式・フィル・行列・結合・並べ替え)を戻すための控え。
    /// 入力欄の undo とは別 — **戻せない操作は事故のとき逃げ道が無い**。
    /// 1手 = シートの控えの束。普通の操作は1枚、Python の実行のように
    /// 複数シートに触るものは全部まとめて1手(どれでも1手で戻せる)。
    /// **どのシートの控えかを一緒に持つ** — シートを切り替えた後の undo が
    /// 別のシートへ他所の中身を書き戻す事故を防ぐ
    undo_stack: Vec<Vec<(usize, sheet::Sheet)>>,
    redo_stack: Vec<Vec<(usize, sheet::Sheet)>>,
    /// シートごとのカーソル・窓・固定(切り替えても場所を失わない)
    sheet_ui: Vec<(Pos, Pos, Option<Pos>)>,
    /// コピーの控え(起点, そのとき書いた TSV)。貼り付け時に系のクリップボードと
    /// 突き合わせ、一致すればアプリ内コピーとして式の参照をずらす
    clip: Option<(Pos, String)>,
    /// コピーの控え(セルそのもの)。形式を選択して貼り付け(値だけ・書式だけ)に使う
    clip_cells: Option<Vec<Vec<Option<Cell>>>>,
    /// コピーした範囲(シート, 左上, 右下)。破線の枠で見せる。Esc で消える
    clip_range: Option<(usize, Pos, Pos)>,
    /// グリッド線(表の薄い線)を出す
    gridlines: bool,
    /// 数式バーの中身。IMEもここに来る(セルの入力は1本のテキスト編集)
    input: Editor,
    path: Option<PathBuf>,
    status: SharedString,
    notes: Vec<SharedString>,
    dirty: bool,
    /// 選んでいるリボンのタブ
    tab: usize,
    /// ファイルの全面ページから「戻る」ときのタブ
    prev_tab: usize,
    /// ボタンに乗っているときの名前(下のステータスバーに出す)
    hover_hint: Option<&'static str>,
    /// ファイルのページの右側(0=詳細情報 1=最近開いた)
    file_view: u8,
    /// **フォルダから探す**(2026-08-17 発注者。SFIND の写真)。
    /// writer と同じ形。xlsx はセルの字を串刺しで探す
    fd_term: Editor,
    fd_glob: Editor,
    fd_dir: Option<PathBuf>,
    fd_field: usize,
    fd_hits: Vec<ui::search::FileHits>,
    fd_tally: ui::search::Tally,
    fd_at: Option<(usize, usize)>,
    fd_peek: String,
    /// 表示の倍率(表示タブのズーム。0.5〜2.0)
    zoom: f32,
    /// 数式バーを見せるか(表示タブ)
    show_formula_bar: bool,
    /// 行番号・列名の見出しを見せるか(表示タブ)
    show_headers: bool,
    /// **紙の切れ目を画面に見せる**(本家の改ページプレビューの破線)。
    /// 既定は消す — いつも出ていると帳票の罫線と紛れる
    pub(crate) show_breaks: bool,
    /// 自動復旧の控えを取る間隔(秒)。0 なら取らない。
    /// **原本は上書きしない** — 別の場所に控えるだけ(io::write_recover)
    pub(crate) recover_secs: u64,
    /// CSV に書き出すときの文字コードと区切り。**日本の会計ソフトは
    /// まだ CP932 のものがある** — UTF-8 固定だと渡せない
    pub(crate) csv_kind: &'static str,
    /// 最近使った記号(新しい順・最大12)。**次に同じ物を探させない**
    pub(crate) recent_symbols: Vec<String>,
    /// 最近使った書体(新しい順・最大12)。書体の一覧の頭に出す。
    /// recent_symbols と同じ器・同じ運び
    pub(crate) recent_fonts: Vec<String>,
    /// 最後に控えを取った時刻
    pub(crate) recover_at: std::time::Instant,
    /// 0 の値を見せるか(表示タブ。消しても値は 0 のまま)
    show_zeros: bool,
    /// 画面を暗くする(インターフェイステーマ)。**セルは白のまま** —
    /// 画面と紙の一致を守る(writer の「紙は白のまま」と同じ考え)
    dark: bool,
    /// 自動で再計算するか(数式タブの「計算方法」。手動のときは F9)
    auto_calc: bool,
    /// 見張り(ウォッチウィンドウ)。(シート番号, セル)
    watch: Vec<(usize, Pos)>,
    /// AI に頼み中(終わるまで次の頼みは断る)
    ai_busy: bool,
    /// **左パネル(会話)と右パネル(セルの設定)**(2026-08-15 発注者
    /// 「左右のパネルを整備して、AI も使えるように」)。
    /// リボンの表示タブの「左パネル」「右パネル」で開け閉めする
    pub(crate) left_open: bool,
    pub(crate) right_open: bool,
    /// 会話のやりとり(自分か, 字)。**画面だけの物** — ブックには載らない
    /// (データとプログラムを分ける、の決めのまま)
    pub(crate) chat_log: Vec<(bool, String)>,
    /// 会話の入力欄
    pub(crate) chat_in: Editor,
    /// AI が出した**変更案**(officework の Python)。人が「入れる」を
    /// 押すまで走らない — **押したのは人**、が残る形にするため
    pub(crate) chat_plan: Option<String>,
    /// 会話の入力欄に焦点があるか(打鍵をそちらへ回す)
    pub(crate) chat_focus: bool,
    /// 変更案の台本が裏で走っている間(押し重ねを止める)
    pub(crate) chat_busy: bool,
    /// 走らせて落ちたときの誤り(「直してもらう」に添えて送る)。
    /// 成功したら消える — 古い誤りを持ち回らない
    pub(crate) chat_err: Option<String>,
    /// 左パネルのいまの面(0=会話 1=コメント)。柱のアイコンで切り替える
    pub(crate) left_face: u8,
    /// 右パネルのいまの面(0=セルの設定 1=図形と画像)
    pub(crate) right_face: u8,
    /// 描画の道具(0=ペン 1=蛍光ペン 2=消しゴム)。writer と同じ形
    tool: Option<u8>,
    /// 描きかけの線(ドラッグ中)
    ink_cur: Option<Vec<(f32, f32)>>,
}

impl HasEditor for Calc {
    // 小さな入力のパネル(名前の定義など)・ソルバーの小窓が開いている間は、
    // 打鍵(IME含む)はそこへ
    fn editor(&mut self) -> &mut Editor {
        // ファイルの面の「フォルダから探す」の欄(2026-08-17)
        if self.tab == 0 && self.file_view == 3 {
            return if self.fd_field == 0 { &mut self.fd_term } else { &mut self.fd_glob };
        }
        // .py の編集面が開いている間は、打鍵は全部そこへ
        if let Some(p) = &mut self.py_edit {
            return &mut p.ed;
        }
        if let Some(ed) = &mut self.name_edit {
            return ed;
        }
        if let Some(a) = &mut self.fn_args {
            if !a.eds.is_empty() {
                let i = a.focus.min(a.eds.len() - 1);
                return &mut a.eds[i];
            }
        }
        if let Some(d) = &mut self.fn_dlg {
            return &mut d.search;
        }
        // 絞り込みつきの一覧(書体・入力規則)が開いている間は、打鍵はその検索欄へ
        if let Some(ed) = &mut self.pick_filter {
            return ed;
        }
        if let Some(sv) = &mut self.solver {
            return sv.focused();
        }
        if let Some((_, ed)) = &mut self.filter_panel {
            return ed; // ▼のパネルの検索欄
        }
        if let Some(d) = &mut self.dv_dlg {
            return d.focused();
        }
        // 会話の欄に焦点があるとき(欄を押した後)は打鍵をそこへ。
        // **旗が立っている間だけ** — 開いているだけでは表の打鍵を奪わない
        if self.chat_focus {
            return &mut self.chat_in;
        }
        match &mut self.prompt {
            Some((_, ed)) => ed,
            None => &mut self.input,
        }
    }
    fn editor_ref(&self) -> &Editor {
        if let Some(p) = &self.py_edit {
            return &p.ed;
        }
        if let Some(ed) = &self.name_edit {
            return ed;
        }
        if let Some(a) = &self.fn_args {
            if !a.eds.is_empty() {
                let i = a.focus.min(a.eds.len() - 1);
                return &a.eds[i];
            }
        }
        if let Some(d) = &self.fn_dlg {
            return &d.search;
        }
        if let Some(ed) = &self.pick_filter {
            return ed;
        }
        if let Some(sv) = &self.solver {
            return sv.focused_ref();
        }
        if let Some((_, ed)) = &self.filter_panel {
            return ed;
        }
        if let Some(d) = &self.dv_dlg {
            return d.focused_ref();
        }
        if self.chat_focus {
            return &self.chat_in;
        }
        match &self.prompt {
            Some((_, ed)) => ed,
            None => &self.input,
        }
    }
    /// 数学オートコレクト(`\alpha ` → `α `)。**掛けない所が3つある**:
    ///
    /// - `.py` の編集面 — `\alpha` は Python では別の意味を持つ綴りで、
    ///   置き換えたら台本が黙って壊れる
    /// - 式(`=` で始まる打ちかけ)— 式は**打った通りに残す**
    /// - 設定で切っているとき(既定は入。詳細設定で切れる)
    fn math_autocorrect(&self) -> bool {
        self.autocorrect
            && self.py_edit.is_none()
            && !self.editor_ref().text().starts_with('=')
    }
    fn on_autocorrect(&mut self, was: &str) {
        self.status =
            ui::tf!("{} を記号に替えました(Backspace で綴りに戻ります)", was).into();
    }
    fn on_edited(&mut self) {
        // 検索を打ち替えたら一覧の選択は先頭に戻す
        if let Some(d) = &mut self.fn_dlg {
            d.sel = 0;
        }
        // コンボの検索欄を打ち替えたら、絞り込み後の選択を先頭へ戻す
        if self.pick_filter.is_some() {
            self.pick_filter_edited();
        }
        // 引数を打ち替えたら結果の下見を計算し直す
        if self.fn_args.is_some() {
            self.fn_args_recalc();
        }
        // パネル・小窓・名前ボックス・コンボの検索欄への打鍵は文書を変えない
        if self.prompt.is_none() && self.name_edit.is_none()
            && self.fn_dlg.is_none() && self.fn_args.is_none()
            && self.filter_panel.is_none() && self.dv_dlg.is_none()
            && self.pick_filter.is_none()
        {
            self.dirty = true;
            // 式の直入力の支援: 打ちかけの関数名の補完一覧と、引数のヒント
            self.formula_assist();
        }
    }
}


impl Drop for Calc {
    fn drop(&mut self) {
        // 置きっぱなしのロックは他の人の警告になってしまう。最後の保険
        self.release_lock();
    }
}

/// ポイント編集でつまめる取っ手の種類。
#[derive(Clone, Copy, PartialEq)]
enum PtHandle {
    /// 頂点そのもの
    Vertex,
    /// 手前から入る曲線の制御点
    CtrlIn,
    /// 次へ出る曲線の制御点
    CtrlOut,
}

/// カスタムプロパティの型の選び(画面の言葉と `CustomVal` の間)。
/// **知らない型は選べない** — 読んだだけの `Other` はここに無い
#[derive(Clone, Copy, PartialEq)]
enum PropKind {
    Text,
    Number,
    Date,
    Bool,
}

impl PropKind {
    /// 画面の言葉から選ぶ。空欄と読めない綴りは文字に落とす
    fn parse(t: &str) -> Self {
        match t.trim() {
            "数" | "数値" => Self::Number,
            "日付" => Self::Date,
            "はい・いいえ" | "はい/いいえ" | "真偽" => Self::Bool,
            _ => Self::Text,
        }
    }

    /// 画面に出す言葉。**印は literal に付ける** — `ui::t!` は文言の門番が
    /// 走査できる形しか受けない(鍵=日本語の文そのもの)
    fn label(self) -> &'static str {
        match self {
            Self::Text => ui::t!("文字"),
            Self::Number => ui::t!("数"),
            Self::Date => ui::t!("日付"),
            Self::Bool => ui::t!("はい・いいえ"),
        }
    }
}

/// AI に頼む仕事(calc 流)。
///
/// **2026-08-15、9つの動詞を廃した**(発注者「いまの AI のボタンは全部
/// 要らない」「会話形式にして、書類を修正できるように」)。要約・翻訳・
/// 敬語・ふりがな…は、左パネルの会話に「〜して」と打てば通る — しかも
/// ボタンでは作れない頼み方(「表にして、列は日付と金額で」)もできる。
/// 残るのは会話だけ。
#[derive(Clone)]
enum CalcAi {
    /// **会話**。答えを書類に入れず、**左パネルに返す**。表を直す頼みなら
    /// officework の Python を書かせ、人が「入れる」を押してから走る
    Chat(String),
}

impl CalcAi {
    /// モデルへの言いつけ(system)と、何を渡すか
    fn prompt(&self) -> (&'static str, &'static str) {
        match self {
            // **会話**。表を直す頼みなら officework の Python を書かせる。
            // 直接いじらせない — 人が見て「入れる」を押して初めて走る。
            // Python にするのは、AI がいちばん正確に書ける形であり(xlwings と
            // openpyxl の形)、**人が読んで確かめられる**から
            CalcAi::Chat(_) => (
                "あなたは表計算を手伝う相談相手です。日本語で短く答えます。\n\
                 **表を直す頼み**(並べ替え・色・書式・行や列の出し入れ・\
                 計算の追加など)のときは、まず1〜2文で何をするかを言い、\
                 続けて ```python の囲みの中に officework の台本だけを書きます。\
                 台本の作法: 先頭は `from officework import calc as xw` と\
                 `wb = xw.Book.attach()` と `s = wb.sheets.active`。\
                 **attach です** — `xw.Book()` は新しいブックを作ってしまい、\
                 いま開いている表が消えます。範囲は `s[\"A1:C9\"]`。\
                 値は `.value`、書式は `.font.bold` `.number_format` `.fill` など。\
                 **保存はしない**(人が見て決める)。説明は囲みの外に書きます。\n\
                 表を直さない頼み(意味を訊く・式を1つ教える等)は、\
                 囲みを使わず本文だけで答えます。",
                "",
            ),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            CalcAi::Chat(_) => ui::t!("会話"),
        }
    }
}

/// **アプリを起動する。** `main.rs` はこれを呼ぶだけです
/// (2026-08-19 に切り出しました。1つのウィンドウに表と文章の両方を
/// 載せるには、バイナリではなくライブラリである必要があります)。
pub fn run() {
    let arg = std::env::args().nth(1).map(PathBuf::from);
    application().with_assets(ui::Icons).run(move |cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![std::borrow::Cow::Borrowed(font_data())])
            .expect("フォント登録");
        // 共通+calc の表と、settings.toml の key.* の上書き。読めなかった
        // 行の言い分は起動後に状態行へ(ui::key_warnings — 黙って捨てない)
        // 設定ファイルに書いた AI の宛先を環境変数へ移す(起動に一度)。
        // **環境変数が先** — その場の上書きは触らない
        ui::settings::ai_env_from_settings();
        cx.bind_keys(ui::bindings_for("calc", "jo_edit"));
        // **JO_KEYLOG=1 で打鍵と行き先を書き出す。** 「鍵が束縛に届いた」と
        // 「受け口が動いた」は別物で、前者だけ見て入れたつもりになると
        // キーの嘘になる(2026-08-10 に7つやった)。ここで見えるのは前者
        // まで — 効いたかどうかは tools/key_check.py で中身を見ること
        if std::env::var("JO_KEYLOG").is_ok() {
            std::mem::forget(cx.observe_keystrokes(|e, _, _| {
                eprintln!("KEY {} -> {:?}", e.keystroke, e.action.as_ref().map(|a| a.name()));
            }));
        }
        // 前に閉じたときの姿で開く。控えが無ければ既定の大きさで中央に
        let saved = ui::winstate::load("calc");
        let bounds = match saved {
            Some(st) => Bounds::new(gpui::point(px(st.x), px(st.y)), size(px(st.w), px(st.h))),
            None => Bounds::centered(None, size(px(1060.0), px(820.0)), cx),
        };
        let wb = if saved.is_some_and(|st| st.maximized) {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        };
        let arg2 = arg.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(wb),
                ..Default::default()
            },
            move |window, cx| {
                // plugins の関数の名前を先に登録簿へ — ブックを開く前に
                // 揃っていないと `=集計(A1)` が UDF だと分からない
                crate::py::refresh_udfs_if_changed();
                let view = cx.new(|cx| Calc::new(arg2.clone(), cx));
                // Python(officework)の受け口。この機械の中だけのユニックス
                // ソケット(Windows には無い — mod rpc の注記)
                #[cfg(unix)]
                crate::rpc::start(view.clone(), cx);
                // plugins の関数を呼んでいるセルを裏で計算し続ける見張り
                crate::py::start_udf_watch(view.clone(), cx);
                window.focus(&view.focus_handle(cx), cx);
                // 動かす・伸ばすたびに控える — 閉じる経路が何本あっても漏れない。
                // 全画面は控えない(次も全画面で開くと出口が分かりにくい)
                view.update(cx, |_, cx| {
                    cx.observe_window_bounds(window, |_, window, _| {
                        let wb = window.window_bounds();
                        if matches!(wb, WindowBounds::Fullscreen(_)) {
                            return;
                        }
                        let b = wb.get_bounds();
                        ui::winstate::save("calc", ui::winstate::WinState {
                            x: f32::from(b.origin.x),
                            y: f32::from(b.origin.y),
                            w: f32::from(b.size.width),
                            h: f32::from(b.size.height),
                            maximized: matches!(wb, WindowBounds::Maximized(_)),
                        });
                    })
                    .detach();
                });
                // WM からの「閉じる」(Alt+F4 等)も同じ確認を通す。
                // 書きかけがあれば「まだ閉じない」と答え、確認は別のスレッドで出す
                let v = view.clone();
                window.on_window_should_close(cx, move |_, cx| {
                    let quit_now = v.update(cx, |this, cx| {
                        this.commit();
                        if this.dirty && this.path.is_some() {
                            this.request_quit(cx);
                            false
                        } else {
                            this.release_lock();
                            true
                        }
                    });
                    if quit_now {
                        cx.quit();
                    }
                    quit_now
                });
                // **自動復旧の控え。** 30秒ごとに見て、変更があって
                // 間隔を過ぎていれば控える。原本は上書きしない
                {
                    let v = view.clone();
                    cx.spawn(async move |cx| {
                        loop {
                            // 見に行く間隔は控えの間隔より細かく(短い設定を
                            // 待たせない)。ただし毎秒は回さない
                            let poll = v
                                .update(cx, |c: &mut Calc, _| c.recover_secs.clamp(5, 30));
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(poll))
                                .await;
                            let due = v.update(cx, |c: &mut Calc, _| {
                                c.recover_secs > 0
                                    && c.dirty
                                    && c.recover_at.elapsed().as_secs() >= c.recover_secs
                            });
                            if due {
                                v.update(cx, |c: &mut Calc, cx| c.write_recover(cx));
                            }
                        }
                    })
                    .detach();
                }
                if std::env::var_os("JO_SELFTEST").is_some() {
                    // 画面が実際に動くかの自己診断: B列の幅を1秒ごとに広げ狭めし、
                    // 15秒で自動終了する。**操作は要らない** — 見ているだけで、
                    // 「モデルは動くのに画面が止まる」疑いを切り分けられる
                    let v = view.clone();
                    cx.spawn(async move |cx| {
                        for i in 0..15u32 {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(1000))
                                .await;
                            v.update(cx, |c, cx| {
                                let w = if i % 2 == 0 { 20.0 } else { 5.0 };
                                c.book.sheets[0].col_width.insert(1, w);
                                eprintln!("tick {}", i + 1);
                                c.status = ui::tf!("自己診断 {}/15: B列の幅 {}(勝手に動けば描画は健全)", i + 1, w)
                                .into();
                                cx.notify();
                            });
                        }
                        cx.update(|cx| cx.quit());
                    })
                    .detach();
                }
                view
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
