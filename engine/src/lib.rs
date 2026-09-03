//! kumihan — 組版エンジンの核。Japanese-Office(仮称)の心臓。
//!
//! やること: 文書(段落の列)を、実フォントの字幅で行に組み、
//! 置かれた文字の座標(紙面)を返す。UIにもPDFにも依存しない。
//! 画面も紙も、この紙面を別の面に写すだけ — だから画面と印刷が一致する。
//!
//! v0 の範囲: 横組み。JIS X 4051 のうち
//!   - 行頭禁則(。、」などが行頭に来ない — 追い出しで直す)
//!   - 行末禁則(「(『 などが行末に残らない)
//!   - 欧文の語中で改行しない(語ごと次行へ)
//!
//! 縦書き・ルビ・均等割付・ぶら下げは K4(モデルはそれを妨げない形にする)。
//!
//! **1枚が 3,767 行になったので持ち場で割った**(2026-08-11)。中身は
//! 1行も変えていない — 置き場だけ。仕切りは元から
//! `// ---------- 文書モデル ----------` と書いてあったので、そこで切った:
//!
//! - [`doc`] 文書の模型。段落・ラン・表・文書、そして組み上がった紙面
//! - [`layout`] 組版。字幅・禁則・行分割・段組み・頁割り
//!
//! 型は全部ここから見える(`use kumihan::Paragraph` は変わらない)。
//!
//! **セルの模型と式の計算もここにあります**(2026-08-26。SEKKEI
//! 「エンジンは3つに分ける」)。前は `sheet` に置いていましたが、
//! `sheet` は xlsx の交換を受け持つエンジンなので、模型と計算の芯は
//! 交換の形式から離してこちらへ移しました。
//!
//! - [`book`] セルの模型。ブック・シート・セル・値・表の定義
//! - [`calc`] 式の計算。**計算の道は1本**(2026-08-19 に測った決め)
//! - [`grid`] 式が表に求める面5つ

pub mod adoc;
/// 文書をブロックの番号で読み書きする(エージェント・Python・MCP の共通の語彙)
pub mod blocks;
pub(crate) mod inline;
pub mod atomic;
/// ブック ⇄ AsciiDoc(ブックの正本を .sheet.adoc にする)
pub mod book_adoc;
/// シートの、格子に載らない意味を表で持つ(`[.names]` などの印)
pub mod book_meta;
/// `.sheet.adoc` の往復で落ちる持ち物を数える
pub mod holes;
/// ブックの見た目 — テンプレート(`テンプレート.adoc`)
pub mod booktmpl;
/// セルの中の書き方(AsciiDoc)
pub mod cellmark;
pub mod distill;
/// 表のセルの**見え**を決める(画面と紙が同じ答えを得るための1本)
pub mod look;
/// 表のデザイン(見出しの帯・合計行・縞々)
pub mod tabledesign;
pub mod theme;
pub mod edit;
pub mod font;
pub mod html;
/// 意味だけの本文 + テンプレート → HTML + CSS(Web・アプリ・帳票の土台)
pub mod html_write;
/// 雛形にデータを流し込む(帳票の芯)
pub mod fill;
/// 数式を組む(LaTeX → PNG)。typst + mitex
pub mod suushiki;

/// **OMML(docx と xlsx の数式)を LaTeX に読む。**
/// 組むのは [`suushiki`] なので、読んだ物はそのまま紙に出せます
pub mod omml;
pub use edit::Editor;

mod doc;
mod layout;

pub use doc::*;
pub use layout::*;

#[cfg(test)]
mod holes_tests;
#[cfg(test)]
mod tests;
