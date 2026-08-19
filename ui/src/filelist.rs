//! **フォルダの中身の一覧 — 文章と表で1本**(統合の段7。2026-08-19 発注者
//! 「フォルダーの一覧は、ファイルを選択して開くためのものです」)。
//!
//! *どのタブでも同じ場所・同じ姿*で出します。前は writer と calc が同じ
//! 一覧を別々に描いていて、写しがずれる型でした(`ui::tabrow` と同じ形で
//! 1本にしています)。
//!
//! *描く位置は編集画面の右パネルのままです。* 一覧は設定・ページ・スタイルと
//! 同じ並びの1つの面で、officework 側に出すと**パネルが2枚**になります。
//! 動かしたのは「どう描くか」の持ち主です。
//!
//! **押したときに何をするかは、ここでは持ちません。** 呼ぶ側が
//! `cx.listener` で結びます — 埋め込みなら officework に頼み、単体なら
//! 自分で開く、という違いがアプリの側にあるためです。ここが持つのは
//! *行の見た目*と*押せるかどうか*までです。

use crate::folder;
use gpui::{div, prelude::*, px, Div, Rgba, SharedString, Stateful};

/// 色。**画面のテーマから受け取ります**(ここでは決めない)。
pub struct Look {
    /// 開ける物の名前
    pub fg: Rgba,
    /// 添えの字(フォルダ名・種類の札・開けない物)
    pub dim: Rgba,
    /// いま開いている行の地、と乗ったときの地
    pub hover: Rgba,
    /// 画面の拡大率(calc は `us` を掛ける。writer は 1.0)
    pub scale: f32,
}

/// 一覧の頭(フォルダ名、または「開いていません」)。
///
/// **フォルダの名前だけ**を出します。長い径路を全部出すと折り返して、
/// 一覧の場所を食うためです。
pub fn header(look: &Look, dir: Option<&std::path::Path>) -> Div {
    let s = look.scale;
    match dir {
        None => div()
            .text_size(px(s * 11.0))
            .text_color(look.dim)
            .child(crate::t!("フォルダを開いていません(ファイル > 開く)")),
        Some(d) => {
            let 名 = d
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| d.display().to_string());
            div().text_size(px(s * 10.5)).text_color(look.dim).child(SharedString::from(名))
        }
    }
}

/// 「空のフォルダです」。
pub fn empty(look: &Look) -> Div {
    div()
        .text_size(px(look.scale * 11.0))
        .text_color(look.dim)
        .child(crate::t!("(空のフォルダです)"))
}

/// 並べる中身。**200 件で切ります** — 切ったことは呼ぶ側が言うこと。
pub fn entries(dir: &std::path::Path) -> Vec<folder::Entry> {
    folder::list(dir).into_iter().take(200).collect()
}

/// 行1つ。**押す結び付けは付いていません** — 呼ぶ側が
/// `.on_click(cx.listener(…))` を足します。
///
/// `開ける` が偽なら指の形も乗ったときの色も付けません —
/// **できないことを、できるように見せない**。
pub fn row(look: &Look, i: usize, e: &folder::Entry, いま: bool) -> Stateful<Div> {
    let s = look.scale;
    let 開ける = e.kind.can_open();
    let hover = look.hover;
    let mut 行 = div()
        .id(SharedString::from(format!("fl-{i}")))
        .px_1()
        .py_0p5()
        .rounded_sm()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .bg(if いま { look.hover } else { gpui::transparent_black().into() })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(px(s * 11.5))
                .text_color(if 開ける { look.fg } else { look.dim })
                .child(SharedString::from(e.name.clone())),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(s * 9.0))
                .text_color(look.dim)
                .child(SharedString::from(e.kind.label().to_string())),
        );
    if 開ける {
        行 = 行.cursor_pointer().hover(move |st| st.bg(hover));
    }
    行
}
