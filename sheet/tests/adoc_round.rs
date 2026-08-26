//! **実物のブックで adoc の往復を測る。**
//!
//! 作り物の表ではなく `sample/*.xlsx` を読んで、adoc に書き、読み戻して、
//! **値がそのまま戻るか**を1枚ずつ見ます。往復が緑でも「見える物が合って
//! いる」とは言えないので、セルの値を1つずつ突き合わせます。

use sheet::model::Pos;

/// 値の並びを取り出す(比べるため)
fn value_table(b: &sheet::Book) -> Vec<(String, Vec<(u32, u32, String)>)> {
    b.sheets
        .iter()
        .map(|s| {
            let (rows, cols) = s.extent();
            let mut v = Vec::new();
            for r in 0..rows {
                for c in 0..cols {
                    let d = s.value(Pos::new(r, c)).display();
                    if !d.is_empty() {
                        v.push((r, c, d));
                    }
                }
            }
            (s.name.clone(), v)
        })
        .collect()
}

#[test]
fn a_real_book_round_trips_through_adoc() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("sample");
    let mut seen = 0;
    let mut matched = 0;
    let mut report = Vec::new();

    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("sample が無いので飛ばす: {}", dir.display());
        return;
    };
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "xlsx"))
        .collect();
    paths.sort();

    for p in paths {
        let Ok(f) = std::fs::File::open(&p) else { continue };
        let Ok((from, _)) = sheet::xlsx::read(std::io::BufReader::new(f)) else { continue };
        seen += 1;
        let name = p.file_name().unwrap().to_string_lossy().to_string();

        let src = sheet::adoc::write(&from);
        let (back, _) = sheet::adoc::parse(&src).expect("adoc が読めない");

        let a = value_table(&from);
        let b = value_table(&back);
        if a == b {
            matched += 1;
        } else {
            let sheet = a.len().min(b.len());
            let mut delta = Vec::new();
            for i in 0..sheet {
                if a[i] != b[i] {
                    let difference = a[i].1.iter().zip(b[i].1.iter()).filter(|(x, y)| x != y).take(2).collect::<Vec<_>>();
                    delta.push(format!("{} 元{}件→戻{}件 例{:?}", a[i].0, a[i].1.len(), b[i].1.len(), difference));
                }
            }
            report.push(format!("{name}: シート {}→{} / {}", a.len(), b.len(), delta.join(" | ")));
        }
    }

    println!("実物 {seen} 冊のうち {matched} 冊が値まで往復した");
    for r in &report {
        println!("  ちがい: {r}");
    }
    assert!(seen > 0, "実物を1冊も読めていない");
    assert_eq!(matched, seen, "値が往復しないブックがある:\n  {}", report.join("\n  "));
}

/// **画像は黙って落とさない。** 3 MB のブックが 1 KB の adoc になるので、
/// 言わないと消えたことに気づけない(2026-08-19 に実物で見つけた)
#[test]
fn images_are_counted() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("sample").join("写真の台帳.xlsx");
    let Ok(f) = std::fs::File::open(&p) else { return };
    let (b, _) = sheet::xlsx::read(std::io::BufReader::new(f)).expect("読めない");
    let r = sheet::adoc::write_report(&b);
    assert!(r.iter().any(|x| x.contains("画像")), "画像を黙って落とした: {r:?}");
}
