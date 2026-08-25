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
            .child(crate::t!("No folder is open (File > Open)")),
        Some(d) => {
            let 名 = d
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| d.display().to_string());
            div().text_size(px(s * 10.5)).text_color(look.dim).child(SharedString::from(名))
        }
    }
}

/// **上のフォルダへ戻る行。** いちばん上のときは出しません。
///
/// 押す結び付けは呼ぶ側が足します(`row` と同じ作法)。
pub fn up_row(look: &Look, dir: &std::path::Path) -> Option<Stateful<Div>> {
    let 親 = dir.parent()?.to_path_buf();
    let s = look.scale;
    Some(
        div()
            .id("fl-up")
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .hover(move |st| st.bg(look.hover))
            .text_size(px(s * 11.5))
            .text_color(look.fg)
            .child(SharedString::from("‹"))
            .child(SharedString::from(crate::tf!(
                "Up ({})",
                親.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| 親.display().to_string())
            ))),
    )
}

/// 「空のフォルダです」。
pub fn empty(look: &Look) -> Div {
    div()
        .text_size(px(look.scale * 11.0))
        .text_color(look.dim)
        .child(crate::t!("(the folder is empty)"))
}

/// 並べる中身。**200 件で切ります** — 切ったことは呼ぶ側が言うこと。
/// 上限。これより多いフォルダでは、切ったことを画面に出します
pub const 一覧の上限: usize = 200;

/// 並べる物と、切って落とした数。
///
/// **黙って切りません**(2026-08-26)。前は 200 件で切っていて、
/// それ以上のファイルは*あるのに出ない*状態でした。
pub fn entries_with_rest(dir: &std::path::Path) -> (Vec<folder::Entry>, usize) {
    let 全部 = folder::list(dir);
    let 残り = 全部.len().saturating_sub(一覧の上限);
    (全部.into_iter().take(一覧の上限).collect(), 残り)
}

pub fn entries(dir: &std::path::Path) -> Vec<folder::Entry> {
    entries_with_rest(dir).0
}

/// 切って落とした分の断り。0 件なら出しません。
pub fn rest_note(look: &Look, 残り: usize) -> Option<Div> {
    (残り > 0).then(|| {
        div()
            .text_size(px(look.scale * 10.5))
            .text_color(look.dim)
            .child(crate::tf!("{} more (not shown)", 残り))
    })
}

/// 行の右に置く操作の絵(名前を変える・消す)。
///
/// **乗せたときだけ濃くなります。** いつも黒い字で「名前」「消す」が
/// 並んでいると、一覧そのものが読めません(2026-08-26 発注者
/// 「filemanager と同じユーザーインタフェースにしろ」)。
pub fn row_button(look: &Look, i: usize, 印: &'static str, _名: SharedString) -> Stateful<Div> {
    let s = look.scale;
    let hover = look.hover;
    let 絵 = match 印 {
        "ren" => "icons/py-edit.svg",
        _ => "icons/cell-del.svg",
    };
    div()
        .id(SharedString::from(format!("fl-{印}-{i}")))
        .flex_none()
        .p_0p5()
        .rounded_sm()
        .cursor_pointer()
        .opacity(0.45)
        .hover(move |st| st.bg(hover).opacity(1.0))
        .child(
            gpui::svg()
                .path(SharedString::from(絵))
                .size(px(s * 13.0))
                .text_color(look.fg),
        )
}

/// 一覧の頭に置く「新しく作る」の絵。
pub fn make_button(look: &Look, 印: &'static str, _名: SharedString) -> Stateful<Div> {
    let s = look.scale;
    let hover = look.hover;
    let 絵 = match 印 {
        "folder" => "icons/py-folder.svg",
        "sheet" => "icons/instable.svg",
        _ => "icons/py-new.svg",
    };
    div()
        .id(SharedString::from(format!("fl-new-{印}")))
        .flex_none()
        .p_1()
        .rounded_sm()
        .cursor_pointer()
        .hover(move |st| st.bg(hover))
        .child(
            gpui::svg()
                .path(SharedString::from(絵))
                .size(px(s * 15.0))
                .text_color(look.fg),
        )
}

/// 行1つ。/// 行1つ。**押す結び付けは付いていません** — 呼ぶ側が
/// `.on_click(cx.listener(…))` を足します。
///
/// `開ける` が偽なら指の形も乗ったときの色も付けません —
/// **できないことを、できるように見せない**。
pub fn row(look: &Look, i: usize, e: &folder::Entry, いま: bool) -> Stateful<Div> {
    let s = look.scale;
    // **フォルダも押せます**(2026-08-26)。`can_open()` は「この道具で
    // *中身を開ける*か」なので、フォルダは偽です。フォルダは開くのでは
    // なく*中へ入る*ので、押せるかどうかは別に見ます
    let 押せる = e.kind.can_open() || e.kind == folder::Kind::Folder;
    let hover = look.hover;
    let mut 行 = div()
        .id(SharedString::from(format!("fl-{i}")))
        .px_1()
        .py_0p5()
        .rounded_sm()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .bg(if いま { look.hover } else { gpui::transparent_black().into() })
        // **行の頭に絵。** 種類は絵で分かるので、右端に字で書きません
        // (ファイル管理の道具の作法。2026-08-26 発注者)
        .child(
            gpui::svg()
                .path(SharedString::from(icon_of(e.kind)))
                .size(px(s * 14.0))
                .flex_none()
                .text_color(if 押せる { look.fg } else { look.dim }),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(px(s * 11.5))
                .text_color(if 押せる { look.fg } else { look.dim })
                .child(SharedString::from(e.name.clone())),
        );
    if 押せる {
        行 = 行.cursor_pointer().hover(move |st| st.bg(hover));
    }
    行
}

/// 種類に当てる絵。**フォルダと、それ以外**を見分けられれば足ります。
fn icon_of(k: folder::Kind) -> &'static str {
    match k {
        folder::Kind::Folder => "icons/py-folder.svg",
        folder::Kind::Sheet => "icons/instable.svg",
        folder::Kind::Doc => "icons/blankpage.svg",
        _ => "icons/blankpage.svg",
    }
}
