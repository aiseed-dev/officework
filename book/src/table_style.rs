//! **The look a built-in table style gives a cell** (fill, font colour, bold
//! and the four edges).
//!
//! The styles are ECMA-376's presetTableStyles.xml, written by
//! `tools/gen_table_styles.py` as `table_styles.txt`. A table read from an
//! xlsx names its style (`<tableStyleInfo name="TableStyleMedium9"/>`) and
//! keeps no colours of its own, so without this the paper and the screen drew
//! the table plain while Excel drew its header and stripes (2026-10-13).
//!
//! The elements are laid on in the order of ECMA-376 Part 1, 18.8.40
//! (tableStyleElement): whole table, column stripes, row stripes, last
//! column, first column, header row, total row, then the corner cells; a
//! later one wins. A side of an element's border applies to the outer edge
//! of the element's area, `vertical` and `horizontal` to the lines inside it.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::{BStyle, Edge, Pos, TableDef};

const DATA: &str = include_str!("table_styles.txt");

/// A theme colour: index and tint
type Tc = (u8, f32);

#[derive(Debug, Clone)]
struct El {
    size: u32,
    fill: Option<Tc>,
    font: Option<Tc>,
    bold: bool,
    /// top, bottom, left, right, vertical, horizontal
    sides: [Option<(BStyle, Tc)>; 6],
}

fn tc(s: &str) -> Option<Tc> {
    if s == "-" {
        return None;
    }
    let mut it = s.split(':');
    let i = it.next()?.parse().ok()?;
    let t = it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    Some((i, t))
}

fn presets() -> &'static HashMap<String, Vec<(String, El)>> {
    static P: OnceLock<HashMap<String, Vec<(String, El)>>> = OnceLock::new();
    P.get_or_init(|| {
        let mut m: HashMap<String, Vec<(String, El)>> = HashMap::new();
        for line in DATA.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            let f: Vec<&str> = line.split(' ').collect();
            if f.len() != 12 {
                continue;
            }
            let side = |s: &str| -> Option<(BStyle, Tc)> {
                let (st, c) = s.split_once(':')?;
                Some((BStyle::from_xlsx(st), tc(c)?))
            };
            let el = El {
                size: f[2].parse().unwrap_or(1).max(1),
                fill: tc(f[3]),
                font: tc(f[4]),
                bold: f[5] == "b",
                sides: [side(f[6]), side(f[7]), side(f[8]), side(f[9]), side(f[10]), side(f[11])],
            };
            m.entry(f[0].to_string()).or_default().push((f[1].to_string(), el));
        }
        m
    })
}

/// What a table style gives one cell. None: the style says nothing of it
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableLook {
    pub fill: Option<String>,
    pub color: Option<String>,
    pub bold: Option<bool>,
    pub top: Option<Edge>,
    pub bottom: Option<Edge>,
    pub left: Option<Edge>,
    pub right: Option<Edge>,
}

impl TableLook {
    /// **Lay the table's look under a cell's own format**: what the cell
    /// states (its fill, its font colour, a drawn edge) stays, and the rest
    /// takes the table's. The paper and the screen both draw with this
    pub fn apply(&self, f: &mut crate::CellFormat) {
        if f.fill.is_none() {
            f.fill = self.fill.clone();
        }
        if f.color.is_none() {
            f.color = self.color.clone();
        }
        if self.bold == Some(true) {
            f.bold = true;
        }
        for (own, from) in [(&mut f.borders.top, self.top), (&mut f.borders.bottom, self.bottom),
                            (&mut f.borders.left, self.left), (&mut f.borders.right, self.right)] {
            if !own.on {
                if let Some(e) = from {
                    *own = e;
                }
            }
        }
    }
}

/// The order of ECMA-376 18.8.40, earliest first
const ORDER: [&str; 13] = [
    "wholeTable", "firstColumnStripe", "secondColumnStripe", "firstRowStripe", "secondRowStripe",
    "lastColumn", "firstColumn", "headerRow", "totalRow", "firstHeaderCell", "lastHeaderCell",
    "firstTotalCell", "lastTotalCell",
];

/// **The look of cell `p` in table `t`** with the book's theme colours
/// (`Book::theme`; empty means the default). None when `p` is outside the
/// table or its style is not a built-in one
pub fn table_look(t: &TableDef, theme: &[String], p: Pos) -> Option<TableLook> {
    if p.row < t.a.row || p.row > t.b.row || p.col < t.a.col || p.col > t.b.col {
        return None;
    }
    // An empty name: the file gives the table no style
    let name = t.style.as_deref().unwrap_or("TableStyleMedium2");
    if name.is_empty() {
        return None;
    }
    let els = presets().get(name)?;
    let (r0, r1, c0, c1) = (t.a.row, t.b.row, t.a.col, t.b.col);
    // The data rows: without the header and the total row
    let d0 = if t.header { r0 + 1 } else { r0 };
    let d1 = if t.totals && r1 > d0 { r1 - 1 } else { r1 };
    let in_data = p.row >= d0 && p.row <= d1;
    let colour = |c: Tc| crate::theme::resolve(theme, c.0, c.1);
    let mut out = TableLook::default();
    for kind in ORDER {
        let Some((_, el)) = els.iter().find(|(k, _)| k == kind) else { continue };
        // The area this element covers around `p` (rows, columns), if any
        let area: Option<(u32, u32, u32, u32)> = match kind {
            "wholeTable" => Some((r0, r1, c0, c1)),
            "firstColumnStripe" | "secondColumnStripe" if t.banded_cols && in_data => {
                let n = (p.col - c0) / el.size;
                let first = n % 2 == 0;
                (first == (kind == "firstColumnStripe")).then(|| {
                    let s = c0 + n * el.size;
                    (d0, d1, s, (s + el.size - 1).min(c1))
                })
            }
            "firstRowStripe" | "secondRowStripe" if t.banded_rows && in_data => {
                let n = (p.row - d0) / el.size;
                let first = n % 2 == 0;
                (first == (kind == "firstRowStripe")).then(|| {
                    let s = d0 + n * el.size;
                    (s, (s + el.size - 1).min(d1), c0, c1)
                })
            }
            "lastColumn" if t.last_col && p.col == c1 => Some((r0, r1, c1, c1)),
            "firstColumn" if t.first_col && p.col == c0 => Some((r0, r1, c0, c0)),
            "headerRow" if t.header && p.row == r0 => Some((r0, r0, c0, c1)),
            "totalRow" if t.totals && p.row == r1 => Some((r1, r1, c0, c1)),
            "firstHeaderCell" if t.header && t.first_col && p.row == r0 && p.col == c0 => Some((r0, r0, c0, c0)),
            "lastHeaderCell" if t.header && t.last_col && p.row == r0 && p.col == c1 => Some((r0, r0, c1, c1)),
            "firstTotalCell" if t.totals && t.first_col && p.row == r1 && p.col == c0 => Some((r1, r1, c0, c0)),
            "lastTotalCell" if t.totals && t.last_col && p.row == r1 && p.col == c1 => Some((r1, r1, c1, c1)),
            _ => None,
        };
        let Some((ar0, ar1, ac0, ac1)) = area else { continue };
        if let Some(c) = el.fill {
            out.fill = Some(colour(c));
        }
        if let Some(c) = el.font {
            out.color = Some(colour(c));
        }
        if el.bold {
            out.bold = Some(true);
        }
        let edge = |s: Option<(BStyle, Tc)>| -> Option<Edge> {
            let (st, c) = s?;
            Some(Edge::line(st, u32::from_str_radix(&colour(c), 16).ok()))
        };
        let [top, bottom, left, right, vertical, horizontal] = el.sides;
        let pick = |outer: bool, side: Option<(BStyle, Tc)>, inner: Option<(BStyle, Tc)>| {
            if outer { edge(side) } else { edge(inner) }
        };
        if let Some(e) = pick(p.row == ar0, top, horizontal) {
            out.top = Some(e);
        }
        if let Some(e) = pick(p.row == ar1, bottom, horizontal) {
            out.bottom = Some(e);
        }
        if let Some(e) = pick(p.col == ac0, left, vertical) {
            out.left = Some(e);
        }
        if let Some(e) = pick(p.col == ac1, right, vertical) {
            out.right = Some(e);
        }
    }
    Some(out)
}

/// [`table_look`] for the first table of `tables` that holds `p`
pub fn look_at(tables: &[TableDef], theme: &[String], p: Pos) -> Option<TableLook> {
    tables.iter().find_map(|t| table_look(t, theme, p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn medium9() -> TableDef {
        TableDef {
            name: "Sales".into(),
            style: Some("TableStyleMedium9".into()),
            a: Pos::new(2, 0),
            b: Pos::new(6, 4),
            header: true,
            totals: false,
            banded_rows: true,
            banded_cols: false,
            first_col: false,
            last_col: false,
            filter: true,
        }
    }

    /// TableStyleMedium9 in Excel's 2023 theme: the header is bold white on
    /// accent 1, and the data rows stripe 60% and 80% lighter accent 1
    /// (Excel 16.0.20430's PDF: #156082, #83CCEB, #C0E6F5)
    #[test]
    fn medium9_gives_the_header_and_the_stripes() {
        let theme: Vec<String> = crate::theme::OFFICE.iter().map(|s| s.to_string()).collect();
        let t = medium9();
        let h = table_look(&t, &theme, Pos::new(2, 1)).unwrap();
        assert_eq!(h.fill.as_deref(), Some(crate::theme::OFFICE[4]));
        assert_eq!(h.color.as_deref(), Some(crate::theme::OFFICE[0]));
        assert_eq!(h.bold, Some(true));
        let first = table_look(&t, &theme, Pos::new(3, 1)).unwrap();
        let second = table_look(&t, &theme, Pos::new(4, 1)).unwrap();
        assert_eq!(first.fill.as_deref(), Some(crate::theme::resolve(&theme, 4, 0.599994).as_str()));
        assert_eq!(second.fill.as_deref(), Some(crate::theme::resolve(&theme, 4, 0.799982).as_str()));
        assert!(table_look(&t, &theme, Pos::new(7, 1)).is_none());
    }
}
