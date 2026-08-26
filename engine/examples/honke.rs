//! 本家 asciidoctor の .adoc を全部通して、断りと往復の変化を数える
fn main() {
    let root = std::path::Path::new("vendor/asciidoctor");
    let mut files = Vec::new();
    fn collect_into(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_into(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("adoc") {
                out.push(p);
            }
        }
    }
    collect_into(root, &mut files);
    files.sort();
    let (mut note_div, mut same, mut changed) = (0, 0, 0);
    let mut ledger: std::collections::BTreeMap<String, usize> = Default::default();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        match kumihan::adoc::parse_full(&src) {
            Err(_) => note_div += 1,
            Ok((doc, notes)) => {
                for n in notes {
                    let name = n.split(" ×").next().unwrap_or(&n).to_string();
                    *ledger.entry(name).or_default() += 1;
                }
                if kumihan::adoc::write(&doc) == src { same += 1 } else { changed += 1 }
            }
        }
    }
    println!("{} 枚: 断り {note_div} / 1バイトも変わらない {same} / 変わった {changed}", files.len());
    let mut v: Vec<_> = ledger.into_iter().collect();
    v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (k, n) in v.iter().take(12) {
        println!("  {n:4} {k}");
    }
}
