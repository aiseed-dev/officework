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

pub mod combo;
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

/// `.py` を編集する道具で開く。**プログラムの編集は表計算の仕事ではない**
/// (発注者 2026-08-15。データとプログラムを分けた以上、calc の中に
/// 編集面を持つのは筋が通らない)。順は:
///
/// 1. settings.toml の `editor`(利用者が決めた道具。zed でも何でも)
/// 2. 隣にいる officework の writer(素の文字として開ける)
/// 3. 機械の既定(xdg-open — .py に何が結ばれていても、それが答え)
///
/// 返りは開いた道具の名前(状態行に出すため)
pub fn open_for_edit(path: &str) -> Result<String, String> {
    // (1) 利用者の決めが最優先
    if let Some(ed) = settings::get("editor").filter(|s| !s.trim().is_empty()) {
        return match std::process::Command::new(&ed).arg(path).spawn() {
            Ok(mut c) => {
                std::thread::spawn(move || {
                    let _ = c.wait();
                });
                Ok(ed)
            }
            Err(e) => Err(format!("{ed}: {e}")),
        };
    }
    // (2) 隣の writer(配り物は同じ場所に居る)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let w = dir.join(if cfg!(windows) { "writer.exe" } else { "writer" });
            if w.exists() {
                return match std::process::Command::new(&w).arg(path).spawn() {
                    Ok(mut c) => {
                        std::thread::spawn(move || {
                            let _ = c.wait();
                        });
                        Ok("writer".into())
                    }
                    Err(e) => Err(format!("writer: {e}")),
                };
            }
        }
    }
    // (3) 機械の既定
    match open_outside(path) {
        Opened::Yes | Opened::JustNow => Ok("機械の既定の道具".into()),
        Opened::Failed => Err("開ける道具がありません".into()),
    }
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
        /// 定番の増強(2026-08-14 発注者「割り当てが足りない」)。
        /// **calc だけ**が受け口を持つ
        CellFormat, SelectCol, SelectRow, AutoSum, FillDown, FillRight,
        Jump, ToggleFilter, MakeTable, AddComment,
        /// 同じ増強の **writer だけ**の側(揃え・改ページ・文字の大小)
        AlignLeft, AlignCenter, AlignRight, AlignJustify, PageBreak,
        FontBigger, FontSmaller,
    ]
);

/// 標準の割り当て。アプリの起動時に一度呼ぶ。
/// **Alt のキーヒントの札**(2026-08-13、台帳「Alt キーヒント」)。
///
/// `n` 個の物に、打ち分けられる札を1つずつ配る。**札は画面に重ねて出す** —
/// 本家は言葉から頭文字を取るが、こちらのリボンは日本語なので頭文字が
/// 取れない。だから**順番に配って、配った札をその場に見せる**。
/// 覚える物ではなく、読む物にする。
///
/// 並びは打ちやすい順(ホームポジション → 上段 → 下段 → 数字)。
/// 36 を超えたら**全部2文字**にする — 1文字と2文字を混ぜると、
/// `A` を打った時に「A で決まり」か「AS の途中」か決められない
pub fn key_hints(n: usize) -> Vec<String> {
    const POOL: &[u8] = b"ASDFGHJKLQWERTYUIOPZXCVBNM1234567890";
    let one = |i: usize| (POOL[i] as char).to_string();
    if n <= POOL.len() {
        return (0..n).map(one).collect();
    }
    // 2文字。頭の字ごとに POOL 個ぶら下がる
    (0..n)
        .map(|i| format!("{}{}", one(i / POOL.len()), one(i % POOL.len())))
        .collect()
}

/// 既定の割り当ての表(鍵, 操作名)。**この表が正本** — 束縛は
/// [`bindings_for`] がここから作り、settings.toml の `key.操作名 = "鍵"` が
/// 上書きし、tools/keys_check.py が手引きの表との揃いを見る。
///
/// 同じ操作に行が2つある物(ctrl-f と ctrl-h の Find など)はどちらも効く。
/// **受け口の無いアプリに束縛を作らない** — 前は1本の表を両アプリに配り、
/// 「束縛はあるが writer では動かない」鍵があった(sugata の部屋
/// 「キーの嘘」)。表を 共通/calc/writer に割って、その状態を無くした
/// (2026-08-14)
pub const KEYS_COMMON: &[(&str, &str)] = &[
    ("backspace", "Backspace"),
    ("delete", "Delete"),
    ("left", "Left"),
    ("right", "Right"),
    ("shift-left", "SelectLeft"),
    ("shift-right", "SelectRight"),
    ("ctrl-left", "WordLeft"),
    ("ctrl-right", "WordRight"),
    ("ctrl-shift-left", "SelectWordLeft"),
    ("ctrl-shift-right", "SelectWordRight"),
    ("pageup", "PageUp"),
    ("pagedown", "PageDown"),
    ("ctrl-f", "Find"),
    // Ctrl+H(本家の「検索と置換」)も同じ口へ — ここのパネルは
    // 探す言葉 → 置き換える言葉 の2段で、空なら検索だけ
    ("ctrl-h", "Find"),
    ("ctrl-b", "Bold"),
    ("ctrl-i", "Italic"),
    ("ctrl-u", "Underline"),
    ("ctrl-5", "Strikeout"),
    ("ctrl-p", "Print"),
    ("f11", "FullScreen"),
    ("ctrl-shift-s", "SaveAs"),
    // F12 も名前を付けて保存(本家と同じ。2026-08-14 に追加)
    ("f12", "SaveAs"),
    ("ctrl-0", "ZoomReset"),
    ("f1", "Help"),
    ("ctrl-;", "InsDate"),
    ("ctrl-:", "InsTime"),
    ("ctrl-home", "DocHome"),
    ("ctrl-end", "DocEnd"),
    ("shift-up", "SelectUp"),
    ("shift-down", "SelectDown"),
    ("ctrl-a", "SelectAll"),
    ("home", "Home"),
    ("end", "End"),
    ("enter", "Enter"),
    ("up", "Up"),
    ("down", "Down"),
    ("tab", "Tab"),
    ("shift-tab", "ShiftTab"),
    ("ctrl-z", "Undo"),
    ("ctrl-shift-z", "Redo"),
    ("ctrl-y", "Redo"),
    ("ctrl-s", "Save"),
    ("ctrl-o", "Open"),
    ("ctrl-c", "Copy"),
    ("ctrl-x", "Cut"),
    ("ctrl-v", "Paste"),
    ("ctrl-q", "Quit"),
    ("menu", "ContextMenu"),
    ("shift-f10", "ContextMenu"),
    ("ctrl-=", "UiBigger"),
    ("ctrl-shift-=", "UiBigger"),
    ("ctrl--", "UiSmaller"),
    ("ctrl-k", "InsLink"),
    ("escape", "Cancel"),
];

/// calc だけの割り当て(受け口が calc にしか無い物)
pub const KEYS_CALC: &[(&str, &str)] = &[
    ("ctrl-up", "EdgeUp"),
    ("ctrl-down", "EdgeDown"),
    ("ctrl-shift-up", "SelectEdgeUp"),
    ("ctrl-shift-down", "SelectEdgeDown"),
    ("f2", "EditCell"),
    ("shift-f3", "InsertFn"),
    ("ctrl-shift-%", "PercentFmt"),
    ("ctrl-e", "FlashFill"),
    ("ctrl-shift-v", "PasteValues"),
    ("ctrl-shift-enter", "ArrayEnter"),
    ("f9", "Recalc"),
    ("shift-f9", "RecalcSheet"),
    ("alt-enter", "NewLine"),
    ("alt-pageup", "PrevSheet"),
    ("alt-pagedown", "NextSheet"),
    // 本家の鍵(2026-08-14 に追加)。Alt 版も当面残す — 衝突しない
    ("ctrl-pageup", "PrevSheet"),
    ("ctrl-pagedown", "NextSheet"),
    ("f4", "CycleRef"),
    ("alt-s", "SlicerMulti"),
    ("alt-c", "SlicerClear"),
    // ここから 2026-08-14 の増強(本家の定番)
    ("ctrl-1", "CellFormat"),
    ("ctrl-space", "SelectCol"),
    ("shift-space", "SelectRow"),
    ("alt-=", "AutoSum"),
    ("ctrl-d", "FillDown"),
    ("ctrl-r", "FillRight"),
    ("ctrl-g", "Jump"),
    ("f5", "Jump"),
    ("ctrl-shift-l", "ToggleFilter"),
    ("ctrl-t", "MakeTable"),
    ("shift-f2", "AddComment"),
];

/// writer だけの割り当て。ctrl-e は calc ではフラッシュフィル、
/// writer では中央揃え — **本家の手の記憶がアプリごとに違う**ので、
/// 同じ鍵でも表を分けて別の操作に割り当てる
pub const KEYS_WRITER: &[(&str, &str)] = &[
    ("ctrl-e", "AlignCenter"),
    ("ctrl-l", "AlignLeft"),
    ("ctrl-r", "AlignRight"),
    ("ctrl-j", "AlignJustify"),
    ("ctrl-enter", "PageBreak"),
    ("ctrl-]", "FontBigger"),
    ("ctrl-[", "FontSmaller"),
];

/// 操作名 → 束縛を1本作る。知らない名前は None(呼ぶ側が言う)
fn make_binding(key: &str, name: &str, context: &'static str) -> Option<KeyBinding> {
    macro_rules! table {
        ($($n:ident),+ $(,)?) => {
            $(if name.eq_ignore_ascii_case(stringify!($n)) {
                return Some(KeyBinding::new(key, $n, Some(context)));
            })+
        };
    }
    table!(
        Backspace, Delete, Left, Right, SelectLeft, SelectRight, SelectAll,
        SelectUp, SelectDown, Home, End, Enter, Undo, Redo, Save, Open, Up,
        Down, Tab, ShiftTab, Copy, Cut, Paste, PasteValues, Quit, ContextMenu,
        Cancel, WordLeft, WordRight, SelectWordLeft, SelectWordRight, PageUp,
        PageDown, EdgeUp, EdgeDown, SelectEdgeUp, SelectEdgeDown, Find,
        DocHome, DocEnd, EditCell, Recalc, RecalcSheet, NewLine, UiBigger,
        UiSmaller, InsLink, Bold, Italic, Underline, Strikeout, ArrayEnter,
        InsertFn, PercentFmt, Print, FullScreen, SaveAs, FlashFill, ZoomReset,
        Help, InsDate, InsTime, PrevSheet, NextSheet, CycleRef, SlicerMulti,
        SlicerClear, CellFormat, SelectCol, SelectRow, AutoSum, FillDown,
        FillRight, Jump, ToggleFilter, MakeTable, AddComment, AlignLeft,
        AlignCenter, AlignRight, AlignJustify, PageBreak, FontBigger,
        FontSmaller,
    );
    None
}

static KEY_WARNINGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// 起動時の鍵の合成で見つけた言い分(知らない操作名・読めない鍵・
/// 取り合い)。アプリが状態行に出す。**黙って捨てない**ための出口
pub fn key_warnings() -> &'static [String] {
    KEY_WARNINGS.get().map(Vec::as_slice).unwrap_or(&[])
}

/// アプリの既定の表(共通+アプリ固有)。マニュアル生成の道具も
/// この並びを読む
pub fn default_keys(app: &str) -> Vec<(&'static str, &'static str)> {
    let own: &[(&str, &str)] = match app {
        "calc" => KEYS_CALC,
        "writer" => KEYS_WRITER,
        _ => &[],
    };
    KEYS_COMMON.iter().chain(own).copied().collect()
}

/// 合成で見つけた言い分。**翻訳は掛けない**(bindings_for が最後に
/// 掛ける)— 芯を言語から切り離し、試験が文言に依らないようにする
#[derive(Debug, PartialEq)]
pub enum KeyWarn {
    /// 知らない操作名
    UnknownAction(String),
    /// 読めない鍵(操作名, 鍵)
    BadKey(String, String),
    /// 同じ鍵の取り合い(鍵, 先の操作, 後の操作 — 後が勝つ)
    Contested(String, String, String),
}

/// 既定の表と上書きの**合成の芯**(純関数 — 試験がここを直に叩く)。
///
/// 決め(2026-08-14): 名前の照合は大文字小文字を見ない。1つの操作に
/// 複数の鍵は「,」区切り。上書きは**その操作の既定の鍵を全部置き換える**。
/// 空文字なら外す。知らない名前・読めない鍵は**その行だけ捨てて、
/// 言い分に残す**。取り合い(同じ鍵に別の操作)は後の者が勝ち、それも言う
pub fn compose_keys(
    defaults: &[(&str, &str)],
    overrides: &[(String, String)],
    known: &dyn Fn(&str) -> bool,
) -> (Vec<(String, String)>, Vec<KeyWarn>) {
    let mut rows: Vec<(String, String)> = defaults
        .iter()
        .map(|(k, n)| (k.to_string(), n.to_string()))
        .collect();
    let mut warns: Vec<KeyWarn> = Vec::new();
    for (name, keys) in overrides {
        if !known(name) {
            warns.push(KeyWarn::UnknownAction(name.clone()));
            continue;
        }
        let mut good: Vec<String> = Vec::new();
        for key in keys.split(',').map(str::trim).filter(|k| !k.is_empty()) {
            let ok = key
                .split_whitespace()
                .all(|part| gpui::Keystroke::parse(part).is_ok());
            if ok {
                good.push(key.to_string());
            } else {
                warns.push(KeyWarn::BadKey(name.clone(), key.to_string()));
            }
        }
        // 空文字は「外す」の意思。読める鍵が1つも無い書き損じなら
        // **既定を残す** — 書き間違い1つで鍵が全部消えるのは酷
        if good.is_empty() && !keys.trim().is_empty() {
            continue;
        }
        rows.retain(|(_, n)| !n.eq_ignore_ascii_case(name));
        rows.extend(good.into_iter().map(|k| (k, name.clone())));
    }
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if rows[i].0 == rows[j].0 && !rows[i].1.eq_ignore_ascii_case(&rows[j].1) {
                warns.push(KeyWarn::Contested(
                    rows[i].0.clone(), rows[i].1.clone(), rows[j].1.clone(),
                ));
            }
        }
    }
    (rows, warns)
}

/// 標準の割り当て+settings.toml の `key.操作名 = "鍵"` の上書き。
/// アプリの起動時に一度呼ぶ。読めなかった行の言い分は [`key_warnings`]
pub fn bindings_for(app: &str, context: &'static str) -> Vec<KeyBinding> {
    let overrides = settings::get_prefixed("key.");
    let (rows, warns) = compose_keys(
        &default_keys(app),
        &overrides,
        // 名前が操作として実在するか(束縛を1本試作して確かめる —
        // 操作の一覧を二重に持たないため)
        &|n| make_binding("ctrl-a", n, context).is_some(),
    );
    let out = rows
        .iter()
        .filter_map(|(k, n)| make_binding(k, n, context))
        .collect();
    // 言い分はここで初めて言葉になる(compose_keys は言語を知らない)
    let said = warns
        .iter()
        .map(|w| match w {
            KeyWarn::UnknownAction(n) => {
                crate::tf!("設定の key.{} は知らない操作名です", n).to_string()
            }
            KeyWarn::BadKey(n, k) => {
                crate::tf!("設定の key.{} の鍵が読めません: {}", n, k).to_string()
            }
            KeyWarn::Contested(k, a, b) => crate::tf!(
                "鍵 {} は {} と {} の取り合いです({} が勝ちます)", k, a, b, b
            )
            .to_string(),
        })
        .collect();
    let _ = KEY_WARNINGS.set(said);
    out
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

#[cfg(test)]
mod key_tests {
    use super::compose_keys;

    fn known(n: &str) -> bool {
        ["Bold", "Italic", "Find"].iter().any(|k| k.eq_ignore_ascii_case(n))
    }
    fn over(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn 上書きは既定の鍵を置き換え_他の操作は無傷() {
        let defaults = [("ctrl-b", "Bold"), ("ctrl-f", "Find"), ("ctrl-h", "Find")];
        let (rows, warns) = compose_keys(&defaults, &over(&[("bold", "alt-b")]), &known);
        assert!(rows.contains(&("alt-b".into(), "bold".into())), "{rows:?}");
        assert!(!rows.iter().any(|(k, _)| k == "ctrl-b"));
        assert_eq!(rows.iter().filter(|(_, n)| n == "Find").count(), 2);
        assert!(warns.is_empty(), "{warns:?}");
    }

    #[test]
    fn 知らない操作名は言い分になり_既定は動かない() {
        let defaults = [("ctrl-b", "Bold")];
        let (rows, warns) = compose_keys(&defaults, &over(&[("nosuch", "ctrl-x")]), &known);
        assert_eq!(warns, vec![super::KeyWarn::UnknownAction("nosuch".into())]);
        assert!(rows.contains(&("ctrl-b".into(), "Bold".into())));
    }

    #[test]
    fn 読めない鍵だけの上書きは既定を残して言う() {
        let defaults = [("ctrl-b", "Bold")];
        let (rows, warns) =
            compose_keys(&defaults, &over(&[("bold", "nosuchmod-b")]), &known);
        assert_eq!(
            warns,
            vec![super::KeyWarn::BadKey("bold".into(), "nosuchmod-b".into())]
        );
        // 書き損じで鍵が消えたら酷 — 既定の ctrl-b は生きている
        assert!(rows.contains(&("ctrl-b".into(), "Bold".into())), "{rows:?}");
    }

    #[test]
    fn 空文字の上書きは鍵を外す() {
        let defaults = [("ctrl-b", "Bold"), ("ctrl-i", "Italic")];
        let (rows, warns) = compose_keys(&defaults, &over(&[("bold", "")]), &known);
        assert!(warns.is_empty(), "{warns:?}");
        assert!(!rows.iter().any(|(_, n)| n.eq_ignore_ascii_case("bold")));
        assert!(rows.iter().any(|(_, n)| n == "Italic"));
    }

    #[test]
    fn 同じ鍵の取り合いは言い分になり_後の者が勝つ側に居る() {
        let defaults = [("ctrl-b", "Bold"), ("ctrl-i", "Italic")];
        let (rows, warns) = compose_keys(&defaults, &over(&[("find", "ctrl-b")]), &known);
        assert_eq!(
            warns,
            vec![super::KeyWarn::Contested("ctrl-b".into(), "Bold".into(), "find".into())]
        );
        // 後の者(find)の行が表の後ろに居る — GPUI は後から結んだ方を優先
        let bold_at = rows.iter().position(|(k, n)| k == "ctrl-b" && n == "Bold").unwrap();
        let find_at = rows.iter().position(|(k, n)| k == "ctrl-b" && n == "find").unwrap();
        assert!(find_at > bold_at);
    }

    #[test]
    fn 複数の鍵はコンマで並べられる() {
        let defaults = [("ctrl-b", "Bold")];
        let (rows, warns) = compose_keys(&defaults, &over(&[("bold", "alt-b, ctrl-shift-b")]), &known);
        assert!(warns.is_empty(), "{warns:?}");
        assert!(rows.contains(&("alt-b".into(), "bold".into())));
        assert!(rows.contains(&("ctrl-shift-b".into(), "bold".into())));
    }
}
