//! writer の画面のパネル(検索・しおり・履歴・ルビ・校正…)。
//! view.rs の render からの**純移動**(2026-08-12 部屋割りの5歩目 —
//! calc の picks.rs に当たる部屋)。挙動と文言は一切変えない。
//! render が使う色(テーマ)だけを引数で受け取り、評価の順も元のまま。

use crate::*;

/// 外側の柱の幅(px)。アイコン1つぶん(calc の RAIL と揃える)
const RAIL: f32 = 34.0;

/// render に返すパネルの束。欄の名前は view.rs にあった let の名前そのもの
pub(crate) struct Panels {
    pub find_panel: Option<gpui::Div>,
    pub hf_panel: Option<gpui::Div>,
    pub cmt_panel: Option<gpui::Div>,
    pub wm_panel: Option<gpui::Div>,
    pub bm_panel: Option<gpui::Div>,
    /// スタイルの新設(ネイティブ文書だけ)
    pub style_new_panel: Option<gpui::Div>,
    pub hist_panel: Option<gpui::Div>,
    pub chat_panel: Option<gpui::Div>,
    pub pw_panel: Option<gpui::Div>,
    pub url_panel: Option<gpui::Div>,
    pub fm_panel: Option<gpui::Div>,
    pub nav_panel: Option<gpui::Div>,
    /// 右パネル(柱つき。中身の側が巻ける)
    pub rp_panel: Option<gpui::Div>,
    pub lk_panel: Option<gpui::Div>,
    pub ai_panel: Option<gpui::Div>,
    pub sd_panel: Option<gpui::Div>,
    pub rb_panel: Option<gpui::Div>,
    pub eq_panel: Option<gpui::Div>,
    pub plug_panel: Option<gpui::Div>,
    pub xr_panel: Option<gpui::Div>,
    /// 一覧は4つとも `ui::picklist` が描きます(窓の根へ置きます。
    /// 記号はマス目の並び)
    pub font_panel: Option<gpui::Stateful<gpui::Div>>,
    pub size_panel: Option<gpui::Stateful<gpui::Div>>,
    pub style_panel: Option<gpui::Stateful<gpui::Div>>,
    pub symbol_panel: Option<gpui::Stateful<gpui::Div>>,
    /// 表の大きさを打つ欄(2026-08-25)
    pub tbl_panel: Option<gpui::Div>,
    /// 一覧の仕事の欄(2026-08-26)
    pub fl_panel: Option<gpui::Div>,
    /// この機械の標準の書体を選ぶ一覧(2026-08-26)
    pub user_font_panel: Option<gpui::Stateful<gpui::Div>>,
    /// 日付の形の一覧(2026-08-25)
    pub date_panel: Option<gpui::Stateful<gpui::Div>>,
    /// 書き出す形の一覧(2026-08-25)
    pub export_panel: Option<gpui::Stateful<gpui::Div>>,
    pub proof_panel: Option<gpui::Div>,
}

impl Writer {
    /// パネルに焦点があり、木への打鍵を受けるか。編集の欄が開いている
    /// 間は受けない(矢印はそちらの物)
    pub(crate) fn fl_takes_keys(&self) -> bool {
        self.fl_focus
            && self.rp_open
            && self.rp_tab == 3
            && !self.chat_open
            && !self.ai_chat_focus
            && self.fl_job.is_none()
    }

    /// 木で選ばれている物を開く(Enter)。フォルダなら開閉
    pub(crate) fn fl_open_selected(&mut self) {
        let Some(p) = self.fl_tree.selected.clone() else { return };
        if p.is_dir() {
            self.fl_tree.toggle(&p);
            return;
        }
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        let kind = ui::folder::kind_of(&name);
        self.remember_folder();
        if !kind.can_open() {
            self.status = match ui::open_outside(&p.display().to_string()) {
                ui::Opened::Yes => ui::tf!("opening_application_system_chose", name).into(),
                ui::Opened::JustNow => ui::t!("just_opened").into(),
                ui::Opened::Failed => {
                    ui::tf!("no_application_associated_file", p.display().to_string()).into()
                }
            };
            return;
        }
        if self.embedded || kind.is_sheet() {
            self.open_request = Some(p);
        } else {
            self.open_in_tab(p);
        }
    }

    /// パネルを全部組む(順番は view.rs にあった時のまま)。
    /// 色は render のテーマの束 — パネルが使う6つだけを受け取る
    #[allow(clippy::too_many_arguments)] // 色は6つとも別物。束ねると呼ぶ側から見えない
    pub(crate) fn panels(
        &mut self,
        dk: bool,
        th_btn: gpui::Rgba,
        th_btn_hover: gpui::Rgba,
        th_cmd_border: gpui::Rgba,
        th_status: gpui::Rgba,
        th_top_fg: gpui::Rgba,
        cx: &mut Context<Self>,
    ) -> Panels {
        let us = self.ui_scale;
        // 置換のパネル
        let find_panel = if !self.find_open {
            None
        } else {
            let field = |label: &str, ed: &Editor, active: bool| {
                // caret は | で見せる(専用の入力部品を作らない割り切り)
                let mut s = ed.text().to_string();
                let cur = ed.cursor().min(s.len());
                if active {
                    s.insert(cur, '|');
                }
                div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 64.0)).text_size(px(us * 11.5))
                        .text_color(rgb(0x66707A)).child(SharedString::from(label.to_string())))
                    .child(div().flex_1().px_2().py_1().rounded_sm()
                        .border_1()
                        .border_color(if active { rgb(0x1B6E3C) } else { rgb(0xC6CDD3) })
                        .bg(gpui::white())
                        .text_size(px(us * 12.5))
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(s)))
            };
            let btn = |id: &str, label: &str| {
                div().id(SharedString::from(id.to_string()))
                    .px_2p5().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                    .text_size(px(us * 11.5)).cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF5EE)))
                    .child(SharedString::from(label.to_string()))
            };
            // 範囲の選び。**選んでいる物は塗る**(押せる物と見分けが付くように)
            let range_button = |id: &'static str, label: &str, on: bool| {
                div().id(id)
                    .px_2p5().py_0p5().rounded_sm()
                    .border_1()
                    .border_color(if on { rgb(0x1B6E3C) } else { rgb(0xC6CDD3) })
                    .bg(if on { rgb(0xCFE6D8) } else { rgb(0xFFFFFF) })
                    .text_color(if on { rgb(0x1B6E3C) } else { rgb(0x66707A) })
                    .text_size(px(us * 11.0)).cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF5EE)))
                    .child(SharedString::from(label.to_string()))
            };
            Some(div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 430.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(field(ui::t!("find"), &self.find_ed, self.find_field == 0)
                    .id("find-f").cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| { this.find_field = 0; cx.notify() })))
                .child(field(ui::t!("replace_2"), &self.repl_ed, self.find_field == 1)
                    .id("find-r").cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| { this.find_field = 1; cx.notify() })))
                // **探す範囲**(2026-08-20 発注者「検索には3種類必要です」)。
                // この文書 / このファイル / フォルダ。**「シート」とは呼ばない** —
                // 文章の画面の言葉は「文書」です
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().text_size(px(us * 11.0)).text_color(rgb(0x66707A))
                        .child(ui::t!("search")))
                    .child(range_button("sc-doc", ui::t!("document_5"), !self.find_file)
                        .on_click(cx.listener(|t, _, _, cx| { t.find_file = false; cx.notify() })))
                    .child(range_button("sc-file", ui::t!("file_2"), self.find_file)
                        .on_click(cx.listener(|t, _, _, cx| { t.find_file = true; cx.notify() })))
                    .child(range_button("sc-dir", ui::t!("folder_3"), false)
                        .on_click(cx.listener(|t, _, _, cx| {
                            // フォルダ全体はファイルのページの「フォルダから探す」
                            t.tab = 0;
                            t.file_view = 2;
                            cx.notify()
                        }))))
                .child(div().flex().flex_row().gap_2()
                    .child(btn("f-next", ui::t!("next_enter"))
                        .on_click(cx.listener(|this, _, _, cx| { this.find_next(); cx.notify() })))
                    .child(btn("f-one", ui::t!("replace"))
                        .on_click(cx.listener(|this, _, _, cx| { this.replace_current(); cx.notify() })))
                    .child(btn("f-all", ui::t!("replace_all"))
                        .on_click(cx.listener(|this, _, _, cx| { this.replace_all(); cx.notify() })))
                    .child(div().flex_1())
                    .child(btn("f-close", ui::t!("close"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.find_open = false; cx.notify()
                        })))))
        };

        // ヘッダー・フッターの編集のパネル。開いている間、打鍵はここに入る
        let hf_panel = self.hf_edit.map(|footer| {
            let title = if footer { ui::t!("footer") } else { ui::t!("header") };
            // キャレットは | で見せる(検索のパネルと同じ割り切り)。
            // ページ番号の印は読める形で見せる
            let mut s = self.hf_ed.text().to_string();
            let cur = self.hf_ed.cursor().min(s.len());
            s.insert(cur, '|');
            let shown = s
                .replace(kumihan::PAGE_MARK, "《ページ番号》")
                .replace(kumihan::PAGES_MARK, "《ページ数》");
            let btn = |id: &str, label: &str| {
                div().id(SharedString::from(id.to_string()))
                    .px_2p5().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                    .text_size(px(us * 11.5)).cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF5EE)))
                    .child(SharedString::from(label.to_string()))
            };
            let mut field = div().flex_1().px_2().py_1().rounded_sm()
                .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                .text_size(px(us * 12.5)).flex().flex_col();
            for ln in shown.split('\n') {
                field = field.child(div().whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(ln.to_string())));
            }
            div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 430.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(SharedString::from(ui::tf!("editing_shared_all_pages_2", title))))
                .child(field)
                .child(div().flex().flex_row().gap_2()
                    .child(btn("hf-num", ui::t!("insert_page_number"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("pagenum", cx);
                            cx.notify()
                        })))
                    .child(div().flex_1())
                    .child(btn("hf-close", ui::t!("close_esc"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.hf_edit = None;
                            this.status = "".into();
                            cx.notify()
                        }))))
        });

        // コメントのパネルと、カーソルの段落のコメントの一覧
        let cmt_panel = if !self.cmt_edit {
            // パネルが閉じていても、カーソルの段落にコメントがあれば見せる
            let cur = self.ed.cursor();
            let mut at = 0usize;
            let mut found: Option<Vec<(String, String)>> = None;
            if self.show_comments && self.target == Target::Body {
                for p in self.doc.paragraphs() {
                    let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
                    if at <= cur && cur <= at + len && !p.comments.is_empty() {
                        found = Some(p.comments.iter()
                            .map(|c| (c.author.clone(), c.text.clone()))
                            .collect());
                        break;
                    }
                    at += len + 1;
                }
            }
            found.map(|cs| {
                let mut d = div().absolute().left(px(us * 16.0)).bottom(px(us * 16.0)).w(px(us * 300.0))
                    .p_3().rounded_md().bg(rgb(0xFFF6E6))
                    .border_1().border_color(rgb(0xE8D5A8))
                    .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0x8A4B00))
                        .child(ui::t!("comments_paragraph_edit_via")));
                for (author, text) in cs {
                    d = d.child(div().mt_1p5().text_size(px(us * 11.5)).text_color(rgb(0x5A4A28))
                        .child(SharedString::from(format!("{author}: {text}"))));
                }
                d
            })
        } else {
            // 編集のパネル(検索のパネルと同じ作法。| がキャレット)
            let mut t = self.cmt_ed.text().to_string();
            let cur = self.cmt_ed.cursor().min(t.len());
            t.insert(cur, '|');
            let mut field = div().flex_1().px_2().py_1().rounded_sm()
                .border_1().border_color(rgb(0xE08A00)).bg(gpui::white())
                .text_size(px(us * 12.5)).flex().flex_col();
            for ln in t.split('\n') {
                field = field.child(div().whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(ln.to_string())));
            }
            Some(div().absolute().left(px(us * 16.0)).bottom(px(us * 16.0)).w(px(us * 360.0))
                .p_3().rounded_md().bg(rgb(0xFFF6E6))
                .border_1().border_color(rgb(0xE8D5A8))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x8A4B00))
                    .child(ui::t!("comment_close_empty_remove")))
                .child(field)
                .child(div().flex().flex_row()
                    .child(div().flex_1())
                    .child(div().id("cmt-close").px_2p5().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x8A4B00)).text_color(rgb(0x8A4B00))
                        .text_size(px(us * 11.5)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xF7ECD8)))
                        .child(ui::t!("close_esc"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cmt_edit = false;
                            this.status = "".into();
                            cx.notify()
                        })))))
        };

        // 透かしのパネル
        let wm_panel = if !self.wm_edit {
            None
        } else {
            let mut t = self.wm_ed.text().to_string();
            let cur = self.wm_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 360.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("watermark_close_empty_remove")))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x165E83)).bg(gpui::white())
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t)))
                .child(div().flex().flex_row()
                    .child(div().flex_1())
                    .child(div().id("wm-close").px_2p5().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x165E83)).text_color(rgb(0x165E83))
                        .text_size(px(us * 11.5)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF2F7)))
                        .child(ui::t!("close_esc"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.wm_edit = false;
                            this.status = "".into();
                            cx.notify()
                        })))))
        };

        // しおりのパネル(名前の入力欄+一覧)
        // **スタイルの新設**(2026-08-16)。ネイティブ文書で見た目を直に
        // 変えようとしたときに出る。名前を付けるとテンプレートに入り、
        // 同じスタイルの所が一度に変わる — 直接書式より楽な道にする
        let style_new_panel = self.style_new.as_ref().map(|d| {
            let mut t = self.style_ed.text().to_string();
            let cur = self.style_ed.cursor().min(t.len());
            t.insert(cur, '|');
            let renaming = !d.name.is_empty();
            // 何を掛けるのかを人の言葉で1行に
            let mut what: Vec<String> = Vec::new();
            if let Some(s) = d.size_pt {
                what.push(ui::tf!("size_pt_2", s.to_string()).to_string());
            }
            if let Some(f) = &d.font {
                what.push(ui::tf!("typeface", f.clone()).to_string());
            }
            if d.bold {
                what.push(ui::t!("bold").to_string());
            }
            if d.italic {
                what.push(ui::t!("italic").to_string());
            }
            if d.underline {
                what.push(ui::t!("underline").to_string());
            }
            if let Some(c) = &d.color {
                what.push(ui::tf!("colour", c.clone()).to_string());
            }
            if let Some(c) = &d.shade {
                what.push(ui::tf!("shading_2", c.clone()).to_string());
            }
            div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 400.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    // 名前があれば「名前を変える」の途中。無ければ新設
                    .child(SharedString::from(if renaming {
                        ui::tf!("rename_style_give_new_name", d.name.clone()).to_string()
                    } else {
                        ui::t!("new_style_give_look").to_string()
                    })))
                .child(div().text_size(px(us * 11.0)).text_color(rgb(0x66707A))
                    .child(SharedString::from(if renaming {
                        String::new()
                    } else {
                        ui::tf!("what_applies", what.join("・")).to_string()
                    })))
                .child(div().flex().flex_row().gap_2().items_center()
                    .child(div().flex_1().px_2().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                        .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(t)))
                    .child(div().id("style-ok").px_2p5().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                        .text_size(px(us * 11.5)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF5EE)))
                        .child(ui::t!("confirm_enter"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.style_commit();
                            cx.notify()
                        }))))
                .child(div().text_size(px(us * 11.0)).text_color(rgb(0x66707A))
                    .child(if renaming { "" } else { ui::t!("existing_name_replaced_goes") }))
        });

        let bm_panel = if !self.bm_open {
            None
        } else {
            let mut t = self.bm_ed.text().to_string();
            let cur = self.bm_ed.cursor().min(t.len());
            t.insert(cur, '|');
            // 一覧(名前と、その段落の頭のバイト位置)
            let mut items: Vec<(String, usize)> = Vec::new();
            let mut at = 0usize;
            for p in self.doc.paragraphs() {
                let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
                for b in &p.bookmarks {
                    items.push((b.clone(), at));
                }
                at += len + 1;
            }
            let mut d = div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 340.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("bookmarks_type_name_add_click")))
                .child(div().flex().flex_row().gap_2().items_center()
                    .child(div().flex_1().px_2().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                        .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(t)))
                    .child(div().id("bm-add").px_2p5().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                        .text_size(px(us * 11.5)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF5EE)))
                        .child(ui::t!("add_enter"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.bm_add();
                            cx.notify()
                        }))));
            if items.is_empty() {
                d = d.child(div().text_size(px(us * 11.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("no_bookmarks_yet")));
            }
            for (i, (name, b0)) in items.into_iter().enumerate() {
                let name2 = name.clone();
                d = d.child(div().flex().flex_row().items_center().gap_2()
                    .child(div()
                        .id(SharedString::from(format!("bm-{i}")))
                        .flex_1().px_2().py_0p5().rounded_sm()
                        .text_size(px(us * 12.5)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF2F7)))
                        .child(SharedString::from(name.clone()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.switch_target(Target::Body);
                            this.ed.move_to(b0, false);
                            this.follow_caret();
                            this.status = ui::tf!("jumped_bookmark", name).into();
                            cx.notify()
                        })))
                    .child(div()
                        .id(SharedString::from(format!("bmx-{i}")))
                        .px_1p5().py_0p5().rounded_sm()
                        .text_size(px(us * 11.5)).text_color(rgb(0x9AA5AE)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xF6E5E2)).text_color(rgb(0xC0392B)))
                        .child("✕")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            for b in &mut this.doc.blocks {
                                if let kumihan::Block::Para(p) = b {
                                    p.bookmarks.retain(|x| *x != name2);
                                }
                            }
                            this.dirty = true;
                            this.status = ui::t!("bookmark_removed").into();
                            cx.notify()
                        }))));
            }
            Some(d)
        };

        // バージョン履歴のパネル(控えの一覧。押すと名無しの複製で開く)
        let hist_panel = if !self.hist_open {
            None
        } else {
            let items = self.versions();
            let mut d = div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 360.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("version_history_copy_per")));
            if items.is_empty() {
                d = d.child(div().text_size(px(us * 11.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("no_copies_yet_overwrite")));
            }
            for (i, (disp, q)) in items.into_iter().enumerate() {
                d = d.child(div()
                    .id(SharedString::from(format!("hist-{i}")))
                    .px_2().py_0p5().rounded_sm()
                    .text_size(px(us * 12.5)).cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF2F7)))
                    .child(SharedString::from(disp))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_version(&q);
                        cx.notify()
                    })));
            }
            Some(d)
        };

        // チャットのパネル(申し送り帳の最近の行+入力欄)
        let chat_panel = if !self.chat_open {
            None
        } else {
            let mut t = self.chat_ed.text().to_string();
            let cur = self.chat_ed.cursor().min(t.len());
            t.insert(cur, '|');
            let lines = self.chat_lines();
            let mut d = div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 420.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("chat_message_file_next")));
            if lines.is_empty() {
                d = d.child(div().text_size(px(us * 11.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("no_messages_yet")));
            }
            for l in lines {
                d = d.child(div().text_size(px(us * 12.0))
                    .whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(l)));
            }
            d = d.child(div().flex().flex_row().gap_2().items_center()
                .child(div().flex_1().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t)))
                .child(div().id("chat-send").px_2p5().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                    .text_size(px(us * 11.5)).cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF5EE)))
                    .child(ui::t!("send_enter"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.chat_send();
                        cx.notify()
                    }))));
            Some(d)
        };

        // パスワードのパネル(伏せ字)。**開くときだけ出ます** —
        // 暗号化を掛けるボタンは 2026-08-18 に外しました
        let pw_panel = if !self.pw_open {
            None
        } else {
            let text = self.pw_ed.text();
            let before = text[..self.pw_ed.cursor().min(text.len())].chars().count();
            let total = text.chars().count();
            let masked = format!(
                "{}|{}",
                "●".repeat(before),
                "●".repeat(total - before)
            );
            let title = ui::t!("password_document_encrypted");
            Some(div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 380.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(SharedString::from(title.to_string())))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(masked)))
                .child(div().text_size(px(us * 10.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("scheme_ecma_376_agile"))))
        };

        // URL のパネル(JS なしの閲覧の入口)
        let url_panel = if !self.url_open {
            None
        } else {
            let mut t = self.url_ed.text().to_string();
            let cur = self.url_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 460.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("open_url_enter_fetches")))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t))))
        };

        // 記入のパネル(HTML の form。欄を押して打ち、送信で送る)
        let fm_panel = if !self.fm_open || self.html_forms.is_empty() {
            None
        } else {
            let fm = self.html_forms[0].clone();
            let mut d = div().absolute().right(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 340.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("fill_click_field_type")));
            for (i, f) in fm.fields.iter().enumerate() {
                if f.hidden {
                    continue;
                }
                let editing = self.fm_field == Some(i);
                let shown = if editing {
                    let mut t = self.fm_ed.text().to_string();
                    let cur = self.fm_ed.cursor().min(t.len());
                    t.insert(cur, '|');
                    t
                } else {
                    f.value.clone()
                };
                let hint = if f.options.is_empty() {
                    String::new()
                } else {
                    format!("({})", f.options.join(" / "))
                };
                let val = f.value.clone();
                d = d.child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 90.0)).text_size(px(us * 11.5))
                        .text_color(rgb(0x66707A))
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(format!("{}{hint}", f.name))))
                    .child(div()
                        .id(SharedString::from(format!("fm-{i}")))
                        .flex_1().px_2().py_0p5().rounded_sm()
                        .border_1()
                        .border_color(if editing { rgb(0x1B6E3C) } else { rgb(0xC6CDD3) })
                        .bg(gpui::white())
                        .text_size(px(us * 12.5)).cursor_pointer()
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(shown))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.fm_ed = Editor::new(&val);
                            this.fm_field = Some(i);
                            cx.notify()
                        }))));
            }
            d = d.child(div().flex().flex_row().gap_2()
                .child(div().id("fm-send").px_3().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                    .text_size(px(us * 12.0)).cursor_pointer()
                    .hover(|st| st.bg(rgb(0xEAF5EE)))
                    .child(ui::tf!("submit", fm.method.to_uppercase(), fm.action))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.fm_submit(cx);
                        cx.notify()
                    }))));
            Some(d)
        };

        // **外側の柱**(発注者 2026-08-15。calc と同じ作法)。
        // 面を切り替えるアイコンを縦に並べる
        let rail = || div().flex_none().w(px(RAIL)).h_full()
            .flex().flex_col().items_center().gap_1().py_1();
        // **柱の釦も場所を控える**(2026-08-16。点検の道具のため)。
        // 鍵は `&'static str` が要るので、呼ぶ側が静的な名前も渡す
        let boxes = self.btn_box.clone();
        let rail_button = move |id: String, icon: &'static str, label_text: String, on: bool| {
            let rec = boxes.clone();
            let key: &'static str = match id.as_str() {
                "rf-here" => "@rf-here",
                "rf-page" => "@rf-page",
                "rf-style" => "@rf-style",
                "rf-files" => "@rf-files",
                "nf-head" => "@nf-head",
                "nf-cmt" => "@nf-cmt",
                "nf-find" => "@nf-find",
                "nf-ai" => "@nf-ai",
                _ => "@rail",
            };
            div()
                .id(SharedString::from(id))
                .relative()
                .child(gpui::canvas(
                    move |b: gpui::Bounds<gpui::Pixels>, _, _| {
                        rec.borrow_mut().insert(key, (
                            f32::from(b.origin.x),
                            f32::from(b.origin.y),
                            f32::from(b.size.width),
                            f32::from(b.size.height),
                        ));
                    },
                    |_, _: (), _, _| {},
                ).absolute().size_full())
                .w(px(RAIL - 8.0)).h(px(RAIL - 8.0))
                .rounded_sm().cursor_pointer()
                .flex().items_center().justify_center()
                .bg(if on { th_btn_hover } else { gpui::transparent_black().into() })
                .border_1()
                .border_color(if on { th_btn } else { gpui::transparent_black().into() })
                .hover(move |s| s.bg(th_btn_hover))
                .tooltip(move |_, cx| cx.new(|_| crate::view::Tip(label_text.clone().into(), us)).into())
                .tooltip_show_delay(std::time::Duration::from_millis(150))
                .child(gpui::svg()
                    .path(SharedString::from(format!("icons/{icon}.svg")))
                    .size(px(us * 18.0))
                    .text_color(if on { th_btn } else { th_status }))
        };

        // 左パネル(本家のナビゲーション)。見出し / コメント / 検索 / AI
        let nav_panel = if !self.nav_open {
            None
        } else {
            let panel_bg = if dk { rgb(0x1B1E21) } else { rgb(0xF1F3F5) };
            let mut d = div()
                .flex_1().min_w(px(us * 0.0)).h_full().overflow_hidden()
                .p_2()
                .flex().flex_col().gap_1();
            // **面の切り替えは外側の柱へ移した**(発注者 2026-08-15
            // 「左右のパネルの外側にアイコンをおいて操作を変更できるように」)。
            // 前は上に文字のタブが4つ並んでいて、その分だけ中身が狭かった。
            // 左は**対話する相手**(2026-08-14 の決め)— 見出し・コメント・
            // 検索・AI。照合は添字(nav_tab)
            match self.nav_tab {
                // 見出し(押すと飛ぶ)
                0 => {
                    let heads = self.headings();
                    if heads.is_empty() {
                        d = d.child(div().text_size(px(us * 11.0)).text_color(th_status)
                            .child(ui::t!("no_headings_set_them")));
                    }
                    for (i, (lv, text, byte)) in heads.into_iter().take(40).enumerate() {
                        let b = byte;
                        d = d.child(div()
                            .id(SharedString::from(format!("nav-{i}")))
                            .px_2().py_0p5().rounded_sm().cursor_pointer()
                            .text_size(px(us * 12.0)).text_color(th_top_fg)
                            .whitespace_nowrap().overflow_hidden()
                            .hover(|st| st.bg(th_btn_hover))
                            .child(SharedString::from(format!(
                                "{}{text}",
                                "　".repeat((lv as usize).saturating_sub(1))
                            )))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.switch_target(Target::Body);
                                this.ed.move_to(b.min(this.ed.text().len()), false);
                                this.follow_caret();
                                cx.notify()
                            })));
                    }
                }
                // コメント(押すとその段落へ飛ぶ)
                1 => {
                    let mut items: Vec<(usize, String, String, usize)> = Vec::new();
                    let mut at = 0usize;
                    for (pi, p) in self.doc.paragraphs().enumerate() {
                        let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
                        for c in &p.comments {
                            items.push((pi, c.author.clone(), c.text.clone(), at));
                        }
                        at += len + 1;
                    }
                    if items.is_empty() {
                        d = d.child(div().text_size(px(us * 11.0)).text_color(th_status)
                            .child(ui::t!("no_comments")));
                    }
                    for (i, (_, who, text, byte)) in items.into_iter().take(30).enumerate() {
                        let b = byte;
                        d = d.child(div()
                            .id(SharedString::from(format!("navc-{i}")))
                            .px_2().py_1().rounded_sm().cursor_pointer()
                            .bg(if dk { rgb(0x22262A) } else { rgb(0xFFFFFF) })
                            .hover(|st| st.bg(th_btn_hover))
                            .flex().flex_col()
                            .child(div().text_size(px(us * 10.5)).text_color(th_status)
                                .child(SharedString::from(who)))
                            .child(div().text_size(px(us * 11.5)).text_color(th_top_fg)
                                .whitespace_nowrap().overflow_hidden()
                                .child(SharedString::from(text)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.switch_target(Target::Body);
                                this.ed.move_to(b.min(this.ed.text().len()), false);
                                this.follow_caret();
                                cx.notify()
                            })));
                    }
                }
                // 検索(当たった場所を並べ、押すと飛ぶ)
                2 => {
                    let term = self.find_ed.text().to_string();
                    d = d.child(div()
                        .id("nav-find")
                        .px_2().py_1().rounded_sm().cursor_pointer()
                        .border_1().border_color(rgb(0x1B6E3C))
                        .bg(if dk { rgb(0x22262A) } else { rgb(0xFFFFFF) })
                        .text_size(px(us * 12.0)).text_color(th_top_fg)
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(if term.is_empty() {
                            ui::t!("type_term_search_dialog").to_string()
                        } else {
                            term.clone()
                        }))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("replace", cx);
                            cx.notify()
                        })));
                    if !term.is_empty() {
                        let text = self.ed.text().to_string();
                        let mut hits = 0usize;
                        for (i, at) in text.match_indices(&term).take(30).enumerate() {
                            hits += 1;
                            let b = at.0;
                            let s0 = text[..b].rfind('\n').map(|x| x + 1).unwrap_or(0);
                            let e0 = text[b..].find('\n').map(|x| b + x).unwrap_or(text.len());
                            let line: String =
                                text[s0..e0].chars().take(40).collect();
                            d = d.child(div()
                                .id(SharedString::from(format!("navf-{i}")))
                                .px_2().py_0p5().rounded_sm().cursor_pointer()
                                .text_size(px(us * 11.5)).text_color(th_top_fg)
                                .whitespace_nowrap().overflow_hidden()
                                .hover(|st| st.bg(th_btn_hover))
                                .child(SharedString::from(line))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.switch_target(Target::Body);
                                    this.ed.move_to(b, false);
                                    this.follow_caret();
                                    cx.notify()
                                })));
                        }
                        if hits == 0 {
                            d = d.child(div().text_size(px(us * 11.0)).text_color(th_status)
                                .child(ui::t!("not_found_2")));
                        }
                    }
                }
                // ── AI と相談する ─────────────────────────────────
                // **答えは文書に入れない。** 直した文は下の欄に置き、
                // 「入れる」を押して初めて文書が変わる。writer には
                // calc のような Python の橋が無いので、入るのは**文そのもの**
                _ => {
                    let accent = rgb(0x165E83);
                    let button = move |id: &'static str, label: String, enabled: bool| {
                        div().id(SharedString::from(id))
                            .px_2().py_0p5().rounded_sm().cursor_pointer()
                            .text_size(px(us * 11.5))
                            .text_color(if enabled { th_top_fg } else { th_status })
                            .border_1()
                            .border_color(if enabled { accent } else { th_cmd_border })
                            .hover(move |st| st.bg(th_btn_hover))
                            .child(SharedString::from(label))
                    };
                    d = d.child(div().text_size(px(us * 10.5)).text_color(th_status).child(
                        ui::t!("can_ask_about_selection").to_string()));
                    let mut chat = div().id("ai-chat-log").flex().flex_col().gap_1().mt_1()
                        .flex_1().min_h(px(us * 0.0)).overflow_y_scroll();
                    if self.ai_chat_log.is_empty() {
                        chat = chat.child(div().text_size(px(us * 11.0)).text_color(th_status)
                            .child(ui::t!("e_g_make_paragraph").to_string()));
                    }
                    for (self_of, text) in &self.ai_chat_log {
                        chat = chat.child(div().text_size(px(us * 11.5))
                            .text_color(if *self_of { accent } else { th_top_fg })
                            .child(format!("{} {}", if *self_of { "▸" } else { "◂" }, text)));
                    }
                    d = d.child(chat);
                    if let Some(plan) = self.ai_chat_plan.clone() {
                        d = d.child(div().text_size(px(us * 10.5)).text_color(th_status).mt_1()
                            .child(ui::t!("text_insert_nothing_goes").to_string()));
                        d = d.child(div().id("ai-chat-plan")
                            .max_h(px(us * 160.0)).overflow_y_scroll()
                            .p_1().rounded_sm()
                            .bg(if dk { rgb(0x14171A) } else { rgb(0xFFFFFF) })
                            .border_1().border_color(th_cmd_border)
                            .text_size(px(us * 11.0)).text_color(th_top_fg)
                            .children(plan.lines().map(|l| div().child(l.to_string()))));
                        d = d.child(div().flex().flex_row().gap_1().mt_1()
                            .child(button("ai-chat-run", ui::t!("apply").to_string(), true)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ai_chat_insert();
                                    cx.notify()
                                })))
                            .child(button("ai-chat-drop", ui::t!("cancel").to_string(), false)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ai_chat_plan = None;
                                    this.status =
                                        ui::t!("discarded_text_nothing_changed").into();
                                    cx.notify()
                                }))));
                    }
                    d = d.child(div()
                        .id("ai-chat-in")
                        .mt_1().p_1().rounded_sm().cursor_text()
                        .bg(if dk { rgb(0x14171A) } else { rgb(0xFFFFFF) })
                        .border_1()
                        .border_color(if self.ai_chat_focus { accent } else { th_cmd_border })
                        .text_size(px(us * 11.5)).text_color(th_top_fg)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.ai_chat_focus = true;
                            cx.notify()
                        }))
                        .child(if self.ai_chat_in.text().is_empty() {
                            if self.ai_chat_focus {
                                "|".to_string()
                            } else {
                                ui::t!("click_here_type_enter").to_string()
                            }
                        } else if self.ai_chat_focus {
                            let mut s = self.ai_chat_in.text().to_string();
                            let cur = self.ai_chat_in.cursor().min(s.len());
                            s.insert(cur, '|');
                            s
                        } else {
                            self.ai_chat_in.text().to_string()
                        }));
                    let mut r = div().flex().flex_row().gap_1().mt_1();
                    r = r.child(button("ai-chat-send", ui::t!("send").to_string(), !self.ai_busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.ai_chat_send(cx);
                            cx.notify()
                        })));
                    if self.ai_busy {
                        r = r.child(div().text_size(px(us * 10.5)).text_color(th_status)
                            .child(ui::t!("thinking").to_string()));
                    }
                    d = d.child(r);
                }
            }
            let rail_div = rail()
                .child(rail_button("nf-head".into(), "contents", ui::t!("heading").to_string(), self.nav_tab == 0).on_click(
                    cx.listener(|t, _, _, cx| { t.nav_tab = 0; cx.notify() })))
                .child(rail_button("nf-cmt".into(), "co-showcomment", ui::t!("comment").to_string(), self.nav_tab == 1).on_click(
                    cx.listener(|t, _, _, cx| { t.nav_tab = 1; cx.notify() })))
                .child(rail_button("nf-find".into(), "replace", ui::t!("find").to_string(), self.nav_tab == 2).on_click(
                    cx.listener(|t, _, _, cx| { t.nav_tab = 2; cx.notify() })))
                .child(rail_button("nf-ai".into(), "ai-ask", ui::t!("ai").to_string(), self.nav_tab == 3).on_click(
                    cx.listener(|t, _, _, cx| { t.nav_tab = 3; cx.notify() })));
            Some(div()
                .flex_none().w(px(250.0 + RAIL)).h_full()
                .m_1().rounded_sm().bg(panel_bg)
                .border_1().border_color(th_cmd_border)
                .flex().flex_row()
                .child(rail_div)
                .child(div().flex_none().w(px(us * 1.0)).h_full().bg(th_cmd_border))
                .child(d))
        };

        // 右パネル(本家の設定パネル)。**いる場所の設定を、その場で直す**
        let rp_panel = if !self.rp_open {
            None
        } else {
            let panel_bg = if dk { rgb(0x1B1E21) } else { rgb(0xF1F3F5) };
            let (pi, _) = self.cursor_para();
            let para = self.doc.paragraphs().nth(pi).cloned();
            let f = self.doc.char_format_at(self.ed.selection());
            let size_now = self.size_now();
            let head = |t: &'static str| {
                div().text_size(px(us * 11.0)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83)).mt_1().child(t)
            };
            // 小さなボタン(押すと run_cmd。入っていれば色が付く)
            // 見出しは**訳したもの**を受ける。訳をこの中でやらないのは、鍵の走査が
            // ソースを字句で読むから — `ui::t!(…)` は呼ぶ側に残す(calc と同じ作法)。
            // 註の書き方にも同じ罠がある: 括弧の中を**引用符つきで**書くと走査は
            // ここも鍵と数える(それで「…」が対訳表に紛れ込んだ 2026-08-10)
            let btn = |this: &Writer, id: &'static str, label: SharedString| {
                let on = this.toggled(id);
                div().id(SharedString::from(format!("rp-{id}")))
                    .px_2().py_0p5().rounded_sm().cursor_pointer()
                    .border_1()
                    .border_color(if on { th_btn } else { th_cmd_border })
                    .bg(if on { th_btn_hover } else { gpui::transparent_black().into() })
                    .text_size(px(us * 11.5))
                    .text_color(if on { th_btn } else { th_top_fg })
                    .hover(move |st| st.bg(th_btn_hover))
                    .child(label)
            };
            let row = || div().flex().flex_row().flex_wrap().gap_1();
            // 左と同じく**場所を取る**(重ねない)。巻けるようにもする —
            // 表の面が足された分、230px の幅では下が切れる
            let face = self.rp_tab;
            // **どの枝が組んだか**を控える(値を読んだだけでは分からない)
            self.rp_drawn.set(9);
            let return_rp;
            let mut d = div().id("rp-panel")
                .flex_1().min_w(px(us * 0.0)).h_full().overflow_y_scroll()
                .p_2()
                .flex().flex_col().gap_1()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(match face {
                        1 => ui::t!("page_settings_whole_document"),
                        2 => ui::t!("styles_edit_template"),
                        3 => ui::t!("files_what_folder"),
                        _ => ui::t!("settings_adjust_where_cursor"),
                    }));
            // **ページは「いる場所」ではない。** 文書ぜんぶに掛かる決めなので、
            // 柱で別の面に分けた(発注者 2026-08-15「外側にアイコンをおいて
            // 操作を変更できるように」)
            if face == 1 {
                self.rp_drawn.set(1);
                d = d.child(div().text_size(px(us * 11.0)).text_color(th_status)
                    .child(SharedString::from(ui::tf!("mm_margins_mm_columns", self.pg.w_mm, self.pg.h_mm, self.pg.left_mm, self.pg.cols(), if self.doc.vertical { ui::t!("vertical") } else { "" }))));
                d = d.child(row()
                    .child(btn(self, "pageorient", ui::t!("orientation").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("pageorient", cx); cx.notify() })))
                    .child(btn(self, "pagesize", ui::t!("paper").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("pagesize", cx); cx.notify() })))
                    .child(btn(self, "pagemargins", ui::t!("margins").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("pagemargins", cx); cx.notify() })))
                    .child(btn(self, "columns", ui::t!("columns").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("columns", cx); cx.notify() })))
                    .child(btn(self, "direction", ui::t!("vertical_2").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("direction", cx); cx.notify() }))));
                d = d.child(head(ui::t!("header_footer")));
                d = d.child(row()
                    .child(btn(self, "edit-header", ui::t!("edit").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("edit-header", cx); cx.notify() })))
                    .child(btn(self, "pagenum", ui::t!("page_number").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("pagenum", cx); cx.notify() })))
                    .child(btn(self, "watermark", ui::t!("watermark").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("watermark", cx); cx.notify() })))
                    .child(btn(self, "pagecolor", ui::t!("page_colour_2").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("pagecolor", cx); cx.notify() }))));
                let rail_div = rail()
                    .child(rail_button("rf-here".into(), "format", ui::t!("settings_adjust_where_cursor").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 0; cx.notify() })))
                    .child(rail_button("rf-page".into(), "pagesize", ui::t!("page_settings_whole_document").to_string(), true).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 1; cx.notify() })))
                    // **スタイルの柱はどの面でも出す**(2026-08-31 Opus の
                    // 指摘 — この枝だけ欠けていて、ページ設定の面で消えていた)
                    .child(rail_button("rf-style".into(), "styles", ui::t!("styles_edit_template").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 2; cx.notify() })))
                    // **フォルダのファイル一覧**(2026-08-19 発注者
                    // 「フォルダー内のファイル一覧を右パネルに表示」)
                    .child(rail_button("rf-files".into(), "py-folder", ui::t!("files_what_folder").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 3; cx.notify() })));
                return_rp = Some(div()
                    .flex_none().w(px(230.0 + RAIL)).h_full()
                    .m_1().rounded_sm().bg(panel_bg)
                    .border_1().border_color(th_cmd_border)
                    .flex().flex_row()
                    .child(d)
                    .child(div().flex_none().w(px(us * 1.0)).h_full().bg(th_cmd_border))
                    .child(rail_div));
            } else if face == 2 {
                self.rp_drawn.set(2);
            // **スタイルの面**(2026-08-16。ネイティブ文書だけ)。
            // いまの段落が着ているスタイルと、テンプレートの一覧を出す。
            // 押すと着替え、直すとテンプレートが変わって**同じスタイルの所が
            // 一度に変わる** — ライブ合成の効き目がここに出る
                let wearing = para
                    .as_ref()
                    .and_then(|p| {
                        p.style_id.clone().or_else(|| {
                            kumihan::theme::Theme::role_name(p.style).map(|s| s.to_string())
                        })
                    })
                    .unwrap_or_else(|| ui::t!("body").to_string());
                d = d.child(div().text_size(px(us * 11.0)).text_color(th_status).child(
                    SharedString::from(ui::tf!("paragraph", wearing.clone())),
                ));
                // 役割のスタイル(段落そのものの意味)は先に、名前つきは後に
                let mut names: Vec<String> =
                    // **テンプレートのスタイル名。** 文書の形式の値なので日本語のまま
                    // です(engine の role_name() と揃えます)
                    ["本文", "表題", "見出し1", "見出し2", "見出し3", "見出し4", "見出し5", "引用"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                for s in &self.tmpl.styles {
                    if !names.contains(&s.name) {
                        names.push(s.name.clone());
                    }
                }
                let mut r = row();
                // **外す道**(2026-08-17)。見た目のボタンは一覧へ案内するので、
                // ここに「なし」が無いと外せません
                r = r.child(
                    div()
                        .id("rp-st-none")
                        .px_2().py_0p5().rounded_sm().cursor_pointer()
                        .border_1().border_color(th_cmd_border)
                        .text_size(px(us * 11.5)).text_color(th_status)
                        .hover(move |st| st.bg(th_btn_hover))
                        .child(SharedString::from(ui::t!("none").to_string()))
                        .on_click(cx.listener(|t, _, _, cx| {
                            t.strip_style();
                            cx.notify()
                        })),
                );
                for name in names {
                    let on = name == wearing;
                    let n2 = name.clone();
                    r = r.child(
                        div()
                            .id(SharedString::from(format!("rp-st-{name}")))
                            .px_2().py_0p5().rounded_sm().cursor_pointer()
                            .border_1()
                            .border_color(if on { th_btn } else { th_cmd_border })
                            .bg(if on { th_btn_hover } else { gpui::transparent_black().into() })
                            .text_size(px(us * 11.5))
                            .text_color(if on { th_btn } else { th_top_fg })
                            .hover(move |st| st.bg(th_btn_hover))
                            .child(SharedString::from(name.clone()))
                            .on_click(cx.listener(move |t, _, _, cx| {
                                t.wear_style(&n2);
                                cx.notify()
                            })),
                    );
                }
                d = d.child(r);
                // 字を選んでいれば、その字が着ている文字スタイルも出す
                if let Some(cs) = self.selected_char_style() {
                    d = d.child(div().text_size(px(us * 11.0)).text_color(th_status).child(
                        SharedString::from(ui::tf!("selected_text_style", cs)),
                    ));
                }
                // **新しく作る・名前を変える**(2026-09-02)。作った物は
                // テンプレートに入り、名前を変えると本文の名指しも変わる
                d = d.child(row()
                    .child(btn(self, "st-new", ui::t!("new_style").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.style_new_start(); cx.notify() })))
                    .child(btn(self, "st-rename", ui::t!("rename").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.style_rename_start(); cx.notify() }))));
                // いま着ているスタイルの中身(テンプレートが持っている値)。
                // **直すとテンプレートに書かれる**(2026-09-02 発注者「書く処理を
                // 実装する」)。定義がまだ無いスタイルも、直せば節ができる
                let def = self.tmpl.style(&wearing).cloned().unwrap_or_default();
                let mut w: Vec<String> = Vec::new();
                if let Some(s) = def.size_pt {
                    w.push(ui::tf!("size_pt_2", s.to_string()).to_string());
                }
                if let Some(f) = &def.font {
                    w.push(ui::tf!("typeface", f.clone()).to_string());
                }
                if def.bold {
                    w.push(ui::t!("bold").to_string());
                }
                if def.italic {
                    w.push(ui::t!("italic").to_string());
                }
                if def.underline {
                    w.push(ui::t!("underline").to_string());
                }
                if let Some(c) = &def.color {
                    w.push(ui::tf!("colour", c.clone()).to_string());
                }
                if let Some(a) = def.align {
                    w.push(match a {
                        kumihan::Align::Left => ui::t!("left"),
                        kumihan::Align::Center => ui::t!("centered"),
                        kumihan::Align::Right => ui::t!("right"),
                        _ => ui::t!("justify"),
                    }.to_string());
                }
                if let Some(l) = def.line_spacing {
                    w.push(format!("{} {l}", ui::t!("line_spacing")));
                }
                if def.space_after_pt != 0.0 {
                    w.push(format!("{} {}pt", ui::t!("space_after"), def.space_after_pt));
                }
                d = d.child(head(ui::t!("what_style_holds")));
                d = d.child(div().text_size(px(us * 11.0)).text_color(th_status).child(
                    SharedString::from(if w.is_empty() {
                        ui::t!("document_default").to_string()
                    } else {
                        w.join("・")
                    }),
                ));
                d = d.child(row()
                    .child(btn(self, "st-bigger", ui::t!("larger_text").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.tweak_style(1); cx.notify() })))
                    .child(btn(self, "st-smaller", ui::t!("smaller_text").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.tweak_style(-1); cx.notify() }))));
                let flag = |id: &'static str, label: SharedString, on: bool| {
                    div().id(SharedString::from(format!("rp-{id}")))
                        .px_2().py_0p5().rounded_sm().cursor_pointer()
                        .border_1()
                        .border_color(if on { th_btn } else { th_cmd_border })
                        .bg(if on { th_btn_hover } else { gpui::transparent_black().into() })
                        .text_size(px(us * 11.5))
                        .text_color(if on { th_btn } else { th_top_fg })
                        .hover(move |st| st.bg(th_btn_hover))
                        .child(label)
                };
                d = d.child(row()
                    .child(flag("st-bold", ui::t!("bold").into(), def.bold).on_click(
                        cx.listener(|t, _, _, cx| { t.toggle_style_flag("bold"); cx.notify() })))
                    .child(flag("st-italic", ui::t!("italic").into(), def.italic).on_click(
                        cx.listener(|t, _, _, cx| { t.toggle_style_flag("italic"); cx.notify() })))
                    .child(flag("st-underline", ui::t!("underline").into(), def.underline).on_click(
                        cx.listener(|t, _, _, cx| { t.toggle_style_flag("underline"); cx.notify() }))));
                d = d.child(head(ui::t!("alignment")));
                d = d.child(row()
                    .child(flag("st-left", ui::t!("left").into(), def.align == Some(kumihan::Align::Left)).on_click(
                        cx.listener(|t, _, _, cx| { t.set_style_align(kumihan::Align::Left); cx.notify() })))
                    .child(flag("st-center", ui::t!("centered").into(), def.align == Some(kumihan::Align::Center)).on_click(
                        cx.listener(|t, _, _, cx| { t.set_style_align(kumihan::Align::Center); cx.notify() })))
                    .child(flag("st-right", ui::t!("right").into(), def.align == Some(kumihan::Align::Right)).on_click(
                        cx.listener(|t, _, _, cx| { t.set_style_align(kumihan::Align::Right); cx.notify() })))
                    .child(flag("st-justify", ui::t!("justify").into(), def.align == Some(kumihan::Align::Justify)).on_click(
                        cx.listener(|t, _, _, cx| { t.set_style_align(kumihan::Align::Justify); cx.notify() }))));
                d = d.child(row()
                    .child(btn(self, "st-ls-wider", ui::t!("line_spacing_wider").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.tweak_style_line_spacing(1); cx.notify() })))
                    .child(btn(self, "st-ls-narrower", ui::t!("line_spacing_narrower").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.tweak_style_line_spacing(-1); cx.notify() })))
                    .child(btn(self, "st-sa-more", ui::t!("space_after_more").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.tweak_style_space_after(1); cx.notify() })))
                    .child(btn(self, "st-sa-less", ui::t!("space_after_less").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.tweak_style_space_after(-1); cx.notify() }))));
                d = d.child(div().text_size(px(us * 11.0)).text_color(th_status)
                    .child(ui::t!("editing_changes_template_every")));
                let rail_div = rail()
                    .child(rail_button("rf-here".into(), "format", ui::t!("settings_adjust_where_cursor").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 0; cx.notify() })))
                    .child(rail_button("rf-page".into(), "pagesize", ui::t!("page_settings_whole_document").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 1; cx.notify() })))
                    .child(rail_button("rf-style".into(), "styles", ui::t!("styles_edit_template").to_string(), true).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 2; cx.notify() })))
                    // **フォルダのファイル一覧**(2026-08-19 発注者
                    // 「フォルダー内のファイル一覧を右パネルに表示」)
                    .child(rail_button("rf-files".into(), "py-folder", ui::t!("files_what_folder").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 3; cx.notify() })));
                return_rp = Some(div()
                    .flex_none().w(px(230.0 + RAIL)).h_full()
                    .m_1().rounded_sm().bg(panel_bg)
                    .border_1().border_color(th_cmd_border)
                    .flex().flex_row()
                    .child(d)
                    .child(div().flex_none().w(px(us * 1.0)).h_full().bg(th_cmd_border))
                    .child(rail_div));
            } else if face == 3 {
                // **フォルダの中身**(2026-08-19 発注者)。選ぶと開きます。
                // 種類はファイルの名前で決まります(二重の拡張子)
                self.rp_drawn.set(3);
                // **一覧は ui::filelist の1本**(統合の段7)。表の画面と同じ姿。
                // 押したときの行き先だけがアプリの物
                let look = ui::filelist::Look {
                    fg: th_top_fg, dim: th_status, hover: th_btn_hover, scale: us,
                };
                let dir = self.folder();
                d = d.child(ui::filelist::header(&look, dir.as_deref()));
                if let Some(dir) = dir.as_deref() {
                    // **上のフォルダへ戻れます**(2026-08-26)。
                    // 中へ入れても戻れないと、一方通行です
                    if let Some(top) = ui::filelist::up_row(&look, dir) {
                        let parent = dir.parent().map(|p| p.to_path_buf());
                        d = d.child(top.on_click(cx.listener(move |t, _, _, cx| {
                            if let Some(parent) = parent.clone() {
                                t.show_folder(parent);
                            }
                            cx.notify()
                        })));
                    }
                    // **作る道**(2026-08-26 発注者「ファイルマネージャと
                    // 同じ機能をもっていないといけない」)
                    d = d.child(
                        div().flex().flex_row().gap_1().pb_1()
                            .child(ui::filelist::make_button(&look, "folder",
                                ui::t!("folder").into())
                                .on_click(cx.listener(|t, _, _, cx| {
                                    t.fl_start(crate::FlJob::NewFolder);
                                    cx.notify()
                                })))
                            .child(ui::filelist::make_button(&look, "doc",
                                ui::t!("document").into())
                                .on_click(cx.listener(|t, _, _, cx| {
                                    t.fl_start(crate::FlJob::NewDoc);
                                    cx.notify()
                                }))),
                    );
                    // **木の根を、いま見ているフォルダに同期**(2026-08-31
                    // 発注者「IDE にあるものと同じでいい」)。フォルダを
                    // 替えたら展開は根ごと捨てる
                    if self.fl_tree.root() != dir {
                        self.fl_tree.set_root(dir);
                    }
                    let (rows, rest) =
                        self.fl_tree.rows_capped(ui::filelist::LIST_CAP);
                    if rows.is_empty() {
                        d = d.child(ui::filelist::empty(&look));
                    }
                    for (i, r) in rows.into_iter().enumerate() {
                        let e = r.entry.clone();
                        let can_open = e.kind.can_open();
                        let is_a_table = e.kind.is_sheet();
                        let path = e.path.clone();
                        // **フォルダはその場で開閉**(IDE の木)。中へ入る
                        // のではなく、字下げして下に出す。改名と削除の絵は
                        // ファイルと同じに付ける
                        if e.kind == ui::folder::Kind::Folder {
                            let on = self.fl_tree.selected.as_deref() == Some(e.path.as_path());
                            let line = ui::filelist::tree_row(&look, i, &r, on)
                                .on_click(cx.listener(move |t, _, _, cx| {
                                    t.fl_focus = true; // 行を押したら焦点はパネル
                                    t.fl_tree.toggle(&path);
                                    t.fl_tree.selected = Some(path.clone());
                                    cx.notify()
                                }));
                            let path2 = e.path.clone();
                            let path3 = e.path.clone();
                            d = d.child(
                                div().flex().flex_row().items_center().gap_1()
                                    .child(div().flex_1().min_w(px(0.0)).child(line))
                                    .child(ui::filelist::row_button(&look, i, "ren",
                                        ui::t!("rename").into())
                                        .on_click(cx.listener(move |t, _, _, cx| {
                                            t.fl_start(crate::FlJob::Rename(path2.clone()));
                                            cx.notify()
                                        })))
                                    .child(ui::filelist::row_button(&look, i, "del",
                                        ui::t!("erase").into())
                                        .on_click(cx.listener(move |t, _, _, cx| {
                                            t.fl_start(crate::FlJob::Delete(path3.clone()));
                                            cx.notify()
                                        }))),
                            );
                            continue;
                        }
                        let current = self.path.as_deref() == Some(e.path.as_path())
                            || self.fl_tree.selected.as_deref() == Some(e.path.as_path());
                        let mut line = ui::filelist::tree_row(&look, i, &r, current);
                        line = line.on_click(cx.listener(move |t, _, _, cx| {
                            t.fl_focus = true; // 行を押したら焦点はパネル
                            t.fl_tree.selected = Some(path.clone());
                            t.remember_folder();
                            if !can_open {
                                // **こちらで開けない種類は、機械の関連付けに渡します**
                                // (2026-08-24 発注者「何のツールでも使えるようにする」)。
                                // .ipynb なら JupyterLab、.py なら決めた道具が起きます。
                                // 断るのではなく渡すのが、綴りを預かる側の仕事です
                                // **機械の関連付けに渡します。** `open_for_edit` は
                                // 使いません — あれは「.py を編集する道具」の道で、
                                // 隣の writer に落ちます。実機で押したら .ipynb が
                                // writer で開きました(2026-08-24)。JupyterLab で
                                // 開くべき物なので、機械の決めをそのまま使います
                                t.status = match ui::open_outside(&path.display().to_string()) {
                                    ui::Opened::Yes => ui::tf!("opening_application_system_chose",
                                        path.file_name().unwrap_or_default().to_string_lossy().to_string()).into(),
                                    ui::Opened::JustNow => ui::t!("just_opened").into(),
                                    ui::Opened::Failed => ui::tf!("no_application_associated_file",
                                        path.display().to_string()).into(),
                                };
                                cx.notify();
                                return;
                            }
                            // **埋め込みなら種類を問わず officework に頼む**(段1)
                            if t.embedded || is_a_table {
                                t.open_request = Some(path.clone());
                            } else {
                                t.open_in_tab(path.clone());
                            }
                            cx.notify()
                        }));
                        let path2 = e.path.clone();
                        let path3 = e.path.clone();
                        d = d.child(
                            div().flex().flex_row().items_center().gap_1()
                                .child(div().flex_1().min_w(px(0.0)).child(line))
                                .child(ui::filelist::row_button(&look, i, "ren",
                                    // **名前を変える釦**。リボンの「名前」
                                    // (defname)とは別の意味です
                                    ui::t!("rename").into())
                                    .on_click(cx.listener(move |t, _, _, cx| {
                                        t.fl_start(crate::FlJob::Rename(path2.clone()));
                                        cx.notify()
                                    })))
                                .child(ui::filelist::row_button(&look, i, "del",
                                    ui::t!("erase").into())
                                    .on_click(cx.listener(move |t, _, _, cx| {
                                        t.fl_start(crate::FlJob::Delete(path3.clone()));
                                        cx.notify()
                                    }))),
                        );
                    }
                    // **切った分は言います**(2026-08-26)。黙って落とすと、
                    // あるはずのファイルが無いように見えます
                    if let Some(note_div) = ui::filelist::rest_note(&look, rest) {
                        d = d.child(note_div);
                    }
                }
                let rail_div = rail()
                    .child(rail_button("rf-here".into(), "format", ui::t!("settings_adjust_where_cursor").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 0; cx.notify() })))
                    .child(rail_button("rf-page".into(), "pagesize", ui::t!("page_settings_whole_document").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 1; cx.notify() })))
                    // **スタイルの柱はどの面でも出す**(2026-08-31 — この枝も欠けていた)
                    .child(rail_button("rf-style".into(), "styles", ui::t!("styles_edit_template").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 2; cx.notify() })))
                    .child(rail_button("rf-files".into(), "py-folder", ui::t!("files_what_folder").to_string(), true).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 3; cx.notify() })));
                // **焦点がパネルにある間は枠の色で見せる**(2026-08-31 発注者
                // 「枠の色は任せる」)。押した行から矢印で動かせる印
                return_rp = Some(div()
                    .flex_none().w(px(230.0 + RAIL)).h_full()
                    .m_1().rounded_sm().bg(panel_bg)
                    .border_1().border_color(if self.fl_focus { th_btn } else { th_cmd_border })
                    .flex().flex_row()
                    .child(d.overflow_y_scroll())
                    .child(div().flex_none().w(px(us * 1.0)).h_full().bg(th_cmd_border))
                    .child(rail_div));
            } else {

            // **いる場所に追従する。** 表の中なら表の面、段落に数式や画像が
            // あればその面を、文字・段落・ページの前に出す
            // (発注者 2026-08-14「選んでいる物の設定に切り替わるように」)。
            // 出すだけで下の面も残す — 表の中でも字は太字にしたい
            if let Some((_, line, row_box, n_rows, n_cols)) = self.cursor_table() {
                d = d.child(head(ui::t!("table_2")));
                d = d.child(div().text_size(px(us * 11.0)).text_color(th_status)
                    .child(SharedString::from(ui::tf!(
                        "rows_columns_now_row", n_rows, n_cols, line + 1, row_box + 1))));
                d = d.child(row()
                    .child(btn(self, "tb-row-up", ui::t!("row_above").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_add_row(false); cx.notify() })))
                    .child(btn(self, "tb-row-dn", ui::t!("row_below").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_add_row(true); cx.notify() })))
                    .child(btn(self, "tb-row-del", ui::t!("delete_row").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_del_row(); cx.notify() }))));
                d = d.child(row()
                    .child(btn(self, "tb-col-l", ui::t!("column_left").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_add_col(false); cx.notify() })))
                    .child(btn(self, "tb-col-r", ui::t!("column_right").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_add_col(true); cx.notify() })))
                    .child(btn(self, "tb-col-del", ui::t!("delete_column").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_del_col(); cx.notify() }))));
            }
            // 数式と画像は**段落が持つ**(writer に図形の選択という状態は無い)。
            // 数式は絵だが `tex` に原文を積んであるので、直せる
            if let Some(p0) = &para {
                let icon: Vec<&kumihan::InlineImage> =
                    p0.images.iter().chain(p0.images_new.iter()).collect();
                let formula_of: Vec<&&kumihan::InlineImage> =
                    icon.iter().filter(|im| im.tex.is_some()).collect();
                if let Some(im) = formula_of.first() {
                    let tex = im.tex.clone().unwrap_or_default();
                    d = d.child(head(ui::t!("formula_2")));
                    d = d.child(div().text_size(px(us * 10.5)).text_color(th_status)
                        .child(SharedString::from(tex.clone())));
                    d = d.child(row().child(
                        btn(self, "eq-edit", ui::t!("edit_equation").into()).on_click(
                            cx.listener(move |t, _, _, cx| {
                                // 原文を欄に載せて開く。**打ち直しにしない**
                                t.eq_ed = Editor::new(&tex);
                                t.eq_open = true;
                                t.status = ui::t!("editing_equation_enter_re").into();
                                cx.notify()
                            }))));
                } else if let Some(im) = icon.first() {
                    d = d.child(head(ui::t!("images")));
                    d = d.child(div().text_size(px(us * 11.0)).text_color(th_status)
                        .child(SharedString::from(
                            ui::tf!("mm", im.w_mm, im.h_mm))));
                    d = d.child(row()
                        .child(btn(self, "img-small", ui::t!("smaller").into()).on_click(
                            cx.listener(|t, _, _, cx| { t.image_scale(0.9); cx.notify() })))
                        .child(btn(self, "img-big", ui::t!("bigger").into()).on_click(
                            cx.listener(|t, _, _, cx| { t.image_scale(1.1); cx.notify() }))));
                }
            }

            // 文字
            d = d.child(head(ui::t!("character")))
                .child(row()
                    .child(btn(self, "bold", ui::t!("bold").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("bold", cx); cx.notify() })))
                    .child(btn(self, "italic", ui::t!("italic").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("italic", cx); cx.notify() })))
                    .child(btn(self, "underline", ui::t!("underline").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("underline", cx); cx.notify() })))
                    .child(btn(self, "strikeout", ui::t!("strikethrough").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("strikeout", cx); cx.notify() }))))
                .child(row()
                    .child(div().text_size(px(us * 11.0)).text_color(th_status)
                        .child(SharedString::from(ui::tf!("size_pt_font", if size_now.fract() == 0.0 {
                                format!("{}", size_now as i32)
                            } else {
                                format!("{size_now}")
                            }, self.font_name)))))
                .child(row()
                    .child(btn(self, "decfont", ui::t!("smaller").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("decfont", cx); cx.notify() })))
                    .child(btn(self, "incfont", ui::t!("bigger").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("incfont", cx); cx.notify() })))
                    .child(btn(self, "fontcolor", ui::t!("colour_3").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("fontcolor", cx); cx.notify() })))
                    .child(btn(self, "clearstyle", ui::t!("clear_formatting").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("clearstyle", cx); cx.notify() }))));
            if f.field.is_some() {
                d = d.child(div().text_size(px(us * 10.5)).text_color(th_status)
                    .child(ui::t!("cross_reference_update_via")));
            }
            if let Some(rt) = &f.ruby {
                d = d.child(div().text_size(px(us * 10.5)).text_color(th_status)
                    .child(SharedString::from(ui::tf!("ruby", rt))));
            }

            // 段落
            let (al, ls, ind, lst) = match &para {
                Some(p) => (p.align, p.spacing(), p.indent, p.list),
                None => (Align::Left, 1.0, 0, ListKind::None),
            };
            d = d.child(head(ui::t!("paragraphs")))
                .child(row()
                    .children([
                        ("align-left", ui::t!("left"), Align::Left),
                        ("align-center", ui::t!("centre"), Align::Center),
                        ("align-right", ui::t!("right"), Align::Right),
                        ("align-just", ui::t!("justify"), Align::Justify),
                        ("align-dist", ui::t!("distribute"), Align::Distribute),
                    ].map(|(id, label, a)| {
                        let on = al == a;
                        div().id(SharedString::from(format!("rp-{id}")))
                            .px_2().py_0p5().rounded_sm().cursor_pointer()
                            .border_1()
                            .border_color(if on { th_btn } else { th_cmd_border })
                            .bg(if on { th_btn_hover } else { gpui::transparent_black().into() })
                            .text_size(px(us * 11.5))
                            .text_color(if on { th_btn } else { th_top_fg })
                            .hover(move |st| st.bg(th_btn_hover))
                            .child(label)
                            .on_click(cx.listener(move |t, _, _, cx| {
                                t.run_cmd(id, cx);
                                cx.notify()
                            }))
                    })))
                .child(row()
                    .child(div().text_size(px(us * 11.0)).text_color(th_status)
                        .child(SharedString::from(ui::tf!("line_spacing_indent", ls, ind)))))
                .child(row()
                    .child(btn(self, "linespace", ui::t!("line_spacing").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("linespace", cx); cx.notify() })))
                    .child(btn(self, "decoffset", ui::t!("indent").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("decoffset", cx); cx.notify() })))
                    .child(btn(self, "incoffset", ui::t!("indent_2").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("incoffset", cx); cx.notify() }))))
                .child(row()
                    // **✓ は見出しだけ** — 鍵は素のまま(calc の freeze と同じ作法)
                    .child(btn(self, "markers", {
                        let l = ui::t!("bullets");
                        if lst == ListKind::Bullet { format!("{l} ✓").into() } else { l.into() }
                    }).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("markers", cx); cx.notify() })))
                    .child(btn(self, "numbering", {
                        let l = ui::t!("numbering");
                        if lst == ListKind::Number { format!("{l} ✓").into() } else { l.into() }
                    }).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("numbering", cx); cx.notify() })))
                    .child(btn(self, "paracolor", ui::t!("shading").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("paracolor", cx); cx.notify() })))
                    .child(btn(self, "borders", ui::t!("borders").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("borders", cx); cx.notify() }))));

            // ページ
            let rail_div = rail()
                .child(rail_button("rf-here".into(), "format", ui::t!("settings_adjust_where_cursor").to_string(), face == 0).on_click(
                    cx.listener(|t, _, _, cx| { t.rp_tab = 0; cx.notify() })))
                .child(rail_button("rf-page".into(), "pagesize", ui::t!("page_settings_whole_document").to_string(), face == 1).on_click(
                    cx.listener(|t, _, _, cx| { t.rp_tab = 1; cx.notify() })))
                // **スタイルの面はネイティブ文書だけ**(2026-08-16)。互換の
                // 文書にはテンプレートが無く、押しても見せる物が無い —
                // できないことを、できるように見せない
                .children(self.native.then(|| {
                    rail_button("rf-style".into(), "styles", ui::t!("styles_edit_template").to_string(), face == 2)
                        .on_click(cx.listener(|t, _, _, cx| { t.rp_tab = 2; cx.notify() }))
                }))
                // **フォルダのファイル一覧**(2026-08-19)
                .child(rail_button("rf-files".into(), "py-folder", ui::t!("files_what_folder").to_string(), false).on_click(
                    cx.listener(|t, _, _, cx| { t.rp_tab = 3; cx.notify() })));
            return_rp = Some(div()
                .flex_none().w(px(230.0 + RAIL)).h_full()
                .m_1().rounded_sm().bg(panel_bg)
                .border_1().border_color(th_cmd_border)
                .flex().flex_row()
                .child(d)
                .child(div().flex_none().w(px(us * 1.0)).h_full().bg(th_cmd_border))
                .child(rail_div));
            }
            return_rp
        };

        // リンクのパネル(押すと辿る。公開 Web も見える — JS は実行しない)
        let lk_panel = if !self.lk_open || self.html_links.is_empty() {
            None
        } else {
            let mut d = div().absolute().right(px(us * 16.0)).bottom(px(us * 8.0)).w(px(us * 340.0))
                .max_h(px(us * 300.0)).overflow_hidden()
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_1()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(SharedString::from(ui::tf!("links_click_follow_esc", self.html_links.len()))));
            for (i, (href, text)) in self.html_links.iter().take(16).enumerate() {
                let href2 = href.clone();
                d = d.child(div()
                    .id(SharedString::from(format!("lk-{i}")))
                    .px_2().py_0p5().rounded_sm().cursor_pointer()
                    .text_size(px(us * 12.0)).text_color(rgb(0x165E83))
                    .whitespace_nowrap().overflow_hidden()
                    .hover(|st| st.bg(rgb(0xEAF2F7)))
                    .child(SharedString::from(text.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.follow_link(href2.clone(), cx);
                        cx.notify()
                    })));
            }
            if self.html_links.len() > 16 {
                d = d.child(div().text_size(px(us * 10.5)).text_color(rgb(0x66707A))
                    .child(SharedString::from(ui::tf!("more_not_shown", self.html_links.len() - 16))));
            }
            Some(d)
        };

        // AI に頼むパネル
        let ai_panel = if !self.ai_open {
            None
        } else {
            let mut t = self.ai_ed.text().to_string();
            let cur = self.ai_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 460.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(SharedString::from(ui::tf!("destination_esc_cancels", if self.ai_macro { ui::t!("ask_ai_macro_script_2") } else { ui::t!("ask_ai") }, ui::ai::backend().label()))))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(rgb(0xFFFFFF))
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t)))
                .child(div().text_size(px(us * 10.5)).text_color(rgb(0x66707A))
                    .child(if self.ai_macro {
                        ui::t!("scripts_only_placed_plug")
                    } else {
                        ui::t!("answer_goes_cursor_ctrl")
                    })))
        };

        // 記入欄の選択肢を聞くパネル
        let sd_panel = if !self.sd_open {
            None
        } else {
            let mut t = self.sd_ed.text().to_string();
            let cur = self.sd_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 400.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(SharedString::from(if self.sd_naming {
                        ui::t!("field_name_type_press").to_string()
                    } else {
                        ui::tf!("choices_comma_separated_enter", self.sd_kind.label())
                    })))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(rgb(0xFFFFFF))
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t))))
        };

        // ルビのパネル(読みの入力)
        let rb_panel = if !self.rb_open {
            None
        } else {
            let mut t = self.rb_ed.text().to_string();
            let cur = self.rb_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 360.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("ruby_type_reading_press_enter")))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t))))
        };

        // 数式のパネル(LaTeX を打つ)。**組むのはエンジン(typst + mitex)**。
        // 打った原文は絵と一緒に残る
        let eq_panel = if !self.eq_open {
            None
        } else {
            let mut t = self.eq_ed.text().to_string();
            let cur = self.eq_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 460.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("equation_type_latex_press")))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t)))
                .child(div().text_size(px(us * 10.5)).text_color(rgb(0x60707C))
                    .child(ui::t!("e_g_frac_sqrt"))))
        };

        // プラグインのパネル(置き場の .py 一覧。押すとサンドボックスの中で実行)
        let plug_panel = if !self.plug_open {
            None
        } else {
            let dir = plugins_dir();
            let mut items: Vec<PathBuf> = std::fs::read_dir(&dir)
                .ok()
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.path())
                        .filter(|p| p.extension().is_some_and(|e| e == "py"))
                        .collect()
                })
                .unwrap_or_default();
            items.sort();
            let mut d = div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 420.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("plugins_click_run_sandbox")))
                .child(div().text_size(px(us * 11.0)).text_color(rgb(0x66707A))
                    .child(SharedString::from(ui::tf!("location", dir.display()))));
            if items.is_empty() {
                d = d.child(div().text_size(px(us * 11.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("none_yet_put_py")));
            }
            for (i, q) in items.into_iter().enumerate() {
                let name = q
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                d = d.child(div()
                    .id(SharedString::from(format!("plug-{i}")))
                    .px_2().py_0p5().rounded_sm()
                    .text_size(px(us * 12.5)).cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF2F7)))
                    .child(SharedString::from(name))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.plug_open = false;
                        this.run_macro_file(q.clone(), cx);
                        cx.notify()
                    })));
            }
            Some(d)
        };

        // 相互参照のパネル(しおり一覧 → 文字/ページを挿す。更新もここ)
        let xr_panel = if !self.xr_open {
            None
        } else {
            let names: Vec<String> = self
                .doc
                .paragraphs()
                .flat_map(|p| p.bookmarks.iter().cloned())
                .collect();
            let mut d = div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 360.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().flex().flex_row().items_center()
                    .child(div().flex_1().text_size(px(us * 11.5))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0x165E83))
                        .child(ui::t!("cross_reference_insert_bookmarks")))
                    .child(div().id("xr-refresh").px_2().py_0p5().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                        .text_size(px(us * 11.0)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF5EE)))
                        .child(ui::t!("update_references"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_refs();
                            cx.notify()
                        }))));
            if names.is_empty() {
                d = d.child(div().text_size(px(us * 11.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("no_bookmarks_add_them")));
            }
            for (i, name) in names.into_iter().enumerate() {
                let n1 = name.clone();
                let n2 = name.clone();
                d = d.child(div().flex().flex_row().items_center().gap_2()
                    .child(div().flex_1().text_size(px(us * 12.5))
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(name)))
                    .child(div().id(SharedString::from(format!("xrt-{i}")))
                        .px_2().py_0p5().rounded_sm()
                        .border_1().border_color(rgb(0x165E83)).text_color(rgb(0x165E83))
                        .text_size(px(us * 11.0)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF2F7)))
                        .child(ui::t!("character"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.insert_ref(&n1, false);
                            cx.notify()
                        })))
                    .child(div().id(SharedString::from(format!("xrp-{i}")))
                        .px_2().py_0p5().rounded_sm()
                        .border_1().border_color(rgb(0x165E83)).text_color(rgb(0x165E83))
                        .text_size(px(us * 11.0)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF2F7)))
                        .child(ui::t!("page_2"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.insert_ref(&n2, true);
                            cx.notify()
                        }))));
            }
            Some(d)
        };

        // フォントの一覧。この機械にある日本語の書体だけ
        // **一覧は3つとも ui::picklist が描きます**(2026-08-20。SEKKEI
        // 「リボンのドロップダウンを1つの仕組みにする」の手順2)。
        //
        // 前は3つそれぞれが自前で描いていて、出る場所も左上の決め打ち
        // (left 16 / top 8)でした。押したボタンの真下に出す決め
        // (2026-08-15 発注者)が表の画面にしか効いていませんでした。
        //
        // *置き場も変えます。* この層は編集の面の中にいたので、リボンの
        // 高さぶん下から始まっていました。窓の根へ移して、ボタンの箱
        // (`btn_box`。窓の座標)をそのまま使えるようにします
        // **開いているのは多くて1つ。**鍵をそのまま渡します(2026-08-22)
        let font_panel = (self.open_list == Some("fontname"))
            .then(|| self.draw_list("fontname", cx));
        let size_panel = (self.open_list == Some("fontsize"))
            .then(|| self.draw_list("fontsize", cx));
        let style_panel = (self.open_list == Some("parastyle"))
            .then(|| self.draw_list("parastyle", cx));

        // 記号の一覧。**3つの一覧と同じ仕組み**(ui::picklist)で、マス目の並びで
        // 出します(2026-08-21。前は右上に固定の自前の格子でした)
        let symbol_panel = (self.open_list == Some("inssymbol"))
            .then(|| self.draw_list("inssymbol", cx));
        // **足したら、ここにも足す。** 開くだけ足して描く側を忘れると、
        // 一覧は開いているのに画面に何も出ません(2026-08-25 に実機で
        // 見つけました — 押してもマス目が出ませんでした)
        // 一覧の仕事(作る・名前を変える・消す)の欄(2026-08-26)
        let fl_panel = match &self.fl_job {
            None => None,
            Some(job) => {
                use crate::FlJob as J;
                let erase_at = matches!(job, J::Delete(_));
                let heading = match job {
                    J::NewFolder => ui::t!("name_new_folder_type").to_string(),
                    J::NewDoc => ui::t!("name_new_document_type").to_string(),
                    J::Rename(_) => ui::t!("new_name_type_press").to_string(),
                    J::Delete(p) => ui::tf!("delete_enter_delete_esc_cancel",
                        p.file_name().unwrap_or_default().to_string_lossy().to_string()).to_string(),
                    J::Footnote(_) => ui::t!("footnote_type_note_press_enter").to_string(),
                    J::TextArt => ui::t!("text_art_type_text_decorate").to_string(),
                };
                let mut d = div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 400.0))
                    .p_3().rounded_md().bg(rgb(0xF7F9FA))
                    .border_1().border_color(rgb(0xC6CDD3))
                    .flex().flex_col().gap_2()
                    .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0x165E83))
                        .child(SharedString::from(heading)));
                if !erase_at {
                    let mut t = self.fl_ed.text().to_string();
                    let cur = self.fl_ed.cursor().min(t.len());
                    t.insert(cur, '|');
                    d = d.child(div().px_2().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                        .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(t)));
                } else {
                    // **ごみ箱には入りません。** そう書いておきます
                    d = d.child(div().text_size(px(us * 10.5)).text_color(rgb(0xB00020))
                        .child(ui::t!("not_go_trash_cannot")));
                }
                Some(d)
            }
        };

        // 表の大きさ。**選ぶのではなく打ちます**(2026-08-25 発注者)
        let tbl_panel = if !self.tbl_open {
            None
        } else {
            let mut t = self.tbl_ed.text().to_string();
            let cur = self.tbl_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(us * 16.0)).top(px(us * 8.0)).w(px(us * 360.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("table_size_type_rows_columns")))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t)))
                .child(div().text_size(px(us * 10.5)).text_color(rgb(0x5A6672))
                    .child(ui::t!("table_spans_text_width"))))
        };
        let date_panel = (self.open_list == Some("datetime"))
            .then(|| self.draw_list("datetime", cx));
        let export_panel = (self.open_list == Some("f-export"))
            .then(|| self.draw_list("f-export", cx));
        // 揃え方と保護の種類の一覧も、この置き場で描く(中身は keys.rs)
        let user_font_panel = self
            .open_list
            .filter(|k| {
                matches!(
                    *k,
                    "user-font" | "img-align" | "prot-doc" | "insshape" | "insshape-2"
                        | "inssmartart" | "inssmartart-2"
                )
            })
            .map(|k| self.draw_list(k, cx));

        // 校正の指摘
        let proof_panel = if self.proof.is_empty() && self.proof_msg.is_empty() {
            None
        } else {
            let mut d = div().absolute().right(px(us * 16.0)).bottom(px(us * 16.0)).w(px(us * 300.0))
                .p_3().rounded_md().bg(gpui::white())
                .border_1().border_color(rgb(0xC6CDD3))
                .child(div().text_size(px(us * 11.5)).font_weight(gpui::FontWeight::BOLD)
                       .text_color(rgb(0x165E83))
                       .child(SharedString::from(ui::tf!("proofread", self.proof_msg))));
            for n in &self.proof {
                // どちらの道具が出したかを隠さない。辞書の指摘は GPU 無しで再現できる
                let tool = match n.source {
                    ui::check::Source::Dictionary => ui::t!("dictionary"),
                    ui::check::Source::Model => ui::t!("model"),
                };
                let cand = if n.candidates.is_empty() {
                    ui::t!("no_suggestions").to_string()
                } else {
                    n.candidates.join(" / ")
                };
                d = d.child(div().mt_1p5().text_size(px(us * 11.5))
                    .child(SharedString::from(
                        ui::tf!("from_to_pair", n.found, cand, n.kind.label(), tool))));
            }
            Some(d)
        };

        Panels {
            find_panel, hf_panel, cmt_panel, wm_panel, bm_panel, style_new_panel, hist_panel,
            chat_panel, pw_panel, url_panel, fm_panel, nav_panel, rp_panel,
            lk_panel, ai_panel, sd_panel, rb_panel, eq_panel, plug_panel, xr_panel,
            font_panel, size_panel, style_panel, symbol_panel, proof_panel,
            tbl_panel, date_panel, export_panel, fl_panel, user_font_panel,
        }
    }

    /// **一覧の中身**(鍵, 見出し)。鍵で引き当て、見出しを画面に出します。
    pub(crate) fn list_items(&self, kind: &str) -> Vec<(String, String)> {
        match kind {
            // 記号。事務の書類で使うものだけ(飾りの絵文字は入れない)。
            // 鍵=字そのもの — 訳す物ではありません
            "inssymbol" => [
                "〒", "※", "→", "←", "↑", "↓", "℃", "±", "×", "÷",
                "①", "②", "③", "④", "⑤", "⑥", "⑦", "⑧", "⑨", "⑩",
                "㈱", "㈲", "№", "〆", "〜", "…", "・", "「", "」", "『",
                "』", "【", "】", "○", "●", "◎", "△", "▲", "□", "■",
            ]
            .iter()
            .map(|s| (s.to_string(), s.to_string()))
            .collect(),
            // **書き出す形。** 文章の節から出せるのは4つです
            // (手引き `docs/ja/commands/ファイル/エクスポート.adoc` の表)。
            // `.adoc` はここに出しません — *保存の側*だからです
            "f-export" => vec![
                ("docx".into(), ui::t!("word_document_docx").to_string()),
                ("html".into(), ui::t!("web_page_html").to_string()),
                ("pdf".into(), ui::t!("pdf_pdf").to_string()),
                ("text".into(), ui::t!("plain_text_txt").to_string()),
            ],
            // 日付の形。**西暦と和暦**(鍵=出す字そのもの — 訳しません)
            "datetime" => crate::cmds::date_shape(),
            "img-align" | "prot-doc" | "insshape" | "insshape-2" | "inssmartart"
            | "inssmartart-2" => self.extra_list_items(kind),
            // **この機械の標準の書体**(2026-08-26)。中身は書体の一覧と同じ
            "user-font" => self.list_items("fontname"),
            "fontname" => {
                // **数で切りません**(2026-08-20)。前は先頭 24 件で切っていて、
                // 25 件目からは選べませんでした。代わりに絞り込みを付けます
                let q = self.font_filter.as_ref().map(|e| e.text().to_string()).unwrap_or_default();
                // **画面の言語の字が組める書体だけ**(2026-08-26)。前は
                // 日本語で決め打っていたので、韓国語の画面ではハングルの
                // 出ない書体ばかりが並んでいました
                let script = kumihan::font::script_of(ui::language());
                kumihan::font::list()
                    .iter()
                    .filter(|f| f.covers(script) && f.regular)
                    .map(|f| f.name.clone())
                    .filter(|n| q.is_empty() || n.to_lowercase().contains(&q.to_lowercase()))
                    .map(|n| (n.clone(), n))
                    .collect()
            }
            "fontsize" => {
                // 並びは共通の表+この文書の標準(テンプレートの大きさ。既定 10.5)。
                // 前は writer 独自の12個で、+/−(共通の表を辿る)と食い違って
                // いました — 一覧で 10.5 を選べるのに + を押すと 11 に飛びました
                let std = self.doc.size_pt.unwrap_or(kumihan::DEFAULT_PT);
                ui::combo::sizes_with(Some(std))
                    .into_iter()
                    .map(|pt| (pt.to_string(), ui::combo::size_label(pt)))
                    .collect()
            }
            // 照合は番号(`set_para_style`)。見出しは訳してよい字です
            _ => (0u8..=5)
                .map(|n| {
                    let label = match n {
                        0 => ui::t!("normal"),
                        1 => ui::t!("heading_1"),
                        2 => ui::t!("heading_2"),
                        3 => ui::t!("heading_3"),
                        4 => ui::t!("heading_4"),
                        _ => ui::t!("heading_5"),
                    };
                    (n.to_string(), label.to_string())
                })
                .collect(),
        }
    }

    /// **一覧を描く**(手順2)。位置は押したボタンの真下です。
    fn draw_list(
        &self,
        kind: &'static str,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let us = self.ui_scale;
        let items = self.list_items(kind);
        // ボタンの箱は窓の座標で控えてあります(`btn_box`)。まだ描いて
        // いなければ左上へ逃がします — 黙って消えるよりは出すほうがよい
        // 2段目の一覧(`-2`)は、1段目と同じボタンの下に重ねます
        let btn = kind.trim_end_matches("-2");
        let (bx, by, bw, bh) = self.btn_box.borrow().get(btn).copied().unwrap_or((16.0, 8.0, 0.0, 0.0));
        // 記号はマス目の並び(1行10個)なので、高さは行の数で見積もります
        let want_h = if kind == "inssymbol" {
            (items.len() as f32 / 10.0).ceil() * 32.0 + 16.0
        } else {
            items.len() as f32 * 26.0 + if kind == "fontname" { 40.0 } else { 0.0 } + 12.0
        };
        // 10個 × 28 + マス目の間(9×4)+ 内側の余白ぶん
        const SYM_W: f32 = 334.0;
        let (up, at, max_h) = ui::combo::pop_place(by, by + bh, want_h, self.view_h_px);
        // 記号は幅が分かっているので、右端で切れないよう幅ごと寄せます
        let x = if kind == "inssymbol" {
            ui::combo::pop_x_w(bx, self.view_w_px, SYM_W)
        } else {
            ui::combo::pop_x(bx, self.view_w_px)
        };
        let widths = match kind {
            "fontname" => ui::picklist::Width::Range(200.0, ui::combo::POP_W),
            "fontsize" => ui::picklist::Width::Range(bw.max(96.0), 140.0),
            "inssymbol" => ui::picklist::Width::Fixed(SYM_W),
            _ => ui::picklist::Width::Range(bw.max(160.0), 240.0),
        };
        let filter = self.font_filter.as_ref().map(|ed| {
            let mut t = ed.text().to_string();
            let cur = ed.cursor().min(t.len());
            t.insert(cur, '|');
            (t, ed.text().is_empty())
        });
        let draw_with_font = kind == "fontname";
        ui::picklist::panel(
            &ui::picklist::Look {
                bg: gpui::rgb(0xFFFFFF),
                border: gpui::rgb(0xC6CDD3),
                fg: gpui::rgb(0x1B1B1B),
                dim: gpui::rgb(0x66707A),
                ghost: gpui::rgb(0x9AA3AB),
                hover: gpui::rgb(0xEAF2F7),
                accent: gpui::rgb(0x165E83),
                // **文章の画面は倍率を掛けません**(2026-08-20 発注者。
                // 画面の文字の大きさは基本的に変えない決め)
                scale: us,
            },
            Some(&ui::picklist::Place {
                x,
                at,
                up,
                max_h,
                width: widths,
                // マス目で並べる物。記号は 28px、表の大きさは 26px の角
                grid: match kind {
                    "inssymbol" => Some(28.0),
                    _ => None,
                },
            }),
            // **何に掛かるかを頭に出します。** 前の版が出していた案内で、
            // 「選んだ所だけ」なのか「段落ぜんぶ」なのかは、押す前に
            // 分かっていないと困ります
            match kind {
                "fontname" => Some(ui::t!("font_applies_selected_paragraph").into()),
                "parastyle" => Some(ui::t!("paragraph_style_applies_selected").into()),
                // 分類の一覧には題を、形の一覧には選んだ分類の名前を出します
                "insshape" => Some(ui::t!("shape_category").into()),
                "insshape-2" => Some(crate::keys::shape_cat_label(self.list_cat).into()),
                "inssmartart-2" => crate::keys::smartart()
                    .into_iter()
                    .find(|(k, _, _)| *k == self.list_cat)
                    .map(|(_, l, _)| l.into()),
                _ => None,
            },
            filter,
            &items,
            self.pick_sel,
            move |key: &str| ui::picklist::Deco {
                swatch: None,
                font: draw_with_font.then(|| key.to_string()),
            },
            cx,
            move |this: &mut Writer, key, cx| this.choose_list(kind, key, cx),
        )
    }

    /// 一覧の項を選んだ。**閉じるのもここ**です。
    pub(crate) fn choose_list(&mut self, kind: &str, key: &str, cx: &mut gpui::Context<Self>) {
        // 記号は**閉じません** — 続けて何個も入れる使い方(前からの形)を
        // 保ちます。閉じるのは Esc か、他のボタンを押したとき(close_menus)
        if kind == "inssymbol" {
            self.ed.insert(key);
            self.on_edited();
            return;
        }
        self.open_list = None;
        self.font_filter = None;
        self.pick_sel = 0;
        match kind {
            "f-export" => match key {
                "docx" => self.export_as(cx, "docx"),
                "html" => self.save_html(cx),
                "pdf" => self.save_pdf(cx),
                _ => self.export_as(cx, "txt"),
            },
            // **この機械の標準の書体を決める**(2026-08-26 発注者
            // 「ユーザーとしての標準設定は、HOME/~.config/ ディレクトリにおく」)
            "user-font" => self.set_user_font(key),
            "img-align" | "prot-doc" => self.choose_extra_list(kind, key),
            "insshape" | "insshape-2" | "inssmartart" | "inssmartart-2" => {
                self.choose_shape_list(kind, key, cx)
            }
            "datetime" => {
                self.checkpoint(false); // 日付
                if self.hf_edit.is_some() {
                    self.hf_ed.insert(key);
                } else {
                    self.ed.insert(key);
                }
                self.on_edited();
                self.status =
                    ui::tf!("date_inserted_fixed_text", key).into();
            }
            "fontname" => {
                let sel = self.ed.selection();
                self.flush_target();
                self.doc.apply_font(sel, Some(key.to_string()));
                self.dirty = true;
                self.relayout_keep();
                self.status = ui::tf!("font", key).into();
            }
            "fontsize" => {
                let Ok(pt) = key.parse::<f32>() else { return };
                let sel = self.ed.selection();
                self.flush_target();
                self.doc.apply_size(sel, move |_| pt);
                self.dirty = true;
                self.relayout_keep();
                self.status = ui::tf!("size_pt", pt).into();
            }
            _ => {
                if let Ok(n) = key.parse::<u8>() {
                    self.set_para_style(n);
                }
            }
        }
        cx.notify();
    }

    /// 一覧の件数(↑↓ の端を決めるのに使います)。
    pub(crate) fn n_items(&self, kind: &str) -> usize {
        self.list_items(kind).len()
    }

    /// **Enter で今選んでいる項に決める**(手順2)。決めたら真。
    pub(crate) fn decide_list(&mut self, cx: &mut gpui::Context<Self>) -> bool {
        // **記号は Enter で決めません**(続けて何個も入れる形なので)
        let kind = match self.open_list {
            Some(k) if k != "inssymbol" => k,
            _ => return false,
        };
        let items = self.list_items(kind);
        match items.get(self.pick_sel) {
            Some((key, _)) => {
                let key = key.clone();
                self.choose_list(kind, &key, cx);
            }
            // 絞り込んで1つも残らなかったとき。**黙って閉じません** —
            // 打った字が悪いのか、そういう書体が無いのかが分かるように
            None => {
                self.status = ui::t!("there_no_font_name").into();
            }
        }
        true
    }
}
