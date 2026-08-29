//! リボン(タブ+コマンド)。**Euro-Office の現物から生成している。**
//!
//! このファイルは手で書かない。`gen_ribbon.py` が
//! `vendor/web-apps/apps/*/main/app/template/Toolbar.template` の並び順と
//! 同 app の `locale/ja.json` の名前から起こす。
//! だから「Euro-Office と全く同じか」は台本を回し直せば確かめられる。
//!
//! ```text
//! python3 ui/gen_ribbon.py ja > ui/src/ribbon.rs
//! ```
//!
//! **全部入れる**(2026-08-04 発注者確定で改訂。以前は共同編集・保護・
//! プラグイン・AI・マクロを「入れない」としていた)。乗り換える人の
//! 目当てを消さないため、タブもボタンも本家どおり並べる。
//! **VBA 型のマクロを持たないことだけは不変** — マクロのボタンの実体は
//! サンドボックス(bubblewrap)の中の Python で、文書の中に実行コードは置かない。
//!
//! **できないものを、できるように見せない。** 実装済みのコマンドだけを押せる形にし、
//! 未実装は灰色で残す。並びを Euro-Office に合わせたまま、
//! 「今どこまで出来ているか」がそのまま画面に出る。

/// ボタンの性格(2026-08-21 発注者「押せるボタンだけでなくトグルボタンを
/// 作って」)。**押した後どうなるかが違う**ので、描き方も変わります。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// 押すと1回きりの働きをします(既定)。押した後は元の見た目に戻ります
    Push,
    /// **入っているか切れているか**があります(数式バー・0を表示する・
    /// 左パネル)。入っている間は押された形で出すので、*見れば分かります*
    Toggle,
    /// **いくつかのうち1つだけが入ります**(標準 / 改ページ プレビュー)。
    ///
    /// 入切とは性格が違います(2026-08-21 発注者「改ページ プレビューは、
    /// 性格がちがうのでは」)。入切は互いに関わりませんが、こちらは
    /// *どれか1つが必ず入っていて、別のを押すと前のが切れます*。
    Mode,
}

/// 1つのコマンド。`ready=false` は未実装(押せない灰色)。
/// `icon` は Euro-Office の slot 名で、埋め込んだアイコン(icons.rs)を引く鍵。
#[derive(Clone, Copy)]
pub struct Cmd {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub ready: bool,
    pub kind: Kind,
}

/// 押すボタン(押せる)
pub(crate) const fn c(id: &'static str, label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id, label, icon, ready: true, kind: Kind::Push }
}
/// 入切のボタン(押せる)。画面は今の状態を押された形で見せます
pub(crate) const fn t(id: &'static str, label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id, label, icon, ready: true, kind: Kind::Toggle }
}
/// 押すボタン(まだ押せない灰色)
#[allow(dead_code)] // 灰色ゼロの今は未使用だが、ロケール表の生成が使う形
pub(crate) const fn x(label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id: "", label, icon, ready: false, kind: Kind::Push }
}
/// 入切のボタン(まだ押せない灰色)
#[allow(dead_code)]
pub(crate) const fn xt(label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id: "", label, icon, ready: false, kind: Kind::Toggle }
}
/// 表示の切り替え(まだ押せない灰色)
#[allow(dead_code)]
pub(crate) const fn xm(label: &'static str, icon: &'static str) -> Cmd {
    Cmd { id: "", label, icon, ready: false, kind: Kind::Mode }
}

/// いまの言語のリボン。**語だけが違う** — id・並び・ready・icon は
/// どの言語でも ja(WRITER/CALC)と同一(下の試験が保証)。
/// 内部の論理(タブ名の照合など)は ja の表で書いてよい —
/// 添字がそのまま対応する
pub fn writer_tabs() -> &'static [Tab] {
    crate::ribbon_tables::tabs(crate::settings::language())
        .map(|(w, _)| w)
        .unwrap_or(WRITER)
}

pub fn calc_tabs() -> &'static [Tab] {
    crate::ribbon_tables::tabs(crate::settings::language())
        .map(|(_, c)| c)
        .unwrap_or(CALC)
}

pub struct Tab {
    pub name: &'static str,
    pub cmds: &'static [Cmd],
}

// ---- 利用者が足したボタン -------------------------------------------------
//
// **静的な表(CALC/WRITER)には入れない。** 14言語をボタン単位で突き合わせる
// 門番(tools/ribbon_locale_check.py)が「言語ごとに数が違う」と言い出す。
// 利用者の札は利用者自身の言葉なので、そもそも訳さない — 対訳の表にも
// 入れない(2026-08-16 発注者「システム定義とユーザー定義に分ける」)。

/// 利用者のボタンの id の頭。押されたら `~/.config/officework/ribbon/<名前>.py`
/// を走らせる、という約束
pub const USER_PREFIX: &str = "py:";

/// 名乗りに絵が無い(または知らない絵の名前だった)ときの既定
const USER_ICON: &str = "py-run";

/// 利用者のボタン1つ — ボタンと、出る段(ja の段名)
pub struct UserBtn {
    pub cmd: Cmd,
    pub tab: &'static str,
}

type Shape = Vec<(String, u64, std::time::SystemTime)>;
static USER: std::sync::RwLock<Option<(Shape, &'static [UserBtn])>> =
    std::sync::RwLock::new(None);

/// 利用者が `~/.config/officework/ribbon` に置いたマクロのボタン。
///
/// **描くたびに置き場を読まない。** 画面は1秒に何十回も組み直されるので、
/// 走査は [`refresh_user_cmds`] が姿の変わったときだけ行う(UDF の見張りと
/// 同じ形)。ここは控えを返すだけ。
pub fn user_btns() -> &'static [UserBtn] {
    if let Ok(g) = USER.read() {
        if let Some((_, c)) = g.as_ref() {
            return c;
        }
    }
    refresh_user_cmds();
    USER.read().ok().and_then(|g| g.as_ref().map(|(_, c)| *c)).unwrap_or(&[])
}

/// その段に出る利用者のボタン。段は**ja の段名**で照合する — 表の内部の
/// 照合が ja なのと同じ(添字も名前も言語で動かない)
pub fn user_cmds_for(tab_ja: &str) -> Vec<&'static Cmd> {
    user_btns().iter().filter(|b| b.tab == tab_ja).map(|b| &b.cmd).collect()
}

/// 置き場の姿が変わっていればボタンを作り直す。返りは作り直したか。
///
/// 作った札と id は `&'static` として漏らす(`Box::leak`)。静的な表と同じ型で
/// 扱えるようにするため — 漏れるのは**置き場を書き換えた回数**だけで、
/// 1回あたり数十バイト。描くたびに漏れる作りではない。
pub fn refresh_user_cmds() -> bool {
    let dir = pyrun::ribbon_dir();
    let now = pyrun::shape_in(&dir);
    let Ok(mut g) = USER.write() else { return false };
    if g.as_ref().map(|(s, _)| s) == Some(&now) {
        return false;
    }
    let btns: Vec<UserBtn> = pyrun::ribbon_decls(&dir)
        .into_iter()
        .map(|d| {
            let icon =
                if crate::icons::find(&d.icon).is_some() { d.icon } else { USER_ICON.into() };
            UserBtn {
                cmd: Cmd {
                    id: Box::leak(format!("{USER_PREFIX}{}", d.module).into_boxed_str()),
                    label: Box::leak(d.label.into_boxed_str()),
                    icon: Box::leak(icon.into_boxed_str()),
                    ready: true,
                    // 利用者のマクロは押すボタン。入切にしたければ .py の側で
                    // 状態を持つことになるので、いまは押す形だけ
                    kind: Kind::Push,
                },
                tab: Box::leak(d.tab.into_boxed_str()),
            }
        })
        .collect();
    *g = Some((now, Box::leak(btns.into_boxed_slice())));
    true
}

pub const WRITER: &[Tab] = &[
    Tab { name: "File", cmds: &[
        c("open", "Open", "open"),
        c("save", "Save", "save"),
        c("pdf", "Print", "print"),
    ]},
    Tab { name: "Home", cmds: &[
        c("copy", "Copy", "copy"),
        c("cut", "Cut", "cut"),
        c("paste", "Paste", "paste"),
        c("fontname", "Font", "fontname"),
        c("fontsize", "Font size", "fontsize"),
        c("incfont", "Increment font size", "incfont"),
        c("decfont", "Decrement font size", "decfont"),
        c("changecase", "Change case", "changecase"),
        c("ruby", "Ruby", "ruby"),
        c("ai-furigana", "Furigana", "ai-furigana"),
        c("bold", "Bold", "bold"),
        c("italic", "Italic", "italic"),
        c("underline", "Underline", "underline"),
        c("strikeout", "Strikethrough", "strikeout"),
        c("superscript", "Superscript", "superscript"),
        c("subscript", "Subscript", "subscript"),
        c("highlight", "Highlight colour", "highlight"),
        c("fontcolor", "Font colour", "fontcolor"),
        c("clearstyle", "Clear style", "clearstyle"),
        c("markers", "Bullet", "markers"),
        c("numbering", "Numbering", "numbering"),
        c("multilevels", "Multilevel list", "multilevels"),
        c("decoffset", "Decrease indent", "decoffset"),
        c("incoffset", "Increase indent", "incoffset"),
        c("linespace", "Paragraph line spacing", "linespace"),
        c("direction", "Text direction", "direction"),
        c("align-left", "Align left", "align-left"),
        c("align-center", "Centre", "align-center"),
        c("align-right", "Align right", "align-right"),
        c("align-just", "Justified", "align-just"),
        c("align-dist", "Distributed", "align-dist"),
        c("hidenchars", "Nonprinting characters", "hidenchars"),
        c("paracolor", "Shading", "paracolor"),
        c("borders", "Borders", "borders"),
        c("parastyle", "Paragraph style", "styles"),
        c("replace", "Replace", "replace"),
        c("selectall", "Select all", "select-all"),
    ]},
    Tab { name: "Insert", cmds: &[
        c("blankpage", "Insert blank page", "blankpage"),
        c("pagebreak", "Breaks", "pagebreak"),
        c("instable", "Insert table", "instable"),
        c("insimage", "Insert image", "insertimage"),
        c("insshape", "Insert shape", "insshape"),
        c("inssmartart", "Insert SmartArt", "inssmartart"),
        c("inschart", "Insert chart", "inschart"),
        c("instext", "Insert text box", "instext"),
        c("instextart", "Insert Text Art", "instextart"),
        c("dropcap", "Insert drop cap", "dropcap"),
        c("text-from-file", "Text from File", "text-from-file"),
        c("edit-header", "Edit header", "edit-header"),
        c("edit-footer", "Edit footer", "edit-footer"),
        c("pagenum", "Page number", "pagenum"),
        c("datetime", "Date & Time", "datetime"),
        c("numpages", "Number of pages", "numpages"),
        c("insequation", "Insert equation", "insequation"),
        c("inssymbol", "Insert symbol", "inssymbol"),
        c("controls", "Insert content controls", "controls"),
    ]},
    Tab { name: "Draw", cmds: &[
        c("pen", "Pen", "pen"),
        c("highlighter", "Highlighter", "highlighter"),
        c("eraser", "Eraser", "eraser"),
    ]},
    Tab { name: "Layout", cmds: &[
        c("pagemargins", "Margins", "pagemargins"),
        c("pageorient", "Page orientation", "pageorient"),
        c("pagesize", "Page size", "pagesize"),
        c("columns", "Insert column", "columns"),
        c("line-numbers", "Show line numbers", "line-numbers"),
        c("hyphenation", "Change hyphenation", "hyphenation"),
        // 図形まわり。本家の並びのとおりで、表の側と同じ扱い
        // (2026-08-21 発注者「calc と同じようにして」)。
        //
        // **本家にはもう1つ「折り返し」があります**(`img-wrapping`)。
        // 絵の実体がまだ無いので入れていません — 表の側にも無いボタンで、
        // 絵を描いて icons.rs に足せば、ここに1行足すだけで出ます
        c("img-movefrwd", "Bring forward", "img-movefrwd"),
        c("img-movebkwd", "Send Backward", "img-movebkwd"),
        c("img-align", "Alignment", "img-align"),
        c("img-group", "Group", "img-group"),
        x("Merge shapes", "shapes-merge"),
        c("watermark", "Edit watermark", "watermark"),
        c("pagecolor", "Change page colour", "pagecolor"),
        c("colorschemas", "Change colour theme", "colorschemas"),
    ]},
    Tab { name: "References", cmds: &[
        c("toc", "Table of contents", "contents"),
        c("add-text", "Add Text", "add-text"),
        c("toc-update", "Update table of contents", "contents-update"),
        c("bookmarks", "Bookmark", "bookmarks"),
        c("caption", "Caption", "caption"),
        c("crossref", "Cross-reference", "crossref"),
        c("footnote", "Footnote", "footnote"),
        c("tof", "Table of figures", "tof"),
        c("tof-update", "Update table of figures", "tof-update"),
    ]},
    Tab { name: "Forms", cmds: &[
        c("form-text", "Text Field", "form-text"),
        c("form-combo", "Combo box", "form-combo"),
        c("form-dropdown", "Dropdown", "form-dropdown"),
        c("form-checkbox", "Checkbox", "form-checkbox"),
        c("form-radio", "Radio Button", "form-radio"),
        c("form-image", "Picture", "form-image"),
        c("form-email", "Email Address", "form-email"),
        c("form-phone", "Phone Number", "form-phone"),
        c("form-complex", "Complex Field", "form-complex"),
        c("form-signature", "Signature", "form-signature"),
        c("form-name", "Name", "form-name"),
    ]},
    Tab { name: "Collaboration", cmds: &[
        c("coauth-mode", "Co-editing mode", "coauth-mode"),
        c("co-addcomment", "Add Comment", "co-addcomment"),
        c("co-delcomment", "Delete comment", "co-delcomment"),
        c("co-showcomment", "Show comments", "co-showcomment"),
        c("co-chat", "Chat", "co-chat"),
        c("track-changes", "Track changes", "track-changes"),
        c("co-history", "Version history", "co-history"),
    ]},
    Tab { name: "Protection", cmds: &[
        c("prot-sign", "Add digital signature", "prot-sign"),
        c("prot-doc", "Protection", "prot-doc"),
    ]},
    Tab { name: "View", cmds: &[
        t("nav", "Navigation", "nav"),
        c("fit-page", "Fit to page", "fit-page"),
        c("fit-width", "Fit to width", "fit-width"),
        c("zoom100", "Zoom to 100%", "zoom100"),
        c("zoom-in", "Zoom in", "zoom-in"),
        c("zoom-out", "Zoom out", "zoom-out"),
        c("printview", "Print layout", "printview"),
        c("multipage", "Multiple pages", "multipage"),
        t("darkmode", "Dark mode", "darkmode"),
        c("ui-bigger", "Bigger UI text", "ui-bigger"),
        c("ui-smaller", "Smaller UI text", "ui-smaller"),
        c("ruler", "Rulers", "ruler"),
        t("show-toolbar", "Always Show Toolbar", "show-toolbar"),
        t("show-statusbar", "Status Bar", "show-statusbar"),
        t("show-left", "Left Panel", "show-left"),
        t("show-right", "Right Panel", "show-right"),
    ]},
    // calc と同じく**マクロの段**へ(2026-08-16)。「一覧」は置き場の
    // .py、「ファイルから」は置き場の外の .py
    Tab { name: "Macros", cmds: &[
        c("py-list", "Macro list", "plug-manage"),
        c("py-folder", "Open folder", "py-folder"),
        c("ai-macro", "Write macro", "ai-macro"),
    ]},
];

pub const CALC: &[Tab] = &[
    Tab { name: "File", cmds: &[
        c("open", "Open", "open"),
        c("save", "Save", "save"),
        c("pdf", "Print", "print"),
    ]},
    Tab { name: "Home", cmds: &[
        c("copy", "Copy", "copy"),
        c("cut", "Cut", "cut"),
        c("paste", "Paste", "paste"),
        c("copystyle", "Format painter", "copystyle"),
        c("fontname", "Font", "fontname"),
        c("fontsize", "Font size", "fontsize"),
        c("incfont", "Increment font size", "incfont"),
        c("decfont", "Decrement font size", "decfont"),
        c("changecase", "Change case", "changecase"),
        c("bold", "Bold", "bold"),
        c("italic", "Italic", "italic"),
        c("underline", "Underline", "underline"),
        c("strikeout", "Strikethrough", "strikeout"),
        c("subscript", "Subscript", "subscript"),
        c("fontcolor", "Font colour", "fontcolor"),
        c("fillparag", "Fill colour", "fillparag"),
        c("borders", "Borders", "borders"),
        c("top", "Align top", "top"),
        c("middle", "Align middle", "middle"),
        c("bottom", "Align bottom", "bottom"),
        c("wrap", "Wrap text", "wrap"),
        c("text-orient", "Text orientation", "text-orient"),
        c("align-left", "Align left", "align-left"),
        c("align-center", "Centre", "align-center"),
        c("align-right", "Align right", "align-right"),
        c("align-just", "Justified", "align-just"),
        c("align-dist", "Distributed", "align-dist"),
        c("merge", "Merge and centre", "merge"),
        c("direction", "Right-to-left text", "direction"),
        // ホームの Σ は**オートSUM**(2026-08-13 発注者指摘)。前は関数の
        // 挿入(fx と同じ小窓)を置いていたが、本家のホームの Σ は
        // 「上の数値をまとめて =SUM()」の方。関数の挿入は数式タブと fx に居る
        c("sum", "AutoSum", "autosum"),
        c("fill-num", "Fill", "fill-num"),
        c("defname", "Name manager", "named-range"),
        c("clear", "Clear", "clear"),
        c("sort-desc", "Sort descending", "sortdesc"),
        c("sort-asc", "Sort ascending", "sortasc"),
        c("setfilter", "Filters", "setfilter"),
        c("clear-filter", "Clear filter", "clear-filter"),
        c("format", "Number format", "format"),
        c("currency", "Currency style", "currency"),
        c("percents", "Percent style", "percents"),
        c("comma", "Comma style", "comma"),
        c("digit-dec", "Decrease decimal", "digit-dec"),
        c("digit-inc", "Increase decimal", "digit-inc"),
        c("cell-ins", "Insert cells", "cell-ins"),
        c("cell-del", "Delete cells", "cell-del"),
        c("cell-format", "Format cells", "cell-format"),
        c("condformat", "Conditional formatting", "condformat"),
        c("table-tpl", "Format as table template", "table-tpl"),
        c("cell-styles", "Cell Style", "styles"),
        c("replace", "Replace", "replace"),
        c("selectall", "Select all", "select-all"),
    ]},
    Tab { name: "Insert", cmds: &[
        c("pivot-insert", "Insert Pivot Table", "add-pivot"),
        c("instable", "Insert table", "instable"),
        c("insimage", "Insert image", "insimage-c"),
        c("insshape", "Insert shape", "insshape"),
        c("inssmartart", "Insert SmartArt", "inssmartart"),
        c("inscheckbox", "Checkbox", "inscheckbox"),
        c("insrecommend", "Insert recommended chart", "insrecommend"),
        c("inschart", "Insert chart", "inschart"),
        c("inssparkline", "Insert sparkline", "inssparkline"),
        c("co-addcomment", "Comment", "ins-comment"),
        // ここに c("insrecommend", "グラフを挿入", "smartpicker") が居た
        // (2026-08-16 に外した)。id は上の「推奨チャートを挿入」と同じ、
        // 札は上の「グラフを挿入」と同じで、**押すと推奨チャートが出る**。
        // 同じ働きのボタンが2つあり、片方は別の札を着ていた
        c("inshyperlink", "Add link", "inshyperlink"),
        c("insslicer", "Insert slicer", "insslicer"),
        c("instext", "Insert text box", "instext"),
        c("instextart", "Insert Text Art", "instextart"),
        c("edit-header", "Header & Footer", "editheader"),
        c("insequation", "Insert equation", "insequation"),
        c("inssymbol", "Insert symbol", "inssymbol"),
    ]},
    Tab { name: "Draw", cmds: &[
        c("draw-select", "Select", "select-tool"),
        c("pen", "Pen", "pen"),
        c("highlighter", "Highlighter", "highlighter"),
        c("eraser", "Eraser", "eraser"),
    ]},
    Tab { name: "Layout", cmds: &[
        c("pagemargins", "Margins", "pagemargins"),
        c("pageorient", "Page orientation", "pageorient"),
        c("pagesize", "Page size", "pagesize"),
        c("printarea", "Print Area", "printarea"),
        c("pagebreak", "Breaks", "pagebreak"),
        c("edit-header", "Header & Footer", "editheader"),
        c("scale", "Scale To Fit", "scale"),
        c("fit-pages", "Fit to paper", "fit-pages"),
        c("printarea-add", "Add to area", "printarea-add"),
        c("show-breaks", "Page breaks", "show-breaks"),
        c("printtitles", "Print titles", "printtitles"),
        c("rtl-sheet", "Switch the sheet direction so that the first column is on the right side", "rtl-sheet"),
        c("print-gridlines", "Print gridlines", "print-gridlines"),
        c("print-headings", "Print headings", "print-headings"),
        c("img-movefrwd", "Bring forward", "img-movefrwd"),
        c("img-movebkwd", "Send Backward", "img-movebkwd"),
        c("img-align", "Alignment", "img-align"),
        c("img-group", "Group", "img-group"),
        c("shapes-merge", "Merge shapes", "shapes-merge"),
        c("colorschemas", "Change colour theme", "colorschemas"),
    ]},
    Tab { name: "Formula", cmds: &[
        c("insert-function", "Insert function", "additional-formula"),
        // **式から呼べる Python の関数**(funcs の置き場)。人が押して
        // 走るマクロとは別物なので、マクロの段ではなくここに置く
        // (2026-08-16 発注者「UDF とマクロに区分しないといけないのでは」)
        c("func-list", "Python functions", "py-list"),
        c("sum", "AutoSum", "autosum"),
        c("fn-recent", "Recently used", "recent"),
        c("fn-financial", "Financial", "financial"),
        c("fn-logical", "Logical", "logical"),
        c("fn-text", "Text functions", "text"),
        c("fn-datetime", "Date & Time", "datetime"),
        c("fn-lookup", "Lookup & Reference", "lookup"),
        c("fn-math", "Math & Trig", "math"),
        c("fn-more", "More functions", "more"),
        c("defname", "Name manager", "named-range-huge"),
        c("paste-name", "Paste name", "paste-name"),
        c("trace-prec", "Trace Precedents", "trace-prec"),
        c("trace-dep", "Trace Dependents", "trace-dep"),
        c("remove-arrows", "Remove arrows", "remove-arrows"),
        c("show-formulas", "Show formulas", "show-formulas"),
        c("watch", "Watch window", "watch-window"),
        c("calc-mode", "Calculation options", "calculate"),
    ]},
    Tab { name: "Data", cmds: &[
        c("data-from-text", "Text to data", "data-from-text"),
        c("data-external-links", "External links (import as values)", "data-external-links"),
        c("setfilter", "Filters", "setfilter"),
        c("clear-filter", "Clear filter", "clear-filter"),
        c("sort-desc", "Sort descending", "sortdesc"),
        c("sort-asc", "Sort ascending", "sortasc"),
        c("custom-sort", "Sort", "custom-sort"),
        c("text-column", "Text to columns", "text-column"),
        c("rem-duplicates", "Remove duplicates", "rem-duplicates"),
        c("data-validation", "Data Validation", "data-validation"),
        t("dv-mark", "Circle invalid data", "dv-mark"),
        c("goal-seek", "Goal Seek", "goal-seek"),
        c("scenario", "Scenario", "scenario"),
        c("forecast", "Forecast Sheet", "forecast"),
        c("solver", "Solver", "solver"),
        c("group", "Group", "group"),
        c("ungroup", "Ungroup", "ungroup"),
        c("show-details", "Show details", "show-details"),
        c("hide-details", "Hide detail", "hide-details"),
        c("subtotal", "Subtotals", "subtotal"),
        c("datatable", "Data table", "datatable"),
        c("python", "Python", "python"),
        c("csv-kind", "CSV format", "csv-kind"),
        c("flash-fill", "Fill by example", "flash-fill"),
    ]},
        // Python は本家に無いタブ。**このソフトの芯なので独立させる**
    // (2026-08-09 発注者「python をメインのメニューに追加してきちんとやれ」)。
    // データタブのボタン1個に埋もれていて、.py を編集するには @edit と
    // 打つしかなかった — 日本語の名前は IME を挟むので Enter が変換に
    // 食われて辿り着けない。**打たずに選べる**のがこのタブの目的。
    // gen_ribbon.py の APP_TABS にも同じ並びを入れてある(生成し直しても出る)
    // **Python はブックと切り離した。** データとプログラムを分けた以上、
    // このタブは「いま開いているブックに何かをする」場所ではない
    // (発注者 2026-08-15 の指摘)。UDF は作れば SUM と同じで、呼ぶのは
    // セルの式。マクロはマクロ側がブックを新規に作ったり読み込んだりする —
    // どちらも「いまのブック」を前提にしない。だからここに残るのは
    // **編集の口だけ**。手続きの実行・一行のコード・計算し直すは外した
    // **マクロの段は1本**(2026-08-16 発注者「プラグインはマクロだけで
    // いいのでは」)。**走らせられるのは置き場の .py だけ**(同日
    // 発注者「calc, writer から起動できるのは、置き場を固定するのがいい」)
    // — 任意の場所の .py を選んで走らせる口(旧 plug-macros)は外した。
    // 外から持ってきた物は「置き場を開く」で置いてから、一覧で選ぶ。
    // **置く手が1つ挟まる**のが門(置く前に読む)。前は Python の段とプラグインの段に割れていて、
    // `py-list` と `plug-manage` が同じ置き場を2通りに並べていた。
    // 「プラグイン」は作り手の言葉で、使う人の言葉は「マクロ」
    // (2026-08-14 の決め)
    Tab { name: "Macros", cmds: &[
        c("rec-toggle", "Record actions", "py-run"),
        c("py-new", "New .py", "py-new"),
        c("py-list", "Macro list", "py-list"),
        c("ribbon-list", "Ribbon macros", "py-line"),
        c("py-folder", "Open folder", "py-folder"),
    ]},
    Tab { name: "Pivot Table", cmds: &[
        c("pivot-insert", "Insert Pivot Table", "pivot-insert"),
        c("pivot-fields", "Field list", "pivot-fields"),
        c("pivot-refresh", "Update", "pivot-refresh"),
        c("pivot-refresh-all", "Update all", "pivot-refresh-all"),
        c("pivot-source", "Data source", "pivot-source"),
        c("pivot-chart", "PivotChart", "pivot-chart"),
        c("pivot-select", "Select", "pivot-select"),
        c("pivot-totals", "Grand Total", "pivot-totals"),
        c("pivot-subtotals", "Subtotals", "pivot-subtotals"),
        c("pivot-blank", "Blank Rows", "pivot-blank"),
        c("pivot-showas", "Show values as", "pivot-showas"),
        c("pivot-layout", "Report Layout", "pivot-layout"),
        c("pivot-style", "Style", "pivot-style"),
    ]},
    Tab { name: "Table Design", cmds: &[
        c("td-header", "Header row", "td-header"),
        c("td-total", "Total row", "td-total"),
        c("td-band-row", "Banded Rows", "td-band-row"),
        c("td-first", "First column", "td-first"),
        c("td-last", "Last column", "td-last"),
        c("td-band-col", "Banded columns", "td-band-col"),
        c("td-filter", "Filter button", "td-filter"),
        c("rem-duplicates", "Remove Duplicates", "td-remdup"),
        c("td-torange", "Convert to range", "td-torange"),
        c("td-resize", "Resize table", "td-resize"),
    ]},
    Tab { name: "Collaboration", cmds: &[
        c("coauth-mode", "Co-editing mode", "coauth-mode"),
        c("co-addcomment", "Add Comment", "co-addcomment"),
        c("co-delcomment", "Delete comment", "co-delcomment"),
        c("co-showcomment", "Show comments", "co-showcomment"),
        c("co-chat", "Chat", "co-chat"),
        c("co-history", "Version history", "co-history"),
    ]},
    Tab { name: "Protection", cmds: &[
        // 本家 SSE の並び: 暗号化 / ブック / シート / 範囲。
        // ブックと範囲は未実装(灰)。署名は本家に無いこちらのボタン — 末尾
        c("prot-encrypt", "Encrypt", "prot-encrypt"),
        xt("Protect workbook", "protect-workbook"),
        c("prot-doc", "Protect sheet", "protect-sheet"),
        xt("Protect Range", "protect-range"),
        c("prot-sign", "Add digital signature", "prot-sign"),
        c("cell-lock", "Cell lock", "cell-lock"),
        c("prot-allow", "Allowed actions", "prot-allow"),
        c("recover", "Recover", "recover"),
        c("recover-every", "Backup interval", "recover-every"),
        c("read-only-rec", "Suggest read-only", "read-only-rec"),
    ]},
    Tab { name: "View", cmds: &[
        c("sheet-view", "Sheet View", "sheet-view"),
        xm("Normal", "view-normal"),
        xm("Page Break Preview", "view-pagebreak"),
        c("zoom-in", "Zoom in", "zoom-in"),
        c("zoom-out", "Zoom out", "zoom-out"),
        c("zoom100", "Zoom to 100%", "zoom100"),
        c("ui-bigger", "Bigger UI text", "ui-bigger"),
        c("ui-smaller", "Smaller UI text", "ui-smaller"),
        t("darkmode", "Dark mode", "theme"),
        c("freeze", "Freeze panes", "freeze"),
        t("split", "Split", "split"),
        t("formula-bar", "Formula Bar", "formula-bar"),
        c("show-gridlines", "Gridlines", "show-gridlines"),
        t("show-headings", "Headings", "show-headings"),
        t("show-zeros", "Show zeros", "show-zeros"),
        // **左右のパネル**(2026-08-15 発注者)。writer の表示タブと同じ
        // id・同じ札 — 訳が既にあるので新しい鍵は増えない
        t("show-left", "Left Panel", "show-left"),
        t("show-right", "Right Panel", "show-right"),
    ]},
];

/// 実装済みのコマンド数 / 全体(進み具合を隠さない)
pub fn progress(tabs: &[Tab]) -> (usize, usize) {
    let all: usize = tabs.iter().map(|t| t.cmds.len()).sum();
    let ready: usize = tabs.iter().flat_map(|t| t.cmds).filter(|c| c.ready).count();
    (ready, all)
}

#[cfg(test)]
mod tests {
    use super::*;


    /// **どのボタンにも実体のアイコンがある。**
    ///
    /// アイコンは [`crate::icons::find`] が引ける物しか出せない。表に足し忘れると
    /// **ボタンだけが無地で出る** — 押せるし配線もされているので、
    /// 配線の試験(`wiring_tests`)も文言の門番も素通りする。
    ///
    /// **描く側と同じ口(`find`)で引く。** 表は `ICONS` と `OWN_ICONS` の
    /// 二枚あり、片方だけ見ると「無い」と誤って数える(最初それで
    /// 数を間違えた)。
    ///
    /// **いま欠けている物は下に並べて許してある。** 全部描くまで赤には
    /// できないが、**これ以上増やさない**ための止め木になる。
    /// 描いたら一覧から外す(外し忘れも落ちる)。
    /// 2026-08-13 に数えて 77 件。すべて calc の持ち場
    /// **実体の無いアイコン**の一覧。ここに載っている id は無地のボタンで出る。
    /// 2026-08-13 に 77 個ぜんぶ描いたので空。**増えても減っても試験が落ちる**
    /// (下の2つの assert が両方向で見ている)。
    const ICONLESS_BUTTONS: &[&str] = &[];

    #[test]
    fn no_icon_without_a_file_is_added() {
        let mut missing: Vec<&str> = Vec::new();
        for tabs in [WRITER, CALC] {
            for t in tabs {
                for cmd in t.cmds {
                    if cmd.icon.is_empty() {
                        continue;
                    }
                    if crate::icons::find(cmd.icon).is_none() && !missing.contains(&cmd.icon) {
                        missing.push(cmd.icon);
                    }
                }
            }
        }
        let fresh: Vec<&&str> =
            missing.iter().filter(|m| !ICONLESS_BUTTONS.contains(m)).collect();
        assert!(fresh.is_empty(),
            "実体の無いアイコンが増えた: {fresh:?}(絵を描いて icons.rs に足す)");
        let was_fixed: Vec<&&str> =
            ICONLESS_BUTTONS.iter().filter(|a| !missing.contains(a)).collect();
        assert!(was_fixed.is_empty(),
            "アイコンができているのに一覧に残っている: {was_fixed:?}(一覧から外す)");
    }

    #[test]
    fn language_tables_differ_only_in_wording() {
        // id・並び・ready・icon が ja と一致しない表は配線が壊れる —
        // ここで固定する(語は違ってよい。空の語は出さない)
        let mut pairs: Vec<(&[Tab], &[Tab])> = Vec::new();
        for l in lang::i18n::languages() {
            if l == "ja" {
                continue;
            }
            let (w, c) = crate::ribbon_tables::tabs(l)
                .unwrap_or_else(|| panic!("言語 {l} のリボンの表が無い(登録簿のずれ)"));
            pairs.push((WRITER, w));
            pairs.push((CALC, c));
        }
        for (ja, other) in pairs {
            assert_eq!(ja.len(), other.len(), "タブの数が違う");
            for (a, b) in ja.iter().zip(other) {
                assert!(!b.name.is_empty(), "タブ名が空");
                assert_eq!(a.cmds.len(), b.cmds.len(), "「{}」のボタンの数が違う", a.name);
                for (x, y) in a.cmds.iter().zip(b.cmds) {
                    assert_eq!(x.id, y.id, "id がずれた(配線が壊れる)");
                    assert_eq!(x.icon, y.icon, "「{}」の icon が違う", x.id);
                    assert_eq!(x.ready, y.ready, "「{}」の ready が違う", x.id);
                    assert!(!y.label.is_empty(), "「{}」の語が空", x.id);
                }
            }
        }
    }

    #[test]
    fn button_ids_do_not_repeat_within_a_tab() {
        // 画面はボタン1つ1つに gpui の鍵を与える。**段の中で鍵が重なると、
        // 後のボタンの押下が拾われない** — ボタンは出るのに押しても何も
        // 起きない、という形で出る(2026-08-16 実機で踏んだ。鍵が絵の名前
        // だったころ、利用者のマクロが rec-toggle と同じ py-run を名乗った)。
        // 鍵は id(灰色は札)なので、ここが一意ならあの症状は起きない
        for (app, tabs) in [("writer", WRITER), ("calc", CALC)] {
            for tab in tabs {
                let mut seen: Vec<&str> = Vec::new();
                for c in tab.cmds {
                    let k = if c.id.is_empty() { c.label } else { c.id };
                    assert!(!seen.contains(&k), "{app} の「{}」で鍵が重なった: {k}", tab.name);
                    seen.push(k);
                }
            }
        }
    }

    #[test]
    fn user_buttons_do_not_mix_into_the_static_table() {
        // 14言語を突き合わせる門番は静的な表を数える。利用者の札は
        // 利用者自身の言葉で、訳もしない — 表に混ぜたら数が合わなくなる
        for tabs in [WRITER, CALC] {
            for tab in tabs {
                for c in tab.cmds {
                    assert!(
                        !c.id.starts_with(USER_PREFIX),
                        "静的な表に利用者の id が混ざっている: {}",
                        c.id
                    );
                }
            }
        }
    }

    #[test]
    fn every_vendor_tab_is_present() {
        // 発注者確定(2026-08-04): メニューは制限しない。実装しないものも
        // 場所は本家どおり(灰色)。タブごと消すことはしない
        for tabs in [WRITER, CALC] {
            // **「プラグイン」は「マクロ」に改名した**(2026-08-16 発注者
            // 「プラグインはマクロだけでいいのでは」)。本家に同じ段はあるが、
            // 使う人の言葉に寄せた — 段を消したのではなく名を替えた
            for want in ["Collaboration", "Protection", "Macros"] {
                assert!(
                    tabs.iter().any(|t| t.name == want),
                    "タブが無い: {want}"
                );
            }
        }
        assert!(WRITER.iter().any(|t| t.name == "Forms"), "writer にタブが無い: Forms");
        for want in ["Pivot Table", "Table Design"] {
            assert!(CALC.iter().any(|t| t.name == want), "calc にタブが無い: {want}");
        }
    }

    #[test]
    fn ready_and_not_ready_are_distinguished() {
        // 「押せるのに何も起きない」を作らないための検査
        for tabs in [WRITER, CALC] {
            for t in tabs {
                for cmd in t.cmds {
                    assert_eq!(cmd.ready, !cmd.id.is_empty(),
                        "{} の「{}」: ready と id が食い違う", t.name, cmd.label);
                }
            }
        }
    }

    #[test]
    fn the_euro_office_tabs_are_all_present() {
        let names: Vec<&str> = WRITER.iter().map(|t| t.name).collect();
        for want in ["File", "Home", "Insert", "Layout", "References"] {
            assert!(names.contains(&want), "文書に「{want}」タブが無い: {names:?}");
        }
        let names: Vec<&str> = CALC.iter().map(|t| t.name).collect();
        for want in ["File", "Home", "Insert", "Layout", "Formula", "Data"] {
            assert!(names.contains(&want), "表計算に「{want}」タブが無い: {names:?}");
        }
    }

    #[test]
    fn every_language_has_the_same_number_of_items() {
        // 言葉が変わるだけで、リボンの構造は Euro-Office と同じ形
        assert!(WRITER.len() >= 5, "タブが少なすぎる: {}", WRITER.len());
        assert!(CALC.len() >= 6, "タブが少なすぎる: {}", CALC.len());
    }

    #[test]
    fn no_name_is_empty() {
        for tabs in [WRITER, CALC] {
            for t in tabs {
                assert!(!t.name.is_empty());
                for cmd in t.cmds {
                    assert!(!cmd.label.is_empty(), "{} に名無しのコマンド", t.name);
                }
            }
        }
    }
}
