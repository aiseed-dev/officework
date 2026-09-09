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
    let font = crate::font_for_book(b)?;
    // **列幅の物差し。** ブックの標準の書体の数字1文字の幅(画素)
    let mdw = suuji_haba(b);
    let sheets: Vec<(&book::Sheet, paper::Paper, paper::grid::PrintSetup)> = b
        .sheets
        .iter()
        .filter(|s| !s.hidden)
        .map(|s| (s, paper_of(s), setup_of_mdw(s, b.date1904, mdw, default_pt_of(b))))
        .collect();
    if sheets.is_empty() {
        return Err("刷るシートがありません(全部隠れています)".into());
    }
    // **セルが名指しした書体を集めて渡します**(2026-08-31。Fable の指摘2)。
    // 前は1本しか埋められず、明朝のセルもゴシックのセルも同じ書体で出ていました。
    // 機械に無い書体は置き替えます(`for_document` が系統を保ちます)
    let mut fonts: Vec<(String, Vec<u8>)> = vec![("".into(), font.to_vec())];
    let mut mita: std::collections::BTreeSet<String> = Default::default();
    for s in &b.sheets {
        let namae = s.cells.values().filter_map(|c| c.fmt.font.clone()).chain(
            s.rich_runs.values().flatten().filter_map(|r| r.font.clone()),
        );
        for na in namae {
            if !mita.insert(na.clone()) {
                continue;
            }
            if let Ok((fam, _)) = kumihan::font::for_document(Some(&na)) {
                if let Ok(d) = kumihan::font::load(fam) {
                    fonts.push((na.clone(), d));
                }
            }
            // **半角だけ別の書体で組む書体**(ＭＳ Ｐ明朝など)は、もう1本
            // 足します(2026-08-31 発注者)。名前の後ろに印を付けて分けます —
            // 紙の側がその印で引きます
            if let Some(fam) = kumihan::font::hankaku_no_kae(&na) {
                if let Ok(d) = kumihan::font::load(fam) {
                    fonts.push((format!("{na}{}", paper::grid::HANKAKU_SIRUSI), d));
                }
            }
        }
    }
    let mut cut = 0;
    kumihan::atomic::save(to, |f| {
        cut = paper::grid::book_to_pdf_fonts(&sheets, &fonts, f)?;
        Ok(())
    })?;
    Ok(cut)
}

/// シートの紙の設定。**シートごとに効きます**(1冊に縦と横が混ざってよい)
pub(crate) fn paper_of(s: &book::Sheet) -> paper::Paper {
    // 用紙の番号は Excel の決め(ECMA-376 第1部 `ST_PaperSize`)。9 = A4。
    //
    // **12 と 13 は JIS の B です**(2026-09-09)。仕様書は 12 を
    // 「B4 (JIS) 257mm × 364mm」、13 を「B5 (JIS) 182mm × 257mm」と書いて
    // います。前は ISO の B4(250×353)と B5(176×250)を入れていたので、
    // 岐阜労働局の様式(001823803.xlsx)で Excel の 1032×729pt に対して
    // 1001×709pt の紙になり、1枚に入る行が減って紙が増えていました
    let (w, h) = match s.paper_size.unwrap_or(9) {
        8 => (297.0, 420.0),
        11 => (148.0, 210.0),
        12 => (257.0, 364.0),
        13 => (182.0, 257.0),
        1 => (215.9, 279.4),
        5 => (215.9, 355.6),
        _ => (210.0, 297.0),
    };
    let (w, h) = if s.landscape { (h, w) } else { (w, h) };
    // **上下も渡します**(2026-08-30)。前は左だけ渡していて、頁割りが
    // それを上下にも使っていました。上下と左右が違う設定の表では、
    // 2頁目からの本文の頭がずれます
    let (l, _r, t, b) = s.margins_mm.unwrap_or((20.0, 20.0, 20.0, 20.0));
    paper::Paper { width_mm: w, height_mm: h, margin_mm: l, top_mm: t, bottom_mm: b }
}


/// 数字1文字の幅つき([`suuji_haba`] が出します)。`default_pt` は大きさを
/// 言っていないセルの字の大きさ(ブックの標準の書体。[`default_pt_of`])
pub(crate) fn setup_of_mdw(
    s: &book::Sheet,
    date1904: bool,
    mdw_pt: f32,
    default_pt: f32,
) -> paper::grid::PrintSetup {
    paper::grid::PrintSetup {
        areas: s.print_areas.clone(),
        margins_mm: s.margins_mm,
        date1904,
        mdw_pt,
        default_pt,
    }
}

/// **大きさを言っていないセルの字の大きさ(pt)。** ブックの標準の書体の
/// 大きさ。無ければ `book::DEFAULT_CELL_PT`
pub(crate) fn default_pt_of(b: &book::Book) -> f32 {
    b.default_font.as_ref().map(|(_, pt)| *pt).filter(|p| *p > 0.0).unwrap_or(book::DEFAULT_CELL_PT)
}

/// **そのブックの数字1文字の幅(画素)。** 標準の書体から測ります。
/// 分からなければ 0(紙の側が 7 に落とします)
pub(crate) fn suuji_haba(b: &book::Book) -> f32 {
    crate::suuji_haba_of(b)
}


#[cfg(test)]
mod tests {
    /// **用紙の番号 12・13 は JIS の B。**
    ///
    /// ECMA-376 は 12 を「B4 (JIS) 257mm × 364mm」、13 を
    /// 「B5 (JIS) 182mm × 257mm」と決めています。ISO の B4(250×353)を
    /// 入れていたので、岐阜労働局の様式(001823803.xlsx)で Excel より
    /// 紙が 3cm 小さく、1枚に入る行が減っていました(2026-09-09)。
    #[test]
    fn paper_twelve_and_thirteen_are_the_jis_b_sizes() {
        let mut s = book::Sheet::default();
        for (no, w, h) in [(12u32, 257.0, 364.0), (13, 182.0, 257.0), (9, 210.0, 297.0)] {
            s.paper_size = Some(no);
            let p = super::paper_of(&s);
            assert!((p.width_mm - w).abs() < 0.1 && (p.height_mm - h).abs() < 0.1,
                    "用紙 {no} は {w}×{h}mm のはずが {}×{}mm", p.width_mm, p.height_mm);
        }
    }

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
