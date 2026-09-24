//! **Filling a form from data** (2026-09-24, docs/sekkei/drawlist.ja.adoc).
//!
//! A form is a workbook whose cells hold marks such as `{氏名}` inside their
//! text. The data is another workbook (a `.sheet.adoc` of tables). Filling
//! puts the data's values where the marks are; a mark the data does not
//! answer becomes empty. The cells that hold marks are the form's fields.
//!
//! The marks:
//!
//! - `{氏名}`: the value next to 氏名 in a two-column table (name, value),
//!   such as 基本 or 自由記入
//! - `{日付.年}` `{日付.月}` `{日付.日}`: the year, month or day of a date
//!   written `2026-09-24`
//! - `{日付.元号}` `{日付.和暦年}`: the era and the year in it (令和, 8),
//!   the first year of an era written 元 as official forms do
//! - `{通勤時間.時間}` `{通勤時間.分}`: the hours or minutes of a time
//!   written `1時間10分` (JIS prints 約　時間　分 round them)
//! - `{年齢}`: the age on 日付 of someone born on 生年月日
//! - `{学歴・職歴.3.内容}`: row 3 (after the header) of the table 学歴・職歴,
//!   column 内容
//! - `{本人希望.2}`: line 2 of a value written on several lines
//! - `{性別:男・女}`: a choice. The options are written as they stand
//!   (`男・女`, or `男 ・ 女` with the spaces), and the one the data names is to be circled; filling
//!   returns where it is in the cell's text ([`Choice`]) for the layout to
//!   measure and circle
//!
//! - `{送達場所=住所}`: a box to tick. It is ■ when the data's 送達場所
//!   is 住所 and □ otherwise; the boxes of one name are one choice field
//!
//! When a table has more rows in the data than the form has rows for, the
//! rows that do not fit go on to a 別紙 (an attached sheet) in the same
//! columns ([`overflow`], [`with_bessi`]).
//!
//! A shape whose `field` is set (the photo box, `写真`) takes the picture
//! file the data names.
//!
//! For editing, the marks are gathered by the data item they stand for
//! ([`groups`]): `{日付.年}` `{日付.月}` `{日付.日}` are one date field
//! `日付`, and `{本人希望.1}`…`{本人希望.4}` one field `本人希望`.
//! [`set`] writes a field's new value back into the data.

use crate::{Book, Cell, Pos, Sheet, SheetImage, Value};

/// One field of a form: the cell that holds marks, and the names they use.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub sheet: usize,
    pub at: Pos,
    /// The mark names in the cell, in order (`氏名`, `日付.年`, …)
    pub names: Vec<String>,
}

/// The marks in a piece of text, in order.
pub fn marks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(a) = rest.find('{') {
        let Some(b) = rest[a..].find('}') else { break };
        let name = &rest[a + 1..a + b];
        if !name.is_empty() && !name.contains('{') {
            out.push(name.to_string());
        }
        rest = &rest[a + b + 1..];
    }
    out
}

/// The fields of a form: every cell whose text holds a mark.
pub fn fields(form: &Book) -> Vec<Field> {
    let mut out = Vec::new();
    for (si, s) in form.sheets.iter().enumerate() {
        for (p, c) in &s.cells {
            if let Value::Text(t) = &c.value {
                let names = marks(t);
                if !names.is_empty() {
                    out.push(Field { sheet: si, at: *p, names });
                }
            }
        }
    }
    out.sort_by_key(|f| (f.sheet, f.at.row, f.at.col));
    out
}

/// The data a form is filled from.
pub struct Data<'a> {
    book: &'a Book,
}

impl<'a> Data<'a> {
    pub fn new(book: &'a Book) -> Data<'a> {
        Data { book }
    }

    fn sheet(&self, name: &str) -> Option<&Sheet> {
        self.book.sheets.iter().find(|s| s.name == name)
    }

    /// The value next to `key` in any two-column table (name, value).
    fn pair(&self, key: &str) -> Option<String> {
        self.book.sheets.iter().find_map(|s| {
            let (rows, _) = s.extent();
            (0..rows).find_map(|r| {
                (s.value(Pos::new(r, 0)).display().trim() == key)
                    .then(|| s.value(Pos::new(r, 1)).display())
            })
        })
    }

    /// Row `n` (1 = the first row after the header) of table `t`, column
    /// `col` named in the header.
    fn row(&self, t: &str, n: u32, col: &str) -> Option<String> {
        let s = self.sheet(t)?;
        let (_, cols) = s.extent();
        let c = (0..cols).find(|c| s.value(Pos::new(0, *c)).display().trim() == col)?;
        let v = s.value(Pos::new(n, c)).display();
        (!v.is_empty()).then_some(v)
    }

    /// Whether some two-column table has a row named `key` (its value may
    /// be empty)
    fn has_pair(&self, key: &str) -> bool {
        self.book.sheets.iter().any(|s| {
            let (rows, _) = s.extent();
            (0..rows).any(|r| s.value(Pos::new(r, 0)).display().trim() == key)
        })
    }

    /// Whether table `t` exists with a column named `col` in its header
    fn has_column(&self, t: &str, col: &str) -> bool {
        self.sheet(t).is_some_and(|s| {
            let (_, cols) = s.extent();
            (0..cols).any(|c| s.value(Pos::new(0, c)).display().trim() == col)
        })
    }

    /// The value the data holds for a field name, as written there
    /// (`2026-09-24` for a date, every line of a multi-line value).
    pub fn raw(&self, name: &str) -> Option<String> {
        match name.split('.').collect::<Vec<_>>().as_slice() {
            [t, n, col] if n.parse::<u32>().is_ok() => self.row(t, n.parse().ok()?, col),
            _ => self.pair(name),
        }
    }

    /// What one mark stands for; None when the data does not say.
    pub fn answer(&self, mark: &str) -> Option<String> {
        if let Some((name, option)) = box_of(mark) {
            let chosen = self.pair(name).is_some_and(|v| v.trim() == option);
            return Some(if chosen { "■" } else { "□" }.to_string());
        }
        if mark == "年齢" {
            return self.age();
        }
        let parts: Vec<&str> = mark.split('.').collect();
        match parts.as_slice() {
            [key] => self.pair(key),
            [key, part @ ("年" | "月" | "日")] => {
                let d = date_parts(&self.pair(key)?)?;
                Some(match *part {
                    "年" => d.0.to_string(),
                    "月" => d.1.to_string(),
                    _ => d.2.to_string(),
                })
            }
            [key, part @ ("時間" | "分")] => duration_part(&self.pair(key)?, part),
            [key, part @ ("元号" | "和暦年")] => {
                let (era, y) = wareki(date_parts(&self.pair(key)?)?)?;
                Some(match *part {
                    "元号" => era.to_string(),
                    _ if y == 1 => "元".to_string(),
                    _ => y.to_string(),
                })
            }
            [key, n] if n.parse::<usize>().is_ok() => {
                let n: usize = n.parse().ok()?;
                self.pair(key)?.lines().nth(n.checked_sub(1)?).map(str::to_string)
            }
            [t, n, col] => self.row(t, n.parse().ok()?, col),
            _ => None,
        }
    }

    /// The age on 日付 of someone born on 生年月日.
    fn age(&self) -> Option<String> {
        let (y, m, d) = date_parts(&self.pair("生年月日")?)?;
        let (ty, tm, td) = date_parts(&self.pair("日付")?)?;
        let mut age = ty - y;
        if (tm, td) < (m, d) {
            age -= 1;
        }
        (age >= 0).then(|| age.to_string())
    }
}

/// The Japanese era of a date and the year in it, from the day each era
/// began (明治 is counted from 1868-10-23, its year of the Gregorian
/// calendar; earlier dates have none).
fn wareki((y, m, d): (i32, u32, u32)) -> Option<(&'static str, i32)> {
    const ERAS: [(&str, (i32, u32, u32)); 5] = [
        ("令和", (2019, 5, 1)),
        ("平成", (1989, 1, 8)),
        ("昭和", (1926, 12, 25)),
        ("大正", (1912, 7, 30)),
        ("明治", (1868, 10, 23)),
    ];
    ERAS.iter().find(|(_, start)| (y, m, d) >= *start).map(|(era, start)| (*era, y - start.0 + 1))
}

/// The number before 時間 or 分 in a time written `1時間10分`, or None.
fn duration_part(v: &str, unit: &str) -> Option<String> {
    let at = v.find(unit)?;
    // 分 in 時間 does not count: look for the unit that ends a number
    let digits: String = v[..at].chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    (!digits.is_empty()).then(|| digits.chars().rev().collect())
}

/// `2026-09-24` (or `2026/9/24`) as its year, month and day.
fn date_parts(s: &str) -> Option<(i32, u32, u32)> {
    let mut it = s.trim().split(['-', '/']);
    let y = it.next()?.trim().parse().ok()?;
    let m = it.next()?.trim().parse().ok()?;
    let d = it.next()?.trim().parse().ok()?;
    Some((y, m, d))
}

/// **The form with the data put in.** Each mark is replaced by its answer,
/// or by nothing when the data does not answer it.
pub fn fill(form: &Book, data: &Book) -> Book {
    fill_in(form, data, None)
}

/// [`fill`], also putting pictures in the shapes that are fields. A picture
/// file named in the data is looked for in `dir` (the data file's folder).
pub fn fill_in(form: &Book, data_book: &Book, dir: Option<&std::path::Path>) -> Book {
    fill_choices(form, data_book, dir).0
}

/// **A table whose data does not fit the form's rows**: its name, the
/// columns the form's marks use (in the order they first appear), and the
/// rows (1 = the first after the header) to carry on a 別紙.
#[derive(Debug, Clone, PartialEq)]
pub struct Overflow {
    pub table: String,
    pub columns: Vec<String>,
    pub rows: std::ops::RangeInclusive<u32>,
}

/// **The tables that need a 別紙**, from the marks a form holds
/// (`{学歴・職歴.3.内容}`): for each table, the highest row the form has a
/// mark for, against the last row of the data with anything in those
/// columns.
pub fn overflow<'a>(marks: impl IntoIterator<Item = &'a str>, data_book: &Book) -> Vec<Overflow> {
    let data = Data::new(data_book);
    let mut tables: Vec<(String, Vec<String>, u32)> = Vec::new();
    for m in marks {
        let [t, n, col] = m.split('.').collect::<Vec<_>>()[..] else { continue };
        let Ok(n) = n.parse::<u32>() else { continue };
        match tables.iter_mut().find(|(x, _, _)| x == t) {
            Some((_, cols, top)) => {
                if !cols.iter().any(|c| c == col) {
                    cols.push(col.to_string());
                }
                *top = (*top).max(n);
            }
            None => tables.push((t.to_string(), vec![col.to_string()], n)),
        }
    }
    let mut out = Vec::new();
    for (t, cols, top) in tables {
        let Some(s) = data.sheet(&t) else { continue };
        let (rows, _) = s.extent();
        let last = (1..rows).rev().find(|r| cols.iter().any(|c| data.row(&t, *r, c).is_some()));
        if let Some(last) = last.filter(|l| *l > top) {
            out.push(Overflow { table: t, columns: cols, rows: top + 1..=last });
        }
    }
    out
}

/// **The form with a 別紙 sheet for the rows that do not fit**, or the form
/// as it is when every table fits. The 別紙 holds, for each table that
/// overflows, its name with （続き）, a header of the columns, and one row
/// of marks per row to carry (`{学歴・職歴.23.年}` …), so it fills and
/// reports like the rest of the form. Each column takes the width, the
/// format and the row height of the form's cell for it (a merged cell's
/// whole width), with thin lines all round; the page is A4 portrait with
/// the form's margins.
pub fn with_bessi(form: &Book, data: &Book) -> Book {
    let all: Vec<Field> = fields(form);
    let names: Vec<String> = all.iter().flat_map(|f| f.names.iter().cloned()).collect();
    let over = overflow(names.iter().map(String::as_str), data);
    if over.is_empty() {
        return form.clone();
    }
    let mut out = form.clone();
    let first = &form.sheets[0];
    let basis = form.col_basis;
    let mut s = Sheet::new("別紙");
    s.paper_size = Some(9);
    s.margins_mm = first.margins_mm;
    let edge = crate::Edge::THIN;
    let text = |v: &str, fmt: &crate::CellFormat| Cell { formula: None, value: Value::Text(v.into()), fmt: fmt.clone() };
    // The form's cell for a column of a table: its format, its width
    // (a merged cell's whole width) and its row's height
    let cell_of = |t: &str, col: &str| -> (crate::CellFormat, f32, Option<f32>) {
        let found = all.iter().find(|f| {
            f.names.iter().any(|n| {
                let p: Vec<&str> = n.split('.').collect();
                p.len() == 3 && p[0] == t && p[2] == col
            })
        });
        let Some(f) = found else { return (crate::CellFormat::default(), 30.0, None) };
        let sh = &form.sheets[f.sheet];
        let (a, z) = sh
            .merges
            .iter()
            .find(|(a, z)| (a.row..=z.row).contains(&f.at.row) && (a.col..=z.col).contains(&f.at.col))
            .copied()
            .unwrap_or((f.at, f.at));
        let w: f32 = (a.col..=z.col).map(|c| sh.col_haba_mm(c, &basis)).sum();
        let h: f32 = (a.row..=z.row).map(|r| sh.row_height.get(&r).copied().unwrap_or(sh.default_row_height.unwrap_or(crate::DEFAULT_ROW_PT))).sum();
        let fmt = sh.cells.get(&f.at).map(|c| c.fmt.clone()).unwrap_or_default();
        (fmt, w, Some(h))
    };
    let face = first.cells.values().find_map(|c| c.fmt.font.clone());
    let title = crate::CellFormat { size_c: Some(1400), font: face.clone(), ..Default::default() };
    let plain = crate::CellFormat { size_c: Some(1100), font: face, ..Default::default() };
    let mut r = 0u32;
    s.set(Pos::new(r, 0), text("別紙", &title));
    s.row_height.insert(r, 24.0);
    r += 2;
    let mut widths: Vec<f32> = Vec::new();
    for o in &over {
        let cols: Vec<(crate::CellFormat, f32, Option<f32>)> = o.columns.iter().map(|c| cell_of(&o.table, c)).collect();
        let row_h = cols.iter().filter_map(|c| c.2).fold(0.0f32, f32::max);
        s.set(Pos::new(r, 0), text(&format!("{}（続き）", o.table), &plain));
        s.row_height.insert(r, 20.0);
        r += 1;
        let lined = |f: &crate::CellFormat| {
            let mut f = f.clone();
            f.borders = Default::default();
            f.borders.top = edge;
            f.borders.bottom = edge;
            f.borders.left = edge;
            f.borders.right = edge;
            f.fill = None;
            f
        };
        for (c, (col, (fmt, _, _))) in o.columns.iter().zip(&cols).enumerate() {
            let mut head = lined(fmt);
            head.align = crate::HAlign::Center;
            head.valign = crate::VAlign::Middle;
            head.wrap = false;
            s.set(Pos::new(r, c as u32), text(col, &head));
        }
        s.row_height.insert(r, 20.0);
        r += 1;
        for n in o.rows.clone() {
            for (c, (col, (fmt, _, _))) in o.columns.iter().zip(&cols).enumerate() {
                s.set(Pos::new(r, c as u32), text(&format!("{{{}.{n}.{col}}}", o.table), &lined(fmt)));
            }
            if row_h > 0.0 {
                s.row_height.insert(r, row_h);
            }
            r += 1;
        }
        r += 1;
        for (c, (_, w, _)) in cols.iter().enumerate() {
            if widths.len() <= c {
                widths.push(0.0);
            }
            widths[c] = widths[c].max(*w);
        }
    }
    for (c, w) in widths.iter().enumerate() {
        s.set_col_xlsx(c as u32, basis.mm_to_chars(*w), &basis);
    }
    out.sheets.push(s);
    out
}

/// An option to circle: the chars `start..start + len` of the filled cell's
/// text, counted without line breaks.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub sheet: usize,
    pub at: Pos,
    pub start: usize,
    pub len: usize,
}

/// **One text with its marks answered from the data**: the cell's text
/// of a sheet form, or a run's text of a document form. The chosen options
/// of choice marks are pushed to `picked` as (first char, count), counted
/// without line breaks as the page counts them.
pub fn fill_text(t: &str, data: &Data, picked: &mut Vec<(usize, usize)>) -> String {
    let filled = fill_text_ranges(t, data);
    let count = |t: &str| t.chars().filter(|c| *c != '\n').count();
    for (a, b) in &filled.chosen {
        picked.push((count(&filled.text[..*a]), count(&filled.text[*a..*b])));
    }
    filled.text
}

/// A text with its marks answered, and where the answers went.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filled {
    pub text: String,
    /// Each mark and the bytes of `text` its answer took
    pub marks: Vec<(String, usize, usize)>,
    /// The bytes of the chosen options of choice marks
    pub chosen: Vec<(usize, usize)>,
}

/// [`fill_text`], saying in bytes of the filled text where each answer and
/// each chosen option went (a document form asks the layout for them).
pub fn fill_text_ranges(t: &str, data: &Data) -> Filled {
    let mut text = String::new();
    let mut marks = Vec::new();
    let mut chosen = Vec::new();
    let mut rest = t;
    while let Some(a) = rest.find('{') {
        let Some(b) = rest[a..].find('}') else { break };
        text.push_str(&rest[..a]);
        let mark = &rest[a + 1..a + b];
        let from = text.len();
        match choice_of(mark) {
            Some((name, _)) => {
                // The options as the mark writes them, spaces and all
                // (`男 ・ 女`), the chosen one found where it stands
                let pick = data.pair(name).map(|v| v.trim().to_string());
                let raw = mark.split_once(':').map(|(_, o)| o).unwrap_or("");
                for (i, piece) in raw.split('・').enumerate() {
                    if i > 0 {
                        text.push('・');
                    }
                    let o = piece.trim();
                    if !o.is_empty() && pick.as_deref() == Some(o) {
                        let lead = piece.len() - piece.trim_start().len();
                        let at = text.len() + lead;
                        chosen.push((at, at + o.len()));
                    }
                    text.push_str(piece);
                }
            }
            None => text.push_str(&data.answer(mark).unwrap_or_default()),
        }
        marks.push((mark.to_string(), from, text.len()));
        rest = &rest[a + b + 1..];
    }
    text.push_str(rest);
    Filled { text, marks, chosen }
}

/// The options a mark offers: those of a choice (`性別:男・女`), or the one
/// of a box to tick (`送達場所=住所`); none for other marks.
pub fn options_of(mark: &str) -> Vec<String> {
    if let Some((_, o)) = choice_of(mark) {
        return o.iter().map(|x| x.to_string()).collect();
    }
    box_of(mark).map(|(_, o)| vec![o.to_string()]).unwrap_or_default()
}

/// The name and the option of a box to tick (`送達場所=住所`).
fn box_of(mark: &str) -> Option<(&str, &str)> {
    let (name, option) = mark.split_once('=')?;
    let (name, option) = (name.trim(), option.trim());
    (!name.is_empty() && !option.is_empty()).then_some((name, option))
}

/// The name and the options of a choice mark (`性別:男・女`).
fn choice_of(mark: &str) -> Option<(&str, Vec<&str>)> {
    let (name, opts) = mark.split_once(':')?;
    let opts: Vec<&str> = opts.split('・').map(str::trim).filter(|o| !o.is_empty()).collect();
    (!opts.is_empty()).then_some((name.trim(), opts))
}

/// [`fill_in`], also saying which options of the choice marks are chosen.
pub fn fill_choices(
    form: &Book,
    data_book: &Book,
    dir: Option<&std::path::Path>,
) -> (Book, Vec<Choice>) {
    let data = Data::new(data_book);
    let mut choices = Vec::new();
    // Rows that do not fit go on to a 別紙
    let mut out = with_bessi(form, data_book);
    let basis = form.col_basis;
    for s in &mut out.sheets {
        let sheet_sizes = s.clone_sizes(&basis);
        let mut images = Vec::new();
        for list in [&mut s.shapes, &mut s.shapes_new] {
            for sp in list.iter_mut() {
                let Some(name) = sp.field.clone() else { continue };
                let Some(file) = data.pair(&name).filter(|f| !f.trim().is_empty()) else { continue };
                let path = match dir {
                    Some(d) => d.join(file.trim()),
                    None => std::path::PathBuf::from(file.trim()),
                };
                let Ok(bytes) = std::fs::read(&path) else { continue };
                let Some((iw, ih)) = image_size(&bytes) else { continue };
                let (bw, bh) = box_px(&sheet_sizes, basis, sp);
                // Inside the shape, keeping the picture's proportions, centred
                let k = (bw / iw as f32).min(bh / ih as f32);
                let (w, h) = (iw as f32 * k, ih as f32 * k);
                images.push(SheetImage {
                    at: sp.at,
                    dx_px: sp.dx_px + (bw - w) / 2.0,
                    dy_px: sp.dy_px + (bh - h) / 2.0,
                    width_px: w,
                    height_px: h,
                    data: bytes,
                });
                // The instructions in the box give way to the photo; the frame stays
                sp.text = None;
            }
        }
        s.images_new.extend(images);
    }
    for (si, s) in out.sheets.iter_mut().enumerate() {
        let keys: Vec<Pos> = s.cells.keys().copied().collect();
        for p in keys {
            let Some(c) = s.cells.get(&p) else { continue };
            let Value::Text(t) = &c.value else { continue };
            if marks(t).is_empty() {
                continue;
            }
            let mut picked = Vec::new();
            let text = fill_text(t, &data, &mut picked);
            choices.extend(picked.into_iter().map(|(start, len)| Choice { sheet: si, at: p, start, len }));
            let fmt = c.fmt.clone();
            s.set(p, Cell { formula: None, value: Value::Text(text), fmt });
        }
    }
    (out, choices)
}

/// The column widths and row heights a shape's box is measured with.
struct Sizes {
    col_mm: Vec<f32>,
    row_pt: std::collections::BTreeMap<u32, f32>,
    default_row_pt: f32,
}

impl Sheet {
    /// The widths of the columns shapes reach, and the row heights
    fn clone_sizes(&self, basis: &crate::ColBasis) -> Sizes {
        let last = self.shapes.iter().chain(self.shapes_new.iter())
            .filter_map(|sp| sp.to.map(|(p, _, _)| p.col))
            .max()
            .unwrap_or(0);
        Sizes {
            col_mm: (0..=last).map(|c| self.col_haba_mm(c, basis)).collect(),
            row_pt: self.row_height.clone(),
            default_row_pt: self.default_row_height.unwrap_or(crate::DEFAULT_ROW_PT),
        }
    }
}

/// **The size (px) a shape is drawn at.** A shape held by two cells
/// (`to`, xlsx twoCellAnchor) stretches with them, so its box comes from
/// the columns and rows it spans, as the page draws it; otherwise it is the
/// size the shape states.
fn box_px(z: &Sizes, basis: crate::ColBasis, sp: &crate::SheetShape) -> (f32, f32) {
    let Some((to, tdx, tdy)) = sp.to else { return (sp.width_px, sp.height_px) };
    if to.col < sp.at.col || to.row < sp.at.row {
        return (sp.width_px, sp.height_px);
    }
    let px_per_mm = 96.0 / 25.4;
    let w_mm: f32 = (sp.at.col..to.col)
        .map(|c| z.col_mm.get(c as usize).copied().unwrap_or_else(|| basis.default_mm(8.0)))
        .sum();
    let h_pt: f32 = (sp.at.row..to.row).map(|r| z.row_pt.get(&r).copied().unwrap_or(z.default_row_pt)).sum();
    (
        (w_mm * px_per_mm - sp.dx_px + tdx).max(1.0),
        (h_pt * 96.0 / 72.0 - sp.dy_px + tdy).max(1.0),
    )
}

/// **The names the form asks for that the data does not hold at all**, in
/// the order the form first asks. They are filled with nothing like any
/// other unanswered mark, but a name missing altogether is usually a
/// mistake (電話番号 written for 電話), so the caller reports it. Not
/// listed: rows beyond the end of a table (the form prepares more rows than
/// the data fills) and names that are there with an empty value (an
/// optional field left blank). A missing table or column is named as
/// `表` or `表.列`.
pub fn missing(form: &Book, data_book: &Book) -> Vec<String> {
    let mut names: Vec<String> = fields(form).into_iter().flat_map(|f| f.names).collect();
    for s in &form.sheets {
        names.extend(s.shapes.iter().chain(s.shapes_new.iter()).filter_map(|sp| sp.field.clone()));
    }
    missing_of(names, data_book)
}

/// [`missing`] for marks found elsewhere (a document form's runs).
pub fn missing_of(names: Vec<String>, data_book: &Book) -> Vec<String> {
    let data = Data::new(data_book);
    let mut out: Vec<String> = Vec::new();
    let mut push = |n: String| {
        if !out.contains(&n) {
            out.push(n);
        }
    };
    for m in names {
        let m = choice_of(&m)
            .map(|(n, _)| n.to_string())
            .or_else(|| box_of(&m).map(|(n, _)| n.to_string()))
            .unwrap_or(m);
        let parts: Vec<&str> = m.split('.').collect();
        match parts.as_slice() {
            ["年齢"] => {
                for key in ["生年月日", "日付"] {
                    if !data.has_pair(key) {
                        push(key.to_string());
                    }
                }
            }
            [t, n, col] if n.parse::<u32>().is_ok() => {
                if data.sheet(t).is_none() {
                    push(t.to_string());
                } else if !data.has_column(t, col) {
                    push(format!("{t}.{col}"));
                }
            }
            [key, ..] => {
                if !data.has_pair(key) {
                    push(key.to_string());
                }
            }
            [] => {}
        }
    }
    out
}

/// The width and height of a PNG or JPEG picture.
fn image_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.starts_with(b"\x89PNG") && b.len() >= 24 {
        let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
        return Some((w, h));
    }
    if b.starts_with(&[0xFF, 0xD8]) {
        // Walk the segments to a start-of-frame marker (C0–CF but C4, C8, CC)
        let mut i = 2;
        while i + 9 < b.len() {
            if b[i] != 0xFF {
                i += 1;
                continue;
            }
            let m = b[i + 1];
            let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
            if (0xC0..=0xCF).contains(&m) && ![0xC4, 0xC8, 0xCC].contains(&m) {
                let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
                let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
                return (w > 0 && h > 0).then_some((w, h));
            }
            i += 2 + len;
        }
    }
    None
}

/// What kind of input a field takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Multiline,
    Date,
    Image,
    /// One of the options written in the mark (`性別:男・女`)
    Choice,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Multiline => "multiline",
            Kind::Date => "date",
            Kind::Image => "image",
            Kind::Choice => "choice",
        }
    }
}

/// One field to edit: a data item and the places of the form showing it.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub kind: Kind,
    /// The value the data holds (empty when it holds none)
    pub value: String,
    /// The cells (sheet, position) whose marks stand for this item
    pub cells: Vec<(usize, Pos)>,
    /// The shape (sheet, index in `shapes` then `shapes_new`) for a picture
    pub shape: Option<(usize, usize)>,
    /// The options of a choice
    pub options: Vec<String>,
}

/// The data item a mark stands for, and its kind. `年齢` is worked out, so
/// it is not an item.
pub fn item_of(mark: &str) -> Option<(String, Kind)> {
    if mark == "年齢" {
        return None;
    }
    if let Some((name, _)) = choice_of(mark) {
        return Some((name.to_string(), Kind::Choice));
    }
    if let Some((name, _)) = box_of(mark) {
        return Some((name.to_string(), Kind::Choice));
    }
    Some(match mark.split('.').collect::<Vec<_>>().as_slice() {
        [key, "年" | "月" | "日" | "元号" | "和暦年"] => (key.to_string(), Kind::Date),
        [key, "時間" | "分"] => (key.to_string(), Kind::Text),
        [key, n] if n.parse::<u32>().is_ok() => (key.to_string(), Kind::Multiline),
        _ => (mark.to_string(), Kind::Text),
    })
}

/// **The fields of a form, one per data item**, in the order they first
/// appear (by sheet, row and column), with the values the data holds.
pub fn groups(form: &Book, data_book: &Book) -> Vec<Group> {
    let bessi = with_bessi(form, data_book);
    let form = &bessi;
    let data = Data::new(data_book);
    let mut out: Vec<Group> = Vec::new();
    for f in fields(form) {
        let wraps = form.sheets[f.sheet].cells.get(&f.at).is_some_and(|c| c.fmt.wrap);
        for m in &f.names {
            let Some((name, kind)) = item_of(m) else { continue };
            let g = match out.iter_mut().position(|g| g.name == name) {
                Some(i) => &mut out[i],
                None => {
                    let options = choice_of(m)
                        .map(|(_, o)| o.iter().map(|x| x.to_string()).collect())
                        .unwrap_or_default();
                    out.push(Group { name: name.clone(), kind, value: String::new(),
                                     cells: Vec::new(), shape: None, options });
                    out.last_mut().expect("just pushed")
                }
            };
            if wraps && g.kind == Kind::Text {
                g.kind = Kind::Multiline;
            }
            // Each box of a name adds its option
            if let Some((_, option)) = box_of(m) {
                if !g.options.iter().any(|o| o == option) {
                    g.options.push(option.to_string());
                }
            }
            if !g.cells.contains(&(f.sheet, f.at)) {
                g.cells.push((f.sheet, f.at));
            }
        }
    }
    for (si, s) in form.sheets.iter().enumerate() {
        for (k, sp) in s.shapes.iter().chain(s.shapes_new.iter()).enumerate() {
            if let Some(name) = &sp.field {
                out.push(Group { name: name.clone(), kind: Kind::Image, value: String::new(),
                                 cells: Vec::new(), shape: Some((si, k)), options: Vec::new() });
            }
        }
    }
    for g in &mut out {
        g.value = data.raw(&g.name).unwrap_or_default();
        if g.kind == Kind::Text && g.value.contains('\n') {
            g.kind = Kind::Multiline;
        }
    }
    out
}

/// **Writes a field's new value into the data.** A table field
/// (`学歴・職歴.3.年`) sets that row and column of the table, adding rows
/// as needed; any other name sets the value next to it in a two-column
/// table, or adds a row to the first such table when the name is new.
pub fn set(data: &mut Book, name: &str, value: &str) -> Result<(), String> {
    let text = |v: &str| Cell { value: Value::Text(v.to_string()), ..Default::default() };
    if let [t, n, col] = name.split('.').collect::<Vec<_>>().as_slice() {
        if let (Ok(n), Some(s)) = (n.parse::<u32>(), data.sheets.iter_mut().find(|s| s.name == *t)) {
            let (_, cols) = s.extent();
            let c = (0..cols)
                .find(|c| s.value(Pos::new(0, *c)).display().trim() == *col)
                .ok_or_else(|| format!("{t} に列「{col}」がありません"))?;
            s.set(Pos::new(n, c), text(value));
            return Ok(());
        }
    }
    let two_cols = |s: &Sheet| s.extent().1 == 2;
    for s in data.sheets.iter_mut().filter(|s| two_cols(s)) {
        let (rows, _) = s.extent();
        if let Some(r) = (0..rows).find(|r| s.value(Pos::new(*r, 0)).display().trim() == name) {
            s.set(Pos::new(r, 1), text(value));
            return Ok(());
        }
    }
    let s = data
        .sheets
        .iter_mut()
        .find(|s| two_cols(s))
        .ok_or("データに「名前・値」の 2 列の表がありません")?;
    let (rows, _) = s.extent();
    s.set(Pos::new(rows, 0), text(name));
    s.set(Pos::new(rows, 1), text(value));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Book {
        let mut b = Book::new();
        b.sheets.clear();
        let mut kihon = Sheet::new("基本");
        for (r, (k, v)) in [("日付", "2026-09-24"), ("氏名", "山田 太郎"), ("生年月日", "1990-10-01")]
            .iter()
            .enumerate()
        {
            kihon.set(Pos::new(r as u32, 0), Cell::input(k));
            kihon.set(Pos::new(r as u32, 1), Cell { value: Value::Text(v.to_string()), ..Default::default() });
        }
        let mut reki = Sheet::new("学歴・職歴");
        for (c, h) in ["年", "月", "内容"].iter().enumerate() {
            reki.set(Pos::new(0, c as u32), Cell::input(h));
        }
        reki.set(Pos::new(1, 0), Cell::input("2009"));
        reki.set(Pos::new(1, 2), Cell::input("千代田大学 入学"));
        let mut jiyu = Sheet::new("自由記入");
        jiyu.set(Pos::new(0, 0), Cell::input("本人希望"));
        jiyu.set(Pos::new(0, 1), Cell { value: Value::Text("1 行目\n2 行目".into()), ..Default::default() });
        b.sheets = vec![kihon, reki, jiyu];
        b
    }

    #[test]
    fn marks_are_filled_from_the_data() {
        let mut form = Book::new();
        let s = &mut form.sheets[0];
        s.set(Pos::new(0, 0), Cell { value: Value::Text("{日付.年}年{日付.月}月{日付.日}日現在".into()), ..Default::default() });
        s.set(Pos::new(1, 0), Cell { value: Value::Text("{氏名}".into()), ..Default::default() });
        s.set(Pos::new(2, 0), Cell { value: Value::Text("(満{年齢}歳)".into()), ..Default::default() });
        s.set(Pos::new(3, 0), Cell { value: Value::Text("{学歴・職歴.1.年}".into()), ..Default::default() });
        s.set(Pos::new(3, 1), Cell { value: Value::Text("{学歴・職歴.1.内容}".into()), ..Default::default() });
        s.set(Pos::new(4, 0), Cell { value: Value::Text("{学歴・職歴.2.年}".into()), ..Default::default() });
        s.set(Pos::new(5, 0), Cell { value: Value::Text("{本人希望.2}".into()), ..Default::default() });
        s.set(Pos::new(6, 0), Cell { value: Value::Text("{通勤時間}".into()), ..Default::default() });
        s.set(Pos::new(7, 0), Cell::input("見出し"));
        assert_eq!(fields(&form).len(), 8);
        let out = fill(&form, &data());
        let v = |r, c| out.sheets[0].value(Pos::new(r, c)).display();
        assert_eq!(v(0, 0), "2026年9月24日現在");
        assert_eq!(v(1, 0), "山田 太郎");
        assert_eq!(v(2, 0), "(満35歳)", "born 1990-10-01, on 2026-09-24");
        assert_eq!(v(3, 0), "2009");
        assert_eq!(v(3, 1), "千代田大学 入学");
        assert_eq!(v(4, 0), "", "a row the data does not have is empty");
        assert_eq!(v(5, 0), "2 行目");
        assert_eq!(v(6, 0), "", "a field the data leaves out is empty");
        assert_eq!(v(7, 0), "見出し");
    }

    #[test]
    fn a_date_is_given_in_the_japanese_era() {
        let era = |v: &str| {
            let mut d = data();
            set(&mut d, "日付", v).unwrap();
            let dd = Data::new(&d);
            (dd.answer("日付.元号").unwrap_or_default(), dd.answer("日付.和暦年").unwrap_or_default())
        };
        assert_eq!(era("2026-09-25"), ("令和".into(), "8".into()));
        assert_eq!(era("2019-05-01"), ("令和".into(), "元".into()));
        assert_eq!(era("2019-04-30"), ("平成".into(), "31".into()));
        assert_eq!(era("1989-01-07"), ("昭和".into(), "64".into()));
        assert_eq!(era("1990-05-01"), ("平成".into(), "2".into()));
    }

    #[test]
    fn hours_and_minutes_are_taken_from_a_time() {
        let mut d = data();
        set(&mut d, "通勤時間", "1時間10分").unwrap();
        let dd = Data::new(&d);
        assert_eq!(dd.answer("通勤時間.時間").as_deref(), Some("1"));
        assert_eq!(dd.answer("通勤時間.分").as_deref(), Some("10"));
        let mut d = data();
        set(&mut d, "通勤時間", "45分").unwrap();
        let dd = Data::new(&d);
        assert_eq!(dd.answer("通勤時間.時間"), None);
        assert_eq!(dd.answer("通勤時間.分").as_deref(), Some("45"));
    }

    #[test]
    fn marks_are_gathered_by_data_item() {
        let mut form = Book::new();
        let s = &mut form.sheets[0];
        let t = |v: &str| Cell { value: Value::Text(v.into()), ..Default::default() };
        s.set(Pos::new(0, 0), t("{日付.年}年{日付.月}月{日付.日}日現在"));
        s.set(Pos::new(1, 0), t("{生年月日.年}年 (満{年齢}歳)"));
        s.set(Pos::new(2, 0), t("{本人希望.1}"));
        s.set(Pos::new(3, 0), t("{本人希望.2}"));
        s.set(Pos::new(4, 0), t("{学歴・職歴.1.内容}"));
        s.shapes.push(crate::SheetShape { field: Some("写真".into()), ..Default::default() });
        let g = groups(&form, &data());
        let names: Vec<&str> = g.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["日付", "生年月日", "本人希望", "学歴・職歴.1.内容", "写真"]);
        assert_eq!(g[0].kind, Kind::Date);
        assert_eq!(g[0].value, "2026-09-24");
        assert_eq!(g[2].kind, Kind::Multiline);
        assert_eq!(g[2].cells.len(), 2);
        assert_eq!(g[2].value, "1 行目\n2 行目");
        assert_eq!(g[3].value, "千代田大学 入学");
        assert_eq!(g[4].kind, Kind::Image);
    }

    #[test]
    fn a_choice_shows_its_options_and_says_which_is_chosen() {
        let mut form = Book::new();
        let t = |v: &str| Cell { value: Value::Text(v.into()), ..Default::default() };
        form.sheets[0].set(Pos::new(0, 0), t("※ {性別:男・女}"));
        form.sheets[0].set(Pos::new(1, 0), t("{配偶者:有・無}"));
        let mut d = data();
        set(&mut d, "性別", "女").unwrap();
        let (out, ch) = fill_choices(&form, &d, None);
        assert_eq!(out.sheets[0].value(Pos::new(0, 0)).display(), "※ 男・女");
        // 女 is the 5th char of "※ 男・女"; 配偶者 is not in the data, so none is circled
        assert_eq!(ch, [Choice { sheet: 0, at: Pos::new(0, 0), start: 4, len: 1 }]);
        assert_eq!(out.sheets[0].value(Pos::new(1, 0)).display(), "有・無");
        // Spaces round the options stay, and the chosen one is found past them
        form.sheets[0].set(Pos::new(0, 0), t("{性別:男 ・ 女}"));
        let (out, ch) = fill_choices(&form, &d, None);
        assert_eq!(out.sheets[0].value(Pos::new(0, 0)).display(), "男 ・ 女");
        assert_eq!((ch[0].start, ch[0].len), (4, 1));
        let g = groups(&form, &d);
        assert_eq!((g[0].name.as_str(), g[0].kind, g[0].value.as_str()), ("性別", Kind::Choice, "女"));
        assert_eq!(g[0].options, ["男", "女"]);
        assert_eq!(missing(&form, &d), ["配偶者"]);
    }

    #[test]
    fn rows_that_do_not_fit_go_on_to_a_bessi() {
        let mut form = Book::new();
        let t = |v: &str| Cell { value: Value::Text(v.into()), ..Default::default() };
        // the form has room for one row of the history
        form.sheets[0].set(Pos::new(0, 0), t("{学歴・職歴.1.年}"));
        form.sheets[0].set(Pos::new(0, 1), t("{学歴・職歴.1.内容}"));
        let mut d = data();
        set(&mut d, "学歴・職歴.2.年", "2013").unwrap();
        set(&mut d, "学歴・職歴.2.内容", "千代田大学 卒業").unwrap();
        set(&mut d, "学歴・職歴.3.内容", "株式会社千代田商事 入社").unwrap();
        let over = overflow(["学歴・職歴.1.年", "学歴・職歴.1.内容"], &d);
        assert_eq!(over, [Overflow { table: "学歴・職歴".into(), columns: vec!["年".into(), "内容".into()], rows: 2..=3 }]);
        let out = fill(&form, &d);
        assert_eq!(out.sheets.len(), 2);
        let b = &out.sheets[1];
        assert_eq!(b.name, "別紙");
        let texts: Vec<String> = b.cells.values().map(|c| c.value.display()).filter(|v| !v.is_empty()).collect();
        for want in ["別紙", "学歴・職歴（続き）", "年", "内容", "2013", "千代田大学 卒業", "株式会社千代田商事 入社"] {
            assert!(texts.iter().any(|t| t == want), "{want} is not on the 別紙: {texts:?}");
        }
        // the form's own row keeps the first
        assert_eq!(out.sheets[0].value(Pos::new(0, 1)).display(), "千代田大学 入学");
        // a table that fits needs none
        assert_eq!(fill(&form, &data()).sheets.len(), 1);
        // the 別紙's rows are fields too
        let g = groups(&form, &d);
        assert!(g.iter().any(|g| g.name == "学歴・職歴.3.内容"));
    }

    #[test]
    fn a_box_is_ticked_for_the_chosen_option() {
        let mut form = Book::new();
        let t = |v: &str| Cell { value: Value::Text(v.into()), ..Default::default() };
        form.sheets[0].set(Pos::new(0, 0), t("{送達場所=住所}住所"));
        form.sheets[0].set(Pos::new(1, 0), t("{送達場所=勤務先}勤務先"));
        form.sheets[0].set(Pos::new(2, 0), t("{受取人=親族}親族"));
        let mut d = data();
        set(&mut d, "送達場所", "勤務先").unwrap();
        let out = fill(&form, &d);
        let v = |r| out.sheets[0].value(Pos::new(r, 0)).display();
        assert_eq!((v(0), v(1), v(2)), ("□住所".into(), "■勤務先".into(), "□親族".into()));
        let g = groups(&form, &d);
        assert_eq!((g[0].name.as_str(), g[0].kind), ("送達場所", Kind::Choice));
        assert_eq!(g[0].options, ["住所", "勤務先"]);
        assert_eq!(g[0].value, "勤務先");
        assert_eq!(missing(&form, &d), ["受取人"]);
    }

    #[test]
    fn names_the_data_lacks_altogether_are_listed() {
        let mut form = Book::new();
        let s = &mut form.sheets[0];
        let t = |v: &str| Cell { value: Value::Text(v.into()), ..Default::default() };
        s.set(Pos::new(0, 0), t("{氏名} {電話} {性別}"));
        s.set(Pos::new(1, 0), t("{学歴・職歴.9.年}"));
        s.set(Pos::new(2, 0), t("{学歴・職歴.1.場所} {免許・資格.1.年}"));
        s.set(Pos::new(3, 0), t("(満{年齢}歳) {本人希望.3}"));
        s.shapes.push(crate::SheetShape { field: Some("写真".into()), ..Default::default() });
        let mut d = data();
        set(&mut d, "性別", "").unwrap();
        // 性別 is there but empty, row 9 of the history is past its end,
        // and 本人希望 has fewer lines: none of those is missing
        assert_eq!(missing(&form, &d), ["電話", "学歴・職歴.場所", "免許・資格", "写真"]);
    }

    #[test]
    fn a_new_value_is_written_back_into_the_data() {
        let mut d = data();
        set(&mut d, "氏名", "山田 花子").unwrap();
        set(&mut d, "学歴・職歴.2.内容", "千代田大学 卒業").unwrap();
        set(&mut d, "通勤時間", "約 30 分").unwrap();
        let dd = Data::new(&d);
        assert_eq!(dd.raw("氏名").as_deref(), Some("山田 花子"));
        assert_eq!(dd.raw("学歴・職歴.2.内容").as_deref(), Some("千代田大学 卒業"));
        assert_eq!(dd.raw("通勤時間").as_deref(), Some("約 30 分"));
        assert!(set(&mut d, "学歴・職歴.1.場所", "x").is_err());
    }

    #[test]
    fn a_photo_goes_inside_its_box() {
        let dir = std::env::temp_dir().join(format!("form-photo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A PNG header is enough: 300 × 400
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&300u32.to_be_bytes());
        png.extend_from_slice(&400u32.to_be_bytes());
        std::fs::write(dir.join("p.png"), &png).unwrap();
        let mut d = data();
        set(&mut d, "写真", "p.png").unwrap();
        let mut form = Book::new();
        form.sheets[0].shapes.push(crate::SheetShape {
            field: Some("写真".into()), width_px: 120.0, height_px: 200.0,
            text: Some("写真をはる位置".into()), ..Default::default()
        });
        let out = fill_in(&form, &d, Some(&dir));
        let im = &out.sheets[0].images_new[0];
        assert_eq!((im.width_px, im.height_px), (120.0, 160.0));
        assert_eq!((im.dx_px, im.dy_px), (0.0, 20.0));
        assert_eq!(out.sheets[0].shapes[0].text, None);

        // A box held by two cells takes its size from the rows it spans:
        // two rows of 30pt are 80px high, so the picture is 60 × 80
        let mut form = Book::new();
        form.sheets[0].row_height.insert(0, 30.0);
        form.sheets[0].row_height.insert(1, 30.0);
        form.sheets[0].shapes.push(crate::SheetShape {
            field: Some("写真".into()), width_px: 120.0, height_px: 200.0,
            to: Some((Pos::new(2, 5), 0.0, 0.0)), ..Default::default()
        });
        let out = fill_in(&form, &d, Some(&dir));
        let im = &out.sheets[0].images_new[0];
        assert!((im.height_px - 80.0).abs() < 0.01, "{}", im.height_px);
        assert!((im.width_px - 60.0).abs() < 0.01, "{}", im.width_px);
        std::fs::remove_dir_all(&dir).ok();
    }
}
