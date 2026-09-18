//! リボンの en の**語の表** — 構造は持たない(2026-08-31 の作り替え)。
//! 骨組みは ribbon.rs の1本だけで、ここには (英語の札, 語) の対しか無い。
//! 言語の表は実行時に ribbon::localized が骨組みへ差し込んで組む —
//! 骨組みが言語ごとにずれる余地は、作りから消してある。
//!
//! このファイルは手で書かない:
//!
//! ```text
//! python3 ui/gen_ribbon_locale.py en > face/src/ribbon_en.rs
//! ```
//!
//! 対訳は vendor/web-apps のロケール(本家の語)。本家に無いこちらの
//! ボタンは gen_ribbon_locale.py の OVERRIDES 表で訳す。

pub const WORDS: &[(&str, &str)] = &[
];

/// 同じ札で働きが違うボタンの語(id か icon で引く)
pub const WORDS_BY_ID: &[(&str, &str)] = &[
];

