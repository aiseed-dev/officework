//! **Mac で字が描かれるかを確かめる小さな窓**(2026-09-10)。
//!
//!     cargo run --release -p officework --example moji
//!
//! 6 秒で自分から終わります。標準エラーに、機械の書体の数と、5 つの書体名が
//! どの書体に決まったかを出します。alpha.2 で字が一つも出なかったとき、
//! これで「書体は 11 しか無く、全部 FontId(1) に落ちる」と分かり、
//! gpui_platform の `font-kit` の機能が抜けていたことに辿り着きました。
//! Mac の描画を疑うときは、まずこれを回してください。
use gpui::{div, font, prelude::*, px, rgb, size, App, Bounds, TextRun, WindowBounds, WindowOptions};
use gpui_platform::application;

fn main() {
    application().run(|cx: &mut App| {
        let data = ops::font_data();
        eprintln!("font bytes {}", data.len());
        cx.text_system()
            .add_fonts(vec![std::borrow::Cow::Borrowed(data)])
            .expect("add_fonts");
        let names = cx.text_system().all_font_names();
        eprintln!("font names {} (first {:?})", names.len(), &names[..names.len().min(8)]);
        cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(500.0), px(300.0)), cx))), ..Default::default() },
            |_, cx| cx.new(|_| Moji),
        )
        .unwrap();
        cx.activate(true);
        cx.spawn(async move |cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(6)).await;
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}

struct Moji;
fn shiraberu(window: &mut gpui::Window) {
    let ts = window.text_system();
    for fam in [".SystemUIFont", "Helvetica", "Arial", "ヒラギノ角ゴシック", "Hiragino Sans"] {
        let f = font(fam);
        let id = ts.resolve_font(&f);
        let text: gpui::SharedString = "Hello あいう".into();
        let line = ts.shape_line(text.clone(), px(14.0), &[TextRun { len: text.len(), font: f.clone(), color: rgb(0xffffff).into(), background_color: None, underline: None, strikethrough: None }], None);
        eprintln!("{fam:24} id={id:?} width={:?} len={}", line.width, line.len());
    }
}
static ONCE: std::sync::Once = std::sync::Once::new();
impl gpui::Render for Moji {
    fn render(&mut self, window: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        ONCE.call_once(|| shiraberu(window));
        div().size_full().bg(rgb(0x202020)).flex().flex_col().gap_2().p_4()
            .child(div().text_color(rgb(0xffffff)).text_size(px(24.0)).child("既定 Hello あいう"))
            .child(div().text_color(rgb(0xffffff)).text_size(px(24.0)).font_family("Helvetica").child("Helvetica Hello"))
            .child(div().text_color(rgb(0xff8080)).text_size(px(24.0)).font_family("ヒラギノ角ゴシック").child("ヒラギノ あいう"))
            .child(div().w(px(200.0)).h(px(20.0)).bg(rgb(0x4080ff)))
    }
}
