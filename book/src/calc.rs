//! 式の評価と再計算。
//!
//! 範囲は「Euro-Office ができている範囲」で十分という方針のうち、
//! **事務で実際に使うところ**に絞る: 四則・比較・括弧・セル参照・範囲・
//! よく使う関数(SUM/AVERAGE/COUNT/COUNTA/MIN/MAX/IF/ROUND/ABS/AND/OR/NOT/
//! CONCATENATE)。
//!
//! **マクロは実装しない。** これは機能不足ではなく設計判断で、
//! 「開く=実行」という攻撃経路を最初から持たないため(migration-kit DESIGN.md §5)。
//!
//! 循環参照は検出してエラーにする(黙って0を返さない)。
//!
//! **1枚が 5,909 行になったので持ち場で割った**(2026-08-10)。中身は
//! 1行も変えていない — 置き場だけ:
//!
//! - [`parse`] 字句と構文。式を読んで、その場で畳む
//! - [`funcs`] 関数の library。`SUM` から動的配列まで
//! - [`run`] 再計算の駆動。依存・順序・スピル・循環の検出・UDF
//! - [`df`] `=df(...)` の列の定義。再計算の前に表の列を埋める
//!
//! 外から見える名前はここに集めてある。呼ぶ側は `kumihan::calc::…` のまま。

mod df;
pub mod funcs;
mod parse;
mod run;

// **外から呼ばれる名前はここに集める。** どのモジュールに置いたかを
// 呼ぶ側に知らせない — 置き場を変えても `kumihan::calc::…` は変わらない
pub use funcs::date_serial;
pub use funcs::{date_serial_at, excel_epoch};
// 日付の粒でまとめる(タイムライン)に要る。通し番号 → 年月日
pub use funcs::civil_from_days;
// crate の中だけで使う物は、外へは出さない(割る前と同じ見え方)。
// EXCEL_EPOCH_DAYS は 1904 対応(6acc71a)で excel_epoch(date1904) に
// 置き換わり、再輸出の使い手が消えた
pub(crate) use funcs::{era_of, weekday0};
pub use df::is_df_formula;
pub use parse::cell_filename;
pub use run::{
    deps, eval_in, eval_once, eval_py_call, is_py_formula, is_udf_name, py_cell_stamp, recalc, recalc_all,
    recalc_book, set_udf_names, PyArg,
};

#[cfg(test)]
mod tests;
