//! **docx を書く。** こちらが作り直す部品以外は原本から持ち越す。

use std::io::{Cursor, Read, Seek, Write};

use kumihan::{Align, Block, Comment, Document, ListKind, ParaStyle,
              Paragraph, VMerge, PAGES_MARK, PAGE_MARK,
              TRK_DEL_E, TRK_DEL_S, TRK_INS_E, TRK_INS_S};
use quick_xml::events::Event;
use quick_xml::Writer;

use super::read::*;

pub(super) const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;

pub(super) const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

pub(super) const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";


/// **新規の文書の用紙。** 既定は A4 と余白 20mm(1134 twip)です。
///
/// 教師(Word の空)は余白 1440 twip(25.4mm)ですが、**余白だけは
/// 決め済みの 20mm** にします(writer の既定と1本の道)。
///
/// **用紙はいまの言語で決まります**(2026-08-30 発注者)。前は A4 の
/// 決め打ちでした。アメリカ英語のときだけレターになります — 同梱の
/// テンプレートの `[文書.en-us]` がそう言っています。
pub(super) fn default_sect() -> String {
    let p = kumihan::theme::default_theme().page.unwrap_or_default();
    sect_xml(&p)
}

/// **用紙の設定から `sectPr` を組む。**
///
/// 新しい節を作るときの元にします。前は空の `<w:sectPr></w:sectPr>` から
/// 始めていたので、途中の節が紙も余白も持たず、開いたソフトの既定
/// (英語圏の Word なら Letter・余白1インチ)で表示されていました。
/// 文書末の節も、後から書き替えた辺の余白しか持ちませんでした
/// (2026-08-30)。
///
/// 横長のときは `w:orient="landscape"` も書きます。Word の「印刷の向き」は
/// 縦横の寸法ではなくこの印を見ます。
pub fn sect_xml(p: &kumihan::PageSetup) -> String {
    let tw = |mm: f32| (mm * 1440.0 / 25.4).round() as i64;
    let muki = if p.w_mm > p.h_mm { r#" w:orient="landscape""# } else { "" };
    format!(
        concat!(
            r#"<w:sectPr><w:pgSz{} w:w="{}" w:h="{}"/>"#,
            r#"<w:pgMar w:top="{}" w:right="{}" w:bottom="{}" w:left="{}" "#,
            r#"w:header="851" w:footer="992" w:gutter="0"/></w:sectPr>"#
        ),
        muki,
        tw(p.w_mm),
        tw(p.h_mm),
        tw(p.top_mm),
        tw(p.right_mm),
        tw(p.bottom_mm),
        tw(p.left_mm),
    )
}

pub(super) const RNS_DOC: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// テンプレートのスタイル名を、docx の styleId にする。
///
/// 役割の決まった5つだけ Word の組み込みに寄せる。**読み手(Word・
/// python-docx・LibreOffice)が「見出し」として扱うのは styleId が
/// `Heading1` などのときだけ**なので、ここを日本語の名前のままにすると、
/// 見出しが本文として読まれて目次にも出ない。
/// 利用者が新しく作った名前は、そのまま styleId にする(日本語でも通る)。
/// **テンプレートのスタイル名 → docx の (styleId, w:name)。**
///
/// 「一覧に出す名前」と「保存で書かれる名前」を1箇所から取るための入り口です。
/// 2つに分けて書くと食い違います。
pub fn style_names(name: &str) -> (String, String) {
    let id = style_id_of(name);
    let n = style_name_of(name, &id);
    (id, n)
}

fn style_id_of(name: &str) -> String {
    match name {
        "本文" => "Normal".into(),
        "表題" => "Title".into(),
        "引用" => "Quote".into(),
        _ => match name.strip_prefix("見出し").and_then(|n| n.parse::<u8>().ok()) {
            Some(n) if (1..=9).contains(&n) => format!("Heading{n}"),
            _ => name.to_string(),
        },
    }
}

/// 読み手に見せるスタイルの名前。組み込みの分は Word の英語の名前でないと
/// 「組み込みの見出し」と同じ物だと見てもらえない
fn style_name_of(name: &str, id: &str) -> String {
    match id {
        "Normal" => "Normal".into(),
        "Title" => "Title".into(),
        "Quote" => "Quote".into(),
        _ => match id.strip_prefix("Heading") {
            Some(n) => format!("heading {n}"),
            None => name.to_string(),
        },
    }
}

/// 画面の言語の札を docx の `w:lang` の形に(`ja` → `ja-JP`)
///
/// 英語は `en-us` と `en-gb` の2つです(2026-08-30 発注者)。
/// 前は `en` をそのまま書いていました。短い形も札としては通りますが、
/// Word は国まで書きます。
fn doc_lang_tag(lang: &str) -> String {
    match lang {
        "ja" => "ja-JP".into(),
        "ko" => "ko-KR".into(),
        "zh" => "zh-CN".into(),
        "zh-tw" => "zh-TW".into(),
        "en-us" => "en-US".into(),
        "en-gb" => "en-GB".into(),
        other => other.to_string(),
    }
}

/// `w:lang` の `w:val` — **ラテン文字の言葉に使う言語**です。
///
/// Word は日本語の文書でもここに `en-US` と書きます。英語の文書のときだけ、
/// 選んだほうの英語にします
fn latin_lang_tag(lang: &str) -> &'static str {
    match lang {
        "en-gb" => "en-GB",
        _ => "en-US",
    }
}

/// テンプレート(`Theme`)から docx の `styles.xml` を作る。
///
/// **これが「テンプレートを docx に通す」の本体です**(2026-08-18)。
/// 本文の側には見た目を焼き付けません — HTML の書き出しが本文と CSS に
/// 分かれるのと同じ形で、docx でも本文と styles.xml に分かれます。
/// 受け取った人は Word のスタイルの一覧から見た目を変えられます。
fn styles_from_theme(theme: &kumihan::theme::Theme) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles xmlns:w=\"{W_NS}\">"
    );
    // 文書の既定(書体と大きさ)。テンプレートの `[文書]` の分。
    //
    // **書体は必ず書きます。** 空にすると Word が機械ごとに別の書体を
    // 当てるので、同じファイルが機械によって違って見えます
    // (2026-08-26 発注者「初期設定ができていないのでは」)。
    //
    // 名指しではなく**テーマ参照**にします。タイトルはゴシック、本文は
    // 明朝、という役ごとの分けができるのはテーマだけです
    // (2026-08-26 発注者「これは、書くべきです」)
    let mut dd = String::new();
    match &theme.font {
        // テンプレートが名指ししていれば、その1つを本文に当てます
        Some(f) => {
            let f = esc(f);
            dd.push_str(&format!(
                r#"<w:rFonts w:ascii="{f}" w:hAnsi="{f}" w:eastAsia="{f}" w:cs="{f}"/>"#
            ));
        }
        // **名指しが無ければテーマを参照します。** 役ごとに書体を変える
        // ためです — 直に名前を書くと、タイトルも本文も同じになります
        None => dd.push_str(crate::theme::MINOR_REF),
    }
    if let Some(pt) = theme.size_pt {
        let half = (pt * 2.0).round() as i64;
        dd.push_str(&format!(r#"<w:sz w:val="{half}"/><w:szCs w:val="{half}"/>"#));
    }
    // カーニングと言語。**教師(Word の空)が持っている物**で、無いと
    // 日本語の組みが本家と揃いません
    let lang = kumihan::font::default_language();
    dd.push_str(&format!(
        r#"<w:kern w:val="2"/><w:lang w:val="{}" w:eastAsia="{}" w:bidi="ar-SA"/>"#,
        latin_lang_tag(&lang),
        esc(&doc_lang_tag(&lang))
    ));
    if !dd.is_empty() {
        s.push_str(&format!(
            "<w:docDefaults><w:rPrDefault><w:rPr>{dd}</w:rPr></w:rPrDefault></w:docDefaults>"
        ));
    }
    // Normal は必ず要る(既定の段落スタイル)。テンプレートに「本文」が
    // あればその中身で、無ければ空で
    s.push_str(
        r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/>"#,
    );
    if let Some(d) = theme.styles.iter().find(|d| d.name == "本文") {
        s.push_str(&style_body_xml(d, None));
    }
    s.push_str("</w:style>");
    for d in &theme.styles {
        if d.name == "本文" {
            continue;
        }
        let id = style_id_of(&d.name);
        let name = style_name_of(&d.name, &id);
        // 見出しは outlineLvl を付ける。目次と見出しの折り畳みがこれを見る
        let lvl = id
            .strip_prefix("Heading")
            .and_then(|n| n.parse::<u8>().ok())
            .map(|n| n - 1);
        s.push_str(&format!(
            r#"<w:style w:type="paragraph" w:styleId="{}"><w:name w:val="{}"/><w:basedOn w:val="Normal"/>"#,
            esc(&id),
            esc(&name),
        ));
        s.push_str(&style_body_xml(d, lvl));
        // **タイトルと見出しはゴシック**(テーマの major)。テンプレートが
        // 書体を名指ししていなければ、役の参照を入れます
        // (2026-08-26 発注者「タイトルはゴシック、本文は明朝」)
        if d.font.is_none() && (id == "Title" || id.starts_with("Heading")) {
            let rpr = crate::theme::MAJOR_REF;
            match s.rfind("<w:rPr>") {
                // すでに rPr があれば、その先頭へ(スキーマの並びで rFonts は先)
                Some(i) if s[i..].contains("</w:rPr>") => s.insert_str(i + 7, rpr),
                _ => s.push_str(&format!("<w:rPr>{rpr}</w:rPr>")),
            }
        }
        s.push_str("</w:style>");
    }
    // **本文が名乗る styleId は、定義が無くても置きます。**
    // 定義が無いと読み手は Normal に落とすので、引用や見出しが本文と同じに
    // 見えます(2026-08-18 に python-docx で確かめて見つけました)。
    // テンプレートが何も言っていないスタイルなので中身は空 — 名乗りだけです
    for (id, name, lvl) in [
        ("Title", "Title", None),
        ("Quote", "Quote", None),
        ("Heading1", "heading 1", Some(0u8)),
        ("Heading2", "heading 2", Some(1)),
        ("Heading3", "heading 3", Some(2)),
    ] {
        if s.contains(&format!(r#"w:styleId="{id}""#)) {
            continue;
        }
        let outline = lvl
            .map(|n| format!(r#"<w:pPr><w:outlineLvl w:val="{n}"/></w:pPr>"#))
            .unwrap_or_default();
        s.push_str(&format!(
            r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{name}"/><w:basedOn w:val="Normal"/>{outline}</w:style>"#
        ));
    }
    s.push_str("</w:styles>");
    s
}

/// スタイル1つの中身(`w:pPr` と `w:rPr`)。並びはスキーマの順
fn style_body_xml(d: &kumihan::theme::StyleDef, outline: Option<u8>) -> String {
    let mut p = String::new();
    if let Some(sh) = &d.shade {
        p.push_str(&format!(
            r#"<w:shd w:val="clear" w:color="auto" w:fill="{}"/>"#,
            esc(sh)
        ));
    }
    let mut sp = String::new();
    if d.space_before_pt > 0.0 {
        sp.push_str(&format!(r#" w:before="{}""#, (d.space_before_pt * 20.0).round() as i64));
    }
    if d.space_after_pt > 0.0 {
        sp.push_str(&format!(r#" w:after="{}""#, (d.space_after_pt * 20.0).round() as i64));
    }
    if let Some(ls) = d.line_spacing.filter(|v| *v > 0.0) {
        sp.push_str(&format!(r#" w:line="{}" w:lineRule="auto""#, (ls * 240.0).round() as i64));
    }
    if !sp.is_empty() {
        p.push_str(&format!("<w:spacing{sp}/>"));
    }
    // 1行目の字下げ。**全角の文字数 × 字の大きさ** を twips に
    if let Some(ch) = d.first_line_chars.filter(|c| *c > 0.0) {
        let pt = d.size_pt.unwrap_or(10.5);
        let tw = (ch * pt * 20.0).round() as i64;
        p.push_str(&format!(r#"<w:ind w:firstLine="{tw}"/>"#));
    }
    if let Some(a) = d.align {
        p.push_str(&format!(r#"<w:jc w:val="{}"/>"#, a.as_docx()));
    }
    if let Some(n) = outline {
        p.push_str(&format!(r#"<w:outlineLvl w:val="{n}"/>"#));
    }
    let mut r = String::new();
    if let Some(f) = &d.font {
        let f = esc(f);
        r.push_str(&format!(
            r#"<w:rFonts w:ascii="{f}" w:hAnsi="{f}" w:eastAsia="{f}" w:cs="{f}"/>"#
        ));
    }
    if d.bold {
        r.push_str("<w:b/>");
    }
    if d.italic {
        r.push_str("<w:i/>");
    }
    if let Some(c) = &d.color {
        r.push_str(&format!(r#"<w:color w:val="{}"/>"#, esc(c)));
    }
    if let Some(pt) = d.size_pt {
        let half = (pt * 2.0).round() as i64;
        r.push_str(&format!(r#"<w:sz w:val="{half}"/><w:szCs w:val="{half}"/>"#));
    }
    if d.underline {
        r.push_str(r#"<w:u w:val="single"/>"#);
    }
    let mut out = String::new();
    if !p.is_empty() {
        out.push_str(&format!("<w:pPr>{p}</w:pPr>"));
    }
    if !r.is_empty() {
        out.push_str(&format!("<w:rPr>{r}</w:rPr>"));
    }
    out
}

/// 文書を document.xml の本体にする。
pub fn write_document_xml(doc: &Document) -> String {
    write_document_parts(doc).0
}

/// 本体と、このアプリで挿した画像(出て来る順)を返す。
pub fn write_document_parts(doc: &Document) -> (String, Vec<std::sync::Arc<Vec<u8>>>) {
    let (body, media, _) = write_document_full(doc);
    (body, media)
}

/// 本体・挿した画像・段落のコメント(参照した順)を返す。
/// 画像の番号(rIdJO1〜)とコメントの番号(1〜)はこの順で振られる。
// **引数を束ねません。** どれも別々の物で、まとめた構造体を作ると
// 「何を渡したか」が呼ぶ側から見えなくなります
#[allow(clippy::too_many_arguments)]
pub(super) fn write_para(w: &mut Writer<Cursor<Vec<u8>>>, p: &Paragraph,
        imgn: &mut usize, media: &mut Vec<std::sync::Arc<Vec<u8>>>,
        cmts: &mut Vec<Comment>, bmn: &mut usize,
        trkn: &mut usize, author: &str, base: f32) {
        use quick_xml::events::{BytesEnd, BytesStart as BS, BytesText};
        // ドロップキャップは Word の作法どおり
        // 「枠の段落(頭の1字・大きめ)+本文の段落」に割って書く
        if p.dropcap {
            if let Some(ch) = p.runs.first().and_then(|r| r.text.chars().next()) {
                let r0 = p.runs.first().unwrap();
                let cap_pt = ((r0.pt(base) * 2.8 * 2.0).round() as i32).to_string();
                let font_xml = r0.font.as_deref().map(|f| format!(
                    r#"<w:rFonts w:ascii="{f}" w:hAnsi="{f}" w:eastAsia="{f}"/>"#,
                    f = esc(f))).unwrap_or_default();
                let _ = w.get_mut().write_all(format!(
                    concat!(
                        r#"<w:p><w:pPr><w:framePr w:dropCap="drop" w:lines="2" "#,
                        r#"w:wrap="around" w:vAnchor="text" w:hAnchor="text"/></w:pPr>"#,
                        r#"<w:r><w:rPr>{font}<w:sz w:val="{sz}"/></w:rPr>"#,
                        r#"<w:t xml:space="preserve">{t}</w:t></w:r></w:p>"#
                    ),
                    font = font_xml, sz = cap_pt, t = esc(&ch.to_string())
                ).as_bytes());
                let mut rest = p.clone();
                rest.dropcap = false;
                if let Some(r) = rest.runs.first_mut() {
                    r.text = r.text[ch.len_utf8()..].to_string();
                }
                write_para(w, &rest, imgn, media, cmts, bmn, trkn, author, base);
                return;
            }
        }
        w.write_event(Event::Start(BS::new("w:p"))).unwrap();
        // 段落の性質。既定のものは書かない — 余計な指定を増やさない
        let has_ppr = p.align != Align::Left
            || p.page_break_before
            || p.list != ListKind::None
            || p.indent > 0
            || p.first_line_twips != 0
            || (p.spacing() - 1.0).abs() > 0.001
            || p.shade.is_some()
            || p.boxed
            // 節の区切りだけを持つ段落もある(区切り用の空段落)。
            // ここを足し忘れると pPr ごと書かれず、**区切りが黙って消える**
            || p.sect.is_some()
            || p.style != ParaStyle::Body
            // **段落の前後の空き**(2026-08-15)。ここを足さないと pPr ごと
            // 書かれず、空きだけを持つ段落で黙って消える — 上の
            // 「区切りが黙って消える」と同じ抜け方
            || p.space_before_pt > 0.0
            || p.space_after_pt > 0.0
            // **行の高さを書くときも pPr が要ります。** これを足さないと、
            // 何も飾りの無い普通の段落だけ pPr ごと書かれず、行の高さが
            // 入りません(2026-08-29 に 200 段落の文書で、見出し1つにしか
            // 入っていないのを見て気づきました)
            || SARA.with(|c| c.get())
            || p.style_id.is_some();
        if has_ppr {
            w.write_event(Event::Start(BS::new("w:pPr"))).unwrap();
            // 段落のスタイル(pPr の先頭に置く — スキーマの並び)。
            // **原文の styleId が第一** — 役割を知らない名前も、日本語版
            // Word の "1" も、読んだままを返す(2026-08-12 発注者確定)
            if let Some(id) = &p.style_id {
                let mut st = BS::new("w:pStyle");
                st.push_attribute(("w:val", id.as_str()));
                w.write_event(Event::Empty(st)).unwrap();
            } else {
                match p.style {
                    ParaStyle::Heading(n) => {
                        let mut st = BS::new("w:pStyle");
                        st.push_attribute(("w:val", format!("Heading{n}").as_str()));
                        w.write_event(Event::Empty(st)).unwrap();
                    }
                    ParaStyle::Toc(n) => {
                        let mut st = BS::new("w:pStyle");
                        st.push_attribute(("w:val", format!("TOC{n}").as_str()));
                        w.write_event(Event::Empty(st)).unwrap();
                    }
                    ParaStyle::Tof => {
                        let mut st = BS::new("w:pStyle");
                        st.push_attribute(("w:val", "TableofFigures"));
                        w.write_event(Event::Empty(st)).unwrap();
                    }
                    ParaStyle::Quote => {
                        let mut st = BS::new("w:pStyle");
                        st.push_attribute(("w:val", "Quote"));
                        w.write_event(Event::Empty(st)).unwrap();
                    }
                    // 文書の表題。Word の「表題」(Title)に当たる
                    ParaStyle::Title => {
                        let mut st = BS::new("w:pStyle");
                        st.push_attribute(("w:val", "Title"));
                        w.write_event(Event::Empty(st)).unwrap();
                    }
                    ParaStyle::Body => {}
                }
            }
            if p.page_break_before {
                w.write_event(Event::Empty(BS::new("w:pageBreakBefore"))).unwrap();
            }
            // 囲み枠(4辺とも同じ細線)
            if p.boxed {
                w.write_event(Event::Start(BS::new("w:pBdr"))).unwrap();
                for side in ["top", "left", "bottom", "right"] {
                    let tag = format!("w:{side}");
                    let mut b = BS::new(tag.as_str());
                    b.push_attribute(("w:val", "single"));
                    b.push_attribute(("w:sz", "4"));
                    b.push_attribute(("w:space", "4"));
                    b.push_attribute(("w:color", "000000"));
                    w.write_event(Event::Empty(b)).unwrap();
                }
                w.write_event(Event::End(BytesEnd::new("w:pBdr"))).unwrap();
            }
            // 背景色
            if let Some(c) = &p.shade {
                let mut sh = BS::new("w:shd");
                sh.push_attribute(("w:val", "clear"));
                sh.push_attribute(("w:color", "auto"));
                sh.push_attribute(("w:fill", c.as_str()));
                w.write_event(Event::Empty(sh)).unwrap();
            }
            if p.list != ListKind::None {
                w.write_event(Event::Start(BS::new("w:numPr"))).unwrap();
                let mut lv = BS::new("w:ilvl");
                lv.push_attribute(("w:val", p.indent.min(8).to_string().as_str()));
                w.write_event(Event::Empty(lv)).unwrap();
                let mut id = BS::new("w:numId");
                id.push_attribute(("w:val", if p.list == ListKind::Number { "2" } else { "1" }));
                w.write_event(Event::Empty(id)).unwrap();
                w.write_event(Event::End(BytesEnd::new("w:numPr"))).unwrap();
            }
            if p.indent > 0 || p.left_twips > 0 || p.first_line_twips != 0 {
                let mut ind = BS::new("w:ind");
                // **読んだ twip をそのまま返します**(2026-08-30)。段数へ
                // 丸めてから書き戻すと、開いて保存しただけで字下げが
                // 動きます(1文字が2文字になる)
                if p.left_twips > 0 {
                    ind.push_attribute(("w:left", p.left_twips.to_string().as_str()));
                } else if p.indent > 0 {
                    ind.push_attribute(("w:left", (p.indent as u32 * 420).to_string().as_str()));
                }
                // 1行目の字下げ(twip のまま往復)。負はぶら下げ
                if p.first_line_twips > 0 {
                    ind.push_attribute(("w:firstLine", p.first_line_twips.to_string().as_str()));
                } else if p.first_line_twips < 0 {
                    ind.push_attribute(("w:hanging", (-p.first_line_twips).to_string().as_str()));
                }
                w.write_event(Event::Empty(ind)).unwrap();
            }
            // 行間と**段落の前後の空き**。前は w:line だけ書いていたので、
            // Word の文書を開いて保存すると before / after が黙って消えていた
            // (2026-08-15)。twips = pt × 20
            let has_line_spacing = (p.spacing() - 1.0).abs() > 0.001;
            // **原本が高さそのものを言っていたら、そのまま書き戻します。**
            // 倍率に直して書くと、開き直したときに別の数になります
            let before = (p.space_before_pt * 20.0).round() as u32;
            let after = (p.space_after_pt * 20.0).round() as u32;
            // **新しく作った文書には行の高さを明に書きます。**
            //
            // 書かないと、開いた側が自分の既定を当てます。Word の「1行」は
            // 字の大きさの 1.15 倍ほど、こちらは 10.5pt に対して 6.4mm
            // (1.73 倍)なので、同じ文書が別の頁数に折れます。200 段落の
            // 文書で、こちらは5ページ・LibreOffice は3ページでした
            // (2026-08-29 に実際に変換して分かりました)。頁数がずれると、
            // ページに貼り付く図形の置き場も、目次の頁番号も狂います。
            //
            // **開いた元がある文書では書きません。** 原本が「1.5行」と
            // 言っているのを、こちらの mm に置き替えるのは書き替えです。
            // **倍率の指定がある段落は触りません。** 絶対の高さで書くと
            // 「1.5倍」という指定そのものが消え、開き直すと別の数になります
            // (2026-08-29 に往復の試験が落ちて気づきました)
            let gyou_tw = (SARA.with(|c| c.get()) && !has_line_spacing).then(|| {
                let lh = kumihan::LINE_MM * p.spacing() * kumihan::head_scale_of(p.style);
                (lh * 72.0 * 20.0 / 25.4).round() as u32
            });
            if has_line_spacing || before > 0 || after > 0 || gyou_tw.is_some()
                || p.line_pt.is_some()
            {
                let mut sp = BS::new("w:spacing");
                if before > 0 {
                    sp.push_attribute(("w:before", before.to_string().as_str()));
                }
                if after > 0 {
                    sp.push_attribute(("w:after", after.to_string().as_str()));
                }
                if let Some((pt, chodo)) = p.line_pt {
                    sp.push_attribute(("w:line", ((pt * 20.0).round() as i64).to_string().as_str()));
                    sp.push_attribute(("w:lineRule", if chodo { "exact" } else { "atLeast" }));
                } else if let Some(tw) = gyou_tw {
                    // **`atLeast`(この高さ以上)にします。** `exact` だと、
                    // 段落の中に大きい字があるとき Word が字を切ります。
                    // 普通の本文では natural(字の 1.15 倍ほど)より
                    // こちらの方が高いので、同じ高さになります
                    sp.push_attribute(("w:line", tw.to_string().as_str()));
                    sp.push_attribute(("w:lineRule", "atLeast"));
                } else if has_line_spacing {
                    sp.push_attribute(("w:line", ((p.spacing() * 240.0).round() as u32).to_string().as_str()));
                    sp.push_attribute(("w:lineRule", "auto"));
                }
                w.write_event(Event::Empty(sp)).unwrap();
            }
            if p.align != Align::Left {
                let mut jc = BS::new("w:jc");
                jc.push_attribute(("w:val", p.align.as_docx()));
                w.write_event(Event::Empty(jc)).unwrap();
            }
            // 見出しの階層。スタイル定義(styles.xml)が無い文書でも
            // 見出しとして扱われるように、outlineLvl も付けておく
            if let ParaStyle::Heading(n) = p.style {
                let mut ol = BS::new("w:outlineLvl");
                ol.push_attribute(("w:val", (n - 1).to_string().as_str()));
                w.write_event(Event::Empty(ol)).unwrap();
            }
            // 節の区切りを原文のまま返す。**pPr の一番後ろ**に置く —
            // スキーマ(CT_PPr)で sectPr は rPr の次、pPrChange の前と
            // 決まっていて、順を守らないと Word が開けない
            if let Some(sb) = &p.sect {
                let _ = w.get_mut().write_all(sb.raw.as_bytes());
            }
            w.write_event(Event::End(BytesEnd::new("w:pPr"))).unwrap();
        }
        // しおり。範囲は段落まるごと。名前は読んだものを返す(REF が指す)
        let bm0 = *bmn;
        for name in &p.bookmarks {
            *bmn += 1;
            let _ = w.get_mut().write_all(format!(
                r#"<w:bookmarkStart w:id="{}" w:name="{}"/>"#, *bmn, esc(name)).as_bytes());
        }
        // 段落のコメント。範囲は段落まるごと(段落単位の粒度)。
        // 番号は文書を通しで振り、comments.xml 側と同じ順で並ぶ
        let cmt_ids: Vec<usize> = p.comments.iter()
            .map(|c| {
                cmts.push(c.clone());
                cmts.len()
            })
            .collect();
        for id in &cmt_ids {
            let _ = w.get_mut().write_all(
                format!(r#"<w:commentRangeStart w:id="{id}"/>"#).as_bytes());
        }
        // 読めなかった要素(画像など)を原文のまま返す。
        // 段落の並びの中の位置は失われ、末尾に戻る(正直な限界)
        for a in &p.anchors {
            // anchors は宣言付きの <w:r>…</w:r> を丸ごと持っている
            let _ = w.get_mut().write_all(a.as_bytes());
        }
        for run in &p.runs {
            // 脚注・文末脚注の印。**字を持たない run** なので、下の
            // 「空の run は飛ばす」より先に書く。docx は印を run の中に置き、
            // 位置がそのまま「どの語に付いた注か」を表す
            if let Some(fr) = &run.fmt.footnote {
                let tag = if fr.endnote { "endnoteReference" } else { "footnoteReference" };
                let style = if fr.endnote { "EndnoteReference" } else { "FootnoteReference" };
                let _ = w.get_mut().write_all(format!(
                    concat!(r#"<w:r><w:rPr><w:rStyle w:val="{style}"/></w:rPr>"#,
                            r#"<w:{tag} w:id="{id}"/></w:r>"#),
                    style = style, tag = tag, id = esc(&fr.id)).as_bytes());
                continue;
            }
            if run.text.is_empty() { continue }
            // 相互参照はフィールドとして書く(見えている値をキャッシュに持つ)
            if let Some(rf) = &run.fmt.field {
                let instr = if rf.page {
                    format!(" PAGEREF {} \\h ", rf.name)
                } else {
                    format!(" REF {} \\h ", rf.name)
                };
                let b = if run.fmt.bold { "<w:b/>" } else { "" };
                let color = run.fmt.color.as_deref()
                    .map(|c| format!(r#"<w:color w:val="{c}"/>"#))
                    .unwrap_or_default();
                // 大きさは指定のある run だけ書く(無指定を焼き込まない)
                let sz_xml = run.size_pt
                    .map(|pt| format!(r#"<w:sz w:val="{}"/>"#, (pt * 2.0).round() as i32))
                    .unwrap_or_default();
                let _ = w.get_mut().write_all(format!(
                    concat!(
                        r#"<w:fldSimple w:instr="{instr}"><w:r><w:rPr>{b}{color}{sz}"#,
                        r#"</w:rPr>"#,
                        r#"<w:t xml:space="preserve">{t}</w:t></w:r></w:fldSimple>"#
                    ),
                    instr = esc(&instr),
                    b = b,
                    color = color,
                    sz = sz_xml,
                    t = esc(&run.text),
                ).as_bytes());
                continue;
            }
            // ページ番号・ページ数の印はフィールドとして書く
            // (印の前後で run を割る)。中の「1」は開く側が計算し直す種
            // 0=普通 1=挿入(w:ins) 2=削除(w:del)。変更履歴の印で切り替える
            let mut chunks: Vec<(u8, Option<char>, String)> =
                vec![(0, None, String::new())];
            let mut mode = 0u8;
            for ch in run.text.chars() {
                match ch {
                    TRK_INS_S => { mode = 1; chunks.push((mode, None, String::new())); }
                    TRK_DEL_S => { mode = 2; chunks.push((mode, None, String::new())); }
                    TRK_INS_E | TRK_DEL_E => {
                        mode = 0;
                        chunks.push((mode, None, String::new()));
                    }
                    PAGE_MARK | PAGES_MARK => chunks.push((mode, Some(ch), String::new())),
                    _ => chunks.last_mut().unwrap().2.push(ch),
                }
            }
            for (mode, mark, chunk) in chunks {
            if let Some(mk) = mark {
                let instr = if mk == PAGE_MARK { " PAGE " } else { " NUMPAGES " };
                let _ = w.get_mut().write_all(format!(
                    r#"<w:fldSimple w:instr="{instr}"><w:r><w:t>1</w:t></w:r></w:fldSimple>"#
                ).as_bytes());
            }
            if chunk.is_empty() { continue }
            // 変更履歴。挿入・削除は w:ins / w:del で包む(著者つき)
            if mode != 0 {
                *trkn += 1;
                let tag = if mode == 1 { "ins" } else { "del" };
                let _ = w.get_mut().write_all(format!(
                    r#"<w:{tag} w:id="{}" w:author="{}">"#, *trkn, esc(author)
                ).as_bytes());
            }
            let ttag = if mode == 2 { "w:delText" } else { "w:t" };
            // 記入欄(コンテンツコントロール)。run を w:sdt で包む
            let sdt = if mode == 0 { run.fmt.sdt.as_deref() } else { None };
            if let Some(sd) = sdt {
                use kumihan::SdtKind as K;
                let mut pr = String::new();
                if !sd.alias.is_empty() {
                    pr.push_str(&format!(r#"<w:alias w:val="{}"/>"#, esc(&sd.alias)));
                }
                // うちだけの種類は印(jo:*)が要る。名前も付いていたら
                // 「jo:email:連絡先」に合成する(読む側の split_tag と対)
                let marker = sd.kind.as_tag();
                let tag = if sd.tag.is_empty() || sd.tag == marker {
                    marker.to_string()
                } else if marker.is_empty() {
                    sd.tag.clone()
                } else {
                    format!("{marker}:{}", sd.tag)
                };
                if !tag.is_empty() {
                    pr.push_str(&format!(r#"<w:tag w:val="{}"/>"#, esc(&tag)));
                }
                match sd.kind {
                    K::Combo | K::Dropdown => {
                        let tag = if sd.kind == K::Combo {
                            "w:comboBox"
                        } else {
                            "w:dropDownList"
                        };
                        pr.push_str(&format!("<{tag}>"));
                        for it in &sd.items {
                            pr.push_str(&format!(
                                r#"<w:listItem w:displayText="{0}" w:value="{0}"/>"#,
                                esc(it)
                            ));
                        }
                        pr.push_str(&format!("</{tag}>"));
                    }
                    K::Picture => pr.push_str("<w:picture/>"),
                    K::Date => pr.push_str("<w:date/>"),
                    // チェックは Word 2010 の拡張。素の w:text でも中身は残る
                    K::Checkbox | K::Text | K::Email | K::Phone | K::Complex
                    | K::Signature => pr.push_str("<w:text/>"),
                }
                let _ = w.get_mut().write_all(
                    format!("<w:sdt><w:sdtPr>{pr}</w:sdtPr><w:sdtContent>").as_bytes(),
                );
            }
            // ルビ。基底の run を w:ruby(rt + rubyBase)で包む(Word の作法)
            let ruby_rt = if mode == 0 {
                run.fmt.ruby.as_deref().filter(|t| !t.is_empty())
            } else {
                None
            };
            if let Some(rt) = ruby_rt {
                // ルビの寸法は無くても書く(hps が無いと Word がルビを出さない)。
                // 無指定の run は文書の既定で導く — run 自身の w:sz とは別の話
                let rpt = run.pt(base);
                let hps = rpt.round() as i32; // 半分の大きさ(半ポイント)
                let base_sz = (rpt * 2.0).round() as i32;
                let raise = (rpt * 2.0 * 0.9).round() as i32;
                let _ = w.get_mut().write_all(format!(
                    concat!(
                        // w:ruby は run の中(ECMA-376 §17.3.3.25 — 親は w:r)。
                        // 段落直下に置くと LibreOffice が基底ごと落とす(実測)
                        r#"<w:r><w:ruby><w:rubyPr><w:rubyAlign w:val="center"/>"#,
                        r#"<w:hps w:val="{hps}"/><w:hpsRaise w:val="{raise}"/>"#,
                        r#"<w:hpsBaseText w:val="{base}"/><w:lid w:val="ja-JP"/>"#,
                        r#"</w:rubyPr><w:rt><w:r><w:rPr>"#,
                        r#"<w:rFonts w:hint="eastAsia"/><w:sz w:val="{hps}"/>"#,
                        r#"</w:rPr><w:t xml:space="preserve">{rt}</w:t></w:r></w:rt>"#,
                        r#"<w:rubyBase>"#
                    ),
                    hps = hps, raise = raise, base = base_sz, rt = esc(rt)
                ).as_bytes());
            }
            // リンク。**読んだ的をそのまま返す** — 包まないと、開いて保存
            // しただけでリンクが黙って消える(2026-08-13 に踏んだ)
            let linked = run.fmt.link.as_deref().and_then(|u| {
                LINKS.with(|m| m.borrow().get(u).copied()).map(|i| (u.to_string(), i))
            });
            if let Some((_, i)) = &linked {
                let mut hl = BS::new("w:hyperlink");
                hl.push_attribute(("r:id", link_rid(*i).as_str()));
                w.write_event(Event::Start(hl)).unwrap();
            }
            w.write_event(Event::Start(BS::new("w:r"))).unwrap();
            w.write_event(Event::Start(BS::new("w:rPr"))).unwrap();
            // 文字スタイル。読んだ名前をそのまま返す(スキーマで rPr の先頭)。
            // 定義は styles.xml の持ち物(2026-08-12 発注者確定 — 捨てない)
            if let Some(s) = &run.fmt.style_id {
                let mut rs = BS::new("w:rStyle");
                rs.push_attribute(("w:val", s.as_str()));
                w.write_event(Event::Empty(rs)).unwrap();
            }
            // 書体は文書の設定なので、読んだものをそのまま返す。
            // 日本語は eastAsia が本体。ascii/hAnsi にも同じ名前を入れておかないと
            // Word が英数字だけ別の書体で出す
            if let Some(f) = &run.font {
                let mut rf = BS::new("w:rFonts");
                rf.push_attribute(("w:ascii", f.as_str()));
                rf.push_attribute(("w:hAnsi", f.as_str()));
                rf.push_attribute(("w:eastAsia", f.as_str()));
                w.write_event(Event::Empty(rf)).unwrap();
            }
            // 文字の書式。付いているものだけ書く
            if run.fmt.bold { w.write_event(Event::Empty(BS::new("w:b"))).unwrap() }
            if run.fmt.italic { w.write_event(Event::Empty(BS::new("w:i"))).unwrap() }
            if run.fmt.underline {
                let mut u = BS::new("w:u");
                u.push_attribute(("w:val", "single"));
                w.write_event(Event::Empty(u)).unwrap();
            }
            if run.fmt.strike { w.write_event(Event::Empty(BS::new("w:strike"))).unwrap() }
            if run.fmt.superscript || run.fmt.subscript {
                let mut va = BS::new("w:vertAlign");
                va.push_attribute(("w:val",
                    if run.fmt.superscript { "superscript" } else { "subscript" }));
                w.write_event(Event::Empty(va)).unwrap();
            }
            if let Some(h) = &run.fmt.highlight {
                let mut hl = BS::new("w:highlight");
                hl.push_attribute(("w:val", h.as_str()));
                w.write_event(Event::Empty(hl)).unwrap();
            }
            if let Some(c) = &run.fmt.color {
                let mut col = BS::new("w:color");
                col.push_attribute(("w:val", c.as_str()));
                w.write_event(Event::Empty(col)).unwrap();
            }
            // **指定のある run だけ w:sz を書く。** 常に書くと、無指定
            // (文書の既定に従う)が往復のたびに「10.5pt 指定」へ化ける
            // (2026-08-13、本家 python-docx との突き合わせで発覚した焼き付き)
            if let Some(pt) = run.size_pt {
                let mut sz = BS::new("w:sz");
                sz.push_attribute(("w:val",
                    format!("{}", (pt * 2.0).round() as i32).as_str()));
                w.write_event(Event::Empty(sz)).unwrap();
            }
            w.write_event(Event::End(BytesEnd::new("w:rPr"))).unwrap();
            for (i, seg) in chunk.split('\n').enumerate() {
                if i > 0 { w.write_event(Event::Empty(BS::new("w:br"))).unwrap(); }
                // タブは要素(w:tab)。w:t の中に生のタブを書くと Word が潰す
                for (j, part) in seg.split('\t').enumerate() {
                    if j > 0 { w.write_event(Event::Empty(BS::new("w:tab"))).unwrap(); }
                    if part.is_empty() { continue }
                    let mut t = BS::new(ttag);
                    t.push_attribute(("xml:space", "preserve"));
                    w.write_event(Event::Start(t)).unwrap();
                    w.write_event(Event::Text(BytesText::new(part))).unwrap();
                    w.write_event(Event::End(BytesEnd::new(ttag))).unwrap();
                }
            }
            w.write_event(Event::End(BytesEnd::new("w:r"))).unwrap();
            if ruby_rt.is_some() {
                let _ = w.get_mut().write_all(b"</w:rubyBase></w:ruby></w:r>");
            }
            if sdt.is_some() {
                let _ = w.get_mut().write_all(b"</w:sdtContent></w:sdt>");
            }
            if linked.is_some() {
                w.write_event(Event::End(BytesEnd::new("w:hyperlink"))).unwrap();
            }
            if mode != 0 {
                let tag = if mode == 1 { "ins" } else { "del" };
                let _ = w.get_mut().write_all(format!("</w:{tag}>").as_bytes());
            }
            }
        }
        // このアプリで挿した画像。部品(media・rels)は write_with が同じ番号で作る
        for im in &p.images_new {
            *imgn += 1;
            let n = *imgn;
            let (cx, cy) = ((im.w_mm * 36000.0) as i64, (im.h_mm * 36000.0) as i64);
            // **数式なら原文(LaTeX)を代替テキストに積む。** 絵は組んだ結果
            // でしかない — これが無いと、こちらで開き直しても直せなくなる。
            // 渡した先の Word では絵として見え、読み上げにも意味が伝わる
            let descr = match &im.tex {
                Some(t) => format!(
                    r#" descr="{}{}""#,
                    crate::read::TEX_SIRUSI,
                    crate::read::esc(t)
                ),
                None => String::new(),
            };
            let xml = format!(
                r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{n}" name="図{n}"{descr}/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{n}" name="図{n}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdJO{n}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
            );
            let _ = w.get_mut().write_all(xml.as_bytes());
            media.push(im.bytes.clone());
        }
        for id in &cmt_ids {
            let _ = w.get_mut().write_all(format!(
                r#"<w:commentRangeEnd w:id="{id}"/><w:r><w:commentReference w:id="{id}"/></w:r>"#
            ).as_bytes());
        }
        for i in 0..p.bookmarks.len() {
            let _ = w.get_mut().write_all(
                format!(r#"<w:bookmarkEnd w:id="{}"/>"#, bm0 + i + 1).as_bytes());
        }
        w.write_event(Event::End(BytesEnd::new("w:p"))).unwrap();
}

/// ヘッダー・フッターの部品(headerN.xml / footerN.xml)の中身を作る。
pub(super) fn hf_xml(hf: &kumihan::HeadFoot, footer: bool, base: f32) -> String {
    use quick_xml::events::{BytesEnd, BytesStart as BS};
    let root_name = if footer { "w:ftr" } else { "w:hdr" };
    let mut w = Writer::new(Cursor::new(Vec::new()));
    let mut root = BS::new(root_name);
    root.push_attribute(("xmlns:w", W_NS));
    root.push_attribute(("xmlns:r", RNS_DOC));
    w.write_event(Event::Start(root)).unwrap();
    // 画像の挿入・コメントはヘッダーには無い(集め先はここでは増えない)
    let (mut imgn, mut media, mut cmts, mut bmn, mut trkn) =
        (0usize, Vec::new(), Vec::new(), 0usize, 0usize);
    for p in &hf.paragraphs {
        write_para(&mut w, p, &mut imgn, &mut media, &mut cmts, &mut bmn, &mut trkn, "", base);
    }
    w.write_event(Event::End(BytesEnd::new(root_name))).unwrap();
    let body = String::from_utf8(w.into_inner().into_inner()).unwrap();
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n{body}")
}

/// 足した注を `footnotes.xml`(`endnotes.xml`)に織り込む。
///
/// **原本があるときは丸ごと作り直さない。** 仕切り線の定義や、こちらが
/// 模型に持っていない書式がそこにあるので、**閉じ札の直前へ差し込む**だけにする
/// (`sectPr` を原文のまま返すのと同じ作法)。
///
/// 原本が無いときだけ、仕切り線ごと新しく作る — Word は
/// `separator` と `continuationSeparator` が無いと注を出さない。
pub(super) fn notes_xml(orig: Option<&str>, add: &[&kumihan::Footnote], endnote: bool, base: f32) -> String {
    let tag = if endnote { "endnote" } else { "footnote" };
    let root = format!("w:{tag}s");
    let mut body = String::new();
    for n in add {
        let mut w = Writer::new(Cursor::new(Vec::new()));
        let (mut imgn, mut media, mut cmts, mut bmn, mut trkn) =
            (0usize, Vec::new(), Vec::new(), 0usize, 0usize);
        for p in &n.paragraphs {
            write_para(&mut w, p, &mut imgn, &mut media, &mut cmts, &mut bmn, &mut trkn, "", base);
        }
        let inner = String::from_utf8(w.into_inner().into_inner()).unwrap();
        body.push_str(&format!(
            "<w:{tag} w:id=\"{id}\">{inner}</w:{tag}>",
            tag = tag, id = esc(&n.id), inner = inner));
    }
    match orig {
        // 原本の閉じ札の直前へ差し込む(中身は1文字も触らない)
        Some(o) => match o.rfind(&format!("</{root}>")) {
            Some(at) => format!("{}{}{}", &o[..at], body, &o[at..]),
            None => o.to_string(),
        },
        None => {
            // 仕切り線の定義。**これが無いと Word は注を出さない**
            let sep = format!(
                concat!(
                    "<w:{tag} w:type=\"separator\" w:id=\"-1\"><w:p><w:r>",
                    "<w:separator/></w:r></w:p></w:{tag}>",
                    "<w:{tag} w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:r>",
                    "<w:continuationSeparator/></w:r></w:p></w:{tag}>",
                ),
                tag = tag);
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
                 <{root} xmlns:w=\"{W_NS}\">{sep}{body}</{root}>",
                root = root, sep = sep, body = body)
        }
    }
}


/// 本文に出てくるリンクの的を**文書の順**で集める(重複は最初の1つ)。
/// 書き手と関係(rels)が**同じ番号**を使うための一本道 — 別々に数えると
/// r:id が食い違い、Word が「修復」に入る。
pub(super) fn collect_links(doc: &Document) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let push = |p: &Paragraph, out: &mut Vec<String>| {
        for r in &p.runs {
            if let Some(u) = &r.fmt.link {
                if !out.iter().any(|x| x == u) {
                    out.push(u.clone());
                }
            }
        }
    };
    for b in &doc.blocks {
        match b {
            Block::Para(p) => push(p, &mut out),
            Block::Table(t) => {
                for row in &t.rows {
                    for c in row {
                        for p in &c.paragraphs {
                            push(p, &mut out);
                        }
                    }
                }
            }
        }
    }
    out
}

/// リンクの関係の Id(番号は collect_links の並び)
pub(super) fn link_rid(i: usize) -> String {
    format!("rIdJOlnk{}", i + 1)
}

thread_local! {
    /// 書いている間だけの「URL → 番号」。書き手(write_para)は段落しか
    /// 見えないので、文書ぜんたいで決まる番号をここから引く
    static LINKS: std::cell::RefCell<std::collections::BTreeMap<String, usize>> =
        std::cell::RefCell::new(Default::default());
}

thread_local! {
    /// **新しく作った文書か。** 真のとき、段落に行の高さを明に書きます。
    ///
    /// 開いた元がある文書では触りません — 原本を壊さないためです
    /// (2026-08-29。下の `GYOU` の説明を見てください)。
    static SARA: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn write_document_full(doc: &Document) -> (String, Vec<std::sync::Arc<Vec<u8>>>, Vec<Comment>) {
    use quick_xml::events::{BytesEnd, BytesStart as BS};
    // リンクの番号を敷く(この文書を書いている間だけ)
    LINKS.with(|m| {
        let mut m = m.borrow_mut();
        m.clear();
        for (i, u) in collect_links(doc).into_iter().enumerate() {
            m.insert(u, i);
        }
    });
    let mut w = Writer::new(Cursor::new(Vec::new()));

    let mut root = BS::new("w:document");
    root.push_attribute(("xmlns:w", W_NS));
    root.push_attribute(("xmlns:r", RNS_DOC));
    root.push_attribute(("xmlns:wp",
        "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"));
    root.push_attribute(("xmlns:a", "http://schemas.openxmlformats.org/drawingml/2006/main"));
    root.push_attribute(("xmlns:pic",
        "http://schemas.openxmlformats.org/drawingml/2006/picture"));
    w.write_event(Event::Start(root)).unwrap();
    // ページの色。w:body の前に置く(スキーマの並び)。
    // 見せるには settings.xml の displayBackgroundShape も要る(write_with が足す)
    if let Some(c) = &doc.page_color {
        let mut bg = BS::new("w:background");
        bg.push_attribute(("w:color", c.as_str()));
        w.write_event(Event::Empty(bg)).unwrap();
    }
    w.write_event(Event::Start(BS::new("w:body"))).unwrap();

    let mut imgn = 0usize;
    let mut media: Vec<std::sync::Arc<Vec<u8>>> = Vec::new();
    let mut cmts: Vec<Comment> = Vec::new();
    let mut bmn = 0usize;
    let mut trkn = 0usize;
    let author = doc.track_author.clone().unwrap_or_default();
    let base = doc.base_pt();
    for b in &doc.blocks {
        match b {
            Block::Para(p) => write_para(&mut w, p, &mut imgn, &mut media, &mut cmts, &mut bmn, &mut trkn, &author, base),
            Block::Table(t) => {
                w.write_event(Event::Start(BS::new("w:tbl"))).unwrap();
                // 罫線(事務様式は罫線が見えないと様式にならない)
                w.write_event(Event::Start(BS::new("w:tblPr"))).unwrap();
                // スタイル名(読んだ名前を返すだけ — 定義は styles.xml の持ち物)。
                // スキーマ(CT_TblPr)の並び: tblStyle → jc → tblBorders → tblLayout
                if let Some(st) = &t.style {
                    let mut e = BS::new("w:tblStyle");
                    e.push_attribute(("w:val", st.as_str()));
                    w.write_event(Event::Empty(e)).unwrap();
                }
                if let Some(a) = t.align {
                    let mut e = BS::new("w:jc");
                    e.push_attribute(("w:val", a.as_docx()));
                    w.write_event(Event::Empty(e)).unwrap();
                }
                // **読んだ辺をそのまま返します**(2026-08-30)。前は必ず
                // 6辺とも書いていたので、下線だけの様式を開いて保存すると
                // 枠だらけになりました
                let bd = t.borders;
                w.write_event(Event::Start(BS::new("w:tblBorders"))).unwrap();
                for (side, hiku) in [
                    ("top", bd.top), ("left", bd.left), ("bottom", bd.bottom),
                    ("right", bd.right), ("insideH", bd.inside_h), ("insideV", bd.inside_v),
                ] {
                    let tag = format!("w:{side}");
                    let mut e = BS::new(tag.as_str());
                    if hiku {
                        e.push_attribute(("w:val", "single"));
                        e.push_attribute(("w:sz", "4"));
                        e.push_attribute(("w:color", "000000"));
                    } else {
                        // **「引かない」と明に言います。** 黙って書かないと、
                        // 表のスタイルの罫線が出てきます
                        e.push_attribute(("w:val", "nil"));
                    }
                    w.write_event(Event::Empty(e)).unwrap();
                }
                w.write_event(Event::End(BytesEnd::new("w:tblBorders"))).unwrap();
                // 列幅の固定。既定(autofit)は書かない — docx の既定と同じ
                if t.fixed_layout {
                    let mut e = BS::new("w:tblLayout");
                    e.push_attribute(("w:type", "fixed"));
                    w.write_event(Event::Empty(e)).unwrap();
                }
                w.write_event(Event::End(BytesEnd::new("w:tblPr"))).unwrap();
                // 列幅を返す(読んだものを捨てると、保存で表の形が変わる)。
                // tblGrid は ECMA-376 の必須部品 — 幅の指定が無い(等分)表でも
                // **幅なしの gridCol を格子の列数ぶん書く**。省くと python-docx が
                // 表を読めない(2026-08-12 の突き合わせで発覚)
                w.write_event(Event::Start(BS::new("w:tblGrid"))).unwrap();
                if t.col_mm.is_empty() {
                    let cols = t
                        .rows
                        .iter()
                        .map(|r| r.iter().map(|c| c.span()).sum::<usize>())
                        .max()
                        .unwrap_or(1);
                    for _ in 0..cols {
                        w.write_event(Event::Empty(BS::new("w:gridCol"))).unwrap();
                    }
                } else {
                    for mm in &t.col_mm {
                        let mut g = BS::new("w:gridCol");
                        let tw = (mm * 20.0 * 72.0 / 25.4).round() as i64;
                        g.push_attribute(("w:w", tw.to_string().as_str()));
                        w.write_event(Event::Empty(g)).unwrap();
                    }
                }
                w.write_event(Event::End(BytesEnd::new("w:tblGrid"))).unwrap();
                for (ri, row) in t.rows.iter().enumerate() {
                    w.write_event(Event::Start(BS::new("w:tr"))).unwrap();
                    // 行の高さ。0 は「中身なり」なので何も書きません
                    let h = t.row_mm.get(ri).copied().unwrap_or(0.0);
                    if h > 0.0 {
                        w.write_event(Event::Start(BS::new("w:trPr"))).unwrap();
                        let mut e = BS::new("w:trHeight");
                        let tw = (h * 1440.0 / 25.4).round().max(1.0) as u32;
                        let tw = tw.to_string();
                        e.push_attribute(("w:val", tw.as_str()));
                        // **少なくともこの高さ**(atLeast)。中身が入り切らない
                        // ときに字を切らないための決めで、Word の既定と同じです
                        e.push_attribute(("w:hRule", "atLeast"));
                        w.write_event(Event::Empty(e)).unwrap();
                        w.write_event(Event::End(BytesEnd::new("w:trPr"))).unwrap();
                    }
                    // 格子の何列目かは、前のセルが呑んだ分も数えます
                    let mut ci = 0usize;
                    for cell in row {
                        w.write_event(Event::Start(BS::new("w:tc"))).unwrap();
                        // セル結合を返す(読んだものを捨てると様式の枠が壊れる)
                        // セルの幅。**格子(w:tblGrid)だけでは足りません** —
                        // Word も python-docx もセルごとの `w:tcW` を見ます
                        // (2026-08-27 に python-docx で開いて気づきました)
                        let cw: f32 = (0..cell.span())
                            .filter_map(|k| t.col_mm.get(ci + k))
                            .sum();
                        if cell.col_span > 1
                            || cell.v_merge != VMerge::None
                            || cell.valign != book::VAlign::Top
                            || cw > 0.0
                        {
                            w.write_event(Event::Start(BS::new("w:tcPr"))).unwrap();
                            if cw > 0.0 {
                                let mut e = BS::new("w:tcW");
                                let tw = (cw * 1440.0 / 25.4).round().max(1.0) as u32;
                                let tw = tw.to_string();
                                e.push_attribute(("w:w", tw.as_str()));
                                e.push_attribute(("w:type", "dxa"));
                                w.write_event(Event::Empty(e)).unwrap();
                            }
                            if cell.col_span > 1 {
                                let mut g = BS::new("w:gridSpan");
                                let v = cell.col_span.to_string();
                                g.push_attribute(("w:val", v.as_str()));
                                w.write_event(Event::Empty(g)).unwrap();
                            }
                            match cell.v_merge {
                                VMerge::Start => {
                                    let mut m = BS::new("w:vMerge");
                                    m.push_attribute(("w:val", "restart"));
                                    w.write_event(Event::Empty(m)).unwrap();
                                }
                                VMerge::Continue => {
                                    // val 無しが「続き」(docx の既定)
                                    w.write_event(Event::Empty(BS::new("w:vMerge"))).unwrap();
                                }
                                VMerge::None => {}
                            }
                            // 縦位置。**docx の既定は上揃え**なので、上のときは
                            // 何も書きません(書くと原本と差分が出ます)
                            if let Some(v) = match cell.valign {
                                book::VAlign::Middle => Some("center"),
                                book::VAlign::Bottom => Some("bottom"),
                                _ => None,
                            } {
                                let mut e = BS::new("w:vAlign");
                                e.push_attribute(("w:val", v));
                                w.write_event(Event::Empty(e)).unwrap();
                            }
                            w.write_event(Event::End(BytesEnd::new("w:tcPr"))).unwrap();
                        }
                        if cell.paragraphs.is_empty() {
                            write_para(&mut w, &Paragraph { line_spacing: 1.0, ..Default::default() },
                                 &mut imgn, &mut media, &mut cmts, &mut bmn, &mut trkn, &author, base);
                        } else {
                            for p in &cell.paragraphs {
                                write_para(&mut w, p, &mut imgn, &mut media, &mut cmts, &mut bmn, &mut trkn, &author, base)
                            }
                        }
                        w.write_event(Event::End(BytesEnd::new("w:tc"))).unwrap();
                        ci += cell.span();
                    }
                    w.write_event(Event::End(BytesEnd::new("w:tr"))).unwrap();
                }
                w.write_event(Event::End(BytesEnd::new("w:tbl"))).unwrap();
            }
        }
    }

    // 節の設定を原文のまま返す(用紙・余白・ヘッダーの参照)。
    // このアプリで足したヘッダー・フッターの参照が無ければ差し込む
    // (参照が無いと、部品を書いても表示されない)
    let mut sect = doc.sect_raw.clone();
    {
        let cur = sect.as_deref().unwrap_or("");
        let mut refs = String::new();
        // 透かしはヘッダーに入るので、透かしがあればヘッダーの参照も要る
        // (編集できないヘッダー(表入り)には差し込まないので、そこは除く)
        let need_hdr = !doc.header.paragraphs.is_empty()
            || (doc.watermark.as_deref().is_some_and(|t| !t.is_empty())
                && !(doc.header.paragraphs.is_empty() && doc.header.part.is_some()));
        if need_hdr && hf_ref(cur, "headerReference").is_none() {
            refs.push_str(r#"<w:headerReference w:type="default" r:id="rIdJOhdr"/>"#);
        }
        if !doc.footer.paragraphs.is_empty() && hf_ref(cur, "footerReference").is_none() {
            refs.push_str(r#"<w:footerReference w:type="default" r:id="rIdJOftr"/>"#);
        }
        if !refs.is_empty() {
            sect = Some(match sect {
                // 参照は sectPr の先頭に置く(スキーマの並び)
                Some(s) if s.contains("</w:sectPr>") => match s.find('>') {
                    Some(i) => format!("{}{}{}", &s[..i + 1], refs, &s[i + 1..]),
                    None => s,
                },
                // 無い・空(<w:sectPr/>)なら作る
                _ => format!("<w:sectPr>{refs}</w:sectPr>"),
            });
        }
    }
    // 縦書きの旗。sectPr の textDirection を旗に合わせて置き直す
    if doc.vertical {
        let mut s2 = sect.take().unwrap_or_else(|| "<w:sectPr></w:sectPr>".into());
        if !s2.contains("</w:sectPr>") {
            // <w:sectPr/> の形
            s2 = "<w:sectPr></w:sectPr>".into();
        }
        if !s2.contains("<w:textDirection") {
            if let Some(i) = s2.rfind("</w:sectPr>") {
                s2.insert_str(i, r#"<w:textDirection w:val="tbRl"/>"#);
            }
        }
        sect = Some(s2);
    } else if let Some(s2) = sect.as_mut() {
        if let Some(i) = s2.find("<w:textDirection") {
            if let Some(j) = s2[i..].find("/>") {
                s2.replace_range(i..i + j + 2, "");
            }
        }
    }
    // **用紙は必ず書きます**(2026-08-26 発注者「初期設定ができていないのでは」)。
    // 前は指定が無ければ `sectPr` ごと書かず、読む機械の既定に落ちていました
    // — 英語圏の Word なら Letter になります。同じファイルが国によって
    // 違う紙に組まれるので、A4 と余白を名指しします
    let sect = sect.unwrap_or_else(default_sect);
    let _ = w.get_mut().write_all(sect.as_bytes());
    w.write_event(Event::End(BytesEnd::new("w:body"))).unwrap();
    w.write_event(Event::End(BytesEnd::new("w:document"))).unwrap();
    let body = String::from_utf8(w.into_inner().into_inner()).unwrap();
    (format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n{body}"), media, cmts)
}

/// docx として書き出す(最小の OPC パッケージ)。
pub fn write<W: Write + Seek>(doc: &Document, dst: W) -> Result<(), String> {
    write_with(doc, None::<std::io::Cursor<Vec<u8>>>, dst)
}

/// 保存する。`original` に**開いた元のファイル**を渡すと、
/// こちらが作り直す `word/document.xml` 以外の部品
/// (画像の実体・スタイル・ヘッダー・フッター・設定)を**そのまま持ち越す**。
///
/// 渡さないと部品ごと消える — 「開いて保存したらロゴが消えた」は
/// 書類の事故として重いので、アプリからは必ず元を渡すこと。
/// 画像の中身から拡張子と content type を見分ける。
pub(super) fn image_kind(bytes: &[u8]) -> (&'static str, &'static str) {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        ("png", "image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        ("jpeg", "image/jpeg")
    } else {
        ("bin", "application/octet-stream")
    }
}

/// このアプリで足したスタイル(styles_new)を、styles.xml へ追記する形の
/// XML にする。**既に同じ styleId が居れば足さない**(開き直して保存、で
/// 二重に増えないため)。定義は最小 — 名前の名乗りだけで、見た目は
/// 直接書式が第一のまま。
/// 自作スタイルの見た目を `w:pPr` と `w:rPr` にする。
///
/// **三択(入・切・言わない)を守ります。** 言っていない物は書きません —
/// 元になるスタイルから受け継ぐという意味だからです。
/// 字と段落の見た目を1つの `<w:style>` の中身に。
///
/// **`w:pPr` は1つにまとめます。** 塗りと揃えを別々の `w:pPr` に書くと
/// Word が後ろの1つしか見ません。
fn style_look_xml_with(
    l: &kumihan::StyleLook,
    pl: &kumihan::StyleParaLook,
) -> String {
    let mut ppr = String::new();
    if let Some(a) = pl.align {
        let v = match a {
            kumihan::Align::Left => "left",
            kumihan::Align::Center => "center",
            kumihan::Align::Right => "right",
            kumihan::Align::Justify => "both",
            kumihan::Align::Distribute => "distribute",
        };
        ppr.push_str(&format!(r#"<w:jc w:val="{v}"/>"#));
    }
    // 空きと行間は1つの `w:spacing` にまとめます(schema の決め)
    let mut sp = String::new();
    if let Some(v) = pl.space_before_pt {
        sp.push_str(&format!(r#" w:before="{}""#, (v * 20.0).round() as i32));
    }
    if let Some(v) = pl.space_after_pt {
        sp.push_str(&format!(r#" w:after="{}""#, (v * 20.0).round() as i32));
    }
    if let Some(v) = pl.line_spacing {
        sp.push_str(&format!(
            r#" w:line="{}" w:lineRule="auto""#,
            (v * 240.0).round() as i32
        ));
    }
    if !sp.is_empty() {
        ppr.push_str(&format!("<w:spacing{sp}/>"));
    }
    // 字下げも1つの `w:ind` にまとめます。1段 = 全角2文字 = 480 twip
    let mut ind = String::new();
    if let Some(v) = pl.indent {
        ind.push_str(&format!(r#" w:left="{}""#, v as i32 * 480));
    }
    if let Some(v) = pl.first_line_twips {
        if v >= 0 {
            ind.push_str(&format!(r#" w:firstLine="{v}""#));
        } else {
            ind.push_str(&format!(r#" w:hanging="{}""#, -v));
        }
    }
    if !ind.is_empty() {
        ppr.push_str(&format!("<w:ind{ind}/>"));
    }
    if let Some(c) = &l.fill {
        ppr.push_str(&format!(
            r#"<w:shd w:val="clear" w:color="auto" w:fill="{}"/>"#,
            esc(c)
        ));
    }
    if l.is_empty() && ppr.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    if !ppr.is_empty() {
        out.push_str(&format!("<w:pPr>{ppr}</w:pPr>"));
    }
    let mut r = String::new();
    if let Some(f) = &l.font {
        let f = esc(f);
        r.push_str(&format!(
            r#"<w:rFonts w:ascii="{f}" w:hAnsi="{f}" w:eastAsia="{f}" w:cs="{f}"/>"#
        ));
    }
    for (on, tag) in [(l.bold, "b"), (l.italic, "i"), (l.strike, "strike")] {
        match on {
            Some(true) => r.push_str(&format!("<w:{tag}/>")),
            // **わざわざ切る**のと「言わない」は違います
            Some(false) => r.push_str(&format!(r#"<w:{tag} w:val="0"/>"#)),
            None => {}
        }
    }
    match l.underline {
        Some(true) => r.push_str(r#"<w:u w:val="single"/>"#),
        Some(false) => r.push_str(r#"<w:u w:val="none"/>"#),
        None => {}
    }
    if let Some(c) = &l.color {
        r.push_str(&format!(r#"<w:color w:val="{}"/>"#, esc(c)));
    }
    if let Some(pt) = l.size_pt {
        let half = (pt * 2.0).round() as i64;
        r.push_str(&format!(r#"<w:sz w:val="{half}"/><w:szCs w:val="{half}"/>"#));
    }
    if !r.is_empty() {
        out.push_str(&format!("<w:rPr>{r}</w:rPr>"));
    }
    out
}

/// スタイル定義の性質を XML に。**順は schema のとおり** —
/// `w:name` の次が `w:basedOn`、その後ろに一覧まわりの旗が並びます。
/// 順を崩すと Word が開けません。
fn style_props_xml(s: &kumihan::StyleInfo) -> String {
    let mut out = String::new();
    if let Some(b) = &s.based_on {
        out.push_str(&format!(r#"<w:basedOn w:val="{}"/>"#, esc(b)));
    }
    if let Some(p) = s.priority {
        out.push_str(&format!(r#"<w:uiPriority w:val="{p}"/>"#));
    }
    for (on, tag) in [
        (s.hidden, "w:semiHidden"),
        (s.unhide_when_used, "w:unhideWhenUsed"),
        (s.quick_style, "w:qFormat"),
        (s.locked, "w:locked"),
    ] {
        if on {
            out.push_str(&format!("<{tag}/>"));
        }
    }
    out
}

/// **原本のスタイルの性質を書き戻す。**
///
/// 据え置きが基本なので、`doc.styles` の中身が読んだときと変わっている
/// スタイルだけを差し替えます。触っていないスタイルは1字も動きません。
///
/// 差し替えるのは `w:name` の後ろの性質(`w:basedOn` `w:uiPriority`
/// `w:semiHidden` `w:unhideWhenUsed` `w:qFormat` `w:locked`)だけで、
/// 見た目(`w:rPr` `w:pPr`)には触りません。
fn patch_style_props(xml: &str, doc: &Document) -> String {
    // 読み直して、変わっている物を拾う
    let moto = crate::read::parse_styles(xml);
    let kawatta: Vec<&kumihan::StyleInfo> = doc
        .styles
        .iter()
        .filter(|s| {
            moto.iter().any(|m| {
                m.id == s.id
                    && (m.based_on != s.based_on
                        || m.hidden != s.hidden
                        || m.unhide_when_used != s.unhide_when_used
                        || m.locked != s.locked
                        || m.quick_style != s.quick_style
                        || m.priority != s.priority
                        || m.para != s.para)
            })
        })
        .collect();
    if kawatta.is_empty() {
        return xml.to_string();
    }
    let mut out = xml.to_string();
    for s in kawatta {
        let Some(i) = out.find(&format!("w:styleId=\"{}\"", esc(&s.id))) else { continue };
        // この `<w:style` の中身の始まりと終わり
        let Some(hajime) = out[..i].rfind("<w:style ") else { continue };
        let Some(atama) = out[i..].find('>').map(|e| i + e + 1) else { continue };
        let Some(owari) = out[atama..].find("</w:style>").map(|e| atama + e) else { continue };
        let naka = &out[atama..owari];
        // `w:name` は残し、性質だけ入れ替える
        let namae = naka
            .find("<w:name")
            .and_then(|n| naka[n..].find('>').map(|e| naka[n..n + e + 1].to_string()))
            .unwrap_or_default();
        let mut nokori = naka.to_string();
        for tag in ["w:basedOn", "w:uiPriority", "w:semiHidden", "w:unhideWhenUsed",
                    "w:qFormat", "w:locked"] {
            nokori = drop_empty_tag(&nokori, tag);
        }
        if !namae.is_empty() {
            nokori = nokori.replacen(&namae, "", 1);
        }
        // 段落の見た目を触っていれば、古い `w:pPr` を落として書き直します。
        // **字の見た目(`w:rPr`)には触りません** — 据え置きが基本です
        let moto_para = moto.iter().find(|m| m.id == s.id).map(|m| &m.para);
        if moto_para != Some(&s.para) {
            nokori = drop_block(&nokori, "w:pPr");
        }
        let mi = if moto_para != Some(&s.para) {
            style_look_xml_with(&kumihan::StyleLook::default(), &s.para)
        } else {
            String::new()
        };
        let atarashii = format!("{namae}{}{mi}{nokori}", style_props_xml(s));
        out.replace_range(atama..owari, &atarashii);
        let _ = hajime;
    }
    out
}

/// `<w:pPr>…</w:pPr>` を丸ごと1つ落とす
fn drop_block(xml: &str, tag: &str) -> String {
    let hiraki = format!("<{tag}");
    let toji = format!("</{tag}>");
    let Some(i) = xml.find(&hiraki) else { return xml.to_string() };
    let Some(e) = xml[i..].find('>').map(|e| i + e) else { return xml.to_string() };
    // 空要素(`<w:pPr/>`)なら、そこまでで終わり
    if xml[..e].ends_with('/') {
        return format!("{}{}", &xml[..i], &xml[e + 1..]);
    }
    match xml[e..].find(&toji) {
        Some(j) => format!("{}{}", &xml[..i], &xml[e + j + toji.len()..]),
        None => xml.to_string(),
    }
}

/// `<w:qFormat/>` や `<w:uiPriority w:val="9"/>` を1つ落とす
fn drop_empty_tag(xml: &str, tag: &str) -> String {
    let t = format!("<{tag}");
    match xml.find(&t) {
        Some(i) => match xml[i..].find('>') {
            Some(e) => format!("{}{}", &xml[..i], &xml[i + e + 1..]),
            None => xml.to_string(),
        },
        None => xml.to_string(),
    }
}

fn styles_new_xml(doc: &Document, existing: &str) -> String {
    let mut out = String::new();
    for s in &doc.styles_new {
        if existing.contains(&format!("w:styleId=\"{}\"", esc(&s.id))) {
            continue;
        }
        out.push_str(&format!(
            r#"<w:style w:type="{}" w:styleId="{}"><w:name w:val="{}"/>"#,
            esc(&s.kind),
            esc(&s.id),
            esc(&s.name),
        ));
        out.push_str(&style_props_xml(s));
        out.push_str(&style_look_xml_with(&s.look, &s.para));
        out.push_str("</w:style>");
    }
    out
}

pub fn write_with<R: Read + Seek, W: Write + Seek>(
    doc: &Document,
    original: Option<R>,
    dst: W,
) -> Result<(), String> {
    write_with_theme(doc, original, None, dst)
}

/// テンプレートつきで保存する。`theme` を渡すと、その中身から
/// `word/styles.xml` を作って入れる(渡さなければ今までどおり)。
///
/// **ネイティブ文書(.adoc)を docx で書き出すときに使います。** 本文は
/// 意味だけ、見た目はスタイル定義、という分け方をそのまま docx に移した
/// 形です。受け取った docx を保存し直すときは渡しません — 相手の
/// スタイル定義を、こちらのテンプレートで上書きしないためです。
pub fn write_with_theme<R: Read + Seek, W: Write + Seek>(
    doc: &Document,
    original: Option<R>,
    theme: Option<&kumihan::theme::Theme>,
    dst: W,
) -> Result<(), String> {
    let mut zip = zip::ZipWriter::new(dst);
    let opts: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // **図形は組む所が控えへ入れます**([`paper::doc_with_shapes`])。
    // docx の図形は「どの段落に留まるか」でページが決まり、ここは模型から
    // 作り直す道なので組み上がりを知りません。2026-08-29 まではここで
    // 1ページ目だけ差し込んでいましたが、2ページ目以降が違うページへ
    // 出るので、知っている側へ移しました
    // 開いた元があるかを、段落を書く所へ伝えます(行の高さの決めに使います)
    SARA.with(|c| c.set(original.is_none()));
    let (body, new_media, cmts_out) = write_document_full(doc);
    SARA.with(|c| c.set(false));
    // 今回こちらが作り直す部品の名前(これ以外の joimg は既存画像として持ち越す)
    let regen_media: Vec<String> = new_media.iter().enumerate()
        .map(|(i, m)| {
            let (ext, _) = image_kind(m);
            format!("word/media/joimg{}.{ext}", i + 1)
        })
        .collect();
    // ヘッダー・フッター。モデルに持っているもの(編集できたもの)だけ作り直す。
    // paragraphs が空 = 触っていない/持てなかった部品 → 原本のまま持ち越す。
    // 透かしはヘッダーの1段落目に VML として差し込む(Word の作法)
    let mut hdr_src = doc.header.clone();
    if let Some(text) = doc.watermark.as_deref().filter(|t| !t.is_empty()) {
        if hdr_src.paragraphs.is_empty() && hdr_src.part.is_some() {
            // 編集できないヘッダー(表入り)には差し込まない(壊すより見送る)
        } else {
            if hdr_src.paragraphs.is_empty() {
                hdr_src.paragraphs.push(Paragraph::default());
            }
            if let Some(wrapped) = wrap_with_ns(&watermark_vml(text), &Default::default()) {
                hdr_src.paragraphs[0].anchors.push(wrapped);
            }
        }
    }
    let hdr: Option<(String, String)> = (!hdr_src.paragraphs.is_empty()).then(|| (
        hdr_src.part.clone().unwrap_or_else(|| "word/johdr1.xml".to_string()),
        hf_xml(&hdr_src, false, doc.base_pt()),
    ));
    let ftr: Option<(String, String)> = (!doc.footer.paragraphs.is_empty()).then(|| (
        doc.footer.part.clone().unwrap_or_else(|| "word/joftr1.xml".to_string()),
        hf_xml(&doc.footer, true, doc.base_pt()),
    ));

    // このアプリで足した注。**あるときだけ**部品を書き直す
    // (無ければ原本の部品がそのまま持ち越される = 今までどおり)
    let notes_add: Vec<&kumihan::Footnote> =
        doc.footnotes.iter().filter(|n| n.added && !n.endnote).collect();
    let ends_add: Vec<&kumihan::Footnote> =
        doc.footnotes.iter().filter(|n| n.added && n.endnote).collect();
    let mut orig_notes: Option<String> = None;
    let mut orig_ends: Option<String> = None;

    // [Content_Types] と本文の rels は、挿した画像のぶんを織り込んで作り直す
    let mut orig_ct: Option<String> = None;
    let mut orig_rels: Option<String> = None;
    let mut orig_settings: Option<String> = None;
    let mut orig_core: Option<String> = None;
    let mut orig_root_rels: Option<String> = None;
    let mut orig_has_styles = false;
    let has_props = doc.props != Default::default();
    if let Some(src) = original {
        if let Ok(mut z) = zip::ZipArchive::new(src) {
            for i in 0..z.len() {
                let mut f = match z.by_index(i) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let name = f.name().to_string();
                // 本文だけがこちらの管轄。他の部品は原本のまま
                if name == "word/document.xml" {
                    continue;
                }
                // テンプレートを渡されたときは、原本のスタイル定義を
                // 持ち越さずこちらで作り直す(下の styles_from_theme)
                if name == "word/styles.xml" {
                    if theme.is_some() {
                        continue;
                    }
                    orig_has_styles = true; // 原本の定義を持ち越す(下で作らない)
                }
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_err() {
                    continue;
                }
                // このアプリで足したスタイルは、原本の styles.xml へ
                // **追記だけ**する(core.xml と同じ外科術 — 作り直さない)
                if name == "word/styles.xml" {
                    let s0 = String::from_utf8_lossy(&buf).to_string();
                    // 原本のスタイルは据え置きですが、**性質を触った物だけ**は
                    // 書き戻します(2026-08-28)。触っていない物は1字も動きません
                    let mut s = patch_style_props(&s0, doc);
                    let add = styles_new_xml(doc, &s);
                    if !add.is_empty() {
                        if let Some(pnt) = s.rfind("</w:styles>") {
                            s.insert_str(pnt, &add);
                        }
                    }
                    if s != s0 {
                        zip.start_file(name, opts).map_err(|e| e.to_string())?;
                        zip.write_all(s.as_bytes()).map_err(|e| e.to_string())?;
                        continue;
                    }
                }
                if name == "[Content_Types].xml" {
                    orig_ct = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                if name == "word/_rels/document.xml.rels" {
                    orig_rels = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                // 設定(ページの色・ハイフネーション)は織り込んで書き直す
                if name == "word/settings.xml" {
                    orig_settings = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                // 文書の情報。作成者などを織り込んで書き直す
                if name == "docProps/core.xml" {
                    orig_core = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                if name == "_rels/.rels" {
                    orig_root_rels = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                // コメントを持っているなら comments.xml はこちらが作り直す
                if name == "word/comments.xml" && !cmts_out.is_empty() {
                    continue;
                }
                // 注を足したなら、その部品はこちらが書き直す(原文は控えて
                // **差し込むだけ** — 仕切り線や持っていない書式を失わないため)
                if name == "word/footnotes.xml" && !notes_add.is_empty() {
                    orig_notes = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                if name == "word/endnotes.xml" && !ends_add.is_empty() {
                    orig_ends = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                // 今回作り直す画像の実体だけは持ち越さない(二重に持たない)。
                // 開き直した後の joimg は「既存の画像」なので、普通に持ち越す
                if regen_media.contains(&name) {
                    continue;
                }
                // 作り直すヘッダー・フッターの部品も持ち越さない(後で書く)
                if hdr.as_ref().is_some_and(|(n, _)| *n == name)
                    || ftr.as_ref().is_some_and(|(n, _)| *n == name)
                {
                    continue;
                }
                zip.start_file(name, opts).map_err(|e| e.to_string())?;
                zip.write_all(&buf).map_err(|e| e.to_string())?;
            }
        }
    }

    // [Content_Types]。挿した画像の拡張子とヘッダー・フッターの宣言が無ければ足す
    {
        let mut ct = orig_ct.unwrap_or_else(|| CONTENT_TYPES.to_string());
        let mut add = String::new();
        for m in &new_media {
            let (ext, ty) = image_kind(m);
            let decl = format!(r#"<Default Extension="{ext}" ContentType="{ty}"/>"#);
            if !ct.contains(&format!("Extension=\"{ext}\"")) && !add.contains(&decl) {
                add.push_str(&decl);
            }
        }
        for (name, ty) in [
            (hdr.as_ref().map(|(n, _)| n.as_str()),
             "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"),
            (ftr.as_ref().map(|(n, _)| n.as_str()),
             "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"),
            ((doc.page_color.is_some() || doc.hyphenate || doc.protection.is_some())
                .then_some("word/settings.xml"),
             "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"),
            ((!cmts_out.is_empty()).then_some("word/comments.xml"),
             "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"),
            ((!notes_add.is_empty()).then_some("word/footnotes.xml"),
             "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"),
            ((!ends_add.is_empty()).then_some("word/endnotes.xml"),
             "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"),
            ((has_props || orig_core.is_some()).then_some("docProps/core.xml"),
             "application/vnd.openxmlformats-package.core-properties+xml"),
            ((!orig_has_styles).then_some("word/styles.xml"),
             "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"),
            ((!orig_has_styles).then_some("word/theme/theme1.xml"),
             "application/vnd.openxmlformats-officedocument.theme+xml"),
        ] {
            let Some(name) = name else { continue };
            if !ct.contains(&format!("PartName=\"/{name}\"")) {
                add.push_str(&format!(
                    r#"<Override PartName="/{name}" ContentType="{ty}"/>"#));
            }
        }
        if !add.is_empty() {
            if let Some(p) = ct.rfind("</Types>") {
                ct.insert_str(p, &add);
            }
        }
        zip.start_file("[Content_Types].xml", opts).map_err(|e| e.to_string())?;
        zip.write_all(ct.as_bytes()).map_err(|e| e.to_string())?;
    }
    {
        let mut rr = orig_root_rels.unwrap_or_else(|| ROOT_RELS.to_string());
        if (has_props || orig_core.is_some())
            && !rr.contains("Target=\"docProps/core.xml\"")
        {
            if let Some(i) = rr.rfind("</Relationships>") {
                rr.insert_str(i, concat!(
                    r#"<Relationship Id="rIdJOcore" "#,
                    r#"Type="http://schemas.openxmlformats.org/package/2006/"#,
                    r#"relationships/metadata/core-properties" "#,
                    r#"Target="docProps/core.xml"/>"#,
                ));
            }
        }
        zip.start_file("_rels/.rels", opts).map_err(|e| e.to_string())?;
        zip.write_all(rr.as_bytes()).map_err(|e| e.to_string())?;
    }

    // 文書の情報(docProps/core.xml)。原本の他の欄(日時など)は残し、
    // こちらが持つ5つの欄だけ置き直す。空の欄は消す
    if has_props || orig_core.is_some() {
        let mut cx = orig_core.unwrap_or_else(|| concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<cp:coreProperties ",
            "xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" ",
            "xmlns:dc=\"http://purl.org/dc/elements/1.1/\" ",
            "xmlns:dcterms=\"http://purl.org/dc/terms/\" ",
            "xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" ",
            "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
            "</cp:coreProperties>",
        ).to_string());
        for (tag, val) in [
            ("dc:creator", &doc.props.creator),
            ("dc:title", &doc.props.title),
            ("cp:keywords", &doc.props.keywords),
            ("dc:subject", &doc.props.subject),
            ("dc:description", &doc.props.description),
        ] {
            // 既存の欄を外す(<tag>…</tag> と <tag/> の両方)
            let open = format!("<{tag}");
            while let Some(i) = cx.find(&open) {
                let close = format!("</{tag}>");
                if let Some(j) = cx[i..].find(&close) {
                    cx.replace_range(i..i + j + close.len(), "");
                } else if let Some(j) = cx[i..].find("/>") {
                    cx.replace_range(i..i + j + 2, "");
                } else {
                    break;
                }
            }
            if !val.is_empty() {
                if let Some(i) = cx.rfind("</cp:coreProperties>") {
                    cx.insert_str(i, &format!("<{tag}>{}</{tag}>", esc(val)));
                }
            }
        }
        zip.start_file("docProps/core.xml", opts).map_err(|e| e.to_string())?;
        zip.write_all(cx.as_bytes()).map_err(|e| e.to_string())?;
    }

    // スタイル定義。テンプレートがあればそこから作り、無ければ同梱の
    // 既定のひな型を入れます。
    // このアプリで足したスタイルはその後ろに追記する
    if !orig_has_styles {
        // **新規の文書は同梱の既定のひな型を持ちます**(2026-08-26 発注者
        // 「初期設定ができていないのでは」)。前は最小の6スタイルだけで、
        // 既定の書体も大きさも styles.xml に入っていませんでした。それでは
        // 開く機械ごとに見た目が変わります
        let fallback;
        let t = match theme {
            Some(t) => t,
            None => {
                fallback = kumihan::theme::default_theme();
                &fallback
            }
        };
        let mut s = styles_from_theme(t);
        let add = styles_new_xml(doc, &s);
        if let Some(pnt) = s.rfind("</w:styles>") {
            s.insert_str(pnt, &add);
        }
        zip.start_file("word/styles.xml", opts).map_err(|e| e.to_string())?;
        zip.write_all(s.as_bytes()).map_err(|e| e.to_string())?;

        // **役ごとの書体はテーマが持ちます**(major = ゴシック、
        // minor = 明朝)。styles.xml が参照するので、対で置きます
        zip.start_file("word/theme/theme1.xml", opts).map_err(|e| e.to_string())?;
        zip.write_all(crate::theme::xml().as_bytes()).map_err(|e| e.to_string())?;
    }

    // 本文の rels。原本の関係(既存の画像・ヘッダー等)は残し、
    // 挿した画像のぶん(rIdJO1〜)と、新しく作るヘッダー・フッターを足す
    if orig_rels.is_some() || !new_media.is_empty() || hdr.is_some() || ftr.is_some()
        || doc.page_color.is_some() || doc.hyphenate || doc.protection.is_some()
        || !cmts_out.is_empty() || !orig_has_styles || !collect_links(doc).is_empty()
    {
        let mut rels = orig_rels.unwrap_or_else(|| {
            concat!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
                "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>"
            ).to_string()
        });
        // 今回作り直す番号(rIdJO1〜n)だけ除いておく。残すと Id が重なる。
        // それより先の rIdJO は開き直した既存画像の参照なので触らない
        for n in 1..=new_media.len() {
            let needle = format!("<Relationship Id=\"rIdJO{n}\"");
            if let Some(i) = rels.find(&needle) {
                if let Some(j) = rels[i..].find("/>") {
                    rels.replace_range(i..i + j + 2, "");
                }
            }
        }
        let mut add = String::new();
        for (i, m) in new_media.iter().enumerate() {
            let n = i + 1;
            let (ext, _) = image_kind(m);
            add.push_str(&format!(
                r#"<Relationship Id="rIdJO{n}" Type="{RNS_DOC}/image" Target="media/joimg{n}.{ext}"/>"#
            ));
        }
        // リンクの関係(外部の的)。**書いた r:id は必ず宣言する** —
        // 宣言の無い r:id は Word が「修復」に入る。前の保存が残した
        // 同じ Id は除いてから置き直す(番号は collect_links の並び)
        for i in 0..collect_links(doc).len() {
            let rid = link_rid(i);
            let needle = format!("<Relationship Id=\"{rid}\"");
            if let Some(at) = rels.find(&needle) {
                if let Some(j) = rels[at..].find("/>") {
                    rels.replace_range(at..at + j + 2, "");
                }
            }
        }
        for (i, url) in collect_links(doc).into_iter().enumerate() {
            add.push_str(&format!(
                r#"<Relationship Id="{}" Type="{RNS_DOC}/hyperlink" Target="{}" TargetMode="External"/>"#,
                link_rid(i),
                esc(&url),
            ));
        }
        // スタイル定義(styles.xml)への関係。まっさらの文書だけ
        if !orig_has_styles && !rels.contains("Target=\"styles.xml\"") {
            add.push_str(&format!(
                r#"<Relationship Id="rIdJOsty" Type="{RNS_DOC}/styles" Target="styles.xml"/>"#
            ));
        }
        // コメント(comments.xml)への関係。無いときだけ足す
        if !cmts_out.is_empty() && !rels.contains("Target=\"comments.xml\"") {
            add.push_str(&format!(
                r#"<Relationship Id="rIdJOcm" Type="{RNS_DOC}/comments" Target="comments.xml"/>"#
            ));
        }
        // 注(footnotes.xml / endnotes.xml)への関係。無いときだけ足す
        for (need, id, kind, target) in [
            (!notes_add.is_empty(), "rIdJOfn", "footnotes", "footnotes.xml"),
            (!ends_add.is_empty(), "rIdJOen", "endnotes", "endnotes.xml"),
        ] {
            if need && !rels.contains(&format!("Target=\"{target}\"")) {
                add.push_str(&format!(
                    r#"<Relationship Id="{id}" Type="{RNS_DOC}/{kind}" Target="{target}"/>"#
                ));
            }
        }
        // **スタイルとテーマへの関係。** こちらが styles.xml を作ったときは
        // 対で theme1.xml も置くので、両方の関係が要ります。無いと Word が
        // テーマの書体を解けず、役ごとの分け(見出し=ゴシック/本文=明朝)が
        // 効きません
        if !orig_has_styles {
            for (rid, kind, target) in [
                ("rIdJOsty", "styles", "styles.xml"),
                ("rIdJOthm", "theme", "theme/theme1.xml"),
            ] {
                if !rels.contains(&format!("Target=\"{target}\"")) {
                    add.push_str(&format!(
                        r#"<Relationship Id="{rid}" Type="{RNS_DOC}/{kind}" Target="{target}"/>"#
                    ));
                }
            }
        }
        // 設定(settings.xml)への関係。素の文書に設定を足したときだけ要る
        if (doc.page_color.is_some() || doc.hyphenate || doc.protection.is_some())
            && !rels.contains("Target=\"settings.xml\"")
        {
            add.push_str(&format!(
                r#"<Relationship Id="rIdJOset" Type="{RNS_DOC}/settings" Target="settings.xml"/>"#
            ));
        }
        // このアプリで作るヘッダー・フッター(johdr/joftr)の関係。
        // 原本由来の部品(header1.xml 等)は参照も rels も原本のまま
        for (rid, ty, part) in [
            hdr.as_ref().map(|(n, _)| ("rIdJOhdr", "header", n.as_str())),
            ftr.as_ref().map(|(n, _)| ("rIdJOftr", "footer", n.as_str())),
        ].into_iter().flatten() {
            let Some(target) = part.strip_prefix("word/") else { continue };
            if !target.starts_with("johdr") && !target.starts_with("joftr") {
                continue;
            }
            // 前の保存が残した同じ Id は除く(残すと Id が重なる)
            let needle = format!("<Relationship Id=\"{rid}\"");
            if let Some(i) = rels.find(&needle) {
                if let Some(j) = rels[i..].find("/>") {
                    rels.replace_range(i..i + j + 2, "");
                }
            }
            add.push_str(&format!(
                r#"<Relationship Id="{rid}" Type="{RNS_DOC}/{ty}" Target="{target}"/>"#
            ));
        }
        // **自己完結の `<Relationships/>` も受ける。** 閉じ札を探すだけだと、
        // 関係の1つも無い文書に足したぶんが**黙って落ちる**
        // (画像・コメント・設定も同じ道を通るので、そこも一緒に直る)
        if !add.is_empty() {
            match rels.rfind("</Relationships>") {
                Some(p) => rels.insert_str(p, &add),
                None => {
                    if let Some(p) = rels.rfind("/>") {
                        rels.replace_range(p..p + 2, &format!(">{add}</Relationships>"));
                    }
                }
            }
        }
        zip.start_file("word/_rels/document.xml.rels", opts).map_err(|e| e.to_string())?;
        zip.write_all(rels.as_bytes()).map_err(|e| e.to_string())?;
    }
    // 画像の実体
    for (i, m) in new_media.iter().enumerate() {
        let n = i + 1;
        let (ext, _) = image_kind(m);
        zip.start_file(format!("word/media/joimg{n}.{ext}"), opts)
            .map_err(|e| e.to_string())?;
        zip.write_all(m).map_err(|e| e.to_string())?;
    }
    // ヘッダー・フッターの部品
    for (name, xml) in [&hdr, &ftr].into_iter().flatten() {
        zip.start_file(name.clone(), opts).map_err(|e| e.to_string())?;
        zip.write_all(xml.as_bytes()).map_err(|e| e.to_string())?;
    }
    // コメントの部品
    if !cmts_out.is_empty() {
        zip.start_file("word/comments.xml", opts).map_err(|e| e.to_string())?;
        zip.write_all(comments_xml(&cmts_out).as_bytes()).map_err(|e| e.to_string())?;
    }
    // 注の部品。原本があれば差し込み、無ければ仕切り線ごと作る
    for (add_list, orig, endnote, part) in [
        (&notes_add, &orig_notes, false, "word/footnotes.xml"),
        (&ends_add, &orig_ends, true, "word/endnotes.xml"),
    ] {
        if add_list.is_empty() {
            continue;
        }
        zip.start_file(part, opts).map_err(|e| e.to_string())?;
        zip.write_all(notes_xml(orig.as_deref(), add_list, endnote, doc.base_pt()).as_bytes())
            .map_err(|e| e.to_string())?;
    }
    // 設定(settings.xml)。ページの色を見せる旗と、ハイフネーションの旗を
    // 織り込む。原本の settings は他の設定ごと生かす(丸ごと作り直さない)
    if doc.page_color.is_some() || doc.hyphenate || doc.protection.is_some()
        || orig_settings.is_some()
    {
        let mut st = orig_settings.unwrap_or_else(|| concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<w:settings xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"></w:settings>"
        ).to_string());
        if doc.page_color.is_some() && !st.contains("displayBackgroundShape") {
            if let Some(i) = st.rfind("</w:settings>") {
                st.insert_str(i, "<w:displayBackgroundShape/>");
            }
        }
        if doc.hyphenate {
            if !st.contains("<w:autoHyphenation") {
                if let Some(i) = st.rfind("</w:settings>") {
                    st.insert_str(i, "<w:autoHyphenation/>");
                }
            }
        } else if let Some(i) = st.find("<w:autoHyphenation") {
            if let Some(j) = st[i..].find("/>") {
                st.replace_range(i..i + j + 2, "");
            }
        }
        // 文書の保護。旗に合わせて置き直す(古いものは外してから)
        if let Some(i) = st.find("<w:documentProtection") {
            if let Some(j) = st[i..].find("/>") {
                st.replace_range(i..i + j + 2, "");
            }
        }
        if let Some(edit) = &doc.protection {
            if let Some(i) = st.rfind("</w:settings>") {
                st.insert_str(i, &format!(
                    r#"<w:documentProtection w:edit="{}" w:enforcement="1"/>"#,
                    esc(edit)
                ));
            }
        }
        zip.start_file("word/settings.xml", opts).map_err(|e| e.to_string())?;
        zip.write_all(st.as_bytes()).map_err(|e| e.to_string())?;
    }

    zip.start_file("word/document.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}