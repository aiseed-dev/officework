//! 実機で見くらべるための xlsx を作る。
//!     cargo run -p paper --example zukei_xlsx -- /path/to/out.xlsx
fn main() {
    let to = std::env::args().nth(1).expect("出す先");
    let mut b = book::Book::new();
    let s = &mut b.sheets[0];
    s.name = "図形".into();
    s.set(book::Pos::new(0, 0), book::Cell {
        value: book::Value::Text("図形の見くらべ".into()), ..Default::default() });
    let hako = |kind: &str, fill: &str, line: &str, alpha: f32, shadow: bool,
                dx: f32, dy: f32| book::SheetShape {
        at: book::Pos::new(2, 0), dx_px: dx, dy_px: dy,
        width_px: 110.0, height_px: 70.0, kind: kind.into(),
        fill: Some(fill.into()), line: Some(line.into()),
        line_w: 1.5, alpha, shadow, ..Default::default()
    };
    for (i, k) in ["rect", "roundRect", "ellipse", "rightArrow", "diamond", "line"]
        .iter().enumerate()
    {
        s.shapes_new.push(hako(k, "DDE7F0", "2E5A87", 1.0, false,
            20.0 + (i % 3) as f32 * 130.0, 20.0 + (i / 3) as f32 * 90.0));
    }
    s.shapes_new.push(hako("roundRect", "4472C4", "2E5A87", 1.0, true, 20.0, 200.0));
    s.shapes_new.push(hako("ellipse", "C0504D", "8C3A38", 0.5, false, 150.0, 200.0));
    let mut sp = hako("rect", "FFF2CC", "BF8F00", 1.0, false, 280.0, 200.0);
    sp.rot = 20.0;
    sp.text = Some("回した箱".into());
    s.shapes_new.push(sp);
    let f = std::fs::File::create(&to).expect("作れない");
    sheet::xlsx::write(&b, f).expect("書けない");
    println!("{to}");
}
