//! **意味だけの本文 + テンプレート → HTML + CSS。**
//!
//! 発注者 2026-08-17「web ビルダー、アプリビルダー、帳票ビルダーを優先」。
//! 3つとも土台はこれです。本文(.adoc)は意味だけを持ち、見た目はテンプレート
//! (.toml)が持つ、という分け方がそのまま HTML と CSS に写ります。
//!
//! - 段落の役割 → `h1` `h2` `h3` `p` `blockquote` `ul` `ol`
//! - スタイルの名前 → `class`(CSS 側で見た目を決める)
//! - テンプレートのスタイル → CSS の規則
//!
//! **見た目を HTML の中に埋め込みません。** 埋め込むと、テンプレートを
//! 差し替えても何も変わらない、という本末転倒になります。強調(太字・斜体)は
//! 意味なので `strong` `em` で出します。
//!
//! 読む側は [`crate::html`] にあります。あちらは他所の HTML を受ける口で、
//! こちらは自分の文書を出す口です。

use crate::doc::{Align, Block, Document, InlineImage, ListKind, ParaStyle, Run, Sdt, SdtKind};
use crate::theme::{StyleDef, Theme};
use std::sync::Arc;

/// 出来上がりの HTML。`css` は別ファイルにも埋め込みにも使えます。
pub struct Page {
    pub html: String,
    pub css: String,
    /// 一緒に書き出す画像(HTML から見た相対の径路, 中身)。
    ///
    /// **画像を HTML の中に埋め込みません。** 埋め込むと文字数が何倍にもなり、
    /// 直すときに画像だけ差し替えることも出来なくなります。呼ぶ側が
    /// この並びをファイルに書きます。
    pub assets: Vec<(String, Arc<Vec<u8>>)>,
}

/// 本文を組む途中の控え。**脚注と画像は後ろでまとめる**ので、出てきた順に
/// ここへ集めます。
#[derive(Default)]
struct Ctx {
    /// 脚注の文章(出てきた順)。番号は並びの位置
    notes: Vec<String>,
    assets: Vec<(String, Arc<Vec<u8>>)>,
}

impl Ctx {
    /// 画像を控えて、HTML から参照する径路を返します。
    ///
    /// 径路はファイルが持っているもの(`src`)を使います。docx 由来の画像は
    /// 径路を持たないので、こちらで名前を付けます。
    fn asset(&mut self, im: &InlineImage) -> String {
        if let Some(s) = &im.src {
            if !im.bytes.is_empty() && !self.assets.iter().any(|(p, _)| p == s) {
                self.assets.push((s.clone(), im.bytes.clone()));
            }
            return s.clone();
        }
        // 名前の付け方は adoc の書き出しと同じ物を使います
        let path = format!(
            "images/図{}.{}",
            self.assets.len() + 1,
            crate::adoc::image_ext(&im.bytes)
        );
        self.assets.push((path.clone(), im.bytes.clone()));
        path
    }
}

/// HTML の特殊文字を逃がします。**属性にも本文にも同じ物を使います**
/// (`"` を逃がさないと class の値で壊れます)。
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// スタイルの名前 → CSS の class 名。
///
/// 日本語の名前をそのまま class に使えます(CSS も HTML も Unicode を
/// 受けます)が、空白と記号は使えないので `-` に替えます。
fn class_of(name: &str) -> String {
    let mut o = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            o.push(c);
        } else {
            o.push('-');
        }
    }
    format!("st-{o}")
}

/// run 1つを HTML に。**太字と斜体は意味**なので `strong` / `em` で出します。
fn run_html(r: &Run, doc: &Document, ctx: &mut Ctx) -> String {
    // 記入欄は**その場**に出します。周りの字と並ぶのが記入用紙の形です
    if let Some(sdt) = &r.fmt.sdt {
        return field_html(sdt);
    }
    // 脚注の印(字を持たない run)。文章は後ろにまとめて、行き帰りの
    // リンクで結びます。adoc の `footnote:[…]` に対応します
    if let Some(fr) = &r.fmt.footnote {
        let text: String = doc
            .footnotes
            .iter()
            .find(|f| f.id == fr.id && f.endnote == fr.endnote)
            .map(|f| {
                f.paragraphs
                    .iter()
                    .flat_map(|p| p.runs.iter())
                    .map(|r| r.text.as_str())
                    .collect()
            })
            .unwrap_or_default();
        ctx.notes.push(text);
        let n = ctx.notes.len();
        return format!("<sup class=\"fn\"><a id=\"fnref{n}\" href=\"#fn{n}\">{n}</a></sup>");
    }
    // 相互参照。**見えている字は写しでしかない**ので、リンクの先はしおりの
    // 名前にします(adoc の `<<名前>>`)
    if let Some(f) = &r.fmt.field {
        let label = if r.text.is_empty() { esc(&f.name) } else { esc(&r.text) };
        return format!("<a href=\"#{}\">{label}</a>", esc(&f.name));
    }
    let mut s = esc(&r.text);
    if let Some(name) = &r.fmt.style_id {
        // 等幅は `code`。Web で意味のある札があるものは、その札で出します
        s = if name == crate::adoc::MONO {
            format!("<code>{s}</code>")
        } else {
            format!("<span class=\"{}\">{s}</span>", class_of(name))
        };
    }
    if r.fmt.superscript {
        s = format!("<sup>{s}</sup>");
    }
    if r.fmt.subscript {
        s = format!("<sub>{s}</sub>");
    }
    if r.fmt.italic {
        s = format!("<em>{s}</em>");
    }
    if r.fmt.bold {
        s = format!("<strong>{s}</strong>");
    }
    if let Some(href) = &r.fmt.link {
        s = format!("<a href=\"{}\">{s}</a>", esc(href));
    }
    if let Some(yomi) = &r.fmt.ruby {
        // ふりがなは HTML の ruby がそのまま受けます
        s = format!("<ruby>{s}<rt>{}</rt></ruby>", esc(yomi));
    }
    s
}

fn runs_html(runs: &[Run], doc: &Document, ctx: &mut Ctx) -> String {
    runs.iter().map(|r| run_html(r, doc, ctx)).collect()
}

/// 段落に入っている画像 → `img`。
///
/// 数式は**原文(LaTeX)を `data-tex` に残します。** 絵は組んだ結果でしかない
/// ので、受け取った側が組み直せるようにします(模型が `tex` を運ぶのと同じ
/// 理由です)。
fn imgs_html(p: &crate::doc::Paragraph, ctx: &mut Ctx) -> String {
    let mut o = String::new();
    for im in p.images_new.iter().chain(p.images.iter()) {
        // **絵が無い数式は、原文をそのまま出します。** 空の img を出すと
        // 壊れた画像の印が並ぶだけです(絵はまだ組んでいないだけなので)。
        // 径路を持つ画像は、中身が無くてもファイルが隣にあるので出します
        if im.bytes.is_empty() && im.src.is_none() {
            if let Some(tex) = &im.tex {
                o.push_str(&format!(
                    "<span class=\"stem\" data-tex=\"{}\">{}</span>",
                    esc(tex),
                    esc(tex)
                ));
            }
            continue;
        }
        let src = ctx.asset(im);
        let size = format!(" style=\"width:{}mm;height:{}mm\"", im.w_mm, im.h_mm);
        match &im.tex {
            Some(tex) => o.push_str(&format!(
                "<img class=\"stem\" src=\"{}\" alt=\"{}\" data-tex=\"{}\"{size}>",
                esc(&src),
                esc(tex),
                esc(tex)
            )),
            None => o.push_str(&format!("<img src=\"{}\" alt=\"\"{size}>", esc(&src))),
        }
    }
    o
}

/// 記入欄1つ → HTML の入力欄。
///
/// **名前は `tag`(機械で引く名前)を使います。** 画面に出す名前(`alias`)は
/// 人が読むためのもので、送られる側が見るのは `tag` です。docx の記入欄も
/// 同じ分け方をしています。
fn field_html(s: &Sdt) -> String {
    let name = esc(if s.tag.is_empty() { &s.alias } else { &s.tag });
    let label = esc(if s.alias.is_empty() { &s.tag } else { &s.alias });
    let id = format!("f-{name}");
    let input = match s.kind {
        SdtKind::Dropdown | SdtKind::Combo => {
            let opts: String = s
                .items
                .iter()
                .map(|o| format!("<option>{}</option>", esc(o)))
                .collect();
            // コンボは打つこともできるので、選択肢を候補として添えます
            if s.kind == SdtKind::Combo {
                format!(
                    "<input id=\"{id}\" name=\"{name}\" list=\"l-{name}\">\
                     <datalist id=\"l-{name}\">{opts}</datalist>"
                )
            } else {
                format!("<select id=\"{id}\" name=\"{name}\">{opts}</select>")
            }
        }
        SdtKind::Checkbox => {
            format!("<input id=\"{id}\" name=\"{name}\" type=\"checkbox\">")
        }
        SdtKind::Date => format!("<input id=\"{id}\" name=\"{name}\" type=\"date\">"),
        SdtKind::Email => format!("<input id=\"{id}\" name=\"{name}\" type=\"email\">"),
        SdtKind::Phone => format!("<input id=\"{id}\" name=\"{name}\" type=\"tel\">"),
        SdtKind::Picture => {
            format!("<input id=\"{id}\" name=\"{name}\" type=\"file\" accept=\"image/*\">")
        }
        // 複数行と署名は、いまは自由記入として出します。署名の絵を描かせる
        // 部品は持っていないので、**無い機能を有るように見せません**
        SdtKind::Complex | SdtKind::Signature => {
            format!("<textarea id=\"{id}\" name=\"{name}\" rows=\"3\"></textarea>")
        }
        SdtKind::Text => format!("<input id=\"{id}\" name=\"{name}\">"),
    };
    format!("<label class=\"field\" for=\"{id}\">{label}</label>{input}")
}

/// 文書の中の記入欄を、出てくる順に集めます。
pub fn fields(doc: &Document) -> Vec<Sdt> {
    let mut v = Vec::new();
    for b in &doc.blocks {
        let paras: Vec<_> = match b {
            Block::Para(p) => vec![p],
            Block::Table(t) => t
                .rows
                .iter()
                .flat_map(|r| r.iter())
                .flat_map(|c| c.paragraphs.iter())
                .collect(),
        };
        for p in paras {
            for r in &p.runs {
                if let Some(s) = &r.fmt.sdt {
                    if !v.iter().any(|x: &Sdt| x.tag == s.tag && x.alias == s.alias) {
                        v.push((**s).clone());
                    }
                }
            }
        }
    }
    v
}

/// 段落の役割 → タグ。
///
/// **題名があるときは見出しを1つ下げます。** AsciiDoc の `= 題` は文書の
/// 題名、`== 章` は最初の節です。どちらも `h1` にすると、1ページに `h1` が
/// 2つ並んで意味が壊れます(読み上げも検索も見出しの階層を見ます)。
fn tag_of(style: ParaStyle, 題名あり: bool) -> &'static str {
    let d = usize::from(題名あり);
    match style {
        ParaStyle::Heading(n) => match (n as usize + d).min(6) {
            1 => "h1",
            2 => "h2",
            3 => "h3",
            4 => "h4",
            5 => "h5",
            _ => "h6",
        },
        ParaStyle::Quote => "blockquote",
        ParaStyle::Title => "h1",
        _ => "p",
    }
}

/// テンプレートのスタイル1つ → CSS の規則。
fn css_rule(sel: &str, d: &StyleDef) -> String {
    let mut v: Vec<String> = Vec::new();
    if let Some(pt) = d.size_pt {
        v.push(format!("font-size:{pt}pt"));
    }
    if let Some(f) = &d.font {
        v.push(format!("font-family:{:?}", f));
    }
    if d.bold {
        v.push("font-weight:bold".into());
    }
    if d.italic {
        v.push("font-style:italic".into());
    }
    if d.underline {
        v.push("text-decoration:underline".into());
    }
    if let Some(c) = &d.color {
        v.push(format!("color:#{c}"));
    }
    if let Some(c) = &d.shade {
        v.push(format!("background:#{c}"));
    }
    if let Some(a) = d.align {
        v.push(format!(
            "text-align:{}",
            match a {
                Align::Left => "left",
                Align::Center => "center",
                Align::Right => "right",
                Align::Justify | Align::Distribute => "justify",
            }
        ));
    }
    if d.space_before_pt != 0.0 {
        v.push(format!("margin-top:{}pt", d.space_before_pt));
    }
    if d.space_after_pt != 0.0 {
        v.push(format!("margin-bottom:{}pt", d.space_after_pt));
    }
    if let Some(ls) = d.line_spacing {
        v.push(format!("line-height:{ls}"));
    }
    // 1行目の字下げ。**全角の文字数**なので em で出す(字の大きさに付いて回る)
    if let Some(f) = d.first_line_chars {
        v.push(format!("text-indent:{f}em"));
    }
    if v.is_empty() {
        return String::new();
    }
    format!("{sel} {{ {} }}\n", v.join("; "))
}

/// テンプレート → CSS。
///
/// 役割の名前(本文・見出し1〜3・引用)はタグに、それ以外の名前は class に
/// 割り当てます。**組み方の3値もここで効きます** — 横幅が可変なら紙の幅を
/// 使わず、区切りが無ければページの区切りも入れません。
pub fn css(th: &Theme, 題名あり: bool) -> String {
    let mut o = String::new();
    o.push_str("/* officework が書き出した見た目。本文(HTML)は触らずに、ここだけ直せます */\n");

    // 文書全体
    let mut body: Vec<String> = Vec::new();
    if let Some(f) = &th.font {
        body.push(format!("font-family:{:?}", f));
    }
    if let Some(pt) = th.size_pt {
        body.push(format!("font-size:{pt}pt"));
    }
    if th.setting.fluid {
        // 横幅可変(Web の組み方)。読みやすい上限だけ置きます
        body.push("max-width:42em".into());
        body.push("margin:0 auto".into());
        body.push("padding:0 1em".into());
    } else if let Some(p) = &th.page {
        // 紙と同じ幅(帳票の組み方)
        body.push(format!("width:{}mm", p.w_mm - p.left_mm - p.right_mm));
        body.push("margin:0 auto".into());
    }
    if !body.is_empty() {
        o.push_str(&format!("body {{ {} }}\n", body.join("; ")));
    }

    // 節ごとに1枚(発表の組み方)は、印刷のときだけ効かせます
    if th.setting.per_section() {
        o.push_str("@media print { h1 { break-before: page } }\n");
    }
    if th.setting.keep {
        o.push_str("p, blockquote, li { break-inside: avoid }\n");
    }
    // 改ページ(adoc の `<<<`)。画面では続けて見せ、印刷で折ります
    o.push_str("@media print { .pagebreak { break-before: page } }\n");
    // 画像は入れ物より大きくしません(スマホで横にはみ出さないように)
    o.push_str("img { max-width: 100% }\n");

    for d in &th.styles {
        // **本文の側と同じ数え方にします。** 題名があると見出しが1つ下がるので、
        // 「見出し1」の規則は h2 に当てないと効きません
        let sel = match d.name.as_str() {
            "本文" => "p".to_string(),
            "見出し1" | "見出し2" | "見出し3" => {
                let n: usize = d.name.chars().last().and_then(|c| c.to_digit(10)).unwrap_or(1) as usize;
                format!("h{}", (n + usize::from(題名あり)).min(6))
            }
            "引用" => "blockquote".to_string(),
            other => format!(".{}", class_of(other)),
        };
        o.push_str(&css_rule(&sel, d));
    }
    o
}

/// 文書 → HTML の本体(`<body>` の中身)。
///
/// **合成しません。** 見た目はテンプレートが CSS として持つので、ここは
/// 意味だけを出します。
/// 註記の style_id → Web の役割の名前。
///
/// **AsciiDoc の註記は5つ**です(`NOTE:` `TIP:` `IMPORTANT:` `WARNING:`
/// `CAUTION:`)。docx には同じ物が無いので段落のスタイルとして往復し、
/// Web ではまとめて `aside` に出します。
fn 註記の種(名: &str) -> Option<&'static str> {
    Some(match 名 {
        "註記" | "NOTE" => "note",
        "こつ" | "TIP" => "tip",
        "大事" | "IMPORTANT" => "important",
        "警告" | "WARNING" => "warning",
        "注意" | "CAUTION" => "caution",
        _ => return None,
    })
}

/// 中身が段落として出る塊かどうか。
///
/// コードと字のままの塊は改行がそのまま意味を持つので `pre` に入れます。
/// 例・傍注・入れ物は文章なので、段落に割ります。
fn 中が段落(塊: Option<&str>) -> bool {
    matches!(塊, Some("example") | Some("sidebar") | Some("open"))
}

/// 註記の色。(左の線, 下地)
///
/// **見た目は要素が持ちます。** CSS を外して本文だけ持ち出しても、
/// 註記が註記に見えるようにするためです(Flet や Flutter と同じ考え方で、
/// 上から降ってくる規則に頼りません)。`class` も付けるので、
/// 揃えたい人はスタイルシートで上書きできます。
fn 註記の色(種: &str) -> (&'static str, &'static str) {
    match 種 {
        "tip" => ("#2e7d32", "#f1f8e9"),
        "important" => ("#6a1b9a", "#f3e5f5"),
        "warning" => ("#ef6c00", "#fff3e0"),
        "caution" => ("#c62828", "#ffebee"),
        _ => ("#1565c0", "#e3f2fd"),
    }
}

/// 註記の入れ物の見た目。左に色の線を引いて、下地を薄く敷きます。
fn 註記の飾り(種: &str) -> String {
    let (線, 下地) = 註記の色(種);
    format!(
        " style=\"border-left:4px solid {線};background:{下地};\
         padding:.6em 1em;margin:1em 0\""
    )
}

/// コードと字のままの塊の見た目。等幅で、折り返さずに横へ送ります。
const CODE_STYLE: &str = " style=\"font-family:ui-monospace,SFMono-Regular,\
    Consolas,'Noto Sans Mono',monospace;background:#f6f8fa;padding:.8em 1em;\
    border-radius:4px;overflow-x:auto\"";

/// 塊の開きの札。`[NOTE]` が前に付いていれば註記にします。
///
/// **見た目は札に書き込みます。** 中身だけを他所へ貼っても崩れません。
fn 塊の開き(塊: Option<&str>, 印: Option<&str>) -> String {
    if let Some(種) = 印.and_then(註記の種) {
        return format!(
            "<aside class=\"admonition {種}\" role=\"note\"{}><p style=\"margin:0\">",
            註記の飾り(種));
    }
    match 塊 {
        Some("literal") => format!("<pre{CODE_STYLE}>"),
        Some("example") => "<div class=\"example\" style=\"border:1px solid #d0d7de;\
             padding:.8em 1em;margin:1em 0;border-radius:4px\"><p style=\"margin:0\">".into(),
        Some("sidebar") => "<aside class=\"sidebar\" style=\"background:#f6f8fa;\
             padding:.8em 1em;margin:1em 0;border-radius:4px\"><p style=\"margin:0\">".into(),
        Some("open") => "<div class=\"open\" style=\"margin:1em 0\"><p style=\"margin:0\">".into(),
        Some("pass") => String::new(),
        _ => format!("<pre{CODE_STYLE}><code>"),
    }
}

/// 塊の閉じの札。開きと必ず対にします。
fn 塊の閉じ(塊: Option<&str>, 印: Option<&str>) -> String {
    if 印.and_then(註記の種).is_some() {
        return "</p></aside>\n".into();
    }
    match 塊 {
        Some("literal") => "</pre>\n".into(),
        Some("example") => "</p></div>\n".into(),
        Some("sidebar") => "</p></aside>\n".into(),
        Some("open") => "</p></div>\n".into(),
        Some("pass") => "\n".into(),
        _ => "</code></pre>\n".into(),
    }
}

pub fn body(doc: &Document) -> String {
    build(doc).0
}

/// 本文 → HTML と、一緒に書き出す画像。
///
/// 書き出した本文と、一緒に置く部品(名前と中身)。
pub type 本文と部品 = (String, Vec<(String, Arc<Vec<u8>>)>);

/// 画像を持つ文書を書き出すときはこちらを使います。[`body`] は HTML だけを
/// 返す入り口で、中身は同じです。
pub fn body_with_assets(doc: &Document) -> 本文と部品 {
    let (html, ctx) = build(doc);
    (html, ctx.assets)
}

fn build(doc: &Document) -> (String, Ctx) {
    let mut o = String::new();
    let mut ctx = Ctx::default();
    // 箇条書きは連続する段落をまとめます(HTML の ul / ol は入れ物なので)
    let mut list: Vec<ListKind> = Vec::new();
    // ラベル付きリスト(`dl`)の途中か
    let mut dl中 = false;
    // コードの塊(`pre`)の途中か
    let mut pre中 = false;
    // **いま開いている塊の種類。** `----` はコード、`....` は字のまま、
    // `====` は例、`****` は傍注、`--` は入れ物です。
    // 前は全部 `<pre><code>` に落としていたので、例も傍注も註記も
    // コードに見えていました(2026-08-25 に実物を流して見つけました)
    let mut 塊: Option<&'static str> = None;
    // `[NOTE]` のように、塊の直前の指定の行が言う種類
    let mut 次の塊の印: Option<String> = None;
    // 目次の行が続いている間(nav で包む)
    let mut 目次中 = false;
    let 題名あり = !doc.props.title.is_empty();
    // **開いている段を積んで持ちます。**(2026-08-25)
    // 前は「いま開いているリストは1つ」としか持っていなかったので、
    // `**` や `..` で深くした段が Web では平らになっていました。
    // 模型は `indent` で段を持っているのに、書き出しで捨てていたことになります。
    //
    // 深い段は*親の項目の中*に入れます。これが入れ子のリストの正しい形で、
    // 読み上げも折りたたみもここを見ます。
    let close = |o: &mut String, 段: &mut Vec<ListKind>| {
        while let Some(k) = 段.pop() {
            o.push_str("</li>\n");
            o.push_str(if k == ListKind::Bullet { "</ul>\n" } else { "</ol>\n" });
        }
    };

    // 表題の段落(ParaStyle::Title)があればそれが出ます。段落が無く文書の
    // 情報にだけ題名があるとき(docx から来た文書など)はここで出します
    let 題の段落 = doc
        .blocks
        .iter()
        .any(|b| matches!(b, Block::Para(p) if p.style == ParaStyle::Title));
    if 題名あり && !題の段落 {
        o.push_str(&format!("<h1 class=\"title\">{}</h1>\n", esc(&doc.props.title)));
    }
    for b in &doc.blocks {
        let Block::Para(p) = b else {
            // 表は素直に写します(見た目は CSS 側)
            if let Block::Table(t) = b {
                close(&mut o, &mut list);
                o.push_str("<table>\n");
                // 表の題は `caption`(本家と同じ)
                if let Some(名) = &t.title {
                    o.push_str(&format!("  <caption>{}</caption>\n", esc(名)));
                }
                // **桁の指定**(`[cols="1,3"]`)。Web では割合でそのまま書けます
                if !t.col_ratio.is_empty() {
                    let 和: f32 = t.col_ratio.iter().sum();
                    if 和 > 0.0 {
                        o.push_str("  <colgroup>");
                        for v in &t.col_ratio {
                            o.push_str(&format!("<col style=\"width:{:.1}%\">", v / 和 * 100.0));
                        }
                        o.push_str("</colgroup>\n");
                    }
                }
                for (ri, row) in t.rows.iter().enumerate() {
                    // 見出しの行は `thead` に入れます(Web の作法)
                    if t.header_row && ri == 0 {
                        o.push_str("  <thead>\n");
                    }
                    o.push_str("  <tr>");
                    for (k, cell) in row.iter().enumerate() {
                        // 結合の数え方は adoc の書き出しと同じ関数を使います
                        if cell.v_merge == crate::doc::VMerge::Continue {
                            continue; // 縦結合の続きはセルを書かない(上の rowspan が占める)
                        }
                        let mut at = String::new();
                        if cell.span() > 1 {
                            at.push_str(&format!(" colspan=\"{}\"", cell.span()));
                        }
                        if cell.v_merge == crate::doc::VMerge::Start {
                            let n = crate::adoc::vspan_of(t, ri, crate::adoc::grid_col(row, k));
                            if n > 1 {
                                at.push_str(&format!(" rowspan=\"{n}\""));
                            }
                        }
                        let inner: String = cell
                            .paragraphs
                            .iter()
                            .map(|q| runs_html(&q.runs, doc, &mut ctx))
                            .collect::<Vec<_>>()
                            .join("<br>");
                        // 見出しの行のセルは `th`(読み上げも検索も見出しとして扱う)
                        let 札 = if t.header_row && ri == 0 { "th" } else { "td" };
                        o.push_str(&format!("<{札}{at}>{inner}</{札}>"));
                    }
                    o.push_str("</tr>\n");
                    if t.header_row && ri == 0 {
                        o.push_str("  </thead>\n");
                    }
                }
                o.push_str("</table>\n");
            }
            continue;
        };
        let inner = format!("{}{}", imgs_html(p, &mut ctx), runs_html(&p.runs, doc, &mut ctx));
        // **作業のリスト**(`* [ ] やること` / `* [x] 済んだこと`)。
        // 読み手は種類を見分けているのに、書き出しが本文の字として
        // 出していました(印の `* [ ]` がそのままページに出ていた)
        let 作業 = p.style_id.as_deref() == Some("チェック");
        let (種, 段数, inner) = if 作業 {
            let 字: String = p.runs.iter().map(|r| r.text.as_str()).collect();
            let 星 = 字.chars().take_while(|c| *c == '*').count().max(1);
            let 残り = 字.trim_start_matches('*').trim_start();
            let (済, 本文) = if let Some(r) = 残り.strip_prefix("[x] ").or_else(|| 残り.strip_prefix("[X] ")) {
                (true, r)
            } else {
                (false, 残り.trim_start_matches("[ ] "))
            };
            // **見た目は札が持ちます。** 印だけ消して素の箇条書きにすると、
            // 済んだかどうかが読めなくなります
            let 箱 = if 済 {
                "<input type=\"checkbox\" checked disabled \
                 style=\"margin-right:.5em\">"
            } else {
                "<input type=\"checkbox\" disabled style=\"margin-right:.5em\">"
            };
            (ListKind::Bullet, 星 - 1, format!("{箱}{}", esc(本文)))
        } else {
            (p.list, p.indent as usize, inner)
        };
        if 種 != ListKind::None {
            let n = 段数;
            // 深すぎる段を閉じる(親の項目も一緒に閉じます)
            while list.len() > n + 1 {
                let k = list.pop().expect("段があるはず");
                o.push_str("</li>\n");
                o.push_str(if k == ListKind::Bullet { "</ul>\n" } else { "</ol>\n" });
            }
            if list.len() == n + 1 {
                o.push_str("</li>\n");          // 同じ段の前の項目を閉じる
                if list[n] != 種 {
                    let 古 = list.pop().expect("段があるはず");
                    o.push_str(if 古 == ListKind::Bullet { "</ul>\n" } else { "</ol>\n" });
                }
            }
            // **作業のリストは印を消します。** `list-style:none` を札に
            // 書き込むので、CSS が無くても点は出ません
            let 飾り = if 作業 { " style=\"list-style:none;padding-left:0\"" } else { "" };
            while list.len() < n + 1 {
                let 開き = if 種 == ListKind::Bullet {
                    format!("<ul{飾り}>\n")
                } else {
                    "<ol>\n".to_string()
                };
                o.push_str(&開き);
                list.push(種);
            }
            o.push_str(&format!("  <li>{inner}"));   // 閉じは次の項目か、段を閉じるとき
            continue;
        }
        // **目次は nav にまとめます**(2026-08-18)。前は普通の段落として
        // 並んでいたので、Web では本文と見分けが付きませんでした。
        // 中身は作り直した静的な字です(ページ番号は紙のもの)
        let 目次の行 = matches!(p.style, ParaStyle::Toc(_) | ParaStyle::Tof);
        if 目次の行 != 目次中 {
            o.push_str(if 目次の行 { "<nav class=\"toc\">\n" } else { "</nav>\n" });
            目次中 = 目次の行;
        }
        // **コードの塊は `pre` にします**(2026-08-18)。
        // `----` と `[source,python]` の行は、ここからここまでがコードだと
        // いう印です。文章ではないので、ページには出しません
        let 名 = p.style_id.as_deref();
        // **塊の区切りを見て、種類を覚えます。** 中身の段落は種類を
        // 持たないので、開いた印のほうから決めるしかありません
        if 名 == Some("塊の区切り") && 塊.is_none() {
            let 印: String = p.runs.iter().map(|r| r.text.as_str()).collect();
            塊 = Some(match 印.trim() {
                "...." => "literal",
                "====" => "example",
                "****" => "sidebar",
                "--" | "~~~~" => "open",
                "++++" => "pass",
                _ => "code",
            });
        }
        if 名 == Some("塊の中") {
            close(&mut o, &mut list);
            if dl中 {
                o.push_str("</dl>\n");
                dl中 = false;
            }
            let 字: String = p.runs.iter().map(|r| r.text.as_str()).collect();
            // **文章の塊では、空の行は段落の切れ目です。**
            // そのまま段落にすると `<p></p>` が出ます
            if 中が段落(塊) && 字.trim().is_empty() {
                continue;
            }
            if !pre中 {
                o.push_str(&塊の開き(塊, 次の塊の印.as_deref()));
                pre中 = true;
            } else {
                o.push_str(if 中が段落(塊) { "</p>\n<p>" } else { "\n" });
            }
            // **そのまま通す塊だけは逃がしません。** 生の HTML を書く
            // ための塊なので、逃がすと役に立ちません
            if 塊 == Some("pass") {
                o.push_str(&字);
            } else {
                o.push_str(&esc(&字));
            }
            continue;
        }
        if pre中 {
            o.push_str(&塊の閉じ(塊, 次の塊の印.as_deref()));
            pre中 = false;
            塊 = None;
            次の塊の印 = None;
        }
        // **横の区切り線は hr です。** 前は印の字がそのまま出ていました
        if 名 == Some("横の区切り線") {
            close(&mut o, &mut list);
            if dl中 {
                o.push_str("</dl>\n");
                dl中 = false;
            }
            o.push_str("<hr style=\"border:0;border-top:1px solid #d0d7de;margin:2em 0\">\n");
            continue;
        }
        // **註記は aside に役割を付けます。** 前はただの段落だったので、
        // Web では本文と見分けが付きませんでした
        if let Some(種) = 名.and_then(註記の種) {
            close(&mut o, &mut list);
            if dl中 {
                o.push_str("</dl>\n");
                dl中 = false;
            }
            o.push_str(&format!(
                "<aside class=\"admonition {種}\" role=\"note\"{}>{inner}</aside>\n",
                註記の飾り(種)));
            continue;
        }
        if 名 == Some("指定の行") {
            // `[NOTE]` のような指定は、次の塊の種類になります
            let 字: String = p.runs.iter().map(|r| r.text.as_str()).collect();
            let 中 = 字.trim().trim_start_matches('[').trim_end_matches(']');
            if 註記の種(中).is_some() {
                次の塊の印 = Some(中.to_string());
            }
            continue;
        }
        if 名 == Some("塊の区切り") {
            continue;
        }
        // **ラベル付きリスト**(`項目:: 値`)は `dl` / `dt` / `dd` に。
        // 続いている間は1つの `dl` にまとめます(2026-08-18)
        if p.style_id.as_deref() == Some("説明のリスト") {
            let 字: String = p.runs.iter().map(|r| r.text.as_str()).collect();
            if let Some((項, _)) = 字.split_once(":: ") {
                close(&mut o, &mut list);
                if !dl中 {
                    o.push_str("<dl>\n");
                    dl中 = true;
                }
                // **値の側は run のまま出します**(2026-08-18)。字だけを
                // 取ると、記入欄・リンク・ルビが消えます(申込用紙を
                // 通して見つけました)
                let 切れ目 = 項.len() + ":: ".len();
                let mut 値の並び: Vec<Run> = Vec::new();
                let mut at = 0usize;
                for r in &p.runs {
                    let 終 = at + r.text.len();
                    // **字を持たない run(記入欄・脚注)を落とさない。**
                    // 長さで切ると、切れ目にぴったり座っている物が消えます
                    if 終 <= 切れ目 && at < 切れ目 {
                        at = 終;
                        continue;
                    }
                    let mut r2 = r.clone();
                    if at < 切れ目 {
                        r2.text = r.text[切れ目 - at..].to_string();
                    }
                    値の並び.push(r2);
                    at = 終;
                }
                if let Some(first) = 値の並び.first_mut() {
                    first.text = first.text.trim_start().to_string();
                }
                let 値 = runs_html(&値の並び, doc, &mut ctx);
                o.push_str(&format!("  <dt>{}</dt><dd>{}</dd>\n", esc(項.trim()), 値));
                continue;
            }
        }
        if dl中 {
            o.push_str("</dl>\n");
            dl中 = false;
        }
        close(&mut o, &mut list);
        let tag = tag_of(p.style, 題名あり);
        // class は**スタイルの名前と改ページ**の2つが入ります
        let mut cls: Vec<String> = Vec::new();
        if p.style == ParaStyle::Title {
            cls.push("title".into());
        }
        if let ParaStyle::Toc(n) = p.style {
            cls.push(format!("toc{n}"));
        }
        if let Some(n) = &p.style_id {
            cls.push(class_of(n));
        }
        if p.page_break_before {
            cls.push("pagebreak".into());
        }
        let cls = if cls.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", cls.join(" "))
        };
        // しおりは**段落の id** にします。相互参照のリンクの先がこれです。
        // 2つ以上あるときは、余りを空の span で置きます(名前を落とさない)
        let id = match p.bookmarks.first() {
            Some(b) => format!(" id=\"{}\"", esc(b)),
            None => String::new(),
        };
        let 余り: String = p
            .bookmarks
            .iter()
            .skip(1)
            .map(|b| format!("<span id=\"{}\"></span>", esc(b)))
            .collect();
        o.push_str(&format!("<{tag}{id}{cls}>{余り}{inner}</{tag}>\n"));
    }
    if pre中 {
        o.push_str(&塊の閉じ(塊, 次の塊の印.as_deref()));
    }
    close(&mut o, &mut list);
    if dl中 {
        o.push_str("</dl>\n");
    }
    close(&mut o, &mut list);
    if 目次中 {
        o.push_str("</nav>\n");
    }
    // 脚注は最後に並べます。番号から本文へ戻れるようにします
    if !ctx.notes.is_empty() {
        o.push_str("<ol class=\"footnotes\">\n");
        for (i, t) in ctx.notes.iter().enumerate() {
            let n = i + 1;
            o.push_str(&format!(
                "  <li id=\"fn{n}\">{} <a href=\"#fnref{n}\">↩</a></li>\n",
                esc(t)
            ));
        }
        o.push_str("</ol>\n");
    }
    (o, ctx)
}

/// 文書とテンプレート → 1枚の HTML(CSS は `<style>` に入れます)。
///
/// テンプレートに `[送り先]` があり、文書に記入欄があれば、本文ごと
/// `<form>` で包んで送るボタンを付けます(**アプリの形**)。
pub fn page(doc: &Document, th: &Theme) -> Page {
    let css = css(th, !doc.props.title.is_empty());
    let title = if doc.props.title.is_empty() {
        "officework".to_string()
    } else {
        doc.props.title.clone()
    };
    let (inner, ctx) = build(doc);
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"ja\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<style>\n{css}</style>\n</head>\n<body>\n{}</body>\n</html>\n",
        esc(&title),
        wrap_form(inner, doc, th)
    );
    Page { html, css, assets: ctx.assets }
}

/// 記入欄があり、送り先も決まっていれば `<form>` で包みます。
///
/// **どちらか欠けたら包みません。** 送り先の無い form は押しても何も
/// 起きないので、出来ないことを出来るように見せないためです。
fn wrap_form(inner: String, doc: &Document, th: &Theme) -> String {
    let Some(sub) = &th.submit else { return inner };
    if sub.action.is_empty() || fields(doc).is_empty() {
        return inner;
    }
    format!(
        "<form action=\"{}\" method=\"{}\">\n{inner}<p><button type=\"submit\">{}</button></p>\n</form>\n",
        esc(&sub.action),
        esc(&sub.method),
        esc(&sub.label)
    )
}
