//! **同梱の既定テンプレートが、言語ごとに何を持っているかを出す。**
//!
//! 手引きの「言語と既定の書式」の表が実物と合っているかを見るために
//! 使います。書体が `None` なのは、テンプレートが名指しせず、テーマ
//! (`ooxml::theme::fonts`)が言語から選ぶためです。
//!
//! `cargo run --example tmpl_font -p kumihan`

fn main() {
    let t = kumihan::theme::default_theme();
    println!("既定テンプレートの [文書] の書体: {:?}", t.font);
    println!("既定テンプレートの [文書] の大きさ: {:?}", t.size_pt);
    println!("言語ごとの [文書]: {} 件", t.lang_docs.len());
    for (l, d) in t.lang_docs.iter().take(4) {
        println!("   {l}: 書体 {:?} 大きさ {:?}", d.font, d.size_pt);
    }
    for l in ["ja", "en"] {
        let f = t.for_language(l);
        println!("for_language({l}) → 書体 {:?} 大きさ {:?}", f.font, f.size_pt);
    }
}
