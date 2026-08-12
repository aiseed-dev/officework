//! **文書とファイル。** 開く・保存・版・チャット・AI・記入欄。

use crate::*;

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
            encrypt_pw: None,
            pw_open: false,
            pw_ed: Editor::new(""),
            pw_pending: None,
            doc_undo: None,
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
                w.set_doc(Document::plain(
                    "ここに打てます。日本語入力(IME)もそのまま使えます。\n\
                     Ctrl+S で docx として保存、Ctrl+O で開く。マクロはありません。",
                    SIZE_PT,
                ));
                w.dirty = false;
            }
        }
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
            Target::Body => self.doc.set_body_text(self.ed.text(), SIZE_PT),
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
        let m = Metrics::new(&self.font_bytes).expect("フォント");
        // 段組みなら1段の行長で組み、ページの物理座標へ折る。
        // 折った後の座標は画面もクリックも PDF もそのまま使える
        let y0 = self.pg.top_mm + 4.0;
        if self.doc.vertical {
            // 縦書き: 行長 = 紙の縦の使い幅で組み、右からの列へ写す(K4)
            let measure =
                (self.pg.h_mm - self.pg.top_mm - self.pg.bottom_mm - 8.0).max(20.0);
            self.page = layout(
                &self.doc,
                &m,
                &Frame { measure_mm: measure, line_height_mm: LINE_MM, y0_mm: y0 },
            );
            kumihan::fold_vertical(&mut self.page, &self.pg, y0, LINE_MM);
        } else {
            self.page = layout(
                &self.doc,
                &m,
                &Frame { measure_mm: self.pg.column_measure_mm(), line_height_mm: LINE_MM, y0_mm: y0 },
            );
            kumihan::fold_columns(&mut self.page, &self.pg, y0);
        }
        self.refresh_hf();
    }

    /// いまの紙面の総頁(紙と同じ折り方で数える)。
    pub(crate) fn total_pages(&self) -> usize {
        self.page_offsets.len().max(1)
    }

    /// 巻物の y → (ページ, ページの中の y)。筆はページに固定する。
    pub(crate) fn page_of_roll(&self, y: f32) -> (usize, f32) {
        let p = self.page_offsets.iter().rposition(|o| y >= *o - 0.01).unwrap_or(0);
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
                                .unwrap_or((SIZE_PT, None, Default::default()));
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
                                .unwrap_or((SIZE_PT, None, Default::default()));
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
                            size_pt: SIZE_PT,
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
        self.page_offsets = paper::paginate(&self.page, paper::Paper {
            width_mm: self.pg.w_mm,
            height_mm: self.pg.h_mm,
            margin_mm: self.pg.left_mm,
        }).1;
        // 複数ページ(見開き)。**画面だけ**の折り方 — PDF は 1ページずつ
        // (save_pdf は組み直してから写す)。縦書きとは併せない
        if self.multipage && !self.page.vertical {
            let offs = self.page_offsets.clone();
            kumihan::fold_pages(&mut self.page, &self.pg, &offs, 2, PAGE_GAP_MM);
        }
        let total = self.total_pages();
        self.header_lines =
            kumihan::layout_hf(&self.doc.header, &m, &self.pg, LINE_MM, 1, total, false);
        self.footer_lines =
            kumihan::layout_hf(&self.doc.footer, &m, &self.pg, LINE_MM, 1, total, true);
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
                                this.doc_undo = Some(this.doc.clone());
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

    /// 最近開いた・保存した文書の控え(~/.config/office/recent-writer.txt)
    pub(crate) fn recent_file() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".config/office/recent-writer.txt")
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
        self.set_doc(Document::plain("", SIZE_PT));
        self.dirty = false;
        self.status = ui::t!("新しい文書です").into();
        true
    }

    /// 名前を付けて保存(いつでもダイアログ。別のスレッド — rfd は同期)
    pub(crate) fn save_as(&mut self, cx: &mut Context<Self>) {
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new().add_filter(ui::t!("Word文書"), &["docx"]).save_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(mut p) = r {
                    if p.extension().is_none() {
                        p.set_extension("docx");
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
        self.doc.set_body_text(self.ed.text(), SIZE_PT);
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
        let (doc, notes, forms, links) = kumihan::html::parse_full(&text, SIZE_PT);
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
            AiJob::Continue => text[..sel.end.min(text.len())].to_string(),
            AiJob::Macro(_) => String::new(),
            AiJob::Ask(_) if sel.is_empty() => String::new(),
            _ if sel.is_empty() => text.clone(),
            _ => text[sel.clone()].to_string(),
        };
        if body.trim().is_empty() && !matches!(job, AiJob::Ask(_) | AiJob::Macro(_)) {
            self.status = ui::t!("文章がありません(打つか、選んでから押してください)").into();
            return;
        }
        let (sys, ask) = job.prompt();
        let user = match &job {
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
        self.doc_undo = Some(self.doc.clone());
        let label = job.label();
        match job {
            // 要約は文書の頭に、印つきの段落として置く
            AiJob::Summary => {
                let text = self.ed.text().to_string();
                let joined = ui::tf!("【要約】{}\n\n{}", out, text);
                self.ed = Editor::new(&joined);
                self.doc.set_body_text(self.ed.text(), SIZE_PT);
            }
            // 置き換え(選択が無ければ全文)
            AiJob::Rewrite(_, _) | AiJob::Translate | AiJob::Table => {
                let out = if matches!(job, AiJob::Table) {
                    // | 区切りの行を、読みやすい字の表に直す(表の挿入は次の課題)
                    out.lines()
                        .map(|l| {
                            l.trim().trim_matches('|').replace('|', "　")
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    out
                };
                let r = if sel.is_empty() { 0..self.ed.text().len() } else { sel };
                self.ed.move_to(r.start, false);
                self.ed.move_to(r.end, true);
                self.ed.insert(&out);
                self.doc.set_body_text(self.ed.text(), SIZE_PT);
            }
            // 続き・自由な頼みは、カーソル(選択の終わり)の後ろへ
            // Macro は上で受けて return 済み
            AiJob::Macro(_) => unreachable!(),
            AiJob::Continue | AiJob::Ask(_) => {
                let at = sel.end.min(self.ed.text().len());
                self.ed.move_to(at, false);
                self.ed.insert(&format!("\n{out}"));
                self.doc.set_body_text(self.ed.text(), SIZE_PT);
            }
            // ふりがなは |語《よみ》 を**うちのルビ**に直して振る
            AiJob::Furigana => {
                let base = if sel.is_empty() { 0 } else { sel.start };
                let (plain, rubies) = strip_ruby_marks(&out, base);
                let r = if sel.is_empty() { 0..self.ed.text().len() } else { sel };
                self.ed.move_to(r.start, false);
                self.ed.move_to(r.end, true);
                self.ed.insert(&plain);
                self.doc.set_body_text(self.ed.text(), SIZE_PT);
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
        self.doc.set_body_text(self.ed.text(), SIZE_PT);
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
    pub(crate) fn open_plain(&mut self, p: PathBuf, bytes: Vec<u8>) {
        self.target = Target::Body;
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
            rfd::FileDialog::new().add_filter(ui::t!("Word文書"), &["docx"]).save_file()
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
