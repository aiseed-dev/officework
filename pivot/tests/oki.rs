//! **ピボットをブックへ置く道(apply)の試験。** run の11本(shukei.rs)の
//! 続きで、book への結線 — 置く・太字・広さの控え・置き直しの消し込み —
//! を見る。期待値は shukei.rs と同じ8行の手の検算。

use book::{Book, Cell, Pos, Value};

fn fixture() -> (Book, String) {
    let mut b = Book::new();
    let name = b.sheets[0].name.clone();
    let rows: &[[&str; 4]] = &[
        ["月", "区分", "品名", "金額"],
        ["4月", "文具", "ペン", "100"],
        ["4月", "文具", "ノート", "200"],
        ["4月", "家具", "机", "1000"],
        ["5月", "文具", "ペン", "150"],
        ["5月", "家具", "机", "1100"],
        ["5月", "家具", "椅子", "500"],
        ["6月", "文具", "ノート", "250"],
        ["6月", "文具", "ペン", "120"],
    ];
    let s = &mut b.sheets[0];
    for (r, row) in rows.iter().enumerate() {
        for (c, text) in row.iter().enumerate() {
            s.set(Pos::new(r as u32, c as u32), Cell::input(text));
        }
    }
    (b, name)
}

fn def(sheet: &str) -> book::PivotDef {
    book::PivotDef {
        sheet: sheet.to_string(),
        src: (Pos::parse("A1").unwrap(), Pos::parse("D9").unwrap()),
        rows_sel: vec!["区分".into()],
        value: "金額".into(),
        agg: "sum".into(),
        totals: true,
        dest: Pos::parse("F1").unwrap(),
        ..Default::default()
    }
}

/// dest の列で、1列目が `label` の行の隣の数を返す
fn num_beside(b: &Book, label: &str) -> Option<f64> {
    let s = &b.sheets[0];
    for r in 0..20 {
        let p = Pos::new(r, 5); // F 列
        if let Some(c) = s.get(p) {
            if matches!(&c.value, Value::Text(t) if t == label) {
                if let Some(v) = s.get(Pos::new(r, 6)) {
                    if let Value::Number(n) = v.value {
                        return Some(n);
                    }
                }
            }
        }
    }
    None
}

#[test]
fn apply_writes_the_sums_where_the_def_points() {
    let (mut b, name) = fixture();
    let mut d = def(&name);
    let (h, w) = pivot::apply(&mut b, &mut d).unwrap();
    // 見出し + 明細2 + 総計 = 4行、区分と値の2列
    assert_eq!((h, w), (4, 2));
    assert_eq!(d.size, (4, 2), "広さが控えに入っていない");
    assert_eq!(num_beside(&b, "文具"), Some(820.0));
    assert_eq!(num_beside(&b, "家具"), Some(2600.0));
}

#[test]
fn head_and_total_rows_are_bold_data_rows_are_not() {
    let (mut b, name) = fixture();
    let mut d = def(&name);
    pivot::apply(&mut b, &mut d).unwrap();
    let s = &b.sheets[0];
    let bold = |r: u32| s.get(Pos::new(r, 5)).map(|c| c.fmt.bold).unwrap_or(false);
    assert!(bold(0), "見出しが太字でない");
    assert!(bold(3), "総計が太字でない");
    assert!(!bold(1) && !bold(2), "明細まで太字になっている");
}

#[test]
fn re_applying_a_smaller_grid_clears_the_old_cells() {
    let (mut b, name) = fixture();
    let mut d = def(&name);
    pivot::apply(&mut b, &mut d).unwrap(); // 4行置いた
    // 家具を隠すと 見出し+文具+総計 の3行に縮む
    d.hide = vec![("区分".into(), vec!["家具".into()])];
    let (h, _) = pivot::apply(&mut b, &mut d).unwrap();
    assert_eq!(h, 3);
    let s = &b.sheets[0];
    assert!(
        s.get(Pos::new(3, 5)).is_none() && s.get(Pos::new(3, 6)).is_none(),
        "縮んだのに古い4行目が残っている: {:?}",
        (s.get(Pos::new(3, 5)), s.get(Pos::new(3, 6)))
    );
}

#[test]
fn a_missing_sheet_is_refused_by_name() {
    let (mut b, _) = fixture();
    let mut d = def("無いシート");
    let e = pivot::apply(&mut b, &mut d).unwrap_err();
    assert!(e.contains("無いシート"), "断りに名前が無い: {e}");
}

#[test]
fn a_source_with_only_a_header_is_refused() {
    let (mut b, name) = fixture();
    let mut d = def(&name);
    d.src = (Pos::parse("A1").unwrap(), Pos::parse("D1").unwrap());
    let e = pivot::apply(&mut b, &mut d).unwrap_err();
    assert!(!e.is_empty(), "中身の無い表を黙って通した");
}
