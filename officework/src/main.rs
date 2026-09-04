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
#[cfg(unix)]
fn file_to_open(line: &str) -> Option<String> {
    let o = ops::Jobj::parse(line)?;
    (o.str("cmd")? == "open").then(|| o.str("path"))?
}

/// **宛先の道**(命令に付いた `path`。統合の段10)。
///
/// 道具(MCP・Python の橋)が「開いてから操作する」を1往復でできるように
/// します。付いていなければ、いま見ているタブが相手です。
///
/// # `path` が宛先ではない命令
///
/// `open` は**開く相手**、`save` と `to_pdf` は**書き出す先**です。
/// ここで宛先と読むと、その道が丸ごと壊れます — 実際に壊れていました
/// (2026-08-20)。`{"cmd":"save","path":"新しい名前.docx"}` が
/// 「新しい名前.docx のタブを探す」になり、まだ無いので
/// 「そのファイルは見つかりません」で断られていました。
/// **名前を付けて保存が道具から使えない**状態です。
#[cfg(unix)]
fn dest(line: &str) -> Option<String> {
    let o = ops::Jobj::parse(line)?;
    match o.str("cmd")?.as_str() {
        "open" | "save" | "to_pdf" => None,
        _ => o.str("path"),
    }
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
    /// **前回の姿**(開いていたファイルの並びと、見ていたタブ)。
    /// 2つ目は開けなくなっていた数 — 黙って減らさないので画面で言います
    Before(face::session::Session, usize),
    /// 覚えているフォルダが無い。**窓を出してから**選んでもらう
    AskFolder,
}

/// 起動の形を決める。**引数 → 前回の姿 → 選んでもらう**の順。
///
/// 引数つき(関連付けから1つのファイルを開く形)は**そのファイルだけ**を
/// 開きます — 「これを見たい」と言われたときに他の物まで開くと邪魔です。
///
/// **選ぶ画面はここでは開きません。** `rfd` は同期で、返るまで戻って
/// きません。窓を作る前にこれを呼ぶと、答えが返らないときに*窓が1つも
/// 無いまま固まります* — 使う人からはアプリが起動しないのと同じです
/// (2026-08-19 に実際にそうなりました)。窓を出してから別の糸で聞きます。
fn launch_shape(arg: Option<std::path::PathBuf>) -> Start {
    if let Some(p) = arg {
        // **フォルダを渡されたら、フォルダとして開きます**(2026-08-24)。
        // 前はファイルとして開こうとして、読めずに白紙が出ていました。
        // 綴りはフォルダなので、`officework 仕事のフォルダ` は
        // 「その綴りを開く」の意味です(エディタと同じ作法)
        return if p.is_dir() { Start::Folder(p) } else { Start::File(p) };
    }
    // **前の版から上げた人の「前回のフォルダ」も拾います**(1回だけ)
    let before = face::session::inherit(ui::settings::get("folder"));
    let (before, dropped) = face::session::prune(&before);
    if !before.files.is_empty() {
        return Start::Before(before, dropped);
    }
    match before.folder {
        Some(d) => Start::Folder(d),
        None => Start::AskFolder,
    }
}

/// **その名前は表か。** 中身は見ません(SEKKEI「画面を1つにする」)。
/// 起動・受け口・一覧のクリックの**3つとも同じ判定を通す**ための1箇所です。
fn is_table(p: &std::path::Path) -> bool {
    p.file_name()
        .map(|n| ui::folder::kind_of(&n.to_string_lossy()))
        .is_some_and(|k| k.is_sheet())
}

/// 開いているファイル1枚ぶん。**タブ1つ = ファイル1つ = 編集画面1つ**
/// (SEKKEI「writer と calc を officework に統合する」段2)。
#[derive(Clone)]
enum Pane {
    /// 文章(`.adoc` `.docx` …)
    Doc(Entity<writer::Writer>),
    /// 表(`.sheet.adoc` `.xlsx`)
    Sheet(Entity<calc::Calc>),
}

impl Pane {
    /// この画面が開いているファイルの道。
    fn path(&self, cx: &App) -> Option<std::path::PathBuf> {
        match self {
            Pane::Doc(v) => v.read(cx).opened_path().map(|p| p.to_path_buf()),
            Pane::Sheet(v) => v.read(cx).opened_path().map(|p| p.to_path_buf()),
        }
    }

    /// 書きかけがあるか。
    fn draft(&self, cx: &App) -> bool {
        match self {
            Pane::Doc(v) => v.read(cx).has_unsaved(),
            Pane::Sheet(v) => v.read(cx).has_unsaved(),
        }
    }

    /// **いまファイルのページを出しているか**(統合の段8。2026-09-04)。
    fn on_file_page(&self, cx: &App) -> bool {
        match self {
            Pane::Doc(v) => v.read(cx).on_file_page(),
            Pane::Sheet(v) => v.read(cx).on_file_page(),
        }
    }

    /// ファイルのページに並べる項目(左の列)。
    fn file_items(&self, cx: &App) -> Vec<ui::filemenu::Item> {
        match self {
            Pane::Doc(v) => v.read(cx).file_menu(),
            Pane::Sheet(v) => v.read(cx).file_menu(),
        }
    }

    /// **場所の控え**(点検の道具が座標を当てずに押せるように)。
    /// 編集画面の物を借ります — 押した先もその画面なので、控えも1つで足ります
    fn boxes(&self, cx: &App) -> ui::filemenu::Boxes {
        match self {
            Pane::Doc(v) => v.read(cx).btn_boxes(),
            Pane::Sheet(v) => v.read(cx).btn_boxes(),
        }
    }

    /// 画面が暗い側か(タブの行の色を下の編集画面に合わせる)。
    fn is_dark(&self, cx: &App) -> bool {
        match self {
            Pane::Doc(v) => v.read(cx).is_dark(),
            Pane::Sheet(v) => v.read(cx).is_dark(),
        }
    }

    /// タブに出す名前。**二重の拡張子は落とします**(一覧と同じ見せ方)。
    fn name(&self, cx: &App) -> String {
        match self.path(cx) {
            Some(p) => {
                let n = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                ui::folder::display_name(&n, ui::folder::kind_of(&n))
            }
            None => ui::t!("unnamed").to_string(),
        }
    }

    /// いま選んでいるリボンの段が、**揃えた並びで何番目か**。
    /// 番号は画面ごとに違う(文章に無い段があるため)ので、
    /// 持ち越すときは**揃えた並びの位置**に直してから渡します。
    fn tab_pos(&self, cx: &App) -> Option<usize> {
        let order = ui::tabs::merged();
        match self {
            Pane::Doc(v) => {
                let i = v.read(cx).ribbon_tab();
                order.iter().position(|s| s.doc == Some(i))
            }
            Pane::Sheet(v) => {
                let i = v.read(cx).ribbon_tab();
                order.iter().position(|s| s.sheet == Some(i))
            }
        }
    }

    /// 揃えた並びの位置で段を選ぶ。**この画面に無い段なら動かしません** —
    /// 無理に近くの段へ寄せると、押していないのに別の段が開いて見えます。
    fn align_tabs(&self, positions: usize, cx: &mut App) {
        let order = ui::tabs::merged();
        let Some(slot) = order.get(positions) else { return };
        match self {
            Pane::Doc(v) => {
                if let Some(i) = slot.doc {
                    v.update(cx, |w, _| w.set_ribbon_tab(i));
                }
            }
            Pane::Sheet(v) => {
                if let Some(i) = slot.sheet {
                    v.update(cx, |c, _| c.set_ribbon_tab(i));
                }
            }
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
    move_focus: bool,
    /// **編集画面が変わったら描き直すための見張り**(統合の段8。2026-09-04)。
    ///
    /// ファイルのページの右側は編集画面が組みます。その中の押しは編集画面に
    /// しか届かないので、こちらが見張っていないと**押しても画面が変わりません**。
    /// `Subscription` は落とすと止まるので、ここで持ちます
    subs: Vec<gpui::Subscription>,
}

impl Office {
    fn new(start: Start, cx: &mut Context<Self>) -> Office {
        // **前回の姿で始める**(段4)。開いていた並びをそのまま作り直します
        if let Start::Before(before, dropped) = &start {
            let mut tabs: Vec<Pane> = Vec::new();
            for f in &before.files {
                tabs.push(if is_table(f) {
                    Pane::Sheet(table_to_make(Some(f.clone()), cx))
                } else {
                    Pane::Doc(doc_to_make(Some(f.clone()), cx))
                });
            }
            let at = before.at.min(tabs.len().saturating_sub(1));
            let o = Office { tabs, at, move_focus: true, subs: Vec::new() };
            // **黙って減らさない。** 開けなかった数は状態行で言います
            if *dropped > 0 {
                o.told(&ui::tf!("file_s_open_not", dropped.to_string()), cx);
                // **控えも今の姿に直します。** 直さないと、消えたファイルが
                // 記録に残り続け、開くたびに同じ数を報せることになります
                o.save_snapshot(cx);
            }
            return o;
        }
        let path = match &start {
            Start::File(p) => Some(p.clone()),
            Start::Folder(_) | Start::AskFolder | Start::Before(..) => None,
        };
        let pane = if path.as_deref().is_some_and(is_table) {
            Pane::Sheet(table_to_make(path, cx))
        } else {
            let w = doc_to_make(path, cx);
            // フォルダで始めるときは、一覧を開いた姿にします
            if let Start::Folder(d) = start {
                w.update(cx, |w, _| w.show_folder(d));
            }
            Pane::Doc(w)
        };
        Office { tabs: vec![pane], at: 0, move_focus: false, subs: Vec::new() }
    }
}

/// 表の編集画面を作る。**埋め込みの印は必ず立てる** —
/// 立て忘れると、その画面だけ一覧のクリックを自分で握ります
fn table_to_make(path: Option<std::path::PathBuf>, cx: &mut Context<Office>) -> Entity<calc::Calc> {
    // **`funcs/*.py` を先に読み直す。** ブックを開く前に揃っていないと、
    // `=集計(A1)` が UDF だと分からず `#NAME?` になります
    // (2026-08-21。前は統合アプリで UDF が1つも効きませんでした)
    calc::refresh_udfs_if_changed();
    let c = cx.new(|cx| calc::Calc::new(path, cx));
    c.update(cx, |c, _| c.set_embedded());
    // 置き場が変われば計算し直す見張り。**表を1つ作るたびに**始めます
    calc::start_udf_watch(c.clone(), cx);
    c
}

/// 文章の編集画面を作る(同上)。
fn doc_to_make(path: Option<std::path::PathBuf>, cx: &mut Context<Office>) -> Entity<writer::Writer> {
    let w = cx.new(|cx| writer::Writer::new(path, cx));
    w.update(cx, |w, _| w.set_embedded());
    w
}

impl Office {
    /// **編集画面が変わったら描き直す**(統合の段8。2026-09-04)。
    ///
    /// ファイルのページの右側は編集画面が組むので、その中で何か押されても
    /// 変わるのは編集画面だけです。こちらも描き直さないと、押した結果が
    /// 見えません。タブの名前と書きかけの印も、これで新しくなります
    fn watch_all(&mut self, cx: &mut Context<Self>) {
        let mina: Vec<Pane> = self.tabs.clone();
        for p in &mina {
            self.watch(p, cx);
        }
    }

    fn watch(&mut self, pane: &Pane, cx: &mut Context<Self>) {
        let s = match pane {
            Pane::Doc(v) => cx.observe(v, |_, _, cx| cx.notify()),
            Pane::Sheet(v) => cx.observe(v, |_, _, cx| cx.notify()),
        };
        self.subs.push(s);
    }

    /// **ファイルのページを描く**(統合の段8 の本体。2026-09-04)。
    ///
    /// このページは全面で、ほかの部品と隣り合いません。だから割り込みでは
    /// なく**ページごと差し替え**ます。編集画面は埋め込みのとき自分では
    /// 組まず、officework がここで組みます(SEKKEI「ページごと差し替え」)。
    ///
    /// * 左の列(戻る・新規・開く・最近…)は **officework の持ち物**です。
    ///   並びは編集画面に聞き([`Pane::file_items`])、押しはその画面へ返します
    /// * 右側(いま見ているファイルの詳細と操作)は**編集画面が組みます**。
    ///   編集の中身なので、層の線を越えないためです
    fn file_page(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let dk = self.showing().is_dark(cx);
        let look = ui::filemenu::SideLook {
            bg: if dk { gpui::rgb(0x1B1E21) } else { gpui::rgb(0xF1F3F5) },
            border: if dk { gpui::rgb(0x33383D) } else { gpui::rgb(0xE1E6EA) },
            fg: if dk { gpui::rgb(0xCFD6DC) } else { gpui::rgb(0x444B52) },
            gray: if dk { gpui::rgb(0x565D64) } else { gpui::rgb(0xB6BDC4) },
            hover: if dk { gpui::rgb(0x2C333A) } else { gpui::rgb(0xE2E6EA) },
            scale: ui::ui_scale(),
        };
        let items = self.showing().file_items(cx);
        // **押しは、いま見ている画面へそのまま返します。** 捌くのは
        // `ui::filemenu::run` と各画面の `file_menu_click` で、単体で
        // 動くときと同じ道です
        let hako = self.showing().boxes(cx);
        let sb = ui::filemenu::sidebar(&look, &items, Some(hako), cx, |this: &mut Office, id, cx| {
            match this.showing().clone() {
                Pane::Doc(v) => v.update(cx, |w, cx| w.file_menu_click(id, cx)),
                Pane::Sheet(v) => v.update(cx, |c, cx| c.file_menu_click(id, cx)),
            }
            cx.notify()
        });
        // 右側。**編集画面の文脈で組みます** — 中の押しはその画面に届き、
        // こちらは [`Office::watch`] の見張りで描き直します
        let pane = match self.showing().clone() {
            Pane::Doc(v) => v.update(cx, |w, cx| w.file_pane(cx)),
            Pane::Sheet(v) => v.update(cx, |c, cx| c.file_pane(cx)),
        };
        // **控えを書き出します**(点検の道具のため)。ページを描くのが
        // officework になったので、編集画面の `render` は通りません —
        // ここで頼まないと、実機の点検が左の列を押せなくなります
        if let Pane::Doc(v) = self.showing() {
            v.read(cx).dump_ui();
        }
        div()
            .flex_1()
            .min_h(px(0.0))
            .flex()
            .flex_row()
            .child(sb)
            .child(pane)
            .into_any_element()
    }

    /// **「このファイルを開いてほしい」を受け取る**(統合の段1)。
    ///
    /// 埋め込みの編集画面は、一覧を押されると**種類を問わず** `open_request`
    /// に置きます。ここが受け取って、タブとして開きます。
    ///
    /// **もう画面を作り直しません**(段2)。すでに開いているファイルなら
    /// そのタブへ行き、無ければ新しいタブを足すだけです。書きかけは
    /// タブの中に生きたまま残るので、**持ち替えの断りは要らなくなりました**。
    fn open_request(&mut self, cx: &mut Context<Self>) {
        // 「開く」の窓を出してほしい(Ctrl+O。統合の段3)
        let show_window = match self.showing() {
            Pane::Doc(v) => v.update(cx, |w, _| std::mem::take(&mut w.open_dialog_request)),
            Pane::Sheet(v) => v.update(cx, |c, _| std::mem::take(&mut c.open_dialog_request)),
        };
        if show_window {
            self.window_to_open(cx);
        }
        let request = match self.showing() {
            Pane::Doc(v) => v.update(cx, |w, _| w.open_request.take()),
            Pane::Sheet(v) => v.update(cx, |c, _| c.open_request.take()),
        };
        let Some(p) = request else { return };
        self.open_in_tab(p, cx);
    }

    /// **開くファイルを選ぶ窓**(統合の段3)。
    ///
    /// 文章も表も同じ窓から選べます — *どちらの画面で開くかは名前が決める*ので、
    /// 選ぶ側で種類を絞る理由がありません。前は編集画面ごとに窓があり、
    /// 文章の画面からは表が選べませんでした。
    ///
    /// `rfd` は同期なので**別の糸**で開きます(主の糸で呼ぶと画面が止まります)。
    fn window_to_open(&self, cx: &mut Context<Self>) {
        // 開き始める場所は**いま見ているタブの隣**(無ければ前回の姿のフォルダ)
        let dir = self
            .showing()
            .path(cx)
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .or_else(|| face::session::load().folder);
        let ask = cx.background_executor().spawn(async move {
            let mut d = rfd::FileDialog::new()
                .add_filter(ui::t!("files_can_open"), &["adoc", "docx", "xlsx", "xltx"])
                .add_filter(ui::t!("officework_documents_spreadsheets"), &["adoc"])
                .add_filter("Word (.docx)", &["docx"])
                .add_filter("Excel (.xlsx)", &["xlsx"]);
            if let Some(d0) = dir.filter(|p| p.is_dir()) {
                d = d.set_directory(d0);
            }
            d.pick_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = r {
                    this.open_in_tab(p, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// **自動復旧の控えを取る見張り**(2026-08-21)。
    ///
    /// 開いている全部のタブを見て、頃合いのものだけ控えます。原本は
    /// 上書きしません — 落ちたときに失う分を減らすための別の控えです。
    ///
    /// 間隔は各タブが持ちます(`控えの間隔` のボタン)。見に行く間隔は
    /// いちばん短いタブに合わせます — 短く設定したタブを待たせないためです。
    fn backup_watch(view: gpui::Entity<Office>, cx: &mut App) {
        // **前に落ちた跡があれば、開いたときに言います。**
        //
        // 控えは普通のファイル(.adoc)なので「開く」でそのまま開けます。
        // 黙っていると、取ってあることに気づかれません。
        //
        // *表の控え(.xlsx)はここでは言いません* — 表の画面が起きるときに
        // 自分で言い、しかも「保護タブの隣の『復旧』で開けます」と場所まで
        // 案内します。こちらが後から上書きすると、その案内が消えます。
        let rest = ops::stale_recovers("adoc").len();
        if rest > 0 {
            view.update(cx, |o: &mut Office, cx| {
                let sentence = ui::tf!(
                    "there_auto_recovery_copies",
                    rest
                )
                .to_string();
                match &o.tabs[o.at] {
                    Pane::Doc(v) => v.update(cx, |w, _| w.say(sentence)),
                    Pane::Sheet(v) => v.update(cx, |c, _| c.say(sentence)),
                }
            });
        }
        cx.spawn(async move |cx| {
            loop {
                let poll = view.update(cx, |o: &mut Office, cx| {
                    o.tabs
                        .iter()
                        .map(|t| match t {
                            Pane::Doc(v) => v.read(cx).recover_poll(),
                            Pane::Sheet(v) => v.read(cx).recover_poll(),
                        })
                        .min()
                        .unwrap_or(30)
                });
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(poll))
                    .await;
                view.update(cx, |o: &mut Office, cx| {
                    // **借りを先に終える。** タブを更新する間は o を借りられない
                    let sentence: Vec<_> = o
                        .tabs
                        .iter()
                        .filter_map(|t| match t {
                            Pane::Doc(v) if v.read(cx).recover_due() => Some(v.clone()),
                            _ => None,
                        })
                        .collect();
                    let table: Vec<_> = o
                        .tabs
                        .iter()
                        .filter_map(|t| match t {
                            Pane::Sheet(v) if v.read(cx).recover_due() => Some(v.clone()),
                            _ => None,
                        })
                        .collect();
                    for v in sentence {
                        v.update(cx, |w, cx| w.take_recover(cx));
                    }
                    for v in table {
                        v.update(cx, |c, cx| c.take_recover(cx));
                    }
                });
            }
        })
        .detach();
    }

    /// **いまの姿を控える。** タブを開く・閉じる・切り替えるたびに呼びます —
    /// 終了のときだけ書くと、落ちたときに前回の姿が残りません。
    fn save_snapshot(&self, cx: &App) {
        let files: Vec<std::path::PathBuf> =
            self.tabs.iter().filter_map(|t| t.path(cx)).collect();
        // フォルダは、いま見ているファイルの親(無ければ最初のタブの親)
        let folder = self
            .showing()
            .path(cx)
            .or_else(|| files.first().cloned())
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        face::session::save(&face::session::of(folder.as_deref(), &files, self.at));
    }

    /// いま見ているタブ。
    fn showing(&self) -> &Pane {
        &self.tabs[self.at.min(self.tabs.len() - 1)]
    }

    /// **タブとして開く。** すでに開いていればそのタブへ移るだけです —
    /// 同じファイルを二重に開くと、どちらを保存したのか分からなくなります。
    /// **焦点は描くときに移します。** ここは受け口からも呼ばれ、そちらには
    /// `Window` がありません。窓を持っている `render` に1回だけ任せます
    fn open_in_tab(&mut self, p: std::path::PathBuf, cx: &mut Context<Self>) {
        // **いま見ている段を控えてから移ります**(段6)。移った先で同じ段を
        // 開き直すので、ホームを見たまま文書と表を行き来できます
        let tab = self.showing().tab_pos(cx);
        if let Some(i) = self.tabs.iter().position(|t| t.path(cx).as_deref() == Some(p.as_path())) {
            self.at = i;
        } else {
            let pane = if is_table(&p) {
                Pane::Sheet(table_to_make(Some(p), cx))
            } else {
                Pane::Doc(doc_to_make(Some(p), cx))
            };
            self.watch(&pane, cx);
            self.tabs.push(pane);
            self.at = self.tabs.len() - 1;
        }
        if let Some(positions) = tab {
            self.showing().align_tabs(positions, cx);
        }
        self.move_focus = true;
        self.save_snapshot(cx);
        cx.notify();
    }
}

impl Office {
    /// **受け口を開く**(2026-08-19)。名前は `officework` の1つです。
    ///
    /// 来た要求は*いま見せている画面*へ渡します。表を見ているなら表の動詞、
    /// 文章を見ているなら文章の動詞が効きます。どちらを見ているかは
    /// `{"cmd":"ping"}` の答えの `showing` で分かります。
    ///
    /// **Windows では作りません**(2026-08-20 発注者。ops::listen の注記)。
    #[cfg(unix)]
    fn inlet(view: gpui::Entity<Office>, cx: &mut App) {
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
                view.update(cx, |this, cx| {
                    for req in reqs {
                        let resp = this.handle_it(&req.line, cx);
                        let _ = req.reply.send(resp);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// 1要求を、いま見せている画面へ渡す。
    #[cfg(unix)]
    fn handle_it(&mut self, line: &str, cx: &mut Context<Self>) -> String {
        // `ping` だけはここで答える(どちらを見ているかを言うため)
        if line.contains("\"ping\"") {
            let showing = match self.showing() {
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
        if let Some(p) = file_to_open(line) {
            self.open_in_tab(std::path::PathBuf::from(&p), cx);
            return "{\"ok\":true}".into();
        }
        // **宛先の指定があれば、そのタブへ渡します**(段10)。
        // *見ているタブは動かしません* — 道具が裏で触っただけなのに画面が
        // 飛ぶと、人の作業を邪魔します。開いていなければ**開いてから**渡します
        let peer = match dest(line) {
            None => self.at,
            Some(p) => {
                let p = std::path::PathBuf::from(&p);
                match self.tabs.iter().position(|t| t.path(cx).as_deref() == Some(p.as_path())) {
                    Some(i) => i,
                    None => {
                        if !p.is_file() {
                            return ops::err("そのファイルは見つかりません");
                        }
                        let from = self.at;
                        self.open_in_tab(p, cx);
                        let i = self.at;
                        // 開くだけ。**見ているタブは戻します**
                        self.at = from;
                        self.move_focus = false;
                        i
                    }
                }
            }
        };
        match &self.tabs[peer] {
            Pane::Doc(v) => v.update(cx, |w, _| writer::rpc::handle(w, line)),
            Pane::Sheet(v) => v.update(cx, |c, _| ops::handle(c, line)),
        }
    }

    /// **書きかけのあるタブの名前。** 閉じるときに、どれが残っているかを言います。
    /// 数だけ言われても、どれを保存すればよいのか分かりません。
    fn draft_name(&self, cx: &App) -> Vec<String> {
        self.tabs.iter().filter(|t| t.draft(cx)).map(|t| t.name(cx)).collect()
    }

    /// **タブを閉じる。** 書きかけがあるときは閉じません(黙って捨てない)。
    /// 最後の1枚は閉じません — 何も出ていない窓は、使う人には壊れて見えます。
    fn close_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 || i >= self.tabs.len() {
            self.told(ui::t!("last_one_stays_open"), cx);
            return;
        }
        if self.tabs[i].draft(cx) {
            let name = self.tabs[i].name(cx);
            self.told(&ui::tf!("unsaved_changes_save_first", name), cx);
            return;
        }
        self.tabs.remove(i);
        if self.at >= self.tabs.len() {
            self.at = self.tabs.len() - 1;
        } else if self.at > i {
            self.at -= 1;
        }
        self.move_focus = true;
        self.save_snapshot(cx);
        cx.notify();
    }

    /// **フォルダを選んでもらう**(覚えている物が無い初回)。
    ///
    /// 窓が出てから別の糸で聞きます。`rfd` は同期なので主の糸では呼べません
    /// (呼ぶと、答えが返るまで画面が描かれません)。やめたときは何もしません —
    /// *空の文書のまま*で、ファイルの「開く」からいつでも始められます。
    fn ask_folder(&self, cx: &mut Context<Self>) {
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .set_title(ui::t!("officework_choose_folder_open"))
                .pick_folder()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(d) = r {
                    if let Pane::Doc(v) = this.showing() {
                        v.update(cx, |w, _| w.show_folder(d));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// いま見ているタブの状態行に出す。
    fn told(&self, msg: &str, cx: &mut Context<Self>) {
        match self.showing() {
            Pane::Doc(v) => v.update(cx, |w, _| w.say(msg.to_string())),
            Pane::Sheet(v) => v.update(cx, |c, _| c.say(msg.to_string())),
        }
    }
}

impl Render for Office {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // **開く頼みは描く前に見ます。** 見落とすと、押しても何も
        // 起きないように見えます
        self.open_request(cx);
        // タブを替えた回だけ焦点を移します(受け口から替わった分もここで拾う)
        if self.move_focus {
            self.move_focus = false;
            window.focus(&self.showing().focus(cx), cx);
        }
        // ---- タブの行(段2)----
        //
        // **文書も表も同じ並びに出ます。** 前は writer の中にタブがあり、
        // 文書しか並びませんでした。持ち主が officework になったので、
        // 種類を問わず1本に並びます。
        //
        // 1枚しか開いていないときは出しません — 何も選べない行は邪魔です
        // (writer が前からそうしている作法に揃えます)。
        let dk = self.showing().is_dark(cx);
        let ground = if dk { gpui::rgb(0x1B1E21) } else { gpui::rgb(0xF1F3F5) };
        let stroke = if dk { gpui::rgb(0x33383D) } else { gpui::rgb(0xE1E6EA) };
        let text = if dk { gpui::rgb(0xCFD6DC) } else { gpui::rgb(0x444B52) };
        let dim_text = if dk { gpui::rgb(0x9AA5AE) } else { gpui::rgb(0x66707A) };
        let sel = if dk { gpui::rgb(0x22262A) } else { gpui::rgb(0xFFFFFF) };
        let tabs = (self.tabs.len() > 1).then(|| {
            let mut row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .px_2()
                .py_0p5()
                .bg(ground)
                .border_b_1()
                .border_color(stroke);
            // **文字の大きさの設定を、ここでも読む**(2026-08-20 発注者)。
            // 表の画面だけ大きくなって、その上のタブの行が元のままだと
            // 揃いません
            let times = ui::ui_scale();
            for i in 0..self.tabs.len() {
                let on = i == self.at;
                let mut label_text = self.tabs[i].name(cx);
                if self.tabs[i].draft(cx) {
                    // **書きかけの印。** 閉じる前に気づけるように
                    label_text.push('*');
                }
                row = row.child(
                    div()
                        .id(gpui::SharedString::from(format!("tab{i}")))
                        .px_2p5()
                        .py_0p5()
                        .rounded_sm()
                        .cursor_pointer()
                        .bg(if on { sel } else { gpui::transparent_black().into() })
                        .border_1()
                        .border_color(if on { stroke } else { gpui::transparent_black().into() })
                        .text_size(px(times * 11.5))
                        .text_color(if on { text } else { dim_text })
                        .child(gpui::SharedString::from(label_text))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let tab = this.showing().tab_pos(cx);
                            this.at = i;
                            if let Some(positions) = tab {
                                this.showing().align_tabs(positions, cx);
                            }
                            this.move_focus = true;
                            this.save_snapshot(cx);
                            cx.notify()
                        })),
                );
                row = row.child(
                    div()
                        .id(gpui::SharedString::from(format!("tabx{i}")))
                        .px_1()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_size(px(times * 11.0))
                        .text_color(dim_text)
                        .child("×")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.close_tab(i, cx);
                            cx.notify()
                        })),
                );
            }
            row
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .children(tabs)
            // **ファイルのページはページごと差し替えます**(段8。2026-09-04)。
            // 編集画面は埋め込みのとき自分では組まないので、ここで組まないと
            // 何も出ません
            .child(if self.showing().on_file_page(cx) {
                self.file_page(cx)
            } else {
                div().flex_1().min_h(px(0.0)).child(match self.showing() {
                    Pane::Doc(v) => v.clone().into_any_element(),
                    Pane::Sheet(v) => v.clone().into_any_element(),
                }).into_any_element()
            })
    }
}

/// **すでに動いている本体があれば、そちらで開いて終わる**(段11)。
///
/// 開けたら真。ファイルの管理画面から2枚目を開いたときに、窓がもう1つ
/// 立つのではなく**タブが1枚増える**のが、統合した後の正しい姿です。
///
/// 引数が無いときは何もしません — 名前を渡されずに呼ばれたのは
/// 「新しく始めたい」ということなので、動いている窓を前に出しても
/// 頼んだことになりません。
///
/// **Windows では受け口を作らない決め**なので、ここも通りません。
/// その OS では今までどおり窓が増えます。
#[cfg(unix)]
fn hand_to_running(arg: Option<&std::path::Path>) -> bool {
    let Some(p) = arg else { return false };
    let Ok(p) = p.canonicalize() else { return false };
    let request =
        format!("{{\"cmd\":\"open\",\"path\":\"{}\"}}", ops::jesc(&p.to_string_lossy()));
    ops::ask("officework", &request).is_some_and(|reply| reply.contains("\"ok\":true"))
}

#[cfg(not(unix))]
fn hand_to_running(_arg: Option<&std::path::Path>) -> bool {
    false
}

fn main() {
    let arg = std::env::args().nth(1).map(std::path::PathBuf::from);
    // **2つ目は窓を増やさず、動いている方のタブにします**(段11)
    if hand_to_running(arg.as_deref()) {
        return;
    }
    // **窓を開ける前に決めます。** 「フォルダを選ぶ画面」は同期で開くので、
    // GPUI が動き出す前に済ませます(主の糸を塞ぐ相手がまだいません)
    let start = launch_shape(arg);
    application().with_assets(ui::Icons).run(move |cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![std::borrow::Cow::Borrowed(ops::font_data())])
            .expect("フォント登録");
        // **いまの言語をエンジンへ渡す**(2026-08-26)。標準の書体と
        // 大きさは言語で変わるので、これを忘れると日本語の既定で出ます
        ui::init_language();
        ui::settings::ai_env_from_settings();
        // **割り当ては文脈で分けて、両方を入れます**(統合の段5)。
        //
        // 前は writer の表だけを `jo_edit` の1文脈に入れていました。そのため
        // **表の画面で calc 専用の 31 鍵が1つも効かず**(Alt+Enter のセル内改行、
        // Alt+S / Alt+C のスライサー、Ctrl+1 など)、しかも意味の食い違う鍵が
        // 2つありました — `Ctrl+E`(表=フラッシュフィル / 文章=中央揃え)と
        // `Ctrl+R`(表=右へコピー / 文章=右揃え)。
        //
        // **1文脈に混ぜると、この2つはどちらかが必ず負けます。** 文脈を分ければ
        // 同じ鍵が画面ごとに別の意味を持てます。編集画面は自分の文脈だけを
        // 名乗るので、いま見ているタブの側の割り当てが効きます
        cx.bind_keys(ui::bindings_for("writer", "jo_doc"));
        cx.bind_keys(ui::bindings_for("calc", "jo_sheet"));
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
                // **使う Python は人が選べます**(2026-09-04 発注者「自由に
                // 環境が選択できるのがいい」)。同梱はやめたので、設定に
                // 書いてあればそれを、無ければいつもの探し方になります
                pyrun::set_python(
                    ui::settings::get("python").filter(|s| !s.trim().is_empty()).map(Into::into),
                );
                let view = cx.new(|cx| Office::new(start2.clone(), cx));
                // **見張りは実体ができてから掛けます**(統合の段8。2026-09-04)。
                // `cx.new` の中で掛けると、まだ自分が居ないので届きません
                view.update(cx, |this, cx| this.watch_all(cx));
                // 焦点は中の編集画面へ渡します
                view.update(cx, |this, cx| {
                    window.focus(&this.showing().focus(cx), cx)
                });
                // Ctrl+= / Ctrl+-(画面の文字の大きさ)。文章の画面は窓の全体に
                // 結ぶ作りなので(`writer::run()` と同じ)、ここでも結びます。
                // 相手は**いま見ているタブ**です。表のタブは自分で受けます
                {
                    let v = view.clone();
                    writer::bind_ui_scale_keys_with(cx, move |cx| match v.read(cx).showing() {
                        Pane::Doc(w) => Some(w.clone()),
                        Pane::Sheet(_) => None,
                    });
                }
                // **受け口を開く。** 名前は officework の1つ。
                // Windows ではソケットを作らない(決め — SEKKEI「受け口は
                // writer にも」の節)
                #[cfg(unix)]
                Office::inlet(view.clone(), cx);
                // **自動復旧の控えを取る見張り**(2026-08-21)。
                //
                // *この仕組みは配っているアプリで死んでいました。* 見張りは
                // `calc::run()` と `writer::run()` の中にあり、単体を起こした
                // ときしか動きません。統合してから officework が主になった
                // のに、こちらへは移していませんでした — 実機で確かめたら
                // 表も文章も控えが1つも取れていませんでした。
                //
                // **全部のタブを見ます。** 裏のタブの書きかけも守ります。
                Office::backup_watch(view.clone(), cx);
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
                        let rest: Vec<String> = this.draft_name(cx);
                        if rest.is_empty() {
                            return true;
                        }
                        this.told(
                            &ui::tf!("there_unsaved_changes_save_them",
                                     rest.join(" / ")),
                            cx,
                        );
                        cx.notify();
                        false
                    })
                });
                // 覚えているフォルダが無ければ、**窓が出てから**選んでもらう
                if matches!(start2, Start::AskFolder) {
                    view.update(cx, |this, cx| this.ask_folder(cx));
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

#[cfg(all(test, unix))]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// **`path` が宛先になる命令とならない命令**(2026-08-20 に壊れていた)。
    ///
    /// `save` の `path` は書き出す先です。宛先と読むと「その名前のタブを
    /// 探す」になり、まだ無いので断られます — 名前を付けて保存が道具から
    /// 使えなくなります。
    #[test]
    fn the_save_path_is_not_read_as_a_target() {
        for cmd in ["open", "save", "to_pdf"] {
            let line = format!("{{\"cmd\":\"{cmd}\",\"path\":\"/tmp/新しい名前.docx\"}}");
            assert_eq!(dest(&line), None, "{cmd} の path を宛先と読んでいる");
        }
    }

    /// 操作の命令に付いた `path` は、いままでどおり宛先です(段10)。
    #[test]
    fn an_operation_commands_path_is_its_target() {
        let line = "{\"cmd\":\"get\",\"path\":\"/tmp/台帳.sheet.adoc\",\"a1\":\"A1\"}";
        assert_eq!(dest(line), Some("/tmp/台帳.sheet.adoc".to_string()));
        // 付いていなければ、いま見ているタブが相手
        assert_eq!(dest("{\"cmd\":\"get\",\"a1\":\"A1\"}"), None);
    }

    /// `open` の道は「開く相手」として別の口が拾います。
    /// **フォルダを渡したらフォルダとして開く**(2026-08-24)。
    /// 前は何を渡してもファイル扱いで、フォルダを渡すと読めずに白紙が
    /// 出ていました。綴りはフォルダなので、ここが入口です
    #[test]
    fn a_folder_argument_opens_as_a_folder() {
        let d = std::env::temp_dir().join(format!("jo-start-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("報告書.adoc");
        std::fs::write(&f, "= 題\n").unwrap();

        assert!(matches!(launch_shape(Some(d.clone())), Start::Folder(_)), "フォルダをファイル扱いした");
        assert!(matches!(launch_shape(Some(f)), Start::File(_)), "ファイルをフォルダ扱いした");

        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_open_commands_path_is_what_to_open() {
        let line = "{\"cmd\":\"open\",\"path\":\"/tmp/台帳.sheet.adoc\"}";
        assert_eq!(file_to_open(line).as_deref(), Some("/tmp/台帳.sheet.adoc"));
        assert_eq!(file_to_open("{\"cmd\":\"get\",\"a1\":\"A1\"}"), None);
    }

    /// **名前で行き先の画面が決まる**(中身は見ません)。
    #[test]
    fn the_name_decides_sheet_or_document() {
        assert!(is_table(std::path::Path::new("/tmp/台帳.sheet.adoc")));
        assert!(is_table(std::path::Path::new("/tmp/台帳.xlsx")));
        assert!(!is_table(std::path::Path::new("/tmp/報告書.adoc")));
        assert!(!is_table(std::path::Path::new("/tmp/報告書.docx")));
    }
}
