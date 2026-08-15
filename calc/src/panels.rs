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
        let 薄 = if dk { rgb(0x8A939B) } else { rgb(0x767E86) };
        let 主 = rgb(0x1B6E3C);

        // 小さな見出し
        let 見出し = move |t: String| {
            div().text_size(px(us * 10.5)).text_color(薄).mt_2().mb_0p5().child(t)
        };
        // 押せる小さなボタン
        let 釦 = move |id: &'static str, t: String, 効き: bool| {
            div()
                .id(SharedString::from(id))
                .px_2().py_0p5().rounded_sm().cursor_pointer()
                .text_size(px(us * 11.5))
                .text_color(if 効き { fg } else { 薄 })
                .border_1()
                .border_color(if 効き { 主 } else { line })
                .hover(move |s| s.bg(if dk { rgb(0x2C333A) } else { rgb(0xEAF5EE) }))
                .child(t)
        };
        let 列 = || div().flex().flex_row().flex_wrap().gap_1();
        // **外側の柱。** 面を切り替えるアイコンを縦に並べる
        // (発注者 2026-08-15「左右のパネルの外側にアイコンをおいて
        // 操作を変更できるように」)。ONLYOFFICE と同じ置き方
        let 柱 = || div().flex_none().w(px(RAIL * us)).h_full()
            .flex().flex_col().items_center().gap_1().py_1();
        let 柱釦 = move |id: &'static str, icon: &'static str, 札: String, on: bool| {
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
                .border_color(if on { 主 } else { gpui::transparent_black().into() })
                .hover(move |s| s.bg(if dk { rgb(0x2C333A) } else { rgb(0xEAF5EE) }))
                .tooltip(move |_, cx| cx.new(|_| crate::view::Tip(札.clone().into(), us)).into())
                .child(gpui::svg()
                    .path(SharedString::from(format!("icons/{icon}.svg")))
                    .size(px(us * 18.0))
                    .text_color(if on { 主 } else { 薄 }))
        };

        // ── 右: セルの設定 ─────────────────────────────────────────
        let 右 = if !self.right_open {
            None
        } else {
            let f = self.sheet().get(self.cursor).map(|c| c.fmt.clone()).unwrap_or_default();
            // **外枠を回す**(発注者 2026-08-15)。内側の1辺だけだと窓の
            // 地とパネルが地続きに見え、どこまでがパネルか分からなかった。
            // 少し内側に置いて四方を囲む — 枠が窓の縁に潰されない
            let 面 = self.right_face;
            let mut d = div()
                .id("right-panel")
                .flex_1().min_w(px(0.0)).h_full().overflow_y_scroll()
                .p_2()
                .flex().flex_col().gap_0p5();
            if 面 == 1 {
                // ── 図形と画像 ───────────────────────────────────
                d = d.child(div().text_size(px(us * 12.5)).font_weight(gpui::FontWeight::BOLD)
                    .text_color(fg).child(ui::t!("図形と画像").to_string()));
                let 図 = self.shape_sel;
                let 絵 = self.img_sel;
                if 図.is_none() && 絵.is_none() {
                    // **選んでいないと言う。** 押せない釦を並べて黙るより、
                    // 何をすれば効くかを書く
                    d = d.child(div().text_size(px(us * 11.0)).text_color(薄).child(
                        ui::t!("図形も画像も選んでいません(表の上の図形か絵を押してください)")
                            .to_string()));
                } else {
                    d = d.child(div().text_size(px(us * 10.5)).text_color(薄)
                        .child(if 図.is_some() {
                            ui::t!("図形を選んでいます").to_string()
                        } else {
                            ui::t!("画像を選んでいます").to_string()
                        }));
                    d = d.child(見出し(ui::t!("重なり").to_string()));
                    let mut r = 列();
                    for (id, 札, act) in [
                        ("sp-front", ui::t!("最前面へ"), "sh-front"),
                        ("sp-fwd", ui::t!("前へ"), "sh-forward"),
                        ("sp-bwd", ui::t!("後ろへ"), "sh-backward"),
                        ("sp-back", ui::t!("最背面へ"), "sh-back"),
                    ] {
                        r = r.child(釦(id, 札.to_string(), false).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.shape_menu_action(act);
                                cx.notify()
                            })));
                    }
                    d = d.child(r);
                    d = d.child(見出し(ui::t!("向き").to_string()));
                    let mut r = 列();
                    for (id, 札, act) in [
                        ("sp-rot-l", ui::t!("左へ回す"), "sh-rot-l"),
                        ("sp-rot-r", ui::t!("右へ回す"), "sh-rot-r"),
                        ("sp-flip-h", ui::t!("左右を返す"), "sh-flip-h"),
                        ("sp-flip-v", ui::t!("上下を返す"), "sh-flip-v"),
                    ] {
                        r = r.child(釦(id, 札.to_string(), false).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.shape_menu_action(act);
                                cx.notify()
                            })));
                    }
                    d = d.child(r);
                    d = d.child(見出し(ui::t!("そのほか").to_string()));
                    d = d.child(列()
                        .child(釦("sp-del", ui::t!("消す").to_string(), false).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.shape_menu_action("sh-del");
                                cx.notify()
                            }))));
                }
            } else {
            d = d.child(div().text_size(px(us * 12.5)).font_weight(gpui::FontWeight::BOLD)
                .text_color(fg).child(ui::t!("セルの設定").to_string()));
            d = d.child(div().text_size(px(us * 10.5)).text_color(薄)
                .child(ui::tf!("いま: {}", self.sel_label()).to_string()));

            // 文字
            d = d.child(見出し(ui::t!("文字").to_string()));
            let mut r = 列();
            for (id, 札, on) in [
                ("rp-bold", ui::t!("太字"), f.bold),
                ("rp-italic", ui::t!("斜体"), f.italic),
                ("rp-under", ui::t!("下線"), f.underline),
            ] {
                let cmd = match id {
                    "rp-bold" => "bold",
                    "rp-italic" => "italic",
                    _ => "underline",
                };
                r = r.child(釦(id, 札.to_string(), on).on_click(
                    cx.listener(move |this, _, _, cx| { this.run_cmd(cmd, cx); cx.notify() })));
            }
            d = d.child(r);

            // 揃え
            d = d.child(見出し(ui::t!("揃え").to_string()));
            let mut r = 列();
            for (id, 札, cmd, on) in [
                ("rp-al", ui::t!("左"), "align-left", f.align == sheet::model::HAlign::Left),
                ("rp-ac", ui::t!("中央"), "align-center", f.align == sheet::model::HAlign::Center),
                ("rp-ar", ui::t!("右"), "align-right", f.align == sheet::model::HAlign::Right),
                ("rp-wrap", ui::t!("折り返す"), "wrap", f.wrap),
            ] {
                r = r.child(釦(id, 札.to_string(), on).on_click(
                    cx.listener(move |this, _, _, cx| { this.run_cmd(cmd, cx); cx.notify() })));
            }
            d = d.child(r);

            // 表示形式(よく使う物だけ。全部は小窓に残す)
            d = d.child(見出し(ui::t!("表示形式").to_string()));
            let 今 = f.number_format.clone().unwrap_or_default();
            let mut r = 列();
            for (id, 札, code) in [
                ("nf-std", ui::t!("標準"), ""),
                ("nf-yen", ui::t!("通貨"), "¥#,##0"),
                ("nf-comma", ui::t!("桁区切り"), "#,##0"),
                ("nf-pct", ui::t!("パーセント"), "0.00%"),
                ("nf-code", ui::t!("品番(0000)"), "0000"),
                ("nf-date", ui::t!("日付"), "yyyy/m/d"),
            ] {
                let on = 今 == code;
                let c = code.to_string();
                r = r.child(釦(id, 札.to_string(), on).on_click(
                    cx.listener(move |this, _, _, cx| { this.set_number_format(&c); cx.notify() })));
            }
            d = d.child(r);

            // 罫線 — **場所 × ペン**(うちの直交モデル。MS の型スタンプは持たない)
            d = d.child(見出し(ui::t!("罫線のペン").to_string()));
            let 今線 = self.pen_style;
            let mut r = 列();
            for (id, 札, st) in [
                ("pen-thin", ui::t!("細"), sheet::model::BStyle::Thin),
                ("pen-medium", ui::t!("中"), sheet::model::BStyle::Medium),
                ("pen-thick", ui::t!("太"), sheet::model::BStyle::Thick),
                ("pen-dashed", ui::t!("破線"), sheet::model::BStyle::Dashed),
                ("pen-double", ui::t!("二重"), sheet::model::BStyle::Double),
            ] {
                r = r.child(釦(id, 札.to_string(), 今線 == st).on_click(
                    cx.listener(move |this, _, _, cx| { this.pen_style = st; cx.notify() })));
            }
            d = d.child(r);
            d = d.child(見出し(ui::t!("引く場所(続けて押せます)").to_string()));
            let mut r = 列();
            for (id, 札, cmd) in [
                ("bd-all", ui::t!("格子"), "border-all"),
                ("bd-out", ui::t!("外枠"), "border-outer"),
                ("bd-top", ui::t!("上"), "border-top"),
                ("bd-bottom", ui::t!("下"), "border-bottom"),
                ("bd-left", ui::t!("左"), "border-left"),
                ("bd-right", ui::t!("右"), "border-right"),
                ("bd-none", ui::t!("消す"), "border-none"),
            ] {
                r = r.child(釦(id, 札.to_string(), false).on_click(
                    cx.listener(move |this, _, _, cx| { this.run_cmd(cmd, cx); cx.notify() })));
            }
            d = d.child(r);

            // 塗り — **色見本を直に並べる。** 開きっぱなしのパネルなので
            // 「一覧を開いて選んで閉じる」の3手が1手になる
            d = d.child(見出し(ui::t!("塗り").to_string()));
            let 今塗 = f.fill.clone();
            let mut r = 列();
            for (i, (_, 札, hex)) in crate::util::fill_colors().into_iter().enumerate() {
                let on = 今塗.as_deref() == hex;
                let h = hex.map(|s| s.to_string());
                let l = 札.to_string();
                let 見本 = match hex {
                    Some(x) => u32::from_str_radix(x, 16).unwrap_or(0xFFFFFF),
                    None => 0xFFFFFF,
                };
                r = r.child(div()
                    .id(SharedString::from(format!("fillsw{i}")))
                    .w(px(us * 20.0)).h(px(us * 20.0)).rounded_sm().cursor_pointer()
                    .bg(rgb(見本))
                    // 色なしは斜めの線でなく「/」の字で示す(絵を増やさない)
                    .border_1().border_color(if on { 主 } else { line })
                    .when(on, |s| s.border_2())
                    .flex().items_center().justify_center()
                    .text_size(px(us * 10.0)).text_color(薄)
                    .child(if hex.is_none() { "/" } else { "" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_fill(h.as_deref(), &l);
                        cx.notify()
                    })));
            }
            d = d.child(r);

            // 字下げ — **模型は前からあったのに、掛ける道が無かった。**
            // ここで初めて人の手が届く(1段 = 全角約1字)
            d = d.child(見出し(ui::t!("字下げ").to_string()));
            let mut r = 列();
            r = r.child(釦("ind-dec", "−".to_string(), f.indent > 0).on_click(
                cx.listener(|this, _, _, cx| { this.bump_indent(-1); cx.notify() })));
            r = r.child(div().text_size(px(us * 11.5)).text_color(fg)
                .child(format!("{}", f.indent)));
            r = r.child(釦("ind-inc", "+".to_string(), true).on_click(
                cx.listener(|this, _, _, cx| { this.bump_indent(1); cx.notify() })));
            d = d.child(r);

            // 文字の向き — 一覧と同じ6つ(鍵も同じ。xlsx の数え方で上向きが正)
            d = d.child(見出し(ui::t!("文字の向き").to_string()));
            let 今角 = f.rotation.unwrap_or(0);
            let mut r = 列();
            for (id, 札, deg) in [
                ("rot-0", ui::t!("角度なし"), 0),
                ("rot-45", ui::t!("左上がり 45度"), 45),
                ("rot-135", ui::t!("右下がり 45度"), 135),
                ("rot-90", ui::t!("上向き 90度"), 90),
                ("rot-180", ui::t!("下向き 90度"), 180),
                ("rot-255", ui::t!("縦書き(1字ずつ積む)"), 255),
            ] {
                let l = 札.to_string();
                r = r.child(釦(id, 札.to_string(), 今角 == deg).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.set_rotation(deg, &l);
                        cx.notify()
                    })));
            }
            d = d.child(r);

            // 条件付き書式 — **値を訊かないものだけ**をここに置く。
            // 「値より大きいと…」のように打ち込みの要る規則は今までどおり
            // リボンの一覧から(小窓が開くので、パネルの連打には向かない)
            d = d.child(見出し(ui::t!("条件付き書式").to_string()));
            let mut r = 列();
            for (id, 札, act) in [
                ("cf-neg", ui::t!("0未満を赤字"), "cond-neg"),
                ("cf-dup", ui::t!("重複"), "cond-dup"),
                ("cf-uniq", ui::t!("一意"), "cond-uniq"),
                ("cf-avg-a", ui::t!("平均より上"), "cond-avg-above"),
                ("cf-avg-b", ui::t!("平均より下"), "cond-avg-below"),
                ("cf-bar", ui::t!("データバー"), "cond-bar"),
                ("cf-scale", ui::t!("色の濃淡"), "cond-scale"),
                ("cf-icons", ui::t!("アイコン"), "cond-icons"),
                ("cf-clear", ui::t!("消す"), "cond-clear"),
            ] {
                r = r.child(釦(id, 札.to_string(), false).on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.menu_action(act, window, cx);
                        cx.notify()
                    })));
            }
            d = d.child(r);
            }
            // **外側の柱**(面を切り替えるアイコン)。パネルの外枠は
            // 柱ごと囲む — 柱もパネルの一部だから
            let 柱d = 柱()
                .child(柱釦("rf-cell", "cell-format", ui::t!("セルの設定").to_string(), 面 == 0)
                    .on_click(cx.listener(|this, _, _, cx| { this.right_face = 0; cx.notify() })))
                .child(柱釦("rf-shape", "insshape", ui::t!("図形と画像").to_string(), 面 == 1)
                    .on_click(cx.listener(|this, _, _, cx| { this.right_face = 1; cx.notify() })));
            Some(div()
                .flex_none().w(px((W + RAIL) * us)).h_full()
                .m_1().rounded_sm().bg(bg)
                .border_1().border_color(line)
                .flex().flex_row()
                .child(d)
                // 柱は**外側**(窓の縁の側)。仕切りの線を1本
                .child(div().flex_none().w(px(1.0)).h_full().bg(line))
                .child(柱d)
                .into_any_element())
        };

        // ── 左: 会話 ──────────────────────────────────────────────
        let 左 = if !self.left_open {
            None
        } else {
            let 面 = self.left_face;
            // **コメントの面は柱だけ**(発注者 2026-08-15
            // 「コメントの時は、左パネルはアイコンだけの表示でいいのでは」)。
            // コメントはセルの吹き出しで見えるので、板を出す値打ちがない —
            // そのぶん表が広く使える。押したときにコメントの表示も入れる
            let mut d = div()
                .flex_1().min_w(px(0.0)).h_full().overflow_hidden()
                .p_2()
                .flex().flex_col().gap_1();
            if 面 == 0 {
            // 見出しの行。**新しい会話**は Agent Panel と同じく頭に置く
            d = d.child(div().flex().flex_row().items_center().gap_2()
                .child(div().flex_1().text_size(px(us * 12.5))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(fg).child(ui::t!("AI と相談する").to_string()))
                .child(釦("chat-new", ui::t!("新しい会話").to_string(), false).on_click(
                    cx.listener(|this, _, _, cx| { this.chat_reset(); cx.notify() }))));
            d = d.child(div().text_size(px(us * 10.5)).text_color(薄).child(
                ui::t!("選んだ範囲について聞けます。表を変えるときは、\
                        やることを先に見せます — 押すまで表は変わりません。").to_string()));

            // やりとり
            // **残りの高さを全部使う**(固定の高さ + 余白の詰め物、だと
            // 上に空きが溜まる)。やりとりが増えたらここが伸びて巻物になる
            let mut 会話 = div().id("chat-log").flex().flex_col().gap_1().mt_1()
                .flex_1().min_h(px(0.0)).overflow_y_scroll();
            if self.chat_log.is_empty() {
                会話 = 会話.child(div().text_size(px(us * 11.0)).text_color(薄).child(
                    ui::t!("例: 売上の多い順に並べて / 上位5件に色をつけて \
                            / 合計の行を足して").to_string()));
            }
            for (自分, 字) in &self.chat_log {
                会話 = 会話.child(
                    div().text_size(px(us * 11.5))
                        .text_color(if *自分 { 主 } else { fg })
                        .child(format!("{} {}", if *自分 { "▸" } else { "◂" }, 字)));
            }
            d = d.child(会話);

            // **落ちたら直してもらう。** 誤りを添えて頼み直す一押し —
            // 走らせて直す、が Agent Panel の芯(2026-08-16)
            if self.chat_err.is_some() {
                d = d.child(列().mt_1()
                    .child(釦("chat-fix", ui::t!("直してもらう").to_string(), true).on_click(
                        cx.listener(|this, _, _, cx| { this.chat_fix(cx); cx.notify() })))
                    .child(釦("chat-err-drop", ui::t!("そのままにする").to_string(), false)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.chat_err = None;
                            cx.notify()
                        }))));
            }
            // 変更案(Python)。**押すまで走らない**
            if let Some(plan) = self.chat_plan.clone() {
                d = d.child(見出し(ui::t!("変更案(押すまで動きません)").to_string()));
                d = d.child(div().id("chat-plan")
                    .max_h(px(us * 150.0)).overflow_y_scroll()
                    .p_1().rounded_sm()
                    .bg(if dk { rgb(0x14171A) } else { rgb(0xFFFFFF) })
                    .border_1().border_color(line)
                    .text_size(px(us * 10.5)).text_color(fg)
                    .children(plan.lines().map(|l| div().child(l.to_string()))));
                let mut r = 列().mt_1();
                r = r.child(釦("chat-run", ui::t!("入れる").to_string(), true).on_click(
                    cx.listener(|this, _, _, cx| { this.chat_run(cx); cx.notify() })));
                r = r.child(釦("chat-drop", ui::t!("やめる").to_string(), false).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.chat_plan = None;
                        this.status = ui::t!("変更案を捨てました(何もしていません)").into();
                        cx.notify()
                    })));
                d = d.child(r);
            }

            // 入力
            d = d.child(div()
                .p_1().rounded_sm()
                .bg(if dk { rgb(0x14171A) } else { rgb(0xFFFFFF) })
                .border_1().border_color(if self.chat_focus { 主 } else { line })
                .text_size(px(us * 11.5)).text_color(fg)
                .id("chat-in")
                .cursor_text()
                .on_click(cx.listener(|this, _, _, cx| { this.chat_focus = true; cx.notify() }))
                // 焦点があるときは打った所に「|」を差す(fn_dlg と同じ描き方)
                .child(if self.chat_in.text().is_empty() {
                    if self.chat_focus {
                        "|".to_string()
                    } else {
                        ui::t!("ここを押して書き、Enter で送る").to_string()
                    }
                } else if self.chat_focus {
                    let mut t = self.chat_in.text().to_string();
                    let cur = self.chat_in.cursor().min(t.len());
                    t.insert(cur, '|');
                    t
                } else {
                    self.chat_in.text().to_string()
                }));
            let mut r = 列().mt_1();
            r = r.child(釦("chat-send", ui::t!("送る").to_string(), !self.ai_busy).on_click(
                cx.listener(|this, _, _, cx| { this.chat_send(cx); cx.notify() })));
            if self.ai_busy {
                r = r.child(div().text_size(px(us * 10.5)).text_color(薄)
                    .child(ui::t!("考えています…").to_string()));
            }
            d = d.child(r);
            // **宛先はここで替える**(Agent Panel はモデルを下に出す)。
            // 詳細設定まで行かずに、話しながら切り替えられる
            let 宛 = ui::ai::backend();
            d = d.child(div()
                .id("chat-where")
                .mt_1().px_1().py_0p5().rounded_sm().cursor_pointer()
                .text_size(px(us * 10.5)).text_color(薄)
                .hover(move |s| s.bg(if dk { rgb(0x2C333A) } else { rgb(0xEAF5EE) }))
                .child(ui::tf!("宛先: {}(押すと替わる)", 宛.label()).to_string())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.run_cmd("ai-where", cx);
                    cx.notify()
                })));
            }
            // **外側の柱**(左パネルは窓の左端の側)。会話とコメントを切り替える
            let 柱d = 柱()
                .child(柱釦("lf-ai", "ai-ask", ui::t!("AI と相談する").to_string(), 面 == 0)
                    .on_click(cx.listener(|this, _, _, cx| { this.left_face = 0; cx.notify() })))
                .child(柱釦("lf-cmt", "co-showcomment", ui::t!("コメント").to_string(), 面 == 1)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.left_face = 1;
                        // **コメントを見えるようにする。** 押したのに何も
                        // 起きないと、切り替わったのか分からない
                        this.show_comments = true;
                        this.status = ui::t!("コメントを表示しています(セルの吹き出し)").into();
                        cx.notify()
                    })));
            let mut 包み = div()
                .flex_none()
                .w(px((if 面 == 0 { W + RAIL } else { RAIL }) * us))
                .h_full()
                .m_1().rounded_sm().bg(bg)
                .border_1().border_color(line)
                .flex().flex_row()
                .child(柱d);
            if 面 == 0 {
                包み = 包み
                    .child(div().flex_none().w(px(1.0)).h_full().bg(line))
                    .child(d);
            }
            Some(包み.into_any_element())
        };
        (左, 右)
    }
}
