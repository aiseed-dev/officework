//! adoc を読んで、模型と書き戻しを見る(手で確かめる道具)
fn main() {
    let p = std::env::args().nth(1).expect("adoc");
    let src = std::fs::read_to_string(&p).unwrap();
    let (doc, notes) = kumihan::adoc::parse_full(&src).unwrap();
    if !notes.is_empty() { println!("読めなかった: {}", notes.join("・")); }
    for (i, b) in doc.blocks.iter().enumerate() {
        match b {
            kumihan::Block::Para(p) => {
                let text: String = p.runs.iter().map(|r| r.text.as_str()).collect();
                println!("{i:2} style={:?} id={:?} raw={} {:?}",
                    p.style, p.style_id, p.raw_adoc.is_some(), text);
            }
            kumihan::Block::Table(t) =>
                println!("{i:2} 表 割合={:?} mm={:?}", t.col_ratio, t.col_mm),
        }
    }
    let out = kumihan::adoc::write(&doc);
    println!("--- 書き戻し ---\n{out}");
    println!("--- 往復: {} ---", if out == src { "一致" } else { "変わった" });
}
