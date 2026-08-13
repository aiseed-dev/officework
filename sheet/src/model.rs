//! 表計算のモデル — セル・シート・ブック。UI非依存。
//!
//! セルの中身は「入力されたもの」と「計算された値」を分けて持つ。
//! 式は入力の一種であり、値はその結果。xlsx も同じ持ち方をしている
//! (`<f>` が式、`<v>` が最後に計算された値)。
//!
//! **1枚が 3,961 行になったので持ち場で割った**(2026-08-10)。中身は
//! 1行も変えていない — 置き場だけ:
//!
//! - [`types`] 型そのもの。`Pos` `Value` `Cell` `Sheet` `Book` と付随の実装
//! - [`fmt`] 表示形式。`#,##0` や `yyyy年m月d日` を値に当てる
//! - [`refs`] 式の中の参照を動かす。行の挿入・R1C1・オフセット
//! - [`ops`] シートの操作。行列の出し入れ・並べ替え
//!
//! 型は全部ここから見える(`use sheet::model::Cell` は変わらない)。

mod fmt;
mod ops;
mod refs;
mod types;

pub use fmt::format_value;
pub use refs::{
    formula_from_r1c1, formula_to_r1c1, map_refs, offset_refs, rename_refs_in,
    rename_sheet_refs, shift_refs, MapRef,
};
pub use types::*;

mod boolops;
pub use boolops::{combine, flatten, outline, to_points, BoolOp};

#[cfg(test)]
mod tests;
