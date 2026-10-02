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
fn comments_and_pictures_are_reported_not_dropped_in_silence() {
    let body = r#"<table:table table:name="S"><table:table-row>
<table:table-cell office:value-type="string"><office:annotation><text:p>note</text:p></office:annotation><text:p>v</text:p></table:table-cell>
</table:table-row></table:table>"#;
    let (b, rep) = read(ods(body, ""));
    assert_eq!(b.sheets[0].value(Pos::new(0, 0)), Value::Text("v".into()));
    assert_eq!(rep.unsupported, vec![("office:annotation".to_string(), 1)]);
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
