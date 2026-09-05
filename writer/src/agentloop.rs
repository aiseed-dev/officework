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
番号で指します。\
文書の全部を読み直さないでください。長い文書ではトークンが尽きます。\
**値を自分で作らないでください。** 記入欄・数値・日付・氏名のような\
「決まっている値」は、人が言った物か、人が指した資料にある物だけを入れます。\
分からなければ聞き返してください。\
保存は人の確認が要ります。求められたときだけ doc_save を呼んでください。";

impl Writer {
    /// **文章の画面が名乗る道具**: 文書の9つ(受け口へ直結)。
    ///
    /// マクロ(`run_macro`)はまだ足しません — 文書のマクロを
    /// サンドボックスで走らせる道は、表計算の側にしかありません
    fn agent_tools() -> Vec<lang::model::ToolDef> {
        let mut v = agent::tools::doc_tools();
        // 文書のマクロ: adoc の字を受け渡す Python(2026-09-05)
        v.push(agent::tools::doc_macro_tool());
        v
    }

    /// いまの宛先の行(種類で分岐するため。`agent_dest` は Endpoint の形)
    pub(crate) fn agent_dest_row(&self) -> Option<face::settings::AiDest> {
        let rows = face::settings::ai_list();
        let last = face::settings::ai_last();
        rows.iter()
            .find(|r| Some(r.name.as_str()) == last.as_deref())
            .or(rows.first())
            .cloned()
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

    /// パネルに1行積む(記録にも1行書く)。
    pub(crate) fn chat_push(&mut self, row: ChatRow) {
        self.record_row(&row);
        self.ai_chat_log.push(row);
    }

    // ── 会話の記録(表計算と同じ形。2026-09-05)──
    // 文書の隣の `<名前>.agent.txt` に1行1件。名前の無い文書は記録しない

    fn record_path(&self) -> Option<std::path::PathBuf> {
        let p = self.path.as_ref()?;
        let stem = p.file_stem()?.to_string_lossy().to_string();
        Some(p.with_file_name(format!("{stem}.agent.txt")))
    }

    fn record_row(&mut self, row: &ChatRow) {
        let Some(p) = self.record_path() else { return };
        let esc = |s: &str| s.replace('\\', "\\\\").replace('\n', "\\n");
        let line = match row {
            ChatRow::Me(t) => format!("人: {}", esc(t)),
            ChatRow::Ai(t) => format!("答え: {}", esc(t)),
            ChatRow::Tool(t, one) => format!("道具: {}{}", esc(t), if *one { "(1手)" } else { "" }),
        };
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
            let _ = writeln!(f, "{line}");
        }
    }

    fn record_header(&mut self) {
        let Some(p) = self.record_path() else { return };
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let off: i64 = std::env::var("JO_TZ_OFF_HOURS").ok().and_then(|v| v.parse().ok()).unwrap_or(9);
        let (y, m, d) = book::calc::civil_from_days((secs + off * 3600).div_euclid(86400));
        let hm = (secs + off * 3600).rem_euclid(86400);
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
            let _ = writeln!(f, "== {y}-{m:02}-{d:02} {:02}:{:02}", hm / 3600, hm % 3600 / 60);
        }
    }

    /// 記録の読み直し — 最後の会話ぶんを欄へ戻す(左パネルを開いたとき、欄が
    /// 空なら)。モデルの履歴は戻さない — 見えるのは控え
    pub(crate) fn agent_load_record(&mut self) {
        if !self.ai_chat_log.is_empty() {
            return;
        }
        let Some(p) = self.record_path() else { return };
        let Ok(s) = std::fs::read_to_string(&p) else { return };
        let lines: Vec<&str> = s.lines().collect();
        let from = lines.iter().rposition(|l| l.starts_with("== ")).map(|i| i + 1).unwrap_or(0);
        let un = |s: &str| s.replace("\\n", "\n").replace("\\\\", "\\");
        let mut rows = Vec::new();
        for l in &lines[from..] {
            if let Some(t) = l.strip_prefix("人: ") {
                rows.push(ChatRow::Me(un(t)));
            } else if let Some(t) = l.strip_prefix("答え: ") {
                rows.push(ChatRow::Ai(un(t)));
            } else if let Some(t) = l.strip_prefix("道具: ") {
                let one = t.ends_with("(1手)");
                rows.push(ChatRow::Tool(un(t.trim_end_matches("(1手)")), one));
            }
        }
        if rows.is_empty() {
            return;
        }
        self.ai_chat_log.push(ChatRow::Tool(ui::t!("previous_record").to_string(), false));
        self.ai_chat_log.extend(rows);
    }

    /// **新しい会話にする。** やりとりも履歴も捨てます(文書は触りません)
    pub(crate) fn chat_reset(&mut self) {
        self.ai_chat_log.clear();
        self.agent = None;
        if let Some(mut cc) = self.agent_cc.take() {
            cc.stop();
        }
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
        if self.agent.is_none() {
            // 新しい会話 — 記録に日付つきの見出しを置く
            self.record_header();
        }
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
        // **宛先「Claude Code」**は、改変していない claude を子プロセスで(定額の道)。
        // 道具は officework-mcp が受け口へ運ぶので、ここでは道具を回さない
        if self.agent_dest_row().is_some_and(|r| r.kind() == face::settings::AiKind::ClaudeCode) {
            self.agent_send_cc(user, cx);
            return;
        }
        self.agent_step(ep, msgs, cx);
    }

    /// 宛先「Claude Code」に1つ送り、返りを刻みで拾う(agent::claude_code)
    fn agent_send_cc(&mut self, user: String, cx: &mut Context<Self>) {
        use agent::claude_code as cc;
        if self.agent_cc.is_none() {
            if !cc::logged_in("claude") {
                self.ai_busy = false;
                self.agent_state = AgentState::Unset;
                self.status = ui::t!("claude_code_login_in_terminal").into();
                return;
            }
            let model = self.agent_dest_row().map(|r| r.model).unwrap_or_default();
            let near = self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()));
            let launch = match cc::launch_for(&model, AGENT_SYSTEM, None, near.as_deref()) {
                Ok(l) => l,
                Err(e) => {
                    self.ai_busy = false;
                    self.agent_state = AgentState::Failed;
                    self.chat_push(ChatRow::Ai(format!("({e})")));
                    self.status = format!("AI: {e}").into();
                    return;
                }
            };
            match cc::ClaudeCode::spawn(&launch) {
                Ok(p) => self.agent_cc = Some(p),
                Err(e) => {
                    self.ai_busy = false;
                    self.agent_state = AgentState::Failed;
                    self.chat_push(ChatRow::Ai(format!("({e})")));
                    self.status = format!("AI: {e}").into();
                    return;
                }
            }
        }
        if let Err(e) = self.agent_cc.as_mut().expect("上で置いた").send(&user) {
            self.ai_busy = false;
            self.agent_state = AgentState::Failed;
            self.chat_push(ChatRow::Ai(format!("({e})")));
            return;
        }
        // 100ms ごとに拾う(rpc の 30ms と同じ作法。往復が終わるまで)
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
                let done = this.update(cx, |this, cx| {
                    let d = this.agent_cc_poll();
                    cx.notify();
                    d
                });
                match done {
                    Ok(false) => continue,
                    _ => break,
                }
            }
        })
        .detach();
    }

    /// 子プロセスの返りを拾って画面へ。1往復が終わったら true
    fn agent_cc_poll(&mut self) -> bool {
        use agent::claude_code::Cc;
        let Some(cc) = self.agent_cc.as_mut() else { return true };
        let got = cc.try_recv();
        let mut done = false;
        for c in got {
            match c {
                Cc::Init { mcp_ok, errors, .. } => {
                    self.agent_state = AgentState::Connected;
                    if !mcp_ok {
                        self.chat_push(ChatRow::Tool(ui::tf!("officework_mcp_not_connected", errors.join(" / ")).to_string(), false));
                    }
                }
                Cc::Event(e) => {
                    if let Some(ag) = self.agent.as_mut() {
                        ag.log.push(e);
                    }
                    self.agent_drain_log();
                }
                Cc::Retry(_) => self.agent_state = AgentState::Connecting,
                Cc::Done { ok, text, .. } => {
                    if !ok {
                        self.chat_push(ChatRow::Ai(format!("({text})")));
                        self.agent_state = AgentState::Failed;
                    } else {
                        self.agent_state = AgentState::Connected;
                        self.status = ui::t!("answered_left_panel").into();
                    }
                    self.ai_busy = false;
                    done = true;
                }
                Cc::Exit(code) => {
                    self.chat_push(ChatRow::Ai(ui::tf!("claude_code_exited", code.unwrap_or(-1)).to_string()));
                    self.agent_state = AgentState::Failed;
                    self.ai_busy = false;
                    self.agent_cc = None;
                    done = true;
                }
            }
        }
        done
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
            if c.name == "run_macro" {
                // マクロは裏の糸で走る(終わりの callback が続きを呼ぶ)
                self.agent_run_macro(c, cx);
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
        // **捌き手はどの OS でも組みます**(2026-09-04。ソケットを開く
        // `rpc::start` だけが `#[cfg(unix)]`)。道具の呼びはここを通り、
        // `officework-mcp` の `doc_*` と同じ動詞に届きます
        let reply = crate::rpc::handle(self, &line);
        // 受け口は `{"ok":true,…}` か `{"err":"…"}` を返します
        match ops::Jobj::parse(&reply).and_then(|r| r.str("err")) {
            Some(e) => Err(e),
            None => Ok(reply),
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

    /// 裏に跨った道具(マクロ)が終わった。結果を入れて、残りを続ける
    fn agent_finish_call(&mut self, c: lang::model::ToolCall, r: Result<String, String>, cx: &mut Context<Self>) {
        let ag = self.agent.as_mut().expect("agent_send が置いた");
        ag.tool_result(&c, r);
        self.agent_drain_log();
        if !self.agent_calls.is_empty() {
            self.agent_calls.remove(0);
        }
        self.agent_run_calls(cx);
    }

    /// **文書のマクロ**(2026-09-05)。docx を通さず、文書の AsciiDoc の字を
    /// `src` で渡し、`out` に入った字を読み直して1手で入れる。コードは見える
    /// .py として文書の隣に置く(見えないコードは運ばない)。実行は
    /// サンドボックス(pyrun。網なし・60秒)。誤りの尻尾はモデルに返して直させる
    fn agent_run_macro(&mut self, c: lang::model::ToolCall, cx: &mut Context<Self>) {
        let args = if c.arguments.trim().is_empty() { "{}" } else { c.arguments.as_str() };
        let o = ops::Jobj::parse(args);
        let Some(code) = o.as_ref().and_then(|o| o.str("code")) else {
            self.agent_finish_call(c, Err("code がありません".into()), cx);
            return;
        };
        let name: String = o
            .as_ref()
            .and_then(|o| o.str("name"))
            .unwrap_or_else(|| "agent_macro".into())
            .chars()
            .map(|ch| if ch.is_alphanumeric() || ch == '_' || ch == '-' { ch } else { '_' })
            .collect();
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            if let Err(e) = std::fs::write(dir.join(format!("{name}.py")), code.as_bytes()) {
                self.agent_finish_call(c, Err(format!("マクロを置けません: {e}")), cx);
                return;
            }
        }
        self.flush_target();
        let src = kumihan::adoc::write(&self.doc);
        let dir = pyrun::cage_work_dir("jo-wagent");
        let _ = std::fs::create_dir_all(&dir);
        let in_a = dir.join("in.adoc");
        let out_a = dir.join("out.adoc");
        if let Err(e) = std::fs::write(&in_a, src) {
            self.agent_finish_call(c, Err(format!("文書を渡せません: {e}")), cx);
            return;
        }
        let script = doc_macro_script(&in_a, &out_a, &code);
        self.status = ui::t!("running_python").into();
        let task = cx.background_executor().spawn(async move {
            let py_path = dir.join("run.py");
            std::fs::write(&py_path, script).map_err(|e| e.to_string())?;
            let py = pyrun::find_python();
            let venv = std::fs::canonicalize(".venv").unwrap_or_default();
            let Some(mut cmd) = pyrun::caged_python(&py, &dir, &[venv], false) else {
                return Err(if cfg!(target_os = "linux") {
                    ui::t!("cant_build_sandbox_code").to_string()
                } else {
                    ui::t!("os_no_sandbox_code").to_string()
                });
            };
            let (ok, out, err) = pyrun::run_with_timeout(cmd.arg(&py_path), 60).map_err(|e| match e {
                pyrun::RunErr::Spawn(e) => format!("Python が起動できません: {e}"),
                pyrun::RunErr::Timeout(s) => ui::tf!("stopped_after_seconds_endless", s).to_string(),
                pyrun::RunErr::Wait(e) => e,
            })?;
            let out = out.trim().to_string();
            if !ok {
                let tail = err.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
                return Err(if tail.trim().is_empty() { "原因不明".into() } else { tail });
            }
            std::fs::read_to_string(&out_a).map_err(|e| format!("結果が読めません: {e}")).map(|s| (s, out))
        });
        let task: gpui::Task<Result<(String, String), String>> = task;
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                let reply = match r {
                    Ok((adoc, out)) => match kumihan::adoc::parse_full(&adoc) {
                        Ok((d, _notes)) => {
                            // 1手として取り込む(頭の属性はそのまま、本文だけ差し替え)
                            this.acted = false;
                            this.checkpoint(false);
                            this.doc.blocks = d.blocks;
                            this.after_block_edit();
                            Ok(if out.is_empty() { "終わりました".to_string() } else { out })
                        }
                        Err(e) => Err(format!("直した字が AsciiDoc として読めません: {e}")),
                    },
                    Err(e) => Err(e),
                };
                this.agent_finish_call(c, reply, cx);
                cx.notify();
            });
        })
        .detach();
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
            let reply = crate::rpc::handle(self, &line);
            match ops::Jobj::parse(&reply).and_then(|x| x.str("err")) {
                Some(e) => Err(e),
                None => Ok(reply),
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



    /// **宛先を足す・直す画面へ**(2026-09-04)。ファイルのページの詳細設定に
    /// AI の宛先の一覧があります。パネルからそこへ跳びます
    fn open_ai_settings(&mut self) {
        self.agent_picking = None;
        self.prev_tab = self.tab.max(1);
        self.tab = 0;
        self.file_view = 2;
        self.status = ui::t!("add_or_edit_destination").into();
    }

    /// **宛先の一覧を画面の行に直す**(2026-09-04)。
    ///
    /// いま使っている物に印を付けます。細かい字はモデル名 — 同じ提供元に
    /// 別のモデルを並べたときに見分けが付きません
    pub(crate) fn dest_rows(&self) -> Option<Vec<ui::agentpanel::DestRow>> {
        let rows = self.agent_picking.as_ref()?;
        let now = self.agent_dest().map(|(n, _)| n);
        Some(
            rows.iter()
                .map(|d| ui::agentpanel::DestRow {
                    name: d.name.clone(),
                    detail: if d.model.is_empty() { d.url.clone() } else { d.model.clone() },
                    now: Some(&d.name) == now.as_ref(),
                })
                .collect(),
        )
    }

    /// パネルの物が押されたとき(描きは [`ui::agentpanel::body`] と共通)。
    pub(crate) fn agent_panel_click(&mut self, id: &str, cx: &mut Context<Self>) {
        use ui::agentpanel::id as pid;
        // **宛先の一覧の行**(`dest:3`)。番号で選びます
        if let Some(n) = id.strip_prefix(pid::DEST) {
            if let (Ok(i), Some(rows)) = (n.parse::<usize>(), self.agent_picking.clone()) {
                if let Some(d) = rows.get(i) {
                    face::settings::set_ai_last(&d.name);
                    self.agent_state = ui::agentpanel::AgentState::Idle;
                    self.status = ui::tf!("ai_destination_remembered", d.name.clone()).into();
                }
            }
            self.agent_picking = None;
            return;
        }
        match id {
            pid::NEW => self.chat_reset(),
            pid::INPUT => self.ai_chat_focus = true,
            pid::SEND => self.ai_chat_send(cx),
            // **押すと一覧が開きます**(前は次へ回るだけでした)。
            // 手元のモデルを探すのはここだけ — 描くたびに港は叩きません
            pid::WHERE => {
                self.agent_picking = match self.agent_picking {
                    Some(_) => None,
                    None => Some(face::settings::ai_list_all()),
                };
            }
            pid::DEST_EDIT => self.open_ai_settings(),
            pid::SAVE_OK => self.agent_confirm_save(true, cx),
            pid::SAVE_NO => self.agent_confirm_save(false, cx),
            _ => {}
        }
    }
}

/// 文書のマクロの台本。`src` を渡し、`out` を受け取る。**利用者のコードの前後
/// だけ**をこちらが書く(python-docx は使わない)
pub(crate) fn doc_macro_script(in_a: &std::path::Path, out_a: &std::path::Path, code: &str) -> String {
    format!(
        concat!(
            "import io, sys\n",
            "sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')\n",
            "src = open({in_a:?}, encoding='utf-8').read()\n",
            "out = None\n",
            "# ---- エージェントのコード ----\n",
            "{code}\n",
            "# ----\n",
            "if out is None:\n",
            "    raise SystemExit('out に、直した AsciiDoc の字を入れてください')\n",
            "if not isinstance(out, str):\n",
            "    raise SystemExit('out は字(str)にしてください')\n",
            "open({out_a:?}, 'w', encoding='utf-8').write(out)\n"
        ),
        in_a = in_a.to_string_lossy(),
        out_a = out_a.to_string_lossy(),
        code = code
    )
}

#[cfg(test)]
mod tests {
    use crate::*;
    use lang::model::ToolCall;

    /// マクロの台本は src を渡して out を受け取る。out が無ければ止まる
    #[test]
    fn the_document_macro_script_hands_src_and_takes_out() {
        let s = crate::agentloop::doc_macro_script(
            std::path::Path::new("/tmp/in.adoc"),
            std::path::Path::new("/tmp/out.adoc"),
            "out = src.replace('旧', '新')",
        );
        assert!(s.contains("src = open(\"/tmp/in.adoc\""));
        assert!(s.contains("out = src.replace('旧', '新')"));
        assert!(s.contains("if out is None"));
        assert!(s.contains("write(out)"));
        assert!(!s.contains("docx"), "python-docx を通さない");
    }

    /// 会話の記録: 名前のある文書なら隣の .agent.txt に残り、開き直すと戻る
    #[gpui::test]
    fn the_conversation_is_recorded_next_to_the_document(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("officework-wrec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("報告.adoc");
        std::fs::write(&p, "本文。\n").unwrap();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.native = false;
            this.set_doc(kumihan::adoc::parse("本文。\n").unwrap());
            this.path = Some(p.clone());
            this.chat_push(ChatRow::Me("短くして".into()));
            this.chat_push(ChatRow::Tool("doc_replace_blocks 2".into(), true));
            this.chat_push(ChatRow::Ai("直しました".into()));
            let rec = std::fs::read_to_string(dir.join("報告.agent.txt")).unwrap();
            assert!(rec.contains("人: 短くして") && rec.contains("道具: doc_replace_blocks 2(1手)") && rec.contains("答え: 直しました"), "{rec}");
            this.ai_chat_log.clear();
            this.agent_load_record();
            assert_eq!(this.ai_chat_log.len(), 4, "控えの印 + 3 行: {:?}", this.ai_chat_log);
            assert_eq!(this.ai_chat_log[3], ChatRow::Ai("直しました".into()));
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

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
            // この機械の宛先の一覧で答えが変わる(宛先「Claude Code」なら、
            // officework-mcp が無い試験の環境では Failed と言って止まる)
            assert!(
                this.ai_busy || matches!(this.agent_state, AgentState::Unset | AgentState::Failed),
                "送りが何も起こさない"
            );
        });
    }
}
