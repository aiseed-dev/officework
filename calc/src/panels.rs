//! 左右のパネル(2026-08-15 発注者「左右のパネルを整備して、AI も使えるように」)。
//!
//! - **左 = 対話する相手**(いまは AI の会話)
//! - **右 = 選んでいる物の設定**(いまはセルの設定)
//!
//! 決めの出どころは docs/sekkei/ui.ja.md。**右は「いる場所の設定」**なので、
//! 小窓や一覧に散っていた物を寄せる。開きっぱなしなので**連打で効く** —
//! 罫線のように「ペンを選んだまま何箇所にも引く」仕事がここで生きる。
//!
//! **枠だけ作らない**(発注者「枠だけ作っても意味ない」)。この便で
//! 塗り・文字・揃え・表示形式・罫線(場所×ペン)と、会話が動く。
//!
//! パネルは格子に**重ねない — 横に並んで場所を取る**。重ねた最初の版は
//! 実機で行番号と A・B 列を隠し、右のパネルは面の `overflow_hidden` に
//! 切られて出てこなかった(2026-08-15)。置き場は view.rs の
//! 「格子の面」を包む横並びの中。
use gpui::prelude::*;
use gpui::{div, px, rgb, Context, SharedString, Window};

use crate::Calc;

/// パネルの幅(px)。writer の 250 と揃える
const W: f32 = 250.0;
/// 外側の柱の幅(px)。アイコン1つぶん
const RAIL: f32 = 34.0;

impl Calc {
    /// 左右のパネルを組む。返りは (左, 右)
    pub(crate) fn panels(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Option<gpui::AnyElement>, Option<gpui::AnyElement>) {
        let dk = self.dark;
        let us = self.ui_scale;
        let bg = if dk { rgb(0x1B1E21) } else { rgb(0xF1F3F5) };
        let line = if dk { rgb(0x33383D) } else { rgb(0xE1E6EA) };
        let fg = if dk { rgb(0xCFD6DC) } else { rgb(0x444B52) };
        let faint = if dk { rgb(0x8A939B) } else { rgb(0x767E86) };
        let accent = rgb(0x1B6E3C);

        // 小さな見出し
        let heading = move |t: String| {
            div().text_size(px(us * 10.5)).text_color(faint).mt_2().mb_0p5().child(t)
        };
        // 押せる小さなボタン
        let button = move |id: &'static str, t: String, enabled: bool| {
            div()
                .id(SharedString::from(id))
                .px_2().py_0p5().rounded_sm().cursor_pointer()
                .text_size(px(us * 11.5))
                .text_color(if enabled { fg } else { faint })
                .border_1()
                .border_color(if enabled { accent } else { line })
                .hover(move |s| s.bg(if dk { rgb(0x2C333A) } else { rgb(0xEAF5EE) }))
                .child(t)
        };
        let row_box = || div().flex().flex_row().flex_wrap().gap_1();
        // **外側の柱。** 面を切り替えるアイコンを縦に並べる
        // (発注者 2026-08-15「左右のパネルの外側にアイコンをおいて
        // 操作を変更できるように」)。ONLYOFFICE と同じ置き方
        let rail = || div().flex_none().w(px(RAIL * us)).h_full()
            .flex().flex_col().items_center().gap_1().py_1();
        let rail_button = move |id: &'static str, icon: &'static str, label_text: String, on: bool| {
            div()
                .id(SharedString::from(id))
                .w(px(RAIL * us - 8.0)).h(px(RAIL * us - 8.0))
                .rounded_sm().cursor_pointer()
                .flex().items_center().justify_center()
                .bg(if on {
                    if dk { rgb(0x2C333A) } else { rgb(0xFFFFFF) }
                } else {
                    gpui::transparent_black().into()
                })
                .border_1()
                .border_color(if on { accent } else { gpui::transparent_black().into() })
                .hover(move |s| s.bg(if dk { rgb(0x2C333A) } else { rgb(0xEAF5EE) }))
                .tooltip(move |_, cx| cx.new(|_| crate::view::Tip(label_text.clone().into(), us)).into())
                .tooltip_show_delay(std::time::Duration::from_millis(150))
                .child(gpui::svg()
                    .path(SharedString::from(format!("icons/{icon}.svg")))
                    .size(px(us * 18.0))
                    .text_color(if on { accent } else { faint }))
        };

        // ── 右: セルの設定 ─────────────────────────────────────────
        let right_of = if !self.right_open {
            None
        } else {
            let f = self.sheet().get(self.cursor).map(|c| c.fmt.clone()).unwrap_or_default();
            // **外枠を回す**(発注者 2026-08-15)。内側の1辺だけだと窓の
            // 地とパネルが地続きに見え、どこまでがパネルか分からなかった。
            // 少し内側に置いて四方を囲む — 枠が窓の縁に潰されない
            let face = self.right_face;
            let mut d = div()
                .id("right-panel")
                .flex_1().min_w(px(0.0)).h_full().overflow_y_scroll()
                .p_2()
                .flex().flex_col().gap_0p5();
            if face == 2 {
                // ── フォルダの中身 ───────────────────────────────
                //
                // **文章の画面と同じ物**(2026-08-19)。表を押せばここで開き、
                // 文書を押せば officework が受け取って文章の画面にします。
                // これで表と文章を行き来できます
                d = d.child(div().text_size(px(us * 12.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(fg).child(ui::t!("files_what_folder").to_string()));
                // **一覧は ui::filelist の1本**(統合の段7)。文章の画面と同じ姿。
                // 押したときの行き先だけがアプリの物
                let look = ui::filelist::Look {
                    fg, dim: faint, hover: line, scale: us,
                };
                let dir = self.folder();
                d = d.child(ui::filelist::header(&look, dir.as_deref()));
                if let Some(dir) = dir.as_deref() {
                    // **上のフォルダへ戻れます**(2026-08-26)。
                    // 中へ入れても戻れないと、一方通行です
                    if let Some(top) = ui::filelist::up_row(&look, dir) {
                        let parent = dir.parent().map(|p| p.to_path_buf());
                        d = d.child(top.on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(parent) = parent.clone() {
                                this.show_folder(parent);
                            }
                            cx.notify()
                        })));
                    }
                    // **作る道**(2026-08-26 発注者)
                    d = d.child(
                        div().flex().flex_row().gap_1().pb_1()
                            .child(ui::filelist::make_button(&look, "folder",
                                ui::t!("folder").into())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.fl_start(crate::FlJob::NewFolder);
                                    cx.notify()
                                })))
                            .child(ui::filelist::make_button(&look, "sheet",
                                ui::t!("sheet_2").into())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.fl_start(crate::FlJob::NewSheet);
                                    cx.notify()
                                }))),
                    );
                    let (list, rest) = ui::filelist::entries_with_rest(dir);
                    if list.is_empty() {
                        d = d.child(ui::filelist::empty(&look));
                    }
                    for (i, e) in list.into_iter().enumerate() {
                        let can_open = e.kind.can_open();
                        let is_a_doc = e.kind.is_doc();
                        let path = e.path.clone();
                        // **フォルダは中へ入ります**(2026-08-26 発注者)
                        if e.kind == ui::folder::Kind::Folder {
                            let line = ui::filelist::row(&look, i, &e, false)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.show_folder(path.clone());
                                    cx.notify()
                                }));
                            d = d.child(line);
                            continue;
                        }
                        let current = self.path.as_deref() == Some(e.path.as_path());
                        let mut line = ui::filelist::row(&look, i, &e, current);
                        line = line.on_click(cx.listener(move |this, _, _, cx| {
                            this.remember_folder();
                            if !can_open {
                                // **こちらで開けない種類は、機械の関連付けに渡します**
                                // (2026-08-24 発注者「何のツールでも使えるようにする」)。
                                // writer の一覧と同じ扱いです
                                // **機械の関連付けに渡します。** `open_for_edit` は
                                // 使いません — あれは「.py を編集する道具」の道で、
                                // 隣の writer に落ちます。実機で押したら .ipynb が
                                // writer で開きました(2026-08-24)。JupyterLab で
                                // 開くべき物なので、機械の決めをそのまま使います
                                this.status = match ui::open_outside(&path.display().to_string()) {
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
                            if this.embedded || is_a_doc {
                                this.open_request = Some(path.clone());
                            } else {
                                this.open(path.clone());
                            }
                            cx.notify()
                        }));
                        let path2 = e.path.clone();
                        let path3 = e.path.clone();
                        d = d.child(
                            div().flex().flex_row().items_center().gap_1()
                                .child(div().flex_1().min_w(px(0.0)).child(line))
                                .child(ui::filelist::row_button(&look, i, "ren",
                                    ui::t!("name").into())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.fl_start(crate::FlJob::Rename(path2.clone()));
                                        cx.notify()
                                    })))
                                .child(ui::filelist::row_button(&look, i, "del",
                                    ui::t!("erase").into())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.fl_start(crate::FlJob::Delete(path3.clone()));
                                        cx.notify()
                                    }))),
                        );
                    }
                    // **切った分は言います**(2026-08-26)
                    if let Some(note_div) = ui::filelist::rest_note(&look, rest) {
                        d = d.child(note_div);
                    }
                }
            } else if face == 1 {
                // ── 図形と画像 ───────────────────────────────────
                d = d.child(div().text_size(px(us * 12.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(fg).child(ui::t!("shapes_images").to_string()));
                let shape_of = self.shape_sel;
                let icon = self.img_sel;
                if shape_of.is_none() && icon.is_none() {
                    // **選んでいないと言う。** 押せない釦を並べて黙るより、
                    // 何をすれば効くかを書く
                    d = d.child(div().text_size(px(us * 11.0)).text_color(faint).child(
                        ui::t!("no_shape_picture_selected")
                            .to_string()));
                } else {
                    d = d.child(div().text_size(px(us * 10.5)).text_color(faint)
                        .child(if shape_of.is_some() {
                            ui::t!("shape_selected").to_string()
                        } else {
                            ui::t!("picture_selected").to_string()
                        }));
                    d = d.child(heading(ui::t!("stacking").to_string()));
                    let mut r = row_box();
                    for (id, label_text, act) in [
                        ("sp-front", ui::t!("bring_front"), "sh-front"),
                        ("sp-fwd", ui::t!("bring_forward"), "sh-forward"),
                        ("sp-bwd", ui::t!("send_backward"), "sh-backward"),
                        ("sp-back", ui::t!("send_back"), "sh-back"),
                    ] {
                        r = r.child(button(id, label_text.to_string(), false).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.shape_menu_action(act);
                                cx.notify()
                            })));
                    }
                    d = d.child(r);
                    d = d.child(heading(ui::t!("orientation").to_string()));
                    let mut r = row_box();
                    for (id, label_text, act) in [
                        ("sp-rot-l", ui::t!("rotate_left"), "sh-rot-l"),
                        ("sp-rot-r", ui::t!("rotate_right"), "sh-rot-r"),
                        ("sp-flip-h", ui::t!("flip_horizontally"), "sh-flip-h"),
                        ("sp-flip-v", ui::t!("flip_vertically"), "sh-flip-v"),
                    ] {
                        r = r.child(button(id, label_text.to_string(), false).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.shape_menu_action(act);
                                cx.notify()
                            })));
                    }
                    d = d.child(r);
                    d = d.child(heading(ui::t!("other_2").to_string()));
                    d = d.child(row_box()
                        .child(button("sp-del", ui::t!("erase").to_string(), false).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.shape_menu_action("sh-del");
                                cx.notify()
                            }))));
                }
            } else {
            d = d.child(div().text_size(px(us * 12.5)).font_weight(gpui::FontWeight::BOLD)
                .text_color(fg).child(ui::t!("cell_settings").to_string()));
            d = d.child(div().text_size(px(us * 10.5)).text_color(faint)
                .child(ui::tf!("now", self.sel_label()).to_string()));

            // 文字
            d = d.child(heading(ui::t!("character").to_string()));
            let mut r = row_box();
            for (id, label_text, on) in [
                ("rp-bold", ui::t!("bold"), f.bold),
                ("rp-italic", ui::t!("italic"), f.italic),
                ("rp-under", ui::t!("underline"), f.underline),
            ] {
                let cmd = match id {
                    "rp-bold" => "bold",
                    "rp-italic" => "italic",
                    _ => "underline",
                };
                r = r.child(button(id, label_text.to_string(), on).on_click(
                    cx.listener(move |this, _, _, cx| { this.run_cmd(cmd, cx); cx.notify() })));
            }
            d = d.child(r);

            // 揃え
            d = d.child(heading(ui::t!("alignment").to_string()));
            let mut r = row_box();
            for (id, label_text, cmd, on) in [
                ("rp-al", ui::t!("left"), "align-left", f.align == kumihan::book::HAlign::Left),
                ("rp-ac", ui::t!("centre"), "align-center", f.align == kumihan::book::HAlign::Center),
                ("rp-ar", ui::t!("right"), "align-right", f.align == kumihan::book::HAlign::Right),
                ("rp-wrap", ui::t!("wrap"), "wrap", f.wrap),
            ] {
                r = r.child(button(id, label_text.to_string(), on).on_click(
                    cx.listener(move |this, _, _, cx| { this.run_cmd(cmd, cx); cx.notify() })));
            }
            d = d.child(r);

            // 表示形式(よく使う物だけ。全部は小窓に残す)
            d = d.child(heading(ui::t!("number_format").to_string()));
            let now = f.number_format.clone().unwrap_or_default();
            let mut r = row_box();
            for (id, label_text, code) in [
                ("nf-std", ui::t!("normal"), ""),
                ("nf-yen", ui::t!("currency"), "¥#,##0"),
                ("nf-comma", ui::t!("thousands_separator"), "#,##0"),
                ("nf-pct", ui::t!("percent"), "0.00%"),
                ("nf-code", ui::t!("item_code_0000"), "0000"),
                ("nf-date", ui::t!("date"), "yyyy/m/d"),
            ] {
                let on = now == code;
                let c = code.to_string();
                r = r.child(button(id, label_text.to_string(), on).on_click(
                    cx.listener(move |this, _, _, cx| { this.set_number_format(&c); cx.notify() })));
            }
            d = d.child(r);

            // 罫線 — **場所 × ペン**(うちの直交モデル。MS の型スタンプは持たない)
            d = d.child(heading(ui::t!("border_pen").to_string()));
            let cur_stroke = self.pen_style;
            let mut r = row_box();
            for (id, label_text, st) in [
                ("pen-thin", ui::t!("thin"), kumihan::book::BStyle::Thin),
                ("pen-medium", ui::t!("middle"), kumihan::book::BStyle::Medium),
                ("pen-thick", ui::t!("thick"), kumihan::book::BStyle::Thick),
                ("pen-dashed", ui::t!("dashed"), kumihan::book::BStyle::Dashed),
                ("pen-double", ui::t!("double"), kumihan::book::BStyle::Double),
            ] {
                r = r.child(button(id, label_text.to_string(), cur_stroke == st).on_click(
                    cx.listener(move |this, _, _, cx| { this.pen_style = st; cx.notify() })));
            }
            d = d.child(r);
            d = d.child(heading(ui::t!("where_draw_press_repeatedly").to_string()));
            let mut r = row_box();
            for (id, label_text, cmd) in [
                ("bd-all", ui::t!("grid"), "border-all"),
                ("bd-out", ui::t!("outline"), "border-outer"),
                ("bd-top", ui::t!("top"), "border-top"),
                ("bd-bottom", ui::t!("bottom"), "border-bottom"),
                ("bd-left", ui::t!("left"), "border-left"),
                ("bd-right", ui::t!("right"), "border-right"),
                ("bd-none", ui::t!("erase"), "border-none"),
            ] {
                r = r.child(button(id, label_text.to_string(), false).on_click(
                    cx.listener(move |this, _, _, cx| { this.run_cmd(cmd, cx); cx.notify() })));
            }
            d = d.child(r);

            // 塗り — **色見本を直に並べる。** 開きっぱなしのパネルなので
            // 「一覧を開いて選んで閉じる」の3手が1手になる
            d = d.child(heading(ui::t!("fill_color").to_string()));
            let cur_fill = f.fill.clone();
            let mut r = row_box();
            for (i, (_, label_text, hex)) in crate::util::fill_colors().into_iter().enumerate() {
                let on = cur_fill.as_deref() == hex;
                let h = hex.map(|s| s.to_string());
                let l = label_text.to_string();
                let sample = match hex {
                    Some(x) => u32::from_str_radix(x, 16).unwrap_or(0xFFFFFF),
                    None => 0xFFFFFF,
                };
                r = r.child(div()
                    .id(SharedString::from(format!("fillsw{i}")))
                    .w(px(us * 20.0)).h(px(us * 20.0)).rounded_sm().cursor_pointer()
                    .bg(rgb(sample))
                    // 色なしは斜めの線でなく「/」の字で示す(絵を増やさない)
                    .border_1().border_color(if on { accent } else { line })
                    .when(on, |s| s.border_2())
                    .flex().items_center().justify_center()
                    .text_size(px(us * 10.0)).text_color(faint)
                    .child(if hex.is_none() { "/" } else { "" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_fill(h.as_deref(), &l);
                        cx.notify()
                    })));
            }
            d = d.child(r);

            // 字下げ — **模型は前からあったのに、掛ける道が無かった。**
            // ここで初めて人の手が届く(1段 = 全角約1字)
            d = d.child(heading(ui::t!("indent_3").to_string()));
            let mut r = row_box();
            r = r.child(button("ind-dec", "−".to_string(), f.indent > 0).on_click(
                cx.listener(|this, _, _, cx| { this.bump_indent(-1); cx.notify() })));
            r = r.child(div().text_size(px(us * 11.5)).text_color(fg)
                .child(format!("{}", f.indent)));
            r = r.child(button("ind-inc", "+".to_string(), true).on_click(
                cx.listener(|this, _, _, cx| { this.bump_indent(1); cx.notify() })));
            d = d.child(r);

            // 文字の向き — 一覧と同じ6つ(鍵も同じ。xlsx の数え方で上向きが正)
            d = d.child(heading(ui::t!("text_orientation").to_string()));
            let cur_angle = f.rotation.unwrap_or(0);
            let mut r = row_box();
            for (id, label_text, deg) in [
                ("rot-0", ui::t!("no_rotation"), 0),
                ("rot-45", ui::t!("rotate_up_45"), 45),
                ("rot-135", ui::t!("rotate_down_45"), 135),
                ("rot-90", ui::t!("rotate_up_90"), 90),
                ("rot-180", ui::t!("rotate_down_90"), 180),
                ("rot-255", ui::t!("vertical_stack_one_character"), 255),
            ] {
                let l = label_text.to_string();
                r = r.child(button(id, label_text.to_string(), cur_angle == deg).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.set_rotation(deg, &l);
                        cx.notify()
                    })));
            }
            d = d.child(r);

            // 条件付き書式 — **値を訊かないものだけ**をここに置く。
            // 「値より大きいと…」のように打ち込みの要る規則は今までどおり
            // リボンの一覧から(小窓が開くので、パネルの連打には向かない)
            d = d.child(heading(ui::t!("conditional_formatting").to_string()));
            let mut r = row_box();
            for (id, label_text, act) in [
                ("cf-neg", ui::t!("negative_red"), "cond-neg"),
                ("cf-dup", ui::t!("duplicates"), "cond-dup"),
                ("cf-uniq", ui::t!("unique"), "cond-uniq"),
                ("cf-avg-a", ui::t!("above_average"), "cond-avg-above"),
                ("cf-avg-b", ui::t!("below_average"), "cond-avg-below"),
                ("cf-bar", ui::t!("data_bar"), "cond-bar"),
                ("cf-scale", ui::t!("colour_scale"), "cond-scale"),
                ("cf-icons", ui::t!("icons"), "cond-icons"),
                ("cf-clear", ui::t!("erase"), "cond-clear"),
            ] {
                r = r.child(button(id, label_text.to_string(), false).on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.menu_action(act, window, cx);
                        cx.notify()
                    })));
            }
            d = d.child(r);
            }
            // **外側の柱**(面を切り替えるアイコン)。パネルの外枠は
            // 柱ごと囲む — 柱もパネルの一部だから
            let rail_div = rail()
                .child(rail_button("rf-cell", "cell-format", ui::t!("cell_settings").to_string(), face == 0)
                    .on_click(cx.listener(|this, _, _, cx| { this.right_face = 0; cx.notify() })))
                .child(rail_button("rf-shape", "insshape", ui::t!("shapes_images").to_string(), face == 1)
                    .on_click(cx.listener(|this, _, _, cx| { this.right_face = 1; cx.notify() })))
                // **フォルダの中身**(2026-08-19)。文章の画面と同じ物
                .child(rail_button("rf-files", "py-folder", ui::t!("files_what_folder").to_string(), face == 2)
                    .on_click(cx.listener(|this, _, _, cx| { this.right_face = 2; cx.notify() })));
            Some(div()
                .flex_none().w(px((W + RAIL) * us)).h_full()
                .m_1().rounded_sm().bg(bg)
                .border_1().border_color(line)
                .flex().flex_row()
                .child(d)
                // 柱は**外側**(窓の縁の側)。仕切りの線を1本
                .child(div().flex_none().w(px(1.0)).h_full().bg(line))
                .child(rail_div)
                .into_any_element())
        };

        // ── 左: 会話 ──────────────────────────────────────────────
        let left = if !self.left_open {
            None
        } else {
            let face = self.left_face;
            // **コメントの面は柱だけ**(発注者 2026-08-15
            // 「コメントの時は、左パネルはアイコンだけの表示でいいのでは」)。
            // コメントはセルの吹き出しで見えるので、板を出す値打ちがない —
            // そのぶん表が広く使える。押したときにコメントの表示も入れる
            let mut d = div()
                .flex_1().min_w(px(0.0)).h_full().overflow_hidden()
                .p_2()
                .flex().flex_col().gap_1();
            if face == 0 {
            // 見出しの行。**新しい会話**は Agent Panel と同じく頭に置く
            d = d.child(div().flex().flex_row().items_center().gap_2()
                .child(div().flex_1().text_size(px(us * 12.5))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(fg).child(ui::t!("ask_ai").to_string()))
                .child(button("chat-new", ui::t!("new_conversation").to_string(), false).on_click(
                    cx.listener(|this, _, _, cx| { this.chat_reset(); cx.notify() }))));
            d = d.child(div().text_size(px(us * 10.5)).text_color(faint).child(
                ui::t!("can_ask_about_selected").to_string()));

            // やりとり
            // **残りの高さを全部使う**(固定の高さ + 余白の詰め物、だと
            // 上に空きが溜まる)。やりとりが増えたらここが伸びて巻物になる
            let mut chat = div().id("chat-log").flex().flex_col().gap_1().mt_1()
                .flex_1().min_h(px(0.0)).overflow_y_scroll();
            if self.chat_log.is_empty() {
                chat = chat.child(div().text_size(px(us * 11.0)).text_color(faint).child(
                    ui::t!("e_g_sort_sales").to_string()));
            }
            for (self_of, text) in &self.chat_log {
                chat = chat.child(
                    div().text_size(px(us * 11.5))
                        .text_color(if *self_of { accent } else { fg })
                        .child(format!("{} {}", if *self_of { "▸" } else { "◂" }, text)));
            }
            d = d.child(chat);

            // **落ちたら直してもらう。** 誤りを添えて頼み直す一押し —
            // 走らせて直す、が Agent Panel の芯(2026-08-16)
            if self.chat_err.is_some() {
                d = d.child(row_box().mt_1()
                    .child(button("chat-fix", ui::t!("ask_fix").to_string(), true).on_click(
                        cx.listener(|this, _, _, cx| { this.chat_fix(cx); cx.notify() })))
                    .child(button("chat-err-drop", ui::t!("leave").to_string(), false)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.chat_err = None;
                            cx.notify()
                        }))));
            }
            // 変更案(Python)。**押すまで走らない**
            if let Some(plan) = self.chat_plan.clone() {
                d = d.child(heading(ui::t!("proposed_change_nothing_happens").to_string()));
                d = d.child(div().id("chat-plan")
                    .max_h(px(us * 150.0)).overflow_y_scroll()
                    .p_1().rounded_sm()
                    .bg(if dk { rgb(0x14171A) } else { rgb(0xFFFFFF) })
                    .border_1().border_color(line)
                    .text_size(px(us * 10.5)).text_color(fg)
                    .children(plan.lines().map(|l| div().child(l.to_string()))));
                let mut r = row_box().mt_1();
                r = r.child(button("chat-run", ui::t!("apply").to_string(), true).on_click(
                    cx.listener(|this, _, _, cx| { this.chat_run(cx); cx.notify() })));
                r = r.child(button("chat-drop", ui::t!("cancel").to_string(), false).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.chat_plan = None;
                        this.status = ui::t!("proposed_change_discarded_nothing").into();
                        cx.notify()
                    })));
                d = d.child(r);
            }

            // 入力
            d = d.child(div()
                .p_1().rounded_sm()
                .bg(if dk { rgb(0x14171A) } else { rgb(0xFFFFFF) })
                .border_1().border_color(if self.chat_focus { accent } else { line })
                .text_size(px(us * 11.5)).text_color(fg)
                .id("chat-in")
                .cursor_text()
                .on_click(cx.listener(|this, _, _, cx| { this.chat_focus = true; cx.notify() }))
                // 焦点があるときは打った所に「|」を差す(fn_dlg と同じ描き方)
                .child(if self.chat_in.text().is_empty() {
                    if self.chat_focus {
                        "|".to_string()
                    } else {
                        ui::t!("click_here_type_enter").to_string()
                    }
                } else if self.chat_focus {
                    let mut t = self.chat_in.text().to_string();
                    let cur = self.chat_in.cursor().min(t.len());
                    t.insert(cur, '|');
                    t
                } else {
                    self.chat_in.text().to_string()
                }));
            let mut r = row_box().mt_1();
            r = r.child(button("chat-send", ui::t!("send").to_string(), !self.ai_busy).on_click(
                cx.listener(|this, _, _, cx| { this.chat_send(cx); cx.notify() })));
            if self.ai_busy {
                r = r.child(div().text_size(px(us * 10.5)).text_color(faint)
                    .child(ui::t!("thinking").to_string()));
            }
            d = d.child(r);
            // **宛先はここで替える**(Agent Panel はモデルを下に出す)。
            // 詳細設定まで行かずに、話しながら切り替えられる
            let addressee = ui::ai::backend();
            d = d.child(div()
                .id("chat-where")
                .mt_1().px_1().py_0p5().rounded_sm().cursor_pointer()
                .text_size(px(us * 10.5)).text_color(faint)
                .hover(move |s| s.bg(if dk { rgb(0x2C333A) } else { rgb(0xEAF5EE) }))
                .child(ui::tf!("destination_press_change", addressee.label()).to_string())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.run_cmd("ai-where", cx);
                    cx.notify()
                })));
            }
            // **外側の柱**(左パネルは窓の左端の側)。会話とコメントを切り替える
            let rail_div = rail()
                .child(rail_button("lf-ai", "ai-ask", ui::t!("ask_ai").to_string(), face == 0)
                    .on_click(cx.listener(|this, _, _, cx| { this.left_face = 0; cx.notify() })))
                .child(rail_button("lf-cmt", "co-showcomment", ui::t!("comment").to_string(), face == 1)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.left_face = 1;
                        // **コメントを見えるようにする。** 押したのに何も
                        // 起きないと、切り替わったのか分からない
                        this.show_comments = true;
                        this.status = ui::t!("showing_comments_cell_balloons").into();
                        cx.notify()
                    })));
            let mut wrapper = div()
                .flex_none()
                .w(px((if face == 0 { W + RAIL } else { RAIL }) * us))
                .h_full()
                .m_1().rounded_sm().bg(bg)
                .border_1().border_color(line)
                .flex().flex_row()
                .child(rail_div);
            if face == 0 {
                wrapper = wrapper
                    .child(div().flex_none().w(px(1.0)).h_full().bg(line))
                    .child(d);
            }
            Some(wrapper.into_any_element())
        };
        (left, right_of)
    }
}
