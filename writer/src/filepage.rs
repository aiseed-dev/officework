//! **ファイルのページの右側**(統合の段8 の本体。2026-08-20)。
//!
//! 前は `view.rs` の `render` の中で組んでいました。組んだ物を外へ渡せないと、
//! officework がページを描くときに右側だけ頼むことができません。
//!
//! *左の列は `ui::filemenu::sidebar` が描きます。* ここは右側 —
//! いま見ているファイルの詳細と操作 — だけです。編集の中身なので
//! **画面に残します**(SEKKEI「ページごと差し替え」)。

use crate::{Document, Target, Writer};
use gpui::{div, prelude::*, px, rgb, Context, SharedString};
use crate::io::lock_identity;
use kumihan::Editor;

impl Writer {
    /// ファイルのページの右側を組む。
    ///
    /// 単体で動くときは `render` が左の列と並べます。officework に
    /// 埋め込まれているときは officework が呼びます。
    /// **返す型は [`gpui::AnyElement`] です**(2026-09-04。統合の段8)。
    /// officework は文章の画面と表の画面を同じ形で置くので、
    /// 片方が `Div`・もう片方が `Stateful<Div>` だと受けられません
    pub fn file_pane(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
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
            let field = |this: &Writer, i: usize, ed: &Editor, w: f32, ph: &'static str| {
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
            let push_btn = |id: &'static str, label_text: SharedString| {
                div().id(id).px_3().py_1().rounded_sm().cursor_pointer()
                    .border_1().border_color(th_btn).text_color(th_btn)
                    .text_size(px(us * 12.0))
                    .hover(move |s| s.bg(th_btn_hover))
                    .child(label_text)
            };
            pane = pane
                .child(div().text_size(px(us * 16.0)).font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("search_folder")))
                .child(div().flex().flex_row().items_center().gap_2()
                    .child(field(self, 0, &self.fd_term, 280.0, "探す字"))
                    .child(field(self, 1, &self.fd_glob, 120.0, "*.txt"))
                    .child(push_btn("fd-dir", ui::t!("choose_folder").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.find_dir_dialog(cx); cx.notify() })))
                    .child(push_btn("fd-go", ui::t!("search_enter").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.find_in_folder(); cx.notify() }))))
                .child(div().text_size(px(us * 11.5)).text_color(th_status)
                    .child(SharedString::from(match self.find_dir() {
                        Some(d) => ui::tf!("folder_2", d.display()).to_string(),
                        None => ui::t!("no_folder_chosen_yet").to_string(),
                    })));
            // 当たりの一覧(ファイルごとに見出し + 行番号つきの行)
            let mut list = div().id("fd-list")
                .flex_none().h(px(us * 320.0)).overflow_y_scroll()
                .p_2().rounded_sm().bg(gpui::white())
                .border_1().border_color(th_cmd_border)
                .flex().flex_col().gap_0p5().text_size(px(us * 12.0));
            if self.fd_hits.is_empty() {
                list = list.child(div().text_color(th_status)
                    .child(ui::t!("nothing_searched_yet")));
            }
            self.fd_box.borrow_mut().clear();
            for (fi, f) in self.fd_hits.iter().enumerate() {
                list = list.child(div().mt_1().text_color(th_btn)
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
                    list = list.child(div()
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
            pane = pane.child(list);
            // **下の窓と「読み込み」**(発注者 2026-08-17
            // 「下に読み込みボタンを置くのはどうか」)。見て、これだと
            // 分かってから開く — 押し間違いで文書が入れ替わらない
            pane = pane.child(div().flex().flex_row().items_center().gap_2()
                .child(push_btn("fd-load", ui::t!("load").into()).on_click(
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
            // **詳細設定は行の表で持ちます**(統合の段8。2026-09-04)。
            // 描くのは [`ui::filemenu::options`] の1本で、表の画面と共通です。
            // 前は同じ 220 行が両方に写してあり、8行のうち7行が同じ物でした。
            //
            // 器は `~/.config/officework/settings.toml`
            // (SEKKEI「設定 — 器と言語」。環境変数が一時上書きで優先)
            let rows = self.option_rows();
            pane = pane.child(ui::filemenu::options(
                &ui::filemenu::OptLook { dim: th_status, chip: item_bg, scale: us },
                &ui::t!("advanced_settings"),
                &ui::tf!("location", ui::settings::path().display()),
                &rows,
                cx,
                |this: &mut Writer, id: String, cx| this.option_click(&id, cx),
            ));
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
            let user_place =
                ui::settings::dir().join(kumihan::theme::user_template_name());
            let user = kumihan::theme::read_theme(&user_place);
            let folder_place = self
                .tmpl_path
                .clone()
                .or_else(|| self.folder().map(|d| d.join(Self::FOLDER_TEMPLATE)));
            let folder = folder_place.as_deref().and_then(kumihan::theme::read_theme);
            // **書体と大きさは言語で変わります**(2026-08-26 発注者)。
            // その段が `[文書.en]` のような言語ごとの分を持っているときは、
            // いまの言語の分を出します — 出さないと、画面の数字と
            // ファイルの中身が食い違って見えます
            let lang = ui::language();
            let notes = |th: Option<&kumihan::theme::Theme>| -> Option<String> {
                let th = th?.for_language(lang);
                match (th.font, th.size_pt) {
                    (Some(f), Some(s)) => Some(format!("{f} {s}pt")),
                    (Some(f), None) => Some(f),
                    (None, Some(s)) => Some(format!("{s}pt")),
                    (None, None) => None,
                }
            };
            let tab = |name: &str, value: Option<String>, place: Option<String>| {
                let value = value.unwrap_or_else(|| ui::t!("not_specified").to_string());
                div().flex().flex_col().gap_1().pb_2()
                    .child(div().flex().flex_row().items_center().gap_2()
                        .child(div().w(px(us * 120.0)).text_color(th_status)
                            .child(SharedString::from(name.to_string())))
                        .child(div().child(SharedString::from(value))))
                    .child(div().pl(px(us * 120.0)).text_size(px(us * 10.0))
                        .text_color(th_status)
                        .child(SharedString::from(
                            place.unwrap_or_else(|| ui::t!("not_created_yet").to_string()))))
            };
            pane = pane
                .child(div().text_size(px(us * 16.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("formatting_defaults")))
                .child(div().text_color(th_status)
                    .child(ui::t!("each_level_fills_what")))
                .child(div().text_color(th_status).text_size(px(us * 11.0))
                    .child(ui::tf!("current_language_font_size", lang)))
                .child(div().h(px(us * 8.0)))
                // 1段目 — この文書
                .child(tab(ui::t!("document_5"), self.doc.font.clone(),
                          Some(ui::t!("font_stored_document_open").to_string())))
                // 2段目 — 綴り
                .child(tab(ui::t!("folder_4"),
                          notes(folder.as_ref()),
                          folder_place.map(|p| p.display().to_string())))
                // 3段目 — 利用者
                .child(tab(ui::t!("account_computer"),
                          notes(user.as_ref()),
                          Some(user_place.display().to_string())))
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
            for (i, (name, path)) in list.into_iter().enumerate() {
                let name2 = name.clone();
                pane = pane.child(
                    ui::filemenu::recent_row(&look, i, std::path::Path::new(&name))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tab = this.prev_tab;
                            this.open(path.clone());
                            // **控えを原本と取り違えないよう、道は持たせない**
                            this.path = None;
                            this.dirty = true;
                            this.status = ui::tf!(
                                "opened_backup_original_check",
                                name2.clone()
                            )
                            .into();
                            cx.notify()
                        })),
                );
            }
            // **壊れたまま拾って開く。** 読めないファイルから字だけを拾い、
            // 読み取り専用で開く。道は持たせないので、元のファイルは
            // 上書きしない(保存は名前を付けて)
            pane = pane.child(div().h(px(us * 8.0)));
            pane = pane.child(div().id("salvage-open")
                .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                .child(ui::t!("salvage_open_read_only"))
                .on_click(cx.listener(|_, _, _, cx| {
                    let ask = cx.background_executor().spawn(async {
                        rfd::FileDialog::new().pick_file()
                    });
                    cx.spawn(async move |this, cx| {
                        let r = ask.await;
                        let _ = this.update(cx, |this, cx| {
                            if let Some(p) = r {
                                this.tab = this.prev_tab;
                                this.salvage_open(&p);
                            }
                            cx.notify();
                        });
                    })
                    .detach();
                })));
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
        pane.into_any_element()
    }
}

/// 壊れたファイルから字だけを拾う。docx(zip)なら `word/document.xml` の

impl Writer {
    /// **詳細設定に並べる行**(統合の段8。2026-09-04)。
    ///
    /// 表の画面の同じ関数と**並びを揃えます**。違うのは、表だけが持つ
    /// 「参照形式」の行が向こうにあることだけです
    fn option_rows(&self) -> Vec<ui::filemenu::OptRow> {
        use ui::filemenu::{OptCell, OptRow};
        let lang = ui::settings::get("language").unwrap_or_else(|| "ja".into());
        let mut rows = vec![
            // 札ではなく**その言語自身の名前**を出します。`pt` と `pt-br` は
            // 札のままでは見分けられません
            OptRow::one(ui::t!("language_ribbon_messages"), "set-lang",
                        ui::language_label(&lang).to_string()),
            // **画面の明暗**(2026-08-20 発注者)。**紙は白いまま** —
            // 暗くするのは周りだけです
            OptRow::one(ui::t!("interface_theme_light_dark"), "set-theme",
                        if self.dark { ui::t!("dark") } else { ui::t!("light") }),
            // **画面の文字の大きさ**(Ctrl+= / Ctrl+- と同じ実体)。紙は変わりません
            OptRow {
                label: ui::t!("ui_text_size").to_string(),
                cells: vec![
                    OptCell::Button { id: "set-ui-minus".into(), text: "−".into() },
                    OptCell::Text(format!("{}%", (self.ui_scale * 100.0).round() as i32)),
                    OptCell::Button { id: "set-ui-plus".into(), text: "+".into() },
                ],
                gap: false,
            },
            // **コメントの名乗り。** 器は表と同じ `user_name` です。
            // **未設定なら名乗りません** — 機械のユーザー名は使いません
            OptRow::one(ui::t!("comment_signature"), "set-username", {
                let a = ui::comment_author();
                if a.is_empty() { ui::t!("anonymous").to_string() } else { a }
            }),
            // **反復計算。** 文書の表もセル関数を持つので、循環参照を回して
            // 解く道は表計算と同じに要ります。器はアプリの設定です —
            // `.adoc` の文書には xlsx の `calcPr` に当たる置き場がありません
            OptRow::one(ui::t!("iterative_calculation_circular_references"), "set-iter",
                        match ui::calc_iter_setting() {
                            Some((n, _)) => ui::tf!("up_passes", n),
                            None => ui::t!("off_switch").to_string(),
                        }),
            // **数学オートコレクト。** 仕掛けは前から共通で、表だけが名乗って
            // いました。器も表と同じ1つの綴りを見ます
            OptRow::one(ui::t!("math_autocorrect"), "set-autocorrect",
                        if self.autocorrect { ui::t!("type_alpha_get") } else { ui::t!("off_switch") }),
            // **AI の宛先**(2026-08-15 発注者)。表の画面と同じ形です
            // **使う Python は人が選べます**(2026-09-04 発注者「自由に環境が
            // 選択できるのがいい」)。同梱はやめたので、機械にある物か、
            // 開いているフォルダの `.venv` を使います。
            //
            // 押すと**この機械で見つかった物が並びます**。既定は開いている
            // フォルダの `.venv` です(いちばん上)
            OptRow::one(ui::t!("python_location"), "set-python",
                        pyrun::find_python().display().to_string()),
            OptRow::one(ui::t!("ai_destination"), "set-ai",
                        ui::ai::backend().label().to_string()).gap(),
        ];
        // **見るだけの7行は ui::env_rows の1本**(2026-08-20)。
        // 鍵そのものは出しません。手元のモデルだけは「使えます」と言いません
        // (繋がるか確かめずに言えば嘘になります)
        // **見つかった Python を選ばせます**(押している間だけ並べます)
        if let Some(cands) = &self.py_picking {
            let now = pyrun::find_python();
            for (i, c) in cands.iter().enumerate() {
                let mark = if *c == now { "● " } else { "○ " };
                rows.push(OptRow::one(
                    format!("{mark}{}", c.display()),
                    // 番号は押しの側で拾います(`python:3`)
                    format!("python:{i}"),
                    ui::t!("use_this").to_string(),
                ));
            }
        }
        rows.extend(ui::env_rows(&lock_identity()).into_iter().map(|(k, v)| OptRow::view(k, v)));
        rows
    }

    /// 詳細設定の行が押されたとき。
    fn option_click(&mut self, id: &str, cx: &mut Context<Self>) {
        match id {
            "set-lang" => self.status = ui::cycle_language().into(),
            "set-theme" => self.run_cmd("darkmode", cx),
            "set-ui-minus" => self.run_cmd("ui-smaller", cx),
            "set-ui-plus" => self.run_cmd("ui-bigger", cx),
            "set-username" => {
                self.cmt_name_edit = true;
                self.cmt_name_ed = Editor::new(&ui::comment_author());
                self.status = ui::t!("type_name_press_enter").into();
            }
            "set-iter" => {
                let (_, msg) = ui::toggle_calc_iter(ui::calc_iter_setting(), !cfg!(test));
                self.status = msg.into();
                self.lay();
            }
            "set-autocorrect" => {
                let (on, msg) = ui::toggle_math_autocorrect(self.autocorrect, !cfg!(test));
                self.autocorrect = on;
                self.status = msg.into();
            }
            // **Python を選び直す**(2026-09-04)。設定に控えて、その場から効かせます。
            // 空にすると「決めていない」に戻り、いつもの探し方(開いている
            // フォルダの `.venv` → python3)になります
            "set-python" => {
                self.py_picking = match self.py_picking {
                    Some(_) => None,
                    // 探すのは**開く時だけ**です(描くたびに走ると遅くなります)
                    None => Some(pyrun::python_candidates()),
                };
            }
            id if id.starts_with("python:") => {
                if let (Ok(i), Some(c)) =
                    (id["python:".len()..].parse::<usize>(), self.py_picking.clone())
                {
                    if let Some(p) = c.get(i) {
                        ui::settings::set("python", &p.display().to_string());
                        pyrun::set_python(Some(p.clone()));
                        self.status = ui::tf!("python_chosen", p.display().to_string()).into();
                    }
                }
                self.py_picking = None;
            }
            "set-ai" => self.run_cmd("ai-where", cx),
            // 見るだけの行は押せません(押せる物だけが id を持ちます)
            _ => {}
        }
    }
}

/// 段落ごとの字、それ以外は文字として読める分。返すのは段落の並び
pub(crate) fn salvage_text(bytes: &[u8]) -> Vec<String> {
    // zip なら document.xml を探す。読める部品が無ければ空
    if bytes.starts_with(b"PK") {
        let Some(xml) = document_xml_bytes(bytes) else { return Vec::new() };
        let xml = String::from_utf8_lossy(&xml);
        return xml
            .split("</w:p>")
            .map(texts_in)
            .filter(|t| !t.trim().is_empty())
            .collect();
    }
    let text = String::from_utf8_lossy(bytes);
    text.lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.trim().is_empty() && l.chars().all(|c| !c.is_control() || c == '\t'))
        .collect()
}

/// zip から `word/document.xml` の中身を取り出す。目次(central directory)が
/// 壊れていても、頭から順に部品を読んで探す(後ろが欠けた docx はこれで拾える)
fn document_xml_bytes(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    if let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) {
        if let Ok(mut f) = z.by_name("word/document.xml") {
            let mut xml = Vec::new();
            if f.read_to_end(&mut xml).is_ok() {
                return Some(xml);
            }
        }
    }
    let mut cur = std::io::Cursor::new(bytes);
    while let Ok(Some(mut f)) = zip::read::read_zipfile_from_stream(&mut cur) {
        if f.name() == "word/document.xml" {
            let mut xml = Vec::new();
            return f.read_to_end(&mut xml).ok().map(|_| xml);
        }
    }
    None
}

/// `<w:t>` の中の字をつなぐ(壊れた XML でも、タグの間の字だけを拾う)
fn texts_in(xml: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<w:t") {
        let after = &rest[i + 4..];
        // <w:t> か <w:t xml:space=...>。<w:tab/> や <w:tbl> は違う
        let Some(gt) = after.find('>') else { break };
        let head = &after[..gt];
        if !(head.is_empty() || head.starts_with(' ')) || head.ends_with('/') {
            rest = &after[gt + 1..];
            continue;
        }
        let body = &after[gt + 1..];
        let Some(end) = body.find("</w:t>") else { break };
        out.push_str(&unescape(&body[..end]));
        rest = &body[end + 6..];
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

impl Writer {
    /// **壊れたまま拾って開く(読み取り専用)。** 普通に読めれば普通に
    /// 開く。読めなければ字だけを拾い、道を持たない読み取り専用の文書に
    /// する。保存しても元のファイルは上書きしない
    pub(crate) fn salvage_open(&mut self, p: &std::path::Path) {
        let bytes = match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => {
                self.status = ui::tf!("cant_open", e).into();
                return;
            }
        };
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        let paras = salvage_text(&bytes);
        if paras.is_empty() {
            self.status = ui::tf!("salvage_nothing", name).into();
            return;
        }
        let n = paras.len();
        let mut doc = Document::plain(&paras.join("\n"));
        doc.protection = Some("readOnly".into());
        self.target = Target::Body;
        self.native = false;
        self.hf_edit = None;
        self.track = false;
        self.track_base = None;
        self.set_doc(doc);
        self.path = None;
        self.dirty = true;
        self.notes.clear();
        self.status = ui::tf!("salvaged_opened", n, name).into();
    }
}
