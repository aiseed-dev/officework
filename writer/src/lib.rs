//! writer — docx互換のワープロ。calc とは**別のソフト**。
//!
//! 一つの巨大なスイートにしない。文書は writer、表計算は calc。
//! 共有するのは書式(docx/xlsx)と核(kumihan)、そして入力の結線(ui)だけ。
//!
//! **マクロは無い。** 文書の中に実行コードを置かないので、
//! 「開く=実行」という攻撃経路が最初から存在しない。
//!
//!   writer            空で開く
//!   writer 文書.docx  その文書を開く
//!
//! 打てる: 日本語(IME)・BackSpace/Delete・矢印・Shift+矢印で選択・Ctrl+A・
//!         Enter で改段落・Ctrl+Z/Ctrl+Shift+Z・Ctrl+S 保存・Ctrl+O 開く

pub(crate) use std::ops::Range;
pub(crate) use std::path::PathBuf;

pub(crate) use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, SharedString, UTF16Selection, Window,
    WindowBounds, WindowOptions,
};
pub(crate) use gpui_platform::application;
pub(crate) use kumihan::{layout, Align, Document, Editor, Frame, ListKind, Metrics, Sheet as Page};
pub(crate) use ui::{handler, ribbon, HasEditor};

/// 画面の 1mm。**96dpi 固定**(機械の実 dpi は読まない — 発注者確定
/// 2026-08-14)。紙は物理で決まり PDF は 72dpi の pt で出るので、画面だけ
/// 機械ごとに動かすと印刷と合わせられない。calc の util::cell_font_px と
/// 同じ筋(pt→px は 96/72)。実寸で見たいときは拡大で合わせる
const PX_PER_MM: f32 = 96.0 / 25.4;
/// 見開きのページの間の空き(mm)
const PAGE_GAP_MM: f32 = 8.0;

/// `|語《よみ》` の記法をほどき、(素の文, [(本文の範囲, 読み)]) を返す。
/// 範囲は base からのバイト位置(差し込む先の頭)。
/// pywashi と同じ記法なので、あちらの資産とも行き来できる
fn strip_ruby_marks(src: &str, base: usize) -> (String, Vec<(std::ops::Range<usize>, String)>) {
    let mut plain = String::with_capacity(src.len());
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find('|').or_else(|| rest.find('｜')) {
        let mark_len = rest[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        let after = &rest[i + mark_len..];
        let Some(o) = after.find('《') else {
            plain.push_str(&rest[..i + mark_len]);
            rest = after;
            continue;
        };
        let Some(c) = after[o..].find('》') else {
            plain.push_str(&rest[..i + mark_len]);
            rest = after;
            continue;
        };
        plain.push_str(&rest[..i]);
        let word = &after[..o];
        let yomi = &after[o + '《'.len_utf8()..o + c];
        let start = base + plain.len();
        plain.push_str(word);
        out.push((start..base + plain.len(), yomi.to_string()));
        rest = &after[o + c + '》'.len_utf8()..];
    }
    plain.push_str(rest);
    (plain, out)
}

/// AI に頼む仕事。**返事をどう使うかまで決めてから頼む**
/// (使い道の決まっていない答えは受け取らない)
#[derive(Clone, Debug)]
enum AiJob {
    /// 選択にふりがな(ルビ)を振る。**会話では代われない** —
    /// 入るのが素の字ではなくルビの書式だから(2026-08-15 に AI タブを
    /// 廃したとき、この1つとマクロ台本だけが残った)
    Furigana,
    /// 自由に頼む(答えはカーソルの位置へ挿す)。マクロ台本の聞き取りが使う
    Ask(String),
    /// マクロ台本を書かせる(答えは文書に入れず、プラグイン置き場に
    /// .py で置く — 人が読んで確かめてから実行する。自動では走らせない)
    Macro(String),
    /// **会話**(左パネル)。答えは文書に入れず、パネルへ返す。
    /// 文を直す頼みなら、置き換える文を囲みに入れて見せ、
    /// **人が「入れる」を押すまで文書に触らない**
    Chat(String),
}

impl AiJob {
    /// モデルへの言いつけ(system)と、何を渡すか
    fn prompt(&self) -> (&'static str, &'static str) {
        match self {
            AiJob::Furigana => (
                "あなたは日本語のふりがなを付ける道具です。渡された文章のうち、                 読みが難しい漢字の語にだけ、|語《よみ》 の形でふりがなを付けて                 返します。文字そのものは1字も変えず、《》以外は足しません。                 やさしい語には付けません。本文だけを返します。",
                "次にふりがなを付けてください。",
            ),
            AiJob::Ask(_) => (
                "あなたは日本語の文書を扱う道具です。頼まれたことに対する答えの                 本文だけを返します。前置き・後書き・見出しは書きません。",
                "",
            ),
            // **会話**。writer には Python の橋が無い(calc だけ)ので、
            // 台本ではなく**置き換える文そのもの**を囲みで受け取る。
            // 人が「入れる」を押して初めて文書が変わる
            AiJob::Chat(_) => (
                "あなたは文書づくりを手伝う相談相手です。日本語で短く答えます。\n\
                 **文を直す頼み**(書き直し・敬語にする・短くする・訳す・\
                 続きを書く・箇条書きにする など)のときは、まず1〜2文で\
                 何をするかを言い、続けて ``` の囲みの中に\
                 **文書に入れる文だけ**を書きます。囲みの中に説明・見出し・\
                 引用符は入れません。\n\
                 文を直さない頼み(意味を訊く・書き方を相談する等)は、\
                 囲みを使わず本文だけで答えます。",
                "",
            ),
            // 台本の作法 = サンドボックスの中の python-docx。前置きの関数だけを使わせ、
            // ラベル走査と cell(i,j) ループ(実測 140 倍遅い)を禁じる
            AiJob::Macro(_) => (
                "あなたは writer(docx 互換ワープロ)のマクロ台本を書く道具です。\
                 Python のコードだけを返してください(説明・前置き・\
                 コードフェンスは書かない)。台本はサンドボックスの中で実行され、\
                 d = python-docx の Document が渡されています。\
                 import docx / Document() / d.save() は書きません。\
                 記入欄の読み書きは必ず次の関数を使います: \
                 fill(名前, 値)=名前の記入欄すべてに書く / \
                 fill_one(名前, 値)=最初の一つに書く / \
                 extract(名前)=値を読む / fields()=(名前, 値)の一覧 / \
                 render(辞書)={{名前}} 雛形への差し込み / \
                 tpl_fields()=差し込み口の一覧。\
                 ラベルの文字列を探して隣のセルに書く走査はしません(誤爆する)。\
                 表を歩くときは for row in tb.rows: for c in row.cells: の形にし、\
                 tb.cell(i, j) をループの中で呼びません(遅い)。\
                 ネットとファイルの読み書きはできません(サンドボックスの外に出ない)。\
                 最後に print で何をしたかを一行で報告し、できない・危うい頼みは \
                 raise SystemExit(\"理由\") で断ります。",
                "",
            ),
        }
    }

    /// ステータスに出す名前(見せる字だけ — 照合には使わない)
    fn label(&self) -> &'static str {
        match self {
            AiJob::Furigana => ui::t!("ふりがな"),
            AiJob::Ask(_) => ui::t!("頼み"),
            AiJob::Macro(_) => ui::t!("マクロ台本"),
            AiJob::Chat(_) => ui::t!("会話"),
        }
    }
}

/// 図表番号の頭(「図 」)。**貼る字と探す字を同じ雛形から取る**ための1箇所。
///
/// 番号を付けるときは `ui::tf!("図 {}", n)` で貼り、次の番号を決めるときと
/// 図表目次を作るときは段落の頭がこれで始まるかを見る。雛形は訳されるので
/// (独 "Abbildung {}"、韓 "그림 {}")、探す側に生の「図 」を書くと**日本語
/// 以外では一度も見つからず、図がすべて 1 番になり、図表目次も空になる**。
/// 同じ鍵 `"図 {}"` から穴の手前を切り出せば、二つが食い違う余地がない。
///
/// 穴が頭に来る訳(「{} 図」)が来たら頭は空になる — 空の頭は
/// `strip_prefix` が必ず通ってしまうので、そのときは日本語の形に戻す
pub(crate) fn caption_head() -> &'static str {
    let head = ui::t!("図 {}").split("{}").next().unwrap_or("");
    if head.is_empty() { "図 " } else { head }
}

/// gpui の文字は行の高さが既定で黄金比(1.618×文字サイズ)なので、
/// グリフは div の頭から余白の半分ぶん下に描かれる。自前で引く線
/// (変換の下線・下線・取り消し線・蛍光ペン)はそのぶん下げて
/// グリフの実位置に合わせる — 合わせないと下線が文字を横切る
const HALF_LEADING: f32 = 0.309; // (1.618 - 1) / 2
const SIZE_PT: f32 = 10.5;
const LINE_MM: f32 = 6.4;

/// いま編集しているもの。本文か、表のセルか。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    Body,
    Cell { table: usize, row: usize, col: usize },
}

/// ボタンの箱の控え(id → x, y, 幅, 高さ)。点検の道具が読みます。
type ボタンの箱 =
    std::rc::Rc<std::cell::RefCell<std::collections::HashMap<&'static str, (f32, f32, f32, f32)>>>;
/// 一覧の当たりの控え(段, 項目, x, y, 幅, 高さ)。
type 一覧の箱 = std::rc::Rc<std::cell::RefCell<Vec<(usize, usize, f32, f32, f32, f32)>>>;

pub struct Writer {
    focus: FocusHandle,
    doc: Document,
    /// **1つのファイルに入っている他の文書**(2026-08-19 発注者「同時に
    /// 送付する請求書の原稿をまとめて保存する」)。
    ///
    /// いま見ている物は `doc` にあり、ここの `doc_at` 番目は空の置き場に
    /// なっています。切り替えるときに入れ替えます — こうすると `self.doc`
    /// を見ている 166 箇所をそのまま残せます
    docs: Vec<Document>,
    /// いま見ている文書は何枚目か
    doc_at: usize,
    /// **このファイルを開いてほしい**(2026-08-19。統合の段1)。
    ///
    /// 一覧でファイルを押されたときにここへ入れます。`officework` が毎回見て、
    /// 名前で行き先を決めて開きます。*同じウィンドウで開く*ための受け渡しで、
    /// 別のアプリを起こすわけではありません。
    ///
    /// **`embedded` が立っているときは種類を問わず全部ここに入ります。**
    /// 前は「表だけ渡し、文書は自分で開く」と半分ずつ持っていましたが、
    /// 開く物の持ち主が officework になったので、頼み方を1本にしました。
    pub open_request: Option<PathBuf>,
    /// **「開く」の窓を出してほしい**(統合の段3)。
    ///
    /// 埋め込みのときは Ctrl+O を自分で捌かず、ここに立てます。
    /// どの編集画面で開くかは名前で決まるので、**選ぶ窓は officework が出す**の
    /// が筋です — 文章の画面から表を選べないと、使う人には理由が分かりません。
    pub open_dialog_request: bool,
    /// **`officework` の中に埋め込まれているか**(2026-08-19。統合の段1)。
    ///
    /// 立っていると、一覧のクリックは自分で開かず `open_request` に置きます。
    /// 単体で起動したときは寝ているので、今までどおり自分で開きます —
    /// **単体の writer の振る舞いは変わりません**。
    /// **検索の範囲**(2026-08-20 発注者「検索には3種類必要です」)。
    /// 偽=この文書だけ / 真=このファイル(中の全部の文書)。
    /// フォルダ全体は別の道(`find_in_folder`)です
    /// **数学オートコレクト**(`\alpha` → α)を掛けるか。
    /// 器は settings.toml の `math_autocorrect`(calc と同じ1つの綴り)
    autocorrect: bool,
    find_file: bool,
    embedded: bool,
    /// **開いている他のファイル**(2026-08-19)。いま見ている物は
    /// `Writer` の欄にあり、ここの `file_at` 番目は空の置き場です
    files: Vec<OpenFile>,
    /// いま見ているファイルは何枚目か
    file_at: usize,
    ed: Editor,
    page: Page,
    path: Option<PathBuf>,
    status: SharedString,
    notes: Vec<SharedString>,
    dirty: bool,
    /// マウスでドラッグ選択の途中か(押した位置から離すまで選択を伸ばす)
    drag_select: bool,
    /// 右クリックのメニュー(出ている場所。編集領域の px)
    menu_at: Option<(f32, f32)>,
    /// 選んでいるリボンのタブ
    tab: usize,
    /// 画面に使う書体名(文書の指定に従う)
    font_name: SharedString,
    /// 画面の倍率。**紙は変わらない** — 見る大きさだけの話
    zoom: f32,
    /// 縦のスクロール(紙の座標 mm)。0 が紙の頭
    scroll_mm: f32,
    /// カーソルを描くか。530 ミリ秒ごとに入れ替えて点滅させます
    /// (2026-08-17 発注者「カーソルは点滅すべきでは」)。
    /// 打っている間は消しません — 消えると打ち間違いに気づきにくくなります
    caret_on: bool,
    /// 編集領域の高さ(px)。描画のたびに実測し、キャレット追従に使う
    view_h_px: f32,
    /// いま編集しているもの。**Editor は常にこの対象の文章を持つ**
    target: Target,
    /// 編集記号(段落記号・空白)を見せるか
    show_marks: bool,
    /// ルーラー(mm の目盛り)を見せるか
    ruler: bool,
    /// 行番号を見せるか(見え方だけ。文書は変わらない)
    line_numbers: bool,
    /// コメントの印と一覧を見せるか(見え方だけ)
    show_comments: bool,
    /// **いま開いている一覧**(2026-08-22 に旗4つから1つにしました)。
    ///
    /// 中身は `一覧を描く` に渡す鍵そのもの — `"fontname"` / `"fontsize"` /
    /// `"parastyle"` / `"inssymbol"`。`None` なら何も開いていません。
    ///
    /// 前は `font_list` / `size_list` / `style_list` / `symbols` の4つの
    /// bool でした。**同時に2つは立たない**のに4つあったので、開くたびに
    /// 残り3つを倒す行が要り、53 か所に散っていました。1つにすると
    /// 「倒し忘れ」がそもそも書けません。
    open_list: Option<&'static str>,
    /// **一覧の中で選んでいる位置**(↑↓ の相手)。表の画面と同じ持ち方です
    pick_sel: usize,
    /// **書体の一覧の絞り込み。** 打つほど減ります。表の画面に前からある形で、
    /// これが無いと 24 件で切るしかありませんでした(25件目から選べない)
    font_filter: Option<Editor>,
    /// ダークモード(紙以外を暗く。文書は変わらない)
    dark: bool,
    /// **自動復旧の控えを取る間隔(秒)。** 0 なら取りません
    /// (2026-08-21 の B-3。表にしかありませんでした)
    pub(crate) recover_secs: u64,
    /// 最後に控えを取った時刻
    pub(crate) recover_at: std::time::Instant,
    /// **画面の文字の大きさ**(2026-08-21 発注者「双方でできるように
    /// したいです」)。リボン・タブの行・状態行・パネル・ファイルのページ
    /// が追従します。**紙は変わりません** — 紙の大きさは `zoom` の話で、
    /// こちらは画面の設えの話です。表の画面と同じ作りです
    pub(crate) ui_scale: f32,
    /// 画像の実体 → gpui の画像(作り直すと毎フレーム復号されるため控える)
    image_cache: std::collections::HashMap<usize, std::sync::Arc<gpui::Image>>,
    /// 組版に使うフォントの実体。**文書の書体に従う**(開くたびに引き直す)
    font_bytes: std::sync::Arc<Vec<u8>>,
    /// 用紙。**文書の設定に従う**(既定 A4・余白20mm)
    pg: kumihan::PageSetup,
    /// **リボンのボタンの場所**(id → 窓の中の x, y, 幅, 高さ)。描くたびに書く。
    ///
    /// 使い道は**実機の点検だけ**(tools/writer_shot.py)。calc には rpc の
    /// `{"cmd":"ribbon"}` があるが writer には受け口が無く、座標を目分量で
    /// 当てて何度も外した(2026-08-16。3回外し、外した拍子に発注者の打鍵まで
    /// 拾った)。**網は開けない** — 環境変数 `OFFICEWORK_UI_DUMP` が指す
    /// ファイルへ書き出すだけで、既定では何も起きない
    btn_box: ボタンの箱,
    /// 前に書き出した中身(同じなら書かない — 毎フレーム書くのは無駄)
    ui_dump_last: std::cell::RefCell<String>,
    /// **右パネルが実際に描いた面**(点検用。状態と食い違ったら分かる)
    rp_drawn: std::cell::Cell<u8>,
    /// **窓の論理の大きさ**(点検用)。物理との比が倍率 —
    /// 道具が目分量で当てると半分の位置を押す(2026-08-17 に踏んだ)
    win_wh: std::cell::Cell<(f32, f32)>,
    /// **フォルダから探す**(2026-08-17 発注者。SFIND の写真)。
    /// ファイルの面の3つ目の中身。探す字・場所・絞りと、当たりの一覧
    fd_term: Editor,
    fd_glob: Editor,
    fd_dir: Option<PathBuf>,
    /// 欄のどれを打っているか(0=字 1=絞り)
    fd_field: usize,
    fd_hits: Vec<ui::search::FileHits>,
    fd_tally: ui::search::Tally,
    /// 下に見せている当たり(ファイルの添字, 当たりの添字)
    fd_at: Option<(usize, usize)>,
    /// 下に見せる中身(当たりの前後)
    fd_peek: String,
    fd_busy: bool,
    /// **一覧の当たりの場所**(点検用。id は "fd-h-<ファイル>-<当たり>")。
    /// 箱の鍵は `&'static str` なので、控える数を上から数本に絞る
    fd_box: 一覧の箱,
    /// **ネイティブ文書(.adoc)を開いている**(2026-08-16)。
    /// 中身は意味だけで、見た目は [`Self::theme`] が持つ。false は互換
    /// (docx)— 直接書式が本文に入っている、今までの文書
    native: bool,
    /// いま効いているテンプレート。ネイティブでは紙面を組む前に
    /// `theme::compose` で流し込む(画面は常に「本文×テンプレート」)。
    /// 互換の文書では使わない。**配色の `theme` とは別物** — あちらは
    /// 画面の明暗で、こちらは文書の見た目の元
    tmpl: kumihan::theme::Theme,
    /// **いまのテンプレートを読んだ場所。** None は同梱の既定。
    ///
    /// 書式を直したときに、配られたテンプレートを書き替えたのか、この文書
    /// だけの写しを作ったのかを言い分けるために持ちます(発注者 2026-08-18
    /// 「テンプレートは指示する人が作る」)
    tmpl_path: Option<PathBuf>,
    /// 置換のパネル。開いている間、打鍵は検索欄に入る
    find_open: bool,
    /// 0=検索語 1=置換後
    find_field: usize,
    find_ed: Editor,
    repl_ed: Editor,
    /// ヘッダー・フッターの編集のパネル。Some(false)=ヘッダー / Some(true)=フッター。
    /// 開いている間、打鍵はここに入る(検索のパネルと同じ方式)
    hf_edit: Option<bool>,
    hf_ed: Editor,
    /// コメントのパネル(開いている間、打鍵はここに入る)と、付け先の段落番号
    /// **名乗りを打っている最中か**(詳細設定の「コメントの名乗り」)。
    /// 器は `settings.toml` の `user_name` で、表と同じ
    cmt_name_edit: bool,
    cmt_name_ed: Editor,
    cmt_edit: bool,
    cmt_ed: Editor,
    cmt_para: usize,
    /// 透かしのパネル
    wm_edit: bool,
    wm_ed: Editor,
    /// しおりのパネル(名前の入力欄つきの一覧)
    bm_open: bool,
    bm_ed: Editor,
    /// **スタイルの新設**(2026-08-16。ネイティブ文書だけ)。
    /// 見た目を直に変える操作を遮り、「この見た目に名前を付ける」へ
    /// 誘導する。中身は(掛けたい見た目, 名前の欄)。
    /// Word の失敗を設計で防ぐ要 — 直接書式より**楽な道が名前を付ける道**
    style_new: Option<kumihan::theme::StyleDef>,
    style_ed: Editor,
    /// バージョン履歴のパネル(上書き保存のたびに残る控えの一覧)
    hist_open: bool,
    /// プラグインのパネル(置き場の .py 一覧)
    plug_open: bool,
    /// リボンの絵ボタンに乗ったときの説明(下のステータスバーに出す)
    hover_hint: Option<&'static str>,
    /// 編集領域の幅(px。ページ幅に合わせるの計算に使う)
    view_w_px: f32,
    /// 左パネル(ナビゲーション)。0=見出し 1=コメント 2=検索
    nav_open: bool,
    nav_tab: u8,
    /// 右パネルのいまの面(0=いる場所の設定 1=ページ)。柱のアイコンで切り替える
    rp_tab: u8,
    /// 右パネル(いる場所の設定を直す盤)
    rp_open: bool,
    /// 表示の入切(本家の表示タブ)。リボンのボタン・ステータスバー・右のパネル
    show_toolbar: bool,
    show_statusbar: bool,
    /// ファイルのページ(タブ0)から戻る先のタブ
    prev_tab: usize,
    /// ファイルのページの右側(0=詳細情報 1=最近開いた)
    file_view: u8,
    /// 文書の情報で編集中の欄(0=作成者 1=タイトル 2=タグ 3=件名 4=コメント)
    file_field: Option<u8>,
    prop_ed: Editor,
    /// HTML の記入(form)。開いた HTML の欄と、送り先の起点
    html_forms: Vec<kumihan::html::Form>,
    html_links: Vec<(String, String)>,
    html_origin: Option<String>,
    /// 相対リンクを解く土台(開いた URL そのもの)
    html_base: Option<String>,
    lk_open: bool,
    fm_open: bool,
    fm_field: Option<usize>,
    fm_ed: Editor,
    /// URL を開くパネル
    url_open: bool,
    url_ed: Editor,
    /// いまの配色(レイアウト > 配色の変更)
    theme: usize,
    /// AI に自由に頼むパネル
    ai_open: bool,
    ai_ed: Editor,
    /// AI が働いている間は真(二重に頼まない)
    ai_busy: bool,
    // ── 左パネルの会話(2026-08-15。ナビの4つ目のタブ)────────────────
    // **co-chat(共同編集のチャット)とは別物。** あちらは人と人、
    // こちらは人と AI。名前を ai_chat_* にして取り違えを断つ
    /// やりとり(自分か, 字)
    pub(crate) ai_chat_log: Vec<(bool, String)>,
    /// 用件の欄
    pub(crate) ai_chat_in: Editor,
    /// 欄に焦点があるか。**旗が立っている間だけ**打鍵を奪う
    pub(crate) ai_chat_focus: bool,
    /// 置き換える文の案(囲みの中身)。**押すまで文書に触らない**
    pub(crate) ai_chat_plan: Option<String>,
    /// 複数ページ(見開き。画面だけの見え方 — 紙は1ページずつのまま)
    multipage: bool,
    /// **印刷モード。** 紙を1枚ずつ積んで見せる。
    ///
    /// 既定の編集モードは**切れ目の無い巻物**で、頁の間隔は紙の高さより
    /// 詰まっている(余白ぶん。実測で紙 297mm に対し 260mm)。だから紙の絵を
    /// 後ろに敷くだけでは重なる — 中身を折り直して初めてページが見える。
    /// 節で紙が変わる文書は、この形でないと出せない
    paged: bool,
    /// 印刷モードの、頁ごとの上端(折った後の mm)。紙の絵をここへ置く
    page_tops: Vec<f32>,
    /// 頁ごとの紙。**紙(PDF)と同じ物を使う** — 画面と印刷で食い違わない
    page_papers: Vec<paper::Paper>,
    /// **合成の写しから取ったページの飾り**(ヘッダー, フッター)。
    /// テンプレートが持つ物と、文書が持つ物(docx 由来)の合成の結果。
    /// `doc` は意味だけのままなので、飾りはここを見る(2026-08-18)
    dress_hf: (kumihan::HeadFoot, kumihan::HeadFoot),
    /// 同じく(透かし, ページの色)
    dress_page: (Option<String>, Option<String>),
    /// 記入欄の選択肢を聞くパネル(コンボ・ドロップダウンを挿すとき)
    sd_open: bool,
    sd_ed: Editor,
    sd_kind: kumihan::SdtKind,
    /// パネルが「選択肢」でなく「記入欄の名前」を聞いている
    sd_naming: bool,
    /// AI のパネルが「頼む」でなく「マクロ台本」を聞いている
    ai_macro: bool,
    /// 終了確認のパネル(未保存の変更があるときに出る。窓の中の中央)
    quit_ask: bool,
    /// ルビのパネル(選んだ字に読みを振る)
    rb_open: bool,
    rb_ed: Editor,
    rb_range: std::ops::Range<usize>,
    /// 数式のパネル(LaTeX を打つ)。**組むのは Python** — 自前で組版は
    /// 書かない(calc がグラフを matplotlib に任せるのと同じ分業)。
    /// 打った原文は絵と一緒に持ち越すので、開き直しても直せる
    eq_open: bool,
    eq_ed: Editor,
    /// 暗号化のパスワード。Some なら保存で ECMA-376 Standard に包む
    encrypt_pw: Option<String>,
    /// パスワードのパネル。pw_pending が Some なら「開くために聞いている」
    pw_open: bool,
    pw_ed: Editor,
    pw_pending: Option<PathBuf>,
    /// マクロで置き換える直前の文書(Ctrl+Z で1手で戻すため)
    /// **取り消しの控え。** 文書ごと控える(平文だけでは書式が戻らない)。
    ///
    /// 前は「平文の取り消し(`Editor`)」と「マクロ用に文書を1枚だけ控える」の
    /// 二本立てだった。**書式を変える操作はどちらにも乗っていなかった** —
    /// 太字も揃えも Ctrl+Z で戻らなかった(2026-08-13 に測って分かった)。
    /// 一本にまとめ、打鍵も書式もここへ積む
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    /// 直前の一手が打鍵だったか。**続けて打った分は1手にまとめる** —
    /// 1文字ごとに文書を控えると重いし、戻し方も細かすぎて使いにくい
    typing_run: bool,
    /// いま走っている命令が、もう控えを取ったか。
    /// **一手で2枚控えると、Ctrl+Z を2回押さないと戻らない** —
    /// 「空白ページ」は自分で控えたあと、中で打鍵と段落の変更を呼ぶので
    /// 3枚積まれていた(2026-08-13)
    acted: bool,
    /// チャット(文書の隣の申し送り帳)のパネルと入力欄
    chat_open: bool,
    chat_ed: Editor,
    /// 相互参照のパネル(しおり一覧から「文字」「ページ」を挿す)
    xr_open: bool,
    /// 描画の道具(0=ペン 1=蛍光ペン 2=消しゴム)。Some の間はマウスが筆
    tool: Option<u8>,
    /// 書きかけの筆
    ink_cur: Option<kumihan::Stroke>,
    /// 筆の取り消しの控え(1操作 = 1枚)
    ink_undo: Vec<Vec<kumihan::Stroke>>,
    /// 直前の adoc 保存で、筆を何枚の絵にしたか(状態行で言うため)
    ink_svg_count: usize,
    /// 様式(升目)で対応が付かなかった物。組むたびに入れ替わる
    form_notes: Vec<String>,
    /// ページの繰り上げ量(紙と同じ折り方)。筆のページ⇔巻物の変換に使う
    page_offsets: Vec<f32>,
    /// 各ページに**載る最初の行の y**(巻物の座標。1枚目は -∞)。
    ///
    /// **どの枚に属するかはこちらで決める。** `page_offsets` は「紙の上端」で、
    /// 最初の行より余白ぶん上にある — 巻物は空きを詰めて流れるので、その
    /// 上端は前の枚の終わりより手前に来ることがある。境として使うと前の枚の
    /// 末尾が次の枚に化ける(2026-08-17、発表の組み方で踏んだ)
    page_starts: Vec<f32>,
    /// ページごとに載る脚注(`self.page.notes` の添字)。**紙と同じ割り当て** —
    /// PDF と画面で脚注の出るページが食い違わないよう、同じ `paginate_full`
    /// から受け取る
    page_notes: Vec<Vec<usize>>,
    /// 変更履歴を記録中か。記録開始時点の段落の写しを持つ
    track: bool,
    track_base: Option<Vec<String>>,
    /// 自分が置いた排他ロック(.~lock.名前#)。閉じるときに外す
    my_lock: Option<PathBuf>,
    /// 先客の名乗り(user@host)。居る間は上書き保存をしない
    locked_by: Option<String>,
    /// 紙面に出すヘッダー・フッターの行(1ページ目の番号で組んだもの)
    header_lines: Vec<kumihan::Line>,
    footer_lines: Vec<kumihan::Line>,
    /// 校正の指摘(レビュー > 校正)。英語は辞書、日本語はモデル
    proof: Vec<ui::check::Finding>,
    proof_msg: SharedString,
    /// 辞書は起動時に1回だけ読む
    checker: ui::check::Checker,
}

impl Writer {
    /// **いま押せるか**(2026-08-21 の B-5「灰色をボタン単位に」)。
    /// 考え方は表の同じ関数の註のとおりです。
    pub(crate) fn 押せるか(&self, id: &str) -> bool {
        match id {
            // 目次・図表目次の「更新」は、**もう入れてあるときだけ**。
            // 入れていない文書で押しても、更新する物がありません
            "toc-update" => self
                .doc
                .paragraphs()
                .any(|p| matches!(p.style, kumihan::ParaStyle::Toc(_))),
            "tof-update" => self
                .doc
                .paragraphs()
                .any(|p| p.style == kumihan::ParaStyle::Tof),
            // コメントの削除は、いまの段落にコメントが付いているときだけ
            "co-delcomment" => self
                .doc
                .paragraphs()
                .nth(self.cursor_para().0)
                .is_some_and(|p| !p.comments.is_empty()),
            _ => true,
        }
    }

    /// **控えを取る頃合いか**(2026-08-21)。統合アプリが全部のタブを
    /// 見て回るので、判定はここに出します。
    ///
    /// *前は見張りが `run()` の中にありました* — 単体を起こしたときしか
    /// 動かず、**配っている officework では控えが1つも取れていません
    /// でした**(実機で確かめた)。
    pub fn recover_due(&self) -> bool {
        self.recover_secs > 0
            && self.dirty
            && self.recover_at.elapsed().as_secs() >= self.recover_secs
    }

    /// 見に行く間隔(控えの間隔より細かく。ただし毎秒は回さない)
    pub fn recover_poll(&self) -> u64 {
        self.recover_secs.clamp(5, 30)
    }

    /// 控えを取る(原本は上書きしません)
    pub fn take_recover(&mut self, cx: &mut Context<Self>) {
        self.write_recover(cx);
    }

    /// **書きかけがあるか**(`officework` が持ち替えの前に聞きます)。
    ///
    /// *開いている全部のタブを見ます。* 持ち替えは画面ごと作り直すので、
    /// 裏のタブの書きかけも一緒に消えます — いま見ている物だけを見ると、
    /// 「保存したのに消えた」が起きます
    pub fn has_unsaved(&self) -> bool {
        self.dirty || (0..self.files.len()).any(|i| self.file_dirty(i))
    }

    /// 状態行に出す(持ち替えを断った理由を言うため)。
    pub fn say(&mut self, msg: impl Into<gpui::SharedString>) {
        self.status = msg.into();
    }

    /// **`officework` の中に埋め込まれたと伝える**(統合の段1)。
    ///
    /// これを立てると、一覧のクリックは自分で開かず `open_request` に置きます。
    pub fn set_embedded(&mut self) {
        self.embedded = true;
    }

    /// **いま選んでいるリボンの段**(`officework` が画面をまたいで持ち越す)。
    /// **入切のボタンが、いま入っているか**(2026-08-21 発注者
    /// 「押せるボタンだけでなくトグルボタンを作って」)。表の画面と同じ形です。
    pub(crate) fn 入っているか(&self, id: &str) -> bool {
        match id {
            "show-toolbar" => self.show_toolbar,
            "show-statusbar" => self.show_statusbar,
            "nav" | "show-left" => self.nav_open,
            "show-right" => self.rp_open,
            "darkmode" => self.dark,
            _ => false,
        }
    }

    pub fn ribbon_tab(&self) -> usize {
        self.tab
    }

    /// リボンの段を選ぶ。**この画面に無い段は動かしません**。
    pub fn set_ribbon_tab(&mut self, i: usize) {
        if i < ribbon::writer_tabs().len() {
            self.tab = i;
        }
    }

    /// 画面が暗い側か(`officework` がタブの行の色を合わせるのに使う)。
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    /// **いま開いているファイルの道**(`officework` がタブを引き当てるのに使う)。
    /// まだ名前が無ければ `None` です。
    pub fn opened_path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    /// **この文書を開く**(`officework` が頼む口。統合の段1)。
    ///
    /// いまは writer の中のタブで開きます。段2 でタブの持ち主が officework に
    /// 移ったら、ここは「いま見ている物を差し替える」だけになります。
    pub fn open_path(&mut self, p: PathBuf) {
        self.open_in_tab(p);
    }

    /// **フォルダを開いた姿にする**(`officework` の起動。SEKKEI「A-1」)。
    ///
    /// ファイルは開きません。右のパネルにそのフォルダの中身を出すだけで、
    /// 何を開くかは使う人が選びます — *フォルダを開くとは、そういうこと*です
    /// (エディタと同じ形。SEKKEI「アプリはフォルダを開く形にする」)。
    pub fn show_folder(&mut self, dir: std::path::PathBuf) {
        ui::settings::set("folder", &dir.display().to_string());
        self.rp_open = true;
        self.rp_tab = 3; // フォルダの中身
        self.status = ui::tf!("{} を開きました(右の一覧から選んでください)",
                              dir.display().to_string())
            .into();
    }

    /// 打った分を取り消して、文書の字に戻す(保護と、編集できない塊で使う)
    pub(crate) fn undo_typing(&mut self) {
        self.ed.clear_marked();
        let want = match self.target {
            Target::Body => self.doc.body_text(),
            Target::Cell { table, row, col } => self
                .doc
                .tables()
                .nth(table)
                .and_then(|t| t.rows.get(row))
                .and_then(|r| r.get(col))
                .map(cell_text)
                .unwrap_or_default(),
        };
        while self.ed.text() != want {
            if !self.ed.undo() {
                self.ed = Editor::new(&want);
                break;
            }
        }
    }

    /// カーソルのある段落が「原文のまま持ち越した塊」か
    pub(crate) fn raw_para_at_cursor(&self) -> bool {
        let (pi, _) = self.cursor_para();
        self.doc.paragraphs().nth(pi).is_some_and(|p| p.raw_adoc.is_some())
    }

}

impl HasEditor for Writer {
    fn editor(&mut self) -> &mut Editor {
        // 置換・ヘッダーのパネルが開いている間、入力(IME含む)はそちらへ入る。
        // 別の入力部品を作らず、同じ Editor と結線を使い回す
        // 書体の一覧が開いている間、打鍵は絞り込みの欄へ流します
        if let Some(f) = self.font_filter.as_mut() {
            f
        } else if self.pw_open {
            &mut self.pw_ed
        } else if self.file_field.is_some() {
            &mut self.prop_ed
        } else if self.find_open {
            if self.find_field == 0 { &mut self.find_ed } else { &mut self.repl_ed }
        } else if self.hf_edit.is_some() {
            &mut self.hf_ed
        } else if self.cmt_name_edit {
            &mut self.cmt_name_ed
        } else if self.cmt_edit {
            &mut self.cmt_ed
        } else if self.wm_edit {
            &mut self.wm_ed
        } else if self.file_view == 3 && self.tab == 0 {
            if self.fd_field == 0 { &mut self.fd_term } else { &mut self.fd_glob }
        } else if self.style_new.is_some() {
            &mut self.style_ed
        } else if self.bm_open {
            &mut self.bm_ed
        } else if self.url_open {
            &mut self.url_ed
        } else if self.fm_field.is_some() {
            &mut self.fm_ed
        } else if self.rb_open {
            &mut self.rb_ed
        } else if self.eq_open {
            &mut self.eq_ed
        } else if self.sd_open {
            &mut self.sd_ed
        } else if self.ai_open {
            &mut self.ai_ed
        } else if self.chat_open {
            &mut self.chat_ed
        } else if self.ai_chat_focus {
            // 左パネルの会話の欄(押した後だけ)。**開いているだけでは
            // 本文の打鍵を奪わない**
            &mut self.ai_chat_in
        } else {
            &mut self.ed
        }
    }
    fn before_edit(&mut self, typing: bool) {
        self.checkpoint(typing);
    }
    /// **数学オートコレクト**(`\alpha` → α)。2026-08-20 発注者
    /// 「双方でできるようにしたいです」。
    ///
    /// 仕掛けは前から `ui::handler` の共通の物で、**calc だけが名乗り出て**
    /// いました。文章のほうが数式の綴りを打つ場面は多いくらいです。
    ///
    /// 掛けない場所は calc と同じ考え方で、**本文と表のセルだけ**に掛けます —
    /// 検索の欄や名前の欄で勝手に替わると、探せない・付けられないになります。
    fn math_autocorrect(&self) -> bool {
        self.autocorrect
            && !self.find_open
            && !self.pw_open
            && !self.bm_open
            && !self.url_open
            && self.file_field.is_none()
            && self.fm_field.is_none()
            // 数式の小窓は **TeX の綴りをそのまま渡す**(置き換えたら式が壊れる)
            && !self.eq_open
    }
    fn on_autocorrect(&mut self, was: &str) {
        self.status =
            ui::tf!("{} を記号に替えました(Backspace で綴りに戻ります)", was).into();
    }
    fn editor_ref(&self) -> &Editor {
        if let Some(f) = self.font_filter.as_ref() {
            f
        } else if self.pw_open {
            &self.pw_ed
        } else if self.file_field.is_some() {
            &self.prop_ed
        } else if self.find_open {
            if self.find_field == 0 { &self.find_ed } else { &self.repl_ed }
        } else if self.hf_edit.is_some() {
            &self.hf_ed
        } else if self.cmt_name_edit {
            &self.cmt_name_ed
        } else if self.cmt_edit {
            &self.cmt_ed
        } else if self.wm_edit {
            &self.wm_ed
        } else if self.file_view == 3 && self.tab == 0 {
            if self.fd_field == 0 { &self.fd_term } else { &self.fd_glob }
        } else if self.style_new.is_some() {
            &self.style_ed
        } else if self.bm_open {
            &self.bm_ed
        } else if self.url_open {
            &self.url_ed
        } else if self.fm_field.is_some() {
            &self.fm_ed
        } else if self.rb_open {
            &self.rb_ed
        } else if self.eq_open {
            &self.eq_ed
        } else if self.sd_open {
            &self.sd_ed
        } else if self.ai_open {
            &self.ai_ed
        } else if self.chat_open {
            &self.chat_ed
        } else if self.ai_chat_focus {
            &self.ai_chat_in
        } else {
            &self.ed
        }
    }
    fn on_edited(&mut self) {
        if self.pw_open || self.find_open {
            // パスワード・検索欄への打鍵は文書を変えない
            return;
        }
        if self.chat_open || self.file_field.is_some() || self.rb_open || self.eq_open
            || self.url_open || self.fm_field.is_some() || self.sd_open
            || self.ai_open {
            // チャット・文書の情報・ルビの入力欄。打鍵は(確定まで)文書を変えない
            return;
        }
        // **本家の AsciiDoc の塊は編集させません**(発注者 2026-08-18
        // 「セクションから下を Visual を編集できるというのではどうか」)。
        // 註記やコードの塊は、意味を分かっていないまま触ると壊れます。
        // 見せる・保存する・そのまま返すはしますが、打鍵は断ります
        if self.target == Target::Body && self.raw_para_at_cursor() {
            self.undo_typing();
            self.status = ui::t!(
                "ここは AsciiDoc の塊です。うちでは編集できません(原文のまま保存します)"
            )
            .into();
            return;
        }
        if self.protected() {
            // 読み取り専用の保護。**打った分を取り消して、文書は変えない。**
            // パネル(ヘッダー等)の打鍵は文書に入る前なので、パネルごと閉じて捨てる
            if self.hf_edit.is_some() || self.wm_edit || self.cmt_edit {
                self.hf_edit = None;
                self.wm_edit = false;
                self.cmt_edit = false;
            }
            if !self.bm_open {
                self.undo_typing();
            }
            self.status =
                ui::t!("読み取り専用で保護されています(保護タブの「保護」で解除できます)").into();
            return;
        }
        if let Some(footer) = self.hf_edit {
            // パネルの打鍵はその場で文書のヘッダー・フッターに反映する
            let text = self.hf_ed.text().to_string();
            let hf = if footer { &mut self.doc.footer } else { &mut self.doc.header };
            kumihan::set_paras_text(&mut hf.paragraphs, &text);
            self.dirty = true;
            self.refresh_hf();
            return;
        }
        if self.bm_open {
            // しおりのパネルは名前の入力欄。打鍵は文書を変えない
            return;
        }
        if self.wm_edit {
            // 透かしのパネル。空にすると外れる
            let text = self.wm_ed.text().to_string();
            self.doc.watermark = if text.is_empty() { None } else { Some(text) };
            self.dirty = true;
            return;
        }
        if self.cmt_edit {
            // コメントのパネル。空にすると外れる(1つ目のコメントを編集する)
            let text = self.cmt_ed.text().to_string();
            // **名乗りは設定から**(表と同じ器)。設定していなければ空 =
            // 名乗らない。前は `$USER` を黙って入れていて、配った docx に
            // OS の利用者名が乗っていた(2026-08-20)
            let author = ui::comment_author();
            let pi = self.cmt_para;
            let mut i = 0usize;
            for b in &mut self.doc.blocks {
                if let kumihan::Block::Para(p) = b {
                    if i == pi {
                        if text.is_empty() {
                            if !p.comments.is_empty() {
                                p.comments.remove(0);
                            }
                        } else if let Some(c) = p.comments.first_mut() {
                            c.text = text.clone();
                        } else {
                            p.comments.push(kumihan::Comment { author, text: text.clone() });
                        }
                        break;
                    }
                    i += 1;
                }
            }
            self.dirty = true;
            return;
        }
        self.dirty = true;
        self.relayout();
        self.follow_caret();
    }
}


/// **開いているファイル1つぶんの持ち物**(2026-08-19 発注者「Zed と同じ
/// ように複数ファイルを開くことができるようにして」)。
///
/// いま見ているファイルの分は `Writer` の欄にそのまま入っていて、ここの
/// `file_at` 番目は空の置き場です。切り替えるときに入れ替えます —
/// 文書のタブ(`docs`)と同じ作法で、`self.doc` を見ている所を書き替えずに
/// 済みます。
pub(crate) struct OpenFile {
    doc: Document,
    docs: Vec<Document>,
    doc_at: usize,
    ed: Editor,
    path: Option<PathBuf>,
    dirty: bool,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    scroll_mm: f32,
    native: bool,
    tmpl: kumihan::theme::Theme,
    tmpl_path: Option<PathBuf>,
    notes: Vec<SharedString>,
}

impl Default for OpenFile {
    fn default() -> Self {
        OpenFile {
            doc: Document::default(),
            docs: Vec::new(),
            doc_at: 0,
            ed: Editor::new(""),
            path: None,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            scroll_mm: 0.0,
            native: false,
            tmpl: kumihan::theme::default_theme(),
            tmpl_path: None,
            notes: Vec::new(),
        }
    }
}

/// 取り消しの控え1枚。**文書と平文とカーソルを揃えて持つ** —
/// 別々に戻すと、文書と画面の言うことが食い違う。
#[derive(Clone)]
pub(crate) struct Snapshot {
    doc: Document,
    text: String,
    cursor: usize,
    target: Target,
}

impl Writer {
    /// パネル(ヘッダー・置換など)を編集中か。
    /// **パネルの打鍵は文書を変えない**ので、そちらは今までどおり
    /// `Editor` 自身の取り消しに任せる
    pub(crate) fn in_panel(&self) -> bool {
        self.pw_open
            || self.file_field.is_some()
            || self.find_open
            || self.hf_edit.is_some()
            || self.cmt_edit
            || self.cmt_name_edit
            || self.open_list.is_some()
            || self.wm_edit
            || self.bm_open
            || self.url_open
            || self.fm_field.is_some()
            || self.rb_open
            || self.eq_open
            || self.sd_open
            || self.ai_open
            || self.chat_open
    }

    /// **文書を変える前に**、いまの姿を控える。
    ///
    /// `typing` が真なら打鍵の一手。直前も打鍵なら控えない(まとめる)。
    /// 控えたら redo は捨てる — 枝分かれした先へは戻れない
    pub(crate) fn checkpoint(&mut self, typing: bool) {
        if self.in_panel() {
            return;
        }
        if typing && self.typing_run {
            return;
        }
        if self.acted {
            return; // この一手ではもう控えた
        }
        self.undo_stack.push(Snapshot {
            doc: self.doc.clone(),
            text: self.ed.text().to_string(),
            cursor: self.ed.cursor(),
            target: self.target,
        });
        // 深すぎる控えは持たない(文書を丸ごと持つので)
        const KEEP: usize = 100;
        if self.undo_stack.len() > KEEP {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
        self.typing_run = typing;
        self.acted = true;
    }

    /// 控えを1枚戻す(または進める)
    pub(crate) fn restore(&mut self, s: Snapshot) -> Snapshot {
        let now = Snapshot {
            doc: self.doc.clone(),
            text: self.ed.text().to_string(),
            cursor: self.ed.cursor(),
            target: self.target,
        };
        self.doc = s.doc;
        self.ed = Editor::new(&s.text);
        let len = self.ed.text().len();
        self.ed.move_to(s.cursor.min(len), false);
        self.target = s.target;
        self.typing_run = false;
        self.pg = self.doc.page.unwrap_or(self.pg);
        self.relayout_keep();
        self.dirty = true;
        now
    }
}

impl Focusable for Writer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EntityInputHandler for Writer {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _w: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        handler::text_for_range(self, r, actual)
    }
    fn selected_text_range(
        &mut self,
        _ignore: bool,
        _w: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: handler::selected_range_utf16(self), reversed: false })
    }
    fn marked_text_range(&self, _w: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
        handler::marked_range_utf16(self)
    }
    fn unmark_text(&mut self, _w: &mut Window, _cx: &mut Context<Self>) {
        handler::unmark(self);
    }
    fn replace_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        _w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        handler::replace(self, r, text);
        // 打っている間は消さない。消えると打ち間違いに気づきにくくなります
        self.caret_on = true;
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        sel: Option<Range<usize>>,
        _w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        handler::replace_and_mark(self, r, text, sel);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _r: Range<usize>,
        bounds: Bounds<gpui::Pixels>,
        _w: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<gpui::Pixels>> {
        // IME の候補窓をキャレットの下に出す(スクロールと倍率を織り込む)
        let pxmm = PX_PER_MM * self.zoom;
        let (x, y, pt) = self.caret_xy();
        Some(Bounds::new(
            gpui::point(
                bounds.origin.x + px(28.0 + x * pxmm),
                bounds.origin.y + px(14.0 + (y - self.scroll_mm) * pxmm),
            ),
            size(px(2.0), px(pt * 96.0 / 72.0 * self.zoom)),
        ))
    }
    fn character_index_for_point(
        &mut self,
        _p: gpui::Point<gpui::Pixels>,
        _w: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
    fn text_length_utf16(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> Option<usize> {
        Some(handler::text_len_utf16(self))
    }
}

mod cmds;
mod io;
pub(crate) use io::*;
mod py;
pub(crate) use py::*;
mod util;
pub(crate) use util::*;
mod filepage;
mod view;
mod panels;
pub(crate) use panels::Panels;
mod doc;
mod keys;
// RPC はユニックスソケットが設計(この機械の中だけ・ネイティブファースト)。
// Windows ではこの受け口ごと開かない — calc の mod rpc と同じ線
#[cfg(unix)]
/// 受け口(JSON 1行)。`officework` からも捌き手を呼びます
pub mod rpc;
mod text;

#[cfg(test)]
mod tests;

/// **アプリを起動する。** `main.rs` はこれを呼ぶだけです
/// (2026-08-19 に切り出しました。1つのウィンドウに表と文章の両方を
/// 載せるには、バイナリではなくライブラリである必要があります)。
/// **置きっぱなしの錠は他の人の警告になります。** 最後の保険(表と同じ形)。
///
/// 文章は「編集権」のボタンを押したときだけ錠を取ります。前はそれを
/// 外す道が閉じるときに無く、**押して閉じると錠が残っていました**
/// (2026-08-21。表には前から `Drop` があり、文章だけ抜けていました)。
impl Drop for Writer {
    fn drop(&mut self) {
        self.release_lock();
    }
}

pub fn run() {
    let arg = std::env::args().nth(1).map(PathBuf::from);
    application().with_assets(ui::Icons).run(move |cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![std::borrow::Cow::Borrowed(font_data())])
            .expect("フォント登録");
        // 共通+writer の表と、settings.toml の key.* の上書き(calc と同じ形)
        // 設定ファイルに書いた AI の宛先を環境変数へ移す(起動に一度)。
        // **環境変数が先** — その場の上書きは触らない
        ui::settings::ai_env_from_settings();
        cx.bind_keys(ui::bindings_for("writer", "jo_doc"));
        // 前に閉じたときの姿で開く。控えが無ければ既定の大きさで中央に
        let saved = ui::winstate::load("writer");
        let bounds = match saved {
            Some(st) => Bounds::new(gpui::point(px(st.x), px(st.y)), size(px(st.w), px(st.h))),
            None => Bounds::centered(None, size(px(900.0), px(1000.0)), cx),
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
                let view = cx.new(|cx| Writer::new(arg2.clone(), cx));
                window.focus(&view.focus_handle(cx), cx);
                // **受け口を開く**(2026-08-19)。calc と同じ形で、Python や
                // AI の道具から文書を操れます(Windows ではソケットを作らない)
                #[cfg(unix)]
                rpc::start(view.clone(), cx);
                // **自動復旧の控えを取る見張り**(2026-08-21 の B-3)。
                // 表と同じ形です。原本は上書きしません
                {
                    let v = view.clone();
                    cx.spawn(async move |cx| {
                        loop {
                            // 見に行く間隔は控えの間隔より細かく(短い設定を
                            // 待たせない)。ただし毎秒は回さない
                            let poll =
                                v.update(cx, |w: &mut Writer, _| w.recover_secs.clamp(5, 30));
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(poll))
                                .await;
                            let due = v.update(cx, |w: &mut Writer, _| {
                                w.recover_secs > 0
                                    && w.dirty
                                    && w.recover_at.elapsed().as_secs() >= w.recover_secs
                            });
                            if due {
                                v.update(cx, |w: &mut Writer, cx| w.write_recover(cx));
                            }
                        }
                    })
                    .detach();
                }
                // 動かす・伸ばすたびに控える — 閉じる経路が何本あっても漏れない。
                // 全画面は控えない(次も全画面で開くと出口が分かりにくい)
                view.update(cx, |_, cx| {
                    cx.observe_window_bounds(window, |_, window, _| {
                        let wb = window.window_bounds();
                        if matches!(wb, WindowBounds::Fullscreen(_)) {
                            return;
                        }
                        let b = wb.get_bounds();
                        ui::winstate::save("writer", ui::winstate::WinState {
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
                view
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
