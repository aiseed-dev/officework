//! 影と半透明が PDF と絵で同じに出るかを、並べて見るための例。
//!     cargo run -p paper --features e --example kage_kurabe
fn main() {
    let (fam, _) = kumihan::font::for_document(None).expect("書体");
    let data = kumihan::font::load(fam).expect("読めない");
    let hako = |kind: &str, fill: &str, line: &str, alpha: f32, shadow: bool| {
        book::SheetShape {
            at: book::Pos::new(0, 0), width_px: 110.0, height_px: 70.0,
            kind: kind.into(), fill: Some(fill.into()), line: Some(line.into()),
            line_w: 1.5, alpha, shadow, ..Default::default()
        }
    };
    let leaf = paper::grid::shapes_leaf(
        &[
            (hako("roundRect", "4472C4", "2E5A87", 1.0, true), 25.0, 270.0),
            (hako("ellipse", "C0504D", "8C3A38", 0.5, false), 80.0, 260.0),
            (hako("rect", "9BBB59", "6E8B3D", 0.5, true), 135.0, 270.0),
        ],
        paper::Paper::default(),
    );
    let _ = std::fs::create_dir_all("test/out");
    let e = paper::e::egaku_with(&leaf, 210.0, 297.0, 3.0, Some(&data));
    std::fs::write("test/out/kage.png", e.png().unwrap()).unwrap();
    let f = std::fs::File::create("test/out/kage.pdf").unwrap();
    paper::pdfw::write_pages(&[leaf], 210.0, 297.0, &data, f).unwrap();
    println!("test/out/kage.png と .pdf");
}
