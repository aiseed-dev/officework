//! いまの道(printpdf)と新しい道(pdf-writer + subsetter)を見比べる。
fn main() {
    let src = "= 四月の売上\n\n本文です。*太字*と _斜体_ と #色つき# があります。\n\n\
               == 明細\n\n|===\n|品名 |金額\n\n|ボールペン |1,200\n|ノート |480\n|===\n";
    let doc = kumihan::adoc::parse(src).expect("読めない");
    let (sheet, page, bytes) = paper::doc_to_sheet(&doc, None).expect("組めない");
    let pp = paper::Paper::from_page(&page);

    let mut a = Vec::new();
    paper::to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut a)).expect("いまの道");
    std::fs::write("test/out/cmp_printpdf.pdf", &a).unwrap();

    let mut b = Vec::new();
    let lost = paper::pdfw::sheet_to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut b))
        .expect("新しい道");
    std::fs::write("test/out/cmp_pdfw.pdf", &b).unwrap();

    println!("いまの道   {:>10} バイト", a.len());
    println!("新しい道   {:>10} バイト  ({:.0} 分の1)", b.len(), a.len() as f32 / b.len() as f32);
    if !lost.is_empty() {
        println!("載らない物: {lost:?}");
    }
}
