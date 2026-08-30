//! **ブック ⇄ AsciiDoc**(SEKKEI「エンジンの統一」4段目、
//! docs/sekkei/calc.ja.adoc「calc の adoc 形式」2026-08-18 発注者)。
//!
//! ブックの正本を `.adoc` にするための読み書きです。xlsx は受け渡しの形式に
//! なります(writer と docx の関係と同じ)。
//!
//! *1つのシート = 1つの表。* 表の題(`.売上台帳`)がシート名になります。
//!
//! ....
//! .売上台帳
//! |===
//! |月 |品名 |数量 |金額
//!
//! |4月 |ボールペン |12 |=売上台帳[@数量]*150
//! |===
//! ....
//!
//! *式はそのまま字で書きます。* 本家は `=` で始まるセルを印として食いません。
//! 読むときに計算し直すので、値は持ちません — **式が正本**です。
//!
//! *表の読み書きは書き直しません。* AsciiDoc の表の綴りは
//! [`kumihan::adoc`] に1つあるので、ブックを文書に写して渡します
//! (docs/sekkei/calc.ja.adoc「置き場」)。
//!
//! *持てない物は数えて返します*(writer と同じ作法)。書式・列幅・図形・
//! ピボットは adoc の表に居場所が無いので、[`parse`] と [`write_report`] が
//! 何を落としたかを日本語で返します。見た目はテンプレートの持ち場です。

use book::calc::recalc_all;
use book::{Book, Cell, Pos, Sheet, TableDef, Value};
use crate::{Block, Cellbox, Document, Table, VMerge};

/// ブックを adoc の字にする。
pub fn write(book: &Book) -> String {
    crate::adoc::write(&to_doc(book))
}

/// 書くときに**落ちる物**を数える。日本語で1件1行。
///
/// **2026-08-26 にほとんど無くなりました。** 見た目はテンプレート
/// (`.tmpl.adoc`)、格子に載らない意味は役割の印を付けた表が持ちます。
/// 何が往復するかは [`crate::holes`] が機械で数えています。
pub fn write_report(book: &Book) -> Vec<String> {
    let mut out = Vec::new();
    // **画像の実体は adoc に入りません**(binary です)。隣のファイルに
    // 出すので、保存する側が [`crate::book_meta::image_files`] を書くこと。
    // 書き忘れると絵だけ消えるので、数えて言います
    let images: usize = book.sheets.iter().map(|s| s.images.len() + s.images_new.len()).sum();
    if images > 0 {
        out.push(format!("画像 {images} 件(実体は隣のファイルに出します)"));
    }
    let pivot = book.pivots.len();
    if pivot > 0 {
        out.push(format!("ピボットテーブル {pivot} 件(まだ adoc に置けません)"));
    }
    out
}

/// adoc の字をブックにする。**読めなかった物は数えて返す。**
///
/// 表でない段落(見出し・本文)は、ブックに居場所が無いので落とし、
/// 何を落としたかを2つ目の返り値に入れます。
pub fn parse(src: &str) -> Result<(Book, Vec<String>), String> {
    let (doc, mut report) = crate::adoc::parse_full(src)?;

    let mut book = Book::new();
    book.sheets.clear();
    let mut paras = 0;
    for b in &doc.blocks {
        match b {
            // 役割の印の付いた表はシートではありません。後で入れます
            Block::Table(t) if t.role.is_some() => {}
            Block::Table(t) => book.sheets.push(to_sheet(t, book.sheets.len())),
            // 見出しや本文は表計算のブックに居場所が無い
            Block::Para(p) => {
                if !p.runs.iter().all(|r| r.text.trim().is_empty()) {
                    paras += 1;
                }
            }
        }
    }
    if paras > 0 {
        report.push(format!("表の外の段落 {paras} 件(ブックには表しか入りません)"));
    }
    // 1枚も無いブックは作らない(calc は必ず1枚から始まる)
    if book.sheets.is_empty() {
        book.sheets.push(Sheet::new("Sheet1"));
    }
    take_book_settings(&doc, &mut book);
    crate::book_meta::take_all(&doc, &mut book.sheets);
    // **値は持たないので、読んだ所で計算する。** 式が正本
    recalc_all(&mut book);
    Ok((book, report))
}

// ---------- ブック → 文書 ----------

fn to_doc(book: &Book) -> Document {
    let mut d = Document::default();
    put_book_settings(&mut d, book);
    for s in &book.sheets {
        d.blocks.push(Block::Table(to_table(s)));
        // **格子に載らない意味**(名前の定義・入力規則など)は役割の印を
        // 付けた表にします。シートの表の直後に置くので、人が読んでも
        // どのシートの物か分かります
        for t in crate::book_meta::tables_of(s) {
            d.blocks.push(Block::Table(t));
        }
    }
    d
}

/// **ブックの設定を文書の属性に置く。**
///
/// AsciiDoc の文書属性(`:calc-manual: true`)です。表に入らない、
/// ブック1つに1つしかない設定はここが居場所になります。
///
/// 名前を英語にしてあるのは、**属性の名前は書式の一部**だからです
/// (表の題や列の見出しと違って、画面に出る字ではありません)。
fn put_book_settings(d: &mut Document, book: &Book) {
    d.props.title = book.props.title.clone();

    let mut put = |k: &str, v: String| {
        if !v.is_empty() {
            d.attrs.push((k.to_string(), v));
        }
    };
    // **AsciiDoc の決まった名前に寄せます**(`:author:` `:keywords:`)。
    // 普通の AsciiDoc の道具で読める形にするためです。作成者が複数の
    // ときは AsciiDoc の作法どおり `;` で区切ります
    put("author", book.props.creators.join("; "));
    put("subject", book.props.subject.clone());
    put("keywords", book.props.keywords.clone());
    put("description", book.props.description.clone());
    if book.calc_manual {
        put("calc-manual", "true".into());
    }
    if let Some((n, eps)) = book.calc_iter {
        put("calc-iterate", format!("{n},{eps}"));
    }
    if book.read_only_rec {
        put("read-only-recommended", "true".into());
    }
    // ブックの構造の保護(2026-08-30)
    if book.lock_structure {
        put("lock-structure", "true".into());
    }
    if book.date1904 {
        put("date-1904", "true".into());
    }
}

/// 文書の属性からブックの設定を読む。**知らない属性は触りません**
/// (利用者が自分の覚え書きを書くことがあります)。
fn take_book_settings(d: &Document, book: &mut Book) {
    book.props.title = d.props.title.clone();

    for (k, v) in &d.attrs {
        let yes = v.eq_ignore_ascii_case("true");
        match k.as_str() {
            "author" => {
                book.props.creators = v
                    .split(';')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            }
            "subject" => book.props.subject = v.clone(),
            "keywords" => book.props.keywords = v.clone(),
            "description" => book.props.description = v.clone(),
            "calc-manual" => book.calc_manual = yes,
            "read-only-recommended" => book.read_only_rec = yes,
            "lock-structure" => book.lock_structure = yes,
            "date-1904" => book.date1904 = yes,
            "calc-iterate" => {
                if let Some((n, eps)) = v.split_once(',') {
                    if let (Ok(n), Ok(eps)) = (n.trim().parse(), eps.trim().parse()) {
                        book.calc_iter = Some((n, eps));
                    }
                }
            }
            _ => {}
        }
    }
}

/// シートを文書の表にする。
fn to_table(s: &Sheet) -> Table {
    // **結合は中身が無くても書きます。** `extent` は「中身のあるセル」の
    // 範囲なので、空の升目どうしの結合はその外に出ます。書かないと
    // 開き直したとき結合が消えます(2026-08-26 に測って気づいた)
    let (mut rows, mut cols) = s.extent();
    for (a, b) in &s.merges {
        rows = rows.max(b.row + 1).max(a.row + 1);
        cols = cols.max(b.col + 1).max(a.col + 1);
    }
    let mut t = Table {
        title: (!s.name.is_empty()).then(|| s.name.clone()),
        header_row: has_header(s),
        ..Default::default()
    };
    for r in 0..rows {
        let mut row = Vec::new();
        let mut c = 0;
        while c < cols {
            let p = Pos::new(r, c);
            match merge_at(s, p) {
                // 結合の左の列 — 1つのセルが幅のぶんを占める
                Some((a, b)) if c == a.col => {
                    let w = (b.col - a.col + 1) as u8;
                    let v = if b.row > a.row {
                        if r == a.row {
                            VMerge::Start
                        } else {
                            VMerge::Continue
                        }
                    } else {
                        VMerge::None
                    };
                    // 中身は左上のセルだけが持つ
                    let text = if r == a.row { cell_text(s, a) } else { String::new() };
                    row.push(cellbox(&text, w, v));
                    c += w as u32;
                }
                // 結合に呑まれた所 — 左の col_span が覆うので出さない
                Some(_) => c += 1,
                None => {
                    row.push(cellbox(&cell_text(s, p), 1, VMerge::None));
                    c += 1;
                }
            }
        }
        t.rows.push(row);
    }
    t
}

/// このセルを含む結合があれば、その範囲。
fn merge_at(s: &Sheet, p: Pos) -> Option<(Pos, Pos)> {
    s.merges
        .iter()
        .find(|(a, b)| p.row >= a.row && p.row <= b.row && p.col >= a.col && p.col <= b.col)
        .copied()
}

/// セルに書く字。**式があれば式**(`=` を付けて)、無ければ値。
fn cell_text(s: &Sheet, p: Pos) -> String {
    match s.get(p) {
        Some(c) => match &c.formula {
            Some(f) => format!("={f}"),
            None => c.value.display(),
        },
        None => String::new(),
    }
}

/// 升1つ。**セルの中の改行は「行の折り」で、段落の切れ目ではありません**
/// (2026-08-31 発注者)。
///
/// `Document::plain` は改行で段落に割ります。表計算のセルでそれをすると、
/// 「区　分 / Type」の1升が2段落になり、間が段落の空きだけ開きます。
/// 1段落のまま持ち、書くときに AsciiDoc の改行(行末の ` +`)にします。
fn cellbox(text: &str, col_span: u8, v_merge: VMerge) -> Cellbox {
    Cellbox {
        paragraphs: vec![crate::Paragraph {
            runs: vec![crate::Run {
                text: text.to_string(),
                size_pt: None,
                font: None,
                fmt: Default::default(),
            }],
            line_spacing: 1.0,
            ..Default::default()
        }],
        col_span,
        v_merge,
        ..Default::default()
    }
}

/// 1行目が見出しか。表の定義が「見出しあり」で1行目から始まっていれば真。
fn has_header(s: &Sheet) -> bool {
    s.tables.iter().any(|t| t.header && t.a.row == 0)
}

// ---------- 文書 → シート ----------

fn to_sheet(t: &Table, nth: usize) -> Sheet {
    let name = match &t.title {
        Some(n) if !n.trim().is_empty() => n.clone(),
        _ => format!("Sheet{}", nth + 1),
    };
    let mut s = Sheet::new(&name);
    let text = t.text_rows();
    for (r, row) in text.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if cell.trim().is_empty() {
                continue;
            }
            // `=` で始まるだけの字(`= 見出し`)は `Cell::input` が字として
            // 受けます(2026-08-19 から決めが1つになりました)
            let v = if keep_as_text(cell) {
                Cell { formula: None, value: Value::Text(cell.trim().to_string()), fmt: Default::default() }
            } else {
                Cell::input(cell)
            };
            s.set(Pos::new(r as u32, c as u32), v);
        }
    }
    s.merges = merges_of(t);
    // 題と見出しがあれば、そのまま表の定義にする —
    // これで `=SUM(売上台帳[金額])` が宣言なしで書ける
    let cols = text.iter().map(|r| r.len()).max().unwrap_or(0);
    if t.title.is_some() && cols > 0 && !text.is_empty() {
        s.tables.push(TableDef {
            name: name.clone(),
            a: Pos::new(0, 0),
            b: Pos::new(text.len() as u32 - 1, cols as u32 - 1),
            header: t.header_row,
            ..Default::default()
        });
    }
    s
}

/// **数に読んではいけない字か。**
///
/// adoc の表は中身が全部字なので、読むときに数と字を見分けます
/// (docs/sekkei/calc.ja.adoc「失う物」)。困るのは*頭に 0 の付いた番号*で、
/// `001` を数にすると `1` になり、**伝票番号や郵便番号が黙って変わります**。
/// 実物 16 冊で測ったところ、5 冊がこれに当たりました(2026-08-19)。
///
/// 見分け方は「0 の次が数字なら番号」です。`0.5` や `0` は数のままにします。
fn keep_as_text(s: &str) -> bool {
    let t = s.trim();
    // 頭が 0 で、次も数字 → 番号(0001・007)
    t.len() > 1 && t.starts_with('0') && t.as_bytes().get(1).is_some_and(|c| c.is_ascii_digit())
}

/// 表の結合(`col_span` と `v_merge`)を、ブックの結合の範囲に戻す。
fn merges_of(t: &Table) -> Vec<(Pos, Pos)> {
    let mut out: Vec<(Pos, Pos)> = Vec::new();
    for (r, row) in t.rows.iter().enumerate() {
        let mut c = 0u32;
        for cell in row {
            let w = cell.span() as u32;
            match cell.v_merge {
                // 続きは、上の結合を1行ぶん伸ばす
                VMerge::Continue => {
                    if let Some(m) = out.iter_mut().find(|(a, b)| a.col == c && b.row + 1 == r as u32) {
                        m.1.row = r as u32;
                    }
                }
                VMerge::Start => out.push((Pos::new(r as u32, c), Pos::new(r as u32, c + w - 1))),
                VMerge::None => {
                    if w > 1 {
                        out.push((Pos::new(r as u32, c), Pos::new(r as u32, c + w - 1)));
                    }
                }
            }
            c += w;
        }
    }
    out
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    fn book_of() -> Book {
        let mut b = Book::new();
        b.sheets.clear();
        let mut s = Sheet::new("売上台帳");
        for (a1, v) in [
            ("A1", "月"), ("B1", "品名"), ("C1", "数量"), ("D1", "金額"),
            ("A2", "4月"), ("B2", "ボールペン"), ("C2", "12"), ("D2", "=C2*150"),
            ("A3", "5月"), ("B3", "ノート"), ("C3", "5"), ("D3", "=C3*80"),
        ] {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s.tables.push(TableDef {
            name: "売上台帳".into(),
            a: Pos::new(0, 0),
            b: Pos::new(2, 3),
            header: true,
            ..Default::default()
        });
        b.sheets.push(s);
        book::calc::recalc_all(&mut b);
        b
    }

    fn value(b: &Book, sheet: usize, a1: &str) -> String {
        b.sheets[sheet].value(Pos::parse(a1).unwrap()).display()
    }

    /// **書いた字が本家の形になっている。** 題・見出しの空行・式がそのまま
    #[test]
    fn the_written_text_shape() {
        let src = write(&book_of());
        assert!(src.contains(".売上台帳"), "表の題が無い:\n{src}");
        assert!(src.contains("|月 |品名 |数量 |金額"), "見出しの行が無い:\n{src}");
        // 式は値ではなく式のまま
        assert!(src.contains("=C2*150"), "式が値になっている:\n{src}");
        assert!(!src.contains("1800"), "値を書いてしまっている:\n{src}");
    }

    /// **往復してブックが戻る。** 値・式・シート名・見出し
    #[test]
    fn round_trips() {
        let from = book_of();
        let (back, report) = parse(&write(&from)).expect("読めない");
        assert_eq!(back.sheets.len(), 1);
        assert_eq!(back.sheets[0].name, "売上台帳");
        assert_eq!(value(&back, 0, "B2"), "ボールペン");
        // 式が生きている = 読んだ所で計算されている
        assert_eq!(value(&back, 0, "D2"), "1800");
        assert_eq!(value(&back, 0, "D3"), "400");
        assert_eq!(back.sheets[0].get(Pos::parse("D2").unwrap()).unwrap().formula.as_deref(), Some("C2*150"));
        assert!(report.is_empty(), "落とし物があるはずがない: {report:?}");
    }

    /// **構造化参照が往復する。** 題と見出しから表の定義が戻るので、
    /// 宣言を書かなくても列の名前で引ける(設計の見本と同じ形)。
    ///
    /// 合計は**別の列**に置く — adoc では表がそのまま格子なので、
    /// 金額の列の下に置くと自分の列を合計する循環参照になる
    #[test]
    fn structured_references_round_trip() {
        let mut b = Book::new();
        b.sheets.clear();
        let mut s = Sheet::new("売上台帳");
        for (a1, v) in [
            ("A1", "品名"), ("B1", "数量"), ("C1", "金額"), ("D1", "全体"),
            ("A2", "ボールペン"), ("B2", "12"), ("C2", "=売上台帳[@数量]*150"), ("D2", "=SUM(売上台帳[金額])"),
            ("A3", "ノート"), ("B3", "5"), ("C3", "=売上台帳[@数量]*80"),
        ] {
            s.set(Pos::parse(a1).unwrap(), Cell::input(v));
        }
        s.tables.push(TableDef {
            name: "売上台帳".into(),
            a: Pos::new(0, 0),
            b: Pos::new(2, 3),
            header: true,
            ..Default::default()
        });
        b.sheets.push(s);
        book::calc::recalc_all(&mut b);
        assert_eq!(value(&b, 0, "C2"), "1800");
        assert_eq!(value(&b, 0, "D2"), "2200");

        let (back, _) = parse(&write(&b)).expect("読めない");
        assert_eq!(value(&back, 0, "C2"), "1800", "往復で この行の参照 が切れた");
        assert_eq!(value(&back, 0, "D2"), "2200", "往復で構造化参照が切れた");
    }

    /// **人が手で書いた式が壊れない**(2026-08-19 に踏んだ穴)。
    ///
    /// `=A2*B2*C2` の `*B2*` を太字と読むと、印が消えて `A2B2C2` という
    /// 別の式になり、黙って `#NAME?` に化けていた
    #[test]
    fn a_hand_written_formula_is_not_eaten_by_bold() {
        let (b, _) = parse(".表\n|===\n|A |B |C |D\n\n|2 |3 |4 |=A2*B2*C2\n|===\n").expect("読めない");
        assert_eq!(
            b.sheets[0].get(Pos::parse("D2").unwrap()).unwrap().formula.as_deref(),
            Some("A2*B2*C2"),
            "式の * が太字の印として食われた"
        );
        assert_eq!(value(&b, 0, "D2"), "24");
    }

    /// **式でない字を式にしない。** `=` の後ろに空白があれば式ではない
    /// (セルの中の見出しの書き方)。字のまま入り、往復しても字のまま
    #[test]
    fn an_equals_with_a_space_is_not_a_formula() {
        let (b, _) = parse(".表\n|===\n|= 見出し |ふつう\n|===\n").expect("読めない");
        assert_eq!(b.sheets[0].get(Pos::parse("A1").unwrap()).unwrap().formula, None, "式にしてしまった");
        assert_eq!(value(&b, 0, "A1"), "= 見出し");
        // 往復しても字のまま
        let (back, _) = parse(&write(&b)).expect("読めない");
        assert_eq!(value(&back, 0, "A1"), "= 見出し");
    }

    /// 複数のシートは複数の表になる
    #[test]
    fn several_sheets_round_trip() {
        let mut b = book_of();
        let mut s2 = Sheet::new("控え");
        s2.set(Pos::parse("A1").unwrap(), Cell::input("あ"));
        b.sheets.push(s2);
        let (back, _) = parse(&write(&b)).expect("読めない");
        assert_eq!(back.sheets.len(), 2);
        assert_eq!(back.sheets[1].name, "控え");
        assert_eq!(value(&back, 1, "A1"), "あ");
    }

    /// **横の結合が往復する**
    #[test]
    fn horizontal_merges_round_trip() {
        let mut b = Book::new();
        b.sheets.clear();
        let mut s = Sheet::new("様式");
        s.set(Pos::parse("A1").unwrap(), Cell::input("題"));
        s.set(Pos::parse("A2").unwrap(), Cell::input("左"));
        s.set(Pos::parse("B2").unwrap(), Cell::input("右"));
        s.merges.push((Pos::new(0, 0), Pos::new(0, 1)));
        b.sheets.push(s);
        let (back, _) = parse(&write(&b)).expect("読めない");
        assert_eq!(back.sheets[0].merges, vec![(Pos::new(0, 0), Pos::new(0, 1))]);
        assert_eq!(value(&back, 0, "A1"), "題");
        assert_eq!(value(&back, 0, "A2"), "左");
        assert_eq!(value(&back, 0, "B2"), "右");
    }

    /// 表の外の段落は落とし、**落としたことを言う**
    #[test]
    fn paragraphs_outside_tables_are_counted() {
        let (b, report) = parse("= 見出し\n\nこれは本文です。\n\n.表\n|===\n|あ |い\n|===\n").expect("読めない");
        assert_eq!(b.sheets.len(), 1);
        assert_eq!(b.sheets[0].name, "表");
        assert!(report.iter().any(|r| r.contains("表の外の段落")), "黙って落とした: {report:?}");
    }

    /// 表が1つも無くても、ブックは1枚から始まる
    #[test]
    fn no_table_means_a_one_sheet_book() {
        let (b, _) = parse("= ただの文書\n\n本文。\n").expect("読めない");
        assert_eq!(b.sheets.len(), 1);
    }

    /// **落とす物を数えて返す。** 書式や図形は adoc の表に載らない
    #[test]
    fn counts_what_is_dropped() {
        // **列の幅はもう落ちません**(2026-08-26)。テンプレートが持ちます。
        // 落ちるのは、実体が binary で adoc に入らない画像だけです
        let mut b = book_of();
        b.sheets[0].col_width.insert(0, 20.0);
        assert!(write_report(&b).is_empty(), "落ちない物を落ちると言っている");

        b.sheets[0].images.push(book::SheetImage {
            at: Pos::parse("D5").expect("番地"),
            dx_px: 0.0, dy_px: 0.0, width_px: 96.0, height_px: 96.0,
            data: vec![0x89, b'P', b'N', b'G'],
        });
        let r = write_report(&b);
        assert!(r.iter().any(|x| x.contains("画像")), "画像を言っていない: {r:?}");
        assert_eq!(crate::book_meta::image_files(&b).len(), 1, "実体が出ない");
    }
    /// **頭に 0 の付いた番号が数に化けない**(実物 16 冊のうち 5 冊が
    /// これに当たった。2026-08-19 に測って見つけた)
    #[test]
    fn a_leading_zero_number_stays_text() {
        let (b, _) = parse(".台帳\n|===\n|番号 |数\n\n|001 |12\n|0007 |0.5\n|===\n").expect("読めない");
        assert_eq!(value(&b, 0, "A2"), "001", "伝票番号が数に化けた");
        assert_eq!(value(&b, 0, "A3"), "0007");
        // 本当の数は数のまま
        assert_eq!(value(&b, 0, "B2"), "12");
        assert_eq!(value(&b, 0, "B3"), "0.5", "0.5 まで字にしてしまった");
    }

    /// **セルの中の改行は、AsciiDoc の改行(` +`)で書く。**
    ///
    /// 2026-08-31 発注者「adoc では rich_runs は使わない。だからこそ、
    /// うまく表示できるように変換できるようにして」。飾りは持ち込まない
    /// ので、せめて**行の折りが行の折りとして見える**必要があります。
    /// 空行で書くと段落が2つになり、間が余分に空きます。
    #[test]
    fn an_in_cell_break_is_written_as_a_line_break() {
        let mut b = Book::new();
        b.sheets.clear();
        let mut s = Sheet::new("覚え");
        s.set(Pos::parse("A1").unwrap(), Cell::input("区　分
Type"));
        s.set(Pos::parse("B1").unwrap(), Cell::input("隣"));
        b.sheets.push(s);
        let src = write(&b);
        assert!(src.contains("区　分 +
Type"), "改行が ` +` になっていない:\n{src}");
        assert!(!src.contains("区　分

Type"), "空行で書いている(段落が2つになる)");
        // 隣の升も同じ行のまま
        let (back, _) = parse(&src).expect("読めない");
        assert_eq!(value(&back, 0, "A1"), "区　分
Type", "改行が戻らない");
        assert_eq!(value(&back, 0, "B1"), "隣", "隣の升が次の行へ落ちた");
    }

    /// **折返しのセルが往復する**(2026-08-19)。中に改行のあるセルは
    /// `a|` で書かれ、段落の切れ目が残る
    #[test]
    fn wrapped_cells_round_trip() {
        let mut b = Book::new();
        b.sheets.clear();
        let mut s = Sheet::new("覚え");
        s.set(Pos::parse("A1").unwrap(), Cell::input("一行目\n二行目"));
        s.set(Pos::parse("B1").unwrap(), Cell::input("ふつう"));
        b.sheets.push(s);
        let src = write(&b);
        assert!(src.contains("a|一行目"), "折返しのセルが a| になっていない:\n{src}");
        let (back, _) = parse(&src).expect("読めない");
        assert_eq!(value(&back, 0, "A1"), "一行目\n二行目", "段落の切れ目が潰れた");
        assert_eq!(value(&back, 0, "B1"), "ふつう");
    }

    /// **升の中の縦棒で行が割れない**(2026-08-20 に見つけた)。
    ///
    /// `|` は升の切れ目そのものです。逃がさずに書くと、`A|B` の1升が
    /// 2升に割れて**その行から右がずれます**。`|===` を含む升なら、
    /// 表がそこで終わったことになって残りが消えます。
    ///
    /// 読む側は前から `\|` を飛ばしていたので、足りなかったのは
    /// 書く側だけでした。**保存で中身が壊れる**種類の欠陥です。
    #[test]
    fn a_bar_inside_a_cell_does_not_split_the_row() {
        for content in ["A|B", "|見出し\n|中身", "|===\nおしまい", "|"] {
            let mut b = Book::new();
            b.sheets.clear();
            let mut s = Sheet::new("覚え");
            s.set(Pos::parse("A1").unwrap(), Cell::input(content));
            s.set(Pos::parse("B1").unwrap(), Cell::input("番人"));
            b.sheets.push(s);
            let src = write(&b);
            let (back, _) = parse(&src).expect("読めない");
            assert_eq!(value(&back, 0, "A1"), content, "中身が戻らない:\n{src}");
            assert_eq!(value(&back, 0, "B1"), "番人", "隣の升まで壊れた:\n{src}");
        }
    }

    /// **式の中の縦棒も往復する。** 式は字のまま書く決めですが、`|` だけは
    /// 逃がします(升の切れ目なので)。読む側で戻さないと、逆斜線が式に残ります
    #[test]
    fn a_bar_inside_a_formula_round_trips() {
        let mut b = Book::new();
        b.sheets.clear();
        let mut s = Sheet::new("覚え");
        s.set(Pos::parse("A1").unwrap(), Cell::input("=\"A|B\""));
        s.set(Pos::parse("B1").unwrap(), Cell::input("番人"));
        b.sheets.push(s);
        let src = write(&b);
        let (back, _) = parse(&src).expect("読めない");
        let f = back.sheets[0].get(Pos::parse("A1").unwrap()).and_then(|c| c.formula.clone());
        // 式は頭の `=` を落として持ちます(`Cell::input` の決め)
        assert_eq!(f.as_deref(), Some("\"A|B\""), "式が戻らない:\n{src}");
        assert_eq!(value(&back, 0, "A1"), "A|B", "答えが違う");
        assert_eq!(value(&back, 0, "B1"), "番人");
    }

}
