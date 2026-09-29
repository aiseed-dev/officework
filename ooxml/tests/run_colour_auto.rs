//! A run that says `<w:color w:val="auto"/>` keeps the automatic colour over
//! the colour of its character style (ECMA-376 17.3.2.6: the element, when
//! present, sets the colour at this level of the style hierarchy, and
//! `auto` lets the consumer choose it).

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
<w:style w:type="paragraph" w:default="1" w:styleId="a"><w:name w:val="Normal"/></w:style>
<w:style w:type="character" w:styleId="ad"><w:name w:val="Placeholder Text"/><w:rPr><w:color w:val="808080"/></w:rPr></w:style>
</w:styles>"#);
    zip.finish().unwrap().into_inner()
}

#[test]
fn a_run_that_says_auto_is_not_coloured_by_its_character_style() {
    let src = docx(concat!(
        r#"<w:p><w:r><w:rPr><w:rStyle w:val="ad"/><w:color w:val="auto"/></w:rPr><w:t>[会社名]</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:rPr><w:rStyle w:val="ad"/></w:rPr><w:t>[番地]</w:t></w:r></w:p>"#,
    ));
    let (doc, _) = ooxml::read(Cursor::new(&src)).expect("not read");
    let d = kumihan::theme::compose(&doc, &kumihan::theme::default_theme());
    let colour = |t: &str| {
        d.paragraphs()
            .flat_map(|p| p.runs.iter())
            .find(|r| r.text == t)
            .expect("run not found")
            .fmt
            .color
            .clone()
    };
    assert_eq!(colour("[会社名]"), None, "the run's automatic colour lost to the style");
    assert_eq!(colour("[番地]").as_deref(), Some("808080"), "the style's colour is gone");

    // Saving keeps the run's own `auto`, so Word does not colour it either
    let mut out = Vec::new();
    ooxml::write(&doc, Cursor::new(&mut out)).expect("not written");
    let mut z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
    let mut xml = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    let at = xml.find("[会社名]").unwrap();
    let run = &xml[xml[..at].rfind("<w:r>").or_else(|| xml[..at].rfind("<w:r ")).unwrap()..at];
    assert!(run.contains(r#"<w:color w:val="auto"/>"#), "auto not written back: {run}");
}
