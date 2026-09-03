//! **表の決まりを docx から読めているか。**
//!
//! 日本の事務は表で出来ているので、表に掛かる `w:tblPr` / `w:trPr` /
//! `w:tcPr` の決まりは1つずつ見張ります(2026-09-03 発注者
//! 「日本は、表をよく使う国なので、表はこれ以外でも規約があれば出して」)。
//!
//! 幅・インデント・見出しの行・セルの余白・斜線・均等割り付けの6つです。

fn hyou(naka: &str) -> kumihan::Table {
    let xml = format!(
        r#"<?xml version="1.0"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>{naka}</w:body></w:document>"#
    );
    let (doc, _) = ooxml::parse_document_xml(&xml);
    doc.blocks
        .into_iter()
        .find_map(|b| if let kumihan::Block::Table(t) = b { Some(t) } else { None })
        .expect("表が読めていない")
}

/// セル1つの `w:tc`。`pr` は `w:tcPr` の中身
fn tc(pr: &str, ji: &str) -> String {
    format!("<w:tc><w:tcPr>{pr}</w:tcPr><w:p><w:r><w:t>{ji}</w:t></w:r></w:p></w:tc>")
}

#[test]
fn a_table_width_in_percent_is_read() {
    // `w:tblW w:type="pct"` は 1/50 % — 5000 が 100% です
    let t = hyou(&format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/></w:tblPr>
<w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr>{}</w:tr></w:tbl>"#,
        tc("", "あ")
    ));
    assert_eq!(t.width_pct, Some(100.0), "幅の割合が読めていない");

    // `dxa`(twip)は `w:gridCol` と同じ値なので、割合は持ちません
    let t = hyou(&format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="9000" w:type="dxa"/></w:tblPr>
<w:tblGrid><w:gridCol w:w="9000"/></w:tblGrid>
<w:tr>{}</w:tr></w:tbl>"#,
        tc("", "あ")
    ));
    assert_eq!(t.width_pct, None, "twip の幅まで割合として持っている");
}

#[test]
fn the_table_indent_is_measured_the_old_way_when_the_file_says_nothing() {
    // **`w:tblInd` はセルの左余白と打ち消し合います**(compatibilityMode が
    // 無い docx は Word 2013 より前の測り方)。内閣府の調査票がこの形で、
    // `w:tblInd` の 108twip とセルの左余白の 108twip で 0 になります。
    //
    // ここは `parse_document_xml`(設定を読まない入り口)なので、原文の
    // ままの値が入ります。打ち消しは zip から読む [`ooxml::read`] の仕事です
    let t = hyou(&format!(
        r#"<w:tbl><w:tblPr><w:tblInd w:w="108" w:type="dxa"/></w:tblPr>
<w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr>{}</w:tr></w:tbl>"#,
        tc("", "あ")
    ));
    assert!((t.indent_mm - 108.0 * 25.4 / 1440.0).abs() < 0.01, "インデント {}", t.indent_mm);
}

#[test]
fn the_first_row_can_say_it_is_the_heading() {
    let t = hyou(&format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr><w:trPr><w:tblHeader/></w:trPr>{}</w:tr>
<w:tr>{}</w:tr></w:tbl>"#,
        tc("", "見出し"),
        tc("", "中身")
    ));
    assert!(t.header_row, "見出しの行(w:tblHeader)が読めていない");

    // 2行目だけに付いていても、模型は1つしか持てないので落とします
    let t = hyou(&format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr>{}</w:tr>
<w:tr><w:trPr><w:tblHeader/></w:trPr>{}</w:tr></w:tbl>"#,
        tc("", "中身"),
        tc("", "見出し")
    ));
    assert!(!t.header_row, "最初の行でない見出しまで拾っている");
}

#[test]
fn a_cell_can_carry_its_own_margins() {
    let t = hyou(&format!(
        r#"<w:tbl><w:tblPr><w:tblCellMar>
<w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/>
<w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/>
</w:tblCellMar></w:tblPr>
<w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr>{}{}</w:tr></w:tbl>"#,
        tc("", "普通"),
        tc(
            r#"<w:tcMar><w:top w:w="300" w:type="dxa"/><w:left w:w="600" w:type="dxa"/></w:tcMar>"#,
            "広い"
        )
    ));
    let mm = |tw: f32| tw * 25.4 / 1440.0;
    assert_eq!(
        t.rows[0][0].mar_mm, None,
        "何も言っていないセルが余白を持っている(表の指定に従うべき)"
    );
    let m = t.rows[0][1].mar_mm.expect("セルの余白が読めていない");
    assert!((m[0] - mm(300.0)).abs() < 0.01, "上の余白 {}", m[0]);
    assert!((m[3] - mm(600.0)).abs() < 0.01, "左の余白 {}", m[3]);
    // 表の指定も読みます(前はスタイルの分しか読んでいませんでした)
    let tm = t.cell_mar_mm.expect("表のセルの余白が読めていない");
    assert!((tm[3] - mm(108.0)).abs() < 0.01, "表の左の余白 {}", tm[3]);
}

#[test]
fn a_cell_can_be_crossed_out_with_a_diagonal() {
    let t = hyou(&format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr>{}{}</w:tr></w:tbl>"#,
        tc(
            r#"<w:tcBorders><w:tl2br w:val="single" w:sz="4"/></w:tcBorders>"#,
            "斜め"
        ),
        tc(
            r#"<w:tcBorders><w:tl2br w:val="single" w:sz="4"/><w:tr2bl w:val="single" w:sz="4"/></w:tcBorders>"#,
            "×"
        )
    ));
    assert!(t.rows[0][0].borders.diag_down, "左上から右下の斜線が読めていない");
    assert!(!t.rows[0][0].borders.diag_up, "引いていない斜線を引いている");
    assert!(t.rows[0][1].borders.diag_down && t.rows[0][1].borders.diag_up, "×が読めていない");
}

#[test]
fn a_cell_can_spread_its_text_across_the_width() {
    let t = hyou(&format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr>{}{}</w:tr></w:tbl>"#,
        tc("<w:tcFitText/>", "氏名"),
        tc("", "山田")
    ));
    assert!(t.rows[0][0].fit_text, "均等割り付け(w:tcFitText)が読めていない");
    assert!(!t.rows[0][1].fit_text, "言っていないセルまで均等割り付けにしている");
}

/// **斜線は紙面に線として出ます。**
///
/// 表計算側は前から引いていました(2026-08-31)。文書側も同じ線を引きます
#[test]
fn the_diagonal_reaches_the_page() {
    let t = hyou(&format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr>{}</w:tr></w:tbl>"#,
        tc(
            r#"<w:tcBorders><w:tl2br w:val="single" w:sz="4"/></w:tcBorders>"#,
            "斜め"
        )
    ));
    let mut doc = kumihan::Document::default();
    doc.blocks.push(kumihan::Block::Table(t));
    let fam = kumihan::font::for_document(None).expect("日本語の書体が要る").0;
    let data = std::fs::read(&fam.path).expect("書体が読めない");
    let m = kumihan::Metrics::new(&data).unwrap();
    let sheet = kumihan::layout(
        &doc,
        &m,
        &kumihan::Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 },
    );
    let naname = sheet
        .rules
        .iter()
        .filter(|r| (r[0] - r[2]).abs() > 1.0 && (r[1] - r[3]).abs() > 1.0)
        .count();
    assert_eq!(naname, 1, "斜めの線が {naname} 本(1本のはず)");
}

/// 均等割り付けは紙面で字の間が開きます
#[test]
fn the_spread_text_reaches_the_page() {
    let t = hyou(&format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid>
<w:tr>{}</w:tr></w:tbl>"#,
        tc("<w:tcFitText/>", "氏名")
    ));
    assert!(t.rows[0][0].fit_text);
    let mut doc = kumihan::Document::default();
    doc.blocks.push(kumihan::Block::Table(t));
    let fam = kumihan::font::for_document(None).expect("日本語の書体が要る").0;
    let data = std::fs::read(&fam.path).expect("書体が読めない");
    let m = kumihan::Metrics::new(&data).unwrap();
    let sheet = kumihan::layout(
        &doc,
        &m,
        &kumihan::Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 },
    );
    let line = sheet.lines.first().expect("字が無い");
    let hi: Vec<f32> = line.cells.iter().map(|c| c.x_mm).collect();
    assert_eq!(hi.len(), 2, "字の数");
    let aida = hi[1] - hi[0];
    assert!(aida > 20.0, "字の間が {aida}mm しか開いていない(幅いっぱいに配るはず)");
}

/// **保存で落とさない。** 読んだ決まりは書き出しても残ります
#[test]
fn the_table_rules_survive_a_save() {
    let mut t = kumihan::Table {
        width_pct: Some(100.0),
        header_row: true,
        cell_mar_mm: Some([0.0, 1.9, 0.0, 1.9]),
        col_mm: vec![40.0, 40.0],
        ..Default::default()
    };
    let hitotsu = |ji: &str| kumihan::Cellbox {
        paragraphs: vec![kumihan::Paragraph {
            line_spacing: 1.0,
            runs: vec![kumihan::Run {
                text: ji.to_string(),
                size_pt: None,
                font: None,
                fmt: Default::default(),
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut midashi = hitotsu("見出し");
    midashi.borders.bottom = Some(true);
    midashi.borders.top = Some(false);
    let mut naname = hitotsu("斜め");
    naname.borders.diag_down = true;
    naname.borders.diag_up = true;
    let mut hiroi = hitotsu("広い");
    hiroi.mar_mm = Some([5.0, 2.0, 1.0, 10.0]);
    let mut kubaru = hitotsu("氏名");
    kubaru.fit_text = true;
    t.rows.push(vec![midashi, naname]);
    t.rows.push(vec![hiroi, kubaru]);

    let mut doc = kumihan::Document::default();
    doc.blocks.push(kumihan::Block::Table(t));
    let mut buf = std::io::Cursor::new(Vec::new());
    ooxml::write(&doc, &mut buf).expect("書けない");
    buf.set_position(0);
    let (modori, _) = ooxml::read(buf).expect("読めない");
    let kumihan::Block::Table(t2) = &modori.blocks[0] else { panic!("表が返らない") };

    assert_eq!(t2.width_pct, Some(100.0), "幅の割合が消えた");
    assert!(t2.header_row, "見出しの行が消えた");
    assert!(t2.rows[0][1].borders.diag_down && t2.rows[0][1].borders.diag_up, "斜線が消えた");
    // セルが自分で言った辺も返します
    assert_eq!(t2.rows[0][0].borders.bottom, Some(true), "セルの下罫線が消えた");
    assert_eq!(t2.rows[0][0].borders.top, Some(false), "「引かない」が消えた");
    assert_eq!(t2.rows[0][0].borders.left, None, "言っていない辺が付いた");
    assert!(t2.rows[1][1].fit_text, "均等割り付けが消えた");
    let m = t2.rows[1][0].mar_mm.expect("セルの余白が消えた");
    assert!((m[3] - 10.0).abs() < 0.05, "左の余白 {}", m[3]);
    let tm = t2.cell_mar_mm.expect("表のセルの余白が消えた");
    assert!((tm[1] - 1.9).abs() < 0.05, "表の右の余白 {}", tm[1]);
}
