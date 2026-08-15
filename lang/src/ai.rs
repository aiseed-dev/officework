//! AI の宛先(SEKKEI「AI メニュー」の宛先の規律)。
//!
//! **どこへ送るかは人が決める。** 文書やブックの中身を外へ出すかは
//! 現場の判断なので、既定は手元(ローカル)のまま、選んだ宛先を
//! `~/.config/officework/ai.txt` に覚える。**鍵が文書に入ることは決してない。**
//!
//! 宛先は3つ:
//!
//! - **ローカル** — 127.0.0.1 の OpenAI 互換(校正と同じ口)。外に出ない
//! - **Claude Agent** — Anthropic の Agent SDK(Python)を子で呼ぶ。
//!   `pip install claude-agent-sdk` だけで足りる(SDK が自分の実行体を
//!   同梱している)。**認証は API の鍵**
//! - **Claude(API)** — Anthropic の API を直に叩く。Python も要らない
//!
//! # 鍵で認証する(2026-08-15 に作り替えた)
//!
//! 前は「手元の `claude` コマンドを子で呼び、その認証に相乗りする」道を
//! 持っていた。**これは規約に反する**:
//!
//! > Unless previously approved, Anthropic does not allow third party
//! > developers to offer claude.ai login or rate limits for their products,
//! > including agents built on the Claude Agent SDK. Use the API key
//! > authentication methods described in the Quickstart instead.
//!
//! 禁じられているのは「**開発者が自社製品でログインと枠を提供すること**」。
//! そこで相乗りの道をやめ、公式の Agent SDK に載せ替えて、
//! **鍵(`ANTHROPIC_API_KEY`)を要る物にした**。うちはログインの口を作らない。
//!
//! 画面に "Claude Code" とは書かない(Anthropic の商標の指針で許されて
//! いない。使えるのは "Claude" / "Claude Agent")。
//!
//! 繋がらなければ「できません」と言う。**黙って空の結果にしない。**

use crate::model::{self, Endpoint};

/// AI の宛先
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backend {
    /// 手元のモデル(既定。基幹網の外に出ない)
    #[default]
    Local,
    /// Anthropic の Agent SDK(Python)経由。認証は API の鍵
    ClaudeAgent,
    /// **OpenAI Codex の CLI**(手元の `codex`)。認証はその CLI が持つ物を
    /// そのまま使う — ChatGPT の契約でも動く。
    ///
    /// **発注者の判断(2026-08-15)**「Anthropic には承認を取る前提で、
    /// OpenAI の定額が使えるようにしておく」。OpenAI 側に明文の許可を
    /// 見つけたわけではない(禁止も見つからなかった、という状態)。
    /// 向こうの認証の頁は、プログラムから使う道として**鍵**を挙げている
    OpenAiCodex,
    /// Anthropic の API(鍵は環境変数から)
    ClaudeApi,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Backend::Local => "手元のモデル(外に出ない)",
            Backend::ClaudeAgent => "Claude Agent(SDK 経由。鍵は環境変数から)",
            Backend::OpenAiCodex => "OpenAI Codex(手元の codex を使う)",
            Backend::ClaudeApi => "Claude(API。鍵は環境変数から)",
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Backend::Local => "local",
            Backend::ClaudeAgent => "claude-agent",
            Backend::OpenAiCodex => "openai-codex",
            Backend::ClaudeApi => "claude-api",
        }
    }
    fn from_str(s: &str) -> Backend {
        match s.trim() {
            "claude-agent" => Backend::ClaudeAgent,
            "openai-codex" => Backend::OpenAiCodex,
            // 2026-08-15 に廃した宛先。**黙って読み替えない** — 手元の
            // モデルへ落ちるので、選び直してもらう
            "claude-cli" => Backend::Local,
            "claude-api" => Backend::ClaudeApi,
            _ => Backend::Local,
        }
    }
    /// 次の宛先(ボタンで回して選ぶ)
    pub fn next(self) -> Backend {
        match self {
            Backend::Local => Backend::ClaudeAgent,
            Backend::ClaudeAgent => Backend::OpenAiCodex,
            Backend::OpenAiCodex => Backend::ClaudeApi,
            Backend::ClaudeApi => Backend::Local,
        }
    }
}

fn config_path() -> std::path::PathBuf {
    pyrun::config_dir().join("ai.txt")
}

/// 覚えている宛先を読む(環境変数 `JO_AI` が優先)
pub fn backend() -> Backend {
    if let Ok(v) = std::env::var("JO_AI") {
        return Backend::from_str(&v);
    }
    std::fs::read_to_string(config_path())
        .map(|s| Backend::from_str(&s))
        .unwrap_or_default()
}

/// 宛先を覚える
pub fn set_backend(b: Backend) {
    let p = config_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, b.as_str());
}

/// Agent SDK に渡す小さな橋。**標準入力は使えない**(pyrun の
/// `run_with_timeout` が閉じる)ので、頼みは JSON のファイルで渡す。
///
/// **道具は与えない**(`tools=[]`)— 手元のファイルを触らせない。
/// 一往復で終わらせる(`max_turns=1`)。
const BRIDGE: &str = r#"import json, sys
import anyio
from claude_agent_sdk import query, ClaudeAgentOptions

spec = json.load(open(sys.argv[1], encoding="utf-8"))

async def main():
    out = []
    opts = ClaudeAgentOptions(
        system_prompt=spec.get("system") or None,
        max_turns=1,
        tools=[],
        model=spec.get("model") or None,
    )
    async for m in query(prompt=spec["user"], options=opts):
        for b in getattr(m, "content", None) or []:
            t = getattr(b, "text", None)
            if t:
                out.append(t)
    sys.stdout.write("".join(out))

anyio.run(main)
"#;

/// 走らせられなかった理由を人の言葉に(pyrun の `RunErr` は素の型)
fn run_err(e: &pyrun::RunErr) -> String {
    match e {
        pyrun::RunErr::Spawn(io) => format!("起こせません: {io}"),
        pyrun::RunErr::Timeout(s) => format!("{s} 秒で切りました(返事が来ません)"),
        pyrun::RunErr::Wait(m) => m.clone(),
    }
}

/// `codex` の居場所(PATH → よくある置き場)。`JO_CODEX` で名指しできる
pub fn codex_cli() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("JO_CODEX") {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let p = std::path::Path::new(dir).join("codex");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    for rel in [".local/bin/codex", ".npm-global/bin/codex", ".volta/bin/codex", ".bun/bin/codex"] {
        let p = home.join(rel);
        if p.is_file() {
            return Some(p);
        }
    }
    for p in ["/usr/local/bin/codex", "/opt/homebrew/bin/codex"] {
        let p = std::path::Path::new(p);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    None
}

/// Codex に頼む。**`codex exec` は最終の答えだけを stdout に出す**
/// (途中経過は stderr)。
///
/// 気をつけている所が3つある:
///
/// - **git のリポジトリを要求する**ので、作業用の空の置き場を作って
///   `git init` してから、そこを cwd にして呼ぶ
/// - **`--ephemeral`** — 会話の控えをディスクに残さない。人の文書が
///   知らない場所に溜まらないようにする
/// - 言いつけ(system)を渡す口が無いので、**頼みの頭に畳んで**渡す
///
/// **未検証**(2026-08-15): この機械に `codex` が無く、実機で通していない。
/// 通すときは `codex exec --ephemeral "こんにちは"` が答えを返すことを
/// 先に手で確かめること
fn codex_ask(system: &str, user: &str) -> Result<String, String> {
    let exe = codex_cli().ok_or("codex が見つかりません")?;
    let dir = std::env::temp_dir().join("officework-ai").join("codex");
    std::fs::create_dir_all(&dir).map_err(|e| format!("置き場を作れません: {e}"))?;
    // git のリポジトリでないと断られる。無ければ作る(空のままでよい)
    if !dir.join(".git").exists() {
        let mut g = std::process::Command::new("git");
        g.arg("init").arg("-q").current_dir(&dir);
        let _ = pyrun::run_with_timeout(&mut g, 30);
    }
    let prompt = if system.trim().is_empty() {
        user.to_string()
    } else {
        format!("{system}\n\n---\n{user}")
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("exec").arg("--ephemeral").current_dir(&dir);
    if let Ok(m) = std::env::var("JO_AI_MODEL") {
        if !m.is_empty() {
            cmd.arg("--model").arg(m);
        }
    }
    cmd.arg(&prompt);
    match pyrun::run_with_timeout(&mut cmd, 180) {
        Ok((true, out, _)) if !out.trim().is_empty() => Ok(out.trim().to_string()),
        Ok((true, _, _)) => Err("答えが空です".to_string()),
        Ok((false, _, err)) => {
            let last = err
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("理由が分かりません");
            Err(format!("codex: {last}"))
        }
        Err(e) => Err(format!("codex を動かせません: {}", run_err(&e))),
    }
}

/// Agent SDK(Python)が入っているか。入っていなければ入れ方を言う
fn agent_sdk_ready() -> Result<(), String> {
    let py = pyrun::find_python();
    let mut cmd = std::process::Command::new(py);
    cmd.arg("-c").arg("import claude_agent_sdk");
    match pyrun::run_with_timeout(&mut cmd, 30) {
        Ok((true, _, _)) => Ok(()),
        Ok((false, _, _)) => Err("Agent SDK が入っていません(pip install claude-agent-sdk \
             で入ります。実行体は SDK が同梱しているので、ほかに要る物は \
             ありません)"
            .to_string()),
        Err(e) => Err(format!("Python を動かせません: {}", run_err(&e))),
    }
}

/// いまの宛先が使えるか。使えないなら**理由を言う**(黙って空にしない)
pub fn ready(b: Backend) -> Result<(), String> {
    match b {
        Backend::Local => Ok(()),
        // **鍵を要る物にする。** Agent SDK は環境によっては他の資格情報も
        // 拾えるが、うちが提供するのは鍵での認証だけ(規約の言う
        // 「API key authentication」)。無ければここで断る
        // Codex は**その CLI が持っている認証**を使う(ChatGPT の契約でも
        // 動く)。うちはログインの口を作らない — 入れて `codex` で
        // 一度サインインしておくのは使う人の仕事
        Backend::OpenAiCodex => codex_cli().map(|_| ()).ok_or_else(|| {
            "codex が見つかりません(npm i -g @openai/codex で入ります)。\
             入れたあと一度 `codex` を起こしてサインインしてください。\
             置き場が変わっているなら JO_CODEX に道を教えてください"
                .to_string()
        }),
        Backend::ClaudeAgent => {
            if !std::env::var("ANTHROPIC_API_KEY").is_ok_and(|k| !k.is_empty()) {
                return Err("ANTHROPIC_API_KEY がありません(鍵は環境変数からだけ読みます \
                     — 文書にも設定にも書きません)"
                    .to_string());
            }
            agent_sdk_ready()
        }
        Backend::ClaudeApi => {
            if std::env::var("ANTHROPIC_API_KEY").is_ok_and(|k| !k.is_empty()) {
                Ok(())
            } else {
                Err("ANTHROPIC_API_KEY がありません(鍵は環境変数からだけ読みます \
                     — 文書にも設定にも書きません)"
                    .to_string())
            }
        }
    }
}

/// 頼む。返るのは本文だけ。**失敗は必ず言葉で返す**
pub fn ask(b: Backend, system: &str, user: &str) -> Result<String, String> {
    ready(b)?;
    match b {
        Backend::Local => {
            let ep = Endpoint::default();
            model::chat(&ep, system, user, 0.2).map(|r| r.content)
        }
        Backend::ClaudeAgent => agent_sdk_ask(system, user),
        Backend::OpenAiCodex => codex_ask(system, user),
        Backend::ClaudeApi => claude_api_ask(system, user),
    }
}

/// Agent SDK(Python)に頼む。**文書の中身はコマンド行に載せない** —
/// 一時のファイルに JSON で置いて径路だけ渡し、終わったら消す
fn agent_sdk_ask(system: &str, user: &str) -> Result<String, String> {
    let dir = std::env::temp_dir().join("officework-ai");
    std::fs::create_dir_all(&dir).map_err(|e| format!("置き場を作れません: {e}"))?;
    // 同じ名前で重ならないように、頼みの指紋を混ぜる
    let mut h = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    (system, user).hash(&mut h);
    let spec = dir.join(format!("ask-{:x}.json", h.finish()));
    let bridge = dir.join("bridge.py");
    let model = std::env::var("JO_AI_MODEL").unwrap_or_default();
    let body = format!(
        "{{\"system\":{},\"user\":{},\"model\":{}}}",
        format!("\"{}\"", model::esc(system)),
        format!("\"{}\"", model::esc(user)),
        format!("\"{}\"", model::esc(&model))
    );
    std::fs::write(&spec, body).map_err(|e| format!("頼みを置けません: {e}"))?;
    std::fs::write(&bridge, BRIDGE).map_err(|e| format!("橋を置けません: {e}"))?;
    let py = pyrun::find_python();
    let mut cmd = std::process::Command::new(py);
    cmd.arg(&bridge).arg(&spec);
    let r = pyrun::run_with_timeout(&mut cmd, 180);
    let _ = std::fs::remove_file(&spec);
    match r {
        Ok((true, out, _)) if !out.trim().is_empty() => Ok(out.trim().to_string()),
        Ok((true, _, _)) => Err("答えが空です".to_string()),
        Ok((false, _, err)) => {
            let last = err
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("理由が分かりません");
            Err(format!("Claude Agent: {last}"))
        }
        Err(e) => Err(format!("Agent SDK を動かせません: {}", run_err(&e))),
    }
}

/// Anthropic の API に頼む(鍵は環境変数からだけ)/// Anthropic の API に頼む(鍵は環境変数からだけ)
fn claude_api_ask(system: &str, user: &str) -> Result<String, String> {
    let key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| "鍵がありません")?;
    let m = std::env::var("JO_AI_MODEL").unwrap_or_else(|_| "claude-sonnet-5".into());
    let body = format!(
        r#"{{"model":"{}","max_tokens":4096,"system":"{}","messages":[{{"role":"user","content":"{}"}}]}}"#,
        model::esc(&m),
        model::esc(system),
        model::esc(user)
    );
    let raw = model::post_https(
        "api.anthropic.com",
        "/v1/messages",
        &body,
        &[
            ("x-api-key", key.as_str()),
            ("anthropic-version", "2023-06-01"),
        ],
    )?;
    // {"content":[{"type":"text","text":"…"}], …}
    model::extract_text_field(&raw, "text")
        .ok_or_else(|| format!("答えが読めません: {}", raw.chars().take(200).collect::<String>()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **廃した宛先は手元へ落とす。** 2026-08-15 に `claude-cli`(手元の
    /// claude コマンドに相乗りする道)をやめた。古い ai.txt を読んだときに
    /// 黙って別の外の宛先へ繋ぎ替えない — いちばん安全な手元へ倒す
    #[test]
    fn 廃した宛先は手元へ落ちる() {
        assert_eq!(Backend::from_str("claude-cli"), Backend::Local);
    }

    #[test]
    fn 宛先は覚えた形で戻る() {
        for b in [Backend::Local, Backend::ClaudeAgent, Backend::OpenAiCodex, Backend::ClaudeApi] {
            assert_eq!(Backend::from_str(b.as_str()), b);
        }
        // 知らない字はローカル(いちばん安全な側)へ倒す
        assert_eq!(Backend::from_str("なにこれ"), Backend::Local);
    }

    #[test]
    fn 宛先は順に回る() {
        let mut b = Backend::Local;
        for _ in 0..4 {
            b = b.next();
        }
        assert_eq!(b, Backend::Local, "4回で一周しない");
    }

    #[test]
    fn 使えない宛先は理由を言う() {
        // 鍵が無い環境では API は理由つきで断る(黙って空にしない)
        if std::env::var("ANTHROPIC_API_KEY").is_err() {
            let e = ready(Backend::ClaudeApi).unwrap_err();
            assert!(e.contains("ANTHROPIC_API_KEY"), "理由が薄い: {e}");
        }
        // Agent SDK も**鍵が要る**(うちが提供するのは鍵での認証だけ)。
        // 鍵が無ければ、SDK が入っているかを見るより先にそう言う
        if std::env::var("ANTHROPIC_API_KEY").is_err() {
            let e = ready(Backend::ClaudeAgent).unwrap_err();
            assert!(e.contains("ANTHROPIC_API_KEY"), "理由が薄い: {e}");
        }
        // Codex は鍵ではなく CLI を見る(その CLI が持つ認証を使う)
        if codex_cli().is_none() {
            let e = ready(Backend::OpenAiCodex).unwrap_err();
            assert!(e.contains("codex"), "理由が薄い: {e}");
        }
        assert!(ready(Backend::Local).is_ok(), "手元はいつでも頼める");
    }
}
