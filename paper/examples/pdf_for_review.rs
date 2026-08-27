//! 発注者に見ていただく PDF を作る。
//!
//!     cargo run -p paper --example pdf_for_review
fn main() {
    let src = "\
= 令和8年4月分 売上報告書
:author: 総務課

*四月の売上*は前月比で 12% 増えました。内訳は下の表のとおりです。

== 明細

|===
|品名 |数量 |単価 |金額

|ボールペン |12 |150 |1,800
|ノート |24 |120 |2,880
|クリアファイル |50 |45 |2,250
|===

NOTE: 単価は税抜きです。註記の帯が紙に出るかを見ます。

== 備考

- 単価は税抜きです
- クリアファイルは _まとめ買い_ の割引が入っています

日本語の行組みは JIS X 4051 の禁則で折ります。行頭に「。」や「、」が
来ないよう追い出し、行末に開き括弧(が残らないようにします。欧文の
word も語の途中では切りません。
";
    let doc = kumihan::adoc::parse(src).expect("読めない");
    let (sheet, page, bytes) = paper::doc_to_sheet(&doc, None).expect("組めない");
    let pp = paper::Paper { width_mm: page.w_mm, height_mm: page.h_mm, margin_mm: page.left_mm };

    let mut new = Vec::new();
    let lost = paper::pdfw::sheet_to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut new))
        .expect("新しい道");
    std::fs::write("test/out/見本-新しい書き手.pdf", &new).unwrap();

    let mut old = Vec::new();
    paper::to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut old)).expect("いまの道");
    std::fs::write("test/out/見本-いまの書き手.pdf", &old).unwrap();

    println!("新しい書き手  {:>10} バイト", new.len());
    println!("いまの書き手  {:>10} バイト", old.len());
    println!("用紙 {:.0}x{:.0}mm 余白 {:.0}mm", page.w_mm, page.h_mm, page.left_mm);
    if !lost.is_empty() {
        println!("載らない物: {lost:?}");
    }
}
