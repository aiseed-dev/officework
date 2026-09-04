//! **文書とファイル。** 開く・保存・版・チャット・AI・記入欄。

use crate::*;

/// 素の文字として扱う拡張子。**マクロの .py は .py のまま往復する**
/// (発注者 2026-08-14「pyedit は使うな、writer を使え」)。
/// docx に化けさせない — 化けたら plugins から読めなくなる
/// コードを見せる等幅の書体。**入っていなければ font.rs の代替が効く**
/// (系統を保って選び直す)ので、ここは名指しでよい
pub(crate) const MONO: &str = "BIZ UDゴシック";

/// **ネイティブ文書の拡張子**(2026-08-16)。意味だけを持つ AsciiDoc。
/// `.adoc` が正、`.asciidoc` も受ける(AsciiDoc の世間の綴り)
pub(crate) fn is_native_ext(e: &str) -> bool {
    e.eq_ignore_ascii_case("adoc") || e.eq_ignore_ascii_case("asciidoc")
}

pub(crate) fn is_plain_ext(e: &str) -> bool {
    ["py", "txt", "md", "toml", "json", "csv"]
        .iter()
        .any(|k| e.eq_ignore_ascii_case(k))
}

/// **組みの姿** — 紙面を1回組むのに要るものだけを持つ小さな器。
///
/// `self` を借りずに組めるようにするためにある。発表(跨がない)は写しに
/// 改ページの印を足しながら**何度も組み直す**ので、その間 `self` の側は
/// 書ける形でなければならない(2026-08-17)。
pub(crate) struct Look {
    pub pg: kumihan::PageSetup,
    pub vertical: bool,
    pub group: kumihan::theme::Setting,
    pub view_w_px: f32,
}

impl Look {
    /// 1回ぶんの組み(合成済みの写し → 紙面)。**組みの本体はここ1箇所**。
    pub(crate) fn lay_once(&self, src: &Document, m: &Metrics) -> Page {
        // 段組みなら1段の行長で組み、ページの物理座標へ折る。
        // 折った後の座標は画面もクリックも PDF もそのまま使える
        let y0 = self.pg.top_mm + 4.0;
        let mut page;
        if self.vertical {
            // 縦書き: 行長 = 紙の縦の使い幅で組み、右からの列へ写す(K4)
            let measure = (self.pg.h_mm - self.pg.top_mm - self.pg.bottom_mm - 8.0).max(20.0);
            page = layout(
                src,
                m,
                &Frame { measure_mm: measure, line_height_mm: LINE_MM, y0_mm: y0 },
            );
            kumihan::fold_vertical(&mut page, &self.pg, y0, LINE_MM);
        } else {
            // **組み方の3値**(2026-08-16 の決め、2026-08-17 に通した)。
            // 横幅=可変 なら紙の幅ではなく窓の幅で組み、区切り=なし なら
            // ページに折らない(1本の流れ = Web の姿)。
            // 区切り=節(発表)は**合成の側**で見出し1 に改ページの印が
            // 付いているので、ここでは何もしない — 折り手がそこで割る
            let measure = if self.group.fluid {
                // 画面の画素 → mm(紙の幅は使わない)。**余白は紙と同じだけ
                // 左右に空ける** — 前は 16mm 決め打ちで、窓が広い機械では
                // 本文と表が紙からはみ出していた(2026-08-18 実機で見つけた)
                let margins = self.pg.left_mm + self.pg.right_mm;
                ((self.view_w_px / crate::PX_PER_MM) - margins).max(40.0)
            } else {
                self.pg.column_measure_mm()
            };
            page = layout(
                src,
                m,
                &Frame { measure_mm: measure, line_height_mm: LINE_MM, y0_mm: y0 },
            );
            if !self.group.endless() {
                kumihan::fold_columns(&mut page, &self.pg, y0);
            }
        }
        page
    }
}

impl Writer {
    pub fn new(path: Option<PathBuf>, cx: &mut Context<Self>) -> Writer {
        let mut w = Writer {
            focus: cx.focus_handle(),
            doc: Document::default(),
            docs: Vec::new(),
            doc_at: 0,
            open_request: None,
            open_dialog_request: false,
            // 器は settings.toml。書いていなければ入(綴りは "0" で切)。
            // **calc と同じ綴りを見ます** — 片方で切ったらもう片方でも切れる
            autocorrect: ui::settings::get("math_autocorrect")
                .map(|v| v != "0")
                .unwrap_or(true),
            find_file: false,
            embedded: false,
            files: Vec::new(),
            file_at: 0,
            ed: Editor::new(""),
            page: Page::default(),
            path: None,
            status: "".into(),
            notes: Vec::new(),
            dirty: false,
            drag_select: false,
            menu_at: None,
            tab: 1, // ファイルは全面ページなので、開きはホーム(calc と同じ)
            zoom: 1.0,
            scroll_mm: 0.0,
            caret_on: true,
            view_h_px: 800.0,
            target: Target::Body,
            show_marks: false,
            ruler: true,
            line_numbers: false,
            show_comments: true,
            open_list: None,
            // 次の起動も同じ明暗で(表と同じ器)
            dark: ui::dark_at_start(),
            ui_scale: ui::ui_scale(),
            // 既定は5分ごと(表と同じ)。試験は環境変数で縮める
            recover_secs: std::env::var("JO_RECOVER_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            recover_at: std::time::Instant::now(),
            image_cache: Default::default(),
            font_bytes: std::sync::Arc::new(font_data().to_vec()),
            pg: kumihan::PageSetup::default(),
            btn_box: Default::default(),
            ui_dump_last: Default::default(),
            rp_drawn: Default::default(),
            win_wh: Default::default(),
            fd_term: Editor::new(""),
            fd_glob: Editor::new(""),
            fd_dir: None,
            fd_field: 0,
            fd_hits: Vec::new(),
            fd_tally: Default::default(),
            fd_at: None,
            fd_peek: String::new(),
            fd_busy: false,
            fd_box: Default::default(),
            native: false,
            style_new: None,
            style_ed: Editor::new(""),
            tmpl: kumihan::theme::default_theme(),
            tmpl_path: None, // 同梱の既定を着ている
            find_open: false,
            find_field: 0,
            find_ed: Editor::new(""),
            repl_ed: Editor::new(""),
            hf_edit: None,
            hf_ed: Editor::new(""),
            pick_sel: 0,
            table_size: (3, 3),
            chosen_folder: None,
            fl_job: None,
            fl_ed: Editor::new(""),
            fl_tree: ui::tree::Tree::default(),
            fl_focus: false,
            tbl_open: false,
            tbl_ed: Editor::new(""),
            font_filter: None,
            cmt_name_edit: false,
            cmt_name_ed: Editor::new(""),
            cmt_edit: false,
            cmt_ed: Editor::new(""),
            cmt_para: 0,
            wm_edit: false,
            wm_ed: Editor::new(""),
            bm_open: false,
            bm_ed: Editor::new(""),
            hist_open: false,
            plug_open: false,
            hover_hint: None,
            view_w_px: 900.0,
            nav_open: false,
            nav_tab: 0,
            rp_tab: 0,
            rp_open: false,
            show_toolbar: true,
            show_statusbar: true,
            prev_tab: 1,
            file_view: 0,
            file_field: None,
            prop_ed: Editor::new(""),
            html_forms: Vec::new(),
            html_links: Vec::new(),
            html_origin: None,
            html_base: None,
            lk_open: false,
            fm_open: false,
            fm_field: None,
            fm_ed: Editor::new(""),
            url_open: false,
            url_ed: Editor::new(""),
            theme: 0,
            ai_open: false,
            ai_ed: Editor::new(""),
            ai_busy: false,
            ai_chat_log: Vec::new(),
            agent: None,
            agent_shown: 0,
            agent_state: AgentState::Idle,
            agent_calls: Vec::new(),
            agent_save: None,
            agent_picking: None,
            ai_chat_in: Editor::new(""),
            ai_chat_focus: false,
            ai_chat_plan: None,
            multipage: false,
            sd_open: false,
            sd_ed: Editor::new(""),
            sd_kind: kumihan::SdtKind::Text,
            sd_naming: false,
            ai_macro: false,
            quit_ask: false,
            rb_open: false,
            rb_ed: Editor::new(""),
            rb_range: 0..0,
            eq_open: false,
            eq_ed: Editor::new(""),
            encrypt_pw: None,
            ink_svg_count: 0,
            form_notes: Vec::new(),
            pw_open: false,
            pw_ed: Editor::new(""),
            pw_pending: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            typing_run: false,
            acted: false,
            chat_open: false,
            chat_ed: Editor::new(""),
            xr_open: false,
            tool: None,
            ink_cur: None,
            track: false,
            track_base: None,
            my_lock: None,
            locked_by: None,
            ink_undo: Vec::new(),
            page_offsets: vec![0.0],
            shape_cache: Default::default(),
            shape_sel: None,
            shape_pick: Vec::new(),
            list_cat: "basic_shapes",
            merge_op: 2,
            shape_drag: None,
            page_starts: vec![f32::NEG_INFINITY],
            page_notes: vec![Vec::new()],
            page_tops: vec![0.0],
            para_deco: Vec::new(),
            page_papers: Vec::new(),
            dress_hf: Default::default(),
            dress_page: (None, None),
            header_lines: Vec::new(),
            footer_lines: Vec::new(),
            font_name: kumihan::font::for_document(None)
                .map(|(f, _)| SharedString::from(f.name.clone()))
                .unwrap_or_else(|_| "sans-serif".into()),
            proof: Vec::new(),
            proof_msg: "".into(),
            checker: ui::check::Checker::default(),
        };
        match path {
            Some(p) => w.open(p),
            None => {
                // **新しい文書は adoc 形式で始めます**(2026-08-17 発注者
                // 「構造が不明確というのが docx の基本的な問題でしょう。
                // もう、adoc からはじめましょう」)。
                //
                // docx は本文と書式が混ざるので、後から機械で構造を拾い直す
                // ことになります。最初から分けて書けば、その作業が要りません。
                // 受け取った docx は今までどおり開けます。
                w.native = true;
                w.tmpl = kumihan::theme::default_theme();
                w.set_doc(Document::plain(
                    "ここに打てます。日本語入力(IME)もそのまま使えます。\n\
                     Ctrl+S で保存、Ctrl+O で開く。書式は名前を付けて使います。",
                ));
                w.dirty = false;
            }
        }
        // settings.toml の key.* に読めない行があれば、開いた時に言う
        // (calc と同じ — 黙って捨てない)
        if let Some(warn) = ui::key_warnings().first() {
            w.status = warn.clone().into();
        }
        // カーソルの点滅。530 ミリ秒は Windows の既定と同じ間隔です
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(530))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        this.caret_on = !this.caret_on;
                        cx.notify();
                    })
                    .is_err()
                {
                    break; // 窓が閉じた
                }
            }
        })
        .detach();
        w
    }

    pub(crate) fn set_doc(&mut self, doc: Document) {
        self.ed = Editor::new(&doc.body_text());
        self.doc = doc;
        self.relayout();
    }

    /// 編集中のテキストを文書に反映してから組み直す。
    /// いまの編集内容を、編集先(本文かセル)へ書き戻す。
    pub(crate) fn flush_target(&mut self) {
        match self.target {
            Target::Body => self.doc.set_body_text(self.ed.text()),
            Target::Cell { table, row, col } => {
                let text = self.ed.text().to_string();
                if let Some(kumihan::Block::Table(tb)) = self
                    .doc
                    .blocks
                    .iter_mut()
                    .filter(|b| matches!(b, kumihan::Block::Table(_)))
                    .nth(table)
                {
                    if let Some(cell) = tb.rows.get_mut(row).and_then(|r| r.get_mut(col)) {
                        set_cell_text(cell, &text);
                    }
                }
            }
        }
    }

    /// いま表の中にいるか。いるなら (表の番号, 行, 列, 行数, 列数)。
    ///
    /// 右パネルが**いる場所に追従する**ための入口(2026-08-15)。
    /// 表そのものの選択という状態は持たない — カーソルの居場所で決める
    pub(crate) fn cursor_table(&self) -> Option<(usize, usize, usize, usize, usize)> {
        let Target::Cell { table, row, col } = self.target else { return None };
        let t = self.doc.tables().nth(table)?;
        let rows = t.rows.len();
        let cols = t.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        Some((table, row, col, rows, cols))
    }

    /// 表の中の空のセル1つ(行や列を足すときの中身)
    fn empty_cell() -> kumihan::Cellbox {
        kumihan::Cellbox {
            paragraphs: vec![kumihan::Paragraph {
                runs: vec![kumihan::Run {
                    text: String::new(),
                    size_pt: None,
                    font: None,
                    fmt: Default::default(),
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// 行を足す(`below` なら下、でなければ上)。
    ///
    /// **表の操作はここが最初**(2026-08-15)。それまで writer には
    /// 3×3 を末尾に置く「表の挿入」しか無く、**行の足し方が無かった** —
    /// 帳票は必ず行が増えるので、これが無いと表は使い物にならない
    pub(crate) fn table_add_row(&mut self, below: bool) {
        let Some((ti, row, _, _, cols)) = self.cursor_table() else { return };
        self.checkpoint(false);
        self.flush_target();
        let at = if below { row + 1 } else { row };
        if let Some(tb) = self.table_mut(ti) {
            let cells: Vec<_> = (0..cols.max(1)).map(|_| Self::empty_cell()).collect();
            let at = at.min(tb.rows.len());
            tb.rows.insert(at, cells);
        }
        self.dirty = true;
        self.relayout_keep();
        self.status = if below {
            ui::t!("added_row_below").into()
        } else {
            ui::t!("added_row_above").into()
        };
    }

    /// 列を足す(`right` なら右、でなければ左)。列幅の表も一緒に伸ばす
    pub(crate) fn table_add_col(&mut self, right: bool) {
        let Some((ti, _, col, _, _)) = self.cursor_table() else { return };
        self.checkpoint(false);
        self.flush_target();
        let at = if right { col + 1 } else { col };
        if let Some(tb) = self.table_mut(ti) {
            for r in &mut tb.rows {
                let at = at.min(r.len());
                r.insert(at, Self::empty_cell());
            }
            // 列幅を持っている表なら、隣の幅を写して1本増やす
            if !tb.col_mm.is_empty() {
                let w = tb.col_mm.get(col.min(tb.col_mm.len() - 1)).copied().unwrap_or(20.0);
                let at = at.min(tb.col_mm.len());
                tb.col_mm.insert(at, w);
            }
        }
        self.dirty = true;
        self.relayout_keep();
        self.status = if right {
            ui::t!("added_column_right").into()
        } else {
            ui::t!("added_column_left").into()
        };
    }

    /// いまの行を消す。**最後の1行は消さない**(表が消えるのは別の操作)
    pub(crate) fn table_del_row(&mut self) {
        let Some((ti, row, _, rows, _)) = self.cursor_table() else { return };
        if rows <= 1 {
            self.status = ui::t!("last_row_cannot_deleted").into();
            return;
        }
        self.checkpoint(false);
        self.flush_target();
        if let Some(tb) = self.table_mut(ti) {
            if row < tb.rows.len() {
                tb.rows.remove(row);
            }
        }
        // カーソルは1つ上の行へ(消えた行に居続けない)。
        // **書き戻さずに移る** — 消した行はもう無い
        let next = row.saturating_sub(1);
        self.retarget_fresh(Target::Cell { table: ti, row: next, col: 0 });
        self.dirty = true;
        self.relayout_keep();
        self.status = ui::t!("row_deleted_ctrl_z").into();
    }

    /// いまの列を消す。**最後の1列は消さない**
    pub(crate) fn table_del_col(&mut self) {
        let Some((ti, row, col, _, cols)) = self.cursor_table() else { return };
        if cols <= 1 {
            self.status = ui::t!("last_column_cannot_deleted").into();
            return;
        }
        self.checkpoint(false);
        self.flush_target();
        if let Some(tb) = self.table_mut(ti) {
            for r in &mut tb.rows {
                if col < r.len() {
                    r.remove(col);
                }
            }
            if col < tb.col_mm.len() {
                tb.col_mm.remove(col);
            }
        }
        let next = col.saturating_sub(1);
        self.retarget_fresh(Target::Cell { table: ti, row, col: next });
        self.dirty = true;
        self.relayout_keep();
        self.status = ui::t!("column_deleted_ctrl_z").into();
    }

    /// いまの段落の画像を拡げる・縮める(縦横の比は保つ)。
    ///
    /// **数式も画像なので同じ道で効く** — 式の絵だけ大きくしたい、は
    /// 普通の頼み。下限 5mm・上限 400mm(紙より大きくしない)
    pub(crate) fn image_scale(&mut self, k: f32) {
        let (pi, _) = self.cursor_para();
        self.checkpoint(false);
        self.flush_target();
        let mut touched = 0usize;
        // 段落は流れ(blocks)の中に表と混ざっている。段落だけを数えて引く
        if let Some(p) = self
            .doc
            .blocks
            .iter_mut()
            .filter_map(|b| match b {
                kumihan::Block::Para(p) => Some(p),
                _ => None,
            })
            .nth(pi)
        {
            for im in p.images.iter_mut().chain(p.images_new.iter_mut()) {
                im.w_mm = (im.w_mm * k).clamp(5.0, 400.0);
                im.h_mm = (im.h_mm * k).clamp(5.0, 400.0);
                touched += 1;
            }
        }
        if touched == 0 {
            self.status = ui::t!("there_no_picture_paragraph").into();
            return;
        }
        self.dirty = true;
        self.relayout_keep();
        self.status = if k < 1.0 {
            ui::t!("made_picture_smaller").into()
        } else {
            ui::t!("made_picture_bigger").into()
        };
    }

    /// **書き戻さずに**編集先を移す。消した行・列の後始末に使う。
    ///
    /// `switch_target` は移る前に今の文章を書き戻すが、**もう無い場所へ
    /// 書き戻すと事故になる**。2026-08-15 実機で踏んだ: 行を消したあと
    /// `target` を Body に置いてから switch_target を呼んだら、手元に
    /// 残っていたセルの字が**本文の1段落目を潰した**(「表のある文書」が
    /// 「2-2」になった)。消したあとは書き戻す先が無い — 読み直すだけ
    fn retarget_fresh(&mut self, next: Target) {
        self.target = next;
        let text = match next {
            Target::Body => self.doc.body_text(),
            Target::Cell { table, row, col } => self
                .doc
                .tables()
                .nth(table)
                .and_then(|t| t.rows.get(row))
                .and_then(|r| r.get(col))
                .map(cell_text)
                .unwrap_or_default(),
        };
        self.ed = Editor::new(&text);
    }

    /// 表を番号で引く(本文の流れの中の何番目の表か)
    fn table_mut(&mut self, i: usize) -> Option<&mut kumihan::Table> {
        self.doc
            .blocks
            .iter_mut()
            .filter_map(|b| match b {
                kumihan::Block::Table(t) => Some(t),
                _ => None,
            })
            .nth(i)
    }

    /// 編集先を切り替える。いまの内容を書き戻してから、次の文章を持つ。
    pub(crate) fn switch_target(&mut self, next: Target) {
        if self.target == next {
            return;
        }
        self.flush_target();
        self.target = next;
        let text = match next {
            Target::Body => self.doc.body_text(),
            Target::Cell { table, row, col } => self
                .doc
                .tables()
                .nth(table)
                .and_then(|t| t.rows.get(row))
                .and_then(|r| r.get(col))
                .map(cell_text)
                .unwrap_or_default(),
        };
        self.ed = Editor::new(&text);
        self.status = match next {
            Target::Body => ui::t!("body").into(),
            Target::Cell { row, col, .. } => {
                ui::tf!("editing_table_cell_row", row + 1, col + 1).into()
            }
        };
    }

    pub(crate) fn relayout(&mut self) {
        self.flush_target();
        self.lay();
    }

    /// **組み直しの本体。** 合成も組み方の3値もここにしかありません。
    ///
    /// [`relayout`](Self::relayout) との違いは、編集中の字を本文へ戻すかどうか
    /// だけです。前は組み直しが2箇所にあり、片方(`relayout_keep`)が
    /// **テンプレートを合成していませんでした** — 書式を触った直後や、数式を
    /// 組んだ直後だけ、色や書体が消えた紙面になっていました(2026-08-18 に
    /// 見本を実機で見て気づきました)。
    pub(crate) fn lay(&mut self) {
        // 字の寸法は **Arc の写しの上**で持つ — 組みの本体(lay_once)は
        // self を書くので、self を借りたままにできない
        let fb = self.font_bytes.clone();
        let m = Metrics::new(&fb).expect("フォント");
        // **画面は常に「本文×テンプレート」の合成**(2026-08-16)。
        // 合成は写しの上で行い、`self.doc`(意味の正本)は触らない —
        // 保存されるのは意味だけ、が守られる。互換の文書は素通し
        //
        // 発表(跨がない)のときだけ、写しに改ページの印を足しながら
        // 何度か組み直すので、写しは**書ける形**で持つ
        let mut composed = self.native.then(|| kumihan::theme::compose(&self.doc, &self.tmpl));
        // **様式(セル)は写しの側で組みます**(2026-08-18)。本文は
        // `項目:: 値` のまま残るので、保存してもセルは本文に漏れません。
        // 対応の付かない項目と埋まらないセルは、ここで受け取って状態行に出します
        self.form_notes = match composed.as_mut() {
            Some(c) => kumihan::theme::apply_forms(c, &self.tmpl),
            None => Vec::new(),
        };
        // **表の式を計算して、見せる字にします**(2026-08-19)。
        // 写しの上だけで置き換えるので、保存されるのは `=SUM(…)` の式のまま
        // です(式が正本)。式が1つも無ければ写しも作りません
        if ops::table::has_formula(composed.as_ref().unwrap_or(&self.doc)) {
            let c = composed.get_or_insert_with(|| self.doc.clone());
            ops::table::fill_with(c, ui::calc_iter_setting());
        }
        let group = if self.native { self.tmpl.setting } else { Default::default() };
        // **ページの飾りは合成の写しから取ります**(2026-08-18)。
        // テンプレートに書いたヘッダー・透かし・縦書きが画面と紙に出ます。
        // `self.doc` は意味だけのまま(保存に漏れない)
        let deco = composed.as_ref().unwrap_or(&self.doc);
        // 段落の背景色と囲みは**合成後**の段落から控える(画面の帯の元)
        self.para_deco = {
            let mut v = Vec::new();
            let mut at = 0usize;
            for p in deco.paragraphs() {
                let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
                if p.shade.is_some() || p.boxed {
                    v.push((at..at + len, p.shade.clone(), p.boxed));
                }
                at += len + 1;
            }
            v
        };
        self.dress_hf = (deco.header.clone(), deco.footer.clone());
        self.dress_page = (deco.watermark.clone(), deco.page_color.clone());
        let vertical = deco.vertical;
        let snapshot = Look { pg: self.pg, vertical, group, view_w_px: self.view_w_px };
        self.page = snapshot.lay_once(composed.as_ref().unwrap_or(&self.doc), &m);
        self.refresh_hf();
        // **跨がない**(発表)。折った結果を見て、境をまたいだ段落があれば
        // 写しにその段落の改ページの印を足し、**折り手に折り直させる**。
        // refresh_hf の後でないと頁の境が分からない
        if group.keep {
            if let Some(c) = composed.as_mut() {
                self.keep_paragraphs_whole(c, &m, &snapshot);
            }
        }
    }

    /// **段落が枚を跨がないように送る**(発表の組み方)。
    ///
    /// 折ってみて、境をまたいだ段落があれば**写しのその段落に改ページの印を
    /// 付けて組み直す**。自分で頁の高さを数えないのは、本物の折り手
    /// (脚注が本文の底を上げる)と食い違わないため。1回につき1段落なので、
    /// 段落の数で必ず止まる。
    ///
    /// **印を付けて組み直す**のがこの手の要。組んだ後の `breaks` に境だけ
    /// 足しても、行は巻物の位置のまま動かない(2026-08-17 の踏み跡)。
    fn keep_paragraphs_whole(&mut self, c: &mut Document, m: &Metrics, snapshot: &Look) {
        let n = c.paragraphs().count();
        for _ in 0..n.min(200) {
            let Some(i) = self.straddling_para(c) else { return };
            let Some(p) = c.paragraphs_mut().nth(i) else { return };
            if p.page_break_before {
                // 既に改めている所で跨いでいる = その段落だけで1枚に
                // 収まらない。送っても直らないので諦める
                return;
            }
            p.page_break_before = true;
            self.page = snapshot.lay_once(c, m);
            self.refresh_hf();
        }
    }

    /// 頁の境を跨いでいる最初の段落(番号)。無ければ None。
    fn straddling_para(&self, c: &Document) -> Option<usize> {
        if self.page_offsets.len() <= 1 {
            return None;
        }
        // 段落の頭のバイト位置(本文の流れ)
        let mut at = 0usize;
        let starts: Vec<usize> = c
            .paragraphs()
            .map(|p| {
                let s = at;
                at += p.runs.iter().map(|r| r.text.len()).sum::<usize>() + 1;
                s
            })
            .collect();
        for (i, &s) in starts.iter().enumerate() {
            let e = starts.get(i + 1).copied().unwrap_or(usize::MAX);
            let mut pages: Vec<usize> = Vec::new();
            for line in &self.page.lines {
                if !line.from_body || line.byte0 < s || line.byte0 >= e {
                    continue;
                }
                let (pg, _) = self.page_of_roll(line.y_mm);
                if !pages.contains(&pg) {
                    pages.push(pg);
                }
            }
            if pages.len() > 1 {
                return Some(i);
            }
        }
        None
    }

    /// いまの紙面の総頁(紙と同じ折り方で数える)。
    pub(crate) fn total_pages(&self) -> usize {
        self.page_offsets.len().max(1)
    }

    /// 巻物の y → (ページ, ページの中の y)。筆はページに固定する。
    ///
    /// **枚は `page_starts`(その枚の最初の行)で決め、枚の中の位置は
    /// `page_offsets`(紙の上端)から測る。** 2つの物差しが要るのは、
    /// 巻物が空きを詰めて流れるため — 詳しくは `page_starts` の註。
    pub(crate) fn page_of_roll(&self, y: f32) -> (usize, f32) {
        // **紙を積んだ表示では、行の y は積んだ後の座標です**(`fold_print` が
        // 各頁の中身を `page_tops[k]` から置き直す)。だから頁は紙の上端で
        // 引き、頁の中の位置も紙の上端から測ります。2026-09-02 に紙を積むのが
        // 普通の表示になって、発表の試験でページの割り当てがずれて見つけた穴
        // (前の「印刷レイアウト」でも同じでした)
        if self.sheets() && self.page_tops.len() > 1 && self.page_tops.len() == self.page_offsets.len() {
            let p = self.page_tops.iter().rposition(|t| y >= *t - 0.01).unwrap_or(0);
            return (p, y - self.page_tops[p]);
        }
        let p = self.page_starts.iter().rposition(|s| y >= *s - 0.01).unwrap_or(0);
        (p, y - self.page_offsets.get(p).copied().unwrap_or(0.0))
    }

    // ---- 描画(ペン・蛍光ペン・消しゴム) ----

    pub(crate) fn ink_begin(&mut self, x: f32, y_roll: f32) {
        let Some(tool) = self.tool else { return };
        if tool == 2 {
            self.ink_erase(x, y_roll);
            return;
        }
        let (page, y) = self.page_of_roll(y_roll);
        self.ink_cur = Some(kumihan::Stroke {
            page,
            highlighter: tool == 1,
            points: vec![(x, y)],
        });
    }

    pub(crate) fn ink_move(&mut self, x: f32, y_roll: f32) {
        if self.tool == Some(2) {
            self.ink_erase(x, y_roll);
            return;
        }
        let oy = self
            .ink_cur
            .as_ref()
            .and_then(|st| self.page_offsets.get(st.page))
            .copied()
            .unwrap_or(0.0);
        let Some(st) = self.ink_cur.as_mut() else { return };
        let y = y_roll - oy;
        if let Some((lx, ly)) = st.points.last() {
            if (x - lx).abs() + (y - ly).abs() < 0.4 {
                return; // 細かすぎる点は間引く
            }
        }
        st.points.push((x, y));
    }

    pub(crate) fn ink_end(&mut self) {
        if let Some(st) = self.ink_cur.take() {
            if st.points.len() >= 2 {
                self.key_checkpoint(false); // 1筆 = Ctrl+Z の1手
                self.ink_undo.push(self.doc.ink.clone());
                self.doc.ink.push(st);
                self.dirty = true;
            }
        }
    }

    /// 消しゴム。なぞった近く(3mm)に点を持つ筆を丸ごと消す。
    pub(crate) fn ink_erase(&mut self, x: f32, y_roll: f32) {
        let (page, y) = self.page_of_roll(y_roll);
        let near = |st: &kumihan::Stroke| {
            st.page == page
                && st.points.iter().any(|(sx, sy)| (sx - x).abs() < 3.0 && (sy - y).abs() < 3.0)
        };
        if self.doc.ink.iter().any(near) {
            self.key_checkpoint(false); // 消しゴムの1回 = Ctrl+Z の1手
            self.ink_undo.push(self.doc.ink.clone());
            self.doc.ink.retain(|st| !near(st));
            self.dirty = true;
        }
    }

    /// 保存用の写し。筆(ペン)を、そのページに載っている段落の控えへ
    /// 図形(自由曲線)として差し込む。モデル本体は触らない —
    /// 保存のたびに増えないように、写しに差す。
    /// **その高さ(巻物の座標、mm)に居る段落**の通し番号。
    /// 筆の線を本文のどこに結びつけるかを決めるのに使います
    fn para_at_y(&self, y_mm: f32) -> usize {
        let mut starts: Vec<usize> = Vec::new();
        let mut at = 0usize;
        for p in self.doc.paragraphs() {
            starts.push(at);
            at += p.runs.iter().map(|r| r.text.len()).sum::<usize>() + 1;
        }
        let Some(l) = self
            .page
            .lines
            .iter()
            .rfind(|l| l.from_body && l.y_mm <= y_mm)
        else {
            return 0;
        };
        starts.iter().rposition(|s| *s <= l.byte0).unwrap_or(0)
    }

    /// **ページ → そのページに最初に載る段落**の対応。返りは
    /// (ページ番号(0始まり) → 段落の通し番号, 段落の通し番号 → ブロックの番号)。
    ///
    /// 筆(手描きの線)はページの座標で持っているので、本文のどこに
    /// 結びつけるかを決めるのにこれが要ります。docx の図形と adoc の画像で
    /// 同じ答えを使います
    fn page_head_paras(
        &self,
        doc: &Document,
    ) -> (std::collections::BTreeMap<usize, usize>, Vec<usize>) {
        let (pages, _) = paper::paginate(&self.page, paper::Paper::from_page(&self.pg));
        let mut starts: Vec<usize> = Vec::new();
        let mut at = 0usize;
        for p in doc.paragraphs() {
            starts.push(at);
            at += p.runs.iter().map(|r| r.text.len()).sum::<usize>() + 1;
        }
        let mut page_para: std::collections::BTreeMap<usize, usize> = Default::default();
        for (l, pg) in self.page.lines.iter().zip(&pages) {
            if !l.from_body {
                continue;
            }
            let pi = starts.iter().rposition(|s| *s <= l.byte0).unwrap_or(0);
            page_para.entry(pg - 1).or_insert(pi);
        }
        let para_block_idx: Vec<usize> = doc
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| matches!(b, kumihan::Block::Para(_)))
            .map(|(i, _)| i)
            .collect();
        (page_para, para_block_idx)
    }

    /// `tmpl` を渡すと、そのテンプレートの**ページの飾り**(用紙・ヘッダー・
    /// フッター・透かし・ページの色・縦書き)を写しに入れます。段落の書式は
    /// 入れません — そちらは `styles.xml` が持ちます
    /// (`ooxml::write_with_theme` と対になっています)。
    pub(crate) fn doc_for_save(&self, tmpl: Option<&kumihan::theme::Theme>) -> Document {
        let mut doc = self.doc.clone();
        if let Some(th) = tmpl {
            kumihan::theme::compose_page(&mut doc, th);
            // **様式(セル)は docx にも出します。** 画面と同じ表になります
            kumihan::theme::apply_forms(&mut doc, th);
        }
        // 相互参照は保存の写しで計算し直す(docx のキャッシュを新しく保つ。
        // 画面の平文はそのまま — 見えている値の更新は「参照を更新」で)
        doc.refresh_fields(|name, page| self.ref_value(name, page));
        // **表の式は docx へ値で焼く**(2026-08-20 発注者。SEKKEI「3段目」)。
        //
        // いままで docx には `=SUM(B2:B4)` の字がそのまま出ていました。
        // Word で開いた相手には**答えでなく式が見えます** — 画面・HTML・紙は
        // 写しの値を見せているので、docx だけが素通しでずれていました。
        //
        // **`.adoc` の正本は式のまま**です(この写しの上でだけ焼く)。
        // docx は受け渡しの形式で、往復の正本ではありません — docx にした物を
        // 開き直しても式は戻りません(値になっています)。
        // 式が無ければ写しも作りません(いまの組みと同じ倹約)
        if ops::table::has_formula(&doc) {
            ops::table::fill_with(&mut doc, ui::calc_iter_setting());
        }
        // 変更履歴: 記録開始時点との差分を印の字にする(ooxml が w:ins/w:del に)
        if self.track {
            if let Some(base) = &self.track_base {
                use kumihan::{TRK_DEL_E, TRK_DEL_S, TRK_INS_E, TRK_INS_S};
                let cur: Vec<String> = doc.paragraphs().map(para_text).collect();
                let (marks, deleted) = track_diff(base, &cur);
                doc.track_author =
                    Some(std::env::var("USER").unwrap_or_else(|_| "writer".into()));
                let mut pi = 0usize;
                for b in &mut doc.blocks {
                    let kumihan::Block::Para(p) = b else { continue };
                    let mark = marks.get(pi).copied().unwrap_or(PMark::Same);
                    match mark {
                        PMark::Same => {}
                        PMark::New => {
                            let t = para_text(p);
                            let (pt, font, fmt) = p.runs.first()
                                .map(|r| (r.size_pt, r.font.clone(), r.fmt.clone()))
                                .unwrap_or((None, None, Default::default()));
                            p.runs = vec![kumihan::Run {
                                text: format!("{TRK_INS_S}{t}{TRK_INS_E}"),
                                size_pt: pt, font, fmt,
                            }];
                        }
                        PMark::Changed(bi) => {
                            let t = para_text(p);
                            let (pre, del, ins, suf) = split_diff(&base[bi], &t);
                            let (pt, font, fmt) = p.runs.first()
                                .map(|r| (r.size_pt, r.font.clone(), r.fmt.clone()))
                                .unwrap_or((None, None, Default::default()));
                            let mut text = pre;
                            if !del.is_empty() {
                                text.push(TRK_DEL_S);
                                text.push_str(&del);
                                text.push(TRK_DEL_E);
                            }
                            if !ins.is_empty() {
                                text.push(TRK_INS_S);
                                text.push_str(&ins);
                                text.push(TRK_INS_E);
                            }
                            text.push_str(&suf);
                            p.runs = vec![kumihan::Run { text, size_pt: pt, font, fmt }];
                        }
                    }
                    pi += 1;
                }
                // 消えた段落は、その場所に「全部削除」の段落として置く
                let pbi: Vec<usize> = doc.blocks.iter().enumerate()
                    .filter(|(_, b)| matches!(b, kumihan::Block::Para(_)))
                    .map(|(i, _)| i)
                    .collect();
                let mut dels = deleted.clone();
                dels.sort_by_key(|(at, _)| *at);
                for (at, bi) in dels.into_iter().rev() {
                    let pos = pbi.get(at).copied().unwrap_or(doc.blocks.len());
                    doc.blocks.insert(pos, kumihan::Block::Para(kumihan::Paragraph {
                        line_spacing: 1.0,
                        runs: vec![kumihan::Run {
                            text: format!("{TRK_DEL_S}{}{TRK_DEL_E}", base[bi]),
                            size_pt: None,
                            font: None,
                            fmt: Default::default(),
                        }],
                        ..Default::default()
                    }));
                }
            }
        }
        if doc.ink.is_empty() {
            return doc;
        }
        let (page_para, para_block_idx) = self.page_head_paras(&doc);
        let ink = std::mem::take(&mut doc.ink);
        for (i, st) in ink.iter().enumerate() {
            let pi = page_para.get(&st.page).copied().unwrap_or(0);
            let Some(bi) = para_block_idx.get(pi).copied() else { continue };
            if let Some(kumihan::Block::Para(p)) = doc.blocks.get_mut(bi) {
                p.anchors.push(ooxml::ink_anchor_run(st, 9001 + i));
            }
        }
        doc
    }

    /// 紙面に出すヘッダー・フッターの行を組み直す(番号は1ページ目のもの。
    /// 各ページの本当の番号は PDF で入る)。
    pub(crate) fn refresh_hf(&mut self) {
        let m = Metrics::new(&self.font_bytes).expect("フォント");
        // **区切りなし(Web の組み方)は頁に数えない。** 組み手が折らないのに
        // 数え手だけ折ると、1本のはずの流れが「3ページ」と言われる
        // (2026-08-17 に踏んだ)
        if self.native && self.tmpl.setting.endless() {
            self.page_offsets = vec![0.0];
            self.page_starts = vec![f32::NEG_INFINITY];
            self.page_notes.clear();
            self.page_papers.clear();
            self.page_tops.clear();
            return;
        }
        // **紙と同じ折り方を、同じ関数から受け取る。** 脚注はその頁の
        // 本文の底を上げるので、別に数えると画面と PDF がずれる
        let pn = paper::paginate_full(&self.page, paper::Paper::from_page(&self.pg));
        self.page_offsets = pn.offsets;
        self.page_starts = pn.starts;
        self.page_notes = pn.notes;
        self.page_papers = pn.papers;
        // **紙を1枚ずつ積む**(2026-09-02 からこれが普通の表示。前は
        // 「印刷レイアウト」の切り替えでした)。折らないと紙の絵と中身が
        // 重なる(頁の間隔は紙の高さより詰まっているため)。節で紙が変わる
        // 文書は、この形で紙の大きさの違いが出ます
        self.page_tops = if self.sheets() {
            let offs = self.page_offsets.clone();
            let sts = self.page_starts.clone();
            let papers: Vec<kumihan::PageSetup> = self.page_papers.iter()
                .map(|q| kumihan::PageSetup {
                    w_mm: q.width_mm, h_mm: q.height_mm,
                    left_mm: q.margin_mm, right_mm: q.margin_mm,
                    top_mm: self.pg.top_mm, bottom_mm: self.pg.bottom_mm,
                    columns: self.pg.columns,
                })
                .collect();
            kumihan::fold_print(&mut self.page, &papers, &offs, &sts, PAGE_GAP_MM)
        } else {
            vec![0.0]
        };
        // 複数ページ(見開き)。**画面だけ**の折り方 — PDF は 1ページずつ
        // (save_pdf は組み直してから写す)。縦書きとは併せない
        if self.multipage && !self.page.vertical {
            let offs = self.page_offsets.clone();
            let sts = self.page_starts.clone();
            kumihan::fold_pages(&mut self.page, &self.pg, &offs, &sts, 2, PAGE_GAP_MM);
        }
        let total = self.total_pages();
        // 飾りは合成の写しから(テンプレートのヘッダーもここに入っている)
        self.header_lines =
            kumihan::layout_hf(&self.dress_hf.0, &m, &self.pg, LINE_MM, 1, total, false,
                               self.doc.base_pt());
        self.footer_lines =
            kumihan::layout_hf(&self.dress_hf.1, &m, &self.pg, LINE_MM, 1, total, true,
                               self.doc.base_pt());
    }

    /// ヘッダー・フッターの編集のパネルを開く(もう一度で閉じる)。
    pub(crate) fn open_hf(&mut self, footer: bool) {
        if self.hf_edit == Some(footer) {
            self.hf_edit = None;
            return;
        }
        let hf = if footer { &self.doc.footer } else { &self.doc.header };
        let which = if footer { ui::t!("footer") } else { ui::t!("header") };
        if hf.paragraphs.is_empty() && hf.part.is_some() {
            // 読めたが持てなかった部品(表入りなど)。嘘の編集をさせない
            self.status = ui::tf!("contains_table_version_cant", which).into();
            return;
        }
        self.find_open = false;
        self.hf_edit = Some(footer);
        self.hf_ed = Editor::new(&kumihan::paras_text(&hf.paragraphs));
        self.status = ui::tf!("editing_shared_all_pages", which).into();
    }

    /// 文書の書体を実体に結ぶ。無ければ系統を保って代替し、**そう言う**。
    pub(crate) fn adopt_font(&mut self) {
        // **文書が何も言っていなければ、テンプレートの書体を使います**
        // (2026-08-26)。前は文書しか見ていなかったので、フォルダの
        // テンプレートに書体を書いても画面は変わりませんでした。
        //
        // ここで `self.doc.font` に写さないのが大事です。写すと、文書が
        // 自分で言っていないことを言い出して、保存したときに書体が本文へ
        // 焼き付いてしまいます。
        let wanted = self.doc.font.clone().or_else(|| self.tmpl.font.clone());
        match kumihan::font::for_document(wanted.as_deref()) {
            Ok((fam, exact)) => {
                if let Ok(b) = kumihan::font::load(fam) {
                    self.font_bytes = std::sync::Arc::new(b);
                    self.font_name = SharedString::from(fam.name.clone());
                }
                if !exact {
                    if let Some(w) = &wanted {
                        // **下の帯に出します**(2026-08-26 発注者「メッセージで
                        // ダイアログを出すな。下にパネルに表示でいいのでは」)。
                        // 前は紙の右上に浮く小窓で、閉じる道がありませんでした
                        self.notes.push(
                            ui::tf!("font_missing_showing", w, fam.name).into(),
                        );
                        self.status =
                            ui::tf!("font_not_installed_used", w, fam.name)
                                .into();
                    }
                }
            }
            Err(e) => self.status = e.into(),
        }
    }

    /// パスワードのパネルの Enter。開き待ちがあれば解いて開き、
    /// 無ければ「次の保存から暗号化」を決める(空なら解除)
    pub(crate) fn pw_commit(&mut self) {
        let pw = self.pw_ed.text().to_string();
        if let Some(p) = self.pw_pending.take() {
            let bytes = match std::fs::read(&p) {
                Ok(b) => b,
                Err(e) => {
                    self.pw_open = false;
                    self.status = ui::tf!("cant_open", e).into();
                    return;
                }
            };
            match ooxml::crypt::decrypt(&bytes, &pw) {
                Ok(plain) => {
                    self.pw_open = false;
                    self.open_plain(p.clone(), plain);
                    if self.path.as_deref() == Some(p.as_path()) {
                        self.encrypt_pw = Some(pw);
                        self.status = ui::tf!("saving_keeps_same_password", self.status)
                        .into();
                    }
                }
                Err(e) => {
                    // パネルは開いたまま。打ち直せる
                    self.pw_pending = Some(p);
                    self.pw_ed = Editor::new("");
                    self.status = e.into();
                }
            }
        } else {
            // **こちらから暗号化を掛ける道はありません**(2026-08-18)。
            // このパネルが開くのは、暗号化された docx を開くときだけです
            self.pw_open = false;
        }
    }

    /// 原本の中身(暗号化されていれば解いた平文)。部品の持ち越しに使う
    pub(crate) fn original_plain(&self) -> Option<Vec<u8>> {
        let bytes = std::fs::read(self.path.as_ref()?).ok()?;
        if ooxml::crypt::is_encrypted(&bytes) {
            let pw = self.encrypt_pw.as_ref()?;
            ooxml::crypt::decrypt(&bytes, pw).ok()
        } else {
            Some(bytes)
        }
    }

    /// **様式(セル)で言うことがあれば、状態行に出します。**
    /// 対応の付かない項目と埋まらないセルを黙って落とすと、空欄の申請書が
    /// できあがります(2026-08-18)
    pub(crate) fn form_status(&self) -> Option<String> {
        (!self.form_notes.is_empty()).then(|| self.form_notes.join("・"))
    }

    /// 読み取り専用の保護が掛かっているか(保護タブの「保護」で入切)
    pub(crate) fn protected(&self) -> bool {
        self.doc.protection.is_some()
    }

    /// マクロ = **サンドボックス(bubblewrap)の中の Python** が python-docx で文書の
    /// **複製**を直し、直った複製を読み込む(失敗しても文書は無傷)。
    /// 文書にコードは載せない — 「開く=実行」を作らない設計はそのまま。
    /// 台本の中で d が python-docx の Document、fill(名前, 値) が
    /// 名前つき記入欄への記入(macro_script 参照)。戻すのは Ctrl+Z の1手
    pub(crate) fn run_macro_file(&mut self, py_file: PathBuf, cx: &mut Context<Self>) {
        self.flush_target();
        let user_code = match std::fs::read_to_string(&py_file) {
            Ok(c) => c,
            Err(e) => {
                self.status = ui::tf!("cant_read_macro", e).into();
                return;
            }
        };
        let dir = std::env::temp_dir().join(format!("jo-wmacro-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let in_d = dir.join("in.docx");
        let out_d = dir.join("out.docx");
        // 複製は保存と同じ道で作る(原本の部品も持ち越す。暗号化は解いて)
        let original: Option<std::io::Cursor<Vec<u8>>> =
            self.original_plain().map(std::io::Cursor::new);
        let doc_out = self.doc_for_save(None);
        let w = std::fs::File::create(&in_d)
            .map_err(|e| e.to_string())
            .and_then(|f| ooxml::write_with(&doc_out, original, std::io::BufWriter::new(f)));
        if let Err(e) = w {
            self.status = ui::tf!("cant_hand_macro", e).into();
            return;
        }
        let script = macro_script(&in_d, &out_d, &user_code);
        let name = py_file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.status = ui::tf!("running_macro_python_sandbox", name).into();
        let task = cx.background_executor().spawn(async move {
            let py_path = dir.join("run.py");
            std::fs::write(&py_path, script).map_err(|e| e.to_string())?;
            let py = find_python();
            // 囲いは calc と同じ pyrun(ネット無し)。前はここに bwrap の生の
            // 写しがあり、Flatpak の分岐が入っていなかった — 共有で自然に直る。
            // 組めない機械では素の Python(マクロは自分で選んだ .py)
            let venv = std::fs::canonicalize(".venv").unwrap_or_default();
            let mut cmd = match pyrun::caged_python(&py, &dir, &[venv], false) {
                Some(c) => c,
                None => std::process::Command::new(&py),
            };
            let o = cmd
                .arg(&py_path)
                .output()
                .map_err(|e| ui::tf!("cant_start_python", e))?;
            let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !o.status.success() {
                let err = String::from_utf8_lossy(&o.stderr);
                let last = err
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or(ui::t!("cause_unknown"))
                    .to_string();
                return Err(if err.contains("No module named 'docx'") {
                    ui::t!("python_docx_missing_pip").to_string()
                } else {
                    last
                });
            }
            std::fs::read(&out_d)
                .map_err(|e| ui::tf!("cant_read_result", e))
                .map(|b| (b, out))
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok((bytes, out)) => {
                        match ooxml::read(std::io::Cursor::new(bytes)) {
                            Ok((doc, rep)) => {
                                this.checkpoint(false);
                                this.target = Target::Body;
                                this.notes = rep
                                    .unsupported
                                    .iter()
                                    .map(|(n, c)| {
                                        SharedString::from(format!("{n} × {c}"))
                                    })
                                    .collect();
                                this.pg = doc.page.unwrap_or_default();
                                this.set_doc(doc);
                                this.adopt_font();
                                this.relayout_keep();
                                this.dirty = true;
                                this.status = if out.is_empty() {
                                    ui::tf!("ran_macro_ctrl_z", name)
                                        .into()
                                } else {
                                    ui::tf!("macro_ctrl_z_undoes", name, out.lines().last().unwrap_or_default())
                                    .into()
                                };
                            }
                            Err(e) => this.status = ui::tf!("cant_read_result", e).into(),
                        }
                    }
                    Err(e) => this.status = ui::tf!("macro", e).into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 最近使ったファイルは **face::recent の1つの一覧**(統合の段8)。
    /// 文章と表で分けません — 使う人から見ればファイルはファイルです
    pub(crate) fn note_recent(p: &std::path::Path) {
        ui::recent::note(p);
    }

    pub(crate) fn recent_list() -> Vec<PathBuf> {
        ui::recent::list()
    }

    /// 新しい文書。未保存の変更があるときは作らない(黙って捨てない)。
    /// 返り値: 作ったか
    pub(crate) fn new_doc(&mut self) -> bool {
        if self.dirty {
            self.status =
                ui::t!("there_unsaved_changes_save").into();
            return false;
        }
        self.release_lock();
        self.locked_by = None;
        self.path = None;
        self.encrypt_pw = None;
        self.notes = Vec::new();
        self.target = Target::Body;
        self.pg = kumihan::PageSetup::default();
        self.set_doc(Document::plain(""));
        self.dirty = false;
        self.status = ui::t!("new_document").into();
        true
    }

    /// **形を決めてから保存先を聞く**(手引き
    /// `docs/ja/commands/ファイル/エクスポート.adoc`)。
    ///
    /// 「名前を付けて保存」と違って、*保存先は変わりません* — 書き出しは
    /// 別の形に写す操作で、いま書いている文書はそのままです。
    pub(crate) fn export_as(&mut self, cx: &mut Context<Self>, ext: &'static str) {
        let name = match ext {
            "docx" => ui::t!("word_document"),
            _ => ui::t!("plain_text_file"),
        };
        let src_of = self.path.clone();
        let ask = cx.background_executor().spawn(async move {
            let mut d = rfd::FileDialog::new().add_filter(name, &[ext]);
            if let Some(p) = src_of.as_ref().and_then(|p| p.parent()) {
                d = d.set_directory(p);
            }
            if let Some(n) = src_of.as_ref().and_then(|p| p.file_stem()) {
                d = d.set_file_name(format!("{}.{ext}", n.to_string_lossy()));
            }
            d.save_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(mut p) = r {
                    if p.extension().is_none() {
                        p.set_extension(ext);
                    }
                    // **書き出しの先は覚えません。** いま書いている文書の
                    // 保存先は元のままです(保存とは別の操作)
                    let src_path = this.path.clone();
                    let was_plain = this.native;
                    this.save_to(p);
                    this.path = src_path;
                    this.native = was_plain;
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 名前を付けて保存(いつでもダイアログ。別のスレッド — rfd は同期)
    pub(crate) fn save_as(&mut self, cx: &mut Context<Self>) {
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .add_filter(ui::t!("officework_document"), &["adoc"])
                .add_filter(ui::t!("word_document"), &["docx"])
                .save_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(mut p) = r {
                    if p.extension().is_none() {
                        // **拡張子を書かなければ、いまの形式のまま。**
                        // adoc で書いていたのに docx で保存されると、書式が
                        // 本文に焼き付いて元に戻せません
                        p.set_extension(if this.native { "adoc" } else { "docx" });
                    }
                    this.save_to(p);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 文書の情報の欄を確定する(Enter)
    pub(crate) fn commit_prop(&mut self) {
        let Some(i) = self.file_field.take() else { return };
        if self.protected() {
            self.status =
                ui::t!("protected_read_only_protection").into();
            return;
        }
        let text = self.prop_ed.text().to_string();
        let pr = &mut self.doc.props;
        match i {
            0 => pr.creator = text,
            1 => pr.title = text,
            2 => pr.keywords = text,
            3 => pr.subject = text,
            _ => pr.description = text,
        }
        self.dirty = true;
        self.status = ui::t!("document_info_recorded_goes").into();
    }

    /// ルビのパネルの Enter。控えた範囲に読みを付ける(空なら外す)
    pub(crate) fn rb_commit(&mut self) {
        self.rb_open = false;
        let text = self.rb_ed.text().trim().to_string();
        let range = self.rb_range.clone();
        if range.is_empty() {
            return;
        }
        self.doc.set_body_text(self.ed.text());
        let ruby = (!text.is_empty()).then(|| text.clone());
        self.doc.apply_char_format(range, move |f| f.ruby = ruby.clone());
        self.dirty = true;
        self.relayout_keep();
        self.status = if text.is_empty() {
            ui::t!("ruby_removed").into()
        } else {
            ui::tf!("ruby_set_saved_w", text).into()
        };
    }

    /// 数式のパネルの Enter。**組むのはエンジン(typst)** — 打った LaTeX を
    /// 渡して絵をもらい、カーソルの段落に置く。原文も一緒に持たせるので、
    /// 開き直しても直せる(絵だけだと消して打ち直しになる)
    pub(crate) fn eq_commit(&mut self) {
        self.eq_open = false;
        let tex = self.eq_ed.text().trim().to_string();
        if tex.is_empty() {
            self.status = "".into();
            return;
        }
        let size = self.doc.size_pt.unwrap_or(SIZE_PT);
        match crate::py::kumu_suushiki(&tex, size, self.doc.font.as_deref()) {
            Ok((bytes, w_mm, h_mm)) => {
                self.checkpoint(false);
                let im = kumihan::InlineImage {
                    bytes: std::sync::Arc::new(bytes),
                    w_mm,
                    h_mm,
                    tex: Some(tex.clone()),
                    src: None,
                    off: 0,
                };
                // 挿すのはカーソルの段落。**images_new にだけ入れる** —
                // 組版(layout)は images と images_new の両方を描くので、
                // 両方に入れると画面に二つ出る(実機で踏んだ)。
                // images は「読み込んだ絵」の持ち場で、保存では書かれない
                let cur = self.ed.cursor();
                self.ed.move_to(cur, false);
                self.para(|p| p.images_new.push(im.clone()));
                self.dirty = true;
                self.status = ui::tf!("equation_placed_typeset",
                                      crate::py::suushiki_no_kumi_kata()).into();
            }
            // **黙って何も起きない、をしない。** 組めない理由をそのまま見せる
            Err(e) => {
                self.status = ui::tf!("cannot_typeset_equation", e).into();
            }
        }
    }

    /// 上書きの前に、いまの中身を控える(最大9世代)。**中身は `ops::history`**
    /// — writer と calc で同じ物を使います
    pub(crate) fn keep_version(&self, p: &std::path::Path) {
        ops::history::keep(p);
    }

    /// 控えの一覧(新しい順)。(表示名, パス)
    pub(crate) fn versions(&self) -> Vec<(String, PathBuf)> {
        ops::history::list(self.path.as_deref())
    }

    /// 控えを開く。いまのファイルは動かさず、**名無しの複製**として読む
    /// (保存すると名前を聞く。元へ戻したいなら同じ名前で保存する — 
    /// 黙って元のファイルを書き戻したりしない)
    pub(crate) fn open_version(&mut self, q: &std::path::Path) {
        let bytes = match std::fs::read(q) {
            Ok(b) => b,
            Err(e) => {
                self.status = ui::tf!("cant_read_copy", e).into();
                return;
            }
        };
        let bytes = if ooxml::crypt::is_encrypted(&bytes) {
            match self.encrypt_pw.as_ref().map(|pw| ooxml::crypt::decrypt(&bytes, pw)) {
                Some(Ok(b)) => b,
                _ => {
                    self.status =
                        ui::t!("copy_encrypted_current_password").into();
                    return;
                }
            }
        } else {
            bytes
        };
        match ooxml::read(std::io::Cursor::new(bytes)) {
            Ok((doc, rep)) => {
                self.release_lock();
                self.locked_by = None;
                self.hist_open = false;
                self.target = Target::Body;
                self.notes = rep
                    .unsupported
                    .iter()
                    .map(|(n, c)| SharedString::from(format!("{n} × {c}")))
                    .collect();
                self.pg = doc.page.unwrap_or_default();
                self.set_doc(doc);
                self.adopt_font();
                self.relayout_keep();
                self.path = None;
                self.dirty = true;
                self.status = ui::t!("opened_copy_untitled_saving").into();
            }
            Err(e) => self.status = ui::tf!("cant_read_copy", e).into(),
        }
    }

    /// チャット(申し送り帳)の置き場。文書の隣の 名前.docx.chat.txt
    pub(crate) fn chat_path(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| {
            let mut os = p.as_os_str().to_owned();
            os.push(".chat.txt");
            PathBuf::from(os)
        })
    }

    /// 申し送りの最近の行(古い順で最大12行)
    pub(crate) fn chat_lines(&self) -> Vec<String> {
        let Some(cp) = self.chat_path() else { return Vec::new() };
        let Ok(text) = std::fs::read_to_string(cp) else { return Vec::new() };
        let mut v: Vec<String> =
            text.lines().rev().take(12).map(str::to_string).collect();
        v.reverse();
        v
    }

    /// 申し送り帳に名乗りと日時つきで1行書き足す
    pub(crate) fn chat_send(&mut self) {
        let text = self.chat_ed.text().trim().to_string();
        if text.is_empty() {
            return;
        }
        let Some(cp) = self.chat_path() else {
            self.status =
                ui::t!("not_file_yet_save").into();
            return;
        };
        let stamp = ui::now_stamp();
        let line = format!("[{stamp}] {}: {text}\n", lock_identity());
        use std::io::Write as _;
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&cp)
            .and_then(|mut f| f.write_all(line.as_bytes()))
        {
            Ok(_) => {
                self.chat_ed = Editor::new("");
                self.status =
                    ui::t!("message_left_chat_txt").into();
            }
            Err(e) => self.status = ui::tf!("cant_write_chat", e).into(),
        }
    }

    /// 自分のロックを外す(閉じる・別のファイルへ移るとき)。
    pub(crate) fn release_lock(&mut self) {
        if let Some(lp) = self.my_lock.take() {
            let _ = std::fs::remove_file(lp);
        }
    }

    /// このファイルのロックを見て、先客が居れば警告、居なければ自分が取る。
    pub(crate) fn acquire_lock(&mut self, p: &std::path::Path) {
        self.release_lock();
        match foreign_lock(p) {
            Some(who) => {
                self.locked_by = Some(who);
                // ロックは取らない(先客の邪魔をしない)
            }
            None => {
                self.locked_by = None;
                let lp = lock_path_for(p);
                // LibreOffice と同じ気持ちの中身(名乗りだけ)
                if std::fs::write(&lp, format!("{},;", lock_identity())).is_ok() {
                    self.my_lock = Some(lp);
                }
            }
        }
    }

    pub(crate) fn open(&mut self, p: PathBuf) {
        let bytes = match std::fs::read(&p) {
            Ok(b) => b,
            Err(e) => {
                self.status = ui::tf!("cant_open", e).into();
                return;
            }
        };
        // **開いたファイルまでの道を木の中で開く**(IDE の auto-reveal)。
        // 根がまだ無ければ先に立てる — 後のパネルの同期が展開を捨てない
        // よう、パネルと同じ答え(選んだフォルダ、無ければ親)で立てる
        if self.fl_tree.root().as_os_str().is_empty() {
            if let Some(dir) = self
                .chosen_folder
                .clone()
                .or_else(|| p.parent().map(|x| x.to_path_buf()))
            {
                self.fl_tree.set_root(dir);
            }
        }
        self.fl_tree.reveal(&p);
        // HTML(JS なしの閲覧 — SEKKEI「writer の HTML」)
        if p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm")
        }) {
            self.open_html(&p, &bytes);
            return;
        }
        // **ネイティブ文書(.adoc)**(2026-08-16)。意味だけを持ち、
        // 見た目はテンプレート — 素の文字とは扱いが違うので先に見る
        if p.extension().and_then(|e| e.to_str()).is_some_and(is_native_ext) {
            self.open_adoc(&p, &bytes);
            return;
        }
        // 素の文字(.py / .txt / .md)。**マクロを書くのは writer の仕事**
        // (発注者 2026-08-14「pyedit は使うな、writer を使え」)。
        // 段落 = 行として読み、保存も素の文字で返す(書式は付けない)
        if p.extension().and_then(|e| e.to_str()).is_some_and(is_plain_ext) {
            self.open_text(&p, &bytes);
            return;
        }
        if ooxml::crypt::is_encrypted(&bytes) {
            // パネルでパスワードを聞き、Enter(pw_commit)が続きをやる
            self.pw_pending = Some(p);
            self.pw_open = true;
            self.pw_ed = Editor::new("");
            self.status =
                ui::t!("document_encrypted_type_password").into();
            return;
        }
        self.open_plain(p, bytes);
    }

    /// HTML を開く。文書モデルに写すので、画面・PDF・docx 保存はそのまま
    /// 効く(HTML 書き出しは作らない — 互換は書式の境界で守る)。
    /// JS は実行しない。理解しない要素は帳簿へ。文字コードは UTF-8 → CP932
    pub(crate) fn open_html(&mut self, p: &std::path::Path, bytes: &[u8]) {
        self.native = false; // docx と同じ扱いに戻す(上の open_plain の註)
        let text = match std::str::from_utf8(bytes) {
            Ok(t) => t.to_string(),
            Err(_) => {
                let (t, _, bad) = encoding_rs::SHIFT_JIS.decode(bytes);
                if bad {
                    self.status =
                        ui::t!("unreadable_encoding_neither_utf").into();
                    return;
                }
                t.into_owned()
            }
        };
        let (doc, notes, forms, links) = kumihan::html::parse_full(&text);
        self.html_forms = forms;
        self.html_links = links;
        self.fm_field = None;
        self.fm_open = !self.html_forms.is_empty();
        self.lk_open = !self.html_links.is_empty() && self.html_base.is_some();
        self.target = Target::Body;
        self.hf_edit = None;
        self.track = false;
        self.track_base = None;
        self.encrypt_pw = None;
        self.release_lock();
        self.locked_by = None;
        self.notes = notes.into_iter().map(SharedString::from).collect();
        self.pg = kumihan::PageSetup::default();
        self.set_doc(doc);
        self.adopt_font();
        self.relayout_keep();
        // 保存は docx として名前を聞く(HTML には書き戻さない)
        self.path = None;
        self.dirty = true;
        self.status = ui::tf!("html_loaded_javascript_never", p.file_name().unwrap_or_default().to_string_lossy(), if self.fm_open { ui::t!("fill_panel_top_right") } else { "" })
        .into();
    }

    /// URL のパネルの Enter。GET して HTML として開く(いま繋いだ相手が起点)
    pub(crate) fn url_commit(&mut self, cx: &mut Context<Self>) {
        let url = self.url_ed.text().trim().to_string();
        if url.is_empty() {
            return;
        }
        self.url_open = false;
        let task = cx.background_executor().spawn(async move { http_fetch(&url, None) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok((bytes, final_url)) => this.adopt_fetched(&final_url, &bytes),
                    Err(e) => this.status = ui::tf!("cant_open", e).into(),
                }
                cx.notify();
            });
        })
        .detach();
        self.status = ui::tf!("fetching", self.url_ed.text()).into();
    }

    /// AI に頼んで、返事を文書に反映する。**別のスレッドで待つ**(画面は止めない)。
    /// 反映は必ず doc_undo に控えてから = **Ctrl+Z の1手で戻る**。
    /// 宛先が使えなければ理由を言う(黙って空にしない)
    pub(crate) fn ai_go(&mut self, job: AiJob, cx: &mut Context<Self>) {
        if self.protected() {
            self.status =
                ui::t!("protected_read_only_protection").into();
            return;
        }
        if self.ai_busy {
            self.status = ui::t!("still_thinking_please_wait").into();
            return;
        }
        let back = ui::ai::backend();
        if let Err(e) = ui::ai::ready(back) {
            self.status = format!("AI: {e}").into();
            return;
        }
        self.switch_target(Target::Body);
        self.flush_target();
        let sel = self.ed.selection();
        let text = self.ed.text().to_string();
        // 渡すもの: 選択があればそこ、無ければ全文(続きはカーソルまで)
        let body = match &job {
            AiJob::Macro(_) => String::new(),
            // 会話は**選んでいなくても通す**(「この書き方でいい?」のように
            // 範囲の要らない用件がある)。選んでいれば、そこが相手
            AiJob::Ask(_) | AiJob::Chat(_) if sel.is_empty() => String::new(),
            _ if sel.is_empty() => text.clone(),
            _ => text[sel.clone()].to_string(),
        };
        if body.trim().is_empty()
            && !matches!(job, AiJob::Ask(_) | AiJob::Macro(_) | AiJob::Chat(_))
        {
            self.status = ui::t!("no_text_type_select").into();
            return;
        }
        let (sys, ask) = job.prompt();
        let user = match &job {
            // **用件そのものが本体。** 選んだ字は付け合わせ
            AiJob::Chat(q) => {
                if body.trim().is_empty() {
                    q.clone()
                } else {
                    format!("{q}\n\n---\n{body}")
                }
            }
            AiJob::Ask(q) => {
                if body.trim().is_empty() {
                    q.clone()
                } else {
                    format!("{q}\n\n---\n{body}")
                }
            }
            // マクロには本文でなく、記入欄の名前一覧を渡す(台本の的)
            AiJob::Macro(q) => {
                let names = self.sdt_names();
                if names.is_empty() {
                    ui::tf!("document_no_named_form", q)
                } else {
                    ui::tf!("form_fields_document", q, names.join("、"))
                }
            }
            _ => format!("{ask}\n\n---\n{body}"),
        };
        let (sys, job2) = (sys.to_string(), job.clone());
        self.ai_busy = true;
        self.status = ui::tf!("asking_ai", back.label(), job.label())
        .into();
        let task = cx
            .background_executor()
            .spawn(async move { ui::ai::ask(back, &sys, &user) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                this.ai_busy = false;
                match r {
                    Ok(out) => this.ai_apply(job2, sel, out, cx),
                    Err(e) => this.status = format!("AI: {e}").into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 文書の名前つき記入欄の名前(重複なし・出現順)。
    /// マクロ台本を書く AI に「的」として渡す
    pub(crate) fn sdt_names(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut push = |sd: &kumihan::Sdt| {
            if !sd.tag.is_empty() && sd.tag != sd.kind.as_tag() && !out.contains(&sd.tag)
            {
                out.push(sd.tag.clone());
            }
        };
        for p in self.doc.paragraphs() {
            for r in &p.runs {
                if let Some(sd) = r.fmt.sdt.as_deref() {
                    push(sd);
                }
            }
        }
        for t in self.doc.tables() {
            for row in &t.rows {
                for c in row {
                    for p in &c.paragraphs {
                        for r in &p.runs {
                            if let Some(sd) = r.fmt.sdt.as_deref() {
                                push(sd);
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// 返事を文書へ入れる。**1手で戻せる**(doc_undo に控える)
    pub(crate) fn ai_apply(
        &mut self,
        job: AiJob,
        sel: std::ops::Range<usize>,
        out: String,
        _cx: &mut Context<Self>,
    ) {
        let out = out.trim().to_string();
        if out.is_empty() {
            self.status = ui::t!("ai_answer_empty_nothing").into();
            return;
        }
        // **会話は文書に入れない。** 左パネルに返し、置き換える文の案は
        // 人が「入れる」を押すまで文書に触らせない(押したのは人、が残る形)
        if matches!(job, AiJob::Chat(_)) {
            let plan = crate::util::extract_box(&out);
            let show = match &plan {
                Some(code) => {
                    // 囲みの外の説明だけを会話に出す(文そのものは下の欄に置く)
                    let desc = out.split("```").next().unwrap_or("").trim().to_string();
                    if desc.is_empty() {
                        let _ = code;
                        ui::t!("here_change").to_string()
                    } else {
                        desc
                    }
                }
                None => out.clone(),
            };
            self.ai_chat_log.push(ChatRow::Ai(show));
            self.ai_chat_plan = plan;
            self.status = if self.ai_chat_plan.is_some() {
                ui::t!("revised_text_ready_read").into()
            } else {
                ui::t!("answered_left_panel").into()
            };
            return;
        }
        // マクロ台本は文書に入れない — プラグイン置き場に .py で置き、
        // 人が読んで確かめてから一覧から実行する(開く=実行なしのまま)
        if matches!(job, AiJob::Macro(_)) {
            let code = strip_code_fence(&out);
            if code.trim().is_empty() {
                self.status = ui::t!("ai_script_empty_nothing").into();
                return;
            }
            let dir = plugins_dir();
            let _ = std::fs::create_dir_all(&dir);
            // 1つ目も訳を通す(ここだけ生の字だと、ja 以外で名前が揃わない)
            let mut i = 1;
            let mut path = dir.join(ui::tf!("ai_script_py", i));
            while path.exists() {
                i += 1;
                path = dir.join(ui::tf!("ai_script_py", i));
            }
            match std::fs::write(&path, &code) {
                Ok(()) => {
                    self.plug_open = true; // 置いた台本がすぐ見えるように
                    self.status = ui::tf!("script_placed_read_first", path.display())
                    .into();
                }
                Err(e) => self.status = ui::tf!("cant_place_script", e).into(),
            }
            return;
        }
        self.checkpoint(false);
        let label = job.label();
        match job {
            // Macro と Chat は上で受けて return 済み
            AiJob::Macro(_) | AiJob::Chat(_) => unreachable!(),
            // 自由な頼みは、カーソル(選択の終わり)の後ろへ
            AiJob::Ask(_) => {
                let at = sel.end.min(self.ed.text().len());
                self.ed.move_to(at, false);
                self.ed.insert(&format!("\n{out}"));
                self.doc.set_body_text(self.ed.text());
            }
            // ふりがなは |語《よみ》 を**うちのルビ**に直して振る
            AiJob::Furigana => {
                let base = if sel.is_empty() { 0 } else { sel.start };
                let (plain, rubies) = strip_ruby_marks(&out, base);
                let r = if sel.is_empty() { 0..self.ed.text().len() } else { sel };
                self.ed.move_to(r.start, false);
                self.ed.move_to(r.end, true);
                self.ed.insert(&plain);
                self.doc.set_body_text(self.ed.text());
                let n = rubies.len();
                for (range, yomi) in rubies {
                    self.doc.apply_char_format(range, move |f| {
                        f.ruby = Some(yomi.clone())
                    });
                }
                self.dirty = true;
                self.relayout_keep();
                self.status =
                    ui::tf!("furigana_set_places_one", n)
                        .into();
                return;
            }
        }
        self.dirty = true;
        self.relayout();
        self.status =
            ui::tf!("ai_inserted_one_ctrl", label).into();
    }

    /// **辞書でふりがなを振る**(2026-08-20 発注者「取り敢えずは辞書で」)。
    ///
    /// 振れたら真。辞書が無ければ偽を返し、呼ぶ側がモデルに回します。
    ///
    /// *外に出ません。待ちもありません。* 選んだ所(選んでいなければ本文
    /// ぜんぶ)の漢字の語に、辞書の読みを当てます。
    ///
    /// **読みが割れる語は数えて言います。** 辞書は1位を返しますが、それが
    /// 正しいとは限りません(人気《にんき/ひとけ》)。黙って確定せずに
    /// 「何箇所は読みが割れている」と伝えます — 直すのは人の仕事です。
    pub(crate) fn furigana_by_dict(&mut self) -> bool {
        if !ui::dict::available() {
            return false;
        }
        let sel = self.ed.selection();
        let full_text = self.ed.text().to_string();
        let (start, target_of) = if sel.is_empty() {
            (0usize, full_text.as_str())
        } else {
            (sel.start, &full_text[sel.clone()])
        };
        let cands = ui::dict::ruby_targets(target_of);
        if cands.is_empty() {
            self.status = ui::t!("there_no_kanji_words").into();
            return true;
        }
        let mut n = 0usize;
        let mut split_into = 0usize;
        for s in &cands {
            let Some(yomi) = s.readings.first() else { continue };
            // **読みが語と同じなら振りません**(ひらがなの語に振っても無駄)
            if yomi == &s.base {
                continue;
            }
            if s.readings.len() > 1 {
                split_into += 1;
            }
            let at = start + s.at;
            let r = at..at + s.base.len();
            let y = yomi.clone();
            self.doc.apply_char_format(r, move |f| f.ruby = Some(y.clone()));
            n += 1;
        }
        self.dirty = true;
        self.relayout_keep();
        self.status = if split_into > 0 {
            ui::tf!(
                "added_readings_places_them",
                n.to_string(),
                split_into.to_string()
            )
            .into()
        } else {
            ui::tf!("furigana_set_places_one", n.to_string())
                .into()
        };
        true
    }

    // **会話の送りは [`crate::agentloop`] に移りました**(2026-09-04。
    // agent.ja.adoc の段10)。前は1往復で答えを囲みに入れ、人が「入れる」を
    // 押す形でした。いまは道具で文書を読み書きし、書き替えは1手で入ります

    /// **直した文を入れる。** ここが「人が押した」の一点 —
    /// 押すまで AI は文書に触らない(2026-08-09 の決めを、人の一押しとして残す)。
    ///
    /// writer には calc のような Python の橋が無いので、入るのは**文そのもの**。
    /// 選んでいればそこを置き換え、選んでいなければカーソルの後ろへ挿す。
    /// どちらも Ctrl+Z 一手で戻る
    pub(crate) fn ai_chat_insert(&mut self) {
        let Some(plan) = self.ai_chat_plan.clone() else { return };
        if self.protected() {
            self.status =
                ui::t!("protected_read_only_protection").into();
            return;
        }
        self.switch_target(Target::Body);
        self.flush_target();
        self.checkpoint(false);
        let sel = self.ed.selection();
        let replaced = !sel.is_empty();
        if replaced {
            self.ed.move_to(sel.start, false);
            self.ed.move_to(sel.end, true);
            self.ed.insert(&plan);
        } else {
            let at = sel.end.min(self.ed.text().len());
            self.ed.move_to(at, false);
            self.ed.insert(&format!("\n{plan}"));
        }
        self.doc.set_body_text(self.ed.text());
        self.dirty = true;
        self.relayout();
        self.ai_chat_plan = None;
        self.ai_chat_log.push(ChatRow::Ai(ui::t!("applied").to_string()));
        self.status = if replaced {
            ui::t!("replaced_selection_ctrl_z").into()
        } else {
            ui::t!("inserted_after_cursor_ctrl").into()
        };
    }

    /// 記入欄(コンテンツコントロール)を挿す。選択があればそれを欄にし、
    /// 無ければ空欄の字を置いて欄にする。**中は普通に打てる**(欄は保たれる)
    pub(crate) fn insert_sdt(&mut self, kind: kumihan::SdtKind, items: Vec<String>) {
        use kumihan::SdtKind as K;
        self.switch_target(Target::Body);
        let sel = self.ed.selection();
        // 欄の初期の中身(選択があればその字)
        let range = if sel.is_empty() {
            let init = match kind {
                K::Checkbox => "☐".to_string(),
                K::Dropdown | K::Combo => {
                    items.first().cloned().unwrap_or_else(|| ui::t!("four_spaces").into())
                }
                K::Date => std::process::Command::new("date")
                    .arg("+%Y年%-m月%-d日")
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|| ui::t!("four_spaces").into()),
                K::Picture => ui::t!("image").to_string(),
                _ => ui::t!("four_spaces").to_string(),
            };
            let at = self.ed.cursor();
            self.ed.insert(&init);
            self.on_edited();
            at..at + init.len()
        } else {
            sel
        };
        let alias = kind.label().to_string();
        let tag = kind.as_tag().to_string();
        self.doc.set_body_text(self.ed.text());
        self.doc.apply_char_format(range.clone(), move |f| {
            f.sdt = Some(Box::new(kumihan::Sdt {
                kind,
                alias: alias.clone(),
                tag: tag.clone(),
                items: items.clone(),
            }))
        });
        self.dirty = true;
        self.relayout_keep();
        self.ed.move_to(range.end, false);
        self.status = ui::tf!("field_inserted_type_inside", kind.label())
        .into();
    }

    /// いる場所の記入欄(あれば)
    pub(crate) fn sdt_at(&self) -> Option<kumihan::Sdt> {
        self.doc
            .char_format_at(self.ed.selection())
            .sdt
            .as_deref()
            .cloned()
    }

    /// 選択肢のパネルの Enter(コンボ・ドロップダウンを挿す。
    /// 名前を聞いていたときは付け替えへ)
    pub(crate) fn sd_commit(&mut self) {
        self.sd_open = false;
        if self.sd_naming {
            self.sd_naming = false;
            self.sd_name_commit();
            return;
        }
        let items: Vec<String> = self
            .sd_ed
            .text()
            .split(&[',', '、', '/'][..])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if items.is_empty() {
            self.status = ui::t!("no_choices_type_them").into();
            return;
        }
        self.insert_sdt(self.sd_kind, items);
    }

    /// 名前のパネルの Enter。カーソルの記入欄の alias / tag をまるごと打ち替える
    /// (run が割れていても sdt_range_at が一つに繋げる)
    pub(crate) fn sd_name_commit(&mut self) {
        let name = self.sd_ed.text().trim().to_string();
        if name.is_empty() {
            self.status = ui::t!("no_name_given_field").into();
            return;
        }
        let Some(range) = self.doc.sdt_range_at(self.ed.cursor()) else {
            self.status =
                ui::t!("no_form_field_found").into();
            return;
        };
        let name2 = name.clone();
        self.doc.apply_char_format(range, move |f| {
            if let Some(sd) = f.sdt.as_deref_mut() {
                sd.alias = name2.clone();
                sd.tag = name2.clone();
            }
        });
        self.dirty = true;
        self.relayout_keep();
        self.status = ui::tf!("field_named_docx_w", name, name)
        .into();
    }


    /// 入切のボタンが「いま入っているか」。押した結果が画面に残るものは、
    /// ボタンの側にも出す(押したのに何も変わらないように見えるのを防ぐ)
    pub(crate) fn toggled(&self, id: &str) -> bool {
        match id {
            "nav" | "show-left" => self.nav_open,
            "show-toolbar" => self.show_toolbar,
            "show-statusbar" => self.show_statusbar,
            "multipage" => self.multipage,
            "show-right" => self.rp_open,
            "ruler" => self.ruler,
            "darkmode" => self.dark,
            "hidenchars" => self.show_marks,
            "line-numbers" => self.line_numbers,
            "direction" => self.doc.vertical,
            "track-changes" => self.track,
            "co-showcomment" => self.show_comments,
            "prot-doc" => self.doc.protection.is_some(),
            _ => false,
        }
    }

    /// ページ幅・ページ全体に合わせる(見えている大きさから倍率を出す)。
    /// width=true なら幅だけ、false なら高さも見て小さい方に合わせる
    pub(crate) fn fit_zoom(&mut self, width: bool) {
        // 紙は左 28px に置き、右にも同じだけ余白を見る
        let zw = (self.view_w_px - 56.0) / (self.pg.w_mm * PX_PER_MM);
        let z = if width {
            zw
        } else {
            zw.min((self.view_h_px - 28.0) / (self.pg.h_mm * PX_PER_MM))
        };
        self.zoom = z.clamp(0.2, 5.0);
        self.status = ui::tf!("fit_zoom", if width { ui::t!("width_2") } else { ui::t!("page_2") }, (self.zoom * 100.0).round() as i32)
        .into();
    }

    /// 見出しの一覧(ナビゲーション用)。(深さ, 字, 本文のバイト位置)
    pub(crate) fn headings(&self) -> Vec<(u8, String, usize)> {
        let mut out = Vec::new();
        let mut at = 0usize;
        for p in self.doc.paragraphs() {
            let text: String = p.runs.iter().map(|r| r.text.as_str()).collect();
            if let kumihan::ParaStyle::Heading(n) = p.style {
                out.push((n, text.clone(), at));
            }
            at += text.len() + 1;
        }
        out
    }

    /// 取ってきた HTML を開き、起点と土台を控える(リンクと送信の解決に使う)
    pub(crate) fn adopt_fetched(&mut self, url: &str, bytes: &[u8]) {
        let scheme_end = url.find("://").map(|i| i + 3).unwrap_or(0);
        let host = url[scheme_end..].split('/').next().unwrap_or("");
        self.html_origin = Some(format!("{}{host}", &url[..scheme_end]));
        self.html_base = Some(url.to_string());
        self.open_html(std::path::Path::new(url), bytes);
    }

    /// リンクを辿る(GET して同じ道で開く)
    pub(crate) fn follow_link(&mut self, href: String, cx: &mut Context<Self>) {
        let base = self.html_base.clone().unwrap_or_default();
        let url = resolve_url(&base, &href);
        self.status = ui::tf!("fetching", url).into();
        let task = cx.background_executor().spawn(async move { http_fetch(&url, None) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok((bytes, final_url)) => this.adopt_fetched(&final_url, &bytes),
                    Err(e) => this.status = ui::tf!("cant_open", e).into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 記入の欄の Enter。パネルの欄へ書き戻す
    pub(crate) fn fm_commit(&mut self) {
        let Some(i) = self.fm_field.take() else { return };
        let text = self.fm_ed.text().to_string();
        if let Some(fm) = self.html_forms.first_mut() {
            if let Some(f) = fm.fields.get_mut(i) {
                f.value = text;
            }
        }
        self.status = ui::t!("filled_press_submit_send").into();
    }

    /// フォームを送る。POST は urlencoded、GET は ?query。
    /// 網の線引き: いま開いている起点(html_origin)へだけ
    pub(crate) fn fm_submit(&mut self, cx: &mut Context<Self>) {
        let Some(fm) = self.html_forms.first().cloned() else { return };
        let Some(origin) = self.html_origin.clone() else {
            self.status =
                ui::t!("cant_submit_local_html").into();
            return;
        };
        let url = if fm.action.starts_with("http://") {
            if !fm.action.starts_with(&origin) {
                self.status = ui::t!("target_differs_origin_not").into();
                return;
            }
            fm.action.clone()
        } else if fm.action.starts_with('/') {
            format!("{origin}{}", fm.action)
        } else {
            format!("{origin}/{}", fm.action)
        };
        let q: String = fm
            .fields
            .iter()
            .map(|f| format!("{}={}", urlenc(&f.name), urlenc(&f.value)))
            .collect::<Vec<_>>()
            .join("&");
        let post = fm.method == "post";
        self.status = ui::t!("sending").into();
        let task = cx.background_executor().spawn(async move {
            if post {
                http_fetch(&url, Some(&q))
            } else {
                http_fetch(&format!("{url}?{q}"), None)
            }
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok((bytes, final_url)) => this.adopt_fetched(&final_url, &bytes),
                    Err(e) => this.status = ui::tf!("cant_send", e).into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 平文(zip)の docx を読み込む。open と pw_commit の共通の続き
    /// 素の文字を開く(1行 = 1段落)。等幅の書体にして、字下げが読めるように
    pub(crate) fn open_text(&mut self, p: &std::path::Path, bytes: &[u8]) {
        self.native = false; // docx と同じ扱いに戻す(上の open_plain の註)
        // UTF-8 として読む(.py は UTF-8 が既定 — PEP 263)。読めない字は
        // 置き換えの記号にする。**黙って開かないより、開いて見せてから直す**
        let text = String::from_utf8_lossy(bytes).into_owned();
        self.target = Target::Body;
        self.hf_edit = None;
        self.track = false;
        self.track_base = None;
        self.encrypt_pw = None;
        self.notes.clear();
        let mut doc = Document::plain(&text);
        // **コードは等幅で、折り返さずに読む。** 明朝の紙面で組むと
        // 字下げ(Python では構文)が読めず、長い行が紙の幅で折れる。
        // 書体は run に持たせる — 保存は素の文字なので、この書式は
        // 画面と PDF にだけ効き、ファイルには残らない
        for b in &mut doc.blocks {
            if let kumihan::Block::Para(para) = b {
                for r in &mut para.runs {
                    r.font = Some(MONO.into());
                    r.size_pt = Some(10.0);
                }
            }
        }
        // 紙は横向き(A4 を寝かせる)。長い行が折り返しにくい。
        // 余白も詰める — コードは端まで使う
        self.pg = kumihan::PageSetup {
            w_mm: 297.0, h_mm: 210.0,
            left_mm: 12.0, right_mm: 12.0, top_mm: 12.0, bottom_mm: 12.0,
            columns: 1,
        };
        self.set_doc(doc);
        self.adopt_font();
        self.path = Some(p.to_path_buf());
        self.dirty = false;
        let line = text.lines().count();
        self.status = ui::tf!(
            "lines_opened_plain_text",
            p.file_name().unwrap_or_default().to_string_lossy(),
            line
        )
        .into();
    }

    /// **ネイティブ文書(.adoc)を開く。** 中身は意味だけで、見た目は
    /// テンプレートが持つ(SEKKEI「本文とテンプレートを分ける」)。
    pub(crate) fn open_adoc(&mut self, p: &std::path::Path, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
        // **1つのファイルに文書が何枚も入っていることがあります**
        // (同時に送る請求書の原稿など。2026-08-19)。`= 題` で切れています
        let (mut docs_of, ledger) = match kumihan::adoc::parse_many_full(&text) {
            Ok(d) => d,
            Err(e) => {
                // **読めない所は言う。** 黙って本文に化けさせない
                self.status = ui::tf!("cannot_read", p.display().to_string(), e).into();
                return;
            }
        };
        let doc = docs_of.first().cloned().unwrap_or_default();
        let n_sheets = docs_of.len();
        self.target = Target::Body;
        self.hf_edit = None;
        self.track = false;
        self.track_base = None;
        self.encrypt_pw = None;
        // **うちが扱わない AsciiDoc の書き方は、帳簿に出します。**
        // 字は本文として残りますが意味は落ちています。黙って化けさせると、
        // 書いた人は出来上がりを見るまで気づけません(2026-08-18)
        self.notes = ledger.iter().map(|n| SharedString::from(n.clone())).collect();
        self.native = true;
        let (tmpl, tmpl_path, notes) = self.load_template(doc.template.as_deref(), p);
        self.tmpl = tmpl;
        self.tmpl_path = tmpl_path;
        // 用紙はテンプレートが持つ(本文は持たない)
        self.pg = self.tmpl.page.unwrap_or_default();
        self.set_doc(doc);
        // 2枚目からは控えへ。0 番は置き場(いま `doc` にあります)
        if n_sheets > 1 {
            docs_of[0] = kumihan::Document::default();
            self.docs = std::mem::take(&mut docs_of);
        } else {
            self.docs.clear();
        }
        self.doc_at = 0;
        self.adopt_font();
        self.path = Some(p.to_path_buf());
        // **数式は開いたときに組みます。** 本文が持っているのは LaTeX の原文
        // だけなので、組まないと紙面には何も出ません(2026-08-18 に実機で
        // 見つけました — 白紙の段落になっていました)
        let cannot_typeset = self.render_formulas();
        self.dirty = false;
        self.status = ui::tf!(
            "text_adoc_formatting_comes",
            p.file_name().unwrap_or_default().to_string_lossy(),
            notes
        )
        .into();
        if n_sheets > 1 {
            self.status =
                ui::tf!("documents", self.status.clone(), n_sheets.to_string()).into();
        }
        if cannot_typeset > 0 {
            self.status =
                ui::tf!("equations_not_typeset", self.status.clone(), cannot_typeset).into();
        }
        if !ledger.is_empty() {
            self.status = ui::tf!("uses_markup_not_handle",
                                  self.status.clone(), ledger.join("・")).into();
        }
        // **様式の食い違いは開いたときに言います。** 印刷してから気づくのでは
        // 遅いので、セルを組んだその場で出します
        if let Some(says) = self.form_status() {
            self.status = ui::tf!("form", self.status.clone(), says).into();
        }
        // **書き出し先ごとのテンプレートが壊れていれば、開いたときに言います**
        // (2026-09-02)。前は黙って `テンプレート.toml` に落ちていたので、
        // 置いた人には「効かない」としか分かりませんでした
        for purpose_of in ["印刷", "web"] {
            if let Some(e) = self.purpose_template_error(purpose_of) {
                self.notes.push(SharedString::from(e.clone()));
                self.status = format!("{}。{e}", self.status).into();
            }
        }
    }

    /// 本文の中の数式(LaTeX の原文だけを持つ画像)を組みます。返りは組めなかった数。
    ///
    /// 組むのはエンジン(typst)です。**画面に出すためだけ**なので、
    /// 文書が汚れた印は立てません — 保存で本文に入るのは原文の側です。
    pub(crate) fn render_formulas(&mut self) -> usize {
        let size = self.doc.size_pt.unwrap_or(kumihan::DEFAULT_PT);
        let mut cannot_typeset = 0usize;
        let mut laid_out = false;
        let font = self.doc.font.clone();
        for p in self.doc.paragraphs_mut() {
            for im in p.images_new.iter_mut().chain(p.images.iter_mut()) {
                let Some(tex) = im.tex.clone() else { continue };
                if !im.bytes.is_empty() {
                    continue;
                }
                match crate::py::kumu_suushiki(&tex, size, font.as_deref()) {
                    Ok((bytes, w_mm, h_mm)) => {
                        im.bytes = std::sync::Arc::new(bytes);
                        im.w_mm = w_mm;
                        im.h_mm = h_mm;
                        laid_out = true;
                    }
                    Err(_) => cannot_typeset += 1,
                }
            }
        }
        if laid_out {
            self.relayout_keep();
        }
        cannot_typeset
    }

    /// **フォルダの書式のファイルの名前。**
    ///
    /// 発注者 2026-08-18「原則は、ディレクトリーの書式用のファイルをひとつ
    /// おく。それがテンプレート」。名前を書かなくても、同じフォルダにこの
    /// ファイルがあれば、そのフォルダの文書はこれを使います。
    pub(crate) const FOLDER_TEMPLATE: &'static str = "テンプレート.toml";

    /// **綴りのテンプレートが無いとき、下に敷く物**と、その呼び名。
    ///
    /// この機械の標準(`~/.config/officework/テンプレート.toml`)があれば
    /// それを、無ければ同梱の既定を返します。呼び名を一緒に返すのは、
    /// 状態の行で「同梱の既定」と言い切ってしまうと、自分で決めた標準が
    /// 効いているのに嘘になるからです。
    fn std_template() -> (kumihan::theme::Theme, String) {
        let at = ui::settings::dir().join(kumihan::theme::user_template_name());
        if at.exists() {
            (Self::user_theme(), ui::t!("computers_defaults").to_string())
        } else {
            (kumihan::theme::default_theme(), ui::t!("built_default").to_string())
        }
    }

    /// テンプレートを探して読む。返りは(テンプレート, 読んだ場所, 言い分)。
    /// 場所が None なら、綴りのテンプレートは使っていません。
    ///
    /// 探す順:
    ///
    /// 1. 本文の頭に `:template: 名前` があれば、その名前で **隣 → 置き場**
    /// 2. 名前が無ければ、**同じフォルダの `テンプレート.toml`**
    /// 3. どちらも無ければ、この機械の標準 → 同梱の既定
    ///
    /// 1が2より先なのは、**書いてあることが決まりより強い**からです。
    ///
    /// 見つかったテンプレートには、**この機械の標準を下に敷きます**
    /// (2026-08-26)。綴りのテンプレートが言っていないことは、自分が
    /// いつも使う書式で埋まります。
    fn load_template(
        &self,
        name: Option<&str>,
        doc_path: &std::path::Path,
    ) -> (kumihan::theme::Theme, Option<PathBuf>, String) {
        let Some(name) = name else {
            // 名指しが無いときは、フォルダの書式のファイルを使います
            if let Some(dir) = doc_path.parent() {
                let at = dir.join(Self::FOLDER_TEMPLATE);
                if let Ok(src) = std::fs::read_to_string(&at) {
                    return match kumihan::theme::parse(&src) {
                        Ok(th) => (
                            kumihan::theme::merge(
                                th.for_current_language(),
                                Self::user_theme(),
                            ),
                            Some(at.clone()),
                            at.display().to_string(),
                        ),
                        Err(e) => {
                            let (th, name) = Self::std_template();
                            (
                                th,
                                None,
                                ui::tf!("not_read_used",
                                        at.display().to_string(), name, e).to_string(),
                            )
                        }
                    };
                }
                // **書式のファイルらしい物があるのに名前が違うときは言います。**
                // 黙って既定で開くと、置いた人は「効かない」としか分かりません
                if let Some(other_of) = other_pattern(dir) {
                    let (th, name) = Self::std_template();
                    return (
                        th,
                        None,
                        ui::tf!("use_folder_rename_write",
                                name, other_of, Self::FOLDER_TEMPLATE).to_string(),
                    );
                }
            }
            let (th, name) = Self::std_template();
            return (th, None, name);
        };
        let mut cands = Vec::new();
        if let Some(dir) = doc_path.parent() {
            cands.push(dir.join(format!("{name}.toml")));
        }
        cands.push(ui::settings::path().with_file_name("templates").join(format!("{name}.toml")));
        for c in cands {
            let Ok(src) = std::fs::read_to_string(&c) else { continue };
            return match kumihan::theme::parse(&src) {
                Ok(th) => (
                    kumihan::theme::merge(th.for_current_language(), Self::user_theme()),
                    Some(c.clone()),
                    c.display().to_string(),
                ),
                // **壊れたテンプレートは黙って既定に落ちない** — どこが
                // 悪いか言わないと、直す手がかりが無い
                Err(e) => {
                    let (th, name) = Self::std_template();
                    (
                        th,
                        None,
                        ui::tf!("not_read_used", c.display().to_string(), name, e)
                            .to_string(),
                    )
                }
            };
        }
        let (th, name) = Self::std_template();
        (
            th,
            None,
            ui::tf!("template_not_found_used", name, name).to_string(),
        )
    }

    /// **書き出し先ごとのテンプレート。** 無ければいま着ている物。
    ///
    /// 発注者 2026-08-18「表示用、印刷用、Web用、アプリ用と複数のテンプレートを
    /// 持つのも悪くないのでは」。**混ぜないので複雑になりません** — 一度に効くのは
    /// 1枚のままで、どの1枚かが書き出し先で決まるだけです。
    ///
    /// 名前は `テンプレート-<用途>.toml`(フォルダの決まりと同じ形)。
    /// 用途は `web` と `印刷` の2つで、画面と保存は `テンプレート.toml` です。
    /// 返りは(テンプレート, 使った場所。None は今のまま)。
    /// **利用者の標準テンプレート**(2026-08-26 発注者
    /// 「ユーザーとしての標準設定は、HOME/~.config/ ディレクトリにおく」)。
    /// **いま効いている字の大きさの下敷き。**
    ///
    /// 文書が自分で言っていればそれ、言っていなければテンプレートの
    /// 大きさです。テンプレートの大きさは言語で変わるので(`[文書.ko]`)、
    /// ここを通さないと画面に 10.5pt と出たまま、紙は 10pt で組まれます。
    pub(crate) fn base_pt(&self) -> f32 {
        self.doc.size_pt.or(self.tmpl.size_pt).unwrap_or(crate::SIZE_PT)
    }

    /// カーソルの位置の字の大きさ(リボンの欄に出す物)。
    pub(crate) fn size_now(&self) -> f32 {
        self.doc.size_at_with(self.ed.selection(), self.base_pt())
    }

    pub(crate) fn user_theme() -> kumihan::theme::Theme {
        kumihan::theme::user_theme(&ui::settings::dir())
    }

    /// **この機械の標準の書体を決める。**
    ///
    /// `~/.config/officework/テンプレート.toml` に書き替えます。綴りと文書が
    /// 何も言っていないとき、ここが効きます。
    ///
    /// 書き先は `[文書]` ではなく **`[文書.<いまの言語>]`** です
    /// (2026-08-26 発注者「PCやディレクトリーの標準テンプレートには、
    /// フォントやサイズが言語によって違うことを考慮しないといけない」)。
    /// 日本語で選んだ書体を、英語の画面に切り替えたときまで押しつけると、
    /// ラテン文字が日本語の書体で出ます。
    ///
    /// 既にあるファイルは*その行だけ*直します。人が手で書いた他の設定や
    /// 注釈を消さないためです。
    pub(crate) fn set_user_font(&mut self, font_of: &str) {
        let store_dir = ui::settings::dir();
        let at = store_dir.join(kumihan::theme::user_template_name());
        let from = std::fs::read_to_string(&at).unwrap_or_default();
        let section = format!("文書.{}", ui::language());
        let fresh = kumihan::theme::put(&from, &section, "書体", &format!("\"{font_of}\""));
        if let Err(e) = std::fs::create_dir_all(&store_dir)
            .and_then(|_| std::fs::write(&at, fresh))
        {
            self.status = ui::tf!("cant_write", e).into();
            return;
        }
        // **その場で効かせます。** 開き直さないと変わらないのでは、
        // 選んだ結果が見えません。
        //
        // まだ保存していない文書には道がないので、開いている綴りの中に
        // ある物として探します(`load_template` は親のフォルダしか見ま
        // せんから、名前は何でもかまいません)。
        let path = self
            .path
            .clone()
            .or_else(|| self.folder().map(|d| d.join("まだ保存していない.adoc")));
        if let Some(path) = path {
            let (th, tp, _) = self.load_template(self.doc.template.as_deref(), &path);
            self.tmpl = th;
            self.tmpl_path = tp;
        } else {
            self.tmpl = Self::user_theme();
        }
        self.adopt_font();
        self.relayout();
        self.status = ui::tf!(
            "set_default_font_computer",
            ui::language(),
            font_of,
            at.display()
        )
        .into();
    }

    pub(crate) fn template_for(&self, purpose_of: &str) -> (kumihan::theme::Theme, Option<String>) {
        let now = || (self.tmpl.clone(), None);
        if !self.native {
            // 互換の文書(docx)には型紙がない。既定で出す
            return (Self::user_theme(), None);
        }
        // **保存する前でも綴りの書式が効きます**(2026-08-26)。前は
        // 保存先の親からしか探さなかったので、書いている間はずっと
        // 別の見た目で、保存した瞬間に変わっていました
        let Some(dir) = self
            .path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .or_else(|| self.folder())
        else {
            return now();
        };
        let at = dir.join(format!("テンプレート-{purpose_of}.toml"));
        let Ok(src) = std::fs::read_to_string(&at) else { return now() };
        match kumihan::theme::parse(&src) {
            // **利用者の段を下に敷きます。** 綴りが言っていないことは、
            // この機械で自分がいつも使う書式で埋まります
            Ok(th) => (
                kumihan::theme::merge(th.for_current_language(), Self::user_theme()),
                Some(at.display().to_string()),
            ),
            // **壊れていたら黙って落ちない。** どれを使ったかは呼ぶ側が言う
            Err(_) => now(),
        }
    }

    /// **紙の折り方で組んだ紙面。** 印刷用のテンプレートがあるときだけ返します
    /// (無ければ画面の紙面がそのまま紙の紙面です)。
    ///
    /// 発注者 2026-08-18「『画面と紙は同じ紙面』という約束はやめにしたら」。
    /// 約束は外しましたが、**数える所(ページ番号・目次)は紙で数えます** —
    /// 目次が「3ページ」と言うのに紙の3ページに無い、が起きないためです。
    ///
    /// 返りは(紙面, 用紙, 使ったファイル)。
    pub(crate) fn print_layout(&self) -> Option<(Page, kumihan::PageSetup, String)> {
        // **用途はファイル名の一部**(テンプレート-印刷.toml)。文書の
        // 形式の値なので日本語のままです
        let (th, used) = self.template_for("印刷");
        let used = used?;
        let m = Metrics::new(&self.font_bytes).ok()?;
        let pg = th.page.unwrap_or(self.pg);
        let snapshot = Look {
            pg,
            vertical: self.doc.vertical,
            group: th.setting,
            view_w_px: self.view_w_px,
        };
        Some((snapshot.lay_once(&kumihan::theme::compose(&self.doc, &th), &m), pg, used))
    }

    /// 保存した先のフォルダに書式のファイルがあれば着る。返りは着た場所。
    ///
    /// **名指し(`:template:`)がある文書は触りません。** 書いてあることが
    /// 決まりより強い、という順番は読むときと同じです。
    fn adopt_folder_template(&mut self, doc_path: &std::path::Path) -> Option<String> {
        if self.doc.template.is_some() {
            return None;
        }
        let at = doc_path.parent()?.join(Self::FOLDER_TEMPLATE);
        if self.tmpl_path.as_deref() == Some(at.as_path()) {
            return None; // もう着ている
        }
        let th = kumihan::theme::read_theme(&at)?;
        self.tmpl = kumihan::theme::merge(th.for_current_language(), Self::user_theme());
        self.tmpl_path = Some(at.clone());
        self.pg = self.tmpl.page.unwrap_or(self.pg);
        self.relayout_keep();
        Some(at.display().to_string())
    }

    /// **フォルダから探す**(2026-08-17 発注者。SFIND の写真)。
    ///
    /// 素の字は face が読み、**.docx は writer が中身を渡す** — 一度 txt に
    /// 落としてから探す手間が消える。当たりは一覧に出て、選ぶと下に見え、
    /// 下の「読み込み」で初めて開く(見て、これだと分かってから開く)。
    pub(crate) fn find_in_folder(&mut self) {
        let Some(dir) = self.find_dir() else {
            self.status = ui::t!("choose_folder_search").into();
            return;
        };
        let term = self.fd_term.text().to_string();
        if term.trim().is_empty() {
            self.status = ui::t!("search_text_empty").into();
            return;
        }
        self.fd_busy = true;
        self.fd_at = None;
        self.fd_peek.clear();
        // **docx の本文を渡す。** 読めない物は None を返して face に任せる
        let extract = |p: &std::path::Path| -> Option<String> {
            let e = p.extension().and_then(|x| x.to_str())?.to_ascii_lowercase();
            if e != "docx" {
                return None;
            }
            let bytes = std::fs::read(p).ok()?;
            let (doc, _) = ooxml::read(std::io::Cursor::new(bytes)).ok()?;
            Some(doc.body_text())
        };
        let q = ui::search::Query {
            term: term.clone(),
            glob: self.fd_glob.text().to_string(),
            case: false,
            max_files: 4000,
            max_hits: 3000,
            extract: &extract,
        };
        let (hits, tally) = ui::search::walk(&dir, &q);
        self.fd_hits = hits;
        self.fd_tally = tally;
        self.fd_busy = false;
        // 報せは ui::search の1本(writer と calc で同じ文)
        self.status = ui::search::tally_message(&tally).into();
    }

    /// **探す場所。** 選んでいなければ(1)前に選んだ場所(settings.toml)
    /// (2)いま開いている文書の隣、の順に決める。
    ///
    /// 開いている文書の隣は**当たり前の出発点**で、そこから始められないと
    /// 「場所を選ぶ」を毎回押すことになる(2026-08-17)
    /// 開いているファイルの枚数。
    pub(crate) fn file_count(&self) -> usize {
        self.files.len().max(1)
    }

    /// 何枚目かのファイルの名前(上のタブに出します)。
    pub(crate) fn file_name(&self, i: usize) -> String {
        let path = if i == self.file_at {
            self.path.clone()
        } else {
            self.files.get(i).and_then(|f| f.path.clone())
        };
        match path {
            Some(p) => {
                let n = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                ui::folder::display_name(&n, ui::folder::kind_of(&n))
            }
            None => ui::t!("unnamed").to_string(),
        }
    }

    /// 何枚目かのファイルに書きかけがあるか(タブに印を出します)。
    pub(crate) fn file_dirty(&self, i: usize) -> bool {
        if i == self.file_at { self.dirty } else { self.files.get(i).is_some_and(|f| f.dirty) }
    }

    /// いま見ているファイルの持ち物を取り出す(入れ替えのため)。
    fn take_open(&mut self) -> OpenFile {
        OpenFile {
            doc: std::mem::take(&mut self.doc),
            docs: std::mem::take(&mut self.docs),
            doc_at: self.doc_at,
            ed: std::mem::replace(&mut self.ed, Editor::new("")),
            path: self.path.take(),
            dirty: self.dirty,
            undo_stack: std::mem::take(&mut self.undo_stack),
            redo_stack: std::mem::take(&mut self.redo_stack),
            scroll_mm: self.scroll_mm,
            native: self.native,
            tmpl: std::mem::replace(&mut self.tmpl, kumihan::theme::default_theme()),
            tmpl_path: self.tmpl_path.take(),
            notes: std::mem::take(&mut self.notes),
        }
    }

    /// 取り出した持ち物を据える。
    fn put_open(&mut self, f: OpenFile) {
        self.doc = f.doc;
        self.docs = f.docs;
        self.doc_at = f.doc_at;
        self.ed = f.ed;
        self.path = f.path;
        self.dirty = f.dirty;
        self.undo_stack = f.undo_stack;
        self.redo_stack = f.redo_stack;
        self.scroll_mm = f.scroll_mm;
        self.native = f.native;
        self.tmpl = f.tmpl;
        self.tmpl_path = f.tmpl_path;
        self.notes = f.notes;
    }

    /// 見るファイルを替える。
    pub(crate) fn show_file(&mut self, i: usize) {
        if i == self.file_at || i >= self.files.len() {
            return;
        }
        self.flush_target();
        self.target = Target::Body;
        self.hf_edit = None;
        let now = self.take_open();
        self.files[self.file_at] = now;
        self.file_at = i;
        let next = std::mem::take(&mut self.files[i]);
        self.put_open(next);
        self.adopt_font();
        self.lay();
        self.status = ui::tf!(
            "now_showing",
            self.path.as_ref().map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
                .unwrap_or_default()
        )
        .into();
    }

    /// **新しいタブでファイルを開く**(2026-08-19)。
    ///
    /// 既に開いているファイルなら、そのタブへ行くだけです — 同じファイルを
    /// 二重に開くと、どちらを保存したのか分からなくなります。
    pub(crate) fn open_in_tab(&mut self, p: PathBuf) {
        // もう開いているか
        if self.path.as_deref() == Some(p.as_path()) {
            return;
        }
        if let Some(i) = self.files.iter().position(|f| f.path.as_deref() == Some(p.as_path())) {
            self.show_file(i);
            return;
        }
        // 1枚目のときは、いまの場所を並びに登録してから足す
        if self.files.is_empty() {
            let now = self.take_open();
            self.files.push(now);
            self.file_at = 0;
        } else {
            let now = self.take_open();
            self.files[self.file_at] = now;
        }
        self.files.push(OpenFile::default());
        self.file_at = self.files.len() - 1;
        let fresh = std::mem::take(&mut self.files[self.file_at]);
        self.put_open(fresh);
        self.open(p);
    }

    /// タブを閉じる。**書きかけがあるときは閉じません**(黙って捨てない)。
    pub(crate) fn close_file(&mut self, i: usize) -> bool {
        if self.files.len() <= 1 || i >= self.files.len() {
            return false;
        }
        let draft = self.file_dirty(i);
        if draft {
            self.status = ui::t!("there_unsaved_changes_save_first").into();
            return false;
        }
        if i == self.file_at {
            // いま見ている物を閉じる — 隣へ移ってから外す
            let goes_to = if i + 1 < self.files.len() { i + 1 } else { i - 1 };
            self.show_file(goes_to);
        }
        self.files.remove(i);
        if self.file_at > i {
            self.file_at -= 1;
        }
        true
    }

    /// このファイルに入っている文書の枚数。
    pub(crate) fn doc_count(&self) -> usize {
        self.docs.len().max(1)
    }

    /// 何枚目かの文書の名前(下のタブに出します)。
    pub(crate) fn doc_name(&self, i: usize) -> String {
        let title = if i == self.doc_at {
            self.doc.props.title.clone()
        } else {
            self.docs.get(i).map(|d| d.props.title.clone()).unwrap_or_default()
        };
        if title.trim().is_empty() {
            ui::tf!("document_2", (i + 1).to_string()).to_string()
        } else {
            title
        }
    }

    /// 見る文書を替える。
    ///
    /// いま見ている物を控えへ戻し、行き先を取り出します。
    /// **編集中の字は先に本文へ戻します** — 戻さないと打ちかけが消えます。
    pub(crate) fn show_doc(&mut self, i: usize) {
        if i == self.doc_at || i >= self.docs.len() {
            return;
        }
        self.flush_target();
        // いま見ている物を置き場へ戻し、行き先と入れ替える
        std::mem::swap(&mut self.doc, &mut self.docs[self.doc_at]);
        self.doc_at = i;
        std::mem::swap(&mut self.doc, &mut self.docs[i]);
        self.ed = Editor::new(&self.doc.body_text());
        self.lay();
    }

    /// 保存するときの並び(いま見ている物を戻した形)。
    pub(crate) fn docs_for_save(&self) -> Vec<Document> {
        if self.docs.len() <= 1 {
            return vec![self.doc.clone()];
        }
        let mut v = self.docs.clone();
        v[self.doc_at] = self.doc.clone();
        v
    }

    /// **いま開いているフォルダ。** 右パネルのファイル一覧が並べる場所です。
    ///
    /// 開いているファイルの親を使います。まだ何も開いていなければ、
    /// 前に使ったフォルダ(`settings.toml` の `folder`)です。
    /// 一覧に出すフォルダ。
    ///
    /// **人が選んだ物がいちばん強い**(2026-08-26)。前は開いている
    /// ファイルの親を先に返していたので、ファイルを開いたまま
    /// 「フォルダーを開く」で別の綴りを選んでも一覧が変わりませんでした。
    pub(crate) fn folder(&self) -> Option<PathBuf> {
        if let Some(d) = self.chosen_folder.as_ref().filter(|d| d.is_dir()) {
            return Some(d.clone());
        }
        if let Some(p) = self.path.as_ref().and_then(|p| p.parent()) {
            return Some(p.to_path_buf());
        }
        ui::settings::get("folder").map(PathBuf::from).filter(|p| p.is_dir())
    }

    /// フォルダを覚える(次に起動したときここを開きます)。
    pub(crate) fn remember_folder(&self) {
        // **埋め込みのときは書きません**(統合の段4)。開いていた場所を
        // 覚えるのは officework の `session.txt` の仕事になりました。
        // 両方が書くと、食い違ったときにどちらが正しいか誰にも言えません
        if self.embedded {
            return;
        }
        if let Some(d) = self.folder() {
            ui::settings::set("folder", &d.display().to_string());
        }
    }

    pub(crate) fn find_dir(&self) -> Option<PathBuf> {
        if let Some(d) = &self.fd_dir {
            return Some(d.clone());
        }
        if let Some(s) = ui::settings::get("find_dir") {
            let p = PathBuf::from(s);
            if p.is_dir() {
                return Some(p);
            }
        }
        self.path.as_ref().and_then(|p| p.parent()).map(|d| d.to_path_buf())
    }

    /// 探す場所を選ぶ(**窓は別のスレッド**)
    pub(crate) fn find_dir_dialog(&mut self, cx: &mut Context<Self>) {
        let start = self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let ask = cx.background_executor().spawn(async move {
            let mut d = rfd::FileDialog::new();
            if let Some(s) = start {
                d = d.set_directory(s);
            }
            d.pick_folder()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = r {
                    this.status = ui::tf!("folder_2", p.display().to_string()).into();
                    // 次に開いたときも同じ場所から始められるように控える
                    ui::settings::set("find_dir", &p.display().to_string());
                    this.fd_dir = Some(p);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 当たりを1つ選ぶ。**開かない** — 下にその前後を見せるだけ
    pub(crate) fn find_peek(&mut self, fi: usize, hi: usize) {
        let Some(f) = self.fd_hits.get(fi) else { return };
        let Some(h) = f.hits.get(hi) else { return };
        self.fd_at = Some((fi, hi));
        // 前後を見せる(SFIND の下の窓と同じ役)。読めなければ当たりの行だけ
        let body = std::fs::read_to_string(&f.path).ok();
        self.fd_peek = match body {
            Some(b) => {
                let lines: Vec<&str> = b.split('\n').collect();
                let i = (h.line as usize).saturating_sub(1);
                let from = i.saturating_sub(6);
                let to = (i + 7).min(lines.len());
                lines[from..to]
                    .iter()
                    .enumerate()
                    .map(|(k, l)| format!("{:05} {l}", from + k + 1))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            None => format!("{:05} {}", h.line, h.text),
        };
        self.status = ui::tf!(
            "line_load_below_opens",
            f.path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            h.line.to_string()
        )
        .into();
    }

    /// 下の「読み込み」。**選んでいる当たりの文書を開き、その位置へ飛ぶ**
    pub(crate) fn find_load(&mut self) {
        let Some((fi, hi)) = self.fd_at else {
            self.status = ui::t!("pick_hit_first_load").into();
            return;
        };
        let Some(f) = self.fd_hits.get(fi).cloned() else { return };
        let at = f.hits.get(hi).map(|h| h.at).unwrap_or(0);
        if self.dirty {
            self.status = ui::t!("open_document_unsaved_changes").into();
            return;
        }
        self.open(f.path.clone());
        // 開いた文書の中のその位置へ(素の字なら当たりの位置がそのまま効く)
        let n = self.ed.text().len();
        self.ed.move_to(at.min(n), false);
        self.tab = self.prev_tab.max(1);
        self.relayout_keep();
    }

    /// **リボンのボタンの場所を書き出す**(実機の点検のためだけ)。
    ///
    /// 環境変数 `OFFICEWORK_UI_DUMP` が指すファイルへ JSON を1つ。
    /// **設定していなければ何もしない** — 網も socket も開けない。
    /// calc の rpc `{"cmd":"ribbon"}` に当たるものを、writer には受け口が
    /// 無いのでファイルで渡す(2026-08-16。座標を目分量で当てて3回外し、
    /// 外した拍子に発注者の打鍵まで拾った)。
    pub fn dump_ui(&self) {
        let Some(path) = std::env::var_os("OFFICEWORK_UI_DUMP") else { return };
        let boxes: Vec<String> = self
            .btn_box
            .borrow()
            .iter()
            .map(|(id, (x, y, w, h))| {
                format!("{{\"id\":\"{id}\",\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}")
            })
            .collect();
        // プロジェクトパネルの木(見える行の数と、選ばれている径路)。
        // 画が古くても、reveal が効いたかはここで分かる
        let fl_sel = self
            .fl_tree
            .selected
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let body = format!(
            "{{\"tab\":{},\"native\":{},\"rp_open\":{},\"rp_tab\":{},\"rp_drawn\":{},\"file_view\":{},\"win_w\":{},\"win_h\":{},\"fd_files\":{},\"fd_hits\":{},\"sel\":[{},{}],\"fd_boxes\":[{}],\"status\":{:?},\"boxes\":[{}],\"fl_rows\":{},\"fl_sel\":{fl_sel:?}}}",
            self.tab,
            self.native,
            self.rp_open,
            self.rp_tab,
            self.rp_drawn.get(),
            self.file_view,
            self.win_wh.get().0,
            self.win_wh.get().1,
            self.fd_hits.len(),
            self.fd_tally.hits,
            self.ed.selection().start,
            self.ed.selection().end,
            self
                .fd_box
                .borrow()
                .iter()
                .map(|(f, h, x, y, w, hh)| {
                    format!("{{\"f\":{f},\"h\":{h},\"x\":{x},\"y\":{y},\"w\":{w},\"hh\":{hh}}}")
                })
                .collect::<Vec<_>>()
                .join(","),
            self.status.to_string(),
            boxes.join(","),
            self.fl_tree.rows().len()
        );
        // 同じ中身なら書かない(毎フレーム書くのは無駄)
        if *self.ui_dump_last.borrow() == body {
            return;
        }
        *self.ui_dump_last.borrow_mut() = body.clone();
        let _ = std::fs::write(path, body);
    }

    /// **adoc 形式にする** — docx を本文(.adoc)と書式(.toml)に分けます。
    ///
    /// 書式は無くなりません。同じ見た目の所をまとめて名前を付け、書式の
    /// ファイルへ移します。「意味だけにする」と呼んでいましたが、書式を
    /// 捨てるように読めるので改めました(2026-08-17 発注者)。
    /// (SEKKEI 段階D。2026-08-16)。
    ///
    /// **非可逆なので明示の1手**。押すまでは docx のまま扱い、原本据え置きの
    /// 資産(読めなかった部品・節・変更履歴)を守る。押した後は
    /// ネイティブになり、保存先は .adoc になる。
    pub(crate) fn distill_now(&mut self) {
        if self.native {
            self.status = ui::t!("document_already_adoc_form").into();
            return;
        }
        self.switch_target(Target::Body);
        self.flush_target();
        self.checkpoint(false);
        let (doc, th, rep) = kumihan::distill::distill(&self.doc);
        self.tmpl = th;
        self.native = true;
        // テンプレートの名前は文書の名前から(隣に .toml で置ける形)
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| ui::t!("documents_template").to_string());
        let mut doc = doc;
        doc.template = Some(name.clone());
        self.pg = self.tmpl.page.unwrap_or_default();
        self.set_doc(doc);
        self.adopt_font();
        self.dirty = true;
        // **落ちた物を数えて言う。** 「何も失っていない」と嘘をつかない
        let mut says = if rep.dropped == 0 {
            ui::tf!(
                "converted_adoc_formats_moved",
                rep.styles.to_string(),
                rep.paragraphs.to_string()
            )
        } else {
            ui::tf!(
                "converted_adoc_formats_moved_into",
                rep.styles.to_string(),
                rep.paragraphs.to_string(),
                rep.dropped.to_string()
            )
        }
        .to_string();
        // **書式と本文を2つのファイルに書き、保存先を .adoc に移します**
        // (2026-09-02)。前はどちらも書かず、保存先も docx のままだったので、
        // Ctrl+S が元の docx を上書きし、.adoc を開き直すと「テンプレートが
        // 見つからない」になっていました
        if let Some(docx_at) = self.path.clone() {
            let toml_at = docx_at.with_file_name(format!("{name}.toml"));
            let adoc_at = docx_at.with_extension("adoc");
            match std::fs::write(&toml_at, kumihan::theme::write(&self.tmpl)) {
                Ok(()) => {
                    self.tmpl_path = Some(toml_at.clone());
                    // 元の docx の側のロックを外し、.adoc の側を取ります
                    self.release_lock();
                    self.acquire_lock(&adoc_at);
                    self.path = Some(adoc_at.clone());
                    let toml_name = toml_at.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let adoc_name = adoc_at.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if adoc_at.exists() {
                        // 既にある .adoc を黙って潰しません。保存で上書きになることを言います
                        says = format!(
                            "{says}。{}",
                            ui::tf!("template_written_adoc_exists", toml_name, adoc_name)
                        );
                    } else {
                        match self.save_adoc_to(&adoc_at) {
                            Ok(()) => {
                                self.dirty = false;
                                Self::note_recent(&adoc_at);
                                says = format!(
                                    "{says}。{}",
                                    ui::tf!("wrote_template_and_body", toml_name, adoc_name)
                                );
                            }
                            Err(e) => says = format!("{says}。{}", ui::tf!("cant_save", e)),
                        }
                    }
                }
                Err(e) => says = format!("{says}。{}", ui::tf!("cant_write_template", e)),
            }
        } else {
            says = format!("{says}。{}", ui::t!("not_file_yet_save_first"));
        }
        self.relayout();
        self.status = says.into();
    }

    /// **見た目を直に変える操作**(ネイティブでは封じる)。
    ///
    /// 意味の側(強調・上付き・下付き・見出し・引用・リスト)は封じない —
    /// AsciiDoc に落ちるので保存で残る。ここに並ぶのは**落ちる物**で、
    /// 直に掛けても保存で消える。だから掛けさせず、名前を付けさせる
    /// (発注者 2026-08-16「直接書式は原則封じる」)。
    pub(crate) const LOOK_IDS: &'static [&'static str] = &[
        "fontname",
        "fontsize",
        "incfont",
        "decfont",
        "fontcolor",
        "highlight",
        "underline",
        "strikeout",
    ];

    /// ネイティブ文書で見た目の操作が来たら遮り、スタイルの新設へ誘導する。
    /// 返りが true なら、呼んだ側は普通の処理をしない
    pub(crate) fn look_guard(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        if !self.native || !Self::LOOK_IDS.contains(&id) {
            return false;
        }
        // **見た目のボタンは、スタイルの一覧へ案内します**(2026-08-17)。
        //
        // 前は押すたびに「名前を付けてください」と聞いていました。すると
        //
        // - 同じ見た目を使い回せず、押すたびにスタイルが増える
        // - 外す方法が無い
        //
        // という状態でした。外す道として「もう一度押したら脱ぐ」を足しかけ
        // ましたが、それも筋が通りません。色のボタンを押しただけで、色以外も
        // 持っているスタイルごと外れてしまうためです(発注者の指摘)。
        //
        // 意味だけの文書で「下線」を押す人が本当にしたいのは、その字に役割を
        // 与えることです。だから一覧を出して、選ぶか・外すか・新しく作るかを
        // その場で決められるようにします。太字と斜体は意味(強調)なので、
        // ここには来ません(そのまま効きます)。
        self.rp_open = true;
        self.rp_tab = 2;
        self.status = ui::t!(
            "adoc_form_format_given"
        )
        .into();
        cx.notify();
        true
    }

    /// テンプレートを書く先のフォルダ。開いている文書の隣です。
    ///
    /// まだ保存していない文書には隣が無いので、開いている綴りを使います。
    fn template_dir(&self) -> Option<PathBuf> {
        self.path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .or_else(|| self.folder())
    }

    /// いま着ているテンプレートを、ファイルから読み直して着ます。
    ///
    /// テンプレートを書いた後に呼びます。書いた字がそのまま画面に効く
    /// ことを、読み直しで確かめる形です。
    fn reload_template(&mut self) {
        let path = self
            .path
            .clone()
            .or_else(|| self.template_dir().map(|d| d.join("まだ保存していない.adoc")));
        let Some(path) = path else { return };
        let (th, tp, _) = self.load_template(self.doc.template.as_deref(), &path);
        self.tmpl = th;
        self.tmpl_path = tp;
        self.pg = self.tmpl.page.unwrap_or(self.pg);
        self.adopt_font();
        self.relayout_keep();
    }

    /// **テンプレートを直す**(2026-09-02 発注者「書く処理を実装する」)。
    ///
    /// 書き先は手引き「配られたテンプレートは書き替わりません」の決めの
    /// とおりです。開いている文書の隣にあるテンプレートならその場で
    /// 書き替え、元が別の場所(この機械の標準・置き場・同梱の既定)なら
    /// 文書の隣に写しを作って、そちらに書きます。
    ///
    /// 返りは、写しを作ったときにその旨を言う字です。書けなかったときは
    /// Err で理由を返します。書けたときは読み直して画面に効かせます。
    fn edit_template(
        &mut self,
        f: impl FnOnce(&str) -> String,
    ) -> Result<(PathBuf, Option<String>), String> {
        if !self.native {
            return Err(ui::t!("adoc_form_format_given").to_string());
        }
        let Some(dir) = self.template_dir() else {
            return Err(ui::t!("not_file_yet_save_first").to_string());
        };
        let user_at = ui::settings::dir().join(kumihan::theme::user_template_name());
        let user_at = user_at.exists().then_some(user_at);
        let target = kumihan::theme::write_target(
            &dir,
            self.doc.template.as_deref(),
            self.tmpl_path.as_deref(),
            user_at.as_deref(),
        );
        kumihan::theme::rewrite(&target, f)?;
        let copied = match &target.origin {
            kumihan::theme::Origin::InPlace => None,
            kumihan::theme::Origin::CopyOf(p) => Some(
                ui::tf!("made_copy_for_document", target.at.display(), p.display()).to_string(),
            ),
            kumihan::theme::Origin::CopyOfBuiltIn => Some(
                ui::tf!("made_copy_for_document", target.at.display(), ui::t!("built_default"))
                    .to_string(),
            ),
        };
        self.reload_template();
        Ok((target.at, copied))
    }

    /// いまの段落が着ているスタイルの名前。名指しが無ければ役割の名前です。
    pub(crate) fn wearing_style(&self) -> String {
        let (pi, _) = self.cursor_para();
        self.doc
            .paragraphs()
            .nth(pi)
            .and_then(|p| {
                p.style_id
                    .clone()
                    .or_else(|| kumihan::theme::Theme::role_name(p.style).map(|s| s.to_string()))
            })
            .unwrap_or_else(|| "本文".to_string())
    }

    /// 選んだ字が着ている文字スタイル。選んでいなければ None です。
    ///
    /// 選択の頭の run の名前を見ます。選択の中で名前が混ざっていても、
    /// 頭の分を出します。
    pub(crate) fn selected_char_style(&self) -> Option<String> {
        let sel = self.ed.selection();
        if sel.start == sel.end {
            return None;
        }
        let mut at = 0usize;
        for p in self.doc.paragraphs() {
            for r in &p.runs {
                let end = at + r.text.len();
                if sel.start < end {
                    return r.fmt.style_id.clone();
                }
                at = end;
            }
            at += 1;
        }
        None
    }

    /// 役割で出る固定の名前か(本文・表題・見出し・引用)。
    ///
    /// これらは段落の役割そのものなので、名前を変えられません。
    pub(crate) fn is_role_style(name: &str) -> bool {
        matches!(
            name,
            "本文" | "表題" | "見出し1" | "見出し2" | "見出し3" | "見出し4" | "見出し5" | "引用"
        )
    }

    /// テンプレートが持っているスタイルの定義。無ければ名前だけの空の定義です。
    fn style_def_now(&self, name: &str) -> kumihan::theme::StyleDef {
        self.tmpl
            .style(name)
            .cloned()
            .unwrap_or_else(|| kumihan::theme::StyleDef { name: name.to_string(), ..Default::default() })
    }

    /// **スタイルの定義を直してテンプレートに書く**(右パネル)。
    ///
    /// 直るのはテンプレートなので、同じスタイルの所が一度に変わります。
    /// 写しを作ったときは状態行でそう言います。
    pub(crate) fn edit_style(&mut self, name: &str, f: impl FnOnce(&mut kumihan::theme::StyleDef)) {
        let mut def = self.style_def_now(name);
        f(&mut def);
        match self.edit_template(|src| kumihan::theme::put_style(src, &def)) {
            Ok((at, copied)) => {
                let mut says = ui::tf!("style_written_to", name.to_string(), at.display()).to_string();
                if let Some(c) = copied {
                    says = format!("{says}。{c}");
                }
                self.status = says.into();
            }
            Err(e) => self.status = ui::tf!("cant_write_template", e).into(),
        }
    }

    /// **いま着ているスタイルの字の大きさを1段動かす**(右パネル)。
    ///
    /// 大きさを持っていないスタイルは、いま効いている大きさから数えます。
    pub(crate) fn tweak_style(&mut self, step: i32) {
        let name = self.wearing_style();
        let base = self.base_pt();
        self.edit_style(&name, |d| {
            let now = d.size_pt.unwrap_or(base);
            d.size_pt = Some((now + step as f32).clamp(4.0, 200.0));
        });
    }

    /// 太字・斜体・下線を切り替える(右パネル)。`which` は "bold" / "italic" / "underline"。
    pub(crate) fn toggle_style_flag(&mut self, which: &str) {
        let name = self.wearing_style();
        let which = which.to_string();
        self.edit_style(&name, |d| match which.as_str() {
            "bold" => d.bold = !d.bold,
            "italic" => d.italic = !d.italic,
            "underline" => d.underline = !d.underline,
            _ => {}
        });
    }

    /// 揃えを決める(右パネル)。
    pub(crate) fn set_style_align(&mut self, a: kumihan::Align) {
        let name = self.wearing_style();
        self.edit_style(&name, |d| d.align = Some(a));
    }

    /// 行間を 0.25 ずつ動かす(右パネル)。1.0 より下には行きません。
    pub(crate) fn tweak_style_line_spacing(&mut self, step: i32) {
        let name = self.wearing_style();
        self.edit_style(&name, |d| {
            let now = d.line_spacing.unwrap_or(1.0);
            d.line_spacing = Some(((now + step as f32 * 0.25) * 100.0).round() / 100.0)
                .filter(|v| *v > 1.0);
        });
    }

    /// 段落の後の空きを 2pt ずつ動かす(右パネル)。0 より下には行きません。
    pub(crate) fn tweak_style_space_after(&mut self, step: i32) {
        let name = self.wearing_style();
        self.edit_style(&name, |d| {
            d.space_after_pt = (d.space_after_pt + step as f32 * 2.0).max(0.0);
        });
    }

    /// **新しく作る**の入り口(右パネル)。名前の欄を開きます。
    ///
    /// 中身はいま着ているスタイルの写しから始めます。名前を決めると
    /// テンプレートに入り、選んだ所がそのスタイルを着ます。
    pub(crate) fn style_new_start(&mut self) {
        let wearing = self.wearing_style();
        let mut def = self.style_def_now(&wearing);
        // 名前が空 = 新設。名前があれば「名前を変える」の途中です
        def.name = String::new();
        self.style_new = Some(def);
        self.style_ed = Editor::new("");
        self.find_open = false;
        self.status = ui::t!("new_style_give_look").into();
    }

    /// **名前を変える**の入り口(右パネル)。いまの名前を欄に入れて開きます。
    ///
    /// 役割の名前(本文・見出し1 など)は段落の役割そのものなので変えられません。
    pub(crate) fn style_rename_start(&mut self) {
        let wearing = self.selected_char_style().unwrap_or_else(|| self.wearing_style());
        if Self::is_role_style(&wearing) {
            self.status = ui::tf!("role_style_cannot_rename", wearing).into();
            return;
        }
        self.style_new = Some(kumihan::theme::StyleDef { name: wearing.clone(), ..Default::default() });
        self.style_ed = Editor::new(&wearing);
        self.find_open = false;
        self.status = ui::tf!("rename_style_give_new_name", wearing).into();
    }

    /// スタイルの名前を決める(名前の欄で Enter)。
    ///
    /// 名前の欄が「新しく作る」から開いていれば、その名前のスタイルを
    /// テンプレートに書き、選んでいる所に掛けます。「名前を変える」から
    /// 開いていれば、テンプレートの節と本文の名指しを新しい名前にします。
    pub(crate) fn style_commit(&mut self) {
        let Some(pending) = self.style_new.take() else { return };
        let name = self.style_ed.text().trim().to_string();
        if name.is_empty() {
            self.status = ui::t!("name_empty_cancelled").into();
            return;
        }
        if !pending.name.is_empty() {
            self.style_rename_commit(&pending.name, &name);
            return;
        }
        self.checkpoint(false);
        // **選んでいれば字に、選んでいなければ段落に**(2026-08-16)。
        // 語を1つ選んで見た目を変えようとしたのに段落ぜんぶが変わる、では
        // 直接書式の手軽さに勝てません。選択の有無が意図そのものです
        self.switch_target(Target::Body);
        self.flush_target();
        let sel = self.ed.selection();
        let text = sel.start != sel.end;
        let n = name.clone();
        if !text {
            self.doc.apply_para(sel, |p| p.style_id = Some(n.clone()));
        } else {
            self.doc.apply_char_format(sel, |f| f.style_id = Some(n.clone()));
        }
        self.dirty = true;
        // **同じ名前が既にあれば、そのまま着ます**(定義は書き替えません)。
        // 無ければ、開いたときの見た目を写した定義をテンプレートに書きます
        let notes = if self.tmpl.style(&name).is_some() {
            self.relayout_keep();
            ui::t!("format_template").to_string()
        } else {
            let def = kumihan::theme::StyleDef { name: name.clone(), ..pending };
            match self.edit_template(|src| kumihan::theme::put_style(src, &def)) {
                Ok((at, copied)) => {
                    let mut says = ui::tf!("style_written_to", name.clone(), at.display()).to_string();
                    if let Some(c) = copied {
                        says = format!("{says}。{c}");
                    }
                    says
                }
                Err(e) => {
                    self.relayout_keep();
                    ui::tf!("cant_write_template", e).to_string()
                }
            }
        };
        self.status = if text {
            ui::tf!("selected_text_now_uses", name, notes)
        } else {
            ui::tf!("paragraph_now_uses", name, notes)
        }
        .into();
    }

    /// 名前を変える。テンプレートの節と、本文の中でその名前を指している
    /// 段落と字の両方を、新しい名前にします。
    fn style_rename_commit(&mut self, from: &str, to: &str) {
        if from == to {
            return;
        }
        if Self::is_role_style(to) || self.tmpl.style(to).is_some() {
            self.status = ui::tf!("style_name_exists", to.to_string()).into();
            return;
        }
        let (old_section, new_section) =
            (kumihan::theme::style_section(from), kumihan::theme::style_section(to));
        let (f, t) = (from.to_string(), to.to_string());
        // テンプレートに節が無いときも本文の名指しだけは変えます
        let had_def = self.tmpl.style(from).is_some();
        let written = if had_def {
            self.edit_template(|src| kumihan::theme::rename_section(src, &old_section, &new_section))
        } else {
            Ok((PathBuf::new(), None))
        };
        match written {
            Ok((_, copied)) => {
                self.checkpoint(false);
                self.switch_target(Target::Body);
                self.flush_target();
                for p in self.doc.paragraphs_mut() {
                    if p.style_id.as_deref() == Some(f.as_str()) {
                        p.style_id = Some(t.clone());
                    }
                    for r in &mut p.runs {
                        if r.fmt.style_id.as_deref() == Some(f.as_str()) {
                            r.fmt.style_id = Some(t.clone());
                        }
                    }
                }
                self.dirty = true;
                self.relayout_keep();
                let mut says = ui::tf!("renamed_2", f, t).to_string();
                if let Some(c) = copied {
                    says = format!("{says}。{c}");
                }
                self.status = says.into();
            }
            Err(e) => self.status = ui::tf!("cant_write_template", e).into(),
        }
    }

    /// **スタイルを着替える**(右パネル。2026-08-16)。役割の名前なら
    /// 段落の役割そのものを替え、そうでなければ `style_id` で名指す
    /// スタイルを外します(選んでいれば字から、選んでいなければ段落から)。
    /// 役割で出る見出しなどは「本文」に戻します。
    pub(crate) fn strip_style(&mut self) {
        self.switch_target(Target::Body);
        self.checkpoint(false);
        self.flush_target();
        let sel = self.ed.selection();
        if sel.start == sel.end {
            self.doc.apply_para(sel, |p| {
                p.style = kumihan::ParaStyle::Body;
                p.style_id = None;
            });
        } else {
            self.doc.apply_char_format(sel, |f| f.style_id = None);
        }
        self.dirty = true;
        self.relayout_keep();
        self.status = ui::t!("style_removed").into();
    }

    /// スタイルを着る(右パネル)。
    ///
    /// 役割の名前(本文・見出し1 など)は段落の役割を替えます。それ以外の
    /// 名前は、**字を選んでいれば字に、選んでいなければ段落に**掛けます。
    /// 字に付いた名前は `[.名前]#字#` として保存されます。
    pub(crate) fn wear_style(&mut self, name: &str) {
        self.switch_target(Target::Body);
        self.checkpoint(false);
        self.flush_target();
        let sel = self.ed.selection();
        let role = match name {
            "本文" => Some(kumihan::ParaStyle::Body),
            "表題" => Some(kumihan::ParaStyle::Title),
            "見出し1" => Some(kumihan::ParaStyle::Heading(1)),
            "見出し2" => Some(kumihan::ParaStyle::Heading(2)),
            "見出し3" => Some(kumihan::ParaStyle::Heading(3)),
            "見出し4" => Some(kumihan::ParaStyle::Heading(4)),
            "見出し5" => Some(kumihan::ParaStyle::Heading(5)),
            "引用" => Some(kumihan::ParaStyle::Quote),
            _ => None,
        };
        let n = name.to_string();
        let text = sel.start != sel.end && role.is_none();
        if text {
            self.doc.apply_char_format(sel, |f| f.style_id = Some(n.clone()));
        } else {
            self.doc.apply_para(sel, |p| match role {
                // 役割で出る名前は、役割の側で持つ(二重に名乗らない)
                Some(r) => {
                    p.style = r;
                    p.style_id = None;
                }
                None => p.style_id = Some(n.clone()),
            });
        }
        self.dirty = true;
        self.relayout_keep();
        self.status = if text {
            ui::tf!("selected_text_now_uses", name.to_string(), ui::t!("format_template"))
        } else {
            ui::tf!("now_using", name.to_string())
        }
        .into();
    }

    /// 書き出し先ごとのテンプレート(`テンプレート-印刷.toml` など)が
    /// 壊れていれば、その理由を返します。無い・読めるなら None です。
    ///
    /// 開いたときに状態行で言うために使います。黙って `テンプレート.toml`
    /// に落ちると、置いた人には「効かない」としか分かりません。
    pub(crate) fn purpose_template_error(&self, purpose_of: &str) -> Option<String> {
        let dir = self.template_dir()?;
        let at = dir.join(kumihan::theme::purpose_template_name(purpose_of));
        let src = std::fs::read_to_string(&at).ok()?;
        kumihan::theme::parse(&src)
            .err()
            .map(|e| ui::tf!("not_read_used", at.display(), Self::FOLDER_TEMPLATE, e).to_string())
    }

    /// 印刷用のテンプレートが持つ**ページの飾り**。
    ///
    /// 返りは(ヘッダー, フッター, 透かし, ページの色)です。印刷用の
    /// テンプレートが無ければ None で、そのときは画面の飾りが紙にも出ます。
    /// PDF を書く側(`write_pdf`)がこれを見て、画面の飾りの代わりに使います。
    pub(crate) fn print_dress(
        &self,
    ) -> Option<((kumihan::HeadFoot, kumihan::HeadFoot), (Option<String>, Option<String>))> {
        let (th, used) = self.template_for("印刷");
        used?;
        let mut deco = kumihan::Document::default();
        kumihan::theme::compose_page(&mut deco, &th);
        Some(((deco.header, deco.footer), (deco.watermark, deco.page_color)))
    }

    /// ネイティブ文書として保存する(.adoc)。**意味だけを書く**
    /// **筆(手描きの線)を SVG の絵にして本文に置きます**(2026-08-18)。
    ///
    /// ネイティブ文書(.adoc)は手描きの線を持てません。前は保存で黙って
    /// 消えていました。いまは、そのページの線をまとめて1枚の SVG にし、
    /// `image::` の段落としてそのページの先頭の段落の後ろに入れます。
    /// 独自の書き方を1つも足さずに済み、HTML にも PDF にも docx にも
    /// 画像として乗り、後から段落ごと消せます。
    ///
    /// **紙の上の位置は残りません。** 線は本文の流れの中の絵になります。
    /// 返りは作った絵の枚数(呼ぶ側が状態行で言います)。
    fn ink_to_images(&mut self, dir: &std::path::Path) -> Result<usize, String> {
        if self.doc.ink.is_empty() {
            return Ok(0);
        }
        let (_, para_block_idx) = self.page_head_paras(&self.doc.clone());
        // ページごとにまとめる。線の順は引いた順のまま
        let mut per_page: std::collections::BTreeMap<usize, Vec<kumihan::Stroke>> =
            Default::default();
        for st in std::mem::take(&mut self.doc.ink) {
            per_page.entry(st.page).or_default().push(st);
        }
        // すでに使われている名前を避ける
        let mut used: Vec<String> = Vec::new();
        for b in &self.doc.blocks {
            if let kumihan::Block::Para(p) = b {
                for im in p.images.iter().chain(p.images_new.iter()) {
                    if let Some(s) = &im.src {
                        used.push(s.clone());
                    }
                }
            }
        }
        // **後ろのページから入れます。** 前から入れるとブロックの番号がずれます
        let mut sheet = 0usize;
        for (page, strokes) in per_page.iter().rev() {
            let refs: Vec<&kumihan::Stroke> = strokes.iter().collect();
            let Some((svg, w_mm, h_mm)) = kumihan::strokes_to_svg(&refs) else { continue };
            // 画面に出すための写し(SVG のままでは gpui が描けません)
            let png = match ui::svg_to_png(svg.as_bytes(), 3.0) {
                Ok((png, _, _)) => png,
                Err(e) => return Err(e),
            };
            let mut n = page + 1;
            let rel = loop {
                let rel = format!("images/筆{n}.svg");
                if !used.contains(&rel) {
                    break rel;
                }
                n += 1;
            };
            used.push(rel.clone());
            let to = dir.join(&rel);
            if let Some(d) = to.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            std::fs::write(&to, svg.as_bytes()).map_err(|e| e.to_string())?;
            let im = kumihan::InlineImage {
                bytes: std::sync::Arc::new(png),
                w_mm,
                h_mm,
                tex: None,
                src: Some(rel), // 実体はもう置いたので、名前付けの対象にしない
                off: 0,
            };
            let para = kumihan::Block::Para(kumihan::Paragraph {
                images_new: vec![im],
                ..Default::default()
            });
            // **線を引いた高さの、すぐ上の段落に付けます。** ページの先頭に
            // 付けると、紙の下に引いた線が本文の頭へ跳びます
            // (2026-08-18 に実機で見つけました)
            let upper_edge = strokes
                .iter()
                .filter_map(|s| s.bbox().map(|b| b.1))
                .fold(f32::INFINITY, f32::min);
            let oy = self
                .page_offsets
                .get(*page)
                .copied()
                .unwrap_or(*page as f32 * self.pg.h_mm);
            let pi = self.para_at_y(upper_edge + oy);
            match para_block_idx.get(pi).copied() {
                Some(bi) if bi < self.doc.blocks.len() => {
                    self.doc.blocks.insert(bi + 1, para)
                }
                _ => self.doc.blocks.push(para),
            }
            sheet += 1;
        }
        Ok(sheet)
    }

    pub(crate) fn save_adoc_to(&mut self, p: &std::path::Path) -> Result<(), String> {
        self.flush_target();
        // **筆は先に絵にします。** 画像の名前付けより前にやらないと、
        // 作った絵が名前の無い画像として二重に扱われます
        let dir0 = p.parent().unwrap_or(std::path::Path::new("."));
        self.ink_svg_count = self.ink_to_images(dir0)?;
        // **画像に径路を与えてから書きます。** adoc は画像を `image::径路[]` で
        // 指すので、径路の無い画像(docx 由来・画面から挿した物)は書けません。
        // 名前を付けて本文の隣に置きます
        let image = kumihan::adoc::assign_image_paths(&mut self.doc);
        let dir = p.parent().unwrap_or(std::path::Path::new("."));
        for (rel, bytes) in &image {
            let to = dir.join(rel);
            if let Some(d) = to.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            std::fs::write(&to, bytes.as_slice()).map_err(|e| e.to_string())?;
        }
        // **文書が何枚も入っていれば、全部書きます**(2026-08-19)。
        // いま見ている物を控えへ戻した並びを渡します
        let text = if self.docs.len() > 1 {
            kumihan::adoc::write_many(&self.docs_for_save())
        } else {
            kumihan::adoc::write(&self.doc)
        };
        std::fs::write(p, text).map_err(|e| e.to_string())
    }

    /// **自動復旧の控えを書く**(2026-08-21 の B-3)。
    ///
    /// 中身を写してから別スレッドで書きます — 長い文書で画面が止まらない
    /// ように。うまくいったことは言いません(数分ごとに出ては邪魔なので、
    /// しくじったときだけ言います)。
    ///
    /// **原本は触りません。** 落ちたときに失う分を減らすための別の控えです。
    pub(crate) fn write_recover(&mut self, cx: &mut Context<Self>) {
        self.flush_target();
        let dst = crate::io::backup_path(self.path.as_deref());
        // 何枚も入っているファイルなら全部。保存と同じ形にします
        let text = if self.docs.len() > 1 {
            kumihan::adoc::write_many(&self.docs_for_save())
        } else {
            kumihan::adoc::write(&self.doc)
        };
        let orig = self.path.clone();
        let task = cx.background_executor().spawn(async move {
            if let Some(d) = dst.parent() {
                std::fs::create_dir_all(d).ok()?;
            }
            std::fs::write(&dst, text).ok()?;
            ops::note_recover_origin(&dst, orig.as_deref());
            Some(())
        });
        cx.spawn(async move |this, cx| {
            let ok = task.await.is_some();
            let _ = this.update(cx, |w: &mut Writer, _| {
                w.recover_at = std::time::Instant::now();
                if !ok {
                    // **黙って諦めない。** 控えが取れていないことは言う
                    w.status =
                        ui::t!("cant_write_auto_recovery")
                            .into();
                }
            });
        })
        .detach();
    }

    /// 無事に保存できたら控えは要りません(消し忘れると次の起動で
    /// 「落ちた後です」と嘘を言います)
    pub(crate) fn drop_recover(&self) {
        ops::drop_recover(self.path.as_deref(), "adoc", "未保存の文書");
    }

    /// 素の文字として保存する(.py / .txt / .md)。段落を改行でつなぐ
    pub(crate) fn save_text_to(&mut self, p: &std::path::Path) -> Result<(), String> {
        self.flush_target();
        let text = self.doc.body_text();
        // 末尾の改行は残す(POSIX の作法。git の差分が汚れない)
        let text = if text.ends_with('\n') { text } else { format!("{text}\n") };
        std::fs::write(p, text).map_err(|e| e.to_string())
    }

    pub(crate) fn open_plain(&mut self, p: PathBuf, bytes: Vec<u8>) {
        self.target = Target::Body;
        // **docx を開いたら docx の扱いに戻します。** 新しい文書は adoc 形式で
        // 始まるので、ここで戻さないと docx を開いても adoc のままになり、
        // 保存で書式が本文から消えます(2026-08-17、adoc から始める形にして
        // 見つかった)
        self.native = false;
        // 前の文書のパネルが残っていると、打鍵が新しい文書のヘッダーを潰す
        self.hf_edit = None;
        self.track = false;
        self.track_base = None;
        // 前の文書のパスワードを引きずらない(暗号化して開いた時だけ
        // pw_commit が後から入れ直す)
        self.encrypt_pw = None;
        match ooxml::read(std::io::Cursor::new(bytes)) {
            Ok((doc, rep)) => {
                self.notes = rep
                    .unsupported
                    .iter()
                    .map(|(n, c)| SharedString::from(format!("{n} × {c}")))
                    .collect();
                self.status = ui::tf!("paragraphs_tables", rep.paragraphs, doc.tables().count(), p.file_name().unwrap_or_default().to_string_lossy())
                .into();
                self.pg = doc.page.unwrap_or_default();
                self.set_doc(doc);
                self.adopt_font();
                self.relayout_keep();
                // 排他(共有フォルダの「後勝ちで潰す」を防ぐ。calc と同じ)
                self.acquire_lock(&p);
                if let Some(who) = self.locked_by.clone() {
                    self.status = ui::tf!("open_overwrite_save_blocked", self.status, who)
                    .into();
                }
                if self.doc.protection.is_some() {
                    self.status = ui::tf!("protected_read_only_protection_tab", self.status)
                    .into();
                }
                Self::note_recent(&p);
                self.path = Some(p);
                self.dirty = false;
            }
            Err(e) => self.status = ui::tf!("cant_open", e).into(),
        }
    }

    /// 保存。名前が無ければ選ばせる(**ダイアログは別のスレッド** — rfd は同期で、
    /// メインスレッドで開くと画面ごと固まる。calc と同じ作法)。
    /// `then_quit` なら保存が済んだときだけ終了する — 書きかけを黙って捨てない。
    pub(crate) fn save(&mut self, then_quit: bool, cx: &mut Context<Self>) {
        if let Some(p) = self.path.clone() {
            if self.locked_by.is_none() {
                self.save_to(p);
                if then_quit && !self.dirty {
                    self.release_lock();
                    cx.quit();
                }
                return;
            }
            // 先客の作業を後勝ちで潰さない。別の名前でなら保存できる
            self.status = ui::tf!("open_no_overwrite_saving", self.locked_by.as_deref().unwrap_or(ui::t!("someone")))
            .into();
        }
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .add_filter(ui::t!("officework_document"), &["adoc"])
                .add_filter(ui::t!("word_document"), &["docx"])
                .save_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Some(p) => {
                        this.save_to(p);
                        if then_quit && !this.dirty {
                            this.release_lock();
                            cx.quit();
                        }
                    }
                    None => this.status = ui::t!("save_cancelled_no_name").into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn save_to(&mut self, p: PathBuf) {
        // **ネイティブ文書(.adoc)は意味だけを返す**(2026-08-16)。
        // 見た目はテンプレートが持っているので、書くものは何も無い
        if p.extension().and_then(|e| e.to_str()).is_some_and(is_native_ext) {
            // **消える物を先に数えます。** 書いた後では、何が消えたか分かりません
            let dropping = kumihan::adoc::dropped(&self.doc);
            match self.save_adoc_to(&p) {
                Ok(()) => {
                    self.path = Some(p.clone());
                    self.native = true;
                    self.dirty = false;
                    // **保存した先のフォルダに書式のファイルがあれば、それを着ます。**
                    // フォルダの書式はそのフォルダの文書に効く、という決まりなので、
                    // 入れた文書だけ違う見た目のままだと辻褄が合いません
                    let restyled = self.adopt_folder_template(&p);
                    let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    // **筆を絵にしたら、画面を組み直します。** 模型では
                    // 段落が1つ増えているので、組み直さないと線も絵も
                    // 見えなくなります(2026-08-18 に実機で見つけました)
                    if self.ink_svg_count > 0 {
                        self.relayout();
                    }
                    // **必ず言います。** 紙の上の位置は残らず、本文の流れの
                    // 中の絵になるので、黙ると「消えた」に見えます
                    self.status = if self.ink_svg_count > 0 {
                        ui::tf!("saved_strokes_became_svg",
                                name, self.ink_svg_count).into()
                    } else if let Some(at) = restyled {
                        ui::tf!("saved_uses_folders_format", name, at).into()
                    } else if dropping.is_empty() {
                        ui::tf!("saved_text_only_formatting", name).into()
                    } else {
                        // **黙って捨てません。** adoc は意味だけを持つので、
                        // ページの飾りと直接書式はここで消えます
                        ui::tf!("saved_cannot_kept_format",
                                name, dropping.join("・")).into()
                    };
                }
                Err(e) => self.status = ui::tf!("cant_save", e).into(),
            }
            return;
        }
        // 素の文字(.py / .txt / .md)は素のまま返す — docx に化けさせない
        if p.extension().and_then(|e| e.to_str()).is_some_and(is_plain_ext) {
            match self.save_text_to(&p) {
                Ok(()) => {
                    self.path = Some(p.clone());
                    self.dirty = false;
                    self.drop_recover();
                    self.status = ui::tf!(
                        "saved_plain_text",
                        p.file_name().unwrap_or_default().to_string_lossy()
                    )
                    .into();
                }
                Err(e) => self.status = ui::tf!("cant_save", e).into(),
            }
            return;
        }
        self.flush_target();
        // 元のファイルの部品(画像・スタイル・ヘッダー等)を持ち越す。
        // 上書き保存では読み終えてから書く(同じファイルを同時に開かない)
        let original: Option<std::io::Cursor<Vec<u8>>> =
            self.original_plain().map(std::io::Cursor::new);
        // **ネイティブ文書(.adoc)を docx で書き出すときは、テンプレートを
        // 通します**(2026-08-18)。本文は意味だけ、見た目は styles.xml、と
        // いう分け方をそのまま docx に移す。`テンプレート-docx.toml` が
        // あればそちらを使う(書き出し先ごとの書式)。
        // 受け取った docx を保存し直すときは渡さない — 相手のスタイル定義を
        // こちらのテンプレートで上書きしないため
        let (docx_tmpl, tmpl_at) = if self.native {
            let (th, at) = self.template_for("docx");
            (Some(th), at)
        } else {
            (None, None)
        };
        let doc_out = self.doc_for_save(docx_tmpl.as_ref());
        // バージョン履歴: 上書きの前に、いままでの中身を控えとして残す
        if p.exists() {
            self.keep_version(&p);
        }
        let saved = if let Some(pw) = self.encrypt_pw.clone() {
            // 暗号化は zip 丸ごとが単位 — 一度メモリへ書いてから包む
            let mut plain = Vec::new();
            ooxml::write_with_theme(
                &doc_out, original, docx_tmpl.as_ref(), std::io::Cursor::new(&mut plain))
                .and_then(|_| ooxml::crypt::encrypt(&plain, &pw))
                .and_then(|enc| {
                    kumihan::atomic::save(&p, |mut f| {
                        use std::io::Write as _;
                        f.write_all(&enc).map_err(|e| e.to_string())
                    })
                })
        } else {
            kumihan::atomic::save(&p, |f| {
                ooxml::write_with_theme(
                    &doc_out, original, docx_tmpl.as_ref(), std::io::BufWriter::new(f))
            })
        };
        match saved {
            Ok(_) => {
                let caveat = if self.notes.is_empty() {
                    ""
                } else {
                    // 読めなかった要素は本文から消えている。黙って保存しない
                    ui::t!("elements_not_read_not")
                };
                let enc_note =
                    if self.encrypt_pw.is_some() { ui::t!("encrypted") } else { "" };
                let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                // **ネイティブ文書から作った docx は、渡すための形です。**
                // 見た目はスタイル定義の側に入っているので、この writer で
                // 開き直しても直接書式は付いていません。そこを黙らない
                self.status = if docx_tmpl.is_some() {
                    let from = tmpl_at.unwrap_or_else(|| ui::t!("format_use").to_string());
                    ui::tf!("exported_turned_into_style",
                            name, from).into()
                } else {
                    ui::tf!("saved", name, enc_note, caveat).into()
                };
                // **ネイティブ文書からの docx は「書き出し」です。**
                // 原稿は adoc の側にあるので、保存先を docx へ移しません —
                // 移すと、次の Ctrl+S が adoc ではなく docx を上書きします
                if docx_tmpl.is_none() {
                    // 保存先のロックを取り直す(別の名前で保存したときは
                    // 新しいファイルの側を守る。同じ名前なら実質そのまま)
                    self.acquire_lock(&p);
                    self.path = Some(p.clone());
                    self.dirty = false;
                    self.drop_recover();
                }
                Self::note_recent(&p);
            }
            Err(e) => self.status = ui::tf!("cant_save", e).into(),
        }
    }

}

/// フォルダの中の、書式のファイルらしい物(.toml)を1つ返す。
///
/// 名前が `テンプレート.toml` でないものを見つけるためだけに使います。
/// 2つ以上あればどれとも決められないので None を返します。
fn other_pattern(dir: &std::path::Path) -> Option<String> {
    let mut hit = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("toml") {
            continue;
        }
        let name = p.file_name()?.to_string_lossy().to_string();
        if hit.is_some() {
            return None; // 2つ以上ある — 選べない
        }
        hit = Some(name);
    }
    hit
}
