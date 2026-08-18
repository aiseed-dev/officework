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

use crate::calc::recalc_all;
use crate::model::{Book, Cell, Pos, Sheet, TableDef, Value};
use kumihan::{Block, Cellbox, Document, Table, VMerge};

/// ブックを adoc の字にする。
pub fn write(book: &Book) -> String {
    kumihan::adoc::write(&to_doc(book))
}

/// 書くときに**落ちる物**を数える。日本語で1件1行。
///
/// 値が消えるわけではありません — 見た目と規則が adoc の表に載らない、
/// という意味です。載せ先はテンプレートで、そちらはまだこれからです。
pub fn write_report(book: &Book) -> Vec<String> {
    let mut out = Vec::new();
    let n = |x: usize| x;
    let mut 書式 = 0;
    let mut 図形 = 0;
    let mut 画像 = 0;
    let ピボット = book.pivots.len();
    let mut 幅 = 0;
    for s in &book.sheets {
        書式 += s.cells.values().filter(|c| !c.fmt.is_plain()).count();
        図形 += s.shapes.len();
        画像 += s.images.len() + s.images_new.len();
        幅 += s.col_width.len() + s.row_height.len();
    }
    if 書式 > 0 {
        out.push(format!("セルの書式 {} 件(見た目はテンプレートの持ち場です)", n(書式)));
    }
    if 幅 > 0 {
        out.push(format!("列の幅・行の高さ {} 件(見た目はテンプレートの持ち場です)", n(幅)));
    }
    if 図形 > 0 {
        out.push(format!("図形 {} 件(adoc の表には置けません)", n(図形)));
    }
    // **画像はいちばん重い落とし物。** 3 MB のブックが 1 KB の adoc になる
    // ことがあるので、黙って落とすと消えたことに気づけません。外のファイルに
    // 出す道(writer の `image::`)はこれからです
    if 画像 > 0 {
        out.push(format!("画像 {} 件(まだ外のファイルに出せません)", n(画像)));
    }
    if ピボット > 0 {
        out.push(format!("ピボットテーブル {} 件(adoc の表には置けません)", n(ピボット)));
    }
    out
}

/// adoc の字をブックにする。**読めなかった物は数えて返す。**
///
/// 表でない段落(見出し・本文)は、ブックに居場所が無いので落とし、
/// 何を落としたかを2つ目の返り値に入れます。
pub fn parse(src: &str) -> Result<(Book, Vec<String>), String> {
    let (doc, mut report) = kumihan::adoc::parse_full(src)?;

    let mut book = Book::new();
    book.sheets.clear();
    let mut 段落 = 0;
    for b in &doc.blocks {
        match b {
            Block::Table(t) => book.sheets.push(to_sheet(t, book.sheets.len())),
            // 見出しや本文は表計算のブックに居場所が無い
            Block::Para(p) => {
                if !p.runs.iter().all(|r| r.text.trim().is_empty()) {
                    段落 += 1;
                }
            }
        }
    }
    if 段落 > 0 {
        report.push(format!("表の外の段落 {段落} 件(ブックには表しか入りません)"));
    }
    // 1枚も無いブックは作らない(calc は必ず1枚から始まる)
    if book.sheets.is_empty() {
        book.sheets.push(Sheet::new("Sheet1"));
    }
    // **値は持たないので、読んだ所で計算する。** 式が正本
    recalc_all(&mut book);
    Ok((book, report))
}

// ---------- ブック → 文書 ----------

fn to_doc(book: &Book) -> Document {
    let mut d = Document::default();
    for s in &book.sheets {
        d.blocks.push(Block::Table(to_table(s)));
    }
    d
}

/// シートを文書の表にする。
fn to_table(s: &Sheet) -> Table {
    let (rows, cols) = s.extent();
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

fn cellbox(text: &str, col_span: u8, v_merge: VMerge) -> Cellbox {
    Cellbox {
        paragraphs: Document::plain(text).paragraphs().cloned().collect(),
        col_span,
        v_merge,
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
            // **式でない字を式にしない。** `= 見出し` のように `=` で始まる
            // だけの字は、`Cell::input` に渡すと式として取られてしまう。
            // 式かどうかの決めは読み手と同じ物を使う(2箇所に書かない)
            let v = if !kumihan::adoc::is_formula_cell(cell) && 字のままにする(cell) {
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
///
/// `=` で始まるだけの字(`= 見出し`)も、式ではないので字のままにします。
fn 字のままにする(s: &str) -> bool {
    let t = s.trim();
    if t.starts_with('=') {
        return true; // 式は上で分けてある = ここに来るのは式でない `=` の字
    }
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

    fn 台帳() -> Book {
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
        crate::calc::recalc_all(&mut b);
        b
    }

    fn 値(b: &Book, sheet: usize, a1: &str) -> String {
        b.sheets[sheet].value(Pos::parse(a1).unwrap()).display()
    }

    /// **書いた字が本家の形になっている。** 題・見出しの空行・式がそのまま
    #[test]
    fn 書いた字の形() {
        let src = write(&台帳());
        assert!(src.contains(".売上台帳"), "表の題が無い:\n{src}");
        assert!(src.contains("|月 |品名 |数量 |金額"), "見出しの行が無い:\n{src}");
        // 式は値ではなく式のまま
        assert!(src.contains("=C2*150"), "式が値になっている:\n{src}");
        assert!(!src.contains("1800"), "値を書いてしまっている:\n{src}");
    }

    /// **往復してブックが戻る。** 値・式・シート名・見出し
    #[test]
    fn 往復で戻る() {
        let 元 = 台帳();
        let (戻り, report) = parse(&write(&元)).expect("読めない");
        assert_eq!(戻り.sheets.len(), 1);
        assert_eq!(戻り.sheets[0].name, "売上台帳");
        assert_eq!(値(&戻り, 0, "B2"), "ボールペン");
        // 式が生きている = 読んだ所で計算されている
        assert_eq!(値(&戻り, 0, "D2"), "1800");
        assert_eq!(値(&戻り, 0, "D3"), "400");
        assert_eq!(戻り.sheets[0].get(Pos::parse("D2").unwrap()).unwrap().formula.as_deref(), Some("C2*150"));
        assert!(report.is_empty(), "落とし物があるはずがない: {report:?}");
    }

    /// **構造化参照が往復する。** 題と見出しから表の定義が戻るので、
    /// 宣言を書かなくても列の名前で引ける(設計の見本と同じ形)。
    ///
    /// 合計は**別の列**に置く — adoc では表がそのまま格子なので、
    /// 金額の列の下に置くと自分の列を合計する循環参照になる
    #[test]
    fn 構造化参照が往復する() {
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
        crate::calc::recalc_all(&mut b);
        assert_eq!(値(&b, 0, "C2"), "1800");
        assert_eq!(値(&b, 0, "D2"), "2200");

        let (戻り, _) = parse(&write(&b)).expect("読めない");
        assert_eq!(値(&戻り, 0, "C2"), "1800", "往復で この行の参照 が切れた");
        assert_eq!(値(&戻り, 0, "D2"), "2200", "往復で構造化参照が切れた");
    }

    /// **人が手で書いた式が壊れない**(2026-08-19 に踏んだ穴)。
    ///
    /// `=A2*B2*C2` の `*B2*` を太字と読むと、印が消えて `A2B2C2` という
    /// 別の式になり、黙って `#NAME?` に化けていた
    #[test]
    fn 手で書いた式が太字に食われない() {
        let (b, _) = parse(".表\n|===\n|A |B |C |D\n\n|2 |3 |4 |=A2*B2*C2\n|===\n").expect("読めない");
        assert_eq!(
            b.sheets[0].get(Pos::parse("D2").unwrap()).unwrap().formula.as_deref(),
            Some("A2*B2*C2"),
            "式の * が太字の印として食われた"
        );
        assert_eq!(値(&b, 0, "D2"), "24");
    }

    /// **式でない字を式にしない。** `=` の後ろに空白があれば式ではない
    /// (セルの中の見出しの書き方)。字のまま入り、往復しても字のまま
    #[test]
    fn 空白のある等号は式ではない() {
        let (b, _) = parse(".表\n|===\n|= 見出し |ふつう\n|===\n").expect("読めない");
        assert_eq!(b.sheets[0].get(Pos::parse("A1").unwrap()).unwrap().formula, None, "式にしてしまった");
        assert_eq!(値(&b, 0, "A1"), "= 見出し");
        // 往復しても字のまま
        let (戻り, _) = parse(&write(&b)).expect("読めない");
        assert_eq!(値(&戻り, 0, "A1"), "= 見出し");
    }

    /// 複数のシートは複数の表になる
    #[test]
    fn 複数のシートが往復する() {
        let mut b = 台帳();
        let mut s2 = Sheet::new("控え");
        s2.set(Pos::parse("A1").unwrap(), Cell::input("あ"));
        b.sheets.push(s2);
        let (戻り, _) = parse(&write(&b)).expect("読めない");
        assert_eq!(戻り.sheets.len(), 2);
        assert_eq!(戻り.sheets[1].name, "控え");
        assert_eq!(値(&戻り, 1, "A1"), "あ");
    }

    /// **横の結合が往復する**
    #[test]
    fn 横の結合が往復する() {
        let mut b = Book::new();
        b.sheets.clear();
        let mut s = Sheet::new("様式");
        s.set(Pos::parse("A1").unwrap(), Cell::input("題"));
        s.set(Pos::parse("A2").unwrap(), Cell::input("左"));
        s.set(Pos::parse("B2").unwrap(), Cell::input("右"));
        s.merges.push((Pos::new(0, 0), Pos::new(0, 1)));
        b.sheets.push(s);
        let (戻り, _) = parse(&write(&b)).expect("読めない");
        assert_eq!(戻り.sheets[0].merges, vec![(Pos::new(0, 0), Pos::new(0, 1))]);
        assert_eq!(値(&戻り, 0, "A1"), "題");
        assert_eq!(値(&戻り, 0, "A2"), "左");
        assert_eq!(値(&戻り, 0, "B2"), "右");
    }

    /// 表の外の段落は落とし、**落としたことを言う**
    #[test]
    fn 表の外の段落は数えて返す() {
        let (b, report) = parse("= 見出し\n\nこれは本文です。\n\n.表\n|===\n|あ |い\n|===\n").expect("読めない");
        assert_eq!(b.sheets.len(), 1);
        assert_eq!(b.sheets[0].name, "表");
        assert!(report.iter().any(|r| r.contains("表の外の段落")), "黙って落とした: {report:?}");
    }

    /// 表が1つも無くても、ブックは1枚から始まる
    #[test]
    fn 表が無ければ1枚のブック() {
        let (b, _) = parse("= ただの文書\n\n本文。\n").expect("読めない");
        assert_eq!(b.sheets.len(), 1);
    }

    /// **落とす物を数えて返す。** 書式や図形は adoc の表に載らない
    #[test]
    fn 落とす物を数える() {
        let mut b = 台帳();
        b.sheets[0].col_width.insert(0, 20.0);
        let r = write_report(&b);
        assert!(r.iter().any(|x| x.contains("列の幅")), "落とし物を言っていない: {r:?}");
    }
    /// **頭に 0 の付いた番号が数に化けない**(実物 16 冊のうち 5 冊が
    /// これに当たった。2026-08-19 に測って見つけた)
    #[test]
    fn 頭が0の番号は字のまま() {
        let (b, _) = parse(".台帳\n|===\n|番号 |数\n\n|001 |12\n|0007 |0.5\n|===\n").expect("読めない");
        assert_eq!(値(&b, 0, "A2"), "001", "伝票番号が数に化けた");
        assert_eq!(値(&b, 0, "A3"), "0007");
        // 本当の数は数のまま
        assert_eq!(値(&b, 0, "B2"), "12");
        assert_eq!(値(&b, 0, "B3"), "0.5", "0.5 まで字にしてしまった");
    }

    /// **折返しのセルが往復する**(2026-08-19)。中に改行のあるセルは
    /// `a|` で書かれ、段落の切れ目が残る
    #[test]
    fn 折返しのセルが往復する() {
        let mut b = Book::new();
        b.sheets.clear();
        let mut s = Sheet::new("覚え");
        s.set(Pos::parse("A1").unwrap(), Cell::input("一行目\n二行目"));
        s.set(Pos::parse("B1").unwrap(), Cell::input("ふつう"));
        b.sheets.push(s);
        let src = write(&b);
        assert!(src.contains("a|一行目"), "折返しのセルが a| になっていない:\n{src}");
        let (戻り, _) = parse(&src).expect("読めない");
        assert_eq!(値(&戻り, 0, "A1"), "一行目\n二行目", "段落の切れ目が潰れた");
        assert_eq!(値(&戻り, 0, "B1"), "ふつう");
    }

}
