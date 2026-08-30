//! **紙面を PNG にする — アプリが動いていなくても。**
//!
//! 2026-08-29 発注者「PNG を書き出す入り口は共通ライブラリーに置いてほしい」。
//!
//! ここは [`crate::pdf`] と対です。同じ紙面から、片方は PDF に、片方は
//! 画素になります。**組む所は1本**なので、絵と紙が食い違いません。
//!
//! # 置き場について
//!
//! 絵にする実体は [`paper::e`] にあります(層の表で「写し先」)。
//! `book` には置けません — `book` は依存を1つも持たない決めで、絵にするには
//! 組み上がった紙面と描画のライブラリーが要るからです。
//!
//! ここに置いたのは、PDF と同じ入り口の形にするためです。Python からは
//! `wb.save("x.png")` のように拡張子で振り分けます。
//!
//! # 頁が複数あるとき
//!
//! PNG は1枚の絵しか持てないので、頁ごとにファイルを分けます。
//! **1枚目は渡された名前のまま**、2枚目からは名前に `-2`・`-3` を足します。
//! `out.png` を渡して3頁あれば、`out.png`・`out-2.png`・`out-3.png` です。

use std::path::{Path, PathBuf};

/// 何も言われなかったときの細かさ(dpi)。
///
/// 150 dpi なら A4 が 1240×1754 画素です。画面で見るにも、資料に貼るにも
/// 足ります。印刷に回すなら 300 を渡してください。
pub const DPI: f32 = 150.0;

/// 受ける細かさの上限。600 dpi が印刷の実用の上なので、その倍まで
pub const DPI_MAX: f32 = 1200.0;

/// dpi を「1mm 何画素か」に直す
fn bai(dpi: f32) -> f32 {
    dpi / 25.4
}

/// k 頁目のファイル名。**1頁目は渡された名前のまま**です
fn na(to: &Path, k: usize) -> PathBuf {
    if k == 0 {
        return to.to_path_buf();
    }
    let stem = to.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = to.extension().map(|s| s.to_string_lossy().into_owned());
    let mut f = format!("{stem}-{}", k + 1);
    if let Some(e) = ext {
        f.push('.');
        f.push_str(&e);
    }
    to.with_file_name(f)
}

/// 紙面の並びを PNG の並びにして置く。返りは書いた枚数
fn kaku(
    leaves: &[(paper::pdfw::Leaf, (f32, f32))],
    font: &[u8],
    to: &Path,
    dpi: f32,
) -> Result<usize, String> {
    if leaves.is_empty() {
        return Err("刷る物がありません".into());
    }
    // **上も止めます。** A4 を 1200 dpi で描くと 9921×13984 画素、
    // 画素の並びだけで 555MB になります。桁を打ち間違えたときに
    // 機械を止めるより、断る方が親切です
    // **NaN も弾きます。** `dpi > 0.0` の否定で書くと NaN が通りますが、
    // 比べ方が読みにくいので、範囲の中にあるかを直に書きます
    if !(0.0 < dpi && dpi <= DPI_MAX) {
        return Err(format!(
            "細かさ(dpi)は 0 より大きく {DPI_MAX} までにしてください: {dpi}"
        ));
    }
    let b = bai(dpi);
    for (k, (leaf, (w, h))) in leaves.iter().enumerate() {
        let e = paper::e::egaku_with(leaf, *w, *h, b, Some(font));
        let png = e.png()?;
        // **書けてから置き替えます。** 途中で落ちても元の絵が残ります
        kumihan::atomic::save(&na(to, k), |mut f| {
            std::io::Write::write_all(&mut f, &png).map_err(|e| e.to_string())
        })?;
    }
    Ok(leaves.len())
}

/// **文書を PNG にする。** 1頁1枚です。
///
/// テンプレートを渡すと見た目を合成してから組みます(渡さなければ同梱の
/// 既定)。`dpi` は細かさで、[`DPI`] が既定です。
pub fn doc(
    d: &kumihan::Document,
    theme: Option<&kumihan::theme::Theme>,
    to: &Path,
    dpi: f32,
) -> Result<usize, String> {
    let (sheet, page, font) = paper::doc_to_sheet(d, theme)?;
    let ookisa = (page.w_mm, page.h_mm);
    let leaves: Vec<_> =
        paper::doc_leaves(&sheet, page).into_iter().map(|l| (l, ookisa)).collect();
    kaku(&leaves, &font, to, dpi)
}

/// **ブックを PNG にする。** シートの頁ごとに1枚です。
///
/// 見えないシート(hidden)は刷りません — [`crate::pdf::book`] と同じです。
/// 紙の大きさはシートごとに効くので、1冊に縦と横が混ざっていて構いません。
pub fn book(b: &book::Book, to: &Path, dpi: f32) -> Result<usize, String> {
    let font = crate::try_font_data()?;
    let mut leaves = Vec::new();
    for s in b.sheets.iter().filter(|s| !s.hidden) {
        let p = crate::pdf::paper_of(s);
        let setup = crate::pdf::setup_of(s, b.date1904);
        for leaf in paper::grid::sheet_leaves(s, p, &setup)? {
            leaves.push((leaf, (p.width_mm, p.height_mm)));
        }
    }
    if leaves.is_empty() {
        return Err("刷るシートがありません(全部隠れています)".into());
    }
    kaku(&leaves, font, to, dpi)
}

#[cfg(test)]
mod tests {
    /// PNG の頭の8バイト(これで PNG かどうかが分かります)
    const SHIRUSHI: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

    fn okiba(na: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(na)
    }

    /// **アプリが動いていなくても文書が PNG になる。**
    #[test]
    fn a_document_becomes_a_png_without_the_app() {
        let d = kumihan::adoc::parse("= 請求書\n\n合計は 1,200 円です。\n").expect("読めない");
        let to = okiba("ops_png_doc.png");
        let n = super::doc(&d, None, &to, super::DPI).expect("PNG が出ない");
        assert_eq!(n, 1, "1頁のはずが {n} 枚");
        let bytes = std::fs::read(&to).expect("置けていない");
        assert!(bytes.starts_with(SHIRUSHI), "PNG になっていない");
        let _ = std::fs::remove_file(&to);
    }

    /// **ブックも PNG になる。**
    #[test]
    fn a_workbook_becomes_a_png() {
        let mut b = book::Book::new();
        b.sheets[0].name = "見積".into();
        for (at, v) in [("A1", "品名"), ("B1", "金額"), ("A2", "ボールペン"), ("B2", "1200")] {
            let p = book::Pos::parse(at).expect("番地");
            b.sheets[0]
                .set(p, book::Cell { value: book::Value::Text(v.into()), ..Default::default() });
        }
        let to = okiba("ops_png_book.png");
        let n = super::book(&b, &to, super::DPI).expect("PNG が出ない");
        assert_eq!(n, 1, "1頁のはずが {n} 枚");
        assert!(std::fs::read(&to).expect("置けていない").starts_with(SHIRUSHI));
        let _ = std::fs::remove_file(&to);
    }

    /// **頁が増えたら名前に番号が付く。** 1枚目は渡した名前のまま
    #[test]
    fn more_pages_get_numbered_names() {
        let mut honbun = String::from("= 長い文書\n\n");
        for i in 1..=400 {
            honbun.push_str(&format!("{i} 行目の本文です。ここは頁を溢れさせるための字です。\n\n"));
        }
        let d = kumihan::adoc::parse(&honbun).expect("読めない");
        let to = okiba("ops_png_many.png");
        let n = super::doc(&d, None, &to, 50.0).expect("PNG が出ない");
        assert!(n > 1, "1枚しか出ていない(頁が溢れていない)");
        assert!(std::fs::read(&to).expect("1枚目が無い").starts_with(SHIRUSHI));
        let futatsume = okiba("ops_png_many-2.png");
        assert!(std::fs::read(&futatsume).expect("2枚目が無い").starts_with(SHIRUSHI));
        for k in 1..=n {
            let f = if k == 1 { to.clone() } else { okiba(&format!("ops_png_many-{k}.png")) };
            let _ = std::fs::remove_file(f);
        }
    }

    /// 全部隠れていたら断る(黙って空の絵を出さない)
    #[test]
    fn a_book_with_everything_hidden_is_refused() {
        let mut b = book::Book::new();
        b.sheets[0].hidden = true;
        let to = okiba("ops_png_hidden.png");
        assert!(super::book(&b, &to, super::DPI).is_err(), "隠れているのに刷った");
    }

    /// 細かさが桁違いなら断る。**下も上も**見ます
    #[test]
    fn a_bad_dpi_is_refused() {
        let d = kumihan::adoc::parse("= 題\n\n本文。\n").expect("読めない");
        let to = okiba("ops_png_dpi.png");
        for warui in [0.0, -100.0, f32::NAN, super::DPI_MAX + 1.0, 100_000.0] {
            assert!(super::doc(&d, None, &to, warui).is_err(), "{warui} を受けてしまった");
        }
    }
}
