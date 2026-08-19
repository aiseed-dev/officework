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
    pub font_panel: Option<gpui::Div>,
    pub size_panel: Option<gpui::Div>,
    pub style_panel: Option<gpui::Div>,
    pub symbol_panel: Option<gpui::Div>,
    pub proof_panel: Option<gpui::Div>,
}

impl Writer {
    /// パネルを全部組む(順番は view.rs にあった時のまま)。
    /// 色は render のテーマの束 — パネルが使う6つだけを受け取る
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
                    .child(div().w(px(64.0)).text_size(px(11.5))
                        .text_color(rgb(0x66707A)).child(SharedString::from(label.to_string())))
                    .child(div().flex_1().px_2().py_1().rounded_sm()
                        .border_1()
                        .border_color(if active { rgb(0x1B6E3C) } else { rgb(0xC6CDD3) })
                        .bg(gpui::white())
                        .text_size(px(12.5))
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(s)))
            };
            let btn = |id: &str, label: &str| {
                div().id(SharedString::from(id.to_string()))
                    .px_2p5().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                    .text_size(px(11.5)).cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF5EE)))
                    .child(SharedString::from(label.to_string()))
            };
            Some(div().absolute().left(px(16.0)).top(px(8.0)).w(px(430.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(field(ui::t!("検索"), &self.find_ed, self.find_field == 0)
                    .id("find-f").cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| { this.find_field = 0; cx.notify() })))
                .child(field(ui::t!("置換後"), &self.repl_ed, self.find_field == 1)
                    .id("find-r").cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| { this.find_field = 1; cx.notify() })))
                .child(div().flex().flex_row().gap_2()
                    .child(btn("f-next", ui::t!("次へ (Enter)"))
                        .on_click(cx.listener(|this, _, _, cx| { this.find_next(); cx.notify() })))
                    .child(btn("f-one", ui::t!("置換"))
                        .on_click(cx.listener(|this, _, _, cx| { this.replace_current(); cx.notify() })))
                    .child(btn("f-all", ui::t!("すべて置換"))
                        .on_click(cx.listener(|this, _, _, cx| { this.replace_all(); cx.notify() })))
                    .child(div().flex_1())
                    .child(btn("f-close", ui::t!("閉じる"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.find_open = false; cx.notify()
                        })))))
        };

        // ヘッダー・フッターの編集のパネル。開いている間、打鍵はここに入る
        let hf_panel = self.hf_edit.map(|footer| {
            let title = if footer { ui::t!("フッター") } else { ui::t!("ヘッダー") };
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
                    .text_size(px(11.5)).cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF5EE)))
                    .child(SharedString::from(label.to_string()))
            };
            let mut field = div().flex_1().px_2().py_1().rounded_sm()
                .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                .text_size(px(12.5)).flex().flex_col();
            for ln in shown.split('\n') {
                field = field.child(div().whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(ln.to_string())));
            }
            div().absolute().left(px(16.0)).top(px(8.0)).w(px(430.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(SharedString::from(ui::tf!("{}の編集 — 全ページ共通", title))))
                .child(field)
                .child(div().flex().flex_row().gap_2()
                    .child(btn("hf-num", ui::t!("ページ番号を挿入"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("pagenum", cx);
                            cx.notify()
                        })))
                    .child(div().flex_1())
                    .child(btn("hf-close", ui::t!("閉じる (Esc)"))
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
                let mut d = div().absolute().left(px(16.0)).bottom(px(16.0)).w(px(300.0))
                    .p_3().rounded_md().bg(rgb(0xFFF6E6))
                    .border_1().border_color(rgb(0xE8D5A8))
                    .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0x8A4B00))
                        .child(ui::t!("この段落のコメント(レビュー > コメント で編集)")));
                for (author, text) in cs {
                    d = d.child(div().mt_1p5().text_size(px(11.5)).text_color(rgb(0x5A4A28))
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
                .text_size(px(12.5)).flex().flex_col();
            for ln in t.split('\n') {
                field = field.child(div().whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(ln.to_string())));
            }
            Some(div().absolute().left(px(16.0)).bottom(px(16.0)).w(px(360.0))
                .p_3().rounded_md().bg(rgb(0xFFF6E6))
                .border_1().border_color(rgb(0xE8D5A8))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x8A4B00))
                    .child(ui::t!("コメント — 空にして閉じると外れる")))
                .child(field)
                .child(div().flex().flex_row()
                    .child(div().flex_1())
                    .child(div().id("cmt-close").px_2p5().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x8A4B00)).text_color(rgb(0x8A4B00))
                        .text_size(px(11.5)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xF7ECD8)))
                        .child(ui::t!("閉じる (Esc)"))
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
            Some(div().absolute().left(px(16.0)).top(px(8.0)).w(px(360.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("透かし — 空にして閉じると外れる")))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x165E83)).bg(gpui::white())
                    .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t)))
                .child(div().flex().flex_row()
                    .child(div().flex_1())
                    .child(div().id("wm-close").px_2p5().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x165E83)).text_color(rgb(0x165E83))
                        .text_size(px(11.5)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF2F7)))
                        .child(ui::t!("閉じる (Esc)"))
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
            // 何を掛けるのかを人の言葉で1行に
            let mut what: Vec<String> = Vec::new();
            if let Some(s) = d.size_pt {
                what.push(ui::tf!("大きさ {}pt", s.to_string()).to_string());
            }
            if let Some(f) = &d.font {
                what.push(ui::tf!("書体 {}", f.clone()).to_string());
            }
            if d.bold {
                what.push(ui::t!("太字").to_string());
            }
            if d.italic {
                what.push(ui::t!("斜体").to_string());
            }
            if d.underline {
                what.push(ui::t!("下線").to_string());
            }
            if let Some(c) = &d.color {
                what.push(ui::tf!("色 #{}", c.clone()).to_string());
            }
            if let Some(c) = &d.shade {
                what.push(ui::tf!("帯 #{}", c.clone()).to_string());
            }
            div().absolute().left(px(16.0)).top(px(8.0)).w(px(400.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("スタイルの新設 — 見た目に名前を付けます")))
                .child(div().text_size(px(11.0)).text_color(rgb(0x66707A))
                    .child(SharedString::from(ui::tf!("掛けるもの: {}", what.join("・")))))
                .child(div().flex().flex_row().gap_2().items_center()
                    .child(div().flex_1().px_2().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                        .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(t)))
                    .child(div().id("style-ok").px_2p5().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                        .text_size(px(11.5)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF5EE)))
                        .child(ui::t!("決める (Enter)"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.style_commit();
                            cx.notify()
                        }))))
                .child(div().text_size(px(11.0)).text_color(rgb(0x66707A))
                    .child(ui::t!("同じ名前があれば置き換えます。テンプレートに入るので、同じスタイルの所が一度に変わります")))
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
            let mut d = div().absolute().left(px(16.0)).top(px(8.0)).w(px(340.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("しおり — 名前を打って追加。押すとそこへ移る")))
                .child(div().flex().flex_row().gap_2().items_center()
                    .child(div().flex_1().px_2().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                        .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(t)))
                    .child(div().id("bm-add").px_2p5().py_1().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                        .text_size(px(11.5)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF5EE)))
                        .child(ui::t!("追加 (Enter)"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.bm_add();
                            cx.notify()
                        }))));
            if items.is_empty() {
                d = d.child(div().text_size(px(11.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("(まだしおりはありません)")));
            }
            for (i, (name, b0)) in items.into_iter().enumerate() {
                let name2 = name.clone();
                d = d.child(div().flex().flex_row().items_center().gap_2()
                    .child(div()
                        .id(SharedString::from(format!("bm-{i}")))
                        .flex_1().px_2().py_0p5().rounded_sm()
                        .text_size(px(12.5)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF2F7)))
                        .child(SharedString::from(name.clone()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.switch_target(Target::Body);
                            this.ed.move_to(b0, false);
                            this.follow_caret();
                            this.status = ui::tf!("しおり「{}」へ移りました", name).into();
                            cx.notify()
                        })))
                    .child(div()
                        .id(SharedString::from(format!("bmx-{i}")))
                        .px_1p5().py_0p5().rounded_sm()
                        .text_size(px(11.5)).text_color(rgb(0x9AA5AE)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xF6E5E2)).text_color(rgb(0xC0392B)))
                        .child("✕")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            for b in &mut this.doc.blocks {
                                if let kumihan::Block::Para(p) = b {
                                    p.bookmarks.retain(|x| *x != name2);
                                }
                            }
                            this.dirty = true;
                            this.status = ui::t!("しおりを外しました").into();
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
            let mut d = div().absolute().left(px(16.0)).top(px(8.0)).w(px(360.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("バージョン履歴 — 上書き保存のたびの控え(9世代まで)")));
            if items.is_empty() {
                d = d.child(div().text_size(px(11.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("(まだ控えはありません。上書き保存すると増えます)")));
            }
            for (i, (disp, q)) in items.into_iter().enumerate() {
                d = d.child(div()
                    .id(SharedString::from(format!("hist-{i}")))
                    .px_2().py_0p5().rounded_sm()
                    .text_size(px(12.5)).cursor_pointer()
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
            let mut d = div().absolute().left(px(16.0)).top(px(8.0)).w(px(420.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("チャット — 文書の隣の申し送り帳(.chat.txt)")));
            if lines.is_empty() {
                d = d.child(div().text_size(px(11.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("(まだ書き込みはありません)")));
            }
            for l in lines {
                d = d.child(div().text_size(px(12.0))
                    .whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(l)));
            }
            d = d.child(div().flex().flex_row().gap_2().items_center()
                .child(div().flex_1().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t)))
                .child(div().id("chat-send").px_2p5().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                    .text_size(px(11.5)).cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF5EE)))
                    .child(ui::t!("送信 (Enter)"))
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
            let title = ui::t!("パスワード — この文書は暗号化されています");
            Some(div().absolute().left(px(16.0)).top(px(8.0)).w(px(380.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(SharedString::from(title.to_string())))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(masked)))
                .child(div().text_size(px(10.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("方式は ECMA-376 Agile(AES-256)。\
                            Word や LibreOffice でも開けます。\
                            パスワードを忘れると誰にも開けません"))))
        };

        // URL のパネル(JS なしの閲覧の入口)
        let url_panel = if !self.url_open {
            None
        } else {
            let mut t = self.url_ed.text().to_string();
            let cur = self.url_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(16.0)).top(px(8.0)).w(px(460.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("URL を開く — Enter で取りに行く(JS は実行しません)")))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t))))
        };

        // 記入のパネル(HTML の form。欄を押して打ち、送信で送る)
        let fm_panel = if !self.fm_open || self.html_forms.is_empty() {
            None
        } else {
            let fm = self.html_forms[0].clone();
            let mut d = div().absolute().right(px(16.0)).top(px(8.0)).w(px(340.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("記入 — 欄を押して打ち、Enter で控え、送信で送る")));
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
                    .child(div().w(px(90.0)).text_size(px(11.5))
                        .text_color(rgb(0x66707A))
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(format!("{}{hint}", f.name))))
                    .child(div()
                        .id(SharedString::from(format!("fm-{i}")))
                        .flex_1().px_2().py_0p5().rounded_sm()
                        .border_1()
                        .border_color(if editing { rgb(0x1B6E3C) } else { rgb(0xC6CDD3) })
                        .bg(gpui::white())
                        .text_size(px(12.5)).cursor_pointer()
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
                    .text_size(px(12.0)).cursor_pointer()
                    .hover(|st| st.bg(rgb(0xEAF5EE)))
                    .child(ui::tf!("送信({} {})", fm.method.to_uppercase(), fm.action))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.fm_submit(cx);
                        cx.notify()
                    }))));
            Some(d)
        };

        // **外側の柱**(発注者 2026-08-15。calc と同じ作法)。
        // 面を切り替えるアイコンを縦に並べる
        let 柱 = || div().flex_none().w(px(RAIL)).h_full()
            .flex().flex_col().items_center().gap_1().py_1();
        // **柱の釦も場所を控える**(2026-08-16。点検の道具のため)。
        // 鍵は `&'static str` が要るので、呼ぶ側が静的な名前も渡す
        let boxes = self.btn_box.clone();
        let 柱釦 = move |id: String, icon: &'static str, 札: String, on: bool| {
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
                .tooltip(move |_, cx| cx.new(|_| crate::view::Tip(札.clone().into())).into())
                .tooltip_show_delay(std::time::Duration::from_millis(150))
                .child(gpui::svg()
                    .path(SharedString::from(format!("icons/{icon}.svg")))
                    .size(px(18.0))
                    .text_color(if on { th_btn } else { th_status }))
        };

        // 左パネル(本家のナビゲーション)。見出し / コメント / 検索 / AI
        let nav_panel = if !self.nav_open {
            None
        } else {
            let panel_bg = if dk { rgb(0x1B1E21) } else { rgb(0xF1F3F5) };
            let mut d = div()
                .flex_1().min_w(px(0.0)).h_full().overflow_hidden()
                .p_2()
                .flex().flex_col().gap_1();
            // **面の切り替えは外側の柱へ移した**(発注者 2026-08-15
            // 「左右のパネルの外側にアイコンをおいて操作を変更できるように」)。
            // 前は上に文字の耳が4つ並んでいて、その分だけ中身が狭かった。
            // 左は**対話する相手**(2026-08-14 の決め)— 見出し・コメント・
            // 検索・AI。照合は添字(nav_tab)
            match self.nav_tab {
                // 見出し(押すと飛ぶ)
                0 => {
                    let heads = self.headings();
                    if heads.is_empty() {
                        d = d.child(div().text_size(px(11.0)).text_color(th_status)
                            .child(ui::t!("(見出しがありません。ホーム > 段落のスタイルで)")));
                    }
                    for (i, (lv, text, byte)) in heads.into_iter().take(40).enumerate() {
                        let b = byte;
                        d = d.child(div()
                            .id(SharedString::from(format!("nav-{i}")))
                            .px_2().py_0p5().rounded_sm().cursor_pointer()
                            .text_size(px(12.0)).text_color(th_top_fg)
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
                        d = d.child(div().text_size(px(11.0)).text_color(th_status)
                            .child(ui::t!("(コメントはありません)")));
                    }
                    for (i, (_, who, text, byte)) in items.into_iter().take(30).enumerate() {
                        let b = byte;
                        d = d.child(div()
                            .id(SharedString::from(format!("navc-{i}")))
                            .px_2().py_1().rounded_sm().cursor_pointer()
                            .bg(if dk { rgb(0x22262A) } else { rgb(0xFFFFFF) })
                            .hover(|st| st.bg(th_btn_hover))
                            .flex().flex_col()
                            .child(div().text_size(px(10.5)).text_color(th_status)
                                .child(SharedString::from(who)))
                            .child(div().text_size(px(11.5)).text_color(th_top_fg)
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
                        .text_size(px(12.0)).text_color(th_top_fg)
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(if term.is_empty() {
                            ui::t!("(検索のパネルで語を打つ → ここに出ます)").to_string()
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
                                .text_size(px(11.5)).text_color(th_top_fg)
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
                            d = d.child(div().text_size(px(11.0)).text_color(th_status)
                                .child(ui::t!("(見つかりません)")));
                        }
                    }
                }
                // ── AI と相談する ─────────────────────────────────
                // **答えは文書に入れない。** 直した文は下の欄に置き、
                // 「入れる」を押して初めて文書が変わる。writer には
                // calc のような Python の橋が無いので、入るのは**文そのもの**
                _ => {
                    let 主 = rgb(0x165E83);
                    let 釦 = move |id: &'static str, label: String, 効き: bool| {
                        div().id(SharedString::from(id))
                            .px_2().py_0p5().rounded_sm().cursor_pointer()
                            .text_size(px(11.5))
                            .text_color(if 効き { th_top_fg } else { th_status })
                            .border_1()
                            .border_color(if 効き { 主 } else { th_cmd_border })
                            .hover(move |st| st.bg(th_btn_hover))
                            .child(SharedString::from(label))
                    };
                    d = d.child(div().text_size(px(10.5)).text_color(th_status).child(
                        ui::t!("選んだところについて聞けます。文を直すときは、\
                                直した文を先に見せます — 押すまで入りません。").to_string()));
                    let mut 会話 = div().id("ai-chat-log").flex().flex_col().gap_1().mt_1()
                        .flex_1().min_h(px(0.0)).overflow_y_scroll();
                    if self.ai_chat_log.is_empty() {
                        会話 = 会話.child(div().text_size(px(11.0)).text_color(th_status)
                            .child(ui::t!("例: この段落を敬語にして / 半分の長さに \
                                           / 言い方が硬くないか見て").to_string()));
                    }
                    for (自分, 字) in &self.ai_chat_log {
                        会話 = 会話.child(div().text_size(px(11.5))
                            .text_color(if *自分 { 主 } else { th_top_fg })
                            .child(format!("{} {}", if *自分 { "▸" } else { "◂" }, 字)));
                    }
                    d = d.child(会話);
                    if let Some(plan) = self.ai_chat_plan.clone() {
                        d = d.child(div().text_size(px(10.5)).text_color(th_status).mt_1()
                            .child(ui::t!("入れる文(押すまで入りません)").to_string()));
                        d = d.child(div().id("ai-chat-plan")
                            .max_h(px(160.0)).overflow_y_scroll()
                            .p_1().rounded_sm()
                            .bg(if dk { rgb(0x14171A) } else { rgb(0xFFFFFF) })
                            .border_1().border_color(th_cmd_border)
                            .text_size(px(11.0)).text_color(th_top_fg)
                            .children(plan.lines().map(|l| div().child(l.to_string()))));
                        d = d.child(div().flex().flex_row().gap_1().mt_1()
                            .child(釦("ai-chat-run", ui::t!("入れる").to_string(), true)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ai_chat_insert();
                                    cx.notify()
                                })))
                            .child(釦("ai-chat-drop", ui::t!("やめる").to_string(), false)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ai_chat_plan = None;
                                    this.status =
                                        ui::t!("入れる文を捨てました(何もしていません)").into();
                                    cx.notify()
                                }))));
                    }
                    d = d.child(div()
                        .id("ai-chat-in")
                        .mt_1().p_1().rounded_sm().cursor_text()
                        .bg(if dk { rgb(0x14171A) } else { rgb(0xFFFFFF) })
                        .border_1()
                        .border_color(if self.ai_chat_focus { 主 } else { th_cmd_border })
                        .text_size(px(11.5)).text_color(th_top_fg)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.ai_chat_focus = true;
                            cx.notify()
                        }))
                        .child(if self.ai_chat_in.text().is_empty() {
                            if self.ai_chat_focus {
                                "|".to_string()
                            } else {
                                ui::t!("ここを押して書き、Enter で送る").to_string()
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
                    r = r.child(釦("ai-chat-send", ui::t!("送る").to_string(), !self.ai_busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.ai_chat_send(cx);
                            cx.notify()
                        })));
                    if self.ai_busy {
                        r = r.child(div().text_size(px(10.5)).text_color(th_status)
                            .child(ui::t!("考えています…").to_string()));
                    }
                    d = d.child(r);
                }
            }
            let 柱d = 柱()
                .child(柱釦("nf-head".into(), "contents", ui::t!("見出し").to_string(), self.nav_tab == 0).on_click(
                    cx.listener(|t, _, _, cx| { t.nav_tab = 0; cx.notify() })))
                .child(柱釦("nf-cmt".into(), "co-showcomment", ui::t!("コメント").to_string(), self.nav_tab == 1).on_click(
                    cx.listener(|t, _, _, cx| { t.nav_tab = 1; cx.notify() })))
                .child(柱釦("nf-find".into(), "replace", ui::t!("検索").to_string(), self.nav_tab == 2).on_click(
                    cx.listener(|t, _, _, cx| { t.nav_tab = 2; cx.notify() })))
                .child(柱釦("nf-ai".into(), "ai-ask", ui::t!("AI").to_string(), self.nav_tab == 3).on_click(
                    cx.listener(|t, _, _, cx| { t.nav_tab = 3; cx.notify() })));
            Some(div()
                .flex_none().w(px(250.0 + RAIL)).h_full()
                .m_1().rounded_sm().bg(panel_bg)
                .border_1().border_color(th_cmd_border)
                .flex().flex_row()
                .child(柱d)
                .child(div().flex_none().w(px(1.0)).h_full().bg(th_cmd_border))
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
            let size_now = self.doc.size_at(self.ed.selection()).unwrap_or(SIZE_PT);
            let head = |t: &'static str| {
                div().text_size(px(11.0)).font_weight(gpui::FontWeight::BOLD)
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
                    .text_size(px(11.5))
                    .text_color(if on { th_btn } else { th_top_fg })
                    .hover(move |st| st.bg(th_btn_hover))
                    .child(label)
            };
            let row = || div().flex().flex_row().flex_wrap().gap_1();
            // 左と同じく**場所を取る**(重ねない)。巻けるようにもする —
            // 表の面が足された分、230px の幅では下が切れる
            let 面 = self.rp_tab;
            // **どの枝が組んだか**を控える(値を読んだだけでは分からない)
            self.rp_drawn.set(9);
            let return_rp;
            let mut d = div().id("rp-panel")
                .flex_1().min_w(px(0.0)).h_full().overflow_y_scroll()
                .p_2()
                .flex().flex_col().gap_1()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(match 面 {
                        1 => ui::t!("ページ — 文書ぜんぶの決め"),
                        2 => ui::t!("スタイル — テンプレートを直す"),
                        3 => ui::t!("ファイル — フォルダの中身"),
                        _ => ui::t!("設定 — いる場所を直す"),
                    }));
            // **ページは「いる場所」ではない。** 文書ぜんぶに掛かる決めなので、
            // 柱で別の面に分けた(発注者 2026-08-15「外側にアイコンをおいて
            // 操作を変更できるように」)
            if 面 == 1 {
                self.rp_drawn.set(1);
                d = d.child(div().text_size(px(11.0)).text_color(th_status)
                    .child(SharedString::from(ui::tf!("{:.0}×{:.0}mm / 余白 {:.0}mm / {}段{}", self.pg.w_mm, self.pg.h_mm, self.pg.left_mm, self.pg.cols(), if self.doc.vertical { ui::t!(" / 縦書き") } else { "" }))));
                d = d.child(row()
                    .child(btn(self, "pageorient", ui::t!("向き").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("pageorient", cx); cx.notify() })))
                    .child(btn(self, "pagesize", ui::t!("用紙").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("pagesize", cx); cx.notify() })))
                    .child(btn(self, "pagemargins", ui::t!("余白").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("pagemargins", cx); cx.notify() })))
                    .child(btn(self, "columns", ui::t!("段組み").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("columns", cx); cx.notify() })))
                    .child(btn(self, "direction", ui::t!("縦書き").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("direction", cx); cx.notify() }))));
                d = d.child(head(ui::t!("ヘッダーとフッター")));
                d = d.child(row()
                    .child(btn(self, "edit-header", ui::t!("編集").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("edit-header", cx); cx.notify() })))
                    .child(btn(self, "pagenum", ui::t!("ページ番号").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("pagenum", cx); cx.notify() })))
                    .child(btn(self, "watermark", ui::t!("透かし").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("watermark", cx); cx.notify() })))
                    .child(btn(self, "pagecolor", ui::t!("ページの色").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("pagecolor", cx); cx.notify() }))));
                let 柱d = 柱()
                    .child(柱釦("rf-here".into(), "format", ui::t!("設定 — いる場所を直す").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 0; cx.notify() })))
                    .child(柱釦("rf-page".into(), "pagesize", ui::t!("ページ — 文書ぜんぶの決め").to_string(), true).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 1; cx.notify() })))
                    // **フォルダのファイル一覧**(2026-08-19 発注者
                    // 「フォルダー内のファイル一覧を右パネルに表示」)
                    .child(柱釦("rf-files".into(), "py-folder", ui::t!("ファイル — フォルダの中身").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 3; cx.notify() })));
                return_rp = Some(div()
                    .flex_none().w(px(230.0 + RAIL)).h_full()
                    .m_1().rounded_sm().bg(panel_bg)
                    .border_1().border_color(th_cmd_border)
                    .flex().flex_row()
                    .child(d)
                    .child(div().flex_none().w(px(1.0)).h_full().bg(th_cmd_border))
                    .child(柱d));
            } else if 面 == 2 {
                self.rp_drawn.set(2);
            // **スタイルの面**(2026-08-16。ネイティブ文書だけ)。
            // いまの段落が着ているスタイルと、テンプレートの一覧を出す。
            // 押すと着替え、直すとテンプレートが変わって**同じスタイルの所が
            // 一度に変わる** — ライブ合成の効き目がここに出る
                let 着ている = para
                    .as_ref()
                    .and_then(|p| {
                        p.style_id.clone().or_else(|| {
                            kumihan::theme::Theme::role_name(p.style).map(|s| s.to_string())
                        })
                    })
                    .unwrap_or_else(|| ui::t!("本文").to_string());
                d = d.child(div().text_size(px(11.0)).text_color(th_status).child(
                    SharedString::from(ui::tf!("いまの段落: {}", 着ている.clone())),
                ));
                // 役割のスタイル(段落そのものの意味)は先に、名前つきは後に
                let mut names: Vec<String> =
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
                        .text_size(px(11.5)).text_color(th_status)
                        .hover(move |st| st.bg(th_btn_hover))
                        .child(SharedString::from(ui::t!("なし").to_string()))
                        .on_click(cx.listener(|t, _, _, cx| {
                            t.strip_style();
                            cx.notify()
                        })),
                );
                for name in names {
                    let on = name == 着ている;
                    let n2 = name.clone();
                    r = r.child(
                        div()
                            .id(SharedString::from(format!("rp-st-{name}")))
                            .px_2().py_0p5().rounded_sm().cursor_pointer()
                            .border_1()
                            .border_color(if on { th_btn } else { th_cmd_border })
                            .bg(if on { th_btn_hover } else { gpui::transparent_black().into() })
                            .text_size(px(11.5))
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
                // いま着ているスタイルの中身(テンプレートが持っている値)
                if let Some(def) = self.tmpl.style(&着ている) {
                    let mut w: Vec<String> = Vec::new();
                    if let Some(s) = def.size_pt {
                        w.push(ui::tf!("大きさ {}pt", s.to_string()).to_string());
                    }
                    if let Some(f) = &def.font {
                        w.push(ui::tf!("書体 {}", f.clone()).to_string());
                    }
                    if def.bold {
                        w.push(ui::t!("太字").to_string());
                    }
                    if def.italic {
                        w.push(ui::t!("斜体").to_string());
                    }
                    if def.underline {
                        w.push(ui::t!("下線").to_string());
                    }
                    if let Some(c) = &def.color {
                        w.push(ui::tf!("色 #{}", c.clone()).to_string());
                    }
                    d = d.child(head(ui::t!("このスタイルの中身")));
                    d = d.child(div().text_size(px(11.0)).text_color(th_status).child(
                        SharedString::from(if w.is_empty() {
                            ui::t!("(文書の既定のまま)").to_string()
                        } else {
                            w.join("・")
                        }),
                    ));
                    d = d.child(row()
                        .child(btn(self, "st-bigger", ui::t!("字を大きく").into()).on_click(
                            cx.listener(|t, _, _, cx| { t.tweak_style(1); cx.notify() })))
                        .child(btn(self, "st-smaller", ui::t!("字を小さく").into()).on_click(
                            cx.listener(|t, _, _, cx| { t.tweak_style(-1); cx.notify() }))));
                }
                d = d.child(div().text_size(px(11.0)).text_color(th_status)
                    .child(ui::t!("直すとテンプレートが変わり、同じスタイルの所が一度に変わります")));
                let 柱d = 柱()
                    .child(柱釦("rf-here".into(), "format", ui::t!("設定 — いる場所を直す").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 0; cx.notify() })))
                    .child(柱釦("rf-page".into(), "pagesize", ui::t!("ページ — 文書ぜんぶの決め").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 1; cx.notify() })))
                    .child(柱釦("rf-style".into(), "styles", ui::t!("スタイル — テンプレートを直す").to_string(), true).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 2; cx.notify() })))
                    // **フォルダのファイル一覧**(2026-08-19 発注者
                    // 「フォルダー内のファイル一覧を右パネルに表示」)
                    .child(柱釦("rf-files".into(), "py-folder", ui::t!("ファイル — フォルダの中身").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 3; cx.notify() })));
                return_rp = Some(div()
                    .flex_none().w(px(230.0 + RAIL)).h_full()
                    .m_1().rounded_sm().bg(panel_bg)
                    .border_1().border_color(th_cmd_border)
                    .flex().flex_row()
                    .child(d)
                    .child(div().flex_none().w(px(1.0)).h_full().bg(th_cmd_border))
                    .child(柱d));
            } else if 面 == 3 {
                // **フォルダの中身**(2026-08-19 発注者)。選ぶと開きます。
                // 種類はファイルの名前で決まります(二重の拡張子)
                self.rp_drawn.set(3);
                match self.folder() {
                    None => {
                        d = d.child(div().text_size(px(11.0)).text_color(th_status)
                            .child(ui::t!("フォルダを開いていません(ファイル > 開く)")));
                    }
                    Some(dir) => {
                        // **フォルダの名前だけ**を出します。長い径路を全部
                        // 出すと3行に折り返して、一覧の場所を食います
                        let 名 = dir.file_name().map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| dir.display().to_string());
                        d = d.child(div().text_size(px(10.5)).text_color(th_status)
                            .child(SharedString::from(名)));
                        let 一覧 = ui::folder::list(&dir);
                        if 一覧.is_empty() {
                            d = d.child(div().text_size(px(11.0)).text_color(th_status)
                                .child(ui::t!("(空のフォルダです)")));
                        }
                        for (i, e) in 一覧.into_iter().take(200).enumerate() {
                            // **writer が開けるのは文書だけ**です。表は
                            // 一覧に出しますが、まだ押せません(画面を
                            // 切り替える仕組みは「画面を1つにする」4段目)。
                            // できないことを、できるように見せない
                            let 開ける = e.kind.is_doc();
                            let 道 = e.path.clone();
                            let 札 = e.kind.label().to_string();
                            let いま = self.path.as_deref() == Some(e.path.as_path());
                            let mut 行 = div()
                                .id(SharedString::from(format!("fl-{i}")))
                                .px_1().py_0p5().rounded_sm()
                                .flex().flex_row().items_center().gap_1()
                                .bg(if いま { th_btn_hover } else { gpui::transparent_black().into() })
                                .child(div().flex_1().min_w(px(0.0)).text_size(px(11.5))
                                    .text_color(if 開ける { th_top_fg } else { th_status })
                                    .child(SharedString::from(e.name.clone())))
                                .child(div().flex_none().text_size(px(9.0)).text_color(th_status)
                                    .child(SharedString::from(札)));
                            // **開けない物は押せません。** できないことを、
                            // できるように見せない
                            if 開ける {
                                行 = 行.cursor_pointer()
                                    .hover(move |s| s.bg(th_btn_hover))
                                    .on_click(cx.listener(move |t, _, _, cx| {
                                        t.open(道.clone());
                                        t.remember_folder();
                                        cx.notify()
                                    }));
                            }
                            d = d.child(行);
                        }
                    }
                }
                let 柱d = 柱()
                    .child(柱釦("rf-here".into(), "format", ui::t!("設定 — いる場所を直す").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 0; cx.notify() })))
                    .child(柱釦("rf-page".into(), "pagesize", ui::t!("ページ — 文書ぜんぶの決め").to_string(), false).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 1; cx.notify() })))
                    .child(柱釦("rf-files".into(), "py-folder", ui::t!("ファイル — フォルダの中身").to_string(), true).on_click(
                        cx.listener(|t, _, _, cx| { t.rp_tab = 3; cx.notify() })));
                return_rp = Some(div()
                    .flex_none().w(px(230.0 + RAIL)).h_full()
                    .m_1().rounded_sm().bg(panel_bg)
                    .border_1().border_color(th_cmd_border)
                    .flex().flex_row()
                    .child(d.overflow_y_scroll())
                    .child(div().flex_none().w(px(1.0)).h_full().bg(th_cmd_border))
                    .child(柱d));
            } else {

            // **いる場所に追従する。** 表の中なら表の面、段落に数式や画像が
            // あればその面を、文字・段落・ページの前に出す
            // (発注者 2026-08-14「選んでいる物の設定に切り替わるように」)。
            // 出すだけで下の面も残す — 表の中でも字は太字にしたい
            if let Some((_, 行, 列, 行数, 列数)) = self.cursor_table() {
                d = d.child(head(ui::t!("表")));
                d = d.child(div().text_size(px(11.0)).text_color(th_status)
                    .child(SharedString::from(ui::tf!(
                        "{}行 × {}列 — いま {}行目 {}列目", 行数, 列数, 行 + 1, 列 + 1))));
                d = d.child(row()
                    .child(btn(self, "tb-row-up", ui::t!("上に行").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_add_row(false); cx.notify() })))
                    .child(btn(self, "tb-row-dn", ui::t!("下に行").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_add_row(true); cx.notify() })))
                    .child(btn(self, "tb-row-del", ui::t!("行を消す").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_del_row(); cx.notify() }))));
                d = d.child(row()
                    .child(btn(self, "tb-col-l", ui::t!("左に列").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_add_col(false); cx.notify() })))
                    .child(btn(self, "tb-col-r", ui::t!("右に列").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_add_col(true); cx.notify() })))
                    .child(btn(self, "tb-col-del", ui::t!("列を消す").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.table_del_col(); cx.notify() }))));
            }
            // 数式と画像は**段落が持つ**(writer に図形の選択という状態は無い)。
            // 数式は絵だが `tex` に原文を積んであるので、直せる
            if let Some(p0) = &para {
                let 絵: Vec<&kumihan::InlineImage> =
                    p0.images.iter().chain(p0.images_new.iter()).collect();
                let 式: Vec<&&kumihan::InlineImage> =
                    絵.iter().filter(|im| im.tex.is_some()).collect();
                if let Some(im) = 式.first() {
                    let tex = im.tex.clone().unwrap_or_default();
                    d = d.child(head(ui::t!("数式")));
                    d = d.child(div().text_size(px(10.5)).text_color(th_status)
                        .child(SharedString::from(tex.clone())));
                    d = d.child(row().child(
                        btn(self, "eq-edit", ui::t!("式を直す").into()).on_click(
                            cx.listener(move |t, _, _, cx| {
                                // 原文を欄に載せて開く。**打ち直しにしない**
                                t.eq_ed = Editor::new(&tex);
                                t.eq_open = true;
                                t.status = ui::t!("式を直します(Enter で組み直し・Esc で取りやめ)").into();
                                cx.notify()
                            }))));
                } else if let Some(im) = 絵.first() {
                    d = d.child(head(ui::t!("画像")));
                    d = d.child(div().text_size(px(11.0)).text_color(th_status)
                        .child(SharedString::from(
                            ui::tf!("{:.0}×{:.0}mm", im.w_mm, im.h_mm))));
                    d = d.child(row()
                        .child(btn(self, "img-small", ui::t!("小さく").into()).on_click(
                            cx.listener(|t, _, _, cx| { t.image_scale(0.9); cx.notify() })))
                        .child(btn(self, "img-big", ui::t!("大きく").into()).on_click(
                            cx.listener(|t, _, _, cx| { t.image_scale(1.1); cx.notify() }))));
                }
            }

            // 文字
            d = d.child(head(ui::t!("文字")))
                .child(row()
                    .child(btn(self, "bold", ui::t!("太字").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("bold", cx); cx.notify() })))
                    .child(btn(self, "italic", ui::t!("斜体").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("italic", cx); cx.notify() })))
                    .child(btn(self, "underline", ui::t!("下線").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("underline", cx); cx.notify() })))
                    .child(btn(self, "strikeout", ui::t!("取消").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("strikeout", cx); cx.notify() }))))
                .child(row()
                    .child(div().text_size(px(11.0)).text_color(th_status)
                        .child(SharedString::from(ui::tf!("大きさ {} pt / 書体 {}", if size_now.fract() == 0.0 {
                                format!("{}", size_now as i32)
                            } else {
                                format!("{size_now}")
                            }, self.font_name)))))
                .child(row()
                    .child(btn(self, "decfont", ui::t!("小さく").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("decfont", cx); cx.notify() })))
                    .child(btn(self, "incfont", ui::t!("大きく").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("incfont", cx); cx.notify() })))
                    .child(btn(self, "fontcolor", ui::t!("色").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("fontcolor", cx); cx.notify() })))
                    .child(btn(self, "clearstyle", ui::t!("書式を消す").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("clearstyle", cx); cx.notify() }))));
            if f.field.is_some() {
                d = d.child(div().text_size(px(10.5)).text_color(th_status)
                    .child(ui::t!("(ここは相互参照。更新は 参考資料 > 相互参照)")));
            }
            if let Some(rt) = &f.ruby {
                d = d.child(div().text_size(px(10.5)).text_color(th_status)
                    .child(SharedString::from(ui::tf!("ルビ「{}」", rt))));
            }

            // 段落
            let (al, ls, ind, lst) = match &para {
                Some(p) => (p.align, p.spacing(), p.indent, p.list),
                None => (Align::Left, 1.0, 0, ListKind::None),
            };
            d = d.child(head(ui::t!("段落")))
                .child(row()
                    .children([
                        ("align-left", ui::t!("左"), Align::Left),
                        ("align-center", ui::t!("中央"), Align::Center),
                        ("align-right", ui::t!("右"), Align::Right),
                        ("align-just", ui::t!("両端"), Align::Justify),
                        ("align-dist", ui::t!("均等"), Align::Distribute),
                    ].map(|(id, label, a)| {
                        let on = al == a;
                        div().id(SharedString::from(format!("rp-{id}")))
                            .px_2().py_0p5().rounded_sm().cursor_pointer()
                            .border_1()
                            .border_color(if on { th_btn } else { th_cmd_border })
                            .bg(if on { th_btn_hover } else { gpui::transparent_black().into() })
                            .text_size(px(11.5))
                            .text_color(if on { th_btn } else { th_top_fg })
                            .hover(move |st| st.bg(th_btn_hover))
                            .child(label)
                            .on_click(cx.listener(move |t, _, _, cx| {
                                t.run_cmd(id, cx);
                                cx.notify()
                            }))
                    })))
                .child(row()
                    .child(div().text_size(px(11.0)).text_color(th_status)
                        .child(SharedString::from(ui::tf!("行間 {:.2} / 字下げ {}", ls, ind)))))
                .child(row()
                    .child(btn(self, "linespace", ui::t!("行間").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("linespace", cx); cx.notify() })))
                    .child(btn(self, "decoffset", ui::t!("◂ 字下げ").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("decoffset", cx); cx.notify() })))
                    .child(btn(self, "incoffset", ui::t!("字下げ ▸").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("incoffset", cx); cx.notify() }))))
                .child(row()
                    // **✓ は見出しだけ** — 鍵は素のまま(calc の freeze と同じ作法)
                    .child(btn(self, "markers", {
                        let l = ui::t!("箇条書き");
                        if lst == ListKind::Bullet { format!("{l} ✓").into() } else { l.into() }
                    }).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("markers", cx); cx.notify() })))
                    .child(btn(self, "numbering", {
                        let l = ui::t!("番号");
                        if lst == ListKind::Number { format!("{l} ✓").into() } else { l.into() }
                    }).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("numbering", cx); cx.notify() })))
                    .child(btn(self, "paracolor", ui::t!("背景").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("paracolor", cx); cx.notify() })))
                    .child(btn(self, "borders", ui::t!("囲み").into()).on_click(cx.listener(
                        |t, _, _, cx| { t.run_cmd("borders", cx); cx.notify() }))));

            // ページ
            let 柱d = 柱()
                .child(柱釦("rf-here".into(), "format", ui::t!("設定 — いる場所を直す").to_string(), 面 == 0).on_click(
                    cx.listener(|t, _, _, cx| { t.rp_tab = 0; cx.notify() })))
                .child(柱釦("rf-page".into(), "pagesize", ui::t!("ページ — 文書ぜんぶの決め").to_string(), 面 == 1).on_click(
                    cx.listener(|t, _, _, cx| { t.rp_tab = 1; cx.notify() })))
                // **スタイルの面はネイティブ文書だけ**(2026-08-16)。互換の
                // 文書にはテンプレートが無く、押しても見せる物が無い —
                // できないことを、できるように見せない
                .children(self.native.then(|| {
                    柱釦("rf-style".into(), "styles", ui::t!("スタイル — テンプレートを直す").to_string(), 面 == 2)
                        .on_click(cx.listener(|t, _, _, cx| { t.rp_tab = 2; cx.notify() }))
                }))
                // **フォルダのファイル一覧**(2026-08-19)
                .child(柱釦("rf-files".into(), "py-folder", ui::t!("ファイル — フォルダの中身").to_string(), false).on_click(
                    cx.listener(|t, _, _, cx| { t.rp_tab = 3; cx.notify() })));
            return_rp = Some(div()
                .flex_none().w(px(230.0 + RAIL)).h_full()
                .m_1().rounded_sm().bg(panel_bg)
                .border_1().border_color(th_cmd_border)
                .flex().flex_row()
                .child(d)
                .child(div().flex_none().w(px(1.0)).h_full().bg(th_cmd_border))
                .child(柱d));
            }
            return_rp
        };

        // リンクのパネル(押すと辿る。公開 Web も見える — JS は実行しない)
        let lk_panel = if !self.lk_open || self.html_links.is_empty() {
            None
        } else {
            let mut d = div().absolute().right(px(16.0)).bottom(px(8.0)).w(px(340.0))
                .max_h(px(300.0)).overflow_hidden()
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_1()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(SharedString::from(ui::tf!("リンク({}件。押すと辿る。Esc で閉じる)", self.html_links.len()))));
            for (i, (href, text)) in self.html_links.iter().take(16).enumerate() {
                let href2 = href.clone();
                d = d.child(div()
                    .id(SharedString::from(format!("lk-{i}")))
                    .px_2().py_0p5().rounded_sm().cursor_pointer()
                    .text_size(px(12.0)).text_color(rgb(0x165E83))
                    .whitespace_nowrap().overflow_hidden()
                    .hover(|st| st.bg(rgb(0xEAF2F7)))
                    .child(SharedString::from(text.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.follow_link(href2.clone(), cx);
                        cx.notify()
                    })));
            }
            if self.html_links.len() > 16 {
                d = d.child(div().text_size(px(10.5)).text_color(rgb(0x66707A))
                    .child(SharedString::from(ui::tf!("(あと {} 件は出していません)", self.html_links.len() - 16))));
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
            Some(div().absolute().left(px(16.0)).top(px(8.0)).w(px(460.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(SharedString::from(ui::tf!("{} — 宛先 {}(Esc で取りやめ)", if self.ai_macro { ui::t!("AI にマクロ台本を頼む") } else { ui::t!("AI に頼む") }, ui::ai::backend().label()))))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(rgb(0xFFFFFF))
                    .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t)))
                .child(div().text_size(px(10.5)).text_color(rgb(0x66707A))
                    .child(if self.ai_macro {
                        ui::t!("台本はプラグイン置き場に置くだけです。読んで確かめてから\
                         一覧で実行してください(自動では走りません)")
                    } else {
                        ui::t!("答えはカーソルの位置に入ります。Ctrl+Z で1手で戻せます")
                    })))
        };

        // 記入欄の選択肢を聞くパネル
        let sd_panel = if !self.sd_open {
            None
        } else {
            let mut t = self.sd_ed.text().to_string();
            let cur = self.sd_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(16.0)).top(px(8.0)).w(px(400.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(SharedString::from(if self.sd_naming {
                        ui::t!("記入欄の名前 — 打って Enter(例: 氏名。\
                         マクロの fill(名前, 値) が引く鍵)").to_string()
                    } else {
                        ui::tf!("{}の選択肢 — カンマ区切りで打って Enter(例: 赤,青,黄)", self.sd_kind.label())
                    })))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(rgb(0xFFFFFF))
                    .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t))))
        };

        // ルビのパネル(読みの入力)
        let rb_panel = if !self.rb_open {
            None
        } else {
            let mut t = self.rb_ed.text().to_string();
            let cur = self.rb_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(16.0)).top(px(8.0)).w(px(360.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("ルビ — 読みを打って Enter(空で外す。Esc で取りやめ)")))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t))))
        };

        // 数式のパネル(LaTeX を打つ)。**組むのは Python** — TeX があれば
        // そちらで組み、無ければ matplotlib。打った原文は絵と一緒に残る
        let eq_panel = if !self.eq_open {
            None
        } else {
            let mut t = self.eq_ed.text().to_string();
            let cur = self.eq_ed.cursor().min(t.len());
            t.insert(cur, '|');
            Some(div().absolute().left(px(16.0)).top(px(8.0)).w(px(460.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("数式 — LaTeX を打って Enter(Esc で取りやめ)")))
                .child(div().px_2().py_1().rounded_sm()
                    .border_1().border_color(rgb(0x1B6E3C)).bg(gpui::white())
                    .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(t)))
                .child(div().text_size(px(10.5)).text_color(rgb(0x60707C))
                    .child(ui::t!("例: \\frac{a+b}{2} / \\sqrt{x^2+y^2}"))))
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
            let mut d = div().absolute().left(px(16.0)).top(px(8.0)).w(px(420.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x165E83))
                    .child(ui::t!("プラグイン — 押すとサンドボックス(bubblewrap)の中で実行")))
                .child(div().text_size(px(11.0)).text_color(rgb(0x66707A))
                    .child(SharedString::from(ui::tf!("置き場: {}", dir.display()))));
            if items.is_empty() {
                d = d.child(div().text_size(px(11.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("(まだありません。置き場に .py を置いてください。\
                            台本の d が python-docx の文書、fill(名前, 値)・\
                            extract(名前)・fields() で記入欄の出し入れ)")));
            }
            for (i, q) in items.into_iter().enumerate() {
                let name = q
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                d = d.child(div()
                    .id(SharedString::from(format!("plug-{i}")))
                    .px_2().py_0p5().rounded_sm()
                    .text_size(px(12.5)).cursor_pointer()
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
            let mut d = div().absolute().left(px(16.0)).top(px(8.0)).w(px(360.0))
                .p_3().rounded_md().bg(rgb(0xF7F9FA))
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_2()
                .child(div().flex().flex_row().items_center()
                    .child(div().flex_1().text_size(px(11.5))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0x165E83))
                        .child(ui::t!("相互参照 — しおりの文字かページ番号を挿す")))
                    .child(div().id("xr-refresh").px_2().py_0p5().rounded_sm()
                        .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                        .text_size(px(11.0)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF5EE)))
                        .child(ui::t!("参照を更新"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_refs();
                            cx.notify()
                        }))));
            if names.is_empty() {
                d = d.child(div().text_size(px(11.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("(しおりがありません。参考資料 > ブックマークで付けてください)")));
            }
            for (i, name) in names.into_iter().enumerate() {
                let n1 = name.clone();
                let n2 = name.clone();
                d = d.child(div().flex().flex_row().items_center().gap_2()
                    .child(div().flex_1().text_size(px(12.5))
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(name)))
                    .child(div().id(SharedString::from(format!("xrt-{i}")))
                        .px_2().py_0p5().rounded_sm()
                        .border_1().border_color(rgb(0x165E83)).text_color(rgb(0x165E83))
                        .text_size(px(11.0)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF2F7)))
                        .child(ui::t!("文字"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.insert_ref(&n1, false);
                            cx.notify()
                        })))
                    .child(div().id(SharedString::from(format!("xrp-{i}")))
                        .px_2().py_0p5().rounded_sm()
                        .border_1().border_color(rgb(0x165E83)).text_color(rgb(0x165E83))
                        .text_size(px(11.0)).cursor_pointer()
                        .hover(|s| s.bg(rgb(0xEAF2F7)))
                        .child(ui::t!("ページ"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.insert_ref(&n2, true);
                            cx.notify()
                        }))));
            }
            Some(d)
        };

        // フォントの一覧。この機械にある日本語の書体だけ
        let font_panel = if !self.font_list {
            None
        } else {
            let names: Vec<String> = kumihan::font::list()
                .iter()
                .filter(|f| f.japanese && f.regular)
                .map(|f| f.name.clone())
                .take(24)
                .collect();
            let mut d = div().absolute().left(px(16.0)).top(px(8.0)).w(px(280.0))
                .p_2().rounded_md().bg(gpui::white())
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_0p5()
                .child(div().text_size(px(10.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("書体(選んだ段落に掛かる)")));
            for name in names {
                let shown = SharedString::from(name.clone());
                let is_current = self.font_name.as_ref() == name.as_str();
                d = d.child(div()
                    .id(SharedString::from(format!("font-{name}")))
                    .px_2().py_0p5().rounded_sm()
                    .text_size(px(12.5))
                    .font_family(shown.clone())
                    .bg(if is_current { rgb(0xEAF5EE) } else { rgb(0xFFFFFF) })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF2F7)))
                    .child(shown)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let n = name.clone();
                        let sel = this.ed.selection();
                        this.flush_target();
                        this.doc.apply_font(sel, Some(n.clone()));
                        this.dirty = true;
                        this.relayout_keep();
                        this.font_list = false;
                        this.status = ui::tf!("書体を「{}」に", n).into();
                        cx.notify();
                    })));
            }
            Some(d)
        };

        // 大きさの一覧
        let size_panel = if !self.size_list {
            None
        } else {
            let mut d = div().absolute().left(px(16.0)).top(px(8.0)).w(px(200.0))
                .p_2().rounded_md().bg(gpui::white())
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_row().flex_wrap().gap_1();
            for pt in [8.0f32, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 22.0, 26.0, 36.0] {
                d = d.child(div()
                    .id(SharedString::from(format!("pt-{pt}")))
                    .px_2().py_1().rounded_sm().text_size(px(12.0))
                    .cursor_pointer().hover(|s| s.bg(rgb(0xEAF2F7)))
                    .child(SharedString::from(format!("{pt}")))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let sel = this.ed.selection();
                        this.flush_target();
                        this.doc.apply_size(sel, move |_| pt);
                        this.dirty = true;
                        this.relayout_keep();
                        this.size_list = false;
                        this.status = ui::tf!("大きさを {}pt に", pt).into();
                        cx.notify();
                    })));
            }
            Some(d)
        };

        // 段落のスタイルの一覧(標準・見出し1〜3)
        let style_panel = if !self.style_list {
            None
        } else {
            let mut d = div().absolute().left(px(16.0)).top(px(8.0)).w(px(240.0))
                .p_2().rounded_md().bg(gpui::white())
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_0p5()
                .child(div().text_size(px(10.5)).text_color(rgb(0x66707A))
                    .child(ui::t!("段落のスタイル(選んだ段落に掛かる)")));
            // 照合は番号(set_para_style)。名前は見せる字だけなので訳してよい
            for (n, label, pt, bold) in [
                (0u8, ui::t!("標準"), 12.5f32, false),
                (1, ui::t!("見出し1"), 16.0, true),
                (2, ui::t!("見出し2"), 14.0, true),
                (3, ui::t!("見出し3"), 12.5, true),
                // 見出し4・5 は 2026-08-18 に足しました(AsciiDoc の
                // `=====` `======` と同じ段です)
                (4, ui::t!("見出し4"), 12.0, true),
                (5, ui::t!("見出し5"), 11.5, true),
            ] {
                let mut item = div()
                    .id(SharedString::from(format!("style-{n}")))
                    .px_2().py_0p5().rounded_sm()
                    .text_size(px(pt))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF2F7)))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_para_style(n);
                        this.style_list = false;
                        cx.notify();
                    }));
                if bold {
                    item = item.font_weight(gpui::FontWeight::BOLD);
                }
                d = d.child(item);
            }
            Some(d)
        };

        // 記号の一覧。事務の書類で使うものだけ(飾りの絵文字は入れない)
        let symbol_panel = if !self.symbols {
            None
        } else {
            const SYMS: &[&str] = &[
                "〒", "※", "→", "←", "↑", "↓", "℃", "±", "×", "÷",
                "①", "②", "③", "④", "⑤", "⑥", "⑦", "⑧", "⑨", "⑩",
                "㈱", "㈲", "№", "〆", "〜", "…", "・", "「", "」", "『",
                "』", "【", "】", "○", "●", "◎", "△", "▲", "□", "■",
            ];
            let mut d = div().absolute().right(px(16.0)).top(px(8.0)).w(px(340.0))
                .p_2().rounded_md().bg(gpui::white())
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_row().flex_wrap().gap_1();
            for s in SYMS {
                d = d.child(div()
                    .id(SharedString::from(format!("sym-{s}")))
                    .w(px(28.0)).h(px(28.0)).rounded_sm()
                    .flex().items_center().justify_center()
                    .text_size(px(15.0)).cursor_pointer()
                    .hover(|st| st.bg(rgb(0xEAF2F7)))
                    .child(SharedString::from(*s))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.ed.insert(s);
                        this.on_edited();
                        cx.notify();
                    })));
            }
            Some(d)
        };

        // 校正の指摘
        let proof_panel = if self.proof.is_empty() && self.proof_msg.is_empty() {
            None
        } else {
            let mut d = div().absolute().right(px(16.0)).bottom(px(16.0)).w(px(300.0))
                .p_3().rounded_md().bg(gpui::white())
                .border_1().border_color(rgb(0xC6CDD3))
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                       .text_color(rgb(0x165E83))
                       .child(SharedString::from(ui::tf!("校正 — {}", self.proof_msg))));
            for n in &self.proof {
                // どちらの道具が出したかを隠さない。辞書の指摘は GPU 無しで再現できる
                let tool = match n.source {
                    ui::check::Source::Dictionary => ui::t!("辞書"),
                    ui::check::Source::Model => ui::t!("モデル"),
                };
                let cand = if n.candidates.is_empty() {
                    ui::t!("候補なし").to_string()
                } else {
                    n.candidates.join(" / ")
                };
                d = d.child(div().mt_1p5().text_size(px(11.5))
                    .child(SharedString::from(
                        ui::tf!("{} → {}  ({}・{})", n.found, cand, n.kind.label(), tool))));
            }
            Some(d)
        };

        Panels {
            find_panel, hf_panel, cmt_panel, wm_panel, bm_panel, style_new_panel, hist_panel,
            chat_panel, pw_panel, url_panel, fm_panel, nav_panel, rp_panel,
            lk_panel, ai_panel, sd_panel, rb_panel, eq_panel, plug_panel, xr_panel,
            font_panel, size_panel, style_panel, symbol_panel, proof_panel,
        }
    }
}
