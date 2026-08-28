//! **PDF を作る — アプリが動いていなくても。**
//!
//! 2026-08-27 発注者「エンジンで pdf をつくるところまで」「共通エンジンに
//! 組み込む」「マクロで使えるようにして」。
//!
//! これまで PDF を作れたのは動いているアプリだけでした。[`crate::Host`] の
//! `to_pdf` を実装しているのが `calc` だけだったからです。組む所も紙にする
//! 所もエンジンに在ったのに、**その2つを繋ぐ道がアプリの中にしか無い**、
//! という形でした。
//!
//! ここはその道を操作の言葉の側に置いた物です。マクロからも、Python の口
//! からも、アプリの画面からも、**同じ1本**を通ります。
//!
//! # openpyxl にも python-docx にも無い
//!
//! 本家は PDF を作れません(別のソフトを呼ぶしかありません)。こちらは
//! 組版のエンジンを持っているので、**そのまま紙にできます**。

use std::path::Path;

/// **文書を PDF にする。**
///
/// テンプレートを渡すと見た目を合成してから組みます(渡さなければ同梱の
/// 既定)。行の高さも段組みもエンジンの1つを見るので、**画面で開いた紙面と
/// 同じ物**が出ます。
pub fn doc(
    d: &kumihan::Document,
    theme: Option<&kumihan::theme::Theme>,
    to: &Path,
) -> Result<(), String> {
    // **書けてから置き替えます。** 途中で落ちても元の PDF が残ります
    kumihan::atomic::save(to, |f| paper::doc_to_pdf(d, theme, f))
}

/// **ブックを1つの PDF にする。** 頁番号はブック通しです。
///
/// 見えないシート(hidden)は刷りません — 画面と同じです。
/// 返りは切れた列の数の合計で、0 でなければ紙からはみ出しています。
pub fn book(b: &book::Book, to: &Path) -> Result<u32, String> {
    let font = crate::try_font_data()?;
    let sheets: Vec<(&book::Sheet, paper::Paper, paper::grid::PrintSetup)> = b
        .sheets
        .iter()
        .filter(|s| !s.hidden)
        .map(|s| (s, paper_of(s), setup_of(s, b.date1904)))
        .collect();
    if sheets.is_empty() {
        return Err("刷るシートがありません(全部隠れています)".into());
    }
    let mut cut = 0;
    kumihan::atomic::save(to, |f| {
        cut = paper::grid::book_to_pdf(&sheets, font, f)?;
        Ok(())
    })?;
    Ok(cut)
}

/// シートの紙の設定。**シートごとに効きます**(1冊に縦と横が混ざってよい)
fn paper_of(s: &book::Sheet) -> paper::Paper {
    // 用紙の番号は Excel の決め。9 = A4
    let (w, h) = match s.paper_size.unwrap_or(9) {
        8 => (297.0, 420.0),
        11 => (148.0, 210.0),
        12 => (250.0, 353.0),
        13 => (176.0, 250.0),
        1 => (215.9, 279.4),
        5 => (215.9, 355.6),
        _ => (210.0, 297.0),
    };
    let (w, h) = if s.landscape { (h, w) } else { (w, h) };
    let margin = s.margins_mm.map(|(l, _, _, _)| l).unwrap_or(20.0);
    paper::Paper { width_mm: w, height_mm: h, margin_mm: margin }
}

fn setup_of(s: &book::Sheet, date1904: bool) -> paper::grid::PrintSetup {
    paper::grid::PrintSetup {
        areas: s.print_areas.clone(),
        margins_mm: s.margins_mm,
        date1904,
    }
}

#[cfg(test)]
mod tests {
    /// **アプリが動いていなくても文書が PDF になる。**
    #[test]
    fn a_document_becomes_a_pdf_without_the_app() {
        let d = kumihan::adoc::parse("= 請求書\n\n合計は 1,200 円です。\n").expect("読めない");
        let to = std::env::temp_dir().join("ops_pdf_doc.pdf");
        super::doc(&d, None, &to).expect("PDF が出ない");
        let bytes = std::fs::read(&to).expect("置けていない");
        assert!(bytes.starts_with(b"%PDF"), "PDF になっていない");
        let _ = std::fs::remove_file(&to);
    }

    /// **ブックも1つの PDF になる。** openpyxl には無い所です
    #[test]
    fn a_workbook_becomes_one_pdf() {
        let mut b = book::Book::new();
        b.sheets[0].name = "見積".into();
        for (at, v) in [("A1", "品名"), ("B1", "金額"), ("A2", "ボールペン"), ("B2", "1200")] {
            let p = book::Pos::parse(at).expect("番地");
            b.sheets[0].set(p, book::Cell {
                value: book::Value::Text(v.into()),
                ..Default::default()
            });
        }
        let to = std::env::temp_dir().join("ops_pdf_book.pdf");
        let cut = super::book(&b, &to).expect("PDF が出ない");
        assert_eq!(cut, 0, "紙からはみ出した列がある");
        assert!(std::fs::read(&to).expect("置けていない").starts_with(b"%PDF"));
        let _ = std::fs::remove_file(&to);
    }

    /// 全部隠れていたら断る(黙って空の PDF を出さない)
    #[test]
    fn a_book_with_everything_hidden_is_refused() {
        let mut b = book::Book::new();
        b.sheets[0].hidden = true;
        let to = std::env::temp_dir().join("ops_pdf_hidden.pdf");
        assert!(super::book(&b, &to).is_err(), "隠れているのに刷った");
    }
}
