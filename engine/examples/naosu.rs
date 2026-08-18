//! adoc を読んで、新しい書き方に揃えて書き戻す
fn main() {
    for p in std::env::args().skip(1) {
        let src = std::fs::read_to_string(&p).expect("読めません");
        let (doc, _) = kumihan::adoc::parse_full(&src).expect("形");
        let out = kumihan::adoc::write(&doc);
        if out == src {
            println!("そのまま: {p}");
        } else {
            std::fs::write(&p, &out).expect("書けません");
            println!("揃えた:   {p}");
        }
    }
}
