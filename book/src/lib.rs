//! book — **表計算のブックの模型と、式の計算。**
//!
//! ここは**どのエンジンの持ち物でもありません**(2026-08-26 発注者
//! 「計算のメインの置き場が、どのエンジンの持ち物でもない共通ライブラリ」)。
//!
//! 3つのエンジン(`sheet` = xlsx、`ooxml` = docx、`kumihan` = adoc)は
//! どれも、開く・保存の境でこの模型と行き来します。計算の芯をどれか1つの
//! エンジンに置くと、他の2つがそのエンジンを引きずります。
//!
//! - [`types`] 型そのもの。`Pos` `Value` `Cell` `Sheet` `Book` と付随の実装
//! - [`fmt`] 表示形式。`#,##0` や `yyyy年m月d日` を値に当てる
//! - [`refs`] 式の中の参照を動かす。行の挿入・R1C1・オフセット
//! - [`ops`] シートの操作。行列の出し入れ・並べ替え
//! - [`calc`] 式の計算。**計算の道は1本**(2026-08-19 に測った決め)
//! - [`grid`] 式が表に求める面5つ
//!
//! 型は全部ここから見えます(`use book::Cell`)。
//!
//! **交換の形式は知りません。** xlsx の索引も adoc の綴りもここには
//! 出てきません。それは各エンジンの持ち場です。

/// 式の計算。表に求める面は [`grid::Grid`] の5つだけ
pub mod calc;
/// 暦の名前(月・曜日)を言語ごとに引く
pub mod datetime_names;
/// 値を引ける表 — 式の計算が表に求める面はこれだけ
pub mod grid;
/// どの言語で組むかを決める、1本の規則(環境変数 → 設定 → OS → en)
pub mod lang;
/// テーマ色の組と、番号+明るさの加減から色を解く
pub mod theme;

mod fmt;
mod ops;
/// OOXML の図形の定義(187種)。生成物 — 手で直さない
mod preset_gen;
/// その定義を点の列にする解釈器
mod preset_spec;
mod refs;
mod types;

pub use preset_spec::{spec_has, spec_names, spec_polys, Poly};

pub use fmt::{format_value, hyoujun_no_kouho};
pub use refs::{
    formula_from_r1c1, formula_to_r1c1, map_refs, offset_refs, rename_refs_in,
    rename_sheet_refs, shift_refs, MapRef,
};
pub use types::*;

mod boolops;
pub use boolops::{combine, flatten, outline, to_points, BoolOp};

/// **`=` で始まる字が式か、セルの中の見出しか。**
///
/// 見分けるのは*空白*です。式は `=` の後ろに空白を置きません
/// (`=SUM(A1)` は式、`= 見出し` は見出し)。
///
/// **決めはこの1つだけ**です。打ち込みも `.adoc` の読み書きも同じ字を
/// 同じ意味に取ります — 2箇所に書くと必ずずれます(2026-08-19 に踏んだ)。
pub fn is_formula_cell(s: &str) -> bool {
    let t = s.trim_start();
    t.len() > 1 && t.starts_with('=') && !t.trim_start_matches('=').starts_with(' ')
}

#[cfg(test)]
mod tests;
