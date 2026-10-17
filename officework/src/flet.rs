//! **officework as the window of a Flet program** (SEKKEI "検討: officework を
//! Flet の画面側にする"; the rules are in docs/sekkei/flet-client.ja.adoc).
//!
//! Flet starts its client as `flet <page url> <pid file> [assets]`. When
//! officework is started that way (Flet finds it through `FLET_VIEW_PATH`,
//! under the name `flet`), it writes the pid file, connects to the page as
//! the client, keeps the control tree the Python side sends, and ends when
//! the Python side closes the socket.
//!
//! What is here now (step 2, 2026-10-17): the start, the registration, the
//! tree, a window that lists what arrived, and the end. Drawing the controls
//! as controls, and sending events, is step 3.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gpui::{div, prelude::*, px, size, App, Bounds, Context, Window, WindowBounds, WindowOptions};
use gpui_platform::application;

use fletview::client::{apply, page_props};
use fletview::message::RegisterResponse;
use fletview::{Client, ControlId, Incoming, Sender, Tree};

/// The arguments Flet starts the client with
pub struct Args {
    pub url: String,
    pub pid_file: PathBuf,
    /// The assets folder; used once images are drawn (step 3)
    #[allow(dead_code)]
    pub assets: Option<PathBuf>,
}

#[cfg(unix)]
fn is_socket(path: &str) -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::metadata(path).map(|m| m.file_type().is_socket()).unwrap_or(false)
}

#[cfg(not(unix))]
fn is_socket(_path: &str) -> bool {
    false
}

/// Recognises `officework <url> <pid file> [assets]`: the first argument is
/// `tcp://host:port` or a Unix socket, and a pid file follows. A document
/// path never looks like that.
pub fn parse(args: &[String]) -> Option<Args> {
    let url = args.get(1)?;
    let pid_file = args.get(2)?;
    if !(url.starts_with("tcp://") || is_socket(url)) {
        return None;
    }
    Some(Args { url: url.clone(), pid_file: PathBuf::from(pid_file), assets: args.get(3).map(PathBuf::from) })
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

/// What the socket thread hands to the window
enum Event {
    Registered(RegisterResponse, Sender),
    In(Incoming),
    /// The socket closed or failed; the text says which
    Ended(String),
}

type Queue = Arc<Mutex<Vec<Event>>>;

const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;

/// The window of a Flet page
struct FletView {
    tree: Tree,
    sender: Option<Sender>,
    status: String,
    title: String,
    ended: bool,
}

impl FletView {
    fn new() -> FletView {
        FletView { tree: Tree::new(), sender: None, status: "つないでいます".into(), title: "Flet".into(), ended: false }
    }

    fn take(&mut self, ev: Event, cx: &mut Context<Self>) {
        match ev {
            Event::Registered(r, sender) => {
                match self.tree.set_root(&r.page_patch) {
                    Ok(_) => self.status = format!("登録しました(session {})", r.session_id.unwrap_or_default()),
                    Err(e) => self.status = format!("Page が読めません: {e}"),
                }
                self.sender = Some(sender);
            }
            Event::In(incoming) => match apply(&mut self.tree, &incoming) {
                Ok(Some(changed)) => self.status = format!("差分を当てました({} 個の部品)", changed.len()),
                Ok(None) => {
                    if let Incoming::Crashed(m) = &incoming {
                        self.status = format!("Python が落ちました: {m}");
                    }
                }
                Err(e) => self.status = format!("差分が当てられません: {e}"),
            },
            Event::Ended(why) => {
                self.status = why;
                self.ended = true;
                // Flet's own client closes with the program; so does this window
                cx.quit();
            }
        }
        if let Some(t) = self.tree.root.and_then(|r| self.tree.get(r)).and_then(|p| p.get_str("title")) {
            self.title = t.to_string();
        }
        cx.notify();
    }

    /// One line per control under `id`, indented by depth
    fn lines(&self, id: ControlId, depth: usize, out: &mut Vec<String>) {
        let Some(c) = self.tree.get(id) else { return };
        let what = ["value", "content", "label", "text", "route"]
            .iter()
            .find_map(|k| c.get_str(k).map(|v| format!(": {v}")))
            .unwrap_or_default();
        out.push(format!("{}{}{}", "  ".repeat(depth), c.ctype, what));
        for prop in ["views", "controls", "content"] {
            for child in self.tree.children(id, prop) {
                self.lines(child, depth + 1, out);
            }
        }
    }
}

impl Render for FletView {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&self.title);
        let mut lines = Vec::new();
        if let Some(root) = self.tree.root {
            self.lines(root, 0, &mut lines);
        }
        let mut col = div().flex().flex_col().size_full().p_4().gap_1().bg(gpui::rgb(0xFFFFFF)).text_color(gpui::rgb(0x222222));
        col = col.child(div().text_lg().child(self.title.clone()));
        for l in lines {
            col = col.child(div().child(l));
        }
        col.child(div().mt_4().text_color(gpui::rgb(0x66707A)).child(self.status.clone()))
    }
}

/// Reads from the socket on its own thread and queues what arrives
fn listen(url: String, queue: Queue) {
    std::thread::spawn(move || {
        let push = |e: Event| {
            if let Ok(mut q) = queue.lock() {
                q.push(e);
            }
        };
        let result = (|| -> std::io::Result<()> {
            let mut c = Client::connect(&url)?;
            let r = c.register(page_props(WIDTH as f64, HEIGHT as f64, platform()))?;
            push(Event::Registered(r, c.sender()));
            loop {
                push(Event::In(c.next()?));
            }
        })();
        push(Event::Ended(match result {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => "Python が終わりました".to_string(),
            Err(e) => format!("つながりが切れました: {e}"),
            Ok(()) => "終わりました".to_string(),
        }));
    });
}

/// Starts as the client of the page and runs until the program ends
pub fn run(args: Args) {
    if let Err(e) = std::fs::write(&args.pid_file, std::process::id().to_string()) {
        eprintln!("pid ファイルに書けません({}): {e}", args.pid_file.display());
    }
    let queue: Queue = Arc::new(Mutex::new(Vec::new()));
    listen(args.url.clone(), queue.clone());
    application().with_assets(ui::Icons).run(move |cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![std::borrow::Cow::Borrowed(ops::font_data())])
            .expect("フォント登録");
        let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
        let queue = queue.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions { title: Some("Flet".into()), ..Default::default() }),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|_| FletView::new());
                // Every 30 ms, what the socket thread queued is applied on the
                // main thread (the same rule as calc's inlet)
                let v = view.clone();
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor().timer(std::time::Duration::from_millis(30)).await;
                        let events: Vec<Event> = match queue.lock() {
                            Ok(mut q) => std::mem::take(&mut *q),
                            Err(_) => Vec::new(),
                        };
                        if events.is_empty() {
                            continue;
                        }
                        let _ = v.update(cx, |this, cx| {
                            for e in events {
                                this.take(e, cx);
                            }
                        });
                    }
                })
                .detach();
                let v = view.clone();
                window.on_window_should_close(cx, move |_, cx| {
                    let _ = &v;
                    cx.quit();
                    true
                });
                view
            },
        )
        .expect("窓が開けません");
        cx.activate(true);
    });
}
