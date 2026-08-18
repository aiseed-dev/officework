//! **1つのファイルに文書を何枚も**(2026-08-19 発注者)。
//!
//! 同時に送る請求書の原稿をまとめて置く使い方です。切れ目は `= 題` で、
//! 新しい印は足していません。

use kumihan::adoc;

fn 題(d: &kumihan::Document) -> String {
    d.props.title.clone()
}

fn 本文(d: &kumihan::Document) -> String {
    d.body_text()
}

#[test]
fn 請求書を3枚まとめて読める() {
    let src = ":doctype: book\n\n\
              = 請求書 山田商店\n\n金額 12,000 円\n\n\
              = 請求書 鈴木工業\n\n金額 8,400 円\n\n\
              = 請求書 佐藤商会\n\n金額 3,300 円\n";
    let docs = adoc::parse_many(src).expect("読めない");
    assert_eq!(docs.len(), 3, "文書の数が合わない");
    assert_eq!(題(&docs[0]), "請求書 山田商店");
    assert_eq!(題(&docs[2]), "請求書 佐藤商会");
    assert!(本文(&docs[1]).contains("8,400"), "中身が混ざった: {:?}", 本文(&docs[1]));
    assert!(!本文(&docs[1]).contains("12,000"), "前の文書の中身が混ざった");
}

#[test]
fn 一枚だけならいままでどおり() {
    let docs = adoc::parse_many("= 報告書\n\n本文です。\n").expect("読めない");
    assert_eq!(docs.len(), 1);
    assert_eq!(題(&docs[0]), "報告書");
    // 題の無い文書も1枚として読める
    let docs = adoc::parse_many("本文だけ。\n").expect("読めない");
    assert_eq!(docs.len(), 1);
}

/// **塊の中の `= ` では切らない。** ここを間違えると文書が壊れる
#[test]
fn 塊の中の等号では切らない() {
    let src = ":doctype: book\n\n\
              = 手引き\n\n\
              書き方の例です。\n\n\
              ....\n= これは中身\n本文\n....\n\n\
              = 次の文書\n\n二枚目。\n";
    let docs = adoc::parse_many(src).expect("読めない");
    assert_eq!(docs.len(), 2, "塊の中で切ってしまった");
    assert_eq!(題(&docs[0]), "手引き");
    assert_eq!(題(&docs[1]), "次の文書");
}

/// 表の中の `= ` でも切らない(式のセルは `=` で始まる)
#[test]
fn 表の中の等号では切らない() {
    let src = ":doctype: book\n\n\
              = 明細\n\n\
              |===\n|品名 |金額\n\n|机 |=B2*2\n|===\n\n\
              = 次\n\n二枚目。\n";
    let docs = adoc::parse_many(src).expect("読めない");
    assert_eq!(docs.len(), 2, "表の中で切ってしまった");
}

#[test]
fn 書いて読むと戻る() {
    let src = ":doctype: book\n\n= 甲\n\nあ。\n\n= 乙\n\nい。\n";
    let docs = adoc::parse_many(src).expect("読めない");
    let out = adoc::write_many(&docs);
    assert!(out.starts_with(":doctype: book"), "本家の印が無い:\n{out}");
    let 戻り = adoc::parse_many(&out).expect("読み直せない");
    assert_eq!(戻り.len(), 2);
    assert_eq!(題(&戻り[0]), "甲");
    assert_eq!(題(&戻り[1]), "乙");
    assert!(本文(&戻り[1]).contains("い。"));
}

/// 1枚のときは `:doctype: book` を付けない(いままでの字と同じ)
#[test]
fn 一枚のときは印を付けない() {
    let docs = adoc::parse_many("= 報告書\n\n本文です。\n").expect("読めない");
    let out = adoc::write_many(&docs);
    assert!(!out.contains(":doctype: book"), "1枚なのに印が付いた:\n{out}");
    assert_eq!(out, adoc::write(&docs[0]), "1枚のときは write と同じ字であること");
}

/// 名前の無い文書には番号で名前を付ける(タブに出すため)
#[test]
fn 名前が無ければ番号を付ける() {
    let docs = adoc::parse_many(":doctype: book\n\n= 甲\n\nあ。\n").expect("読めない");
    let mut two = docs.clone();
    two.push(adoc::parse("名前のない本文。\n").expect("読めない"));
    let out = adoc::write_many(&two);
    assert!(out.contains("= 文書 2"), "名前が付いていない:\n{out}");
    let 戻り = adoc::parse_many(&out).expect("読み直せない");
    assert_eq!(戻り.len(), 2, "名前が無いと切れ目が分からなくなる");
}
