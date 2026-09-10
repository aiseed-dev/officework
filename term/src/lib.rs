//! **端末のパネル。** 芯は alacritty_terminal(端末のエミュレータと pty)、描くのは
//! gpui。Zed の terminal / terminal_view の蒸留です(2026-09-08 発注者
//! 「ターミナルをつくって」)。
//!
//! 持つ物は3つだけです。シェルを起こして読み書きする [`Terminal`]、キーを
//! バイト列にする [`keys`]、画面に描く [`TermView`]。検索・リンク・タスク・
//! vi モードは持ちません(要る時に足す)。

pub mod keys;
mod view;

pub use view::TermView;

use alacritty_terminal::event::{Event, EventListener, Notify as _, OnResize as _, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Notifier};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{cell::Flags, Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use alacritty_terminal::tty;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// 端末からの知らせを渡す口(alacritty の EventListener)
#[derive(Clone)]
struct Listener(Sender<Event>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

/// 端末の大きさ(列・行・セルの画素)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
    pub cell_w: u16,
    pub cell_h: u16,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn columns(&self) -> usize {
        self.cols as usize
    }
}

impl From<Size> for WindowSize {
    fn from(s: Size) -> Self {
        WindowSize { num_lines: s.rows, num_cols: s.cols, cell_width: s.cell_w, cell_height: s.cell_h }
    }
}

/// 描くための1セル
#[derive(Debug, Clone, PartialEq)]
pub struct CellPaint {
    pub c: char,
    pub fg: (u8, u8, u8),
    pub bg: Option<(u8, u8, u8)>,
    pub bold: bool,
    pub underline: bool,
    pub wide: bool,
}

/// 描くための画面(見えている行だけ)
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub rows: Vec<Vec<CellPaint>>,
    /// カーソル(列, 行)。隠れていれば None
    pub cursor: Option<(usize, usize)>,
    pub cols: usize,
    /// 見ている所が下端から何行上か(0 なら最新)
    pub display_offset: usize,
    pub title: String,
    pub exited: Option<i32>,
}

/// **動いている端末**(シェルの子プロセス + エミュレータ)。スレッドをまたいで
/// 持てるので、gpui の描き直しの閉包にも渡せます
pub struct Terminal {
    term: Arc<FairMutex<Term<Listener>>>,
    notifier: Mutex<Notifier>,
    events: Mutex<Receiver<Event>>,
    size: Mutex<Size>,
    title: Mutex<String>,
    exited: Mutex<Option<i32>>,
    theme: Palette,
}

impl Terminal {
    /// シェルを起こす。`cwd` はシェルの最初のフォルダ。`shell` が None なら
    /// 機械の既定(unix は $SHELL、Windows は PowerShell)
    pub fn spawn(cwd: Option<PathBuf>, shell: Option<(String, Vec<String>)>, size: Size) -> Result<Terminal, String> {
        let (tx, rx) = channel();
        let listener = Listener(tx);
        let config = Config { scrolling_history: 10_000, ..Config::default() };
        let term = Term::new(config, &size, listener.clone());
        let term = Arc::new(FairMutex::new(term));
        let mut env = std::collections::HashMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());
        // 子の中で officework を使う人のために、動いている本体の受け口が
        // 分かるよう印を置く(officework の Python はこれを見ない — 参考)
        env.insert("OFFICEWORK_TERMINAL".to_string(), "1".to_string());
        let options = tty::Options {
            shell: shell.map(|(p, a)| tty::Shell::new(p, a)),
            working_directory: cwd,
            drain_on_exit: true,
            env,
            #[cfg(windows)]
            escape_args: false,
        };
        let pty = tty::new(&options, size.into(), 0).map_err(|e| format!("シェルを起こせません: {e}"))?;
        let event_loop = EventLoop::new(term.clone(), listener, pty, true, false)
            .map_err(|e| format!("端末の読み書きを始められません: {e}"))?;
        let notifier = Notifier(event_loop.channel());
        let _thread = event_loop.spawn();
        Ok(Terminal {
            term,
            notifier: Mutex::new(notifier),
            events: Mutex::new(rx),
            size: Mutex::new(size),
            title: Mutex::new(String::new()),
            exited: Mutex::new(None),
            theme: Palette::default(),
        })
    }

    /// キーや貼り付けの字を送る
    pub fn input(&self, bytes: impl Into<Vec<u8>>) {
        let b: Vec<u8> = bytes.into();
        if b.is_empty() {
            return;
        }
        // 打ったら最新の所へ戻す(Zed と同じ)
        self.term.lock().scroll_display(Scroll::Bottom);
        self.notifier.lock().expect("端末の錠").notify(b);
    }

    /// 大きさを変える(変わった時だけ pty にも伝える)
    pub fn resize(&self, size: Size) {
        let mut cur = self.size.lock().expect("端末の錠");
        if *cur == size || size.cols == 0 || size.rows == 0 {
            return;
        }
        *cur = size;
        self.term.lock().resize(size);
        self.notifier.lock().expect("端末の錠").on_resize(size.into());
    }

    pub fn size(&self) -> Size {
        *self.size.lock().expect("端末の錠")
    }

    /// 履歴をさかのぼる(正で上、負で下)
    pub fn scroll(&self, lines: i32) {
        self.term.lock().scroll_display(Scroll::Delta(lines));
    }

    /// 端末が DECCKM(アプリのカーソルキー)に入っているか
    pub fn app_cursor(&self) -> bool {
        self.term.lock().mode().contains(TermMode::APP_CURSOR)
    }

    /// 溜まった知らせを引き取る。描き直しが要れば true
    pub fn drain(&self) -> bool {
        let mut changed = false;
        let rx = self.events.lock().expect("端末の錠");
        while let Ok(e) = rx.try_recv() {
            match e {
                Event::Wakeup | Event::MouseCursorDirty | Event::CursorBlinkingChange => changed = true,
                Event::Title(t) => {
                    *self.title.lock().expect("端末の錠") = t;
                    changed = true;
                }
                Event::ResetTitle => {
                    self.title.lock().expect("端末の錠").clear();
                    changed = true;
                }
                Event::PtyWrite(s) => self.notifier.lock().expect("端末の錠").notify(s.into_bytes()),
                Event::ChildExit(code) => {
                    *self.exited.lock().expect("端末の錠") = Some(code);
                    changed = true;
                }
                Event::Exit => changed = true,
                Event::ClipboardStore(..) | Event::ClipboardLoad(..) | Event::ColorRequest(..) | Event::TextAreaSizeRequest(_) => {}
                Event::Bell => {}
            }
        }
        changed
    }

    /// 見えている画面を写す(描く側はこれだけ見る)
    pub fn snapshot(&self) -> Snapshot {
        let term = self.term.lock();
        let content = term.renderable_content();
        let size = self.size();
        let mut rows: Vec<Vec<CellPaint>> = (0..size.rows as usize)
            .map(|_| Vec::with_capacity(size.cols as usize))
            .collect();
        for cell in content.display_iter {
            let line = cell.point.line.0;
            // 履歴の行は負になる。見えている範囲は 0..rows
            if line < 0 || line as usize >= rows.len() {
                continue;
            }
            let flags = cell.flags;
            if flags.contains(Flags::WIDE_CHAR_SPACER) || flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let (mut fg, mut bg) = (self.theme.rgb(cell.fg, true, flags), self.theme.rgb(cell.bg, false, flags));
            if flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let bg = if bg == self.theme.bg { None } else { Some(bg) };
            rows[line as usize].push(CellPaint {
                c: if flags.contains(Flags::HIDDEN) { ' ' } else { cell.c },
                fg,
                bg,
                bold: flags.intersects(Flags::BOLD),
                underline: flags.intersects(Flags::UNDERLINE | Flags::DOUBLE_UNDERLINE),
                wide: flags.contains(Flags::WIDE_CHAR),
            });
        }
        let cursor = if content.mode.contains(TermMode::SHOW_CURSOR) && content.display_offset == 0 {
            let p = content.cursor.point;
            (p.line.0 >= 0).then_some((p.column.0, p.line.0 as usize))
        } else {
            None
        };
        Snapshot {
            rows,
            cursor,
            cols: size.cols as usize,
            display_offset: content.display_offset,
            title: self.title.lock().expect("端末の錠").clone(),
            exited: *self.exited.lock().expect("端末の錠"),
        }
    }

    /// 既定の地の色と字の色
    pub fn colors(&self) -> ((u8, u8, u8), (u8, u8, u8)) {
        (self.theme.bg, self.theme.fg)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Ok(n) = self.notifier.lock() {
            let _ = n.0.send(alacritty_terminal::event_loop::Msg::Shutdown);
        }
    }
}

/// 色の表(xterm の 16 色 + 216 色の立方体 + 24 段の灰色)。暗い地
#[derive(Debug, Clone)]
pub struct Palette {
    pub bg: (u8, u8, u8),
    pub fg: (u8, u8, u8),
    named: [(u8, u8, u8); 16],
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            bg: (0x1B, 0x1E, 0x21),
            fg: (0xD6, 0xDB, 0xDF),
            named: [
                (0x1B, 0x1E, 0x21), // black
                (0xE0, 0x6C, 0x75), // red
                (0x98, 0xC3, 0x79), // green
                (0xE5, 0xC0, 0x7B), // yellow
                (0x61, 0xAF, 0xEF), // blue
                (0xC6, 0x78, 0xDD), // magenta
                (0x56, 0xB6, 0xC2), // cyan
                (0xC8, 0xCC, 0xD0), // white
                (0x5C, 0x63, 0x70), // bright black
                (0xF0, 0x8A, 0x92), // bright red
                (0xB5, 0xE0, 0x9A), // bright green
                (0xF2, 0xD4, 0x97), // bright yellow
                (0x87, 0xC4, 0xFF), // bright blue
                (0xDC, 0x9C, 0xEC), // bright magenta
                (0x7E, 0xD3, 0xDE), // bright cyan
                (0xF4, 0xF6, 0xF8), // bright white
            ],
        }
    }
}

impl Palette {
    fn rgb(&self, c: Color, is_fg: bool, flags: Flags) -> (u8, u8, u8) {
        match c {
            Color::Spec(Rgb { r, g, b }) => (r, g, b),
            Color::Indexed(i) => self.indexed(i),
            Color::Named(n) => {
                let base = match n {
                    NamedColor::Foreground | NamedColor::Cursor => self.fg,
                    NamedColor::Background => self.bg,
                    NamedColor::DimForeground => dim(self.fg),
                    NamedColor::BrightForeground => self.fg,
                    NamedColor::DimBlack => dim(self.named[0]),
                    NamedColor::DimRed => dim(self.named[1]),
                    NamedColor::DimGreen => dim(self.named[2]),
                    NamedColor::DimYellow => dim(self.named[3]),
                    NamedColor::DimBlue => dim(self.named[4]),
                    NamedColor::DimMagenta => dim(self.named[5]),
                    NamedColor::DimCyan => dim(self.named[6]),
                    NamedColor::DimWhite => dim(self.named[7]),
                    other => self.named[(other as usize).min(15)],
                };
                // 太字の時は明るい方の色(端末の慣習)
                if is_fg && flags.contains(Flags::BOLD) && (n as usize) < 8 {
                    self.named[n as usize + 8]
                } else if is_fg && flags.contains(Flags::DIM) {
                    dim(base)
                } else {
                    base
                }
            }
        }
    }

    fn indexed(&self, i: u8) -> (u8, u8, u8) {
        match i {
            0..=15 => self.named[i as usize],
            16..=231 => {
                let i = i - 16;
                let step = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                (step(i / 36), step((i / 6) % 6), step(i % 6))
            }
            232..=255 => {
                let v = 8 + (i - 232) * 10;
                (v, v, v)
            }
        }
    }
}

fn dim((r, g, b): (u8, u8, u8)) -> (u8, u8, u8) {
    (r * 2 / 3, g * 2 / 3, b * 2 / 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_covers_named_indexed_and_true_colors() {
        let p = Palette::default();
        assert_eq!(p.indexed(196), (255, 0, 0));
        assert_eq!(p.indexed(232), (8, 8, 8));
        assert_eq!(p.rgb(Color::Spec(Rgb { r: 1, g: 2, b: 3 }), true, Flags::empty()), (1, 2, 3));
        assert_eq!(p.rgb(Color::Named(NamedColor::Red), true, Flags::BOLD), p.named[9], "太字の赤は明るい赤");
    }

    /// 本物のシェルを起こして、書いた字が画面に出る所まで(unix だけ)
    #[test]
    #[cfg(unix)]
    fn a_shell_echo_shows_up_in_the_snapshot() {
        let size = Size { cols: 40, rows: 8, cell_w: 8, cell_h: 16 };
        let t = Terminal::spawn(None, Some(("/bin/sh".into(), vec!["-c".into(), "printf 'hello-term'; sleep 0.3".into()])), size).unwrap();
        let mut seen = false;
        for _ in 0..100 {
            t.drain();
            let s = t.snapshot();
            let text: String = s.rows.iter().map(|r| r.iter().map(|c| c.c).collect::<String>()).collect::<Vec<_>>().join("\n");
            if text.contains("hello-term") {
                seen = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        assert!(seen, "シェルの出力が画面に出ない");
        for _ in 0..100 {
            t.drain();
            if t.snapshot().exited.is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        panic!("シェルの終わりが伝わらない");
    }
}
