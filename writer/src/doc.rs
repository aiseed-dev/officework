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
    pub 組: kumihan::theme::Setting,
    pub view_w_px: f32,
}

impl Look {
    /// 1回ぶんの組み(合成済みの写し → 紙面)。**組みの本体はここ1箇所**。
    fn lay_once(&self, src: &Document, m: &Metrics) -> Page {
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
            let measure = if self.組.fluid {
                // 画面の画素 → mm(紙の幅は使わない)。左右に少し余白を残す
                ((self.view_w_px / crate::PX_PER_MM) - 16.0).max(40.0)
            } else {
                self.pg.column_measure_mm()
            };
            page = layout(
                src,
                m,
                &Frame { measure_mm: measure, line_height_mm: LINE_MM, y0_mm: y0 },
            );
            if !self.組.endless() {
                kumihan::fold_columns(&mut page, &self.pg, y0);
            }
        }
        page
    }
}

impl Writer {
    pub(crate) fn new(path: Option<PathBuf>, cx: &mut Context<Self>) -> Writer {
        let mut w = Writer {
            focus: cx.focus_handle(),
            doc: Document::default(),
            ed: Editor::new(""),
            page: Page::default(),
            path: None,
            status: "".into(),
            notes: Vec::new(),
            dirty: false,
            drag_select: false,
            menu_at: None,
            tab: 0,
            zoom: 1.0,
            scroll_mm: 0.0,
            caret_on: true,
            view_h_px: 800.0,
            target: Target::Body,
            symbols: false,
            show_marks: false,
            ruler: true,
            line_numbers: false,
            show_comments: true,
            font_list: false,
            size_list: false,
            style_list: false,
            dark: false,
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
            find_open: false,
            find_field: 0,
            find_ed: Editor::new(""),
            repl_ed: Editor::new(""),
            hf_edit: None,
            hf_ed: Editor::new(""),
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
            page_starts: vec![f32::NEG_INFINITY],
            page_notes: vec![Vec::new()],
            paged: false,
            page_tops: vec![0.0],
            page_papers: Vec::new(),
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
            ui::t!("下に行を足しました").into()
        } else {
            ui::t!("上に行を足しました").into()
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
            ui::t!("右に列を足しました").into()
        } else {
            ui::t!("左に列を足しました").into()
        };
    }

    /// いまの行を消す。**最後の1行は消さない**(表が消えるのは別の操作)
    pub(crate) fn table_del_row(&mut self) {
        let Some((ti, row, _, rows, _)) = self.cursor_table() else { return };
        if rows <= 1 {
            self.status = ui::t!("最後の1行は消せません(表ごと消すのは別の操作です)").into();
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
        self.status = ui::t!("行を消しました(Ctrl+Z で戻せます)").into();
    }

    /// いまの列を消す。**最後の1列は消さない**
    pub(crate) fn table_del_col(&mut self) {
        let Some((ti, row, col, _, cols)) = self.cursor_table() else { return };
        if cols <= 1 {
            self.status = ui::t!("最後の1列は消せません(表ごと消すのは別の操作です)").into();
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
        self.status = ui::t!("列を消しました(Ctrl+Z で戻せます)").into();
    }

    /// いまの段落の画像を拡げる・縮める(縦横の比は保つ)。
    ///
    /// **数式も画像なので同じ道で効く** — 式の絵だけ大きくしたい、は
    /// 普通の頼み。下限 5mm・上限 400mm(紙より大きくしない)
    pub(crate) fn image_scale(&mut self, k: f32) {
        let (pi, _) = self.cursor_para();
        self.checkpoint(false);
        self.flush_target();
        let mut 触った = 0usize;
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
                触った += 1;
            }
        }
        if 触った == 0 {
            self.status = ui::t!("この段落に絵がありません").into();
            return;
        }
        self.dirty = true;
        self.relayout_keep();
        self.status = if k < 1.0 {
            ui::t!("絵を小さくしました").into()
        } else {
            ui::t!("絵を大きくしました").into()
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
            Target::Body => ui::t!("本文").into(),
            Target::Cell { row, col, .. } => {
                ui::tf!("表のセル({}行 {}列)を編集中", row + 1, col + 1).into()
            }
        };
    }

    pub(crate) fn relayout(&mut self) {
        self.flush_target();
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
        let 組 = if self.native { self.tmpl.setting } else { Default::default() };
        let 姿 = Look { pg: self.pg, vertical: self.doc.vertical, 組, view_w_px: self.view_w_px };
        self.page = 姿.lay_once(composed.as_ref().unwrap_or(&self.doc), &m);
        self.refresh_hf();
        // **跨がない**(発表)。折った結果を見て、境をまたいだ段落があれば
        // 写しにその段落の改ページの印を足し、**折り手に折り直させる**。
        // refresh_hf の後でないと頁の境が分からない
        if 組.keep {
            if let Some(c) = composed.as_mut() {
                self.keep_paragraphs_whole(c, &m, &姿);
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
    fn keep_paragraphs_whole(&mut self, c: &mut Document, m: &Metrics, 姿: &Look) {
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
            self.page = 姿.lay_once(c, m);
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
            self.ink_undo.push(self.doc.ink.clone());
            self.doc.ink.retain(|st| !near(st));
            self.dirty = true;
        }
    }

    /// 保存用の写し。筆(ペン)を、そのページに載っている段落の控えへ
    /// 図形(自由曲線)として差し込む。モデル本体は触らない —
    /// 保存のたびに増えないように、写しに差す。
    pub(crate) fn doc_for_save(&self) -> Document {
        let mut doc = self.doc.clone();
        // 相互参照は保存の写しで計算し直す(docx のキャッシュを新しく保つ。
        // 画面の平文はそのまま — 見えている値の更新は「参照を更新」で)
        doc.refresh_fields(|name, page| self.ref_value(name, page));
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
        let (pages, _) = paper::paginate(&self.page, paper::Paper {
            width_mm: self.pg.w_mm,
            height_mm: self.pg.h_mm,
            margin_mm: self.pg.left_mm,
        });
        // ページ → そのページに最初に載る段落(通し番号)
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
        let pn = paper::paginate_full(&self.page, paper::Paper {
            width_mm: self.pg.w_mm,
            height_mm: self.pg.h_mm,
            margin_mm: self.pg.left_mm,
        });
        self.page_offsets = pn.offsets;
        self.page_starts = pn.starts;
        self.page_notes = pn.notes;
        self.page_papers = pn.papers;
        // 印刷モードは**紙を1枚ずつ積む**。折らないと紙の絵と中身が重なる
        // (頁の間隔は紙の高さより詰まっているため)
        self.page_tops = if self.paged && !self.page.vertical && !self.multipage {
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
        self.header_lines =
            kumihan::layout_hf(&self.doc.header, &m, &self.pg, LINE_MM, 1, total, false,
                               self.doc.base_pt());
        self.footer_lines =
            kumihan::layout_hf(&self.doc.footer, &m, &self.pg, LINE_MM, 1, total, true,
                               self.doc.base_pt());
    }

    /// ヘッダー・フッターの編集のパネルを開く(もう一度で閉じる)。
    pub(crate) fn open_hf(&mut self, footer: bool) {
        if self.hf_edit == Some(footer) {
            self.hf_edit = None;
            return;
        }
        let hf = if footer { &self.doc.footer } else { &self.doc.header };
        let which = if footer { ui::t!("フッター") } else { ui::t!("ヘッダー") };
        if hf.paragraphs.is_empty() && hf.part.is_some() {
            // 読めたが持てなかった部品(表入りなど)。嘘の編集をさせない
            self.status = ui::tf!("この{}には表があり、この版では編集できません(保存では残ります)", which).into();
            return;
        }
        self.find_open = false;
        self.hf_edit = Some(footer);
        self.hf_ed = Editor::new(&kumihan::paras_text(&hf.paragraphs));
        self.status = ui::tf!("{}を編集中(全ページ共通。Esc で閉じる)", which).into();
    }

    /// 文書の書体を実体に結ぶ。無ければ系統を保って代替し、**そう言う**。
    pub(crate) fn adopt_font(&mut self) {
        let wanted = self.doc.font.clone();
        match kumihan::font::for_document(wanted.as_deref()) {
            Ok((fam, exact)) => {
                if let Ok(b) = kumihan::font::load(fam) {
                    self.font_bytes = std::sync::Arc::new(b);
                    self.font_name = SharedString::from(fam.name.clone());
                }
                if !exact {
                    if let Some(w) = &wanted {
                        self.notes.push(
                            ui::tf!("書体「{}」が無いので「{}」で表示", w, fam.name).into(),
                        );
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
                    self.status = ui::tf!("開けません: {}", e).into();
                    return;
                }
            };
            match ooxml::crypt::decrypt(&bytes, &pw) {
                Ok(plain) => {
                    self.pw_open = false;
                    self.open_plain(p.clone(), plain);
                    if self.path.as_deref() == Some(p.as_path()) {
                        self.encrypt_pw = Some(pw);
                        self.status = ui::tf!("{}(保存も同じパスワードで暗号化します)", self.status)
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
            self.pw_open = false;
            if pw.is_empty() {
                self.encrypt_pw = None;
                self.status = ui::t!("暗号化しません(次の保存から普通の docx)").into();
            } else {
                self.encrypt_pw = Some(pw);
                self.dirty = true;
                self.status = ui::t!("次の保存から、このパスワードで暗号化します\
                               (AES-128。Word や LibreOffice でも開けます)").into();
            }
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
                self.status = ui::tf!("マクロが読めません: {}", e).into();
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
        let doc_out = self.doc_for_save();
        let w = std::fs::File::create(&in_d)
            .map_err(|e| e.to_string())
            .and_then(|f| ooxml::write_with(&doc_out, original, std::io::BufWriter::new(f)));
        if let Err(e) = w {
            self.status = ui::tf!("マクロに渡せません: {}", e).into();
            return;
        }
        let script = macro_script(&in_d, &out_d, &user_code);
        let name = py_file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.status = ui::tf!("マクロ {} を実行しています…(サンドボックスの中の Python)", name).into();
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
                .map_err(|e| ui::tf!("Python が起動できません: {}", e))?;
            let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !o.status.success() {
                let err = String::from_utf8_lossy(&o.stderr);
                let last = err
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or(ui::t!("原因不明"))
                    .to_string();
                return Err(if err.contains("No module named 'docx'") {
                    ui::t!("python-docx がありません(pip install python-docx。\
                     .venv があればそちらへ)").to_string()
                } else {
                    last
                });
            }
            std::fs::read(&out_d)
                .map_err(|e| ui::tf!("結果が読めません: {}", e))
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
                                    ui::tf!("マクロ {} を実行しました(Ctrl+Z で戻せます)", name)
                                        .into()
                                } else {
                                    ui::tf!("マクロ {}: {}(Ctrl+Z で戻せます)", name, out.lines().last().unwrap_or_default())
                                    .into()
                                };
                            }
                            Err(e) => this.status = ui::tf!("結果が読めません: {}", e).into(),
                        }
                    }
                    Err(e) => this.status = ui::tf!("マクロ: {}", e).into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 最近開いた・保存した文書の控え(~/.config/officework/recent-writer.txt)
    pub(crate) fn recent_file() -> PathBuf {
        pyrun::config_dir().join("recent-writer.txt")
    }

    pub(crate) fn note_recent(p: &std::path::Path) {
        let rf = Self::recent_file();
        if let Some(dir) = rf.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut list: Vec<String> = std::fs::read_to_string(&rf)
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default();
        let me = p.to_string_lossy().to_string();
        list.retain(|x| *x != me);
        list.insert(0, me);
        list.truncate(12);
        let _ = std::fs::write(&rf, list.join("\n"));
    }

    pub(crate) fn recent_list() -> Vec<PathBuf> {
        std::fs::read_to_string(Self::recent_file())
            .map(|s| s.lines().map(PathBuf::from).filter(|p| p.exists()).collect())
            .unwrap_or_default()
    }

    /// 新しい文書。未保存の変更があるときは作らない(黙って捨てない)。
    /// 返り値: 作ったか
    pub(crate) fn new_doc(&mut self) -> bool {
        if self.dirty {
            self.status =
                ui::t!("未保存の変更があります。先に保存してください(Ctrl+S)").into();
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
        self.status = ui::t!("新しい文書です").into();
        true
    }

    /// 名前を付けて保存(いつでもダイアログ。別のスレッド — rfd は同期)
    pub(crate) fn save_as(&mut self, cx: &mut Context<Self>) {
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .add_filter(ui::t!("officework の文書"), &["adoc"])
                .add_filter(ui::t!("Word文書"), &["docx"])
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
                ui::t!("読み取り専用で保護されています(保護タブの「保護」で解除できます)").into();
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
        self.status = ui::t!("文書の情報を控えました(保存で docx に入ります)").into();
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
            ui::t!("ルビを外しました").into()
        } else {
            ui::tf!("ルビ「{}」を振りました(保存で docx の w:ruby に)", text).into()
        };
    }

    /// 数式のパネルの Enter。**組むのは Python** — 打った LaTeX を渡して
    /// 絵をもらい、カーソルの段落に置く。原文も一緒に持たせるので、
    /// 開き直しても直せる(絵だけだと消して打ち直しになる)
    pub(crate) fn eq_commit(&mut self) {
        self.eq_open = false;
        let tex = self.eq_ed.text().trim().to_string();
        if tex.is_empty() {
            self.status = "".into();
            return;
        }
        let size = self.doc.size_pt.unwrap_or(SIZE_PT);
        match crate::py::kumu_suushiki(&tex, size) {
            Ok((bytes, w_mm, h_mm)) => {
                self.checkpoint(false);
                let im = kumihan::InlineImage {
                    bytes: std::sync::Arc::new(bytes),
                    w_mm,
                    h_mm,
                    tex: Some(tex.clone()),
                    src: None,
                };
                // 挿すのはカーソルの段落。**images_new にだけ入れる** —
                // 組版(layout)は images と images_new の両方を描くので、
                // 両方に入れると画面に二つ出る(実機で踏んだ)。
                // images は「読み込んだ絵」の持ち場で、保存では書かれない
                let cur = self.ed.cursor();
                self.ed.move_to(cur, false);
                self.para(|p| p.images_new.push(im.clone()));
                self.dirty = true;
                self.status = ui::tf!("数式を置きました({} で組みました)",
                                      crate::py::suushiki_no_kumi_kata()).into();
            }
            // **黙って何も起きない、をしない。** 組めない理由をそのまま見せる
            Err(e) => {
                self.status = ui::tf!("数式を組めません: {}", e).into();
            }
        }
    }

    /// 上書きの前に、直前の中身を控えとして残す(最大9世代)。
    /// 置き場は同じフォルダの .jo-history/<ファイル名>/<日時>.docx。
    /// 名前は**その中身を保存した日時**(ファイルの mtime)— いつの姿かが分かる
    pub(crate) fn keep_version(&self, p: &std::path::Path) {
        let Some(name) = p.file_name().map(|n| n.to_string_lossy().to_string()) else {
            return;
        };
        let dir = p
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(".jo-history")
            .join(&name);
        if std::fs::create_dir_all(&dir).is_err() {
            return; // 控えられなくても保存は止めない
        }
        let stamp = std::process::Command::new("date")
            .arg("-r")
            .arg(p)
            .arg("+%Y%m%d-%H%M%S")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "0".into());
        let _ = std::fs::copy(p, dir.join(format!("{stamp}.docx")));
        // 増えすぎたら古い控えから消す
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut old: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
            old.sort();
            while old.len() > 9 {
                let _ = std::fs::remove_file(old.remove(0));
            }
        }
    }

    /// 控えの一覧(新しい順)。(表示名, パス)
    pub(crate) fn versions(&self) -> Vec<(String, PathBuf)> {
        let Some(p) = &self.path else { return Vec::new() };
        let Some(name) = p.file_name().map(|n| n.to_string_lossy().to_string()) else {
            return Vec::new();
        };
        let dir = p
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(".jo-history")
            .join(&name);
        let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
        let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        v.sort();
        v.reverse();
        v.into_iter()
            .map(|q| {
                let stem = q
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                // 20260804-183012 → 2026-08-04 18:30(名前は ASCII の日時)
                let disp = if stem.len() >= 13 && stem.is_ascii() {
                    format!(
                        "{}-{}-{} {}:{}",
                        &stem[0..4], &stem[4..6], &stem[6..8], &stem[9..11], &stem[11..13]
                    )
                } else {
                    stem
                };
                let kb = std::fs::metadata(&q).map(|m| m.len() / 1024).unwrap_or(0);
                (format!("{disp}({kb} KB)"), q)
            })
            .collect()
    }

    /// 控えを開く。いまのファイルは動かさず、**名無しの複製**として読む
    /// (保存すると名前を聞く。元へ戻したいなら同じ名前で保存する — 
    /// 黙って元のファイルを書き戻したりしない)
    pub(crate) fn open_version(&mut self, q: &std::path::Path) {
        let bytes = match std::fs::read(q) {
            Ok(b) => b,
            Err(e) => {
                self.status = ui::tf!("控えが読めません: {}", e).into();
                return;
            }
        };
        let bytes = if ooxml::crypt::is_encrypted(&bytes) {
            match self.encrypt_pw.as_ref().map(|pw| ooxml::crypt::decrypt(&bytes, pw)) {
                Some(Ok(b)) => b,
                _ => {
                    self.status =
                        ui::t!("控えは暗号化されています(いまのパスワードでは解けません)").into();
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
                self.status = ui::t!("控えを開きました(名無しの複製。保存で名前を聞きます。\
                               元へ戻すなら同じ名前で保存)").into();
            }
            Err(e) => self.status = ui::tf!("控えが読めません: {}", e).into(),
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
                ui::t!("まだファイルになっていません(保存すると申し送り帳が持てます)").into();
            return;
        };
        let stamp = std::process::Command::new("date")
            .arg("+%Y-%m-%d %H:%M")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
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
                    ui::t!("書き残しました(文書の隣の .chat.txt。開いた人が読めます)").into();
            }
            Err(e) => self.status = ui::tf!("チャットに書けません: {}", e).into(),
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
                self.status = ui::tf!("開けません: {}", e).into();
                return;
            }
        };
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
                ui::t!("この文書は暗号化されています。パスワードを打って Enter").into();
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
                        ui::t!("文字コードが読めません(UTF-8 でも CP932 でもない)").into();
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
        self.status = ui::tf!("HTML を読みました — {}(JS は実行しません。保存は docx{})", p.file_name().unwrap_or_default().to_string_lossy(), if self.fm_open { ui::t!("。記入は右上のパネルから") } else { "" })
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
                    Err(e) => this.status = ui::tf!("開けません: {}", e).into(),
                }
                cx.notify();
            });
        })
        .detach();
        self.status = ui::tf!("取りに行っています… {}", self.url_ed.text()).into();
    }

    /// AI に頼んで、返事を文書に反映する。**別のスレッドで待つ**(画面は止めない)。
    /// 反映は必ず doc_undo に控えてから = **Ctrl+Z の1手で戻る**。
    /// 宛先が使えなければ理由を言う(黙って空にしない)
    pub(crate) fn ai_go(&mut self, job: AiJob, cx: &mut Context<Self>) {
        if self.protected() {
            self.status =
                ui::t!("読み取り専用で保護されています(保護タブの「保護」で解除できます)").into();
            return;
        }
        if self.ai_busy {
            self.status = ui::t!("いま考えています(終わるまでお待ちください)").into();
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
            self.status = ui::t!("文章がありません(打つか、選んでから押してください)").into();
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
                    ui::tf!("{}\n\n(この文書に名前つきの記入欄はありません)", q)
                } else {
                    ui::tf!("{}\n\n【この文書の記入欄の名前】{}", q, names.join("、"))
                }
            }
            _ => format!("{ask}\n\n---\n{body}"),
        };
        let (sys, job2) = (sys.to_string(), job.clone());
        self.ai_busy = true;
        self.status = ui::tf!("AI({})に{}を頼んでいます…", back.label(), job.label())
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
            self.status = ui::t!("AI: 答えが空でした(何もしていません)").into();
            return;
        }
        // **会話は文書に入れない。** 左パネルに返し、置き換える文の案は
        // 人が「入れる」を押すまで文書に触らせない(押したのは人、が残る形)
        if matches!(job, AiJob::Chat(_)) {
            let 案 = crate::util::取り出す囲み(&out);
            let 見せる = match &案 {
                Some(code) => {
                    // 囲みの外の説明だけを会話に出す(文そのものは下の欄に置く)
                    let 説明 = out.split("```").next().unwrap_or("").trim().to_string();
                    if 説明.is_empty() {
                        let _ = code;
                        ui::t!("こう直します。").to_string()
                    } else {
                        説明
                    }
                }
                None => out.clone(),
            };
            self.ai_chat_log.push((false, 見せる));
            self.ai_chat_plan = 案;
            self.status = if self.ai_chat_plan.is_some() {
                ui::t!("直した文ができました(左パネルで読んでから「入れる」)").into()
            } else {
                ui::t!("答えました(左パネル)").into()
            };
            return;
        }
        // マクロ台本は文書に入れない — プラグイン置き場に .py で置き、
        // 人が読んで確かめてから一覧から実行する(開く=実行なしのまま)
        if matches!(job, AiJob::Macro(_)) {
            let code = strip_code_fence(&out);
            if code.trim().is_empty() {
                self.status = ui::t!("AI: 台本が空でした(何もしていません)").into();
                return;
            }
            let dir = plugins_dir();
            let _ = std::fs::create_dir_all(&dir);
            // 1つ目も訳を通す(ここだけ生の字だと、ja 以外で名前が揃わない)
            let mut i = 1;
            let mut path = dir.join(ui::tf!("ai台本{}.py", i));
            while path.exists() {
                i += 1;
                path = dir.join(ui::tf!("ai台本{}.py", i));
            }
            match std::fs::write(&path, &code) {
                Ok(()) => {
                    self.plug_open = true; // 置いた台本がすぐ見えるように
                    self.status = ui::tf!("台本を {} に置きました — 読んで確かめてから、\
                         プラグインの一覧で実行してください(自動では走らせません)", path.display())
                    .into();
                }
                Err(e) => self.status = ui::tf!("台本を置けません: {}", e).into(),
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
                    ui::tf!("ふりがなを {} 箇所に振りました(Ctrl+Z で1手で戻せます)", n)
                        .into();
                return;
            }
        }
        self.dirty = true;
        self.relayout();
        self.status =
            ui::tf!("AI の{}を入れました(Ctrl+Z で1手で戻せます)", label).into();
    }

    /// 会話を送る。**答えは文書でなくパネルへ**返る(AiJob::Chat)
    pub(crate) fn ai_chat_send(&mut self, cx: &mut Context<Self>) {
        let q = self.ai_chat_in.text().trim().to_string();
        if q.is_empty() {
            self.status = ui::t!("用件がありません").into();
            return;
        }
        self.ai_chat_log.push((true, q.clone()));
        self.ai_chat_in = Editor::new("");
        self.ai_chat_plan = None;
        self.ai_go(AiJob::Chat(q), cx);
    }

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
                ui::t!("読み取り専用で保護されています(保護タブの「保護」で解除できます)").into();
            return;
        }
        self.switch_target(Target::Body);
        self.flush_target();
        self.checkpoint(false);
        let sel = self.ed.selection();
        let 置き換えた = !sel.is_empty();
        if 置き換えた {
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
        self.ai_chat_log.push((false, ui::t!("入れました。").to_string()));
        self.status = if 置き換えた {
            ui::t!("選んでいた所を置き換えました(Ctrl+Z で戻せます)").into()
        } else {
            ui::t!("カーソルの後ろに入れました(Ctrl+Z で戻せます)").into()
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
                    items.first().cloned().unwrap_or_else(|| ui::t!("　　　　").into())
                }
                K::Date => std::process::Command::new("date")
                    .arg("+%Y年%-m月%-d日")
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|| ui::t!("　　　　").into()),
                K::Picture => ui::t!("[画像]").to_string(),
                _ => ui::t!("　　　　").to_string(),
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
        self.status = ui::tf!("{}の記入欄を入れました(中は普通に打てます。保存で docx の\
             コンテンツコントロールに)", kind.label())
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
            self.status = ui::t!("選択肢がありません(カンマ区切りで打ってください)").into();
            return;
        }
        self.insert_sdt(self.sd_kind, items);
    }

    /// 名前のパネルの Enter。カーソルの記入欄の alias / tag をまるごと打ち替える
    /// (run が割れていても sdt_range_at が一つに繋げる)
    pub(crate) fn sd_name_commit(&mut self) {
        let name = self.sd_ed.text().trim().to_string();
        if name.is_empty() {
            self.status = ui::t!("名前がありません(記入欄はそのまま)").into();
            return;
        }
        let Some(range) = self.doc.sdt_range_at(self.ed.cursor()) else {
            self.status =
                ui::t!("記入欄が見つかりません(欄の中にカーソルを置いてください)").into();
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
        self.status = ui::tf!("記入欄に名前「{}」を付けました(docx の w:tag。\
             マクロは fill(\"{}\", 値) で記入できます)", name, name)
        .into();
    }

    /// チェックの欄を切り替える(☐ ⇄ ☑)。カーソルがその欄にあるとき
    pub(crate) fn toggle_checkbox(&mut self) -> bool {
        let Some(sd) = self.sdt_at() else { return false };
        if sd.kind != kumihan::SdtKind::Checkbox {
            return false;
        }
        // カーソルの前後の1字を見て入れ替える
        let text = self.ed.text().to_string();
        let cur = self.ed.cursor();
        let (s0, e0) = match text[..cur].char_indices().next_back() {
            Some((i, c)) if c == '☐' || c == '☑' => (i, cur),
            _ => match text[cur..].chars().next() {
                Some(c) if c == '☐' || c == '☑' => (cur, cur + c.len_utf8()),
                _ => return false,
            },
        };
        let now = &text[s0..e0];
        let next = if now == "☑" { "☐" } else { "☑" };
        self.ed.move_to(s0, false);
        self.ed.move_to(e0, true);
        self.ed.insert(next);
        self.on_edited();
        self.status = ui::tf!("チェックを {} にしました", next).into();
        true
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
            "prot-encrypt" => self.encrypt_pw.is_some(),
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
        self.status = ui::tf!("{}に合わせました(ズーム {}%)", if width { ui::t!("幅") } else { ui::t!("ページ") }, (self.zoom * 100.0).round() as i32)
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
        self.status = ui::tf!("取りに行っています… {}", url).into();
        let task = cx.background_executor().spawn(async move { http_fetch(&url, None) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok((bytes, final_url)) => this.adopt_fetched(&final_url, &bytes),
                    Err(e) => this.status = ui::tf!("開けません: {}", e).into(),
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
        self.status = ui::t!("記入しました(送信のボタンで送る)").into();
    }

    /// フォームを送る。POST は urlencoded、GET は ?query。
    /// 網の線引き: いま開いている起点(html_origin)へだけ
    pub(crate) fn fm_submit(&mut self, cx: &mut Context<Self>) {
        let Some(fm) = self.html_forms.first().cloned() else { return };
        let Some(origin) = self.html_origin.clone() else {
            self.status =
                ui::t!("ローカルの HTML からは送れません(URL で開いてください)").into();
            return;
        };
        let url = if fm.action.starts_with("http://") {
            if !fm.action.starts_with(&origin) {
                self.status = ui::t!("送り先が開いた相手と違います(送りません)").into();
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
        self.status = ui::t!("送っています…").into();
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
                    Err(e) => this.status = ui::tf!("送れません: {}", e).into(),
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
        let 行 = text.lines().count();
        self.status = ui::tf!(
            "{}({} 行)— 素の文字として開きました。保存も素の文字です",
            p.file_name().unwrap_or_default().to_string_lossy(),
            行
        )
        .into();
    }

    /// **ネイティブ文書(.adoc)を開く。** 中身は意味だけで、見た目は
    /// テンプレートが持つ(SEKKEI「本文とテンプレートを分ける」)。
    pub(crate) fn open_adoc(&mut self, p: &std::path::Path, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
        let doc = match kumihan::adoc::parse(&text) {
            Ok(d) => d,
            Err(e) => {
                // **読めない所は言う。** 黙って本文に化けさせない
                self.status = ui::tf!("{} が読めません: {}", p.display().to_string(), e).into();
                return;
            }
        };
        self.target = Target::Body;
        self.hf_edit = None;
        self.track = false;
        self.track_base = None;
        self.encrypt_pw = None;
        self.notes.clear();
        self.native = true;
        let (tmpl, 言い分) = self.load_template(doc.template.as_deref(), p);
        self.tmpl = tmpl;
        // 用紙はテンプレートが持つ(本文は持たない)
        self.pg = self.tmpl.page.unwrap_or_default();
        self.set_doc(doc);
        self.adopt_font();
        self.path = Some(p.to_path_buf());
        self.dirty = false;
        self.status = ui::tf!(
            "{} — 本文は adoc、書式は{}",
            p.file_name().unwrap_or_default().to_string_lossy(),
            言い分
        )
        .into();
    }

    /// テンプレートを探して読む。**隣 → 置き場 → 同梱の既定**の順
    /// (名前は文書の頭の `:template:`)。返りは(テンプレート, 言い分)
    fn load_template(
        &self,
        name: Option<&str>,
        doc_path: &std::path::Path,
    ) -> (kumihan::theme::Theme, String) {
        let Some(name) = name else {
            return (kumihan::theme::default_theme(), ui::t!("同梱の既定").to_string());
        };
        let mut cands = Vec::new();
        if let Some(dir) = doc_path.parent() {
            cands.push(dir.join(format!("{name}.toml")));
        }
        cands.push(ui::settings::path().with_file_name("templates").join(format!("{name}.toml")));
        for c in cands {
            let Ok(src) = std::fs::read_to_string(&c) else { continue };
            return match kumihan::theme::parse(&src) {
                Ok(th) => (th, c.display().to_string()),
                // **壊れたテンプレートは黙って既定に落ちない** — どこが
                // 悪いか言わないと、直す手がかりが無い
                Err(e) => (
                    kumihan::theme::default_theme(),
                    ui::tf!("{} が読めないので同梱の既定({})", c.display().to_string(), e)
                        .to_string(),
                ),
            };
        }
        (
            kumihan::theme::default_theme(),
            ui::tf!("テンプレート「{}」が見つからないので同梱の既定", name).to_string(),
        )
    }

    /// **フォルダから探す**(2026-08-17 発注者。SFIND の写真)。
    ///
    /// 素の字は face が読み、**.docx は writer が中身を渡す** — 一度 txt に
    /// 落としてから探す手間が消える。当たりは一覧に出て、選ぶと下に見え、
    /// 下の「読み込み」で初めて開く(見て、これだと分かってから開く)。
    pub(crate) fn find_in_folder(&mut self) {
        let Some(dir) = self.find_dir() else {
            self.status = ui::t!("探す場所を選んでください").into();
            return;
        };
        let term = self.fd_term.text().to_string();
        if term.trim().is_empty() {
            self.status = ui::t!("探す字が空です").into();
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
        // **打ち切りも読めなかった数も言う。** 全部見たように見せない
        let mut s = ui::tf!(
            "{} 件のファイルに {} 件(見たのは {} 件 / {})",
            tally.matched.to_string(),
            tally.hits.to_string(),
            tally.looked.to_string(),
            ui::search::human_size(tally.bytes)
        )
        .to_string();
        if tally.unread > 0 {
            s.push_str(&ui::tf!(" — 読めなかった {} 件", tally.unread.to_string()));
        }
        if tally.cut {
            s.push_str(ui::t!(" — 多いので途中で止めました"));
        }
        self.status = s.into();
    }

    /// **探す場所。** 選んでいなければ(1)前に選んだ場所(settings.toml)
    /// (2)いま開いている文書の隣、の順に決める。
    ///
    /// 開いている文書の隣は**当たり前の出発点**で、そこから始められないと
    /// 「場所を選ぶ」を毎回押すことになる(2026-08-17)
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
                    this.status = ui::tf!("場所: {}", p.display().to_string()).into();
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
            "{} の {} 行目(下の「読み込み」で開きます)",
            f.path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            h.line.to_string()
        )
        .into();
    }

    /// 下の「読み込み」。**選んでいる当たりの文書を開き、その位置へ飛ぶ**
    pub(crate) fn find_load(&mut self) {
        let Some((fi, hi)) = self.fd_at else {
            self.status = ui::t!("当たりを選んでから読み込んでください").into();
            return;
        };
        let Some(f) = self.fd_hits.get(fi).cloned() else { return };
        let at = f.hits.get(hi).map(|h| h.at).unwrap_or(0);
        if self.dirty {
            self.status = ui::t!("いまの文書に未保存の変更があります(保存するか、捨ててから)").into();
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
    pub(crate) fn dump_ui(&self) {
        let Some(path) = std::env::var_os("OFFICEWORK_UI_DUMP") else { return };
        let boxes: Vec<String> = self
            .btn_box
            .borrow()
            .iter()
            .map(|(id, (x, y, w, h))| {
                format!("{{\"id\":\"{id}\",\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}")
            })
            .collect();
        let body = format!(
            "{{\"tab\":{},\"native\":{},\"rp_open\":{},\"rp_tab\":{},\"rp_drawn\":{},\"file_view\":{},\"win_w\":{},\"win_h\":{},\"fd_files\":{},\"fd_hits\":{},\"sel\":[{},{}],\"fd_boxes\":[{}],\"status\":{:?},\"boxes\":[{}]}}",
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
            boxes.join(",")
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
            self.status = ui::t!("この文書はもう adoc 形式です").into();
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
            .unwrap_or_else(|| ui::t!("この文書の型").to_string());
        let mut doc = doc;
        doc.template = Some(name);
        self.pg = self.tmpl.page.unwrap_or_default();
        self.set_doc(doc);
        self.adopt_font();
        self.dirty = true;
        // **落ちた物を数えて言う。** 「何も失っていない」と嘘をつかない
        self.status = if rep.dropped == 0 {
            ui::tf!(
                "adoc 形式にしました — 書式 {} 個をテンプレートに移し、段落 {} 個を本文にしました",
                rep.styles.to_string(),
                rep.paragraphs.to_string()
            )
        } else {
            ui::tf!(
                "adoc 形式にしました — 書式 {} 個をテンプレートに移し、段落 {} 個を本文にしました。段落ごとの書式に収まらない {} 箇所は落ちました(強調や脚注は残っています)",
                rep.styles.to_string(),
                rep.paragraphs.to_string(),
                rep.dropped.to_string()
            )
        }
        .into();
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
            "adoc 形式では、書式に名前を付けて使います。右のスタイルから選ぶか、新しく作ってください"
        )
        .into();
        cx.notify();
        true
    }





    /// スタイルの新設を決める(名前の欄で Enter)。テンプレートに足し、
    /// 選んでいる段落に名前を付け、テンプレートのファイルへ書き戻す
    pub(crate) fn style_commit(&mut self) {
        let Some(mut d) = self.style_new.take() else { return };
        let name = self.style_ed.text().trim().to_string();
        if name.is_empty() {
            self.status = ui::t!("名前が空です(やめました)").into();
            return;
        }
        d.name = name.clone();
        self.checkpoint(false);
        // 同じ名前があれば置き換える(直しの操作にもなる)
        if let Some(i) = self.tmpl.styles.iter().position(|s| s.name == name) {
            self.tmpl.styles[i] = d;
        } else {
            self.tmpl.styles.push(d);
        }
        // **選んでいれば字に、選んでいなければ段落に**(2026-08-16)。
        // 語を1つ選んで見た目を変えようとしたのに段落ぜんぶが変わる、では
        // 直接書式の手軽さに勝てない — 選択の有無が意図そのもの
        self.switch_target(Target::Body);
        self.flush_target();
        let sel = self.ed.selection();
        let 字 = sel.start != sel.end;
        let n = name.clone();
        if !字 {
            self.doc.apply_para(sel, |p| p.style_id = Some(n.clone()));
        } else {
            self.doc.apply_char_format(sel, |f| f.style_id = Some(n.clone()));
        }
        let 書けた = self.save_template();
        self.dirty = true;
        self.relayout_keep();
        self.status = if 字 {
            ui::tf!("選んだ字を「{}」にしました({})", name, 書けた)
        } else {
            ui::tf!("この段落を「{}」にしました({})", name, 書けた)
        }
        .into();
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
        self.status = ui::t!("スタイルを外しました").into();
    }

    pub(crate) fn wear_style(&mut self, name: &str) {
        self.switch_target(Target::Body);
        self.checkpoint(false);
        self.flush_target();
        let sel = self.ed.selection();
        let role = match name {
            "本文" => Some(kumihan::ParaStyle::Body),
            "見出し1" => Some(kumihan::ParaStyle::Heading(1)),
            "見出し2" => Some(kumihan::ParaStyle::Heading(2)),
            "見出し3" => Some(kumihan::ParaStyle::Heading(3)),
            "引用" => Some(kumihan::ParaStyle::Quote),
            _ => None,
        };
        let n = name.to_string();
        self.doc.apply_para(sel, |p| match role {
            // 役割で出る名前は、役割の側で持つ(二重に名乗らない)
            Some(r) => {
                p.style = r;
                p.style_id = None;
            }
            None => p.style_id = Some(n.clone()),
        });
        self.dirty = true;
        self.relayout_keep();
        self.status = ui::tf!("「{}」にしました", name.to_string()).into();
    }

    /// **いま着ているスタイルの字の大きさを1段動かす**(右パネル)。
    /// 直るのはテンプレートなので、**同じスタイルの所が一度に変わる** —
    /// ここがライブ合成の効き目そのもの
    pub(crate) fn tweak_style(&mut self, step: i32) {
        let (pi, _) = self.cursor_para();
        let Some(para) = self.doc.paragraphs().nth(pi) else { return };
        let name = para
            .style_id
            .clone()
            .or_else(|| kumihan::theme::Theme::role_name(para.style).map(|s| s.to_string()))
            .unwrap_or_else(|| ui::t!("本文").to_string());
        let base = self.tmpl.size_pt.unwrap_or(kumihan::DEFAULT_PT);
        let now = self.tmpl.style(&name).and_then(|d| d.size_pt).unwrap_or(base);
        let next = ui::combo::step_size(now, step > 0);
        self.checkpoint(false);
        match self.tmpl.styles.iter_mut().find(|d| d.name == name) {
            Some(d) => d.size_pt = Some(next),
            None => self.tmpl.styles.push(kumihan::theme::StyleDef {
                name: name.clone(),
                size_pt: Some(next),
                ..Default::default()
            }),
        }
        let 書けた = self.save_template();
        self.dirty = true;
        self.relayout_keep();
        self.status =
            ui::tf!("「{}」を {}pt にしました({})", name, next.to_string(), 書けた).into();
    }

    /// テンプレートをファイルへ書き戻す。返りは言い分
    fn save_template(&self) -> String {
        let Some(name) = self.doc.template.clone() else {
            // 名指しが無い = 同梱の既定を着ている。**既定は書き換えない** —
            // 他の文書まで巻き添えになる
            return ui::t!("この文書だけ。テンプレートに残すには :template: で名前を付けてください")
                .to_string();
        };
        let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) else {
            return ui::t!("文書がまだファイルになっていないので、テンプレートは書けません").to_string();
        };
        let at = dir.join(format!("{name}.toml"));
        match std::fs::write(&at, kumihan::theme::write(&self.tmpl)) {
            Ok(()) => ui::tf!("{} に書きました", at.display().to_string()).to_string(),
            Err(e) => ui::tf!("テンプレートが書けません: {}", e).to_string(),
        }
    }

    /// ネイティブ文書として保存する(.adoc)。**意味だけを書く**
    pub(crate) fn save_adoc_to(&mut self, p: &std::path::Path) -> Result<(), String> {
        self.flush_target();
        std::fs::write(p, kumihan::adoc::write(&self.doc)).map_err(|e| e.to_string())
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
                self.status = ui::tf!("{} 段落 / 表 {} — {}", rep.paragraphs, doc.tables().count(), p.file_name().unwrap_or_default().to_string_lossy())
                .into();
                self.pg = doc.page.unwrap_or_default();
                self.set_doc(doc);
                self.adopt_font();
                self.relayout_keep();
                // 排他(共有フォルダの「後勝ちで潰す」を防ぐ。calc と同じ)
                self.acquire_lock(&p);
                if let Some(who) = self.locked_by.clone() {
                    self.status = ui::tf!("{} — **{} が開いています**。上書き保存はできません(別の名前で保存へ)", self.status, who)
                    .into();
                }
                if self.doc.protection.is_some() {
                    self.status = ui::tf!("{} — 読み取り専用で保護されています(保護タブで解除できます)", self.status)
                    .into();
                }
                Self::note_recent(&p);
                self.path = Some(p);
                self.dirty = false;
            }
            Err(e) => self.status = ui::tf!("開けません: {}", e).into(),
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
            self.status = ui::tf!("{} が開いているため上書きしません。別の名前で保存します", self.locked_by.as_deref().unwrap_or(ui::t!("誰か")))
            .into();
        }
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .add_filter(ui::t!("officework の文書"), &["adoc"])
                .add_filter(ui::t!("Word文書"), &["docx"])
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
                    None => this.status = ui::t!("保存をやめました(名前が決まっていません)").into(),
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
            match self.save_adoc_to(&p) {
                Ok(()) => {
                    self.path = Some(p.clone());
                    self.native = true;
                    self.dirty = false;
                    self.status = ui::tf!(
                        "{} に保存しました(本文だけ — 書式はテンプレートの側)",
                        p.file_name().unwrap_or_default().to_string_lossy()
                    )
                    .into();
                }
                Err(e) => self.status = ui::tf!("保存できません: {}", e).into(),
            }
            return;
        }
        // 素の文字(.py / .txt / .md)は素のまま返す — docx に化けさせない
        if p.extension().and_then(|e| e.to_str()).is_some_and(is_plain_ext) {
            match self.save_text_to(&p) {
                Ok(()) => {
                    self.path = Some(p.clone());
                    self.dirty = false;
                    self.status = ui::tf!(
                        "{} に保存しました(素の文字)",
                        p.file_name().unwrap_or_default().to_string_lossy()
                    )
                    .into();
                }
                Err(e) => self.status = ui::tf!("保存できません: {}", e).into(),
            }
            return;
        }
        self.flush_target();
        // 元のファイルの部品(画像・スタイル・ヘッダー等)を持ち越す。
        // 上書き保存では読み終えてから書く(同じファイルを同時に開かない)
        let original: Option<std::io::Cursor<Vec<u8>>> =
            self.original_plain().map(std::io::Cursor::new);
        let doc_out = self.doc_for_save();
        // バージョン履歴: 上書きの前に、いままでの中身を控えとして残す
        if p.exists() {
            self.keep_version(&p);
        }
        let saved = if let Some(pw) = self.encrypt_pw.clone() {
            // 暗号化は zip 丸ごとが単位 — 一度メモリへ書いてから包む
            let mut plain = Vec::new();
            ooxml::write_with(&doc_out, original, std::io::Cursor::new(&mut plain))
                .and_then(|_| ooxml::crypt::encrypt(&plain, &pw))
                .and_then(|enc| {
                    kumihan::atomic::save(&p, |mut f| {
                        use std::io::Write as _;
                        f.write_all(&enc).map_err(|e| e.to_string())
                    })
                })
        } else {
            kumihan::atomic::save(&p, |f| {
                ooxml::write_with(&doc_out, original, std::io::BufWriter::new(f))
            })
        };
        match saved {
            Ok(_) => {
                let caveat = if self.notes.is_empty() {
                    ""
                } else {
                    // 読めなかった要素は本文から消えている。黙って保存しない
                    ui::t!("(読めなかった要素は本文に戻りません)")
                };
                let enc_note =
                    if self.encrypt_pw.is_some() { ui::t!("(暗号化)") } else { "" };
                self.status = ui::tf!("保存しました — {}{}{}", p.file_name().unwrap_or_default().to_string_lossy(), enc_note, caveat)
                .into();
                // 保存先のロックを取り直す(別の名前で保存したときは
                // 新しいファイルの側を守る。同じ名前なら実質そのまま)
                self.acquire_lock(&p);
                Self::note_recent(&p);
                self.path = Some(p);
                self.dirty = false;
            }
            Err(e) => self.status = ui::tf!("保存できません: {}", e).into(),
        }
    }

}
