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

/// 開いているファイル1枚ぶん。**タブ1つ = ファイル1つ = 編集画面1つ**
/// (SEKKEI「writer と calc を officework に統合する」段2)。
enum Pane {
    /// 文章(`.adoc` `.docx` …)
    Doc(Entity<writer::Writer>),
    /// 表(`.sheet.adoc` `.xlsx`)
    Sheet(Entity<calc::Calc>),
}

impl Pane {
    /// この画面が開いているファイルの道。
    fn 道(&self, cx: &App) -> Option<std::path::PathBuf> {
        match self {
            Pane::Doc(v) => v.read(cx).opened_path().map(|p| p.to_path_buf()),
            Pane::Sheet(v) => v.read(cx).opened_path().map(|p| p.to_path_buf()),
        }
    }

    /// 書きかけがあるか。
    fn 書きかけ(&self, cx: &App) -> bool {
        match self {
            Pane::Doc(v) => v.read(cx).has_unsaved(),
            Pane::Sheet(v) => v.read(cx).has_unsaved(),
        }
    }

    fn focus(&self, cx: &App) -> gpui::FocusHandle {
        match self {
            Pane::Doc(v) => v.focus_handle(cx),
            Pane::Sheet(v) => v.focus_handle(cx),
        }
    }
}

/// アプリの本体。**開いた物を並べて持つだけ**で、編集の中身は持ちません。
///
/// 段2 で「1つだけ見せて、持ち替えは作り直し」をやめました。
/// *作り直さないので書きかけは消えません* — 持ち替えの断りもここで外しています。
struct Office {
    /// 開いている物(タブの並び)
    tabs: Vec<Pane>,
    /// いま見ているのは何枚目か
    at: usize,
    /// 次に描くときに焦点を移すか(受け口には `Window` が無いため)
    焦点を移す: bool,
}

impl Office {
    fn new(start: Start, cx: &mut Context<Self>) -> Office {
        let path = match &start {
            Start::File(p) => Some(p.clone()),
            Start::Folder(_) | Start::AskFolder => None,
        };
        // 開いたファイルのフォルダを覚えます(次の起動でここが開きます)
        覚える(path.as_deref());
        let pane = if path.as_deref().is_some_and(表か) {
            Pane::Sheet(作る表(path, cx))
        } else {
            let w = 作る文書(path, cx);
            // フォルダで始めるときは、一覧を開いた姿にします
            if let Start::Folder(d) = start {
                w.update(cx, |w, _| w.show_folder(d));
            }
            Pane::Doc(w)
        };
        Office { tabs: vec![pane], at: 0, 焦点を移す: false }
    }
}

/// 表の編集画面を作る。**埋め込みの印は必ず立てる** —
/// 立て忘れると、その画面だけ一覧のクリックを自分で握ります
fn 作る表(path: Option<std::path::PathBuf>, cx: &mut Context<Office>) -> Entity<calc::Calc> {
    let c = cx.new(|cx| calc::Calc::new(path, cx));
    c.update(cx, |c, _| c.set_embedded());
    c
}

/// 文章の編集画面を作る(同上)。
fn 作る文書(path: Option<std::path::PathBuf>, cx: &mut Context<Office>) -> Entity<writer::Writer> {
    let w = cx.new(|cx| writer::Writer::new(path, cx));
    w.update(cx, |w, _| w.set_embedded());
    w
}

impl Office {
    /// **「このファイルを開いてほしい」を受け取る**(統合の段1)。
    ///
    /// 埋め込みの編集画面は、一覧を押されると**種類を問わず** `open_request`
    /// に置きます。ここが受け取って、タブとして開きます。
    ///
    /// **もう画面を作り直しません**(段2)。すでに開いているファイルなら
    /// そのタブへ行き、無ければ新しいタブを足すだけです。書きかけは
    /// タブの中に生きたまま残るので、**持ち替えの断りは要らなくなりました**。
    fn 開く頼み(&mut self, cx: &mut Context<Self>) {
        let 頼み = match self.見ている() {
            Pane::Doc(v) => v.update(cx, |w, _| w.open_request.take()),
            Pane::Sheet(v) => v.update(cx, |c, _| c.open_request.take()),
        };
        let Some(p) = 頼み else { return };
        self.タブで開く(p, cx);
    }

    /// いま見ているタブ。
    fn 見ている(&self) -> &Pane {
        &self.tabs[self.at.min(self.tabs.len() - 1)]
    }

    /// **タブとして開く。** すでに開いていればそのタブへ移るだけです —
    /// 同じファイルを二重に開くと、どちらを保存したのか分からなくなります。
    /// **焦点は描くときに移します。** ここは受け口からも呼ばれ、そちらには
    /// `Window` がありません。窓を持っている `render` に1回だけ任せます
    fn タブで開く(&mut self, p: std::path::PathBuf, cx: &mut Context<Self>) {
        覚える(Some(&p));
        if let Some(i) = self.tabs.iter().position(|t| t.道(cx).as_deref() == Some(p.as_path())) {
            self.at = i;
        } else {
            let pane = if 表か(&p) {
                Pane::Sheet(作る表(Some(p), cx))
            } else {
                Pane::Doc(作る文書(Some(p), cx))
            };
            self.tabs.push(pane);
            self.at = self.tabs.len() - 1;
        }
        self.焦点を移す = true;
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
            let showing = match self.見ている() {
                Pane::Doc(_) => "doc",
                Pane::Sheet(_) => "sheet",
            };
            return format!(
                "{{\"ok\":true,\"app\":\"officework\",\"showing\":\"{showing}\",\"version\":\"{}\"}}",
                env!("CARGO_PKG_VERSION")
            );
        }
        // **`open` はタブとして開きます**(段2)。名前で行き先の画面が
        // 決まり、すでに開いていればそのタブへ移ります。**断りません** —
        // 画面を作り直さないので、書きかけは消えません
        if let Some(p) = 開くファイル(line) {
            self.タブで開く(std::path::PathBuf::from(&p), cx);
            return "{\"ok\":true}".into();
        }
        match self.見ている() {
            Pane::Doc(v) => v.update(cx, |w, _| writer::rpc::handle(w, line)),
            Pane::Sheet(v) => v.update(cx, |c, _| ops::handle(c, line)),
        }
    }

    /// **書きかけのあるタブの名前。** 閉じるときに、どれが残っているかを言います。
    /// 数だけ言われても、どれを保存すればよいのか分かりません。
    fn 書きかけの名前(&self, cx: &App) -> Vec<String> {
        self.tabs
            .iter()
            .filter(|t| t.書きかけ(cx))
            .map(|t| match t.道(cx) {
                Some(p) => {
                    let n = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    ui::folder::display_name(&n, ui::folder::kind_of(&n))
                }
                None => ui::t!("(名前なし)").to_string(),
            })
            .collect()
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
                    if let Pane::Doc(v) = this.見ている() {
                        v.update(cx, |w, _| w.show_folder(d));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// いま見ているタブの状態行に出す。
    fn 言う(&self, msg: &str, cx: &mut Context<Self>) {
        match self.見ている() {
            Pane::Doc(v) => v.update(cx, |w, _| w.say(msg.to_string())),
            Pane::Sheet(v) => v.update(cx, |c, _| c.say(msg.to_string())),
        }
    }
}

impl Render for Office {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // **開く頼みは描く前に見ます。** 見落とすと、押しても何も
        // 起きないように見えます
        self.開く頼み(cx);
        // タブを替えた回だけ焦点を移します(受け口から替わった分もここで拾う)
        if self.焦点を移す {
            self.焦点を移す = false;
            window.focus(&self.見ている().focus(cx), cx);
        }
        // いま見ているタブをそのまま出します。上のタブの行は段2の続きで
        // ここに足します(いまは編集画面が持っている物がそのまま出ます)
        div().size_full().child(match self.見ている() {
            Pane::Doc(v) => v.clone().into_any_element(),
            Pane::Sheet(v) => v.clone().into_any_element(),
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
                view.update(cx, |this, cx| {
                    window.focus(&this.見ている().focus(cx), cx)
                });
                // **受け口を開く。** 名前は officework の1つ
                Office::受け口(view.clone(), cx);
                // **閉じるときは全部のタブに聞きます。**
                //
                // ここは今まで**誰も見ていませんでした** — 終了確認は writer と
                // calc がそれぞれの `main` で結んでいて、officework の窓には
                // 付いていなかったのです。タブを持てるようになった以上、
                // 黙って閉じると**裏のタブの書きかけまで一度に消えます**。
                //
                // いまは「書きかけがあるなら閉じない」と断るだけです。
                // 保存するか捨てるかを選ばせる確認は段3 で作ります
                let v = view.clone();
                window.on_window_should_close(cx, move |_, cx| {
                    v.update(cx, |this, cx| {
                        let 残り: Vec<String> = this.書きかけの名前(cx);
                        if 残り.is_empty() {
                            return true;
                        }
                        this.言う(
                            &ui::tf!("書きかけがあります(先に保存してください): {}",
                                     残り.join(" / ")),
                            cx,
                        );
                        cx.notify();
                        false
                    })
                });
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
