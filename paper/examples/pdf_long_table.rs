//! 頁をまたぐ長い表で、見出しの行が繰り返されるかを見る。
fn main() {
    let mut src = String::from("= 在庫表\n\n|===\n|品名 |数量 |単価\n\n");
    for i in 1..=60 {
        src.push_str(&format!("|品目{i} |{} |{}\n", i * 3, i * 120));
    }
    src.push_str("|===\n");
    let doc = kumihan::adoc::parse(&src).expect("読めない");
    let (sheet, page, bytes) = paper::doc_to_sheet(&doc, None).expect("組めない");
    let pp = paper::Paper::from_page(&page);
    let mut out = Vec::new();
    paper::pdfw::sheet_to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut out)).expect("PDF");
    std::fs::write("test/out/長い表.pdf", &out).unwrap();
    println!("見出しの表: {:?}", sheet.header_tables);
    println!("{} バイト", out.len());
}
