//! Python(officework)からの遠隔操作の口。
//!
//! Jupyter の xlwings 流の使い勝手(Book / Range / .value / DataFrame)を
//! **動いている calc** に向ける(発注者 2026-08-08 — Qiita の記事の車線)。
//! ユニックスソケット `$XDG_RUNTIME_DIR/officework/calc.sock` に JSON を
//! 1行ずつ。**この機械の中だけ**(TCP は開かない — ネイティブファースト)。
//!
//! スレッドの作法: ソケットのスレッドは状態に触らない。要求を溜め、GPUI の側が
//! 30ms ごとにメインスレッドで捌いて答えを返す(Editor 系と同じ「主で触る」を守る)。
//!
//! **命令の意味は ops へ移した**(SEKKEI「操作の言葉を1本に」段A。2026-08-12)。
//! ここに残るのは calc にしか無い物: ソケットと汲み取り(gpui)、Host の実装
//! (undo の節目・状態行・行の高さ合わせ)、点検用の ribbon / ui_state。

use crate::*;
use ops::{Host, J, Jobj};

/// 口を開く。聞き取りのスレッドを立て、メインスレッドに 30ms の汲み取りを付ける。
pub(crate) fn start(view: gpui::Entity<Calc>, cx: &mut gpui::App) {
    // **ソケットの世話は ops に1本**(2026-08-19)。writer も同じ物を使います
    let queue: ops::Queue = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    if !ops::listen("calc", queue.clone()) {
        return;
    }
    // 30ms ごとに溜まった要求をメインスレッドで捌く
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(30))
                .await;
            let reqs: Vec<ops::Req> = std::mem::take(&mut *queue.lock().expect("受け口の錠"));
            if reqs.is_empty() {
                continue;
            }
            view.update(cx, |calc, cx| {
                for req in reqs {
                    let resp = ops::handle(calc, &req.line);
                    let _ = req.reply.send(resp);
                }
                cx.notify();
            });
        }
    })
    .detach();
}

/// 1要求を捌く(メインスレッド)。答えは JSON 1行。
/// 意味は ops::handle — この包みは試験の呼び出し(署名に cx)を変えないため
#[cfg(test)]
pub(crate) fn handle(calc: &mut Calc, line: &str, _cx: &mut Context<Calc>) -> String {
    ops::handle(calc, line)
}

/// 「動いているアプリの都合」の実装。切れない部分がここに名前で並ぶ —
/// undo の節目・状態行・行の高さ合わせ・画面の点検。これ以外の意味は ops
impl Host for Calc {
    fn app(&self) -> &'static str {
        "calc"
    }
    fn book(&self) -> &sheet::Book {
        &self.book
    }
    fn book_mut(&mut self) -> &mut sheet::Book {
        &mut self.book
    }
    fn active(&self) -> usize {
        self.active
    }
    fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    fn settle(&mut self) {
        self.commit();
    }
    fn dirty(&self) -> bool {
        self.dirty
    }
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    fn mark_once(&mut self) {
        // 手続きの最中は節目を作らない(手続きの頭で1つ置いてある) —
        // 何回書いても Ctrl+Z 一回で手続きの前に戻る
        if !self.rpc_batch {
            self.checkpoint();
        }
    }
    fn after_write(&mut self, si: usize, written: &[Pos]) {
        // 見出しを書いたら行を広げる(手で打ったときと同じ扱い)。
        // いま出ているシートのときだけ — 他のシートの行は触らない
        if si == self.active {
            for p in written {
                self.fit_row_to_cellmark(*p);
            }
        }
    }
    fn wrote(&mut self, n: usize) {
        self.sync_input();
        self.status = ui::tf!("Python wrote {} cells", n).into();
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn selection(&self) -> Option<(usize, Pos, Pos)> {
        let a = self.anchor.unwrap_or(self.cursor);
        let c = self.cursor;
        Some((
            self.active,
            Pos::new(a.row.min(c.row), a.col.min(c.col)),
            Pos::new(a.row.max(c.row), a.col.max(c.col)),
        ))
    }

    fn select(&mut self, si: usize, a: Pos, b: Pos) -> Result<(), String> {
        if !self.commit() {
            return Err(
                "打ちかけの入力が入力規則で戻されました(直すか、Esc で取り消してから)".into(),
            );
        }
        ops::Host::activate_sheet(self, si)?;
        self.cursor = a;
        self.anchor = (a != b).then_some(b);
        self.sync_input();
        Ok(())
    }

    fn activate_sheet(&mut self, si: usize) -> Result<(), String> {
        if si >= self.book.sheets.len() {
            return Err("そのシートがありません".into());
        }
        if si != self.active {
            self.switch_sheet(si);
            if self.active != si {
                // 打ちかけが入力規則で戻された等 — 理由は状態行にある
                return Err(format!("{}", self.status));
            }
        }
        Ok(())
    }

    fn set_status(&mut self, text: &str) -> Result<(), String> {
        self.status = text.to_string().into();
        Ok(())
    }

    fn to_pdf(&mut self, si: usize, p: &std::path::Path) -> Result<String, String> {
        if si >= self.book.sheets.len() {
            return Err("そのシートがありません".into());
        }
        // write_pdf は self.active のシートを出す — 一時的に差し替えて戻す
        // (画面の場所は動かさない。結果は status の文言で返す)
        let prev = self.active;
        self.active = si;
        self.write_pdf(p);
        self.active = prev;
        if p.exists() {
            Ok(format!("{}", self.status))
        } else {
            Err(format!("{}", self.status))
        }
    }

    fn book_to_pdf(&mut self, p: &std::path::Path) -> Result<String, String> {
        let note = Calc::write_book_pdf(self, p)?;
        self.status = note.clone().into();
        Ok(note)
    }

    fn copy_sheet(&mut self, si: usize, name: Option<&str>) -> Result<String, String> {
        let n = self.copy_sheet_at(si, name)?;
        self.status = ui::tf!("Created \"{}\"", n).into();
        Ok(n)
    }

    fn delete_sheet(&mut self, si: usize) -> Result<String, String> {
        let n = self.delete_sheet_at(si)?;
        self.status =
            ui::tf!("Deleted sheet \"{}\" (this can't be undone)", n).into();
        Ok(n)
    }

    fn get_freeze(&mut self, si: usize) -> Result<(u32, u32), String> {
        if si >= self.book.sheets.len() {
            return Err("そのシートがありません".into());
        }
        self.remember_ui(); // sheet_ui をシート数まで育て、いまの画面の固定を写す
        Ok(self
            .sheet_ui
            .get(si)
            .and_then(|u| u.2)
            .map(|p| (p.row, p.col))
            .unwrap_or((0, 0)))
    }

    fn set_freeze(&mut self, si: usize, rows: u32, cols: u32) -> Result<(), String> {
        if si >= self.book.sheets.len() {
            return Err("そのシートがありません".into());
        }
        self.remember_ui();
        let f = (rows > 0 || cols > 0).then_some(Pos::new(rows, cols));
        self.sheet_ui[si].2 = f;
        if si == self.active {
            self.frozen = f;
        }
        self.dirty = true; // 保存の直前に freeze_into_book がモデルへ写す
        Ok(())
    }

    fn set_sheet_hidden(&mut self, si: usize, hidden: bool) -> Result<(), String> {
        Calc::set_sheet_hidden(self, si, hidden)
    }

    fn autofit(&mut self, si: usize, a: Pos, b: Pos, col: bool) -> Result<usize, String> {
        if si >= self.book.sheets.len() {
            return Err("そのシートがありません".into());
        }
        // 測りはいま出ているシートの列幅を見る(折り返しの行高)ので、
        // 別のシートを合わせるときは一時的に差し替えて戻す
        let prev = self.active;
        self.active = si;
        let n = self.autofit_at(a, b, col);
        self.active = prev;
        self.status = if col {
            ui::tf!("Fitted {} columns to their contents (Ctrl+Z undoes it)", n).into()
        } else {
            ui::tf!("Fitted {} rows to their contents (Ctrl+Z undoes it)", n).into()
        };
        Ok(n)
    }

    fn new_book(&mut self) -> Result<(), String> {
        if Calc::new_book(self) {
            Ok(())
        } else {
            Err(format!("{}", self.status))
        }
    }
    fn open(&mut self, p: &std::path::Path) -> Result<(), String> {
        Calc::open(self, p.to_path_buf());
        if self.path.as_deref() != Some(p) {
            return Err(ui::tf!("Can't open: {}", self.status));
        }
        Ok(())
    }
    fn save(&mut self, p: std::path::PathBuf) -> Result<(), String> {
        // **拾い集めたブックで元のファイルを上書きしない**(2026-08-22)。
        // 画面の「保存」だけを塞いでも、こちらの口が空いていたら同じ事故が
        // 起きます。別の名前でなら書けます
        if self.salvaged && self.path.as_deref() == Some(p.as_path()) {
            return Err(ui::t!(
                "This workbook was salvaged, so it will not overwrite. Use Save As (the original file is left alone)"
            )
            .to_string());
        }
        self.save_to(p);
        Ok(())
    }

    fn close_book(&mut self) -> Result<(), String> {
        // アプリは常にブックを1つ持つ造りなので、「閉じる」は**新しい空の
        // ブックに戻る**こと(窓は閉じない — 起動も終了も人の物)
        if Calc::new_book(self) {
            self.status = ui::t!("Workbook closed (this is now a new workbook)").into();
            Ok(())
        } else {
            Err(format!("{}", self.status))
        }
    }

    fn extra(&mut self, cmd: &str, _o: &Jobj) -> Option<String> {
        match cmd {
            // --- 画面の点検用(tools/ribbon_sweep.py が使う)---
            // いまのリボンの段と、押せるボタンの窓の中での場所。
            // **画素を見比べずに位置を検算する**ためにここから読む
            "ribbon" => {
                let boxes: Vec<String> = self
                    .btn_box
                    .borrow()
                    .iter()
                    .map(|(id, (x, y, w, h))| {
                        format!(
                            "{{\"id\":{},\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}",
                            J::S((*id).to_string()).to_json()
                        )
                    })
                    .collect();
                let (px, py, pw, ph) = self.pane_box.get();
                Some(format!(
                    "{{\"ok\":true,\"tab\":{},\"pane\":[{px},{py},{pw},{ph}],\"boxes\":[{}]}}",
                    self.tab,
                    boxes.join(",")
                ))
            }
            // いま何が開いているか。押した結果を**中身で**確かめる
            "ui_state" => {
                let pick_at = match self.pick.as_ref() {
                    Some((v, (x, y))) => format!("{{\"n\":{},\"x\":{x},\"y\":{y}}}", v.len()),
                    None => "null".into(),
                };
                let open: Vec<&str> = [
                    ("menu", self.menu_at.is_some()),
                    ("fmt_panel", self.fmt_panel.is_some()),
                    ("border_pal", self.border_pal.is_some()),
                    ("prompt", self.prompt.is_some()),
                    ("dv_dlg", self.dv_dlg.is_some()),
                    ("fn_dlg", self.fn_dlg.is_some()),
                    ("filter_panel", self.filter_panel.is_some()),
                    ("solver", self.solver.is_some()),
                    ("slicer", !self.slicers.is_empty()),
                    ("key_hint", self.key_hint.is_some()),
                    ("name_edit", self.name_edit.is_some()),
                    ("quit_ask", self.quit_ask),
                    ("shape_sel", self.shape_sel.is_some()),
                ]
                .iter()
                .filter(|(_, on)| *on)
                .map(|(k, _)| *k)
                .collect();
                // 切り替えの類は **open と分ける** — 混ぜると点検の道具が
                // 「開いたから Esc で閉じろ」と誤判定する
                let toggles = format!(
                    "[{},{},{},{},{},{},{},{}]",
                    self.show_formulas,
                    self.show_formula_bar,
                    self.show_zeros,
                    self.gridlines,
                    self.show_headers,
                    self.dark,
                    self.zoom,
                    self.ui_scale
                );
                Some(format!(
                    "{{\"ok\":true,\"tab\":{},\"right_open\":{},\"right_face\":{},\"left_open\":{},\"cur\":{},\"pick\":{},\"open\":{},\"toggles\":{toggles},\"status\":{},\"dirty\":{},\"edits\":{}}}",
                    self.tab,
                    // **パネルの姿も出す**(2026-08-19)。無いと、点検の道具は
                    // 右パネルが開いたかどうかを絵から当てるしかなく、
                    // 実際に外して別の物を開いた
                    self.right_open,
                    self.right_face,
                    self.left_open,
                    // いまのセル。**点検の道具が「押した所に当たったか」を
                    // 確かめられるようにする** — 当たっていない打鍵を
                    // 「効かない鍵」と数えた(2026-08-10)
                    J::S(self.cursor.a1()).to_json(),
                    pick_at,
                    J::A(open.iter().map(|s| J::S(s.to_string())).collect()).to_json(),
                    J::S(self.status.to_string()).to_json(),
                    self.dirty,
                    self.edits
                ))
            }
            _ => None,
        }
    }
}
