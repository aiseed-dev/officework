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

/// **起動したときに何を開くか**(SEKKEI「残りの実施方針」A-1)。
///
/// エディタ(VS Code・Zed)と同じで、*ふだんはフォルダを開きます*。
/// ファイルを名指しで渡す起動も残します — 関連付けから来るときの形です。
#[derive(Clone)]
enum Start {
    /// 引数で渡されたファイル
    File(std::path::PathBuf),
    /// フォルダ(中身の一覧を出す。ファイルは開かない)
    Folder(std::path::PathBuf),
    /// 覚えているフォルダが無い。**窓を出してから**選んでもらう
    AskFolder,
}

/// 起動の形を決める。**引数 → 前回のフォルダ → 選んでもらう**の順。
///
/// **選ぶ画面はここでは開きません。** `rfd` は同期で、返るまで戻って
/// きません。窓を作る前にこれを呼ぶと、答えが返らないときに*窓が1つも
/// 無いまま固まります* — 使う人からはアプリが起動しないのと同じです
/// (2026-08-19 に実際にそうなりました)。窓を出してから別の糸で聞きます。
fn 起動の形(arg: Option<std::path::PathBuf>) -> Start {
    if let Some(p) = arg {
        return Start::File(p);
    }
    match ui::settings::get("folder")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_dir())
    {
        Some(d) => Start::Folder(d),
        None => Start::AskFolder,
    }
}

/// **その名前は表か。** 中身は見ません(SEKKEI「画面を1つにする」)。
/// 起動・受け口・一覧のクリックの**3つとも同じ判定を通す**ための1箇所です。
fn 表か(p: &std::path::Path) -> bool {
    p.file_name()
        .map(|n| ui::folder::kind_of(&n.to_string_lossy()))
        .is_some_and(|k| k.is_sheet())
}

/// 開いたファイルのフォルダを覚える(次の起動でここが開きます)。
///
/// 関連付けからファイルを開いた回も覚えます — *使った場所が次に開く場所*で、
/// 一度も一覧を触っていない人でも「前回のフォルダ」が育ちます。
fn 覚える(path: Option<&std::path::Path>) {
    if let Some(d) = path.and_then(|p| p.parent()).filter(|d| d.is_dir()) {
        ui::settings::set("folder", &d.display().to_string());
    }
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
    fn new(start: Start, cx: &mut Context<Self>) -> Office {
        let path = match &start {
            Start::File(p) => Some(p.clone()),
            Start::Folder(_) | Start::AskFolder => None,
        };
        // 開いたファイルのフォルダを覚えます(次の起動でここが開きます)
        覚える(path.as_deref());
        let shown = if path.as_deref().is_some_and(表か) {
            let c = cx.new(|cx| calc::Calc::new(path, cx));
            // **埋め込みの印**(統合の段1)。一覧のクリックが officework 経由になります
            c.update(cx, |c, _| c.set_embedded());
            Shown::Sheet(c)
        } else {
            let w = cx.new(|cx| writer::Writer::new(path, cx));
            w.update(cx, |w, _| w.set_embedded());
            // フォルダで始めるときは、一覧を開いた姿にします
            if let Start::Folder(d) = start {
                w.update(cx, |w, _| w.show_folder(d));
            }
            Shown::Doc(w)
        };
        Office { shown }
    }
}

impl Office {
    /// **「このファイルを開いてほしい」を受け取る**(統合の段1)。
    ///
    /// 埋め込みの編集画面は、一覧を押されると**種類を問わず** `open_request`
    /// に置きます。ここが名前で行き先を決めます — 同じ種類ならその画面に
    /// そのまま開かせ、違う種類なら持ち替えます。
    ///
    /// **持ち替えはいまのところ画面ごと作り直します。** だから書きかけが
    /// あると消えるので断ります。段2 でタブの持ち主が officework に移ると
    /// 作り直しが無くなり、**この断りは要らなくなります**。
    fn 開く頼み(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let 頼み = match &self.shown {
            Shown::Doc(v) => v.update(cx, |w, _| w.open_request.take()),
            Shown::Sheet(v) => v.update(cx, |c, _| c.open_request.take()),
        };
        let Some(p) = 頼み else { return };
        let いま表 = matches!(self.shown, Shown::Sheet(_));
        // 同じ種類なら、その画面がそのまま開きます(持ち替えではない)
        if 表か(&p) == いま表 {
            match &self.shown {
                Shown::Doc(v) => v.update(cx, |w, _| w.open_path(p)),
                Shown::Sheet(v) => v.update(cx, |c, _| c.open_path(p)),
            }
            cx.notify();
            return;
        }
        if self.書きかけがある(cx) {
            self.言う(ui::t!("書きかけがあります(先に保存してください)"), cx);
            cx.notify();
            return;
        }
        self.見せる(p, !いま表, cx);
        match &self.shown {
            Shown::Doc(v) => window.focus(&v.focus_handle(cx), cx),
            Shown::Sheet(v) => window.focus(&v.focus_handle(cx), cx),
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
            let 表 = 表か(std::path::Path::new(&p));
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

    /// **フォルダを選んでもらう**(覚えている物が無い初回)。
    ///
    /// 窓が出てから別の糸で聞きます。`rfd` は同期なので主の糸では呼べません
    /// (呼ぶと、答えが返るまで画面が描かれません)。やめたときは何もしません —
    /// *空の文書のまま*で、ファイルの「開く」からいつでも始められます。
    fn フォルダを聞く(&self, cx: &mut Context<Self>) {
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .set_title(ui::t!("officework — 開くフォルダを選んでください"))
                .pick_folder()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(d) = r {
                    if let Shown::Doc(v) = &this.shown {
                        v.update(cx, |w, _| w.show_folder(d));
                    }
                }
                cx.notify();
            });
        })
        .detach();
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
        // **作った画面には必ず埋め込みの印を立てます。** 立て忘れると、
        // その画面だけ一覧のクリックを自分で握って持ち替えが効かなくなります
        self.shown = if 表 {
            let c = cx.new(|cx| calc::Calc::new(Some(p), cx));
            c.update(cx, |c, _| c.set_embedded());
            Shown::Sheet(c)
        } else {
            let w = cx.new(|cx| writer::Writer::new(Some(p), cx));
            w.update(cx, |w, _| w.set_embedded());
            Shown::Doc(w)
        };
        cx.notify();
    }
}

impl Render for Office {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // **開く頼みは描く前に見ます。** 見落とすと、押しても何も
        // 起きないように見えます
        self.開く頼み(window, cx);
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
    // **窓を開ける前に決めます。** 「フォルダを選ぶ画面」は同期で開くので、
    // GPUI が動き出す前に済ませます(主の糸を塞ぐ相手がまだいません)
    let start = 起動の形(arg);
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
        let start2 = start.clone();
        cx.open_window(
            WindowOptions { window_bounds: Some(wb), ..Default::default() },
            move |window, cx| {
                let view = cx.new(|cx| Office::new(start2.clone(), cx));
                // 焦点は中の編集画面へ渡します
                view.update(cx, |this, cx| match &this.shown {
                    Shown::Doc(v) => window.focus(&v.focus_handle(cx), cx),
                    Shown::Sheet(v) => window.focus(&v.focus_handle(cx), cx),
                });
                // **受け口を開く。** 名前は officework の1つ
                Office::受け口(view.clone(), cx);
                // 覚えているフォルダが無ければ、**窓が出てから**選んでもらう
                if matches!(start2, Start::AskFolder) {
                    view.update(cx, |this, cx| this.フォルダを聞く(cx));
                }
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
