fn main() {
    let head: Vec<String> =
        ["支店", "月", "品名", "金額"].iter().map(|s| s.to_string()).collect();
    let body: Vec<Vec<String>> = [
        ["東京", "4月", "ペン", "1000"],
        ["東京", "4月", "ノート", "500"],
        ["東京", "5月", "ペン", "1200"],
        ["大阪", "4月", "ペン", "800"],
        ["大阪", "5月", "ノート", "300"],
        ["大阪", "5月", "ペン", "700"],
    ]
    .iter()
    .map(|r| r.iter().map(|s| s.to_string()).collect())
    .collect();

    for (name, spec) in [
        ("行だけ(支店ごとの合計)", pivot::Spec {
            rows: vec!["支店".into()],
            value: "金額".into(),
            agg: "sum".into(),
            totals: true,
            ..Default::default()
        }),
        ("行×列(支店×月)", pivot::Spec {
            rows: vec!["支店".into()],
            cols: vec!["月".into()],
            value: "金額".into(),
            agg: "sum".into(),
            totals: true,
        }),
        ("件数", pivot::Spec {
            rows: vec!["支店".into()],
            value: "金額".into(),
            agg: "count".into(),
            ..Default::default()
        }),
    ] {
        println!("== {name}");
        match pivot::run(&head, &body, &spec) {
            Ok(t) => for (k, r) in t.kinds.iter().zip(&t.rows) { println!("   {k} {}", r.join(" | ")); },
            Err(e) => println!("   落ちた: {e}"),
        }
    }
}
