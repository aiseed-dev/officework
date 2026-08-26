//! xlsx(SpreadsheetML)の読み書き。
//! 読めないものは黙って落とさず `Report` に積む(ooxml と同じ作法)。
//!
//! **1枚が 6,670 行になったので持ち場で割った**(2026-08-10)。中身は
//! 1行も変えていない — 置き場だけ:
//!
//! - [`read`] xlsx を読む。原本の形をそのまま模型へ
//! - [`write`] xlsx を書く。**触っていない部品は原本から持ち越す**
//! - [`styles`] `styles.xml` の出し入れ
//! - [`theme`] `theme1.xml` の出し入れ(色の組そのものは模型の側)
//!
//! 外から見える名前(`read` `write` `write_with` `to_template` `Report`)は
//! ここに集めてある。呼ぶ側は `sheet::xlsx::…` のまま変わらない。

mod read;
/// 壊れた zip から拾う(開いて修復)
mod repair;
/// `styles.xml` — セルの書式を索引で持つ仕組み
pub mod styles;
/// `theme1.xml` の読み書き
pub mod theme;
mod write;

pub use read::{read, Report};
pub use repair::{salvage, Salvage};
pub use write::{to_template, write, write_with};

#[cfg(test)]
mod tests;
