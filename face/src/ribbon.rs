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
    Tab { name: "ファイル", cmds: &[
        c("open", "開く", "open"),
        c("save", "保存", "save"),
        c("pdf", "印刷", "print"),
    ]},
    Tab { name: "ホーム", cmds: &[
        c("copy", "コピー", "copy"),
        c("cut", "切り取り", "cut"),
        c("paste", "貼り付け", "paste"),
        c("fontname", "フォント", "fontname"),
        c("fontsize", "フォントのサイズ", "fontsize"),
        c("incfont", "フォントサイズの拡大", "incfont"),
        c("decfont", "フォントサイズの縮小", "decfont"),
        c("changecase", "大文字小文字を変更", "changecase"),
        c("ruby", "ルビ", "ruby"),
        c("ai-furigana", "ふりがな", "ai-furigana"),
        c("bold", "太字", "bold"),
        c("italic", "斜体", "italic"),
        c("underline", "下線", "underline"),
        c("strikeout", "取り消し線", "strikeout"),
        c("superscript", "上付き", "superscript"),
        c("subscript", "下付き", "subscript"),
        c("highlight", "ハイライトの色", "highlight"),
        c("fontcolor", "フォントの色", "fontcolor"),
        c("clearstyle", "スタイルのクリア", "clearstyle"),
        c("markers", "箇条書き", "markers"),
        c("numbering", "ナンバリング", "numbering"),
        c("multilevels", "複数レベルのリスト", "multilevels"),
        c("decoffset", "インデントを減らす", "decoffset"),
        c("incoffset", "インデントを増やす", "incoffset"),
        c("linespace", "段落の行間", "linespace"),
        c("direction", "テキスト方向", "direction"),
        c("align-left", "左揃え", "align-left"),
        c("align-center", "中央揃え", "align-center"),
        c("align-right", "右揃え", "align-right"),
        c("align-just", "両端揃え", "align-just"),
        c("align-dist", "均等割付", "align-dist"),
        c("hidenchars", "非表示文字", "hidenchars"),
        c("paracolor", "段落の背景色", "paracolor"),
        c("borders", "罫線", "borders"),
        c("parastyle", "段落のスタイル", "styles"),
        c("replace", "置き換え", "replace"),
        c("selectall", "すべて選択", "select-all"),
    ]},
    Tab { name: "挿入", cmds: &[
        c("blankpage", "空白ページの挿入", "blankpage"),
        c("pagebreak", "区切り", "pagebreak"),
        c("instable", "表の挿入", "instable"),
        c("insimage", "画像を挿入", "insertimage"),
        c("insshape", "図形を挿入", "insshape"),
        c("inssmartart", "SmartArtの挿入", "inssmartart"),
        c("inschart", "グラフを挿入", "inschart"),
        c("instext", "テキストボックスの挿入", "instext"),
        c("instextart", "テキストアートの挿入", "instextart"),
        c("dropcap", "ドロップキャップの挿入", "dropcap"),
        c("text-from-file", "ファイルからのテキスト", "text-from-file"),
        c("edit-header", "ヘッダーの編集", "edit-header"),
        c("edit-footer", "フッターの編集", "edit-footer"),
        c("pagenum", "ページ番号", "pagenum"),
        c("datetime", "日付/時刻", "datetime"),
        c("numpages", "ページ数", "numpages"),
        c("insequation", "方程式を挿入", "insequation"),
        c("inssymbol", "記号を挿入", "inssymbol"),
        c("controls", "コンテンツコントロールの挿入", "controls"),
    ]},
    Tab { name: "描画", cmds: &[
        c("pen", "ペン", "pen"),
        c("highlighter", "蛍光ペン", "highlighter"),
        c("eraser", "消しゴム", "eraser"),
    ]},
    Tab { name: "レイアウト", cmds: &[
        c("pagemargins", "余白", "pagemargins"),
        c("pageorient", "印刷の向き", "pageorient"),
        c("pagesize", "ページのサイズ", "pagesize"),
        c("columns", "列の挿入", "columns"),
        c("line-numbers", "行番号を表示する", "line-numbers"),
        c("hyphenation", "ハイフン設定の変更", "hyphenation"),
        // 図形まわり。本家の並びのとおりで、表の側と同じ扱い
        // (2026-08-21 発注者「calc と同じようにして」)。
        //
        // **本家にはもう1つ「折り返し」があります**(`img-wrapping`)。
        // 絵の実体がまだ無いので入れていません — 表の側にも無いボタンで、
        // 絵を描いて icons.rs に足せば、ここに1行足すだけで出ます
        x("前面ヘ移動", "img-movefrwd"),
        x("背面ヘ移動", "img-movebkwd"),
        x("配置", "img-align"),
        x("グループ化", "img-group"),
        x("図形を結合", "shapes-merge"),
        c("watermark", "透かしを編集する", "watermark"),
        c("pagecolor", "ページ色の変更", "pagecolor"),
        c("colorschemas", "配色の変更", "colorschemas"),
    ]},
    Tab { name: "参考資料", cmds: &[
        c("toc", "目次", "contents"),
        c("add-text", "テキストの追加", "add-text"),
        c("toc-update", "目次の更新", "contents-update"),
        c("bookmarks", "ブックマーク", "bookmarks"),
        c("caption", "図表番号", "caption"),
        c("crossref", "相互参照", "crossref"),
        c("footnote", "脚注", "footnote"),
        c("tof", "図表目次", "tof"),
        c("tof-update", "図表目次の更新", "tof-update"),
    ]},
    Tab { name: "フォーム", cmds: &[
        c("form-text", "テキストフィールド", "form-text"),
        c("form-combo", "コンボボックス", "form-combo"),
        c("form-dropdown", "ドロップダウン", "form-dropdown"),
        c("form-checkbox", "チェックボックス", "form-checkbox"),
        c("form-radio", "ラジオボタン", "form-radio"),
        c("form-image", "画像", "form-image"),
        c("form-email", "メールアドレス", "form-email"),
        c("form-phone", "電話番号", "form-phone"),
        c("form-complex", "複合フィールド", "form-complex"),
        c("form-signature", "署名", "form-signature"),
        c("form-name", "名前", "form-name"),
    ]},
    Tab { name: "共同編集", cmds: &[
        c("coauth-mode", "共同編集モード", "coauth-mode"),
        c("co-addcomment", "コメントを追加", "co-addcomment"),
        c("co-delcomment", "コメントを削除", "co-delcomment"),
        c("co-showcomment", "コメントの表示", "co-showcomment"),
        c("co-chat", "チャット", "co-chat"),
        c("track-changes", "変更履歴", "track-changes"),
        c("co-history", "バージョン履歴", "co-history"),
    ]},
    Tab { name: "保護", cmds: &[
        c("prot-sign", "デジタル署名を追加", "prot-sign"),
        c("prot-doc", "保護", "prot-doc"),
    ]},
    Tab { name: "表示", cmds: &[
        t("nav", "ナビゲーション", "nav"),
        c("fit-page", "ページに合わせる", "fit-page"),
        c("fit-width", "幅に合わせる", "fit-width"),
        c("zoom100", "100%に拡大する", "zoom100"),
        c("zoom-in", "拡大", "zoom-in"),
        c("zoom-out", "縮小", "zoom-out"),
        c("printview", "印刷レイアウト", "printview"),
        c("multipage", "複数ページ", "multipage"),
        t("darkmode", "ダークモード", "darkmode"),
        c("ruler", "ルーラー", "ruler"),
        t("show-toolbar", "ツールバーを常に表示する", "show-toolbar"),
        t("show-statusbar", "ステータスバー", "show-statusbar"),
        t("show-left", "左パネル", "show-left"),
        t("show-right", "右パネル", "show-right"),
    ]},
    // calc と同じく**マクロの段**へ(2026-08-16)。「一覧」は置き場の
    // .py、「ファイルから」は置き場の外の .py
    Tab { name: "マクロ", cmds: &[
        c("py-list", "一覧", "plug-manage"),
        c("ai-macro", "マクロを書く", "ai-macro"),
    ]},
];

pub const CALC: &[Tab] = &[
    Tab { name: "ファイル", cmds: &[
        c("open", "開く", "open"),
        c("save", "保存", "save"),
        c("pdf", "印刷", "print"),
    ]},
    Tab { name: "ホーム", cmds: &[
        c("copy", "コピー", "copy"),
        c("cut", "切り取り", "cut"),
        c("paste", "貼り付け", "paste"),
        c("copystyle", "書式のコピー", "copystyle"),
        c("fontname", "フォント", "fontname"),
        c("fontsize", "フォントのサイズ", "fontsize"),
        c("incfont", "フォントサイズの拡大", "incfont"),
        c("decfont", "フォントサイズの縮小", "decfont"),
        c("changecase", "大文字小文字を変更", "changecase"),
        c("bold", "太字", "bold"),
        c("italic", "斜体", "italic"),
        c("underline", "下線", "underline"),
        c("strikeout", "取り消し線", "strikeout"),
        c("subscript", "下付き", "subscript"),
        c("fontcolor", "フォントの色", "fontcolor"),
        c("fillparag", "塗りつぶしの色", "fillparag"),
        c("borders", "罫線", "borders"),
        c("top", "上揃え", "top"),
        c("middle", "上下中央揃え", "middle"),
        c("bottom", "下揃え", "bottom"),
        c("wrap", "折り返して全体を表示する", "wrap"),
        c("text-orient", "方向", "text-orient"),
        c("align-left", "左揃え", "align-left"),
        c("align-center", "中央揃え", "align-center"),
        c("align-right", "右揃え", "align-right"),
        c("align-just", "両端揃え", "align-just"),
        c("align-dist", "均等割付", "align-dist"),
        c("merge", "結合して、中央に配置する", "merge"),
        c("direction", "文字の向き(右横書き)", "direction"),
        // ホームの Σ は**オートSUM**(2026-08-13 発注者指摘)。前は関数の
        // 挿入(fx と同じ小窓)を置いていたが、本家のホームの Σ は
        // 「上の数値をまとめて =SUM()」の方。関数の挿入は数式タブと fx に居る
        c("sum", "オートSUM", "autosum"),
        c("fill-num", "フィル", "fill-num"),
        c("defname", "名前の管理", "named-range"),
        c("clear", "消去", "clear"),
        c("sort-desc", "降順並べ替え", "sortdesc"),
        c("sort-asc", "昇順並べ替え", "sortasc"),
        c("setfilter", "フィルター", "setfilter"),
        c("clear-filter", "フィルターを解除", "clear-filter"),
        c("format", "数値の書式", "format"),
        c("currency", "通貨スタイル", "currency"),
        c("percents", "パーセントのスタイル", "percents"),
        c("comma", "カンマスタイル", "comma"),
        c("digit-dec", "小数点以下の表示桁数を減らす", "digit-dec"),
        c("digit-inc", "小数点以下の表示桁数を増やす", "digit-inc"),
        c("cell-ins", "セルを挿入", "cell-ins"),
        c("cell-del", "セルを削除", "cell-del"),
        c("cell-format", "セルの書式設定", "cell-format"),
        c("condformat", "条件付き書式", "condformat"),
        c("table-tpl", "表として書式設定", "table-tpl"),
        c("cell-styles", "セルのスタイル", "styles"),
        c("replace", "置き換え", "replace"),
        c("selectall", "すべて選択", "select-all"),
    ]},
    Tab { name: "挿入", cmds: &[
        c("pivot-insert", "ピボットテーブルを挿入", "add-pivot"),
        c("instable", "表の挿入", "instable"),
        c("insimage", "画像を挿入", "insimage-c"),
        c("insshape", "図形を挿入", "insshape"),
        c("inssmartart", "SmartArtの挿入", "inssmartart"),
        c("inscheckbox", "チェックボックス", "inscheckbox"),
        c("insrecommend", "推奨チャートを挿入", "insrecommend"),
        c("inschart", "グラフを挿入", "inschart"),
        c("inssparkline", "スパークラインを挿入する", "inssparkline"),
        c("co-addcomment", "コメント", "ins-comment"),
        // ここに c("insrecommend", "グラフを挿入", "smartpicker") が居た
        // (2026-08-16 に外した)。id は上の「推奨チャートを挿入」と同じ、
        // 札は上の「グラフを挿入」と同じで、**押すと推奨チャートが出る**。
        // 同じ働きのボタンが2つあり、片方は別の札を着ていた
        c("inshyperlink", "ハイパーリンクを追加", "inshyperlink"),
        c("insslicer", "スライサーを挿入", "insslicer"),
        c("instext", "テキストボックスの挿入", "instext"),
        c("instextart", "テキストアートの挿入", "instextart"),
        c("edit-header", "ヘッダー/フッター", "editheader"),
        c("insequation", "方程式を挿入", "insequation"),
        c("inssymbol", "記号を挿入", "inssymbol"),
    ]},
    Tab { name: "描画", cmds: &[
        c("draw-select", "選択", "select-tool"),
        c("pen", "ペン", "pen"),
        c("highlighter", "蛍光ペン", "highlighter"),
        c("eraser", "消しゴム", "eraser"),
    ]},
    Tab { name: "レイアウト", cmds: &[
        c("pagemargins", "余白", "pagemargins"),
        c("pageorient", "印刷の向き", "pageorient"),
        c("pagesize", "ページのサイズ", "pagesize"),
        c("printarea", "印刷範囲", "printarea"),
        c("pagebreak", "区切り", "pagebreak"),
        c("edit-header", "ヘッダー/フッター", "editheader"),
        c("scale", "拡大縮小印刷", "scale"),
        c("fit-pages", "紙に収める", "fit-pages"),
        c("printarea-add", "範囲を足す", "printarea-add"),
        c("show-breaks", "紙の切れ目", "show-breaks"),
        c("printtitles", "タイトルを印刷する", "printtitles"),
        c("rtl-sheet", "最初の列が右側に来るようにシートの方向を切り替える", "rtl-sheet"),
        c("print-gridlines", "枠線も印刷", "print-gridlines"),
        c("print-headings", "見出しも印刷", "print-headings"),
        x("前面ヘ移動", "img-movefrwd"),
        x("背面ヘ移動", "img-movebkwd"),
        x("配置", "img-align"),
        x("グループ化", "img-group"),
        x("図形を結合", "shapes-merge"),
        c("colorschemas", "配色の変更", "colorschemas"),
    ]},
    Tab { name: "数式", cmds: &[
        c("insert-function", "関数の挿入", "additional-formula"),
        // **式から呼べる Python の関数**(funcs の置き場)。人が押して
        // 走るマクロとは別物なので、マクロの段ではなくここに置く
        // (2026-08-16 発注者「UDF とマクロに区分しないといけないのでは」)
        c("func-list", "Python の関数", "py-list"),
        c("sum", "オートSUM", "autosum"),
        c("fn-recent", "最近使った関数", "recent"),
        c("fn-financial", "財務", "financial"),
        c("fn-logical", "論理", "logical"),
        c("fn-text", "文字列操作", "text"),
        c("fn-datetime", "日付/時刻", "datetime"),
        c("fn-lookup", "検索/行列", "lookup"),
        c("fn-math", "数学/三角", "math"),
        c("fn-more", "その他の関数", "more"),
        c("defname", "名前の管理", "named-range-huge"),
        c("paste-name", "名前を貼り付け", "paste-name"),
        c("trace-prec", "参照元のトレース", "trace-prec"),
        c("trace-dep", "参照先のトレース", "trace-dep"),
        c("remove-arrows", "トレース矢印の削除", "remove-arrows"),
        c("show-formulas", "数式の表示", "show-formulas"),
        c("watch", "ウォッチウィンドウ", "watch-window"),
        c("calc-mode", "計算方法", "calculate"),
    ]},
    Tab { name: "データ", cmds: &[
        c("data-from-text", "テキストからデータ", "data-from-text"),
        c("data-external-links", "外部リンク(値で取り込む)", "data-external-links"),
        c("setfilter", "フィルター", "setfilter"),
        c("clear-filter", "フィルターを解除", "clear-filter"),
        c("sort-desc", "降順並べ替え", "sortdesc"),
        c("sort-asc", "昇順並べ替え", "sortasc"),
        c("custom-sort", "並べ替え", "custom-sort"),
        c("text-column", "区切り位置", "text-column"),
        c("rem-duplicates", "重複の削除", "rem-duplicates"),
        c("data-validation", "データの入力規則", "data-validation"),
        c("goal-seek", "ゴールシーク", "goal-seek"),
        c("solver", "ソルバー", "solver"),
        c("group", "グループ化", "group"),
        c("ungroup", "グループ解除", "ungroup"),
        c("show-details", "詳細の表示", "show-details"),
        c("hide-details", "詳細の非表示", "hide-details"),
        c("subtotal", "小計", "subtotal"),
        c("datatable", "データテーブル", "datatable"),
        c("python", "Python", "python"),
        c("csv-kind", "CSV の形", "csv-kind"),
        c("flash-fill", "フラッシュフィル", "flash-fill"),
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
    Tab { name: "マクロ", cmds: &[
        c("rec-toggle", "操作を記録", "py-run"),
        c("py-new", "新しい .py", "py-new"),
        c("py-list", "一覧", "py-list"),
        c("ribbon-list", "リボンのマクロ", "py-line"),
        c("py-folder", "置き場を開く", "py-folder"),
    ]},
    Tab { name: "ピボットテーブル", cmds: &[
        c("pivot-insert", "ピボットテーブルを挿入", "pivot-insert"),
        c("pivot-fields", "フィールドリスト", "pivot-fields"),
        c("pivot-refresh", "更新", "pivot-refresh"),
        c("pivot-refresh-all", "すべて更新", "pivot-refresh-all"),
        c("pivot-select", "選択する", "pivot-select"),
        c("pivot-totals", "総計", "pivot-totals"),
        c("pivot-subtotals", "小計", "pivot-subtotals"),
        c("pivot-blank", "空行", "pivot-blank"),
        c("pivot-showas", "計算の種類", "pivot-showas"),
        c("pivot-layout", "レポートのレイアウト", "pivot-layout"),
        c("pivot-style", "スタイル", "pivot-style"),
    ]},
    Tab { name: "表のデザイン", cmds: &[
        c("td-header", "ヘッダー行", "td-header"),
        c("td-total", "合計行", "td-total"),
        c("td-band-row", "縞模様の行", "td-band-row"),
        c("td-first", "最初の列", "td-first"),
        c("td-last", "最後の列", "td-last"),
        c("td-band-col", "縞模様の列", "td-band-col"),
        c("td-filter", "フィルタのボタン", "td-filter"),
        c("rem-duplicates", "重複データを削除", "td-remdup"),
        c("td-torange", "範囲に変換する", "td-torange"),
        c("td-resize", "テーブルのサイズ変更", "td-resize"),
    ]},
    Tab { name: "共同編集", cmds: &[
        c("coauth-mode", "共同編集モード", "coauth-mode"),
        c("co-addcomment", "コメントを追加", "co-addcomment"),
        c("co-delcomment", "コメントを削除", "co-delcomment"),
        c("co-showcomment", "コメントの表示", "co-showcomment"),
        c("co-chat", "チャット", "co-chat"),
        c("co-history", "バージョン履歴", "co-history"),
    ]},
    Tab { name: "保護", cmds: &[
        // 本家 SSE の並び: 暗号化 / ブック / シート / 範囲。
        // ブックと範囲は未実装(灰)。署名は本家に無いこちらのボタン — 末尾
        c("prot-encrypt", "暗号化する", "prot-encrypt"),
        xt("ブックを保護する", "protect-workbook"),
        c("prot-doc", "シートを保護する", "protect-sheet"),
        xt("範囲を保護する", "protect-range"),
        c("prot-sign", "デジタル署名を追加", "prot-sign"),
        c("cell-lock", "セルのロック", "cell-lock"),
        c("prot-allow", "許可する操作", "prot-allow"),
        c("recover", "復旧", "recover"),
        c("recover-every", "控えの間隔", "recover-every"),
        c("read-only-rec", "読み取り専用を勧める", "read-only-rec"),
    ]},
    Tab { name: "表示", cmds: &[
        c("sheet-view", "シートの表示", "sheet-view"),
        xm("標準", "view-normal"),
        xm("改ページ プレビュー", "view-pagebreak"),
        c("zoom-in", "拡大", "zoom-in"),
        c("zoom-out", "縮小", "zoom-out"),
        c("ui-bigger", "画面の文字を大きく", "ui-bigger"),
        c("ui-smaller", "画面の文字を小さく", "ui-smaller"),
        t("darkmode", "ダークモード", "theme"),
        c("freeze", "ウィンドウ枠の固定", "freeze"),
        t("formula-bar", "数式バー", "formula-bar"),
        c("show-gridlines", "枠線表示", "show-gridlines"),
        t("show-headings", "見出し", "show-headings"),
        t("show-zeros", "0を表示する", "show-zeros"),
        // **左右のパネル**(2026-08-15 発注者)。writer の表示タブと同じ
        // id・同じ札 — 訳が既にあるので新しい鍵は増えない
        t("show-left", "左パネル", "show-left"),
        t("show-right", "右パネル", "show-right"),
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
    const アイコンの無いボタン: &[&str] = &[];

    #[test]
    fn 実体の無いアイコンを増やさない() {
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
        let 新しい: Vec<&&str> =
            missing.iter().filter(|m| !アイコンの無いボタン.contains(m)).collect();
        assert!(新しい.is_empty(),
            "実体の無いアイコンが増えた: {新しい:?}(絵を描いて icons.rs に足す)");
        let 直った: Vec<&&str> =
            アイコンの無いボタン.iter().filter(|a| !missing.contains(a)).collect();
        assert!(直った.is_empty(),
            "アイコンができているのに一覧に残っている: {直った:?}(一覧から外す)");
    }

    #[test]
    fn 各言語の表は語だけが違う() {
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
    fn 段の中でボタンの鍵が重ならない() {
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
    fn 利用者のボタンは静的な表に混ざらない() {
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
    fn 本家のタブが全部ある() {
        // 発注者確定(2026-08-04): メニューは制限しない。実装しないものも
        // 場所は本家どおり(灰色)。タブごと消すことはしない
        for tabs in [WRITER, CALC] {
            // **「プラグイン」は「マクロ」に改名した**(2026-08-16 発注者
            // 「プラグインはマクロだけでいいのでは」)。本家に同じ段はあるが、
            // 使う人の言葉に寄せた — 段を消したのではなく名を替えた
            for want in ["共同編集", "保護", "マクロ"] {
                assert!(
                    tabs.iter().any(|t| t.name == want),
                    "タブが無い: {want}"
                );
            }
        }
        assert!(WRITER.iter().any(|t| t.name == "フォーム"), "writer にタブが無い: フォーム");
        for want in ["ピボットテーブル", "表のデザイン"] {
            assert!(CALC.iter().any(|t| t.name == want), "calc にタブが無い: {want}");
        }
    }

    #[test]
    fn 実装済みと未実装が区別されている() {
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
    fn euro_officeのタブが揃っている() {
        let names: Vec<&str> = WRITER.iter().map(|t| t.name).collect();
        for want in ["ファイル", "ホーム", "挿入", "レイアウト", "参考資料"] {
            assert!(names.contains(&want), "文書に「{want}」タブが無い: {names:?}");
        }
        let names: Vec<&str> = CALC.iter().map(|t| t.name).collect();
        for want in ["ファイル", "ホーム", "挿入", "レイアウト", "数式", "データ"] {
            assert!(names.contains(&want), "表計算に「{want}」タブが無い: {names:?}");
        }
    }

    #[test]
    fn どの言語でも並びの数は同じ() {
        // 言葉が変わるだけで、リボンの構造は Euro-Office と同じ形
        assert!(WRITER.len() >= 5, "タブが少なすぎる: {}", WRITER.len());
        assert!(CALC.len() >= 6, "タブが少なすぎる: {}", CALC.len());
    }

    #[test]
    fn 名前が空でない() {
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
