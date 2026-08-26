//! 式と再計算の試験。


use crate::model::{Pos, Sheet, Value};

use super::parse::*;
use super::run::*;

#[cfg(test)]
// **日本語の試験名は家の作法。** ラテン大文字(XMATCH・calcPr・NA)が
// 混じると non_snake_case が鳴るが、読みやすさを取る。製品のコードには許さない
#[allow(non_snake_case)]
mod basic {
    use super::*;
    use crate::model::Cell;

    fn s(pairs: &[(&str, &str)]) -> Sheet {
        let mut sh = Sheet::new("Sheet1");
        for (a1, input) in pairs {
            sh.set(Pos::parse(a1).unwrap(), Cell::input(input));
        }
        recalc(&mut sh);
        sh
    }
    fn v(sh: &Sheet, a1: &str) -> String {
        sh.value(Pos::parse(a1).unwrap()).display()
    }

    #[test]
    fn arithmetic_and_parentheses() {
        let sh = s(&[("A1", "=1+2*3"), ("A2", "=(1+2)*3"), ("A3", "=10/4"),
                     ("A4", "=2^10"), ("A5", "=-3+1")]);
        assert_eq!(v(&sh, "A1"), "7");
        assert_eq!(v(&sh, "A2"), "9");
        assert_eq!(v(&sh, "A3"), "2.5");
        assert_eq!(v(&sh, "A4"), "1024");
        assert_eq!(v(&sh, "A5"), "-2");
    }

    #[test]
    fn cell_references_and_chains_resolve() {
        // 定義の順序が逆でも解ける(依存を先に解く)
        let sh = s(&[("C1", "=B1*2"), ("B1", "=A1+10"), ("A1", "5")]);
        assert_eq!(v(&sh, "B1"), "15");
        assert_eq!(v(&sh, "C1"), "30");
    }

    #[test]
    fn ranges_and_functions() {
        let sh = s(&[("A1", "10"), ("A2", "20"), ("A3", "30"), ("A4", "文字"),
                     ("B1", "=SUM(A1:A3)"), ("B2", "=AVERAGE(A1:A3)"),
                     ("B3", "=COUNT(A1:A4)"), ("B4", "=COUNTA(A1:A4)"),
                     ("B5", "=MAX(A1:A3)"), ("B6", "=MIN(A1:A3)")]);
        assert_eq!(v(&sh, "B1"), "60");
        assert_eq!(v(&sh, "B2"), "20");
        assert_eq!(v(&sh, "B3"), "3", "COUNT は数値だけ数える");
        assert_eq!(v(&sh, "B4"), "4", "COUNTA は空でないものを数える");
        assert_eq!(v(&sh, "B5"), "30");
        assert_eq!(v(&sh, "B6"), "10");
    }

    #[test]
    fn a_missed_lookup_can_be_caught_by_iferror() {
        // 実測で出た形: 見つからない VLOOKUP を IFERROR・IF で受ける
        let sh = s(&[
            ("A2", "りんご"), ("B2", "100"),
            ("A3", "みかん"), ("B3", "80"),
            ("C1", "=IFERROR(VLOOKUP(\"zzz\",A2:B3,2),\"\")"),
            ("C2", "=IFERROR(VLOOKUP(\"みかん\",A2:B3,2),\"\")"),
            ("C3", "=IF(ISBLANK(G4),\"\",VLOOKUP(\"zzz\",A2:B3,2))"),
        ]);
        assert_eq!(v(&sh, "C1"), "", "外れたら第2引数に落ちる");
        assert_eq!(v(&sh, "C2"), "80", "当たればそのまま");
        assert_eq!(v(&sh, "C3"), "", "使わない側のエラーを踏まない");
    }

    #[test]
    fn a_quotation_calculates() {
        // 事務で実際に使う形: 単価×数量、小計、消費税、合計
        let sh = s(&[
            ("A1", "ザボガードF F-02"), ("B1", "4"), ("C1", "125000"), ("D1", "=B1*C1"),
            ("A2", "エンブM"),          ("B2", "2"), ("C2", "98000"),  ("D2", "=B2*C2"),
            ("D3", "=SUM(D1:D2)"),
            ("D4", "=ROUND(D3*0.1,0)"),
            ("D5", "=D3+D4"),
        ]);
        assert_eq!(v(&sh, "D1"), "500000");
        assert_eq!(v(&sh, "D2"), "196000");
        assert_eq!(v(&sh, "D3"), "696000");
        assert_eq!(v(&sh, "D4"), "69600", "消費税");
        assert_eq!(v(&sh, "D5"), "765600", "税込合計");
    }

    #[test]
    fn conditions_and_strings() {
        let sh = s(&[("A1", "12"), ("B1", "=IF(A1>10,\"超過\",\"適正\")"),
                     ("B2", "=IF(A1>100,\"超過\",\"適正\")"),
                     ("B3", "=\"H\"&A1&\"まで\""),
                     ("B4", "=CONCATENATE(\"合計\",A1,\"枚\")"),
                     ("B5", "=LEN(\"日本フネン\")")]);
        assert_eq!(v(&sh, "B1"), "超過");
        assert_eq!(v(&sh, "B2"), "適正");
        assert_eq!(v(&sh, "B3"), "H12まで");
        assert_eq!(v(&sh, "B4"), "合計12枚");
        assert_eq!(v(&sh, "B5"), "5", "日本語は文字数で数える");
    }

    #[test]
    fn division_by_zero_is_an_error() {
        let sh = s(&[("A1", "0"), ("B1", "=10/A1")]);
        assert_eq!(v(&sh, "B1"), "#DIV/0!", "黙って0を返さない");
    }

    #[test]
    fn text_mixed_into_arithmetic_gives_a_value_error() {
        // **0 として続けない。** 文字の混じった列の合計が「それらしい数」に
        // なるのが一番困る(2026-08-10 に ironcalc と突き合わせて判明 —
        // ="あ"+1 が 1 になっていた)
        for f in ["=\"あ\"+1", "=1+\"あ\"", "=\"あ\"*2", "=-\"あ\"", "=2^\"あ\""] {
            let sh = s(&[("A1", f)]);
            assert_eq!(v(&sh, "A1"), "#VALUE!", "{f}");
        }
    }

    #[test]
    fn a_type_error_beats_a_division_by_zero() {
        // Excel も ="あ"/0 は #VALUE!。#DIV/0! ではない
        let sh = s(&[("A1", "=\"あ\"/0"), ("A2", "=5/0")]);
        assert_eq!(v(&sh, "A1"), "#VALUE!");
        assert_eq!(v(&sh, "A2"), "#DIV/0!");
    }

    #[test]
    fn numeric_text_and_booleans_read_as_numbers() {
        // ="5"+1 は 6(Excel も同じ)。真偽は 1/0。& は連結なので数にしない
        let sh = s(&[("A1", "=\"5\"+1"), ("A2", "=TRUE+1"), ("A3", "=\"あ\"&1")]);
        assert_eq!(v(&sh, "A1"), "6");
        assert_eq!(v(&sh, "A2"), "2");
        assert_eq!(v(&sh, "A3"), "あ1");
    }

    #[test]
    fn aggregation_skips_text_unlike_plain_arithmetic() {
        // **混ぜてはいけない2つの数の取り方。** SUM は飛ばし、四則は断る
        let sh = s(&[("A1", "=SUM(1,\"あ\",2)")]);
        assert_eq!(v(&sh, "A1"), "3");
    }


    #[test]
    fn iterative_calculation_converges_a_cycle() {
        // A1 = A1/2 + 1 の不動点は 2。反復なしなら #CIRC!
        let mut b = crate::Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("=A1/2+1"));
        recalc_book(&mut b, 0);
        assert_eq!(
            b.sheets[0].value(Pos::parse("A1").unwrap()).display(),
            "#CIRC!",
            "反復なしで循環が通った"
        );
        b.calc_iter = Some((100, 1e-9));
        recalc_book(&mut b, 0);
        let got = b.sheets[0].value(Pos::parse("A1").unwrap()).as_number();
        assert!((got - 2.0).abs() < 1e-6, "不動点に収束しない: {got}");
        // 相互参照(A2=B2+1, B2=A2 は発散 — 上限で止まりエラーにならない)
        b.sheets[0].set(Pos::parse("A2").unwrap(), Cell::input("=B2+1"));
        b.sheets[0].set(Pos::parse("B2").unwrap(), Cell::input("=A2"));
        recalc_book(&mut b, 0);
        let a2 = b.sheets[0].value(Pos::parse("A2").unwrap());
        assert!(matches!(a2, Value::Number(_)), "上限で止まらずエラー: {a2:?}");
    }

    #[test]
    fn circular_references_are_detected() {
        let sh = s(&[("A1", "=B1+1"), ("B1", "=A1+1")]);
        assert!(v(&sh, "A1").contains("CIRC") || v(&sh, "B1").contains("CIRC"),
            "循環を検出していない: A1={} B1={}", v(&sh, "A1"), v(&sh, "B1"));
    }

    #[test]
    fn an_unknown_function_is_a_name_error() {
        // XLOOKUP も実装済みになった(2026-08-05)ので、本当に無い名前で確かめる
        let sh = s(&[("A1", "=NAINAMAE(1,B1:C9,2)")]);
        assert_eq!(v(&sh, "A1"), "#NAME?", "できないものはできないと言う");
    }

    #[test]
    fn a_broken_formula_does_not_panic() {
        for f in ["=1+", "=(1+2", "=SUM(", "=@#$", "=A1+"] {
            let sh = s(&[("A1", "1"), ("Z9", f)]);
            let got = v(&sh, "Z9");
            assert!(got.starts_with('#'), "{f} → {got}(エラー値になっていない)");
        }
    }
}

#[cfg(test)]
mod more_fn_tests {
    use crate::model::{Cell, Pos, Sheet, Value};

    fn eval(formula: &str, data: &[(&str, f64)]) -> Value {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for (a1, n) in data {
            s.set(Pos::parse(a1).unwrap(), Cell {
                formula: None, value: Value::Number(*n), fmt: Default::default() });
        }
        let out = Pos::parse("Z1").unwrap();
        // 式は = を外して持つ約束(Cell::input と同じ形にする)
        s.set(out, Cell::input(formula));
        crate::recalc(&mut s);
        s.get(out).unwrap().value.clone()
    }

    fn n(formula: &str) -> f64 {
        match eval(formula, &[]) {
            Value::Number(x) => x,
            v => panic!("数でない: {v:?}"),
        }
    }

    #[test]
    fn rounding_down_and_up() {
        assert!((n("=ROUNDDOWN(3.567,2)") - 3.56).abs() < 1e-9);
        assert!((n("=ROUNDUP(3.501,1)") - 3.6).abs() < 1e-9);
        // 負の数で符号が入れ替わらない
        assert!((n("=ROUNDUP(-3.501,1)") + 3.6).abs() < 1e-9);
        assert!((n("=ROUNDDOWN(-3.567,2)") + 3.56).abs() < 1e-9);
    }

    #[test]
    fn the_modulus_cannot_divide_by_zero() {
        // 黙って0を返すと、集計が静かに狂う
        assert_eq!(eval("=MOD(10,0)", &[]), Value::Error("#DIV/0!".into()));
        assert!((n("=MOD(10,3)") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_square_root_of_a_negative_is_an_error() {
        assert_eq!(eval("=SQRT(-1)", &[]), Value::Error("#NUM!".into()));
        assert!((n("=SQRT(9)") - 3.0).abs() < 1e-9);
    }

    #[test]
    fn conditional_sums() {
        let d = [("A1", 100.0), ("A2", 200.0), ("A3", 50.0)];
        assert!((match eval("=SUMIF(A1:A3,\">80\")", &d) {
            Value::Number(x) => x, v => panic!("{v:?}") } - 300.0).abs() < 1e-9);
        assert!((match eval("=COUNTIF(A1:A3,\">80\")", &d) {
            Value::Number(x) => x, v => panic!("{v:?}") } - 2.0).abs() < 1e-9);
    }

    #[test]
    fn text_can_be_sliced() {
        // 日本語は文字数で数える(バイトではない)
        assert_eq!(eval("=LEFT(\"日本フネン\",2)", &[]), Value::Text("日本".into()));
        assert_eq!(eval("=RIGHT(\"日本フネン\",3)", &[]), Value::Text("フネン".into()));
        // MID は1始まり
        assert_eq!(eval("=MID(\"日本フネン\",3,2)", &[]), Value::Text("フネ".into()));
    }

    #[test]
    fn blanks_and_errors_can_be_told_apart() {
        assert_eq!(eval("=ISBLANK(A9)", &[]), Value::Bool(true));
        assert_eq!(eval("=ISBLANK(A1)", &[("A1", 5.0)]), Value::Bool(false));
    }

    #[test]
    fn functions_that_take_an_error() {
        // IFERROR は第1引数のエラーを捕まえて第2引数に落ちる
        // (以前は引数の先行エラー弾きで #N/A が素通りしていた)
        assert_eq!(eval("=IFERROR(MOD(1,0),\"×\")", &[]), Value::Text("×".into()));
        assert_eq!(eval("=IFERROR(A1,\"×\")", &[("A1", 5.0)]), Value::Number(5.0));
        // ISERROR も同じ弾きで壊れていた(エラーを見て TRUE を返せなかった)
        assert_eq!(eval("=ISERROR(MOD(1,0))", &[]), Value::Bool(true));
        assert_eq!(eval("=ISERROR(1)", &[]), Value::Bool(false));
        // IF は選ばなかった側のエラーを踏まない。条件のエラーは伝える
        assert_eq!(eval("=IF(1,\"可\",MOD(1,0))", &[]), Value::Text("可".into()));
        assert_eq!(eval("=IF(0,MOD(1,0),\"否\")", &[]), Value::Text("否".into()));
        assert_eq!(eval("=IF(MOD(1,0),1,2)", &[]), Value::Error("#DIV/0!".into()));
        // 選んだ側がエラーならそのまま伝える
        assert_eq!(eval("=IF(1,MOD(1,0),\"否\")", &[]), Value::Error("#DIV/0!".into()));
    }

    #[test]
    fn products_and_powers() {
        assert!((n("=PRODUCT(2,3,4)") - 24.0).abs() < 1e-9);
        assert!((n("=POWER(2,10)") - 1024.0).abs() < 1e-9);
    }

    #[test]
    fn text_formatting() {
        assert_eq!(eval("=TRIM(\"  余白  \")", &[]), Value::Text("余白".into()));
        assert_eq!(eval("=UPPER(\"abc\")", &[]), Value::Text("ABC".into()));
    }
}

#[cfg(test)]
mod name_tests {
    use super::*;
    use crate::model::Cell;

    #[test]
    fn a_name_can_be_used_in_a_formula() {
        let mut s = Sheet::new("表");
        s.set(Pos::parse("A1").unwrap(), Cell::input("100"));
        s.set(Pos::parse("B1").unwrap(), Cell::input("=単価*2"));
        s.names.push(crate::model::DefinedName::new("単価", "A1"));
        recalc(&mut s);
        assert_eq!(s.value(Pos::parse("B1").unwrap()), Value::Number(200.0),
            "名前が参照に展開されない");
    }

    #[test]
    fn a_range_name_works_in_sum() {
        let mut s = Sheet::new("表");
        for (r, v) in [(0, "10"), (1, "20"), (2, "30")] {
            s.set(Pos::new(r, 0), Cell::input(v));
        }
        s.set(Pos::new(3, 0), Cell::input("=SUM(明細)"));
        s.names.push(crate::model::DefinedName::new("明細", "A1:A3"));
        recalc(&mut s);
        assert_eq!(s.value(Pos::new(3, 0)), Value::Number(60.0));
    }

    #[test]
    fn a_partial_name_match_does_not_substitute() {
        assert_eq!(expand_names("単価計*2", &[crate::model::DefinedName::new("単価", "A1")]),
            "単価計*2", "「単価計」の頭だけ置き換えた");
        assert_eq!(expand_names("\"単価\"&A1", &[crate::model::DefinedName::new("単価", "B9")]),
            "\"単価\"&A1", "文字列の中を置き換えた");
        // 長い名前が勝つ
        assert_eq!(expand_names("単価計", &[
            crate::model::DefinedName::new("単価", "A1"),
            crate::model::DefinedName::new("単価計", "B1")]), "B1");
    }
}

#[cfg(test)]
mod fn_ext_tests {
    use super::*;
    use crate::model::Cell;

    fn sheet_with(cells: &[(&str, &str)]) -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for (a1, v) in cells {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s
    }

    fn value_of(s: &mut Sheet, f: &str) -> Value {
        s.set(Pos::parse("Z99").unwrap(), Cell::input(f));
        recalc(s);
        s.value(Pos::parse("Z99").unwrap())
    }

    #[test]
    fn vlookup_looks_up_a_table() {
        let mut s = sheet_with(&[
            ("A1", "甲"), ("B1", "100"),
            ("A2", "乙"), ("B2", "200"),
            ("A3", "丙"), ("B3", "300"),
        ]);
        assert_eq!(value_of(&mut s, "=VLOOKUP(\"乙\",A1:B3,2)"), Value::Number(200.0));
        assert_eq!(
            value_of(&mut s, "=VLOOKUP(\"丁\",A1:B3,2)"),
            Value::Error("#N/A".into()),
            "無い鍵は正直に #N/A"
        );
    }

    #[test]
    fn index_and_match_work_as_a_pair() {
        let mut s = sheet_with(&[
            ("A1", "品"), ("B1", "数"),
            ("A2", "筆"), ("B2", "12"),
            ("A3", "紙"), ("B3", "34"),
        ]);
        assert_eq!(value_of(&mut s, "=MATCH(\"紙\",A1:A3,0)"), Value::Number(3.0));
        assert_eq!(value_of(&mut s, "=INDEX(A1:B3,3,2)"), Value::Number(34.0));
        assert_eq!(
            value_of(&mut s, "=INDEX(B1:B3,MATCH(\"筆\",A1:A3,0))"),
            Value::Number(12.0),
            "INDEX+MATCH の常套が動かない"
        );
    }

    #[test]
    fn the_date_serial_round_trips_with_the_calendar() {
        let mut s = sheet_with(&[]);
        // 2026-08-04 の通し番号(1899-12-30 起点)
        let serial = match value_of(&mut s, "=DATE(2026,8,4)") {
            Value::Number(n) => n,
            v => panic!("数でない: {v:?}"),
        };
        assert_eq!(value_of(&mut s, &format!("=YEAR({serial})")), Value::Number(2026.0));
        assert_eq!(value_of(&mut s, &format!("=MONTH({serial})")), Value::Number(8.0));
        assert_eq!(value_of(&mut s, &format!("=DAY({serial})")), Value::Number(4.0));
        // 2026-08-04 は火曜(Excel の既定: 日=1 → 火=3)
        assert_eq!(value_of(&mut s, &format!("=WEEKDAY({serial})")), Value::Number(3.0));
        // 既知の値: 1900-01-01 = 2
        assert_eq!(value_of(&mut s, "=DATE(1900,1,1)"), Value::Number(2.0));
    }

    #[test]
    fn financial_formulas_match_the_textbook_values() {
        let mut s = sheet_with(&[]);
        // 年利12%を月利1%、60回、100万円借入 → 月々の返済(教科書値 -22244.45…)
        let pmt = match value_of(&mut s, "=PMT(0.01,60,1000000)") {
            Value::Number(n) => n,
            v => panic!("数でない: {v:?}"),
        };
        assert!((pmt + 22244.45).abs() < 0.5, "PMT が教科書とずれる: {pmt}");
        // 利率0なら単純割り
        assert_eq!(value_of(&mut s, "=PMT(0,10,1000)"), Value::Number(-100.0));
        // FV: 毎月1万円・月利0.5%・12回
        let fv = match value_of(&mut s, "=FV(0.005,12,-10000)") {
            Value::Number(n) => n,
            v => panic!("数でない: {v:?}"),
        };
        assert!((fv - 123355.62).abs() < 1.0, "FV がずれる: {fv}");
    }
}

/// 第1段の拡充(2026-08-05)— 日常と帳票を閉じる約37個。
#[cfg(test)]
mod dan1_tests {
    use super::*;
    use crate::model::Cell;

    fn sheet_with(cells: &[(&str, &str)]) -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for (a1, v) in cells {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s
    }

    fn value_of(s: &mut Sheet, f: &str) -> Value {
        s.set(Pos::parse("Z99").unwrap(), Cell::input(f));
        recalc(s);
        s.value(Pos::parse("Z99").unwrap())
    }

    fn n(s: &mut Sheet, f: &str) -> f64 {
        match value_of(s, f) {
            Value::Number(x) => x,
            v => panic!("{f} が数でない: {v:?}"),
        }
    }

    fn t(s: &mut Sheet, f: &str) -> String {
        match value_of(s, f) {
            Value::Text(x) => x,
            v => panic!("{f} が文字でない: {v:?}"),
        }
    }

    #[test]
    fn aggregation_with_several_conditions() {
        // 台帳: 品名・区分・金額
        let mut s = sheet_with(&[
            ("A1", "筆"), ("B1", "文具"), ("C1", "100"),
            ("A2", "紙"), ("B2", "文具"), ("C2", "200"),
            ("A3", "机"), ("B3", "家具"), ("C3", "900"),
            ("A4", "筆"), ("B4", "文具"), ("C4", "150"),
        ]);
        assert_eq!(n(&mut s, "=SUMIFS(C1:C4,B1:B4,\"文具\",A1:A4,\"筆\")"), 250.0);
        assert_eq!(n(&mut s, "=COUNTIFS(B1:B4,\"文具\",C1:C4,\">120\")"), 2.0);
        assert_eq!(n(&mut s, "=AVERAGEIF(B1:B4,\"文具\",C1:C4)"), 150.0);
        assert_eq!(n(&mut s, "=AVERAGEIFS(C1:C4,B1:B4,\"文具\")"), 150.0);
        assert_eq!(n(&mut s, "=MINIFS(C1:C4,B1:B4,\"文具\")"), 100.0);
        assert_eq!(n(&mut s, "=MAXIFS(C1:C4,B1:B4,\"文具\")"), 200.0);
        // 1件も合わない MINIFS は 0(Excel の約束)、AVERAGEIF は #DIV/0!
        assert_eq!(n(&mut s, "=MINIFS(C1:C4,B1:B4,\"食品\")"), 0.0);
        assert_eq!(
            value_of(&mut s, "=AVERAGEIF(B1:B4,\"食品\",C1:C4)"),
            Value::Error("#DIV/0!".into())
        );
    }

    #[test]
    fn three_argument_sumif_can_use_a_separate_sum_range() {
        // =SUMIF(条件範囲, 条件, 合計範囲) — Excel で最も多い書き方。
        // 条件は B 列で見て、足すのは C 列
        let mut s = sheet_with(&[
            ("A1", "筆"), ("B1", "文具"), ("C1", "100"),
            ("A2", "紙"), ("B2", "文具"), ("C2", "200"),
            ("A3", "机"), ("B3", "家具"), ("C3", "900"),
        ]);
        assert_eq!(n(&mut s, "=SUMIF(B1:B3,\"文具\",C1:C3)"), 300.0);
        // 3つ目を省いたら、条件を見た範囲そのものを足す
        assert_eq!(n(&mut s, "=SUMIF(C1:C3,\">150\")"), 1100.0);
        // 1件も合わなければ 0(Excel の約束)
        assert_eq!(n(&mut s, "=SUMIF(B1:B3,\"食品\",C1:C3)"), 0.0);
        // 範囲の大きさが違えば黙って数を返さない
        assert_eq!(
            value_of(&mut s, "=SUMIF(B1:B3,\"文具\",C1:C2)"),
            Value::Error("#VALUE!".into()),
            "大きさ違いを黙って計算しない"
        );
    }

    #[test]
    fn sumproduct_multiplies_and_adds() {
        let mut s = sheet_with(&[
            ("A1", "4"), ("B1", "100"),
            ("A2", "2"), ("B2", "250"),
        ]);
        assert_eq!(n(&mut s, "=SUMPRODUCT(A1:A2,B1:B2)"), 900.0);
        assert_eq!(
            value_of(&mut s, "=SUMPRODUCT(A1:A2,B1:B1)"),
            Value::Error("#VALUE!".into()),
            "大きさ違いを黙って計算しない"
        );
    }

    #[test]
    fn ifs_switch_and_choose() {
        let mut s = sheet_with(&[("A1", "85")]);
        assert_eq!(
            t(&mut s, "=IFS(A1>=90,\"秀\",A1>=80,\"優\",TRUE,\"可\")"),
            "優"
        );
        assert_eq!(
            value_of(&mut s, "=IFS(A1>=90,\"秀\")"),
            Value::Error("#N/A".into()),
            "どれも真でないなら正直に #N/A"
        );
        // 選ばなかった枝のエラー(1/0)を踏まない
        assert_eq!(t(&mut s, "=IFS(TRUE,\"良\",TRUE,1/0)"), "良");
        assert_eq!(t(&mut s, "=SWITCH(2,1,\"甲\",2,\"乙\",\"他\")"), "乙");
        assert_eq!(t(&mut s, "=SWITCH(9,1,\"甲\",2,\"乙\",\"他\")"), "他");
        assert_eq!(t(&mut s, "=CHOOSE(2,\"松\",\"竹\",\"梅\")"), "竹");
        assert_eq!(
            value_of(&mut s, "=CHOOSE(9,\"松\",\"竹\")"),
            Value::Error("#VALUE!".into())
        );
    }

    #[test]
    fn xlookup_looks_up_by_exact_match() {
        let mut s = sheet_with(&[
            ("A1", "F-01"), ("B1", "防火戸"),
            ("A2", "F-02"), ("B2", "防火ダンパー"),
        ]);
        assert_eq!(t(&mut s, "=XLOOKUP(\"F-02\",A1:A2,B1:B2)"), "防火ダンパー");
        assert_eq!(
            value_of(&mut s, "=XLOOKUP(\"F-09\",A1:A2,B1:B2)"),
            Value::Error("#N/A".into())
        );
        assert_eq!(t(&mut s, "=XLOOKUP(\"F-09\",A1:A2,B1:B2,\"該当なし\")"), "該当なし");
    }

    #[test]
    fn date_arithmetic_follows_the_calendar() {
        let mut s = sheet_with(&[]);
        // 2026-08-05 から: 月末・翌月・月をまたぐ日の丸め
        assert_eq!(
            n(&mut s, "=EOMONTH(DATE(2026,8,5),0)"),
            n(&mut s, "=DATE(2026,8,31)")
        );
        assert_eq!(
            n(&mut s, "=EDATE(DATE(2026,8,5),1)"),
            n(&mut s, "=DATE(2026,9,5)")
        );
        // 1/31 の1ヶ月後は 2/28(在らぬ 2/31 を作らない)
        assert_eq!(
            n(&mut s, "=EDATE(DATE(2026,1,31),1)"),
            n(&mut s, "=DATE(2026,2,28)")
        );
        // 12月から年をまたぐ
        assert_eq!(
            n(&mut s, "=EOMONTH(DATE(2026,12,1),0)"),
            n(&mut s, "=DATE(2026,12,31)")
        );
        assert_eq!(n(&mut s, "=DATEDIF(DATE(2020,4,1),DATE(2026,8,5),\"Y\")"), 6.0);
        assert_eq!(n(&mut s, "=DATEDIF(DATE(2026,5,1),DATE(2026,8,5),\"M\")"), 3.0);
        assert_eq!(n(&mut s, "=DATEDIF(DATE(2026,8,1),DATE(2026,8,5),\"D\")"), 4.0);
        assert_eq!(
            n(&mut s, "=DATEVALUE(\"2026/8/5\")"),
            n(&mut s, "=DATE(2026,8,5)")
        );
        assert_eq!(
            n(&mut s, "=DATEVALUE(\"2026年8月5日\")"),
            n(&mut s, "=DATE(2026,8,5)")
        );
        // 時刻
        assert_eq!(n(&mut s, "=TIME(6,0,0)"), 0.25);
        assert_eq!(n(&mut s, "=HOUR(TIME(18,30,45))"), 18.0);
        assert_eq!(n(&mut s, "=MINUTE(TIME(18,30,45))"), 30.0);
        assert_eq!(n(&mut s, "=SECOND(TIME(18,30,45))"), 45.0);
    }

    #[test]
    fn working_day_arithmetic() {
        let mut s = sheet_with(&[]);
        // 2026-08-05 は水曜。3営業日後は月曜(8/10)
        assert_eq!(
            n(&mut s, "=WORKDAY(DATE(2026,8,5),3)"),
            n(&mut s, "=DATE(2026,8,10)")
        );
        // 休みを教えれば飛ばす(8/10 を祝日に)
        assert_eq!(
            n(&mut s, "=WORKDAY(DATE(2026,8,5),3,DATE(2026,8,10))"),
            n(&mut s, "=DATE(2026,8,11)")
        );
        // 8/3(月)〜8/9(日)の平日は5日
        assert_eq!(
            n(&mut s, "=NETWORKDAYS(DATE(2026,8,3),DATE(2026,8,9))"),
            5.0
        );
    }

    #[test]
    fn string_tools() {
        let mut s = sheet_with(&[]);
        assert_eq!(t(&mut s, "=SUBSTITUTE(\"防火戸の戸\",\"戸\",\"扉\")"), "防火扉の扉");
        assert_eq!(t(&mut s, "=SUBSTITUTE(\"防火戸の戸\",\"戸\",\"扉\",2)"), "防火戸の扉");
        assert_eq!(n(&mut s, "=FIND(\"戸\",\"防火戸の戸\")"), 3.0);
        assert_eq!(n(&mut s, "=FIND(\"戸\",\"防火戸の戸\",4)"), 5.0);
        assert_eq!(
            value_of(&mut s, "=FIND(\"X\",\"防火戸\")"),
            Value::Error("#VALUE!".into())
        );
        assert_eq!(n(&mut s, "=SEARCH(\"abc\",\"xxABCxx\")"), 3.0, "SEARCH は大小を見ない");
        assert_eq!(n(&mut s, "=VALUE(\"¥1,234\")"), 1234.0);
        assert_eq!(n(&mut s, "=VALUE(\"25%\")"), 0.25);
        assert_eq!(t(&mut s, "=TEXTJOIN(\"、\",TRUE,\"松\",\"\",\"竹\")"), "松、竹");
        assert_eq!(t(&mut s, "=TEXTJOIN(\"-\",FALSE,\"a\",\"\",\"b\")"), "a--b");
        assert_eq!(t(&mut s, "=REPT(\"は\",3)"), "ははは");
        assert_eq!(t(&mut s, "=CHAR(65)"), "A");
        assert_eq!(n(&mut s, "=CODE(\"A\")"), 65.0);
    }

    #[test]
    fn text_renders_through_a_number_format() {
        let mut s = sheet_with(&[]);
        assert_eq!(t(&mut s, "=TEXT(DATE(2026,8,5),\"yyyy/m/d\")"), "2026/8/5");
        assert_eq!(t(&mut s, "=TEXT(DATE(2026,8,5),\"yyyy年m月d日\")"), "2026年8月5日");
        // 2026-08-05 は水曜
        assert_eq!(t(&mut s, "=TEXT(DATE(2026,8,5),\"aaa\")"), "水");
        assert_eq!(t(&mut s, "=TEXT(DATE(2026,8,5),\"aaaa\")"), "水曜日");
        assert_eq!(t(&mut s, "=TEXT(TIME(9,5,0),\"h:mm\")"), "9:05");
        assert_eq!(t(&mut s, "=TEXT(1234567,\"#,##0\")"), "1,234,567", "数の形式も同じ道");
        assert_eq!(t(&mut s, "=TEXT(0.25,\"0%\")"), "25%");
    }

    #[test]
    fn functions_that_answer_a_position() {
        let mut s = sheet_with(&[("B2", "9")]);
        // Z99 で計算しているので、引数なしは自分の位置
        assert_eq!(n(&mut s, "=ROW()"), 99.0);
        assert_eq!(n(&mut s, "=COLUMN()"), 26.0);
        assert_eq!(n(&mut s, "=ROW(B2)"), 2.0);
        assert_eq!(n(&mut s, "=COLUMN(B2)"), 2.0);
        assert_eq!(n(&mut s, "=ROWS(A1:B3)"), 3.0);
        assert_eq!(n(&mut s, "=COLUMNS(A1:B3)"), 2.0);
    }

    #[test]
    fn rank_and_largest_first() {
        let mut s = sheet_with(&[
            ("A1", "70"), ("A2", "90"), ("A3", "80"), ("A4", "90"),
        ]);
        assert_eq!(n(&mut s, "=LARGE(A1:A4,1)"), 90.0);
        assert_eq!(n(&mut s, "=LARGE(A1:A4,3)"), 80.0);
        assert_eq!(n(&mut s, "=SMALL(A1:A4,1)"), 70.0);
        assert_eq!(n(&mut s, "=RANK(80,A1:A4)"), 3.0, "同値の90が2つで80は3位");
        assert_eq!(n(&mut s, "=RANK(90,A1:A4)"), 1.0, "同値は同順位");
        assert_eq!(n(&mut s, "=RANK(70,A1:A4,1)"), 1.0, "昇順なら最小が1位");
        assert_eq!(
            value_of(&mut s, "=LARGE(A1:A4,9)"),
            Value::Error("#NUM!".into())
        );
    }
}

/// 第2段の拡充(2026-08-05)— 統計・数学で「表計算らしさ」を出す約45個。
#[cfg(test)]
mod dan2_tests {
    use super::*;
    use crate::model::Cell;

    fn sheet_with(cells: &[(&str, &str)]) -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for (a1, v) in cells {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s
    }

    fn value_of(s: &mut Sheet, f: &str) -> Value {
        s.set(Pos::parse("Z99").unwrap(), Cell::input(f));
        recalc(s);
        s.value(Pos::parse("Z99").unwrap())
    }

    fn n(s: &mut Sheet, f: &str) -> f64 {
        match value_of(s, f) {
            Value::Number(x) => x,
            v => panic!("{f} が数でない: {v:?}"),
        }
    }

    #[test]
    fn statistics_for_grading() {
        let mut s = sheet_with(&[
            ("A1", "70"), ("A2", "80"), ("A3", "80"), ("A4", "90"), ("A5", "100"),
        ]);
        assert_eq!(n(&mut s, "=MEDIAN(A1:A5)"), 80.0);
        assert_eq!(n(&mut s, "=MEDIAN(A1:A4)"), 80.0, "偶数個は真ん中2つの平均");
        assert_eq!(n(&mut s, "=MODE(A1:A5)"), 80.0);
        assert_eq!(
            value_of(&mut s, "=MODE(A1:A2)"),
            Value::Error("#N/A".into()),
            "重複が無ければ最頻値は無い"
        );
        // 母集団の分散: 平均84、偏差平方和 (196+16+16+36+256)=520 → 104
        assert!((n(&mut s, "=VARP(A1:A5)") - 104.0).abs() < 1e-9);
        assert!((n(&mut s, "=VAR(A1:A5)") - 130.0).abs() < 1e-9, "標本分散は n-1 で割る");
        assert!((n(&mut s, "=STDEVP(A1:A5)") - 104.0f64.sqrt()).abs() < 1e-9);
        assert!((n(&mut s, "=STDEV(A1:A5)") - 130.0f64.sqrt()).abs() < 1e-9);
        assert_eq!(
            value_of(&mut s, "=STDEV(A1)"),
            Value::Error("#DIV/0!".into()),
            "1個から標本標準偏差は出ない"
        );
        assert_eq!(n(&mut s, "=PERCENTILE(A1:A5,0.5)"), 80.0);
        assert_eq!(n(&mut s, "=PERCENTILE(A1:A5,0.25)"), 80.0);
        assert_eq!(n(&mut s, "=QUARTILE(A1:A5,0)"), 70.0, "第0四分位は最小");
        assert_eq!(n(&mut s, "=QUARTILE(A1:A5,4)"), 100.0, "第4四分位は最大");
        assert_eq!(n(&mut s, "=SUMSQ(3,4)"), 25.0);
    }

    #[test]
    fn correlation_and_regression() {
        // y = 2x + 1 きっかり(相関1・傾き2・切片1)
        let mut s = sheet_with(&[
            ("A1", "1"), ("B1", "3"),
            ("A2", "2"), ("B2", "5"),
            ("A3", "3"), ("B3", "7"),
        ]);
        assert!((n(&mut s, "=CORREL(A1:A3,B1:B3)") - 1.0).abs() < 1e-12);
        assert!((n(&mut s, "=SLOPE(B1:B3,A1:A3)") - 2.0).abs() < 1e-12);
        assert!((n(&mut s, "=INTERCEPT(B1:B3,A1:A3)") - 1.0).abs() < 1e-12);
        assert!((n(&mut s, "=FORECAST(10,B1:B3,A1:A3)") - 21.0).abs() < 1e-12);
        assert_eq!(
            value_of(&mut s, "=CORREL(A1:A3,B1:B2)"),
            Value::Error("#N/A".into()),
            "大きさ違いを黙って計算しない"
        );
    }

    #[test]
    fn combinatorics_and_number_theory() {
        let mut s = sheet_with(&[]);
        assert_eq!(n(&mut s, "=FACT(5)"), 120.0);
        assert_eq!(n(&mut s, "=COMBIN(10,3)"), 120.0);
        assert_eq!(n(&mut s, "=PERMUT(10,3)"), 720.0);
        assert_eq!(n(&mut s, "=GCD(12,18,24)"), 6.0);
        assert_eq!(n(&mut s, "=LCM(4,6)"), 12.0);
        assert_eq!(value_of(&mut s, "=FACT(200)"), Value::Error("#NUM!".into()));
    }

    #[test]
    fn trigonometry_and_logarithms() {
        let mut s = sheet_with(&[]);
        assert!((n(&mut s, "=SIN(PI()/2)") - 1.0).abs() < 1e-12);
        assert!((n(&mut s, "=COS(0)") - 1.0).abs() < 1e-12);
        assert!((n(&mut s, "=TAN(PI()/4)") - 1.0).abs() < 1e-12);
        assert!((n(&mut s, "=DEGREES(PI())") - 180.0).abs() < 1e-12);
        assert!((n(&mut s, "=RADIANS(180)") - std::f64::consts::PI).abs() < 1e-12);
        assert!((n(&mut s, "=ASIN(1)") - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        // Excel の約束: ATAN2(x, y) で点 (1,1) は 45度
        assert!((n(&mut s, "=ATAN2(1,1)") - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
        assert!((n(&mut s, "=EXP(1)") - std::f64::consts::E).abs() < 1e-12);
        assert!((n(&mut s, "=LN(EXP(2))") - 2.0).abs() < 1e-12);
        assert_eq!(n(&mut s, "=LOG10(1000)"), 3.0);
        assert_eq!(n(&mut s, "=LOG(8,2)"), 3.0);
        assert_eq!(n(&mut s, "=LOG(100)"), 2.0, "底の既定は10");
        assert_eq!(value_of(&mut s, "=LN(0)"), Value::Error("#NUM!".into()));
        assert_eq!(value_of(&mut s, "=ASIN(2)"), Value::Error("#NUM!".into()));
    }

    #[test]
    fn the_rounding_family() {
        let mut s = sheet_with(&[]);
        assert_eq!(n(&mut s, "=CEILING(6.1,2)"), 8.0);
        assert_eq!(n(&mut s, "=FLOOR(6.9,2)"), 6.0);
        assert_eq!(n(&mut s, "=CEILING(-2.5,-2)"), -4.0, "負の基準は0から遠ざかる");
        assert_eq!(n(&mut s, "=MROUND(7,3)"), 6.0);
        assert_eq!(n(&mut s, "=MROUND(8,3)"), 9.0);
        assert_eq!(n(&mut s, "=EVEN(3)"), 4.0);
        assert_eq!(n(&mut s, "=EVEN(-3)"), -4.0, "0から遠ざかる");
        assert_eq!(n(&mut s, "=ODD(2)"), 3.0);
        assert_eq!(n(&mut s, "=SIGN(-5)"), -1.0);
        assert_eq!(n(&mut s, "=SIGN(0)"), 0.0);
        assert_eq!(
            value_of(&mut s, "=CEILING(2.5,-2)"),
            Value::Error("#NUM!".into()),
            "符号違いを黙って丸めない"
        );
    }

    #[test]
    fn random_numbers_stay_in_range() {
        let mut s = sheet_with(&[]);
        for _ in 0..20 {
            let r = n(&mut s, "=RAND()");
            assert!((0.0..1.0).contains(&r), "RAND が範囲外: {r}");
            let d = n(&mut s, "=RANDBETWEEN(1,6)");
            assert!((1.0..=6.0).contains(&d) && d.fract() == 0.0, "さいころが変: {d}");
        }
        assert_eq!(value_of(&mut s, "=RANDBETWEEN(6,1)"), Value::Error("#NUM!".into()));
    }

    #[test]
    fn information_functions() {
        let mut s = sheet_with(&[("A1", "9"), ("A2", "文字")]);
        assert_eq!(value_of(&mut s, "=ISNUMBER(A1)"), Value::Bool(true));
        assert_eq!(value_of(&mut s, "=ISNUMBER(A2)"), Value::Bool(false));
        assert_eq!(value_of(&mut s, "=ISNUMBER(1/0)"), Value::Bool(false), "エラーは数でない");
        assert_eq!(value_of(&mut s, "=ISTEXT(A2)"), Value::Bool(true));
        assert_eq!(value_of(&mut s, "=ISEVEN(4)"), Value::Bool(true));
        assert_eq!(value_of(&mut s, "=ISODD(4)"), Value::Bool(false));
        assert_eq!(n(&mut s, "=COUNTBLANK(A1:A5)"), 3.0);
    }
}

/// 第3段の拡充(2026-08-05)— 計算で決まる参照とスピル。
#[cfg(test)]
mod dan3_tests {
    use super::*;
    use crate::model::Cell;

    fn sheet_with(cells: &[(&str, &str)]) -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for (a1, v) in cells {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s
    }

    fn v(s: &Sheet, a1: &str) -> Value {
        s.value(Pos::parse(a1).unwrap())
    }

    #[test]
    fn offset_shifts_a_reference() {
        let mut s = sheet_with(&[
            ("A1", "10"), ("B1", "20"),
            ("A2", "30"), ("B2", "40"),
            ("A3", "50"),
            ("Z1", "=OFFSET(A1,1,1)"),
            ("Z2", "=SUM(OFFSET(A1,0,0,3,1))"),
            ("Z3", "=OFFSET(A1,-1,0)"),
            ("Z4", "=OFFSET(A1,0,0,2,2)"),
        ]);
        recalc(&mut s);
        assert_eq!(v(&s, "Z1"), Value::Number(40.0), "1行1列ずらして B2");
        assert_eq!(v(&s, "Z2"), Value::Number(90.0), "高さ3の範囲を SUM に渡す");
        assert_eq!(v(&s, "Z3"), Value::Error("#REF!".into()), "表の外は正直に #REF!");
        assert_eq!(v(&s, "Z4"), Value::Error("#VALUE!".into()),
            "複数セルを1セルの場所に置けない");
    }

    #[test]
    fn indirect_resolves_a_reference_from_text() {
        let mut s = sheet_with(&[
            ("B2", "99"),
            ("C1", "2"),
            ("Z1", "=INDIRECT(\"B2\")"),
            ("Z2", "=INDIRECT(\"B\"&C1)"),
            ("Z3", "=SUM(INDIRECT(\"B1:B3\"))"),
            ("Z4", "=INDIRECT(\"別の表!A1\")"),
            ("Z5", "=INDIRECT(\"ほげ\")"),
        ]);
        recalc(&mut s);
        assert_eq!(v(&s, "Z1"), Value::Number(99.0));
        assert_eq!(v(&s, "Z2"), Value::Number(99.0), "組み立てた参照が解けない");
        assert_eq!(v(&s, "Z3"), Value::Number(99.0), "範囲の間接参照が関数に渡らない");
        assert_eq!(v(&s, "Z4"), Value::Error("#REF!".into()),
            "別のシートはまだ — 黙って自シートと読まない");
        assert_eq!(v(&s, "Z5"), Value::Error("#REF!".into()));
    }

    #[test]
    fn indirection_keeps_up_when_the_target_is_a_formula() {
        // A1 は B1 を間接参照、B1 は C1 の式 — 依存が読めないので複数周で収束
        let mut s = sheet_with(&[
            ("A1", "=INDIRECT(\"B1\")"),
            ("B1", "=C1+1"),
            ("C1", "5"),
        ]);
        recalc(&mut s);
        assert_eq!(v(&s, "A1"), Value::Number(6.0), "1周目の古い値で止まっている");
    }

    #[test]
    fn sequence_spills() {
        let mut s = sheet_with(&[("A1", "=SEQUENCE(3,2)")]);
        recalc(&mut s);
        for (a1, n) in [("A1", 1.0), ("B1", 2.0), ("A2", 3.0), ("B2", 4.0),
                        ("A3", 5.0), ("B3", 6.0)] {
            assert_eq!(v(&s, a1), Value::Number(n), "{a1} が違う");
        }
        assert_eq!(s.spills.get(&Pos::parse("A1").unwrap()), Some(&(3, 2)));
        // 縮めたら残骸は消える
        s.set(Pos::parse("A1").unwrap(), Cell::input("=SEQUENCE(2,1)"));
        recalc(&mut s);
        assert_eq!(v(&s, "A2"), Value::Number(2.0));
        assert_eq!(v(&s, "A3"), Value::Empty, "縮めた後に残骸が残った");
        assert_eq!(v(&s, "B1"), Value::Empty);
        assert_eq!(s.spills.get(&Pos::parse("A1").unwrap()), Some(&(2, 1)));
    }

    #[test]
    fn an_occupied_cell_blocks_the_spill() {
        let mut s = sheet_with(&[
            ("A1", "=SEQUENCE(3,1)"),
            ("A3", "既にある"),
        ]);
        recalc(&mut s);
        assert_eq!(v(&s, "A1"), Value::Error("#SPILL!".into()),
            "先客を黙って潰してはいけない");
        assert_eq!(v(&s, "A3"), Value::Text("既にある".into()), "先客が消えた");
        assert_eq!(v(&s, "A2"), Value::Empty, "途中まで書いてはいけない");
        // 先客がどけば次の再計算であふれる
        s.set(Pos::parse("A3").unwrap(), Cell::default());
        recalc(&mut s);
        assert_eq!(v(&s, "A1"), Value::Number(1.0));
        assert_eq!(v(&s, "A3"), Value::Number(3.0));
    }

    #[test]
    fn filter_sort_and_unique() {
        let mut s = sheet_with(&[
            ("A1", "筆"), ("B1", "100"), ("C1", "1"),
            ("A2", "紙"), ("B2", "300"), ("C2", "0"),
            ("A3", "机"), ("B3", "200"), ("C3", "1"),
            ("E1", "=FILTER(A1:B3,C1:C3)"),
            ("H1", "=SORT(A1:B3,2,-1)"),
            ("K1", "=UNIQUE(C1:C3)"),
            ("M1", "=FILTER(A1:B3,B1:B3>999,\"該当なし\")"),
        ]);
        recalc(&mut s);
        // FILTER: C=1 の行だけ
        assert_eq!(v(&s, "E1"), Value::Text("筆".into()));
        assert_eq!(v(&s, "F1"), Value::Number(100.0));
        assert_eq!(v(&s, "E2"), Value::Text("机".into()));
        // SORT: 金額の大きい順
        assert_eq!(v(&s, "H1"), Value::Text("紙".into()));
        assert_eq!(v(&s, "H2"), Value::Text("机".into()));
        assert_eq!(v(&s, "H3"), Value::Text("筆".into()));
        // UNIQUE: 1 と 0
        assert_eq!(v(&s, "K1"), Value::Number(1.0));
        assert_eq!(v(&s, "K2"), Value::Number(0.0));
        assert_eq!(s.spills.get(&Pos::parse("K1").unwrap()), Some(&(2, 1)));
        // 1件も無いときは第3引数
        assert_eq!(v(&s, "M1"), Value::Text("該当なし".into()));
    }

    #[test]
    fn the_spill_record_round_trips_through_xlsx() {
        let mut book = crate::Book::new();
        book.sheets[0] = sheet_with(&[("A1", "=SEQUENCE(3,1)"), ("C1", "=SUM(A1:A3)")]);
        book.sheets[0].name = "Sheet1".into();
        recalc(&mut book.sheets[0]);
        assert_eq!(v(&book.sheets[0], "C1"), Value::Number(6.0),
            "スピルの結果を普通の式が拾えない");
        let mut buf = std::io::Cursor::new(Vec::new());
        crate::xlsx::write(&book, &mut buf).unwrap();
        let (mut back, _) = crate::xlsx::read(std::io::Cursor::new(buf.into_inner())).unwrap();
        assert_eq!(back.sheets[0].spills.get(&Pos::parse("A1").unwrap()), Some(&(3, 1)),
            "スピルの記録が往復しない");
        // 開き直して再計算しても、自分の跡を先客と間違えない
        recalc(&mut back.sheets[0]);
        assert_eq!(v(&back.sheets[0], "A1"), Value::Number(1.0),
            "開き直しで偽の #SPILL! になった");
        assert_eq!(v(&back.sheets[0], "A3"), Value::Number(3.0));
    }

    #[test]
    fn an_array_formula_combined_with_operators_spills_too() {
        // 2026-08-05 まで #配列単独 と断っていた形。要素ごとに計算して広がる
        let mut s = sheet_with(&[("A1", "=SEQUENCE(3,1)+1")]);
        recalc(&mut s);
        assert_eq!(v(&s, "A1"), Value::Number(2.0));
        assert_eq!(v(&s, "A2"), Value::Number(3.0));
        assert_eq!(v(&s, "A3"), Value::Number(4.0));
    }
}

/// 第4段の拡充(2026-08-05)— Excel で作った実物のファイルが読める穴埋め。
#[cfg(test)]
mod dan4_tests {
    use super::*;
    use crate::model::Cell;

    fn sheet_with(cells: &[(&str, &str)]) -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for (a1, v) in cells {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s
    }

    fn value_of(s: &mut Sheet, f: &str) -> Value {
        s.set(Pos::parse("Z99").unwrap(), Cell::input(f));
        recalc(s);
        s.value(Pos::parse("Z99").unwrap())
    }

    fn n(s: &mut Sheet, f: &str) -> f64 {
        match value_of(s, f) {
            Value::Number(x) => x,
            v => panic!("{f} が数でない: {v:?}"),
        }
    }

    fn t(s: &mut Sheet, f: &str) -> String {
        match value_of(s, f) {
            Value::Text(x) => x,
            v => panic!("{f} が文字でない: {v:?}"),
        }
    }

    #[test]
    fn subtotal_is_the_filter_staple() {
        let mut s = sheet_with(&[
            ("A1", "10"), ("A2", "20"), ("A3", "30"), ("A4", "文字"),
        ]);
        assert_eq!(n(&mut s, "=SUBTOTAL(9,A1:A4)"), 60.0, "9=SUM");
        assert_eq!(n(&mut s, "=SUBTOTAL(109,A1:A4)"), 60.0, "109 も SUM と同じに扱う");
        assert_eq!(n(&mut s, "=SUBTOTAL(1,A1:A3)"), 20.0, "1=AVERAGE");
        assert_eq!(n(&mut s, "=SUBTOTAL(2,A1:A4)"), 3.0, "2=COUNT(数だけ)");
        assert_eq!(n(&mut s, "=SUBTOTAL(3,A1:A4)"), 4.0, "3=COUNTA");
        assert_eq!(n(&mut s, "=SUBTOTAL(4,A1:A3)"), 30.0);
        assert_eq!(n(&mut s, "=SUBTOTAL(5,A1:A3)"), 10.0);
    }

    #[test]
    fn renamed_and_selection_functions() {
        let mut s = sheet_with(&[("A1", "70"), ("A2", "90"), ("A3", "90")]);
        assert_eq!(t(&mut s, "=CONCAT(\"防火\",\"戸\")"), "防火戸");
        assert_eq!(t(&mut s, "=IFNA(NA(),\"無し\")"), "無し");
        assert_eq!(value_of(&mut s, "=ISNA(NA())"), Value::Bool(true));
        assert_eq!(value_of(&mut s, "=ISNA(1/0)"), Value::Bool(false), "#DIV/0! は NA でない");
        assert_eq!(value_of(&mut s, "=ISERR(1/0)"), Value::Bool(true));
        assert_eq!(n(&mut s, "=RANK.EQ(90,A1:A3)"), 1.0);
        assert_eq!(n(&mut s, "=RANK.AVG(90,A1:A3)"), 1.5, "同値2つの順位の平均");
        assert_eq!(value_of(&mut s, "=TRUE()"), Value::Bool(true), "括弧つきの TRUE()");
        assert_eq!(t(&mut s, "=HYPERLINK(\"https://例\",\"表示名\")"), "表示名");
    }

    #[test]
    fn the_new_rounding_and_quotient() {
        let mut s = sheet_with(&[]);
        assert_eq!(n(&mut s, "=CEILING.MATH(6.1)"), 7.0, "基準の既定は1");
        assert_eq!(n(&mut s, "=CEILING.MATH(-6.1,2)"), -6.0, "負は0へ寄るのが既定");
        assert_eq!(n(&mut s, "=FLOOR.MATH(-6.1,2)"), -8.0);
        assert_eq!(n(&mut s, "=QUOTIENT(7,2)"), 3.0);
        assert_eq!(n(&mut s, "=QUOTIENT(-7,2)"), -3.0, "商は0へ切る");
        assert_eq!(value_of(&mut s, "=QUOTIENT(7,0)"), Value::Error("#DIV/0!".into()));
    }

    #[test]
    fn the_classic_lookup_and_transpose() {
        let mut s = sheet_with(&[
            ("A1", "10"), ("B1", "甲"),
            ("A2", "20"), ("B2", "乙"),
            ("A3", "30"), ("B3", "丙"),
            ("D1", "=LOOKUP(25,A1:A3,B1:B3)"),
            ("E1", "=TRANSPOSE(A1:B3)"),
        ]);
        recalc(&mut s);
        assert_eq!(s.value(Pos::parse("D1").unwrap()), Value::Text("乙".into()),
            "25以下でいちばん大きい 20 の行");
        // 3行2列 → 2行3列にあふれる
        assert_eq!(s.value(Pos::parse("E1").unwrap()), Value::Number(10.0));
        assert_eq!(s.value(Pos::parse("G1").unwrap()), Value::Number(30.0));
        assert_eq!(s.value(Pos::parse("E2").unwrap()), Value::Text("甲".into()));
        assert_eq!(s.value(Pos::parse("G2").unwrap()), Value::Text("丙".into()));
    }

    #[test]
    fn date_weeks_and_day_counts() {
        let mut s = sheet_with(&[]);
        assert_eq!(n(&mut s, "=DAYS(DATE(2026,8,5),DATE(2026,8,1))"), 4.0);
        assert_eq!(n(&mut s, "=DAYS360(DATE(2026,1,31),DATE(2026,3,1))"), 31.0,
            "30/360 の数え方");
        assert!((n(&mut s, "=YEARFRAC(DATE(2026,1,1),DATE(2026,7,1))") - 0.5).abs() < 1e-9);
        // 2026-01-01 は木曜 → 第1週。2026-08-05 は?(自前の暦で数える)
        assert_eq!(n(&mut s, "=WEEKNUM(DATE(2026,1,1))"), 1.0);
        assert_eq!(n(&mut s, "=ISOWEEKNUM(DATE(2026,1,1))"), 1.0,
            "木曜を含む週が ISO の第1週");
        assert_eq!(t(&mut s, "=ADDRESS(5,2)"), "$B$5");
        assert_eq!(t(&mut s, "=ADDRESS(5,2,4)"), "B5", "4=相対");
    }

    #[test]
    fn the_iterative_financial_solution() {
        let mut s = sheet_with(&[
            ("A1", "-1000"), ("A2", "500"), ("A3", "500"), ("A4", "500"),
        ]);
        // IRR: -1000 + 500/(1+r) + 500/(1+r)^2 + 500/(1+r)^3 = 0 → 約 23.4%
        let irr = n(&mut s, "=IRR(A1:A4)");
        assert!((irr - 0.2337).abs() < 0.001, "IRR がずれる: {irr}");
        // RATE: PMT(0.01,60,1000000) の逆算 → 月利1%
        let rate = n(&mut s, "=RATE(60,-22244.4477,1000000)");
        assert!((rate - 0.01).abs() < 1e-6, "RATE がずれる: {rate}");
        // NPV: 利率0なら単純合計
        assert!((n(&mut s, "=NPV(0,A2:A4)") - 1500.0).abs() < 1e-9);
    }

    #[test]
    fn the_rest_of_the_text_tools() {
        let mut s = sheet_with(&[]);
        assert_eq!(t(&mut s, "=PROPER(\"hello world\")"), "Hello World");
        assert_eq!(value_of(&mut s, "=EXACT(\"Abc\",\"abc\")"), Value::Bool(false));
        assert_eq!(value_of(&mut s, "=EXACT(\"戸\",\"戸\")"), Value::Bool(true));
        assert_eq!(t(&mut s, "=FIXED(1234.567,1)"), "1,234.6");
        assert_eq!(t(&mut s, "=YEN(1234567)"), "¥1,234,567.00");
        assert_eq!(t(&mut s, "=YEN(1234567,0)"), "¥1,234,567");
        assert_eq!(n(&mut s, "=NUMBERVALUE(\"1.234,56\",\",\",\".\")"), 1234.56,
            "欧州式の区切りも読める");
        assert_eq!(t(&mut s, "=T(\"文字\")"), "文字");
        assert_eq!(t(&mut s, "=T(123)"), "");
        assert_eq!(n(&mut s, "=N(TRUE)"), 1.0);
        assert_eq!(n(&mut s, "=TYPE(\"a\")"), 2.0);
        assert_eq!(n(&mut s, "=TYPE(1/0)"), 16.0, "エラーの型は16");
        assert_eq!(t(&mut s, "=UNICHAR(12354)"), "あ");
        assert_eq!(n(&mut s, "=UNICODE(\"あ\")"), 12354.0);
    }

    #[test]
    fn the_byte_length_family_counts_full_width_as_two() {
        let mut s = sheet_with(&[]);
        assert_eq!(n(&mut s, "=LENB(\"防火戸\")"), 6.0);
        assert_eq!(n(&mut s, "=LENB(\"abc\")"), 3.0);
        assert_eq!(n(&mut s, "=LENB(\"ｱｲｳ\")"), 3.0, "半角カナは1");
        assert_eq!(t(&mut s, "=LEFTB(\"防火戸\",4)"), "防火");
        assert_eq!(t(&mut s, "=LEFTB(\"防火戸\",3)"), "防", "半端な1バイトは取らない");
        assert_eq!(t(&mut s, "=RIGHTB(\"防火戸\",2)"), "戸");
        assert_eq!(t(&mut s, "=MIDB(\"防火戸\",3,2)"), "火");
    }

    #[test]
    fn full_width_and_half_width_conversion() {
        let mut s = sheet_with(&[]);
        assert_eq!(t(&mut s, "=ASC(\"ＡＢＣ１２３\")"), "ABC123");
        assert_eq!(t(&mut s, "=ASC(\"カタカナ\")"), "ｶﾀｶﾅ");
        assert_eq!(t(&mut s, "=ASC(\"ガンダム\")"), "ｶﾞﾝﾀﾞﾑ", "濁点は2文字に割れる");
        assert_eq!(t(&mut s, "=JIS(\"ｶﾞﾝﾀﾞﾑ\")"), "ガンダム", "濁点が1文字に組み直る");
        assert_eq!(t(&mut s, "=JIS(\"abc 123\")"), "ａｂｃ　１２３");
        // 往復して戻る
        assert_eq!(t(&mut s, "=JIS(ASC(\"パピプペポ・ヴ\"))"), "パピプペポ・ヴ");
    }

    #[test]
    fn japanese_era_text() {
        let mut s = sheet_with(&[]);
        assert_eq!(t(&mut s, "=DATESTRING(DATE(2026,8,5))"), "令和08年08月05日");
        assert_eq!(t(&mut s, "=DATESTRING(DATE(1989,1,7))"), "昭和64年01月07日",
            "改元の前日は前の元号");
        assert_eq!(t(&mut s, "=DATESTRING(DATE(1989,1,8))"), "平成01年01月08日");
        assert_eq!(t(&mut s, "=DATESTRING(DATE(2019,5,1))"), "令和01年05月01日");
    }

    #[test]
    fn the_a_suffixed_stats_count_text_as_zero() {
        let mut s = sheet_with(&[("A1", "10"), ("A2", "文字"), ("A3", "20")]);
        assert_eq!(n(&mut s, "=AVERAGEA(A1:A3)"), 10.0, "(10+0+20)/3");
        assert_eq!(n(&mut s, "=MAXA(A1:A3)"), 20.0);
        assert_eq!(n(&mut s, "=MINA(A1:A3)"), 0.0, "文字の0が最小");
    }
}

/// 残件の掃討(2026-08-05)— 和暦の表示形式・配列の入れ子・
/// ふりがな・別のシートへの間接参照。
#[cfg(test)]
mod dan5_tests {
    use super::*;
    use crate::model::Cell;

    fn sheet_with(cells: &[(&str, &str)]) -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for (a1, v) in cells {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s
    }

    fn value_of(s: &mut Sheet, f: &str) -> Value {
        s.set(Pos::parse("Z99").unwrap(), Cell::input(f));
        recalc(s);
        s.value(Pos::parse("Z99").unwrap())
    }

    fn t(s: &mut Sheet, f: &str) -> String {
        match value_of(s, f) {
            Value::Text(x) => x,
            v => panic!("{f} が文字でない: {v:?}"),
        }
    }

    #[test]
    fn the_japanese_era_number_format() {
        let mut s = sheet_with(&[]);
        assert_eq!(t(&mut s, "=TEXT(DATE(2026,8,5),\"ggge年m月d日\")"), "令和8年8月5日");
        assert_eq!(t(&mut s, "=TEXT(DATE(2026,8,5),\"gge\")"), "令8");
        assert_eq!(t(&mut s, "=TEXT(DATE(2026,8,5),\"ge\")"), "R8");
        assert_eq!(t(&mut s, "=TEXT(DATE(1989,1,7),\"ggge年\")"), "昭和64年");
        assert_eq!(t(&mut s, "=TEXT(DATE(2026,8,5),\"ggg ee\")"), "令和 08", "ee は0詰め");
    }

    #[test]
    fn arrays_can_be_mixed_into_a_formula() {
        let mut s = sheet_with(&[
            ("A1", "10"), ("B1", "1"),
            ("A2", "20"), ("B2", "0"),
            ("A3", "30"), ("B3", "1"),
        ]);
        assert_eq!(value_of(&mut s, "=SUM(FILTER(A1:A3,B1:B3))"), Value::Number(40.0),
            "SUM(FILTER(…)) の定番が通らない");
        assert_eq!(value_of(&mut s, "=COUNTA(UNIQUE(B1:B3))"), Value::Number(2.0));
        assert_eq!(value_of(&mut s, "=SUM(SEQUENCE(10))"), Value::Number(55.0));
        assert_eq!(value_of(&mut s, "=SUM(FILTER(A1:A3,B1:B3))+1"), Value::Number(41.0),
            "集計に食わせた残りの四則も通る");
    }

    #[test]
    fn furigana_reads_and_round_trips() {
        let mut book = crate::Book::new();
        book.sheets[0] = sheet_with(&[("A1", "日本"), ("A2", "ふりがな無し")]);
        book.sheets[0].name = "Sheet1".into();
        book.sheets[0].phonetics.insert(Pos::parse("A1").unwrap(), "ニホン".into());
        // PHONETIC 関数: 読みがあれば読み、無ければ字そのもの
        let s = &mut book.sheets[0];
        assert_eq!(value_of(s, "=PHONETIC(A1)"), Value::Text("ニホン".into()));
        assert_eq!(value_of(s, "=PHONETIC(A2)"), Value::Text("ふりがな無し".into()));
        // xlsx を往復しても読みが残る(rPh — 欧米の実装が落とす宝)
        let mut buf = std::io::Cursor::new(Vec::new());
        crate::xlsx::write(&book, &mut buf).unwrap();
        let (back, _) = crate::xlsx::read(std::io::Cursor::new(buf.into_inner())).unwrap();
        assert_eq!(
            back.sheets[0].phonetics.get(&Pos::parse("A1").unwrap()),
            Some(&"ニホン".to_string()),
            "ふりがなが保存で落ちた"
        );
    }

    #[test]
    fn an_indirect_reference_to_another_sheet() {
        let mut book = crate::Book::new();
        book.sheets[0] = sheet_with(&[("A1", "=INDIRECT(\"台帳!B2\")"),
            ("A2", "=SUM(INDIRECT(\"台帳!B1:B3\"))"),
            ("A3", "=INDIRECT(\"'台帳'!B2\")")]);
        book.sheets[0].name = "表紙".into();
        let mut daicho = sheet_with(&[("B1", "10"), ("B2", "20"), ("B3", "=B1+B2")]);
        daicho.name = "台帳".into();
        book.sheets.push(daicho);
        recalc_all(&mut book);
        let v = |a1: &str| book.sheets[0].value(Pos::parse(a1).unwrap());
        assert_eq!(v("A1"), Value::Number(20.0), "別のシートの1セルが引けない");
        assert_eq!(v("A2"), Value::Number(60.0), "別のシートの範囲が SUM に渡らない");
        assert_eq!(v("A3"), Value::Number(20.0), "'名前'! の引用が剥けない");
        // 1枚だけの再計算では正直に #REF!
        let mut alone = sheet_with(&[("A1", "=INDIRECT(\"台帳!B2\")")]);
        recalc(&mut alone);
        assert_eq!(alone.value(Pos::parse("A1").unwrap()), Value::Error("#REF!".into()));
    }

    #[test]
    fn three_argument_sumif_indirecting_to_another_sheet() {
        // 実物の xlsx で出た形。条件範囲と合計範囲が別々に INDIRECT で来る
        let mut book = crate::Book::new();
        book.sheets[0] =
            sheet_with(&[("A1", "=SUMIF(INDIRECT(\"4月!A1:A3\"),\"りんご\",INDIRECT(\"4月!B1:B3\"))")]);
        book.sheets[0].name = "表紙".into();
        let mut april = sheet_with(&[
            ("A1", "りんご"), ("B1", "100"),
            ("A2", "みかん"), ("B2", "200"),
            ("A3", "りんご"), ("B3", "50"),
        ]);
        april.name = "4月".into();
        book.sheets.push(april);
        recalc_all(&mut book);
        assert_eq!(
            book.sheets[0].value(Pos::parse("A1").unwrap()),
            Value::Number(150.0),
            "3引数 SUMIF が黙って違う数を返している"
        );
    }
}

/// 配列数式と演算子の組み合わせ(2026-08-05)。
#[cfg(test)]
mod dan6_tests {
    use super::*;
    use crate::model::Cell;

    fn sheet_with(cells: &[(&str, &str)]) -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for (a1, v) in cells {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s
    }

    fn v(s: &Sheet, a1: &str) -> Value {
        s.value(Pos::parse(a1).unwrap())
    }

    #[test]
    fn element_wise_arithmetic_and_concatenation() {
        let mut s = sheet_with(&[
            ("A1", "10"), ("A2", "20"), ("A3", "30"),
            ("C1", "=SEQUENCE(3,1)*10+A1:A3"),
            ("E1", "=\"第\"&SEQUENCE(3,1)&\"回\""),
        ]);
        recalc(&mut s);
        assert_eq!(v(&s, "C1"), Value::Number(20.0), "10*1+10");
        assert_eq!(v(&s, "C2"), Value::Number(40.0));
        assert_eq!(v(&s, "C3"), Value::Number(60.0));
        assert_eq!(v(&s, "E1"), Value::Text("第1回".into()));
        assert_eq!(v(&s, "E3"), Value::Text("第3回".into()));
    }

    /// **6つの記号が同じ基準で比べる**(2026-08-22)。
    ///
    /// 前は `=` と `<>` だけが甘く、`<` `>` `<=` `>=` は厳密でした。
    /// そのため `=0.1+0.2=0.3` も `=(0.1+0.2)>0.3` も真という、同時には
    /// 成り立たないはずの答えが出ていました。
    #[test]
    fn equal_means_neither_greater_nor_less() {
        let mut s = sheet_with(&[
            ("A1", "=0.1+0.2=0.3"),
            ("A2", "=(0.1+0.2)>0.3"),
            ("A3", "=(0.1+0.2)<0.3"),
            ("A4", "=(0.1+0.2)>=0.3"),
            ("A5", "=(0.1+0.2)<=0.3"),
            ("A6", "=(0.1+0.2)<>0.3"),
        ]);
        recalc(&mut s);
        assert_eq!(v(&s, "A1"), Value::Bool(true), "= が甘くない");
        assert_eq!(v(&s, "A2"), Value::Bool(false), "等しいのに大きい");
        assert_eq!(v(&s, "A3"), Value::Bool(false), "等しいのに小さい");
        assert_eq!(v(&s, "A4"), Value::Bool(true), ">= が成り立たない");
        assert_eq!(v(&s, "A5"), Value::Bool(true), "<= が成り立たない");
        assert_eq!(v(&s, "A6"), Value::Bool(false), "<> が甘くない");
    }

    /// **甘さは相対**(2026-08-22)。前は差に `f64::EPSILON` を
    /// そのまま当てていたので、小さい数では甘すぎました。
    #[test]
    fn the_tolerance_is_the_same_for_small_and_large_numbers() {
        let mut s = sheet_with(&[
            // 9倍違う。**前はこれが真でした**
            ("A1", "=0.000000000000000001=0.000000000000000009"),
            ("A2", "=0.000000000000000001<0.000000000000000009"),
            // 2倍違う。**前はこれも真でした**
            ("A3", "=0.00000000000000000001=0.00000000000000000002"),
            // 大きい数。刻みより小さい差は等しい
            ("A4", "=10000000000=10000000001"),
            ("A5", "=1000000000000000=1000000000000000.1"),
            // 0 どうしは等しい(割り算をしない)
            ("A6", "=0=0"),
            ("A7", "=0>0"),
        ]);
        recalc(&mut s);
        assert_eq!(v(&s, "A1"), Value::Bool(false), "9倍違うのに等しい");
        assert_eq!(v(&s, "A2"), Value::Bool(true), "小さい方が小さくない");
        assert_eq!(v(&s, "A3"), Value::Bool(false), "2倍違うのに等しい");
        assert_eq!(v(&s, "A4"), Value::Bool(false), "1 は 1e10 の刻みより大きい");
        assert_eq!(v(&s, "A5"), Value::Bool(true), "刻みより小さい差は等しい");
        assert_eq!(v(&s, "A6"), Value::Bool(true));
        assert_eq!(v(&s, "A7"), Value::Bool(false));
    }

    #[test]
    fn comparison_and_parentheses() {
        let mut s = sheet_with(&[
            ("A1", "=SEQUENCE(3,1)>=2"),
            ("C1", "=(SEQUENCE(2,1)+1)*2"),
        ]);
        recalc(&mut s);
        assert_eq!(v(&s, "A1"), Value::Bool(false));
        assert_eq!(v(&s, "A2"), Value::Bool(true));
        assert_eq!(v(&s, "A3"), Value::Bool(true));
        assert_eq!(v(&s, "C1"), Value::Number(4.0));
        assert_eq!(v(&s, "C2"), Value::Number(6.0));
    }

    #[test]
    fn mismatched_shapes_give_an_error() {
        // {1;2;3} + {1;2} → 3行目は #N/A(Excel の配列数式と同じ)
        let mut s = sheet_with(&[("A1", "=SEQUENCE(3,1)+SEQUENCE(2,1)")]);
        recalc(&mut s);
        assert_eq!(v(&s, "A1"), Value::Number(2.0));
        assert_eq!(v(&s, "A2"), Value::Number(4.0));
        assert_eq!(v(&s, "A3"), Value::Error("#N/A".into()));
    }

    #[test]
    fn element_wise_calculation_works_inside_arguments() {
        let mut s = sheet_with(&[
            ("A1", "1"), ("A2", "2"), ("A3", "3"),
            ("C1", "=SUM(SEQUENCE(3,1)*2)"),
            ("C2", "=SUM(A1:A3*10)"),
            ("C3", "=SUMPRODUCT(A1:A3,A1:A3)"),
        ]);
        recalc(&mut s);
        assert_eq!(v(&s, "C1"), Value::Number(12.0), "SUM(SEQUENCE*2)");
        assert_eq!(v(&s, "C2"), Value::Number(60.0), "範囲の要素ごとの倍が SUM に渡らない");
        assert_eq!(v(&s, "C3"), Value::Number(14.0), "既存の SUMPRODUCT はそのまま");
    }

    #[test]
    fn falling_back_to_an_aggregate_keeps_a_single_value() {
        let mut s = sheet_with(&[("A1", "=SUM(SEQUENCE(3,1))+1"), ("B1", "9")]);
        recalc(&mut s);
        assert_eq!(v(&s, "A1"), Value::Number(7.0));
        assert!(s.spills.is_empty(), "1つの値なのにスピルの記録が残った");
        assert_eq!(v(&s, "B1"), Value::Number(9.0), "隣に何も書いていない");
    }
}

#[cfg(test)]
mod py_cell_tests {
    use super::*;
    use crate::model::Cell;

    #[test]
    fn a_py_cell_keeps_its_value_and_is_not_rerun() {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::parse("A1").unwrap(), Cell::input("10"));
        let mut py = Cell::input("=PY(\"倍\",A1)");
        py.value = Value::Number(20.0); // 前に計算した値
        s.set(Pos::parse("B1").unwrap(), py);
        s.set(Pos::parse("C1").unwrap(), Cell::input("=B1+5"));
        recalc(&mut s);
        assert_eq!(s.value(Pos::parse("B1").unwrap()), Value::Number(20.0), "PY の値が流された");
        assert_eq!(s.value(Pos::parse("C1").unwrap()), Value::Number(25.0), "下流が古い値を見ない");
        // 一度も計算していない PY は #PY? の印
        s.set(Pos::parse("D1").unwrap(), Cell::input("=PY(\"倍\",A1)"));
        recalc(&mut s);
        assert_eq!(s.value(Pos::parse("D1").unwrap()), Value::Error("#PY?".into()));
        // 式の途中の PY は正直に断る
        s.set(Pos::parse("E1").unwrap(), Cell::input("=PY(\"倍\",A1)+1"));
        recalc(&mut s);
        assert_eq!(s.value(Pos::parse("E1").unwrap()), Value::Error("#PY単独".into()));
    }

    #[test]
    fn a_py_call_resolves_to_its_arguments() {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::parse("A1").unwrap(), Cell::input("1"));
        s.set(Pos::parse("A2").unwrap(), Cell::input("2"));
        s.set(Pos::parse("B1").unwrap(), Cell::input("3"));
        s.set(Pos::parse("B2").unwrap(), Cell::input("4"));
        recalc(&mut s);
        let (name, args) =
            eval_py_call(&s, "PY(\"集計\", A1:B2, 10, \"甲\")").expect("解けない");
        assert_eq!(name, "集計");
        assert_eq!(args.len(), 3);
        match &args[0] {
            PyArg::Rect(cols, vs) => {
                assert_eq!(*cols, 2);
                assert_eq!(vs.len(), 4, "2x2 のはず");
            }
            _ => panic!("範囲が形を失った"),
        }
        match (&args[1], &args[2]) {
            (PyArg::One(Value::Number(n)), PyArg::One(Value::Text(t))) => {
                assert_eq!(*n, 10.0);
                assert_eq!(t, "甲");
            }
            _ => panic!("引数の型が違う"),
        }
    }
}

/// 直書きの別シート参照(2026-08-08。それまでは `!` を読めず #ERROR! だった)。
/// 他所の xlsx にはこの形の式が並の頻度で入っている — 乗り換えの壁だった所
#[cfg(test)]
mod cross_sheet_tests {
    use super::*;
    use crate::model::Cell;

    fn sheet_named(name: &str, cells: &[(&str, &str)]) -> Sheet {
        let mut s = Sheet { name: name.into(), ..Default::default() };
        for (a1, v) in cells {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s
    }

    /// 表紙 + 4月 + '5月 実績' の3枚。表紙の式を引数で差し替えて値を見る
    fn ans(formula: &str) -> Value {
        let mut book = crate::Book::new();
        book.sheets[0] = sheet_named("表紙", &[("A1", formula)]);
        book.sheets.push(sheet_named("4月", &[("B1", "100"), ("B2", "200"), ("B3", "文")]));
        book.sheets.push(sheet_named("5月 実績", &[("B2", "50")]));
        recalc_all(&mut book);
        book.sheets[0].value(Pos::parse("A1").unwrap())
    }

    #[test]
    fn a_literal_cross_sheet_reference_resolves() {
        // 1セル・和文のシート名(Excel が普通に書く形)
        assert_eq!(ans("=4月!B1"), Value::Number(100.0));
        // 範囲は関数の中で並びとして渡る
        assert_eq!(ans("=SUM(4月!B1:B2)"), Value::Number(300.0));
        assert_eq!(ans("=COUNTA(4月!B1:B3)"), Value::Number(3.0));
        // 式の中で他の値と混ぜられる
        assert_eq!(ans("=4月!B1*2+1"), Value::Number(201.0));
        // 引用符つき(空白を含む名前)
        assert_eq!(ans("='5月 実績'!B2"), Value::Number(50.0));
        // 自分のシート名は普通の参照として働く
        let mut book = crate::Book::new();
        book.sheets[0] = sheet_named("表紙", &[("A1", "=表紙!C3"), ("C3", "7")]);
        recalc_all(&mut book);
        assert_eq!(book.sheets[0].value(Pos::parse("A1").unwrap()), Value::Number(7.0));
    }

    #[test]
    fn an_unknown_sheet_or_a_single_sheet_calculation_is_a_ref_error() {
        // 黙って自分のシートと読まない
        assert_eq!(ans("=無い月!B1"), Value::Error("#REF!".into()));
        // 1枚だけの再計算(others が空)でも #REF! — 嘘の値を出さない
        let mut only = sheet_named("表紙", &[("A1", "=4月!B1")]);
        recalc(&mut only);
        assert_eq!(only.value(Pos::parse("A1").unwrap()), Value::Error("#REF!".into()));
    }

    #[test]
    fn the_existing_notation_is_not_broken() {
        // INDIRECT の道は今までどおり
        assert_eq!(ans("=INDIRECT(\"4月!B1\")"), Value::Number(100.0));
        assert_eq!(ans("=SUM(INDIRECT(\"4月!B1:B2\"))"), Value::Number(300.0));
        // 同じシートの参照・範囲・関数名は `!` を足しても変わらない
        let mut s = sheet_named("表紙", &[
            ("A1", "10"), ("A2", "20"),
            ("B1", "=SUM(A1:A2)"), ("B2", "=A1<>A2"), ("B3", "=NOT(A1=A2)"),
        ]);
        recalc(&mut s);
        assert_eq!(s.value(Pos::parse("B1").unwrap()), Value::Number(30.0));
        assert_eq!(s.value(Pos::parse("B2").unwrap()), Value::Bool(true));
        assert_eq!(s.value(Pos::parse("B3").unwrap()), Value::Bool(true));
    }

    #[test]
    fn chained_formulas_across_sheets_resolve() {
        // 4月!B1 → 集計!A1 → 表紙!A1 の2段(再計算の周回が足りるか)
        let mut book = crate::Book::new();
        book.sheets[0] = sheet_named("表紙", &[("A1", "=集計!A1+1")]);
        book.sheets.push(sheet_named("集計", &[("A1", "=4月!B1*2")]));
        book.sheets.push(sheet_named("4月", &[("B1", "100")]));
        recalc_all(&mut book);
        assert_eq!(book.sheets[0].value(Pos::parse("A1").unwrap()), Value::Number(201.0));
    }
}

/// SUBTOTAL/AGGREGATE の 101〜111(隠した行を飛ばす)。2026-08-08 実装 —
/// それまでは 1〜11 と同じに扱っていて、畳んだ台帳で黙って違う数が出ていた
#[cfg(test)]
mod subtotal_hidden_tests {
    use super::*;
    use crate::model::Cell;

    fn sheet4() -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for (a1, v) in [("A1", "10"), ("A2", "20"), ("A3", "30"), ("A4", "40")] {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s
    }

    fn val(s: &mut Sheet, f: &str) -> Value {
        s.set(Pos::parse("C1").unwrap(), Cell::input(f));
        recalc(s);
        s.value(Pos::parse("C1").unwrap())
    }

    #[test]
    fn only_the_one_hundred_series_skips_hidden_rows() {
        let mut s = sheet4();
        // 2行目(A2=20)を畳む
        s.row_hidden.insert(1);
        // 9 = SUM(全部数える)/ 109 = SUM(隠した行を飛ばす)
        assert_eq!(val(&mut s, "=SUBTOTAL(9,A1:A4)"), Value::Number(100.0));
        assert_eq!(val(&mut s, "=SUBTOTAL(109,A1:A4)"), Value::Number(80.0), "隠した行を飛ばしていない");
        // 平均・個数・最大・最小も同じ規則
        assert_eq!(val(&mut s, "=SUBTOTAL(101,A1:A4)"), Value::Number(80.0 / 3.0));
        assert_eq!(val(&mut s, "=SUBTOTAL(102,A1:A4)"), Value::Number(3.0));
        assert_eq!(val(&mut s, "=SUBTOTAL(104,A1:A4)"), Value::Number(40.0));
        assert_eq!(val(&mut s, "=SUBTOTAL(105,A1:A4)"), Value::Number(10.0));
        // AGGREGATE も同じ(第2引数は無視の指定)
        assert_eq!(val(&mut s, "=AGGREGATE(109,0,A1:A4)"), Value::Number(80.0));
        assert_eq!(val(&mut s, "=AGGREGATE(9,0,A1:A4)"), Value::Number(100.0));
    }

    #[test]
    fn without_hidden_rows_nothing_changes() {
        let mut s = sheet4();
        assert_eq!(val(&mut s, "=SUBTOTAL(9,A1:A4)"), Value::Number(100.0));
        assert_eq!(val(&mut s, "=SUBTOTAL(109,A1:A4)"), Value::Number(100.0));
        // 隠れ行を飛ばすのは SUBTOTAL の中だけ — 普通の SUM は影響を受けない
        let mut s2 = sheet4();
        s2.row_hidden.insert(1);
        assert_eq!(val(&mut s2, "=SUM(A1:A4)"), Value::Number(100.0));
        assert_eq!(val(&mut s2, "=AVERAGE(A1:A4)"), Value::Number(25.0));
    }
}

/// 構造化参照(2026-08-08 実装。台帳 第3便 [中])。
/// 表オブジェクトの列を見出しの字で引く — Excel の `=SUM(Table1[金額])`
#[cfg(test)]
mod table_ref_tests {
    use super::*;
    use crate::model::{Cell, TableDef};

    /// A1:C4 の表(見出し + 3行)。名前は「売上表」
    fn with_table(totals: bool) -> Sheet {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        let rows = [
            ("A1", "品名"), ("B1", "数量"), ("C1", "金額"),
            ("A2", "筆"), ("B2", "2"), ("C2", "100"),
            ("A3", "紙"), ("B3", "3"), ("C3", "200"),
            ("A4", "机"), ("B4", "1"), ("C4", "900"),
        ];
        for (a1, v) in rows {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        if totals {
            s.set(Pos::parse("A5").unwrap(), Cell::input("合計"));
            s.set(Pos::parse("C5").unwrap(), Cell::input("1200"));
        }
        s.tables.push(TableDef {
            name: "売上表".into(),
            a: Pos::parse("A1").unwrap(),
            b: Pos::parse(if totals { "C5" } else { "C4" }).unwrap(),
            header: true,
            totals,
            ..Default::default()
        });
        s
    }

    fn at(s: &mut Sheet, cell: &str, f: &str) -> Value {
        s.set(Pos::parse(cell).unwrap(), Cell::input(f));
        recalc(s);
        s.value(Pos::parse(cell).unwrap())
    }

    #[test]
    fn a_table_column_can_be_looked_up_by_its_header_text() {
        let mut s = with_table(false);
        assert_eq!(at(&mut s, "E1", "=SUM(売上表[金額])"), Value::Number(1200.0));
        assert_eq!(at(&mut s, "E2", "=AVERAGE(売上表[数量])"), Value::Number(2.0));
        assert_eq!(at(&mut s, "E3", "=COUNTA(売上表[品名])"), Value::Number(3.0));
        // 単独なら先頭の値
        assert_eq!(at(&mut s, "E4", "=売上表[金額]"), Value::Number(100.0));
        // 知らない列・知らない表は #REF!(黙って違う所を読まない)
        assert_eq!(at(&mut s, "E5", "=SUM(売上表[無い列])"), Value::Error("#REF!".into()));
        assert_eq!(at(&mut s, "E6", "=SUM(無い表[金額])"), Value::Error("#REF!".into()));
    }

    #[test]
    fn the_total_row_sits_outside_the_data_body() {
        let mut s = with_table(true);
        // C5 の 1200 は合計行なので二重に数えない
        assert_eq!(at(&mut s, "E1", "=SUM(売上表[金額])"), Value::Number(1200.0));
    }

    #[test]
    fn a_this_row_reference_points_at_the_same_rows_column() {
        let mut s = with_table(false);
        // 表を D 列(税)まで広げて、その中で [@金額] を使う。
        // **名前を省いた形は表の中でだけ効く**(Excel と同じ)
        s.set(Pos::parse("D1").unwrap(), Cell::input("税"));
        s.tables[0].b = Pos::parse("D4").unwrap();
        assert_eq!(at(&mut s, "D3", "=[@金額]*2"), Value::Number(400.0));
        // 表の名前つきなら表の外の同じ行からも引ける
        assert_eq!(at(&mut s, "E3", "=売上表[@数量]"), Value::Number(3.0));
        // 見出しの行では引けない
        assert_eq!(at(&mut s, "E1", "=売上表[@金額]"), Value::Error("#REF!".into()));
        // 表の外で名前を省いたら引けない(どの表か決まらない)
        assert_eq!(at(&mut s, "G3", "=[@金額]"), Value::Error("#REF!".into()));
    }

    #[test]
    fn without_a_table_the_old_reading_applies() {
        // 表オブジェクトが無いシートで [ が出たら式のエラー(黙って0にしない)
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::parse("A1").unwrap(), Cell::input("=SUM(無い表[金額])"));
        recalc(&mut s);
        assert_eq!(s.value(Pos::parse("A1").unwrap()), Value::Error("#REF!".into()));
    }
}

/// LET(2026-08-08 実装。台帳 第3便 [中])。
/// 長い式の途中結果に名前を付けて、読みやすく・二度計算しない
#[cfg(test)]
mod let_tests {
    use super::*;
    use crate::model::Cell;

    fn v(f: &str) -> Value {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        for (a1, x) in [("A1", "10"), ("A2", "20"), ("A3", "30")] {
            s.set(Pos::parse(a1).unwrap(), Cell::input(x));
        }
        s.set(Pos::parse("E1").unwrap(), Cell::input(f));
        recalc(&mut s);
        s.value(Pos::parse("E1").unwrap())
    }

    #[test]
    fn names_can_be_bundled_into_a_formula() {
        assert_eq!(v("=LET(x,5,x*2)"), Value::Number(10.0));
        // 複数の束縛。後の束縛から前の名前が見える
        assert_eq!(v("=LET(x,5,y,x+1,x*y)"), Value::Number(30.0));
        // セルや関数の結果も束ねられる(二度計算しないのが本来の狙い)
        assert_eq!(v("=LET(s,SUM(A1:A3),s/3)"), Value::Number(20.0));
        // 本体が名前1つだけ(次が `)` なので束縛と取り違えない)
        assert_eq!(v("=LET(x,7,x)"), Value::Number(7.0));
        // 入れ子。内側の同じ名前が外側を隠す
        assert_eq!(v("=LET(x,1,LET(x,2,x))"), Value::Number(2.0));
        // 内側を抜けたら外側の名前に戻る
        assert_eq!(v("=LET(x,1,LET(y,2,y)+x)"), Value::Number(3.0));
    }

    #[test]
    fn text_and_booleans_can_be_bundled_too() {
        assert_eq!(v("=LET(t,\"あ\",t&\"い\")"), Value::Text("あい".into()));
        assert_eq!(v("=LET(b,A1>5,IF(b,\"大\",\"小\"))"), Value::Text("大".into()));
    }

    #[test]
    fn a_different_shape_is_refused_honestly() {
        // 名前と値だけで本体が無い
        assert_eq!(v("=LET(x,5)"), Value::Error("#VALUE!".into()));
        // LET の外へ名前は漏れない
        assert_eq!(v("=LET(x,5,x)+x"), Value::Error("#NAME?".into()));
        // 知らない名前は今までどおり #NAME?
        assert_eq!(v("=UNKNOWNNAME+1"), Value::Error("#NAME?".into()));
        // 和文の知らない名前も #NAME?(2026-08-09 に揃った — plugins の関数を
        // `=集計(A1)` と日本語で書けるように、名前の頭を ASCII に限るのをやめた。
        // それまでは字句で落ちて #ERROR! だった)
        assert_eq!(v("=しらない名前+1"), Value::Error("#NAME?".into()));
    }
}

/// TEXTSPLIT / TEXTBEFORE / TEXTAFTER(2026-08-08 実装。台帳 第3便 [中])
#[cfg(test)]
mod text_split_tests {
    use super::*;
    use crate::model::Cell;

    fn v(f: &str) -> Value {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::parse("E1").unwrap(), Cell::input(f));
        recalc(&mut s);
        s.value(Pos::parse("E1").unwrap())
    }

    #[test]
    fn takes_the_parts_before_and_after_a_separator() {
        assert_eq!(v("=TEXTBEFORE(\"甲-乙-丙\",\"-\")"), Value::Text("甲".into()));
        assert_eq!(v("=TEXTAFTER(\"甲-乙-丙\",\"-\")"), Value::Text("乙-丙".into()));
        // 何番目か(2つ目の区切り)
        assert_eq!(v("=TEXTBEFORE(\"甲-乙-丙\",\"-\",2)"), Value::Text("甲-乙".into()));
        assert_eq!(v("=TEXTAFTER(\"甲-乙-丙\",\"-\",2)"), Value::Text("丙".into()));
        // 負は後ろから
        assert_eq!(v("=TEXTAFTER(\"甲-乙-丙\",\"-\",-1)"), Value::Text("丙".into()));
        assert_eq!(v("=TEXTBEFORE(\"甲-乙-丙\",\"-\",-1)"), Value::Text("甲-乙".into()));
        // 見つからなければ #N/A、4つ目を渡せばその値
        assert_eq!(v("=TEXTBEFORE(\"甲乙\",\"-\")"), Value::Error("#N/A".into()));
        assert_eq!(v("=TEXTBEFORE(\"甲乙\",\"-\",1,\"無\")"), Value::Text("無".into()));
        // 区切りが空・0番目は #VALUE!(黙って全部を返さない)
        assert_eq!(v("=TEXTBEFORE(\"甲乙\",\"\")"), Value::Error("#VALUE!".into()));
        assert_eq!(v("=TEXTAFTER(\"甲-乙\",\"-\",0)"), Value::Error("#VALUE!".into()));
    }

    #[test]
    fn textsplit_spills_sideways() {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::parse("A1").unwrap(), Cell::input("=TEXTSPLIT(\"甲,乙,丙\",\",\")"));
        recalc(&mut s);
        assert_eq!(s.value(Pos::parse("A1").unwrap()), Value::Text("甲".into()));
        assert_eq!(s.value(Pos::parse("B1").unwrap()), Value::Text("乙".into()));
        assert_eq!(s.value(Pos::parse("C1").unwrap()), Value::Text("丙".into()));
    }

    #[test]
    fn a_row_separator_splits_vertically_too() {
        let mut s = Sheet { name: "表".into(), ..Default::default() };
        s.set(
            Pos::parse("A1").unwrap(),
            Cell::input("=TEXTSPLIT(\"甲,乙;丙,丁\",\",\",\";\")"),
        );
        recalc(&mut s);
        assert_eq!(s.value(Pos::parse("A1").unwrap()), Value::Text("甲".into()));
        assert_eq!(s.value(Pos::parse("B1").unwrap()), Value::Text("乙".into()));
        assert_eq!(s.value(Pos::parse("A2").unwrap()), Value::Text("丙".into()));
        assert_eq!(s.value(Pos::parse("B2").unwrap()), Value::Text("丁".into()));
        // 区切りが両方とも空なら #VALUE!
        let mut s2 = Sheet { name: "表".into(), ..Default::default() };
        s2.set(Pos::parse("A1").unwrap(), Cell::input("=TEXTSPLIT(\"甲乙\",\"\")"));
        recalc(&mut s2);
        assert_eq!(s2.value(Pos::parse("A1").unwrap()), Value::Error("#VALUE!".into()));
    }
}

/// 串刺し集計(2026-08-08 実装。台帳 第3便 [中])。
/// `=SUM(4月:6月!B2)` — ブックの並び順で2枚の間の全シートを集める
#[cfg(test)]
mod sheet3_tests {
    use super::*;
    use crate::model::Cell;

    /// 表紙 / 4月 / 5月 / 6月 / 別 の5枚(この並び)
    fn book5(formula: &str) -> crate::Book {
        let mut b = crate::Book::new();
        let mut cover = Sheet { name: "表紙".into(), ..Default::default() };
        cover.set(Pos::parse("A1").unwrap(), Cell::input(formula));
        b.sheets[0] = cover;
        for (n, v) in [("4月", "10"), ("5月", "20"), ("6月", "30"), ("別", "999")] {
            let mut s = Sheet { name: n.into(), ..Default::default() };
            s.set(Pos::parse("B2").unwrap(), Cell::input(v));
            b.sheets.push(s);
        }
        b
    }

    fn ans(formula: &str) -> Value {
        let mut b = book5(formula);
        recalc_all(&mut b);
        b.sheets[0].value(Pos::parse("A1").unwrap())
    }

    #[test]
    fn aggregates_across_two_sheets_by_order() {
        // 4月〜6月 = 10+20+30(「別」の 999 は入らない)
        assert_eq!(ans("=SUM(4月:6月!B2)"), Value::Number(60.0));
        assert_eq!(ans("=SUM(4月:5月!B2)"), Value::Number(30.0));
        // 逆順に書いても同じ(Excel と同じ)
        assert_eq!(ans("=SUM(6月:4月!B2)"), Value::Number(60.0));
        // 1枚だけを挟む形
        assert_eq!(ans("=SUM(5月:5月!B2)"), Value::Number(20.0));
        // 平均・個数も同じ並びで効く
        assert_eq!(ans("=AVERAGE(4月:6月!B2)"), Value::Number(20.0));
        assert_eq!(ans("=COUNT(4月:6月!B2)"), Value::Number(3.0));
    }

    #[test]
    fn the_order_holds_even_across_its_own_sheet() {
        // 表紙(1枚目)を含む範囲。自分の A1 は式なので B2 を見る
        let mut b = book5("=SUM(表紙:5月!B2)");
        b.sheets[0].set(Pos::parse("B2").unwrap(), Cell::input("5"));
        recalc_all(&mut b);
        // 表紙 5 + 4月 10 + 5月 20
        assert_eq!(b.sheets[0].value(Pos::parse("A1").unwrap()), Value::Number(35.0));
    }

    #[test]
    fn unknown_names_and_range_shapes() {
        assert_eq!(ans("=SUM(4月:無い月!B2)"), Value::Error("#REF!".into()));
        // 範囲を跨ぐ形(B2:B3)も集められる
        let mut b = book5("=SUM(4月:6月!B2:B3)");
        for (i, v) in [("4月", "1"), ("5月", "2"), ("6月", "3")] {
            let k = b.sheets.iter().position(|s| s.name == i).unwrap();
            b.sheets[k].set(Pos::parse("B3").unwrap(), Cell::input(v));
        }
        recalc_all(&mut b);
        // B2 の 10+20+30 と B3 の 1+2+3
        assert_eq!(b.sheets[0].value(Pos::parse("A1").unwrap()), Value::Number(66.0));
    }
}

#[cfg(test)]
mod new_fn_tests {
    use super::*;
    use crate::model::Cell;

    /// A1 に式を入れて計算し、表示を返す。表は B 列から置く
    fn ev(formula: &str, table: &[(&str, &str)]) -> String {
        let mut s = Sheet::default();
        for (a1, v) in table {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s.set(Pos::parse("A1").unwrap(), Cell::input(formula));
        recalc(&mut s);
        s.get(Pos::parse("A1").unwrap()).unwrap().value.display()
    }

    #[test]
    // **日本語の試験名は家の作法。** ラテン大文字が混じると non_snake_case が鳴る
    #[allow(non_snake_case)]
    fn replace_counts_in_characters() {
        // **バイトで数えると日本語で崩れる**
        assert_eq!(ev("=REPLACE(\"あいうえお\",2,2,\"XY\")", &[]), "あXYえお");
        assert_eq!(ev("=REPLACE(\"abcdef\",1,3,\"Z\")", &[]), "Zdef");
        // 位置が 0 以下は断る(黙って先頭に入れない)
        assert_eq!(ev("=REPLACE(\"abc\",0,1,\"Z\")", &[]), "#VALUE!");
    }

    #[test]
    // **日本語の試験名は家の作法。** ラテン大文字が混じると non_snake_case が鳴る
    #[allow(non_snake_case)]
    fn xmatch_searches_backwards_and_refuses_approximate() {
        let t = [("B1", "い"), ("B2", "ろ"), ("B3", "い")];
        assert_eq!(ev("=XMATCH(\"い\",B1:B3)", &t), "1");
        assert_eq!(ev("=XMATCH(\"い\",B1:B3,0,-1)", &t), "3", "後ろから探せていない");
        assert_eq!(ev("=XMATCH(\"は\",B1:B3)", &t), "#N/A");
        // 近似(1)は**黙って合わせず**断る
        assert_eq!(ev("=XMATCH(\"い\",B1:B3,1)", &t), "#VALUE!");
    }

    #[test]
    fn database_functions_filter_by_a_criteria_table() {
        // 表: B1:C4(見出し + 3行)、条件表: E1:E2
        let t = [
            ("B1", "品"), ("C1", "額"),
            ("B2", "机"), ("C2", "100"),
            ("B3", "椅子"), ("C3", "200"),
            ("B4", "机"), ("C4", "300"),
            ("E1", "品"), ("E2", "机"),
        ];
        assert_eq!(ev("=DSUM(B1:C4,\"額\",E1:E2)", &t), "400");
        assert_eq!(ev("=DAVERAGE(B1:C4,\"額\",E1:E2)", &t), "200");
        assert_eq!(ev("=DCOUNT(B1:C4,\"額\",E1:E2)", &t), "2");
        assert_eq!(ev("=DMAX(B1:C4,\"額\",E1:E2)", &t), "300");
        // DGET は**1件でなければ返さない**(2件あるので #NUM!)
        assert_eq!(ev("=DGET(B1:C4,\"額\",E1:E2)", &t), "#NUM!");
        // 列は番号でも指せる
        assert_eq!(ev("=DSUM(B1:C4,2,E1:E2)", &t), "400");
    }

    #[test]
    fn the_extended_spill_returns_a_sequence() {
        let mut s = Sheet::default();
        for (a1, v) in [("B1", "3"), ("B2", "1"), ("B3", "2"),
                        ("C1", "さ"), ("C2", "あ"), ("C3", "い")] {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        // SORTBY: C 列を B 列の順で並べ替える
        s.set(Pos::parse("E1").unwrap(), Cell::input("=SORTBY(C1:C3,B1:B3)"));
        recalc(&mut s);
        let g = |a1: &str| s.get(Pos::parse(a1).unwrap()).map(|c| c.value.display()).unwrap_or_default();
        assert_eq!((g("E1"), g("E2"), g("E3")), ("あ".into(), "い".into(), "さ".into()));

        // TAKE / DROP
        let mut s2 = Sheet::default();
        for (i, v) in ["1", "2", "3", "4"].iter().enumerate() {
            s2.set(Pos::new(i as u32, 1), Cell::input(v));
        }
        s2.set(Pos::parse("D1").unwrap(), Cell::input("=TAKE(B1:B4,2)"));
        s2.set(Pos::parse("E1").unwrap(), Cell::input("=DROP(B1:B4,-3)"));
        recalc(&mut s2);
        let h = |a1: &str| s2.get(Pos::parse(a1).unwrap()).map(|c| c.value.display()).unwrap_or_default();
        assert_eq!((h("D1"), h("D2")), ("1".into(), "2".into()), "TAKE が先頭2つでない");
        assert_eq!(h("E1"), "1", "DROP(-3) が先頭1つでない");

        // VSTACK は縦に積む
        let mut s3 = Sheet::default();
        s3.set(Pos::parse("B1").unwrap(), Cell::input("1"));
        s3.set(Pos::parse("C1").unwrap(), Cell::input("2"));
        s3.set(Pos::parse("E1").unwrap(), Cell::input("=VSTACK(B1,C1)"));
        recalc(&mut s3);
        let k = |a1: &str| s3.get(Pos::parse(a1).unwrap()).map(|c| c.value.display()).unwrap_or_default();
        assert_eq!((k("E1"), k("E2")), ("1".into(), "2".into()), "縦に積めていない");
    }
}

#[cfg(test)]
mod cell_filename_tests {
    use super::*;
    use crate::Book;

    /// Excel の `CELL("filename")` は **`径路[ファイル名]シート名`**。
    /// 実物はここから `]` の後ろを取ってシート名にする
    #[test]
    fn lays_out_the_path_file_name_and_sheet_name() {
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(
            cell_filename(&format!("{sep}帳票{sep}売上.xlsx"), "4月"),
            format!("{sep}帳票{sep}[売上.xlsx]4月")
        );
        // 径路の無い名前だけでも壊れない
        assert_eq!(cell_filename("売上.xlsx", "4月"), "[売上.xlsx]4月");
    }

    /// **保存していないブックは空文字**(Excel と同じ)。
    /// `#NAME?` にはしない — 実装できる物を誤りにして回避させない
    #[test]
    fn before_saving_it_is_an_empty_string() {
        assert_eq!(cell_filename("", "Sheet1"), "");
    }

    /// 常套句がそのまま通ること。`=MID(CELL("filename",A1),
    /// FIND("]",CELL("filename",A1))+1, 31)` でシート名が取れる
    #[test]
    fn the_idiom_for_extracting_a_sheet_name_works() {
        let mut b = Book::new();
        b.path = format!("{s}home{s}dev{s}売上.xlsx", s = std::path::MAIN_SEPARATOR);
        b.sheets[0].name = "四月".into();
        let s = &mut b.sheets[0];
        s.set(Pos::parse("A1").unwrap(), crate::Cell::input("=CELL(\"filename\",A1)"));
        s.set(
            Pos::parse("A2").unwrap(),
            crate::Cell::input(
                "=MID(CELL(\"filename\",A1), FIND(\"]\",CELL(\"filename\",A1))+1, 31)",
            ),
        );
        recalc_book(&mut b, 0);
        let g = |a1: &str| {
            b.sheets[0].get(Pos::parse(a1).unwrap()).map(|c| c.value.display()).unwrap_or_default()
        };
        assert!(g("A1").ends_with("[売上.xlsx]四月"), "いま {}", g("A1"));
        assert_eq!(g("A2"), "四月", "シート名が取り出せない");
    }

    /// 種別が違えば今までどおり `#NAME?`。**できない物をできる顔で
    /// 答えない** — "address" に 0 を返すほうが黙って壊れる
    #[test]
    fn nothing_but_the_file_name_answers_yet() {
        let mut b = Book::new();
        b.path = "/tmp/x.xlsx".into();
        b.sheets[0].set(Pos::parse("A1").unwrap(), crate::Cell::input("=CELL(\"address\",A1)"));
        recalc_book(&mut b, 0);
        assert_eq!(
            b.sheets[0].get(Pos::parse("A1").unwrap()).map(|c| c.value.display()).unwrap_or_default(),
            "#NAME?"
        );
    }

    /// 1枚だけの再計算(ブックが無い)では径路を知らない = 空文字
    #[test]
    fn recalculation_without_a_workbook_yields_an_empty_string() {
        let mut s = Sheet::new("Sheet1");
        s.set(Pos::parse("A1").unwrap(), crate::Cell::input("=CELL(\"filename\")"));
        recalc(&mut s);
        assert_eq!(
            s.get(Pos::parse("A1").unwrap()).map(|c| c.value.display()).unwrap_or_default(),
            ""
        );
    }
}

/// **シート以外の表でも式が動く**(SEKKEI「エンジンの統一」2段目)。
///
/// 式の計算は `Sheet` ではなく [`Grid`](crate::grid::Grid)(値を引ける表)を
/// 受けるようになった。ここでは**この試験だけの表**を書いて渡し、
/// `Sheet` でなくても同じ答えが出ることを確かめる。
///
/// 製品の中で `Grid` を実装しているのは、いまのところ `Sheet` だけ。
/// 文書の表の計算は `ops::table` がシートに写して行う(道を1本にするため)
#[cfg(test)]
#[allow(non_snake_case)]
mod calculation_works_on_tables_outside_a_sheet {
    use crate::calc::eval_in;
    use crate::grid::Grid;
    use crate::model::{Pos, TableDef, Value};

    /// 九九の表。**升目の値を式で答える**だけの、いちばん小さい表。
    /// 中身を持たなくても計算に載ることを示す
    struct TimesTable {
        defs: Vec<TableDef>,
    }

    impl TimesTable {
        fn new() -> TimesTable {
            // 1行目を見出しにして、構造化参照も通ることを見る
            TimesTable {
                defs: vec![TableDef {
                    name: "九九".into(),
                    a: Pos::new(0, 0),
                    b: Pos::new(9, 9),
                    header: true,
                    ..Default::default()
                }],
            }
        }
    }

    impl Grid for TimesTable {
        fn name(&self) -> &str {
            "九九"
        }
        fn value(&self, p: Pos) -> Value {
            // 見出しの行は列の名前、それ以外は掛け算の答え
            if p.row == 0 {
                return Value::Text(format!("{}の段", p.col + 1));
            }
            Value::Number(((p.row + 1) * (p.col + 1)) as f64)
        }
        fn tables(&self) -> &[TableDef] {
            &self.defs
        }
    }

    /// 番地の参照と範囲。B2 は 2×2 = 4
    #[test]
    fn lookup_by_address() {
        let g = TimesTable::new();
        assert_eq!(eval_in(&g, Pos::new(0, 0), "=B2"), Value::Number(4.0));
        // B2:B4 = 4, 6, 8
        assert_eq!(eval_in(&g, Pos::new(0, 0), "=SUM(B2:B4)"), Value::Number(18.0));
    }

    /// **構造化参照。** 表の名前と見出しの字で列を引く
    #[test]
    fn lookup_by_table_name_and_header() {
        let g = TimesTable::new();
        // 「3の段」= C 列の本体(2行目〜10行目)= 6,9,12,…,30
        assert_eq!(eval_in(&g, Pos::new(0, 0), "=SUM(九九[3の段])"), Value::Number(162.0));
        assert_eq!(eval_in(&g, Pos::new(0, 0), "=MAX(九九[3の段])"), Value::Number(30.0));
    }

    /// 既定のまま置いた物は既定どおり — ふりがなも隠した行も無い
    #[test]
    fn what_is_not_held_keeps_its_default() {
        let g = TimesTable::new();
        assert!(!g.any_row_hidden());
        assert!(!g.row_hidden(3));
        assert_eq!(g.phonetic(Pos::new(1, 1)), None);
    }
}
