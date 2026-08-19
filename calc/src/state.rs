//! **盤面の状態。** シート・undo・選択・並べ替え・貼り付け・AI。

use crate::*;

impl Calc {
    /// **書きかけがあるか**(`officework` が持ち替えの前に聞きます)。
    /// ブックは1冊なので、この画面の `dirty` がそのまま答えです
    pub fn has_unsaved(&self) -> bool {
        self.dirty
    }

    /// 状態行に出す(持ち替えを断った理由を言うため)。
    pub fn say(&mut self, msg: impl Into<gpui::SharedString>) {
        self.status = msg.into();
    }

    /// **`officework` の中に埋め込まれたと伝える**(統合の段1)。
    ///
    /// これを立てると、一覧のクリックは自分で開かず `open_request` に置きます。
    pub fn set_embedded(&mut self) {
        self.embedded = true;
    }

    /// 画面が暗い側か(`officework` がタブの行の色を合わせるのに使う)。
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    /// **いま開いているファイルの道**(`officework` がタブを引き当てるのに使う)。
    /// まだ名前が無ければ `None` です。
    pub fn opened_path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    /// **このブックを開く**(`officework` が頼む口。統合の段1)。
    pub fn open_path(&mut self, p: PathBuf) {
        self.open(p);
    }

    /// **いま開いているフォルダ。** 右パネルのファイル一覧が並べる場所です。
    /// 開いているブックの親を使い、無ければ前に使ったフォルダです。
    pub(crate) fn folder(&self) -> Option<PathBuf> {
        if let Some(p) = self.path.as_ref().and_then(|p| p.parent()) {
            return Some(p.to_path_buf());
        }
        ui::settings::get("folder").map(PathBuf::from).filter(|p| p.is_dir())
    }

    /// フォルダを覚える(次に起動したときここを開きます)。
    pub(crate) fn remember_folder(&self) {
        if let Some(d) = self.folder() {
            ui::settings::set("folder", &d.display().to_string());
        }
    }

    pub fn new(path: Option<PathBuf>, cx: &mut Context<Self>) -> Calc {
        let mut c = Calc {
            focus: cx.focus_handle(),
            book: Book::new(),
            active: 0,
            cursor: Pos::new(0, 0),
            anchor: None,
            drag: None,
            size_drag: None,
            head_drag: None,
            img_cache: Default::default(),
            find_term: None,
            pivot_pend: None,
            sub_pend: None,
            sort_pend: None,
            pick_note: None,
            pivot_flt: None,
            dedup_pend: None,
            cond_pend: None,
            import_pend: None,
            border_pal: None,
            rec: None,
            rec_fmt_partial: false,
            rec_sel: None,
            fill_drag: None,
            pane_box: std::cell::Cell::new((0.0, 0.0, 0.0, 0.0)),
            pop_at: None,
            menu_direct: false,
            edits: 0,
            btn_box: Rc::new(std::cell::RefCell::new(HashMap::new())),
            pop_btn_w: std::cell::Cell::new(0.0),
            pop_top: std::cell::Cell::new(0.0),
            font_name: kumihan::font::for_document(None)
                .map(|(fam, _)| gpui::SharedString::from(fam.name.clone()))
                .unwrap_or_else(|_| "Noto Sans JP".into()),
            pen_style: sheet::model::BStyle::default(),
            pen_color: None,
            hf_pend: None,
            name_pend: None,
            name_new: None,
            brush: None,
            menu_head: None,
            solver: None,
            sa_cat: 0,
            slicers: Vec::new(),
            slicer_sel: 0,
            slicer_cfg: false,
            slicer_drag: None,
            show_comments: true,
            // 器は settings.toml。書いていなければ入(綴りは "0" で切)
            autocorrect: ui::settings::get("math_autocorrect")
                .map(|v| v != "0")
                .unwrap_or(true),
            comment_list: None,
            key_hint: None,
            alt_armed: false,
            pick_paths: Vec::new(),
            encrypt_pw: None,
            pw_pending: None,
            goal: None,
            py_spills: Default::default(),
            udf_stamp: Default::default(),
            udf_busy: false,
            py_edit: None,
            py_edit_ask: false,
            rpc_batch: false,
            trace: Vec::new(),
            my_lock: None,
            locked_by: None,
            shape_sel: None,
            shape_drag: None,
            shape_rot: None,
            point_edit: None,
            pt_drag: None,
            shape_multi: Vec::new(),
            menu_shape: false,
            shape_clip: None,
            dt_col: None,
            track_from: None,
            img_sel: None,
            img_drag: None,
            wheel: (0.0, 0.0),
            view_w_px: 0.0,
            view_h_px: 0.0,
            edit_armed: false,
            name_edit: None,
            fn_dlg: None,
            fn_args: None,
            ref_pick: None,
            quit_ask: false,
            menu_at: None,
            menu_sub: None,
            pick: None,
            pick_kind: "value",
            pick_filter: None,
            pick_sel: 0,
            sheet_menu_at: None,
            fmt_panel: None,
            prompt: None,
            prop_add: None,
            pw_show: false,
            show_formulas: false,
            view: Pos::new(0, 0),
            frozen: None,
            freeze_shadow: false,
            auto_filter: None,
            filter_panel: None,
            dv_dlg: None,
            ui_scale: ui::settings::get("ui_scale")
                .and_then(|v| v.parse::<f32>().ok())
                .map(|v| v.clamp(0.8, 1.5))
                .unwrap_or(1.0),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            sheet_ui: Vec::new(),
            clip: None,
            clip_cells: None,
            clip_range: None,
            gridlines: true,
            input: Editor::new(""),
            path: None,
            status: "".into(),
            notes: Vec::new(),
            dirty: false,
            tab: 1, // ファイルは全面ページになったので、開きはホーム
            prev_tab: 1,
            hover_hint: None,
            file_view: 0,
            fd_term: Editor::new(""),
            fd_glob: Editor::new(""),
            fd_dir: None,
            fd_field: 0,
            fd_hits: Vec::new(),
            fd_tally: Default::default(),
            fd_at: None,
            fd_peek: String::new(),
            zoom: 1.0,
            show_formula_bar: true,
            show_headers: true,
            show_zeros: true,
            show_breaks: false,
            // 既定は5分。JO_RECOVER_SECS で縮められる(点検と、
            // 落ちやすい環境での駆け込み用)
            recover_secs: std::env::var("JO_RECOVER_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            recover_at: std::time::Instant::now(),
            csv_kind: "UTF-8(BOM付き)・カンマ",
            recent_symbols: Vec::new(),
            recent_fonts: Vec::new(),
            dark: ui::settings::get("theme").as_deref() == Some("dark"),
            auto_calc: true,
            watch: Vec::new(),
            ai_busy: false,
            left_open: false,
            right_open: false,
            chat_log: Vec::new(),
            chat_in: Editor::new(""),
            chat_plan: None,
            chat_focus: false,
            chat_busy: false,
            chat_err: None,
            left_face: 0,
            right_face: 0,
            open_request: None,
            open_dialog_request: false,
            embedded: false,
            tool: None,
            ink_cur: None,
        };
        if let Some(p) = path {
            c.open(p);
        } else {
            // 新規は空白のブック(発注者 2026-08-06。見本を入れない —
            // 試験は自前で表を作り、触れる見本は sample/*.xlsx にある)
            c.status = ui::t!("セルを選んで打つ。Enter で確定して下へ、Ctrl+S で保存").into();
        }
        c.sync_input();
        // 読み取り専用の勧めは、開いたときに言わないと意味がない
        if c.book.read_only_rec {
            c.status = ui::t!(
                "このブックは読み取り専用が勧められています(鍵ではありません — 直せます。保護タブで外せます)"
            )
            .into();
        }
        // **前回落ちた跡があれば黙っていない。** 自動復旧の控えが
        // 残っているのは、前回きちんと保存せずに終わったということ
        let stale = Self::stale_recovers();
        if !stale.is_empty() {
            c.status = ui::tf!(
                "前に保存できずに終わったブックが {} 件あります(保護タブの隣の「復旧」で開けます)",
                stale.len()
            )
            .into();
        }
        // settings.toml の key.* に読めない行があれば、開いた時に言う
        // (黙って捨てると、効かない理由を利用者が探せない)
        if let Some(w) = ui::key_warnings().first() {
            c.status = w.clone().into();
        }
        // **UDF とマクロを分けた**(2026-08-16)。式から呼べるのは funcs の
        // .py だけになったので、前の置き方のままの人には**黙って #PY? に
        // しない** — 移し先を言う
        if pyrun::modules_in(&pyrun::funcs_dir()).is_empty()
            && !pyrun::plugin_modules().is_empty()
        {
            c.status = ui::tf!(
                "式から呼ぶ関数は {} に移してください(plugins は人が押すマクロの置き場になりました)",
                pyrun::funcs_dir().display().to_string()
            )
            .into();
        }
        c
    }

    pub(crate) fn sheet(&self) -> &sheet::Sheet {
        &self.book.sheets[self.active]
    }
    pub(crate) fn sheet_mut(&mut self) -> &mut sheet::Sheet {
        let a = self.active;
        &mut self.book.sheets[a]
    }

    /// 参照の見せ方(R1C1 のときはカーソル基準の R[..]C[..] に)
    pub(crate) fn ref_disp(&self, p: Pos) -> String {
        if self.book.r1c1 {
            sheet::model::formula_to_r1c1(&p.a1(), self.cursor)
        } else {
            p.a1()
        }
    }

    /// 記録に1行足す(記録していなければ何もしない)。
    /// **Python の言葉で書く** — officework.calc(xlwings 流)の形。
    /// 記録した物がそのまま走るのが要件なので、画面の言葉に訳さない
    pub(crate) fn rec_line(&mut self, line: String) {
        // **選んでいる所が変わっていたら、先に選択の行を置く**(2026-08-16
        // 発注者「セルの選択等セル操作についても同じ」)。矢印キーやマウスの
        // 一手ごとに書くと洪水になるので、**何かを記録する直前に**、前に
        // 書いた選択と違っていたら1行入れる。Excel の記録も選択を残す
        if self.rec.is_some() {
            let (a, b) = self.sel_rect();
            let now = if a == b { a.a1() } else { format!("{}:{}", a.a1(), b.a1()) };
            if self.rec_sel.as_deref() != Some(now.as_str()) {
                let sheet = self.book.sheets[self.active].name.clone();
                let v = sheet_var(&sheet);
                if let Some(r) = &mut self.rec {
                    r.push(format!("s{v}[{now:?}].select()"));
                }
                self.rec_sel = Some(now);
            }
        }
        if let Some(r) = &mut self.rec {
            // 同じ行が続くのは押し間違い(太字を2回など)。畳まない —
            // **打った通りを残す**のが記録で、整えるのは人の仕事
            r.push(line);
        }
    }

    /// セルへの書き込みを記録する。値は Python のリテラルにする
    pub(crate) fn rec_set(&mut self, p: Pos, text: &str) {
        if self.rec.is_none() {
            return;
        }
        let sheet = self.book.sheets[self.active].name.clone();
        let lit = if text.is_empty() {
            "None".to_string()
        } else if text.starts_with('=') {
            format!("{text:?}")
        } else if text.parse::<f64>().is_ok() {
            text.to_string()
        } else {
            format!("{text:?}")
        };
        self.rec_line(format!("s{}[{:?}].value = {}", sheet_var(&sheet), p.a1(), lit));
    }

    /// 記録を始める(前の記録は捨てる)
    pub(crate) fn rec_start(&mut self) {
        self.rec = Some(Vec::new());
        self.rec_sel = None;
        self.status = ui::t!(
            "記録を始めました。操作すると Python の行になります(もう一度押すと止まります)"
        )
        .into();
    }

    /// 記録を止めて、**記録そのもの**を返す。
    ///
    /// 2026-08-16 発注者「記録した台本の頭に wb = xw.Book(径路) を持ってくるのが
    /// おかしい。記録だけを記述すればいい。記録をそのままで動かそうとするのが
    /// おかしい」。
    ///
    /// 前はブックを開く行とシートを束ねる行を頭に足して「そのまま走ります」と
    /// 名乗っていた。**走らなかった** — calc がそのブックを開いたままなので
    /// `xw.Book(径路)` は「未保存の変更があります」で断られる。走ると言って
    /// 走らないより、**記録は記録だと言う**方がいい。走らせる物にするのは人の手
    /// (どのブックに掛けるかは、記録した当人しか決められない)。
    pub(crate) fn rec_stop(&mut self) -> Option<String> {
        let lines = self.rec.take()?;
        let sheet = self.book.sheets[self.active].name.clone();
        // 何をどこで記録したかは**記録の一部**。走らせる仕掛けではない。
        // まだファイルになっていないブックは行ごと出さない(空の「ブック:」を
        // 出すくらいなら黙る。訳の鍵も増やさない — これは画面ではなくファイル)
        let book = self
            .path
            .as_ref()
            .map(|p| format!("ブック: {}\n", p.display()))
            .unwrap_or_default();
        let mut out = format!(
            "\"\"\"calc の操作の記録。\n\n{book}シート: {sheet}\n\"\"\"\n\n"
        );
        if lines.is_empty() {
            out.push_str("# 記録された操作はありません\n");
        } else {
            for l in &lines {
                out.push_str(l);
                out.push('\n');
            }
        }
        Some(out)
    }

    pub(crate) fn sync_input(&mut self) {
        let mut s = self.sheet().get(self.cursor).map(|c| c.editable()).unwrap_or_default();
        // R1C1: 見せるときだけ変換(中身は A1 のまま)
        if self.book.r1c1 {
            if let Some(body) = s.strip_prefix('=') {
                s = format!("={}", sheet::model::formula_to_r1c1(body, self.cursor));
            }
        }
        // **昔ながらの配列数式は { } で囲んで見せる。** 普通の式と
        // 見分けがつかないと、直そうとして Enter で潰してしまう
        if self.sheet().cse.contains_key(&self.cursor) && s.starts_with('=') {
            s = format!("{{{s}}}");
        }
        self.input = Editor::new(&s);
        self.edit_armed = false; // セルを移った=編集は仕切り直し
        if self.pick_kind == "fn-complete" {
            self.close_pick(); // 補完の一覧も畳む
        }
        // 入力メッセージ付きの規則のセルに乗ったら、その説明を出す
        if let Some((t, m)) = self
            .sheet()
            .validation_at(self.cursor)
            .and_then(|v| v.input_msg.clone())
        {
            self.status = if t.is_empty() {
                m.into()
            } else if m.is_empty() {
                t.into()
            } else {
                format!("{t}: {m}").into()
            };
        } else if let Some(i) = self.pivot_at(self.cursor) {
            // ピボットに乗ったら、名前と操作の場所を言う(文脈タブの案内)
            let name = self.book.pivots[i].name.clone();
            self.status = ui::tf!(
                "{} の上です — 操作は「ピボットテーブル」のタブで(更新・総計・小計・レイアウト。表を崩す操作は締まります)",
                if name.is_empty() { ui::t!("ピボット").to_string() } else { name }
            )
            .into();
        }
    }

    /// 数式バーの内容をセルに入れて再計算する。
    /// いまの表を控える(次の操作を戻せるように)。やり直しの控えは捨てる。
    pub(crate) fn checkpoint(&mut self) {
        self.edits += 1;
        self.undo_stack
            .push(vec![(self.active, self.book.sheets[self.active].clone())]);
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// 全シートを1手として控える(Python の実行など、どこを変えるか
    /// 分からない操作の前に)。
    pub(crate) fn checkpoint_book(&mut self) {
        self.edits += 1;
        self.undo_stack.push(
            self.book
                .sheets
                .iter()
                .cloned()
                .enumerate()
                .collect(),
        );
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// 控えたシートを見せる(別のシートの操作を戻したなら、そこへ移る —
    /// 見えない場所で表が変わるのは事故のもと)。
    pub(crate) fn show_sheet(&mut self, idx: usize) {
        if idx != self.active && idx < self.book.sheets.len() {
            self.remember_ui();
            self.active = idx;
            self.restore_ui();
            self.anchor = None;
            self.auto_filter = None;
            self.filter_panel = None;
        }
    }

    pub(crate) fn undo_sheet(&mut self) {
        let Some(batch) = self.undo_stack.pop() else {
            self.status = ui::t!("戻すものがありません").into();
            return;
        };
        let mut redo = Vec::new();
        let first = batch.first().map(|(i, _)| *i);
        for (idx, prev) in batch {
            if idx < self.book.sheets.len() {
                redo.push((idx, self.book.sheets[idx].clone()));
                self.book.sheets[idx] = prev;
                recalc_book(&mut self.book, idx);
            }
        }
        self.redo_stack.push(redo);
        if let Some(i) = first {
            self.show_sheet(i);
        }
        self.dirty = true;
        self.sync_input();
        self.status = ui::t!("戻しました").into();
    }

    pub(crate) fn redo_sheet(&mut self) {
        let Some(batch) = self.redo_stack.pop() else {
            self.status = ui::t!("やり直すものがありません").into();
            return;
        };
        let mut undo = Vec::new();
        let first = batch.first().map(|(i, _)| *i);
        for (idx, next) in batch {
            if idx < self.book.sheets.len() {
                undo.push((idx, self.book.sheets[idx].clone()));
                self.book.sheets[idx] = next;
                recalc_book(&mut self.book, idx);
            }
        }
        self.undo_stack.push(undo);
        if let Some(i) = first {
            self.show_sheet(i);
        }
        self.dirty = true;
        self.sync_input();
        self.status = ui::t!("やり直しました").into();
    }

    /// いまのシートのカーソル・窓・固定を控える。
    pub(crate) fn remember_ui(&mut self) {
        while self.sheet_ui.len() < self.book.sheets.len() {
            self.sheet_ui.push((Pos::new(0, 0), Pos::new(0, 0), None));
        }
        self.sheet_ui[self.active] = (self.cursor, self.view, self.frozen);
    }

    pub(crate) fn restore_ui(&mut self) {
        let (c, v, f) = self
            .sheet_ui
            .get(self.active)
            .copied()
            .unwrap_or((Pos::new(0, 0), Pos::new(0, 0), None));
        self.cursor = c;
        self.view = v;
        self.frozen = f;
    }

    /// ファイルの固定枠を画面へ移す。**ブックを入れ替えた直後に呼ぶ。**
    /// これが無いと、「見出し行を固定」と書いてあるファイルを固定なしで開く —
    /// 画面とファイルが別のことを言う状態になる
    pub(crate) fn freeze_from_book(&mut self) {
        self.sheet_ui = self
            .book
            .sheets
            .iter()
            .map(|sh| {
                let f = sh.freeze.map(|f| Pos::new(f.frozen_rows, f.frozen_columns));
                (Pos::new(0, 0), Pos::new(0, 0), f)
            })
            .collect();
        self.frozen = self.sheet_ui.get(self.active).and_then(|u| u.2);
    }

    /// 画面の固定枠をモデルへ移す。**保存の直前に呼ぶ。**
    /// これが無いと、calc で固定してもファイルに載らない。
    /// `frozen` は画面の状態なので、シートごとの控え(`sheet_ui`)から集める —
    /// いま見ていないシートの固定も落とさないため
    pub(crate) fn freeze_into_book(&mut self) {
        self.remember_ui();
        for (i, sh) in self.book.sheets.iter_mut().enumerate() {
            sh.freeze = self.sheet_ui.get(i).and_then(|u| u.2).and_then(|p| {
                // (0, 0) は「固定していない」— 空の固定枠を書かない
                (p.row > 0 || p.col > 0).then_some(sheet::model::FreezePane {
                    frozen_rows: p.row,
                    frozen_columns: p.col,
                })
            });
        }
    }

    /// 画面に出ている行の並び(絞り込み中はその行だけ。グループ化で畳んだ行は
    /// 飛ばす)。描画と当たり判定で共有する。
    /// コメントの一覧に並べる行(**ブック全体**)。並べ方は板が持つ。
    ///
    /// 板を開いていなければ空 — 開いていない板のために毎回全シートを
    /// 舐めない
    pub(crate) fn comment_rows(&self) -> Vec<CommentRow> {
        let Some(cl) = &self.comment_list else { return Vec::new() };
        let mut rows: Vec<CommentRow> = Vec::new();
        for (si, sh) in self.book.sheets.iter().enumerate() {
            for (at, th) in &sh.comments {
                let head = th.entries.first();
                rows.push(CommentRow {
                    sheet: si,
                    at: *at,
                    who: head.map(|e| e.who.clone()).unwrap_or_default(),
                    when: head.map(|e| e.when.clone()).unwrap_or_default(),
                    done: th.done,
                    text: head.map(|e| e.text.clone()).unwrap_or_default(),
                    replies: th.entries.len().saturating_sub(1),
                });
            }
        }
        sort_comments(&mut rows, cl.sort, cl.desc);
        rows
    }

    /// コメントを消す。範囲は本家と同じ3つ(`here` / `mine` / `all`)。
    ///
    /// **`mine` は筋ごと消さない。** 自分の発言だけを抜いて、他の人の返信は
    /// 残す — 自分が頭を書いた筋でも、付いた返信は他人の言葉なので
    /// 黙って落とさない。頭が抜けたら、残った先頭がその筋の頭になる。
    ///
    /// **`all` はブック全体。** 一覧の板と範囲を揃えてある
    pub(crate) fn delete_comments(&mut self, scope: &str) {
        // 名乗りは器(settings.toml)から。**取るのはここだけ** —
        // 芯は名前を引数で受けるので、試験は設定ファイルを触らない
        let me = ui::settings::get("user_name").unwrap_or_default();
        self.delete_comments_by(scope, me.trim());
    }

    /// [`Self::delete_comments`] の芯(`me` = 自分の名乗り)
    pub(crate) fn delete_comments_by(&mut self, scope: &str, me: &str) {
        match scope {
            "here" => {
                let p = self.cursor;
                if self.sheet().comments.contains_key(&p) {
                    self.checkpoint();
                    self.book.sheets[self.active].comments.remove(&p);
                    self.dirty = true;
                    self.status =
                        ui::tf!("{} のコメントを外しました(Ctrl+Z で戻せます)", p.a1()).into();
                } else {
                    self.status = ui::t!("このセルにコメントはありません").into();
                }
            }
            "mine" => {
                // 名乗りが決まっていないと「自分の」が決まらない。
                // **名無しの発言を自分のものと決めつけない**
                if me.is_empty() {
                    self.status = ui::t!("名乗りが決まっていません(詳細設定の「コメントの名乗り」を入れると、自分のコメントだけ消せます)").into();
                    return;
                }
                let (mut said, mut threads) = (0usize, 0usize);
                for sh in &self.book.sheets {
                    for th in sh.comments.values() {
                        let n = th.entries.iter().filter(|e| e.who == me).count();
                        said += n;
                        if n == th.entries.len() {
                            threads += 1;
                        }
                    }
                }
                if said == 0 {
                    self.status =
                        ui::tf!("{} さんのコメントはありません", me).into();
                    return;
                }
                self.checkpoint_book();
                for sh in &mut self.book.sheets {
                    for th in sh.comments.values_mut() {
                        th.entries.retain(|e| e.who != me);
                    }
                    // 空になった筋は消す(空の筋は「コメントが無い」と同じ)
                    sh.comments.retain(|_, th| !th.entries.is_empty());
                }
                self.dirty = true;
                self.status = ui::tf!(
                    "自分の発言 {} 件を消しました(うち {} 筋は丸ごと。他の人の返信は残しています。Ctrl+Z で戻せます)",
                    said, threads
                )
                .into();
            }
            _ => {
                let n: usize = self.book.sheets.iter().map(|s| s.comments.len()).sum();
                if n == 0 {
                    self.status = ui::t!("このブックにコメントはありません").into();
                    return;
                }
                self.checkpoint_book();
                for sh in &mut self.book.sheets {
                    sh.comments.clear();
                }
                self.dirty = true;
                self.status =
                    ui::tf!("ブック全体の {} 件のコメントを消しました(Ctrl+Z で戻せます)", n)
                        .into();
            }
        }
    }

    /// スライサーで残る行か(選びが空なら全部残る)。1行目=見出しは常に残す。
    ///
    /// **板が何枚あっても、全部の板を通った行だけ残る**(かつ)。
    /// Excel と同じ — 「品目=りんご」と「店=北」を別々の板で押したら
    /// 両方に当てはまる行だけが見える
    pub(crate) fn slicer_keeps(&self, r: u32) -> bool {
        if r == 0 {
            return true; // 見出しの行は常に残す
        }
        self.slicers.iter().all(|sl| self.slicer_keeps_one(sl, r))
    }

    /// 板1枚ぶんの判定([`Self::slicer_keeps`] の中身)
    pub(crate) fn slicer_keeps_one(&self, sl: &Slicer, r: u32) -> bool {
        if sl.sel.is_empty() {
            return true;
        }
        let v = self
            .sheet()
            .get(Pos::new(r, sl.col))
            .map(|c| c.value.display())
            .unwrap_or_default();
        let v = if v.is_empty() { ui::t!("(空白)").to_string() } else { v };
        sl.sel.contains(&v)
    }

    // ---- Alt のキーヒント(2026-08-13、台帳「Alt キーヒント」)----

    /// キーヒントを出す/畳む(Alt を単独で押して離したとき)
    pub(crate) fn toggle_key_hints(&mut self) {
        if self.key_hint.take().is_some() {
            self.status = ui::t!("キーヒントを畳みました").into();
            return;
        }
        // 小窓中はリボンが無効 — 押せない物に札を配らない
        if self.dialog_open() {
            return;
        }
        self.key_hint = Some(String::new());
        self.status =
            ui::t!("キーヒント: 札の文字を打つと段を選び、もう一度でボタンを押します(Esc でやめる)")
                .into();
    }

    /// いま札を配る相手。**段を選ぶ前は段、選んだあとはその段のボタン。**
    ///
    /// 返すのは (札, 引き当ての鍵)。段なら鍵は段の番号、ボタンなら命令の id。
    /// **押せないボタンには札を配らない** — 押しても何も起きない札を
    /// 見せるのは「できないものを、できるように見せない」に反する
    pub(crate) fn hint_targets(&self) -> Vec<(String, HintTo)> {
        let tabs = ribbon::calc_tabs();
        let Some(typed) = &self.key_hint else { return Vec::new() };
        // 段の札(隠れている文脈タブは配らない)
        let visible: Vec<usize> = (0..tabs.len())
            .filter(|i| !self.ctx_tab_hidden(&tabs[*i]))
            .collect();
        if typed.is_empty() || !typed.starts_with('#') {
            return ui::key_hints(visible.len())
                .into_iter()
                .zip(visible.into_iter().map(HintTo::Tab))
                .collect();
        }
        // 段を選んだあと(頭に # を付けて見分ける)。押せるボタンだけ
        let ids: Vec<&'static str> = tabs[self.tab]
            .cmds
            .iter()
            .filter(|c| Calc::HANDLED.contains(&c.id))
            .map(|c| c.id)
            .collect();
        ui::key_hints(ids.len())
            .into_iter()
            .zip(ids.into_iter().map(HintTo::Cmd))
            .collect()
    }

    /// キーヒントに1文字打たれた。**当たれば実行、外れれば畳む** —
    /// 当たらない札を打ったまま居座ると、次に打つ字が格子へ行くのか
    /// 札へ行くのか分からなくなる
    pub(crate) fn hint_type(&mut self, ch: &str, cx: &mut Context<Self>) {
        let Some(typed) = self.key_hint.clone() else { return };
        let head = typed.starts_with('#');
        let now = format!("{}{}", typed.trim_start_matches('#'), ch.to_uppercase());
        let targets = self.hint_targets();
        // 打った分で始まる札が無ければ、打ち間違い
        if !targets.iter().any(|(h, _)| h.starts_with(&now)) {
            self.key_hint = None;
            self.status = ui::tf!("その札はありません(「{}」)", ch).into();
            cx.notify();
            return;
        }
        match targets.iter().find(|(h, _)| *h == now) {
            Some((_, HintTo::Tab(i))) => {
                let i = *i;
                self.prev_tab = self.tab;
                self.tab = i;
                self.key_hint = Some("#".into()); // 段の中の札へ進む
                self.status = ui::tf!("{} の段(札を打つと押します。Esc でやめる)",
                    ribbon::calc_tabs()[i].name).into();
            }
            Some((_, HintTo::Cmd(id))) => {
                let id = *id;
                self.key_hint = None;
                self.run_from_ribbon(id, 0.0, cx);
            }
            // まだ途中(2文字の札の1文字目)
            None => self.key_hint = Some(format!("{}{now}", if head { "#" } else { "" })),
        }
        cx.notify();
    }

    /// 文脈タブ(ピボット・表のデザイン)がいま隠れているか。
    /// 描くところと同じ見分け方を使う — 別々に書くと札とタブがずれる
    pub(crate) fn ctx_tab_hidden(&self, tb: &ribbon::Tab) -> bool {
        Self::ctx_tab_hidden_with(
            tb,
            self.pivot_at(self.cursor).is_some(),
            self.sheet().tables.iter().any(|t| t.contains(self.cursor)),
        )
    }

    /// [`Self::ctx_tab_hidden`] の芯。描くところは `self` を借りたまま
    /// `self.tab` を書き換えるので、旗を先に取ってからこちらを呼ぶ
    pub(crate) fn ctx_tab_hidden_with(tb: &ribbon::Tab, on_pivot: bool, in_table: bool) -> bool {
        (tb.cmds.iter().any(|c| c.id == "pivot-layout") && !on_pivot)
            || (tb.cmds.iter().any(|c| c.id == "td-header") && !in_table)
    }

    /// 板の左上(格子の面の px)。`at` が無ければ**右から順に自動で並べる**。
    ///
    /// 自動のときも数で返す — ドラッグの始まりに「いまどこに居るか」が要る。
    /// `.right()` で置くと、掴んだ瞬間に位置が分からない
    pub(crate) fn slicer_origin(&self, i: usize) -> (f32, f32) {
        let Some(sl) = self.slicers.get(i) else { return (0.0, 0.0) };
        if let Some(at) = sl.at {
            return at;
        }
        // 自分より前の板の幅を足して、右端から左へ寄せる
        let pane_w = self.pane_box.get().2;
        let mut right = 24.0;
        for j in 0..i {
            right += self.slicers[j].w + 8.0;
        }
        ((pane_w - right - sl.w).max(0.0), ROW_H + 16.0)
    }

    /// 板の大きさを変える。**一定の比率**が入っていれば、片方を動かすと
    /// もう片方も同じ率で動く(本家の「一定の割合」)
    pub(crate) fn slicer_resize(&mut self, i: usize, is_w: bool, d: f32) {
        let Some(sl) = self.slicers.get_mut(i) else { return };
        let (w0, h0) = (sl.w, sl.h);
        if is_w {
            sl.w = (w0 + d).clamp(120.0, 640.0);
            if sl.ratio && w0 > 0.0 {
                sl.h = (h0 * sl.w / w0).clamp(80.0, 900.0);
            }
        } else {
            sl.h = (h0 + d).clamp(80.0, 900.0);
            if sl.ratio && h0 > 0.0 {
                sl.w = (w0 * sl.h / h0).clamp(120.0, 640.0);
            }
        }
        let (w, h) = (sl.w, sl.h);
        self.status = ui::tf!("大きさ: {}×{}px", format!("{w:.0}"), format!("{h:.0}")).into();
    }

    /// 板を掴んだ(見出しの上でマウスを押した)
    pub(crate) fn slicer_grab(&mut self, i: usize, x: f32, y: f32) {
        let o = self.slicer_origin(i);
        self.slicer_drag = Some((i, (x, y), o));
    }

    /// 板を引いている。**格子の面から出さない** — 面の外へ出すと掴み直せない
    pub(crate) fn slicer_drag_at(&mut self, x: f32, y: f32) {
        let Some((i, (gx, gy), (ox, oy))) = self.slicer_drag else { return };
        let (_, _, pw, ph) = self.pane_box.get();
        let Some(sl) = self.slicers.get_mut(i) else { return };
        let nx = (ox + (x - gx)).clamp(0.0, (pw - sl.w).max(0.0));
        let ny = (oy + (y - gy)).clamp(0.0, (ph - 40.0).max(0.0));
        sl.at = Some((nx, ny));
    }

    /// いま触っている板を1枚閉じる(Esc)。閉じたら true。
    pub(crate) fn close_slicer(&mut self) -> bool {
        if self.slicers.is_empty() {
            return false;
        }
        let i = self.slicer_sel.min(self.slicers.len() - 1);
        let col = self.slicers.remove(i).col;
        self.slicer_sel = i.min(self.slicers.len().saturating_sub(1));
        // 板が全部無くなったら設定の板も畳む(相手のいない設定は出さない)
        self.slicer_cfg &= !self.slicers.is_empty();
        self.status = ui::tf!("{} 列のスライサーを閉じました", col_name(col)).into();
        true
    }

    /// いま触っている板。**番号がずれていたら最後の板**に寄せる —
    /// 板を閉じたあとに Alt+S が何も起こさないのを避ける
    pub(crate) fn slicer_cur(&mut self) -> Option<&mut Slicer> {
        if self.slicers.is_empty() {
            return None;
        }
        if self.slicer_sel >= self.slicers.len() {
            self.slicer_sel = self.slicers.len() - 1;
        }
        self.slicers.get_mut(self.slicer_sel)
    }

    /// 窓に入る行数。**セルの大きさは固定**で、窓が大きいほど多くの行が
    /// 見える(発注者 2026-08-06)。まだ窓の大きさを知らない(描画前・試験)
    /// なら従来の既定。少し多めに数えても、はみ出しは器が刈る
    pub(crate) fn rows_fit(&self) -> u32 {
        self.rows_fit_in(self.view_h_px)
    }

    pub(crate) fn rows_fit_in(&self, budget: f32) -> u32 {
        if self.view_h_px <= 0.0 {
            return ROWS; // 描画前・試験は従来の既定
        }
        let (mut h, mut n, mut r) = (0.0f32, 0u32, self.view.row);
        while h < budget && n < 300 {
            h += self.row_px(r);
            r += 1;
            n += 1;
        }
        n.max(3)
    }

    /// 端の追従・ページ移動用: 額縁(リボン・数式バー・シートのタブ・状態行)を
    /// 差し引いた「確実に丸ごと見える」行数
    pub(crate) fn rows_snug(&self) -> u32 {
        self.rows_fit_in(self.view_h_px - 270.0)
    }

    /// 窓に入る列数(rows_fit と同じ役割)
    pub(crate) fn cols_fit(&self) -> u32 {
        self.cols_fit_in(self.view_w_px)
    }

    pub(crate) fn cols_fit_in(&self, budget: f32) -> u32 {
        if self.view_w_px <= 0.0 {
            return COLS;
        }
        let (mut w, mut n, mut c) = (0.0f32, 0u32, self.view.col);
        while w < budget && n < 120 {
            w += self.col_px(c);
            c += 1;
            n += 1;
        }
        n.max(2)
    }

    pub(crate) fn cols_snug(&self) -> u32 {
        self.cols_fit_in(self.view_w_px - HEAD_W - 24.0)
    }

    pub(crate) fn visible_rows(&self) -> Vec<u32> {
        let hidden = &self.sheet().row_hidden;
        let fit = self.rows_fit();
        if self.filter_active() {
            // 絞り込み中は頭から詰めて見せる(範囲の後ろの行も続けて出す)
            let (rows, _) = self.sheet().extent();
            let last = self.auto_filter.as_ref().map(|f| f.range.1.row + 1).unwrap_or(0);
            return (0..rows.max(last))
                .filter(|r| {
                    !hidden.contains(r) && self.filter_keeps(*r) && self.slicer_keeps(*r)
                })
                .take(fit as usize)
                .collect();
        }
        if self.slicers.iter().any(|sl| !sl.sel.is_empty()) {
            // スライサーで絞る: 見出し+選んだ値の行(絞り込みと同じ流儀)
            let (rows, _) = self.sheet().extent();
            (0..rows)
                .filter(|r| !hidden.contains(r) && self.slicer_keeps(*r))
                .take(fit as usize)
                .collect()
        } else {
            // 畳んだ行のぶん多めに見て、画面の行数まで詰める
            let extra = hidden.len() as u32;
            grid_rows(self.frozen, self.view, fit + extra)
                .into_iter()
                .filter(|r| !hidden.contains(r))
                .take(fit as usize)
                .collect()
        }
    }

    /// 画面に出ている列の並び(畳んだ列は飛ばす)。visible_rows と同じ役割。
    pub(crate) fn visible_cols(&self) -> Vec<u32> {
        let hidden = &self.sheet().col_hidden;
        let extra = hidden.len() as u32;
        let fit = self.cols_fit();
        let mut v: Vec<u32> = grid_cols(self.frozen, self.view, fit + extra)
            .into_iter()
            .filter(|c| !hidden.contains(c))
            .take(fit as usize)
            .collect();
        if self.sheet().rtl {
            // 右から左のシートは列を逆順に並べる。**描画も当たり判定も
            // この一点を通る**ので、掴む場所と見える場所がずれない
            v.reverse();
        }
        v
    }

    /// 格子の中の位置(px、格子領域の左上原点)からセルを逆算する。
    /// 見出しの行の上なら None。
    pub(crate) fn cell_at(&self, x: f32, y: f32) -> Option<Pos> {
        if x < self.head_w() || y < self.head_h() {
            return None;
        }
        Some(Pos { row: self.row_at(y)?, col: self.col_at(x)? })
    }

    /// この x はどの列の上か(見出し・セルのどちらでも)。
    pub(crate) fn col_at(&self, x: f32) -> Option<u32> {
        let cols: Vec<(u32, f32)> = self.visible_cols()
            .into_iter()
            .map(|c| (c, self.col_px(c)))
            .collect();
        index_at(&cols, self.head_w(), x)
    }

    pub(crate) fn row_at(&self, y: f32) -> Option<u32> {
        let rows: Vec<(u32, f32)> = self
            .visible_rows()
            .into_iter()
            .map(|r| (r, self.row_px(r)))
            .collect();
        index_at(&rows, self.head_h(), y)
    }

    /// 列をまるごと選ぶ(使われている高さまで)。`a` が起点、`b` が動く側。
    /// 列をまるごと選ぶ。**表の端まで**(使われている範囲で止めない —
    /// 発注者 2026-08-14。列の見出しを押したら、その列は全部が対象。
    /// 空の行に書式を掛ける・貼り付ける、が普通にできる)
    pub(crate) fn select_cols(&mut self, a: u32, b: u32) {
        self.anchor = Some(Pos::new(LAST_ROW, a));
        self.cursor = Pos::new(0, b);
        self.sync_input();
        let (lo, hi) = (a.min(b), a.max(b));
        self.status = if lo == hi {
            ui::tf!("{}列を選択しました(列ぜんぶ)", col_name(lo)).into()
        } else {
            ui::tf!("{}〜{}列を選択しました(列ぜんぶ)", col_name(lo), col_name(hi)).into()
        };
    }

    /// 行をまるごと選ぶ。**表の端まで**(列と同じ決め)
    pub(crate) fn select_rows(&mut self, a: u32, b: u32) {
        self.anchor = Some(Pos::new(a, LAST_COL));
        self.cursor = Pos::new(b, 0);
        self.sync_input();
        let (lo, hi) = (a.min(b), a.max(b));
        self.status = if lo == hi {
            ui::tf!("{}行を選択しました(行ぜんぶ)", lo + 1).into()
        } else {
            ui::tf!("{}〜{}行を選択しました(行ぜんぶ)", lo + 1, hi + 1).into()
        };
    }

    /// 見出しの行の上の、列幅・行高の取っ手(境界 ±GRIP px)。Some((列か, 番号))。
    /// 描画・cell_at と同じ並び(固定・窓・絞り込み)を使う —
    /// ずれると別の境界を掴んでしまう。
    pub(crate) fn size_grip_at(&self, x: f32, y: f32) -> Option<(bool, u32)> {
        if !self.show_headers {
            return None; // 見出しが無ければ掴む縁も無い
        }
        if y < ROW_H && x >= HEAD_W {
            let cols: Vec<(u32, f32)> = self.visible_cols()
                .into_iter()
                .map(|c| (c, self.col_px(c)))
                .collect();
            return grip_hit(&cols, HEAD_W, x).map(|c| (true, c));
        }
        if x < HEAD_W && y >= ROW_H {
            let rows: Vec<(u32, f32)> = self
                .visible_rows()
                .into_iter()
                .map(|r| (r, self.row_px(r)))
                .collect();
            return grip_hit(&rows, ROW_H, y).map(|r| (false, r));
        }
        None
    }

    /// 境界を掴んだまま動いた。列幅・行高をその場で変える(見ながら合わせる)。
    /// 最小幅で止める — ゼロにすると列が消えて掴み直せない。
    pub(crate) fn size_drag_at(&mut self, x: f32, y: f32) {
        if std::env::var_os("JO_MOUSE_LOG").is_some() {
            eprintln!("move x={x:.1} y={y:.1} size_drag={}", self.size_drag.is_some());
        }
        let Some(d) = &self.size_drag else { return };
        let (col, idx, grab, base, moved) = (d.col, d.idx, d.grab, d.base, d.moved);
        if !moved {
            self.checkpoint();
            if let Some(d) = &mut self.size_drag {
                d.moved = true;
            }
        }
        if col {
            let w = (base + x - grab).max(9.0) / PX_PER_CHW;
            let w = (w * 100.0).round() / 100.0;
            self.sheet_mut().col_width.insert(idx, w);
            self.status = ui::tf!("{}列の幅: {}({:.0}px)", col_name(idx), w, w * PX_PER_CHW)
            .into();
        } else {
            let pt = ((base + y - grab) / self.zoom).max(6.0) * 15.0 / 24.0;
            let pt = (pt * 100.0).round() / 100.0;
            self.sheet_mut().row_height.insert(idx, pt);
            self.status = ui::tf!("{}行の高さ: {}pt({:.0}px)", idx + 1, pt, pt * 24.0 / 15.0)
            .into();
        }
        self.dirty = true;
    }

    /// マウスの左を押した(格子領域の座標)。押したセルが選択の始まり。
    /// メニューが出ていたら閉じる(項目の上の押下は stop_propagation でここに来ない)。
    pub(crate) fn mouse_down_at(&mut self, x: f32, y: f32, shift: bool, ctrl: bool, clicks: usize) {
        self.menu_at = None;
        self.menu_direct = false;
        self.close_pick();
        self.border_pal = None;
        // 表を押したら会話の欄から焦点が離れる(打鍵はセルへ戻る)
        self.chat_focus = false;
        // mouse-up を取り逃していても、新しい押下で必ず仕切り直す(自癒)
        self.size_drag = None;
        self.drag = None;
        self.head_drag = None;
        self.fill_drag = None;
        self.shape_drag = None;
        self.shape_rot = None;
        self.pt_drag = None;
        if std::env::var_os("JO_MOUSE_LOG").is_some() {
            eprintln!(
                "down x={x:.1} y={y:.1} clicks={clicks} grip={:?}",
                self.size_grip_at(x, y)
            );
        }
        // 描画の道具が出ていれば筆が最優先(セルは触らない)
        if let Some(t) = self.tool {
            if x >= self.head_w() && y >= self.head_h() {
                if t == 2 {
                    // 消しゴム: なぞった線を1筆消す
                    match self.ink_at(x, y) {
                        Some(i) => {
                            self.checkpoint();
                            self.sheet_mut().shapes_new.remove(i);
                            self.dirty = true;
                            self.status = ui::t!("1筆消しました(Ctrl+Z で戻せます)").into();
                        }
                        None => self.status = ui::t!("線の上をなぞってください").into(),
                    }
                } else {
                    self.ink_cur = Some(vec![(x, y)]);
                }
                return;
            }
        }
        // ポイント編集の取っ手。**図形の体より先に見る** — 点は図形の
        // 上に載っているので、先に見ないと枠のドラッグに取られる
        if let Some(i) = self.point_edit {
            if ctrl {
                // Ctrl+クリック = 頂点の追加/削除。当たれば終わり
                if self.point_add_or_remove(x, y) {
                    return;
                }
            } else if let Some((k, kind)) = self.point_hit(i, x, y) {
                self.commit();
                // **頂点のダブルクリックで角 ⇄ 曲線。** 制御点を出す口が
                // 他に無いと、曲げられない図形しか作れない
                if clicks >= 2 && kind == PtHandle::Vertex {
                    self.point_toggle_curve(k);
                    return;
                }
                self.checkpoint();
                self.pt_drag = Some((k, kind));
                self.status = ui::t!("点をつまみました(Ctrl+クリックで追加/削除)").into();
                return;
            }
        }
        // 選択中の図形の回転の取っ手(枠の上の丸)。図形の体より先に見る
        if let Some(i) = self.shape_sel {
            if let Some((hx, hy)) = self.shape_rot_handle(i) {
                if (x - hx).hypot(y - hy) <= 9.0 {
                    self.commit();
                    self.checkpoint();
                    self.shape_rot = Some(i);
                    self.status = ui::t!("回します(Shift で15度刻み)").into();
                    return;
                }
            }
        }
        // 浮いている図形が最優先(セルの上に描かれているので)
        if let Some((i, (sx, sy), corner)) = self.shape_at(x, y) {
            self.commit();
            // Ctrl+クリック = 選択に足す/外す(整列・分布の下ごしらえ)
            if ctrl {
                if self.shape_sel == Some(i) {
                    self.shape_sel = if self.shape_multi.is_empty() {
                        None
                    } else {
                        Some(self.shape_multi.remove(0))
                    };
                } else if let Some(k) = self.shape_multi.iter().position(|&m| m == i) {
                    self.shape_multi.remove(k);
                } else if self.shape_sel.is_none() {
                    self.shape_sel = Some(i);
                } else {
                    self.shape_multi.push(i);
                }
                let n = self.shape_sel.is_some() as usize + self.shape_multi.len();
                self.status = ui::tf!(
                    "{} 個の図形を選んでいます(右クリック→整列で揃えます)",
                    n
                )
                .into();
                return;
            }
            self.checkpoint();
            self.shape_sel = Some(i);
            self.shape_multi.clear();
            self.shape_drag = Some((i, (x, y), (sx, sy), corner));
            self.status = if corner {
                ui::t!("右下を引いて大きさを変えます").into()
            } else {
                ui::t!("図形を選びました(ドラッグで移動 / 右下で大きさ / Del で削除)").into()
            };
            return;
        }
        self.shape_sel = None;
        // 選択が外れたらポイント編集も畳む — 残すと、選んでいない図形の
        // 点だけが浮いて、押しても何も起きない取っ手になる
        self.point_edit = None;
        self.pt_drag = None;
        self.shape_multi.clear();
        // 浮いている画像(グラフ)も同じ扱い
        if let Some((i, (sx, sy), corner)) = self.image_at(x, y) {
            self.commit();
            self.checkpoint();
            self.img_sel = Some(i);
            self.img_drag = Some((i, (x, y), (sx, sy), corner));
            self.status = if corner {
                ui::t!("右下を引いて大きさを変えます(比は保ちます)").into()
            } else {
                ui::t!("画像を選びました(ドラッグで移動 / 右下で大きさ / Del で削除)").into()
            };
            return;
        }
        self.img_sel = None;
        if self.read_image_at(x, y) {
            // 読み込んだ画像は原文持ち越しが正 — 動かせないと正直に言う
            self.status = ui::t!(
                "読み込んだ画像は動かせません(保存で元の姿を守るため。挿し直せばこのアプリの画像になります)"
            )
            .into();
        }
        // フィルハンドル(選択枠の右下の小さな四角)。セルの選択より先に見る。
        // ダブルクリック = 隣の列の長さに合わせて下へ(本家と同じ)。
        // ドラッグ = 引いた所まで写す。発注者 2026-08-14「Excel では
        // そんな変な操作はしない」— 本家の手はこれ、が発端
        if self.tool.is_none() && self.fn_args.is_none() && self.ref_pick.is_none() {
            let (fa, fb) = self.fill_corner();
            if let Some((_, _, x1, y1)) = self.range_px(fa, fb) {
                if (x - x1).abs() <= 5.0 && (y - y1).abs() <= 5.0 {
                    self.commit();
                    if clicks >= 2 {
                        self.fill_handle_auto();
                    } else {
                        self.fill_drag = Some((fa, fb, fb, ctrl));
                        self.status =
                            ui::t!("下か右へ引いて写します(ダブルクリックで隣の列の長さまで)")
                                .into();
                    }
                    return;
                }
            }
        }
        // 見出しの境界の取っ手が最優先(セルの当たり判定より先に見る)。
        // **ダブルクリックの自動調整は撤去した**(2026-08-03 発注者報告)。
        // 押し直し・掴み直しは 400ms 以内なら click_count が 2,3,… と数えられる
        // (Wayland の仕様)ので、クリック数で分岐するとやり直しのドラッグを
        // 自動調整が横取りする — ドラッグは常にドラッグでなければならない
        let _ = clicks;
        if let Some((is_col, idx)) = self.size_grip_at(x, y) {
            self.commit();
            if std::env::var_os("JO_MOUSE_LOG").is_some() {
                eprintln!("grip: col={is_col} idx={idx} x={x:.0} y={y:.0}");
            }
            self.size_drag = Some(SizeDrag {
                col: is_col,
                idx,
                grab: if is_col { x } else { y },
                base: if is_col { self.col_px(idx) } else { self.row_px(idx) },
                moved: false,
            });
            return;
        }
        // 見出しの左上の角 = **全部のセルを選ぶ**(Excel の作法。
        // 発注者 2026-08-14)。行の見出しでも列の見出しでもない唯一の升
        if x < HEAD_W && y < ROW_H && self.show_headers {
            if !self.commit() {
                return;
            }
            // **表の端まで**(列・行の見出しと同じ決め — 発注者 2026-08-14)。
            // Ctrl+A は「使われている範囲」で別の道具。角は列と行の交わりなので、
            // 列ぜんぶ × 行ぜんぶ = 表ぜんぶが筋
            self.anchor = Some(Pos::new(LAST_ROW, LAST_COL));
            self.cursor = Pos::new(0, 0);
            self.sync_input();
            self.status = ui::t!("表ぜんぶを選択しました").into();
            return;
        }
        // 見出しのクリック = 列・行の選択(Excel の作法)。撫でれば複数列・行
        if y < ROW_H && x >= HEAD_W {
            if let Some(c) = self.col_at(x) {
                if !self.commit() {
                    return;
                }
                if shift {
                    // いまの選択の起点の列から伸ばす
                    let a = self.anchor.map(|p| p.col).unwrap_or(self.cursor.col);
                    self.select_cols(a, c);
                } else {
                    self.select_cols(c, c);
                    self.head_drag = Some((true, c));
                }
            }
            return;
        }
        if x < HEAD_W && y >= ROW_H {
            if let Some(r) = self.row_at(y) {
                if !self.commit() {
                    return;
                }
                if shift {
                    let a = self.anchor.map(|p| p.row).unwrap_or(self.cursor.row);
                    self.select_rows(a, r);
                } else {
                    self.select_rows(r, r);
                    self.head_drag = Some((false, r));
                }
            }
            return;
        }
        // 左上の角 = 使われている範囲の全選択(Ctrl+A と同じ)
        if x < HEAD_W && y < ROW_H {
            if !self.commit() {
                return;
            }
            let (rows, cols) = self.sheet().extent();
            if rows > 0 {
                self.anchor = Some(Pos::new(0, 0));
                self.cursor = Pos::new(rows - 1, cols.saturating_sub(1));
                self.sync_input();
                self.status = ui::tf!("A1:{} を選択しました", self.cursor.a1()).into();
            }
            return;
        }
        let Some(p) = self.cell_at(x, y) else { return };
        // 結合の中はどこを押しても左上(Excel と同じ)。呑まれた見えない
        // セルにカーソルが立つと、そこへ書けてしまう — 帳票の事故
        let p = self.merge_of(p).map(|(a, _)| a).unwrap_or(p);
        // 関数の引数の画面が開いている間は、セルのクリックで
        // **いまの欄に参照が入る**。そのままドラッグすると範囲(A1:C9)になる
        if self.fn_args.is_some() {
            let a1 = p.a1();
            if let Some(a) = &mut self.fn_args {
                if a.eds.is_empty() {
                    return;
                }
                let i = a.focus.min(a.eds.len() - 1);
                a.eds[i] = Editor::new(&a1);
                a.eds[i].move_to(a1.len(), false);
                a.pick_from = Some(p);
            }
            self.fn_args_recalc();
            return;
        }
        // 式の直入力中は、セルのクリックで**参照がカーソルに入る**(Excel の
        // 作法)。入るのは参照を待つ場所(= ( , 演算子の直後)のときだけ —
        // それ以外の場所でのクリックは、従来どおり確定して移動
        if (self.editing() || self.edit_armed) && self.input.text().starts_with('=') {
            let t = self.input.text().to_string();
            let cur = self.input.cursor().min(t.len());
            let prev = t[..cur].trim_end().chars().last();
            if matches!(
                prev,
                Some('=' | '(' | ',' | '+' | '-' | '*' | '/' | ':' | '^' | '&' | '<' | '>' | '%')
            ) {
                let a1 = self.ref_disp(p);
                self.input.insert(&a1);
                let end = self.input.cursor();
                self.ref_pick = Some((p, end - a1.len()..end));
                return;
            }
        }
        // Ctrl+クリックはリンクを開く(基幹網の外は既定のブラウザに任せる)
        if ctrl && !shift {
            if let Some(url) = self.sheet().links.get(&p).cloned() {
                if let Some(loc) = url.strip_prefix('#') {
                    // 帳面の中の場所(#Sheet2!B5 / #B5 / #A1:C9)へ跳ぶ
                    let (name, refs) = match loc.split_once('!') {
                        Some((n, r)) => (Some(n.trim_matches('\'')), r),
                        None => (None, loc),
                    };
                    if let Some(n) = name {
                        match self.book.sheets.iter().position(|s| s.name == n) {
                            Some(i) => self.active = i,
                            None => {
                                self.status = ui::tf!("シート「{}」が見つかりません", n).into();
                                return;
                            }
                        }
                    }
                    let mut it = refs.split(':');
                    let a = it.next().and_then(Pos::parse);
                    let b = it.next().and_then(Pos::parse);
                    if let Some(a) = a {
                        self.anchor = b.map(|_| a);
                        self.cursor = b.unwrap_or(a);
                        self.sync_input();
                        self.status = ui::tf!("リンク先 {} へ移動しました", loc).into();
                    } else {
                        self.status = ui::tf!("リンク先({})が場所として読めません", loc).into();
                    }
                    return;
                }
                self.status = match ui::open_outside(&url) {
                    ui::Opened::Yes => ui::tf!("開きます: {}", url).into(),
                    ui::Opened::JustNow => {
                        ui::t!("さっき開きました(窓が出るまで少し待ってください)").into()
                    }
                    ui::Opened::Failed => {
                        ui::tf!("開けません(xdg-open がありません): {}", url).into()
                    }
                };
                return;
            }
        }
        if !self.commit() {
            // 入力規則で戻された。移動すると打った文字が黙って消えるので留まる
            return;
        }
        // 刷毛(書式のコピー)を持っていたら、押した先に塗って手放す
        if let Some(f) = self.brush.take() {
            self.checkpoint();
            let (a, b) = if shift && self.anchor.is_some() {
                self.sel_rect()
            } else {
                (p, p)
            };
            for r in a.row..=b.row {
                for cch in a.col..=b.col {
                    let q = Pos::new(r, cch);
                    let mut cell = self.sheet().get(q).cloned().unwrap_or_default();
                    cell.fmt = f.clone();
                    self.sheet_mut().set(q, cell);
                }
            }
            self.dirty = true;
            self.cursor = p;
            self.sync_input();
            self.status = ui::tf!("{} に書式を塗りました(Ctrl+Z で戻せます)", p.a1()).into();
            return;
        }
        if shift {
            // いまのセルから伸ばす
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
            self.drag = Some(p);
        }
        self.cursor = p;
        self.sync_input();
        // ダブルクリックはその場で編集(次の打鍵が追記になる — Excel の作法)
        if clicks >= 2 {
            self.edit_armed = true;
            self.input.move_to(self.input.text().len(), false);
            self.status = ui::t!("編集: そのまま打つと続きに入ります(Esc で取消)").into();
        }
    }

    /// 押したまま動いた。通り過ぎたセルまで選択を広げる。
    pub(crate) fn mouse_drag_at(&mut self, x: f32, y: f32) {
        // 式の直入力のセル掴み: 入れた参照を「起点:いま」の範囲に置き換える
        if let Some((from, range)) = self.ref_pick.clone() {
            let Some(p) = self.cell_at(x, y) else { return };
            let (ra, rb) = (from.row.min(p.row), from.row.max(p.row));
            let (ca, cb) = (from.col.min(p.col), from.col.max(p.col));
            let text = if from == p {
                self.ref_disp(p)
            } else {
                format!(
                    "{}:{}",
                    self.ref_disp(Pos::new(ra, ca)),
                    self.ref_disp(Pos::new(rb, cb))
                )
            };
            let mut t = self.input.text().to_string();
            if range.end <= t.len() {
                t.replace_range(range.clone(), &text);
                self.input = Editor::new(&t);
                self.input.move_to(range.start + text.len(), false);
                self.ref_pick = Some((from, range.start..range.start + text.len()));
            }
            return;
        }
        // 関数の引数のセル掴み: なぞった範囲「起点:いま」を欄に入れる
        if self.fn_args.as_ref().is_some_and(|a| a.pick_from.is_some()) {
            let Some(p) = self.cell_at(x, y) else { return };
            if let Some(a) = &mut self.fn_args {
                let Some(from) = a.pick_from else { return };
                let i = a.focus.min(a.eds.len().saturating_sub(1));
                let (ra, rb) = (from.row.min(p.row), from.row.max(p.row));
                let (ca, cb) = (from.col.min(p.col), from.col.max(p.col));
                let text = if from == p {
                    p.a1()
                } else {
                    format!("{}:{}", Pos::new(ra, ca).a1(), Pos::new(rb, cb).a1())
                };
                a.eds[i] = Editor::new(&text);
                a.eds[i].move_to(text.len(), false);
            }
            self.fn_args_recalc();
            return;
        }
        if self.tool == Some(2) {
            // 消しゴムはなぞっている間ずっと効く
            if let Some(i) = self.ink_at(x, y) {
                self.checkpoint();
                self.sheet_mut().shapes_new.remove(i);
                self.dirty = true;
            }
            return;
        }
        if let Some(pts) = &mut self.ink_cur {
            // 近すぎる点は捨てる(点の数を抑える)
            let far = pts
                .last()
                .map(|(lx, ly)| (x - lx).abs() + (y - ly).abs() > 2.0)
                .unwrap_or(true);
            if far {
                pts.push((x, y));
            }
            return;
        }
        if let Some((is_col, start)) = self.head_drag {
            // 見出しから始めた選択は、どこを通っても列・行の選択のまま
            if is_col {
                if let Some(c) = self.col_at(x) {
                    if self.cursor.col != c {
                        self.select_cols(start, c);
                    }
                }
            } else if let Some(r) = self.row_at(y) {
                if self.cursor.row != r {
                    self.select_rows(start, r);
                }
            }
            return;
        }
        let Some(start) = self.drag else { return };
        let Some(p) = self.cell_at(x, y) else { return };
        if self.cursor == p {
            return;
        }
        self.cursor = p;
        self.anchor = if p == start { None } else { Some(start) };
        if self.anchor.is_some() {
            let (a, b) = self.sel_rect();
            self.status = format!("{}:{}", a.a1(), b.a1()).into();
        }
        self.sync_input();
    }

    /// 離した。ドラッグ選択はここで確定する。
    /// フィルハンドルのドラッグ中。下か右の**どちらか**に伸ばす(本家と同じ)
    pub(crate) fn fill_drag_at(&mut self, x: f32, y: f32) {
        let Some((a, b, _, ctrl)) = self.fill_drag else { return };
        let Some(p) = self.cell_at(x, y) else { return };
        let to = if p.row > b.row {
            Pos::new(p.row, b.col)
        } else if p.col > b.col {
            Pos::new(b.row, p.col)
        } else {
            b
        };
        self.fill_drag = Some((a, b, to, ctrl));
    }

    pub(crate) fn mouse_up(&mut self) {
        // フィルハンドルを離した = 写す(伸ばしていなければ何もしない)
        if let Some((a, b, to, ctrl)) = self.fill_drag.take() {
            if to != b {
                self.fill_handle_apply(a, b, to, ctrl);
            }
            return;
        }
        // 関数の引数・式の直入力のセル掴みは、離した所で終わり
        if let Some(a) = &mut self.fn_args {
            a.pick_from = None;
        }
        self.ref_pick = None;
        if let Some(pts) = self.ink_cur.take() {
            self.finish_ink(pts);
            return;
        }
        if std::env::var_os("JO_MOUSE_LOG").is_some() {
            eprintln!(
                "up size_drag={} moved={:?}",
                self.size_drag.is_some(),
                self.size_drag.as_ref().map(|d| d.moved)
            );
        }
        if self.size_drag.take().is_some() {
            // 幅・高さの確定。status は size_drag_at が出している
            return;
        }
        if self.head_drag.take().is_some() {
            return; // 列・行の選択の確定。status は select_* が出している
        }
        if self.pt_drag.take().is_some() {
            return; // 点の移動の確定。status はつまんだ時に出している
        }
        if self.shape_rot.take().is_some() {
            return; // 回転の確定。status はドラッグ中に出している
        }
        if let Some((_, _, _, moved)) = self.shape_drag.take() {
            // 動かしていない(選んだだけ)なら、積んだ控えは戻す
            let _ = moved;
            return;
        }
        if self.img_drag.take().is_some() {
            return; // 画像の移動・大きさの確定。status はドラッグ中に出している
        }
        if self.slicer_drag.take().is_some() {
            return; // スライサーの板の移動の確定
        }
        if self.drag.take().is_some() && self.anchor.is_some() {
            let (a, b) = self.sel_rect();
            self.status = format!("{}:{}", a.a1(), b.a1()).into();
        }
    }

    /// 右クリック。選択の中ならその選択への操作、外ならそのセルへ移ってから
    /// メニューを出す(Excel の作法)。
    pub(crate) fn right_click_at(&mut self, x: f32, y: f32) {
        self.menu_shape = false;
        // 浮いている図形の上 = 図形の専用メニュー(本家の作法)。
        // 図形はセルの上に描かれているので、セルより先に見る
        if let Some((i, _, _)) = self.shape_at(x, y) {
            self.commit();
            // Ctrl+クリックで束ねた選択の中なら保つ(整列へ続く)。外なら選び直す
            if self.shape_sel != Some(i) && !self.shape_multi.contains(&i) {
                self.shape_multi.clear();
                self.shape_sel = Some(i);
            }
            self.menu_at = Some((x, y));
            self.menu_sub = None;
            self.menu_head = None;
            self.menu_shape = true;
            return;
        }
        // 見出しの右クリック = その列・行を選んでからメニュー(Excel の作法)。
        // 既に選択の中なら選び直さない(複数列への操作を保つ)
        if y < ROW_H && x >= HEAD_W {
            if let Some(c) = self.col_at(x) {
                let (a, b) = self.sel_rect();
                if !(self.anchor.is_some() && (a.col..=b.col).contains(&c)) {
                    if !self.commit() {
                        return;
                    }
                    self.select_cols(c, c);
                }
                self.menu_at = Some((x, y));
                self.menu_sub = None;
                self.menu_head = Some(true);
            }
            return;
        }
        if x < HEAD_W && y >= ROW_H {
            if let Some(r) = self.row_at(y) {
                let (a, b) = self.sel_rect();
                if !(self.anchor.is_some() && (a.row..=b.row).contains(&r)) {
                    if !self.commit() {
                        return;
                    }
                    self.select_rows(r, r);
                }
                self.menu_at = Some((x, y));
                self.menu_sub = None;
                self.menu_head = Some(false);
            }
            return;
        }
        if let Some(p) = self.cell_at(x, y) {
            let (a, b) = self.sel_rect();
            let inside = self.anchor.is_some()
                && (a.row..=b.row).contains(&p.row)
                && (a.col..=b.col).contains(&p.col);
            if !inside && p != self.cursor {
                if !self.commit() {
                    // 入力規則で戻された。移動せずメニューも出さない
                    return;
                }
                self.anchor = None;
                self.cursor = p;
                self.sync_input();
            }
        }
        self.menu_at = Some((x, y));
        self.menu_head = None;
        self.menu_sub = None;
    }

    /// 範囲の見えている部分の px 矩形 (x0, y0, x1, y1)。全部画面の外なら None。
    /// フィルハンドルを描く(押す)ときの選択の角。角のセルが結合に
    /// 呑まれていたら**結合の外周**まで広げる — 親セル1個分の角に描くと、
    /// 結合の真ん中に緑の四角が浮く(発注者 2026-08-14「中央にゴミ」)
    pub(crate) fn fill_corner(&self) -> (Pos, Pos) {
        let (a, b) = self.sel_rect();
        let a = self.merge_of(a).map(|(ma, _)| ma).unwrap_or(a);
        let b = self.merge_of(b).map(|(_, mb)| mb).unwrap_or(b);
        (a, b)
    }

    pub(crate) fn range_px(&self, a: Pos, b: Pos) -> Option<(f32, f32, f32, f32)> {
        let (mut x0, mut x1) = (None, None);
        let mut x = HEAD_W;
        for c in self.visible_cols() {
            let w = self.col_px(c);
            if c >= a.col && c <= b.col {
                if x0.is_none() {
                    x0 = Some(x);
                }
                x1 = Some(x + w);
            }
            x += w;
        }
        let (mut y0, mut y1) = (None, None);
        let mut y = ROW_H;
        for r in self.visible_rows() {
            let h = self.row_px(r);
            if r >= a.row && r <= b.row {
                if y0.is_none() {
                    y0 = Some(y);
                }
                y1 = Some(y + h);
            }
            y += h;
        }
        Some((x0?, y0?, x1?, y1?))
    }

    /// **一覧やパレットを出す場所(格子の面の px)。**
    ///
    /// リボンのボタンから開いたときは押したボタンの真下、キー操作や格子の
    /// 上からならいまのセルの下。以前はどこから開いても必ずセルの下に出て
    /// いて、リボンで書体を選ぼうとすると一覧が画面の下の方に飛んでいた
    /// (発注者報告 2026-08-08)。**一覧は押した場所の近くに出す。**
    ///
    pub(crate) fn pop_anchor(&self) -> (f32, f32) {
        // 開くたびに取り直す。リボンから来ていなければ 0(セルに合わせる)
        if self.pop_at.is_none() {
            self.pop_btn_w.set(0.0);
        }
        if let Some(at) = self.pop_at {
            // 上へ開くときの基準(ボタンの上辺)は run_from_ribbon が入れてある
            return at;
        }
        // セルから開くときは**そのセルの上辺**が上へ開くときの基準になる。
        // 下に入らなければセルの上に出す(いちばん下の行で効く)
        let (x, y) = self
            .cell_origin_px(self.cursor)
            .unwrap_or((self.head_w() + 16.0, self.head_h() + 16.0));
        self.pop_top.set(y);
        (x, y + self.row_px(self.cursor.row))
    }

    /// いまのセルの書体名(指定が無ければ既定)。コンボを開くとき今の位置へ送るのに使う
    pub(crate) fn cur_font_name(&self) -> String {
        self.sheet()
            .get(self.cursor)
            .and_then(|c| c.fmt.font.clone())
            .unwrap_or_else(|| "Noto Sans JP".to_string())
    }

    /// いまのセルの文字の大きさ(pt)。指定が無ければ既定 11pt
    pub(crate) fn cur_size_pt(&self) -> f32 {
        self.sheet()
            .get(self.cursor)
            .and_then(|c| c.fmt.size_c)
            .map(|c| c as f32 / 100.0)
            .unwrap_or(11.0)
    }

    /// **小窓(… の側)が開いているか。** [`Calc::DIALOG_IDS`] の腕が立てる
    /// 旗の総和 — 印(…)と1対1で揃える(ずれると印が嘘になる)。
    /// `fn_args` は「関数を挿入」の第2段(引数の画面)なので同じ小窓の続き。
    /// 真の間はリボン全体(タブの切替も)を無効にする — 小窓を出したまま
    /// 他の操作が走って状態が二重になるのを防ぐ。閉じる道は今のまま
    /// (Esc・小窓の中のボタン)。
    pub(crate) fn dialog_open(&self) -> bool {
        self.prompt.is_some()
            || self.fn_dlg.is_some()
            || self.fn_args.is_some()
            || self.fmt_panel.is_some()
            || self.dv_dlg.is_some()
            || self.solver.is_some()
    }

    /// 開いている一覧(▾ の側)を畳む。**他のボタンやタブを押したら閉じ、
    /// 押した操作はそのまま効く**約束(発注者 2026-08-14)。セルの押下は
    /// mouse_down_at が既に畳んでいる — 穴だったリボンの側をこれで塞ぐ。
    pub(crate) fn close_menus(&mut self) {
        self.close_pick();
        self.menu_at = None;
        self.menu_direct = false;
        self.border_pal = None;
    }

    /// リボンのボタンから命令を出す。**押したボタンの場所を控えてから**
    /// run_cmd に渡すので、開いた一覧はそのボタンの真下に出る
    /// ([`Self::pop_anchor`] / [`pop_under`])。
    pub(crate) fn run_from_ribbon(&mut self, id: &'static str, at_x: f32, cx: &mut Context<Self>) {
        // 小窓中はリボンから何も通さない(描画の縛りと二重 — 鍵盤の
        // Alt ヒント経由など、ボタンの絵を通らない道からも入るため)
        if self.dialog_open() {
            return;
        }
        // 開いている一覧は畳んでから走らせる(一覧を開くボタンなら
        // run_cmd が開き直すので、置き換えの動きも自然に出る)
        self.close_menus();
        let pane = self.pane_box.get();
        let btn = self.btn_box.borrow().get(id).copied();
        // 描く前に鍵から呼ばれた等でボタンの場所が無ければ押した点を使う
        self.pop_btn_w.set(btn.map(|b| b.2).unwrap_or(0.0));
        // 上へ開くときの基準はボタンの上辺(面を基準にした y)。
        // 場所が分からない逃げ道では、押した点をボタンの上辺と見なす
        self.pop_top.set(match btn {
            Some(b) => b.1 - pane.1,
            None => -2.0,
        });
        self.pop_at = Some(match btn {
            Some(b) => pop_under(b, pane),
            None => pop_at_click(at_x, pane),
        });
        self.run_cmd(id, cx);
        self.pop_at = None;
    }

    /// **このセルは保護で堰き止められるか。** 保護していないなら誰でも書ける。
    /// 保護中は、`unlocked` を立てたセル(=書式で「ロックを外した」セル)
    /// だけが書ける — 帳票の「記入欄だけ開ける」作法(Excel と同じ)。
    pub(crate) fn cell_locked(&self, p: Pos) -> bool {
        self.sheet().protected
            && !self.sheet().get(p).map(|c| c.fmt.unlocked).unwrap_or(false)
    }

    /// 選んでいる範囲に、保護で書けないセルが1つでもあるか
    pub(crate) fn sel_locked(&self) -> bool {
        if !self.sheet().protected {
            return false;
        }
        let (a, b) = self.sel_rect();
        (a.row..=b.row).any(|r| (a.col..=b.col).any(|c| self.cell_locked(Pos::new(r, c))))
    }

    /// 保護中に断ったときの言い分。**何をすれば通るかまで言う**
    pub(crate) fn protected_msg() -> String {
        ui::t!("シートが保護されています(このセルのロックを外すか、保護タブで解除)").into()
    }

    /// いま表示されているセルの左上(格子領域の px)。画面の外なら None。
    pub(crate) fn cell_origin_px(&self, p: Pos) -> Option<(f32, f32)> {
        let mut x = self.head_w();
        let mut cfound = false;
        for c in self.visible_cols() {
            if c == p.col {
                cfound = true;
                break;
            }
            x += self.col_px(c);
        }
        let mut y = self.head_h();
        let mut rfound = false;
        for r in self.visible_rows() {
            if r == p.row {
                rfound = true;
                break;
            }
            y += self.row_px(r);
        }
        (cfound && rfound).then_some((x, y))
    }

    /// 形式を選択して貼り付け。mode: values / formulas / formats / transpose
    pub(crate) fn paste_special(&mut self, mode: &str, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) else {
            self.status = ui::t!("貼り付けるものがありません").into();
            return;
        };
        if text.is_empty() {
            return;
        }
        // アプリ内のコピーか(系のクリップボードと控えの突き合わせ)
        let internal = matches!(&self.clip, Some((_, t)) if *t == text);
        let at = self.cursor;
        let n = match mode {
            "values" => {
                self.commit();
                self.checkpoint();
                if internal {
                    let cells = self.clip_cells.clone().unwrap_or_default();
                    paste_values_cells(&mut self.book.sheets[self.active], at, &cells)
                } else {
                    let grid = tsv_grid(&text);
                    paste_values_text(&mut self.book.sheets[self.active], at, &grid)
                }
            }
            "formulas" => {
                // 式を**ずらさずそのまま**貼る(普通の貼り付けはずらす方)
                self.commit();
                self.checkpoint();
                let grid = tsv_grid(&text);
                paste_grid(&mut self.book.sheets[self.active], at, &grid, None)
            }
            "formats" => {
                if !internal {
                    self.status =
                        ui::t!("書式は他のアプリからは持って来られません(このアプリでコピーした範囲だけ)").into();
                    return;
                }
                self.commit();
                self.checkpoint();
                let cells = self.clip_cells.clone().unwrap_or_default();
                paste_formats(&mut self.book.sheets[self.active], at, &cells)
            }
            "transpose" => {
                // 行と列を入れ替えて、値を貼る(式は計算結果の値になる —
                // 転置で参照を正しく回すのは別の話なので、黙って混ぜない)
                self.commit();
                self.checkpoint();
                if internal {
                    let cells = transpose(&self.clip_cells.clone().unwrap_or_default());
                    paste_values_cells(&mut self.book.sheets[self.active], at, &cells)
                } else {
                    let grid = transpose(&tsv_grid(&text));
                    paste_values_text(&mut self.book.sheets[self.active], at, &grid)
                }
            }
            _ => return,
        };
        recalc_book(&mut self.book, self.active);
        self.dirty = true;
        self.sync_input();
        self.status = match mode {
            "values" => ui::tf!("{} セルに値だけを貼りました(書式は据え置き)", n),
            "formulas" => ui::tf!("{} セルに式をそのまま貼りました(参照はずらしていません)", n),
            "formats" => ui::tf!("{} セルに書式だけを写しました(中身は残っています)", n),
            _ => ui::tf!("{} セルを転置して貼りました(式は値になっています)", n),
        }
        .into();
    }

    pub(crate) fn a_paste_values(&mut self, _: &ui::PasteValues, _: &mut Window, cx: &mut Context<Self>) {
        self.paste_special("values", cx);
        cx.notify();
    }

    /// メニューの項目を実行する。
    /// いまの列で並べ替え(右クリックとリボンの昇順/降順が同じ道)
    pub(crate) fn sort_active(&mut self, asc: bool) {
        // 範囲を選んでいなければ従来どおり: カーソル列で表全体
        if self.anchor.is_none() {
            self.sort_col(self.cursor.col, asc);
            return;
        }
        let (a, b) = self.sel_rect();
        if a == b {
            self.sort_col(self.cursor.col, asc);
            return;
        }
        // 選択の左右(同じ行)に続きのデータがあるか。あるなら本家と同じく
        // 「拡張して並べ替え/選択だけ」を聞く — 黙って行をずらさない
        let filled = |p: Pos| {
            self.sheet().get(p).map(|c| !c.editable().trim().is_empty()).unwrap_or(false)
        };
        let neighbor = (a.row..=b.row).any(|r| {
            let left = a.col > 0 && filled(Pos::new(r, a.col - 1));
            left || filled(Pos::new(r, b.col + 1))
        });
        if neighbor {
            let at = self.pop_anchor();
            self.sort_pend = Some(asc);
            self.pick_kind = "sort-expand";
            self.pick = Some((
                menu(&[
                    ui::item!("拡張して並べ替え(続きの列も一緒に動く)"),
                    ui::item!("選択した範囲だけ並べ替え(横の列とはずれます)"),
                    ui::item!("やめる"),
                ]),
                at,
            ));
            self.status =
                ui::t!("選択の横にデータが続いています。どう並べ替えますか?").into();
            return;
        }
        self.sort_range_now(a, b, asc);
    }

    /// 選んだ範囲だけを並べ替える(確認の後もここに来る)
    pub(crate) fn sort_range_now(&mut self, a: Pos, b: Pos, asc: bool) {
        self.commit();
        self.checkpoint();
        self.book.sheets[self.active].sort_range(a, b, self.cursor.col, asc);
        self.dirty = true;
        recalc_book(&mut self.book, self.active);
        self.sync_input(); // 古い控えの書き戻しを防ぐ(sort_col と同じ)
        self.status = ui::tf!(
            "{}:{} を{}に並べ替えました(範囲の中だけ。Ctrl+Z で1手)",
            a.a1(), b.a1(),
            if asc { ui::t!("昇順") } else { ui::t!("降順") }
        )
        .into();
    }

    /// カーソルのセルの色(塗り/文字色)を上に集める並べ替え
    pub(crate) fn sort_color_top(&mut self, use_fill: bool) {
        let fmt = self.sheet().get(self.cursor).map(|c| c.fmt.clone()).unwrap_or_default();
        let Some(target) = (if use_fill { fmt.fill } else { fmt.color }) else {
            self.status = if use_fill {
                ui::t!("このセルに塗りつぶしの色がありません").into()
            } else {
                ui::t!("このセルの文字に色が付いていません").into()
            };
            return;
        };
        self.commit();
        self.checkpoint();
        let col = self.cursor.col;
        self.book.sheets[self.active].sort_color_top(col, use_fill, &target, true);
        self.dirty = true;
        recalc_book(&mut self.book, self.active);
        self.sync_input(); // 古い控えの書き戻しを防ぐ(sort_col と同じ)
        self.status = if use_fill {
            ui::t!("セルの色が同じ行を上に集めました").into()
        } else {
            ui::t!("フォントの色が同じ行を上に集めました").into()
        };
    }

    /// 指定の列で並べ替え(▼のパネルの昇順/降順もここに来る)
    pub(crate) fn sort_col(&mut self, c: u32, asc: bool) {
        self.commit();
        self.checkpoint();
        self.book.sheets[self.active].sort_by_column(c, asc, true);
        self.dirty = true;
        recalc_book(&mut self.book, self.active);
        // 数式バーの控えを並べ替え後のセルに合わせる — 同期を怠ると、
        // 次の commit で並べ替え前の古い値が書き戻される
        self.sync_input();
        self.status = ui::tf!("{} 列で{}に並べ替えました", Pos::new(0, c).a1().trim_end_matches('1'), if asc { ui::t!("昇順") } else { ui::t!("降順") })
            .into();
    }

    /// 数式バーの内容をセルへ。**入力規則(list)に合わない値は入れない**
    /// (Excel と同じ)。false を返したら呼び側は移動しないこと —
    /// 打った文字が黙って消える。Esc でセルの保存内容に戻せる。
    /// 描いた1筆(格子の px の列)を図形(折れ線)にして置く。
    /// **既にある図形の仕組みに乗せる** — xlsx へは custGeom で入り、
    /// Excel でも線に見え、消しゴムも移動も Ctrl+Z も全部そのまま効く
    pub(crate) fn finish_ink(&mut self, pts: Vec<(f32, f32)>) {
        if pts.len() < 2 {
            return; // 点を打っただけ(線にならない)
        }
        let (mut x0, mut y0) = (f32::MAX, f32::MAX);
        let (mut x1, mut y1) = (f32::MIN, f32::MIN);
        for (x, y) in &pts {
            x0 = x0.min(*x);
            y0 = y0.min(*y);
            x1 = x1.max(*x);
            y1 = y1.max(*y);
        }
        let (w, h) = ((x1 - x0).max(4.0), (y1 - y0).max(4.0));
        // アンカーは左上の点があるセル。そこからのずらしで位置を覚える
        let at = self.cell_at(x0, y0).unwrap_or(self.view);
        let (ox, oy) = self.cell_origin_px(at).unwrap_or((self.head_w(), self.head_h()));
        let marker = self.tool == Some(1);
        self.checkpoint();
        self.sheet_mut().shapes_new.push(sheet::model::SheetShape {
            at,
            dx_px: x0 - ox,
            dy_px: y0 - oy,
            width_px: w,
            height_px: h,
            kind: if marker { "marker".into() } else { "ink".into() },
            fill: None,
            line: Some(if marker { "FFD54A".into() } else { "1B1B1B".into() }),
            points: pts
                .iter()
                .map(|(x, y)| sheet::model::PathPoint::at((x - x0) / w, (y - y0) / h))
                .collect(),
            ..Default::default()
        });
        self.dirty = true;
        self.status = if marker {
            ui::t!("蛍光ペンで引きました(Ctrl+Z で戻せます)").into()
        } else {
            ui::t!("ペンで描きました(Ctrl+Z で戻せます)").into()
        };
    }

    /// この位置にある手描きの線(いちばん上のもの)。消しゴムが使う
    pub(crate) fn ink_at(&self, x: f32, y: f32) -> Option<usize> {
        let sh = self.sheet();
        for (i, sp) in sh.shapes_new.iter().enumerate().rev() {
            if !matches!(sp.kind.as_str(), "ink" | "marker" | "spark") {
                continue;
            }
            let Some((ox, oy)) = self.cell_origin_px(sp.at) else { continue };
            let (x0, y0) = (ox + sp.dx_px, oy + sp.dy_px);
            let near = if sp.kind == "marker" { 7.0 } else { 4.0 };
            let hit = sp.points.iter().any(|pp| {
                let (px_, py_) = (&pp.at.0, &pp.at.1);
                let (cx, cy) = (x0 + px_ * sp.width_px, y0 + py_ * sp.height_px);
                (cx - x).abs() <= near && (cy - y).abs() <= near
            });
            if hit {
                return Some(i);
            }
        }
        None
    }

    /// 選択範囲(見た目の値)の TSV。AI に渡す形
    pub(crate) fn tsv_display(&self, a: Pos, b: Pos) -> String {
        let sh = self.sheet();
        (a.row..=b.row)
            .map(|r| {
                (a.col..=b.col)
                    .map(|c| sh.get(Pos::new(r, c)).map(|x| x.value.display()).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// AI に頼んで、返事を表に反映する。**別のスレッドで待つ**(画面は止めない)。
    /// 反映は必ず checkpoint してから = **Ctrl+Z の1手で戻る**。
    /// 宛先が使えなければ理由を言う(黙って空にしない)
    pub(crate) fn ai_go(&mut self, job: CalcAi, cx: &mut Context<Self>) {
        if self.sheet().protected {
            self.status =
                ui::t!("シートが保護されています(保護タブの「シートを保護する」で解除)").into();
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
        self.commit();
        // 渡す範囲: 選んでいればそこ。**選んでいなくても通す** —
        // 「この式の意味は」のように範囲の要らない用件がある
        // (2026-08-15 実機で門前払いに気づいた)
        let (a, b) = self.anchor.map(|_| self.sel_rect()).unwrap_or((self.cursor, self.cursor));
        let body = if self.anchor.is_none() {
            String::new()
        } else {
            self.tsv_display(a, b)
        };
        let (sys, _ask) = job.prompt();
        let CalcAi::Chat(q) = &job;
        // 会話は**用件そのもの**が本体。表は付け合わせで、選んでいる
        // 場所の番地も渡す(台本が s["A1:C9"] と書けるように)
        let user = if body.trim().is_empty() {
            q.clone()
        } else {
            format!(
                "{q}\n\n---\nいま選んでいるのはシート「{}」の {} です。\n{body}",
                self.sheet().name,
                self.sel_label(),
            )
        };
        let sys = sys.to_string();
        let job2 = job.clone();
        self.ai_busy = true;
        self.status =
            ui::tf!("AI({})に{}を頼んでいます…", back.label(), job.label()).into();
        let task = cx
            .background_executor()
            .spawn(async move { ui::ai::ask(back, &sys, &user) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                this.ai_busy = false;
                match r {
                    Ok(out) => this.ai_apply(job2, out),
                    Err(e) => this.status = format!("AI: {e}").into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 返事を受け取る。**書類には入れない** — 左パネルへ返すだけ
    /// (2026-08-15、AI タブを廃してからは会話しか通らない)
    pub(crate) fn ai_apply(&mut self, job: CalcAi, out: String) {
        let out = out.trim().to_string();
        if out.is_empty() {
            self.status = ui::t!("AI: 答えが空でした(何もしていません)").into();
            return;
        }
        match job {
            // **会話は書類に入れない。** 左パネルに返し、変更案(Python)は
            // 人が「入れる」を押すまで走らせない — **押したのは人**が残る形
            CalcAi::Chat(_) => {
                let 案 = 取り出す囲み(&out);
                let 見せる = if let Some(code) = &案 {
                    // 囲みの外の説明だけを会話に出す(台本は下の欄に置く)
                    let 説明 = out.replace(&format!("```python\n{code}\n```"), "")
                        .replace(&format!("```\n{code}\n```"), "");
                    let t = 説明.trim().to_string();
                    if t.is_empty() { ui::t!("こう直します。").to_string() } else { t }
                } else {
                    out.clone()
                };
                self.chat_log.push((false, 見せる));
                self.chat_plan = 案;
                self.status = if self.chat_plan.is_some() {
                    ui::t!("変更案ができました(左パネルで中身を見てから「入れる」)").into()
                } else {
                    ui::t!("答えました(左パネル)").into()
                };
            }
        }
    }

    /// いまの計算方法で再計算する(手動なら何もしない — 「計算」で回す)
    pub(crate) fn recalc_if_auto(&mut self) {
        if self.auto_calc {
            recalc_book(&mut self.book, self.active);
        }
    }

    pub(crate) fn commit(&mut self) -> bool {
        // **仕舞うときにも数学オートコレクトを掛ける。** Enter は区切りの
        // 打鍵ではないので、これが無いと「\alpha と打って Enter」— いちばん
        // 普通の終わり方 — だけ替わらない。区切りは足さない(空)
        // 見るのは**セルの打ちかけ**(`self.input`)そのもの — 小窓が開いて
        // いると `editor_ref()` はそちらを指すので、ここでは使わない
        if self.autocorrect && !self.input.text().starts_with('=') {
            self.input.autocorrect_math("");
        }
        let (cur, mut text) = (self.cursor, self.input.text().to_string());
        // R1C1 で打った式は A1 に戻して仕舞う(中身はいつも A1)
        if self.book.r1c1 {
            if let Some(body) = text.strip_prefix('=') {
                text = format!("={}", sheet::model::formula_from_r1c1(body, cur));
            }
        }
        // { } は見せるための飾り(配列数式の印)。中身は = から始まる式
        if text.starts_with("{=") && text.ends_with('}') {
            text = text[1..text.len() - 1].to_string();
        }
        // 変わっていなければ何もしない(移動のたびに履歴が積まれるのを防ぐ)
        let now = self.sheet().get(cur).map(|c| c.editable()).unwrap_or_default();
        if now == text {
            return true;
        }
        // **配列数式の一部は書き換えさせない**(Excel と同じ)。
        // 黙って普通の式に落とすと、範囲の残りが古い値のまま取り残される
        if let Some(o) = self.sheet().cse_anchor(cur) {
            self.sync_input();
            self.status = ui::tf!(
                "{} からの配列数式の一部です。変えるには範囲を選び直して Ctrl+Shift+Enter(消すなら範囲を選んで Delete)",
                o.a1()
            )
            .into();
            return false;
        }
        // シートの保護。打ちかけは捨てて元に戻す(黙って通さない)。
        // **セル単位のロックを見る** — ロックを外したセルは保護中でも書ける
        if self.cell_locked(self.cursor) {
            self.sync_input();
            self.status = Self::protected_msg().into();
            return false;
        }
        // 空白は「空白を無視」(allowBlank)が付いていれば許す(既定)。
        // 式は結果が変わり得るので通す
        if !text.starts_with('=') {
            // 判定は Validation::passes(判定できない規則は堰き止めない)。
            // 文言は規則に付いたエラーの文言が正、無ければ規則の言い直し
            let verdict = self.sheet().validation_at(cur).and_then(|v| {
                let ok = if text.trim().is_empty() {
                    v.allow_blank
                } else {
                    v.passes(self.sheet(), text.trim())
                };
                if ok {
                    None
                } else {
                    let fallback = if v.kind == "list" {
                        format!("候補: {}", v.options(self.sheet()).join(" / "))
                    } else {
                        v.describe()
                    };
                    Some((v.error_msg.clone(), fallback))
                }
            });
            if let Some((em, fallback)) = verdict {
                let stop = em.as_ref().map(|(s, _, _)| s == "stop").unwrap_or(true);
                let said = match &em {
                    Some((_, t, m)) if !t.is_empty() || !m.is_empty() => {
                        if t.is_empty() {
                            m.clone()
                        } else if m.is_empty() {
                            t.clone()
                        } else {
                            format!("{t}: {m}")
                        }
                    }
                    _ => fallback,
                };
                if stop {
                    self.status = ui::tf!(
                        "「{}」は入力規則に合いません({} / Esc で戻す)",
                        text.trim(), said
                    )
                    .into();
                    return false;
                }
                // 警告・情報は通すが言う(Excel の「警告」で続行した形)
                self.status = ui::tf!("入力規則に合いませんが、通しました({})", said).into();
            }
        }
        self.checkpoint();
        // **書式は据え置く。** 打ち直しただけで罫線や塗りが消えるのは帳票の事故
        let fmt = self.sheet().get(cur).map(|c| c.fmt.clone()).unwrap_or_default();
        let mut cell = Cell::input(&text);
        cell.fmt = fmt;
        // Alt+Enter の改行が入っていたら折り返しも立てる(Excel と同じ)
        if text.contains('\n') {
            cell.fmt.wrap = true;
        }
        // 記録(始めていれば)。**書けた物だけを残す** — 断られた入力や
        // 保護で戻された物は台本に入れない(走らせて同じにならない物を書かない)
        self.rec_set(cur, &text);
        self.sheet_mut().set(cur, cell);
        self.fit_row_to_cellmark(cur);
        // 計算方法が手動なら待たされない(F9 / Shift+F9 で手回し)。
        // 今までは常に再計算していて「手動」が効いていなかった
        self.recalc_if_auto();
        self.dirty = true;
        // 中身を変えたらコピーの破線は消す(Excel と同じ)
        self.clip_range = None;
        true
    }

    /// 見出し(`# `)を打ったセルの行を、その大きさに合うまで**広げる**。
    /// 大きさの表は `sheet::cellmark::HEADINGS` が正(画面の文字と同じ所を見る)。
    /// **狭めはしない** — 手で決めた行の高さを打ち直しで壊さないため
    /// (見出しを消したら、行の高さは手で戻す)。
    pub(crate) fn fit_row_to_cellmark(&mut self, at: Pos) {
        let Some(text) = self
            .sheet()
            .get(at)
            .and_then(|c| match &c.value {
                sheet::Value::Text(t) => Some(t.clone()),
                _ => None,
            })
        else {
            return;
        };
        let Some(md) = sheet::cellmark::parse(&text) else { return };
        if !md.iter().any(|l| matches!(l.block, sheet::cellmark::Block::Heading(_))) {
            return; // 見出しが無ければ高さは触らない
        }
        // 折り返しの無いセルは1行に畳んで描くので、要るのは一番高い行のぶんだけ
        let wrap = self.sheet().get(at).map(|c| c.fmt.wrap).unwrap_or(false);
        let base = 15.0; // xlsx の既定の行の高さ(pt)
        let named = self.book.named_styles.clone();
        let want = if wrap {
            sheet::cellmark::wanted_height_pt(&md, base, &named)
        } else {
            md.iter()
                .map(|l| sheet::cellmark::line_scale(l, &named))
                .fold(1.0, f32::max)
                * base
        };
        let now = *self.sheet().row_height.get(&at.row).unwrap_or(&base);
        if want > now + 0.01 {
            self.sheet_mut().row_height.insert(at.row, want);
        }
    }

    /// カーソルを動かす(動かす前に編集中の内容を確定する)。
    /// いま選んでいる長方形(左上, 右下)。
    /// 行の画面高。文書の指定(xlsx の ht、pt)に従う。既定 15pt = 24px
    pub(crate) fn row_px(&self, r: u32) -> f32 {
        self.sheet().row_height.get(&r).map(|pt| pt * 24.0 / 15.0).unwrap_or(ROW_H)
            * self.zoom
    }
}

/// シート名を変数の後ろに付ける形にする。**普通のシート名なら空**
/// (`s["A1"]` と書ける)。2枚目以降や記号の入った名前のときだけ番号を足す
pub(crate) fn sheet_var(name: &str) -> String {
    if name == "Sheet1" || name.is_empty() {
        String::new()
    } else {
        let safe: String = name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        format!("_{safe}")
    }
}

/// AI の答えから ```python …``` の囲みを取り出す。**囲みが無ければ None**
/// (表を直さない答え)。囲みの言語札は省かれることもあるので両方見る
pub(crate) fn 取り出す囲み(out: &str) -> Option<String> {
    let mut it = out.split("```");
    it.next()?; // 囲みの前
    let 中 = it.next()?;
    let 中 = 中.strip_prefix("python").unwrap_or(中);
    let t = 中.trim_start_matches('\n').trim_end().to_string();
    if t.is_empty() { None } else { Some(t) }
}

impl Calc {
    /// いま選んでいる所の名前(右パネルの見出しに出す)
    pub(crate) fn sel_label(&self) -> String {
        let (a, b) = self.sel_rect();
        if a == b { a.a1() } else { format!("{}:{}", a.a1(), b.a1()) }
    }

    /// 塗りを選択に掛ける。**色を選ぶ = べた塗り** — 柄と虹は外す
    /// (選んだ物と見える物が食い違わないように。picks.rs の一覧と同じ決め)
    pub(crate) fn set_fill(&mut self, hex: Option<&str>, label: &str) {
        let c = hex.map(|h| h.to_string());
        self.fmt(move |f| {
            f.fill = c.clone();
            f.fill_pattern = None;
            f.fill_bg = None;
            f.fill_grad = None;
        });
        self.status = if hex.is_some() {
            ui::tf!("塗りを{}にしました", label).into()
        } else {
            ui::t!("塗りを消しました").into()
        };
    }

    /// 文字の向き(xlsx と同じ数え方 — 上向きが正。255 = 縦書き)
    pub(crate) fn set_rotation(&mut self, deg: i32, label: &str) {
        let r = if deg == 0 { None } else { Some(deg) };
        self.fmt(move |f| f.rotation = r);
        self.status = ui::tf!("文字の向きを{}にしました", label).into();
    }

    /// 字下げを増やす・減らす(xlsx の alignment indent。1段 = 全角約1字)。
    ///
    /// **模型は前から持っていたのに、掛ける道が無かった**(2026-08-13 に
    /// 足したのは読み書きだけ)。右パネルで初めて人の手が届く。
    /// 0 より下げず、15 で止める(xlsx の上限)
    pub(crate) fn bump_indent(&mut self, d: i8) {
        let now = self.sheet().get(self.cursor).map(|c| c.fmt.indent).unwrap_or(0);
        let next = (now as i16 + d as i16).clamp(0, 15) as u8;
        if next == now {
            self.status = if d < 0 {
                ui::t!("字下げはこれ以上狭められません").into()
            } else {
                ui::t!("字下げはこれ以上広げられません").into()
            };
            return;
        }
        self.fmt(move |f| f.indent = next);
        self.status = ui::tf!("字下げを {} 段にしました", next).into();
    }

    /// 表示形式を選択に掛ける。**空なら外す**(標準に戻す)
    pub(crate) fn set_number_format(&mut self, code: &str) {
        let (a, b) = self.sel_rect();
        self.checkpoint();
        let s = &mut self.book.sheets[self.active];
        for r in a.row..=b.row {
            for c in a.col..=b.col {
                let p = Pos::new(r, c);
                let mut cell = s.get(p).cloned().unwrap_or_default();
                cell.fmt.number_format =
                    if code.is_empty() { None } else { Some(code.to_string()) };
                s.set(p, cell);
            }
        }
        self.dirty = true;
        self.status = if code.is_empty() {
            ui::t!("表示形式を標準に戻しました").into()
        } else {
            ui::tf!("表示形式を {} にしました", code).into()
        };
    }

    /// 会話を送る。**答えは書類でなくパネルへ**返る(CalcAi::Chat)
    pub(crate) fn chat_send(&mut self, cx: &mut Context<Self>) {
        let t = self.chat_in.text().trim().to_string();
        if t.is_empty() {
            self.status = ui::t!("用件がありません").into();
            return;
        }
        self.chat_in = Editor::new("");
        self.chat_ask(t, cx);
    }

    /// 頼みを1つ送る。**それまでのやりとりを添える**(2026-08-16)。
    ///
    /// 前は毎回1往復で切れていて、「さっきの表を今度は昇順に」が通らなかった。
    /// 一問一答の口(`ask`)しか無いので、**書き起こしを頼みの頭に畳んで**
    /// 渡す。直近だけにするのは、長い会話でセルの中身ごと膨らませないため
    pub(crate) fn chat_ask(&mut self, 用件: String, cx: &mut Context<Self>) {
        self.chat_log.push((true, 用件.clone()));
        self.chat_plan = None;
        // 直近の6つ(3往復)まで。**いま足した自分の発言は除く**
        let n = self.chat_log.len().saturating_sub(1);
        let 前 = &self.chat_log[n.saturating_sub(6)..n];
        let q = if 前.is_empty() {
            用件
        } else {
            let mut s = String::from("これまでのやりとり:\n");
            for (自分, 字) in 前 {
                s.push_str(if *自分 { "私: " } else { "あなた: " });
                s.push_str(字);
                s.push('\n');
            }
            format!("{s}\n続けて、次の頼みに答えてください。\n{用件}")
        };
        self.ai_go(CalcAi::Chat(q), cx);
    }

    /// **新しい会話にする。** やりとりも変更案も捨てる(書類は触らない)
    pub(crate) fn chat_reset(&mut self) {
        self.chat_log.clear();
        self.chat_plan = None;
        self.chat_err = None;
        self.chat_in = Editor::new("");
        self.status = ui::t!("新しい会話にしました(表は触っていません)").into();
    }

    /// **落ちた台本を直してもらう。** 出た誤りをそのまま添えて頼み直す —
    /// Agent Panel の「走らせて、落ちたら直す」の芯はここ(2026-08-16)
    pub(crate) fn chat_fix(&mut self, cx: &mut Context<Self>) {
        let Some(err) = self.chat_err.take() else { return };
        let 案 = self.chat_plan.clone().unwrap_or_default();
        self.chat_ask(
            ui::tf!(
                "さっきの台本が落ちました。誤りを読んで直した台本を出してください。\n\n\
                 【出た誤り】\n{}\n\n【落ちた台本】\n{}",
                err,
                案
            ),
            cx,
        );
    }

    /// **変更案を走らせる。** ここが「人が押した」の一点 —
    /// 押すまで AI は書類に触らない(2026-08-09 の決めの精神を保つ形)。
    /// 台本は officework の橋を通ってこのアプリを操るので、走った跡は
    /// undo の1手として残る。
    ///
    /// **裏で走らせる。** ここで待つと自分待ちになる — 台本は橋越しに
    /// calc へ話しかけるのに、calc は台本の終了を待って命令を捌けない。
    /// 2026-08-15 実機で踏んだ(「calc が応じません(忙しいか、閉じかけ)」)。
    pub(crate) fn chat_run(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = self.chat_plan.clone() else { return };
        if self.chat_busy {
            self.status = ui::t!("いま台本が走っています(終わるまでお待ちください)").into();
            return;
        }
        self.commit();
        self.checkpoint_book();
        let dir = std::env::temp_dir().join("officework-chat");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("plan.py");
        if let Err(e) = std::fs::write(&p, plan.as_bytes()) {
            self.status = ui::tf!("台本を置けません: {}", e).into();
            return;
        }
        self.chat_busy = true;
        self.status = ui::t!("変更案を走らせています…").into();
        let py = crate::py::find_python();
        let task = cx.background_executor().spawn(async move {
            let mut cmd = std::process::Command::new(py);
            cmd.arg(&p);
            crate::py::run_with_timeout(&mut cmd, 60)
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                this.chat_busy = false;
                match r {
                    Ok((true, out, _)) => {
                        this.chat_plan = None;
                        this.chat_err = None;
                        let 尻 = out.lines().rev().take(3).collect::<Vec<_>>().join(" / ");
                        // **結果を会話に戻す。** 次の頼みがこれを踏まえられる
                        this.chat_log.push((
                            false,
                            if 尻.trim().is_empty() {
                                ui::t!("入れました。").to_string()
                            } else {
                                ui::tf!("入れました。{}", 尻).to_string()
                            },
                        ));
                        this.status = if 尻.trim().is_empty() {
                            ui::t!("変更案を入れました(Ctrl+Z で戻せます)").into()
                        } else {
                            ui::tf!("変更案を入れました: {}(Ctrl+Z で戻せます)", 尻).into()
                        };
                        this.reload_from_disk_if_needed();
                    }
                    Ok((false, _, err)) => {
                        // **誤りを控えて会話にも出す。** 「直してもらう」で
                        // そのまま送れるようにする(Agent Panel の作法)
                        let 尻 = err.lines().rev().take(4).collect::<Vec<_>>().join("\n");
                        this.chat_log.push((false, ui::tf!("落ちました。{}", 尻).to_string()));
                        this.chat_err = Some(尻.clone());
                        this.status = ui::tf!("台本が落ちました: {}",
                            err.lines().rev().take(2).collect::<Vec<_>>().join(" / ")).into();
                    }
                    Err(e) => {
                        this.status = ui::tf!("台本が走りませんでした: {}", e).into();
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// 橋越しに書き換わった中身を画面へ。**橋は同じアプリを操るので、
    /// 実際にはこの場で反映済み** — 念のため再計算だけ促す
    pub(crate) fn reload_from_disk_if_needed(&mut self) {
        recalc_book(&mut self.book, self.active);
        self.dirty = true;
    }
}
