//! docx の読み書きの試験。**往復で確かめる。**

use std::io::{Cursor, Read, Write};


use super::read::*;
use super::write::*;

#[cfg(test)]
mod round {
    use super::*;
    use kumihan::{Block, Cellbox, Document, Paragraph, Run, Table};

    fn para(s: &str) -> Paragraph {
        Paragraph { style_id: None, raw_adoc: None, space_before_pt: 0.0, space_after_pt: 0.0,  align: Default::default(), style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, anchors: Vec::new(), sect: None,
                    images: Vec::new(), page_break_before: false,
                    list: Default::default(), indent: 0, first_line_twips: 0, line_spacing: 1.0, shade: None, boxed: false, images_new: Vec::new(), runs: vec![Run { text: s.to_string(), size_pt: Some(10.5), font: None, fmt: Default::default() }] }
    }
    fn doc(parts: &[&str]) -> Document {
        Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: parts.iter().map(|s| Block::Para(para(s))).collect() }
    }
    fn round_trip(d: &Document) -> (Document, Report) {
        let mut buf = Cursor::new(Vec::new());
        write(d, &mut buf).expect("書けない");
        buf.set_position(0);
        read(buf).expect("読めない")
    }
    fn texts(d: &Document) -> Vec<String> {
        d.paragraphs().map(|p| p.runs.iter().map(|r| r.text.as_str()).collect()).collect()
    }

    #[test]
    fn a_japanese_paragraph_round_trips() {
        let d = doc(&[
            "日本フネン株式会社 設備利用申込",
            "事業者名: 〇〇工務店",
            "「防火ドアは、特定防火設備です。」",
        ]);
        let (back, rep) = round_trip(&d);
        assert_eq!(texts(&back), vec![
            "日本フネン株式会社 設備利用申込",
            "事業者名: 〇〇工務店",
            "「防火ドアは、特定防火設備です。」",
        ]);
        assert!(rep.is_lossless(), "未対応が出た: {:?}", rep.unsupported);
    }

    #[test]
    fn font_size_is_preserved() {
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: vec![Block::Para(Paragraph { style_id: None, raw_adoc: None, space_before_pt: 0.0, space_after_pt: 0.0,  align: Default::default(), style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, anchors: Vec::new(), sect: None,
                    images: Vec::new(), page_break_before: false,
                    list: Default::default(), indent: 0, first_line_twips: 0, line_spacing: 1.0, shade: None, boxed: false, images_new: Vec::new(), runs: vec![
            Run { text: "大見出し".into(), size_pt: Some(16.0), font: None, fmt: Default::default() },
            Run { text: "本文".into(), size_pt: Some(10.5), font: None, fmt: Default::default() },
        ]})]};
        let (back, _) = round_trip(&d);
        let runs = &back.paragraphs().next().unwrap().runs;
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].size_pt, Some(16.0));
        assert_eq!(runs[1].size_pt, Some(10.5));
    }

    #[test]
    fn an_empty_paragraph_stays_empty() {
        let (back, _) = round_trip(&doc(&["一", "", "三"]));
        assert_eq!(texts(&back), vec!["一", "", "三"]);
    }

    #[test]
    fn leading_and_trailing_spaces_survive() {
        let (back, _) = round_trip(&doc(&["氏名　　:  山田 太郎 "]));
        assert_eq!(texts(&back)[0], "氏名　　:  山田 太郎 ");
    }

    #[test]
    fn xml_special_characters_survive() {
        let (back, _) = round_trip(&doc(&["A&B <タグ> \"引用\" 'アポ'"]));
        assert_eq!(texts(&back)[0], "A&B <タグ> \"引用\" 'アポ'");
    }

    #[test]
    fn line_breaks_inside_a_paragraph_are_preserved() {
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: vec![Block::Para(Paragraph { style_id: None, raw_adoc: None, space_before_pt: 0.0, space_after_pt: 0.0,  align: Default::default(), style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, anchors: Vec::new(), sect: None,
                    images: Vec::new(), page_break_before: false,
                    list: Default::default(), indent: 0, first_line_twips: 0, line_spacing: 1.0, shade: None, boxed: false, images_new: Vec::new(), runs: vec![
            Run { text: "一行目\n二行目".into(), size_pt: Some(10.5), font: None, fmt: Default::default() }]})]};
        let (back, _) = round_trip(&d);
        assert_eq!(texts(&back)[0], "一行目\n二行目");
    }

    // ---- 表: 日本の事務様式の本体(実物8件すべてに w:tbl があった) ----

    fn cell(s: &str) -> Cellbox {
        Cellbox { paragraphs: vec![para(s)], ..Default::default() }
    }

    #[test]
    fn tables_round_trip() {
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: vec![
            Block::Para(para("(様式3) 会社概要")),
            Block::Table(Table { col_mm: vec![], rows: vec![
                vec![cell("会　社　名"), cell("日本フネン株式会社")],
                vec![cell("所　在　地"), cell("徳島県吉野川市川島町三ツ島新田179-1")],
                vec![cell("資　本　金"), cell("3億1,400万円")],
            ],
        ..Default::default()
    }),
            Block::Para(para("以上")),
        ]};
        let (back, rep) = round_trip(&d);
        let t: Vec<&Table> = back.tables().collect();
        assert_eq!(t.len(), 1, "表が1つ戻る");
        assert_eq!(t[0].rows.len(), 3);
        assert_eq!(t[0].rows[1].len(), 2);
        let v: String = t[0].rows[1][1].paragraphs[0].runs.iter()
            .map(|r| r.text.as_str()).collect();
        assert_eq!(v, "徳島県吉野川市川島町三ツ島新田179-1");
        assert_eq!(texts(&back), vec!["(様式3) 会社概要", "以上"], "本文の順序も保たれる");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
    }

    #[test]
    fn the_order_of_tables_and_body_text_is_preserved() {
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: vec![
            Block::Para(para("前")),
            Block::Table(Table { col_mm: vec![], rows: vec![vec![cell("表1")]],
        ..Default::default()
    }),
            Block::Para(para("中")),
            Block::Table(Table { col_mm: vec![], rows: vec![vec![cell("表2")]],
        ..Default::default()
    }),
            Block::Para(para("後")),
        ]};
        let (back, _) = round_trip(&d);
        let kinds: Vec<&str> = back.blocks.iter().map(|b| match b {
            Block::Para(_) => "段落", Block::Table(_) => "表" }).collect();
        assert_eq!(kinds, vec!["段落", "表", "段落", "表", "段落"]);
    }

    #[test]
    fn an_empty_cell_still_holds_its_column() {
        // 事務様式は「記入欄が空の表」が本体。空セルが消えると様式が壊れる
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: vec![Block::Table(Table { col_mm: vec![], rows: vec![
            vec![cell("氏名"), Cellbox::default()],
            vec![cell("所属"), Cellbox::default()],
        ],
        ..Default::default()
    })]};
        let (back, _) = round_trip(&d);
        let t: Vec<&Table> = back.tables().collect();
        assert_eq!(t[0].rows.len(), 2);
        assert_eq!(t[0].rows[0].len(), 2, "空セルが消えた");
        assert_eq!(t[0].rows[1].len(), 2, "空セルが消えた");
    }

    #[test]
    fn unreadable_elements_are_reported_not_silently_dropped() {
        let xml = r#"<?xml version="1.0"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><w:body>
<w:p><w:r><w:t>前</w:t></w:r></w:p>
<w:p><w:r><w:drawing/></w:r></w:p>
<w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr>
<w:p><w:r><w:t>結合セル</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
</w:body></w:document>"#;
        let (doc, rep) = parse_document_xml(xml);
        assert!(!rep.is_lossless());
        assert!(rep.unsupported.iter().any(|(n, _)| n.contains("drawing")),
            "画像の未対応が報告されていない: {:?}", rep.unsupported);
        // セル結合は読めるようになった。報告ではなくモデルに入る
        assert!(!rep.unsupported.iter().any(|(n, _)| n.contains("セル結合")),
            "読めるのに未対応と報告した: {:?}", rep.unsupported);
        let t: Vec<&Table> = doc.tables().collect();
        assert_eq!(t[0].rows[0][0].col_span, 2, "gridSpan が読めていない");
    }

    #[test]
    fn cell_merges_round_trip() {
        // 様式の見出しは結合で出来ている。往復で消えると枠がずれる
        let mut head = cell("会社概要");
        head.col_span = 2;
        let mut vstart = cell("所在地");
        vstart.v_merge = kumihan::VMerge::Start;
        let mut vcont = Cellbox::default();
        vcont.v_merge = kumihan::VMerge::Continue;
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: vec![
            Block::Table(Table { col_mm: vec![], rows: vec![
                vec![head],
                vec![vstart, cell("本社")],
                vec![vcont, cell("工場")],
            ],
        ..Default::default()
    }),
        ]};
        let (back, rep) = round_trip(&d);
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        let t: Vec<&Table> = back.tables().collect();
        assert_eq!(t[0].rows[0][0].col_span, 2, "gridSpan が往復しない");
        assert_eq!(t[0].rows[1][0].v_merge, kumihan::VMerge::Start,
            "vMerge の始まりが往復しない");
        assert_eq!(t[0].rows[2][0].v_merge, kumihan::VMerge::Continue,
            "vMerge の続きが往復しない");
        assert_eq!(t[0].rows[1][1].v_merge, kumihan::VMerge::None,
            "結合していないセルに結合が付いた");
    }

    #[test]
    fn a_real_template_with_merges_reads_without_loss() {
        // 様式3(会社概要)は gridSpan 15 + vMerge 3 の結合入り
        let src = "/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki/実施要領様式3_会社概要.docx";
        let Ok(bytes) = std::fs::read(src) else { return };
        let (doc, rep) = crate::read(Cursor::new(bytes)).expect("読めない");
        assert!(!rep.unsupported.iter().any(|(n, _)| n.contains("セル結合")),
            "実物のセル結合が未対応のまま: {:?}", rep.unsupported);
        let spans: usize = doc.tables()
            .flat_map(|t| t.rows.iter())
            .flat_map(|r| r.iter())
            .filter(|c| c.col_span > 1)
            .count();
        assert!(spans > 0, "実物の gridSpan がモデルに入っていない");
        // 往復しても結合が残る
        let mut buf = Cursor::new(Vec::new());
        crate::write(&doc, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = crate::read(buf).expect("読み直せない");
        let spans2: usize = back.tables()
            .flat_map(|t| t.rows.iter())
            .flat_map(|r| r.iter())
            .filter(|c| c.col_span > 1)
            .count();
        assert_eq!(spans, spans2, "保存で結合が消えた");
    }
}

#[cfg(test)]
mod font_tests {
    use kumihan::{Block, Document, Paragraph, Run};

    #[test]
    fn font_name_round_trips() {
        // **フォントは文書の設定。** 読んで捨てると、開き直したとき別の字になる
        let doc = Document { shapes: Vec::new(), size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), footnotes: Vec::new(),
            font: None,
            page: None,
            sect_raw: None, header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false,
            blocks: vec![Block::Para(Paragraph { style_id: None, raw_adoc: None, space_before_pt: 0.0, space_after_pt: 0.0,  style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, sect: None,
                align: Default::default(),
                anchors: Vec::new(),
                    images: Vec::new(),
                page_break_before: false,
                    list: Default::default(),
                indent: 0,
                first_line_twips: 0,
                line_spacing: 1.0,
                shade: None, boxed: false, images_new: Vec::new(), runs: vec![Run {
                    text: "日本フネン".into(),
                    size_pt: Some(10.5),
                    font: Some("BIZ UDPゴシック".into()),
                    fmt: Default::default(),
                }],
            })],
        };
        let mut buf = Vec::new();
        crate::write(&doc, std::io::Cursor::new(&mut buf)).unwrap();
        let (back, _) = crate::read(std::io::Cursor::new(&buf)).unwrap();
        let run = back.paragraphs().next().unwrap().runs.first().unwrap();
        assert_eq!(run.font.as_deref(), Some("BIZ UDPゴシック"), "書体名が消えた");
    }

    #[test]
    fn the_japanese_font_is_read_from_eastasia() {
        // ascii しか見ないと、日本語の明朝指定を落とす
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:rPr>
            <w:rFonts w:ascii="Century" w:eastAsia="ＭＳ 明朝"/></w:rPr>
            <w:t>本文</w:t></w:r></w:p></w:body></w:document>"#;
        let doc = crate::parse_document_xml(xml).0;
        let run = doc.paragraphs().next().unwrap().runs.first().unwrap();
        assert_eq!(run.font.as_deref(), Some("ＭＳ 明朝"), "eastAsia を見ていない");
    }
}

#[cfg(test)]
mod fmt_tests {
    use kumihan::{Align, Block, CharFormat, Document, Paragraph, Run};

    fn run(text: &str, fmt: CharFormat) -> Run {
        Run { text: text.into(), size_pt: Some(10.5), font: None, fmt }
    }

    fn roundtrip(doc: &Document) -> Document {
        let mut buf = Vec::new();
        crate::write(doc, std::io::Cursor::new(&mut buf)).unwrap();
        crate::read(std::io::Cursor::new(&buf)).unwrap().0
    }

    #[test]
    fn bold_italic_and_underline_round_trip() {
        let f = CharFormat { bold: true, italic: true, underline: true, ..Default::default() };
        let d = Document { shapes: Vec::new(), size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), footnotes: Vec::new(),
            font: None,
            page: None,
            sect_raw: None, header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false,
            blocks: vec![Block::Para(Paragraph { style_id: None, raw_adoc: None, space_before_pt: 0.0, space_after_pt: 0.0,  align: Align::Left, style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, anchors: Vec::new(), sect: None,
                    images: Vec::new(), page_break_before: false,
                    list: Default::default(), indent: 0, first_line_twips: 0, line_spacing: 1.0, shade: None, boxed: false, images_new: Vec::new(), runs: vec![run("見出し", f.clone())] })],
        };
        let back = roundtrip(&d);
        assert_eq!(back.paragraphs().next().unwrap().runs[0].fmt, f, "書式が消えた");
    }

    #[test]
    fn strikethrough_and_font_color_round_trip() {
        let f = CharFormat { strike: true, color: Some("FF0000".into()), ..Default::default() };
        let d = Document { shapes: Vec::new(), size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), footnotes: Vec::new(),
            font: None,
            page: None,
            sect_raw: None, header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false,
            blocks: vec![Block::Para(Paragraph { style_id: None, raw_adoc: None, space_before_pt: 0.0, space_after_pt: 0.0,  align: Align::Left, style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, anchors: Vec::new(), sect: None,
                    images: Vec::new(), page_break_before: false,
                    list: Default::default(), indent: 0, first_line_twips: 0, line_spacing: 1.0, shade: None, boxed: false, images_new: Vec::new(), runs: vec![run("赤", f.clone())] })],
        };
        assert_eq!(roundtrip(&d).paragraphs().next().unwrap().runs[0].fmt, f);
    }

    #[test]
    fn centering_round_trips() {
        for a in [Align::Center, Align::Right, Align::Justify, Align::Left] {
            let d = Document { shapes: Vec::new(), size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), footnotes: Vec::new(),
                font: None,
                page: None,
                sect_raw: None, header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false,
                blocks: vec![Block::Para(Paragraph { style_id: None, raw_adoc: None, space_before_pt: 0.0, space_after_pt: 0.0,  style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, sect: None,
                    align: a,
                    anchors: Vec::new(),
                    images: Vec::new(),
                    page_break_before: false,
                    list: Default::default(),
                    indent: 0,
                    first_line_twips: 0,
                    line_spacing: 1.0,
                    shade: None, boxed: false, images_new: Vec::new(), runs: vec![run("表題", CharFormat::default())],
                })],
            };
            assert_eq!(roundtrip(&d).paragraphs().next().unwrap().align, a, "{a:?} が消えた");
        }
    }

    #[test]
    fn an_explicitly_cleared_bold_stays_off() {
        // <w:b w:val="0"/> は「太字ではない」。有無だけで見ると間違える
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:rPr>
            <w:b w:val="0"/><w:i/></w:rPr><w:t>本文</w:t></w:r></w:p></w:body></w:document>"#;
        let doc = crate::parse_document_xml(xml).0;
        let f = &doc.paragraphs().next().unwrap().runs[0].fmt;
        assert!(!f.bold, "w:val=0 の太字を太字にした");
        assert!(f.italic, "斜体を落とした");
    }

    #[test]
    fn formats_do_not_mix_between_paragraphs() {
        // 前の段落の太字が次に漏れないこと
        let xml = r#"<w:document xmlns:w="x"><w:body>
            <w:p><w:pPr><w:jc w:val="center"/></w:pPr>
                <w:r><w:rPr><w:b/></w:rPr><w:t>表題</w:t></w:r></w:p>
            <w:p><w:r><w:t>本文</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let doc = crate::parse_document_xml(xml).0;
        let ps: Vec<_> = doc.paragraphs().collect();
        assert!(ps[0].runs[0].fmt.bold);
        assert_eq!(ps[0].align, Align::Center);
        assert!(!ps[1].runs[0].fmt.bold, "太字が次の段落へ漏れた");
        assert_eq!(ps[1].align, Align::Left, "揃えが次の段落へ漏れた");
    }
}

#[cfg(test)]
mod para_tests {
    use kumihan::{Align, Block, Document, ListKind, Paragraph, Run};

    fn para(list: ListKind, indent: u8, spacing: f32) -> Paragraph {
        Paragraph { style_id: None, raw_adoc: None, space_before_pt: 0.0, space_after_pt: 0.0,  style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, sect: None,
            align: Align::Left,
            anchors: Vec::new(),
                    images: Vec::new(),
            page_break_before: false,
            list,
            indent,
            first_line_twips: 0,
            line_spacing: spacing,
            shade: None, boxed: false, images_new: Vec::new(),
            runs: vec![Run {
                text: "項目".into(), size_pt: Some(10.5), font: None, fmt: Default::default(),
            }],
        }
    }

    fn roundtrip(p: Paragraph) -> Paragraph {
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: vec![Block::Para(p)] };
        let mut buf = Vec::new();
        crate::write(&d, std::io::Cursor::new(&mut buf)).unwrap();
        crate::read(std::io::Cursor::new(&buf)).unwrap().0.paragraphs().next().unwrap().clone()
    }

/// **段落の前後の空きが往復で消えない**(2026-08-15)。
    ///
    /// 前は `w:spacing` の `w:line` だけを読み書きしていたので、Word の文書を
    /// 開いて保存すると `w:before` / `w:after` が黙って消えていた。
    /// 帳票の余白は書いた人が決めた物なので、勝手に詰めてはいけない。
    #[test]
    fn paragraph_spacing_round_trips() {
        let mut p = Paragraph { style_id: None, raw_adoc: None, space_before_pt: 12.0, space_after_pt: 6.0,
            align: Default::default(), style: Default::default(), comments: Vec::new(),
            bookmarks: Vec::new(), anchors: Vec::new(), sect: None, images: Vec::new(),
            page_break_before: false, list: ListKind::None, indent: 0, first_line_twips: 0,
            line_spacing: 1.0, shade: None, boxed: false, dropcap: false,
            images_new: Vec::new(),
            runs: vec![Run { text: "間の空いた段落".into(), size_pt: Some(10.5),
                             font: None, fmt: Default::default() }] };
        p.line_spacing = 1.0;
        let q = roundtrip(p);
        assert_eq!(q.space_before_pt, 12.0, "保存で前の空きが消えた");
        assert_eq!(q.space_after_pt, 6.0, "保存で後の空きが消えた");
    }

    /// 空きが無い段落には `w:spacing` を書かない(要らない印を増やさない)
    #[test]
    fn a_paragraph_without_spacing_stays_plain() {
        let p = Paragraph { style_id: None, raw_adoc: None, space_before_pt: 0.0, space_after_pt: 0.0,
            align: Default::default(), style: Default::default(), comments: Vec::new(),
            bookmarks: Vec::new(), anchors: Vec::new(), sect: None, images: Vec::new(),
            page_break_before: false, list: ListKind::None, indent: 0, first_line_twips: 0,
            line_spacing: 1.0, shade: None, boxed: false, dropcap: false,
            images_new: Vec::new(),
            runs: vec![Run { text: "素の段落".into(), size_pt: Some(10.5),
                             font: None, fmt: Default::default() }] };
        let q = roundtrip(p);
        assert_eq!(q.space_before_pt, 0.0);
        assert_eq!(q.space_after_pt, 0.0);
    }

    #[test]
    fn bullet_lists_round_trip() {
        assert_eq!(roundtrip(para(ListKind::Bullet, 0, 1.0)).list, ListKind::Bullet);
        assert_eq!(roundtrip(para(ListKind::Number, 0, 1.0)).list, ListKind::Number);
        assert_eq!(roundtrip(para(ListKind::None, 0, 1.0)).list, ListKind::None);
    }

    #[test]
    fn the_first_line_indent_round_trips() {
        // 2026-08-13 に実測で踏んだ穴: 段落を触ると w:ind の firstLine が
        // 黙って落ちていた(左と寄せは残るのに)。twip の生値で往復すること
        for fl in [420i32, 210, -300] {
            let mut p = para(ListKind::None, 0, 1.0);
            p.first_line_twips = fl;
            assert_eq!(roundtrip(p).first_line_twips, fl, "firstLine {fl} が消えた");
        }
        // 左インデントと同居しても両方残る
        let mut p = para(ListKind::None, 2, 1.0);
        p.first_line_twips = 420;
        let r = roundtrip(p);
        assert_eq!((r.indent, r.first_line_twips), (2, 420), "同居で片方が消えた");
    }

    #[test]
    fn indent_round_trips() {
        for n in [1u8, 3, 8] {
            assert_eq!(roundtrip(para(ListKind::None, n, 1.0)).indent, n, "{n}段が消えた");
        }
    }

    #[test]
    fn line_spacing_round_trips() {
        for s in [1.5f32, 2.0] {
            let got = roundtrip(para(ListKind::None, 0, s)).spacing();
            assert!((got - s).abs() < 0.01, "{s} 倍が {got} になった");
        }
    }

    #[test]
    fn a_default_paragraph_gets_no_extra_spec() {
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: vec![Block::Para(para(ListKind::None, 0, 1.0))] };
        let mut buf = Vec::new();
        crate::write(&d, std::io::Cursor::new(&mut buf)).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&buf)).unwrap();
        let mut s = String::new();
        use std::io::Read;
        z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
        // **行の高さだけは書きます**(2026-08-29)。書かないと開いた側が
        // 自分の既定を当て、同じ文書が別の頁数に折れます。それ以外は
        // 足しません — 頼まれていない飾りを書かないための試験です
        assert!(s.contains("w:lineRule=\"atLeast\""), "行の高さを書いていない");
        for iranai in ["w:jc", "w:ind", "w:numPr", "w:pStyle", "w:before", "w:after"] {
            assert!(!s.contains(iranai), "頼まれていない {iranai} を書いた");
        }
    }

    #[test]
    fn zero_line_spacing_does_not_break() {
        // 0 や負が入っても本文が消えない
        let p = para(ListKind::None, 0, 0.0);
        assert_eq!(p.spacing(), 1.0);
        let p = para(ListKind::None, 0, -3.0);
        assert_eq!(p.spacing(), 1.0);
    }

    #[test]
    fn the_bullet_mark_is_rendered() {
        assert_eq!(para(ListKind::Bullet, 0, 1.0).marker(0).as_deref(), Some("・"));
        assert_eq!(para(ListKind::Number, 0, 1.0).marker(0).as_deref(), Some("1. "));
        assert_eq!(para(ListKind::Number, 0, 1.0).marker(4).as_deref(), Some("5. "));
        assert_eq!(para(ListKind::None, 0, 1.0).marker(0), None);
    }
}

#[cfg(test)]
mod break_round {
    use kumihan::{Block, Document, Paragraph, Run};

    #[test]
    fn the_page_break_spec_round_trips() {
        let mut para = Paragraph::default();
        para.page_break_before = true;
        para.runs.push(Run {
            text: "二頁目".into(), size_pt: Some(10.5), font: None, fmt: Default::default() });
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: vec![Block::Para(para)] };
        let mut buf = Vec::new();
        crate::write(&d, std::io::Cursor::new(&mut buf)).unwrap();
        let back = crate::read(std::io::Cursor::new(&buf)).unwrap().0;
        assert!(back.paragraphs().next().unwrap().page_break_before, "改ページが消えた");
    }
}

#[cfg(test)]
mod size_round {
    // 本家 python-docx が作る形(run に w:sz が無い)の再現。
    // 2026-08-13 まで、開いて保存するだけで w:sz val="21"(10.5pt)が
    // 書き込まれていた — 原本に無かった指定が増える穴
    #[test]
    fn an_unset_size_round_trips_unset() {
        let xml = r#"<w:document xmlns:w="x"><w:body>
            <w:p><w:r><w:t>大きさを指定していない字</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let doc = crate::parse_document_xml(xml).0;
        assert_eq!(doc.paragraphs().next().unwrap().runs[0].size_pt, None,
            "読みで数が湧いた");
        let out = crate::write_document_xml(&doc);
        assert!(!out.contains("<w:sz "),
            "無指定の run に w:sz が書き込まれた(焼き付きの再発): {out}");
    }

    #[test]
    fn a_previous_runs_spec_does_not_bleed_into_the_next() {
        // 指定は run ごと。前の run の 14pt を引きずると、無指定の run が
        // 「14pt 指定」に化けて保存される(焼き付きと同じ形の穴)
        let xml = r#"<w:document xmlns:w="x"><w:body>
            <w:p><w:r><w:rPr><w:sz w:val="28"/></w:rPr><w:t>大きい</w:t></w:r><w:r><w:t>ふつう</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let doc = crate::parse_document_xml(xml).0;
        let runs = &doc.paragraphs().next().unwrap().runs;
        assert_eq!(runs[0].size_pt, Some(14.0));
        assert_eq!(runs[1].size_pt, None, "前の run の指定が染みた");
    }
}

#[cfg(test)]
mod gridcol_round {
    #[test]
    fn column_widths_round_trip() {
        // 読んだ幅を捨てると、保存で表の形が変わる
        let xml = r#"<w:document xmlns:w="x"><w:body><w:tbl>
            <w:tblGrid><w:gridCol w:w="2268"/><w:gridCol w:w="4536"/></w:tblGrid>
            <w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc>
                  <w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr>
        </w:tbl></w:body></w:document>"#;
        let doc = crate::parse_document_xml(xml).0;
        let t = doc.tables().next().expect("表が無い");
        assert_eq!(t.col_mm.len(), 2, "gridCol を読めていない");
        // 2268 twip = 40mm
        assert!((t.col_mm[0] - 40.0).abs() < 0.1, "{}", t.col_mm[0]);
        assert!((t.col_mm[1] - 80.0).abs() < 0.1);

        let mut buf = Vec::new();
        crate::write(&doc, std::io::Cursor::new(&mut buf)).unwrap();
        let back = crate::read(std::io::Cursor::new(&buf)).unwrap().0;
        let bt = back.tables().next().unwrap();
        assert!((bt.col_mm[0] - 40.0).abs() < 0.2, "列幅が保存で変わった");
    }
}

#[cfg(test)]
mod preserve_tests {
    use kumihan::Document;
    use std::io::{Cursor, Read, Write};

    /// スタイルと画像もどきの部品を持つ docx を作る
    fn docx_with_parts() -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o: zip::write::FileOptions<'_, ()> = Default::default();
        let mut put = |n: &str, d: &[u8]| {
            zip.start_file(n, o).unwrap();
            zip.write_all(d).unwrap();
        };
        put("[Content_Types].xml", br#"<Types xmlns="ct"><Default Extension="xml" ContentType="application/xml"/></Types>"#);
        put("_rels/.rels", br#"<Relationships xmlns="r"/>"#);
        put("word/document.xml",
            br#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>Logo included</w:t></w:r></w:p></w:body></w:document>"#);
        put("word/styles.xml", br#"<w:styles xmlns:w="x"/>"#);
        put("word/media/logo.png", b"\x89PNG-fake-bytes");
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn open_and_save_keeps_the_parts() {
        // 「保存したらロゴが消えた」を防ぐ
        let src = docx_with_parts();
        let (doc, _) = crate::read(Cursor::new(&src)).unwrap();
        let mut out = Vec::new();
        crate::write_with(&doc, Some(Cursor::new(&src)), Cursor::new(&mut out)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
        let names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().into()).collect();
        assert!(names.iter().any(|n| n == "word/media/logo.png"), "画像の実体が消えた: {names:?}");
        assert!(names.iter().any(|n| n == "word/styles.xml"), "スタイルが消えた: {names:?}");
        // 画像の中身も同じ
        let mut buf = Vec::new();
        z.by_name("word/media/logo.png").unwrap().read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"\x89PNG-fake-bytes");
        // 本文はこちらが書いたもの
        let mut s = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
        assert!(s.contains("Logo included"), "本文が消えた");
    }

    #[test]
    fn the_body_is_not_written_twice() {
        let src = docx_with_parts();
        let (doc, _) = crate::read(Cursor::new(&src)).unwrap();
        let mut out = Vec::new();
        crate::write_with(&doc, Some(Cursor::new(&src)), Cursor::new(&mut out)).unwrap();
        let z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
        let n = (0..z.len())
            .filter(|i| {
                let mut z2 = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
                z2.by_index(*i).map(|f| f.name() == "word/document.xml").unwrap_or(false)
            })
            .count();
        assert_eq!(n, 1, "document.xml が {n} 個ある");
    }

    #[test]
    fn with_no_original_a_minimal_form_is_written() {
        let doc = Document::plain("新規");
        let mut out = Vec::new();
        crate::write_with(&doc, None::<Cursor<Vec<u8>>>, Cursor::new(&mut out)).unwrap();
        assert!(crate::read(Cursor::new(&out)).is_ok());
    }
}

#[cfg(test)]
mod anchor_tests {
    #[test]
    fn the_images_source_comes_back_on_save() {
        // 理解はしないが、捨てない
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p>
            <w:r><w:t>図は</w:t></w:r>
            <w:r><w:drawing><wp:inline><a:blip r:embed="rId5"/></wp:inline></w:drawing></w:r>
            <w:r><w:t>のとおり</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let (doc, rep) = crate::parse_document_xml(xml);
        assert!(rep.unsupported.iter().any(|(n, _)| n.contains("画像")), "報告が無い");
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.anchors.len(), 1, "原文を控えていない");
        assert!(p.anchors[0].contains("rId5"), "{}", p.anchors[0]);

        let out = crate::write_document_xml(&doc);
        assert!(out.contains("r:embed=\"rId5\""), "保存で画像の参照が消えた");
        assert!(out.contains("のとおり"), "本文が消えた");
    }

    #[test]
    fn text_inside_a_shape_does_not_leak_into_the_body() {
        // a:t の「飾り文字」が本文へ混ざっていた(t を接頭辞に関係なく拾っていた)
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p>
            <w:r><w:t>本文</w:t></w:r>
            <w:r><w:drawing><a:t>飾り文字</a:t></w:drawing></w:r>
        </w:p></w:body></w:document>"#;
        let (doc, _) = crate::parse_document_xml(xml);
        assert_eq!(doc.body_text(), "本文", "図形の中の文字が本文へ漏れた: {:?}", doc.body_text());
    }

    // 数式(OMML)。**型紙は手で組む** — 自分の書き手で書いた文書で往復を
    // 確かめると、読めていない所は行きも帰りも同じように読めないので差が出ない。
    // 下の形はどちらも実物(pandoc / LibreOffice Writer)から写した
    #[test]
    fn equations_come_back_verbatim_on_save() {
        // pandoc の形: xmlns:m は root にあり、m:oMath は裸で来る
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><w:body><w:p>
            <w:r><w:t>式は</w:t></w:r>
            <m:oMath><m:sSup><m:e><m:r><m:t>a</m:t></m:r></m:e><m:sup><m:r><m:t>2</m:t></m:r></m:sup></m:sSup></m:oMath>
            <w:r><w:t>のとおり</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let (doc, rep) = crate::parse_document_xml(xml);
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.anchors.len(), 1, "数式の原文を控えていない");
        assert!(p.anchors[0].contains("<m:sSup>"), "{}", p.anchors[0]);
        // **中の字を本文へ混ぜない。** 混ぜると保存で数式ではなく平文になる
        // (`local()` が接頭辞を落とすので m:t が w:t の枝に落ちていた)
        assert_eq!(doc.body_text(), "式はのとおり",
            "数式の中の字が本文へ漏れた: {:?}", doc.body_text());
        assert!(rep.unsupported.iter().any(|(n, _)| n.contains("数式")), "報告が無い");

        let out = crate::write_document_xml(&doc);
        assert!(out.contains("<m:sSup>"), "保存で数式が消えた");
        assert!(out.contains("のとおり"), "本文が消えた");

        // 二度目の往復でも増えも減りもしない(控えを控え直さない)
        let (doc2, _) = crate::parse_document_xml(&out);
        let p2 = doc2.paragraphs().next().unwrap();
        assert_eq!(p2.anchors.len(), 1, "二度目で数式の控えが増減した");
        assert_eq!(doc2.body_text(), "式はのとおり", "二度目で本文が変わった");
    }

    #[test]
    fn an_equation_declaring_its_own_namespace_is_not_declared_twice() {
        // LibreOffice Writer の形: root に xmlns:m は**無く**、m:oMath が
        // 自分で宣言している。ここへ重ねて足すと属性が二重になり、
        // Word が開けない XML になる
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p>
            <m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><m:r><w:rPr><w:rFonts w:ascii="Cambria Math"/></w:rPr><m:t>x</m:t></m:r></m:oMath>
        </w:p></w:body></w:document>"#;
        let (doc, _) = crate::parse_document_xml(xml);
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.anchors.len(), 1, "数式の原文を控えていない");
        assert_eq!(p.anchors[0].matches("xmlns:m=").count(), 1,
            "名前空間の宣言が二重になった: {}", p.anchors[0]);
        assert_eq!(doc.body_text(), "", "数式の中の字が本文へ漏れた: {:?}", doc.body_text());
        assert!(crate::write_document_xml(&doc).contains("Cambria Math"),
            "保存で数式が消えた");
    }

    #[test]
    fn an_equation_with_xml_space_is_kept() {
        // **実物で踏んだ穴。** `xml:` は XML の定めで最初から結びついていて、
        // どこにも宣言が無いのが正しい。それを「解決できない接頭辞」と数えて
        // いたので、LibreOffice Writer の数式(`<m:t xml:space="preserve">`)が
        // 4つとも丸ごと落ちていた。手で書いた文書では出ない形
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p>
            <m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><m:r><w:rPr><w:rFonts w:ascii="Cambria Math"/></w:rPr><m:t xml:space="preserve">a </m:t></m:r></m:oMath>
        </w:p></w:body></w:document>"#;
        let (doc, rep) = crate::parse_document_xml(xml);
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.anchors.len(), 1, "xml:space で数式が落ちた: {:?}", rep.unsupported);
        // xml: を宣言し直してはいけない(それも壊れた XML になる)
        assert!(!p.anchors[0].contains("xmlns:xml="),
            "xml: を宣言してしまった: {}", p.anchors[0]);
        assert!(crate::write_document_xml(&doc).contains("xml:space"), "保存で数式が消えた");
    }

    #[test]
    fn equations_inside_table_cells_are_carried_over() {
        // 段落は本文にも**表のセルの中にも**ある。控えの受け渡しが本文の
        // 段落でしか働かないと、表の中の数式だけ静かに落ちる
        // (向こう(genoffice)の試験を読んでいて気付いた筋。2026-08-10)
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><w:body>
            <w:tbl><w:tr><w:tc><w:p>
                <w:r><w:t>面積</w:t></w:r>
                <m:oMath><m:r><m:t>πr2</m:t></m:r></m:oMath>
            </w:p></w:tc></w:tr></w:tbl>
        </w:body></w:document>"#;
        let (doc, _) = crate::parse_document_xml(xml);
        let t: Vec<&kumihan::Table> = doc.tables().collect();
        let cell_para = &t[0].rows[0][0].paragraphs[0];
        assert_eq!(cell_para.anchors.len(), 1,
            "表のセルの中で数式を控えていない: {:?}", cell_para.anchors);
        let out = crate::write_document_xml(&doc);
        assert!(out.contains("πr2"), "保存で表の中の数式が消えた: {out}");
        assert_eq!(out.matches("<m:oMath").count(), 1, "数式が二重に出た: {out}");
    }

    #[test]
    fn two_equations_in_one_paragraph_both_survive() {
        // 向こうの試験にこの形がある(multiple oMath fragments in one paragraph)
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><w:body><w:p>
            <m:oMath><m:r><m:t>甲</m:t></m:r></m:oMath>
            <m:oMath><m:r><m:t>乙</m:t></m:r></m:oMath>
        </w:p></w:body></w:document>"#;
        let (doc, _) = crate::parse_document_xml(xml);
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.anchors.len(), 2, "二つ目を落とした: {:?}", p.anchors);
        let out = crate::write_document_xml(&doc);
        assert_eq!(out.matches("<m:oMath").count(), 2, "数式の数が合わない: {out}");
        // 数式どうしの前後は入れ替わらない(段落の頭へ寄るのは全体として)
        assert!(out.find('甲') < out.find('乙'), "数式どうしの順が入れ替わった: {out}");
    }

    #[test]
    fn equations_inside_a_header_are_carried_over() {
        // ヘッダー・フッターは同じ読み手を別の root で通す。
        // 控えの受け渡しがそこでも働くか
        let xml = r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><w:p>
            <m:oMath><m:r><m:t>丙</m:t></m:r></m:oMath>
        </w:p></w:hdr>"#;
        let (doc, _) = crate::parse_document_xml(xml);
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.anchors.len(), 1, "ヘッダーで数式を控えていない: {:?}", p.anchors);
    }

    #[test]
    fn a_standalone_equation_is_not_stored_twice() {
        // m:oMathPara(独立した数式)の中には m:oMath が入っている。
        // 外側を丸ごと控えるので、中の oMath を別に控えてはいけない
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><w:body><w:p>
            <m:oMathPara><m:oMathParaPr><m:jc m:val="center"/></m:oMathParaPr><m:oMath><m:r><m:t>y</m:t></m:r></m:oMath></m:oMathPara>
        </w:p></w:body></w:document>"#;
        let (doc, _) = crate::parse_document_xml(xml);
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.anchors.len(), 1, "控えの数が合わない: {:?}", p.anchors);
        let out = crate::write_document_xml(&doc);
        assert_eq!(out.matches("<m:oMath>").count(), 1, "数式が二重に出た: {out}");
        assert_eq!(out.matches("<m:oMathPara ").count(), 1, "独立の外側の要素が消えたか二重: {out}");
        // 書き手の root は xmlns:m を宣言しないので、控えが自分で持つ必要がある
        assert!(!out.contains("<w:document") || out.matches("xmlns:m=").count() == 1,
            "名前空間の宣言が足りないか二重: {out}");
    }

    #[test]
    fn an_equation_with_an_unknown_prefix_is_dropped_and_reported() {
        // 壊れた XML を書くより、落として帳簿に出す方がまし(画像と同じ作法)
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><w:body><w:p>
            <m:oMath><zz:mystery zz:val="1"/></m:oMath>
        </w:p></w:body></w:document>"#;
        let (doc, rep) = crate::parse_document_xml(xml);
        assert!(doc.paragraphs().next().unwrap().anchors.is_empty(), "壊れた控えを作った");
        assert!(rep.unsupported.iter().any(|(n, _)| n.contains("数式") && n.contains("失われる")),
            "落としたのに報告していない: {:?}", rep.unsupported);
    }

    #[test]
    fn typing_one_character_does_not_lose_the_image() {
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p>
            <w:r><w:t>図</w:t></w:r>
            <w:r><w:drawing><a:blip r:embed="rId7"/></w:drawing></w:r>
        </w:p></w:body></w:document>"#;
        let (mut doc, _) = crate::parse_document_xml(xml);
        doc.set_body_text("図を直した");
        let out = crate::write_document_xml(&doc);
        assert!(out.contains("rId7"), "編集しただけで画像が消えた");
    }

    #[test]
    fn typing_one_character_does_not_lose_the_equation() {
        // 画像と同じ約束を数式にも。`officework.doc` から本文を書き換える
        // 人が居るので、**編集は控えを巻き添えにしない**
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><w:body><w:p>
            <w:r><w:t>式</w:t></w:r>
            <m:oMath><m:r><m:t>E=mc2</m:t></m:r></m:oMath>
        </w:p></w:body></w:document>"#;
        let (mut doc, _) = crate::parse_document_xml(xml);
        doc.set_body_text("式を直した");
        let out = crate::write_document_xml(&doc);
        assert!(out.contains("E=mc2"), "編集しただけで数式が消えた: {out}");
        // 段落を割っても、控えは前半に残って消えも増えもしない
        let (mut doc2, _) = crate::parse_document_xml(xml);
        doc2.set_body_text("上\n下");
        let out2 = crate::write_document_xml(&doc2);
        assert_eq!(out2.matches("<m:oMath").count(), 1,
            "段落を割ったら数式が消えたか二重になった: {out2}");
    }
}

#[cfg(test)]
mod vertalign_tests {
    use kumihan::{Align, Block, CharFormat, Document, Paragraph, Run};

    fn doc_with(fmt: CharFormat) -> Document {
        Document { shapes: Vec::new(), size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), footnotes: Vec::new(),
            font: None,
            page: None,
            sect_raw: None, header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false,
            blocks: vec![Block::Para(Paragraph { style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, sect: None,
                align: Align::Left,
                runs: vec![Run { text: "x2".into(), size_pt: Some(10.5), font: None, fmt }],
                ..Default::default()
            })],
        }
    }

    #[test]
    fn superscript_and_highlight_round_trip() {
        let f = CharFormat {
            superscript: true,
            highlight: Some("yellow".into()),
            ..Default::default()
        };
        let mut buf = Vec::new();
        crate::write(&doc_with(f.clone()), std::io::Cursor::new(&mut buf)).unwrap();
        let (back, _) = crate::read(std::io::Cursor::new(&buf)).unwrap();
        let got = &back.paragraphs().next().unwrap().runs[0].fmt;
        assert!(got.superscript, "上付きが消えた");
        assert_eq!(got.highlight.as_deref(), Some("yellow"), "蛍光ペンが消えた");
    }

    #[test]
    fn subscript_round_trips_too() {
        let f = CharFormat { subscript: true, ..Default::default() };
        let mut buf = Vec::new();
        crate::write(&doc_with(f.clone()), std::io::Cursor::new(&mut buf)).unwrap();
        let (back, _) = crate::read(std::io::Cursor::new(&buf)).unwrap();
        assert!(back.paragraphs().next().unwrap().runs[0].fmt.subscript);
    }
}

#[cfg(test)]
mod sect_tests {

    use crate::{parse_document_xml, write_document_xml};

    /// 途中で用紙の向きが変わる文書。**実物(LibreOffice Writer)の形**から写した
    fn two_sections() -> String {
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:r><w:t>縦の節の一つ目</w:t></w:r></w:p>
<w:p><w:pPr><w:sectPr><w:type w:val="nextPage"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:left="1134" w:right="1134" w:top="1134" w:bottom="1134"/></w:sectPr></w:pPr><w:r><w:t>縦の節の終わり</w:t></w:r></w:p>
<w:p><w:r><w:t>横の節</w:t></w:r></w:p>
<w:sectPr><w:type w:val="nextPage"/><w:pgSz w:orient="landscape" w:w="16838" w:h="11906"/><w:pgMar w:left="1701" w:right="1701" w:top="850" w:bottom="850"/></w:sectPr>
</w:body></w:document>"#.to_string()
    }

    #[test]
    fn a_mid_document_section_break_survives_saving() {
        // **前はここで消えていた。** 2つ目の sectPr が1つ目を上書きし、
        // 模型が節を1つしか持てなかったので、保存で区切りごと失われた
        let (doc, rep) = parse_document_xml(&two_sections());
        let ps: Vec<_> = doc.paragraphs().collect();
        assert!(ps[1].sect.is_some(), "途中の節を段落が持っていない");
        assert!(ps[0].sect.is_none() && ps[2].sect.is_none(),
            "関係の無い段落に節が付いた");
        assert!(doc.sect_raw.as_deref().is_some_and(|s| s.contains("landscape")),
            "最後の節が読めていない: {:?}", doc.sect_raw);
        assert!(rep.unsupported.iter().any(|(n, _)| n.contains("節の区切り")), "帳簿に出ていない");

        let out = write_document_xml(&doc);
        assert_eq!(out.matches("<w:sectPr").count(), 2, "節の数が変わった: {out}");
        assert!(out.contains(r#"w:w="11906""#), "途中の節の用紙が消えた");
        assert!(out.contains("landscape"), "最後の節の向きが消えた");
    }

    #[test]
    fn an_empty_paragraph_holding_only_a_break_survives() {
        // 区切り用の段落は中身が空なことがある。書き手は「既定のものは
        // 書かない」ので、**pPr を書く条件に節を入れ忘れると黙って消える**
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:pPr><w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr></w:pPr></w:p>
<w:p><w:r><w:t>後ろ</w:t></w:r></w:p>
<w:sectPr><w:pgSz w:w="16838" w:h="11906"/></w:sectPr>
</w:body></w:document>"#;
        let (doc, _) = parse_document_xml(xml);
        let out = write_document_xml(&doc);
        assert_eq!(out.matches("<w:sectPr").count(), 2,
            "中身の無い段落の節が消えた: {out}");
    }

    #[test]
    fn sections_do_not_multiply_over_two_round_trips() {
        let (doc, _) = parse_document_xml(&two_sections());
        let once = write_document_xml(&doc);
        let (doc2, _) = parse_document_xml(&once);
        let twice = write_document_xml(&doc2);
        assert_eq!(twice.matches("<w:sectPr").count(), 2, "二度目で節が増減した: {twice}");
        assert_eq!(doc2.paragraphs().filter(|p| p.sect.is_some()).count(), 1,
            "二度目で途中の節の数が変わった");
    }


    #[test]
    fn reads_the_section_kind() {
        // `w:type` が無ければ docx の既定は nextPage(改ページする)
        let xml = |ty: &str| format!(r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:pPr><w:sectPr>{ty}<w:pgSz w:w="11906" w:h="16838"/></w:sectPr></w:pPr><w:r><w:t>前</w:t></w:r></w:p>
<w:p><w:r><w:t>後</w:t></w:r></w:p>
<w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>
</w:body></w:document>"#);
        let read_list = |ty: &str| -> kumihan::SectionBreak {
            let (d, _) = parse_document_xml(&xml(ty));
            let p = d.paragraphs().next().unwrap();
            p.sect.clone().unwrap()
        };
        assert!(read_list(r#"<w:type w:val="continuous"/>"#).continuous,
            "continuous を読めていない");
        assert!(!read_list(r#"<w:type w:val="nextPage"/>"#).continuous,
            "nextPage を continuous と読んだ");
        assert!(!read_list("").continuous, "type 無しの既定が nextPage になっていない");
        // 原文はどの種類でもそのまま持ち越す
        assert!(read_list(r#"<w:type w:val="continuous"/>"#).raw.contains("continuous"),
            "原文から種類が落ちた");
    }

    #[test]
    fn a_single_section_document_is_unchanged() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:r><w:t>本文</w:t></w:r></w:p>
<w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>
</w:body></w:document>"#;
        let (doc, rep) = parse_document_xml(xml);
        assert!(doc.paragraphs().all(|p| p.sect.is_none()), "段落に節が付いた");
        assert!(!rep.unsupported.iter().any(|(n, _)| n.contains("節の区切り")),
            "節が1つなのに区切りを報告した: {:?}", rep.unsupported);
        assert_eq!(write_document_xml(&doc).matches("<w:sectPr").count(), 1);
    }
    #[test]
    fn paper_and_margins_read_and_come_back_on_save() {
        // sectPr を捨てると、保存で用紙設定とヘッダーの参照が消える
        let xml = r#"<w:document xmlns:w="x"><w:body>
            <w:p><w:r><w:t>本文</w:t></w:r></w:p>
            <w:sectPr><w:headerReference r:id="rId8"/>
              <w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/>
              <w:pgMar w:top="1134" w:right="851" w:bottom="1134" w:left="851"/>
            </w:sectPr></w:body></w:document>"#;
        let (doc, _) = crate::parse_document_xml(xml);
        let pg = doc.page.expect("用紙を読めていない");
        // 16838 twip = 297mm(A4 横)
        assert!((pg.w_mm - 297.0).abs() < 0.5, "幅: {}", pg.w_mm);
        assert!((pg.h_mm - 210.0).abs() < 0.5, "高さ: {}", pg.h_mm);
        assert!((pg.left_mm - 15.0).abs() < 0.5, "左余白: {}", pg.left_mm);
        assert!((pg.top_mm - 20.0).abs() < 0.5, "上余白: {}", pg.top_mm);

        let out = crate::write_document_xml(&doc);
        assert!(out.contains("headerReference"), "ヘッダーの参照が消えた");
        assert!(out.contains("w:pgSz"), "用紙が消えた");
    }

    #[test]
    fn columns_are_read() {
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p/>
            <w:sectPr><w:pgSz w:w="11906" w:h="16838"/>
              <w:cols w:num="2" w:space="425"/>
            </w:sectPr></w:body></w:document>"#;
        let (doc, _) = crate::parse_document_xml(xml);
        assert_eq!(doc.page.expect("用紙が無い").columns, 2, "段数が読めない");
    }

    #[test]
    fn a_document_without_paper_keeps_the_defaults() {
        let (doc, _) = crate::parse_document_xml(
            r#"<w:document xmlns:w="x"><w:body><w:p/></w:body></w:document>"#);
        assert!(doc.page.is_none());
        assert!(doc.sect_raw.is_none());
    }
}

#[cfg(test)]
mod shade_tests {
    use super::*;
    use kumihan::{Block, Document, Paragraph, Run};

    #[test]
    fn paragraph_shading_and_border_round_trip() {
        let mut p = Paragraph { style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, sect: None,
            line_spacing: 1.0,
            runs: vec![Run { text: "注意".into(), size_pt: Some(10.5), font: None,
                             fmt: Default::default() }],
            ..Default::default()
        };
        p.shade = Some("FFF2CC".into());
        p.boxed = true;
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false,
                           blocks: vec![Block::Para(p)] };
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        let p = back.paragraphs().next().unwrap();
        assert_eq!(p.shade.as_deref(), Some("FFF2CC"), "背景色が往復しない");
        assert!(p.boxed, "囲み枠が往復しない");
    }
}

#[cfg(test)]
mod bookmark_model_tests {
    use super::*;
    use kumihan::{Block, Document};

    #[test]
    fn bookmark_names_round_trip() {
        let mut d = Document::plain("表紙\n会社の説明\n終わり");
        if let Block::Para(p) = &mut d.blocks[1] {
            p.bookmarks.push("会社名".into());
        }
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        let bs: Vec<usize> = back.paragraphs().map(|p| p.bookmarks.len()).collect();
        assert_eq!(bs, vec![0, 1, 0], "しおりの付き先がずれた");
        assert_eq!(back.paragraphs().nth(1).unwrap().bookmarks[0], "会社名");
    }
}

#[cfg(test)]
mod ref_field_round_tests {
    use super::*;
    use kumihan::{Document, RefField};

    #[test]
    fn cross_references_round_trip() {
        let mut d = Document::plain("仕様は3ページを見る");
        let s0 = "仕様は".len();
        let e0 = "仕様は3ページ".len();
        d.apply_field(s0..e0, Some(RefField { name: "様式".into(), page: true }));
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        assert_eq!(back.body_text(), "仕様は3ページを見る", "見えている値が変わった");
        let f: Vec<_> = back.paragraphs().flat_map(|p| p.runs.iter())
            .filter_map(|r| r.fmt.field.clone().map(|f| (f, r.text.clone())))
            .collect();
        assert_eq!(f.len(), 1, "参照が往復しない");
        assert_eq!(f[0].0, RefField { name: "様式".into(), page: true });
        assert_eq!(f[0].1, "3ページ");
        // XML の上では Word のフィールド
        let out = write_document_xml(&d);
        assert!(out.contains("PAGEREF 様式"), "PAGEREF が無い: {out}");
    }

    #[test]
    fn reads_the_complex_field_form_word_writes() {
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p>
            <w:r><w:t>結果は</w:t></w:r>
            <w:r><w:fldChar w:fldCharType="begin"/></w:r>
            <w:r><w:instrText xml:space="preserve"> REF 結論 \h </w:instrText></w:r>
            <w:r><w:fldChar w:fldCharType="separate"/></w:r>
            <w:r><w:t>別紙のとおり</w:t></w:r>
            <w:r><w:fldChar w:fldCharType="end"/></w:r>
            <w:r><w:t>。</w:t></w:r>
        </w:p></w:body></w:document>"#;
        let (doc, _) = parse_document_xml(xml);
        assert_eq!(doc.body_text(), "結果は別紙のとおり。", "見えている値が繋がらない");
        let f: Vec<_> = doc.paragraphs().flat_map(|p| p.runs.iter())
            .filter(|r| r.fmt.field.is_some()).collect();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].text, "別紙のとおり");
        assert_eq!(f[0].fmt.field.as_ref().unwrap().name, "結論");
    }
}

#[cfg(test)]
mod partial_fmt_tests {
    use super::*;
    use kumihan::Document;

    #[test]
    fn partial_formatting_round_trips() {
        // 編集モデルが run 粒度になったので、段落の途中だけの太字が
        // docx に3つの run として入り、開き直しても残る
        let mut d = Document::plain("防火戸の仕様を確認");
        let s0 = "防火戸の".len();
        let e0 = "防火戸の仕様".len();
        d.apply_char_format(s0..e0, |f| f.bold = true);
        d.apply_size(s0..e0, |_| 14.0);
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        let runs: Vec<_> = back.paragraphs().next().unwrap().runs.iter()
            .map(|r| (r.text.clone(), r.fmt.bold, r.size_pt))
            .collect();
        assert_eq!(runs, vec![
            // 無指定(None)は**無指定のまま**往復する。以前はここが
            // 10.5 だった — 読み書きの両端が数を焼き込んでいた証拠
            ("防火戸の".into(), false, None),
            ("仕様".into(), true, Some(14.0)),
            ("を確認".into(), false, None),
        ], "部分書式が往復しない");
    }
}

#[cfg(test)]
mod vertical_round_tests {
    use super::*;
    use kumihan::Document;

    #[test]
    fn the_vertical_writing_flag_round_trips_and_clears() {
        let mut d = Document::plain("縦の検査");
        d.vertical = true;
        let mut buf = Vec::new();
        write(&d, Cursor::new(&mut buf)).unwrap();
        let (mut back, _) = read(Cursor::new(&buf)).unwrap();
        assert!(back.vertical, "縦書きが往復しない");
        back.vertical = false;
        let mut buf2 = Vec::new();
        write_with(&back, Some(Cursor::new(&buf)), Cursor::new(&mut buf2)).unwrap();
        let (b2, _) = read(Cursor::new(&buf2)).unwrap();
        assert!(!b2.vertical, "横に戻したのに縦のまま");
    }
}

#[cfg(test)]
mod sdt_round_tests {
    use super::*;
    use kumihan::{Document, Sdt, SdtKind};

    #[test]
    fn form_fields_round_trip() {
        let mut d = Document::plain("氏名: 山田 太郎");
        d.apply_char_format(8..21, |f| {
            f.sdt = Some(Box::new(Sdt {
                kind: SdtKind::Text,
                alias: "氏名".into(),
                ..Default::default()
            }))
        });
        let mut buf = Vec::new();
        write(&d, Cursor::new(&mut buf)).unwrap();
        let (back, _) = read(Cursor::new(&buf)).unwrap();
        assert_eq!(back.body_text(), "氏名: 山田 太郎", "本文が変わった");
        let p = back.paragraphs().next().unwrap();
        let r = p.runs.iter().find(|r| r.fmt.sdt.is_some()).expect("欄が無い");
        let sd = r.fmt.sdt.as_ref().unwrap();
        assert_eq!(sd.alias, "氏名");
        assert_eq!(sd.kind, SdtKind::Text);
        assert_eq!(r.text, "山田 太郎", "欄の中身が違う: {}", r.text);
    }

    #[test]
    fn a_custom_kind_round_trips_with_its_kind_even_when_named() {
        // うちだけの種類(jo:email)+「名前」ボタンの名は、
        // w:tag「jo:email:連絡先」に合成して両立させる
        let mut d = Document::plain("宛先: 未記入");
        d.apply_char_format(8..17, |f| {
            f.sdt = Some(Box::new(Sdt {
                kind: SdtKind::Email,
                alias: "連絡先".into(),
                tag: "連絡先".into(),
                ..Default::default()
            }))
        });
        let mut buf = Vec::new();
        write(&d, Cursor::new(&mut buf)).unwrap();
        let (back, _) = read(Cursor::new(&buf)).unwrap();
        let p = back.paragraphs().next().unwrap();
        let r = p.runs.iter().find(|r| r.fmt.sdt.is_some()).expect("欄が無い");
        let sd = r.fmt.sdt.as_ref().unwrap();
        assert_eq!(sd.kind, SdtKind::Email, "種類が落ちた(印が消えた?)");
        assert_eq!(sd.tag, "連絡先", "名前が落ちた");
        // 名前の無い独自種類は、昔どおり印だけ(既存の文書を壊さない)
        let mut d2 = Document::plain("宛先: 未記入");
        d2.apply_char_format(8..17, |f| {
            f.sdt = Some(Box::new(Sdt { kind: SdtKind::Email, ..Default::default() }))
        });
        let mut buf2 = Vec::new();
        write(&d2, Cursor::new(&mut buf2)).unwrap();
        let (back2, _) = read(Cursor::new(&buf2)).unwrap();
        let p2 = back2.paragraphs().next().unwrap();
        let r2 = p2.runs.iter().find(|r| r.fmt.sdt.is_some()).expect("欄が無い");
        assert_eq!(r2.fmt.sdt.as_ref().unwrap().kind, SdtKind::Email);
        assert_eq!(r2.fmt.sdt.as_ref().unwrap().tag, "jo:email");
    }

    #[test]
    fn a_dropdown_field_round_trips_with_its_choices() {
        let mut d = Document::plain("色: 赤");
        d.apply_char_format(5..8, |f| {
            f.sdt = Some(Box::new(Sdt {
                kind: SdtKind::Dropdown,
                alias: "色".into(),
                items: vec!["赤".into(), "青".into()],
                ..Default::default()
            }))
        });
        let mut buf = Vec::new();
        write(&d, Cursor::new(&mut buf)).unwrap();
        let (back, _) = read(Cursor::new(&buf)).unwrap();
        let p = back.paragraphs().next().unwrap();
        let sd = p.runs.iter().find_map(|r| r.fmt.sdt.as_ref()).expect("欄が無い");
        assert_eq!(sd.kind, SdtKind::Dropdown);
        assert_eq!(sd.items, vec!["赤".to_string(), "青".to_string()]);
    }

    #[test]
    fn our_own_kinds_round_trip_via_a_mark() {
        let mut d = Document::plain("mail@example.jp");
        d.apply_char_format(0..15, |f| {
            f.sdt = Some(Box::new(Sdt { kind: SdtKind::Email, ..Default::default() }))
        });
        let mut buf = Vec::new();
        write(&d, Cursor::new(&mut buf)).unwrap();
        let (back, _) = read(Cursor::new(&buf)).unwrap();
        let p = back.paragraphs().next().unwrap();
        let sd = p.runs.iter().find_map(|r| r.fmt.sdt.as_ref()).expect("欄が無い");
        assert_eq!(sd.kind, SdtKind::Email);
    }
}

#[cfg(test)]
mod ruby_round_tests {
    use super::*;
    use kumihan::Document;

    #[test]
    fn ruby_round_trips() {
        let mut d = Document::plain("組版の話");
        d.apply_char_format(0..6, |f| f.ruby = Some("くみはん".into()));
        let mut buf = Vec::new();
        write(&d, Cursor::new(&mut buf)).expect("書けない");
        let xml = {
            let mut z = zip::ZipArchive::new(Cursor::new(&buf)).unwrap();
            let mut s = String::new();
            use std::io::Read as _;
            z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
            s
        };
        assert!(xml.contains("<w:ruby>"), "w:ruby が無い");
        assert!(xml.contains("<w:rubyBase>"), "rubyBase が無い");
        let (back, _) = read(Cursor::new(&buf)).expect("読めない");
        assert_eq!(back.body_text(), "組版の話", "本文が変わった");
        let p = back.paragraphs().next().unwrap();
        let r = p.runs.iter().find(|r| r.text == "組版").expect("基底の run が無い");
        assert_eq!(r.fmt.ruby.as_deref(), Some("くみはん"), "読みが往復しない");
        assert!(
            p.runs.iter().any(|r| r.text.contains("の話") && r.fmt.ruby.is_none()),
            "ルビの無い字に読みが付いた"
        );
    }
}

#[cfg(test)]
mod props_round_tests {
    use super::*;
    use kumihan::Document;

    #[test]
    fn document_properties_round_trip_and_clear_when_emptied() {
        let mut d = Document::plain("本文");
        d.props.creator = "山田 <太郎>".into();
        d.props.title = "検査の書".into();
        let mut first = Vec::new();
        write(&d, Cursor::new(&mut first)).expect("書けない");
        let (mut back, _) = read(Cursor::new(&first)).expect("読めない");
        assert_eq!(back.props.creator, "山田 <太郎>", "作成者が往復しない");
        assert_eq!(back.props.title, "検査の書");
        back.props.title = String::new();
        back.props.keywords = "様式,検査".into();
        let mut second = Vec::new();
        write_with(&back, Some(Cursor::new(&first)), Cursor::new(&mut second)).unwrap();
        let (b2, _) = read(Cursor::new(&second)).unwrap();
        assert_eq!(b2.props.title, "", "空にしたのに残っている");
        assert_eq!(b2.props.keywords, "様式,検査");
        assert_eq!(b2.props.creator, "山田 <太郎>", "触らない欄が消えた");
    }
}

#[cfg(test)]
mod protection_round_tests {
    use super::*;
    use kumihan::Document;

    #[test]
    fn document_protection_round_trips_and_clears_when_removed() {
        let mut d = Document::plain("大事な様式");
        d.protection = Some("readOnly".into());
        let mut first = Vec::new();
        write(&d, Cursor::new(&mut first)).expect("書けない");
        let (mut back, _) = read(Cursor::new(&first)).expect("読めない");
        assert_eq!(back.protection.as_deref(), Some("readOnly"), "保護が往復しない");
        back.protection = None;
        let mut second = Vec::new();
        write_with(&back, Some(Cursor::new(&first)), Cursor::new(&mut second)).unwrap();
        let (back2, _) = read(Cursor::new(&second)).unwrap();
        assert_eq!(back2.protection, None, "解除したのに残っている");
    }
}

#[cfg(test)]
mod hyphenate_round_tests {
    use super::*;
    use kumihan::Document;

    #[test]
    fn the_hyphenation_flag_round_trips() {
        let mut d = Document::plain("hyphenation flag");
        d.hyphenate = true;
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (mut back, _) = read(buf).expect("読めない");
        assert!(back.hyphenate, "旗が往復しない");
        // 切って保存すれば設定からも消える
        back.hyphenate = false;
        let mut buf2 = Cursor::new(Vec::new());
        write(&back, &mut buf2).expect("書けない");
        buf2.set_position(0);
        let (back2, _) = read(buf2).expect("読めない");
        assert!(!back2.hyphenate, "切ったのに残っている");
    }
}

#[cfg(test)]
mod dropcap_round_tests {
    use super::*;
    use kumihan::{Block, Document};

    #[test]
    fn drop_caps_round_trip() {
        let mut d = Document::plain("春はあけぼの。やうやう白くなりゆく山際。\n次の段落");
        if let Block::Para(p) = &mut d.blocks[0] {
            p.dropcap = true;
        }
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        // 枠の段落と本文の段落は、読みで1つに合流する
        let ps: Vec<_> = back.paragraphs().collect();
        assert_eq!(ps.len(), 2, "段落の数が変わった");
        assert!(ps[0].dropcap, "ドロップキャップが消えた");
        let t: String = ps[0].runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(t, "春はあけぼの。やうやう白くなりゆく山際。", "本文が欠けた");
        // 頭の字の大きさは本文と同じに戻る(保存のたびに育たない)。
        // 本文が無指定なら頭も無指定 — 育ちようがない
        assert_eq!(ps[0].runs[0].size_pt, None, "頭の字の大きさが育った");
        assert!(!ps[1].dropcap);
        // XML の上では Word の作法(framePr の枠の段落)になっている
        let out = write_document_xml(&d);
        assert!(out.contains(r#"w:dropCap="drop""#), "framePr が無い: {out}");
    }
}

#[cfg(test)]
mod track_write_tests {
    use super::*;
    use kumihan::{Document, TRK_DEL_E, TRK_DEL_S, TRK_INS_E, TRK_INS_S};

    #[test]
    fn tracked_changes_become_ins_and_del() {
        let mut d = Document::plain(
            &format!("防火{TRK_DEL_S}戸{TRK_DEL_E}{TRK_INS_S}ドア{TRK_INS_E}の仕様"),
        );
        d.track_author = Some("検査".into());
        let out = write_document_xml(&d);
        assert!(out.contains(r#"<w:ins w:id="2" w:author="検査">"#), "w:ins が無い: {out}");
        assert!(out.contains("<w:delText"), "w:delText が無い: {out}");
        assert!(out.contains(r#"<w:del w:id="#), "w:del が無い: {out}");
        // 読み直すと「確定後の姿」(削除は消え、挿入は残る)
        let (back, rep) = parse_document_xml(&out);
        assert_eq!(back.body_text(), "防火ドアの仕様", "確定後の姿にならない");
        assert!(rep.unsupported.iter().any(|(n, _)| n.contains("変更履歴")),
            "履歴があると言っていない: {:?}", rep.unsupported);
    }
}

#[cfg(test)]
mod ink_tests {
    use super::*;
    use kumihan::{Block, Document, Stroke};

    #[test]
    fn pen_strokes_round_trip() {
        let mut d = Document::plain("本文");
        let st = Stroke {
            page: 2,
            highlighter: false,
            points: vec![(30.0, 40.0), (50.0, 60.5), (70.0, 55.0)],
        };
        let hl = Stroke {
            page: 0,
            highlighter: true,
            points: vec![(20.0, 25.0), (90.0, 25.0)],
        };
        // writer と同じ道: 図形を段落の控えに差し込んでから書く
        if let Block::Para(p) = &mut d.blocks[0] {
            p.anchors.push(ink_anchor_run(&st, 1));
            p.anchors.push(ink_anchor_run(&hl, 2));
        }
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert_eq!(back.ink.len(), 2, "筆の数が違う: {}", back.ink.len());
        let b0 = back.ink.iter().find(|s| !s.highlighter).expect("ペンが無い");
        assert_eq!(b0.page, 2, "ページ番号が消えた");
        assert_eq!(b0.points.len(), 3);
        for ((x, y), (wx, wy)) in b0.points.iter().zip(&st.points) {
            assert!((x - wx).abs() < 0.01 && (y - wy).abs() < 0.01,
                "座標がずれた: ({x},{y}) vs ({wx},{wy})");
        }
        assert!(back.ink.iter().any(|s| s.highlighter), "蛍光ペンの区別が消えた");
        // 控えには残っていない(二重にならない)
        assert!(back.paragraphs().next().unwrap().anchors.is_empty(),
            "筆が控えにも残っている");
    }
}

#[cfg(test)]
mod watermark_tests {
    use super::*;
    use kumihan::Document;

    #[test]
    fn the_watermark_round_trips_without_duplicating() {
        let mut d = Document::plain("本文");
        d.watermark = Some("社外秘".into());
        let mut first = Vec::new();
        write(&d, Cursor::new(&mut first)).expect("書けない");
        let (back, _) = read(Cursor::new(&first)).expect("読めない");
        assert_eq!(back.watermark.as_deref(), Some("社外秘"), "透かしが往復しない");
        // もう一度保存しても、図形は1つのまま
        let mut second = Vec::new();
        write_with(&back, Some(Cursor::new(&first)), Cursor::new(&mut second)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&second)).unwrap();
        let mut hx = String::new();
        z.by_name("word/johdr1.xml").unwrap().read_to_string(&mut hx).unwrap();
        assert_eq!(hx.matches("v:textpath").count(), 2,
            "図形が二重(shapetype+shape で2つが正: {})",
            hx.matches("v:textpath").count());
        let (back2, _) = read(Cursor::new(&second)).unwrap();
        assert_eq!(back2.watermark.as_deref(), Some("社外秘"));
    }

    #[test]
    fn removing_the_watermark_removes_the_shape() {
        let mut d = Document::plain("本文");
        d.watermark = Some("下書き".into());
        let mut first = Vec::new();
        write(&d, Cursor::new(&mut first)).unwrap();
        let (mut back, _) = read(Cursor::new(&first)).unwrap();
        back.watermark = None;
        let mut second = Vec::new();
        write_with(&back, Some(Cursor::new(&first)), Cursor::new(&mut second)).unwrap();
        let (back2, _) = read(Cursor::new(&second)).unwrap();
        assert_eq!(back2.watermark, None, "消したのに残っている");
    }
}

#[cfg(test)]
mod comment_tests {
    use super::*;
    use kumihan::{Block, Comment, Document};

    #[test]
    fn paragraph_comments_round_trip() {
        let mut d = Document::plain("一\n二\n三");
        if let Block::Para(p) = &mut d.blocks[1] {
            p.comments.push(Comment {
                author: "検査".into(),
                text: "ここは要確認。\n二行目の注記".into(),
            });
        }
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        let cs: Vec<usize> = back.paragraphs().map(|p| p.comments.len()).collect();
        assert_eq!(cs, vec![0, 1, 0], "コメントの付き先がずれた");
        let c = &back.paragraphs().nth(1).unwrap().comments[0];
        assert_eq!(c.author, "検査", "書いた人が消えた");
        assert_eq!(c.text, "ここは要確認。\n二行目の注記", "本文が変わった");
    }

    #[test]
    fn saving_twice_does_not_duplicate_comments() {
        let mut d = Document::plain("本文");
        if let Block::Para(p) = &mut d.blocks[0] {
            p.comments.push(Comment { author: "私".into(), text: "注記".into() });
        }
        let mut first = Vec::new();
        write(&d, Cursor::new(&mut first)).unwrap();
        let (back, _) = read(Cursor::new(&first)).unwrap();
        let mut second = Vec::new();
        write_with(&back, Some(Cursor::new(&first)), Cursor::new(&mut second)).unwrap();
        let (back2, _) = read(Cursor::new(&second)).unwrap();
        assert_eq!(back2.paragraphs().next().unwrap().comments.len(), 1,
            "保存のたびにコメントが増える");
        let mut z = zip::ZipArchive::new(Cursor::new(&second)).unwrap();
        let mut rels = String::new();
        z.by_name("word/_rels/document.xml.rels").unwrap().read_to_string(&mut rels).unwrap();
        assert_eq!(rels.matches("comments.xml").count(), 1, "関係が二重: {rels}");
    }
}

#[cfg(test)]
mod page_color_tests {
    use super::*;
    use kumihan::Document;

    #[test]
    fn page_color_round_trips_with_its_setting() {
        let mut d = Document::plain("本文");
        d.page_color = Some("E8F1F8".into());
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        assert_eq!(back.page_color.as_deref(), Some("E8F1F8"), "色が往復しない");
        // 見せる設定(displayBackgroundShape)が settings に入っている
        let mut buf2 = Cursor::new(Vec::new());
        write(&back, &mut buf2).expect("書けない");
        buf2.set_position(0);
        let mut z = zip::ZipArchive::new(buf2).unwrap();
        let mut st = String::new();
        z.by_name("word/settings.xml").unwrap().read_to_string(&mut st).unwrap();
        assert_eq!(st.matches("displayBackgroundShape").count(), 1,
            "設定が無いか二重: {st}");
    }

    #[test]
    fn the_originals_settings_survive_with_their_other_entries() {
        // settings を丸ごと作り直すと、原本の設定(既定のタブ幅など)が消える
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o: zip::write::FileOptions<'_, ()> = Default::default();
        let mut put = |n: &str, d: &[u8]| {
            zip.start_file(n, o).unwrap();
            zip.write_all(d).unwrap();
        };
        put("[Content_Types].xml", br#"<Types xmlns="ct"><Default Extension="xml" ContentType="application/xml"/></Types>"#);
        put("_rels/.rels", br#"<Relationships xmlns="r"/>"#);
        put("word/_rels/document.xml.rels", br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#);
        put("word/document.xml", r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>本文</w:t></w:r></w:p></w:body></w:document>"#.as_bytes());
        put("word/settings.xml", r#"<w:settings xmlns:w="x"><w:defaultTabStop w:val="840"/></w:settings>"#.as_bytes());
        let src = zip.finish().unwrap().into_inner();
        let (mut doc, _) = crate::read(Cursor::new(&src)).unwrap();
        doc.page_color = Some("FFF7DC".into());
        let mut out = Vec::new();
        crate::write_with(&doc, Some(Cursor::new(&src)), Cursor::new(&mut out)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
        let mut st = String::new();
        z.by_name("word/settings.xml").unwrap().read_to_string(&mut st).unwrap();
        assert!(st.contains("defaultTabStop"), "原本の設定が消えた: {st}");
        assert!(st.contains("displayBackgroundShape"), "見せる設定が付かない: {st}");
    }
}

#[cfg(test)]
mod bookmark_tests {
    use super::*;

    #[test]
    fn bookmark_and_comment_marks_survive_saving() {
        // 実物の様式はしおりで記入欄を指すものがある。黙って捨てると
        // 相互参照・コメントのアンカーが壊れる
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p>
            <w:bookmarkStart w:id="0" w:name="会社名"/>
            <w:r><w:t>日本フネン</w:t></w:r>
            <w:bookmarkEnd w:id="0"/>
            <w:commentRangeStart w:id="3"/>
            <w:r><w:t>要確認の箇所</w:t></w:r>
            <w:commentRangeEnd w:id="3"/>
            <w:r><w:commentReference w:id="3"/></w:r>
        </w:p></w:body></w:document>"#;
        let (doc, rep) = parse_document_xml(xml);
        assert!(rep.unsupported.iter().any(|(n, _)| n.contains("しおり・コメント")),
            "黙って扱った: {:?}", rep.unsupported);
        let out = write_document_xml(&doc);
        assert!(out.contains(r#"w:name="会社名""#), "しおりが消えた: {out}");
        assert!(out.contains("bookmarkEnd"), "しおりの終わりが消えた");
        assert!(out.contains("commentRangeStart"), "コメントの範囲が消えた");
        assert!(out.contains("commentReference"), "コメントのアンカーが消えた");
        assert!(out.contains("日本フネン") && out.contains("要確認の箇所"), "本文が消えた");
    }
}

#[cfg(test)]
mod list_level_tests {
    use super::*;
    use kumihan::{Block, Document, ListKind};

    #[test]
    fn list_depth_round_trips() {
        let mut d = Document::plain("親\n子");
        for (i, ind) in [(0usize, 0u8), (1, 2)] {
            if let Block::Para(p) = &mut d.blocks[i] {
                p.list = ListKind::Bullet;
                p.indent = ind;
            }
        }
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let inds: Vec<u8> = back.paragraphs().map(|p| p.indent).collect();
        assert_eq!(inds, vec![0, 2], "深さが往復しない");
    }

    #[test]
    fn a_list_without_ind_uses_ilvl_as_depth() {
        // Word のリストは w:ind を numbering.xml に置くので、段落側は ilvl だけのことが多い
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p><w:pPr>
            <w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr>
            </w:pPr><w:r><w:t>子の項目</w:t></w:r></w:p></w:body></w:document>"#;
        let (doc, _) = parse_document_xml(xml);
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.list, ListKind::Bullet);
        assert_eq!(p.indent, 1, "ilvl が深さに入らない");
    }

    #[test]
    fn tabs_round_trip() {
        // w:t の中に生のタブを書くと Word が潰す。要素(w:tab)で書く
        let d = Document::plain("項目\t値");
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert_eq!(back.body_text(), "項目\t値", "タブが消えた");
        // XML の上でも w:tab 要素になっている
        let mut buf2 = Cursor::new(Vec::new());
        write(&back, &mut buf2).expect("書けない");
        buf2.set_position(0);
        let mut z = zip::ZipArchive::new(buf2).unwrap();
        let mut s = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
        assert!(s.contains("<w:tab/>"), "タブが要素になっていない");
        assert!(!s.contains("項目\t"), "w:t に生のタブが残った");
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;
    use kumihan::{Block, Document, ParaStyle};

    #[test]
    fn headings_and_toc_lines_round_trip() {
        let mut d = Document::plain("表題\n本文\n目次の行");
        if let Block::Para(p) = &mut d.blocks[0] { p.style = ParaStyle::Heading(1); }
        if let Block::Para(p) = &mut d.blocks[2] { p.style = ParaStyle::Toc(2); }
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        let ps: Vec<ParaStyle> = back.paragraphs().map(|p| p.style).collect();
        assert_eq!(ps, vec![ParaStyle::Heading(1), ParaStyle::Body, ParaStyle::Toc(2)],
            "段落の役割が往復しない");
    }

    #[test]
    fn headings_from_japanese_word_also_read() {
        // 日本語版 Word の見出し1は style id が「1」。outlineLvl だけでも見出し
        let xml = r#"<w:document xmlns:w="x"><w:body>
            <w:p><w:pPr><w:pStyle w:val="1"/></w:pPr><w:r><w:t>甲</w:t></w:r></w:p>
            <w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>乙</w:t></w:r></w:p>
            <w:p><w:pPr><w:pStyle w:val="af0"/></w:pPr><w:r><w:t>丙</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let (doc, _) = parse_document_xml(xml);
        let ps: Vec<ParaStyle> = doc.paragraphs().map(|p| p.style).collect();
        assert_eq!(ps, vec![ParaStyle::Heading(1), ParaStyle::Heading(2), ParaStyle::Body]);
    }
}

#[cfg(test)]
mod hf_tests {
    use super::*;
    use kumihan::{Align, Document, Paragraph, Run, PAGE_MARK};

    fn para(s: &str) -> Paragraph {
        Paragraph { style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, sect: None,
            line_spacing: 1.0,
            runs: vec![Run { text: s.into(), size_pt: Some(10.5), font: None,
                             fmt: Default::default() }],
            ..Default::default()
        }
    }

    #[test]
    fn headers_and_footers_round_trip() {
        let mut d = Document::plain("本文");
        d.header.paragraphs = vec![para("社外秘")];
        let mut f = para(&format!("- {PAGE_MARK} -"));
        f.align = Align::Center;
        d.footer.paragraphs = vec![f];
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        assert_eq!(kumihan::paras_text(&back.header.paragraphs), "社外秘",
            "ヘッダーが往復しない");
        let ftxt = kumihan::paras_text(&back.footer.paragraphs);
        assert!(ftxt.contains(PAGE_MARK), "ページ番号の印が消えた: {ftxt:?}");
        assert_eq!(back.footer.paragraphs[0].align, Align::Center,
            "フッターの揃えが消えた");
        assert_eq!(back.header.part.as_deref(), Some("word/johdr1.xml"));
        assert_eq!(texts_of(&back), vec!["本文"], "本文が変わった");
    }

    fn texts_of(d: &Document) -> Vec<String> {
        d.paragraphs()
            .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn a_page_number_in_a_complex_field_reads() {
        // Word は PAGE を fldChar(begin/instrText/separate/計算済み/end)で書く
        let xml = r#"<w:hdr xmlns:w="x"><w:p>
            <w:r><w:fldChar w:fldCharType="begin"/></w:r>
            <w:r><w:instrText xml:space="preserve"> PAGE </w:instrText></w:r>
            <w:r><w:fldChar w:fldCharType="separate"/></w:r>
            <w:r><w:t>7</w:t></w:r>
            <w:r><w:fldChar w:fldCharType="end"/></w:r>
        </w:p></w:hdr>"#;
        let (doc, _) = parse_document_xml(xml);
        let t = doc.body_text();
        assert!(!t.contains('7'), "計算済みの見た目(7)が本文へ漏れた: {t:?}");
        assert!(t.contains(PAGE_MARK), "PAGE が印にならない: {t:?}");
    }

    #[test]
    fn unsupported_fields_are_reported_and_dropped() {
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p>
            <w:fldSimple w:instr=" DATE "><w:r><w:t>2026/08/03</w:t></w:r></w:fldSimple>
        </w:p></w:body></w:document>"#;
        let (doc, rep) = parse_document_xml(xml);
        assert!(!doc.body_text().contains("2026"), "計算済みの見た目が漏れた");
        assert!(rep.unsupported.iter().any(|(n, _)| n.contains("フィールド")),
            "黙って落とした: {:?}", rep.unsupported);
    }

    #[test]
    fn the_page_count_field_round_trips() {
        use kumihan::PAGES_MARK;
        let mut d = Document::plain("本文");
        d.footer.paragraphs = vec![para(&format!("{PAGE_MARK} / {PAGES_MARK}"))];
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(rep.is_lossless(), "未対応: {:?}", rep.unsupported);
        let t = kumihan::paras_text(&back.footer.paragraphs);
        assert!(t.contains(PAGE_MARK) && t.contains(PAGES_MARK),
            "印が往復しない: {t:?}");
    }

    /// 原本に header1.xml を持つ最小の docx
    fn docx_with_header(header_xml: &[u8]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o: zip::write::FileOptions<'_, ()> = Default::default();
        let mut put = |n: &str, d: &[u8]| {
            zip.start_file(n, o).unwrap();
            zip.write_all(d).unwrap();
        };
        put("[Content_Types].xml", br#"<Types xmlns="ct"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#);
        put("_rels/.rels", br#"<Relationships xmlns="r"/>"#);
        put("word/_rels/document.xml.rels", br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#);
        put("word/document.xml", r#"<w:document xmlns:w="x" xmlns:r="y"><w:body><w:p><w:r><w:t>本文</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rId9"/><w:pgSz w:w="11906" w:h="16838"/></w:sectPr></w:body></w:document>"#.as_bytes());
        put("word/header1.xml", header_xml);
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn writes_back_into_the_original_header_part() {
        let src = docx_with_header(
            r#"<w:hdr xmlns:w="x"><w:p><w:r><w:t>旧いヘッダー</w:t></w:r></w:p></w:hdr>"#.as_bytes());
        let (mut doc, _) = crate::read(Cursor::new(&src)).unwrap();
        assert_eq!(doc.header.part.as_deref(), Some("word/header1.xml"));
        assert_eq!(kumihan::paras_text(&doc.header.paragraphs), "旧いヘッダー");
        kumihan::set_paras_text(&mut doc.header.paragraphs, "新しいヘッダー");
        let mut out = Vec::new();
        crate::write_with(&doc, Some(Cursor::new(&src)), Cursor::new(&mut out)).unwrap();
        let z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
        let names: Vec<String> = {
            let mut z2 = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
            (0..z2.len()).map(|i| z2.by_index(i).unwrap().name().to_string()).collect()
        };
        drop(z);
        assert_eq!(names.iter().filter(|n| *n == "word/header1.xml").count(), 1,
            "部品が二重になった: {names:?}");
        let mut z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
        let mut s = String::new();
        z.by_name("word/header1.xml").unwrap().read_to_string(&mut s).unwrap();
        assert!(s.contains("新しいヘッダー"), "部品に書き戻っていない: {s}");
        let mut rels = String::new();
        z.by_name("word/_rels/document.xml.rels").unwrap().read_to_string(&mut rels).unwrap();
        assert!(!rels.contains("rIdJOhdr"), "原本の参照に重ねて足した: {rels}");
        let (back, _) = crate::read(Cursor::new(&out)).unwrap();
        assert_eq!(kumihan::paras_text(&back.header.paragraphs), "新しいヘッダー",
            "読み直すと消えている");
    }

    #[test]
    fn a_header_with_a_table_is_carried_over_untouched() {
        // まだ持てないもの(表)が入った部品は編集の対象にせず、原文のまま生かす
        let orig = r#"<w:hdr xmlns:w="x"><w:tbl><w:tr><w:tc><w:p><w:r><w:t>枠</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:hdr>"#.as_bytes();
        let src = docx_with_header(orig);
        let (doc, rep) = crate::read(Cursor::new(&src)).unwrap();
        assert!(doc.header.paragraphs.is_empty(), "編集できない部品を編集の対象にした");
        assert!(rep.unsupported.iter().any(|(n, _)| n.contains("ヘッダーの表")),
            "黙って隠した: {:?}", rep.unsupported);
        let mut out = Vec::new();
        crate::write_with(&doc, Some(Cursor::new(&src)), Cursor::new(&mut out)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
        let mut s = String::new();
        z.by_name("word/header1.xml").unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s.as_bytes(), orig, "触っていない部品が変わった");
    }

    #[test]
    fn saving_twice_does_not_duplicate_relations_or_parts() {
        let mut d = Document::plain("本文");
        d.footer.paragraphs = vec![para(&PAGE_MARK.to_string())];
        let mut first = Vec::new();
        crate::write(&d, Cursor::new(&mut first)).unwrap();
        // 開き直さず、同じモデルからもう一度保存(アプリの上書き保存と同じ形)
        let mut second = Vec::new();
        crate::write_with(&d, Some(Cursor::new(&first)), Cursor::new(&mut second)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&second)).unwrap();
        let mut rels = String::new();
        z.by_name("word/_rels/document.xml.rels").unwrap().read_to_string(&mut rels).unwrap();
        assert_eq!(rels.matches("rIdJOftr").count(), 1, "関係が二重: {rels}");
        let (back, _) = crate::read(Cursor::new(&second)).unwrap();
        assert!(kumihan::paras_text(&back.footer.paragraphs).contains(PAGE_MARK));
    }
}

#[cfg(test)]
mod image_insert_tests {
    use super::*;
    use kumihan::{Block, Document, InlineImage, Paragraph, Run};

    fn png_bytes() -> Vec<u8> {
        // 中身は問わない(読み書きは実体を素通しする)。頭のPNG印だけ本物
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        v.extend_from_slice(&[1, 2, 3, 4, 5]);
        v
    }

    #[test]
    fn an_inserted_image_is_saved_as_a_part_and_reads_back() {
        let mut p = Paragraph { style: Default::default(), comments: Vec::new(), bookmarks: Vec::new(), dropcap: false, sect: None,
            line_spacing: 1.0,
            runs: vec![Run { text: "ロゴの下".into(), size_pt: Some(10.5), font: None,
                             fmt: Default::default() }],
            ..Default::default()
        };
        p.images_new.push(InlineImage {
            bytes: std::sync::Arc::new(png_bytes()),
            w_mm: 50.0,
            h_mm: 30.0,
            tex: None,
                    src: None,
        });
        let d = Document { size_pt: None, note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), shapes: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false,
                           blocks: vec![Block::Para(p)] };
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        let first = buf.into_inner();

        let (back, _) = read(Cursor::new(first.clone())).expect("読めない");
        let bp = back.paragraphs().next().unwrap();
        assert_eq!(bp.images.len(), 1, "挿した画像が読み直せない");
        assert_eq!(*bp.images[0].bytes, png_bytes(), "画像の実体が変わった");
        assert!((bp.images[0].w_mm - 50.0).abs() < 0.5, "大きさが変わった");
        assert!(bp.images_new.is_empty(), "読み直しで二重の持ち場に入った");

        // アプリの保存と同じ形(原本を渡す)でもう一往復しても、壊れず残る
        let mut buf2 = Cursor::new(Vec::new());
        write_with(&back, Some(Cursor::new(&first)), &mut buf2).expect("書けない");
        buf2.set_position(0);
        let (back2, _) = read(buf2).expect("読み直せない");
        assert_eq!(back2.paragraphs().next().unwrap().images.len(), 1,
            "二度目の保存で画像が消えた");
    }

    /// **数式は絵と原文の二枚組で往復する。** 絵は組んだ結果でしかないので、
    /// 原文(LaTeX)が戻らないと開き直したとき直せない(絵を消して打ち直しになる)。
    /// docx の画像の代替テキスト(wp:docPr descr)に積んで運ぶ —
    /// 渡した先の Word では絵として見え、こちらでは式として直せる
    #[test]
    fn equations_round_trip_with_their_source() {
        let shiki = r"\frac{a+b}{2} < \sqrt{x^2} & \alpha";  // < と & も逃がせるか
        let mut p = Paragraph { line_spacing: 1.0, ..Default::default() };
        p.images_new.push(InlineImage {
            bytes: std::sync::Arc::new(png_bytes()),
            w_mm: 12.0,
            h_mm: 8.0,
            tex: Some(shiki.to_string()),
                    src: None,
        });
        let d = Document { blocks: vec![Block::Para(p)], ..Default::default() };
        let mut buf = Cursor::new(Vec::new());
        write(&d, &mut buf).expect("書けない");
        let first = buf.into_inner();

        let (back, _) = read(Cursor::new(first.clone())).expect("読めない");
        let im = &back.paragraphs().next().unwrap().images[0];
        assert_eq!(im.tex.as_deref(), Some(shiki), "数式の原文が往復しない");

        // 原本を渡すもう一往復でも消えない(アプリの保存と同じ形)
        let mut buf2 = Cursor::new(Vec::new());
        write_with(&back, Some(Cursor::new(&first)), &mut buf2).expect("書けない");
        buf2.set_position(0);
        let (back2, _) = read(buf2).expect("読み直せない");
        assert_eq!(back2.paragraphs().next().unwrap().images[0].tex.as_deref(), Some(shiki),
            "二度目の保存で数式の原文が消えた");
    }

    /// **普通の画像を数式と読み違えない。** 人が書いた説明文が代替テキストに
    /// 入っていても、印が無ければ原文として拾わない
    #[test]
    fn an_image_with_a_caption_does_not_become_an_equation() {
        let mut media = std::collections::BTreeMap::new();
        media.insert("rId9".to_string(), std::sync::Arc::new(png_bytes()));
        let raw = |descr: &str| format!(
            r#"<wp:inline><wp:extent cx="360000" cy="180000"/><wp:docPr id="1" name="図1"{descr}/><a:blip r:embed="rId9"/></wp:inline>"#
        );
        // 印のある物だけが数式
        let im = crate::read::image_of(&raw(r#" descr="officework:tex:\frac{1}{2}""#), &media).unwrap();
        assert_eq!(im.tex.as_deref(), Some(r"\frac{1}{2}"), "印つきを拾えない");
        // 人の説明文は式ではない
        let im = crate::read::image_of(&raw(r#" descr="会社のロゴ""#), &media).unwrap();
        assert_eq!(im.tex, None, "人の説明文を数式の原文と読み違えた");
        // 代替テキストが無いのも当然 None
        let im = crate::read::image_of(&raw(""), &media).unwrap();
        assert_eq!(im.tex, None, "無いものを拾った");
    }

}

#[cfg(test)]
mod footnote_report_tests {
    use super::*;

    /// 脚注の**印**は run に持ち、保存で原文どおりの位置へ返す。
    /// 脚注の**文章**(word/footnotes.xml)は原本のまま持ち越される部品なので
    /// 触らない — 落ちていたのは本文の印だけだった。
    /// 2026-08-10、genoffice の読み手と実物 27 枚を突き合わせて分かった穴。
    fn body(inner: &str) -> String {
        format!(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{inner}</w:body></w:document>"#
        )
    }


    /// **仕切り線の定義は脚注ではない。** `word/footnotes.xml` の頭には
    /// `w:type="separator"` と `continuationSeparator` が必ず入っていて、
    /// これを数に入れると番号が2つずれる(実物2枚とも入っていた)
    #[test]
    fn the_separator_line_is_not_counted_as_a_footnote() {
        let xml = concat!(
            r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
            r#"<w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:t>―</w:t></w:r></w:p></w:footnote>"#,
            r#"<w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:t>―</w:t></w:r></w:p></w:footnote>"#,
            r#"<w:footnote w:id="20"><w:p><w:r><w:t>一つ目の脚注。</w:t></w:r></w:p></w:footnote>"#,
            r#"<w:footnote w:id="21"><w:p><w:r><w:t>二つ目の脚注。</w:t></w:r></w:p></w:footnote>"#,
            r#"</w:footnotes>"#,
        );
        let (notes, taken) = parse_notes(xml, false, &Default::default());
        // 仕切りは注に数えないが、**id は取られている**ので控える
        assert!(taken.contains(&"-1".to_string()) && taken.contains(&"0".to_string()),
            "仕切り線の id を控えていない: {taken:?}");
        assert_eq!(notes.len(), 2, "仕切り線を脚注に数えた: {:?}",
            notes.iter().map(|n| n.id.clone()).collect::<Vec<_>>());
        assert_eq!(notes[0].id, "20");
        let t: String = notes[0].paragraphs.iter()
            .flat_map(|p| p.runs.iter().map(|r| r.text.as_str())).collect();
        assert_eq!(t, "一つ目の脚注。", "脚注の文章が読めていない");
        assert!(!notes[0].endnote);
    }

    #[test]
    fn footnote_marks_are_logged() {
        let xml = body(
            r#"<w:p><w:r><w:t>本文</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r></w:p>"#,
        );
        let (doc, rep) = parse_document_xml(&xml);
        assert_eq!(doc.body_text(), "本文", "本文が変わった(印は字ではない)");
        let note = rep.unsupported.iter().find(|(n, _)| n.contains("脚注"))
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| panic!("脚注の印を黙って通した: {:?}", rep.unsupported));
        // **もう起きない損を帳簿が言い続けてはいけない**
        assert!(!note.contains("保存で失われる"),
            "もう起きない損を帳簿が言っている: {note}");
        assert!(write_document_xml(&doc).contains(r#"w:id="1""#), "保存で印が消えた");
    }


    /// **実物(pandoc / LibreOffice Writer)から写した形。** どちらも
    /// `<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr>
    ///  <w:footnoteReference w:id="N"/></w:r>` で一致していた
    fn two_marks() -> String {
        body(concat!(
            r#"<w:p><w:r><w:t>本文の一つ目です</w:t></w:r>"#,
            r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="20"/></w:r>"#,
            r#"<w:r><w:t>。同じ段落にもう一つ</w:t></w:r>"#,
            r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="21"/></w:r>"#,
            r#"<w:r><w:t>。</w:t></w:r></w:p>"#,
        ))
    }

    /// 印は**元の位置**へ返る。段落の頭へ寄せてはいけない —
    /// 脚注は「どの語に付いた注か」が意味そのものなので、
    /// 数式で使った控え(anchors)の作法はここでは使えない
    #[test]
    fn footnote_marks_return_to_their_original_place() {
        let (doc, _) = parse_document_xml(&two_marks());
        let out = write_document_xml(&doc);
        let positions = |s: &str| out.find(s).unwrap_or_else(|| panic!("{s} が無い: {out}"));
        assert!(positions("本文の一つ目です") < positions(r#"w:id="20""#), "一つ目の印が前へ出た");
        assert!(positions(r#"w:id="20""#) < positions("同じ段落にもう一つ"), "一つ目の印が後ろへ流れた");
        assert!(positions("同じ段落にもう一つ") < positions(r#"w:id="21""#), "二つ目の印が前へ出た");
    }

    /// **id は振り直さない。** 書き手ごとに番号の付け方が違い
    /// (pandoc は 20 番台、LibreOffice は 2 番台)、
    /// footnotes.xml 側と番号で繋がっているので、振り直すと切れる
    #[test]
    fn footnote_ids_come_back_verbatim() {
        let (doc, _) = parse_document_xml(&two_marks());
        let out = write_document_xml(&doc);
        assert!(out.contains(r#"w:id="20""#) && out.contains(r#"w:id="21""#),
            "id が変わった: {out}");
        assert_eq!(out.matches("<w:footnoteReference").count(), 2, "印の数が変わった");
    }

    /// 印の run は**字を持たない**。本文の字としては数えない
    #[test]
    fn a_mark_does_not_become_body_text() {
        let (doc, _) = parse_document_xml(&two_marks());
        assert_eq!(doc.body_text(), "本文の一つ目です。同じ段落にもう一つ。",
            "印が本文の字に混ざった: {:?}", doc.body_text());
        let p = doc.paragraphs().next().unwrap();
        assert_eq!(p.runs.iter().filter(|r| r.fmt.footnote.is_some()).count(), 2,
            "印の run が2つ無い");
    }

    /// 文末脚注は脚注と別の札で返る(混ぜると別物になる)
    #[test]
    fn endnotes_come_back_as_endnotes() {
        let xml = body(r#"<w:p><w:r><w:t>本文</w:t></w:r><w:r><w:endnoteReference w:id="7"/></w:r></w:p>"#);
        let (doc, _) = parse_document_xml(&xml);
        let p = doc.paragraphs().next().unwrap();
        assert!(p.runs.iter().any(|r| r.fmt.footnote.as_ref().is_some_and(|f| f.endnote)),
            "文末脚注の印になっていない");
        let out = write_document_xml(&doc);
        assert!(out.contains("<w:endnoteReference"), "脚注に化けた: {out}");
        assert!(!out.contains("<w:footnoteReference"), "脚注も出た: {out}");
    }

    /// 二度往復しても増えも減りもしない
    #[test]
    fn footnote_marks_are_stable_over_two_round_trips() {
        let (doc, _) = parse_document_xml(&two_marks());
        let once = write_document_xml(&doc);
        let (doc2, _) = parse_document_xml(&once);
        let twice = write_document_xml(&doc2);
        assert_eq!(twice.matches("<w:footnoteReference").count(), 2,
            "二度目で印が増減した: {twice}");
        assert_eq!(doc2.body_text(), "本文の一つ目です。同じ段落にもう一つ。");
    }

    /// id の無い印は指す先が引けない。**作り話をせず落として報告する**
    #[test]
    fn a_mark_without_an_id_is_dropped_and_reported() {
        let xml = body(r#"<w:p><w:r><w:t>本文</w:t></w:r><w:r><w:footnoteReference/></w:r></w:p>"#);
        let (doc, rep) = parse_document_xml(&xml);
        assert!(doc.paragraphs().next().unwrap().runs.iter()
            .all(|r| r.fmt.footnote.is_none()), "id が無いのに印を作った");
        assert!(rep.unsupported.iter().any(|(n, _)| n.contains("id が無く")),
            "落としたのに報告していない: {:?}", rep.unsupported);
    }

    #[test]
    fn endnote_marks_are_logged_too() {
        let xml = body(r#"<w:p><w:r><w:endnoteReference w:id="2"/></w:r></w:p>"#);
        let (_, rep) = parse_document_xml(&xml);
        assert!(
            rep.unsupported.iter().any(|(n, _)| n.contains("脚注")),
            "文末脚注の印を黙って落とした: {:?}",
            rep.unsupported
        );
    }

    /// 節が2つある文書。模型は1つしか持てないので、保存で片方が消える。
    /// **消えること自体は直せていない** — 黙って消さないことだけを守る。
    #[test]
    fn the_second_section_break_is_logged() {
        let sect = r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>"#;
        let xml = body(&format!(
            r#"<w:p><w:pPr>{sect}</w:pPr></w:p><w:p><w:r><w:t>次の節</w:t></w:r></w:p>{sect}"#
        ));
        let (_, rep) = parse_document_xml(&xml);
        assert!(
            rep.unsupported.iter().any(|(n, _)| n.contains("節の区切り")),
            "2つ目の節を黙って捨てた: {:?}",
            rep.unsupported
        );
    }

    /// 数式は原文を控えて保存で返す。**組版はしないが、失いもしない。**
    /// 帳簿には出し続ける(読めてはいないので)が、
    /// **もう起きない損(平文になる)を書いてはいけない。**
    #[test]
    fn equations_are_logged() {
        let xml = body(
            r#"<w:p><m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><m:r><m:t>E=mc</m:t></m:r></m:oMath></w:p>"#,
        );
        let (doc, rep) = parse_document_xml(&xml);
        let note = rep
            .unsupported
            .iter()
            .find(|(n, _)| n.contains("数式"))
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| panic!("数式を黙って通した: {:?}", rep.unsupported));
        assert!(
            !note.contains("平文"),
            "もう起きない損を帳簿が言い続けている: {note}"
        );
        assert!(
            write_document_xml(&doc).contains("E=mc"),
            "保存で数式が失われた"
        );
    }

    /// **空の段落 `<w:p/>`。** Word が中身の無い段落をこの形で書く。
    /// Start の枝にしか目が無いと丸ごと落ちる — xlsx の sheetView と同じ形の穴。
    /// 2026-08-10、他人の docx(ONLYOFFICE の試験文書)で 76 段落中 28 個がこれだった。
    #[test]
    fn an_empty_paragraph_reads_in_self_closing_form_too() {
        let xml = body(
            r#"<w:p><w:r><w:t>上</w:t></w:r></w:p><w:p/><w:p><w:r><w:t>下</w:t></w:r></w:p>"#,
        );
        let (doc, _) = parse_document_xml(&xml);
        assert_eq!(doc.paragraphs().count(), 3, "空行が落ちた(段落の番号がずれる)");
        assert_eq!(doc.body_text(), "上\n\n下", "空行が本文に出ない");
    }

    #[test]
    fn an_empty_paragraph_with_attributes_still_reads() {
        // Word は改訂の印を属性で付けたまま自己完結の形にする
        let xml = body(r#"<w:p w:rsidR="00A1"/><w:p><w:r><w:t>本文</w:t></w:r></w:p>"#);
        let (doc, _) = parse_document_xml(&xml);
        assert_eq!(doc.paragraphs().count(), 2, "属性が付くと見落とす");
    }

    #[test]
    fn an_empty_paragraph_inside_a_table_cell_reads() {
        let xml = body(
            r#"<w:tbl><w:tr><w:tc><w:p/><w:p><w:r><w:t>中</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        );
        let (doc, _) = parse_document_xml(&xml);
        let t = doc.tables().next().expect("表が無い");
        assert_eq!(t.rows[0][0].paragraphs.len(), 2, "セルの中の空行が落ちた");
    }

    /// 空の段落は**保存でも残る**(往復して数が変わらない)。
    #[test]
    fn an_empty_paragraph_survives_a_round_trip() {
        let xml = body(r#"<w:p><w:r><w:t>上</w:t></w:r></w:p><w:p/><w:p/><w:p><w:r><w:t>下</w:t></w:r></w:p>"#);
        let (doc, _) = parse_document_xml(&xml);
        let mut buf = Vec::new();
        write(&doc, Cursor::new(&mut buf)).unwrap();
        let (back, _) = read(Cursor::new(&buf)).unwrap();
        assert_eq!(back.paragraphs().count(), 4, "保存で空行が詰まった");
        assert_eq!(back.body_text(), "上\n\n\n下");
    }

    #[test]
    fn no_footnotes_leaves_the_log_empty() {
        let xml = body(
            r#"<w:p><w:r><w:t>ただの本文</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>"#,
        );
        let (_, rep) = parse_document_xml(&xml);
        assert!(rep.is_lossless(), "何も無いのに帳簿が立った: {:?}", rep.unsupported);
    }
}





#[cfg(test)]
mod note_fmt_tests {

    fn fmt(settings: &str) -> (kumihan::NoteNumFmt, kumihan::NoteNumFmt) {
        use std::io::{Cursor, Write};
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o: zip::write::FileOptions<'_, ()> = Default::default();
        let mut put = |n: &str, d: &[u8]| {
            zip.start_file(n, o).unwrap();
            zip.write_all(d).unwrap();
        };
        put("[Content_Types].xml", br#"<Types xmlns="ct"/>"#);
        put("_rels/.rels", br#"<Relationships xmlns="r"/>"#);
        put("word/document.xml",
            r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>ほ</w:t></w:r></w:p></w:body></w:document>"#.as_bytes());
        let s = format!(r#"<w:settings xmlns:w="x">{settings}</w:settings>"#);
        put("word/settings.xml", s.as_bytes());
        let z = zip.finish().unwrap().into_inner();
        let (d, _) = crate::read(Cursor::new(z)).unwrap();
        (d.footnote_fmt, d.endnote_fmt)
    }

    /// **docx の既定を知っているのは読み手。** settings が黙っていれば
    /// 脚注は算用数字、**文末脚注はローマ数字の小文字**(Word も LibreOffice もそう)
    #[test]
    fn silent_settings_make_endnotes_roman() {
        let (f, e) = fmt("");
        assert_eq!(f, kumihan::NoteNumFmt::Decimal);
        assert_eq!(e, kumihan::NoteNumFmt::LowerRoman, "文末脚注の既定が算用数字になっている");
    }

    /// 実物(both-notes.docx)がこの形で書いていた
    #[test]
    fn reads_the_number_format_from_settings() {
        let (f, e) = fmt(concat!(
            r#"<w:footnotePr><w:numFmt w:val="decimal"/></w:footnotePr>"#,
            r#"<w:endnotePr><w:numFmt w:val="lowerRoman"/></w:endnotePr>"#,
        ));
        assert_eq!(f, kumihan::NoteNumFmt::Decimal);
        assert_eq!(e, kumihan::NoteNumFmt::LowerRoman);
    }

    /// **札の中の numFmt だけを見る。** 文書には他にも numFmt があるので、
    /// 範囲を切らずに探すと隣の設定を拾う
    #[test]
    fn does_not_pick_up_the_neighbouring_setting() {
        let (f, e) = fmt(concat!(
            r#"<w:footnotePr><w:numFmt w:val="upperLetter"/></w:footnotePr>"#,
            r#"<w:endnotePr><w:numFmt w:val="decimal"/></w:endnotePr>"#,
        ));
        assert_eq!(f, kumihan::NoteNumFmt::UpperLetter, "脚注が隣を拾った");
        assert_eq!(e, kumihan::NoteNumFmt::Decimal, "文末脚注が隣を拾った");
    }
}


/// 脚注を**新しく作る**(部品ごと書き出す)。
///
/// 読み込んだ注は原本の部品がそのまま持ち越されるので触らない。
/// 足したぶんだけ、`footnotes.xml`・`[Content_Types]` の宣言・
/// `document.xml.rels` の関係を用意する。**この3つは揃っていないと
/// Word が注を出さない**(部品だけあっても、宣言や関係が無ければ無視される)
#[cfg(test)]
mod add_note_tests {
    use std::io::{Cursor, Read, Write};

    /// 注を1つも持たない素の docx(手で組む。現物には依らせない)
    fn plain_docx() -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o: zip::write::FileOptions<'_, ()> = Default::default();
        let mut put = |n: &str, d: &[u8]| {
            zip.start_file(n, o).unwrap();
            zip.write_all(d).unwrap();
        };
        put("[Content_Types].xml",
            br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#);
        put("_rels/.rels", br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#);
        put("word/_rels/document.xml.rels",
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#);
        put("word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>ほんぶん</w:t></w:r></w:p></w:body></w:document>"#.as_bytes());
        zip.finish().unwrap().into_inner()
    }

    fn parts(z: &[u8], name: &str) -> Option<String> {
        let mut a = zip::ZipArchive::new(Cursor::new(z.to_vec())).ok()?;
        let mut f = a.by_name(name).ok()?;
        let mut s = String::new();
        f.read_to_string(&mut s).ok()?;
        Some(s)
    }

    /// 脚注を足して保存する。**3つ揃って初めて Word が出す**
    #[test]
    fn adding_a_footnote_to_a_document_without_notes_creates_the_parts() {
        let src = plain_docx();
        let (mut doc, _) = crate::read(Cursor::new(&src)).unwrap();
        let body = kumihan::Paragraph {
            runs: vec![kumihan::Run { text: "足した脚注の文章。".into(), size_pt: Some(9.0),
                                      font: None, fmt: Default::default() }],
            line_spacing: 1.0,
            ..Default::default()
        };
        let fr = doc.add_footnote(false, vec![body]);
        // 本文の末尾に印を置く
        if let Some(kumihan::Block::Para(p)) = doc.blocks.last_mut() {
            p.runs.push(kumihan::Run {
                text: String::new(), size_pt: Some(10.5), font: None,
                fmt: kumihan::CharFormat { footnote: Some(fr.clone()), ..Default::default() },
            });
        }
        let mut out = Vec::new();
        crate::write_with(&doc, Some(Cursor::new(&src)), Cursor::new(&mut out)).unwrap();

        // 1) 部品そのもの
        let fx = parts(&out, "word/footnotes.xml").expect("footnotes.xml が無い");
        assert!(fx.contains("足した脚注の文章。"), "脚注の文章が入っていない: {fx}");
        // **仕切り線が無いと Word は注を出さない**
        assert!(fx.contains(r#"w:type="separator""#), "仕切り線の定義が無い: {fx}");
        assert!(fx.contains(r#"w:type="continuationSeparator""#), "続きの仕切りが無い: {fx}");
        // 2) 宣言
        let ct = parts(&out, "[Content_Types].xml").unwrap();
        assert!(ct.contains("/word/footnotes.xml"), "宣言が無い: {ct}");
        // 3) 関係
        let rels = parts(&out, "word/_rels/document.xml.rels").unwrap();
        assert!(rels.contains("footnotes.xml"), "関係が無い: {rels}");
        // 本文の印
        let dx = parts(&out, "word/document.xml").unwrap();
        assert!(dx.contains("<w:footnoteReference"), "本文に印が無い: {dx}");

        // 読み直して戻ること
        let (back, _) = crate::read(Cursor::new(&out)).unwrap();
        assert_eq!(back.footnotes.len(), 1, "読み直せない");
        let t: String = back.footnotes[0].paragraphs.iter()
            .flat_map(|p| p.runs.iter().map(|r| r.text.as_str())).collect();
        assert_eq!(t, "足した脚注の文章。");
    }

    /// **既にある注を壊さない。** 原本の部品には仕切り線や、こちらが
    /// 模型に持っていない書式が入っている。丸ごと作り直すと失う
    #[test]
    fn adding_to_existing_footnotes_keeps_the_originals() {
        // 注を1つ持つ docx を組む
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o: zip::write::FileOptions<'_, ()> = Default::default();
        {
            let mut put = |n: &str, d: &[u8]| {
                zip.start_file(n, o).unwrap();
                zip.write_all(d).unwrap();
            };
            put("[Content_Types].xml", br#"<Types xmlns="ct"/>"#);
            put("_rels/.rels", br#"<Relationships xmlns="r"/>"#);
            put("word/_rels/document.xml.rels", br#"<Relationships xmlns="r"/>"#);
            put("word/document.xml",
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>本</w:t></w:r><w:r><w:footnoteReference w:id="2"/></w:r></w:p></w:body></w:document>"#.as_bytes());
            put("word/footnotes.xml",
                concat!(
                    r#"<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
                    r#"<w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>"#,
                    r#"<w:footnote w:id="2"><w:p><w:r><w:t>もとからの脚注</w:t></w:r></w:p></w:footnote>"#,
                    r#"</w:footnotes>"#).as_bytes());
        }
        let src = zip.finish().unwrap().into_inner();

        let (mut doc, _) = crate::read(Cursor::new(&src)).unwrap();
        assert_eq!(doc.footnotes.len(), 1, "もとの注が読めていない");
        let fr = doc.add_footnote(false, vec![kumihan::Paragraph {
            runs: vec![kumihan::Run { text: "あとから足した".into(), size_pt: Some(9.0),
                                      font: None, fmt: Default::default() }],
            line_spacing: 1.0,
            ..Default::default()
        }]);
        // **id は取られている番号を避ける。** 仕切りが -1、もとの注が 2
        assert_ne!(fr.id, "2", "既にある注と同じ id を選んだ");
        assert_ne!(fr.id, "-1", "仕切り線と同じ id を選んだ");

        let mut out = Vec::new();
        crate::write_with(&doc, Some(Cursor::new(&src)), Cursor::new(&mut out)).unwrap();
        let fx = parts(&out, "word/footnotes.xml").unwrap();
        assert!(fx.contains("もとからの脚注"), "もとの注が消えた: {fx}");
        assert!(fx.contains("あとから足した"), "足した注が入っていない: {fx}");
        assert!(fx.contains(r#"w:type="separator""#), "仕切り線が消えた: {fx}");

        let (back, _) = crate::read(Cursor::new(&out)).unwrap();
        assert_eq!(back.footnotes.len(), 2, "読み直すと数が合わない");
    }

    /// 注を足していない文書は**部品を1バイトも触らない**(今までどおり)
    #[test]
    fn adding_nothing_leaves_the_parts_alone() {
        let src = plain_docx();
        let (doc, _) = crate::read(Cursor::new(&src)).unwrap();
        let mut out = Vec::new();
        crate::write_with(&doc, Some(Cursor::new(&src)), Cursor::new(&mut out)).unwrap();
        assert!(parts(&out, "word/footnotes.xml").is_none(),
            "注を足していないのに部品ができた");
        let ct = parts(&out, "[Content_Types].xml").unwrap();
        assert!(!ct.contains("footnotes.xml"), "要らない宣言が入った: {ct}");
    }
}

/// **テンプレートを docx に通す**(2026-08-18)。本文には見た目を焼き付けず、
/// `styles.xml` の側にスタイル定義として入れる。
#[cfg(test)]
mod through_template {
    use super::*;
    use kumihan::{Block, Document, Paragraph, ParaStyle, Run};

    fn heading_doc() -> Document {
        let mut d = Document::plain("本文です");
        d.blocks.push(Block::Para(Paragraph {
            style: ParaStyle::Heading(1),
            runs: vec![Run { text: "章の題".into(), size_pt: None, font: None,
                             fmt: Default::default() }],
            ..Default::default()
        }));
        d.blocks.push(Block::Para(Paragraph {
            style_id: Some("註記".into()),
            runs: vec![Run { text: "気をつけて".into(), size_pt: None, font: None,
                             fmt: Default::default() }],
            ..Default::default()
        }));
        d
    }

    fn write_out(th: Option<&kumihan::theme::Theme>) -> Vec<u8> {
        let mut out = Vec::new();
        crate::write_with_theme(&heading_doc(), None::<Cursor<Vec<u8>>>, th,
                                Cursor::new(&mut out)).unwrap();
        out
    }

    fn parts(zipbytes: &[u8], name: &str) -> Option<String> {
        let mut z = zip::ZipArchive::new(Cursor::new(zipbytes.to_vec())).ok()?;
        let mut f = z.by_name(name).ok()?;
        let mut s = String::new();
        f.read_to_string(&mut s).ok()?;
        Some(s)
    }

    #[test]
    fn the_heading_look_goes_into_the_style_definition() {
        let th = kumihan::theme::default_theme();
        let s = parts(&write_out(Some(&th)), "word/styles.xml").unwrap();
        // 既定のテンプレートの「見出し1」は 16pt の太字
        assert!(s.contains(r#"w:styleId="Heading1""#), "見出し1 が Heading1 にならない: {s}");
        assert!(s.contains(r#"<w:sz w:val="32"/>"#), "16pt が入らない: {s}");
        assert!(s.contains(r#"<w:outlineLvl w:val="0"/>"#), "見出しの段が入らない: {s}");
        // Word の組み込みと同じ物だと見てもらうための名前
        assert!(s.contains(r#"<w:name w:val="heading 1"/>"#), "組み込みの名前でない: {s}");
    }

    #[test]
    fn the_user_name_becomes_the_style_name() {
        let th = kumihan::theme::default_theme();
        let s = parts(&write_out(Some(&th)), "word/styles.xml").unwrap();
        // 「註記」は本家 AsciiDoc の書き方。役割の固定名ではないので日本語のまま
        assert!(s.contains(r#"w:styleId="註記""#), "註記 が styleId にならない: {s}");
        assert!(s.contains(r#"w:fill="FFF6E0""#), "註記の背景色が入らない: {s}");
    }

    #[test]
    fn the_look_is_not_baked_into_the_body() {
        let th = kumihan::theme::default_theme();
        let d = parts(&write_out(Some(&th)), "word/document.xml").unwrap();
        // 見出しは pStyle で名乗るだけ。大きさも太字も本文の側には出さない
        assert!(d.contains(r#"w:val="Heading1""#), "pStyle が無い: {d}");
        assert!(!d.contains(r#"<w:sz w:val="32"/>"#), "本文に大きさが焼き付いた: {d}");
    }

    #[test]
    /// **テンプレートを渡さなくても、同梱の既定が入ります**(2026-08-26
    /// 発注者「初期設定ができていないのでは」)。前は最小の6スタイルだけで、
    /// 既定の書体も大きさも入っていませんでした。それでは開く機械ごとに
    /// 見た目が変わります。
    fn the_bundled_default_is_used_when_no_template_is_given() {
        // **言語を名乗ります。** 下で 10.5pt と ja-JP を直に見ているので、
        // 機械の設定まかせだと、言語を設定していない機械で落ちます
        // (2026-08-30 から、どれも無ければ en です)
        kumihan::font::set_default_language("ja");
        let s = parts(&write_out(None), "word/styles.xml").unwrap();
        assert!(s.contains(r#"w:styleId="Heading1""#), "見出しが無い: {s}");
        assert!(s.contains("註記"), "同梱の既定のスタイルが入らない: {s}");
        // **書体と大きさが styles.xml に入っていること** — ここが穴でした
        assert!(s.contains("<w:docDefaults>"), "文書の既定が無い: {s}");
        assert!(s.contains(r#"<w:sz w:val="21"/>"#), "10.5pt が入らない: {s}");
        assert!(s.contains(r#"w:eastAsia="ja-JP""#), "言語が入らない: {s}");
        // **書体はテーマ参照**(名指しではない)。役ごとに変えるためです
        assert!(s.contains("minorEastAsia"), "本文がテーマを見ていない: {s}");
    }

    /// **タイトルはゴシック、本文は明朝、コードは等幅**(2026-08-26 発注者)。
    ///
    /// 一度は書体を1つだけ名指ししました。それでは役ごとに変えられません。
    /// Word も OnlyOffice もテーマ参照で書いていて、そちらが正しい形でした。
    #[test]
    fn the_three_roles_have_their_own_fonts() {
        kumihan::font::set_default_language("ja");
        let out = write_out(None);
        let s = parts(&out, "word/styles.xml").unwrap();
        let t = parts(&out, "word/theme/theme1.xml").expect("テーマの部品が無い");

        // 役の参照
        assert!(s.contains("minorEastAsia"), "本文が minor を見ていない: {s}");
        for sid in ["Title", "Heading1"] {
            let i = s.find(&format!(r#"w:styleId="{sid}""#)).unwrap_or_else(|| panic!("{sid} が無い"));
            let j = s[i..].find("</w:style>").expect("閉じが無い");
            assert!(s[i..i + j].contains("majorEastAsia"), "{sid} が major を見ていない");
        }
        // テーマの中身 — タイトルはゴシック、本文は明朝
        let font_of = |role: &str| {
            let i = t.find(&format!("<a:{role}Font>")).expect("役が無い");
            let j = t[i..].find(&format!("</a:{role}Font>")).expect("閉じが無い");
            t[i..i + j].to_string()
        };
        assert!(font_of("major").contains("ゴシック"), "タイトルがゴシックでない");
        assert!(font_of("minor").contains("明朝"), "本文が明朝でない");
        // コードは等幅。スタイルが名指しします(役ではありません)
        let i = s.find(r#"w:styleId="等幅""#).expect("等幅が無い");
        let j = s[i..].find("</w:style>").expect("閉じが無い");
        assert!(s[i..i + j].contains("Mono"), "コードが等幅でない: {}", &s[i..i + j]);

        // **部品と関係が対で入っていること。** 片方だけだと Word が
        // テーマを解けず、役の分けが黙って効かなくなります
        let rels = parts(&out, "word/_rels/document.xml.rels").unwrap();
        assert!(rels.contains("theme/theme1.xml"), "テーマへの関係が無い: {rels}");
        let ct = parts(&out, "[Content_Types].xml").unwrap();
        assert!(ct.contains("/word/theme/theme1.xml"), "テーマが目録に無い: {ct}");
    }

    /// **用紙は必ず入ります。**
    ///
    /// 前は指定が無ければ `sectPr` ごと書かず、読む機械の既定に落ちて
    /// いました — 英語圏の Word なら Letter です。同じファイルが国に
    /// よって違う紙に組まれます。
    #[test]
    fn a_new_document_always_names_its_paper() {
        let d = parts(&write_out(None), "word/document.xml").unwrap();
        assert!(d.contains(r#"<w:pgSz w:w="11906" w:h="16838"/>"#), "A4 が入らない: {d}");
        // 余白 20mm = 1134 twip。教師(Word の空)の 25.4mm ではなく、
        // writer の決め済みの既定に揃えます
        assert!(d.contains(r#"w:left="1134""#), "余白 20mm が入らない: {d}");
    }

    /// 渡したテンプレートは、同梱の既定より強い
    #[test]
    fn a_given_template_wins_over_the_bundled_one() {
        let mut th = kumihan::theme::default_theme();
        th.font = Some("ＭＳ 明朝".into());
        let s = parts(&write_out(Some(&th)), "word/styles.xml").unwrap();
        assert!(s.contains("ＭＳ 明朝"), "渡した書体が入らない: {s}");
        assert!(!s.contains("BIZ UD"), "既定の書体で上書きされた: {s}");
    }
}

#[cfg(test)]
mod default_font_tests {
    use std::io::{Cursor, Write};

    /// docDefaults を持つ最小の docx。styles と theme を差し替えられる
    fn docx(styles: &str, theme: Option<&str>) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o: zip::write::FileOptions<'_, ()> = Default::default();
        let mut put = |n: &str, d: &[u8]| {
            zip.start_file(n, o).unwrap();
            zip.write_all(d).unwrap();
        };
        put("[Content_Types].xml", br#"<Types xmlns="ct"><Default Extension="xml" ContentType="application/xml"/></Types>"#);
        put("_rels/.rels", br#"<Relationships xmlns="r"/>"#);
        put("word/document.xml", r#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t>本文</w:t></w:r></w:p></w:body></w:document>"#.as_bytes());
        put("word/styles.xml", styles.as_bytes());
        if let Some(t) = theme {
            put("word/theme/theme1.xml", t.as_bytes());
        }
        zip.finish().unwrap().into_inner()
    }

    /// python-docx の既定の形: rFonts はテーマ名だけで、直後に言語の指定が並ぶ
    const PYDOCX_STYLES: &str = r#"<w:styles xmlns:w="x"><w:docDefaults><w:rPrDefault><w:rPr>
        <w:rFonts w:asciiTheme="minorHAnsi" w:eastAsiaTheme="minorEastAsia" w:hAnsiTheme="minorHAnsi" w:cstheme="minorBidi"/>
        <w:sz w:val="22"/><w:lang w:val="en-US" w:eastAsia="en-US" w:bidi="ar-SA"/>
        </w:rPr></w:rPrDefault></w:docDefaults></w:styles>"#;

    #[test]
    fn the_en_us_language_is_not_mistaken_for_a_font() {
        // w:lang の w:eastAsia="en-US" を書体として拾っていた。
        // theme1.xml が無ければ書体は「指定なし」— en-US ではない
        let src = docx(PYDOCX_STYLES, None);
        let (doc, _) = crate::read(Cursor::new(&src)).unwrap();
        assert_eq!(doc.font, None, "言語の札を書体として読んだ: {:?}", doc.font);
    }

    #[test]
    fn theme_font_names_resolve_through_theme1() {
        let theme = r#"<a:theme xmlns:a="x"><a:fontScheme name="Office">
            <a:majorFont><a:latin typeface="Calibri Light"/><a:ea typeface=""/></a:majorFont>
            <a:minorFont><a:latin typeface="Calibri"/><a:ea typeface="游明朝"/></a:minorFont>
            </a:fontScheme></a:theme>"#;
        let src = docx(PYDOCX_STYLES, Some(theme));
        let (doc, _) = crate::read(Cursor::new(&src)).unwrap();
        assert_eq!(doc.font.as_deref(), Some("游明朝"), "テーマの日本語書体を引けない");
    }

    #[test]
    fn the_japanese_font_also_resolves_via_the_script_table() {
        // Office の既定のテーマは <a:ea typeface=""/> が空で、
        // 日本語は script="Jpan" の表で持つ(python-docx の既定がこの形)
        let theme = r#"<a:theme xmlns:a="x"><a:fontScheme name="Office">
            <a:minorFont><a:latin typeface="Cambria"/><a:ea typeface=""/><a:cs typeface=""/>
            <a:font script="Jpan" typeface="ＭＳ 明朝"/>
            </a:minorFont></a:fontScheme></a:theme>"#;
        let src = docx(PYDOCX_STYLES, Some(theme));
        let (doc, _) = crate::read(Cursor::new(&src)).unwrap();
        assert_eq!(doc.font.as_deref(), Some("ＭＳ 明朝"), "Jpan の表を見ていない");
    }

    #[test]
    fn an_empty_theme_japanese_font_falls_back_to_latin() {
        let theme = r#"<a:theme xmlns:a="x"><a:fontScheme name="Office">
            <a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/></a:minorFont>
            </a:fontScheme></a:theme>"#;
        let src = docx(PYDOCX_STYLES, Some(theme));
        let (doc, _) = crate::read(Cursor::new(&src)).unwrap();
        assert_eq!(doc.font.as_deref(), Some("Calibri"), "欧文の書体にも落ちない");
    }

    #[test]
    fn a_directly_written_font_still_reads() {
        let styles = r#"<w:styles xmlns:w="x"><w:docDefaults><w:rPrDefault><w:rPr>
            <w:rFonts w:ascii="Century" w:eastAsia="ＭＳ 明朝"/>
            <w:lang w:val="en-US" w:eastAsia="en-US"/>
            </w:rPr></w:rPrDefault></w:docDefaults></w:styles>"#;
        let src = docx(styles, None);
        let (doc, _) = crate::read(Cursor::new(&src)).unwrap();
        assert_eq!(doc.font.as_deref(), Some("ＭＳ 明朝"), "eastAsia の直書きが読めない");
    }
}

/// **自作スタイルは見た目を持てる**(2026-08-27。読み物のスタイルの回が
/// これ待ちでした)。前は名前だけで、字の色も大きさも持たせられません
/// でした。
#[cfg(test)]
mod own_style_look {
    use std::io::Cursor;

    fn write_out(doc: &kumihan::Document) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        crate::write(doc, &mut buf).expect("書けない");
        buf.into_inner()
    }

    fn styles_of(bytes: &[u8]) -> String {
        let mut z = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).expect("zip でない");
        let mut f = z.by_name("word/styles.xml").expect("styles.xml が無い");
        let mut s = String::new();
        std::io::Read::read_to_string(&mut f, &mut s).expect("読めない");
        s
    }

    fn made(look: kumihan::StyleLook) -> String {
        let mut d = kumihan::Document::default();
        d.styles_new.push(kumihan::StyleInfo {
            id: "注記".into(),
            name: "注記".into(),
            kind: "paragraph".into(),
            look,
            ..Default::default()
        });
        styles_of(&write_out(&d))
    }

    fn body_of(s: &str) -> String {
        let i = s.find(r#"w:styleId="注記""#).expect("自作スタイルが無い");
        let j = s[i..].find("</w:style>").expect("閉じが無い");
        s[i..i + j].to_string()
    }

    #[test]
    fn a_style_carries_its_look() {
        let s = made(kumihan::StyleLook {
            bold: Some(true),
            color: Some("9C2B2B".into()),
            size_pt: Some(9.0),
            fill: Some("FFF0F0".into()),
            ..Default::default()
        });
        let b = body_of(&s);
        assert!(b.contains("<w:b/>"), "太字が入らない: {b}");
        assert!(b.contains(r#"<w:color w:val="9C2B2B"/>"#), "色が入らない: {b}");
        assert!(b.contains(r#"<w:sz w:val="18"/>"#), "9pt が入らない: {b}");
        assert!(b.contains(r#"w:fill="FFF0F0""#), "塗りが入らない: {b}");
    }

    /// **三択を守ります。** 言っていない物は書きません — 元になるスタイル
    /// から受け継ぐという意味です。`false` は「わざわざ切る」で別の意味です。
    #[test]
    fn saying_nothing_is_not_the_same_as_saying_off() {
        let quiet = body_of(&made(kumihan::StyleLook::default()));
        assert!(!quiet.contains("<w:rPr>"), "何も言っていないのに rPr が出た: {quiet}");

        let off = body_of(&made(kumihan::StyleLook { bold: Some(false), ..Default::default() }));
        assert!(off.contains(r#"<w:b w:val="0"/>"#), "切ったことが書かれない: {off}");
    }
}

/// **ページに貼り付く図形が docx を往復する。**
///
/// 2026-08-29 発注者「docx の図形をやって」。書いた物がそのまま読み戻せ
/// ないと、開いて直して保存するたびに形が痩せます。
#[test]
fn shapes_survive_a_round_trip() {
    let mut doc = kumihan::adoc::parse("= 題\n\n本文です。\n").expect("読めない");
    doc.shapes = vec![kumihan::DocShape {
        page: 0,
        x_mm: 25.0,
        y_mm: 80.0,
        w_mm: 40.0,
        h_mm: 25.0,
        look: book::SheetShape {
            kind: "roundRect".into(),
            fill: Some("DDE7F0".into()),
            line: Some("2E5A87".into()),
            line_w: 2.25,
            alpha: 0.5,
            rot: 20.0,
            shadow: true,
            text: Some("往復".into()),
            ..Default::default()
        },
    }];
    // 保存の前に、そのページの段落へ結び付けます(paper がやる仕事です)。
    // ここでは1ページ目なので、1つ目の段落へ手で入れます
    if let Some(kumihan::Block::Para(p)) = doc.blocks.first_mut() {
        p.anchors.push(crate::shape_anchor_run(&doc.shapes[0].clone(), 9000));
    }
    let motto = doc.shapes[0].clone();
    doc.shapes.clear();

    let mut buf = std::io::Cursor::new(Vec::new());
    crate::write(&doc, &mut buf).expect("書けない");
    buf.set_position(0);
    let (mata, _) = crate::read(buf).expect("読めない");

    assert_eq!(mata.shapes.len(), 1, "図形が読み戻せていない");
    let a = &mata.shapes[0];
    assert_eq!(a.page, motto.page);
    assert!((a.x_mm - motto.x_mm).abs() < 0.01, "x が違う: {}", a.x_mm);
    assert!((a.y_mm - motto.y_mm).abs() < 0.01, "y が違う: {}", a.y_mm);
    assert!((a.w_mm - motto.w_mm).abs() < 0.01, "幅が違う: {}", a.w_mm);
    assert_eq!(a.look.kind, motto.look.kind);
    assert_eq!(a.look.fill, motto.look.fill);
    assert_eq!(a.look.line, motto.look.line);
    assert!((a.look.line_w - motto.look.line_w).abs() < 0.01, "線の太さ");
    assert!((a.look.alpha - motto.look.alpha).abs() < 0.01, "濃さ");
    assert!((a.look.rot - motto.look.rot).abs() < 0.5, "回転");
    assert!(a.look.shadow, "影が消えた");
    // **タグごと拾わない。** `<w:txbxContent>` 自身が `<w:t` で始まるので、
    // 探し始める場所を間違えると `<w:p><w:r><w:t …>往復` が入ります
    assert_eq!(a.look.text.as_deref(), Some("往復"), "図形の中の文字");
}
