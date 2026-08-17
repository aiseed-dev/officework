//! docx を1つ受け取り、**adoc + テンプレートに分けたときの大きさを数える。**
//!
//! 発注者 2026-08-18「テンプレートと文書をわける。これが docx や odt, odp の
//! 複雑さを改善するのでは」。言い分ではなく数で答えるための道具です。
//!
//! ```bash
//! cargo run -p ooxml --example hakaru -- sample/writer/04_月次報告.docx
//! ```
//!
//! 出すのは、docx の部品の数と要素の数、分けた後の本文とテンプレートの
//! 字数・行数です。**中身が同じものを比べます** — 同じ文書を2つの形で
//! 表したときに、人が読む量がどれだけ違うかを見ます。

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("使い方: cargo run -p ooxml --example hakaru -- <docx>");
        std::process::exit(2);
    };
    let bytes = std::fs::read(&path).expect("読めません");

    // ---- docx の側 ----
    let mut 部品 = Vec::new();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).expect("zip ではない");
    for i in 0..zip.len() {
        use std::io::Read;
        let mut f = zip.by_index(i).unwrap();
        let mut s = Vec::new();
        f.read_to_end(&mut s).ok();
        let 名 = f.name().to_string();
        // XML の要素の数(`<` の数から宣言と閉じを引く近似ではなく、開き札を数える)
        let text = String::from_utf8_lossy(&s);
        let 要素 = text.matches('<').count().saturating_sub(text.matches("</").count());
        部品.push((名, s.len(), 要素));
    }
    let 全要素: usize = 部品.iter().map(|p| p.2).sum();

    // ---- 分けた側 ----
    let (doc, _rep) = ooxml::read(std::io::Cursor::new(bytes)).expect("docx が読めません");
    let (本文, 型, r) = kumihan::distill::distill(&doc);
    let adoc = kumihan::adoc::write(&本文);
    let toml = kumihan::theme::write(&型);

    println!("== {path}");
    println!("docx: {} 個の部品、{} 個の XML の要素、{} バイト",
             部品.len(), 全要素, std::fs::metadata(&path).unwrap().len());
    let mut 大きい: Vec<_> = 部品.iter().filter(|p| p.2 > 0).collect();
    大きい.sort_by_key(|p| std::cmp::Reverse(p.2));
    for (名, _b, 要素) in 大きい.iter().take(5) {
        println!("      {要素:>6} 要素  {名}");
    }
    println!("分けた後:");
    println!("      本文(.adoc)     {:>5} 字 / {:>3} 行",
             adoc.chars().count(), adoc.lines().count());
    println!("      書式(.toml)     {:>5} 字 / {:>3} 行 / スタイル {} 個",
             toml.chars().count(), toml.lines().count(), 型.styles.len());
    println!("      落ちた所: {}", r.dropped);
}
