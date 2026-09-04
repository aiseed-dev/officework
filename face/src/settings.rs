//! 設定の器 — `~/.config/officework/settings.toml`(recent・sign.key の隣)。
//!
//! 優先順は **環境変数 > settings.toml > 既定**(現場の検証で一時的に
//! 差し替えたいときのため。SEKKEI「設定 — 器と言語」)。
//! writer と calc で1つのファイルを共有する。
//!
//! 読むのは素朴な `key = "value"` の行だけ(節 `[writer]` などは今は
//! 読み飛ばす)。依存を増やさない — この用途に TOML の全文法は要らない。
//!
//! ```toml
//! language = "en"   # リボンの言葉。ja(既定)か en
//! ```

use std::path::PathBuf;

/// 設定の置き場そのもの。**利用者の標準テンプレートもここ**です
/// (2026-08-26 発注者「ユーザーとしての標準設定は、HOME/~.config/
/// ディレクトリにおく」)。
pub fn dir() -> PathBuf {
    lang::config_dir()
}

/// settings.toml の置き場(recent・sign.key の隣)
pub fn path() -> PathBuf {
    lang::config_dir().join("settings.toml")
}

/// settings.toml から素朴に1つの鍵を読む(`key = "value"` の行)
pub fn get(key: &str) -> Option<String> {
    let s = std::fs::read_to_string(path()).ok()?;
    for line in s.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// 前置きで始まる鍵を全部読む(`key.bold = "ctrl-b"` の類)。
/// 返りは(前置きを剥いだ名前, 値)。並びはファイルの上から順
pub fn get_prefixed(prefix: &str) -> Vec<(String, String)> {
    let Ok(s) = std::fs::read_to_string(path()) else { return Vec::new() };
    let mut out = Vec::new();
    for line in s.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if let Some(name) = k.trim().strip_prefix(prefix) {
                out.push((name.to_string(), v.trim().trim_matches('"').to_string()));
            }
        }
    }
    out
}

/// settings.toml に1つの鍵を書く(他の行は保つ。無ければ行を足す)
/// **設定ファイルの宛先を環境変数へ移す**(起動のときに一度だけ)。
///
/// `lang::model::Endpoint` は環境変数だけを見る。lang は face を知らない
/// (依存の向きが逆)ので、橋渡しはこちら側でやる。
///
/// **環境変数が先**(その場の一時の上書き)。settings.toml は「いつもの
/// 宛先」で、環境変数が立っていれば触らない — 詳細設定の画面に書いてある
/// 約束をそのまま守る。
///
/// 読む鍵(2026-08-15 発注者「自分のサーバーにつながるようにしておけばいい」):
///
/// ```toml
/// ai_url = "https://ai.example.org/v1/chat/completions"
/// ai_model = "gpt-oss-120b"
/// ai_timeout = "120"
/// ```
///
/// **鍵(API キー)はここに書かない。** 環境変数 `OFFICE_API_KEY` から
/// だけ読む — 設定ファイルにも文書にも鍵は入れない、の決めのまま
pub fn ai_env_from_settings() {
    for (k, env) in [
        ("ai_url", "OFFICE_URL"),
        ("ai_model", "OFFICE_MODEL"),
        ("ai_timeout", "OFFICE_TIMEOUT"),
    ] {
        if std::env::var_os(env).is_some() {
            continue; // その場の上書きが先
        }
        if let Some(v) = get(k).filter(|s| !s.trim().is_empty()) {
            // SAFETY: 起動の初めに1度だけ、他の糸が走る前に呼ぶ
            unsafe { std::env::set_var(env, v.trim()) };
        }
    }
}

/// AI の宛先の1行(名前つきの一覧)。oh-my-pi の提供元の切り替えの
/// 蒸留 — 手元もクラウドも同じ一覧の行で、序列は付けない
/// (docs/sekkei/agent.ja.adoc「提供元の一覧」。2026-09-02 発注者)
#[derive(Debug, Clone, PartialEq)]
pub struct AiDest {
    pub name: String,
    pub url: String,
    pub model: String,
    /// 鍵の入っている**環境変数の名前**。鍵そのものは設定に書かない
    pub key_env: Option<String>,
}

/// 宛先の話し方
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiKind {
    /// OpenAI 互換の chat/completions(`lang::model`)
    Chat,
    /// 宛先「Claude Code」: 改変していない `claude` を子プロセスで
    /// (`agent::claude_code`。定額の道。url は `claude-code:`)
    ClaudeCode,
}

/// 宛先「Claude Code」の url の印(Endpoint にはならない)
pub const CLAUDE_CODE_URL: &str = "claude-code:";

impl AiDest {
    /// 話し方。url の印で見分ける(欄を増やさない — 設定の行の形はそのまま)
    pub fn kind(&self) -> AiKind {
        if self.url.starts_with(CLAUDE_CODE_URL) {
            AiKind::ClaudeCode
        } else {
            AiKind::Chat
        }
    }

    /// 宛先(Endpoint)にする。鍵は key_env の環境変数から読む。
    /// key_env が無ければ鍵なし — 他の行の鍵を黙って使い回さない
    pub fn endpoint(&self) -> lang::model::Endpoint {
        let mut ep = lang::model::Endpoint {
            host: "127.0.0.1".into(),
            // http で番号を省いたら 80(apply_url は https なら 443 にする)
            port: 80,
            path: "/v1/chat/completions".into(),
            model: self.model.clone(),
            api_key: self
                .key_env
                .as_ref()
                .and_then(|k| std::env::var(k).ok())
                .filter(|s| !s.is_empty()),
            timeout: std::time::Duration::from_secs(
                std::env::var("OFFICE_TIMEOUT").ok().and_then(|s| s.parse().ok()).unwrap_or(120),
            ),
            tls: false,
        };
        ep.apply_url(&self.url);
        ep
    }
}

/// settings.toml の `[[ai]]` の並びを読む。url の無い行は数えない。
/// 名前を省いた行は url のホスト名で呼ぶ
pub fn parse_ai_list(content: &str) -> Vec<AiDest> {
    let mut out: Vec<AiDest> = Vec::new();
    let mut cur: Option<AiDest> = None;
    let close = |cur: &mut Option<AiDest>, out: &mut Vec<AiDest>| {
        if let Some(d) = cur.take() {
            if !d.url.is_empty() {
                out.push(d);
            }
        }
    };
    for line in content.lines() {
        let t = line.trim();
        if t == "[[ai]]" {
            close(&mut cur, &mut out);
            cur = Some(AiDest {
                name: String::new(),
                url: String::new(),
                model: String::new(),
                key_env: None,
            });
            continue;
        }
        if t.starts_with('[') {
            // 別の節に入ったら、いまの行は締める
            close(&mut cur, &mut out);
            continue;
        }
        let Some(d) = cur.as_mut() else { continue };
        if t.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = t.split_once('=') {
            let v = v.trim().trim_matches('"').to_string();
            match k.trim() {
                "name" => d.name = v,
                "url" => d.url = v,
                "model" => d.model = v,
                "key_env" => d.key_env = Some(v).filter(|s| !s.is_empty()),
                _ => {}
            }
        }
    }
    close(&mut cur, &mut out);
    for d in &mut out {
        if d.name.is_empty() {
            d.name = host_of(&d.url);
        }
    }
    out
}

fn host_of(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split(['/', ':'])
        .next()
        .unwrap_or("")
        .to_string()
}

// ---- 提供元の表(oh-my-pi の蒸留。docs/sekkei/agent.ja.adoc「宛先を選ぶ」2026-09-04)----
//
// 鍵は環境変数の名前だけを持つ。変数が入っていれば、設定なしで宛先に出る。
// 手元のモデル(Ollama・llama.cpp・LM Studio)は港が開いていれば出る。
// 表は 10 行で始め、増やすのは頼まれてから(機能では勝負しない)。
// モデルの名前は 2026-09-04 に各社の公式の資料で確かめた既定。

/// 提供元の1行
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provider {
    pub id: &'static str,
    /// 一覧に出す名前
    pub name: &'static str,
    pub url: &'static str,
    /// 既定のモデル(手元の物は空 = 一覧から選ぶ)
    pub model: &'static str,
    /// 鍵の環境変数。None は鍵なし(手元の物)
    pub key_env: Option<&'static str>,
    /// 接続先を変える環境変数(手元の物。`OLLAMA_HOST` など)
    pub url_env: Option<&'static str>,
}

pub const PROVIDERS: &[Provider] = &[
    Provider { id: "anthropic", name: "Anthropic", url: "https://api.anthropic.com/v1/chat/completions", model: "claude-sonnet-5", key_env: Some("ANTHROPIC_API_KEY"), url_env: None },
    Provider { id: "openai", name: "OpenAI", url: "https://api.openai.com/v1/chat/completions", model: "gpt-5.6-terra", key_env: Some("OPENAI_API_KEY"), url_env: None },
    Provider { id: "google", name: "Google", url: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions", model: "gemini-3.8-flash", key_env: Some("GEMINI_API_KEY"), url_env: None },
    Provider { id: "openrouter", name: "OpenRouter", url: "https://openrouter.ai/api/v1/chat/completions", model: "openrouter/auto", key_env: Some("OPENROUTER_API_KEY"), url_env: None },
    Provider { id: "groq", name: "Groq", url: "https://api.groq.com/openai/v1/chat/completions", model: "openai/gpt-oss-120b", key_env: Some("GROQ_API_KEY"), url_env: None },
    Provider { id: "mistral", name: "Mistral", url: "https://api.mistral.ai/v1/chat/completions", model: "mistral-medium-latest", key_env: Some("MISTRAL_API_KEY"), url_env: None },
    Provider { id: "xai", name: "xAI", url: "https://api.x.ai/v1/chat/completions", model: "grok-4.6", key_env: Some("XAI_API_KEY"), url_env: None },
    Provider { id: "ollama", name: "Ollama", url: "http://127.0.0.1:11434/v1/chat/completions", model: "", key_env: None, url_env: Some("OLLAMA_HOST") },
    Provider { id: "llama.cpp", name: "llama.cpp", url: "http://127.0.0.1:8080/v1/chat/completions", model: "", key_env: None, url_env: Some("LLAMA_CPP_BASE_URL") },
    Provider { id: "lm-studio", name: "LM Studio", url: "http://127.0.0.1:1234/v1/chat/completions", model: "", key_env: None, url_env: Some("LM_STUDIO_BASE_URL") },
];

impl Provider {
    /// 宛先の行にする。手元の物は `url_env` があればそちらの接続先
    pub fn dest(&self) -> AiDest {
        let url = self
            .url_env
            .and_then(|k| std::env::var(k).ok())
            .filter(|s| !s.trim().is_empty())
            .map(|s| local_url(&s))
            .unwrap_or_else(|| self.url.to_string());
        AiDest { name: self.name.to_string(), url, model: self.model.to_string(), key_env: self.key_env.map(|k| k.to_string()) }
    }
}

/// `OLLAMA_HOST=127.0.0.1:11434` や `http://box:8080` を chat/completions の url に
fn local_url(s: &str) -> String {
    let s = s.trim().trim_end_matches('/');
    let with_scheme = if s.contains("://") { s.to_string() } else { format!("http://{s}") };
    if with_scheme.ends_with("/chat/completions") {
        with_scheme
    } else if with_scheme.ends_with("/v1") {
        format!("{with_scheme}/chat/completions")
    } else {
        format!("{with_scheme}/v1/chat/completions")
    }
}

/// 鍵の環境変数が入っている提供元(設定なしで出る)。`env` は試験のために差し替えられる
pub fn providers_with_keys_from(env: impl Fn(&str) -> Option<String>) -> Vec<AiDest> {
    PROVIDERS
        .iter()
        .filter(|p| p.key_env.is_some_and(|k| env(k).is_some_and(|v| !v.trim().is_empty())))
        .map(|p| p.dest())
        .collect()
}

pub fn providers_with_keys() -> Vec<AiDest> {
    providers_with_keys_from(|k| std::env::var(k).ok())
}

/// 手元のモデル(鍵の無い提供元)のうち、港が開いている物。1つ 1 秒で諦める。
/// 60 秒は控えを返す(描くたびに探しに行かない)
pub fn probe_local() -> Vec<AiDest> {
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    type Found = Option<(Instant, Vec<AiDest>)>;
    static CACHE: OnceLock<Mutex<Found>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Some((t, v)) = cache.lock().unwrap().as_ref() {
        if t.elapsed() < Duration::from_secs(60) {
            return v.clone();
        }
    }
    let found: Vec<AiDest> = PROVIDERS
        .iter()
        .filter(|p| p.key_env.is_none())
        .map(|p| p.dest())
        .filter(|d| port_open(&d.url, Duration::from_secs(1)))
        .collect();
    *cache.lock().unwrap() = Some((Instant::now(), found.clone()));
    found
}

/// url のホストの港が開いているか(TCP の接続だけ。HTTP は話さない)
fn port_open(url: &str, wait: std::time::Duration) -> bool {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let hostport = rest.split('/').next().unwrap_or("");
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(80)),
        None => (hostport, if url.starts_with("https") { 443 } else { 80 }),
    };
    use std::net::ToSocketAddrs;
    let Ok(mut addrs) = (host, port).to_socket_addrs() else { return false };
    addrs.any(|a| std::net::TcpStream::connect_timeout(&a, wait).is_ok())
}

/// 宛先「Claude Code」。`claude` が PATH にあれば出る(ログインの有無は
/// 状態の4語の側で見る)。有無は初回だけ確かめて控える
pub fn claude_code_dest() -> Option<AiDest> {
    use std::sync::OnceLock;
    static HAVE: OnceLock<bool> = OnceLock::new();
    let have = *HAVE.get_or_init(|| {
        std::process::Command::new("claude")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    });
    have.then(|| AiDest { name: "Claude Code".into(), url: CLAUDE_CODE_URL.into(), model: "sonnet".into(), key_env: None })
}

/// 宛先の一覧。並びは、自分で書いた `[[ai]]`(無ければ今までの `ai_url` /
/// `ai_model` を1行に。鍵は従来どおり OFFICE_API_KEY)→ 鍵の入っている
/// 提供元 → Claude Code。**手元のモデルは入れない**(港を探すのに時間が
/// 掛かるので、パネルを開く所が [`ai_list_all`] で足す)。
/// 空 = パネルの「未設定」
pub fn ai_list() -> Vec<AiDest> {
    let content = std::fs::read_to_string(path()).unwrap_or_default();
    let mut v = parse_ai_list(&content);
    if v.is_empty() {
        if let Some(url) = get("ai_url").filter(|s| !s.trim().is_empty()) {
            v.push(AiDest {
                name: host_of(&url),
                url,
                model: get("ai_model").unwrap_or_default(),
                key_env: Some("OFFICE_API_KEY".into()),
            });
        }
    }
    for d in providers_with_keys() {
        if !v.iter().any(|x| x.name == d.name || x.url == d.url) {
            v.push(d);
        }
    }
    if let Some(cc) = claude_code_dest() {
        if !v.iter().any(|x| x.kind() == AiKind::ClaudeCode) {
            v.push(cc);
        }
    }
    v
}

/// **`[[ai]]` の並びを書き直す**(2026-09-04。画面の「新しい宛先を足す」の道)。
/// 他の欄(`ai_last` や `language`)と節(`[writer]` など)は触らない。
/// 鍵そのものは書かない — 行が持つのは `key_env`(環境変数の名前)だけ
pub fn set_ai_list(rows: &[AiDest]) -> Result<(), String> {
    let p = path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let cur = std::fs::read_to_string(&p).unwrap_or_default();
    std::fs::write(&p, write_ai_list_into(&cur, rows)).map_err(|e| format!("{}: {e}", p.display()))
}

/// [`set_ai_list`] の芯(字だけ。試験のため)。今ある `[[ai]]` の塊を全部外し、
/// 最初の塊があった所(無ければ末尾)に新しい並びを置く
pub fn write_ai_list_into(content: &str, rows: &[AiDest]) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut kept: Vec<String> = Vec::new();
    let mut at: Option<usize> = None;
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == "[[ai]]" {
            if at.is_none() {
                at = Some(kept.len());
            }
            // 塊は次の節の頭(`[` で始まる行)まで。塊の直前の空行も1つ引く
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with('[') {
                i += 1;
            }
            while kept.last().is_some_and(|l| l.trim().is_empty()) && at == Some(kept.len()) {
                // 最初の塊の直前の空行は、新しい塊が自分で足す
                kept.pop();
                at = Some(kept.len());
            }
            continue;
        }
        kept.push(lines[i].to_string());
        i += 1;
    }
    let mut block: Vec<String> = Vec::new();
    for r in rows {
        if !block.is_empty() || !kept.is_empty() {
            block.push(String::new());
        }
        block.push("[[ai]]".into());
        block.push(format!("name = {}", toml_str(&r.name)));
        block.push(format!("url = {}", toml_str(&r.url)));
        block.push(format!("model = {}", toml_str(&r.model)));
        if let Some(k) = r.key_env.as_deref().filter(|k| !k.trim().is_empty()) {
            block.push(format!("key_env = {}", toml_str(k)));
        }
    }
    let at = at.unwrap_or(kept.len()).min(kept.len());
    // 塊の後ろに節が続くなら、空行で離す
    if at < kept.len() && !block.is_empty() && !kept[at].trim().is_empty() {
        block.push(String::new());
    }
    let mut out: Vec<String> = kept[..at].to_vec();
    out.extend(block);
    out.extend(kept[at..].iter().cloned());
    let mut s = out.join("\n");
    if !s.is_empty() && !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// TOML の基本の文字列。読む側(`parse_ai_list`)は逃がしを読まない素朴な
/// 作りなので、書く側も `"` と `\\` と改行を落とす(名前や URL に要らない字)
fn toml_str(s: &str) -> String {
    let body: String = s.chars().filter(|c| !matches!(c, '"' | '\\' | '\n' | '\r')).collect();
    format!("\"{body}\"")
}

/// [`ai_list`] に、港が開いている手元のモデルを足した物(パネルを開く時に)
pub fn ai_list_all() -> Vec<AiDest> {
    let mut v = ai_list();
    for d in probe_local() {
        if !v.iter().any(|x| x.url == d.url) {
            v.push(d);
        }
    }
    v
}

/// 最後に使った宛先の名前(既定の選び方: これ → 一覧の1番目)
pub fn ai_last() -> Option<String> {
    get("ai_last")
}

pub fn set_ai_last(name: &str) {
    set("ai_last", name);
}

pub fn set(key: &str, value: &str) {
    let p = path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let cur = std::fs::read_to_string(&p).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut done = false;
    for line in cur.lines() {
        let t = line.trim();
        if !done && !t.starts_with('#') && !t.starts_with('[') {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim() == key {
                    lines.push(format!("{key} = \"{value}\""));
                    done = true;
                    continue;
                }
            }
        }
        lines.push(line.to_string());
    }
    if !done {
        // 節([writer] や [[ai]])の前に入れる。末尾に足すと、
        // 節があるときはその節の中の鍵になってしまう
        let at = lines
            .iter()
            .position(|l| l.trim_start().starts_with('['))
            .unwrap_or(lines.len());
        lines.insert(at, format!("{key} = \"{value}\""));
    }
    let _ = std::fs::write(&p, lines.join("\n") + "\n");
}

/// リボンと文言の言語。実体は lang::i18n(1本道 — 表計算の関数や
/// 校正と同じ crate に置き、gpui を知らない層でも引けるように)
pub fn language() -> &'static str {
    lang::i18n::language()
}

#[cfg(test)]
mod tests {
    use super::{parse_ai_list, AiDest};

    /// 提供元の表: id は一意、鍵つきは https の chat/completions、手元は鍵なしで
    /// 接続先の変数を持つ
    #[test]
    fn the_provider_table_is_well_formed_and_keys_pick_rows() {
        use super::{providers_with_keys_from, AiKind, Provider, PROVIDERS, CLAUDE_CODE_URL};
        assert_eq!(PROVIDERS.len(), 10);
        let mut ids: Vec<&str> = PROVIDERS.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), PROVIDERS.len(), "id が重なっている");
        for p in PROVIDERS {
            assert!(p.url.ends_with("/chat/completions"), "{}: {}", p.id, p.url);
            match p.key_env {
                Some(k) => {
                    assert!(p.url.starts_with("https://"), "{}: 鍵つきは https", p.id);
                    assert!(k.ends_with("_API_KEY"), "{}: {k}", p.id);
                    assert!(!p.model.is_empty(), "{}: 既定のモデルが無い", p.id);
                }
                None => {
                    assert!(p.url.starts_with("http://127.0.0.1:"), "{}: 手元は 127.0.0.1", p.id);
                    assert!(p.url_env.is_some(), "{}: 接続先の変数が無い", p.id);
                }
            }
        }
        // 鍵の変数が入っている物だけが出る
        let got = providers_with_keys_from(|k| (k == "GROQ_API_KEY" || k == "XAI_API_KEY").then(|| "x".to_string()));
        assert_eq!(got.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), vec!["Groq", "xAI"]);
        assert_eq!(got[0].key_env.as_deref(), Some("GROQ_API_KEY"));
        assert!(providers_with_keys_from(|_| None).is_empty());
        // 空の値は入っていない扱い
        assert!(providers_with_keys_from(|_| Some(String::new())).is_empty());
        // 話し方の見分け
        let cc = AiDest { name: "Claude Code".into(), url: CLAUDE_CODE_URL.into(), model: "sonnet".into(), key_env: None };
        assert_eq!(cc.kind(), AiKind::ClaudeCode);
        assert_eq!(PROVIDERS[0].dest().kind(), AiKind::Chat);
        let _: Provider = PROVIDERS[7];
    }

    /// `[[ai]]` の書き直し: 他の欄と節は残り、鍵そのものは書かず、読み戻せる
    #[test]
    fn rewriting_the_ai_list_keeps_everything_else_and_round_trips() {
        use super::{write_ai_list_into, AiDest};
        let before = "language = \"ja\"\nai_last = \"手元\"\n\n[[ai]]\nname = \"手元\"\nurl = \"http://127.0.0.1:8000/v1/chat/completions\"\nmodel = \"local\"\n\n[[ai]]\nurl = \"https://ai.example.org/v1/chat/completions\"\nmodel = \"gpt-oss-120b\"\nkey_env = \"OFFICE_API_KEY_KAI\"\n\n[writer]\ntheme = \"dark\"\n";
        let rows = vec![
            AiDest { name: "Claude".into(), url: "https://api.anthropic.com/v1/chat/completions".into(), model: "claude-sonnet-5".into(), key_env: Some("OFFICE_API_KEY_CLAUDE".into()) },
            AiDest { name: "会の箱".into(), url: "http://box:8080/v1/chat/completions".into(), model: "".into(), key_env: None },
        ];
        let after = write_ai_list_into(before, &rows);
        assert!(after.starts_with("language = \"ja\"\nai_last = \"手元\"\n"), "{after}");
        assert!(after.ends_with("[writer]\ntheme = \"dark\"\n"), "{after}");
        assert_eq!(after.matches("[[ai]]").count(), 2);
        assert!(!after.contains("gpt-oss-120b"), "古い行が残った: {after}");
        assert!(!after.contains("sk-"), "鍵らしき物を書いた");
        let back = parse_ai_list(&after);
        assert_eq!(back, rows, "{after}");
        // 何も無いファイルには末尾に置く。空の並びは塊を全部消す
        assert_eq!(parse_ai_list(&write_ai_list_into("", &rows)), rows);
        let none = write_ai_list_into(&after, &[]);
        assert!(!none.contains("[[ai]]") && none.contains("[writer]"), "{none}");
    }

    #[test]
    fn a_local_host_setting_becomes_a_chat_url() {
        use super::local_url;
        assert_eq!(local_url("127.0.0.1:11434"), "http://127.0.0.1:11434/v1/chat/completions");
        assert_eq!(local_url("http://box:8080/v1"), "http://box:8080/v1/chat/completions");
        assert_eq!(local_url("http://box:8080/v1/chat/completions/"), "http://box:8080/v1/chat/completions");
    }

    #[test]
    fn a_closed_port_is_not_open() {
        // 9 番(discard)は普通閉じている。1秒で諦めるので試験は長くならない
        assert!(!super::port_open("http://127.0.0.1:9/v1/chat/completions", std::time::Duration::from_millis(300)));
    }

    #[test]
    fn the_ai_list_reads_named_destinations() {
        let toml = r#"
language = "ja"

[[ai]]
name = "手元"
url = "http://127.0.0.1:8000/v1/chat/completions"
model = "local"

[[ai]]
url = "https://ai.example.org/v1/chat/completions"
model = "gpt-oss-120b"
key_env = "OFFICE_API_KEY_KAI"

[writer]
theme = "dark"
"#;
        let v = parse_ai_list(toml);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].name, "手元");
        assert_eq!(v[0].model, "local");
        assert_eq!(v[0].key_env, None, "鍵の指定が無ければ鍵なし");
        // 名前を省いたらホスト名で呼ぶ
        assert_eq!(v[1].name, "ai.example.org");
        assert_eq!(v[1].key_env.as_deref(), Some("OFFICE_API_KEY_KAI"));
    }

    #[test]
    fn a_destination_becomes_an_endpoint() {
        let d = AiDest {
            name: "会".into(),
            url: "https://ai.example.org/v1/chat/completions".into(),
            model: "m".into(),
            key_env: Some("KEY_THAT_IS_NOT_SET_ANYWHERE".into()),
        };
        let ep = d.endpoint();
        assert!(ep.tls);
        assert_eq!(ep.host, "ai.example.org");
        assert_eq!(ep.port, 443, "https で番号を省いたら 443");
        assert_eq!(ep.model, "m");
        assert_eq!(ep.api_key, None, "環境変数が無ければ鍵なし");
        let d = AiDest {
            name: "手元".into(),
            url: "http://127.0.0.1:8000/v1/chat/completions".into(),
            model: "local".into(),
            key_env: None,
        };
        let ep = d.endpoint();
        assert!(!ep.tls);
        assert_eq!(ep.port, 8000);
    }

    #[test]
    fn a_url_without_ai_blocks_yields_nothing() {
        assert!(parse_ai_list("language = \"ja\"\n").is_empty());
        // url の無い行は数えない
        assert!(parse_ai_list("[[ai]]\nname = \"名前だけ\"\n").is_empty());
    }

    /// **言語を替えると、リボンの表も替わる。**
    ///
    /// 前はここで `OFFICE_LANG` を立てて `language()` を呼び、*この試験
    /// プロセスで最初に呼ぶのがここである*ことに頼っていました。だから
    /// `tabs.rs` の4本は `#[ignore]` を付けて単独で回すことになっていました。
    ///
    /// いまは `set_language` がいつ呼んでも効くので(2026-08-19 の決め)、
    /// 順番に頼らずに書けます。環境変数から決める所そのものは
    /// `lang::i18n` の「言語の決め方」で検べています — 控えを直に触れる
    /// のはあちらだけなので、置き場を分けました。
    ///
    /// **必ず ja に戻します。** 戻さないと、後から走る試験が別の言語で
    /// 表を引いて落ちます。
    #[test]
    fn changing_the_language_changes_the_ribbon_words() {
        // **言語は処理系に1つ。** 読む試験と並ぶと崩れるので錠を取ります
        let _lang = lang::i18n::lang_lock();
        assert!(lang::i18n::set_language("en"));
        assert_eq!(super::language(), "en");
        assert_eq!(crate::ribbon::writer_tabs()[1].name, "Home");
        assert_eq!(crate::ribbon::calc_tabs()[1].name, "Home");

        assert!(lang::i18n::set_language("ja"));
        assert_eq!(crate::ribbon::writer_tabs()[1].name, "ホーム");
        assert_eq!(crate::ribbon::calc_tabs()[1].name, "ホーム");
        // **日本語に戻してから放します** — 錠を放した後に他が読みます
        assert!(lang::i18n::set_language("ja"));
    }
}
