//! 端末を gpui で描く(Zed の terminal_element の蒸留。行ごとに字を組み、
//! 地の色は四角で敷く)。

use crate::{keys, Size, Terminal};
use gpui::{
    canvas, div, fill, point, prelude::*, px, size, App, Bounds, Context, FocusHandle, Font, FontFeatures,
    FontStyle, FontWeight, Hsla, KeyDownEvent, MouseButton, Pixels, Render, ScrollDelta, ScrollWheelEvent,
    SharedString, TextAlign, TextRun, UnderlineStyle, Window,
};
use std::path::PathBuf;
use std::sync::Arc;

/// 字の大きさ(画素)と行の高さの倍率
const FONT_PX: f32 = 13.0;
const LINE_SCALE: f32 = 1.35;

/// **端末のパネル。** 1つの [`Terminal`] を持ち、焦点があれば打鍵を流す
pub struct TermView {
    /// 起こせなかった時は Err(理由)。パネルはその文を出す
    term: Result<Arc<Terminal>, String>,
    focus: FocusHandle,
    font: Font,
    font_px: f32,
    /// 描き直しを促す刻み(33ms)を回しているか
    polling: bool,
}

fn hsla((r, g, b): (u8, u8, u8)) -> Hsla {
    gpui::rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32).into()
}

impl TermView {
    /// シェルを起こしてパネルを作る。`cwd` は開いている文書のフォルダなど
    pub fn new(cwd: Option<PathBuf>, ui_scale: f32, cx: &mut Context<Self>) -> Self {
        let font_px = FONT_PX * ui_scale;
        let cell_h = (font_px * LINE_SCALE) as u16;
        let term = Terminal::spawn(cwd, None, Size { cols: 80, rows: 24, cell_w: (font_px * 0.6) as u16, cell_h }).map(Arc::new);
        let font = Font {
            family: SharedString::from(mono_family()),
            features: FontFeatures::default(),
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
            fallbacks: None,
        };
        let mut v = TermView { term, focus: cx.focus_handle(), font, font_px, polling: false };
        if v.term.is_ok() {
            v.start_polling(cx);
        }
        v
    }

    /// 起こせなかった理由(起きていれば None)
    pub fn error(&self) -> Option<&str> {
        self.term.as_ref().err().map(|s| s.as_str())
    }

    /// 33ms ごとに端末の知らせを引き取り、変わっていれば描き直す
    fn start_polling(&mut self, cx: &mut Context<Self>) {
        if self.polling {
            return;
        }
        self.polling = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(33)).await;
                let alive = this.update(cx, |v, cx| {
                    if v.term.as_ref().is_ok_and(|t| t.drain()) {
                        cx.notify();
                    }
                    true
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// 端末の題(シェルが付けた物)。無ければ空
    pub fn title(&self) -> String {
        self.term.as_ref().map(|t| t.snapshot().title).unwrap_or_default()
    }

    /// シェルが終わっていれば終了コード
    pub fn exited(&self) -> Option<i32> {
        self.term.as_ref().ok().and_then(|t| t.snapshot().exited)
    }

    /// 字を送る(貼り付けなど)
    pub fn paste(&self, text: &str) {
        if let Ok(t) = &self.term {
            t.input(text.as_bytes().to_vec());
        }
    }

    fn key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let Ok(term) = self.term.clone() else { return };
        let ks = &ev.keystroke;
        // Ctrl+Shift+V は貼り付け(端末の慣習)
        if ks.modifiers.control && ks.modifiers.shift && ks.key == "v" {
            if let Some(item) = cx.read_from_clipboard() {
                if let Some(t) = item.text() {
                    self.paste(&t);
                }
            }
            cx.stop_propagation();
            return;
        }
        if let Some(s) = keys::to_esc(ks, term.app_cursor()) {
            term.input(s.into_bytes());
            cx.stop_propagation();
            return;
        }
        if let Some(ch) = &ks.key_char {
            if !ks.modifiers.control && !ks.modifiers.alt && !ks.modifiers.platform {
                term.input(ch.as_bytes().to_vec());
                cx.stop_propagation();
            }
        }
    }
}

/// この機械にある等幅の書体。日本語の幅が半角の2倍に揃う物を先に
fn mono_family() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else if cfg!(windows) {
        "Consolas"
    } else {
        "Noto Sans Mono CJK JP"
    }
}

impl Render for TermView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let term = match &self.term {
            Ok(t) => t.clone(),
            Err(e) => {
                return div().size_full().p_2().text_size(px(self.font_px)).child(e.clone()).into_any_element();
            }
        };
        let (bg, fg) = term.colors();
        let term2 = term.clone();
        let font = self.font.clone();
        let font2 = self.font.clone();
        let font_px = self.font_px;
        let line_h = px(font_px * LINE_SCALE);
        div()
            .id("term-view")
            // アプリの鍵の束縛(`jo_doc && !term` など)がここでは効かないための印
            .key_context("term")
            .track_focus(&self.focus)
            .size_full()
            .bg(hsla(bg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| window.focus(&this.focus, cx)),
            )
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| this.key(ev, cx)))
            .on_scroll_wheel(cx.listener(move |this, ev: &ScrollWheelEvent, _window, cx| {
                let lines = match ev.delta {
                    ScrollDelta::Lines(p) => p.y,
                    ScrollDelta::Pixels(p) => f32::from(p.y) / f32::from(line_h),
                };
                if let Ok(t) = &this.term {
                    t.scroll((lines * 3.0) as i32);
                }
                cx.notify();
            }))
            .child(
                canvas(
                    move |bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App| {
                        // セルの幅は "M" の幅で測る(等幅なので全部同じ)
                        let run = TextRun {
                            len: 1,
                            font: font.clone(),
                            color: gpui::white(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let cw = window.text_system().shape_line("M".into(), px(font_px), &[run], None).width;
                        let cw = if f32::from(cw) > 1.0 { cw } else { px(font_px * 0.6) };
                        let cols = (f32::from(bounds.size.width) / f32::from(cw)).floor().max(2.0) as u16;
                        let rows = (f32::from(bounds.size.height) / f32::from(line_h)).floor().max(1.0) as u16;
                        term.resize(Size { cols, rows, cell_w: f32::from(cw) as u16, cell_h: f32::from(line_h) as u16 });
                        let _ = cx;
                        cw
                    },
                    move |bounds: Bounds<Pixels>, cw: Pixels, window: &mut Window, cx: &mut App| {
                        let snap = term2.snapshot();
                        let fg_h = hsla(fg);
                        for (r, row) in snap.rows.iter().enumerate() {
                            let y = bounds.origin.y + line_h * (r as f32);
                            // 地の色(既定と違うセルだけ)
                            let mut col = 0usize;
                            for c in row {
                                let w = if c.wide { 2 } else { 1 };
                                if let Some(b) = c.bg {
                                    let x = bounds.origin.x + cw * (col as f32);
                                    window.paint_quad(fill(Bounds::new(point(x, y), size(cw * (w as f32), line_h)), hsla(b)));
                                }
                                col += w;
                            }
                            // 字は行ごとに1本の字列にして、色が変わる所で run を切る
                            let mut text = String::with_capacity(row.len());
                            let mut runs: Vec<TextRun> = Vec::new();
                            for c in row {
                                let mut buf = [0u8; 4];
                                let s = c.c.encode_utf8(&mut buf);
                                text.push_str(s);
                                let color = if c.fg == fg { fg_h } else { hsla(c.fg) };
                                let weight = if c.bold { FontWeight::BOLD } else { FontWeight::NORMAL };
                                let underline = c.underline.then(|| UnderlineStyle { thickness: px(1.0), color: Some(color), wavy: false });
                                let same = runs.last().is_some_and(|l| {
                                    l.color == color && l.font.weight == weight && l.underline.is_some() == underline.is_some()
                                });
                                if same {
                                    runs.last_mut().unwrap().len += s.len();
                                } else {
                                    let mut f = font2.clone();
                                    f.weight = weight;
                                    runs.push(TextRun { len: s.len(), font: f, color, background_color: None, underline, strikethrough: None });
                                }
                            }
                            let trimmed = text.trim_end().len();
                            if trimmed == 0 {
                                continue;
                            }
                            let line = window.text_system().shape_line(SharedString::from(text), px(font_px), &runs, None);
                            let _ = line.paint(point(bounds.origin.x, y), line_h, TextAlign::Left, None, window, cx);
                        }
                        if let Some((c, r)) = snap.cursor {
                            let x = bounds.origin.x + cw * (c as f32);
                            let y = bounds.origin.y + line_h * (r as f32);
                            let mut color = fg_h;
                            color.a = 0.6;
                            window.paint_quad(fill(Bounds::new(point(x, y), size(cw, line_h)), color));
                        }
                        if let Some(code) = snap.exited {
                            let note = format!("[終了 {code}]");
                            let run = TextRun { len: note.len(), font: font2.clone(), color: fg_h, background_color: None, underline: None, strikethrough: None };
                            let line = window.text_system().shape_line(SharedString::from(note), px(font_px), &[run], None);
                            let y = bounds.origin.y + line_h * (snap.rows.len() as f32 - 1.0);
                            let _ = line.paint(point(bounds.origin.x, y), line_h, TextAlign::Left, None, window, cx);
                        }
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }
}
