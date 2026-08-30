//! 実物の紙面を絵にして見る。
//!     cargo run -p paper --example e_sample
fn main() {
    // 表計算の紙面を1枚組んで、そのまま絵にします
    let mut sh = book::Sheet::new("売上");
    sh.print_gridlines = true;
    sh.print_headings = true;
    for (c, t) in ["支店", "4月", "5月", "合計"].iter().enumerate() {
        let p = book::Pos::new(0, c as u32);
        let mut cell = book::Cell {
            value: book::Value::Text((*t).into()),
            ..Default::default()
        };
        cell.fmt.bold = true;
        cell.fmt.fill = Some("DDE7F0".into());
        cell.fmt.borders.bottom.on = true;
        sh.set(p, cell);
    }
    for (r, m) in ["東京", "大阪", "名古屋"].iter().enumerate() {
        let r = r as u32 + 1;
        sh.set(book::Pos::new(r, 0), book::Cell {
            value: book::Value::Text((*m).into()), ..Default::default() });
        for c in 1..3u32 {
            let mut cell = book::Cell {
                value: book::Value::Number((r * 300 + c * 120) as f64),
                ..Default::default()
            };
            cell.fmt.number_format = Some("#,##0".into());
            if r.is_multiple_of(2) {
                cell.fmt.fill = Some("F5F7FA".into());
            }
            sh.set(book::Pos::new(r, c), cell);
        }
    }
    let mut bk = book::Book::new();
    bk.sheets.clear();
    bk.sheets.push(sh);
    book::calc::recalc_all(&mut bk);

    // PDF を作る道と同じ入り口を通し、紙面(Leaf)を取り出して絵にします
    let mut pdf = Vec::new();
    let setup = paper::grid::PrintSetup::default();
    let font = ops::font_data();
    paper::grid::sheet_to_pdf(&bk.sheets[0], font, paper::Paper::default(), &setup,
                              std::io::Cursor::new(&mut pdf)).expect("PDF");
    std::fs::write("test/out/紙面.pdf", &pdf).expect("PDF を置く");
    println!("PDF: {} バイト", pdf.len());

    // 同じ紙面を絵に(いまは罫線と塗りだけ。字はこれから)
    let leaf = paper::grid::sheet_leaf(&bk.sheets[0], paper::Paper::default(), &setup)
        .expect("紙面");
    let e = paper::e::egaku_with(&leaf, 210.0, 297.0, 3.0, Some(font));
    std::fs::write("test/out/紙面.png", e.png().expect("PNG")).expect("書き出し");
    println!("test/out/紙面.png: {}×{} 画素 / 指紋 {}", e.w, e.h, e.yubi());
}
