//! The width and colour of a table's own edges (`w:tblBorders`, ECMA-376
//! 17.4.39) are read and written back.

use std::io::{Cursor, Read, Write};

#[test]
fn a_tables_edge_keeps_its_width_and_colour() {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o: zip::write::FileOptions<'_, ()> = Default::default();
    let mut put = |n: &str, d: &str| {
        zip.start_file(n, o).unwrap();
        zip.write_all(d.as_bytes()).unwrap();
    };
    put("[Content_Types].xml", r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#);
    put("_rels/.rels", r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#);
    put("word/document.xml", r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tblPr><w:tblBorders><w:bottom w:val="single" w:sz="18" w:space="0" w:color="9A92BF"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="5000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#);
    let src = zip.finish().unwrap().into_inner();
    let (doc, _) = ooxml::read(Cursor::new(&src)).expect("not read");
    let t = doc.tables().next().expect("no table");
    assert!(t.borders.bottom && !t.borders.top);
    assert_eq!(t.borders.pt[2], 2.25);
    assert_eq!(t.borders.rgb[2], Some([0x9A, 0x92, 0xBF]));

    let mut out = Vec::new();
    ooxml::write(&doc, Cursor::new(&mut out)).expect("not written");
    let mut z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
    let mut xml = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    assert!(xml.contains(r#"<w:bottom w:val="single" w:sz="18" w:color="9A92BF"/>"#), "{xml}");
}
