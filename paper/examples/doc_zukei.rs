//! 文書に貼り付く図形が紙に出るか。
//!     cargo run -p paper --example doc_zukei
fn main() {
    let doc0 = kumihan::adoc::parse("= 図形の入った文書\n\n本文です。図形は本文の上に置きます。\n")
        .expect("読めない");
    let mut doc = doc0;
    let hako = |kind: &str, fill: &str, line: &str, x: f32, y: f32,
                rot: f32, text: Option<&str>, shadow: bool| kumihan::DocShape {
        page: 0, x_mm: x, y_mm: y, w_mm: 40.0, h_mm: 25.0,
        look: book::SheetShape {
            kind: kind.into(),
            fill: Some(fill.into()), line: Some(line.into()),
            line_w: 1.5, alpha: 1.0, rot, shadow,
            text: text.map(|s| s.to_string()),
            ..Default::default()
        },
    };
    doc.shapes = vec![
        hako("rect", "DDE7F0", "2E5A87", 25.0, 80.0, 0.0, Some("四角"), false),
        hako("roundRect", "F5E6D3", "BF8F00", 80.0, 80.0, 0.0, None, true),
        hako("ellipse", "C0504D", "8C3A38", 135.0, 80.0, 0.0, None, false),
        hako("rightArrow", "9BBB59", "6E8B3D", 25.0, 120.0, 0.0, None, false),
        hako("diamond", "8064A2", "403152", 80.0, 120.0, 20.0, None, false),
    ];
    let f = std::fs::File::create("test/out/doc_zukei.pdf").expect("作れない");
    let _ = std::fs::create_dir_all("test/out");
    paper::doc_to_pdf(&doc, None, f).expect("PDF が出ない");
    println!("test/out/doc_zukei.pdf");
}
