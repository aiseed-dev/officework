//! 様式(セル)のテンプレートを読んで見る
fn main() {
    let p = std::env::args().nth(1).expect("toml");
    let src = std::fs::read_to_string(&p).unwrap();
    match kumihan::theme::parse(&src) {
        Ok(th) => {
            for f in &th.forms {
                println!("様式「{}」", f.name);
                for (i, r) in f.rows.iter().enumerate() {
                    println!("  {} 行目: セル={:?} 幅={:?}", i + 1, r.cells, r.widths);
                }
            }
            print!("--- 書き戻し ---\n{}", kumihan::theme::write(&th));
        }
        Err(e) => println!("読めません: {e}"),
    }
}
