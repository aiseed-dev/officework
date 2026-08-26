//! 新規の空の docx を書き出す(教師と比べる用)。
fn main() {
    let d = kumihan::Document::default();
    let mut buf = std::io::Cursor::new(Vec::new());
    ooxml::write(&d, &mut buf).expect("書けない");
    let out = std::env::args().nth(1).unwrap_or_else(|| "test/out/empty_officework.docx".into());
    std::fs::write(&out, buf.into_inner()).expect("置けない");
    println!("{out}");
}
