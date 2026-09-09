//! **docx を組んだ行の位置を出す確認用のスクリプト**(2026-09-09)。Word の PDF と
//! 行の y を並べて、どこで折れ方や送りが変わるかを見るための物です。
//!
//! ```text
//! cargo run --release -p paper --example gyou_probe -- 元.docx 探す字 [前後の行数]
//! ```
//!
//! 「探す字」を含む最初の行の前後を、y(mm)・x(mm)・セル番号(表番号, 行, 列。
//! 中の表は `n` 付き)・字で出します。改ページの位置(`breaks`)も先頭に出します
fn main() -> Result<(), String> {
    let mut a = std::env::args().skip(1);
    let moto = a.next().ok_or("使い方: gyou_probe <docx> <探す字> [前後の行数]")?;
    let key = a.next().unwrap_or_default();
    let n: usize = a.next().and_then(|v| v.parse().ok()).unwrap_or(60);
    let f = std::fs::File::open(&moto).map_err(|e| e.to_string())?;
    let (doc, _) = ooxml::read(std::io::BufReader::new(f))?;
    let (sheet, page, _) = paper::doc_to_sheet(&doc, None)?;
    println!(
        "用紙 {}x{}mm 上 {} 下 {} / 行 {} / 改ページ {:?}",
        page.w_mm, page.h_mm, page.top_mm, page.bottom_mm, sheet.lines.len(), sheet.breaks
    );
    let at = sheet.lines.iter().position(|l| l.text().contains(&key)).unwrap_or(0);
    for (i, l) in sheet.lines.iter().enumerate() {
        if i + 5 < at || i > at + n {
            continue;
        }
        let t: String = l.text().chars().take(16).collect();
        let x = l.cells.first().map(|c| c.x_mm).unwrap_or(-1.0);
        let cell = l.cell.map(|(t, r, c)| {
            let t = if t > usize::MAX / 2 { format!("n{}", t - usize::MAX / 2) } else { t.to_string() };
            format!("({t},{r},{c})")
        });
        println!("{i:5} y={:8.2} x={:6.1} {:>12} {:?}", l.y_mm, x, cell.unwrap_or_default(), t);
    }
    Ok(())
}
