//! 書体をどこから拾っているかを出す。
//!
//!     cargo run -p kumihan --example font_dirs
//!     cargo run -p kumihan --example font_dirs -- IPA   # 名前で絞る
//!
//! fontconfig の `fc-list` と突き合わせるための道具です
//! (2026-08-28 発注者「書体の置き場が決め打ちというのはおかしい」)。
fn main() {
    let sagasu = std::env::args().nth(1);
    let fams = kumihan::font::list();
    if let Some(w) = sagasu {
        for f in fams.iter().filter(|f| f.name.contains(&w) || f.ascii.contains(&w)) {
            println!("{}  {}", f.name, f.path.display());
        }
        return;
    }
    println!("{} 書体", fams.len());
    let mut dirs: Vec<String> = fams
        .iter()
        .filter_map(|f| f.path.parent().map(|p| p.display().to_string()))
        .collect();
    dirs.sort();
    dirs.dedup();
    println!("{} か所", dirs.len());
    for d in dirs {
        println!("  {d}");
    }
}
