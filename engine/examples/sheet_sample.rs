//! 埋めたブックから `.sheet.adoc` を書き出して見せる(目で確かめる用)。
fn main() {
    print!("{}", kumihan::book_adoc::write(&kumihan::holes::filled_book()));
}
