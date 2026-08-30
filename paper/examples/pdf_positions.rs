//! いまの道と新しい道で、字の**位置**が一致するかを見る。
//!
//! pdftotext は字が取れるかしか見ません。**ずれていても分かりません。**
fn main() {
    let src = "= 四月の売上\n\n本文です。日本語の行組みもエンジンが折ります。\n\n\
               == 明細\n\n|===\n|品名 |金額\n\n|ボールペン |1,200\n|===\n";
    let doc = kumihan::adoc::parse(src).expect("読めない");
    let (sheet, page, bytes) = paper::doc_to_sheet(&doc, None).expect("組めない");
    let pp = paper::Paper::from_page(&page);

    let mut a = Vec::new();
    paper::to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut a)).expect("いまの道");
    let mut b = Vec::new();
    paper::pdfw::sheet_to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut b)).expect("新しい道");
    std::fs::write("test/out/pos_old.pdf", &a).unwrap();
    std::fs::write("test/out/pos_new.pdf", &b).unwrap();
    println!("いまの道 {} / 新しい道 {} バイト", a.len(), b.len());
}
