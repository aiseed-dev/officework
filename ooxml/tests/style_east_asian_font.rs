//! A style that names only a Latin theme font keeps the East Asian font of
//! the style it is based on (ECMA-376 17.3.2.26: an `rFonts` attribute that
//! is not present leaves the value of the previous level in the style
//! hierarchy).

use std::io::{Cursor, Write};

#[test]
fn a_latin_theme_font_leaves_the_east_asian_font_to_the_base_style() {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o: zip::write::FileOptions<'_, ()> = Default::default();
    let mut put = |n: &str, d: &str| {
        zip.start_file(n, o).unwrap();
        zip.write_all(d.as_bytes()).unwrap();
    };
    put("[Content_Types].xml", r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#);
    put("_rels/.rels", r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#);
    put("word/_rels/document.xml.rels", r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/></Relationships>"#);
    put("word/document.xml", r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="1"/></w:pPr><w:r><w:t>山田 花子</w:t></w:r></w:p></w:body></w:document>"#);
    put("word/styles.xml", r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:default="1" w:styleId="a"><w:name w:val="Normal"/><w:rPr><w:rFonts w:eastAsia="Hiragino Sans W3"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="1"><w:name w:val="heading 1"/><w:basedOn w:val="a"/><w:rPr><w:rFonts w:asciiTheme="majorHAnsi" w:hAnsiTheme="majorHAnsi"/><w:b/></w:rPr></w:style>
</w:styles>"#);
    put("word/theme/theme1.xml", r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements><a:fontScheme name="t"><a:majorFont><a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Century"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#);
    let src = zip.finish().unwrap().into_inner();
    let (doc, _) = ooxml::read(Cursor::new(src)).expect("not read");
    let (look, _) = doc.style_matome("1").expect("no heading style");
    assert_eq!(look.font_latin.as_deref(), Some("Arial"), "the Latin theme font");
    assert_eq!(look.font.as_deref(), Some("Hiragino Sans W3"), "the East Asian font of the base style");
}
