//! 本文 + 様式のテンプレート → 升目の表を組んで見る
fn main() {
    let mut a = std::env::args().skip(1);
    let adoc = a.next().expect("使い方: masume <adoc> <toml>");
    let toml = a.next().expect("toml");
    let (mut doc, _) = kumihan::adoc::parse_full(&std::fs::read_to_string(&adoc).unwrap()).unwrap();
    let th = kumihan::theme::parse(&std::fs::read_to_string(&toml).unwrap()).unwrap();
    let says = kumihan::theme::apply_forms(&mut doc, &th);
    for s in &says {
        println!("言うこと: {s}");
    }
    for b in &doc.blocks {
        if let kumihan::Block::Table(t) = b {
            println!("升目 {} 行 / 比 {:?}", t.rows.len(), t.col_ratio);
            for row in &t.rows {
                let text: Vec<String> = row.iter()
                    .map(|c| c.paragraphs.iter().flat_map(|p| p.runs.iter())
                        .map(|r| r.text.as_str()).collect())
                    .collect();
                println!("  {:?}", text);
            }
        }
    }
    let c = kumihan::theme::compose(&doc, &th);
    println!("--- HTML ---\n{}", kumihan::html_write::body(&c));
}
