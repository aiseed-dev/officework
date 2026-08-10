//! **当たり判定とキーの動作。** クリック・検索置換・挿入・ショートカット。

use crate::*;

impl Writer {
    pub(crate) fn click_at(&mut self, rel_x: f32, rel_y: f32, extend: bool) {
        let pxmm = PX_PER_MM * self.zoom;
        // 紙は編集領域の (28,14)px に置いてあり、スクロールで上へずれている
        let x_mm = (rel_x - 28.0) / pxmm - self.pg.left_mm;
        let y_mm = (rel_y - 14.0) / pxmm + self.scroll_mm;

        // 表のセルの中なら、そのセルの編集に切り替える
        let hit_box = self.page.cell_boxes.iter().find(|b| {
            x_mm >= b.x_mm && x_mm <= b.x_mm + b.w_mm
                && y_mm >= b.top_mm && y_mm <= b.top_mm + b.h_mm
        }).copied();
        if let Some(b) = hit_box {
            let id = Target::Cell { table: b.table, row: b.row, col: b.col };
            self.switch_target(id);
            // セルの中の行で位置を決める
            let mut hit = 0usize;
            for line in &self.page.lines {
                if line.cell != Some((b.table, b.row, b.col)) {
                    continue;
                }
                if line.y_mm - LINE_MM * 0.8 > y_mm {
                    continue;
                }
                hit = line.byte0;
                let base = line.cells.iter().map(|c| c.off).min().unwrap_or(0);
                let mut x = line.cells.first().map(|c| c.x_mm - self.pg.left_mm).unwrap_or(0.0);
                for c in &line.cells {
                    if x_mm < x + c.w_mm / 2.0 {
                        break;
                    }
                    x += c.w_mm;
                    hit = line.byte0 + (c.off + c.ch.len_utf8()) - base;
                }
            }
            let hit = hit.min(self.ed.text().len());
            self.ed.move_to(hit, extend);
            return;
        }
        // 本文をクリックした。セルを編集していたら本文へ戻る
        self.switch_target(Target::Body);

        if self.page.vertical {
            // 縦書き: 列(x)で行を選び、字は y で選ぶ
            let vx = (rel_x - 28.0) / pxmm; // 紙の絶対 x(mm)
            let mut best: Option<(f32, usize)> = None;
            for (i, line) in self.page.lines.iter().enumerate() {
                if !line.from_body || line.cells.is_empty() {
                    continue;
                }
                let cx = self.page.vert_x.get(i).copied().unwrap_or(0.0)
                    + LINE_MM / 2.0;
                let top = line.y_mm;
                let bot = line.y_mm
                    + line.cells.last().map(|c| c.x_mm + c.w_mm).unwrap_or(0.0);
                let dx = (vx - cx).abs();
                let dy = if y_mm < top {
                    top - y_mm
                } else if y_mm > bot {
                    y_mm - bot
                } else {
                    0.0
                };
                let d = dx + dy * 0.5;
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, i));
                }
            }
            let Some((_, i)) = best else { return };
            let line = &self.page.lines[i];
            let mut byte = line.byte0;
            let base = line.cells.iter().map(|c| c.off).min().unwrap_or(0);
            for c in &line.cells {
                if y_mm < line.y_mm + c.x_mm + c.w_mm / 2.0 {
                    break;
                }
                byte = line.byte0 + (c.off + c.ch.len_utf8()) - base;
            }
            self.ed.move_to(byte.min(self.ed.text().len()), extend);
            return;
        }

        // 一番近いベースラインの本文行を選ぶ(クリックは字の少し上に落ちる)
        let target = y_mm + LINE_MM * 0.3;
        let mut best: Option<(f32, usize)> = None; // (距離, 本文行の通し番号)
        let mut nth = 0usize;
        for line in &self.page.lines {
            if !line.from_body {
                continue;
            }
            let d = (line.y_mm - target).abs();
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, nth));
            }
            nth += 1;
        }
        let Some((_, want)) = best else { return };

        // 行が持つバイト位置から出す(文字数で数え直さない)
        let mut byte = 0usize;
        let mut nth = 0usize;
        for line in &self.page.lines {
            if !line.from_body {
                continue;
            }
            if nth == want {
                byte = line.byte0;
                let base = line.cells.iter().map(|c| c.off).min().unwrap_or(0);
                let mut x = line.cells.first().map(|c| c.x_mm).unwrap_or(0.0);
                for c in &line.cells {
                    if x_mm < x + c.w_mm / 2.0 {
                        break;
                    }
                    x += c.w_mm;
                    byte = line.byte0 + (c.off + c.ch.len_utf8()) - base;
                }
                break;
            }
            nth += 1;
        }
        let byte = byte.min(self.ed.text().len());
        self.ed.move_to(byte, extend);
    }

    /// 次の一致を選ぶ(カーソルの後ろから。末尾まで無ければ頭から一周)。
    pub(crate) fn find_next(&mut self) {
        let term = self.find_ed.text().to_string();
        if term.is_empty() {
            self.status = ui::t!("検索語が空です").into();
            return;
        }
        let text = self.ed.text().to_string();
        let from = self.ed.selection().end;
        let hit = text[from..]
            .find(&term)
            .map(|i| from + i)
            .or_else(|| text.find(&term));
        match hit {
            Some(i) => {
                self.ed.move_to(i, false);
                self.ed.move_to(i + term.len(), true);
                self.status = "".into();
            }
            None => self.status = ui::tf!("「{}」は見つかりません", term).into(),
        }
    }

    /// いま選ばれている一致を置き換えて、次へ。
    pub(crate) fn replace_current(&mut self) {
        if self.protected() {
            self.status =
                ui::t!("読み取り専用で保護されています(保護タブの「保護」で解除できます)").into();
            return;
        }
        let term = self.find_ed.text().to_string();
        let repl = self.repl_ed.text().to_string();
        if term.is_empty() {
            return;
        }
        let sel = self.ed.selection();
        let selected: String = self.ed.text()[sel.clone()].to_string();
        if selected == term {
            self.ed.insert(&repl);
            self.dirty = true;
            self.relayout();
        }
        self.find_next();
    }

    /// 全部置き換える。**何件変えたかを言う**(黙って書き換えない)。
    pub(crate) fn replace_all(&mut self) {
        if self.protected() {
            self.status =
                ui::t!("読み取り専用で保護されています(保護タブの「保護」で解除できます)").into();
            return;
        }
        let term = self.find_ed.text().to_string();
        let repl = self.repl_ed.text().to_string();
        if term.is_empty() {
            return;
        }
        let mut n = 0usize;
        loop {
            let text = self.ed.text().to_string();
            let Some(i) = text.find(&term) else { break };
            self.ed.move_to(i, false);
            self.ed.move_to(i + term.len(), true);
            self.ed.insert(&repl);
            // **1置換ごとに本文へ写す。** まとめて写すと「1回の編集 = 1箇所」の
            // 前提から外れ、最初と最後の一致の間の書式が均されてしまう
            // (SEKKEI「writer の編集モデル」の注意をここで解いた)
            self.doc.set_body_text(self.ed.text(), SIZE_PT);
            n += 1;
            if n > 100_000 {
                break; // 置換後が検索語を含むと止まらなくなるのを防ぐ
            }
        }
        if n > 0 {
            self.dirty = true;
            self.relayout();
        }
        self.status = ui::tf!("{} 件を置き換えました", n).into();
    }

    /// run_cmd が処理できる id。**リボンの ready はこの表の中に限る**
    /// **押せる見た目のボタンが全部ここに載っているか**の契約。
    ///
    /// 読むのは `wiring_tests` と `tools/wiring_check.py` で、製品の道からは
    /// 呼ばれない — だから「使われていない」と言われるが、**消すと
    /// 「押しても何も起きないボタン」を止める仕掛けが消える**
    #[allow(dead_code)]
    pub(crate) const HANDLED: &'static [&'static str] = &[
        "open", "save", "undo", "redo", "selectall", "pdf",
        "bold", "italic", "underline", "strikeout", "fontcolor",
        "superscript", "subscript", "highlight", "clearstyle",
        "align-left", "align-center", "align-right", "align-just", "align-dist",
        "ruby", "direction",
        "controls", "form-text", "form-combo", "form-dropdown", "form-checkbox",
        "form-radio", "form-image", "form-email", "form-phone", "form-complex",
        "form-signature", "form-name",
        "colorschemas",
        "ai-where", "ai-summary", "ai-rewrite", "ai-polite", "ai-plain",
        "ai-translate", "ai-furigana", "ai-continue", "ai-table", "ai-ask",
        "ai-macro",
        "nav", "fit-page", "fit-width", "zoom100", "multipage",
        "show-toolbar", "show-statusbar", "show-left", "show-right",
        "incfont", "decfont", "markers", "numbering",
        "incoffset", "decoffset", "linespace", "pagebreak",
        "instable", "inssymbol", "replace", "changecase", "blankpage",
        "paracolor", "borders", "insimage",
        "spell", "wordcount", "zoom-in", "zoom-out", "hidenchars", "ruler",
        "fontname", "fontsize",
        "pageorient", "pagesize", "pagemargins",
        "edit-header", "edit-footer", "pagenum",
        "parastyle", "toc", "toc-update", "numpages", "datetime",
        "multilevels", "darkmode", "text-from-file", "add-text", "line-numbers",
        "insshape", "inssmartart", "inschart", "smartpicker", "instextart",
        "insequation", "instext", "pagecolor", "comment", "watermark", "bookmarks",
        "caption", "tof", "tof-update", "columns",
        "pen", "highlighter", "eraser", "track-changes", "dropcap", "hyphenation",
        "crossref", "co-addcomment", "co-delcomment", "co-showcomment",
        "prot-doc", "coauth-mode", "co-history", "co-chat",
        "plug-macros", "plug-manage", "prot-encrypt", "prot-sign",
        "copy", "cut", "paste",
    ];

    /// 画像を読んで、カーソルの段落の下に挿す。
    /// SVG(matplotlib の savefig("図.svg") など)は高精細の PNG に直して貼る。
    pub(crate) fn insert_image(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                let is_svg = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
                    || bytes.starts_with(b"<svg")
                    || bytes.starts_with(b"<?xml");
                let (bytes, pw, ph) = if is_svg {
                    match ui::svg_to_png(&bytes, 3.0) {
                        Ok((png, w, h)) => (png, w, h),
                        Err(e) => {
                            self.status = e.into();
                            return;
                        }
                    }
                } else {
                    let Some((pw, ph)) = image_px(&bytes) else {
                        self.status = ui::t!("PNG・JPEG・SVG だけ挿せます").into();
                        return;
                    };
                    (bytes, pw, ph)
                };
                // 96dpi 相当で置き、行長に収まらなければ比例で縮める
                let mut w_mm = pw as f32 * 25.4 / 96.0;
                let mut h_mm = ph as f32 * 25.4 / 96.0;
                let measure = self.pg.measure_mm();
                if w_mm > measure {
                    let k = measure / w_mm;
                    w_mm *= k;
                    h_mm *= k;
                }
                let im = kumihan::InlineImage {
                    bytes: std::sync::Arc::new(bytes),
                    w_mm,
                    h_mm,
                };
                // 選択があっても、挿すのはカーソルの段落だけ
                let cur = self.ed.cursor();
                self.ed.move_to(cur, false);
                self.para(|p| {
                    p.images.push(im.clone()); // 表示
                    p.images_new.push(im.clone()); // 保存
                });
                self.status = if is_svg {
                    ui::t!("SVG を高精細の画像にして挿しました(保存で docx に入ります)").into()
                } else {
                    ui::t!("画像を挿しました(段落の下に付き、保存で docx に入ります)").into()
                };
            }
            Err(e) => self.status = ui::tf!("読めません: {}", e).into(),
        }
    }

    /// テキスト(または docx の本文)をカーソルの位置に差し込む。
    pub(crate) fn insert_text_from(&mut self, path: &std::path::Path) {
        let is_docx = path.extension().and_then(|e| e.to_str()) == Some("docx");
        let text = if is_docx {
            match std::fs::File::open(path).map_err(|e| e.to_string()).and_then(ooxml::read) {
                Ok((d, rep)) => {
                    if !rep.is_lossless() {
                        // 本文だけを差し込む。落ちたもの(画像・表の外の要素)は言う
                        self.notes = rep
                            .unsupported
                            .iter()
                            .map(|(n, c)| SharedString::from(format!("{n} × {c}")))
                            .collect();
                    }
                    d.body_text()
                }
                Err(e) => {
                    self.status = ui::tf!("読めません: {}", e).into();
                    return;
                }
            }
        } else {
            match std::fs::read(path) {
                Ok(b) => match String::from_utf8(b) {
                    Ok(t) => t,
                    Err(_) => {
                        // 文字コードの推測はしない(化けた本文を黙って挿すより断る)
                        self.status = ui::t!("UTF-8 のテキストだけ読めます").into();
                        return;
                    }
                },
                Err(e) => {
                    self.status = ui::tf!("読めません: {}", e).into();
                    return;
                }
            }
        };
        if text.is_empty() {
            self.status = ui::t!("空のファイルです").into();
            return;
        }
        self.switch_target(Target::Body);
        handler::replace(self, None, &text);
        self.status = ui::tf!("{} を差し込みました({} 文字)", path.file_name().unwrap_or_default().to_string_lossy(), text.chars().count())
        .into();
    }

    /// 開くファイルを選ぶ(**ダイアログは別のスレッド**)。
    pub(crate) fn open_dialog(&mut self, cx: &mut Context<Self>) {
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .add_filter(ui::t!("Word文書とHTML"), &["docx", "html", "htm"])
                .pick_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = r {
                    this.open(p);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn backspace(&mut self, _: &ui::Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.editor().backspace();
        self.on_edited();
        cx.notify();
    }
    pub(crate) fn delete(&mut self, _: &ui::Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.editor().delete();
        self.on_edited();
        cx.notify();
    }
    pub(crate) fn left(&mut self, _: &ui::Left, _: &mut Window, cx: &mut Context<Self>) {
        self.editor().move_char(false, false);
        cx.notify();
    }
    pub(crate) fn right(&mut self, _: &ui::Right, _: &mut Window, cx: &mut Context<Self>) {
        self.editor().move_char(true, false);
        cx.notify();
    }
    pub(crate) fn select_left(&mut self, _: &ui::SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.editor().move_char(false, true);
        cx.notify();
    }
    pub(crate) fn select_right(&mut self, _: &ui::SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.editor().move_char(true, true);
        cx.notify();
    }
    pub(crate) fn select_all(&mut self, _: &ui::SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.editor().select_all();
        cx.notify();
    }
    pub(crate) fn word_left(&mut self, _: &ui::WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.word_move(false, false);
        cx.notify();
    }
    pub(crate) fn word_right(&mut self, _: &ui::WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.word_move(true, false);
        cx.notify();
    }
    pub(crate) fn select_word_left(&mut self, _: &ui::SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.word_move(false, true);
        cx.notify();
    }
    pub(crate) fn select_word_right(&mut self, _: &ui::SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.word_move(true, true);
        cx.notify();
    }
    pub(crate) fn a_context_menu(&mut self, _: &ui::ContextMenu, _: &mut Window, cx: &mut Context<Self>) {
        // キーボードから: キャレットのそばに出す
        let pxmm = PX_PER_MM * self.zoom;
        let (x, y, _) = self.caret_xy();
        self.menu_at = Some((
            28.0 + x * pxmm + 8.0,
            14.0 + (y - self.scroll_mm) * pxmm + 8.0,
        ));
        cx.notify();
    }

    pub(crate) fn a_cancel(&mut self, _: &ui::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        // 道具 → メニュー → 検索のパネル → ヘッダーのパネル → 一覧のパネル、の順で戻す
        if self.tool.take().is_some() {
            self.ink_cur = None;
            self.status = ui::t!("文字の編集に戻りました").into();
            cx.notify();
            return;
        }
        if self.menu_at.take().is_some() {
            cx.notify();
            return;
        }
        if self.pw_open {
            self.pw_open = false;
            self.pw_pending = None;
            self.status = "".into();
            cx.notify();
            return;
        }
        if self.tab == 0 {
            // ファイルのページ。欄 → ページの順で閉じる
            if self.file_field.take().is_some() {
                cx.notify();
                return;
            }
            self.tab = self.prev_tab;
            cx.notify();
            return;
        }
        if self.find_open {
            self.find_open = false;
            cx.notify();
            return;
        }
        if self.hf_edit.take().is_some() {
            self.status = "".into();
            cx.notify();
            return;
        }
        if self.cmt_edit {
            self.cmt_edit = false;
            self.status = "".into();
            cx.notify();
            return;
        }
        if self.wm_edit {
            self.wm_edit = false;
            self.status = "".into();
            cx.notify();
            return;
        }
        if self.bm_open {
            self.bm_open = false;
            self.status = "".into();
            cx.notify();
            return;
        }
        if self.xr_open {
            self.xr_open = false;
            self.status = "".into();
            cx.notify();
            return;
        }
        if self.url_open || self.fm_field.is_some() || self.fm_open || self.lk_open {
            if self.fm_field.take().is_none() {
                self.url_open = false;
                self.fm_open = false;
                self.lk_open = false;
            }
            self.status = "".into();
            cx.notify();
            return;
        }
        if self.quit_ask {
            self.quit_ask = false;
            self.status = ui::t!("終了をやめました").into();
            cx.notify();
            return;
        }
        if self.rb_open || self.sd_open || self.ai_open {
            self.rb_open = false;
            self.sd_open = false;
            self.sd_naming = false;
            self.ai_open = false;
            self.ai_macro = false;
            self.status = "".into();
            cx.notify();
            return;
        }
        if self.hist_open || self.chat_open || self.plug_open {
            self.hist_open = false;
            self.chat_open = false;
            self.plug_open = false;
            self.status = "".into();
            cx.notify();
            return;
        }
        if self.font_list || self.size_list || self.symbols || self.style_list {
            self.font_list = false;
            self.size_list = false;
            self.symbols = false;
            self.style_list = false;
            cx.notify();
        }
    }

    /// 文字飾りの割り当て(本家 Ctrl+B / I / U / 5)。リボンのボタンと同じ道。
    /// **calc と writer の両方に置く** — 片方だけだと「キーの嘘」になる
    /// Ctrl+P = 印刷(こちらは PDF に出す)。F11 = 全画面。
    /// Ctrl+Shift+S = 名前を付けて保存。**calc と両方に置く**
    pub(crate) fn do_print(&mut self, _: &ui::Print, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("pdf", cx);
        cx.notify();
    }
    pub(crate) fn do_fullscreen(&mut self, _: &ui::FullScreen, window: &mut Window, cx: &mut Context<Self>) {
        window.toggle_fullscreen();
        cx.notify();
    }
    pub(crate) fn do_save_as_key(&mut self, _: &ui::SaveAs, _: &mut Window, cx: &mut Context<Self>) {
        self.save_as(cx);
        cx.notify();
    }

    /// Ctrl+0 = 表示の倍率を等倍に戻す。**calc と両方に置く**
    pub(crate) fn do_zoom_reset(&mut self, _: &ui::ZoomReset, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom = 1.0;
        self.status = ui::t!("ズームを 100% に戻しました").into();
        cx.notify();
    }
    /// F1 = 手引きの在り処。**窓を開かない** — 別の窓を出すより、
    /// 読む物がどこにあるかを一行で言うほうが早い
    pub(crate) fn do_help(&mut self, _: &ui::Help, _: &mut Window, cx: &mut Context<Self>) {
        self.status = ui::t!(
            "手引き: docs/writer-manual.ja.md(英語は writer-manual.md)。Python は writer-macro-manual.ja.md"
        )
        .into();
        cx.notify();
    }
    /// Ctrl+; = 今日の日付、Ctrl+: = いまの時刻。**値として入れる** —
    /// 関数だと開き直すたびに変わり、日付印にならない
    pub(crate) fn do_ins_date(&mut self, _: &ui::InsDate, _: &mut Window, cx: &mut Context<Self>) {
        self.insert_stamp(false, cx);
    }
    pub(crate) fn do_ins_time(&mut self, _: &ui::InsTime, _: &mut Window, cx: &mut Context<Self>) {
        self.insert_stamp(true, cx);
    }
    pub(crate) fn insert_stamp(&mut self, time: bool, cx: &mut Context<Self>) {
        let stamp = ui::now_stamp();
        let Some((date, clock)) = stamp.split_once(' ') else {
            // 黙って空を入れない
            self.status = ui::t!("いまの時刻が取れませんでした").into();
            cx.notify();
            return;
        };
        let now = if time { clock } else { date };
        self.editor().insert(now);
        self.on_edited();
        self.status = ui::tf!("{} を入れました(値なので後で変わりません)", now).into();
        cx.notify();
    }

    pub(crate) fn do_bold(&mut self, _: &ui::Bold, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("bold", cx);
        cx.notify();
    }
    pub(crate) fn do_italic(&mut self, _: &ui::Italic, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("italic", cx);
        cx.notify();
    }
    pub(crate) fn do_underline(&mut self, _: &ui::Underline, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("underline", cx);
        cx.notify();
    }
    pub(crate) fn do_strikeout(&mut self, _: &ui::Strikeout, _: &mut Window, cx: &mut Context<Self>) {
        self.run_cmd("strikeout", cx);
        cx.notify();
    }
    pub(crate) fn do_find(&mut self, _: &ui::Find, _: &mut Window, cx: &mut Context<Self>) {
        if !self.find_open {
            self.run_cmd("replace", cx); // 検索と置換のパネルを開く
        }
        cx.notify();
    }
    pub(crate) fn doc_home(&mut self, _: &ui::DocHome, _: &mut Window, cx: &mut Context<Self>) {
        self.ed.move_to(0, false);
        self.follow_caret();
        cx.notify();
    }
    pub(crate) fn doc_end(&mut self, _: &ui::DocEnd, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.ed.text().len();
        self.ed.move_to(n, false);
        self.follow_caret();
        cx.notify();
    }
    /// Tab で段落を1段深く、Shift+Tab で1段浅く。
    /// リストではレベル(印も変わる)、普通の段落ではインデントとして効く。
    pub(crate) fn a_tab(&mut self, _: &ui::Tab, _: &mut Window, cx: &mut Context<Self>) {
        if self.find_open || self.hf_edit.is_some() {
            return; // パネルの中では使わない
        }
        self.para(|p| p.indent = (p.indent + 1).min(8));
        cx.notify();
    }
    pub(crate) fn a_shift_tab(&mut self, _: &ui::ShiftTab, _: &mut Window, cx: &mut Context<Self>) {
        if self.find_open || self.hf_edit.is_some() {
            return;
        }
        self.para(|p| p.indent = p.indent.saturating_sub(1));
        cx.notify();
    }

    pub(crate) fn page_up(&mut self, _: &ui::PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.page_move(false);
        cx.notify();
    }
    pub(crate) fn page_down(&mut self, _: &ui::PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.page_move(true);
        cx.notify();
    }
    pub(crate) fn up(&mut self, _: &ui::Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_line(false, false);
        cx.notify();
    }
    pub(crate) fn down(&mut self, _: &ui::Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_line(true, false);
        cx.notify();
    }
    pub(crate) fn select_up(&mut self, _: &ui::SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_line(false, true);
        cx.notify();
    }
    pub(crate) fn select_down(&mut self, _: &ui::SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_line(true, true);
        cx.notify();
    }
    pub(crate) fn home(&mut self, _: &ui::Home, _: &mut Window, cx: &mut Context<Self>) {
        self.editor().move_to(0, false);
        cx.notify();
    }
    pub(crate) fn end(&mut self, _: &ui::End, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.editor_ref().text().len();
        self.editor().move_to(n, false);
        cx.notify();
    }
    pub(crate) fn enter(&mut self, _: &ui::Enter, _: &mut Window, cx: &mut Context<Self>) {
        if self.pw_open {
            self.pw_commit();
            cx.notify();
            return;
        }
        if self.file_field.is_some() {
            self.commit_prop();
            cx.notify();
            return;
        }
        if self.find_open {
            self.find_next();
        } else if self.bm_open {
            self.bm_add();
        } else if self.quit_ask {
            // Enter = 保存して終了(いちばん安全な既定)
            self.quit_ask = false;
            self.save(true, cx);
        } else if self.chat_open {
            self.chat_send();
        } else if self.url_open {
            self.url_commit(cx);
        } else if self.fm_field.is_some() {
            self.fm_commit();
        } else if self.rb_open {
            self.rb_commit();
        } else if self.sd_open {
            self.sd_commit();
        } else if self.ai_open {
            let q = self.ai_ed.text().to_string();
            self.ai_open = false;
            let macro_mode = self.ai_macro;
            self.ai_macro = false;
            if !q.trim().is_empty() {
                let job = if macro_mode { AiJob::Macro(q) } else { AiJob::Ask(q) };
                self.ai_go(job, cx);
            }
        } else {
            self.editor().insert("\n");
            self.on_edited();
        }
        cx.notify();
    }
    pub(crate) fn copy(&mut self, _: &ui::Copy, _: &mut Window, cx: &mut Context<Self>) {
        // パネル(ヘッダー等)を編集中なら、そのパネルの選択が対象
        let e = self.editor_ref();
        let sel = e.selection();
        if sel.is_empty() {
            self.status = ui::t!("コピーする選択がありません").into();
        } else if let Some(s) = e.text().get(sel) {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(s.to_string()));
            self.status = ui::t!("コピーしました").into();
        }
        cx.notify();
    }
    pub(crate) fn cut(&mut self, _: &ui::Cut, _: &mut Window, cx: &mut Context<Self>) {
        let sel = self.editor_ref().selection();
        if sel.is_empty() {
            self.status = ui::t!("切り取る選択がありません").into();
        } else if let Some(s) = self.editor_ref().text().get(sel).map(str::to_string) {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(s));
            // 選択を空文字で置き換える = undo の1手で戻る
            self.editor().insert("");
            self.on_edited();
            self.status = ui::t!("切り取りました").into();
        }
        cx.notify();
    }
    pub(crate) fn paste(&mut self, _: &ui::Paste, _: &mut Window, cx: &mut Context<Self>) {
        match cx.read_from_clipboard().and_then(|i| i.text()) {
            Some(text) if !text.is_empty() => {
                // 通常の入力と同じ道(IME の未確定があれば確定してから)
                handler::replace(self, None, &text);
            }
            _ => self.status = ui::t!("貼り付けるものがありません").into(),
        }
        cx.notify();
    }
    pub(crate) fn undo(&mut self, _: &ui::Undo, _: &mut Window, cx: &mut Context<Self>) {
        // 道具(ペン)の間は筆の一手を戻す
        if self.tool.is_some() {
            if let Some(prev) = self.ink_undo.pop() {
                self.doc.ink = prev;
                self.dirty = true;
            }
            cx.notify();
            return;
        }
        // パネル(ヘッダー等)を編集中なら、そのパネルの一手を戻す
        if self.editor().undo() {
            self.on_edited();
        } else if let Some(prev) = self.doc_undo.take() {
            // マクロで置き換えた文書を、1手で元へ戻す
            self.target = Target::Body;
            self.pg = prev.page.unwrap_or_default();
            self.set_doc(prev);
            self.relayout_keep();
            self.dirty = true;
            self.status = ui::t!("マクロの前に戻しました").into();
        }
        cx.notify();
    }
    pub(crate) fn redo(&mut self, _: &ui::Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor().redo() {
            self.on_edited();
        }
        cx.notify();
    }
    pub(crate) fn do_save(&mut self, _: &ui::Save, _: &mut Window, cx: &mut Context<Self>) {
        self.save(false, cx);
        cx.notify();
    }
    /// 終了の要求。書きかけが無ければ即終了、あれば確認を**別のスレッド**で出す。
    /// 確認のダイアログでメインスレッドを塞がない — 塞ぐと画面ごと固まり、
    /// GNOME に「応答なし」と判定される(calc で踏んで直したのと同じ)。
    pub(crate) fn request_quit(&mut self, cx: &mut Context<Self>) {
        // 確認を出すのは**未保存の変更があるとき**。名前の無い新規でも、
        // 何か書いてあれば出す — 書いた物を黙って捨てない(発注者 2026-08-06。
        // calc と同じ改訂)。本当に空のままなら従来どおり黙って閉じる
        let empty_new = self.path.is_none() && self.ed.text().trim().is_empty();
        if !self.dirty || empty_new {
            self.release_lock();
            cx.quit();
            return;
        }
        // 確認は**窓の中のパネル**で出す。rfd の OS ダイアログは親窓を持てず
        // **スクリーンの中央**に出て、窓から離れすぎる(発注者 2026-08-06)
        self.quit_ask = true;
        cx.notify();
    }

    pub(crate) fn do_quit(&mut self, _: &ui::Quit, _: &mut Window, cx: &mut Context<Self>) {
        self.request_quit(cx);
    }

    pub(crate) fn do_open(&mut self, _: &ui::Open, _: &mut Window, cx: &mut Context<Self>) {
        self.open_dialog(cx);
        cx.notify();
    }
}
