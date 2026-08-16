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

struct Writer {
    focus: FocusHandle,
    doc: Document,
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
    /// 編集領域の高さ(px)。描画のたびに実測し、キャレット追従に使う
    view_h_px: f32,
    /// いま編集しているもの。**Editor は常にこの対象の文章を持つ**
    target: Target,
    /// 記号の一覧を出しているか
    symbols: bool,
    /// 編集記号(段落記号・空白)を見せるか
    show_marks: bool,
    /// ルーラー(mm の目盛り)を見せるか
    ruler: bool,
    /// 行番号を見せるか(見え方だけ。文書は変わらない)
    line_numbers: bool,
    /// コメントの印と一覧を見せるか(見え方だけ)
    show_comments: bool,
    /// フォントの一覧を出しているか
    font_list: bool,
    /// 大きさの一覧を出しているか
    size_list: bool,
    /// 段落のスタイルの一覧を出しているか
    style_list: bool,
    /// ダークモード(紙以外を暗く。文書は変わらない)
    dark: bool,
    /// 画像の実体 → gpui の画像(作り直すと毎フレーム復号されるため控える)
    image_cache: std::collections::HashMap<usize, std::sync::Arc<gpui::Image>>,
    /// 組版に使うフォントの実体。**文書の書体に従う**(開くたびに引き直す)
    font_bytes: std::sync::Arc<Vec<u8>>,
    /// 用紙。**文書の設定に従う**(既定 A4・余白20mm)
    pg: kumihan::PageSetup,
    /// **ネイティブ文書(.adoc)を開いている**(2026-08-16)。
    /// 中身は意味だけで、見た目は [`Self::theme`] が持つ。false は互換
    /// (docx)— 直接書式が本文に入っている、今までの文書
    native: bool,
    /// いま効いているテンプレート。ネイティブでは紙面を組む前に
    /// `theme::compose` で流し込む(画面は常に「本文×テンプレート」)。
    /// 互換の文書では使わない。**配色の `theme` とは別物** — あちらは
    /// 画面の明暗で、こちらは文書の見た目の正本
    tmpl: kumihan::theme::Theme,
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
    cmt_edit: bool,
    cmt_ed: Editor,
    cmt_para: usize,
    /// 透かしのパネル
    wm_edit: bool,
    wm_ed: Editor,
    /// しおりのパネル(名前の入力欄つきの一覧)
    bm_open: bool,
    bm_ed: Editor,
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
    /// 表示の入切(本家の表示タブ)。ボタンの帯・ステータスバー・右のパネル
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
    // ── 左パネルの会話(2026-08-15。ナビの4つ目の耳)────────────────
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
    /// ページの繰り上げ量(紙と同じ折り方)。筆のページ⇔巻物の変換に使う
    page_offsets: Vec<f32>,
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

impl HasEditor for Writer {
    fn editor(&mut self) -> &mut Editor {
        // 置換・ヘッダーのパネルが開いている間、入力(IME含む)はそちらへ入る。
        // 別の入力部品を作らず、同じ Editor と結線を使い回す
        if self.pw_open {
            &mut self.pw_ed
        } else if self.file_field.is_some() {
            &mut self.prop_ed
        } else if self.find_open {
            if self.find_field == 0 { &mut self.find_ed } else { &mut self.repl_ed }
        } else if self.hf_edit.is_some() {
            &mut self.hf_ed
        } else if self.cmt_edit {
            &mut self.cmt_ed
        } else if self.wm_edit {
            &mut self.wm_ed
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
    fn editor_ref(&self) -> &Editor {
        if self.pw_open {
            &self.pw_ed
        } else if self.file_field.is_some() {
            &self.prop_ed
        } else if self.find_open {
            if self.find_field == 0 { &self.find_ed } else { &self.repl_ed }
        } else if self.hf_edit.is_some() {
            &self.hf_ed
        } else if self.cmt_edit {
            &self.cmt_ed
        } else if self.wm_edit {
            &self.wm_ed
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
        if self.protected() {
            // 読み取り専用の保護。**打った分を取り消して、文書は変えない。**
            // パネル(ヘッダー等)の打鍵は文書に入る前なので、パネルごと閉じて捨てる
            if self.hf_edit.is_some() || self.wm_edit || self.cmt_edit {
                self.hf_edit = None;
                self.wm_edit = false;
                self.cmt_edit = false;
            }
            if !self.bm_open {
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
            let author = std::env::var("USER").unwrap_or_else(|_| ui::t!("私").into());
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
mod view;
mod panels;
pub(crate) use panels::Panels;
mod doc;
mod keys;
mod text;

#[cfg(test)]
mod tests;

fn main() {
    let arg = std::env::args().nth(1).map(PathBuf::from);
    application().with_assets(ui::Icons).run(move |cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![std::borrow::Cow::Borrowed(font_data())])
            .expect("フォント登録");
        // 共通+writer の表と、settings.toml の key.* の上書き(calc と同じ形)
        // 設定ファイルに書いた AI の宛先を環境変数へ移す(起動に一度)。
        // **環境変数が先** — その場の上書きは触らない
        ui::settings::ai_env_from_settings();
        cx.bind_keys(ui::bindings_for("writer", "jo_edit"));
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
