//! ピボットの機能を1つずつ出す。
//!     cargo run -q -p pivot --example try
fn main() {
    let head: Vec<String> =
        ["支店", "月", "品名", "金額"].iter().map(|s| s.to_string()).collect();
    let body: Vec<Vec<String>> = [
        ["東京", "2026-04-05", "ペン", "1000"],
        ["東京", "2026-04-20", "ノート", "500"],
        ["東京", "2026-05-02", "ペン", "1200"],
        ["大阪", "2026-04-11", "ペン", "800"],
        ["大阪", "2026-05-09", "ノート", "300"],
        ["大阪", "2026-05-30", "ペン", "700"],
    ]
    .iter()
    .map(|r| r.iter().map(|s| s.to_string()).collect())
    .collect();

    let moto = || pivot::Spec {
        rows: vec!["支店".into()],
        value: "金額".into(),
        agg: "sum".into(),
        totals: true,
        compact: true,
        subtotal_label: "{} 小計".into(),
        grand_label: "総計".into(),
        ..Default::default()
    };

    let mut m = Vec::new();
    m.push(("素の合計", moto()));
    m.push(("絞り込み(ペンだけ)", pivot::Spec {
        hide: vec![("品名".into(), vec!["ノート".into()])], ..moto() }));
    m.push(("グループ化(月)", pivot::Spec {
        rows: vec!["月".into()],
        group_by: vec![("月".into(), "months".into())], ..moto() }));
    m.push(("小計つき(支店×品名)", pivot::Spec {
        rows: vec!["支店".into(), "品名".into()], subtotals: true, ..moto() }));
    m.push(("値のフィルター(1000超)", pivot::Spec {
        vfilter: Some((">".into(), 1000.0)), ..moto() }));
    m.push(("並べ替え(値の大きい順)", pivot::Spec {
        sort: "largest_value_first".into(), totals: false, ..moto() }));
    m.push(("比率", pivot::Spec {
        show_as: "total".into(), totals: false, ..moto() }));
    m.push(("累計", pivot::Spec {
        show_as: "running_total".into(), totals: false, ..moto() }));

    for (name, spec) in m {
        println!("== {name}");
        match pivot::run(&head, &body, &spec) {
            Ok(t) => for (k, r) in t.kinds.iter().zip(&t.rows) {
                println!("   {k} {}", r.join(" | "));
            },
            Err(e) => println!("   落ちた: {e}"),
        }
    }
}
