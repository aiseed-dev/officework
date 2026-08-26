//! writer のリボンのボタンと右クリックの受け口(main.rs から純移動 2026-08-08。
//! 部屋割りの5歩目)。run_cmd が1,095行の最大の塊だった。**純移動**

use crate::*;


/// 共通の命令(`ui::appcmd`)が触る面。**欄はここから増やさない** —
/// 命令を1つ移すたびに、あちらの trait と一緒に1つずつ増やす
/// ファイルのページの共通の腕が触る面(統合の段8 の3)。
impl ui::filemenu::FileScreen for Writer {
    fn tab_to_prev(&mut self) {
        self.tab = self.prev_tab;
    }
    fn set_file_view(&mut self, v: u8) {
        self.file_view = v;
    }
    fn opened(&self) -> Option<std::path::PathBuf> {
        self.path.clone()
    }
    fn new_file(&mut self) -> bool {
        self.new_doc()
    }
    /// **フォルダを開き直す。** 綴りはフォルダなので、仕事を替えるとは
    /// フォルダを替えることです(手引き `docs/ja/commands/ファイル/フォルダーを開く.adoc`)
    fn folder_dialog_now(&mut self, cx: &mut Context<Self>) {
        let start = self
            .path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .or_else(|| ui::settings::get("folder").map(std::path::PathBuf::from));
        let ask = cx.background_executor().spawn(async move {
            let mut d =
                rfd::FileDialog::new().set_title(ui::t!("choose_folder_open"));
            if let Some(s) = start.filter(|d| d.is_dir()) {
                d = d.set_directory(s);
            }
            d.pick_folder()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(d) = r {
                    this.show_folder(d);
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn open_dialog_now(&mut self, cx: &mut Context<Self>) {
        self.open_dialog(cx);
    }
    fn save_now(&mut self, cx: &mut Context<Self>) {
        self.save(false, cx);
    }
    fn save_as_now(&mut self, cx: &mut Context<Self>) {
        self.save_as(cx);
    }
    fn quit_now(&mut self, cx: &mut Context<Self>) {
        self.request_quit(cx);
    }
    fn goto_tab_named(&mut self, name: &str) {
        if let Some(i) = ribbon::WRITER.iter().position(|t| t.name == name) {
            self.prev_tab = i;
            self.tab = i;
        }
    }
}

impl ui::appcmd::Screen for Writer {
    fn zoom_mut(&mut self) -> &mut f32 {
        &mut self.zoom
    }
    fn dark_mut(&mut self) -> &mut bool {
        &mut self.dark
    }
    fn ui_scale_mut(&mut self) -> &mut f32 {
        &mut self.ui_scale
    }
    fn say(&mut self, msg: String) {
        self.status = msg.into();
    }
}

impl Writer {
    /// **一覧が開くボタン。** リボンは ▾ を添える。押すと候補の一覧
    /// (パネル)が出て、選んで終わる。腕の目印: `open_list` を立てる物だけ
    pub(crate) const MENU_IDS: &'static [&'static str] = &[
        "fontname", "fontsize", "parastyle", "inssymbol",
    ];

    /// **小窓が開くボタン。** リボンは … を添える(メニュー項目末尾の
    /// 「…」=「続きの画面がある」の古い約束)。押すと入力のパネルが開き、
    /// 続きの操作が要る。この旗の総和が [`Self::dialog_open`] —
    /// 印と「小窓中はリボン無効」が同じ一覧を見る(ずれると嘘になる)。
    /// 常駐パネルの表示切替(nav / show-left / show-right / edit-header)は
    /// 「すぐ効く」— 無印で、ここにも入れない
    pub(crate) const DIALOG_IDS: &'static [&'static str] = &[
        "replace", "watermark", "bookmarks", "co-addcomment", "co-history",
        "co-chat", "py-list", "form-combo",
        "form-dropdown", "form-name", "ruby", "insequation",
    ];

    /// **小窓(… の側)が開いているか。** [`Self::DIALOG_IDS`] の腕が立てる
    /// 旗の総和 — 印と1対1で揃える。真の間はリボン全体(タブの切替も)を
    /// 無効にする。閉じる道は今のまま(Esc・小窓の中のボタン)。
    /// hf_edit(ヘッダー/フッター)はモード切替なので入れない
    pub(crate) fn dialog_open(&self) -> bool {
        self.find_open
            || self.wm_edit
            || self.bm_open
            || self.cmt_edit
            || self.hist_open
            || self.plug_open
            || self.pw_open
            || self.sd_open
            || self.ai_open
            || self.rb_open
            || self.eq_open
            || self.chat_open
    }

    /// 開いている一覧(▾ の側)を畳む。**他のボタンやタブを押したら閉じ、
    /// 押した操作はそのまま効く**約束(発注者 2026-08-14)
    pub(crate) fn close_menus(&mut self) {
        self.open_list = None;
    }

    /// リボンのボタンから命令を出す。**小窓中は何も通さない** — 描画の
    /// 縛り(灰色・無反応)と二重の錠。閉じる道(Esc・小窓の中のボタン・
    /// 鍵盤の割り当て)は run_cmd 直呼びなので今のまま通る
    pub(crate) fn run_from_ribbon(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if self.dialog_open() {
            return;
        }
        self.run_cmd(id, cx);
    }

    /// **一覧の仕事を始める。** 名前を打つ欄を出します(2026-08-26)。
    pub(crate) fn fl_start(&mut self, job: crate::FlJob) {
        use crate::FlJob as J;
        let (前置き, 案内) = match &job {
            J::NewFolder => (String::new(), ui::t!("type_name_new_folder").to_string()),
            J::NewDoc => (String::new(), ui::t!("type_name_new_document").to_string()),
            J::Rename(p) => (
                p.file_name().unwrap_or_default().to_string_lossy().to_string(),
                ui::t!("type_new_name_press").to_string(),
            ),
            J::Delete(p) => (
                String::new(),
                ui::tf!("delete_enter_delete_esc",
                        p.file_name().unwrap_or_default().to_string_lossy().to_string())
                    .to_string(),
            ),
        };
        self.fl_ed = Editor::new(&前置き);
        self.fl_job = Some(job);
        self.status = 案内.into();
    }

    /// **一覧の仕事を実行する。** 断られたら欄を開いたままにします。
    ///
    /// *ごみ箱には入りません。* 消す前に一度確かめる形にしてあります
    /// (`Delete` は Enter が確かめの返事です)。
    pub(crate) fn fl_commit(&mut self, cx: &mut Context<Self>) {
        use crate::FlJob as J;
        let Some(job) = self.fl_job.clone() else { return };
        let 名 = self.fl_ed.text().to_string();
        let 今 = self.folder();
        let 結果: Result<String, String> = match &job {
            J::NewFolder => match 今 {
                Some(d) => ui::folder::フォルダを作る(&d, &名)
                    .map(|p| ui::tf!("created",
                        p.file_name().unwrap_or_default().to_string_lossy().to_string()).to_string()),
                None => Err(ui::t!("no_folder_open").to_string()),
            },
            J::NewDoc => match 今 {
                Some(d) => {
                    // **拡張子は付けます。** 付け忘れた物が「文字だけの
                    // ファイル」になって開けないのを防ぎます
                    let n = if 名.trim().ends_with(".adoc") {
                        名.trim().to_string()
                    } else {
                        format!("{}.adoc", 名.trim())
                    };
                    let 題 = 名.trim().trim_end_matches(".adoc");
                    ui::folder::ファイルを作る(&d, &n, &format!("= {題}
"))
                        .map(|p| ui::tf!("created",
                            p.file_name().unwrap_or_default().to_string_lossy().to_string()).to_string())
                }
                None => Err(ui::t!("no_folder_open").to_string()),
            },
            J::Rename(元) => ui::folder::名前を変える(元, &名).map(|先| {
                // **いま開いている物なら、道も付け替えます**
                if self.path.as_deref() == Some(元.as_path()) {
                    self.path = Some(先.clone());
                }
                ui::tf!("renamed",
                    先.file_name().unwrap_or_default().to_string_lossy().to_string()).to_string()
            }),
            J::Delete(道) => {
                // **開いたままの物は消しません。** 消してから保存すると
                // 元に戻るので、消えたのか残ったのか分からなくなります
                if self.path.as_deref() == Some(道.as_path()) {
                    Err(ui::t!("cant_delete_file_open").to_string())
                } else {
                    ui::folder::消す(道).map(|_| {
                        ui::tf!("deleted",
                            道.file_name().unwrap_or_default().to_string_lossy().to_string()).to_string()
                    })
                }
            }
        };
        match 結果 {
            Ok(言う) => {
                self.fl_job = None;
                self.status = 言う.into();
            }
            // **断られたら開いたまま。** 打ち直せるようにします
            Err(e) => self.status = e.into(),
        }
        cx.notify();
    }

    /// **打った数で表を挿す。**(2026-08-25)
    ///
    /// 受けるのは `行数,列数` です。`3x4` や `3 4` も同じに読みます —
    /// 打ち方で断らないためです。数が読めなければ、そう言って開いたままに
    /// します(黙って 3×3 を入れると、打ち間違いに気づけません)。
    pub(crate) fn tbl_commit(&mut self, cx: &mut Context<Self>) {
        let 字 = self.tbl_ed.text().to_string();
        let 数: Vec<usize> = 字
            .split(|c: char| !c.is_ascii_digit())
            .filter(|x| !x.is_empty())
            .filter_map(|x| x.parse().ok())
            .collect();
        let (行, 列) = match 数.as_slice() {
            [r, c] if *r >= 1 && *c >= 1 => (*r, *c),
            _ => {
                self.status =
                    ui::t!("type_rows_columns_e").into();
                return;
            }
        };
        // **上限を置きます。** 打ち間違いで 999,999 と入れると、
        // 組むのに何分も掛かって固まったように見えます
        if 行 > 200 || 列 > 50 {
            self.status = ui::t!("too_large_up_200").into();
            return;
        }
        self.tbl_open = false;
        self.table_size = (行, 列);
        self.run_cmd("instable-go", cx);
    }

    pub(crate) fn run_cmd(&mut self, id: &str, cx: &mut Context<Self>) {
        // 一覧(▾)は**他を押したら閉じ、押した操作はそのまま効く**
        // (発注者 2026-08-14)。自分のボタンだけは畳まない — トグル
        // (もう一度押すと閉じる)の動きを壊さないため
        // 自分のボタンだけは畳まない — トグル(もう一度押すと閉じる)の
        // 動きを壊さないため。**旗が1つなので、この判断も1行です**
        if self.open_list != Some(id) {
            self.open_list = None;
        }
        // **打つ欄も、他のボタンを押したら閉じます**(2026-08-25)。
        // 開いたままだと `editor()` が本文でなく欄を返すので、次に押した
        // ボタンが*欄の字*を編集します(試験で「blankpage が戻らない」と
        // して出ました。画面でも同じことが起きます)
        if self.tbl_open && id != "instable-go" {
            self.tbl_open = false;
        }
        // **ネイティブ文書では見た目を直に変えさせない**(2026-08-16)。
        // 名前を付けてスタイルにする道へ寄せる — Word の失敗
        // (直接書式が同じくらい簡単なら誰もスタイルを使わない)を
        // 設計で塞ぐ要。互換(docx)は今までどおり
        if self.look_guard(id, cx) {
            return;
        }
        // **一手 = 控え1枚。** 中で打鍵や段落の変更を呼ぶ命令があるので、
        // ここで旗を落とし、最初の1枚だけを通す
        self.acted = false;
        self.run_cmd_inner(id, cx);
        self.acted = false;
    }
    fn run_cmd_inner(&mut self, id: &str, cx: &mut Context<Self>) {
        // 読み取り専用の保護。文書を変えるボタンはここで断る(見る・出す・
        // 保存・検索の類いは通す)。解除はいつでも「保護」のボタン1手
        const READONLY_OK: &[&str] = &[
            "open", "save", "pdf", "zoom-in", "zoom-out", "ruler", "darkmode",
            "line-numbers", "hidenchars", "selectall", "spell", "wordcount",
            "co-showcomment", "replace", "prot-doc", "coauth-mode",
            "co-history", "co-chat", "prot-sign", "copy",
        ];
        if self.protected() && !READONLY_OK.contains(&id) {
            self.status =
                ui::t!("protected_read_only_protection").into();
            return;
        }
        // **共通の命令は1本の捌き手へ**(2026-08-19)。同じ id の腕を
        // ここに残すと死ぬので、移したら消す
        if ui::appcmd::run(self, id) {
            return;
        }
        match id {
            "open" => self.open_dialog(cx),
            "save" => self.save(false, cx),
            "undo" => { if self.editor().undo() { self.on_edited() } }
            "redo" => { if self.editor().redo() { self.on_edited() } }
            "selectall" => self.ed.select_all(),
            "spell" => self.run_proof(),
            // 文字書式 — 押すたびに入切する(Word と同じ挙動)。
            // **先にカーソル位置の書式で入か切かを決めて、選択全体に写す** —
            // 混ざった選択で run ごとに反転させない(Word の作法)
            "bold" => {
                let on = !self.doc.char_format_at(self.ed.selection()).bold;
                self.toggle(move |f| f.bold = on);
            }
            "italic" => {
                let on = !self.doc.char_format_at(self.ed.selection()).italic;
                self.toggle(move |f| f.italic = on);
            }
            "underline" => {
                let on = !self.doc.char_format_at(self.ed.selection()).underline;
                self.toggle(move |f| f.underline = on);
            }
            "strikeout" => {
                let on = !self.doc.char_format_at(self.ed.selection()).strike;
                self.toggle(move |f| f.strike = on);
            }
            // 上付きと下付きは同時には成らない
            "superscript" => {
                let on = !self.doc.char_format_at(self.ed.selection()).superscript;
                self.toggle(move |f| {
                    f.superscript = on;
                    if on { f.subscript = false }
                });
            }
            "subscript" => {
                let on = !self.doc.char_format_at(self.ed.selection()).subscript;
                self.toggle(move |f| {
                    f.subscript = on;
                    if on { f.superscript = false }
                });
            }
            // 蛍光ペン。黄 → 緑 → 解除(色を選ぶ小窓はまだ無い)
            "highlight" => {
                let next = match self.doc.char_format_at(self.ed.selection())
                    .highlight.as_deref()
                {
                    None => Some("yellow".to_string()),
                    Some("yellow") => Some("green".to_string()),
                    _ => None,
                };
                self.toggle(move |f| f.highlight = next.clone());
            }
            // 書式のクリア。文字書式だけを外す(本文と段落の性質は残す)
            "clearstyle" => self.toggle(|f| *f = Default::default()),
            // 段落の揃え
            "align-left" => self.set_align(Align::Left),
            "align-center" => self.set_align(Align::Center),
            "align-right" => self.set_align(Align::Right),
            "align-just" => self.set_align(Align::Justify),
            // 均等割付(日本語一級)。最後の行も行長いっぱいに字間を配る
            "align-dist" => self.set_align(Align::Distribute),
            // 縦書き(K4)。sectPr の textDirection=tbRl と往復。
            // 初版の約束: 表・段組みは縦にならず、ASCII は1字ずつ縦に積む
            "direction" => {
                self.flush_target();
                self.checkpoint(false);
                self.doc.vertical = !self.doc.vertical;
                self.dirty = true;
                self.relayout();
                self.status = if self.doc.vertical {
                    let caveat = if self.doc.tables().next().is_some() {
                        ui::t!("tables_not_turn_vertical")
                    } else {
                        ""
                    };
                    ui::tf!("vertical_writing_columns_run", caveat)
                        .into()
                } else {
                    ui::t!("back_horizontal_writing").into()
                };
            }
            // ルビ(日本語一級)。選んだ字の上に半分の大きさで読みを振る
            "ruby" => {
                self.switch_target(Target::Body);
                let sel = self.ed.selection();
                if sel.is_empty() {
                    self.status = ui::t!("select_text_set_ruby").into();
                    return;
                }
                self.rb_range = sel.clone();
                let cur = self.doc.char_format_at(sel).ruby.unwrap_or_default();
                self.rb_ed = Editor::new(&cur);
                self.find_open = false;
                self.hf_edit = None;
                self.cmt_edit = false;
                self.rb_open = true;
                self.status =
                    ui::t!("ruby_type_reading_press").into();
            }
            // 脚注。**選んだ字を注へ移し**、跡に印を置く。
            // 空の注を作って別の窓で打たせる形(Word の作法)にはしない —
            // 注を打つ場所をまだ持っていないので、持っていない物を
            // 持っている顔をすることになる
            "footnote" => {
                self.switch_target(Target::Body);
                let sel = self.ed.selection();
                if sel.is_empty() {
                    self.status = ui::t!("select_text_turn_into").into();
                    return;
                }
                let at = sel.start;
                self.checkpoint(false);
                match self.doc.make_footnote(sel, false) {
                    Some(_) => {
                        // 字が注へ移ったので、編集中の平文を取り直す
                        self.ed = Editor::new(&self.doc.body_text());
                        let len = self.ed.text().len();
                        self.ed.move_to(at.min(len), false);
                        self.relayout();
                        self.dirty = true;
                        self.status = ui::t!("moved_selected_text_into").into();
                    }
                    None => {
                        self.status =
                            ui::t!("cannot_make_footnote_selection").into();
                    }
                }
            }
            // 文字の大きさの +/−。**一覧と同じ並びを1段ずつ辿る**(±1pt では
            // ない — calc と同じ ui::combo の判断。半端は隣の一覧値へ寄り、端で
            // 止まる)。並びには文書の標準(テンプレートの大きさ。既定 10.5)を
            // 差し込む — 一覧で選べる値には +/− も止まる
            "incfont" => {
                let 標準 = self.base_pt();
                self.size(move |s| ui::combo::step_size_with(Some(標準), s, true))
            }
            "decfont" => {
                let 標準 = self.base_pt();
                self.size(move |s| ui::combo::step_size_with(Some(標準), s, false))
            }
            // 印刷・PDF。**組み直さない** — 画面と同じ紙面をそのまま写す
            "pdf" => self.save_pdf(cx),
            // 文字色。押すたびに 赤 → 青 → 黒(解除)と回す。
            // 色を選ぶ小窓はまだ無いので、**無い機能を有るように見せず**
            // 使える範囲で回す形にしてある
            // 箇条書き・段落番号。押すたびに入切する
            "markers" => self.para(|p| {
                p.list = if p.list == ListKind::Bullet { ListKind::None } else { ListKind::Bullet }
            }),
            // 複数レベルのリスト。箇条書きにして1段深く(印はレベルで変わる)。
            // 深さは Tab / Shift+Tab でも動かせる
            "multilevels" => {
                self.para(|p| {
                    if p.list == ListKind::None {
                        p.list = ListKind::Bullet;
                    } else {
                        p.indent = (p.indent + 1).min(8);
                    }
                });
                self.status =
                    ui::t!("leveled_list_tab_shift").into();
            }
            "numbering" => self.para(|p| {
                p.list = if p.list == ListKind::Number { ListKind::None } else { ListKind::Number }
            }),
            // インデント。0〜20段に留める
            "incoffset" => self.para(|p| p.indent = (p.indent + 1).min(20)),
            "decoffset" => self.para(|p| p.indent = p.indent.saturating_sub(1)),
            // 行間。1.0 → 1.5 → 2.0 → 1.0 と回す(小窓がまだ無いので)
            // この段落の前で改ページ(押すたびに入切)
            "pagebreak" => self.para(|p| p.page_break_before = !p.page_break_before),
            // 段落の背景色。無し → 薄黄 → 薄青 → 無し、で回す
            "paracolor" => self.para(|p| {
                p.shade = match p.shade.as_deref() {
                    None => Some("FFF2CC".into()),
                    Some("FFF2CC") => Some("DEEAF6".into()),
                    _ => None,
                }
            }),
            // 段落の囲み枠(入切)
            "borders" => self.para(|p| p.boxed = !p.boxed),
            // ドロップキャップ(頭の1字を大きく。押すたびに入切)
            "dropcap" => {
                self.para(|p| p.dropcap = !p.dropcap);
                self.status =
                    ui::t!("drop_cap_toggled_becomes").into();
            }
            // 画像の挿入。段落の下に付く(選択も**別のスレッド**)。
            // 図形・グラフ・SmartArt・テキストアート・方程式も同じ道 —
            // **絵は Python で描いて画像として貼る**(SEKKEI「writer の挿入系」)。
            // 灰色で残すより、方針どおりに動くボタンにする(発注者判断)
            // 数式。**LaTeX で受けて、組むのは Python**(自前で組版は書かない —
            // calc がグラフを matplotlib に任せるのと同じ分業)。TeX が入って
            // いればそちらで組み、無ければ matplotlib に落ちる。
            // 打った原文は絵と一緒に持ち越すので、開き直しても直せる
            "insequation" => {
                self.switch_target(Target::Body);
                self.eq_ed = Editor::new("");
                self.find_open = false;
                self.hf_edit = None;
                self.cmt_edit = false;
                self.rb_open = false;
                self.eq_open = true;
                self.status =
                    ui::t!("equation_type_latex_press_enter").into();
            }
            "insimage" | "insshape" | "inssmartart" | "inschart"
            | "instextart" => {
                if id != "insimage" {
                    self.status =
                        ui::t!("draw_figures_python_matplotlib").into();
                }
                let ask = cx.background_executor().spawn(async {
                    rfd::FileDialog::new()
                        .add_filter(ui::t!("images"), &["png", "jpg", "jpeg", "svg"])
                        .pick_file()
                });
                cx.spawn(async move |this, cx| {
                    let r = ask.await;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(p) = r {
                            this.insert_image(&p);
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            // テキストボックス = 1×1 の表。枠の中に文字が要る様式は
            // 表で組むのが日本の事務の通り相場(SEKKEI)
            "instext" => {
                self.checkpoint(false); // テキストボックス
                let empty = kumihan::Cellbox {
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
                };
                self.flush_target();
                self.doc.blocks.push(kumihan::Block::Table(kumihan::Table {
                    col_mm: vec![80.0],
                    rows: vec![vec![empty]],
                    ..Default::default()
                }));
                self.dirty = true;
                self.relayout_keep();
                self.status =
                    ui::t!("1_1_frame_added").into();
            }
            // 大文字小文字。選択の英字を 全部大文字 ⇄ 全部小文字 で切り替える
            // (小文字が混ざっていれば大文字へ。1手で戻せる)
            "changecase" => {
                let sel = self.ed.selection();
                if sel.is_empty() {
                    self.status = ui::t!("select_text_change").into();
                } else if let Some(t) = self.ed.text().get(sel.clone()) {
                    let up = t.chars().any(|c| c.is_lowercase());
                    let new = if up { t.to_uppercase() } else { t.to_lowercase() };
                    let start = sel.start;
                    let n = new.len();
                    self.ed.insert(&new);
                    // 選択を保つ(続けてもう一度押せるように)
                    self.ed.move_to(start, false);
                    self.ed.move_to(start + n, true);
                    self.on_edited();
                }
            }
            // 空白ページの挿入 = 段落を切って、新しい段落を次の頁の頭から
            "blankpage" => {
                self.checkpoint(false); // 空白ページ
                handler::replace(self, None, "\n");
                self.para(|p| p.page_break_before = true);
                self.status = ui::t!("new_page_starts_here").into();
            }
            // 表の挿入。3×3 を末尾に(大きさを選ぶ小窓はまだ無い)。
            // セル編集が入っているので、挿した表はそのまま書ける
            // **表は行数と列数を打ってから挿します**(2026-08-25 発注者
            // 「行×列を選ぶ画面は、数値入力にしないと選択ではだめでしょう」)。
            //
            // 前は 64 個の組を一覧に並べていました。1つずつ選ぶ形なので、
            // 4×6 を出すのに 64 個から目で探すことになり、使えません。
            // **打った数がそのまま大きさ**になる形にします。
            // 表の横幅は文章が書ける幅で、それを列数で割ります
            // (`col_mm` を空で作り、組む側が行長を割ります)
            "instable" => {
                self.tab = self.prev_tab;
                let (r, c) = self.table_size;
                self.tbl_ed = Editor::new(&format!("{r},{c}"));
                self.tbl_open = true;
                self.status = ui::t!("table_size_type_rows").into();
            }
            "instable-go" => {
                self.checkpoint(false); // 表
                let empty = || kumihan::Cellbox {
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
                };
                self.flush_target();
                let (行, 列) = self.table_size;
                self.doc.blocks.push(kumihan::Block::Table(kumihan::Table {
                    col_mm: vec![],
                    rows: (0..行).map(|_| (0..列).map(|_| empty()).collect()).collect(),
                    ..Default::default()
                }));
                self.dirty = true;
                self.relayout_keep();
                self.status =
                    ui::tf!("table_added_end_click", 行, 列)
                        .into();
            }
            // 記号の一覧(押すと出る/消える)
            "inssymbol" => {
                self.open_list = (self.open_list != Some("inssymbol")).then_some("inssymbol");
                self.pick_sel = 0;
            }
            // ファイルからのテキスト。カーソルの位置に差し込む(undo の1手)
            "text-from-file" => {
                let ask = cx.background_executor().spawn(async {
                    rfd::FileDialog::new()
                        .add_filter(ui::t!("text_word_document"), &["txt", "md", "docx"])
                        .pick_file()
                });
                cx.spawn(async move |this, cx| {
                    let r = ask.await;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(p) = r {
                            this.insert_text_from(&p);
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            // テキストの追加(参考資料)= この段落を目次の材料にする。
            // 押すたびに 標準 → 見出し1 → 2 → 3 → 標準 と回る
            "add-text" => {
                self.checkpoint(false); // 目次に入れる見出し
                let sel = self.ed.selection();
                let now = match self.target {
                    Target::Body => self.doc.para_at(sel).map(|p| p.style).unwrap_or_default(),
                    Target::Cell { .. } => Default::default(),
                };
                let next = match now {
                    kumihan::ParaStyle::Heading(n) if n < 3 => n + 1,
                    kumihan::ParaStyle::Heading(_) => 0,
                    _ => 1,
                };
                self.set_para_style(next);
            }
            // 置換のパネル。開いている間、打鍵は検索欄に入る
            "replace" => {
                self.find_open = !self.find_open;
                self.find_field = 0;
                if self.find_open {
                    self.switch_target(Target::Body);
                    self.status = ui::t!("type_search_term_enter").into();
                }
            }
            // 画面の倍率。50〜200%。紙は変わらない
            // 見え方だけの切り替え(文書は変わらない)
            "hidenchars" => self.show_marks = !self.show_marks,
            // 一覧パネル(フォント・大きさ)。選ぶのはパネルの中
            "fontname" => {
                let 開く = self.open_list != Some("fontname");
                self.open_list = 開く.then_some("fontname");
                // **絞り込みの欄を開く**(手順2)。数で切らない代わりです
                self.font_filter = 開く.then(|| Editor::new(""));
                // **開いたときは今の書体の位置に居る**(表の画面と同じ)。
                // 一覧の頭に飛ぶと、今どれなのかが分からなくなります
                self.pick_sel = if 開く {
                    let 今 = self.font_name.to_string();
                    self.一覧の中身("fontname").iter().position(|(k, _)| *k == 今).unwrap_or(0)
                } else {
                    0
                };
            }
            // 用紙。向き / サイズ / 余白(選ぶ小窓は無いが、回して選べる)
            "pageorient" => self.set_page(|pg| {
                std::mem::swap(&mut pg.w_mm, &mut pg.h_mm);
            }),
            "pagesize" => self.set_page(|pg| {
                // A4 → B5 → A3 → A4(向きは保つ)
                let landscape = pg.w_mm > pg.h_mm;
                let (w, h) = match (pg.w_mm.min(pg.h_mm) * 10.0) as u32 {
                    2100 => (182.0, 257.0), // → B5
                    1820 => (297.0, 420.0), // → A3
                    _ => (210.0, 297.0),    // → A4
                };
                (pg.w_mm, pg.h_mm) = if landscape { (h, w) } else { (w, h) };
            }),
            // 段組み。1 → 2 → 3 → 1 と回る(見た目も docx も追随)
            "columns" => self.set_page(|pg| {
                pg.columns = match pg.cols() {
                    1 => 2,
                    2 => 3,
                    _ => 1,
                };
            }),
            "pagemargins" => self.set_page(|pg| {
                // 標準20 → 狭い12 → 広い30 → 標準
                let next = match pg.left_mm as u32 {
                    20 => 12.0,
                    12 => 30.0,
                    _ => 20.0,
                };
                pg.left_mm = next;
                pg.right_mm = next;
                pg.top_mm = next;
                pg.bottom_mm = next;
            }),
            "fontsize" => self.open_list = (self.open_list != Some("fontsize")).then_some("fontsize"),
            // 段落のスタイルの一覧(標準・見出し1〜3)
            "parastyle" => self.open_list = (self.open_list != Some("parastyle")).then_some("parastyle"),
            // 目次。挿す・挿し直すは同じ道(Toc の印の連続を置き換える)
            "toc" | "toc-update" => self.make_toc(),
            // 図表目次も同じ作法(Tof の印)
            "tof" | "tof-update" => self.make_tof(),
            // ヘッダー・フッターの編集(パネル。開いている間、打鍵はそこへ)
            "edit-header" => self.open_hf(false),
            "edit-footer" => self.open_hf(true),
            // ページ番号・ページ数。開いているパネル(無ければフッター)の
            // カーソル位置に印を入れる
            "pagenum" | "numpages" => {
                if self.hf_edit.is_none() {
                    self.open_hf(true);
                }
                if self.hf_edit.is_some() {
                    let (mark, what) = if id == "pagenum" {
                        (kumihan::PAGE_MARK, ui::t!("page_number"))
                    } else {
                        (kumihan::PAGES_MARK, ui::t!("page_count"))
                    };
                    self.hf_ed.insert(&mark.to_string());
                    self.on_edited();
                    self.status =
                        ui::tf!("inserted_becomes_field_docx", what).into();
                }
            }
            // 日付。**固定の文字**として入れる(開くたび変わるフィールドは、
            // 事務の書類では事故のもと — 提出日が勝手に変わる)
            // **形式の一覧から選びます**(2026-08-25 発注者「形式の一覧は
            // 必要」)。西暦と和暦の両方を出します — 事務の様式は和暦で
            // 書くものが多く、毎回打ち直すのは手間です。
            // **自動更新は作りません** — 印刷した日に文書の日付が変わるのは
            // 事故の元で、固定の字が正です
            "datetime" => {
                self.open_list = (self.open_list != Some("datetime")).then_some("datetime");
                self.pick_sel = 0;
            }
            // **読み飛ばした物をまた出す**(2026-08-26)。断りは閉じられる
            // ようにしたので、見直す道が要ります
            "show-notes" => {
                // **下の帯に並べます。** 浮く小窓は出しません
                self.status = if self.notes.is_empty() {
                    ui::t!("nothing_skipped").into()
                } else {
                    let 中 = self
                        .notes
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(" / ");
                    ui::tf!("skipped", 中).into()
                };
            }
            "ruler" => self.ruler = !self.ruler,
            // ダークモード。**紙は白いまま**(画面と紙の一致)。周りだけ暗くする
            // ダークモード。**紙は白いまま**(画面と紙の一致)。周りだけ暗くする。
            // 判断と控えは ui::toggle_dark の1本(表と共通) — 前は控えて
            // いなかったので、開き直すと明るさが戻っていた
            // 変更履歴。記録中の編集は、保存で Word の w:ins / w:del になる
            "track-changes" => {
                self.flush_target();
                self.track = !self.track;
                if self.track {
                    self.track_base =
                        Some(self.doc.paragraphs().map(para_text).collect());
                    self.status =
                        ui::t!("recording_changes_become_word").into();
                } else {
                    self.track_base = None;
                    self.status =
                        ui::t!("stopped_recording_changes_recorded").into();
                }
            }
            // 描画。ペン・蛍光ペン・消しゴム(もう一度押すか Esc で戻る)。
            // 筆は文書に入り、docx では自由曲線の図形になる(ページに固定)
            "pen" | "highlighter" | "eraser" => {
                let t = match id { "pen" => 0u8, "highlighter" => 1, _ => 2 };
                self.tool = if self.tool == Some(t) { None } else { Some(t) };
                self.ink_cur = None;
                self.status = match self.tool {
                    Some(0) => ui::t!("pen_drag_page_draw").into(),
                    Some(1) => ui::t!("highlighter_drag_mark_light").into(),
                    Some(2) => ui::t!("eraser_trace_line_remove").into(),
                    _ => ui::t!("back_text_editing").into(),
                };
            }
            // 図表番号。カーソルの段落の下に「図 N」を入れる
            // (画像は段落の下に付くので、その下=図の下になる)。
            // 番号は既にある「図 n」の最大 + 1
            "caption" => {
                self.checkpoint(false); // 図表番号
                self.switch_target(Target::Body);
                self.flush_target();
                let mut n = 0usize;
                // 探す頭は貼る雛形と同じところから(crate::caption_head の註)
                let head = crate::caption_head();
                for p in self.doc.paragraphs() {
                    let t: String = p.runs.iter().map(|r| r.text.as_str()).collect();
                    if let Some(rest) = t.trim().strip_prefix(head) {
                        if let Ok(k) = rest.trim().parse::<usize>() {
                            n = n.max(k);
                        }
                    }
                }
                let label = ui::tf!("figure", n + 1);
                let (pi, b0) = self.cursor_para();
                let plen: usize = self
                    .doc
                    .paragraphs()
                    .nth(pi)
                    .map(|p| p.runs.iter().map(|r| r.text.len()).sum())
                    .unwrap_or(0);
                // 編集(undo の1手)と blocks を同じ形で揃える(目次と同じ作法)
                let end = b0 + plen;
                self.ed.move_to(end, false);
                self.ed.move_to(end, true);
                self.ed.insert(&format!("\n{label}"));
                let para_block_idx: Vec<usize> = self
                    .doc
                    .blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| matches!(b, kumihan::Block::Para(_)))
                    .map(|(i, _)| i)
                    .collect();
                let cap = kumihan::Paragraph {
                    align: Align::Center,
                    line_spacing: 1.0,
                    runs: vec![kumihan::Run {
                        text: label.clone(),
                        size_pt: None,
                        font: None,
                        fmt: Default::default(),
                    }],
                    ..Default::default()
                };
                self.doc.blocks.insert(para_block_idx[pi] + 1, kumihan::Block::Para(cap));
                self.dirty = true;
                self.relayout();
                self.follow_caret();
                self.status = ui::tf!("inserted_centred_paragraph", label).into();
            }
            // 相互参照。しおり一覧から「文字」「ページ」を挿すパネル
            "crossref" => {
                self.xr_open = !self.xr_open;
                if self.xr_open {
                    self.bm_open = false;
                    self.find_open = false;
                    self.hf_edit = None;
                    self.cmt_edit = false;
                    self.wm_edit = false;
                    self.status =
                        ui::t!("cross_reference_pick_bookmark").into();
                }
            }
            // しおり。一覧のパネル(名前を打って追加・押して移動・✕で削除)
            "bookmarks" => {
                self.bm_open = !self.bm_open;
                if self.bm_open {
                    self.find_open = false;
                    self.hf_edit = None;
                    self.cmt_edit = false;
                    self.wm_edit = false;
                    self.bm_ed = Editor::new("");
                    self.status =
                        ui::t!("bookmarks_type_name_add").into();
                }
            }
            // 透かし。パネルで文字を打つ(空にして閉じると外れる)。
            // 文書ではヘッダーの中の VML になり、Word でも斜めの薄い字で出る
            "watermark" => {
                if self.wm_edit {
                    self.wm_edit = false;
                    return;
                }
                if self.doc.header.paragraphs.is_empty() && self.doc.header.part.is_some() {
                    self.status =
                        ui::t!("header_contains_table_no").into();
                    return;
                }
                self.find_open = false;
                self.hf_edit = None;
                self.cmt_edit = false;
                self.wm_ed = Editor::new(self.doc.watermark.as_deref().unwrap_or(""));
                self.wm_edit = true;
                self.status = ui::t!("editing_watermark_close_empty").into();
            }
            // ページの色。無し → 薄クリーム → 薄青 → 薄緑 → 無し(文書に入り、
            // 保存で残る。紙(PDF)も同じ色に塗る)
            "pagecolor" => {
                self.checkpoint(false); // ページの色
                self.doc.page_color = match self.doc.page_color.as_deref() {
                    None => Some("FFF7DC".into()),
                    Some("FFF7DC") => Some("E8F1F8".into()),
                    Some("E8F1F8") => Some("EAF5EE".into()),
                    _ => None,
                };
                self.dirty = true;
                self.status = match &self.doc.page_color {
                    Some(c) => ui::tf!("page_colour", c).into(),
                    None => ui::t!("page_colour_none").into(),
                };
            }
            // 行番号(見え方だけ)。折り返した行も1行と数える(見た目の行)
            "line-numbers" => self.line_numbers = !self.line_numbers,
            // 欧文のハイフネーション(入切)。日本語は禁則で折るので変わらない
            "hyphenation" => {
                self.checkpoint(false); // ハイフネーション
                self.doc.hyphenate = !self.doc.hyphenate;
                self.dirty = true;
                self.relayout_keep();
                self.status = if self.doc.hyphenate {
                    ui::t!("hyphenation_english_words_break").into()
                } else {
                    ui::t!("hyphenation_off").into()
                };
            }
            // コメントの印と一覧の表示(見え方だけ)
            "co-showcomment" => {
                self.show_comments = !self.show_comments;
                self.status = if self.show_comments {
                    ui::t!("comments_shown").into()
                } else {
                    ui::t!("comments_hidden_still_there").into()
                };
            }
            // カーソルの段落のコメントを外す
            "co-delcomment" => {
                self.switch_target(Target::Body);
                let (pi, _) = self.cursor_para();
                let mut removed = 0usize;
                let mut i = 0usize;
                for b in &mut self.doc.blocks {
                    if let kumihan::Block::Para(p) = b {
                        if i == pi {
                            removed = p.comments.len();
                            p.comments.clear();
                            break;
                        }
                        i += 1;
                    }
                }
                if removed > 0 {
                    self.dirty = true;
                    self.status =
                        ui::tf!("comments_removed_paragraph", removed).into();
                } else {
                    self.status = ui::t!("paragraph_no_comments").into();
                }
            }
            // コメント(段落単位)。カーソルの段落に付ける
            "co-addcomment" | "comment" => {
                if self.cmt_edit {
                    self.cmt_edit = false;
                    return;
                }
                self.switch_target(Target::Body);
                let (pi, _) = self.cursor_para();
                self.cmt_para = pi;
                let text = self
                    .doc
                    .paragraphs()
                    .nth(pi)
                    .and_then(|p| p.comments.first())
                    .map(|c| c.text.clone())
                    .unwrap_or_default();
                self.cmt_ed = Editor::new(&text);
                self.find_open = false;
                self.hf_edit = None;
                self.cmt_edit = true;
                self.status =
                    ui::t!("editing_comment_attaches_paragraph").into();
            }
            // 文書の保護。readOnly を docx の documentProtection と往復する。
            // パスワードは掛けない(**掛けた振りもしない**)— Word でも
            // 「編集の制限」として見え、解除も同じ1手でできる正直な保護
            "prot-doc" => {
                self.checkpoint(false); // 文書の保護
                if self.doc.protection.is_some() {
                    self.doc.protection = None;
                    self.dirty = true;
                    self.status =
                        ui::t!("protection_removed_editing_allowed").into();
                } else {
                    self.flush_target();
                    self.doc.protection = Some("readOnly".into());
                    // 文書を変えるパネルとペンは店じまい
                    self.hf_edit = None;
                    self.wm_edit = false;
                    self.cmt_edit = false;
                    self.tool = None;
                    self.dirty = true;
                    self.status = ui::t!("protected_read_only_same").into();
                }
            }
            // 共同編集モード。実体はファイルの錠(.~lock)による早い者勝ちの
            // 編集権。押すと錠の今を確かめ、先客が去っていれば編集権を取り直す
            "coauth-mode" => match self.path.clone() {
                None => {
                    self.status =
                        ui::t!("not_file_yet_saving").into();
                }
                Some(p) => {
                    if self.my_lock.is_some() {
                        self.status = ui::tf!("hold_editing_rights_whoever", lock_identity())
                        .into();
                    } else {
                        self.acquire_lock(&p);
                        self.status = match &self.locked_by {
                            Some(who) => ui::tf!("editing_can_read_but", who)
                            .into(),
                            None => ui::t!("previous_editor_left_editing").into(),
                        };
                    }
                }
            },
            // バージョン履歴。上書き保存のたびに .jo-history へ残る控えの一覧
            "co-history" => {
                self.hist_open = !self.hist_open;
                if self.hist_open {
                    self.chat_open = false;
                    self.bm_open = false;
                    self.xr_open = false;
                    self.status = if self.path.is_none() {
                        ui::t!("not_file_yet_once").into()
                    } else {
                        ui::t!("version_history_click_open").into()
                    };
                }
            }
            // チャット。文書の隣の申し送り帳(.chat.txt)へ名乗り付きで追記。
            // サーバーは無いので生放送ではない — ファイル越しの言伝(ことづて)
            "co-chat" => {
                self.chat_open = !self.chat_open;
                if self.chat_open {
                    self.hist_open = false;
                    self.bm_open = false;
                    self.xr_open = false;
                    self.find_open = false;
                    self.chat_ed = Editor::new("");
                    self.status =
                        ui::t!("chat_type_press_enter").into();
                }
            }
            // 表と同じ id(表は py-new / py-list / py-folder で揃っています)
            "py-list" | "plug-manage" => {
                self.plug_open = !self.plug_open;
                if self.plug_open {
                    self.hist_open = false;
                    self.chat_open = false;
                    self.bm_open = false;
                    self.xr_open = false;
                    self.status = ui::tf!("plugins_put_py_files", plugins_dir().display())
                    .into();
                }
            }
            // **暗号化を掛けるボタンは無い**(2026-08-18 発注者「暗号化は、
            // 開くだけ残す」)。writer が保存するのは adoc で、zip では
            // ないので包めない。パスワード付きの docx を開く道は
            // `pw_commit` の側に残っている
            // デジタル署名。**隣の .sig への添え書き**(Ed25519)。
            // Word の署名欄には出ない独自方式 — そう言って出す。
            // 有効なら報告だけ、無効・未署名なら(作り直して)署名する
            "prot-sign" => {
                // **署名の中身は ops に1本**(2026-08-21)。前は同じ 40 行を
                // calc と2つ持っていて、文言だけがずれていました。
                // ここに残すのは訳の要る文言だけです
                let Some(p) = self.path.clone() else {
                    self.status =
                        ui::t!("not_file_yet_save_first").into();
                    return;
                };
                if self.dirty {
                    self.status =
                        ui::t!("there_unsaved_changes_save_before").into();
                    return;
                }
                self.status = match ops::sign_or_verify(&p) {
                    Ok(ops::Signed::Verified(signer)) => {
                        ui::tf!("signature_valid_content_unchanged", signer)
                    }
                    // **アプリの名前は差し込みにしない。** トルコ語の所有格は
                    // 語尾が変わり(Word'ün / Excel'in)、差し込みでは正しく
                    // 書けません。表とは別の文にして、各言語は同じ文の
                    // 製品名だけを差し替えます
                    Ok(ops::Signed::Wrote(name)) => ui::tf!(
                        "signed_written_next_file",
                        name
                    ),
                    Err(ops::SignErr::Read(e)) => ui::tf!("cant_read", e),
                    Err(ops::SignErr::Write(e)) => ui::tf!("cant_write_signature", e),
                    Err(ops::SignErr::Key(e)) => {
                        ui::tf!("cant_sign", key_err_msg(e))
                    }
                }
                .into();
            }
            // クリップボード(リボンから。Ctrl+C/X/V と同じ実体)
            "copy" | "cut" => {
                let e = self.editor_ref();
                let sel = e.selection();
                if sel.is_empty() {
                    self.status = ui::t!("nothing_selected").into();
                } else if let Some(t) = e.text().get(sel).map(str::to_string) {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(t));
                    if id == "cut" {
                        self.editor().insert("");
                        self.on_edited();
                        self.status = ui::t!("cut_selection").into();
                    } else {
                        self.status = ui::t!("copied_2").into();
                    }
                }
            }
            "paste" => match cx.read_from_clipboard().and_then(|i| i.text()) {
                Some(text) if !text.is_empty() => handler::replace(self, None, &text),
                _ => self.status = ui::t!("nothing_paste").into(),
            },
            // 記入欄(コンテンツコントロール)。フォームタブの実体でもある
            "controls" | "form-text" | "form-image" | "form-email" | "form-phone"
            | "form-complex" | "form-signature" | "form-date" => {
                self.checkpoint(false); // 記入欄
                use kumihan::SdtKind as K;
                let kind = match id {
                    "form-image" => K::Picture,
                    "form-email" => K::Email,
                    "form-phone" => K::Phone,
                    "form-complex" => K::Complex,
                    "form-signature" => K::Signature,
                    "form-date" => K::Date,
                    _ => K::Text,
                };
                self.insert_sdt(kind, Vec::new());
            }
            // チェックの欄。**同じボタンで入切**(欄の中にカーソルがあるとき)
            "form-checkbox" | "form-radio" => {
                self.checkpoint(false); // 記入欄(チェック・ラジオ)
                if self.toggle_checkbox() {
                    return;
                }
                self.insert_sdt(kumihan::SdtKind::Checkbox, Vec::new());
            }
            // 選ばせる欄。選択肢をパネルで聞いてから挿す
            "form-combo" | "form-dropdown" => {
                // 既にその欄にいるなら、選択肢を順に回す(選び直し)
                if let Some(sd) = self.sdt_at() {
                    if !sd.items.is_empty() {
                        let text = self.ed.text().to_string();
                        let (pi, _) = self.cursor_para();
                        let _ = pi;
                        // いまの中身の次の選択肢へ
                        if let Some(cur) =
                            sd.items.iter().position(|it| text.contains(it.as_str()))
                        {
                            let now = &sd.items[cur];
                            let next = &sd.items[(cur + 1) % sd.items.len()];
                            if let Some(at) = text.find(now.as_str()) {
                                self.ed.move_to(at, false);
                                self.ed.move_to(at + now.len(), true);
                                self.ed.insert(next);
                                self.on_edited();
                                self.status =
                                    ui::tf!("selected", next).into();
                                return;
                            }
                        }
                    }
                }
                self.sd_kind = if id == "form-combo" {
                    kumihan::SdtKind::Combo
                } else {
                    kumihan::SdtKind::Dropdown
                };
                self.sd_ed = Editor::new("");
                self.sd_open = true;
                self.status =
                    ui::t!("type_choices_comma_separated").into();
            }
            // 記入欄に名前を付ける(docx の w:alias / w:tag)。
            // 名前がフォームの背骨 — マクロは fill(名前, 値) でこの鍵を引く
            "form-name" => {
                self.switch_target(Target::Body);
                let Some(sd) = self.sdt_at() else {
                    self.status =
                        ui::t!("put_cursor_inside_field").into();
                    return;
                };
                // いまの名前をパネルに前置き(種類の既定名のままなら空)
                let now = if sd.tag == sd.kind.as_tag() {
                    String::new()
                } else {
                    sd.tag.clone()
                };
                let mut ed = Editor::new(&now);
                ed.move_to(now.len(), false);
                self.sd_ed = ed;
                self.sd_naming = true;
                self.sd_open = true;
                self.status =
                    ui::t!("type_field_name_press").into();
            }
            // 配色。**その時の値で塗る**(テーマ部品は作らない — Word で
            // 開いても同じ色に見える正直な形)。見出しの色と紙の色を組で当てる
            "colorschemas" => {
                // (名前, 見出しの色, 紙の色)。照合は添字(self.theme)なので
                // 名前は見せる字だけ — 訳してよい(const を外したのはそのため)
                let themes: [(&'static str, &str, Option<&str>); 6] = [
                    (ui::t!("normal"), "1B1B1B", None),
                    (ui::t!("indigo"), "165E83", None),
                    (ui::t!("green"), "1B6E3C", None),
                    (ui::t!("crimson"), "8E3A46", None),
                    (ui::t!("indigo_unbleached_paper"), "165E83", Some("FBF7EE")),
                    (ui::t!("ink_grey_paper"), "2E3338", Some("F2F2F0")),
                ];
                self.flush_target();
                self.checkpoint(false);
                self.theme = (self.theme + 1) % themes.len();
                let (name, head, paper) = themes[self.theme];
                // 見出しの段落の字に色を当てる(段落ごとの範囲で塗る)
                let mut at = 0usize;
                let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
                for p in self.doc.paragraphs() {
                    let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
                    if matches!(p.style, kumihan::ParaStyle::Heading(_)) && len > 0 {
                        ranges.push(at..at + len);
                    }
                    at += len + 1;
                }
                let n = ranges.len();
                for r in ranges {
                    let c = head.to_string();
                    self.doc.apply_char_format(r, move |f| {
                        f.color = (c != "1B1B1B").then(|| c.clone())
                    });
                }
                self.doc.page_color = paper.map(str::to_string);
                self.dirty = true;
                self.relayout_keep();
                self.status = ui::tf!("colour_scheme_applied_headings", name, n)
                .into();
            }
            // ---- AI(モデルに任せる変換と生成の道具箱)----
            // 宛先は人が選ぶ。押すたびに 手元 → Claude Agent → Claude(API)
            // **ふりがなだけ残す。** 会話が入れるのは素の字で、ルビの
            // 書式は付けられない — ここは AI タブを廃したあとも要る仕事
            // (置き場はホームの「ルビ」の隣。2026-08-15)
            // **辞書で振ります**(2026-08-20 発注者「取り敢えずは辞書で
            // いきましょう」)。外に出ず、待ちもありません。
            // 辞書が無い機械では、いままでどおりモデルに頼みます
            "ai-furigana" => {
                if !self.furigana_by_dict() {
                    self.ai_go(AiJob::Furigana, cx);
                }
            }
            // マクロ台本を AI に書かせる。答えは文書に入れず、プラグイン
            // 置き場に .py で置く — 人が読んで確かめてから実行する
            "ai-macro" => {
                if self.ai_open {
                    self.ai_open = false;
                    self.ai_macro = false;
                    return;
                }
                self.ai_ed = Editor::new("");
                self.ai_open = true;
                self.ai_macro = true;
                self.find_open = false;
                self.status = ui::tf!("ask_ai_macro_script", ui::ai::backend().label())
                .into();
            }
            // 表示(本家の表示タブ)。見え方だけを変える — 文書は変わらない
            "nav" => {
                self.nav_open = !self.nav_open;
                self.status = if self.nav_open {
                    ui::t!("navigation_click_heading_jump").into()
                } else {
                    "".into()
                };
            }
            // 印刷レイアウト。**紙を1枚ずつ積んで見せる**(編集は巻物のまま)。
            // 節で紙が変わる文書は、この形でないと紙の大きさの違いが出せない
            "printview" => {
                if self.doc.vertical {
                    self.status =
                        ui::t!("no_print_layout_vertical").into();
                    return;
                }
                self.paged = !self.paged;
                if self.paged {
                    self.multipage = false; // 画面だけの折り方どうし、両立させない
                }
                self.relayout();
                self.status = if self.paged {
                    ui::t!("print_layout_sheets_come").into()
                } else {
                    ui::t!("back_editing_view_one").into()
                };
            }
            "multipage" => {
                if self.doc.vertical {
                    self.status = ui::t!("no_spread_view_vertical").into();
                    return;
                }
                self.multipage = !self.multipage;
                if self.multipage {
                    self.paged = false;
                }
                self.relayout();
                self.status = if self.multipage {
                    ui::t!("spread_view_two_pages").into()
                } else {
                    ui::t!("back_single_page_view").into()
                };
            }
            "fit-page" => self.fit_zoom(false),
            "fit-width" => self.fit_zoom(true),
            "show-toolbar" => {
                self.show_toolbar = !self.show_toolbar;
                self.status = if self.show_toolbar {
                    ui::t!("toolbar_always_shown").into()
                } else {
                    ui::t!("toolbar_collapsed_click_tab").into()
                };
            }
            "show-statusbar" => self.show_statusbar = !self.show_statusbar,
            "show-left" => self.nav_open = !self.nav_open,
            "show-right" => {
                self.rp_open = !self.rp_open;
                self.status = if self.rp_open {
                    ui::t!("right_panel_adjust_settings").into()
                } else {
                    "".into()
                };
            }
            "linespace" => self.para(|p| {
                p.line_spacing = match p.spacing() {
                    s if s < 1.25 => 1.5,
                    s if s < 1.75 => 2.0,
                    _ => 1.0,
                }
            }),
            // 文字カウント。日本語は「単語数」に意味が無いので**文字数**を出す
            "wordcount" => {
                let text = self.ed.text();
                let all = text.chars().filter(|c| *c != '\n').count();
                let ink = text.chars().filter(|c| !c.is_whitespace()).count();
                let paras = text.split('\n').filter(|s| !s.trim().is_empty()).count();
                self.status = ui::tf!("characters_spaces_paragraphs", ink, all, paras).into();
            }
            "fontcolor" => {
                let next = match self.doc.char_format_at(self.ed.selection()).color.as_deref() {
                    None => Some("C00000".to_string()),
                    Some("C00000") => Some("1F4E79".to_string()),
                    _ => None,
                };
                self.toggle(move |f| f.color = next.clone());
            }
            other => {
                // ここに来たら結線漏れ。黙らず画面に出す
                self.status = ui::tf!("unwired_command_bug", other).into();
            }
        }
    }

    // ---- 割り当てられた操作 ----
    /// メニューの項目を実行する。
    pub(crate) fn menu_action(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_at = None;
        match id {
            "cut" => self.cut(&ui::Cut, window, cx),
            "copy" => self.copy(&ui::Copy, window, cx),
            "paste" => self.paste(&ui::Paste, window, cx),
            "selword" => self.select_word(),
            "selline" => self.select_line(),
            "selall" => self.ed.select_all(),
            other => self.run_cmd(other, cx),
        }
        cx.notify();
    }
}

impl Writer {
    /// **ファイルのページに並ぶ物**(統合の段8 の1)。
    ///
    /// 17 個は calc と同じ id・同じ意味で、`URLを開く`・`データを差し込む`・
    /// `adoc 形式にする` が文章の画面だけの物です。
    /// **次の段で officework がこの表を読んでページを描きます。**
    pub fn file_menu(&self) -> Vec<ui::filemenu::Item> {
        use ui::filemenu::Item as I;
        vec![
            I::new("f-back", ui::t!("back")),
            I::new("f-new", ui::t!("new")).gap(),
            I::new("f-tpl", ui::t!("new_template")).grey(),
            I::new("f-open", ui::t!("open")),
            // **フォルダを開き直す**(2026-08-25 発注者「どうしてフォルダーを
            // 開くがないのだ」)。綴りはフォルダなので、仕事を替えるとは
            // フォルダを替えることです。前は起動のときにしか選べませんでした
            I::new("f-folder", ui::t!("open_folder")),
            I::new("f-url", ui::t!("open_url")),
            I::new("f-recent", ui::t!("recent")).on(self.file_view == 1),
            I::new("f-find", ui::t!("search_folder")).on(self.file_view == 3),
            // **前に落ちた跡から開き直す**(2026-08-21 の B-3)。控えが
            // 無いときは灰色にします — 押しても何も無い、をやめるためです
            {
                let i = I::new("f-recover", ui::t!("recover")).on(self.file_view == 4);
                if ops::stale_recovers("adoc").is_empty() { i.grey() } else { i }
            },
            I::new("f-save", ui::t!("save")).gap(),
            I::new("f-saveas", ui::t!("save_2")),
            I::new("f-print", ui::t!("print")),
            // **形を選んで書き出す1つの入り口**
            // (手引き `docs/ja/commands/ファイル/エクスポート.adoc`)。
            // 前は「印刷」「Web の形で書き出す」に分かれていて、
            // どこから何が出せるのかが探しにくい形でした
            I::new("f-export", ui::t!("export")),
            I::new("f-merge", ui::t!("merge_data_csv")),
            I::new("f-html", ui::t!("export_web_html")),
            I::new("f-protect", ui::t!("protect")),
            // **非可逆なので明示の1手**(開いただけでは何も起きない)。
            // もう adoc なら押せない
            {
                let i = I::new("f-distill", ui::t!("convert_adoc_split_text"));
                if self.native { i.grey() } else { i }
            },
            // **書式の標準**(2026-08-26 発注者「スタイルを設定変更できる
            // 画面が必要だろう」)。3段のどれが効いているかを見て直します
            I::new("f-style", ui::t!("formatting_defaults")).on(self.file_view == 5),
            I::new("f-info", ui::t!("info")).gap().on(self.file_view == 0),
            I::new("f-place", ui::t!("open_file_location")),
            I::new("f-quit", ui::t!("quit")).gap(),
            I::new("f-opts", ui::t!("advanced_settings")).tail().on(self.file_view == 2),
            I::new("f-help", ui::t!("help")).grey().tail(),
            I::new("f-req", ui::t!("feature_request")).grey().tail(),
        ]
    }

    /// **ファイルのページの項目を捌く**(統合の段8 の1)。
    ///
    /// 前は画面の中のその場の閉包でした。1つの `match` に集めたので、
    /// **officework から id を渡して呼べます**(次の段)。
    pub fn file_menu_click(&mut self, id: &str, cx: &mut Context<Self>) {
        // **共通の腕は ui::filemenu が先に取ります**(段8 の3)。
        // 同じ id の腕をここに残すと向こうが先に取るので、残した腕は死にます
        if ui::filemenu::run(self, id) || ui::filemenu::run_cx(self, id, cx) {
            cx.notify();
            return;
        }
        match id {
            "f-url" => {
                self.tab = self.prev_tab;
                self.url_open = true;
                self.url_ed = Editor::new("http://127.0.0.1:8765/");
                self.status =
                    ui::t!("type_url_press_enter").into();
            }
            "f-style" => self.file_view = 5,
            "f-print" => self.save_pdf(cx),
            "f-export" => {
                self.tab = self.prev_tab;
                self.open_list = (self.open_list != Some("f-export")).then_some("f-export");
                self.pick_sel = 0;
            }
            "f-merge" => self.merge_csv(cx),
            "f-html" => self.save_html(cx),
            "f-distill" => {
                self.tab = self.prev_tab;
                self.distill_now();
            }
            _ => {}
        }
    }
}

/// 今日の日付を、形ごとの字にする。**(鍵, 出す字)** の並び。
///
/// 事務の様式は和暦で書くものが多いので、西暦と和暦の両方を出します
/// (2026-08-25 発注者「形式の一覧は必要」)。
/// **自動更新はしません** — 入るのは固定の字です。印刷した日に文書の
/// 日付が変わるのは事故の元で、様式の世界では固定が正です。
///
/// 元号は令和・平成・昭和まで。それより前は西暦のまま出します
/// (様式で使う範囲を超えるので、無理に足しません)。
pub(crate) fn 日付の形() -> Vec<(String, String)> {
    let out = std::process::Command::new("date").arg("+%Y %m %d").output();
    let Ok(o) = out else { return Vec::new() };
    if !o.status.success() {
        return Vec::new();
    }
    let t = String::from_utf8_lossy(&o.stdout);
    let mut it = t.split_whitespace().filter_map(|x| x.parse::<i32>().ok());
    let (Some(y), Some(m), Some(d)) = (it.next(), it.next(), it.next()) else {
        return Vec::new();
    };
    let 和 = 和暦(y, m, d);
    let mut v = vec![
        format!("{y}年{m}月{d}日"),
        format!("{y}/{m:02}/{d:02}"),
        format!("{y}-{m:02}-{d:02}"),
    ];
    if let Some((元号, 略, 年)) = 和 {
        v.push(format!("{元号}{年}年{m}月{d}日"));
        v.push(format!("{略}{年}.{m}.{d}"));
    }
    v.into_iter().map(|x| (x.clone(), x)).collect()
}

/// 西暦から元号と年を出す。(元号, 略号, 年)。範囲の外なら None
pub(crate) fn 和暦(y: i32, m: i32, d: i32) -> Option<(&'static str, &'static str, i32)> {
    // (始まりの年, 月, 日, 元号, 略号)
    const 代: &[(i32, i32, i32, &str, &str)] = &[
        (2019, 5, 1, "令和", "R"),
        (1989, 1, 8, "平成", "H"),
        (1926, 12, 25, "昭和", "S"),
    ];
    for (yy, mm, dd, 元号, 略) in 代 {
        if (y, m, d) >= (*yy, *mm, *dd) {
            let n = y - yy + 1;
            // 元年は「1年」ではなく「元年」と書きますが、様式では
            // 数で書くものも多いので数のまま出します
            return Some((元号, 略, n));
        }
    }
    None
}
