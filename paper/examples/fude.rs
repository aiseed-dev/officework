//! 手描きの筆が紙に出るか。蛍光ペンは字の下、ペンは字の上。
//!     cargo run -p paper --features e --example fude
fn main() {
    let doc = kumihan::adoc::parse(
        "= 手描きの確かめ\n\n蛍光ペンはこの行の字の下に敷きます。\n\nペンはこの行の字の上に乗ります。\n",
    ).expect("読めない");
    let (sheet, page, font) = paper::doc_to_sheet(&doc, None).expect("組めない");
    let hiku = |y: f32, hl: bool| kumihan::Stroke {
        page: 0, highlighter: hl,
        points: (0..40).map(|i| {
            let x = 22.0 + i as f32 * 3.5;
            (x, y + (i as f32 * 0.7).sin() * 1.2)
        }).collect(),
    };
    let dress = paper::PageDress {
        ink: vec![hiku(43.0, true), hiku(58.0, false)],
        ..Default::default()
    };
    let leaves = paper::doc_leaves_with(&sheet, page, &dress);
    let leaf = leaves.into_iter().next().expect("紙面");
    let _ = std::fs::create_dir_all("test/out");
    let e = paper::e::egaku_with(&leaf, page.w_mm, page.h_mm, 3.0, Some(&font));
    std::fs::write("test/out/fude.png", e.png().unwrap()).unwrap();
    let f = std::fs::File::create("test/out/fude.pdf").unwrap();
    paper::pdfw::write_pages(&[leaf], page.w_mm, page.h_mm, &font, f).unwrap();
    println!("test/out/fude.png と .pdf");
}
