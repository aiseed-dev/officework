//! 言語の注入(set_language)の検査。
//!
//! **別ファイル(統合試験)なのは過程を分けるため** — OnceLock は
//! プロセスに1回なので、単体試験の中では他の試験が先に language() を
//! 走らせると順番で結果が変わる。統合試験は別のバイナリ = 別の過程で
//! 走るから、まっさらな状態から順番を固定できる。

#[test]
fn 注いだ言語が効き_あとから変えられない() {
    // 知らない札は断る(黙って ja に落とさない)
    assert!(!lang::i18n::set_language("xx"), "知らない札を受けた");
    // まだ language() が走っていないので、注げる
    assert!(lang::i18n::set_language("en"), "最初の注入が効かない");
    assert_eq!(lang::i18n::language(), "en", "注いだ言語で答えない");
    // 一度固まったら、あとからは効かない(効いたふりもしない)
    assert!(!lang::i18n::set_language("de"), "固まった後の注入を受けたことにした");
    assert_eq!(lang::i18n::language(), "en");
    // 訳の表もその言語で引ける
    assert_eq!(lang::i18n::tr("開く"), "Open");
}
