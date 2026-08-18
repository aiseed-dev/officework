//! adoc を HTML にして見る
fn main() {
    let p = std::env::args().nth(1).expect("adoc");
    let src = std::fs::read_to_string(&p).unwrap();
    let (doc, _) = kumihan::adoc::parse_full(&src).unwrap();
    let th = kumihan::theme::default_theme();
    let c = kumihan::theme::compose(&doc, &th);
    println!("{}", kumihan::html_write::body(&c));
}
