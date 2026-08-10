//! **シートの操作。** 行と列の出し入れ、並べ替え。**式の参照も連れて動く。**


use super::refs::*;
use super::types::*;

impl Sheet {
    /// 行を1つ挿し込む。**下にあるものを1つずつ下げる。**
    ///
    /// **残ったセルの式の参照も直す。** 直さないと、行を挿しただけで
    /// 式が別のセルを指し、間違った答えを黙って出す。
    pub fn insert_row(&mut self, at: u32) {
        self.shift(|p| p.row >= at, 1, 0);
        self.fix_formulas(at, 1, true);
        self.shift_merges(at, 1, true);
        self.row_height = self
            .row_height
            .iter()
            .map(|(r, h)| (if *r >= at { r + 1 } else { *r }, *h))
            .collect();
        // グループ化の深さと畳みも一緒に動かす(置き去りにすると
        // 別の行が畳まれて見える)
        self.row_outline = self
            .row_outline
            .iter()
            .map(|(r, l)| (if *r >= at { r + 1 } else { *r }, *l))
            .collect();
        self.row_hidden = self
            .row_hidden
            .iter()
            .map(|r| if *r >= at { r + 1 } else { *r })
            .collect();
    }

    /// 行を1つ抜く。
    pub fn remove_row(&mut self, at: u32) {
        self.cells.retain(|p, _| p.row != at);
        self.shift(|p| p.row > at, -1, 0);
        self.fix_formulas(at, -1, true);
        self.shift_merges(at, -1, true);
        self.row_height = self
            .row_height
            .iter()
            .filter(|(r, _)| **r != at)
            .map(|(r, h)| (if *r > at { r - 1 } else { *r }, *h))
            .collect();
        self.row_outline = self
            .row_outline
            .iter()
            .filter(|(r, _)| **r != at)
            .map(|(r, l)| (if *r > at { r - 1 } else { *r }, *l))
            .collect();
        self.row_hidden = self
            .row_hidden
            .iter()
            .filter(|r| **r != at)
            .map(|r| if *r > at { r - 1 } else { *r })
            .collect();
    }

    pub fn insert_col(&mut self, at: u32) {
        self.shift(|p| p.col >= at, 0, 1);
        self.fix_formulas(at, 1, false);
        self.shift_merges(at, 1, false);
        // 列幅も一緒に動かす
        self.col_width = self
            .col_width
            .iter()
            .map(|(c, w)| (if *c >= at { c + 1 } else { *c }, *w))
            .collect();
        self.col_outline = self
            .col_outline
            .iter()
            .map(|(c, l)| (if *c >= at { c + 1 } else { *c }, *l))
            .collect();
        self.col_hidden = self
            .col_hidden
            .iter()
            .map(|c| if *c >= at { c + 1 } else { *c })
            .collect();
    }

    pub fn remove_col(&mut self, at: u32) {
        self.cells.retain(|p, _| p.col != at);
        self.shift(|p| p.col > at, 0, -1);
        self.fix_formulas(at, -1, false);
        self.shift_merges(at, -1, false);
        self.col_width = self
            .col_width
            .iter()
            .filter(|(c, _)| **c != at)
            .map(|(c, w)| (if *c > at { c - 1 } else { *c }, *w))
            .collect();
        self.col_outline = self
            .col_outline
            .iter()
            .filter(|(c, _)| **c != at)
            .map(|(c, l)| (if *c > at { c - 1 } else { *c }, *l))
            .collect();
        self.col_hidden = self
            .col_hidden
            .iter()
            .filter(|c| **c != at)
            .map(|c| if *c > at { c - 1 } else { *c })
            .collect();
    }

    /// 出し入れに合わせて、**残ったセルの式の参照も直す**。
    /// これをやらないと、行を挿しただけで式が別のセルを指す。
    fn fix_formulas(&mut self, at: u32, delta: i64, is_row: bool) {
        for c in self.cells.values_mut() {
            if let Some(f) = &c.formula {
                c.formula = Some(shift_refs(f, at, delta, is_row));
            }
        }
    }

    /// 行・列の出し入れに合わせて結合の範囲も動かす。
    ///
    /// 削除では**上端と下端で動きが違う**: 上端が消えた行なら次の行が
    /// 滑り込む(据え置き)、下端が消えた行なら1つ縮む。
    fn shift_merges(&mut self, at: u32, delta: i64, is_row: bool) {
        let top = |v: u32| -> u32 {
            if delta > 0 {
                if v >= at { v + 1 } else { v }
            } else if v > at {
                v - 1
            } else {
                v
            }
        };
        let bottom = |v: u32| -> u32 {
            if delta > 0 {
                if v >= at { v + 1 } else { v }
            } else if v >= at {
                v.saturating_sub(1)
            } else {
                v
            }
        };
        for (a, b) in self.merges.iter_mut() {
            if is_row {
                a.row = top(a.row);
                b.row = bottom(b.row);
            } else {
                a.col = top(a.col);
                b.col = bottom(b.col);
            }
        }
        // 1セルに潰れた・裏返った結合は結合ではない
        self.merges.retain(|(a, b)| a <= b && (a.row != b.row || a.col != b.col));
    }

    /// この位置に効く入力規則(最初に見つかったもの)。
    pub fn validation_at(&self, p: Pos) -> Option<&Validation> {
        self.validations.iter().find(|v| v.contains(p))
    }

    /// この位置は結合に呑まれているか(左上を除く)。
    pub fn covered_by_merge(&self, p: Pos) -> bool {
        self.merges.iter().any(|(a, b)| {
            p != *a && (a.row..=b.row).contains(&p.row) && (a.col..=b.col).contains(&p.col)
        })
    }

    fn shift(&mut self, pick: impl Fn(&Pos) -> bool, dr: i64, dc: i64) {
        let moved: Vec<(Pos, Cell)> = self
            .cells
            .iter()
            .filter(|(p, _)| pick(p))
            .map(|(p, c)| (*p, c.clone()))
            .collect();
        for (p, _) in &moved {
            self.cells.remove(p);
        }
        for (p, c) in moved {
            let row = (p.row as i64 + dr).max(0) as u32;
            let col = (p.col as i64 + dc).max(0) as u32;
            self.cells.insert(Pos { row, col }, c);
        }
    }
}

impl Sheet {
    /// 指定した列で並べ替える。
    ///
    /// **見出し行は動かさない**(`header` が true のとき先頭行を据え置く)。
    /// 帳票の並べ替えで見出しが混ざるのは事故なので、既定で守る。
    ///
    /// **行はまるごと動かす。** 選んだ列だけ並べ替えると、
    /// 隣の列との対応が壊れて、静かに嘘の表ができる。
    pub fn sort_by_column(&mut self, col: u32, ascending: bool, header: bool) {
        self.sort_by_columns(&[(col, ascending)], header);
    }

    /// 複数の基準で並べ替える(基準は左から順に強い。sort_by は安定)。
    /// (列, 昇順か)の並び。見出し(header)は据え置く
    pub fn sort_by_columns(&mut self, keys: &[(u32, bool)], header: bool) {
        let (rows, cols) = self.extent();
        if rows == 0 || keys.is_empty() { return }
        let (last_row, last_col) = (rows - 1, cols.saturating_sub(1));
        let first = if header { 1 } else { 0 };
        if last_row < first {
            return;
        }
        // 行をまるごと取り出す
        let mut rows: Vec<(u32, Vec<(u32, Cell)>)> = Vec::new();
        for r in first..=last_row {
            let cells: Vec<(u32, Cell)> = (0..=last_col)
                .filter_map(|c| self.cells.get(&Pos { row: r, col: c }).map(|x| (c, x.clone())))
                .collect();
            rows.push((r, cells));
        }
        rows.sort_by(|a, b| {
            let key = |v: &Vec<(u32, Cell)>, col: u32| {
                v.iter().find(|(c, _)| *c == col).map(|(_, x)| x.value.clone())
            };
            for (col, asc) in keys {
                let o = cmp_value(&key(&a.1, *col), &key(&b.1, *col));
                let o = if *asc { o } else { o.reverse() };
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            std::cmp::Ordering::Equal
        });
        // 置き直す
        for r in first..=last_row {
            for c in 0..=last_col {
                self.cells.remove(&Pos { row: r, col: c });
            }
        }
        for (i, (_, cells)) in rows.into_iter().enumerate() {
            let r = first + i as u32;
            for (c, cell) in cells {
                self.cells.insert(Pos { row: r, col: c }, cell);
            }
        }
    }

    /// 指定の列の**色**で並べ替える — 目当ての色の行を上に集める。
    /// 本家の「選択したセルの色を上に/フォントの色を上に」。順序は安定
    /// (色が合う行どうし・合わない行どうしの元の並びは変えない)。
    pub fn sort_color_top(&mut self, col: u32, use_fill: bool, target: &str, header: bool) {
        let (rows, cols) = self.extent();
        if rows == 0 { return }
        let (last_row, last_col) = (rows - 1, cols.saturating_sub(1));
        let first = if header { 1 } else { 0 };
        if last_row < first { return }
        let mut rows: Vec<Vec<(u32, Cell)>> = Vec::new();
        for r in first..=last_row {
            rows.push(
                (0..=last_col)
                    .filter_map(|c| self.cells.get(&Pos { row: r, col: c }).map(|x| (c, x.clone())))
                    .collect(),
            );
        }
        rows.sort_by_key(|cells| {
            let hit = cells.iter().find(|(c, _)| *c == col).map(|(_, x)| {
                let got = if use_fill { x.fmt.fill.as_deref() } else { x.fmt.color.as_deref() };
                got.is_some_and(|v| v.eq_ignore_ascii_case(target))
            });
            if hit.unwrap_or(false) { 0u8 } else { 1 }
        });
        for r in first..=last_row {
            for c in 0..=last_col {
                self.cells.remove(&Pos { row: r, col: c });
            }
        }
        for (i, cells) in rows.into_iter().enumerate() {
            let r = first + i as u32;
            for (c, cell) in cells {
                self.cells.insert(Pos { row: r, col: c }, cell);
            }
        }
    }

    /// 選んだ範囲**だけ**を並べ替える(範囲の外の列は動かさない)。
    /// 本家の「現在選択されているセルのみの並べ替え」— 隣のデータと
    /// 行がずれるのは承知の上で使う形。見出しは仮定しない
    pub fn sort_range(&mut self, a: Pos, b: Pos, key_col: u32, ascending: bool) {
        if a.row >= b.row {
            return; // 1行なら並べ替えるものが無い
        }
        let key_col = key_col.clamp(a.col, b.col);
        // 範囲の行を(範囲の列だけ)取り出す
        let mut rows: Vec<Vec<(u32, Cell)>> = (a.row..=b.row)
            .map(|r| {
                (a.col..=b.col)
                    .filter_map(|c| {
                        self.cells.get(&Pos { row: r, col: c }).map(|x| (c, x.clone()))
                    })
                    .collect()
            })
            .collect();
        rows.sort_by(|x, y| {
            let key = |v: &Vec<(u32, Cell)>| {
                v.iter().find(|(c, _)| *c == key_col).map(|(_, x)| x.value.clone())
            };
            let o = cmp_value(&key(x), &key(y));
            if ascending { o } else { o.reverse() }
        });
        for r in a.row..=b.row {
            for c in a.col..=b.col {
                self.cells.remove(&Pos { row: r, col: c });
            }
        }
        for (i, cells) in rows.into_iter().enumerate() {
            let r = a.row + i as u32;
            for (c, cell) in cells {
                self.cells.insert(Pos { row: r, col: c }, cell);
            }
        }
    }

    /// 中身が同じ行を落とす。**先に出てきた方を残す。**
    ///
    /// 返すのは落とした行数 — 何件消したかを黙らない。
    pub fn remove_duplicate_rows(&mut self, header: bool) -> usize {
        self.remove_duplicate_rows_in(header, &[])
    }

    /// 中身が同じ行を落とす(比べる列を選べる版。空 = 全列で比べる)。
    /// 行は丸ごと消える — 比べるのが一部の列でも、残すのは先に出てきた行。
    pub fn remove_duplicate_rows_in(&mut self, header: bool, key_cols: &[u32]) -> usize {
        let (rows, cols) = self.extent();
        if rows == 0 { return 0 }
        let (last_row, last_col) = (rows - 1, cols.saturating_sub(1));
        let first = if header { 1 } else { 0 };
        let mut seen: Vec<Vec<String>> = Vec::new();
        let mut keep: Vec<Vec<(u32, Cell)>> = Vec::new();
        let mut dropped = 0usize;
        for r in first..=last_row {
            let cells: Vec<(u32, Cell)> = (0..=last_col)
                .filter_map(|c| self.cells.get(&Pos { row: r, col: c }).map(|x| (c, x.clone())))
                .collect();
            let key: Vec<String> = (0..=last_col)
                .filter(|c| key_cols.is_empty() || key_cols.contains(c))
                .map(|c| {
                    cells.iter().find(|(cc, _)| *cc == c)
                        .map(|(_, x)| x.value.display()).unwrap_or_default()
                })
                .collect();
            // 空の行は重複と見なさない(表の中の空行は区切りとして使われる)
            if key.iter().all(|s| s.is_empty()) {
                keep.push(cells);
                continue;
            }
            if seen.contains(&key) {
                dropped += 1;
                continue;
            }
            seen.push(key);
            keep.push(cells);
        }
        for r in first..=last_row {
            for c in 0..=last_col {
                self.cells.remove(&Pos { row: r, col: c });
            }
        }
        for (i, cells) in keep.into_iter().enumerate() {
            let r = first + i as u32;
            for (c, cell) in cells {
                self.cells.insert(Pos { row: r, col: c }, cell);
            }
        }
        dropped
    }
}

/// 並べ替えの比較。**数は数として、文字は文字として。空は最後。**
pub(super) fn cmp_value(a: &Option<Value>, b: &Option<Value>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let rank = |v: &Option<Value>| match v {
        None => 3,
        Some(Value::Empty) => 3,
        Some(Value::Number(_)) => 0,
        Some(Value::Bool(_)) => 1,
        Some(Value::Text(_)) => 2,
        Some(Value::Error(_)) => 4,
    };
    let (ra, rb) = (rank(a), rank(b));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a, b) {
        (Some(Value::Number(x)), Some(Value::Number(y))) => {
            x.partial_cmp(y).unwrap_or(Ordering::Equal)
        }
        (Some(Value::Text(x)), Some(Value::Text(y))) => x.cmp(y),
        (Some(Value::Bool(x)), Some(Value::Bool(y))) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

impl Sheet {
    /// 全部の式の参照を写像で引き直す。
    fn remap_formulas(&mut self, f: impl Fn(Pos) -> MapRef) {
        for c in self.cells.values_mut() {
            if let Some(fla) = &c.formula {
                c.formula = Some(map_refs(fla, &f));
            }
        }
    }

    /// 結合が「動く帯」の境界をまたいでいないか。またぐなら断る(Excel と同じ)。
    fn merges_cross(&self, in_band: impl Fn(Pos) -> bool) -> bool {
        self.merges.iter().any(|(a, b)| {
            let corners = [
                Pos::new(a.row, a.col),
                Pos::new(a.row, b.col),
                Pos::new(b.row, a.col),
                Pos::new(b.row, b.col),
            ];
            let inside = corners.iter().filter(|p| in_band(**p)).count();
            inside != 0 && inside != corners.len()
        })
    }

    /// 部分的な挿入。選んだ範囲の大きさぶん、帯のセルを右(または下)へずらす。
    /// **動いたセルを指す参照も一緒に動く。** 結合が帯をまたぐときは断る。
    pub fn insert_cells(&mut self, a: Pos, b: Pos, right: bool) -> Result<usize, String> {
        let n = if right { b.col - a.col + 1 } else { b.row - a.row + 1 };
        let in_band = |p: Pos| {
            if right {
                (a.row..=b.row).contains(&p.row) && p.col >= a.col
            } else {
                (a.col..=b.col).contains(&p.col) && p.row >= a.row
            }
        };
        if self.merges_cross(in_band) {
            return Err("結合されたセルが範囲をまたいでいるため、シフトできません".into());
        }
        let shift = |p: Pos| {
            if right { Pos::new(p.row, p.col + n) } else { Pos::new(p.row + n, p.col) }
        };
        // 式の参照を先に引き直す(セルを動かす前の位置で判定する)
        self.remap_formulas(|p| if in_band(p) { MapRef::To(shift(p)) } else { MapRef::Keep });
        // セルを動かす
        let moved: Vec<(Pos, Cell)> = self
            .cells
            .iter()
            .filter(|(p, _)| in_band(**p))
            .map(|(p, c)| (*p, c.clone()))
            .collect();
        let count = moved.len();
        for (p, _) in &moved {
            self.cells.remove(p);
        }
        for (p, c) in moved {
            self.cells.insert(shift(p), c);
        }
        // 帯の中の結合も一緒に
        for (m1, m2) in self.merges.iter_mut() {
            if in_band(*m1) {
                *m1 = shift(*m1);
                *m2 = shift(*m2);
            }
        }
        Ok(count)
    }

    /// 部分的な削除。選んだ範囲を消し、帯の先のセルを左(または上)へ詰める。
    /// **消えた範囲を指していた参照は #REF! になる。**
    pub fn delete_cells(&mut self, a: Pos, b: Pos, left: bool) -> Result<usize, String> {
        let n = if left { b.col - a.col + 1 } else { b.row - a.row + 1 };
        let in_range =
            |p: Pos| (a.row..=b.row).contains(&p.row) && (a.col..=b.col).contains(&p.col);
        let beyond = |p: Pos| {
            if left {
                (a.row..=b.row).contains(&p.row) && p.col > b.col
            } else {
                (a.col..=b.col).contains(&p.col) && p.row > b.row
            }
        };
        let in_band = |p: Pos| in_range(p) || beyond(p);
        if self.merges_cross(in_band) {
            return Err("結合されたセルが範囲をまたいでいるため、シフトできません".into());
        }
        let shift_back = |p: Pos| {
            if left { Pos::new(p.row, p.col - n) } else { Pos::new(p.row - n, p.col) }
        };
        self.remap_formulas(|p| {
            if in_range(p) {
                MapRef::Broken
            } else if beyond(p) {
                MapRef::To(shift_back(p))
            } else {
                MapRef::Keep
            }
        });
        let removed = self.cells.iter().filter(|(p, _)| in_range(**p)).count();
        self.cells.retain(|p, _| !in_range(*p));
        let moved: Vec<(Pos, Cell)> = self
            .cells
            .iter()
            .filter(|(p, _)| beyond(**p))
            .map(|(p, c)| (*p, c.clone()))
            .collect();
        for (p, _) in &moved {
            self.cells.remove(p);
        }
        for (p, c) in moved {
            self.cells.insert(shift_back(p), c);
        }
        self.merges.retain(|(m1, _)| !in_range(*m1));
        for (m1, m2) in self.merges.iter_mut() {
            if beyond(*m1) {
                *m1 = shift_back(*m1);
                *m2 = shift_back(*m2);
            }
        }
        Ok(removed)
    }
}
