//! **エージェントのパネルの描き**(agent.ja.adoc の段10。2026-09-04)。
//!
//! 発注者「Word の方にもいれて。こちらの差がもっと大きくなる」(2026-09-04)。
//! 表計算にあるパネルを文章の画面にも付けます。**同じ物を2度書かない**ため、
//! 描く所をここに1つ置きます。
//!
//! 分け方は [`crate::filemenu::sidebar`] と同じです。**何が並ぶかは画面が
//! 決め、どう描くかはここが持ちます。** 画面の側に残るのは、やりとりの中身
//! (`Chat`)と、押されたときにすることだけです。
//!
//! 外側の柱(面を切り替えるアイコン)は画面に残します。表計算は会話と
//! コメントの2つ、文章の画面はフォルダの一覧も持つので、並ぶ物が違います。

use gpui::prelude::*;
use gpui::{div, px, Context, SharedString};

/// **やりとりの1行。**
///
/// 道具呼びは飾らない1行です。書き替えには「1手」の印だけ付けます
/// (Ctrl+Z で戻せる印 — 2026-09-02 の決め)。
#[derive(Clone, Debug, PartialEq)]
pub enum Chat {
    /// 人が打った字
    Me(String),
    /// AI が返した字
    Ai(String),
    /// 道具を呼んだ跡。2つめは「1手で戻せる」印
    Tool(String, bool),
}

/// **モデルとの繋がりの状態**(2026-09-02 の決め。4語)。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AgentState {
    /// 宛先が決まっていない
    Unset,
    /// 何もしていない(語を出さない)
    #[default]
    Idle,
    Connecting,
    Connected,
    Failed,
}

impl AgentState {
    /// 状態行に出す語。`Idle` は出しません
    pub fn word(self) -> Option<String> {
        match self {
            AgentState::Unset => Some(crate::t!("model_unset").to_string()),
            AgentState::Idle => None,
            AgentState::Connecting => Some(crate::t!("model_connecting").to_string()),
            AgentState::Connected => Some(crate::t!("model_connected").to_string()),
            AgentState::Failed => Some(crate::t!("model_connect_failed").to_string()),
        }
    }
}

/// **宛先の一覧の1行**(押すとその宛先に替わります)。
#[derive(Clone, Debug, PartialEq)]
pub struct DestRow {
    /// 一覧に出す名前
    pub name: String,
    /// 名前の下に小さく出す字(モデル名や繋ぎ先)
    pub detail: String,
    /// いま使っている宛先か
    pub now: bool,
}

/// パネルに出す物(画面が用意します)。
pub struct View<'a> {
    /// やりとり
    pub log: &'a [Chat],
    /// 入力の欄の字と、その中のカーソルの位置
    pub input: &'a str,
    pub cursor: usize,
    /// 入力の欄に焦点があるか
    pub focus: bool,
    /// いま考えているか(送るボタンを止める)
    pub busy: bool,
    /// 保存の確認を待っているか
    pub asking_save: bool,
    pub state: AgentState,
    /// いまの宛先の名前
    pub dest: Option<String>,
    /// やりとりが空のときに出す例
    pub example: String,
    /// 見出しの下の断り書き(何について聞けるか)
    pub note: String,
    /// **宛先を選んでいる最中なら、その一覧**(2026-09-04 発注者
    /// 「AI model を自由に設定」)。`None` は選んでいない
    pub picking: Option<&'a [DestRow]>,
}

/// パネルの色と大きさ。
pub struct Look {
    pub dark: bool,
    pub fg: gpui::Rgba,
    pub faint: gpui::Rgba,
    pub line: gpui::Rgba,
    pub accent: gpui::Rgba,
    pub scale: f32,
}

/// パネルの幅(px)。**文章の画面と表計算で同じ**です
pub const W: f32 = 250.0;
/// 外側の柱の幅(px)。アイコン1つぶん
pub const RAIL: f32 = 34.0;

/// **押されたときに画面へ返す名前。**
///
/// `run` はありません — することが画面ごとに違う(保存の仕方も、
/// 送る先の組み立ても)ので、画面の側で `match` します。
pub mod id {
    /// 新しい会話
    pub const NEW: &str = "chat-new";
    /// 入力の欄(焦点を取る)
    pub const INPUT: &str = "chat-in";
    /// 送る
    pub const SEND: &str = "chat-send";
    /// 宛先を替える
    pub const WHERE: &str = "chat-where";
    /// 保存してよい
    pub const SAVE_OK: &str = "chat-save-ok";
    /// 保存しない
    pub const SAVE_NO: &str = "chat-save-no";
    /// 宛先の一覧の1行(後ろに番号が付きます: `dest:3`)
    pub const DEST: &str = "dest:";
    /// 宛先を足す・直す(設定の画面へ)
    pub const DEST_EDIT: &str = "dest-edit";
}

/// **会話の面を描く**(外側の柱は含みません)。
///
/// `on` は押された物の名前を受け取ります([`id`] の定数)。
pub fn body<V: gpui::Render>(
    look: &Look,
    view: &View,
    cx: &mut Context<V>,
    on: impl Fn(&mut V, &str, &mut Context<V>) + Clone + 'static,
) -> gpui::Div {
    let (us, dk) = (look.scale, look.dark);
    let (fg, faint, line, accent) = (look.fg, look.faint, look.line, look.accent);
    let hover_bg =
        if dk { gpui::rgb(0x2C333A) } else { gpui::rgb(0xEAF5EE) };
    let button = {
        let on = on.clone();
        move |cx: &mut Context<V>, id: &'static str, t: String, enabled: bool| {
            let on = on.clone();
            div()
                .id(SharedString::from(id))
                .px_2()
                .py_0p5()
                .rounded_sm()
                .cursor_pointer()
                .text_size(px(us * 11.5))
                .text_color(if enabled { fg } else { faint })
                .border_1()
                .border_color(if enabled { accent } else { line })
                .hover(move |s| s.bg(hover_bg))
                .child(t)
                .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                    on(this, id, cx);
                    cx.notify()
                }))
        }
    };
    let row_box = || div().flex().flex_row().flex_wrap().gap_1();

    let mut d = div()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .overflow_hidden()
        .p_2()
        .flex()
        .flex_col()
        .gap_1();

    // 見出しの行。**新しい会話**は頭に置きます
    let new_btn = button(cx, id::NEW, crate::t!("new_conversation").to_string(), false);
    d = d.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .text_size(px(us * 12.5))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(fg)
                    .child(crate::t!("ask_ai").to_string()),
            )
            .child(new_btn),
    );
    if !view.note.is_empty() {
        d = d.child(
            div()
                .text_size(px(us * 10.5))
                .text_color(faint)
                .child(view.note.clone()),
        );
    }

    // やりとり。**残りの高さを全部使います**(固定の高さだと上に空きが溜まる)
    let mut log = div()
        .id("chat-log")
        .flex()
        .flex_col()
        .gap_1()
        .mt_1()
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll();
    if view.log.is_empty() {
        log = log.child(
            div()
                .text_size(px(us * 11.0))
                .text_color(faint)
                .child(view.example.clone()),
        );
    }
    for row in view.log {
        log = log.child(match row {
            Chat::Me(t) => {
                div().text_size(px(us * 11.5)).text_color(accent).child(format!("▸ {t}"))
            }
            Chat::Ai(t) => div().text_size(px(us * 11.5)).text_color(fg).child(format!("◂ {t}")),
            Chat::Tool(t, one_step) => div()
                .text_size(px(us * 10.0))
                .text_color(faint)
                .child(if *one_step {
                    format!("· {t} — {}", crate::t!("one_step"))
                } else {
                    format!("· {t}")
                }),
        });
    }
    d = d.child(log);

    // **保存の確認。** 道具 save は実行せずここへ回ります —
    // 確認を取る3つ(保存・削除・外への送信)の1つ目です
    if view.asking_save {
        let ok = button(cx, id::SAVE_OK, crate::t!("save").to_string(), true);
        let no = button(cx, id::SAVE_NO, crate::t!("cancel").to_string(), false);
        d = d
            .child(
                div()
                    .text_size(px(us * 10.5))
                    .text_color(faint)
                    .mt_2()
                    .mb_0p5()
                    .child(crate::t!("agent_wants_save").to_string()),
            )
            .child(row_box().mt_1().child(ok).child(no));
    }

    // 入力。焦点があるときは打った所に「|」を差します
    let on_in = on.clone();
    d = d.child(
        div()
            .p_1()
            .rounded_sm()
            .bg(if dk { gpui::rgb(0x14171A) } else { gpui::rgb(0xFFFFFF) })
            .border_1()
            .border_color(if view.focus { accent } else { line })
            .text_size(px(us * 11.5))
            .text_color(fg)
            .id(SharedString::from(id::INPUT))
            .cursor_text()
            .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                on_in(this, id::INPUT, cx);
                cx.notify()
            }))
            .child(if view.input.is_empty() {
                if view.focus {
                    "|".to_string()
                } else {
                    crate::t!("click_here_type_enter").to_string()
                }
            } else if view.focus {
                let mut t = view.input.to_string();
                t.insert(view.cursor.min(t.len()), '|');
                t
            } else {
                view.input.to_string()
            }),
    );
    let send = button(cx, id::SEND, crate::t!("send").to_string(), !view.busy);
    let mut r = row_box().mt_1().child(send);
    if view.busy {
        r = r.child(
            div()
                .text_size(px(us * 10.5))
                .text_color(faint)
                .child(crate::t!("thinking").to_string()),
        );
    }
    d = d.child(r);

    // **宛先の一覧**(2026-09-04 発注者「AI model を自由に設定」)。
    //
    // 前は押すたびに次へ回るだけで、5つあるうちの3つ目を選ぶのに3回
    // 押す必要がありました。一覧から選びます。手元で動いているモデルも
    // 画面が入れて渡します(港を叩くのはパネルを開く時だけ)
    if let Some(rows) = view.picking {
        let mut list = div()
            .id("dest-list")
            .mt_1()
            .flex()
            .flex_col()
            .gap_0p5()
            .max_h(px(us * 180.0))
            .overflow_y_scroll();
        for (i, r) in rows.iter().enumerate() {
            let on = on.clone();
            let id = format!("{}{i}", id::DEST);
            list = list.child(
                div()
                    .id(SharedString::from(id.clone()))
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(if r.now { hover_bg } else { gpui::transparent_black().into() })
                    .hover(move |s| s.bg(hover_bg))
                    .child(
                        div()
                            .text_size(px(us * 11.5))
                            .text_color(if r.now { accent } else { fg })
                            .child(SharedString::from(r.name.clone())),
                    )
                    .child(
                        div()
                            .text_size(px(us * 10.0))
                            .text_color(faint)
                            .child(SharedString::from(r.detail.clone())),
                    )
                    .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                        on(this, &id, cx);
                        cx.notify()
                    })),
            );
        }
        let edit = {
            let on = on.clone();
            div()
                .id(SharedString::from(id::DEST_EDIT))
                .mt_1()
                .px_1()
                .py_0p5()
                .rounded_sm()
                .cursor_pointer()
                .text_size(px(us * 10.5))
                .text_color(accent)
                .hover(move |s| s.bg(hover_bg))
                .child(crate::t!("add_or_edit_destination").to_string())
                .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                    on(this, id::DEST_EDIT, cx);
                    cx.notify()
                }))
        };
        d = d.child(list).child(edit);
    }

    // **モデルの状態は4語 + 今の宛先の名前**(2026-09-02 の決め)。
    // 押すと一覧(`[[ai]]`)の次の宛先に替わります — 話しながら切り替えられます
    let line_text = match (view.state.word(), view.dest.clone()) {
        (Some(w), Some(n)) => format!("{w} · {}", crate::tf!("destination_press_change", n)),
        (None, Some(n)) => crate::tf!("destination_press_change", n).to_string(),
        _ => crate::t!("model_unset").to_string(),
    };
    let on_where = on.clone();
    d.child(
        div()
            .id(SharedString::from(id::WHERE))
            .mt_1()
            .px_1()
            .py_0p5()
            .rounded_sm()
            .cursor_pointer()
            .text_size(px(us * 10.5))
            .text_color(faint)
            .hover(move |s| s.bg(hover_bg))
            .child(line_text)
            .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                on_where(this, id::WHERE, cx);
                cx.notify()
            })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_idle_state_stays_quiet() {
        assert_eq!(AgentState::Idle.word(), None);
        for s in [AgentState::Unset, AgentState::Connecting, AgentState::Connected,
                  AgentState::Failed] {
            assert!(s.word().is_some(), "{s:?} の語が無い");
        }
    }

    #[test]
    fn a_tool_line_remembers_whether_it_can_be_undone_in_one_step() {
        let a = Chat::Tool("replace_blocks".into(), true);
        let b = Chat::Tool("outline".into(), false);
        assert_ne!(a, b);
        assert_eq!(Chat::Me("こんにちは".into()), Chat::Me("こんにちは".into()));
    }
}
