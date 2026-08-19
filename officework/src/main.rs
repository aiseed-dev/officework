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

/// `{"cmd":"open","path":"…"}` の道を取り出す(浅い読み)。
fn 開くファイル(line: &str) -> Option<String> {
    let o = ops::Jobj::parse(line)?;
    (o.str("cmd")? == "open").then(|| o.str("path"))?
}

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

impl Office {
    /// **持ち替えの頼みを受け取る**(2026-08-19 発注者「calc のファイルが
    /// 表示されるようにして」)。
    ///
    /// 文章の画面で表を押されたら表の画面に、表の画面で文書を押されたら
    /// 文章の画面に、*同じウィンドウで*持ち替えます。
    /// 別のアプリを起こすわけではありません。
    /// **持ち替えは画面ごと作り直します。** だから書きかけがあると消えます —
    /// 断るのはそのためです(writer のタブを閉じるときと同じ作法)。
    fn 持ち替え(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.shown {
            Shown::Doc(v) => {
                let Some(p) = v.update(cx, |w, _| w.hand_off.take()) else { return };
                if v.update(cx, |w, _| {
                    let 書きかけ = w.has_unsaved();
                    if 書きかけ {
                        w.say(ui::t!("書きかけがあります(先に保存してください)"));
                    }
                    書きかけ
                }) {
                    cx.notify();
                    return;
                }
                let n = cx.new(|cx| calc::Calc::new(Some(p), cx));
                window.focus(&n.focus_handle(cx), cx);
                self.shown = Shown::Sheet(n);
            }
            Shown::Sheet(v) => {
                let Some(p) = v.update(cx, |c, _| c.hand_off.take()) else { return };
                if v.update(cx, |c, _| {
                    let 書きかけ = c.has_unsaved();
                    if 書きかけ {
                        c.say(ui::t!("書きかけがあります(先に保存してください)"));
                    }
                    書きかけ
                }) {
                    cx.notify();
                    return;
                }
                let n = cx.new(|cx| writer::Writer::new(Some(p), cx));
                window.focus(&n.focus_handle(cx), cx);
                self.shown = Shown::Doc(n);
            }
        }
        cx.notify();
    }
}

impl Office {
    /// **受け口を開く**(2026-08-19)。名前は `officework` の1つです。
    ///
    /// 来た要求は*いま見せている画面*へ渡します。表を見ているなら表の動詞、
    /// 文章を見ているなら文章の動詞が効きます。どちらを見ているかは
    /// `{"cmd":"ping"}` の答えの `showing` で分かります。
    fn 受け口(view: gpui::Entity<Office>, cx: &mut App) {
        let queue: ops::Queue = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        if !ops::listen("officework", queue.clone()) {
            return;
        }
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(30))
                    .await;
                let reqs: Vec<ops::Req> =
                    std::mem::take(&mut *queue.lock().expect("受け口の錠"));
                if reqs.is_empty() {
                    continue;
                }
                let _ = view.update(cx, |this, cx| {
                    for req in reqs {
                        let resp = this.捌く(&req.line, cx);
                        let _ = req.reply.send(resp);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// 1要求を、いま見せている画面へ渡す。
    fn 捌く(&mut self, line: &str, cx: &mut Context<Self>) -> String {
        // `ping` だけはここで答える(どちらを見ているかを言うため)
        if line.contains("\"ping\"") {
            let showing = match &self.shown {
                Shown::Doc(_) => "doc",
                Shown::Sheet(_) => "sheet",
            };
            return format!(
                "{{\"ok\":true,\"app\":\"officework\",\"showing\":\"{showing}\",\"version\":\"{}\"}}",
                env!("CARGO_PKG_VERSION")
            );
        }
        // **`open` は名前で振り分けます**(2026-08-19)。渡された物が表なら
        // 表の画面に、文書なら文章の画面にしてから開きます。ここを通さないと
        // 「表を開いたのに紙面が出る」ことになります(実際に踏みました)
        if let Some(p) = 開くファイル(line) {
            let 表 = ui::folder::kind_of(
                &std::path::Path::new(&p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
            .is_sheet();
            let いま表 = matches!(self.shown, Shown::Sheet(_));
            if 表 != いま表 {
                // **持ち替えは画面ごと作り直す**ので、書きかけがあると消える。
                // 受け口から来た `open` でも同じ — 断り方は writer の rpc と
                // 揃える(向こうも「書きかけがあります」で止める)
                if self.書きかけがある(cx) {
                    // **窓にも言う。** 断りを頼んだ側にだけ返すと、画面を見て
                    // いる人には「押したのに何も起きない」に見える
                    self.言う(ui::t!("書きかけがあります(先に保存してください)"), cx);
                    return ops::err("書きかけがあります(先に保存してください)");
                }
                self.見せる(std::path::PathBuf::from(&p), 表, cx);
                return "{\"ok\":true}".into();
            }
        }
        match &self.shown {
            Shown::Doc(v) => v.update(cx, |w, _| writer::rpc::handle(w, line)),
            Shown::Sheet(v) => v.update(cx, |c, _| ops::handle(c, line)),
        }
    }

    /// いま見せている画面に書きかけがあるか。
    fn 書きかけがある(&self, cx: &mut Context<Self>) -> bool {
        match &self.shown {
            Shown::Doc(v) => v.read(cx).has_unsaved(),
            Shown::Sheet(v) => v.read(cx).has_unsaved(),
        }
    }

    /// いま見せている画面の状態行に出す。
    fn 言う(&self, msg: &str, cx: &mut Context<Self>) {
        match &self.shown {
            Shown::Doc(v) => v.update(cx, |w, _| w.say(msg.to_string())),
            Shown::Sheet(v) => v.update(cx, |c, _| c.say(msg.to_string())),
        }
    }

    /// 画面を作り直して、そのファイルを見せる。
    fn 見せる(&mut self, p: std::path::PathBuf, 表: bool, cx: &mut Context<Self>) {
        self.shown = if 表 {
            Shown::Sheet(cx.new(|cx| calc::Calc::new(Some(p), cx)))
        } else {
            Shown::Doc(cx.new(|cx| writer::Writer::new(Some(p), cx)))
        };
        cx.notify();
    }
}

impl Render for Office {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // **持ち替えの頼みは描く前に見ます。** 見落とすと、押しても何も
        // 起きないように見えます
        self.持ち替え(window, cx);
        // 選んだ編集画面をそのまま出します。ファイルのタブとフォルダの
        // 一覧は、それぞれの編集画面が持っているものを使います
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
                // **受け口を開く。** 名前は officework の1つ
                Office::受け口(view.clone(), cx);
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
