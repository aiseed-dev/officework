//! **ファイルのページの右側**(統合の段8 の本体。2026-08-20)。
//!
//! 文章の画面の `writer/src/filepage.rs` と対になります。左の列は
//! `ui::filemenu::sidebar` が両方に描き、右側 — いま見ているブックの
//! 詳細と操作 — だけがここです。編集の中身なので**画面に残します**。

use crate::Calc;
use gpui::{div, prelude::*, px, rgb, Context, SharedString};
use crate::io::lock_identity;
use kumihan::Editor;

impl Calc {
    /// ファイルのページの右側を組む。
    ///
    /// 単体で動くときは `render` が左の列と並べます。officework に
    /// 埋め込まれているときは officework が呼びます。
    pub fn file_pane(&mut self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let us = self.ui_scale;
        let item_bg = rgb(0xE2E6EA);
        let gray = rgb(0xB6BDC4);
        let fg = rgb(0x444B52);
        let dim = rgb(0x66707A);
        let mut pane = div().id("file-pane").flex_1().overflow_y_scroll()
            .bg(gpui::white()).p_8()
            .flex().flex_col().gap_3().text_size(px(us * 12.5)).text_color(fg);
        if self.file_view == 2 {
            // 詳細設定 — 器は ~/.config/officework/settings.toml
            // (SEKKEI「設定 — 器と言語」。環境変数が一時上書きで優先)
            let lang_now = ui::settings::get("language").unwrap_or_else(|| "ja".into());
            // 見出しが String の版(ui::env_rows が返す形)
            let row_owned = |label: String, value: String| {
                div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(dim)
                        .child(SharedString::from(label)))
                    .child(div().child(SharedString::from(value)))
            };
            pane = pane
                .child(div().text_size(px(us * 16.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("詳細設定")))
                .child(div().text_color(dim).child(SharedString::from(
                    ui::tf!("置き場: {}", ui::settings::path().display()))))
                .child(div().h(px(6.0)))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(dim)
                        .child(ui::t!("言語(リボンと文言)")))
                    .child(div().id("set-lang")
                        .px_3().py_1().rounded_sm().cursor_pointer()
                        .bg(item_bg)
                        // 札ではなく**その言語自身の名前**を出す。
                        // `pt` と `pt-br` は札のままでは見分けられない
                        .child(SharedString::from(
                            ui::language_label(&lang_now).to_string()))
                        // 中身は ui::cycle_language の1本(段8)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.status = ui::cycle_language().into();
                            cx.notify()
                        }))))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(dim)
                        .child(ui::t!("画面の明暗(テーマ)")))
                    .child(div().id("set-theme")
                        .px_3().py_1().rounded_sm().cursor_pointer()
                        .bg(item_bg)
                        .child(if self.dark { ui::t!("暗い") } else { ui::t!("明るい") })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("darkmode", cx);
                            cx.notify()
                        }))))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(dim)
                        .child(ui::t!("画面の文字の大きさ")))
                    .child(div().id("set-ui-minus")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child("−")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("ui-smaller", cx);
                            cx.notify()
                        })))
                    .child(div().child(SharedString::from(
                        format!("{}%", (self.ui_scale * 100.0).round() as i32))))
                    .child(div().id("set-ui-plus")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child("+")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("ui-bigger", cx);
                            cx.notify()
                        }))))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(dim)
                        .child(ui::t!("反復計算(循環参照)")))
                    .child(div().id("set-iter")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child(match self.book.calc_iter {
                            Some((n, d)) => ui::tf!("入(最大 {} 回・変化 {} まで)", n, d),
                            None => ui::t!("切").into(),
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("calc-iter", cx);
                            cx.notify()
                        }))))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(dim)
                        .child(ui::t!("参照の形式")))
                    .child(div().id("set-refstyle")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child(if self.book.r1c1 { "R1C1" } else { "A1" })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("ref-style", cx);
                            cx.notify()
                        }))))
                // 本家は「オートコレクトのオプション」の小窓(3タブ)。
                // こちらは**記号の置き換えだけ**入れたので入切の1行
                // (「認識される関数」と「入力中の自動フォーマット」は
                // 数式の組版が要る話で、そちらは LaTeX で受けて
                // Python に組ませる車線 — 一列の文字では出せない)
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(dim)
                        .child(ui::t!("数学オートコレクト")))
                    .child(div().id("set-autocorrect")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child(if self.autocorrect {
                            ui::t!("入(\\alpha と打つと α)")
                        } else {
                            ui::t!("切")
                        })
                        // 判断は ui::toggle_math_autocorrect の1本(文章と共通)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let (on, msg) = ui::toggle_math_autocorrect(this.autocorrect, !cfg!(test));
                            this.autocorrect = on;
                            this.status = msg.into();
                            cx.notify()
                        }))))
                // ── AI ────────────────────────────────────────────
                // **宛先を覚えるのはここ**(発注者 2026-08-15
                // 「AI の設定を設定メニューに追加して」)。リボンの AI
                // タブは左パネルの会話に譲って消える予定なので、
                // 覚える設定の持ち場をこちらへ移しておく
                .child(div().h(px(10.0)))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(dim)
                        .child(ui::t!("AI の宛先")))
                    .child(div().id("set-ai")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child(SharedString::from(ui::ai::backend().label().to_string()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("ai-where", cx);
                            cx.notify()
                        }))))
                // **使えないなら理由を出す**(押してみるまで分からない、に
                // しない)。鍵そのものは出さない — 有る無しだけ。
                // **手元のモデルだけは「使えます」と言わない** — 繋がるか
                // 確かめずに言えば嘘になる(宛先は下の「校正の宛先」)
                // **見るだけの7行は ui::env_rows の1本**(2026-08-20)。
                // 前は writer と calc に同じ物が写してあった
                .children(
                    ui::env_rows(&lock_identity())
                        .into_iter()
                        .map(|(k, v)| row_owned(k, v)),
                )
                // コメントに書き残す名乗り。**機械の名前とは別**にする —
                // user@host は錠の相手を見分けるための綴りで、
                // 帳票に残す名前ではない。決めていなければ名乗らない
                // (「不明」のような名前を作らない)
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(dim)
                        .child(ui::t!("コメントの名乗り")))
                    .child(div().id("set-username")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child(SharedString::from(
                            ui::settings::get("user_name")
                                .filter(|s| !s.trim().is_empty())
                                .unwrap_or_else(|| ui::t!("(名乗らない)").into())))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let cur = ui::settings::get("user_name").unwrap_or_default();
                            this.prompt = Some(("user-name", Editor::new(&cur)));
                            cx.notify()
                        }))));
        } else if self.file_view == 3 {
            // **フォルダから探す**(2026-08-17 発注者。SFIND の写真)。
            // writer と同じ形 — 上に欄、真ん中に当たり、下に窓と「読み込み」
            let 欄 = |this: &Calc, i: usize, ed: &Editor, w: f32, ph: &'static str| {
                let mut s = ed.text().to_string();
                if this.fd_field == i {
                    let c = ed.cursor().min(s.len());
                    s.insert(c, '|');
                }
                div().id(SharedString::from(format!("fd-{i}")))
                    .w(px(us * w)).px_2().py_1().rounded_sm().cursor_pointer()
                    .border_1()
                    .border_color(if this.fd_field == i { rgb(0x1B6E3C) } else { rgb(0xC6CDD3) })
                    .bg(gpui::white())
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(if s.is_empty() { ph.to_string() } else { s }))
                    .on_click(cx.listener(move |t: &mut Calc, _, _, cx| {
                        t.fd_field = i;
                        cx.notify()
                    }))
            };
            let 押し = |id: &'static str, 札: SharedString| {
                div().id(id).px_3().py_1().rounded_sm().cursor_pointer()
                    .border_1().border_color(rgb(0x1B6E3C)).text_color(rgb(0x1B6E3C))
                    .text_size(px(us * 12.0))
                    .hover(|s| s.bg(rgb(0xEAF5EE)))
                    .child(札)
            };
            pane = pane
                .child(div().text_size(px(us * 16.0)).font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("フォルダから探す")))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(欄(self, 0, &self.fd_term, 280.0, "探す字"))
                    .child(欄(self, 1, &self.fd_glob, 120.0, "*.xlsx"))
                    .child(押し("fd-dir", ui::t!("場所を選ぶ").into()).on_click(
                        cx.listener(|t: &mut Calc, _, _, cx| { t.find_dir_dialog(cx); cx.notify() })))
                    .child(押し("fd-go", ui::t!("探す (Enter)").into()).on_click(
                        cx.listener(|t: &mut Calc, _, _, cx| { t.find_in_folder(); cx.notify() }))))
                .child(div().text_size(px(us * 11.5)).text_color(dim)
                    .child(SharedString::from(match self.find_dir() {
                        Some(d) => ui::tf!("場所: {}", d.display()).to_string(),
                        None => ui::t!("場所がまだ決まっていません(「場所を選ぶ」)").to_string(),
                    })));
            let mut 一覧 = div().id("fd-list")
                .flex_none().h(px(us * 300.0)).overflow_y_scroll()
                .p_2().rounded_sm().bg(gpui::white())
                .border_1().border_color(rgb(0xC6CDD3))
                .flex().flex_col().gap_0p5().text_size(px(us * 12.0));
            if self.fd_hits.is_empty() {
                一覧 = 一覧.child(div().text_color(dim).child(ui::t!("(まだ探していません)")));
            }
            for (fi, f) in self.fd_hits.iter().enumerate() {
                一覧 = 一覧.child(div().mt_1().text_color(rgb(0x1B6E3C))
                    .child(SharedString::from(format!(
                        "{}   {}   {}",
                        f.path.file_name().unwrap_or_default().to_string_lossy(),
                        ui::search::human_size(f.size),
                        f.path.parent().map(|d| d.display().to_string()).unwrap_or_default()
                    ))));
                for (hi, h) in f.hits.iter().enumerate() {
                    let on = self.fd_at == Some((fi, hi));
                    let line: String = h.text.chars().take(120).collect();
                    一覧 = 一覧.child(div()
                        .id(SharedString::from(format!("fd-h-{fi}-{hi}")))
                        .px_1().rounded_sm().cursor_pointer()
                        .bg(if on { rgb(0xEAF5EE) } else { gpui::transparent_black().into() })
                        .hover(|s| s.bg(rgb(0xEAF5EE)))
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(format!("{:05} {line}", h.line)))
                        .on_click(cx.listener(move |t: &mut Calc, _, _, cx| {
                            t.find_peek(fi, hi);
                            cx.notify()
                        })));
                }
            }
            pane = pane.child(一覧);
            pane = pane.child(div().flex().flex_row().items_center().gap_2()
                .child(押し("fd-load", ui::t!("読み込み").into()).on_click(
                    cx.listener(|t: &mut Calc, _, _, cx| { t.find_load(cx); cx.notify() })))
                .child(div().text_size(px(us * 11.5)).text_color(dim)
                    .child(ui::t!("選んだ当たりの文書を開いて、その場所へ移ります"))));
            pane = pane.child(div().id("fd-peek")
                .flex_1().min_h(px(us * 100.0)).overflow_y_scroll()
                .p_2().rounded_sm().bg(gpui::white())
                .border_1().border_color(rgb(0xC6CDD3))
                .text_size(px(us * 12.0))
                .child(SharedString::from(if self.fd_peek.is_empty() {
                    ui::t!("(当たりを選ぶと、ここに前後が出ます)").to_string()
                } else {
                    self.fd_peek.clone()
                })));
        } else if self.file_view == 1 {
            // **最近開いたの面は ui::filemenu の1本**(段8 の3)。
            // 押したときの行き先だけがアプリの物
            let look = ui::filemenu::PaneLook {
                fg, dim, hover: item_bg, scale: us,
            };
            pane = pane.child(ui::filemenu::pane_title(&look, ui::t!("最近開いた")));
            let list = Self::recent_list();
            if list.is_empty() {
                pane = pane.child(ui::filemenu::recent_empty(&look));
            }
            for (i, q) in list.into_iter().enumerate() {
                pane = pane.child(ui::filemenu::recent_row(&look, i, &q)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tab = this.prev_tab;
                        this.open(q.clone());
                        cx.notify()
                    })));
            }
        } else {
            // 統計(生きた値)とブックの情報(docProps/core.xml から)
            let sheets_n = self.book.sheets.len();
            let mut cells_n = 0usize;
            let mut formulas_n = 0usize;
            for sh in &self.book.sheets {
                cells_n += sh.cells.len();
                formulas_n +=
                    sh.cells.values().filter(|c| c.formula.is_some()).count();
            }
            let shapes_n: usize = self
                .book
                .sheets
                .iter()
                .map(|s| {
                    s.shapes.len() + s.shapes_new.len() + s.images.len()
                        + s.images_new.len()
                })
                .sum();
            pane = pane.child(div().text_size(px(us * 16.0))
                .font_weight(gpui::FontWeight::BOLD)
                .child(ui::t!("ブックの情報")))
                .child(div().text_size(px(us * 13.5))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("統計")));
            // **印を付ける。** 見出し(「統計」)は t! に包んであるのに
            // 行の名前は裸だったので、ポルトガル語で開くと見出しだけが
            // 訳されて中身が日本語のまま並んでいた(2026-08-11、実機で
            // 見つけた)。文言の門番は**印の付いた文しか見られない**ので、
            // 包み忘れは検査を通り抜ける
            for (k, v) in [
                (ui::t!("シート"), sheets_n),
                (ui::t!("使っているセル"), cells_n),
                (ui::t!("式のセル"), formulas_n),
                (ui::t!("図形と画像"), shapes_n),
            ] {
                pane = pane.child(div().flex().flex_row()
                    .child(div().w(px(220.0)).text_color(dim).child(k))
                    .child(SharedString::from(format!("{v}"))));
            }
            pane = pane.child(div().h(px(6.0)))
                .child(div().text_size(px(us * 13.5))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("プロパティ")));
            // 著者は**何人でも**(dc:creator は `;` 区切り)。
            // 一人ずつ札にして、× で外し、「＋」で足す
            let mut authors = div().flex().flex_row().flex_wrap().gap_1();
            for (i, who) in self.book.props.creators.iter().enumerate() {
                authors = authors.child(div()
                    .flex().flex_row().items_center().gap_1()
                    .px_2().py_0p5().rounded_sm()
                    .bg(rgb(0xEFF3F6)).border_1().border_color(rgb(0xE1E6EA))
                    .child(SharedString::from(who.clone()))
                    .child(div()
                        .id(SharedString::from(format!("prop-author-x{i}")))
                        .px_1().cursor_pointer().text_color(gray)
                        .hover(move |s| s.text_color(rgb(0xB00020)))
                        .child("×")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if i < this.book.props.creators.len() {
                                let who = this.book.props.creators.remove(i);
                                this.dirty = true;
                                this.status = ui::tf!("著者「{}」を外しました", who).into();
                            }
                            cx.notify()
                        }))));
            }
            authors = authors.child(div()
                .id("prop-author-add")
                .px_2().py_0p5().rounded_sm().cursor_pointer()
                .border_1().border_color(rgb(0xE1E6EA)).text_color(gray)
                .hover(move |s| s.bg(item_bg))
                .child(ui::t!("＋ 著者を追加"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.prompt = Some(("prop-author-add", Editor::new("")));
                    cx.notify()
                })));
            pane = pane.child(div().flex().flex_row().items_center()
                .child(div().w(px(220.0)).text_color(dim).child(ui::t!("作成者")))
                .child(authors));
            let pr = &self.book.props;
            for (k, v, kind) in [
                (ui::t!("タイトル"), pr.title.clone(), "prop-title"),
                (ui::t!("タグ"), pr.keywords.clone(), "prop-keywords"),
                (ui::t!("件名"), pr.subject.clone(), "prop-subject"),
                (ui::t!("コメント"), pr.description.clone(), "prop-desc"),
            ] {
                let empty = v.is_empty();
                let init = v.clone();
                pane = pane.child(div().flex().flex_row().items_center()
                    .child(div().w(px(220.0)).text_color(dim).child(k))
                    .child(div()
                        .id(SharedString::from(kind))
                        .w(px(320.0)).px_2().py_1().rounded_sm()
                        .border_1().border_color(rgb(0xE1E6EA))
                        .cursor_pointer()
                        .hover(move |s| s.bg(item_bg))
                        .whitespace_nowrap().overflow_hidden()
                        .text_color(if empty { gray } else { fg })
                        .child(SharedString::from(if empty {
                            ui::t!("テキストの追加").to_string()
                        } else {
                            v
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.prompt = Some((kind, Editor::new(&init)));
                            cx.notify()
                        }))));
            }
            // カスタムプロパティ(docProps/custom.xml)。決まった5項目では
            // 足りないものを、名前・型・値で自分で足す
            pane = pane.child(div().h(px(6.0)))
                .child(div().text_size(px(us * 13.5))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("カスタムプロパティ")));
            for (i, p) in self.book.props.custom.iter().enumerate() {
                use sheet::model::CustomVal;
                let (kind, val) = match &p.value {
                    CustomVal::Text(t) => (ui::t!("文字").to_string(), t.clone()),
                    CustomVal::Number(n) => (ui::t!("数").to_string(), format!("{n}")),
                    CustomVal::Date(d) => (ui::t!("日付").to_string(), d.clone()),
                    CustomVal::Bool(b) => (ui::t!("はい・いいえ").to_string(),
                        if *b { ui::t!("はい") } else { ui::t!("いいえ") }.to_string()),
                    // 型を知らない値。**見せるが打ち直させない**
                    CustomVal::Other(t, v) => (t.clone(), v.clone()),
                };
                let linked = p.link.is_some();
                pane = pane.child(div().flex().flex_row().items_center()
                    .child(div().w(px(220.0)).text_color(dim)
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(if linked {
                            // 内容にリンクしている札。繋ぎ直しはしないが外しもしない
                            format!("🔗 {}", p.name)
                        } else {
                            p.name.clone()
                        })))
                    .child(div().w(px(90.0)).text_color(gray).text_size(px(us * 11.5))
                        .child(SharedString::from(kind)))
                    .child(div().w(px(230.0)).whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(val)))
                    .child(div()
                        .id(SharedString::from(format!("prop-custom-x{i}")))
                        .px_1().cursor_pointer().text_color(gray)
                        .hover(move |s| s.text_color(rgb(0xB00020)))
                        .child("×")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if i < this.book.props.custom.len() {
                                let p = this.book.props.custom.remove(i);
                                this.dirty = true;
                                this.status =
                                    ui::tf!("プロパティ「{}」を外しました", p.name).into();
                            }
                            cx.notify()
                        }))));
            }
            pane = pane.child(div()
                .id("prop-custom-add")
                .w(px(220.0)).px_2().py_1().rounded_sm().cursor_pointer()
                .border_1().border_color(rgb(0xE1E6EA)).text_color(gray)
                .hover(move |s| s.bg(item_bg))
                .child(ui::t!("プロパティを追加"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.prop_add = None;
                    this.prompt = Some(("prop-add-name", Editor::new("")));
                    cx.notify()
                })));
            pane = pane.child(div().text_size(px(us * 11.5)).text_color(dim)
                .child(ui::t!("欄を押して打ち、Enter で控える(保存で xlsx の情報に入ります)")));
        }
        pane
    }
}
