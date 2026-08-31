//! **ピボットの集計の試験。** polars 移行(2026-08-29)のとき試験が
//! 1本も無いまま入っていた穴を塞ぐ(2026-08-31 発注者「試験全体を
//! 見直して」)。
//!
//! 期待値は全部**手で検算した数**です。実装の出力を貼って作った期待値は
//! 実装が壊れても緑のままなので、ここには置かない。
//!
//! 材料は8行の小さな売上の表。手で足せる大きさに保つこと。
//!
//! | 月 | 区分 | 品名 | 金額 |
//! |----|------|------|------|
//! | 4月 | 文具 | ペン | 100 |
//! | 4月 | 文具 | ノート | 200 |
//! | 4月 | 家具 | 机 | 1000 |
//! | 5月 | 文具 | ペン | 150 |
//! | 5月 | 家具 | 机 | 1100 |
//! | 5月 | 家具 | 椅子 | 500 |
//! | 6月 | 文具 | ノート | 250 |
//! | 6月 | 文具 | ペン | 120 |
//!
//! 文具 = 100+200+150+250+120 = 820(5件)/ 家具 = 1000+1100+500 = 2600(3件)
//! 総計 = 3420

use pivot::{run, Grid, Spec, KIND_BLANK, KIND_DATA, KIND_SUB, KIND_TOTAL};

fn head() -> Vec<String> {
    ["月", "区分", "品名", "金額"].map(String::from).to_vec()
}

fn body() -> Vec<Vec<String>> {
    [
        ["4月", "文具", "ペン", "100"],
        ["4月", "文具", "ノート", "200"],
        ["4月", "家具", "机", "1000"],
        ["5月", "文具", "ペン", "150"],
        ["5月", "家具", "机", "1100"],
        ["5月", "家具", "椅子", "500"],
        ["6月", "文具", "ノート", "250"],
        ["6月", "文具", "ペン", "120"],
    ]
    .iter()
    .map(|r| r.map(String::from).to_vec())
    .collect()
}

fn spec(rows: &[&str], value: &str, agg: &str) -> Spec {
    Spec {
        rows: rows.iter().map(|s| s.to_string()).collect(),
        value: value.into(),
        agg: agg.into(),
        ..Default::default()
    }
}

/// 1列目が `name` の行を探して返す(順序に依存しない)
fn row_of<'a>(g: &'a Grid, name: &str) -> &'a Vec<String> {
    g.rows
        .iter()
        .find(|r| r.first().map(String::as_str) == Some(name))
        .unwrap_or_else(|| panic!("行「{name}」が無い: {:?}", g.rows))
}

/// 最後の欄を数として読む
fn last_num(r: &[String]) -> f64 {
    r.last()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("最後の欄が数でない: {r:?}"))
}

#[test]
fn sum_by_one_heading_with_grand_total() {
    let mut sp = spec(&["区分"], "金額", "sum");
    sp.totals = true;
    sp.grand_label = "総計".into();
    let g = run(&head(), &body(), &sp).unwrap();

    assert_eq!(last_num(row_of(&g, "文具")), 820.0);
    assert_eq!(last_num(row_of(&g, "家具")), 2600.0);
    assert_eq!(last_num(row_of(&g, "総計")), 3420.0);
    // 整数は小数点を付けない決め(1 を 1.0 と刷らない)
    assert_eq!(row_of(&g, "文具").last().unwrap().trim(), "820");
    // 行の種類: 見出しが1、明細が2、総計が1
    let kinds: String = g.kinds.iter().collect();
    assert_eq!(kinds.matches(KIND_DATA).count(), 2, "{kinds}");
    assert_eq!(kinds.matches(KIND_TOTAL).count(), 1, "{kinds}");
}

#[test]
fn count_mean_min_max_median_all_agree_with_hand_arithmetic() {
    for (agg, bungu, kagu) in [
        ("count", 5.0, 3.0),
        ("mean", 164.0, 2600.0 / 3.0),
        ("min", 100.0, 500.0),
        ("max", 250.0, 1100.0),
        ("median", 150.0, 1000.0),
    ] {
        let g = run(&head(), &body(), &spec(&["区分"], "金額", agg)).unwrap();
        let b = last_num(row_of(&g, "文具"));
        let k = last_num(row_of(&g, "家具"));
        assert!((b - bungu).abs() < 1e-9, "{agg} の文具: {b} ≠ {bungu}");
        assert!((k - kagu).abs() < 1e-9, "{agg} の家具: {k} ≠ {kagu}");
    }
}

#[test]
fn spreading_columns_puts_each_month_in_its_own_cell() {
    let mut sp = spec(&["区分"], "金額", "sum");
    sp.cols = vec!["月".into()];
    let g = run(&head(), &body(), &sp).unwrap();

    // 月の並ぶ行(どの行かは形に依存しない — 4月を含む行を探す)
    let label_row = g
        .rows
        .iter()
        .find(|r| r.iter().any(|c| c == "4月"))
        .expect("月の見出しの行が無い");
    let col = |m: &str| {
        label_row
            .iter()
            .position(|c| c == m)
            .unwrap_or_else(|| panic!("{m} の列が無い: {label_row:?}"))
    };
    let bungu = row_of(&g, "文具");
    let kagu = row_of(&g, "家具");
    let num = |r: &Vec<String>, i: usize| r[i].trim().parse::<f64>().unwrap_or(f64::NAN);

    assert_eq!(num(bungu, col("4月")), 300.0); // 100+200
    assert_eq!(num(bungu, col("5月")), 150.0);
    assert_eq!(num(bungu, col("6月")), 370.0); // 250+120
    assert_eq!(num(kagu, col("4月")), 1000.0);
    assert_eq!(num(kagu, col("5月")), 1600.0); // 1100+500
}

#[test]
fn hiding_a_value_drops_its_rows_before_aggregation() {
    let mut sp = spec(&["区分"], "金額", "sum");
    sp.hide = vec![("区分".into(), vec!["家具".into()])];
    let g = run(&head(), &body(), &sp).unwrap();
    assert_eq!(last_num(row_of(&g, "文具")), 820.0);
    assert!(
        !g.rows.iter().any(|r| r.first().map(String::as_str) == Some("家具")),
        "隠した家具が出ている: {:?}",
        g.rows
    );
}

#[test]
fn a_value_filter_applies_after_aggregation() {
    let mut sp = spec(&["区分"], "金額", "sum");
    sp.vfilter = Some((">".into(), 1000.0));
    let g = run(&head(), &body(), &sp).unwrap();
    // 集計後の 820 は落ち、2600 だけ残る(明細の 1000 や 1100 ではない)
    assert!(
        !g.rows.iter().any(|r| r.first().map(String::as_str) == Some("文具")),
        "820 が > 1000 を通っている: {:?}",
        g.rows
    );
    assert_eq!(last_num(row_of(&g, "家具")), 2600.0);
}

#[test]
fn largest_value_first_puts_kagu_before_bungu() {
    let mut sp = spec(&["区分"], "金額", "sum");
    sp.sort = "largest_value_first".into();
    let g = run(&head(), &body(), &sp).unwrap();
    let names: Vec<&str> = g
        .rows
        .iter()
        .zip(&g.kinds)
        .filter(|(_, k)| **k == KIND_DATA)
        .map(|(r, _)| r[0].as_str())
        .collect();
    assert_eq!(names, ["家具", "文具"], "大きい順になっていない");
}

#[test]
fn subtotals_carry_the_group_sum_and_the_label() {
    let mut sp = spec(&["区分", "品名"], "金額", "sum");
    sp.subtotals = true;
    sp.subtotal_label = "{} 計".into();
    let g = run(&head(), &body(), &sp).unwrap();
    let subs: Vec<(&str, f64)> = g
        .rows
        .iter()
        .zip(&g.kinds)
        .filter(|(_, k)| **k == KIND_SUB)
        .map(|(r, _)| (r[0].as_str(), last_num(r)))
        .collect();
    assert!(subs.contains(&("文具 計", 820.0)), "{subs:?}");
    assert!(subs.contains(&("家具 計", 2600.0)), "{subs:?}");
}

#[test]
fn blank_rows_separate_the_groups() {
    let mut sp = spec(&["区分", "品名"], "金額", "sum");
    sp.blank_rows = true;
    let g = run(&head(), &body(), &sp).unwrap();
    assert!(g.kinds.contains(&KIND_BLANK), "空行が無い: {:?}", g.kinds);
}

#[test]
fn show_as_percent_of_total_uses_one_decimal() {
    let mut sp = spec(&["区分"], "金額", "sum");
    sp.show_as = "total".into();
    let g = run(&head(), &body(), &sp).unwrap();
    // 820/3420 = 23.97…% → 24.0% / 2600/3420 = 76.02…% → 76.0%
    assert_eq!(row_of(&g, "文具").last().unwrap().trim(), "24.0%");
    assert_eq!(row_of(&g, "家具").last().unwrap().trim(), "76.0%");
}

#[test]
fn grouping_dates_by_month_collapses_the_rows() {
    // 日付の列で月ごとにまとめる。8行が3行(月)になる
    let head: Vec<String> = ["日付", "金額"].map(String::from).to_vec();
    let body: Vec<Vec<String>> = [
        ["2026-04-05", "100"],
        ["2026-04-20", "1200"],
        ["2026-05-01", "150"],
        ["2026-05-15", "1600"],
        ["2026-06-03", "250"],
        ["2026-06-28", "120"],
    ]
    .iter()
    .map(|r| r.map(String::from).to_vec())
    .collect();
    let mut sp = spec(&["日付"], "金額", "sum");
    sp.group_by = vec![("日付".into(), "months".into())];
    let g = run(&head, &body, &sp).unwrap();
    let data: Vec<f64> = g
        .rows
        .iter()
        .zip(&g.kinds)
        .filter(|(_, k)| **k == KIND_DATA)
        .map(|(r, _)| last_num(r))
        .collect();
    assert_eq!(data.len(), 3, "月にまとまっていない: {:?}", g.rows);
    let mut sorted = data.clone();
    sorted.sort_by(f64::total_cmp);
    assert_eq!(sorted, [370.0, 1300.0, 1750.0]);
}

#[test]
fn missing_headings_are_refused_by_name() {
    let e = run(&head(), &body(), &spec(&["場所"], "金額", "sum")).unwrap_err();
    assert!(e.contains("場所"), "断りに名前が無い: {e}");
    let e2 = run(&head(), &body(), &Spec::default()).unwrap_err();
    assert!(!e2.is_empty(), "行の見出しが無いときに黙っている");
}
