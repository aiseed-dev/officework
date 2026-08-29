//! 図形の入った docx を書く(実物で確かめるため)。
//!     cargo run -p ooxml --example doc_zukei_docx -- out.docx
fn main() {
    let to = std::env::args().nth(1).expect("出す先");
    let mut doc = kumihan::adoc::parse("= 図形の入った文書\n\n本文です。\n").expect("読めない");
    let hako = |kind: &str, fill: &str, x: f32, y: f32, text: Option<&str>, shadow: bool| {
        kumihan::DocShape {
            page: 0, x_mm: x, y_mm: y, w_mm: 40.0, h_mm: 25.0,
            look: book::SheetShape {
                kind: kind.into(), fill: Some(fill.into()), line: Some("2E5A87".into()),
                line_w: 1.5, alpha: 1.0, shadow,
                text: text.map(|s| s.to_string()), ..Default::default()
            },
        }
    };
    doc.shapes = vec![
        hako("rect", "DDE7F0", 25.0, 80.0, Some("四角"), false),
        hako("roundRect", "F5E6D3", 80.0, 80.0, None, true),
        hako("ellipse", "C0504D", 135.0, 80.0, None, false),
    ];
    let f = std::fs::File::create(&to).expect("作れない");
    ooxml::write(&doc, f).expect("書けない");
    println!("{to}");
}
