//! sheet — xlsx の読み書きと表計算の核。UI非依存。
//!
//! **マクロは実装しない。** 機能不足ではなく設計判断:
//! 文書の中に特権実行コードが同居する形(VBA)をやめ、
//! 「開く=実行」という攻撃経路を最初から持たない
//! (aiseed-migration-kit DESIGN.md §5 と同じ思想)。

/// ブック ⇄ AsciiDoc(ブックの正本を .adoc にする)
pub mod adoc;
/// ブックの見た目(テンプレート)
pub mod booktmpl;
/// 表のセルの**見え**を決める(画面と紙が同じ答えを得るための1本)
pub mod look;
/// セルの中の書き方(AsciiDoc)
pub mod cellmark;
pub mod styles;
pub mod tabledesign;
pub mod theme;
pub mod xlsx;

// **升目の模型と式の計算は kumihan にあります**(2026-08-26)。ここに
// 置いているのは今までの呼び方を残すための再輸出で、呼ぶ側を
// `kumihan::book` / `kumihan::calc` に向け替えたら消します。
pub use kumihan::book as model;
pub use kumihan::{calc, datetime_names, grid};

pub use kumihan::book::{Book, Cell, Pos, Sheet, Value};
pub use kumihan::calc::funcs::civil_from_days;
pub use kumihan::calc::{recalc, recalc_all, recalc_book};
