//! **表のデザイン** — 見出しの帯・合計行・縞々・最初と最後の列・範囲へ戻す。
//!
//! 画面(calc のリボンの「表のデザイン」)と **Python の口**(ops の
//! `table_style` / `table_total` / `table_to_range`)が**同じここ**を呼ぶ。
//!
//! 2026-08-16 に calc から移した。理由は記録の穴 — 手で押すと
//! 「この操作はまだ Python で書けません」と註が出ていた(発注者
//! 「これを全部走るようにしろ」)。画面にしか無い操作は、記録しても走らない。
//! **画面と Python が同じ実装を呼ぶなら、穴は原理的に開かない。**

use crate::model::{Cell, Edge, Pos, Sheet, Value};

/// 表の飾り。`td-*` のボタンと1対1
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Deco {
    /// 1行目を見出しの帯に(太字・薄緑・上罫線)
    Header,
    /// 1行おきの縞々
    BandRow,
    /// 1列おきの縞々
    BandCol,
    /// 最初の列を太字に
    FirstCol,
    /// 最後の列を太字に
    LastCol,
}

impl Deco {
    /// Python の口で使う名前(`s["A1:D9"].table_style("header")`)
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "header" => Deco::Header,
            "band_row" => Deco::BandRow,
            "band_col" => Deco::BandCol,
            "first_col" => Deco::FirstCol,
            "last_col" => Deco::LastCol,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Deco::Header => "header",
            Deco::BandRow => "band_row",
            Deco::BandCol => "band_col",
            Deco::FirstCol => "first_col",
            Deco::LastCol => "last_col",
        }
    }
}

/// 縞々と見出しの帯の色。**セルの塗りとして書く** — 表の様式に頼らないので、
/// Excel で開いても、表オブジェクトを外しても、見た目が残る
const HEADER_FILL: &str = "D5E8DC";
const BAND_FILL: &str = "F1F6F3";

/// 飾りを掛ける(または外す)。返りは書き替えた欄の数。
///
/// **`on = false` では塗りを剥がさない。** 掛ける前の姿を覚えていないので、
/// 剥がすと「元は水色だった」欄まで白にしてしまう。外すのは表の性質
/// (`TableDef` の旗)だけ — 保存したときに Excel が縞を描かなくなる。
/// 色を消したいときは「書式のクリア」で消す。**できないことを、できるように
/// 見せない。**
pub fn deco(s: &mut Sheet, a: Pos, b: Pos, what: Deco, on: bool) -> usize {
    // 表の中なら、表オブジェクトの旗も合わせる(xlsx へ往復する)
    if let Some(i) = s.tables.iter().position(|t| t.contains(a)) {
        let t = &mut s.tables[i];
        match what {
            Deco::Header => t.header = on,
            Deco::BandRow => t.banded_rows = on,
            Deco::BandCol => t.banded_cols = on,
            Deco::FirstCol => t.first_col = on,
            Deco::LastCol => t.last_col = on,
        }
    }
    if !on {
        return 0;
    }
    let mut n = 0usize;
    for r in a.row..=b.row {
        for c in a.col..=b.col {
            let p = Pos::new(r, c);
            let mut cell = s.get(p).cloned().unwrap_or_default();
            let touched = match what {
                Deco::Header if r == a.row => {
                    cell.fmt.bold = true;
                    cell.fmt.fill = Some(HEADER_FILL.into());
                    cell.fmt.borders.top = Edge::THIN;
                    true
                }
                Deco::BandRow if r > a.row && (r - a.row).is_multiple_of(2) => {
                    cell.fmt.fill = Some(BAND_FILL.into());
                    true
                }
                Deco::BandCol if (c - a.col) % 2 == 1 => {
                    cell.fmt.fill = Some(BAND_FILL.into());
                    true
                }
                Deco::FirstCol if c == a.col => {
                    cell.fmt.bold = true;
                    true
                }
                Deco::LastCol if c == b.col => {
                    cell.fmt.bold = true;
                    true
                }
                _ => false,
            };
            if touched {
                s.set(p, cell);
                n += 1;
            }
        }
    }
    n
}

/// すぐ下の行に中身があるか。**黙って上書きしない**ための確認
pub fn below_used(s: &Sheet, a: Pos, b: Pos) -> bool {
    (a.col..=b.col).any(|c| {
        s.get(Pos::new(b.row + 1, c))
            .map(|cell| !cell.value.display().is_empty() || cell.formula.is_some())
            .unwrap_or(false)
    })
}

/// 表のデザインの「合計行」。選択の下の行に、数の列へ `=SUM(…)` を入れて
/// 太字+上罫線にする。1行目が見出し(文字)なら合計の範囲から外す。
/// 文字の列の先頭には「合計」の札。書いた欄の数を返す。
pub fn add_total_row(s: &mut Sheet, a: Pos, b: Pos) -> usize {
    let header =
        (a.col..=b.col).any(|c| matches!(s.get(Pos::new(a.row, c)).map(|x| &x.value), Some(Value::Text(_))));
    let from = if header && b.row > a.row { a.row + 1 } else { a.row };
    let total = b.row + 1;
    let mut n = 0usize;
    for c in a.col..=b.col {
        let numeric = (from..=b.row)
            .any(|r| matches!(s.get(Pos::new(r, c)).map(|x| &x.value), Some(Value::Number(_))));
        let p = Pos::new(total, c);
        let fmt0 = s.get(p).map(|x| x.fmt.clone()).unwrap_or_default();
        let mut cell = if numeric {
            Cell::input(&format!("=SUM({}:{})", Pos::new(from, c).a1(), Pos::new(b.row, c).a1()))
        } else if c == a.col {
            Cell::input("合計")
        } else {
            s.get(p).cloned().unwrap_or_default()
        };
        cell.fmt = fmt0;
        cell.fmt.bold = true;
        cell.fmt.borders.top = Edge::THIN;
        s.set(p, cell);
        n += 1;
    }
    // 表の中なら「合計行がある」を立てる(xlsx へ往復する)
    if let Some(i) = s.tables.iter().position(|t| t.contains(a)) {
        s.tables[i].totals = true;
        if s.tables[i].b.row < total {
            s.tables[i].b.row = total;
        }
    }
    n
}

/// 表オブジェクトを外して普通の範囲に戻す。**書式と式は残る。**
/// 返りは外した表の名前(その場所に表が無ければ `None`)
pub fn to_range(s: &mut Sheet, at: Pos) -> Option<String> {
    let i = s.tables.iter().position(|t| t.contains(at))?;
    Some(s.tables.remove(i).name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_table() -> Sheet {
        let mut s = Sheet::default();
        for (r, row) in [["品", "数"], ["鉛筆", "3"], ["消しゴム", "5"]].iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                s.set(Pos::new(r as u32, c as u32), Cell::input(v));
            }
        }
        s
    }

    #[test]
    fn 見出しの帯は1行目だけに掛かる() {
        let mut s = one_table();
        let n = deco(&mut s, Pos::new(0, 0), Pos::new(2, 1), Deco::Header, true);
        assert_eq!(n, 2, "1行目の2欄だけ");
        assert!(s.get(Pos::new(0, 0)).unwrap().fmt.bold);
        assert!(!s.get(Pos::new(1, 0)).unwrap().fmt.bold, "2行目は触らない");
    }

    #[test]
    fn 外すときは塗りを剥がさない() {
        // 掛ける前の姿を覚えていないので、剥がすと元の色まで消える。
        // 外すのは表の旗だけ、が約束(できないことをできるように見せない)
        let mut s = one_table();
        deco(&mut s, Pos::new(0, 0), Pos::new(2, 1), Deco::Header, true);
        let n = deco(&mut s, Pos::new(0, 0), Pos::new(2, 1), Deco::Header, false);
        assert_eq!(n, 0);
        assert!(s.get(Pos::new(0, 0)).unwrap().fmt.bold, "塗りは残る");
    }

    #[test]
    fn 合計行は数の列にだけ_sum_を入れる() {
        let mut s = one_table();
        add_total_row(&mut s, Pos::new(0, 0), Pos::new(2, 1));
        assert_eq!(s.get(Pos::new(3, 0)).unwrap().value.display(), "合計");
        assert_eq!(s.get(Pos::new(3, 1)).unwrap().formula.as_deref(), Some("SUM(B2:B3)"));
    }

    #[test]
    fn 名前と綴りが往復する() {
        for d in [Deco::Header, Deco::BandRow, Deco::BandCol, Deco::FirstCol, Deco::LastCol] {
            assert_eq!(Deco::from_name(d.name()), Some(d));
        }
        assert_eq!(Deco::from_name("なにか"), None);
    }
}
