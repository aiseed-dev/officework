//! **図形を絵にする速さを測る。**
//!
//!     cargo run --release -p paper --features e --example zukei_hakaru
//!
//! gpui の SVG の道と比べるための、vello 側の数です。画面の側は
//! アプリを動かして測ります(ここからは呼べません)。
fn main() {
    let (fam, _) = kumihan::font::for_document(None).expect("書体");
    let data = kumihan::font::load(fam).expect("読めない");
    let kinds = ["rect", "roundRect", "ellipse", "rightArrow", "diamond", "line"];
    for n in [10usize, 100, 500] {
        let shapes: Vec<_> = (0..n)
            .map(|i| {
                let sp = book::SheetShape {
                    at: book::Pos::new(0, 0),
                    width_px: 60.0,
                    height_px: 40.0,
                    kind: kinds[i % kinds.len()].into(),
                    fill: Some("DDE7F0".into()),
                    line: Some("2E5A87".into()),
                    line_w: 1.5,
                    alpha: 1.0,
                    ..Default::default()
                };
                let (x, y) = (10.0 + (i % 10) as f32 * 19.0, 280.0 - (i / 10) as f32 * 14.0);
                (sp, x, y)
            })
            .collect();
        let t0 = std::time::Instant::now();
        let leaf = paper::grid::shapes_leaf(&shapes, paper::Paper::default());
        let kumu = t0.elapsed();
        for bai in [1.5f32, 3.0, 6.0] {
            // **何度か走らせて真ん中を取ります。** 1回だけだと機械の都合で
            // 倍ちがう数が出ます(下の表は3回の中央値)
            let mut ms: Vec<f64> = Vec::new();
            let mut ookisa = (0, 0);
            for _ in 0..3 {
                let t1 = std::time::Instant::now();
                let e = paper::e::egaku_with(&leaf, 210.0, 297.0, bai, Some(&data));
                ms.push(t1.elapsed().as_secs_f64() * 1000.0);
                ookisa = (e.w, e.h);
            }
            ms.sort_by(|a, b| a.partial_cmp(b).expect("数"));
            println!(
                "図形 {n:4} 個 {:>4}×{:<4} 画素: 組む {:>6.2}ms / 絵にする {:>7.2}ms",
                ookisa.0,
                ookisa.1,
                kumu.as_secs_f64() * 1000.0,
                ms[1]
            );
        }
    }
}
