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

        // ── 右: セルの設定 ─────────────────────────────────────────
        let 右 = if !self.right_open {
            None
        } else {
            let f = self.sheet().get(self.cursor).map(|c| c.fmt.clone()).unwrap_or_default();
            let mut d = div()
                .id("right-panel")
                .flex_none().w(px(W * us)).h_full().overflow_y_scroll()
                .p_2().bg(bg)
                .border_l_1().border_color(line)
                .flex().flex_col().gap_0p5();
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
            Some(d.into_any_element())
        };

        // ── 左: 会話 ──────────────────────────────────────────────
        let 左 = if !self.left_open {
            None
        } else {
            let mut d = div()
                .flex_none().w(px(W * us)).h_full().overflow_hidden()
                .p_2().bg(bg)
                .border_r_1().border_color(line)
                .flex().flex_col().gap_1();
            d = d.child(div().text_size(px(us * 12.5)).font_weight(gpui::FontWeight::BOLD)
                .text_color(fg).child(ui::t!("AI と相談する").to_string()));
            d = d.child(div().text_size(px(us * 10.5)).text_color(薄).child(
                ui::t!("選んでいる範囲が相談の相手になります。表を直す頼みは、\
                        台本にして見せます — 押すまで走りません。").to_string()));

            // やりとり
            // **残りの高さを全部使う**(固定の高さ + 余白の詰め物、だと
            // 上に空きが溜まる)。やりとりが増えたらここが伸びて巻物になる
            let mut 会話 = div().id("chat-log").flex().flex_col().gap_1().mt_1()
                .flex_1().min_h(px(0.0)).overflow_y_scroll();
            if self.chat_log.is_empty() {
                会話 = 会話.child(div().text_size(px(us * 11.0)).text_color(薄).child(
                    ui::t!("例: この表を売上の多い順に並べて / 上位5件に色を付けて \
                            / 合計の行を足して").to_string()));
            }
            for (自分, 字) in &self.chat_log {
                会話 = 会話.child(
                    div().text_size(px(us * 11.5))
                        .text_color(if *自分 { 主 } else { fg })
                        .child(format!("{} {}", if *自分 { "▸" } else { "◂" }, 字)));
            }
            d = d.child(会話);

            // 変更案(Python)。**押すまで走らない**
            if let Some(plan) = self.chat_plan.clone() {
                d = d.child(見出し(ui::t!("変更案(押すまで走りません)").to_string()));
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
            Some(d.into_any_element())
        };
        (左, 右)
    }
}
