//! Font kerning (`w:kern`, ECMA-376 17.3.2.19) is read from the run, the
//! styles and the document default, and a run's own value is written back.

use std::io::{Cursor, Read, Write};

fn docx(body: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o: zip::write::FileOptions<'_, ()> = Default::default();
    let mut put = |n: &str, d: &str| {
        zip.start_file(n, o).unwrap();
        zip.write_all(d.as_bytes()).unwrap();
    };
    put("[Content_Types].xml", r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#);
    put("_rels/.rels", r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#);
    put("word/_rels/document.xml.rels", r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#);
    put("word/document.xml", &format!(r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#));
    put("word/styles.xml", r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:docDefaults><w:rPrDefault><w:rPr><w:kern w:val="2"/><w:sz w:val="24"/></w:rPr></w:rPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="a"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="big"><w:name w:val="Big"/><w:rPr><w:kern w:val="28"/></w:rPr></w:style>
</w:styles>"#);
    zip.finish().unwrap().into_inner()
}

#[test]
fn kerning_comes_from_the_run_the_style_and_the_default() {
    let src = docx(concat!(
        r#"<w:p><w:r><w:t>既定</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="big"/></w:pPr><w:r><w:t>見出し</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:rPr><w:kern w:val="20"/></w:rPr><w:t>自分</w:t></w:r></w:p>"#,
    ));
    let (doc, _) = ooxml::read(Cursor::new(&src)).expect("not read");
    assert_eq!(doc.kern, Some(1.0));
    let d = kumihan::theme::compose(&doc, &kumihan::theme::default_theme());
    let kern = |t: &str| d.paragraphs().flat_map(|p| p.runs.iter()).find(|r| r.text == t).unwrap().fmt.kern;
    assert_eq!(kern("既定"), Some(1.0), "the document default");
    assert_eq!(kern("見出し"), Some(14.0), "the paragraph style");
    assert_eq!(kern("自分"), Some(10.0), "the run's own");

    // A run's own value goes back as it was; the others stay with the styles
    let mut out = Vec::new();
    ooxml::write(&doc, Cursor::new(&mut out)).expect("not written");
    let mut z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
    let mut xml = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    assert_eq!(xml.matches("<w:kern ").count(), 1, "{xml}");
    assert!(xml.contains(r#"<w:kern w:val="20"/>"#), "{xml}");
}
