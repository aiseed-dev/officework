//! **リボンから開く一覧を描く**(SEKKEI「リボンのドロップダウンを1つの
//! 仕組みにする」の手順1。2026-08-20)。
//!
//! 表の画面には約40種類の一覧が1つの仕組みで載っていて、押したボタンの
//! 真下に出る・打つと絞れる・↑↓/Enter/Esc が効く、まで揃っています。
//! 文章の画面の4つ(書体・大きさ・段落スタイル・記号)は以前のままです。
//!
//! *ここは描くところだけ*です。どこに出すか(`face::combo` の `pop_under`
//! など)と、開いているか・選択・絞り込みの字は**呼ぶ側が持ちます**。
//! 表の画面の持ち方をそのまま共通の形にしました。
//!
//! **タブの行(`ui::tabrow`)と同じやり方です。** 写しを作らず、描く所を
//! 1つにして両方から呼びます。

use gpui::{div, prelude::*, px, Div, Rgba, SharedString, Stateful};

/// `RRGGBB` を色に。**ここだけで使います** — `ui` は `ops` に依らない決めなので、
/// `ops::hex` は借りずに自分で読みます
fn 色(s: &str) -> Rgba {
    let b = s.as_bytes();
    let n = |i: usize| -> f32 {
        if b.len() < i * 2 + 2 {
            return 0.0;
        }
        u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap_or(0) as f32 / 255.0
    };
    Rgba { r: n(0), g: n(1), b: n(2), a: 1.0 }
}

/// 一覧の色と大きさ。**画面のテーマから受け取ります**(ここでは決めない)。
pub struct Look {
    /// 一覧の地
    pub bg: Rgba,
    /// 枠と区切りの線
    pub border: Rgba,
    /// ふつうの字
    pub fg: Rgba,
    /// 添えの字(案内・「一覧にありません」)
    pub dim: Rgba,
    /// **絞り込みの欄の、まだ何も打っていないときの字。**
    /// `dim` より薄くします — 打った字と見分けるためで、同じ濃さだと
    /// 「もう何か入っている」ように見えます
    pub ghost: Rgba,
    /// 乗ったとき・選んでいる項の地
    pub hover: Rgba,
    /// 題と「→」の字
    pub accent: Rgba,
    /// 画面の拡大率(表は `us` を掛ける。文章は 1.0)
    pub scale: f32,
}

/// どこに、どれだけの大きさで出すか。**計算は `face::combo` の持ち場**で、
/// ここは受け取った値のとおりに置くだけです。
pub struct Place {
    /// 窓の左からの距離
    pub x: f32,
    /// 上に開くなら窓の下端から、下に開くなら上端からの距離
    pub at: f32,
    /// 上に開くか
    pub up: bool,
    /// 高さの上限(あふれは中で送ります — 数で切り捨てない)
    pub max_h: f32,
    /// 幅。`Fixed` はセルから開いた一覧(その列に合わせる)、
    /// `Range` はリボンから開いた一覧(中身に合わせる)
    pub width: Width,
}

/// 一覧の幅の決め方。
pub enum Width {
    /// 決め打ち(セルから開いた一覧は、その列の幅に合わせます)
    Fixed(f32),
    /// 下限と上限(リボンから開いた一覧は中身に合わせます。書体名は長いので
    /// 狭いと読めず、大きさの一覧は広いと間が抜けます)
    Range(f32, f32),
}

/// 項1つの飾り。**呼ぶ側がクロージャで決めます** — 色の一覧は見本の四角、
/// 書体の一覧はその書体で、という細工がアプリごとにあるためです。
#[derive(Default)]
pub struct Deco {
    /// 色見本。`Some(None)` は「色なし」の四角(白)
    pub swatch: Option<Option<String>>,
    /// この項をこの書体で描く(書体の一覧)
    pub font: Option<String>,
}

/// **一覧を描く。**
///
/// `items` は `(鍵, 見出し)` の組です。**引き当ては鍵**(日本語のまま)で、
/// 画面に出すのは見出しです — 見た目で照合すると、日本語以外で壊れます。
///
/// `filter` は絞り込みの欄に出す字です。`None` なら欄を出しません。
/// カーソルの「|」は呼ぶ側が差した状態で渡します。
///
/// 押したときは `on_pick` を鍵つきで呼びます。閉じるのも呼ぶ側の仕事です
/// (閉じ方がアプリごとに違うため)。
#[allow(clippy::too_many_arguments)]
pub fn panel<V: gpui::Render>(
    look: &Look,
    place: &Place,
    note: Option<SharedString>,
    filter: Option<(String, bool)>,
    items: &[(String, String)],
    sel: usize,
    deco: impl Fn(&str) -> Deco,
    cx: &mut gpui::Context<V>,
    on_pick: impl Fn(&mut V, &str, &mut gpui::Context<V>) + Clone + 'static,
) -> Stateful<Div> {
    let s = look.scale;
    let mut p = div().id("pick-list").absolute().left(px(place.x));
    // 上に開くときは**下辺を開く元に合わせます**(中身が短くても隙間を
    // 空けない)ので bottom で置きます。下に開くときは top です
    p = if place.up { p.bottom(px(place.at)) } else { p.top(px(place.at)) };
    p = match place.width {
        Width::Fixed(w) => p.w(px(w)),
        Width::Range(lo, hi) => p.min_w(px(lo)).max_w(px(hi)),
    };
    let mut p = p
        .max_h(px(place.max_h.max(160.0)))
        .overflow_y_scroll()
        .p_1()
        .rounded_md()
        .bg(look.bg)
        .border_1()
        .border_color(look.border)
        .shadow_lg()
        // 一覧の中を押しても、下の画面には届かせません
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());

    // 題(いま何を選んでいるか)。ピボットの段の案内など
    if let Some(note) = note {
        p = p.child(
            div()
                .px_2()
                .py_1()
                .mb_0p5()
                .border_b_1()
                .border_color(look.border)
                .text_size(px(s * 11.0))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(look.accent)
                // **折り返します。** 1行に押し込むと、幅を越えた案内が
                // 黙って切れます(2026-08-13 実機で見た)
                .child(note),
        );
    }

    // 絞り込みつきの一覧は、頭に検索欄を出します(打鍵はここへ流れます)
    if let Some((text, empty)) = &filter {
        p = p.child(
            div()
                .px_2()
                .py_1()
                .mb_0p5()
                .border_1()
                .border_color(look.border)
                .rounded_sm()
                .text_size(px(s * 12.0))
                .text_color(if *empty { look.ghost } else { look.fg })
                .whitespace_nowrap()
                .overflow_hidden()
                .child(SharedString::from(if *empty {
                    crate::t!("打つと絞り込みます").to_string()
                } else {
                    text.clone()
                })),
        );
    }
    if items.is_empty() && filter.is_some() {
        p = p.child(
            div()
                .px_2()
                .py_1()
                .text_size(px(s * 12.0))
                .text_color(look.dim)
                .child(crate::t!("一覧にありません(このまま Enter で確定)")),
        );
    }

    for (i, (key, label)) in items.iter().enumerate() {
        let d = deco(key);
        let on = i == sel; // ↑↓ の選択(絞り込み後の並びの添字)
        // 「→ 」は次の段へ進むボタン — 並びの項目と見分けます
        let 進む = key.starts_with("→ ");
        let (hover, border) = (look.hover, look.border);
        let mut row = div()
            .id(SharedString::from(format!("pk{i}")))
            .px_2()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .hover(move |st| st.bg(hover))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .text_size(px(s * 12.5))
            // 選んでいる項は下地の色で示します(↑↓・Enter の相手が目で分かる)
            .when(on, |st| st.bg(look.hover))
            .text_color(if 進む { look.accent } else { look.fg })
            .when(進む, |st| {
                st.font_weight(gpui::FontWeight::BOLD).border_t_1().border_color(border).mt_0p5()
            })
            .whitespace_nowrap()
            .overflow_hidden()
            .children(d.swatch.map(|hx| {
                let q = div().w(px(14.0)).h(px(14.0)).rounded_sm().border_1().border_color(border);
                match hx {
                    Some(h) => q.bg(色(&h)),
                    None => q.bg(gpui::white()),
                }
            }));
        if let Some(f) = d.font {
            row = row.font_family(SharedString::from(f));
        }
        let key = key.clone();
        let on_pick = on_pick.clone();
        p = p.child(row.child(SharedString::from(label.clone())).on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(move |v, _, _, cx| {
                cx.stop_propagation();
                on_pick(v, &key, cx);
                cx.notify();
            }),
        ));
    }
    p
}
