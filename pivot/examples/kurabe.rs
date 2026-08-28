//! **Python の台本と答えが合うか。** 同じ指図で両方を回して見比べる。
//!     cargo run -q -p pivot --example kurabe
fn main() {
    let head = ["支店", "月", "品名", "金額"].map(String::from).to_vec();
    let body: Vec<Vec<String>> = [
        ["東京", "4月", "ペン", "1000"], ["東京", "4月", "ノート", "500"],
        ["東京", "5月", "ペン", "1200"], ["大阪", "4月", "ペン", "800"],
        ["大阪", "5月", "ノート", "300"], ["大阪", "5月", "ペン", "700"],
        ["札幌", "4月", "ノート", "400"], ["札幌", "5月", "ペン", "900"],
    ].iter().map(|r| r.iter().map(|s| s.to_string()).collect()).collect();

    let bui = [
        ("行だけ", r#""index":["支店"],"columns":[],"subtotals":false"#,
         pivot::Spec { rows: vec!["支店".into()], ..moto() }),
        ("行×列", r#""index":["支店"],"columns":["月"],"subtotals":false"#,
         pivot::Spec { rows: vec!["支店".into()], cols: vec!["月".into()], ..moto() }),
        ("小計つき", r#""index":["支店","品名"],"columns":[],"subtotals":true"#,
         pivot::Spec { rows: vec!["支店".into(), "品名".into()], subtotals: true, ..moto() }),
    ];
    for (name, extra, spec) in bui {
        let rust = pivot::run(&head, &body, &spec).expect("集計");
        let py = python(&head, &body, extra);
        let r: Vec<String> = rust.rows.iter().map(|r| r.join("|")).collect();
        let onaji = r.len() == py.len() && r.iter().zip(&py).all(|(a, b)| a == b);
        println!("{name}: {}", if onaji { "同じ" } else { "違う" });
        if !onaji {
            for i in 0..r.len().max(py.len()) {
                let a = r.get(i).map(|s| s.as_str()).unwrap_or("(無)");
                let b = py.get(i).map(|s| s.as_str()).unwrap_or("(無)");
                println!("   {} Rust={a:32} Python={b}", if a == b { " " } else { "×" });
            }
        }
    }
}

fn moto() -> pivot::Spec {
    pivot::Spec {
        value: "金額".into(), agg: "sum".into(), totals: true, compact: true,
        subtotal_label: "{} 小計".into(), grand_label: "総計".into(),
        ..Default::default()
    }
}

fn python(head: &[String], body: &[Vec<String>], extra: &str) -> Vec<String> {
    let q = |s: &str| format!("{s:?}");
    let h: Vec<String> = head.iter().map(|s| q(s)).collect();
    let r: Vec<String> = body.iter()
        .map(|row| format!("[{}]", row.iter().map(|s| q(s)).collect::<Vec<_>>().join(",")))
        .collect();
    let spec = format!(
        r#"{{"headers":[{}],"rows":[{}],{extra},"value":"金額","agg":"sum","totals":true,"blank_rows":false,"compact":true,"hide":[],"group":[],"show_as":"","sort":"","vfilter":null,"subtotal_label":"{{}} 小計","grand_label":"総計"}}"#,
        h.join(","), r.join(","));
    let dir = std::env::temp_dir().join("pivot-kurabe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("spec.json"), spec).unwrap();
    std::fs::write(dir.join("p.py"), pyrun::PIVOT_PY).unwrap();
    let o = std::process::Command::new("/home/dev/dev/officework/.venv/bin/python")
        .arg(dir.join("p.py")).arg(dir.join("spec.json")).output().unwrap();
    if !o.status.success() {
        return vec![format!("Python が落ちた: {}",
            String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or(""))];
    }
    String::from_utf8_lossy(&o.stdout).split('\u{1e}')
        .map(|l| l.split('\u{1f}').skip(1).collect::<Vec<_>>().join("|"))
        .collect()
}
