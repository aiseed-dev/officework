//! 埋めたブックから `.tmpl.adoc` を書き出して見せる(目で確かめる用)。
fn main() {
    let b = kumihan::book::holes::filled_book();
    let t = kumihan::booktmpl::from_book(&b);
    print!("{}", kumihan::booktmpl::write(&t));
}
