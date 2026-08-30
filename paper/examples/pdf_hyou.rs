//! 表計算の PDF を実物で見るための見本。
//! `cargo run -p paper --example pdf_hyou` で test/out に落ちます。
use book::{Cell, Pos, Value};

fn main() {
    let mut sh = book::Sheet::new("売上");
    sh.print_headings = true;
    sh.print_gridlines = true;
    sh.header = Some("&L2026年度 売上表&R&P / &N".into());
    sh.footer = Some("&C社外秘".into());
    sh.print_title_rows = Some((0, 0));
    for (c, t) in ["支店", "4月", "5月", "6月", "合計"].iter().enumerate() {
        let p = Pos::new(0, c as u32);
        let mut c = Cell { value: Value::Text((*t).into()), ..Default::default() };
        c.fmt.bold = true;
        c.fmt.fill = Some("#DDE7F0".into());
        c.fmt.borders.bottom.on = true;
        sh.set(p, c);
    }
    let n: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(7);
    let moto = ["札幌", "仙台", "東京", "名古屋", "大阪", "広島", "福岡"];
    let mise: Vec<String> =
        (0..n).map(|i| format!("{}{}", moto[i % moto.len()], i / moto.len() + 1)).collect();
    for (r, m) in mise.iter().enumerate() {
        let r = r as u32 + 1;
        sh.set(Pos::new(r, 0), Cell { value: Value::Text(m.clone()), ..Default::default() });
        for c in 1..4u32 {
            let mut cc = Cell {
                value: Value::Number((r * 137 + c * 41) as f64 * 1000.0),
                ..Default::default()
            };
            cc.fmt.number_format = Some("#,##0".into());
            sh.set(Pos::new(r, c), cc);
        }
        let mut sum = Cell::input(&format!("=SUM(B{0}:D{0})", r + 1));
        sum.fmt.number_format = Some("#,##0".into());
        sh.set(Pos::new(r, 4), sum);
        if r.is_multiple_of(2) {
            for c in 0..5u32 {
                let p = Pos::new(r, c);
                let mut cc = sh.get(p).cloned().unwrap_or_default();
                cc.fmt.fill = Some("#F5F7FA".into());
                sh.set(p, cc);
            }
        }
    }
    let mut bk = book::Book::new();
    bk.sheets.clear();
    bk.sheets.push(sh);
    book::calc::recalc_all(&mut bk);
    let name = format!("test/out/見本-表計算{}.pdf", if n > 7 { "-長い" } else { "" });
    let path = std::path::Path::new(&name);
    ops::pdf::book(&bk, path).expect("PDF");
    println!("{} — {} バイト", path.display(), std::fs::metadata(path).unwrap().len());
}
