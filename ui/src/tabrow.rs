//! **リボンのタブの行 — 文章と表で1本**(統合の段6の後半。2026-08-19
//! 発注者「タブの行だけ移すようにして」)。
//!
//! 並び(15段)・灰色・選んだ印・点検用の場所の控え・押した後の流れを、
//! ここ1本で持ちます。前は writer と calc が同じ行を別々に描いていて
//! (約142行と約201行)、写しがずれる型でした。
//!
//! *描く位置は編集画面の中のままです。* タブの行はすぐ下のボタンの並びと
//! 隣り合っていないと意味が取れず、間にファイル名の行を挟めないためです。
//! 動かしたのは「どう描くか・押すと何が起きるか」の持ち主です。
//! 段の選びが画面をまたいで残る仕組みは officework が持ち回ります(済)。
//! ファイルの入口の付け替え(段8)は、この行の上で行います。

use crate::tabs;
use gpui::{div, prelude::*, px, Context, Div, Rgba, SharedString};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// どちらの画面の行か(揃えた並びのどちらの列で段番号を引くか)。
#[derive(Clone, Copy)]
pub enum Side {
    Doc,
    Sheet,
}

/// 色。**画面のテーマから受け取ります**(ここでは決めない — 文章は青、
/// 表は緑という違いは画面の物です)。
pub struct Look {
    pub row_bg: Rgba,
    pub grey: Rgba,
    pub on_fg: Rgba,
    pub idle_fg: Rgba,
    pub hover_fg: Rgba,
    pub find_fg: Rgba,
    pub underline_on: Rgba,
    /// 文脈タブ(ピボットなど)。使わない画面は他の欄と同じ値でよい
    pub ctx_fg: Rgba,
    pub ctx_bg: Rgba,
}

/// 点検の道具のための場所の控え(`@tab<番号>` で入る)。
pub type Boxes = Rc<RefCell<HashMap<&'static str, (f32, f32, f32, f32)>>>;

/// タブの行を組む。
///
/// `hidden` は文脈タブ(まだ出さない段)、`ctx_tab` は色を付ける段、
/// `hint` は Alt のキーヒントの札です。使わない画面は偽・None を返す
/// だけの物を渡してください。
#[allow(clippy::too_many_arguments)]
pub fn build<V: 'static>(
    cx: &mut Context<V>,
    side: Side,
    current: usize,
    scale: f32,
    disabled: bool,
    look: Look,
    boxes: Boxes,
    hidden: impl Fn(usize) -> bool,
    ctx_tab: impl Fn(usize) -> bool,
    hint: impl Fn(usize) -> Option<String>,
    on_pick: impl Fn(&mut V, usize, &mut Context<V>) + Copy + 'static,
    on_find: impl Fn(&mut V, &mut Context<V>) + Copy + 'static,
) -> Div {
    let mut row = div().flex().flex_row().items_end().gap_1().px_2().bg(look.row_bg);
    for (位置, 段) in tabs::merged().into_iter().enumerate() {
        let 名 = 段.name;
        let idx = match side {
            Side::Doc => 段.doc,
            Side::Sheet => 段.sheet,
        };
        let Some(i) = idx else {
            // この画面には無い段。**灰色で出す**(未実装のボタンと同じ描き方)
            row = row.child(
                div()
                    .id(SharedString::from(format!("tab{位置}")))
                    .px_2p5()
                    .pt_1p5()
                    .text_size(px(scale * 12.0))
                    .text_color(look.grey)
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(名)
                    .child(div().h(px(2.0)).w_full()),
            );
            continue;
        };
        if hidden(i) {
            // 文脈タブ(ピボットなど)は、出る条件が揃うまで出さない
            continue;
        }
        let on = i == current;
        let is_ctx = ctx_tab(i);
        // **段の場所を控える**(`@tab<編集画面の番号>`)。点検の道具が
        // 目分量でなく本当の座標を押せるようにする
        let rec = boxes.clone();
        let key: &'static str = Box::leak(format!("@tab{i}").into_boxed_str());
        let mark = gpui::canvas(
            move |b: gpui::Bounds<gpui::Pixels>, _, _| {
                rec.borrow_mut().insert(
                    key,
                    (
                        f32::from(b.origin.x),
                        f32::from(b.origin.y),
                        f32::from(b.size.width),
                        f32::from(b.size.height),
                    ),
                );
            },
            |_, _: (), _, _| {},
        )
        .absolute()
        .size_full();
        let hover_fg = look.hover_fg;
        row = row.child(
            div()
                .id(SharedString::from(format!("tab{i}")))
                .relative()
                .child(mark)
                // Alt のキーヒントの札(出ているときだけ。黒地に白 — 名札と
                // 同じ。右肩に置き、上へはみ出させない)
                .children(hint(i).map(|h| {
                    div()
                        .absolute()
                        .top(px(0.0))
                        .right(px(-2.0))
                        .px_0p5()
                        .rounded_sm()
                        .bg(gpui::rgb(0x2B2F33))
                        .text_color(gpui::rgb(0xF2F5F7))
                        .text_size(px(scale * 9.5))
                        .child(SharedString::from(h))
                }))
                .px_2p5()
                .pt_1p5()
                .when(is_ctx, |d| d.bg(look.ctx_bg).rounded_t_md())
                .text_size(px(scale * 12.0))
                // 小窓中はタブも灰色・無反応(未実装のボタンと同じ描き方)
                .text_color(if disabled {
                    look.grey
                } else if is_ctx {
                    look.ctx_fg
                } else if on {
                    look.on_fg
                } else {
                    look.idle_fg
                })
                .font_weight(if on { gpui::FontWeight::BOLD } else { gpui::FontWeight::NORMAL })
                .when(!disabled, |d| {
                    d.cursor_pointer()
                        .hover(move |s| s.text_color(hover_fg))
                        .on_click(cx.listener(move |v, _, _, cx| on_pick(v, i, cx)))
                })
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(名)
                // 現在地の下線(デスクトップ版の形)
                .child(div().h(px(2.5)).w_full().rounded_sm().bg(if on && is_ctx {
                    look.ctx_fg
                } else if on {
                    look.underline_on
                } else if is_ctx {
                    look.ctx_bg
                } else {
                    look.row_bg
                })),
        );
    }
    let find_hover = look.hover_fg;
    row.child(div().flex_1()).child(
        div()
            .id("tab-find")
            .px_2()
            .pb_1()
            .text_size(px(scale * 12.0))
            .text_color(look.find_fg)
            .when(!disabled, |d| {
                d.cursor_pointer()
                    .hover(move |s| s.text_color(find_hover))
                    .on_click(cx.listener(move |v, _, _, cx| on_find(v, cx)))
            })
            .child("🔍"),
    )
}
