fn main() {
    // With a family name, print that family's faces (regular, bold, italic).
    // The screen registers all of them, so this is how to check one
    if let Some(name) = std::env::args().nth(1) {
        let faces = kumihan::font::faces(&name);
        println!("{name}: {} 面", faces.len());
        for f in faces {
            println!(
                "  {:<34} bold={} italic={} group={} {}#{}",
                f.name,
                f.bold,
                f.italic,
                f.group,
                f.path.file_name().unwrap().to_string_lossy(),
                f.index
            );
        }
        return;
    }
    let all = kumihan::font::list();
    println!("見つかった書体: {}", all.len());
    for f in all.iter().filter(|f| f.japanese).take(25) {
        println!("  {:<34} {}", f.name, f.path.file_name().unwrap().to_string_lossy());
    }
}
