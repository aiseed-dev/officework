//! 入力の結線 — kumihan の編集モデル(Editor)と GPUI の入力系をつなぐ。
//!
//! writer も calc もここを共有する。**編集できることがこのソフトの存在理由**なので、
//! 入力の道は1本にして、両方のアプリで同じ挙動にする。
//!
//! IME(日本語入力)の要点:
//!   GPUI は UTF-16 の位置で来る。Editor はバイト位置で持つ。境界で必ず変換する。
//!   変換中(marked)は本文に見せるが、確定するまで undo の1手にしない。
//!   この規則は Editor 側に実装済みで、ここはその呼び分けをするだけ。

/// 日本語まわりの中身は `kotoba` にある(gpui を知らない層)。
/// ここから再輸出して、アプリ側の呼び出しは変えない。
pub use lang::ja::{furigana, proof};
pub use lang::{check, spell, Language, Target};
pub use lang::model::Endpoint;
pub use lang::ai;
pub use lang::i18n::{language, language_label, languages, tr, trf};

/// 画面の文言(そのままの文)。ja の文が鍵 — ja では何も変わらない
#[macro_export]
macro_rules! t {
    ($s:literal) => {
        $crate::tr($s)
    };
}

/// 画面の文言(穴埋めつき)。対応する書式は {} / {:.0} / {:?}
#[macro_export]
macro_rules! tf {
    ($s:literal $(, $a:expr)* $(,)?) => {
        $crate::trf($s, &[$(&$a as &dyn ::std::fmt::Display),*])
    };
}

/// 一覧の項 — **鍵と見出しの組** `(&'static str, &'static str)`。鍵は訳さない。
///
/// 一覧の項は「見せる字」と「それが何か」を兼ねていた。見せる字を訳すと
/// 照合が壊れる — だから組にして、**照合は鍵、画面は見出し**と分ける。
/// 日本語のリテラルはここで**1度だけ**書く(鍵と見出しがずれる余地を作らない)。
///
/// ```text
/// self.pick = Some((vec![ui::item!(…), ui::item!(…)], at));
/// ```
///
/// 例に**本物の呼び出しを書かない**のは、鍵の走査がソースを字句で読むから —
/// 書いたとたん、その文が「訳の要る鍵」に数えられる(ここで1敗した)。
/// **`text` と印を付ける**のも同じ理由の続きで、字下げだけの塊は
/// doc-test として組み立てられ、`…` が読めずに落ちる(2026-08-10)。
///
/// 鍵の取り出し(ui/gen_i18n.py と lang/tests/i18n_soroi.rs)は
/// `ui::t!`・`ui::tf!` と並べてこの形も見る。**片方だけ直すと門番が
/// 「使われていない訳」と言い出す** — 走査を足すときは必ず両方。
#[macro_export]
macro_rules! item {
    ($s:literal) => {
        ($s, $crate::tr($s))
    };
}

pub mod icons;
pub mod pyedit;
pub mod ribbon;
// gen_lang:begin(この間は ui/gen_lang.py が生成する — 手で書かない)
pub mod ribbon_de;
pub mod ribbon_en;
pub mod ribbon_es;
pub mod ribbon_fr;
pub mod ribbon_id;
pub mod ribbon_it;
pub mod ribbon_ko;
pub mod ribbon_pt;
pub mod ribbon_pt_br;
pub mod ribbon_ru;
pub mod ribbon_tr;
pub mod ribbon_vi;
pub mod ribbon_zh;
pub mod ribbon_zh_tw;
// gen_lang:end
pub mod ribbon_tables;
pub mod settings;
pub mod winstate;

/// 窓の縁のつかみ(8箇所)。**GNOME の Wayland はサーバー側の飾り(外枠)を
/// 付けない**(SSD 非対応)ので、縁を自前で掴めるようにしないと窓の大きさを
/// 変えられない(発注者 2026-08-06)。枠のある環境(Server 装飾)では空。
/// 使い方: 根の要素の**最後**に `.children(ui::resize_edges(window))` —
/// 後に描く = 先にマウスを受ける、で格子や本文より縁が勝つ
pub fn resize_edges(window: &gpui::Window) -> Vec<gpui::Div> {
    use gpui::{
        div, px, CursorStyle, Decorations, InteractiveElement, MouseButton, ResizeEdge,
        Styled,
    };
    let Decorations::Client { tiling } = window.window_decorations() else {
        return Vec::new();
    };
    const G: f32 = 6.0; // 縁のつかみの太さ
    const C: f32 = 14.0; // 角のつかみの大きさ
    let grab = |edge: ResizeEdge, cur: CursorStyle| {
        div().absolute().cursor(cur).on_mouse_down(
            MouseButton::Left,
            move |_, window, cx| {
                window.start_window_resize(edge);
                cx.stop_propagation();
            },
        )
    };
    let mut v = Vec::new();
    if !tiling.left {
        v.push(grab(ResizeEdge::Left, CursorStyle::ResizeLeftRight)
            .left(px(0.0)).top(px(C)).bottom(px(C)).w(px(G)));
    }
    if !tiling.right {
        v.push(grab(ResizeEdge::Right, CursorStyle::ResizeLeftRight)
            .right(px(0.0)).top(px(C)).bottom(px(C)).w(px(G)));
    }
    if !tiling.top {
        v.push(grab(ResizeEdge::Top, CursorStyle::ResizeUpDown)
            .top(px(0.0)).left(px(C)).right(px(C)).h(px(G)));
    }
    if !tiling.bottom {
        v.push(grab(ResizeEdge::Bottom, CursorStyle::ResizeUpDown)
            .bottom(px(0.0)).left(px(C)).right(px(C)).h(px(G)));
    }
    if !tiling.top && !tiling.left {
        v.push(grab(ResizeEdge::TopLeft, CursorStyle::ResizeUpRightDownLeft)
            .left(px(0.0)).top(px(0.0)).w(px(C)).h(px(C)));
    }
    if !tiling.top && !tiling.right {
        v.push(grab(ResizeEdge::TopRight, CursorStyle::ResizeUpLeftDownRight)
            .right(px(0.0)).top(px(0.0)).w(px(C)).h(px(C)));
    }
    if !tiling.bottom && !tiling.left {
        v.push(grab(ResizeEdge::BottomLeft, CursorStyle::ResizeUpLeftDownRight)
            .left(px(0.0)).bottom(px(0.0)).w(px(C)).h(px(C)));
    }
    if !tiling.bottom && !tiling.right {
        v.push(grab(ResizeEdge::BottomRight, CursorStyle::ResizeUpRightDownLeft)
            .right(px(0.0)).bottom(px(0.0)).w(px(C)).h(px(C)));
    }
    v
}

use std::ops::Range;

use gpui::{actions, AssetSource, KeyBinding, SharedString};

/// リボンのアイコンを gpui に渡す(`svg().path("icons/bold.svg")` で引ける)。
/// フォントと違い、アイコンは**こちらの成果物の一部**なので埋め込んでよい
/// (Euro-Office 由来・AGPL。NOTICE.md に明記)。
pub struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        Ok(path
            .strip_prefix("icons/")
            .and_then(|n| n.strip_suffix(".svg"))
            .and_then(icons::find)
            .map(std::borrow::Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(icons::ICONS.iter().map(|(k, _)| SharedString::from(format!("icons/{k}.svg"))).collect())
    }
}
use kumihan::Editor;

/// SVG を高精細の PNG に直す(matplotlib の `savefig("図.svg")` を貼るため)。
/// 返り値: (PNG のバイト列, 論理の幅px, 高さpx)。幅高さは SVG の寸法
/// (96dpi 相当)で、PNG 自体は scale 倍で描く — 拡大しても粗くならない。
/// 紙に貼るものなので下地は白(透過を PDF の RGB 化で黒く潰さない)。
pub fn svg_to_png(data: &[u8], scale: f32) -> Result<(Vec<u8>, u32, u32), String> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_data(data, &usvg::Options::default())
        .map_err(|e| format!("SVG が読めません: {e}"))?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    let (pw, ph) = ((w * scale).ceil() as u32, (h * scale).ceil() as u32);
    if pw == 0 || ph == 0 || pw > 20000 || ph > 20000 {
        return Err("SVG の大きさが扱えません".into());
    }
    let mut pixmap =
        tiny_skia::Pixmap::new(pw, ph).ok_or_else(|| "画素が確保できません".to_string())?;
    pixmap.fill(tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let png = pixmap.encode_png().map_err(|e| e.to_string())?;
    Ok((png, w.round() as u32, h.round() as u32))
}

/// 外の世界へ開いた結果。呼び手はこれで状態行の文言を分ける —
/// 「黙って何も起きない」を作らないため
pub enum Opened {
    /// 渡した(窓なりブラウザなりが来る)
    Yes,
    /// さっき同じ相手を開けたばかり — 渡していない(窓は来ている途中か、もうある)
    JustNow,
    /// 渡せなかった(xdg-open が無い等)
    Failed,
}

/// 同じ相手への連打を1回にまとめる判定(純粋な部分 — 試験はここを見る)。
/// 直近に同じ target を開けていたら false
fn open_gate(
    last: &mut Option<(String, std::time::Instant)>,
    target: &str,
    now: std::time::Instant,
    within: std::time::Duration,
) -> bool {
    if let Some((t, at)) = last.as_ref() {
        if t == target && now.duration_since(*at) < within {
            return false;
        }
    }
    *last = Some((target.to_string(), now));
    true
}

/// フォルダや URL を外のソフトで開く(xdg-open)。**calc と writer が共に使う。**
///
/// 素の `Command::spawn` を4箇所に散らしていたら、実機でファイルマネージャの
/// 窓が8枚積もった(2026-08-12)。機構: GNOME Files は呼ばれるたびに
/// **新しい窓**を開く+開くまで一拍ある+押した手応えが無い → 連打。
/// 窓を数える手は無いので、**同じ相手は5秒に1回**だけ渡す。
/// 子は看取る(spawn しっ放しだと zombie が積もる)
pub fn open_outside(target: &str) -> Opened {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static LAST: Mutex<Option<(String, Instant)>> = Mutex::new(None);
    {
        let mut last = LAST.lock().unwrap();
        if !open_gate(&mut last, target, Instant::now(), Duration::from_secs(5)) {
            return Opened::JustNow;
        }
    }
    match std::process::Command::new("xdg-open").arg(target).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Opened::Yes
        }
        Err(_) => Opened::Failed,
    }
}

/// いまの日時「YYYY-MM-DD HH:MM」(地方時)。**外部の date を呼ばない** —
/// 呼ぶと Windows で動かないし、スレッドを塞ぐ(引き継ぎの残件でもある)。
/// **calc と writer が共に使う** — 暦の算法を2箇所に持たない。
/// 暦は civil-from-days の素直な算法(1970-01-01 起点)
pub fn now_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // 地方時のずれ(TZ の秒)。取れなければ UTC のまま出す
    let off = std::env::var("TZ_OFFSET_SECS").ok().and_then(|v| v.parse::<i64>().ok());
    let secs = secs + off.unwrap_or_else(local_offset_secs);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rem / 3600, (rem % 3600) / 60)
}

/// 地方時のずれ(秒)。/etc/localtime を読む気は無いので、
/// date が居れば1回だけ聞き、居なければ 0(UTC)— 表示だけの用途
fn local_offset_secs() -> i64 {
    std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let sign = if s.starts_with('-') { -1 } else { 1 };
            let h: i64 = s.get(1..3)?.parse().ok()?;
            let mi: i64 = s.get(3..5)?.parse().ok()?;
            Some(sign * (h * 3600 + mi * 60))
        })
        .unwrap_or(0)
}

/// 1970-01-01 からの日数 → (年, 月, 日)。Howard Hinnant の civil_from_days
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

actions!(
    jo_edit,
    [
        Backspace, Delete, Left, Right, SelectLeft, SelectRight, SelectAll,
        SelectUp, SelectDown,
        Home, End, Enter, Undo, Redo, Save, Open, Up, Down, Tab, ShiftTab,
        Copy, Cut, Paste, PasteValues, Quit, ContextMenu, Cancel,
        WordLeft, WordRight, SelectWordLeft, SelectWordRight, PageUp, PageDown,
        // 表の「データの端へ」(Ctrl+↑↓)。左右は WordLeft/WordRight が兼ねる。
        // **受け口を持つのは calc だけ** — 束縛があっても writer では動かない
        // (docs/sekkei/sugata.ja.md「キーの嘘 — 束縛と受け口は別」)
        EdgeUp, EdgeDown, SelectEdgeUp, SelectEdgeDown,
        Find, DocHome, DocEnd, EditCell, Recalc, RecalcSheet, NewLine,
        UiBigger, UiSmaller, InsLink,
        // 文字飾りの割り当て(本家 Ctrl+B/I/U/5)。**calc と writer の
        // 両方に受け口がある** — 片方だけだと「キーの嘘」になる
        Bold, Italic, Underline, Strikeout,
        /// 昔ながらの配列数式を入れる(Ctrl+Shift+Enter)。**calc だけ**が
        /// 受け口を持つ — writer には表の配列という考えが無い
        ArrayEnter,
        /// 毎日使う鍵。**受け口は両方のアプリに置く**(片方だけだと嘘になる)
        InsertFn, PercentFmt, Print, FullScreen, SaveAs, FlashFill,
        ZoomReset, Help, InsDate, InsTime,
        /// シートの行き来と参照の $ 回し。**calc だけ**が受け口を持つ —
        /// writer にシートも A1 参照も無い
        PrevSheet, NextSheet, CycleRef,
        /// スライサーの板が開いている間だけ意味がある2つ。**calc だけ**
        SlicerMulti, SlicerClear,
    ]
);

/// 標準の割り当て。アプリの起動時に一度呼ぶ。
pub fn bindings(context: &'static str) -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("backspace", Backspace, Some(context)),
        KeyBinding::new("delete", Delete, Some(context)),
        KeyBinding::new("left", Left, Some(context)),
        KeyBinding::new("right", Right, Some(context)),
        KeyBinding::new("shift-left", SelectLeft, Some(context)),
        KeyBinding::new("shift-right", SelectRight, Some(context)),
        KeyBinding::new("ctrl-left", WordLeft, Some(context)),
        KeyBinding::new("ctrl-right", WordRight, Some(context)),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, Some(context)),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, Some(context)),
        KeyBinding::new("ctrl-up", EdgeUp, Some(context)),
        KeyBinding::new("ctrl-down", EdgeDown, Some(context)),
        KeyBinding::new("ctrl-shift-up", SelectEdgeUp, Some(context)),
        KeyBinding::new("ctrl-shift-down", SelectEdgeDown, Some(context)),
        KeyBinding::new("pageup", PageUp, Some(context)),
        KeyBinding::new("pagedown", PageDown, Some(context)),
        KeyBinding::new("f2", EditCell, Some(context)),
        KeyBinding::new("ctrl-f", Find, Some(context)),
        // Ctrl+H(本家の「検索と置換」)も同じ口へ。**ここのパネルは
        // 探す言葉 → 置き換える言葉 の2段で、空なら検索だけ** — 本家の
        // 2つのタブを1つで兼ねているので、鍵も同じ所に着く
        KeyBinding::new("ctrl-h", Find, Some(context)),
        KeyBinding::new("ctrl-b", Bold, Some(context)),
        KeyBinding::new("ctrl-i", Italic, Some(context)),
        KeyBinding::new("ctrl-u", Underline, Some(context)),
        KeyBinding::new("ctrl-5", Strikeout, Some(context)),
        // 関数の挿入(calc だけが受ける — writer に関数の一覧は無い)
        KeyBinding::new("shift-f3", InsertFn, Some(context)),
        // パーセント書式(同上)
        KeyBinding::new("ctrl-shift-%", PercentFmt, Some(context)),
        // 印刷(こちらは PDF に出す)・全画面・名前を付けて保存
        KeyBinding::new("ctrl-p", Print, Some(context)),
        KeyBinding::new("f11", FullScreen, Some(context)),
        KeyBinding::new("ctrl-shift-s", SaveAs, Some(context)),
        // フラッシュフィル(calc だけ — writer に列という考えが無い)
        KeyBinding::new("ctrl-e", FlashFill, Some(context)),
        // 本家と同じ鍵。**受け口の無い割り当ては作らない**(キーの嘘)
        KeyBinding::new("ctrl-0", ZoomReset, Some(context)),
        KeyBinding::new("f1", Help, Some(context)),
        KeyBinding::new("ctrl-;", InsDate, Some(context)),
        KeyBinding::new("ctrl-:", InsTime, Some(context)),
        // ここから下は calc だけ(writer には受け口が無い)
        KeyBinding::new("alt-pageup", PrevSheet, Some(context)),
        KeyBinding::new("alt-pagedown", NextSheet, Some(context)),
        KeyBinding::new("f4", CycleRef, Some(context)),
        KeyBinding::new("alt-s", SlicerMulti, Some(context)),
        KeyBinding::new("alt-c", SlicerClear, Some(context)),
        KeyBinding::new("ctrl-home", DocHome, Some(context)),
        KeyBinding::new("ctrl-end", DocEnd, Some(context)),
        KeyBinding::new("shift-up", SelectUp, Some(context)),
        KeyBinding::new("shift-down", SelectDown, Some(context)),
        KeyBinding::new("ctrl-a", SelectAll, Some(context)),
        KeyBinding::new("home", Home, Some(context)),
        KeyBinding::new("end", End, Some(context)),
        KeyBinding::new("enter", Enter, Some(context)),
        // 昔ながらの配列数式(CSE)。選んだ範囲に同じ式を配列として入れる
        KeyBinding::new("ctrl-shift-enter", ArrayEnter, Some(context)),
        KeyBinding::new("up", Up, Some(context)),
        KeyBinding::new("down", Down, Some(context)),
        KeyBinding::new("tab", Tab, Some(context)),
        KeyBinding::new("shift-tab", ShiftTab, Some(context)),
        KeyBinding::new("ctrl-z", Undo, Some(context)),
        KeyBinding::new("ctrl-shift-z", Redo, Some(context)),
        KeyBinding::new("ctrl-y", Redo, Some(context)),
        KeyBinding::new("ctrl-s", Save, Some(context)),
        KeyBinding::new("ctrl-o", Open, Some(context)),
        KeyBinding::new("ctrl-c", Copy, Some(context)),
        KeyBinding::new("ctrl-x", Cut, Some(context)),
        KeyBinding::new("ctrl-v", Paste, Some(context)),
        // 値だけの貼り付け(新しい Excel と同じ割り当て)
        KeyBinding::new("ctrl-shift-v", PasteValues, Some(context)),
        KeyBinding::new("ctrl-q", Quit, Some(context)),
        // メニューキー(アプリケーションキー)と Shift+F10 は右クリックと同じ
        KeyBinding::new("menu", ContextMenu, Some(context)),
        KeyBinding::new("shift-f10", ContextMenu, Some(context)),
        // 再計算(Excel と同じ。計算方法が手動のときの手回し)
        KeyBinding::new("f9", Recalc, Some(context)),
        KeyBinding::new("shift-f9", RecalcSheet, Some(context)),
        // セルの中の改行(Excel と同じ)
        KeyBinding::new("alt-enter", NewLine, Some(context)),
        // 画面の文字の大きさ(リボン・メニュー・状態行まで全部)
        KeyBinding::new("ctrl-=", UiBigger, Some(context)),
        KeyBinding::new("ctrl-shift-=", UiBigger, Some(context)),
        KeyBinding::new("ctrl--", UiSmaller, Some(context)),
        // ハイパーリンク(Excel と同じ)
        KeyBinding::new("ctrl-k", InsLink, Some(context)),
        KeyBinding::new("escape", Cancel, Some(context)),
    ]
}

/// GPUI の EntityInputHandler が求める操作を、Editor の言葉に翻訳する。
///
/// アプリ側は「編集対象の Editor をくれ」とだけ実装すればよく、
/// UTF-16 との変換や marked の扱いはここで閉じる。
pub trait HasEditor {
    fn editor(&mut self) -> &mut Editor;
    fn editor_ref(&self) -> &Editor;
    /// 本文が変わったときに呼ばれる(組版のやり直し・再計算など)
    fn on_edited(&mut self) {}
    /// **変える直前**に呼ばれる。取り消しの控えを取る場所。
    /// `typing` が真なら打鍵の一手(続けて打った分はまとめてよい)。
    /// 既定は何もしない — 控えを持たないアプリはそのまま
    fn before_edit(&mut self, _typing: bool) {}
    /// 数学オートコレクト(`\alpha` → α)を掛けるか。
    /// **既定は切** — 打鍵の途中で勝手に置き換わる物を、黙って入れない
    fn math_autocorrect(&self) -> bool {
        false
    }
    /// オートコレクトが働いたときに呼ばれる(状態行に出す用)。
    /// `was` は元の綴り
    fn on_autocorrect(&mut self, _was: &str) {}
}

/// EntityInputHandler の中身。アプリの impl から丸ごと委譲する。
pub mod handler {
    use super::*;

    pub fn text_for_range<T: HasEditor>(
        this: &mut T,
        range_utf16: Range<usize>,
        actual: &mut Option<Range<usize>>,
    ) -> Option<String> {
        let e = this.editor_ref();
        let r = e.byte_range(range_utf16);
        actual.replace(e.utf16_range(r.clone()));
        e.text().get(r).map(|s| s.to_string())
    }

    pub fn selected_range_utf16<T: HasEditor>(this: &T) -> Range<usize> {
        let e = this.editor_ref();
        e.utf16_range(e.selection())
    }

    pub fn marked_range_utf16<T: HasEditor>(this: &T) -> Option<Range<usize>> {
        let e = this.editor_ref();
        e.marked_range().map(|r| e.utf16_range(r))
    }

    pub fn unmark<T: HasEditor>(this: &mut T) {
        this.editor().clear_marked();
        this.on_edited();
    }

    /// 確定した文字が来た(通常の入力・IMEの確定・貼り付け)
    pub fn replace<T: HasEditor>(this: &mut T, range_utf16: Option<Range<usize>>, text: &str) {
        this.before_edit(true);
        // **区切りを打った時**に数学オートコレクトを掛ける(`\alpha ` → `α `)。
        // 打っている途中に替えると、`\pi` を打とうとして `\p` で止まった人が
        // 困る。**入れる前に**掛けるので、記号と区切りで1手になり、
        // Backspace 1回で綴りに戻る
        let auto = this.math_autocorrect() && is_delim(text);
        let mut was = None;
        {
            let e = this.editor();
            if let Some(r) = range_utf16 {
                let b = e.byte_range(r);
                e.move_to(b.start, false);
                e.move_to(b.end, true);
            }
            // 変換中なら確定、そうでなければ普通の挿入。
            // どちらも undo の1手になる(Editor 側の規則)
            if e.marked_range().is_some() {
                e.commit_marked(text);
            } else if !auto || !e.autocorrect_math(text) {
                e.insert(text);
            } else {
                was = e.just_autocorrected().map(|s| s.to_string());
            }
        }
        if let Some(was) = was {
            this.on_autocorrect(&was);
        }
        this.on_edited();
    }

    /// オートコレクトの引き金になる打鍵か(綴りの終わりを告げる文字)。
    /// 英字と `\` は綴りの続きなので引き金にしない
    fn is_delim(text: &str) -> bool {
        let mut it = text.chars();
        match (it.next(), it.next()) {
            (Some(c), None) => !c.is_ascii_alphabetic() && c != '\\',
            _ => false, // 2文字以上(貼り付け・IME の確定)は引き金にしない
        }
    }

    /// 変換中の文字が来た(未確定)
    pub fn replace_and_mark<T: HasEditor>(
        this: &mut T,
        range_utf16: Option<Range<usize>>,
        text: &str,
        sel_utf16: Option<Range<usize>>,
    ) {
        {
            let e = this.editor();
            if let Some(r) = range_utf16 {
                let b = e.byte_range(r);
                e.move_to(b.start, false);
                e.move_to(b.end, true);
            }
            // 未確定の中での選択(変換対象の文節)はバイト位置に直す
            let sel = sel_utf16.map(|r| {
                let bytes = |u: usize| {
                    text.char_indices()
                        .scan(0usize, |acc, (b, c)| {
                            let cur = *acc;
                            *acc += c.len_utf16();
                            Some((cur, b))
                        })
                        .find(|(u16pos, _)| *u16pos >= u)
                        .map(|(_, b)| b)
                        .unwrap_or(text.len())
                };
                bytes(r.start)..bytes(r.end)
            });
            e.set_marked(text, sel);
        }
        this.on_edited();
    }

    pub fn text_len_utf16<T: HasEditor>(this: &T) -> usize {
        this.editor_ref().utf16_len()
    }
}

#[cfg(test)]
mod svg_tests {
    #[test]
    fn svgを高精細のpngに直せる() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#165E83"/></svg>"##;
        let (png, w, h) = super::svg_to_png(svg, 3.0).expect("直せない");
        assert_eq!((w, h), (40, 20), "論理の寸法が違う");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']), "PNG になっていない");
        // 3倍の画素で描かれている(頭の IHDR の幅)
        let pw = u32::from_be_bytes(png[16..20].try_into().unwrap());
        assert_eq!(pw, 120, "高精細になっていない: {pw}px");
    }

    #[test]
    fn 壊れたsvgは断る() {
        assert!(super::svg_to_png(b"not svg", 3.0).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct App {
        ed: Editor,
        edits: usize,
    }
    impl HasEditor for App {
        fn editor(&mut self) -> &mut Editor { &mut self.ed }
        fn editor_ref(&self) -> &Editor { &self.ed }
        fn on_edited(&mut self) { self.edits += 1 }
    }

    fn app(s: &str) -> App {
        App { ed: Editor::new(s), edits: 0 }
    }

    #[test]
    fn 同じ相手への連打は1回にまとまる() {
        use std::time::{Duration, Instant};
        let within = Duration::from_secs(5);
        let t0 = Instant::now();
        let mut last = None;
        // 1回目は通り、直後の連打(同じ相手)は止まる
        assert!(open_gate(&mut last, "/tmp/a", t0, within));
        assert!(!open_gate(&mut last, "/tmp/a", t0 + Duration::from_millis(300), within));
        assert!(!open_gate(&mut last, "/tmp/a", t0 + Duration::from_secs(4), within));
        // 別の相手はすぐ通る(URL とフォルダを続けて開くのは正当)
        assert!(open_gate(&mut last, "https://example.com", t0 + Duration::from_secs(1), within));
        // 時間が経てば同じ相手ももう一度通る(窓を閉じてしまった後の開き直し)
        assert!(open_gate(&mut last, "https://example.com", t0 + Duration::from_secs(7), within));
    }

    #[test]
    fn 通常の入力が本文に入る() {
        let mut a = app("");
        handler::replace(&mut a, None, "日本フネン");
        assert_eq!(a.ed.text(), "日本フネン");
        assert_eq!(a.edits, 1, "組版のやり直しが呼ばれる");
    }

    #[test]
    fn ime_の一巡が通る() {
        let mut a = app("特定");
        // 「ぼうか」を打つ(未確定)
        handler::replace_and_mark(&mut a, None, "ぼうか", None);
        assert_eq!(a.ed.text(), "特定ぼうか");
        assert!(handler::marked_range_utf16(&a).is_some());
        // 変換して「防火」(まだ未確定)
        handler::replace_and_mark(&mut a, None, "防火", None);
        assert_eq!(a.ed.text(), "特定防火");
        // 確定
        handler::replace(&mut a, None, "防火");
        assert_eq!(a.ed.text(), "特定防火");
        assert!(handler::marked_range_utf16(&a).is_none());
        // undo は1手で変換前に戻る
        assert!(a.ed.undo());
        assert_eq!(a.ed.text(), "特定");
    }

    #[test]
    fn utf16の範囲指定で置き換わる() {
        let mut a = app("あいうえお");
        // UTF-16 で 1..3 =「いう」
        handler::replace(&mut a, Some(1..3), "XY");
        assert_eq!(a.ed.text(), "あXYえお");
    }

    #[test]
    fn 選択範囲がutf16で返る() {
        let mut a = app("あa亜");
        a.ed.select_all();
        // あ=1, a=1, 亜=1 → 3単位
        assert_eq!(handler::selected_range_utf16(&a), 0..3);
        assert_eq!(handler::text_len_utf16(&a), 3);
    }

    #[test]
    fn 変換の取り消しで跡が残らない() {
        let mut a = app("設備");
        handler::replace_and_mark(&mut a, None, "りよう", None);
        handler::unmark(&mut a);
        assert_eq!(a.ed.text(), "設備");
    }

    #[test]
    fn 文節の選択がバイト位置に直る() {
        let mut a = app("");
        // 「日本フネン」のうち UTF-16 で 0..2 =「日本」が変換対象
        handler::replace_and_mark(&mut a, None, "日本フネン", Some(0..2));
        assert_eq!(a.ed.selection(), 0.."日本".len(), "UTF-16→バイトの変換が違う");
    }
}
