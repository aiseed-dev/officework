//! **officework — 1つのアプリ**(SEKKEI「画面を1つにする」4段目、
//! 2026-08-19 発注者「バイナリは officework にする」)。
//!
//! フォルダを開いて、文章も表も同じウィンドウで編集します。
//! *どちらの編集画面を出すかは、ファイルの名前で決まります*
//! (`.sheet.adoc` は表、`.adoc` は文章)。
//!
//! *編集画面は2つのまま*です。文章は `writer::Writer`、表は `calc::Calc` が
//! そのまま受け持ちます。ここがするのは、どちらを見せるかを選ぶことだけです。
//! 48,566 行を1つの型にまとめても得る物がありません(SEKKEI)。

use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, Entity, Focusable, Window, WindowBounds,
    WindowOptions,
};
use gpui_platform::application;

/// いま見せている編集画面。
enum Shown {
    /// 文章(`.adoc` `.docx` …)
    Doc(Entity<writer::Writer>),
    /// 表(`.sheet.adoc` `.xlsx`)
    Sheet(Entity<calc::Calc>),
}

/// アプリの本体。**選ぶだけ**で、編集の中身は持ちません。
struct Office {
    shown: Shown,
}

impl Office {
    fn new(path: Option<std::path::PathBuf>, cx: &mut Context<Self>) -> Office {
        // **名前で決めます。** 中身は見ません(SEKKEI「画面を1つにする」)
        let 表か = path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| ui::folder::kind_of(&n.to_string_lossy()))
            .is_some_and(|k| k.is_sheet());
        let shown = if 表か {
            Shown::Sheet(cx.new(|cx| calc::Calc::new(path, cx)))
        } else {
            Shown::Doc(cx.new(|cx| writer::Writer::new(path, cx)))
        };
        Office { shown }
    }
}

impl Render for Office {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // いまは選んだ編集画面をそのまま出します。ファイルのタブと
        // フォルダの一覧は、それぞれの編集画面が持っているものを使います
        // (1つに寄せるのは5段目「リボンの段選び」と一緒にやります)
        div().size_full().child(match &self.shown {
            Shown::Doc(v) => v.clone().into_any_element(),
            Shown::Sheet(v) => v.clone().into_any_element(),
        })
    }
}

fn main() {
    let arg = std::env::args().nth(1).map(std::path::PathBuf::from);
    application().with_assets(ui::Icons).run(move |cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![std::borrow::Cow::Borrowed(ops::font_data())])
            .expect("フォント登録");
        ui::settings::ai_env_from_settings();
        // **割り当ては両方を入れます。** 文章と表で同じキーが違う意味を
        // 持つ物はないので、重ねても食い違いません
        cx.bind_keys(ui::bindings_for("writer", "jo_edit"));
        let saved = ui::winstate::load("officework");
        let bounds = match saved {
            Some(st) => Bounds::new(gpui::point(px(st.x), px(st.y)), size(px(st.w), px(st.h))),
            None => Bounds::centered(None, size(px(1100.0), px(1000.0)), cx),
        };
        let wb = if saved.is_some_and(|st| st.maximized) {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        };
        let arg2 = arg.clone();
        cx.open_window(
            WindowOptions { window_bounds: Some(wb), ..Default::default() },
            move |window, cx| {
                let view = cx.new(|cx| Office::new(arg2.clone(), cx));
                // 焦点は中の編集画面へ渡します
                view.update(cx, |this, cx| match &this.shown {
                    Shown::Doc(v) => window.focus(&v.focus_handle(cx), cx),
                    Shown::Sheet(v) => window.focus(&v.focus_handle(cx), cx),
                });
                view.update(cx, |_, cx| {
                    cx.observe_window_bounds(window, |_, window, _| {
                        let wb = window.window_bounds();
                        if matches!(wb, WindowBounds::Fullscreen(_)) {
                            return;
                        }
                        let b = wb.get_bounds();
                        ui::winstate::save(
                            "officework",
                            ui::winstate::WinState {
                                x: f32::from(b.origin.x),
                                y: f32::from(b.origin.y),
                                w: f32::from(b.size.width),
                                h: f32::from(b.size.height),
                                maximized: matches!(wb, WindowBounds::Maximized(_)),
                            },
                        );
                    })
                    .detach();
                });
                view
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
