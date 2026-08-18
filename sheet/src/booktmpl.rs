//! **ブックの見た目 — テンプレート**(SEKKEI「エンジンの統一」4段目、
//! docs/sekkei/calc.ja.adoc「やる順」4)。
//!
//! `.adoc` のブックは**意味だけ**を持ちます(値と式とシート名)。
//! 列の幅・行の高さ・用紙の設定は見た目なので、隣の `テンプレート.adoc` が
//! 持ちます。writer と docx の関係、writer とテンプレートの関係と同じです。
//!
//! *中身は表です*(SEKKEI「スタイルの定義は表で書く」)。設定の書き方
//! (`キー = 値`)ではなく、穴の空いた文書と同じ**表**にしてあります。
//!
//! ....
//! .用紙
//! |===
//! |シート |大きさ |向き |余白 |目盛線
//!
//! |売上台帳 |A4 |横 |20 |true
//! |===
//!
//! .列幅
//! |===
//! |シート |列 |幅
//!
//! |売上台帳 |A |20
//! |===
//! ....
//!
//! *表の読み書きは書き直しません。* `kumihan::adoc` に渡します。
//!
//! *テンプレートの持ち主は、指示する人です*(2026-08-18 発注者)。
//! だから**配られたテンプレートは書き替えません** — 呼ぶ側は、既にある
//! ファイルを上書きしないでください。

use crate::model::{Book, Pos, Sheet};
use kumihan::{Block, Cellbox, Document, Table};

/// 1枚ぶんの見た目。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SheetLook {
    pub name: String,
    /// (列, 幅)。列は 0 から
    pub col_width: Vec<(u32, f32)>,
    /// (行, 高さ)。行は 0 から
    pub row_height: Vec<(u32, f32)>,
    /// 用紙の大きさ(xlsx の番号。9 = A4)
    pub paper_size: Option<u32>,
    pub landscape: Option<bool>,
    /// 余白(mm。左, 右, 上, 下)
    pub margins_mm: Option<(f32, f32, f32, f32)>,
    pub print_gridlines: Option<bool>,
    pub zoom_scale: Option<u32>,
}

/// ブックの見た目ぜんぶ。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookTheme {
    pub sheets: Vec<SheetLook>,
}

impl BookTheme {
    /// 持っている物が何も無いか(何も無ければテンプレートを書く意味がない)
    pub fn is_empty(&self) -> bool {
        self.sheets.iter().all(|s| {
            s.col_width.is_empty()
                && s.row_height.is_empty()
                && s.paper_size.is_none()
                && s.landscape.is_none()
                && s.margins_mm.is_none()
                && s.print_gridlines.is_none()
                && s.zoom_scale.is_none()
        })
    }

    fn 枚(&mut self, name: &str) -> &mut SheetLook {
        if let Some(i) = self.sheets.iter().position(|s| s.name == name) {
            return &mut self.sheets[i];
        }
        self.sheets.push(SheetLook { name: name.to_string(), ..Default::default() });
        self.sheets.last_mut().expect("いま入れた")
    }
}

/// ブックから見た目を取り出す。
pub fn from_book(b: &Book) -> BookTheme {
    let mut t = BookTheme::default();
    for s in &b.sheets {
        let look = SheetLook {
            name: s.name.clone(),
            col_width: s.col_width.iter().map(|(k, v)| (*k, *v)).collect(),
            row_height: s.row_height.iter().map(|(k, v)| (*k, *v)).collect(),
            paper_size: s.paper_size,
            landscape: s.landscape.then_some(true),
            margins_mm: s.margins_mm,
            print_gridlines: s.print_gridlines.then_some(true),
            zoom_scale: s.zoom_scale,
        };
        t.sheets.push(look);
    }
    t
}

/// 見た目をブックに当てる。**そのシートが無ければ黙って飛ばします**
/// (テンプレートは別のブックにも使えるので、名前が合わないのは普通のこと)。
pub fn apply(t: &BookTheme, b: &mut Book) {
    for look in &t.sheets {
        let Some(s) = b.sheets.iter_mut().find(|s| s.name == look.name) else { continue };
        for (c, w) in &look.col_width {
            s.col_width.insert(*c, *w);
        }
        for (r, h) in &look.row_height {
            s.row_height.insert(*r, *h);
        }
        if let Some(p) = look.paper_size {
            s.paper_size = Some(p);
        }
        if let Some(l) = look.landscape {
            s.landscape = l;
        }
        if let Some(m) = look.margins_mm {
            s.margins_mm = Some(m);
        }
        if let Some(g) = look.print_gridlines {
            s.print_gridlines = g;
        }
        if let Some(z) = look.zoom_scale {
            s.zoom_scale = Some(z);
        }
    }
}

// ---------- 書く ----------

/// テンプレートの字にする。
pub fn write(t: &BookTheme) -> String {
    let mut d = Document::default();
    if let Some(tb) = 用紙の表(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = 幅の表(t) {
        d.blocks.push(Block::Table(tb));
    }
    if let Some(tb) = 高さの表(t) {
        d.blocks.push(Block::Table(tb));
    }
    kumihan::adoc::write(&d)
}

fn cell(s: &str) -> Cellbox {
    Cellbox { paragraphs: Document::plain(s).paragraphs().cloned().collect(), ..Default::default() }
}

fn 表(title: &str, 見出し: &[&str], rows: Vec<Vec<String>>) -> Option<Table> {
    if rows.is_empty() {
        return None;
    }
    let mut t = Table {
        title: Some(title.to_string()),
        header_row: true,
        rows: vec![見出し.iter().map(|h| cell(h)).collect()],
        ..Default::default()
    };
    for r in rows {
        t.rows.push(r.iter().map(|x| cell(x)).collect());
    }
    Some(t)
}

/// 数を字にする(整数はそのまま、小数は要るぶんだけ)
fn 数(v: f32) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

fn 用紙の表(t: &BookTheme) -> Option<Table> {
    let rows: Vec<Vec<String>> = t
        .sheets
        .iter()
        .filter(|s| {
            s.paper_size.is_some()
                || s.landscape.is_some()
                || s.margins_mm.is_some()
                || s.print_gridlines.is_some()
                || s.zoom_scale.is_some()
        })
        .map(|s| {
            vec![
                s.name.clone(),
                s.paper_size.map(用紙の名).unwrap_or_default(),
                match s.landscape {
                    Some(true) => "横".into(),
                    Some(false) => "縦".into(),
                    None => String::new(),
                },
                s.margins_mm.map(|(l, r, tp, b)| format!("{},{},{},{}", 数(l), 数(r), 数(tp), 数(b))).unwrap_or_default(),
                match s.print_gridlines {
                    Some(true) => "true".into(),
                    Some(false) => "false".into(),
                    None => String::new(),
                },
                s.zoom_scale.map(|z| z.to_string()).unwrap_or_default(),
            ]
        })
        .collect();
    表("用紙", &["シート", "大きさ", "向き", "余白", "目盛線", "拡大"], rows)
}

fn 幅の表(t: &BookTheme) -> Option<Table> {
    let mut rows = Vec::new();
    for s in &t.sheets {
        for (c, w) in &s.col_width {
            rows.push(vec![s.name.clone(), 列の名(*c), 数(*w)]);
        }
    }
    表("列幅", &["シート", "列", "幅"], rows)
}

fn 高さの表(t: &BookTheme) -> Option<Table> {
    let mut rows = Vec::new();
    for s in &t.sheets {
        for (r, h) in &s.row_height {
            rows.push(vec![s.name.clone(), (r + 1).to_string(), 数(*h)]);
        }
    }
    表("行の高さ", &["シート", "行", "高さ"], rows)
}

/// 列の番号を A1 の綴りの列の名にする(0 → A)
fn 列の名(c: u32) -> String {
    let a1 = Pos::new(0, c).a1();
    a1.trim_end_matches(|ch: char| ch.is_ascii_digit()).to_string()
}

/// 用紙の番号を名前に(xlsx の番号は Excel の決め)
fn 用紙の名(n: u32) -> String {
    match n {
        8 => "A3".into(),
        9 => "A4".into(),
        11 => "A5".into(),
        12 => "B4".into(),
        13 => "B5".into(),
        1 => "Letter".into(),
        5 => "Legal".into(),
        // 知らない番号は**番号のまま返す**(黙って A4 にしない)
        other => other.to_string(),
    }
}

fn 用紙の番号(s: &str) -> Option<u32> {
    match s.trim().to_ascii_uppercase().as_str() {
        "A3" => Some(8),
        "A4" => Some(9),
        "A5" => Some(11),
        "B4" => Some(12),
        "B5" => Some(13),
        "LETTER" => Some(1),
        "LEGAL" => Some(5),
        other => other.parse().ok(),
    }
}

// ---------- 読む ----------

/// テンプレートの字を読む。知らない表は**黙って飛ばします**
/// (テンプレートには writer 向けの節も混じるため)。
pub fn parse(src: &str) -> Result<BookTheme, String> {
    let doc = kumihan::adoc::parse(src)?;
    let mut t = BookTheme::default();
    for b in &doc.blocks {
        let Block::Table(tb) = b else { continue };
        let Some(title) = tb.title.as_deref() else { continue };
        let rows = tb.text_rows();
        // 1行目は見出し
        let body = if tb.header_row && !rows.is_empty() { &rows[1..] } else { &rows[..] };
        match title {
            "用紙" => 用紙を読む(&mut t, body),
            "列幅" => 幅を読む(&mut t, body),
            "行の高さ" => 高さを読む(&mut t, body),
            _ => {}
        }
    }
    Ok(t)
}

fn 取る(row: &[String], i: usize) -> &str {
    row.get(i).map(|s| s.trim()).unwrap_or("")
}

fn 用紙を読む(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = 取る(row, 0);
        if name.is_empty() {
            continue;
        }
        let s = t.枚(name);
        let 大きさ = 取る(row, 1);
        if !大きさ.is_empty() {
            s.paper_size = 用紙の番号(大きさ);
        }
        match 取る(row, 2) {
            "横" => s.landscape = Some(true),
            "縦" => s.landscape = Some(false),
            _ => {}
        }
        let 余白: Vec<f32> = 取る(row, 3).split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if 余白.len() == 4 {
            s.margins_mm = Some((余白[0], 余白[1], 余白[2], 余白[3]));
        } else if 余白.len() == 1 {
            // 1つだけなら四方とも同じ
            s.margins_mm = Some((余白[0], 余白[0], 余白[0], 余白[0]));
        }
        match 取る(row, 4).to_ascii_lowercase().as_str() {
            "true" => s.print_gridlines = Some(true),
            "false" => s.print_gridlines = Some(false),
            _ => {}
        }
        if let Ok(z) = 取る(row, 5).parse() {
            s.zoom_scale = Some(z);
        }
    }
}

fn 幅を読む(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = 取る(row, 0);
        let Some(c) = 列の番号(取る(row, 1)) else { continue };
        let Ok(w) = 取る(row, 2).parse::<f32>() else { continue };
        if !name.is_empty() {
            t.枚(name).col_width.push((c, w));
        }
    }
}

fn 高さを読む(t: &mut BookTheme, rows: &[Vec<String>]) {
    for row in rows {
        let name = 取る(row, 0);
        let Ok(r) = 取る(row, 1).parse::<u32>() else { continue };
        let Ok(h) = 取る(row, 2).parse::<f32>() else { continue };
        if !name.is_empty() && r >= 1 {
            t.枚(name).row_height.push((r - 1, h));
        }
    }
}

/// 列の名(`A`)を番号に。`Pos::parse` に1行目を足して解かせます
fn 列の番号(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Pos::parse(&format!("{s}1")).map(|p| p.col)
}

/// このブックの隣にあるテンプレートの径路(`テンプレート.adoc`)。
pub fn path_for(book: &std::path::Path) -> std::path::PathBuf {
    book.parent().unwrap_or(std::path::Path::new(".")).join("テンプレート.adoc")
}

/// 見た目を落とさずに済むよう、`Sheet` から見た目だけを消す。
/// `.adoc` に書くときに使います(意味だけを書くため)。
pub fn strip(s: &mut Sheet) {
    s.col_width.clear();
    s.row_height.clear();
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::Cell;

    fn 帳簿() -> Book {
        let mut b = Book::new();
        b.sheets[0].name = "売上台帳".into();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("月"));
        b.sheets[0].col_width.insert(0, 20.0);
        b.sheets[0].col_width.insert(3, 12.5);
        b.sheets[0].row_height.insert(0, 24.0);
        b.sheets[0].paper_size = Some(9);
        b.sheets[0].landscape = true;
        b.sheets[0].margins_mm = Some((20.0, 20.0, 15.0, 15.0));
        b
    }

    #[test]
    fn 書いた字が表になっている() {
        let src = write(&from_book(&帳簿()));
        assert!(src.contains(".用紙"), "用紙の表が無い:\n{src}");
        assert!(src.contains(".列幅"), "列幅の表が無い:\n{src}");
        assert!(src.contains("|売上台帳 |A |20"), "列幅の行が無い:\n{src}");
        assert!(src.contains("A4"), "用紙の名前が番号のまま:\n{src}");
    }

    #[test]
    fn 往復で見た目が戻る() {
        let 元 = from_book(&帳簿());
        let 戻り = parse(&write(&元)).expect("読めない");
        assert_eq!(戻り, 元, "往復で見た目が変わった");
    }

    /// **当てるとブックに戻る。** 意味だけの `.adoc` と組み合わせる形
    #[test]
    fn ブックに当てられる() {
        let t = from_book(&帳簿());
        let mut b = Book::new();
        b.sheets[0].name = "売上台帳".into();
        apply(&t, &mut b);
        assert_eq!(b.sheets[0].col_width.get(&0), Some(&20.0));
        assert_eq!(b.sheets[0].col_width.get(&3), Some(&12.5));
        assert_eq!(b.sheets[0].row_height.get(&0), Some(&24.0));
        assert_eq!(b.sheets[0].paper_size, Some(9));
        assert!(b.sheets[0].landscape);
        assert_eq!(b.sheets[0].margins_mm, Some((20.0, 20.0, 15.0, 15.0)));
    }

    /// 名前の合わないシートは**黙って飛ばす**(テンプレートは使い回せる)
    #[test]
    fn 知らないシートは飛ばす() {
        let t = from_book(&帳簿());
        let mut b = Book::new();
        b.sheets[0].name = "別の名前".into();
        apply(&t, &mut b);
        assert!(b.sheets[0].col_width.is_empty(), "知らないシートに当ててしまった");
    }

    /// 知らない表は飛ばす(writer 向けの節が混じっていても落ちない)
    #[test]
    fn 知らない表は飛ばす() {
        let t = parse(".スタイル\n|===\n|名前 |大きさ\n\n|見出し1 |16\n|===\n").expect("読めない");
        assert!(t.is_empty());
    }

    /// 余白は1つだけ書けば四方とも同じ
    #[test]
    fn 余白は1つでもよい() {
        let t = parse(".用紙\n|===\n|シート |大きさ |向き |余白\n\n|表 |A4 |縦 |20\n|===\n").expect("読めない");
        assert_eq!(t.sheets[0].margins_mm, Some((20.0, 20.0, 20.0, 20.0)));
        assert_eq!(t.sheets[0].paper_size, Some(9));
        assert_eq!(t.sheets[0].landscape, Some(false));
    }

    /// 知らない用紙の番号は**番号のまま**(黙って A4 にしない)
    #[test]
    fn 知らない用紙は番号のまま() {
        assert_eq!(用紙の名(99), "99");
        assert_eq!(用紙の番号("99"), Some(99));
    }
}
