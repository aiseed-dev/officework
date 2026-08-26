//! **座標と入力。** 当たり判定・カーソル移動・キーの動作・書式。
//!
//! 画面の px と行列の番号を行き来する所は全部ここ。

use crate::*;

impl Calc {
    /// 見出しの幅・高さ(表示タブで消せる。当たり判定も同じ値を使う)
    pub(crate) fn head_w(&self) -> f32 {
        if self.show_headers { HEAD_W } else { 0.0 }
    }
    pub(crate) fn head_h(&self) -> f32 {
        if self.show_headers { ROW_H } else { 0.0 }
    }

    /// 列の画面幅。文書の指定(xlsx の width)に従う
    pub(crate) fn col_px(&self, c: u32) -> f32 {
        self.sheet()
            .col_width
            .get(&c)
            .copied()
            .or(self.sheet().default_col_width)
            .map(|w| w * PX_PER_CHW)
            .unwrap_or(COL_W)
            * self.zoom
    }

    /// 列の左端(見出しの右から)
    pub(crate) fn col_x(&self, c: u32) -> f32 {
        (0..c).map(|i| self.col_px(i)).sum()
    }

    pub(crate) fn sel_rect(&self) -> (Pos, Pos) {
        let a = self.anchor.unwrap_or(self.cursor);
        let c = self.cursor;
        (Pos::new(a.row.min(c.row), a.col.min(c.col)),
         Pos::new(a.row.max(c.row), a.col.max(c.col)))
    }

    /// Shift+矢印。起点を置いてから動く
    pub(crate) fn extend(&mut self, dr: i32, dc: i32) {
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        if !self.commit() {
            return;
        }
        let r = (self.cursor.row as i32 + dr).max(0) as u32;
        let c = (self.cursor.col as i32 + dc).max(0) as u32;
        self.cursor = Pos::new(r.min(LAST_ROW), c.min(LAST_COL));
        self.follow();
        let (a, b) = self.sel_rect();
        self.status = format!("{}:{}", a.a1(), b.a1()).into();
        self.sync_input();
    }

    /// 見張りから1つ外す(札の × が呼ぶ)。**全部消すのはリボンの「見張り」**
    pub(crate) fn watch_remove(&mut self, si: usize, p: Pos) {
        let before = self.watch.len();
        self.watch.retain(|(s, q)| !(*s == si && *q == p));
        self.status = if self.watch.len() < before {
            ui::tf!("stopped_watching", p.a1()).into()
        } else {
            ui::t!("watch_already_gone").into()
        };
    }

    /// 見張りの札を押したときに、そのセルへ飛ぶ
    pub(crate) fn watch_goto(&mut self, si: usize, p: Pos) {
        self.commit();
        self.active = si.min(self.book.sheets.len().saturating_sub(1));
        self.cursor = p;
        self.anchor = None;
        self.follow();
        self.sync_input();
        self.status = ui::tf!("moved", p.a1()).into();
    }

    /// カーソルが見える位置まで窓を動かす。
    ///
    /// 固定や分割の帯があるときは、その分だけ動ける場所が狭くなります。
    /// 帯の中にカーソルが見えているなら、何も動かしません
    pub(crate) fn follow(&mut self) {
        let top = self.top_band();
        let left = self.left_band();
        let in_band =
            |band: Option<(u32, u32)>, v: u32| band.is_some_and(|(f, s)| f > 0 && (s..s + f).contains(&v));
        let nr = self.rows_snug().saturating_sub(top.map_or(0, |(f, _)| f)).max(1);
        let nc = self.cols_snug().saturating_sub(left.map_or(0, |(f, _)| f)).max(1);
        if !in_band(top, self.cursor.row) {
            if self.cursor.row < self.view.row {
                self.view.row = self.cursor.row;
            }
            if self.cursor.row >= self.view.row + nr {
                self.view.row = self.cursor.row + 1 - nr;
            }
        }
        if !in_band(left, self.cursor.col) {
            if self.cursor.col < self.view.col {
                self.view.col = self.cursor.col;
            }
            if self.cursor.col >= self.view.col + nc {
                self.view.col = self.cursor.col + 1 - nc;
            }
        }
    }

    /// p を呑んでいる結合(あれば (左上, 右下))。
    pub(crate) fn merge_of(&self, p: Pos) -> Option<(Pos, Pos)> {
        self.sheet()
            .merges
            .iter()
            .copied()
            .find(|(a, b)| {
                (a.row..=b.row).contains(&p.row) && (a.col..=b.col).contains(&p.col)
            })
    }

    pub(crate) fn move_cursor(&mut self, dr: i32, dc: i32) {
        // 普通の移動は選択を解く
        self.anchor = None;
        if !self.commit() {
            return; // 入力規則で戻された(status に候補が出ている)
        }
        let from = self.cursor;
        let r = (self.cursor.row as i32 + dr).max(0) as u32;
        let c = (self.cursor.col as i32 + dc).max(0) as u32;
        let mut np = Pos::new(r.min(LAST_ROW), c.min(LAST_COL));
        // 結合は1つのセルとして歩く(Excel と同じ):
        // 外から入ったら左上に立ち、左上から同じ向きへ動いたら反対側の外へ抜ける
        if let Some((a, b)) = self.merge_of(np) {
            let inside_from = self.merge_of(from) == Some((a, b));
            np = if inside_from {
                match (dr.signum(), dc.signum()) {
                    (1, _) => Pos::new((b.row + 1).min(LAST_ROW), np.col),
                    (-1, _) => {
                        if a.row == 0 { a } else { Pos::new(a.row - 1, np.col) }
                    }
                    (_, 1) => Pos::new(np.row, (b.col + 1).min(LAST_COL)),
                    (_, -1) => {
                        if a.col == 0 { a } else { Pos::new(np.row, a.col - 1) }
                    }
                    _ => a,
                }
            } else {
                a
            };
            // 抜けた先も別の結合なら、その左上へ
            if let Some((a2, _)) = self.merge_of(np) {
                np = a2;
            }
        }
        self.cursor = np;
        self.follow();
        self.sync_input();
    }

    // ---- 割り当てられた操作 ----
    pub(crate) fn a_backspace(&mut self, _: &ui::Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.fn_args.is_some() {
            self.editor().backspace();
            self.fn_args_recalc();
        } else if let Some(d) = &mut self.fn_dlg {
            d.search.backspace();
            d.sel = 0;
        } else if self.pick_filtering() {
            // コンボの検索欄の1文字削除(選択は先頭へ)
            self.editor().backspace();
            self.pick_filter_edited();
        } else if self.chat_focus
            || self.name_edit.is_some()
            || self.solver.is_some()
            || self.filter_panel.is_some()
            || self.dv_dlg.is_some()
            || self.prompt.is_some()
        {
            // パネル・小窓の欄へ(editor() が今の宛先を知っている)
            self.editor().backspace();
        } else if self.editing() || self.edit_armed {
            self.input.backspace();
            self.dirty = true;
        } else {
            // セルの上での BackSpace = 中身を消す(Excel と同じ。書式は残る)
            self.clear_selection_now();
        }
        cx.notify();
    }

    /// セルの上での BackSpace / Delete の実体。選択(無ければいまのセル)の
    /// 中身を消す。書式は残す。保護中は断る
    pub(crate) fn clear_selection_now(&mut self) {
        if self.sheet().protected {
            self.status =
                ui::t!("sheet_protected_protection_tab").into();
            return;
        }
        self.checkpoint();
        let n = self.clear_range();
        self.sync_input();
        self.status = ui::tf!("contents_cells_cleared_formatting", n).into();
    }
    /// 選んだ範囲の中身を消す(**書式は残す** — 帳票の枠を壊さない)。
    /// 控えを取ってから呼ぶこと。返すのは消したセルの数。
    pub(crate) fn clear_range(&mut self) -> usize {
        let (a, b) = self.sel_rect();
        let mut n = 0usize;
        for r in a.row..=b.row {
            for c in a.col..=b.col {
                let p = Pos::new(r, c);
                if let Some(cell) = self.sheet().get(p).cloned() {
                    self.book.sheets[self.active].set(p, Cell {
                        formula: None,
                        value: Value::Empty,
                        fmt: cell.fmt,
                    });
                    n += 1;
                }
            }
        }
        recalc_book(&mut self.book, self.active);
        self.dirty = true;
        self.sync_input();
        n
    }

    pub(crate) fn a_delete(&mut self, _: &ui::Delete, _: &mut Window, cx: &mut Context<Self>) {
        // パネル・小窓の欄が開いていれば、その欄の1文字削除(セルに流さない)
        if self.chat_focus
            || self.name_edit.is_some()
            || self.fn_dlg.is_some()
            || self.pick_filtering()
            || self.solver.is_some()
            || self.filter_panel.is_some()
            || self.dv_dlg.is_some()
            || self.prompt.is_some()
        {
            self.editor().delete();
            if self.pick_filtering() {
                self.pick_filter_edited();
            }
            cx.notify();
            return;
        }
        if self.fn_args.is_some() {
            self.editor().delete();
            self.fn_args_recalc();
            cx.notify();
            return;
        }
        if self.cell_locked(self.cursor) || self.sel_locked() {
            self.status = Self::protected_msg().into();
            cx.notify();
            return;
        }
        // **配列数式は範囲ごと消す**(Excel と同じ)。一部だけ消すと、
        // 残りが古い値のまま取り残されて帳票が静かに嘘をつく
        {
            let (a, b) = self.sel_rect();
            let hit: Vec<Pos> = self
                .sheet()
                .cse
                .iter()
                .filter(|(o, (h, w))| {
                    // 選んだ範囲と配列の範囲が重なっているか
                    !(o.row + h - 1 < a.row || o.row > b.row
                        || o.col + w - 1 < a.col || o.col > b.col)
                })
                .map(|(o, _)| *o)
                .collect();
            if !hit.is_empty() {
                let covered = hit.iter().all(|o| {
                    let (h, w) = self.sheet().cse[o];
                    o.row >= a.row && o.col >= a.col
                        && o.row + h - 1 <= b.row && o.col + w - 1 <= b.col
                });
                if !covered {
                    self.status = ui::t!(
                        "cant_delete_part_array"
                    )
                    .into();
                    cx.notify();
                    return;
                }
                self.checkpoint();
                for o in hit {
                    let (h, w) = self.sheet_mut().cse.remove(&o).unwrap_or((1, 1));
                    for r in o.row..o.row + h {
                        for c in o.col..o.col + w {
                            let p = Pos::new(r, c);
                            if let Some(cell) = self.sheet_mut().cells.get_mut(&p) {
                                cell.formula = None;
                                cell.value = sheet::Value::Empty;
                            }
                        }
                    }
                }
                self.dirty = true;
                recalc_book(&mut self.book, self.active);
                self.sync_input();
                self.status = ui::t!("deleted_array_formula_ctrl").into();
                cx.notify();
                return;
            }
        }
        if let Some(i) = self.shape_sel.take() {
            // 束ねた選択(Ctrl+クリック)があればまとめて消す。
            // 後ろから消す=残りの番号がずれない
            let mut idx: Vec<usize> = std::mem::take(&mut self.shape_multi);
            idx.push(i);
            idx.sort_unstable();
            idx.dedup();
            idx.retain(|&k| k < self.sheet().shapes_new.len());
            if !idx.is_empty() {
                self.checkpoint();
                for k in idx.iter().rev() {
                    self.sheet_mut().shapes_new.remove(*k);
                }
                self.dirty = true;
                self.status = if idx.len() == 1 {
                    ui::t!("shape_deleted_ctrl_z").into()
                } else {
                    ui::tf!("deleted_shapes_ctrl_z", idx.len()).into()
                };
            }
            cx.notify();
            return;
        }
        if self.delete_selected_image() {
            cx.notify();
            return;
        }
        if self.editing() || self.edit_armed {
            // 編集中の Delete は1文字(いつもの文字カーソルの右)
            self.input.delete();
            self.dirty = true;
        } else {
            // セルの上での Delete = 中身を消す(選択があれば選択ぶん。Excel と同じ)
            self.clear_selection_now();
        }
        cx.notify();
    }

    /// コピー。選んだ範囲(無ければいまのセル)を TSV で系のクリップボードへ。
    /// 他のアプリにはそのまま貼れる形で、アプリ内には起点を控えて式をずらせる形で。
    pub(crate) fn a_copy(&mut self, _: &ui::Copy, _: &mut Window, cx: &mut Context<Self>) {
        self.copy_now(cx)
    }
    pub(crate) fn copy_now(&mut self, cx: &mut Context<Self>) {
        // .py の編集面が開いている間は、表のセルには一切触らない
        if let Some(p) = &self.py_edit {
            let sel = p.ed.selection();
            if sel.start != sel.end {
                let t = p.ed.text()[sel].to_string();
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(t));
                self.status = ui::t!("copied_code").into();
            }
            cx.notify();
            return;
        }
        if self.input.has_selection() {
            // 数式バーの文字を選んでいるなら、その文字のコピー
            let sel = self.input.selection();
            if let Some(s) = self.input.text().get(sel) {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(s.to_string()));
                self.status = ui::t!("copied_2").into();
            }
            cx.notify();
            return;
        }
        let (a, b) = self.sel_rect();
        let tsv = range_tsv(self.sheet(), a, b);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(tsv.clone()));
        self.clip = Some((a, tsv));
        // セルそのものも控える(形式を選択して貼り付けの材料)
        self.clip_cells = Some(
            (a.row..=b.row)
                .map(|r| {
                    (a.col..=b.col)
                        .map(|c| self.sheet().get(Pos::new(r, c)).cloned())
                        .collect()
                })
                .collect(),
        );
        self.clip_range = Some((self.active, a, b));
        self.status = ui::tf!("copied", a.a1(), b.a1()).into();
        cx.notify();
    }

    /// 切り取り = コピー + 中身を消す(書式は残る。1手で戻せる)。
    pub(crate) fn a_cut(&mut self, _: &ui::Cut, _: &mut Window, cx: &mut Context<Self>) {
        self.cut_now(cx)
    }
    pub(crate) fn cut_now(&mut self, cx: &mut Context<Self>) {
        if let Some(p) = &mut self.py_edit {
            let sel = p.ed.selection();
            if sel.start != sel.end {
                let t = p.ed.text()[sel].to_string();
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(t));
                p.ed.insert("");
                p.follow();
            }
            cx.notify();
            return;
        }
        if self.input.has_selection() {
            let sel = self.input.selection();
            if let Some(s) = self.input.text().get(sel) {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(s.to_string()));
                self.input.insert("");
                self.dirty = true;
                self.status = ui::t!("cut_selection").into();
            }
            cx.notify();
            return;
        }
        let (a, b) = self.sel_rect();
        let tsv = range_tsv(self.sheet(), a, b);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(tsv.clone()));
        // 切り取りの貼り付け先で式をずらさない(移動なので参照はそのまま)。
        // 形式を選択して貼り付けも切り取りでは使えない(Excel と同じ)
        self.clip = None;
        self.clip_cells = None;
        self.clip_range = None;
        self.checkpoint();
        let n = self.clear_range();
        self.status = ui::tf!("cut_cells", n).into();
        cx.notify();
    }

    /// 貼り付け。編集中なら文字として、そうでなければセルの格子として。
    pub(crate) fn a_paste(&mut self, _: &ui::Paste, _: &mut Window, cx: &mut Context<Self>) {
        self.paste_now(cx)
    }
    pub(crate) fn paste_now(&mut self, cx: &mut Context<Self>) {
        if self.py_edit.is_some() {
            let t = cx
                .read_from_clipboard()
                .and_then(|c| c.text())
                .unwrap_or_default();
            if let Some(p) = &mut self.py_edit {
                if !t.is_empty() {
                    p.ed.insert(&t);
                    p.follow();
                }
            }
            cx.notify();
            return;
        }
        if self.sheet().protected {
            self.status =
                ui::t!("sheet_protected_protection_tab").into();
            cx.notify();
            return;
        }
        let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) else {
            self.status = ui::t!("nothing_paste").into();
            cx.notify();
            return;
        };
        if text.is_empty() {
            cx.notify();
            return;
        }
        if self.editing() {
            // 打ちかけの間は文字の貼り付け(書きかけの式に継ぎ足す使い方)
            self.input.insert(&text);
            self.dirty = true;
            cx.notify();
            return;
        }
        // アプリ内のコピーなら、式の相対参照を貼り付け先へずらす
        let shift = match &self.clip {
            Some((org, tsv)) if *tsv == text => Some((
                self.cursor.row as i64 - org.row as i64,
                self.cursor.col as i64 - org.col as i64,
            )),
            _ => None,
        };
        self.checkpoint();
        let at = self.cursor;
        // **このアプリでコピーした範囲なら、書式も一緒に貼る**(本家と同じ。
        // 発注者 2026-08-14)。外から来た TSV には書式が無いので中身だけ —
        // その場合だけ貼り先の書式を据え置く(帳票の枠を壊さない)
        let (n, with_fmt) = match (&self.clip_cells, shift) {
            (Some(cells), Some(_)) => {
                let cells = cells.clone();
                (paste_all_cells(&mut self.book.sheets[self.active], at, &cells, shift), true)
            }
            _ => {
                let grid = tsv_grid(&text);
                (paste_grid(&mut self.book.sheets[self.active], at, &grid, shift), false)
            }
        };
        recalc_book(&mut self.book, self.active);
        self.dirty = true;
        self.sync_input();
        self.status = if with_fmt {
            ui::tf!("pasted_cells_formatting_too", n)
        } else {
            ui::tf!("pasted_cells_text_another", n)
        }
        .into();
        cx.notify();
    }
    /// 数式バーを打ちかけか(バーの中身がセルの保存内容から変わっているか)。
    /// バーには選んだセルの中身が常に写っているので、**空かどうかでは分からない**
    /// — 中身のあるセルで矢印が「見えない文字カーソル」に化け、
    /// セルから出られなくなる(踏んで直した)。
    /// 範囲を選んだまま打ち始めた — 字の入り先は**選択の起点**
    /// (Excel のアクティブセルと同じ)。cursor は動いた側の端に居るので、
    /// 両端を入れ替えて起点へ戻す。選択の範囲は変わらない。
    /// これが無いと「選んで 1 を打って Ctrl+D」の 1 が選択の下端に入り、
    /// 下向きコピーに上書きされて消える(発注者 2026-08-14 に報告された
    /// 「最後の行に入らない」の正体)
    pub(crate) fn edit_at_origin(&mut self) {
        if let Some(a) = self.anchor {
            if a != self.cursor {
                self.anchor = Some(self.cursor);
                self.cursor = a;
                self.follow();
            }
        }
    }

    pub(crate) fn editing(&self) -> bool {
        let saved = self.sheet().get(self.cursor).map(|c| c.editable()).unwrap_or_default();
        self.input.text() != saved
    }

    pub(crate) fn a_left(&mut self, _: &ui::Left, _: &mut Window, cx: &mut Context<Self>) {
        // 小窓 → パネル → 打ちかけの文字 → セル、の順で見る
        if let Some(ed) = &mut self.name_edit { ed.move_char(false, false) }
        else if self.fn_args.is_some() { self.editor().move_char(false, false) }
        else if let Some(d) = &mut self.fn_dlg { d.search.move_char(false, false) }
        else if let Some(ed) = &mut self.pick_filter { ed.move_char(false, false) }
        else if let Some(sv) = &mut self.solver { sv.focused().move_char(false, false) }
        else if let Some((_, ed)) = &mut self.prompt { ed.move_char(false, false) }
        else if self.editing() || self.edit_armed { self.input.move_char(false, false) }
        else { self.move_cursor(0, -1) }
        cx.notify();
    }
    pub(crate) fn a_right(&mut self, _: &ui::Right, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ed) = &mut self.name_edit { ed.move_char(true, false) }
        else if self.fn_args.is_some() { self.editor().move_char(true, false) }
        else if let Some(d) = &mut self.fn_dlg { d.search.move_char(true, false) }
        else if let Some(ed) = &mut self.pick_filter { ed.move_char(true, false) }
        else if let Some(sv) = &mut self.solver { sv.focused().move_char(true, false) }
        else if let Some((_, ed)) = &mut self.prompt { ed.move_char(true, false) }
        else if self.editing() || self.edit_armed { self.input.move_char(true, false) }
        else { self.move_cursor(0, 1) }
        cx.notify();
    }
    pub(crate) fn a_doc_home(&mut self, _: &ui::DocHome, _: &mut Window, cx: &mut Context<Self>) {
        // Ctrl+Home は A1 へ(表計算の作法)
        self.anchor = None;
        if !self.commit() {
            cx.notify();
            return;
        }
        self.cursor = Pos::new(0, 0);
        self.follow();
        self.sync_input();
        cx.notify();
    }
    pub(crate) fn a_doc_end(&mut self, _: &ui::DocEnd, _: &mut Window, cx: &mut Context<Self>) {
        // Ctrl+End は使われている範囲の右下へ
        self.anchor = None;
        if !self.commit() {
            cx.notify();
            return;
        }
        let (rows, cols) = self.sheet().extent();
        if rows > 0 {
            self.cursor = Pos::new(rows - 1, cols.saturating_sub(1));
        }
        self.follow();
        self.sync_input();
        cx.notify();
    }
    /// Ctrl+矢印 の行き先(Excel の作法):
    /// - 隣に中身があれば、**続く塊の終わり**まで飛ぶ
    /// - 隣が空なら、**次に中身のあるセル**まで飛ぶ
    ///
    /// 見つからなければ**使っている範囲の端**で止まる(本家は表の最果て
    /// = 1048576 行目まで飛ぶが、そこへ置き去りにする方が驚きが大きい)
    pub(crate) fn data_edge(&self, dr: i32, dc: i32) -> Pos {
        let has = |p: Pos| {
            self.sheet().get(p).is_some_and(|c| !c.value.is_empty())
        };
        let (rows, cols) = self.sheet().extent();
        let (maxr, maxc) = (rows.saturating_sub(1) as i64, cols.saturating_sub(1) as i64);
        let step = |p: Pos| -> Option<Pos> {
            let (r, c) = (p.row as i64 + dr as i64, p.col as i64 + dc as i64);
            (r >= 0 && c >= 0 && r <= maxr && c <= maxc).then(|| Pos::new(r as u32, c as u32))
        };
        let mut cur = self.cursor;
        let Some(next) = step(cur) else { return cur };
        cur = next;
        if has(next) {
            // 塊の終わりまで(次が空になる手前で止まる)
            while let Some(n) = step(cur) {
                if !has(n) {
                    break;
                }
                cur = n;
            }
        } else {
            // 次の中身まで(無ければ端で止まる)
            while !has(cur) {
                match step(cur) {
                    Some(n) => cur = n,
                    None => break,
                }
            }
        }
        cur
    }

    /// Ctrl+矢印(移動)と Ctrl+Shift+矢印(選択を伸ばす)の共通の実体
    pub(crate) fn go_edge(&mut self, dr: i32, dc: i32, extend: bool, cx: &mut Context<Self>) {
        if !self.commit() {
            cx.notify();
            return;
        }
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
        self.cursor = self.data_edge(dr, dc);
        self.follow();
        self.sync_input();
        if extend {
            let (a, b) = self.sel_rect();
            self.status = format!("{}:{}", a.a1(), b.a1()).into();
        }
        cx.notify();
    }
    // Ctrl+矢印は素では「データの端へ」。**図形を選んでいる間だけ**
    // 1px 動かすほうへ回す(2026-08-13 発注者)。選んでいなければ従来どおり
    pub(crate) fn a_word_left(&mut self, _: &ui::WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.nudge_shape(-1.0, 0.0) {
            cx.notify();
            return;
        }
        self.go_edge(0, -1, false, cx);
    }
    pub(crate) fn a_word_right(&mut self, _: &ui::WordRight, _: &mut Window, cx: &mut Context<Self>) {
        if self.nudge_shape(1.0, 0.0) {
            cx.notify();
            return;
        }
        self.go_edge(0, 1, false, cx);
    }
    pub(crate) fn a_sel_word_left(&mut self, _: &ui::SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.go_edge(0, -1, true, cx);
    }
    pub(crate) fn a_sel_word_right(
        &mut self,
        _: &ui::SelectWordRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.go_edge(0, 1, true, cx);
    }
    pub(crate) fn a_edge_up(&mut self, _: &ui::EdgeUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.nudge_shape(0.0, -1.0) {
            cx.notify();
            return;
        }
        self.go_edge(-1, 0, false, cx);
    }
    pub(crate) fn a_edge_down(&mut self, _: &ui::EdgeDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.nudge_shape(0.0, 1.0) {
            cx.notify();
            return;
        }
        self.go_edge(1, 0, false, cx);
    }
    pub(crate) fn a_sel_edge_up(&mut self, _: &ui::SelectEdgeUp, _: &mut Window, cx: &mut Context<Self>) {
        self.go_edge(-1, 0, true, cx);
    }
    pub(crate) fn a_sel_edge_down(&mut self, _: &ui::SelectEdgeDown, _: &mut Window, cx: &mut Context<Self>) {
        self.go_edge(1, 0, true, cx);
    }
    pub(crate) fn a_page_up(&mut self, _: &ui::PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_cursor(-(self.rows_snug() as i32 - 1).max(1), 0);
        cx.notify();
    }
    pub(crate) fn a_page_down(&mut self, _: &ui::PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_cursor((self.rows_snug() as i32 - 1).max(1), 0);
        cx.notify();
    }
    pub(crate) fn a_up(&mut self, _: &ui::Up, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = &mut self.py_edit {
            p.move_line(false, false);
        } else if let Some(a) = &mut self.fn_args {
            a.focus = a.focus.saturating_sub(1);
        } else if let Some(d) = &mut self.fn_dlg {
            d.sel = d.sel.saturating_sub(1);
        } else if self.dv_menu_move(false) {
            // 入力規則の小窓のドロップダウン(手順3)
        } else if self.pick_filtering() {
            self.pick_move(false);
        } else {
            self.move_cursor(-1, 0);
        }
        cx.notify();
    }
    pub(crate) fn a_down(&mut self, _: &ui::Down, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = &mut self.py_edit {
            p.move_line(true, false);
        } else if let Some(a) = &mut self.fn_args {
            a.focus = (a.focus + 1).min(a.eds.len().saturating_sub(1));
        } else if let Some(d) = &mut self.fn_dlg {
            let n = fn_filtered(d.search.text(), d.group).len();
            d.sel = (d.sel + 1).min(n.saturating_sub(1));
        } else if self.dv_menu_move(true) {
            // 入力規則の小窓のドロップダウン(手順3)
        } else if self.pick_filtering() {
            self.pick_move(true);
        } else {
            self.move_cursor(1, 0);
        }
        cx.notify();
    }
    /// 行頭へ(字の始まり ⇄ 行頭)。**.py の編集面のときだけ働く** —
    /// 表では Home に持ち場が無いので、今までどおり何もしない
    pub(crate) fn a_home(&mut self, _: &ui::Home, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = &mut self.py_edit {
            p.home(false);
            cx.notify();
        }
    }
    pub(crate) fn a_end(&mut self, _: &ui::End, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = &mut self.py_edit {
            p.end(false);
            cx.notify();
        }
    }
    pub(crate) fn a_tab(&mut self, _: &ui::Tab, _: &mut Window, cx: &mut Context<Self>) {
        // .py では字下げ(空白4つ)。Python は字下げが構文なので、
        // タブ文字ではなく空白で入れる
        if let Some(p) = &mut self.py_edit {
            p.ed.insert("x");
            p.follow();
            cx.notify();
            return;
        }
        if let Some(a) = &mut self.fn_args {
            if !a.eds.is_empty() {
                a.focus = (a.focus + 1) % a.eds.len();
            }
        } else {
            self.move_cursor(0, 1);
        }
        cx.notify();
    }
    /// Ctrl+Shift+Enter = **昔ながらの配列数式**。選んだ範囲に同じ式を
    /// 入れ、範囲いっぱいに答えを配る。範囲を選んでいなければ今のセル1つ。
    ///
    /// 動的配列(FILTER などのスピル)がある今でもこれが要るのは、
    /// **古い帳票がこの形で書かれている**から。読めて書けて、同じ手で
    /// 直せないと乗り換えられない。
    /// Shift+F3 = 関数の挿入(本家と同じ鍵)
    pub(crate) fn a_insert_fn(&mut self, _: &ui::InsertFn, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("insert-function", cx);
        cx.notify();
    }
    /// Ctrl+Shift+% = パーセント書式
    pub(crate) fn a_percent(&mut self, _: &ui::PercentFmt, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("percents", cx);
        cx.notify();
    }
    /// Ctrl+P = 印刷。**こちらは紙に出す口を持たないので PDF にする** —
    /// 「印刷しました」と言って何も出ないより、出来た物を渡す
    pub(crate) fn a_print(&mut self, _: &ui::Print, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("pdf", cx);
        cx.notify();
    }
    /// F11 = 全画面(帳票を広く見る)
    pub(crate) fn a_fullscreen(&mut self, _: &ui::FullScreen, window: &mut Window, cx: &mut Context<Self>) {
        window.toggle_fullscreen();
        self.status = ui::t!("toggled_full_screen_f11").into();
        cx.notify();
    }
    /// Ctrl+Shift+S = 名前を付けて保存
    pub(crate) fn a_save_as(&mut self, _: &ui::SaveAs, _: &mut Window, cx: &mut Context<Self>) {
        self.save_as(cx);
        cx.notify();
    }

    /// Ctrl+0 = ズームを 100% に戻す
    pub(crate) fn a_zoom_reset(&mut self, _: &ui::ZoomReset, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom = 1.0;
        self.status = ui::t!("zoom_back_100").into();
        cx.notify();
    }
    /// F1 = 手引き。**中に画面を作らない** — 手引きは docs にある文書なので、
    /// その道を状態行で示す(嘘の「ヘルプ画面」を出すより確か)
    pub(crate) fn a_help(&mut self, _: &ui::Help, _: &mut Window, cx: &mut Context<Self>) {
        self.status = ui::t!(
            "manual_docs_ja_calc"
        )
        .into();
        cx.notify();
    }
    /// Ctrl+; = 今日の日付、Ctrl+: = 今の時刻。**値として入れる** —
    /// TODAY() だと開くたびに変わって、いつ書いたか分からなくなる
    pub(crate) fn a_ins_date(&mut self, _: &ui::InsDate, _: &mut Window, cx: &mut Context<Self>) {
        self.insert_stamp(false, cx);
    }
    pub(crate) fn a_ins_time(&mut self, _: &ui::InsTime, _: &mut Window, cx: &mut Context<Self>) {
        self.insert_stamp(true, cx);
    }
    pub(crate) fn insert_stamp(&mut self, time: bool, cx: &mut Context<Self>) {
        if self.cell_locked(self.cursor) {
            self.status = Self::protected_msg().into();
            cx.notify();
            return;
        }
        // now_stamp は「YYYY-MM-DD HH:MM」。日付か時刻か、要る側だけ取る
        let stamp = now_stamp();
        let Some((date, clock)) = stamp.split_once(' ') else {
            // 黙って空を入れない
            self.status = ui::t!("couldnt_get_current_time").into();
            cx.notify();
            return;
        };
        let now = if time { clock.to_string() } else { date.to_string() };
        self.input.insert(&now);
        self.edit_armed = true;
        self.status = ui::tf!("put_enter_commits_value", now)
            .into();
        cx.notify();
    }
    /// Alt+PageUp / PageDown = 前後のシートへ
    pub(crate) fn a_prev_sheet(&mut self, _: &ui::PrevSheet, _: &mut Window, cx: &mut Context<Self>) {
        // Alt を使った組み合わせなので、キーヒントの見張りは倒す —
        // 離した拍子に札が出ては邪魔になる
        self.alt_armed = false;
        self.hop_sheet(-1, cx);
    }
    pub(crate) fn a_next_sheet(&mut self, _: &ui::NextSheet, _: &mut Window, cx: &mut Context<Self>) {
        // Alt を使った組み合わせなので、キーヒントの見張りは倒す —
        // 離した拍子に札が出ては邪魔になる
        self.alt_armed = false;
        self.hop_sheet(1, cx);
    }
    pub(crate) fn hop_sheet(&mut self, d: i32, cx: &mut Context<Self>) {
        // 隠したシートは飛ばす(タブに出ていないものへ行かない)
        let n = self.book.sheets.len();
        let mut i = self.active as i32;
        for _ in 0..n {
            i = (i + d).rem_euclid(n as i32);
            if !self.book.sheets[i as usize].hidden {
                self.switch_sheet(i as usize);
                cx.notify();
                return;
            }
        }
    }
    /// F4 = 参照の $ を回す(A1 → $A$1 → A$1 → $A1 → A1)。
    /// **打っている式の、カーソルの直前の参照**を回す
    /// Alt+S = スライサーの複数選択の入切、Alt+C = 絞りの解除。
    /// **板が開いている間だけの鍵。** 開いていないときは黙らずにそう言う —
    /// 押して何も起きないと、効かないのか開いていないのか分からない
    pub(crate) fn a_slicer_multi(&mut self, _: &ui::SlicerMulti, _: &mut Window, cx: &mut Context<Self>) {
        // Alt を使った組み合わせなので、キーヒントの見張りは倒す —
        // 離した拍子に札が出ては邪魔になる
        self.alt_armed = false;
        // 効くのは**いま触っている板**(何枚でも開ける造りになったので)
        let now = self.slicer_cur().map(|sl| {
            sl.multi = !sl.multi;
            sl.multi
        });
        self.status = match now {
            Some(true) => ui::t!("multi_select_pressed_values").into(),
            Some(false) => ui::t!("single_select_filter_one").into(),
            None => Self::no_slicer_msg().into(),
        };
        cx.notify();
    }
    pub(crate) fn a_slicer_clear(&mut self, _: &ui::SlicerClear, _: &mut Window, cx: &mut Context<Self>) {
        // Alt を使った組み合わせなので、キーヒントの見張りは倒す —
        // 離した拍子に札が出ては邪魔になる
        self.alt_armed = false;
        let hit = self.slicer_cur().map(|sl| sl.sel.clear()).is_some();
        self.status = if hit {
            ui::t!("slicer_filter_cleared").into()
        } else {
            Self::no_slicer_msg().into()
        };
        cx.notify();
    }
    pub(crate) fn no_slicer_msg() -> String {
        ui::t!("no_slicer_open_open")
            .to_string()
    }
    pub(crate) fn a_cycle_ref(&mut self, _: &ui::CycleRef, _: &mut Window, cx: &mut Context<Self>) {
        let t = self.input.text().to_string();
        if !t.starts_with('=') {
            self.status = ui::t!("f4_works_reference_inside").into();
            cx.notify();
            return;
        }
        match cycle_ref_at(&t, self.input.cursor()) {
            Some((txt, cur)) => {
                self.input = Editor::new(&txt);
                self.input.move_to(cur, false);
                self.edit_armed = true;
                self.status = ui::t!("cycled_reference_f4_again").into();
            }
            None => {
                self.status = ui::t!("no_reference_before_cursor").into();
            }
        }
        cx.notify();
    }

    /// Ctrl+E = フラッシュフィル
    pub(crate) fn a_flash_fill(&mut self, _: &ui::FlashFill, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("flash-fill", cx);
        cx.notify();
    }

    pub(crate) fn a_array_enter(&mut self, _: &ui::ArrayEnter, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.text().to_string();
        self.set_array_formula(&text, cx);
    }

    /// 選んでいる範囲に配列数式を入れる(Ctrl+Shift+Enter の中身)。
    /// 窓を要らない形にして、画面なしの試験からも呼べるようにしてある
    pub(crate) fn set_array_formula(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = text.to_string();
        if !text.starts_with('=') {
            self.status =
                ui::t!("array_formula_needs_formula").into();
            cx.notify();
            return;
        }
        if self.cell_locked(self.cursor) {
            self.status = Self::protected_msg().into();
            cx.notify();
            return;
        }
        let (a, b) = self.sel_rect();
        let (h, w) = (b.row - a.row + 1, b.col - a.col + 1);
        self.checkpoint();
        // 起点に式、覆う範囲を控える。範囲の残りは計算が埋める
        let mut c = self.sheet().get(a).cloned().unwrap_or_default();
        c.formula = Some(text[1..].to_string());
        self.book.sheets[self.active].set(a, c);
        self.book.sheets[self.active].cse.insert(a, (h, w));
        self.cursor = a;
        self.anchor = None;
        self.dirty = true;
        recalc_book(&mut self.book, self.active);
        self.sync_input();
        self.status = ui::tf!(
            "put_array_formula_formula",
            a.a1(),
            b.a1()
        )
        .into();
        cx.notify();
    }

    pub(crate) fn a_enter(&mut self, _: &ui::Enter, _: &mut Window, cx: &mut Context<Self>) {
        // フォルダから探す(ファイルの面)。Enter で探す
        if self.tab == 0 && self.file_view == 3 {
            self.find_in_folder();
            cx.notify();
            return;
        }
        // 会話の欄の Enter = 送る(焦点はそのまま — 続けて書ける)
        if self.chat_focus {
            self.chat_send(cx);
            cx.notify();
            return;
        }
        if let Some(p) = &mut self.py_edit {
            p.newline();
            self.py_edit_ask = false;
            cx.notify();
            return;
        }
        if self.quit_ask {
            // Enter = 保存して終了(いちばん安全な既定)
            self.quit_ask = false;
            self.save(true, cx);
            cx.notify();
            return;
        }
        if self.name_edit.is_some() {
            self.commit_name_box();
            cx.notify();
            return;
        }
        if self.fn_args.is_some() {
            self.fn_args_ok();
            cx.notify();
            return;
        }
        if self.fn_dlg.is_some() {
            self.fn_next();
            cx.notify();
            return;
        }
        // 絞り込みつきの一覧: Enter で選択中の項(合致が無ければ打った字)を確定
        if self.pick_filtering() {
            self.pick_confirm(cx);
            cx.notify();
            return;
        }
        if self.solver.is_some() {
            // 小窓の Enter では何も走らせない(解くのは「解を求める」のボタン)
            cx.notify();
            return;
        }
        if self.dv_dlg.is_some() {
            // **ドロップダウンが開いていれば、まずそれに決める**(手順3)。
            // 開いたまま Enter で小窓ごと閉じると、選んだつもりが入りません
            if self.dv_menu_enter() {
                cx.notify();
                return;
            }
            // 入力規則のパネルの Enter = OK(本家と同じ)
            self.dv_ok(cx);
            return;
        }
        if self.prompt.is_some() {
            self.finish_prompt(cx);
        } else if let Some(i) = self.shape_sel {
            // 図形を選んで Enter = 中の文字を書く(テキストボックス)
            let cur = self
                .sheet()
                .shapes_new
                .get(i)
                .and_then(|sp| sp.text.clone())
                .unwrap_or_default();
            self.prompt = Some(("shape-text", Editor::new(&cur)));
        } else {
            self.move_cursor(1, 0);
        }
        cx.notify();
    }
    pub(crate) fn a_select_left(&mut self, _: &ui::SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.editing() { self.input.move_char(false, true) }
        else { self.extend(0, -1) }
        cx.notify();
    }
    pub(crate) fn a_select_right(&mut self, _: &ui::SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        if self.editing() { self.input.move_char(true, true) }
        else { self.extend(0, 1) }
        cx.notify();
    }
    pub(crate) fn a_select_up(&mut self, _: &ui::SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.extend(-1, 0); cx.notify();
    }
    pub(crate) fn a_select_down(&mut self, _: &ui::SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.extend(1, 0); cx.notify();
    }
    pub(crate) fn a_select_all(&mut self, _: &ui::SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all_now();
        cx.notify();
    }
    /// 全選択の実体。Ctrl+A ともリボンの「すべて選択」とも同じ道を通す
    /// (リボンだけバーの文字選択、という別物にしない)
    pub(crate) fn select_all_now(&mut self) {
        // .py の編集面が開いていれば、選ぶのは**コードの文字**(表ではない)
        if let Some(p) = &mut self.py_edit {
            p.ed.select_all();
            return;
        }
        if self.editing() {
            // 打ちかけの間は、バーの文字の全選択
            self.input.select_all();
        } else {
            // 使われている範囲の全選択(表計算の Ctrl+A)
            let (rows, cols) = self.sheet().extent();
            if rows == 0 {
                self.status = ui::t!("sheet_empty").into();
            } else {
                self.commit();
                self.anchor = Some(Pos::new(0, 0));
                self.cursor = Pos::new(rows - 1, cols.saturating_sub(1));
                self.status = ui::tf!("a1_selected", self.cursor.a1()).into();
                self.sync_input();
            }
        }
    }
    pub(crate) fn a_undo(&mut self, _: &ui::Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = &mut self.py_edit {
            p.ed.undo();
            p.follow();
            cx.notify();
            return;
        }
        if !self.input.undo() {
            self.undo_sheet();
        }
        cx.notify();
    }
    pub(crate) fn a_redo(&mut self, _: &ui::Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = &mut self.py_edit {
            p.ed.redo();
            p.follow();
            cx.notify();
            return;
        }
        if !self.input.redo() {
            self.redo_sheet();
        }
        cx.notify();
    }
    /// F9 = ブック全体の再計算(計算方法が手動のときの手回し。自動でも害はない)
    /// Ctrl+F / Ctrl+H = 検索と置換のパネル。**受け口が無く、割り当てだけが
    /// あった** — 押しても何も起きない「キーの嘘」だった(2026-08-09)
    pub(crate) fn a_find(&mut self, _: &ui::Find, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("replace", cx);
        cx.notify();
    }
    /// 文字飾りの割り当て(本家 Ctrl+B / I / U / 5)。リボンのボタンと同じ道
    pub(crate) fn a_bold(&mut self, _: &ui::Bold, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("bold", cx);
        cx.notify();
    }
    pub(crate) fn a_italic(&mut self, _: &ui::Italic, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("italic", cx);
        cx.notify();
    }
    pub(crate) fn a_underline(&mut self, _: &ui::Underline, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("underline", cx);
        cx.notify();
    }
    pub(crate) fn a_strikeout(&mut self, _: &ui::Strikeout, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("strikeout", cx);
        cx.notify();
    }
    pub(crate) fn a_recalc(&mut self, _: &ui::Recalc, _: &mut Window, cx: &mut Context<Self>) {
        self.commit();
        recalc_book(&mut self.book, self.active);
        self.status = ui::t!("recalculated_whole_workbook").into();
        cx.notify();
    }
    /// Shift+F9 = いまのシートだけ再計算(大きなブックで待たされない)
    pub(crate) fn a_recalc_sheet(&mut self, _: &ui::RecalcSheet, _: &mut Window, cx: &mut Context<Self>) {
        self.commit();
        recalc(&mut self.book.sheets[self.active]);
        self.status = ui::t!("recalculated_sheet_only").into();
        cx.notify();
    }
    /// Ctrl+K = ハイパーリンク(Excel と同じ)
    pub(crate) fn a_ins_link(&mut self, _: &ui::InsLink, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("inshyperlink", cx);
    }
    /// Ctrl+= / Ctrl+- = 画面の文字の大きさ(リボンから状態行まで全部)
    pub(crate) fn a_ui_bigger(&mut self, _: &ui::UiBigger, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("ui-bigger", cx);
    }
    pub(crate) fn a_ui_smaller(&mut self, _: &ui::UiSmaller, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("ui-smaller", cx);
    }
    /// Alt+Enter = セルの中の改行(Excel と同じ)。確定時に折り返しも立てる
    pub(crate) fn a_newline(&mut self, _: &ui::NewLine, _: &mut Window, cx: &mut Context<Self>) {
        // Alt を使った組み合わせなので、キーヒントの見張りは倒す —
        // 離した拍子に札が出ては邪魔になる
        self.alt_armed = false;
        if self.editing() || self.edit_armed {
            self.input.insert("\n");
            cx.notify();
        }
    }
    /// リボンのコマンド。数式タブは選択セルに関数を入れる。
    /// 選んでいるセルの見た目を変える。
    ///
    /// **値の無いセルにも掛ける** — 罫線だけを引くのは帳票では普通の操作。
    pub(crate) fn fmt(&mut self, f: impl Fn(&mut CellFormat)) {
        // 保護中でも「セルの書式設定」を許していれば通す。
        // **ロックそのものの掛け外しは書式ではない** — これを禁じると
        // 保護を解かないと記入欄を作れなくなる(卵と鶏)ので、保護中の
        // ロック操作は run_cmd 側で断る
        if self.sheet().protected && !self.sheet().protect_allow.format_cells {
            self.status = Self::protected_msg().into();
            return;
        }
        self.commit();
        self.checkpoint();
        // 範囲選択があれば全部に掛ける。罫線も塗りも、帳票は範囲でやる仕事
        let (a, b) = self.sel_rect();
        for r in a.row..=b.row {
            for cidx in a.col..=b.col {
                let p = Pos::new(r, cidx);
                let mut c = self.sheet().get(p).cloned().unwrap_or_default();
                f(&mut c.fmt);
                self.book.sheets[self.active].set(p, c);
            }
        }
        self.dirty = true;
        recalc_book(&mut self.book, self.active);
    }

    /// 結合の種類を選んだ後の入り口。**値は消さない** — 左上以外の値は
    /// 隠れるだけで、解除で戻る。値が2つ以上見えているときは先に聞く
    /// (本家と同じ — 画面と Excel では各結合の左上しか見えなくなるから)
    pub(crate) fn merge_selection(&mut self, kind: &str) {
        let (a, b) = self.sel_rect();
        if a == b {
            self.status = ui::t!("select_range_merge_shift").into();
            return;
        }
        if kind == "解除" {
            self.checkpoint();
            let n = self.book.sheets[self.active].unmerge(a, b);
            self.status = ui::tf!("merges_removed", n).into();
            self.dirty = true;
            return;
        }
        // 確認は出さない(発注者 2026-08-08)。左上以外の値は消す —
        // 残すと見えない値が式に効く。消しても Ctrl+Z 一発で戻るので、
        // 警告で手を止めさせる理由が無い
        let filled = (a.row..=b.row)
            .flat_map(|r| (a.col..=b.col).map(move |c| Pos::new(r, c)))
            .filter(|p| {
                self.sheet().get(*p).map(|c| !c.editable().trim().is_empty()).unwrap_or(false)
            })
            .count();
        self.merge_do(a, b, kind);
        if filled >= 2 {
            // 消したことを言う(黙らない。Ctrl+Z 一発で戻るから止めない)
            self.status = ui::tf!(
                "values_outside_top_left",
                self.status
            )
            .into();
        }
    }

    /// 結合の実体(確認の後もここに来る)。kind: 中央/横方向/結合だけ
    ///
    /// 呑まれるセルの中身の扱い(消す・空の左上へ移す)は家の作法として
    /// `Sheet::merge`(sheet::model::ops)にある — Python(pysheet)から
    /// 結合しても同じ結果になるように、2026-08-12 に共有クレートへ移した。
    /// 消すのは Ctrl+Z(この checkpoint)で戻せる — だから確認も出さない。
    /// 横方向は行ごとが1つの結合なので、行ごとに同じ扱い
    pub(crate) fn merge_do(&mut self, a: Pos, b: Pos, kind: &str) {
        self.checkpoint();
        let sh = &mut self.book.sheets[self.active];
        let bundles: Vec<(Pos, Pos)> = if kind == "横方向" {
            (a.row..=b.row)
                .map(|r| (Pos::new(r, a.col), Pos::new(r, b.col)))
                .collect()
        } else {
            vec![(a, b)]
        };
        let mut promoted = false;
        for (ba, bb) in bundles {
            promoted |= sh.merge(ba, bb);
        }

        match kind {
            // 横方向: 行ごとに1本ずつ(本家の Merge Across)
            "横方向" => {
                self.status = ui::tf!(
                    "merged_across_rows",
                    a.a1(), b.a1(), b.row - a.row + 1
                )
                .into();
            }
            // 結合だけ(揃えは触らない — 本家の Merge Cells)
            "結合だけ" => {
                self.status = ui::tf!("merged_alignment_untouched", a.a1(), b.a1()).into();
            }
            _ => {
                // 名のとおり中央揃えも掛ける(解くときは揃えを触らない)
                let mut anchor = sh.get(a).cloned().unwrap_or_default();
                anchor.fmt.align = sheet::model::HAlign::Center;
                anchor.fmt.valign = sheet::model::VAlign::Middle;
                sh.set(a, anchor);
                self.status =
                    ui::tf!("merged_centred", a.a1(), b.a1()).into();
            }
        }
        if promoted {
            // 空だった左上へ最初の値を移したことを言う(黙って動かさない)
            self.status = ui::tf!("top_left_empty_first", self.status).into();
        }
        self.dirty = true;
    }

    /// 行・列を出し入れする。
    pub(crate) fn rowcol(&mut self, f: impl Fn(&mut sheet::Sheet, Pos)) {
        self.commit();
        self.checkpoint();
        let p = self.cursor;
        f(&mut self.book.sheets[self.active], p);
        self.dirty = true;
        recalc_book(&mut self.book, self.active);
    }

    /// 小数点以下の桁を増減する。
    ///
    /// **0〜10 に留める。** 際限なく増やせると、桁だけの帳票が出来上がる。
    pub(crate) fn decimals(&mut self, d: i32) {
        self.fmt(move |f| {
            let now = f
                .number_format
                .as_deref()
                .and_then(|s| s.rsplit_once('.'))
                .map(|(_, dec)| dec.chars().take_while(|c| *c == '0').count() as i32)
                .unwrap_or(0);
            let n = (now + d).clamp(0, 10);
            let comma = f.number_format.as_deref().is_some_and(|s| s.contains(','));
            let head = if comma { "#,##0" } else { "0" };
            f.number_format = Some(if n == 0 {
                head.to_string()
            } else {
                format!("{head}.{}", "0".repeat(n as usize))
            });
        });
    }

    // ---- 定番の増強(2026-08-14 発注者「割り当てが足りない」) ----
    // どれもリボンと同じ道(run_cmd)を通す — 鍵だけの別実装を作らない

    /// Ctrl+1 = セルの書式(本家と同じ)
    pub(crate) fn a_cell_format(&mut self, _: &ui::CellFormat, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("cell-format", cx);
        cx.notify();
    }
    /// Alt+= = オートSUM(ホームの Σ と同じ)
    pub(crate) fn a_auto_sum(&mut self, _: &ui::AutoSum, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("sum", cx);
        cx.notify();
    }
    /// Ctrl+D = 下へコピー(フィルと同じ道)
    pub(crate) fn a_fill_down(&mut self, _: &ui::FillDown, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("fill-num", cx);
        cx.notify();
    }
    /// Ctrl+R = 右へコピー
    pub(crate) fn a_fill_right(&mut self, _: &ui::FillRight, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("fill-right", cx);
        cx.notify();
    }
    /// Ctrl+T = 表にする(すぐ作る側。色を選ぶのはリボンの表↧)
    pub(crate) fn a_make_table(&mut self, _: &ui::MakeTable, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("instable", cx);
        cx.notify();
    }
    /// Shift+F2 = コメント
    pub(crate) fn a_add_comment(&mut self, _: &ui::AddComment, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("co-addcomment", cx);
        cx.notify();
    }
    /// Ctrl+Shift+L = フィルタの付け外し(setfilter 自体が切替の作り)
    pub(crate) fn a_toggle_filter(&mut self, _: &ui::ToggleFilter, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("setfilter", cx);
        cx.notify();
    }
    /// Ctrl+G / F5 = ジャンプ。名前ボックスに焦点を移す(押した時と同じ)
    pub(crate) fn a_jump(&mut self, _: &ui::Jump, _: &mut Window, cx: &mut Context<Self>) {
        self.commit();
        self.name_edit = Some(Editor::new(""));
        self.status = ui::t!(
            "name_box_go_cell")
        .into();
        cx.notify();
    }
    /// Ctrl+Space = 列の選択(いまの選択の列を丸ごと)
    pub(crate) fn a_select_col(&mut self, _: &ui::SelectCol, _: &mut Window, cx: &mut Context<Self>) {
        if self.editing() || self.py_edit.is_some() {
            return; // 打ちかけの間は触らない(字の入力と取り合わない)
        }
        self.select_col_now();
        cx.notify();
    }
    /// 列の丸ごと選択の実体(試験もここを叩く)
    pub(crate) fn select_col_now(&mut self) {
        self.commit();
        let (rows, _) = self.sheet().size();
        let (a, b) = self.sel_rect();
        self.anchor = Some(Pos::new(0, a.col));
        self.cursor = Pos::new(rows.max(1) - 1, b.col);
        self.status = ui::t!("column_selected_shift_space").into();
        self.sync_input();
    }
    /// Shift+Space = 行の選択(いまの選択の行を丸ごと)
    pub(crate) fn a_select_row(&mut self, _: &ui::SelectRow, _: &mut Window, cx: &mut Context<Self>) {
        // 打ちかけの間、Shift+Space は**空白の字**。捌きが鍵を食うので
        // 明示で入れる(黙って何も起きないのが一番悪い)
        if let Some(p) = &mut self.py_edit {
            p.ed.insert(" ");
            cx.notify();
            return;
        }
        if self.editing() {
            self.input.insert(" ");
            self.dirty = true;
            cx.notify();
            return;
        }
        self.select_row_now();
        cx.notify();
    }
    /// 行の丸ごと選択の実体(試験もここを叩く)
    pub(crate) fn select_row_now(&mut self) {
        self.commit();
        let (_, cols) = self.sheet().size();
        let (a, b) = self.sel_rect();
        self.anchor = Some(Pos::new(a.row, 0));
        self.cursor = Pos::new(b.row, cols.max(1) - 1);
        self.status = ui::t!("row_selected_ctrl_space").into();
        self.sync_input();
    }
}
