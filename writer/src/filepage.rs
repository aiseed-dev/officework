//! **ファイルのページの右側**(統合の段8 の本体。2026-08-20)。
//!
//! 前は `view.rs` の `render` の中で組んでいました。組んだ物を外へ渡せないと、
//! officework がページを描くときに右側だけ頼むことができません。
//!
//! *左の列は `ui::filemenu::sidebar` が描きます。* ここは右側 —
//! いま見ているファイルの詳細と操作 — だけです。編集の中身なので
//! **画面に残します**(SEKKEI「ページごと差し替え」)。

use crate::Writer;
use gpui::{div, prelude::*, px, rgb, Context, SharedString};
use crate::io::lock_identity;
use kumihan::Editor;

impl Writer {
    /// ファイルのページの右側を組む。
    ///
    /// 単体で動くときは `render` が左の列と並べます。officework に
    /// 埋め込まれているときは officework が呼びます。
    pub fn file_pane(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let us = self.ui_scale;
        let dk = self.dark;
        let th_cmd_bg = if dk { rgb(0x22262A) } else { rgb(0xFFFFFF) };
        let th_cmd_border = if dk { rgb(0x33383D) } else { rgb(0xE1E6EA) };
        let th_btn = if dk { rgb(0x7FB2D0) } else { rgb(0x165E83) };
        let th_btn_hover = if dk { rgb(0x2C333A) } else { rgb(0xEAF2F7) };
        let th_gray_fg = if dk { rgb(0x565D64) } else { rgb(0xB6BDC4) };
        let th_status = if dk { rgb(0x9AA5AE) } else { rgb(0x66707A) };
        let th_top_fg = if dk { rgb(0xCFD6DC) } else { rgb(0x444B52) };
        let item_bg = if dk { rgb(0x2C333A) } else { rgb(0xE2E6EA) };
        // 統計。**render から持ち込まずここで数えます** — 呼ぶ側が
        // officework になっても同じ数が出るように
        let total_pages = self.page_offsets.len().max(1);
        let nchars = self.doc.body_text().chars().filter(|c| !c.is_whitespace()).count();
        let mut pane = div().flex_1().bg(th_cmd_bg).p_8()
            .flex().flex_col().gap_3().text_size(px(us * 12.5))
            .text_color(th_top_fg);
        if self.file_view == 3 {
            // **フォルダから探す**(2026-08-17 発注者。SFIND の写真)。
            // 上に欄、真ん中に当たりの一覧、下に見せる窓と「読み込み」
            let 欄 = |this: &Writer, i: usize, ed: &Editor, w: f32, ph: &'static str| {
                let mut s = ed.text().to_string();
                if this.fd_field == i && this.file_view == 3 {
                    let c = ed.cursor().min(s.len());
                    s.insert(c, '|');
                }
                div().id(SharedString::from(format!("fd-{i}")))
                    .w(px(w)).px_2().py_1().rounded_sm().cursor_text()
                    .border_1()
                    .border_color(if this.fd_field == i { th_btn } else { th_cmd_border })
                    .bg(gpui::white())
                    .text_size(px(us * 12.5)).whitespace_nowrap().overflow_hidden()
                    .child(SharedString::from(if s.is_empty() { ph.to_string() } else { s }))
                    .on_click(cx.listener(move |t, _, _, cx| { t.fd_field = i; cx.notify() }))
            };
            let 押し = |id: &'static str, 札: SharedString| {
                div().id(id).px_3().py_1().rounded_sm().cursor_pointer()
                    .border_1().border_color(th_btn).text_color(th_btn)
                    .text_size(px(us * 12.0))
                    .hover(move |s| s.bg(th_btn_hover))
                    .child(札)
            };
            pane = pane
                .child(div().text_size(px(us * 16.0)).font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("search_folder")))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(欄(self, 0, &self.fd_term, 280.0, "探す字"))
                    .child(欄(self, 1, &self.fd_glob, 120.0, "*.txt"))
                    .child(押し("fd-dir", ui::t!("choose_folder").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.find_dir_dialog(cx); cx.notify() })))
                    .child(押し("fd-go", ui::t!("search_enter").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.find_in_folder(); cx.notify() }))))
                .child(div().text_size(px(us * 11.5)).text_color(th_status)
                    .child(SharedString::from(match self.find_dir() {
                        Some(d) => ui::tf!("folder_2", d.display()).to_string(),
                        None => ui::t!("no_folder_chosen_yet").to_string(),
                    })));
            // 当たりの一覧(ファイルごとに見出し + 行番号つきの行)
            let mut 一覧 = div().id("fd-list")
                .flex_none().h(px(us * 320.0)).overflow_y_scroll()
                .p_2().rounded_sm().bg(gpui::white())
                .border_1().border_color(th_cmd_border)
                .flex().flex_col().gap_0p5().text_size(px(us * 12.0));
            if self.fd_hits.is_empty() {
                一覧 = 一覧.child(div().text_color(th_status)
                    .child(ui::t!("nothing_searched_yet")));
            }
            self.fd_box.borrow_mut().clear();
            for (fi, f) in self.fd_hits.iter().enumerate() {
                一覧 = 一覧.child(div().mt_1().text_color(th_btn)
                    .child(SharedString::from(format!(
                        "{}   {}   {}",
                        f.path.file_name().unwrap_or_default().to_string_lossy(),
                        ui::search::human_size(f.size),
                        f.path.parent().map(|d| d.display().to_string()).unwrap_or_default()
                    ))));
                for (hi, h) in f.hits.iter().enumerate() {
                    let on = self.fd_at == Some((fi, hi));
                    // 長い行は縮める(一覧が横に流れない)
                    let line: String = h.text.chars().take(120).collect();
                    let rec = self.fd_box.clone();
                    一覧 = 一覧.child(div()
                        .id(SharedString::from(format!("fd-h-{fi}-{hi}")))
                        .relative()
                        .child(gpui::canvas(
                            move |b: gpui::Bounds<gpui::Pixels>, _, _| {
                                rec.borrow_mut().push((
                                    fi,
                                    hi,
                                    f32::from(b.origin.x),
                                    f32::from(b.origin.y),
                                    f32::from(b.size.width),
                                    f32::from(b.size.height),
                                ));
                            },
                            |_, _: (), _, _| {},
                        ).absolute().size_full())
                        .px_1().rounded_sm().cursor_pointer()
                        .bg(if on { th_btn_hover } else { gpui::transparent_black().into() })
                        .hover(move |s| s.bg(th_btn_hover))
                        .whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(format!("{:05} {line}", h.line)))
                        .on_click(cx.listener(move |t, _, _, cx| {
                            t.find_peek(fi, hi);
                            cx.notify()
                        })));
                }
            }
            pane = pane.child(一覧);
            // **下の窓と「読み込み」**(発注者 2026-08-17
            // 「下に読み込みボタンを置くのはどうか」)。見て、これだと
            // 分かってから開く — 押し間違いで文書が入れ替わらない
            pane = pane.child(div().flex().flex_row().items_center().gap_2()
                .child(押し("fd-load", ui::t!("load").into()).on_click(
                    cx.listener(|t, _, _, cx| { t.find_load(); cx.notify() })))
                .child(div().text_size(px(us * 11.5)).text_color(th_status)
                    .child(ui::t!("opens_document_chosen_hit"))));
            pane = pane.child(div().id("fd-peek")
                .flex_1().min_h(px(us * 120.0)).overflow_y_scroll()
                .p_2().rounded_sm().bg(gpui::white())
                .border_1().border_color(th_cmd_border)
                .text_size(px(us * 12.0)).font_family(crate::doc::MONO)
                .child(SharedString::from(if self.fd_peek.is_empty() {
                    ui::t!("pick_hit_surroundings_appear").to_string()
                } else {
                    self.fd_peek.clone()
                })));
        } else if self.file_view == 2 {
            // 詳細設定 — 器は ~/.config/officework/settings.toml
            // (SEKKEI「設定 — 器と言語」。環境変数が一時上書きで優先)
            let lang_now = ui::settings::get("language").unwrap_or_else(|| "ja".into());
            // 見出しが String の版(ui::env_rows が返す形)
            let row_owned = |label: String, value: String| {
                div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(th_status)
                        .child(SharedString::from(label)))
                    .child(div().child(SharedString::from(value)))
            };
            pane = pane
                .child(div().text_size(px(us * 16.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("advanced_settings")))
                .child(div().text_color(th_status).child(SharedString::from(
                    ui::tf!("location", ui::settings::path().display()))))
                .child(div().h(px(us * 6.0)))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(th_status)
                        .child(ui::t!("language_ribbon_messages")))
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
                // **画面の明暗**(2026-08-20 発注者「双方でできるように
                // したいです」)。**紙は白いまま** — 暗くするのは周りだけ
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(th_status)
                        .child(ui::t!("interface_theme_light_dark")))
                    .child(div().id("set-theme")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child(if self.dark { ui::t!("dark") } else { ui::t!("light") })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("darkmode", cx);
                            cx.notify()
                        }))))
                // **コメントの名乗り**(2026-08-20 発注者「双方でできる
                // ようにしたいです」)。器は表と同じ `user_name`。
                // **未設定なら名乗りません** — 機械のユーザー名は使わない
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(th_status)
                        .child(ui::t!("comment_signature")))
                    .child(div().id("set-username")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child(SharedString::from({
                            let a = ui::comment_author();
                            if a.is_empty() { ui::t!("anonymous").to_string() } else { a }
                        }))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cmt_name_edit = true;
                            this.cmt_name_ed = Editor::new(&ui::comment_author());
                            this.status =
                                ui::t!("type_name_press_enter").into();
                            cx.notify()
                        }))))
                // **反復計算**(2026-08-20 発注者「双方でできるように
                // したいです」)。文書の表もセル関数を持つので、循環参照を
                // 回して解く道は表計算と同じに要る。器はアプリの設定 —
                // `.adoc` の文書には xlsx の `calcPr` に当たる置き場が無い
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(th_status)
                        .child(ui::t!("iterative_calculation_circular_references")))
                    .child(div().id("set-iter")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child(match ui::calc_iter_setting() {
                            Some((n, _)) => ui::tf!("up_passes", n),
                            None => ui::t!("off_switch").to_string(),
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            let (_, msg) =
                                ui::toggle_calc_iter(ui::calc_iter_setting(), !cfg!(test));
                            this.status = msg.into();
                            this.lay();
                            cx.notify()
                        }))))
                // **数学オートコレクト**(2026-08-20 発注者「双方でできる
                // ようにしたいです」)。仕掛けは前から共通で、表だけが
                // 名乗り出ていた。器も表と同じ1つの綴りを見る
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(th_status)
                        .child(ui::t!("math_autocorrect")))
                    .child(div().id("set-autocorrect")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child(if self.autocorrect {
                            ui::t!("type_alpha_get")
                        } else {
                            ui::t!("off_switch")
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            let (on, msg) = ui::toggle_math_autocorrect(this.autocorrect, !cfg!(test));
                            this.autocorrect = on;
                            this.status = msg.into();
                            cx.notify()
                        }))))
                // ── AI ────────────────────────────────────────────
                // **宛先を覚えるのはここ**(発注者 2026-08-15
                // 「AI の設定を設定メニューに追加して」)。calc と同じ形
                .child(div().h(px(us * 10.0)))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 200.0)).text_color(th_status)
                        .child(ui::t!("ai_destination")))
                    .child(div().id("set-ai")
                        .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                        .child(SharedString::from(ui::ai::backend().label().to_string()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_cmd("ai-where", cx);
                            cx.notify()
                        }))))
                // **使えないなら理由を出す。** 鍵そのものは出さない。
                // 手元のモデルだけは「使えます」と言わない(繋がるか
                // 確かめずに言えば嘘になる)
                // **見るだけの7行は ui::env_rows の1本**(2026-08-20)。
                // 前は writer と calc に同じ物が写してあった
                .children(
                    ui::env_rows(&lock_identity())
                        .into_iter()
                        .map(|(k, v)| row_owned(k, v)),
                );
        } else if self.file_view == 1 {
            // **最近開いたの面は ui::filemenu の1本**(段8 の3)。
            // 押したときの行き先だけがアプリの物
            let look = ui::filemenu::PaneLook {
                fg: th_top_fg, dim: th_status, hover: item_bg, scale: us,
            };
            pane = pane.child(ui::filemenu::pane_title(&look, ui::t!("recent")));
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
        } else if self.file_view == 5 {
            // **書式の標準は3段**(2026-08-26 発注者)。
            //
            // . *文書* — その文章が自分で持つ
            // . *綴り* — フォルダの `テンプレート.toml`
            // . *利用者* — `~/.config/officework/テンプレート.toml`
            //
            // 下の段は、上が言っていないことだけを埋めます。
            // **どこが効いているかが見えるのが要**です。ＭＳ 明朝の docx を
            // 開いて字が代わったとき、どこを直せばよいか分からないのが
            // 今の困りごとだからです。
            // **重ねる前の姿を読みます。** 重ねた後を見せると、下の段の
            // 言い分が上の段の言い分に見えて、どこを直せばよいのかが
            // かえって分からなくなります(この画面で一度そうなりました)。
            let 利用者の場所 =
                ui::settings::dir().join(kumihan::theme::user_template_name());
            let 利用者 = kumihan::theme::read_theme(&利用者の場所);
            let 綴りの場所 = self
                .tmpl_path
                .clone()
                .or_else(|| self.folder().map(|d| d.join(Self::FOLDER_TEMPLATE)));
            let 綴り = 綴りの場所.as_deref().and_then(kumihan::theme::read_theme);
            // **書体と大きさは言語で変わります**(2026-08-26 発注者)。
            // その段が `[文書.en]` のような言語ごとの分を持っているときは、
            // いまの言語の分を出します — 出さないと、画面の数字と
            // ファイルの中身が食い違って見えます
            let 言語 = ui::language();
            let 言い分 = |th: Option<&kumihan::theme::Theme>| -> Option<String> {
                let th = th?.for_language(&言語);
                match (th.font, th.size_pt) {
                    (Some(f), Some(s)) => Some(format!("{f} {s}pt")),
                    (Some(f), None) => Some(f),
                    (None, Some(s)) => Some(format!("{s}pt")),
                    (None, None) => None,
                }
            };
            let 段 = |名: &str, 値: Option<String>, 場所: Option<String>| {
                let 値 = 値.unwrap_or_else(|| ui::t!("not_specified").to_string());
                div().flex().flex_col().gap_1().pb_2()
                    .child(div().flex().flex_row().items_center().gap_2()
                        .child(div().w(px(us * 120.0)).text_color(th_status)
                            .child(SharedString::from(名.to_string())))
                        .child(div().child(SharedString::from(値))))
                    .child(div().pl(px(us * 120.0)).text_size(px(us * 10.0))
                        .text_color(th_status)
                        .child(SharedString::from(
                            場所.unwrap_or_else(|| ui::t!("not_created_yet").to_string()))))
            };
            pane = pane
                .child(div().text_size(px(us * 16.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("formatting_defaults")))
                .child(div().text_color(th_status)
                    .child(ui::t!("each_level_fills_what")))
                .child(div().text_color(th_status).text_size(px(us * 11.0))
                    .child(ui::tf!("current_language_font_size", 言語)))
                .child(div().h(px(us * 8.0)))
                // 1段目 — この文書
                .child(段(&ui::t!("document_5"), self.doc.font.clone(),
                          Some(ui::t!("font_stored_document_open").to_string())))
                // 2段目 — 綴り
                .child(段(&ui::t!("folder_4"),
                          言い分(綴り.as_ref()),
                          綴りの場所.map(|p| p.display().to_string())))
                // 3段目 — 利用者
                .child(段(&ui::t!("account_computer"),
                          言い分(利用者.as_ref()),
                          Some(利用者の場所.display().to_string())))
                .child(div().h(px(us * 8.0)))
                // **いま実際に使っている書体と大きさ**。3段を重ねた結果です
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(div().w(px(us * 120.0)).text_color(th_status)
                        .child(ui::t!("use_now")))
                    .child(div().font_weight(gpui::FontWeight::BOLD)
                        .child(SharedString::from(format!(
                            "{} {}pt",
                            self.font_name,
                            self.base_pt()
                        )))))
                .child(div().h(px(us * 10.0)))
                .child(div().text_color(th_status).child(
                    ui::t!("choosing_font_makes_default")))
                .child(div().id("style-user-font")
                    .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                    .child(ui::t!("choose_default_font_computer"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tab = this.prev_tab;
                        this.open_list = Some("user-font");
                        this.pick_sel = 0;
                        cx.notify()
                    })));
        } else if self.file_view == 4 {
            // **前に保存できずに終わった控え**(2026-08-21 の B-3)。
            //
            // 見せ方は「最近開いた」と同じです。押すと**控えの方**を開き、
            // 道は持たせません — そのまま上書き保存して原本を潰さないため。
            // 中身を確かめてから「名前を付けて保存」する形です(表と同じ作法)。
            let look = ui::filemenu::PaneLook {
                fg: th_top_fg, dim: th_status, hover: item_bg, scale: us,
            };
            pane = pane.child(ui::filemenu::pane_title(&look, ui::t!("recover")));
            let list = ops::stale_recovers("adoc");
            if list.is_empty() {
                pane = pane.child(ui::filemenu::recent_empty(&look));
            }
            for (i, (名, 道)) in list.into_iter().enumerate() {
                let 名2 = 名.clone();
                pane = pane.child(
                    ui::filemenu::recent_row(&look, i, std::path::Path::new(&名))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tab = this.prev_tab;
                            this.open(道.clone());
                            // **控えを原本と取り違えないよう、道は持たせない**
                            this.path = None;
                            this.dirty = true;
                            this.status = ui::tf!(
                                "opened_backup_original_check",
                                名2.clone()
                            )
                            .into();
                            cx.notify()
                        })),
                );
            }
        } else {
            let text = self.doc.body_text();
            let words = text.split_whitespace().count();
            let chars_all = text.chars().filter(|c| *c != '\n').count();
            let paras = self.doc.paragraphs().count();
            pane = pane.child(div().text_size(px(us * 16.0))
                .font_weight(gpui::FontWeight::BOLD)
                .child(ui::t!("document_info")))
                .child(div().text_size(px(us * 13.5))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("statistics")));
            for (k, v) in [
                (ui::t!("page_2"), total_pages),
                (ui::t!("paragraphs"), paras),
                (ui::t!("words"), words),
                (ui::t!("characters_2"), nchars),
                (ui::t!("characters_spaces"), chars_all),
            ] {
                pane = pane.child(div().flex().flex_row()
                    .child(div().w(px(us * 220.0)).text_color(th_status).child(k))
                    .child(SharedString::from(format!("{v}"))));
            }
            pane = pane.child(div().h(px(us * 6.0)))
                .child(div().text_size(px(us * 13.5))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("properties")));
            let pr = self.doc.props.clone();
            let vals: [(&'static str, String, &'static str); 5] = [
                (ui::t!("author"), pr.creator, ui::t!("add_author")),
                (ui::t!("title"), pr.title, ui::t!("add_text")),
                (ui::t!("tags"), pr.keywords, ui::t!("add_text")),
                (ui::t!("subject"), pr.subject, ui::t!("add_text")),
                (ui::t!("comment"), pr.description, ui::t!("add_text")),
            ];
            for (i, (k, v, ph)) in vals.into_iter().enumerate() {
                let editing = self.file_field == Some(i as u8);
                let shown = if editing {
                    let mut t = self.prop_ed.text().to_string();
                    let cur = self.prop_ed.cursor().min(t.len());
                    t.insert(cur, '|');
                    t
                } else {
                    v.clone()
                };
                let empty = !editing && v.is_empty();
                pane = pane.child(div().flex().flex_row().items_center()
                    .child(div().w(px(us * 220.0)).text_color(th_status).child(k))
                    .child(div()
                        .id(SharedString::from(format!("prop-{i}")))
                        .w(px(us * 320.0)).px_2().py_1().rounded_sm()
                        .border_1()
                        .border_color(if editing {
                            rgb(0x1B6E3C)
                        } else {
                            th_cmd_border
                        })
                        .cursor_pointer()
                        .whitespace_nowrap().overflow_hidden()
                        .text_color(if empty { th_gray_fg } else { th_top_fg })
                        .child(SharedString::from(if empty {
                            ph.to_string()
                        } else {
                            shown
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let cur = match i {
                                0 => this.doc.props.creator.clone(),
                                1 => this.doc.props.title.clone(),
                                2 => this.doc.props.keywords.clone(),
                                3 => this.doc.props.subject.clone(),
                                _ => this.doc.props.description.clone(),
                            };
                            this.prop_ed = Editor::new(&cur);
                            this.file_field = Some(i as u8);
                            cx.notify()
                        }))));
            }
            pane = pane.child(div().text_size(px(us * 11.5)).text_color(th_status)
                .child(ui::t!("click_field_type_enter")));
        }
        pane
    }
}
