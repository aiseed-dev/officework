//! **1つのファイルに文書を何枚も**(2026-08-19 発注者)。
//!
//! 同時に送る請求書の原稿をまとめて置く使い方です。切れ目は `= 題` で、
//! 新しい印は足していません。

use kumihan::adoc;

fn title(d: &kumihan::Document) -> String {
    d.props.title.clone()
}

fn body(d: &kumihan::Document) -> String {
    d.body_text()
}

#[test]
fn 請求書を3枚まとめて読める() {
    let src = "= 請求書 山田商店\n\n金額 12,000 円\n\n\
              = 請求書 鈴木工業\n\n金額 8,400 円\n\n\
              = 請求書 佐藤商会\n\n金額 3,300 円\n";
    let docs = adoc::parse_many(src).expect("読めない");
    assert_eq!(docs.len(), 3, "文書の数が合わない");
    assert_eq!(title(&docs[0]), "請求書 山田商店");
    assert_eq!(title(&docs[2]), "請求書 佐藤商会");
    assert!(body(&docs[1]).contains("8,400"), "中身が混ざった: {:?}", body(&docs[1]));
    assert!(!body(&docs[1]).contains("12,000"), "前の文書の中身が混ざった");
}

#[test]
fn 一枚だけならいままでどおり() {
    let docs = adoc::parse_many("= 報告書\n\n本文です。\n").expect("読めない");
    assert_eq!(docs.len(), 1);
    assert_eq!(title(&docs[0]), "報告書");
    // 題の無い文書も1枚として読める
    let docs = adoc::parse_many("本文だけ。\n").expect("読めない");
    assert_eq!(docs.len(), 1);
}

/// **塊の中の `= ` では切らない。** ここを間違えると文書が壊れる
#[test]
fn 塊の中の等号では切らない() {
    let src = "= 手引き\n\n\
              書き方の例です。\n\n\
              ....\n= これは中身\n本文\n....\n\n\
              = 次の文書\n\n二枚目。\n";
    let docs = adoc::parse_many(src).expect("読めない");
    assert_eq!(docs.len(), 2, "塊の中で切ってしまった");
    assert_eq!(title(&docs[0]), "手引き");
    assert_eq!(title(&docs[1]), "次の文書");
}

/// 表の中の `= ` でも切らない(式のセルは `=` で始まる)
#[test]
fn 表の中の等号では切らない() {
    let src = "= 明細\n\n\
              |===\n|品名 |金額\n\n|机 |=B2*2\n|===\n\n\
              = 次\n\n二枚目。\n";
    let docs = adoc::parse_many(src).expect("読めない");
    assert_eq!(docs.len(), 2, "表の中で切ってしまった");
}

#[test]
fn 書いて読むと戻る() {
    let src = "= 甲\n\nあ。\n\n= 乙\n\nい。\n";
    let docs = adoc::parse_many(src).expect("読めない");
    let out = adoc::write_many(&docs);
    assert!(out.starts_with("[discrete]"), "切れ目の印が無い:\n{out}");
    let back = adoc::parse_many(&out).expect("読み直せない");
    assert_eq!(back.len(), 2);
    assert_eq!(title(&back[0]), "甲");
    assert_eq!(title(&back[1]), "乙");
    assert!(body(&back[1]).contains("い。"));
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
    let docs = adoc::parse_many("= 甲\n\nあ。\n").expect("読めない");
    let mut two = docs.clone();
    two.push(adoc::parse("名前のない本文。\n").expect("読めない"));
    let out = adoc::write_many(&two);
    assert!(out.contains("= 文書 2"), "名前が付いていない:\n{out}");
    let back = adoc::parse_many(&out).expect("読み直せない");
    assert_eq!(back.len(), 2, "名前が無いと切れ目が分からなくなる");
}

/// **本家が警告を出さない字で書く**(2026-08-19 発注者「警告が出ないように
/// 考えろ」)。`= 題` を並べると「部には節が要る」と言われるので、
/// 節ではない見出し(`[discrete]`)にする
#[test]
fn 書く字に切れ目の印が付く() {
    let docs = adoc::parse_many("= 甲\n\nあ。\n\n= 乙\n\nい。\n").expect("読めない");
    assert_eq!(docs.len(), 2);
    let out = adoc::write_many(&docs);
    assert!(out.contains("[discrete]\n= 甲"), "切れ目の印が無い:\n{out}");
    assert!(out.contains("[discrete]\n= 乙"), "切れ目の印が無い:\n{out}");
    // doctype は要らない(印だけで足りる)
    assert!(!out.contains(":doctype:"), "要らない属性が付いた:\n{out}");
    // 読み直せる
    let back = adoc::parse_many(&out).expect("読み直せない");
    assert_eq!(back.len(), 2);
    assert_eq!(title(&back[0]), "甲");
    assert_eq!(title(&back[1]), "乙");
}

/// 印は本文に漏れない
#[test]
fn 切れ目の印は本文に残らない() {
    let src = "[discrete]\n= 甲\n\nあ。\n\n[discrete]\n= 乙\n\nい。\n";
    let docs = adoc::parse_many(src).expect("読めない");
    assert_eq!(docs.len(), 2);
    for d in &docs {
        assert!(!body(d).contains("discrete"), "印が本文に残った: {:?}", body(d));
    }
    assert_eq!(title(&docs[1]), "乙");
}

/// 印の無い `= 題` でも切れる(手で書いたファイル)
#[test]
fn 印が無くても切れる() {
    let docs = adoc::parse_many("= 甲\n\nあ。\n\n= 乙\n\nい。\n").expect("読めない");
    assert_eq!(docs.len(), 2, "印が無いと切れない");
}

/// **表だけの節と、文章の節が1つのファイルに混ざる**(2026-08-24 発注者
/// 「シートも一つのセクションとみなせば追加できるでしょう」)。
///
/// 綴りはこの形です — 新しい形式を作らず、AsciiDoc の節に乗せます。
/// 本家(vendor/asciidoctor)にも警告なしで通ることを確かめてあります。
#[test]
fn 文章の節と表の節が混ざっても往復する() {
    let src = ":doctype: book\n\n[discrete]\n= 報告書\n\n本文です。\n\n\
[discrete]\n= 売上台帳\n\n|===\n|品名 |数\n\n|ボールペン |12\n|===\n";
    let docs = kumihan::adoc::parse_many(src).expect("読めない");
    assert_eq!(docs.len(), 2, "節が2つに分かれない");
    assert_eq!(docs[0].tables().count(), 0, "文章の節に表がある");
    assert_eq!(docs[1].tables().count(), 1, "表の節に表がない");

    let back = kumihan::adoc::write_many(&docs);
    assert!(back.contains("ボールペン"), "表が消えた");
    assert!(back.contains("本文です。"), "本文が消えた");
    assert_eq!(
        kumihan::adoc::parse_many(&back).unwrap().len(),
        2,
        "往復で節の数が変わった"
    );
}
