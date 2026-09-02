//! 数式を組んで PNG にして見せる(目で確かめる用)。
//!
//!     cargo run -p kumihan --example suushiki -- '\frac{a+b}{2}' 出力.png
fn main() {
    let tex = std::env::args().nth(1).unwrap_or_else(|| r"\frac{a+b}{2}".into());
    let out = std::env::args().nth(2).unwrap_or_else(|| "suushiki.png".into());
    let font = kumihan::font::for_document(None).ok().and_then(|(f, _)| kumihan::font::load(f).ok());
    let t0 = std::time::Instant::now();
    match kumihan::suushiki::kumu(&tex, 11.0, font.as_deref()) {
        Ok(k) => {
            std::fs::write(&out, &k.png).expect("書けない");
            println!("{out}: {:.1} x {:.1} mm, {:?}", k.w_mm, k.h_mm, t0.elapsed());
        }
        Err(e) => println!("組めません: {e}"),
    }
}
