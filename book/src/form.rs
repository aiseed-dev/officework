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
//! - `{年齢}`: the age on 日付 of someone born on 生年月日
//! - `{学歴・職歴.3.内容}`: row 3 (after the header) of the table 学歴・職歴,
//!   column 内容
//! - `{本人希望.2}`: line 2 of a value written on several lines

use crate::{Book, Cell, Pos, Sheet, Value};

/// One field of a form: the cell that holds marks, and the names they use.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub sheet: usize,
    pub at: Pos,
    /// The mark names in the cell, in order (`氏名`, `日付.年`, …)
    pub names: Vec<String>,
}

/// The marks in a piece of text, in order.
fn marks(text: &str) -> Vec<String> {
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

    /// What one mark stands for; None when the data does not say.
    pub fn answer(&self, mark: &str) -> Option<String> {
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
    let data = Data::new(data);
    let mut out = form.clone();
    for s in &mut out.sheets {
        let keys: Vec<Pos> = s.cells.keys().copied().collect();
        for p in keys {
            let Some(c) = s.cells.get(&p) else { continue };
            let Value::Text(t) = &c.value else { continue };
            if marks(t).is_empty() {
                continue;
            }
            let mut text = String::new();
            let mut rest = t.as_str();
            while let Some(a) = rest.find('{') {
                let Some(b) = rest[a..].find('}') else { break };
                text.push_str(&rest[..a]);
                text.push_str(&data.answer(&rest[a + 1..a + b]).unwrap_or_default());
                rest = &rest[a + b + 1..];
            }
            text.push_str(rest);
            let fmt = c.fmt.clone();
            s.set(p, Cell { formula: None, value: Value::Text(text), fmt });
        }
    }
    out
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
}
