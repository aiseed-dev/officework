//! **エージェント**(agent.ja.adoc の段10。2026-09-04)。
//!
//! 発注者「Word の方にもいれて。こちらの差がもっと大きくなる」。
//! 表計算にあるものを文章の画面にも付けます。
//!
//! *表計算との違いは道具と、その実行の道だけ*です。表は `ops` の語彙を
//! 直に呼びますが、文章は **`crate::rpc::handle` の動詞**へ流します
//! (`agent::tools::doc_line_for` が道具の名前と引数を1行の JSON に直します)。
//! 受け口は前からあり、`officework-mcp` の `doc_*` も同じ所を通ります。
//!
//! **読みは黙って、書きは実行してから1手として見せます** — 事前の
//! 「よろしいですか」は出しません(agent.ja.adoc「対話の画面の決め」)。
//! 確認を取るのは保存だけです。

use crate::{AgentState, ChatRow, Writer};
use gpui::Context;
use kumihan::Editor;

/// エージェントに渡す、この画面の説明。
const AGENT_SYSTEM: &str = "\
あなたは文書を作るアプリの中で働く助手です。開いている文書を道具で読み書きします。\
本文は AsciiDoc で、見出し・段落・表・記入欄という意味の単位(ブロック)で並んでいます。\
まず doc_outline でブロックの並びを見て、必要な所だけ doc_read_blocks で読みます。\
書き替えは doc_replace_blocks / doc_insert_blocks / doc_delete_blocks で、\
番号で指します。記入欄は doc_fill_fields に [[名前, 値], …] を渡します。\
文書の全部を読み直さないでください。長い文書ではトークンが尽きます。\
保存は人の確認が要ります。求められたときだけ doc_save を呼んでください。";

impl Writer {
    /// **文章の画面が名乗る道具**: 文書の9つ(受け口へ直結)。
    ///
    /// マクロ(`run_macro`)はまだ足しません — 文書のマクロを
    /// サンドボックスで走らせる道は、表計算の側にしかありません
    fn agent_tools() -> Vec<lang::model::ToolDef> {
        agent::tools::doc_tools()
    }

    /// いまの宛先(名前と繋ぎ先)。表計算と同じ一覧を見ます
    pub(crate) fn agent_dest(&self) -> Option<(String, lang::model::Endpoint)> {
        let rows = face::settings::ai_list();
        if rows.is_empty() {
            if std::env::var("OFFICE_URL").is_ok() || std::env::var("OFFICE_HOST").is_ok() {
                let ep = lang::model::Endpoint::default();
                return Some((ep.host.clone(), ep));
            }
            return None;
        }
        let last = face::settings::ai_last();
        let row = rows
            .iter()
            .find(|r| Some(r.name.as_str()) == last.as_deref())
            .unwrap_or(&rows[0]);
        Some((row.name.clone(), row.endpoint()))
    }

    /// 宛先を一覧の次へ回す。話しながら切り替えられます
    pub(crate) fn agent_cycle_dest(&mut self) {
        let rows = face::settings::ai_list();
        if rows.len() < 2 {
            self.status = match rows.first() {
                None => ui::t!("ai_list_empty_write_settings").into(),
                Some(only) => ui::tf!("ai_only_one_destination", only.name.clone()).into(),
            };
            return;
        }
        let last = face::settings::ai_last();
        let at = rows.iter().position(|r| Some(r.name.as_str()) == last.as_deref()).unwrap_or(0);
        let next = &rows[(at + 1) % rows.len()];
        face::settings::set_ai_last(&next.name);
        self.agent_state = AgentState::Idle;
        self.status = ui::tf!("ai_destination_remembered", next.name.clone()).into();
    }

    /// パネルに1行積む。
    pub(crate) fn chat_push(&mut self, row: ChatRow) {
        self.ai_chat_log.push(row);
    }

    /// **新しい会話にする。** やりとりも履歴も捨てます(文書は触りません)
    pub(crate) fn chat_reset(&mut self) {
        self.ai_chat_log.clear();
        self.agent = None;
        self.agent_shown = 0;
        self.agent_calls.clear();
        self.agent_save = None;
        self.ai_chat_in = Editor::new("");
        self.status = ui::t!("started_new_conversation_sheet").into();
    }

    /// 欄の字を送る。
    pub(crate) fn ai_chat_send(&mut self, cx: &mut Context<Self>) {
        let t = self.ai_chat_in.text().trim().to_string();
        if t.is_empty() {
            self.status = ui::t!("nothing_ask").into();
            return;
        }
        self.ai_chat_in = Editor::new("");
        self.agent_send(t, cx);
    }

    /// 会話を1つ送る(エージェントのループの入り口)。
    pub(crate) fn agent_send(&mut self, purpose: String, cx: &mut Context<Self>) {
        if self.ai_busy {
            self.status = ui::t!("still_thinking_please_wait").into();
            return;
        }
        let Some((name, ep)) = self.agent_dest() else {
            self.agent_state = AgentState::Unset;
            self.status = ui::t!("agent_no_destination").into();
            return;
        };
        self.chat_push(ChatRow::Me(purpose.clone()));
        // **選んでいる所は付け合わせ**(表計算が選んだ範囲を付けるのと同じ)。
        // 画面には用件だけを出します — 付け足しまで見せると読みづらいので
        let sel = self.ed.selection();
        let user = if sel.is_empty() {
            purpose
        } else {
            let t = self.ed.text()[sel].to_string();
            format!("{purpose}\n\n---\nいま選んでいるのは次の所です。\n{t}")
        };
        let agent = self.agent.get_or_insert_with(|| agent::Agent::new(AGENT_SYSTEM));
        let msgs = agent.begin(&user);
        self.agent_shown = agent.log.len();
        self.ai_busy = true;
        self.agent_state = AgentState::Connecting;
        self.status = ui::tf!("asking_ai", name, ui::t!("conversation")).into();
        self.agent_step(ep, msgs, cx);
    }

    /// モデルとの1往復を裏で待ち、道具はメインスレッドで実行して続けます。
    /// 道具呼びが無くなるまで、これと [`Self::agent_run_calls`] が回し合います
    fn agent_step(
        &mut self,
        ep: lang::model::Endpoint,
        msgs: Vec<lang::model::Msg>,
        cx: &mut Context<Self>,
    ) {
        let tools = Self::agent_tools();
        let task = cx
            .background_executor()
            .spawn(async move { lang::model::chat_tools(&ep, &msgs, &tools, 0.2) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Err(e) => {
                        this.ai_busy = false;
                        this.agent_state = AgentState::Failed;
                        this.chat_push(ChatRow::Ai(format!("({e})")));
                        this.status = format!("AI: {e}").into();
                    }
                    Ok(out) => {
                        this.agent_state = AgentState::Connected;
                        if out.tool_calls.is_empty() {
                            let ag = this.agent.as_mut().expect("agent_send が置いた");
                            ag.finish(&out.content);
                            this.agent_drain_log();
                            this.ai_busy = false;
                            this.status = ui::t!("answered_left_panel").into();
                        } else {
                            let ag = this.agent.as_mut().expect("agent_send が置いた");
                            ag.note_calls(&out.tool_calls);
                            this.agent_calls = out.tool_calls;
                            this.agent_run_calls(cx);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 途中の道具呼びを順に実行します。保存だけは人の確認で止まります
    fn agent_run_calls(&mut self, cx: &mut Context<Self>) {
        while let Some(c) = self.agent_calls.first().cloned() {
            {
                let ag = self.agent.as_mut().expect("agent_send が置いた");
                if let Err(e) = ag.count_call() {
                    self.agent_calls.clear();
                    self.ai_busy = false;
                    self.chat_push(ChatRow::Ai(format!("({e})")));
                    self.status = format!("AI: {e}").into();
                    return;
                }
                ag.note_call(&c);
            }
            // 呼びの行を**実行の前に**出します — 黙って動かしません
            self.agent_drain_log();
            if c.name == "doc_save" {
                // 保存は実行せず、確認のボタンを出して人を待ちます
                let args = if c.arguments.trim().is_empty() { "{}" } else { c.arguments.as_str() };
                let path = ops::Jobj::parse(args).and_then(|o| o.str("path"));
                self.agent_save = Some((c, path));
                cx.notify();
                return;
            }
            let r = self.agent_call_tool(&c);
            let ag = self.agent.as_mut().expect("agent_send が置いた");
            ag.tool_result(&c, r);
            self.agent_drain_log();
            self.agent_calls.remove(0);
        }
        // 全部済んだ — 次の往復
        match self.agent_dest() {
            Some((_, ep)) => {
                let msgs = self.agent.as_ref().expect("agent_send が置いた").msgs();
                self.agent_step(ep, msgs, cx);
            }
            None => {
                self.ai_busy = false;
                self.agent_state = AgentState::Unset;
            }
        }
    }

    /// **道具を1つ実行する。** 受け口(`rpc`)の動詞へ流します —
    /// `officework-mcp` の `doc_*` と同じ道で、書き替えは1手として入ります
    pub(crate) fn agent_call_tool(&mut self, c: &lang::model::ToolCall) -> Result<String, String> {
        let args = if c.arguments.trim().is_empty() { "{}" } else { c.arguments.as_str() };
        let o = ops::Jobj::parse(args).ok_or_else(|| "引数が読めません".to_string())?;
        let line = agent::tools::doc_line_for(&c.name, &o)?;
        // **受け口は Windows では作りません**(`mod rpc` ごと `#[cfg(unix)]`)。
        // 動詞を捌く所そのものには Unix の物は要らないので、いずれ
        // ソケットと分けるのが筋です。それまではここで旗を持ちます
        #[cfg(unix)]
        {
            let reply = crate::rpc::handle(self, &line);
            // 受け口は `{"ok":true,…}` か `{"err":"…"}` を返します
            match ops::Jobj::parse(&reply).and_then(|r| r.str("err")) {
                Some(e) => Err(e),
                None => Ok(reply),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = line;
            Err("この OS では道具を使えません(受け口が Unix のソケットの中にあります)".to_string())
        }
    }

    /// 会話の記録を画面の行に写します。
    pub(crate) fn agent_drain_log(&mut self) {
        let Some(ag) = &self.agent else { return };
        let mut rows: Vec<ChatRow> = Vec::new();
        for e in &ag.log[self.agent_shown.min(ag.log.len())..] {
            match e {
                // 人の行は agent_send が積みます
                agent::Event::User(_) => {}
                agent::Event::Assistant(t) => rows.push(ChatRow::Ai(t.clone())),
                agent::Event::ToolCall { name, arguments } => {
                    let place = ops::Jobj::parse(arguments).and_then(|o| {
                        o.str("text").or_else(|| o.str("path")).or_else(|| {
                            o.num("start").map(|n| format!("{}", n as i64))
                        })
                    });
                    let line = match place {
                        Some(a) => format!("{name} {a}"),
                        None => name.clone(),
                    };
                    // **書き替えには「1手」の印**(Ctrl+Z で戻せる印)
                    let writes = matches!(
                        name.as_str(),
                        "doc_replace_blocks"
                            | "doc_insert_blocks"
                            | "doc_delete_blocks"
                            | "doc_fill_fields"
                    );
                    rows.push(ChatRow::Tool(line, writes));
                }
                agent::Event::ToolResult { ok, content, .. } => {
                    // 中身は見せません(字の山になる)。しくじりだけ出します
                    if !ok {
                        rows.push(ChatRow::Tool(content.clone(), false));
                    }
                }
            }
        }
        self.agent_shown = ag.log.len();
        for r in rows {
            self.chat_push(r);
        }
    }

    /// 保存の確認に人が答えた。答えは道具の結果としてモデルにも返り、
    /// ループは続きから回ります
    pub(crate) fn agent_confirm_save(&mut self, yes: bool, cx: &mut Context<Self>) {
        let Some((c, path)) = self.agent_save.take() else { return };
        let r = if !yes {
            self.status = ui::t!("save_declined").into();
            // 断りはしくじりではありません — モデルにはそのまま伝えます
            Ok("保存はしていません(人が断りました)".to_string())
        } else {
            let line = match &path {
                Some(p) => format!("{{\"cmd\":\"save\",\"path\":{}}}", ops::J::S(p.clone()).to_json()),
                None => "{\"cmd\":\"save\"}".to_string(),
            };
            #[cfg(unix)]
            {
                let reply = crate::rpc::handle(self, &line);
                match ops::Jobj::parse(&reply).and_then(|x| x.str("err")) {
                    Some(e) => Err(e),
                    None => Ok(reply),
                }
            }
            #[cfg(not(unix))]
            {
                let _ = line;
                Err("この OS では保存の道具を使えません".to_string())
            }
        };
        let ag = self.agent.as_mut().expect("agent_send が置いた");
        ag.tool_result(&c, r);
        self.agent_drain_log();
        if !self.agent_calls.is_empty() {
            self.agent_calls.remove(0);
        }
        self.agent_run_calls(cx);
    }

    /// パネルの物が押されたとき(描きは [`ui::agentpanel::body`] と共通)。
    pub(crate) fn agent_panel_click(&mut self, id: &'static str, cx: &mut Context<Self>) {
        use ui::agentpanel::id as pid;
        match id {
            pid::NEW => self.chat_reset(),
            pid::INPUT => self.ai_chat_focus = true,
            pid::SEND => self.ai_chat_send(cx),
            pid::WHERE => self.agent_cycle_dest(),
            pid::SAVE_OK => self.agent_confirm_save(true, cx),
            pid::SAVE_NO => self.agent_confirm_save(false, cx),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use lang::model::ToolCall;

    fn call(name: &str, args: &str) -> ToolCall {
        ToolCall { id: "c1".into(), name: name.into(), arguments: args.into() }
    }

    /// **道具は受け口の動詞へ届き、書き替えは1手で戻る**(2026-09-04。
    /// agent.ja.adoc の段10)。モデルは要りません — 道具呼びを直に渡します
    #[gpui::test]
    fn a_document_tool_reaches_the_verb_and_undoes_in_one_step(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.native = false;
            let d = kumihan::adoc::parse("= 報告\n\n== 概況\n\n受注は3件。\n").unwrap();
            this.set_doc(d);

            // 読みの道具。名前は doc_* で、受け口の動詞は outline
            let r = this.agent_call_tool(&call("doc_outline", "{}")).expect("地図が読めない");
            assert!(r.contains("\"count\":"), "地図の形が違う: {r}");

            // 書きの道具。**引数の名前は道具の物**(start / end / adoc)で、
            // 受け口の名前(from / to)へは doc_line_for が直します
            let r = this
                .agent_call_tool(&call(
                    "doc_replace_blocks",
                    r#"{"start":2,"end":2,"adoc":"受注は4件。\n"}"#,
                ))
                .expect("書き替えられない");
            assert!(r.contains("\"replaced\""), "答えの形が違う: {r}");
            assert!(this.doc.body_text().contains("受注は4件"), "本文が変わっていない");

            // **1手で戻ります**(道具で直しても、人が打ったのと同じ扱い)
            this.undo_step();
            assert!(!this.doc.body_text().contains("受注は4件"), "1手で戻らない");
        });
    }

    /// 知らない道具と、壊れた引数は**断ります**(黙って何もしない、をやめる)
    #[gpui::test]
    fn an_unknown_tool_and_broken_arguments_are_refused(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            assert!(this.agent_call_tool(&call("sheet_read_range", "{}")).is_err(), "知らない道具を通した");
            assert!(this.agent_call_tool(&call("doc_read_blocks", "{}")).is_err(), "start が無いのに通した");
            assert!(this.agent_call_tool(&call("doc_find", "{\"text\":")).is_err(), "壊れた引数を通した");
        });
    }

    /// パネルの押しが、それぞれの動きに繋がっている
    #[gpui::test]
    fn the_panel_buttons_are_wired(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.chat_push(ChatRow::Me("こんにちは".into()));
            this.ai_chat_focus = false;
            this.agent_panel_click(ui::agentpanel::id::INPUT, cx);
            assert!(this.ai_chat_focus, "欄に焦点が移らない");
            this.agent_panel_click(ui::agentpanel::id::NEW, cx);
            assert!(this.ai_chat_log.is_empty(), "新しい会話でやりとりが消えない");
            // 宛先が決まっていなければ、送りは断って状態を言う
            this.ai_chat_in = Editor::new("直して");
            this.agent_panel_click(ui::agentpanel::id::SEND, cx);
            assert!(
                this.ai_busy || this.agent_state == AgentState::Unset,
                "送りが何も起こさない"
            );
        });
    }
}
