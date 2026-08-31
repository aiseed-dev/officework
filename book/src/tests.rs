//! モデルの試験。


use super::fmt::*;
use super::refs::*;
use super::types::*;

/// **刷る範囲は、中身か飾りのあるセルまで。**
///
/// 2026-08-30、国税庁の酒税の表で見つけました。値も飾りも無いセルが右と
/// 下に並んでいて、それを数えたぶん紙が増えていました(12列のうち中身が
/// あるのは9列)。**空でも罫線のあるセルは刷ります** — 表の枠の一部です。
/// **負の数は書式の負の区画で描く。**
///
/// 2026-08-31、国税庁の酒税の表(Fable の指摘3)。役所の表は負の数を
/// 「△ 5,148」と書きます。書式は `正;負;ゼロ;文字` の区画に分かれますが、
/// 負の区画を見ずに `-` を付けていました。
#[cfg(test)]
mod negative_section_tests {
    use super::*;

    #[test]
    fn a_negative_uses_its_own_section_of_the_format() {
        let n = |v: f64, f: &str| format_value(&Value::Number(v), Some(f), false);
        assert_eq!(n(-5148.0, "#,##0;\"△ \"#,##0"), "△ 5,148", "△ の区画を見ていない");
        assert_eq!(n(1234.0, "#,##0;\"△ \"#,##0"), "1,234", "正の数まで変えた");
        // 括弧の書式(欧米の会計)も同じ道
        assert_eq!(n(-5148.0, "#,##0;[Red](#,##0)"), "(5,148)");
        // 負の区画が無ければ、今までどおり `-` を付けます
        assert_eq!(n(-5148.0, "#,##0"), "-5,148", "区画が無いのに符号が消えた");
    }
}

#[cfg(test)]
mod print_extent_tests {
    use super::*;

    #[test]
    fn the_print_range_stops_at_the_last_thing_worth_printing() {
        let mut s = Sheet { name: "見本".into(), ..Default::default() };
        s.set(Pos::new(0, 0), Cell {
            formula: None, value: Value::Text("あ".into()), fmt: Default::default() });
        // **書式だけ持っていて、値も罫線も塗りも無いセル。**
        // xlsx にはこれが右と下に並んでいることがよくあります
        let mut dake = CellFormat::default();
        dake.color = Some("FF0000".into());
        s.set(Pos::new(9, 9), Cell { formula: None, value: Value::Empty, fmt: dake });
        assert_eq!(s.extent(), (10, 10), "セルの置かれた範囲は変わらない");
        assert_eq!(s.print_extent(), (1, 1), "空のセルまで刷ろうとしている");
        // 罫線を持たせると、刷る範囲に入る
        let mut fmt = CellFormat::default();
        fmt.borders.bottom = Edge::THIN;
        s.set(Pos::new(4, 4), Cell { formula: None, value: Value::Empty, fmt });
        assert_eq!(s.print_extent(), (5, 5), "罫線のあるセルが落ちた");
    }

    /// **表の右の縁を、隣の列の「左の罫線」として書いた帳票。**
    ///
    /// その1列を刷る範囲に数えると、紙が1枚増えます。国税庁の酒税の
    /// 総括表は5シート中4つがこの形でした(2026-08-31)。
    #[test]
    fn a_column_holding_only_a_left_border_is_not_printed() {
        let mut s = Sheet { name: "見本".into(), ..Default::default() };
        for r in 0..3 {
            let mut fmt = CellFormat::default();
            fmt.borders.right = Edge::THIN;
            s.set(Pos::new(r, 0), Cell {
                formula: None, value: Value::Text("あ".into()), fmt });
            // 右隣は、左の罫線だけを持つ空のセル
            let mut hidari = CellFormat::default();
            hidari.borders.left = Edge::THIN;
            s.set(Pos::new(r, 1), Cell { formula: None, value: Value::Empty, fmt: hidari });
        }
        assert_eq!(s.print_extent(), (3, 1), "左の罫線だけの列を刷る範囲に数えた");
        // 上の罫線が付いたら、その列は縁ではないので数える
        let mut ue = CellFormat::default();
        ue.borders.left = Edge::THIN;
        ue.borders.top = Edge::THIN;
        s.set(Pos::new(0, 1), Cell { formula: None, value: Value::Empty, fmt: ue });
        assert_eq!(s.print_extent(), (3, 2), "縁ではない列まで落とした");
    }
}

#[cfg(test)]
mod r1c1_tests {

    use super::*;

    #[test]
    fn a1_and_r1c1_convert_both_ways() {
        let at = Pos::parse("E5").unwrap();
        // 相対・絶対・混在・範囲
        let f = "A1+$B$2*SUM(C3:D4)-E5";
        let r = formula_to_r1c1(f, at);
        assert_eq!(r, "R[-4]C[-4]+R2C2*SUM(R[-2]C[-2]:R[-1]C[-1])-RC", "{r}");
        assert_eq!(formula_from_r1c1(&r, at), "A1+$B$2*SUM(C3:D4)-E5");
        // 関数名 LOG10( と文字列は触らない
        let f2 = "LOG10(A1)&\"B2 のまま\"";
        let r2 = formula_to_r1c1(f2, at);
        assert_eq!(r2, "LOG10(R[-4]C[-4])&\"B2 のまま\"", "{r2}");
        assert_eq!(formula_from_r1c1(&r2, at), f2);
        // ROUND( の R は参照ではない
        assert_eq!(formula_from_r1c1("ROUND(R[1]C,2)", at), "ROUND(E6,2)");
        // 範囲の外に出る相対参照は #REF!
        assert_eq!(formula_from_r1c1("R[-9]C", at), "#REF!");
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    fn s_with(cells: &[(&str, &str)]) -> Sheet {
        let mut s = Sheet::new("試");
        for (p, v) in cells {
            s.set(Pos::parse(p).unwrap(), Cell::input(v));
        }
        s
    }

    fn p(s: &str) -> Pos {
        Pos::parse(s).unwrap()
    }

    #[test]
    fn merging_clears_all_but_the_top_left_and_keeps_formats() {
        let mut s = s_with(&[("A1", "題"), ("B1", "消える"), ("B2", "123")]);
        let mut c = s.get(p("B1")).cloned().unwrap();
        c.fmt.number_format = Some("@".into());
        s.set(p("B1"), c);
        let promoted = s.merge(p("A1"), p("B2"));
        assert!(!promoted, "左上に中身があるので移さない");
        assert_eq!(s.merges, vec![(p("A1"), p("B2"))]);
        assert_eq!(s.value(p("A1")), Value::Text("題".into()));
        assert!(s.value(p("B1")).is_empty(), "呑まれた中身は消える");
        assert!(s.value(p("B2")).is_empty());
        assert_eq!(
            s.get(p("B1")).unwrap().fmt.number_format.as_deref(),
            Some("@"),
            "書式は残る"
        );
    }

    #[test]
    fn the_first_content_moves_to_an_empty_top_left_with_its_format() {
        let mut s = s_with(&[("B1", "題")]);
        let mut c = s.get(p("B1")).cloned().unwrap();
        c.fmt.number_format = Some("@".into());
        s.set(p("B1"), c);
        let promoted = s.merge(p("A1"), p("C1"));
        assert!(promoted, "移したと言う(呼び側が言葉で言うため)");
        assert_eq!(s.value(p("A1")), Value::Text("題".into()));
        assert_eq!(
            s.get(p("A1")).unwrap().fmt.number_format.as_deref(),
            Some("@"),
            "値だけでなく書式ごと移る"
        );
        assert!(s.value(p("B1")).is_empty());
    }

    #[test]
    fn overlapping_merges_are_released_first_and_the_count_returned() {
        let mut s = s_with(&[]);
        s.merge(p("A1"), p("B2"));
        s.merge(p("B2"), p("C3")); // 重なる → 前のが外れる(入れ子は帳票を壊す)
        assert_eq!(s.merges, vec![(p("B2"), p("C3"))]);
        s.merge(p("E1"), p("F1"));
        assert_eq!(s.unmerge(p("A1"), p("Z9")), 2, "掛かる結合を数えて外す");
        assert!(s.merges.is_empty());
        // 向きが逆でも直す
        let mut s = s_with(&[]);
        s.merge(p("B2"), p("A1"));
        assert_eq!(s.merges, vec![(p("A1"), p("B2"))]);
    }
}

#[cfg(test)]
mod cell_basics {
    use super::*;

    #[test]
    fn a1_style_reads_and_writes() {
        for (s, r, c) in [("A1", 0, 0), ("B3", 2, 1), ("Z1", 0, 25),
                          ("AA1", 0, 26), ("AB10", 9, 27), ("$C$5", 4, 2)] {
            let p = Pos::parse(s).unwrap_or_else(|| panic!("{s} を読めない"));
            assert_eq!((p.row, p.col), (r, c), "{s}");
        }
        for s in ["A1", "B3", "Z1", "AA1", "AB10"] {
            assert_eq!(Pos::parse(s).unwrap().a1(), s);
        }
        assert!(Pos::parse("A0").is_none(), "0行は無い");
        assert!(Pos::parse("1A").is_none());
    }

    #[test]
    fn input_splits_into_formula_and_value() {
        assert_eq!(Cell::input("123").value, Value::Number(123.0));
        assert_eq!(Cell::input("1.5").value, Value::Number(1.5));
        assert_eq!(Cell::input("サンプル商事").value, Value::Text("サンプル商事".into()));
        assert_eq!(Cell::input("TRUE").value, Value::Bool(true));
        assert_eq!(Cell::input("=SUM(A1:A3)").formula.as_deref(), Some("SUM(A1:A3)"));
        assert!(Cell::input("  ").formula.is_none());
    }

    #[test]
    fn the_edit_box_shows_the_formula_again() {
        let mut c = Cell::input("=A1+1");
        c.value = Value::Number(42.0);
        assert_eq!(c.editable(), "=A1+1", "計算後も編集欄には式を出す");
        assert_eq!(c.value.display(), "42");
    }

    #[test]
    fn number_display_suits_office_work() {
        assert_eq!(Value::Number(1000.0).display(), "1000", "整数に .0 を付けない");
        assert_eq!(Value::Number(1.5).display(), "1.5");
        assert_eq!(Value::Empty.display(), "");
    }

    /// **手引きに書いた数の見え方を、そのまま試験にする**(2026-08-22)。
    ///
    /// `docs/ja/from-excel.adoc` の「数値の精度と入力の落とし穴」には、
    /// Excel と違う所を1行ずつ書いてあります。文書だけに書いてあると、
    /// 実装が動いたときに黙ってずれます。ここで留めます。
    #[test]
    fn numbers_display_as_the_manual_says() {
        let d = |n: f64| Value::Number(n).display();

        // 「16桁の 4111111111111111 はそのまま出る」
        assert_eq!(d(4111111111111111.0), "4111111111111111");
        // 「2^53 を超える整数は二進の丸めで変わり、
        //   9999999999999999 と打つと 10000000000000000 になる」
        assert_eq!(d(9999999999999999.0), "10000000000000000");
        // 「一般の表示は指数へ自動で切り替えない —
        //   1e21 と打つと 1000000000000000000000、1e-7 は 0.0000001」
        assert_eq!(d(1e21), "1000000000000000000000");
        assert_eq!(d(1e-7), "0.0000001");
        // 「0.1+0.2 は化粧をせず 0.30000000000000004 とそのまま出る」
        assert_eq!(d(0.1 + 0.2), "0.30000000000000004");
        // 論理値は TRUE / FALSE(大文字)
        assert_eq!(Value::Bool(true).display(), "TRUE");
        assert_eq!(Value::Bool(false).display(), "FALSE");
    }
}

#[cfg(test)]
mod rowcol_tests {
    use super::*;

    fn sheet() -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for r in 0..3 {
            s.set(Pos { row: r, col: 0 }, Cell {
                formula: None, value: Value::Number(r as f64), fmt: Default::default() });
        }
        s
    }

    fn at(s: &Sheet, r: u32) -> Option<f64> {
        match s.get(Pos { row: r, col: 0 }).map(|c| c.value.clone()) {
            Some(Value::Number(n)) => Some(n),
            _ => None,
        }
    }

    #[test]
    fn inserting_a_row_pushes_down() {
        let mut s = sheet();
        s.insert_row(1);
        assert_eq!(at(&s, 0), Some(0.0));
        assert_eq!(at(&s, 1), None, "挿した行が空でない");
        assert_eq!(at(&s, 2), Some(1.0), "下がっていない");
        assert_eq!(at(&s, 3), Some(2.0));
    }

    #[test]
    fn deleting_a_row_closes_the_gap() {
        let mut s = sheet();
        s.remove_row(1);
        assert_eq!(at(&s, 0), Some(0.0));
        assert_eq!(at(&s, 1), Some(2.0), "詰まっていない");
        assert_eq!(at(&s, 2), None, "元の場所が残っている");
    }

    #[test]
    fn columns_move_the_same_way() {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos { row: 0, col: 0 }, Cell {
            formula: None, value: Value::Text("左".into()), fmt: Default::default() });
        s.set(Pos { row: 0, col: 1 }, Cell {
            formula: None, value: Value::Text("右".into()), fmt: Default::default() });
        s.insert_col(1);
        assert!(s.get(Pos { row: 0, col: 1 }).is_none());
        assert_eq!(s.get(Pos { row: 0, col: 2 }).map(|c| c.value.clone()),
                   Some(Value::Text("右".into())));
        s.remove_col(0);
        assert_eq!(s.get(Pos { row: 0, col: 1 }).map(|c| c.value.clone()),
                   Some(Value::Text("右".into())));
    }

    #[test]
    fn borders_move_along() {
        // 帳票の枠が置き去りになると書類が壊れる
        let mut s = Sheet { name: "枠".into(), ..Default::default() };
        s.set(Pos { row: 1, col: 0 }, Cell {
            formula: None, value: Value::Empty,
            fmt: CellFormat { borders: Borders::ALL, ..Default::default() } });
        s.insert_row(0);
        assert!(s.get(Pos { row: 1, col: 0 }).is_none(), "元の場所に残っている");
        assert_eq!(s.get(Pos { row: 2, col: 0 }).map(|c| c.fmt.borders), Some(Borders::ALL));
    }

    #[test]
    fn an_empty_sheet_does_not_panic() {
        let mut s = Sheet { name: "空".into(), ..Default::default() };
        s.insert_row(0);
        s.remove_row(0);
        s.insert_col(0);
        s.remove_col(0);
        assert!(s.cells.is_empty());
    }
}

#[cfg(test)]
mod format_tests {
    use super::*;

    fn f(n: f64, code: &str) -> String {
        format_value(&Value::Number(n), Some(code), false)
    }

    #[test]
    fn leading_zero_format() {
        // **品番・会員番号・郵便番号の定番。** 2026-08-15 に種苗の会の
        // 注文書の見本を実機で見て、番号の欄が 0001 でなく 1 で並んで
        // いるので気づいた(#,##0.00 や ¥#,##0 は効いていたので
        // 書式そのものが動いていないようには見えなかった)
        assert_eq!(f(1.0, "0000"), "0001");
        assert_eq!(f(23.0, "0000"), "0023");
        assert_eq!(f(12345.0, "0000"), "12345", "桁が多いときは切らない");
        assert_eq!(f(7.0, "000000"), "000007", "郵便番号の桁");
        // 詰めるのは桁区切りの前(Excel も 00,000 で 1234 → 01,234)
        assert_eq!(f(1234.0, "00,000"), "01,234");
        // # は詰めない。既に効いていた書式を壊していないこと
        assert_eq!(f(1.0, "#,##0"), "1");
        assert_eq!(f(360.0, "¥#,##0"), "¥360");
        assert_eq!(f(1234.5, "#,##0.00"), "1,234.50");
        assert_eq!(f(0.0, "0000"), "0000");
    }

    #[test]
    fn exponent_and_text_formats() {
        assert_eq!(f(12345.0, "0.00E+00"), "1.23E+04");
        assert_eq!(f(0.00123, "0.00E+00"), "1.23E-03");
        assert_eq!(f(-4500.0, "0.00E+00"), "-4.50E+03");
        assert_eq!(f(0.0, "0.00E+00"), "0.00E+00");
        assert_eq!(f(1234.5, "@"), "1234.5", "テキスト形式は素のまま");
    }

    #[test]
    fn thousands_separator() {
        assert_eq!(f(1234567.0, "#,##0"), "1,234,567");
        assert_eq!(f(0.0, "#,##0"), "0");
        assert_eq!(f(999.0, "#,##0"), "999");
    }

    #[test]
    fn decimal_format() {
        // 見たいのは「桁を落として丸める」ことだけ。**3.14159 と書かない** —
        // clippy が π の近似と見て撥ねる(approx_constant)。数に意味は無いので
        // 別の数にする。3桁目で切り下がる同じ場合を見ている
        assert_eq!(f(1.23456, "0.00"), "1.23");
        assert_eq!(f(3.0, "0.00"), "3.00");
        assert_eq!(f(1234.5, "#,##0.0"), "1,234.5");
    }

    #[test]
    fn percent_format() {
        assert_eq!(f(0.25, "0%"), "25%");
        assert_eq!(f(0.1234, "0.00%"), "12.34%");
    }

    #[test]
    fn currency_format() {
        assert_eq!(f(1200.0, "¥#,##0"), "¥1,200");
    }

    #[test]
    fn negative_numbers() {
        assert_eq!(f(-1234.0, "#,##0"), "-1,234");
        assert_eq!(f(-0.5, "0%"), "-50%");
    }

    #[test]
    fn with_no_format_the_value_shows_as_is() {
        assert_eq!(format_value(&Value::Number(1234.0), None, false), "1234");
    }

    #[test]
    fn non_numbers_are_left_alone() {
        assert_eq!(format_value(&Value::Text("品名".into()), Some("#,##0"), false), "品名");
        assert_eq!(format_value(&Value::Error("#DIV/0!".into()), Some("0%"), false), "#DIV/0!");
    }
}

#[cfg(test)]
mod ref_tests {
    use super::*;

    #[test]
    fn refs_below_an_inserted_row_move_down() {
        assert_eq!(shift_refs("=A5+B6", 2, 1, true), "=A6+B7");
    }

    #[test]
    fn rows_above_an_inserted_row_stay() {
        assert_eq!(shift_refs("=A1+A2", 5, 1, true), "=A1+A2");
    }

    #[test]
    fn rows_below_a_deleted_row_move_up() {
        assert_eq!(shift_refs("=A5", 2, -1, true), "=A4");
    }

    #[test]
    fn a_ref_to_a_deleted_row_becomes_a_ref_error() {
        // 黙って隣のセルを指すより、壊れたと言う方がよい
        assert_eq!(shift_refs("=A3+B1", 2, -1, true), "=#REF!+B1");
    }

    #[test]
    fn absolute_refs_keep_their_shape() {
        // 利用者が書いた $ を勝手に消さない
        assert_eq!(shift_refs("=$A$5", 2, 1, true), "=$A$6");
        assert_eq!(shift_refs("=$A5", 2, 1, true), "=$A6");
    }

    #[test]
    fn column_insert_and_delete_work_too() {
        assert_eq!(shift_refs("=C1+A1", 1, 1, false), "=D1+A1");
        assert_eq!(shift_refs("=C1", 1, -1, false), "=B1");
    }

    #[test]
    fn a_function_name_is_not_mistaken_for_a_ref() {
        assert_eq!(shift_refs("=SUM(A5:A9)", 2, 1, true), "=SUM(A6:A10)");
        assert_eq!(shift_refs("=IF(A5>0,1,0)", 2, 1, true), "=IF(A6>0,1,0)");
    }

    #[test]
    fn inside_a_string_is_left_alone() {
        assert_eq!(shift_refs(r#"="A5は合計"&A5"#, 2, 1, true), r#"="A5は合計"&A6"#);
    }

    #[test]
    fn a_formula_of_numbers_only_does_not_change() {
        assert_eq!(shift_refs("=1+2*3", 0, 1, true), "=1+2*3");
    }
}

#[cfg(test)]
mod rowcol_formula_tests {
    use super::*;

    fn sheet() -> Sheet {
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        for r in 0..3 {
            s.set(Pos { row: r, col: 0 }, Cell {
                formula: None, value: Value::Number((r + 1) as f64), fmt: Default::default() });
        }
        // A4 = SUM(A1:A3)
        s.set(Pos { row: 3, col: 0 }, Cell {
            formula: Some("=SUM(A1:A3)".into()), value: Value::Empty, fmt: Default::default() });
        s
    }

    fn f(s: &Sheet, r: u32) -> Option<String> {
        s.get(Pos { row: r, col: 0 }).and_then(|c| c.formula.clone())
    }

    #[test]
    fn inserting_a_row_stretches_formula_refs() {
        // これを直さないと、行を挿した瞬間に合計が合わなくなる
        let mut s = sheet();
        s.insert_row(1);
        assert_eq!(f(&s, 4).as_deref(), Some("=SUM(A1:A4)"), "参照が伸びていない");
    }

    #[test]
    fn deleting_a_row_shrinks_formula_refs() {
        let mut s = sheet();
        s.remove_row(1);
        assert_eq!(f(&s, 2).as_deref(), Some("=SUM(A1:A2)"), "参照が縮んでいない");
    }

    #[test]
    fn deleting_the_target_yields_a_ref_error() {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos { row: 0, col: 0 }, Cell {
            formula: Some("=A3".into()), value: Value::Empty, fmt: Default::default() });
        s.remove_row(2);
        assert_eq!(f(&s, 0).as_deref(), Some("=#REF!"), "壊れたのに黙って別のセルを指した");
    }
}

#[cfg(test)]
mod col_formula_tests {
    use super::*;

    #[test]
    fn inserting_or_deleting_columns_fixes_formulas() {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos { row: 0, col: 3 }, Cell {
            formula: Some("=B1+C1".into()), value: Value::Empty, fmt: Default::default() });
        s.insert_col(1);
        assert_eq!(s.get(Pos { row: 0, col: 4 }).and_then(|c| c.formula.clone()).as_deref(),
                   Some("=C1+D1"), "列を挿しても参照が動いていない");
        s.remove_col(1);
        assert_eq!(s.get(Pos { row: 0, col: 3 }).and_then(|c| c.formula.clone()).as_deref(),
                   Some("=B1+C1"), "列を抜いても参照が戻っていない");
    }
}

#[cfg(test)]
mod sort_tests {
    use super::*;

    fn table(rows: &[(&str, f64)], header: bool) -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        let mut r = 0u32;
        if header {
            s.set(Pos { row: 0, col: 0 }, Cell {
                formula: None, value: Value::Text("品名".into()), fmt: Default::default() });
            s.set(Pos { row: 0, col: 1 }, Cell {
                formula: None, value: Value::Text("金額".into()), fmt: Default::default() });
            r = 1;
        }
        for (name, n) in rows {
            s.set(Pos { row: r, col: 0 }, Cell {
                formula: None, value: Value::Text((*name).into()), fmt: Default::default() });
            s.set(Pos { row: r, col: 1 }, Cell {
                formula: None, value: Value::Number(*n), fmt: Default::default() });
            r += 1;
        }
        s
    }

    fn col0(s: &Sheet, r: u32) -> String {
        s.get(Pos { row: r, col: 0 }).map(|c| c.value.display()).unwrap_or_default()
    }

    #[test]
    fn can_sort_numerically() {
        let mut s = table(&[("丙", 300.0), ("甲", 100.0), ("乙", 200.0)], false);
        s.sort_by_column(1, true, false);
        assert_eq!(col0(&s, 0), "甲");
        assert_eq!(col0(&s, 2), "丙");
    }

    #[test]
    fn the_header_row_stays() {
        // 帳票の並べ替えで見出しが混ざるのは事故
        let mut s = table(&[("丙", 300.0), ("甲", 100.0)], true);
        s.sort_by_column(1, true, true);
        assert_eq!(col0(&s, 0), "品名", "見出しが並べ替えに巻き込まれた");
        assert_eq!(col0(&s, 1), "甲");
    }

    #[test]
    fn a_whole_row_moves() {
        // 選んだ列だけ動かすと、隣の列との対応が壊れて静かに嘘の表になる
        let mut s = table(&[("丙", 300.0), ("甲", 100.0)], false);
        s.sort_by_column(1, true, false);
        let amount = |r: u32| s.get(Pos { row: r, col: 1 }).map(|c| c.value.clone());
        assert_eq!(col0(&s, 0), "甲");
        assert_eq!(amount(0), Some(Value::Number(100.0)), "名前と金額の対応が壊れた");
    }

    #[test]
    fn can_sort_descending_too() {
        let mut s = table(&[("甲", 100.0), ("丙", 300.0)], false);
        s.sort_by_column(1, false, false);
        assert_eq!(col0(&s, 0), "丙");
    }

    #[test]
    fn blanks_sort_last() {
        let mut s = table(&[("甲", 100.0)], false);
        s.set(Pos { row: 1, col: 0 }, Cell {
            formula: None, value: Value::Text("空欄".into()), fmt: Default::default() });
        s.sort_by_column(1, true, false);
        assert_eq!(col0(&s, 0), "甲", "空が先に来た");
    }

    #[test]
    fn data_bar_and_colour_scale_use_their_scale() {
        use crate::{CondKind, CondRule};
        let mut s = Sheet::new("試");
        for (i, v) in ["10", "20", "30"].iter().enumerate() {
            s.set(Pos::new(i as u32, 0), Cell::input(v));
        }
        let rule = CondRule {
            range: (Pos::new(0, 0), Pos::new(2, 0)),
            kind: CondKind::Bar("638EC6".into()),
            look: CondLook::default(),
        };
        let aux = rule.aux(&s);
        assert_eq!(aux.min, 10.0);
        assert_eq!(aux.max, 30.0);
        let t = rule.scalar(Pos::new(1, 0), &Value::Number(20.0), &aux).unwrap();
        assert!((t - 0.5).abs() < 1e-9, "真ん中が 0.5 でない: {t}");
        // 範囲の外は None
        assert!(rule.scalar(Pos::new(9, 9), &Value::Number(20.0), &aux).is_none());
        // スケールの色: 両端は端の色、真ん中は中間色
        let sc = CondRule {
            range: (Pos::new(0, 0), Pos::new(2, 0)),
            kind: CondKind::Scale("FF0000".into(), Some("FFFF00".into()), "00FF00".into()),
            look: CondLook::default(),
        };
        assert_eq!(sc.scale_color(0.0).unwrap(), "FF0000");
        assert_eq!(sc.scale_color(0.5).unwrap(), "FFFF00");
        assert_eq!(sc.scale_color(1.0).unwrap(), "00FF00");
    }

    #[test]
    fn coloured_rows_can_be_gathered_on_top() {
        let mut s = table(&[("甲", 100.0), ("乙", 200.0), ("丙", 300.0)], true);
        // 「丙」の行(row 3)のキー列に塗り
        let p = Pos { row: 3, col: 0 };
        let mut c = s.cells.get(&p).cloned().unwrap();
        c.fmt.fill = Some("FFFF00".into());
        s.cells.insert(p, c);
        s.sort_color_top(0, true, "FFFF00", true);
        assert_eq!(col0(&s, 0), "品名", "見出しが動いた");
        assert_eq!(col0(&s, 1), "丙", "色の行が上に来ない");
        assert_eq!(col0(&s, 2), "甲", "残りの順が崩れた");
        assert_eq!(col0(&s, 3), "乙");
    }

    #[test]
    fn duplicate_rows_can_be_dropped() {
        let mut s = table(&[("甲", 100.0), ("甲", 100.0), ("乙", 200.0)], false);
        let n = s.remove_duplicate_rows(false);
        assert_eq!(n, 1, "落とした件数が違う");
        assert_eq!(col0(&s, 0), "甲");
        assert_eq!(col0(&s, 1), "乙");
        assert_eq!(col0(&s, 2), "", "詰まっていない");
    }

    #[test]
    fn the_header_row_is_not_counted_as_duplicate() {
        let mut s = table(&[("品名", 0.0)], true);
        assert_eq!(s.remove_duplicate_rows(true), 0);
        assert_eq!(col0(&s, 0), "品名");
    }

    #[test]
    fn an_empty_sheet_does_not_panic() {
        let mut s = Sheet { name: "空".into(), ..Default::default() };
        s.sort_by_column(0, true, true);
        assert_eq!(s.remove_duplicate_rows(true), 0);
    }
}

#[cfg(test)]
mod cellshift_tests {
    use super::*;

    fn s3() -> Sheet {
        let mut s = Sheet::new("表");
        s.set(Pos::parse("A1").unwrap(), Cell::input("1"));
        s.set(Pos::parse("A2").unwrap(), Cell::input("2"));
        s.set(Pos::parse("B1").unwrap(), Cell::input("=A2*10"));
        s
    }

    #[test]
    fn shifting_down_moves_refs_along() {
        let mut s = s3();
        // A1 の場所に1セル挿入(A列だけ下へ)
        s.insert_cells(Pos::parse("A1").unwrap(), Pos::parse("A1").unwrap(), false).unwrap();
        assert!(s.get(Pos::parse("A1").unwrap()).is_none(), "挿した場所が空でない");
        assert_eq!(s.value(Pos::parse("A2").unwrap()), Value::Number(1.0));
        assert_eq!(s.value(Pos::parse("A3").unwrap()), Value::Number(2.0));
        // B1 は動かないが、指していた A2 は A3 へ動いた
        assert_eq!(
            s.get(Pos::parse("B1").unwrap()).and_then(|c| c.formula.clone()).as_deref(),
            Some("A3*10"),
            "動いたセルへの参照が付いて動いていない"
        );
    }

    #[test]
    fn shifting_right_moves_only_that_row() {
        let mut s = s3();
        s.insert_cells(Pos::parse("A1").unwrap(), Pos::parse("A1").unwrap(), true).unwrap();
        assert_eq!(s.value(Pos::parse("B1").unwrap()), Value::Number(1.0), "右へ動いていない");
        // 2行目は帯の外。動かない
        assert_eq!(s.value(Pos::parse("A2").unwrap()), Value::Number(2.0));
        // 元の B1 の式は C1 へ動き、A2 への参照はそのまま
        assert_eq!(
            s.get(Pos::parse("C1").unwrap()).and_then(|c| c.formula.clone()).as_deref(),
            Some("A2*10")
        );
    }

    #[test]
    fn shifting_up_turns_lost_refs_into_ref_error() {
        let mut s = s3();
        // A1 を削除して上へ詰める → A2(=1)ではなく元A1が消え、A2の中身が A1 へ
        s.delete_cells(Pos::parse("A1").unwrap(), Pos::parse("A1").unwrap(), false).unwrap();
        assert_eq!(s.value(Pos::parse("A1").unwrap()), Value::Number(2.0), "詰まっていない");
        // B1 が指していた A2 は A1 へ動いた
        assert_eq!(
            s.get(Pos::parse("B1").unwrap()).and_then(|c| c.formula.clone()).as_deref(),
            Some("A1*10")
        );
        // こんどは参照先そのものを消す
        let mut s2 = s3();
        s2.delete_cells(Pos::parse("A2").unwrap(), Pos::parse("A2").unwrap(), false).unwrap();
        assert_eq!(
            s2.get(Pos::parse("B1").unwrap()).and_then(|c| c.formula.clone()).as_deref(),
            Some("#REF!*10"),
            "消えた参照が黙って別のセルを指した"
        );
    }

    #[test]
    fn a_merge_across_the_band_is_refused() {
        let mut s = s3();
        s.merges.push((Pos::parse("A1").unwrap(), Pos::parse("B1").unwrap()));
        let r = s.insert_cells(Pos::parse("A1").unwrap(), Pos::parse("A1").unwrap(), false);
        assert!(r.is_err(), "結合をまたぐシフトを黙って通した");
    }
}

#[cfg(test)]
mod offset_tests {
    use super::*;

    #[test]
    fn relative_refs_all_shift() {
        assert_eq!(offset_refs("=A1+B2", 1, 0), "=A2+B3");
        assert_eq!(offset_refs("=SUM(A1:A3)", 2, 0), "=SUM(A3:A5)");
    }

    #[test]
    fn the_locked_side_stays_put() {
        assert_eq!(offset_refs("=$A$1+A1", 1, 1), "=$A$1+B2");
        assert_eq!(offset_refs("=A$1", 3, 0), "=A$1", "行を固定したのに動いた");
        assert_eq!(offset_refs("=$A1", 0, 3), "=$A1", "列を固定したのに動いた");
    }

    #[test]
    fn off_the_sheet_becomes_a_ref_error() {
        assert_eq!(offset_refs("=A1", -1, 0), "=#REF!");
    }

    #[test]
    fn strings_and_function_names_are_left_alone() {
        assert_eq!(offset_refs(r#"="A1"&A1"#, 1, 0), r#"="A1"&A2"#);
        assert_eq!(offset_refs("=SUM(A1)", 1, 0), "=SUM(A2)");
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn a_literal_option_list_splits() {
        let v = Validation::list(
            (Pos::new(1, 1), Pos::new(9, 1)),
            r#""甲, 乙,丙""#.into(),
        );
        let s = Sheet::default();
        assert_eq!(v.options(&s), vec!["甲", "乙", "丙"], "空白ごと候補にした");
        assert!(v.contains(Pos::new(5, 1)));
        assert!(!v.contains(Pos::new(5, 2)));
    }

    #[test]
    fn range_options_come_from_the_sheet_values() {
        let mut s = Sheet::default();
        for (r, t) in [(1, "東京"), (2, "大阪"), (3, "東京"), (4, "")] {
            s.set(Pos::new(r, 3), Cell::input(t));
        }
        let v = Validation::list(
            (Pos::new(0, 0), Pos::new(0, 0)),
            "$D$2:$D$5".into(),
        );
        assert_eq!(v.options(&s), vec!["東京", "大阪"], "重複と空欄が候補に入った");
        // 解決できない参照は空(制限なしと扱う側の約束)
        let alien = Validation::list(
            (Pos::new(0, 0), Pos::new(0, 0)),
            "Sheet2!$A$1:$A$3".into(),
        );
        assert!(alien.options(&s).is_empty());
    }

    #[test]
    fn header_group_splits_and_joins() {
        let (l, c, r) = hf_split("&L左&C中&R右");
        assert_eq!((l.as_str(), c.as_str(), r.as_str()), ("左", "中", "右"));
        // 印なしは中(xlsx の慣わし)
        assert_eq!(hf_split("題").1, "題");
        assert_eq!(hf_join("", "月次", "&P / &N"), "&C月次&R&P / &N");
        assert_eq!(hf_join("", "", ""), "");
    }

    #[test]
    fn position_based_rules_can_be_looked_up() {
        let mut s = Sheet::default();
        s.validations.push(Validation::list(
            (Pos::new(1, 1), Pos::new(3, 1)),
            r#""a,b""#.into(),
        ));
        assert!(s.validation_at(Pos::new(2, 1)).is_some());
        assert!(s.validation_at(Pos::new(2, 2)).is_none());
    }

    #[test]
    fn an_unreadable_number_rule_blocks_nothing() {
        // 式がセル参照の整数規則 — 判定できないので、文字を打っても止めない
        // (読めない規則で入力を止めない方針。実物の xlsx にはよくある形)
        let s = Sheet::default();
        let mut v = Validation::list((Pos::new(0, 0), Pos::new(0, 0)), "$D$1".into());
        v.kind = "whole".into();
        v.op = "greaterThan".into();
        assert!(v.passes(&s, "abc"), "判定できない規則が文字を堰き止めた");
        assert!(v.passes(&s, "5"));
        // 式が数なら判定できる — 文字はちゃんと止める
        v.formula = "0".into();
        assert!(!v.passes(&s, "abc"));
        assert!(v.passes(&s, "5"));
        assert!(!v.passes(&s, "-1"));
    }
}


#[cfg(test)]
mod shape_tests {
    use super::*;

    #[test]
    fn shape_svg_carries_size_and_colour() {
        let sh = SheetShape {
            at: Pos::new(0, 0),
            width_px: 200.0,
            height_px: 100.0,
            kind: "ellipse".into(),
            fill: Some("FFF2CC".into()),
            line: Some("1B6E3C".into()),
            ..Default::default()
        };
        let svg = sh.to_svg();
        assert!(svg.contains(r#"width="200""#), "{svg}");
        assert!(svg.contains("#FFF2CC") && svg.contains("#1B6E3C"));
        assert!(svg.contains("<ellipse"));
        // 知らない種類は四角で描く(黙って消さない)。
        // **例に使う名前は「まだ描けない物」でなければならない** —
        // hexagon はここの例だったが 2026-08-13 に描けるようになった
        let unknown = SheetShape { kind: "cube".into(), ..sh };
        assert!(!can_draw("cube"), "例に使った形が描けるようになっている");
        assert!(unknown.to_svg().contains("<rect"));
    }
}

#[cfg(test)]
mod valign_tests {
    use super::*;

    #[test]
    fn vertical_distributed_align_round_trips() {
        // **`_ => Bottom` に落ちていた。** 日銀の統計表が使っており、
        // 畳むと保存で消える(2026-08-10)
        assert_eq!(VAlign::from_xlsx("distributed"), VAlign::Distribute);
        assert_eq!(VAlign::Distribute.as_xlsx(), Some("distributed"));
        // 既定(下)は書かない — 触っていない帳票に差分を出さないため
        assert_eq!(VAlign::from_xlsx("bottom"), VAlign::Bottom);
        assert_eq!(VAlign::Bottom.as_xlsx(), None);
    }

    #[test]
    fn an_unknown_vertical_align_falls_to_bottom() {
        // `justify` は実物 31 枚に出ないので変種を作っていない。
        // **出てから足す** — その判断ごと押さえておく
        assert_eq!(VAlign::from_xlsx("justify"), VAlign::Bottom);
        assert_eq!(VAlign::from_xlsx("なにか"), VAlign::Bottom);
    }
}

#[cfg(test)]
mod input_fmt_tests {
    use super::*;

    #[test]
    fn formulas_returning_dates_suggest_a_date_format() {
        // **値は合っていても、形式が無いと画面に通し番号が出る。**
        // =DATE(2026,8,10) は 46244(2026-08-10 に ironcalc と突き合わせて判明)
        for f in ["=DATE(2026,8,10)", "=TODAY()", "=EOMONTH(A1,0)", "=today()"] {
            assert_eq!(Cell::date_format_of(f), Some("yyyy/m/d"), "{f}");
        }
        assert_eq!(Cell::date_format_of("=NOW()"), Some("yyyy/m/d h:mm"));
        assert_eq!(Cell::date_format_of("=TIME(9,30,0)"), Some("h:mm"));
    }

    #[test]
    fn no_date_format_for_functions_returning_numbers() {
        // **年 2026 に日付の形式を付けると 1905年7月18日 になる。**
        // 日付の関数だからと一括りにしない
        for f in ["=YEAR(TODAY())", "=MONTH(A1)", "=DAY(A1)", "=WEEKDAY(A1)", "=DATEDIF(A1,A2,\"D\")"] {
            assert_eq!(Cell::date_format_of(f), None, "{f}");
        }
        // 括弧が続かないなら関数ではない
        assert_eq!(Cell::date_format_of("=DATE"), None);
        assert_eq!(Cell::date_format_of("=A1+1"), None);
        assert_eq!(Cell::date_format_of("普通の字"), None);
    }
}

#[cfg(test)]
mod bracket_tests {
    use super::*;

    fn f(n: f64, code: &str) -> String {
        format_value(&Value::Number(n), Some(code), false)
    }

    /// **角かっこを画面に出さない。** 前は読み飛ばしておらず、
    /// `[Red]46,240` や `[$-446240` がそのまま出ていた
    /// (2026-08-10。実物26枚のうち4枚がこの形の書式を持っていた)
    #[test]
    fn square_brackets_do_not_show() {
        for (n, code) in [
            (1234.0, "[Red]#,##0"),
            (1234.0, "[赤]#,##0"),
            (46240.0, "[$-409]mmmm yyyy"),
            (46240.0, "[$-411]yyyy/m/d"),
        ] {
            let got = f(n, code);
            assert!(!got.contains('['), "{code} → {got}");
            assert!(!got.contains(']'), "{code} → {got}");
        }
    }

    /// **Excel は通貨記号を引用符で書く。** `"¥"#,##0` の `"` で切っていて、
    /// 円記号を丸ごと落としていた — 実物の会計書式がこの形
    #[test]
    fn quoted_currency_signs_are_kept() {
        assert_eq!(f(1234.0, r##""¥"#,##0"##), "¥1,234");
        // 角かっこの中に記号を書く形(こちらも Excel の綴り)
        assert_eq!(f(1234.0, "[$¥-411]#,##0"), "¥1,234");
        assert_eq!(f(1234.0, "[$€-407]#,##0"), "€1,234");
        // 実物26枚に入っている会計書式まるごと
        assert_eq!(f(1234.0, r##""¥"#,##0_);[Red]\("¥"#,##0\)"##), "¥1,234");
    }

    /// 日付の書式は**最初の節だけ**。`;@` が画面に出ていた
    #[test]
    fn the_text_section_is_not_shown() {
        assert_eq!(f(46240.0, "yyyy/m/d;@"), "2026/8/6");
        assert!(!f(46240.0, "[$-409]mmmm\\ yyyy;@").contains(';'));
    }

    /// `\` は次の1字を字として出す逃げ。読まないと `\` が画面に出る
    #[test]
    fn escape_characters_do_not_show() {
        assert_eq!(f(46240.0, "yyyy\\年m\\月"), "2026年8月");
        assert!(!f(46240.0, "[$-409]mmmm\\ yyyy").contains('\\'));
    }

    /// **経過時間は 24 時で巻き戻さない。** 勤怠表の合計がこの書式で、
    /// 前は `[h]:mm` が `[0]:00` になっていた
    #[test]
    fn elapsed_time_does_not_wrap() {
        // 1.0625 日 = 25 時間 30 分
        assert_eq!(f(1.0625, "[h]:mm"), "25:30");
        assert_eq!(f(1.0625, "[mm]"), "1530");
        assert_eq!(f(0.5, "[h]:mm"), "12:00");
        // 札が無ければ普段どおり巻き戻す
        assert_eq!(f(1.0625, "h:mm"), "1:30");
    }

    /// 直したことで**普通の書式が壊れていない**こと
    #[test]
    fn a_plain_format_does_not_change() {
        assert_eq!(f(46240.0, "yyyy/m/d"), "2026/8/6");
        assert_eq!(f(1234.5, "#,##0.0"), "1,234.5");
        assert_eq!(f(1234.0, "¥#,##0"), "¥1,234");
        assert_eq!(f(0.5, "h:mm"), "12:00");
        assert_eq!(f(46240.0, "ggge\"年\"m\"月\"d\"日\""), "令和8年8月6日");
        assert_eq!(f(0.1234, "0.00%"), "12.34%");
    }

    /// **範囲の移動**(2026-08-13)。切り貼りの作法 —
    /// 外から指す式は付いて動き、中の式はそのまま(translate で動く)
    #[test]
    fn moving_carries_formulas_that_point_in() {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::new(0, 0), Cell::input("10"));            // A1 = 10
        s.set(Pos::new(0, 1), Cell::input("=A1*2"));         // B1 = A1*2
        s.set(Pos::new(0, 3), Cell::input("=B1+1"));         // D1 = B1+1
        let n = s.move_range(Pos::new(0, 1), Pos::new(0, 1), 5, 0, false);
        assert_eq!(n, 1, "動いたセルの数");
        // 中の式はそのまま(A1 を指し続ける)
        assert_eq!(
            s.get(Pos::new(5, 1)).unwrap().formula.as_deref(),
            Some("A1*2"),
            "中の式が勝手に動いた"
        );
        // 外の式は追随する(B1 → B6)
        assert_eq!(
            s.get(Pos::new(0, 3)).unwrap().formula.as_deref(),
            Some("B6+1"),
            "外から指す式が付いて動いていない"
        );
        assert!(s.get(Pos::new(0, 1)).map(|c| c.value.is_empty()).unwrap_or(true),
                "元の場所が空になっていない");
    }

    #[test]
    fn translate_shifts_relative_refs_inside() {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::new(0, 0), Cell::input("10"));
        s.set(Pos::new(0, 1), Cell::input("=A1*2"));
        s.move_range(Pos::new(0, 1), Pos::new(0, 1), 5, 0, true);
        assert_eq!(
            s.get(Pos::new(5, 1)).unwrap().formula.as_deref(),
            Some("A6*2"),
            "translate で中の参照がずれていない(openpyxl と同じ定義)"
        );
    }

    #[test]
    fn a_move_overwrites_the_target_and_stays_on_the_sheet() {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::new(0, 0), Cell::input("元"));
        s.set(Pos::new(2, 0), Cell::input("先"));
        s.move_range(Pos::new(0, 0), Pos::new(0, 0), 2, 0, false);
        assert_eq!(s.get(Pos::new(2, 0)).unwrap().value.display(), "元", "上書きしていない");
        // 負の座標へは動かさない(黙って端に寄せない)
        let before = s.cells.len();
        assert_eq!(s.move_range(Pos::new(0, 0), Pos::new(0, 0), -5, 0, false), 0);
        assert_eq!(s.cells.len(), before, "紙の外へ動かした");
    }

}

#[cfg(test)]
mod month_name_tests {
    use super::*;

    /// 2026-08-06(木)
    fn f(code: &str) -> String {
        format_value(&Value::Number(46240.0), Some(code), false)
    }

    /// **月名・曜日名は書式コードの地域で決まる。** 読む人の言語ではない —
    /// `[$-407]` の入ったセルは日本語で開いても独語で出る
    /// (「その帳票が独語で作られた」が残るだけ)
    #[test]
    fn locale_gives_month_and_weekday_names() {
        assert_eq!(f("[$-409]mmmm d, yyyy"), "August 6, 2026");
        assert_eq!(f("[$-407]dddd, d. mmmm yyyy"), "Donnerstag, 6. August 2026");
        assert_eq!(f("[$-40c]dddd d mmmm yyyy"), "jeudi 6 août 2026");
        assert_eq!(f("[$-412]yyyy\"년\" m\"월\" d\"일\" dddd"), "2026년 8월 6일 목요일");
    }

    /// **属格を落とさない。** 露語は「8月」と「8月の」で形が違い、
    /// 日と並ぶときは属格。Август ではなく августа
    #[test]
    fn russian_uses_the_genitive() {
        assert_eq!(f("[$-419]d mmmm yyyy \"г.\""), "6 августа 2026 г.");
        // 日が無ければ主格
        assert_eq!(f("[$-419]mmmm"), "Август");
    }

    /// 短縮と頭文字。mmm=短縮 / mmmm=完全 / mmmmm=頭文字1つ
    #[test]
    fn long_and_short_month_names_are_used_apart() {
        assert_eq!(f("[$-409]mmm d"), "Aug 6");
        assert_eq!(f("[$-409]mmmmm"), "A");
        // **2つまでは数**(名前ではない)。`mm` 単独は月とも分とも取れる
        // 曖昧な形なので日付と見なさない — ここは前からの割り切り
        assert_eq!(f("[$-409]mm/d"), "08/6");
    }

    /// **地域指定が無ければ日本語。** 実物では月名を使う書式は必ず
    /// 指定を持っていた(26枚で2件、いずれも指定つき)ので、
    /// 指定なしは実質こちらで作った書式。素の言語でよい
    #[test]
    fn with_no_locale_it_is_japanese() {
        assert_eq!(f("aaaa"), "木曜日");
        assert_eq!(f("aaa"), "木");
        assert_eq!(f("mmmm"), "8月");
        assert_eq!(f("yyyy\"年\"m\"月\"d\"日\""), "2026年8月6日");
    }

    /// 知らない地域は日本語に落ちる。**近い言語へ勝手に寄せない**
    #[test]
    fn an_unknown_locale_is_not_forced() {
        assert_eq!(f("[$-4ff]mmmm"), "8月");
    }

    /// 月名を入れたことで**数と時刻が壊れていない**こと
    #[test]
    fn numbers_and_times_do_not_change() {
        assert_eq!(f("yyyy/m/d"), "2026/8/6");
        assert_eq!(f("ggge\"年\"m\"月\"d\"日\""), "令和8年8月6日");
        assert_eq!(format_value(&Value::Number(0.5), Some("h:mm"), false), "12:00");
        assert_eq!(format_value(&Value::Number(0.5), Some("h:mm:ss"), false), "12:00:00");
        assert_eq!(format_value(&Value::Number(1234.5), Some("#,##0.0"), false), "1,234.5");
    }
}

/// 図形ギャラリー(台帳 第2便の [中])。**形が形に見えるか**を縛る。
#[cfg(test)]
mod preset_shape_tests {
    use crate::{can_draw, preset_svg, SheetShape};
    use crate::Pos;

    /// 本家の分類に並ぶ、いま描ける形の全部
    const KINDS: &[&str] = &[
        "triangle", "rtTriangle", "parallelogram", "trapezoid", "pentagon", "hexagon",
        "octagon", "plus", "stadium",
        "star4", "star5", "star6", "star8",
        "leftArrow", "upArrow", "downArrow", "leftRightArrow", "upDownArrow",
        "mathPlus", "mathMinus", "mathEqual", "mathNotEqual", "mathMultiply",
        "flowChartProcess", "flowChartDecision", "flowChartInputOutput",
        "flowChartConnector", "flowChartTerminator", "flowChartDocument",
        "wedgeRectCallout", "wedgeEllipseCallout",
    ];

    fn shape(kind: &str) -> SheetShape {
        SheetShape {
            at: Pos::new(0, 0),
            width_px: 120.0,
            height_px: 70.0,
            kind: kind.into(),
            fill: Some("DCE6F1".into()),
            line: Some("1B6E3C".into()),
            ..Default::default()
        }
    }

    #[test]
    fn no_added_shape_falls_back_to_a_rectangle() {
        for k in KINDS {
            assert!(can_draw(k), "{k} が描けない形のまま");
            assert!(preset_svg(k, 0.0, 0.0, 100.0, 60.0, "").is_some(), "{k} の作図が無い");
        }
    }

    #[test]
    fn existing_shapes_count_as_drawable() {
        // **判断は can_draw 1箇所**(描く側と数える側で表が割れない)
        for k in ["rect", "roundRect", "ellipse", "rightArrow", "diamond", "line",
                  "spark", "spark-col", "spark-wl", "ink", "marker"] {
            assert!(can_draw(k), "{k} を描けない形に数えている");
        }
    }

    #[test]
    fn an_unknown_shape_answers_not_drawable() {
        // 数えられて Report に載る側。**黙って四角にしない**ための入り口
        for k in ["cube", "can", "heart", "ribbon", "actionButtonHome", ""] {
            assert!(!can_draw(k), "{k} を描けることにしている");
        }
    }

    #[test]
    fn every_shape_becomes_valid_svg() {
        // 座標の式を間違えると NaN が出て `points="NaN,NaN"` になり、
        // 画面から figure が消える(絵は「出ない」としか言わない)
        for k in KINDS {
            let svg = shape(k).to_svg();
            let mut r = quick_xml::Reader::from_str(&svg);
            let mut buf = Vec::new();
            loop {
                match r.read_event_into(&mut buf) {
                    Ok(quick_xml::events::Event::Eof) => break,
                    Err(e) => panic!("{k} の SVG が壊れている: {e}\n{svg}"),
                    _ => {}
                }
                buf.clear();
            }
            assert!(!svg.contains("NaN"), "{k} の座標に NaN がある:\n{svg}");
            assert!(!svg.contains("inf"), "{k} の座標が無限大:\n{svg}");
        }
    }

    #[test]
    fn a_collapsed_size_does_not_panic() {
        // 幅も高さも 0 の図形(ドラッグの途中で来る)
        for k in KINDS {
            let mut sp = shape(k);
            sp.width_px = 0.0;
            sp.height_px = 0.0;
            let svg = sp.to_svg();
            assert!(!svg.contains("NaN"), "{k} が潰れた大きさで NaN:\n{svg}");
        }
    }
}

/// **`=` の後ろに空白があれば式にしない**(2026-08-19 発注者確定)。
///
/// `= 大` はセルの見出し(`cellmark`)で、`=SUM(A1)` は式。決めは
/// `kumihan::adoc::is_formula_cell` の1つだけを見る — 打ち込みと
/// `.adoc` の読み書きで**同じ字が同じ意味**になる
#[cfg(test)]
#[allow(non_snake_case)]
mod telling_formulas_from_text {
    use crate::{Cell, Value};

    #[test]
    fn an_equals_without_a_space_is_a_formula() {
        assert_eq!(Cell::input("=SUM(A1:A3)").formula.as_deref(), Some("SUM(A1:A3)"));
        assert_eq!(Cell::input("=A1*B1").formula.as_deref(), Some("A1*B1"));
        assert_eq!(Cell::input("=-1").formula.as_deref(), Some("-1"));
    }

    #[test]
    fn an_equals_with_a_space_is_text() {
        let c = Cell::input("= 大");
        assert_eq!(c.formula, None, "見出しを式にした");
        assert_eq!(c.value, Value::Text("= 大".into()));

        // 見出しの段(=== 小)も同じ
        assert_eq!(Cell::input("=== 小").formula, None);
        // Excel 風の `= 1+2` は式にならない(承知の上の割り切り)
        assert_eq!(Cell::input("= 1+2").formula, None);
    }

    /// `=` 1文字だけは式ではない(ただの字)
    #[test]
    fn a_lone_equals_is_text() {
        let c = Cell::input("=");
        assert_eq!(c.formula, None);
        assert_eq!(c.value, Value::Text("=".into()));
    }
}

