//! adoc を標準入力から読み、書き戻した字を標準出力に出す(本家との突き合わせの道具)。
//! 読めなければ終了コード 2 で、理由を標準エラーに出す
use std::io::{Read, Write};
fn main() {
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).expect("読めない");
    match kumihan::adoc::parse(&src) {
        Ok(doc) => {
            std::io::stdout().write_all(kumihan::adoc::write(&doc).as_bytes()).unwrap();
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}
