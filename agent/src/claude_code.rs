//! **宛先「Claude Code」** — 改変していない `claude` を子プロセスで動かす
//! (docs/sekkei/agent.ja.adoc「宛先『Claude Code』— `claude -p` を子プロセスで呼ぶ」。
//! 2026-09-04)。
//!
//! 定額(Pro / Max)を規約の内で使う唯一の形です。officework は `claude` を
//! 改変せず、ログインは Claude Code 自身の流れで済ませ、ここは認証の情報に
//! 触りません。道具は MCP(officework-mcp)で渡します。
//!
//! 1つの会話 = 1つのプロセス。標準入力に JSON を1行書くと1往復で、標準出力の
//! JSON 行を [`Parser`] が [`Cc`] に読みます。gpui を持たないので、画面は
//! [`ClaudeCode::try_recv`] を刻みで呼んで拾います(rpc の 30ms と同じ作法)。
//!
//! 行の形は 2026-09-04 に実物(claude 2.1.259、Max のログイン)で確かめました:
//! `system/init`(model・mcp_servers の status)、`assistant`(content に
//! `tool_use{id,name,input}` か `text`)、`user`(content に
//! `tool_result{tool_use_id,content,is_error}`)、`result`(subtype・session_id・
//! total_cost_usd)、`rate_limit_event`、`system/api_retry`。

use crate::Event;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Duration;

/// 起動の指定
#[derive(Debug, Clone)]
pub struct Launch {
    /// `claude` の実行ファイル(PATH の名前でも径路でも)
    pub claude: String,
    /// `sonnet` / `opus` / `haiku` / `fable`(Claude Code の別名)
    pub model: String,
    /// officework-mcp の径路と引数(`--panel` など)
    pub mcp_command: String,
    pub mcp_args: Vec<String>,
    /// system の文(パネルの AGENT_SYSTEM と同じ)
    pub system: String,
    /// 作業ディレクトリ。綴りではなく設定の置き場にして、プロジェクトの
    /// hooks や CLAUDE.md を読ませない
    pub cwd: PathBuf,
    pub max_turns: u32,
    /// 前の会話の続き(`result` の session_id)
    pub resume: Option<String>,
}

impl Launch {
    /// `claude` に渡す引数。**`--bare` は使わない**(OAuth を読まないので定額が
    /// 効かない。公式に明記)。`--tools ""` で Claude Code 自身の道具を全部止め、
    /// `--strict-mcp-config` でこれ以外の MCP を読まない
    pub fn args(&self) -> Vec<String> {
        let mcp = format!(
            "{{\"mcpServers\":{{\"officework\":{{\"command\":{},\"args\":{}}}}}}}",
            json_str(&self.mcp_command),
            json_list(&self.mcp_args)
        );
        let mut a: Vec<String> = vec![
            "-p".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--input-format".into(),
            "stream-json".into(),
            "--verbose".into(),
            "--model".into(),
            self.model.clone(),
            "--tools".into(),
            String::new(),
            "--mcp-config".into(),
            mcp,
            "--strict-mcp-config".into(),
            "--allowedTools".into(),
            "mcp__officework__*".into(),
            "--permission-mode".into(),
            "dontAsk".into(),
            "--system-prompt".into(),
            self.system.clone(),
            "--max-turns".into(),
            self.max_turns.to_string(),
        ];
        if let Some(id) = &self.resume {
            a.push("--resume".into());
            a.push(id.clone());
        }
        a
    }
}

/// 子プロセスから来た1つ
#[derive(Debug, Clone, PartialEq)]
pub enum Cc {
    /// 始まった。`mcp_ok` は officework-mcp が繋がったか
    Init { model: String, mcp_ok: bool, errors: Vec<String> },
    /// 会話の記録の1行(パネルの表示と `.agent.txt` の材料)
    Event(Event),
    /// 1往復が終わった。`ok` でなければ `text` はしくじりの文
    Done { session_id: Option<String>, ok: bool, text: String },
    /// 繋ぎ直し中(`system/api_retry`)
    Retry(String),
    /// プロセスが終わった(終了コード)
    Exit(Option<i32>),
}

/// JSON の行を [`Cc`] に読む。`tool_use` の id と名前を控え、`tool_result` に
/// 名前を添える(パネルの1行は名前で出す)
#[derive(Default)]
pub struct Parser {
    names: HashMap<String, String>,
}

impl Parser {
    /// 1行を読む。知らない行は空(黙って落とすのではなく、`Cc` にならない
    /// 行 — `rate_limit_event` など — を数えたい所は呼ぶ側で行を見る)
    pub fn line(&mut self, line: &str) -> Vec<Cc> {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { return Vec::new() };
        let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        let sub = v.get("subtype").and_then(|x| x.as_str()).unwrap_or("");
        match (t, sub) {
            ("system", "init") => {
                let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let mcp_ok = v
                    .get("mcp_servers")
                    .and_then(|x| x.as_array())
                    .is_some_and(|a| {
                        a.iter().any(|s| {
                            s.get("name").and_then(|n| n.as_str()) == Some("officework")
                                && s.get("status").and_then(|n| n.as_str()) == Some("connected")
                        })
                    });
                let errors = v
                    .get("mcp_server_errors")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .map(|e| {
                                format!(
                                    "{}: {}",
                                    e.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                                    e.get("message").and_then(|n| n.as_str()).unwrap_or("")
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                vec![Cc::Init { model, mcp_ok, errors }]
            }
            ("system", "api_retry") => {
                let n = v.get("attempt").and_then(|x| x.as_u64()).unwrap_or(0);
                let why = v.get("error").and_then(|x| x.as_str()).unwrap_or("");
                vec![Cc::Retry(format!("{why} ({n})"))]
            }
            ("assistant", _) | ("user", _) => {
                // サブエージェントの行は出さない(v1 では作らないので来ないはず)
                if v.get("parent_tool_use_id").is_some_and(|p| !p.is_null()) {
                    return Vec::new();
                }
                let mut out = Vec::new();
                let blocks = v.get("message").and_then(|m| m.get("content"));
                let Some(blocks) = blocks.and_then(|c| c.as_array()) else { return out };
                for b in blocks {
                    match b.get("type").and_then(|x| x.as_str()) {
                        Some("text") if t == "assistant" => {
                            let s = b.get("text").and_then(|x| x.as_str()).unwrap_or("");
                            if !s.trim().is_empty() {
                                out.push(Cc::Event(Event::Assistant(s.to_string())));
                            }
                        }
                        Some("tool_use") => {
                            let id = b.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            let name = short_name(b.get("name").and_then(|x| x.as_str()).unwrap_or(""));
                            let arguments = b.get("input").map(|i| i.to_string()).unwrap_or_default();
                            self.names.insert(id, name.clone());
                            out.push(Cc::Event(Event::ToolCall { name, arguments }));
                        }
                        Some("tool_result") => {
                            let id = b.get("tool_use_id").and_then(|x| x.as_str()).unwrap_or("");
                            let name = self.names.get(id).cloned().unwrap_or_default();
                            let ok = !b.get("is_error").and_then(|x| x.as_bool()).unwrap_or(false);
                            let content = content_text(b.get("content"));
                            out.push(Cc::Event(Event::ToolResult { name, content, ok }));
                        }
                        _ => {}
                    }
                }
                out
            }
            ("result", _) => {
                let ok = sub == "success";
                let text = v.get("result").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let text = if ok || !text.is_empty() {
                    text
                } else {
                    v.get("errors")
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| format!("Claude Code が止まりました({sub})"))
                };
                let session_id = v.get("session_id").and_then(|x| x.as_str()).map(|s| s.to_string());
                vec![Cc::Done { session_id, ok, text }]
            }
            _ => Vec::new(),
        }
    }
}

/// `mcp__officework__read_range` → `read_range`(パネルの行は道具の名前で)
fn short_name(name: &str) -> String {
    match name.strip_prefix("mcp__") {
        Some(rest) => rest.split_once("__").map(|(_, n)| n).unwrap_or(rest).to_string(),
        None => name.to_string(),
    }
}

/// `tool_result` の content は字か、`{type:text,text}` の並び
fn content_text(c: Option<&serde_json::Value>) -> String {
    match c {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(a)) => a
            .iter()
            .filter_map(|b| b.get("text").and_then(|x| x.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn json_str(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

fn json_list(v: &[String]) -> String {
    serde_json::Value::Array(v.iter().map(|s| serde_json::Value::String(s.clone())).collect()).to_string()
}

/// **officework-mcp の在り処。** 開いている綴りの `.venv` → 設定の置き場の
/// `.venv`(`~/.config/officework/.venv`)→ PATH の順(Python の探し方と同じ
/// 並び)。無ければ None(状態行で「pip install officework[mcp]」を案内する)
pub fn find_mcp(near: Option<&std::path::Path>) -> Option<PathBuf> {
    let bin = if cfg!(windows) { "Scripts/officework-mcp.exe" } else { "bin/officework-mcp" };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = near {
        dirs.push(d.join(".venv"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join(".venv"));
    }
    dirs.push(pyrun::venv_dir());
    for d in dirs {
        let p = d.join(bin);
        if p.is_file() {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(if cfg!(windows) { "officework-mcp.exe" } else { "officework-mcp" }))
        .find(|p| p.is_file())
}

/// パネルの宛先「Claude Code」の起動の指定を組む。作業ディレクトリは設定の
/// 置き場の `agent/`(綴りのフォルダにすると、そこの hooks や CLAUDE.md を
/// 読んでしまう)。`near` は開いているファイルのフォルダ(その `.venv` を先に見る)
pub fn launch_for(
    model: &str,
    system: &str,
    resume: Option<String>,
    near: Option<&std::path::Path>,
) -> Result<Launch, String> {
    launch_for_panel(model, system, resume, near, "sheet")
}

/// [`launch_for`] の、どの画面のパネルから起こすかを言う形。`panel` は
/// "sheet"(表)か "doc"(文書)。officework-mcp はこれで run_macro の中身を
/// 選ぶ(表は `b` と `s`、文書は `src` と `out`)
pub fn launch_for_panel(
    model: &str,
    system: &str,
    resume: Option<String>,
    near: Option<&std::path::Path>,
    panel: &str,
) -> Result<Launch, String> {
    let mcp = find_mcp(near).ok_or_else(|| {
        "officework-mcp がありません。次で入ります:\n  pip install \"officework[mcp]\"".to_string()
    })?;
    let cwd = pyrun::config_dir().join("agent");
    std::fs::create_dir_all(&cwd).map_err(|e| format!("{}: {e}", cwd.display()))?;
    Ok(Launch {
        claude: "claude".into(),
        model: if model.trim().is_empty() { "sonnet".into() } else { model.to_string() },
        mcp_command: mcp.to_string_lossy().to_string(),
        mcp_args: vec![if panel == "doc" { "--panel=doc".into() } else { "--panel".into() }],
        system: system.to_string(),
        cwd,
        max_turns: 30,
        resume,
    })
}

/// 動いている子プロセス
pub struct ClaudeCode {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
    parser: Parser,
    /// `result` で分かった会話の id(続きに使う)
    pub session_id: Option<String>,
    /// 読んだ行の数(何も返らない時の見分けに)
    pub lines: usize,
}

impl ClaudeCode {
    /// 起動する。標準出力は別の糸で行ごとに拾う(読み待ちで画面を止めない)
    pub fn spawn(l: &Launch) -> Result<Self, String> {
        let mut child = Command::new(&l.claude)
            .args(l.args())
            .current_dir(&l.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{} を起動できません: {e}", l.claude))?;
        let stdin = child.stdin.take().ok_or("標準入力が取れません")?;
        let stdout = child.stdout.take().ok_or("標準出力が取れません")?;
        let (tx, rx) = channel::<String>();
        std::thread::spawn(move || {
            let r = BufReader::new(stdout);
            for line in r.lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(ClaudeCode { child, stdin, rx, parser: Parser::default(), session_id: l.resume.clone(), lines: 0 })
    }

    /// 頼みを1つ送る(1往復の始まり)
    pub fn send(&mut self, user: &str) -> Result<(), String> {
        let line = format!(
            "{{\"type\":\"user\",\"message\":{{\"role\":\"user\",\"content\":{}}}}}\n",
            json_str(user)
        );
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("Claude Code に書けません: {e}"))
    }

    /// 溜まっている物を全部読む(待たない)。プロセスが終わっていれば最後に Exit
    pub fn try_recv(&mut self) -> Vec<Cc> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(line) => {
                    self.lines += 1;
                    for c in self.parser.line(&line) {
                        if let Cc::Done { session_id: Some(id), .. } = &c {
                            self.session_id = Some(id.clone());
                        }
                        out.push(c);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if let Ok(Some(st)) = self.child.try_wait() {
                        out.push(Cc::Exit(st.code()));
                    }
                    break;
                }
            }
        }
        out
    }

    /// 1つ来るまで待つ(試験と画面の無い口)。`d` で諦めて空
    pub fn recv_timeout(&mut self, d: Duration) -> Vec<Cc> {
        match self.rx.recv_timeout(d) {
            Ok(line) => {
                self.lines += 1;
                let mut out = Vec::new();
                for c in self.parser.line(&line) {
                    if let Cc::Done { session_id: Some(id), .. } = &c {
                        self.session_id = Some(id.clone());
                    }
                    out.push(c);
                }
                out.extend(self.try_recv());
                out
            }
            Err(_) => self.try_recv(),
        }
    }

    /// いまの往復をやめさせる(SIGINT。途中までの記録は残る)
    pub fn interrupt(&mut self) {
        #[cfg(unix)]
        {
            let _ = Command::new("kill").args(["-INT", &self.child.id().to_string()]).status();
        }
        #[cfg(not(unix))]
        {
            let _ = self.child.kill();
        }
    }

    /// 会話を終える(「新しい会話」)。プロセスを止める
    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ClaudeCode {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// `claude` が動くか(PATH にあって `--version` が返る)
pub fn available(claude: &str) -> bool {
    Command::new(claude)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// ログイン済みか(`claude auth status` は 0 なら済み、1 なら未)。
/// **ログインの画面はここでは作らない**(規約: 資格の収集も仲介もしない)
pub fn logged_in(claude: &str) -> bool {
    Command::new(claude)
        .args(["auth", "status"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 実物(claude 2.1.259)が返した行(長い欄は刈った)
    const INIT: &str = r#"{"type":"system","subtype":"init","model":"claude-sonnet-5","mcp_servers":[{"name":"officework","status":"connected"}],"tools":["mcp__officework__book_info"],"session_id":"fc04"}"#;
    const CALL: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01","name":"mcp__officework__book_info","input":{}}]},"parent_tool_use_id":null,"session_id":"fc04"}"#;
    const RESULT_ERR: &str = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"Error executing tool book_info: calc に繋がりません","is_error":true,"tool_use_id":"toolu_01"}]},"parent_tool_use_id":null,"session_id":"fc04"}"#;
    const TEXT: &str = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"officeworkにつながらず、ブック情報を取得できませんでした。"}]},"parent_tool_use_id":null,"session_id":"fc04"}"#;
    const DONE: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":2,"result":"officeworkにつながらず、ブック情報を取得できませんでした。","session_id":"fc0415fb-8273-4ff9-95dc-38c43e1449d0","total_cost_usd":0.0283978}"#;

    #[test]
    fn the_recorded_lines_become_events_in_order() {
        let mut p = Parser::default();
        assert_eq!(
            p.line(INIT),
            vec![Cc::Init { model: "claude-sonnet-5".into(), mcp_ok: true, errors: vec![] }]
        );
        assert_eq!(p.line(r#"{"type":"rate_limit_event","rate_limit_info":{}}"#), vec![]);
        assert_eq!(
            p.line(CALL),
            vec![Cc::Event(Event::ToolCall { name: "book_info".into(), arguments: "{}".into() })]
        );
        assert_eq!(
            p.line(RESULT_ERR),
            vec![Cc::Event(Event::ToolResult {
                name: "book_info".into(),
                content: "Error executing tool book_info: calc に繋がりません".into(),
                ok: false
            })]
        );
        assert_eq!(
            p.line(TEXT),
            vec![Cc::Event(Event::Assistant("officeworkにつながらず、ブック情報を取得できませんでした。".into()))]
        );
        assert_eq!(
            p.line(DONE),
            vec![Cc::Done {
                session_id: Some("fc0415fb-8273-4ff9-95dc-38c43e1449d0".into()),
                ok: true,
                text: "officeworkにつながらず、ブック情報を取得できませんでした。".into()
            }]
        );
        // 壊れた行は空(落ちない)
        assert_eq!(p.line("not json"), vec![]);
    }

    #[test]
    fn a_failed_mcp_server_shows_in_init() {
        let mut p = Parser::default();
        let l = r#"{"type":"system","subtype":"init","model":"claude-sonnet-5","mcp_servers":[{"name":"officework","status":"failed"}],"mcp_server_errors":[{"name":"officework","type":"invalid_config","message":"no such file"}]}"#;
        assert_eq!(
            p.line(l),
            vec![Cc::Init { model: "claude-sonnet-5".into(), mcp_ok: false, errors: vec!["officework: no such file".into()] }]
        );
    }

    /// 文書のパネルから起こす時は officework-mcp に `--panel=doc` を渡す
    /// (run_macro の中身が文書の形になる)。表は `--panel` のまま
    #[test]
    fn the_document_panel_tells_the_mcp_which_macro_to_offer() {
        let sheet = launch_for("sonnet", "x", None, None);
        let doc = launch_for_panel("sonnet", "x", None, None, "doc");
        match (sheet, doc) {
            (Ok(s), Ok(d)) => {
                assert_eq!(s.mcp_args, vec!["--panel".to_string()]);
                assert_eq!(d.mcp_args, vec!["--panel=doc".to_string()]);
            }
            // officework-mcp が無い機械では両方とも同じ断り
            (Err(a), Err(b)) => assert_eq!(a, b),
            other => panic!("片方だけ起こせる: {other:?}"),
        }
    }

    #[test]
    fn the_arguments_keep_claude_code_unmodified_and_tools_limited_to_ours() {
        let l = Launch {
            claude: "claude".into(),
            model: "sonnet".into(),
            mcp_command: "/x/.venv/bin/officework-mcp".into(),
            mcp_args: vec!["--panel".into()],
            system: "助手です".into(),
            cwd: PathBuf::from("/tmp"),
            max_turns: 30,
            resume: Some("abc".into()),
        };
        let a = l.args();
        let has = |k: &str| a.iter().any(|x| x == k);
        assert!(has("-p") && has("--strict-mcp-config") && has("--verbose"));
        assert!(!has("--bare"), "bare は OAuth を読まないので使わない");
        let at = |k: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone()).unwrap();
        assert_eq!(at("--tools"), "", "Claude Code 自身の道具を止める");
        assert_eq!(at("--allowedTools"), "mcp__officework__*");
        assert_eq!(at("--permission-mode"), "dontAsk");
        assert_eq!(at("--input-format"), "stream-json");
        assert_eq!(at("--output-format"), "stream-json");
        assert_eq!(at("--model"), "sonnet");
        assert_eq!(at("--max-turns"), "30");
        assert_eq!(at("--resume"), "abc");
        assert_eq!(
            at("--mcp-config"),
            r#"{"mcpServers":{"officework":{"command":"/x/.venv/bin/officework-mcp","args":["--panel"]}}}"#
        );
    }

    /// 偽の `claude`(Python の台本)で、起動 → 送る → 受ける → 終える を通す
    #[test]
    #[cfg(unix)]
    fn a_fake_claude_round_trips_through_stdin_and_stdout() {
        let dir = std::env::temp_dir().join(format!("officework-fake-claude-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("claude");
        std::fs::write(
            &bin,
            format!(
                "#!/usr/bin/env python3\nimport sys, json\nprint({init})\nsys.stdout.flush()\nfor line in sys.stdin:\n    o = json.loads(line)\n    text = o['message']['content']\n    print({call})\n    print({res})\n    print(json.dumps({{'type':'assistant','message':{{'role':'assistant','content':[{{'type':'text','text':'答え: '+text}}]}},'parent_tool_use_id':None}}, ensure_ascii=False))\n    print(json.dumps({{'type':'result','subtype':'success','result':'答え: '+text,'session_id':'s1'}}, ensure_ascii=False))\n    sys.stdout.flush()\n",
                init = json_str(INIT),
                call = json_str(CALL),
                res = json_str(RESULT_ERR),
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let l = Launch {
            claude: bin.to_string_lossy().to_string(),
            model: "sonnet".into(),
            mcp_command: "officework-mcp".into(),
            mcp_args: vec![],
            system: "x".into(),
            cwd: dir.clone(),
            max_turns: 3,
            resume: None,
        };
        let mut cc = ClaudeCode::spawn(&l).unwrap();
        cc.send("こんにちは").unwrap();
        let mut got: Vec<Cc> = Vec::new();
        let t0 = std::time::Instant::now();
        while !got.iter().any(|c| matches!(c, Cc::Done { .. })) && t0.elapsed() < Duration::from_secs(20) {
            got.extend(cc.recv_timeout(Duration::from_millis(200)));
        }
        assert!(matches!(got.first(), Some(Cc::Init { mcp_ok: true, .. })), "{got:?}");
        assert!(got.iter().any(|c| matches!(c, Cc::Event(Event::ToolCall { name, .. }) if name == "book_info")));
        assert!(got.iter().any(|c| matches!(c, Cc::Event(Event::Assistant(s)) if s == "答え: こんにちは")));
        assert!(matches!(got.last(), Some(Cc::Done { ok: true, session_id: Some(s), .. }) if s == "s1"), "{got:?}");
        assert_eq!(cc.session_id.as_deref(), Some("s1"));
        cc.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
