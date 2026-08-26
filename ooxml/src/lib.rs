//! docx ⇄ kumihan の文書モデル。
//!
//! 方針(SEKKEI.md「互換は書式の境界で守る」):
//!   - エンジンは継がない。**書式(docx)だけを読み書きする**
//!   - **全部は実装しない。読めないものは読めないと言う** —
//!     解釈できなかった要素は捨てずに `Report` に積んで返す。
//!     黙って落とすのが一番悪い(利用者は失われたことに気づけない)
//!
//! v0 の範囲: 本文の段落・ラン・文字サイズ・改行、そして**表**(w:tbl、
//! セル結合 gridSpan/vMerge を含む)。
//! 表は日本の事務様式の本体なので、v0 から入れる(実物8件すべてに表があった)。
//! 画像・ヘッダ/フッタ・スタイル定義は**未対応として報告する**。
//!
//! **1枚が 4,837 行になったので持ち場で割った**(2026-08-11)。中身は
//! 1行も変えていない — 置き場だけ。仕切りは元から
//! `// ---------- 書く ----------` と書いてあったので、そこで切った:
//!
//! - [`read`] docx を読む。読めなかったものは `Report` に積む
//! - [`write`] docx を書く。**原本の部品は据え置く**
//!
//! 型は `kumihan` のもの。ここは書式の層だけを持つ。

pub mod crypt;

mod read;
/// `theme1.xml` — 役ごとの書体(見出しはゴシック、本文は明朝)
pub mod theme;
mod write;

pub use read::{ink_anchor_run, ink_anchor_xml, parse_document_with, parse_document_xml, read, Report};
pub use write::{write, write_document_parts, write_document_xml, write_with, write_with_theme};

#[cfg(test)]
mod tests;
