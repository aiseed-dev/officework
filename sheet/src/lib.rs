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

// **升目の模型と式の計算は kumihan にあります**(2026-08-26。SEKKEI
// 「エンジンは3つに分ける」)。ここでは再輸出しません — この crate から
// 引けると、xlsx のエンジンが模型を持っているように見えるためです。
// 呼ぶ側は `kumihan::book` と `kumihan::calc` を直に使ってください。
