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

use crate::doc::{Align, Block, Document, ListKind, ParaStyle, Run};
use crate::theme::{StyleDef, Theme};

/// 出来上がりの HTML。`css` は別ファイルにも埋め込みにも使えます。
pub struct Page {
    pub html: String,
    pub css: String,
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
fn run_html(r: &Run) -> String {
    let mut s = esc(&r.text);
    if let Some(name) = &r.fmt.style_id {
        s = format!("<span class=\"{}\">{s}</span>", class_of(name));
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

fn runs_html(runs: &[Run]) -> String {
    runs.iter().map(run_html).collect()
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
pub fn body(doc: &Document) -> String {
    let mut o = String::new();
    // 箇条書きは連続する段落をまとめます(HTML の ul / ol は入れ物なので)
    let mut list: Option<ListKind> = None;
    let 題名あり = !doc.props.title.is_empty();
    let close = |o: &mut String, list: &mut Option<ListKind>| {
        match list.take() {
            Some(ListKind::Bullet) => o.push_str("</ul>\n"),
            Some(ListKind::Number) => o.push_str("</ol>\n"),
            _ => {}
        };
    };

    if !doc.props.title.is_empty() {
        o.push_str(&format!("<h1 class=\"title\">{}</h1>\n", esc(&doc.props.title)));
    }
    for b in &doc.blocks {
        let Block::Para(p) = b else {
            // 表は素直に写します(見た目は CSS 側)
            if let Block::Table(t) = b {
                close(&mut o, &mut list);
                o.push_str("<table>\n");
                for row in &t.rows {
                    o.push_str("  <tr>");
                    for cell in row {
                        let inner: String = cell
                            .paragraphs
                            .iter()
                            .map(|q| runs_html(&q.runs))
                            .collect::<Vec<_>>()
                            .join("<br>");
                        o.push_str(&format!("<td>{inner}</td>"));
                    }
                    o.push_str("</tr>\n");
                }
                o.push_str("</table>\n");
            }
            continue;
        };
        if p.list != ListKind::None {
            if list != Some(p.list) {
                close(&mut o, &mut list);
                o.push_str(if p.list == ListKind::Bullet { "<ul>\n" } else { "<ol>\n" });
                list = Some(p.list);
            }
            o.push_str(&format!("  <li>{}</li>\n", runs_html(&p.runs)));
            continue;
        }
        close(&mut o, &mut list);
        let tag = tag_of(p.style, 題名あり);
        let cls = p
            .style_id
            .as_ref()
            .map(|n| format!(" class=\"{}\"", class_of(n)))
            .unwrap_or_default();
        o.push_str(&format!("<{tag}{cls}>{}</{tag}>\n", runs_html(&p.runs)));
    }
    close(&mut o, &mut list);
    o
}

/// 文書とテンプレート → 1枚の HTML(CSS は `<style>` に入れます)。
pub fn page(doc: &Document, th: &Theme) -> Page {
    let css = css(th, !doc.props.title.is_empty());
    let title = if doc.props.title.is_empty() {
        "officework".to_string()
    } else {
        doc.props.title.clone()
    };
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"ja\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<style>\n{css}</style>\n</head>\n<body>\n{}</body>\n</html>\n",
        esc(&title),
        body(doc)
    );
    Page { html, css }
}
