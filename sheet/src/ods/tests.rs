use std::io::{Cursor, Write};

use book::{Pos, Value};

/// A minimal ods around the given `<office:spreadsheet>` body
fn ods(body: &str, auto_styles: &str) -> Vec<u8> {
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:calcext="urn:org:documentfoundation:names:experimental:calc:xmlns:calcext:1.0" office:version="1.3"><office:automatic-styles>{auto_styles}</office:automatic-styles><office:body><office:spreadsheet>{body}</office:spreadsheet></office:body></office:document-content>"#
    );
    let mut buf = Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let st = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        z.start_file("mimetype", st).unwrap();
        z.write_all(b"application/vnd.oasis.opendocument.spreadsheet").unwrap();
        z.start_file("content.xml", st).unwrap();
        z.write_all(content.as_bytes()).unwrap();
        z.finish().unwrap();
    }
    buf.into_inner()
}

fn read(bytes: Vec<u8>) -> (book::Book, super::Report) {
    super::read(Cursor::new(bytes)).expect("reads")
}

#[test]
fn values_of_every_type_are_read() {
    let body = r#"<table:table table:name="S"><table:table-row>
<table:table-cell office:value-type="float" office:value="1.5"><text:p>1.5</text:p></table:table-cell>
<table:table-cell office:value-type="string"><text:p>a</text:p><text:p>b<text:s text:c="2"/>c</text:p></table:table-cell>
<table:table-cell office:value-type="boolean" office:boolean-value="true"><text:p>TRUE</text:p></table:table-cell>
<table:table-cell office:value-type="date" office:date-value="2026-08-04"><text:p>2026/8/4</text:p></table:table-cell>
<table:table-cell office:value-type="time" office:time-value="PT12H00M00S"><text:p>12:00</text:p></table:table-cell>
<table:table-cell office:value-type="percentage" office:value="0.25"><text:p>25%</text:p></table:table-cell>
</table:table-row></table:table>"#;
    let (b, rep) = read(ods(body, ""));
    let s = &b.sheets[0];
    assert_eq!(s.name, "S");
    assert_eq!(s.value(Pos::new(0, 0)), Value::Number(1.5));
    assert_eq!(s.value(Pos::new(0, 1)), Value::Text("a\nb  c".into()));
    assert_eq!(s.value(Pos::new(0, 2)), Value::Bool(true));
    // 2026-08-04 is serial 46238 counted from 1899-12-30
    assert_eq!(s.value(Pos::new(0, 3)), Value::Number(46238.0));
    assert_eq!(s.value(Pos::new(0, 4)), Value::Number(0.5));
    assert_eq!(s.value(Pos::new(0, 5)), Value::Number(0.25));
    assert!(rep.is_lossless());
}

#[test]
fn repeated_cells_and_rows_are_spread_and_empty_ones_only_move_on() {
    let body = r#"<table:table table:name="S">
<table:table-row table:number-rows-repeated="2"><table:table-cell table:number-columns-repeated="2" office:value-type="float" office:value="7"/><table:table-cell table:number-columns-repeated="16382"/></table:table-row>
<table:table-row table:number-rows-repeated="1048570"><table:table-cell table:number-columns-repeated="16384"/></table:table-row>
</table:table>"#;
    let (b, _) = read(ods(body, ""));
    let s = &b.sheets[0];
    for p in [Pos::new(0, 0), Pos::new(0, 1), Pos::new(1, 0), Pos::new(1, 1)] {
        assert_eq!(s.value(p), Value::Number(7.0), "{p:?}");
    }
    assert_eq!(s.cells.len(), 4);
}

#[test]
fn formulas_keep_their_cached_values_and_merges_are_read() {
    let body = r#"<table:table table:name="S"><table:table-row>
<table:table-cell table:number-columns-spanned="2" table:number-rows-spanned="1" office:value-type="string"><text:p>Title</text:p></table:table-cell><table:covered-table-cell/>
<table:table-cell table:formula="of:=SUM([.A2:.B2])" office:value-type="float" office:value="3"><text:p>3</text:p></table:table-cell>
</table:table-row><table:table-row>
<table:table-cell office:value-type="float" office:value="1"/><table:table-cell office:value-type="float" office:value="2"/>
<table:table-cell table:formula="of:=1/0" office:value-type="string" calcext:value-type="error"><text:p>#DIV/0!</text:p></table:table-cell>
</table:table-row></table:table>"#;
    let (b, _) = read(ods(body, ""));
    let s = &b.sheets[0];
    assert_eq!(s.merges, vec![(Pos::new(0, 0), Pos::new(0, 1))]);
    let f = s.get(Pos::new(0, 2)).unwrap();
    assert_eq!(f.formula.as_deref(), Some("SUM(A2:B2)"));
    assert_eq!(f.value, Value::Number(3.0));
    assert_eq!(s.value(Pos::new(1, 2)), Value::Error("#DIV/0!".into()));
}

#[test]
fn widths_heights_and_hidden_rows_come_from_the_styles() {
    let styles = r#"<style:style style:name="co1" style:family="table-column"><style:table-column-properties style:column-width="2.5cm"/></style:style>
<style:style style:name="co2" style:family="table-column"><style:table-column-properties style:column-width="0.889in"/></style:style>
<style:style style:name="ro1" style:family="table-row"><style:table-row-properties style:row-height="0.5in" style:use-optimal-row-height="false"/></style:style>
<style:style style:name="ta1" style:family="table"><style:table-properties table:display="false"/></style:style>"#;
    let body = r#"<table:table table:name="S" table:style-name="ta1">
<table:table-column table:style-name="co1"/><table:table-column table:style-name="co2" table:number-columns-repeated="16383"/>
<table:table-row table:style-name="ro1"><table:table-cell office:value-type="float" office:value="1"/></table:table-row>
<table:table-row table:visibility="collapse"><table:table-cell office:value-type="float" office:value="2"/></table:table-row>
</table:table>"#;
    let (b, _) = read(ods(body, styles));
    let s = &b.sheets[0];
    assert!(s.hidden);
    assert_eq!(s.col_mm.get(&0).copied(), Some(25.0));
    assert!((s.default_col_mm.unwrap() - 22.58).abs() < 0.01);
    assert_eq!(s.row_height.get(&0).copied(), Some(36.0));
    assert!(s.row_hidden.contains(&1));
}

#[test]
fn a_comment_is_read_and_kept_out_of_the_cell_text() {
    let body = r#"<table:table table:name="S"><table:table-row>
<table:table-cell office:value-type="string"><office:annotation><text:p>note</text:p></office:annotation><text:p>v</text:p></table:table-cell>
</table:table-row></table:table>"#;
    let (b, rep) = read(ods(body, ""));
    assert_eq!(b.sheets[0].value(Pos::new(0, 0)), Value::Text("v".into()));
    assert!(rep.is_lossless(), "{:?}", rep.unsupported);
    assert_eq!(b.sheets[0].comments[&Pos::new(0, 0)].text(), "note");
}

/// A quotation made by LibreOffice 24.2 from sample/見積書.xlsx
/// (`tools/lo_pdf.py --to ods`)
#[test]
fn a_quotation_saved_by_libreoffice() {
    let bytes = include_bytes!("testdata/mitsumori.ods").to_vec();
    let (b, _) = read(bytes);
    let s = &b.sheets[0];
    assert_eq!(s.name, "見積書");
    assert_eq!(s.value(Pos::new(0, 0)), Value::Text("御 見 積 書".into()));
    assert!(s.merges.contains(&(Pos::new(0, 0), Pos::new(0, 5))));
    let total = s.get(Pos::new(5, 2)).unwrap();
    assert_eq!(total.formula.as_deref(), Some("F18"));
    assert_eq!(total.value, Value::Number(640200.0));
    let line = s.get(Pos::new(10, 5)).unwrap();
    assert_eq!(line.formula.as_deref(), Some("C11*E11"));
    assert_eq!(line.value, Value::Number(450000.0));
}

/// A comment, a picture and a shape are written into their cells and read
/// back the same
#[test]
fn comments_pictures_and_shapes_round_trip() {
    let mut b = book::Book::new();
    let sh = &mut b.sheets[0];
    sh.set(Pos::new(0, 0), book::Cell { value: Value::Text("見出し".into()), ..Default::default() });
    sh.comments.insert(
        Pos::new(0, 0),
        book::CommentThread { done: false, entries: vec![book::CommentEntry { who: "山田".into(), when: "2026-10-05T09:00:00".into(), text: "確認".into() }] },
    );
    // A 1x1 PNG
    let png: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1F,
        0x15, 0xC4, 0x89, 0, 0, 0, 13, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0, 0, 0x03, 0x01, 0x01, 0x00, 0xC9, 0xFE,
        0x92, 0xEF, 0, 0, 0, 0, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    sh.images.push(book::SheetImage { at: Pos::new(2, 1), dx_px: 4.0, dy_px: 2.0, width_px: 96.0, height_px: 48.0, data: png.clone(), z: 1 });
    sh.shapes.push(book::SheetShape {
        at: Pos::new(4, 0),
        kind: "roundRect".into(),
        width_px: 200.0,
        height_px: 40.0,
        fill: Some("FFF2CC".into()),
        text: Some("社外秘".into()),
        z: 2,
        ..Default::default()
    });
    let (bytes, rep) = super::write(&b);
    assert!(rep.is_lossless(), "{:?}", rep.left_out);
    let (r, read_rep) = read(bytes);
    assert!(read_rep.is_lossless(), "{:?}", read_rep.unsupported);
    let s = &r.sheets[0];
    assert_eq!(s.value(Pos::new(0, 0)), Value::Text("見出し".into()));
    let t = &s.comments[&Pos::new(0, 0)];
    assert_eq!((t.entries[0].who.as_str(), t.entries[0].text.as_str()), ("山田", "確認"));
    let i = &s.images[0];
    assert_eq!((i.at, i.data.clone()), (Pos::new(2, 1), png));
    assert!((i.width_px - 96.0).abs() < 0.1 && (i.dx_px - 4.0).abs() < 0.1);
    let h = &s.shapes[0];
    assert_eq!((h.at, h.kind.as_str(), h.fill.as_deref(), h.text.as_deref()), (Pos::new(4, 0), "roundRect", Some("FFF2CC"), Some("社外秘")));
    assert!((h.width_px - 200.0).abs() < 0.1);
}

/// A chart (an embedded object) is kept as written: its frame stays in its
/// cell and its folder goes back into the package, under a fresh number
#[test]
fn a_chart_is_kept_through_a_round_trip() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" office:version="1.3"><office:body><office:spreadsheet><table:table table:name="S"><table:table-row><table:table-cell office:value-type="float" office:value="1"><text:p>1</text:p><draw:frame draw:z-index="0" svg:width="6cm" svg:height="4cm" svg:x="0cm" svg:y="0cm"><draw:object xlink:href="./Object 7"/><draw:image xlink:href="./ObjectReplacements/Object 7"/></draw:frame></table:table-cell></table:table-row></table:table></office:spreadsheet></office:body></office:document-content>"#;
    let manifest = r#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="Object 7/" manifest:media-type="application/vnd.oasis.opendocument.chart"/><manifest:file-entry manifest:full-path="Object 7/content.xml" manifest:media-type="text/xml"/></manifest:manifest>"#;
    let mut buf = Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let st = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, data) in [
            ("mimetype", b"application/vnd.oasis.opendocument.spreadsheet".as_slice()),
            ("content.xml", content.as_bytes()),
            ("META-INF/manifest.xml", manifest.as_bytes()),
            ("Object 7/content.xml", b"<chart/>".as_slice()),
            ("ObjectReplacements/Object 7", b"stand-in".as_slice()),
        ] {
            z.start_file(name, st).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }
    let (b, rep) = read(buf.into_inner());
    assert!(rep.is_lossless(), "{:?}", rep.unsupported);
    let k = &b.sheets[0].kept_objects[0];
    assert_eq!(k.at, Some(Pos::new(0, 0)));
    assert_eq!(b.sheets[0].value(Pos::new(0, 0)), Value::Number(1.0));

    let (bytes, _) = super::write(&b);
    let mut z = zip::ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
    let mut chart = String::new();
    std::io::Read::read_to_string(&mut z.by_name("Object 1/content.xml").unwrap(), &mut chart).unwrap();
    assert_eq!(chart, "<chart/>");
    assert!(z.by_name("ObjectReplacements/Object 1").is_ok());
    let (again, _) = read(bytes);
    let k2 = &again.sheets[0].kept_objects[0];
    assert!(k2.xml.contains("\"./Object 1\""), "{}", k2.xml);
    assert_eq!(k2.files.len(), k.files.len());
}

/// Data validation rules are written once before the sheets, their cells
/// name them, and they read back the same, messages included
#[test]
fn data_validation_round_trips() {
    let mut b = book::Book::new();
    let sh = &mut b.sheets[0];
    sh.set(Pos::new(0, 0), book::Cell { value: Value::Text("区分".into()), ..Default::default() });
    let rule = |range, kind: &str, op: &str, f1: &str, f2: &str| book::Validation {
        range,
        formula: f1.into(),
        kind: kind.into(),
        op: op.into(),
        formula2: f2.into(),
        input_msg: None,
        error_msg: None,
        allow_blank: true,
        hide_arrow: false,
    };
    let mut list = rule((Pos::new(1, 0), Pos::new(30, 0)), "list", "", "\"甲,乙,丙\"", "");
    list.input_msg = Some(("区分".into(), "一覧から選びます".into()));
    list.error_msg = Some(("warning".into(), "確認".into(), "一覧に無い値です\n入れ直してください".into()));
    let mut whole = rule((Pos::new(1, 2), Pos::new(5000, 3)), "whole", "between", "1", "100");
    whole.allow_blank = false;
    let length = rule((Pos::new(2, 5), Pos::new(2, 5)), "textLength", "lessThanOrEqual", "8", "");
    // Any value, with only a message to show
    let mut any = rule((Pos::new(4, 6), Pos::new(9, 6)), "", "", "", "");
    any.input_msg = Some(("メモ".into(), "自由に書きます".into()));
    sh.validations = vec![list, whole, length, any];
    let (bytes, rep) = super::write(&b);
    assert!(rep.is_lossless(), "{:?}", rep.left_out);
    let (r, read_rep) = read(bytes);
    assert!(read_rep.is_lossless(), "{:?}", read_rep.unsupported);
    let mut got = r.sheets[0].validations.clone();
    let mut want = b.sheets[0].validations.clone();
    got.sort_by_key(|v| (v.range.0.col, v.range.0.row));
    want.sort_by_key(|v| (v.range.0.col, v.range.0.row));
    assert_eq!(got, want);
}

/// Page breaks, sheet protection, unlocked cells, links, frozen panes and
/// tables are written and read back the same
#[test]
fn breaks_protection_links_and_frozen_panes_round_trip() {
    let mut b = book::Book::new();
    b.sheets.push(book::Sheet::new("Data Sheet"));
    let sh = &mut b.sheets[0];
    for (r, t) in [(0, "見出し"), (1, "社外"), (2, "社内"), (3, "範囲")] {
        sh.set(Pos::new(r, 0), book::Cell { value: Value::Text(t.into()), ..Default::default() });
    }
    sh.row_breaks = vec![20, 40];
    sh.col_breaks = vec![5];
    sh.protected = true;
    sh.protect_allow.insert_rows = true;
    sh.protect_allow.select_locked = false;
    let mut open = book::Cell { value: Value::Number(1.0), ..Default::default() };
    open.fmt.unlocked = true;
    sh.set(Pos::new(1, 2), open);
    let mut hidden = book::Cell { value: Value::Number(2.0), formula: Some("C2*2".into()), ..Default::default() };
    hidden.fmt.formula_hidden = true;
    sh.set(Pos::new(2, 2), hidden);
    sh.links.insert(Pos::new(1, 0), "https://example.com/a?b=1&c=2".into());
    sh.links.insert(Pos::new(2, 0), "#'Data Sheet'!B3".into());
    sh.links.insert(Pos::new(3, 0), "#'Data Sheet'!A1:C4".into());
    sh.freeze = Some(book::FreezePane { frozen_rows: 1, frozen_columns: 2 });
    sh.tables.push(book::TableDef { name: "名簿".into(), a: Pos::new(0, 0), b: Pos::new(3, 2), ..Default::default() });
    b.sheets[1].freeze = Some(book::FreezePane { frozen_rows: 3, frozen_columns: 0 });
    let (bytes, rep) = super::write(&b);
    assert!(rep.is_lossless(), "{:?}", rep.left_out);
    let (r, read_rep) = read(bytes);
    assert!(read_rep.is_lossless(), "{:?}", read_rep.unsupported);
    let (x, y) = (&b.sheets[0], &r.sheets[0]);
    assert_eq!((&y.row_breaks, &y.col_breaks), (&x.row_breaks, &x.col_breaks));
    assert_eq!((y.protected, &y.protect_allow), (true, &x.protect_allow));
    let c = |s: &book::Sheet, r, c| s.get(Pos::new(r, c)).map(|c| (c.fmt.unlocked, c.fmt.formula_hidden));
    assert_eq!((c(y, 1, 2), c(y, 2, 2)), (Some((true, false)), Some((false, true))));
    assert_eq!(y.links, x.links);
    assert_eq!(y.freeze, x.freeze);
    assert_eq!(r.sheets[1].freeze, b.sheets[1].freeze);
    assert_eq!(y.tables, x.tables);
}

/// Header and footer text keeps its font, size and bold, its runs of
/// spaces, and the headers of even and first pages
#[test]
fn header_looks_and_even_and_first_pages_round_trip() {
    let mut b = book::Book::new();
    let sh = &mut b.sheets[0];
    sh.set(Pos::new(0, 0), book::Cell { value: Value::Number(1.0), ..Default::default() });
    sh.header = Some("&C&\"ＭＳ Ｐ明朝,Regular\"&16 第 ２ 表   収入&B支出&R&P / &N".into());
    sh.footer = Some("&L&\"Arial,Bold Italic\"社外秘&C&A".into());
    sh.hf_diff_odd_even = true;
    sh.header_even = Some("&L偶数 &P".into());
    sh.hf_diff_first = true;
    sh.footer_first = Some("&C表紙".into());
    let (bytes, rep) = super::write(&b);
    assert!(rep.is_lossless(), "{:?}", rep.left_out);
    let (r, _) = read(bytes);
    let (x, y) = (&b.sheets[0], &r.sheets[0]);
    assert_eq!(y.header, x.header);
    assert_eq!(y.footer, x.footer);
    assert_eq!((y.hf_diff_odd_even, &y.header_even, &y.footer_even), (true, &x.header_even, &None));
    assert_eq!((y.hf_diff_first, &y.header_first, &y.footer_first), (true, &None, &x.footer_first));
}

/// A turned shape is written with a transform and read back with the same
/// angle and box
#[test]
fn a_turned_shape_round_trips() {
    let mut b = book::Book::new();
    let sh = &mut b.sheets[0];
    sh.set(Pos::new(0, 0), book::Cell { value: Value::Number(1.0), ..Default::default() });
    sh.shapes.push(book::SheetShape {
        at: Pos::new(2, 1),
        kind: "rect".into(),
        dx_px: 10.0,
        dy_px: 5.0,
        width_px: 200.0,
        height_px: 60.0,
        rot: 30.0,
        fill: Some("DDEBF7".into()),
        text: Some("回して  置く".into()),
        z: 1,
        ..Default::default()
    });
    let (bytes, rep) = super::write(&b);
    assert!(rep.is_lossless(), "{:?}", rep.left_out);
    let (r, _) = read(bytes);
    let (x, y) = (&b.sheets[0].shapes[0], &r.sheets[0].shapes[0]);
    assert!((y.rot - 30.0).abs() < 0.01, "{}", y.rot);
    assert_eq!(y.at, x.at);
    for (a, b) in [(x.dx_px, y.dx_px), (x.dy_px, y.dy_px), (x.width_px, y.width_px), (x.height_px, y.height_px)] {
        assert!((a - b).abs() < 0.1, "{a} vs {b}");
    }
    assert_eq!(y.text, x.text);
}
