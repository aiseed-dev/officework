//! 埋めたブックから `.tmpl.adoc` を書き出して見せる(目で確かめる用)。
//!
//!     cargo run -p kumihan --example tmpl_sample -- de
fn main() {
    let lang = std::env::args().nth(1).unwrap_or_else(|| "ja".into());
    kumihan::font::set_default_language(&lang);
    let b = kumihan::holes::filled_book();
    let t = kumihan::booktmpl::from_book(&b);
    print!("{}", kumihan::booktmpl::write(&t));
}
