//! モデルへの問い合わせ。OpenAI互換の `/v1/chat/completions` だけを話す。
//!
//! 宛先はローカル(既定 127.0.0.1)。**基幹網は外に出ない**。
//! Radeon Cloud の専用インスタンスも、SSH のポート転送で 127.0.0.1 に見えるので
//! **同じ経路で届く** — アプリ側の書き換えは要らない。
//!
//! TLS は持たない。素の HTTP だけを話す。
//! HTTPS の宛先(共有API など)へは SSH 転送か手元の中継を挟む。
//! **暗号の実装を自前で抱えないための割り切り**であって、機能不足ではない。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// 宛先。すべて実行時に差し替えられる — **差し替えるのはモデルであって、コードではない**。
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub path: String,
    pub model: String,
    /// Radeon Cloud の専用エンドポイントは Bearer を要求する
    pub api_key: Option<String>,
    pub timeout: Duration,
    /// **暗号を掛けるか。** 手元(127.0.0.1)なら要らないが、
    /// **会のサーバーへ出るなら要る** — 素の TCP では文書が丸見えになる
    /// (2026-08-15 発注者「自分のサーバーにつながるように」)
    pub tls: bool,
}

impl Default for Endpoint {
    /// **`OFFICE_URL` が1本あればそれで足りる。** 人に4つ(host/port/path/
    /// model)を書かせるのは酷なので、`https://ai.example.org/v1/chat/completions`
    /// のような1行を先に見る。無ければ今までの4つに落ちる(後方互換)
    fn default() -> Self {
        let mut ep = Self {
            host: var("OFFICE_HOST", "127.0.0.1"),
            port: std::env::var("OFFICE_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(8000),
            path: var("OFFICE_PATH", "/v1/chat/completions"),
            model: var("OFFICE_MODEL", "local"),
            api_key: std::env::var("OFFICE_API_KEY").ok().filter(|s| !s.is_empty()),
            timeout: Duration::from_secs(
                std::env::var("OFFICE_TIMEOUT").ok().and_then(|s| s.parse().ok()).unwrap_or(120),
            ),
            tls: std::env::var("OFFICE_TLS").is_ok_and(|v| v == "1" || v == "true"),
        };
        if let Ok(u) = std::env::var("OFFICE_URL") {
            if !u.trim().is_empty() {
                ep.apply_url(u.trim());
            }
        }
        ep
    }
}

impl Endpoint {
    /// `https://host:port/path` を読んで宛先に写す。**読めない字は黙って
    /// 捨てない** — 読めた所だけ当てて、残りは今までの値のままにする
    pub fn apply_url(&mut self, u: &str) {
        let (scheme, rest) = match u.split_once("://") {
            Some((s, r)) => (s.to_ascii_lowercase(), r),
            None => (String::new(), u),
        };
        if scheme == "https" {
            self.tls = true;
        } else if scheme == "http" {
            self.tls = false;
        }
        let (hostport, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        let (host, port) = match hostport.rsplit_once(':') {
            Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => {
                (h, p.parse().ok())
            }
            _ => (hostport, None),
        };
        if !host.is_empty() {
            self.host = host.to_string();
        }
        self.port = port.unwrap_or(if self.tls { 443 } else { self.port });
        if !path.is_empty() {
            self.path = path.to_string();
        }
    }

    /// 人に見せる宛先(鍵は出さない)
    pub fn shown(&self) -> String {
        format!(
            "{}://{}:{}{} / {}",
            if self.tls { "https" } else { "http" },
            self.host,
            self.port,
            self.path,
            self.model
        )
    }

    /// 手元だけで完結しているか(外に出ないと言ってよいか)
    pub fn is_local(&self) -> bool {
        matches!(self.host.as_str(), "127.0.0.1" | "localhost" | "::1")
    }
}

fn var(k: &str, d: &str) -> String {
    std::env::var(k).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| d.into())
}

/// 1回のやりとりの結果。速度の計測に使うので、**トークン数と所要時間を必ず持ち帰る**。
#[derive(Debug, Clone, Default)]
pub struct Reply {
    pub content: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub elapsed_ms: u128,
}

impl Reply {
    /// 生成側の tok/s。時間が 0 のときは 0 を返す(割り算で落とさない)
    pub fn tokens_per_sec(&self) -> f64 {
        if self.elapsed_ms == 0 {
            return 0.0;
        }
        self.completion_tokens as f64 * 1000.0 / self.elapsed_ms as f64
    }
}

/// JSON の文字列として安全にする。
pub fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => {}
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// system と user を投げて、返事を受け取る。
///
/// **繋がらなければ Err**。黙って空を返さない —
/// 「指摘なし」「ふりがな不要」と読み違えられるため。
pub fn chat(ep: &Endpoint, system: &str, user: &str, temperature: f32) -> Result<Reply, String> {
    let body = format!(
        r#"{{"model":"{}","temperature":{},"messages":[{{"role":"system","content":"{}"}},{{"role":"user","content":"{}"}}]}}"#,
        esc(&ep.model),
        temperature,
        esc(system),
        esc(user)
    );
    let t0 = std::time::Instant::now();
    let raw = post(ep, &body)?;
    let elapsed_ms = t0.elapsed().as_millis();
    let content = extract_content(&raw)
        .ok_or_else(|| format!("モデルの応答を読めません: {}", head(&raw, 200)))?;
    Ok(Reply {
        content,
        prompt_tokens: usage(&raw, "prompt_tokens"),
        completion_tokens: usage(&raw, "completion_tokens"),
        elapsed_ms,
    })
}

fn head(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn post(ep: &Endpoint, body: &str) -> Result<String, String> {
    // **外へ出すなら暗号を掛ける。** 手元の 127.0.0.1 は素のままでよいが、
    // 会のサーバーへ出す文書を平文で流さない(2026-08-15)
    if ep.tls {
        return post_tls(ep, body);
    }
    let addr = format!("{}:{}", ep.host, ep.port);
    let mut s = TcpStream::connect(&addr)
        .map_err(|e| format!("モデルに繋がりません({addr}): {e}"))?;
    s.set_read_timeout(Some(ep.timeout)).ok();
    let auth = match &ep.api_key {
        Some(k) => format!("Authorization: Bearer {k}\r\n"),
        None => String::new(),
    };
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\n{}\
         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        ep.path,
        ep.host,
        auth,
        body.len(),
        body
    );
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(s);
    let mut status = String::new();
    r.read_line(&mut status).map_err(|e| e.to_string())?;
    // ヘッダを読み飛ばす
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line).map_err(|e| e.to_string())? == 0 || line.trim().is_empty() {
            break;
        }
    }
    let mut out = String::new();
    r.read_to_string(&mut out).map_err(|e| e.to_string())?;
    // 4xx/5xx を成功として扱わない
    if !status.contains(" 200") {
        return Err(format!("モデルが拒否しました: {} / {}", status.trim(), head(&out, 200)));
    }
    Ok(out)
}

/// 宛先の港へ TLS で POST する(`Endpoint::tls`)。[`post_https`] は
/// Anthropic 専用で 443 決め打ちなので、会のサーバー向けに港を選べる形を
/// 別に持つ。**組み方に https が入っていなければ、そう言って断る**
#[cfg(feature = "ai")]
fn post_tls(ep: &Endpoint, body: &str) -> Result<String, String> {
    let sock = TcpStream::connect((ep.host.as_str(), ep.port))
        .map_err(|e| format!("モデルに繋がりません({}:{}): {e}", ep.host, ep.port))?;
    sock.set_read_timeout(Some(ep.timeout)).ok();
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let cfg = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(ep.host.clone())
        .map_err(|_| format!("ホスト名が変です: {}", ep.host))?;
    let conn = rustls::ClientConnection::new(std::sync::Arc::new(cfg), name)
        .map_err(|e| e.to_string())?;
    let mut st = rustls::StreamOwned::new(conn, sock);
    let auth = match &ep.api_key {
        Some(k) => format!("Authorization: Bearer {k}\r\n"),
        None => String::new(),
    };
    let req = format!(
        "POST {} HTTP/1.0\r\nHost: {}\r\nContent-Type: application/json\r\n{}\
         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        ep.path,
        ep.host,
        auth,
        body.len(),
        body
    );
    st.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = String::new();
    st.read_to_string(&mut raw).map_err(|e| e.to_string())?;
    let (head_, body_) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| "答えの形が変です".to_string())?;
    let status = head_.lines().next().unwrap_or("");
    if !status.contains(" 200") {
        return Err(format!("モデルが拒否しました: {} / {}", status.trim(), head(body_, 200)));
    }
    Ok(body_.to_string())
}

/// 組み方に https が入っていないとき(スマホの的)
#[cfg(not(feature = "ai"))]
fn post_tls(_ep: &Endpoint, _body: &str) -> Result<String, String> {
    Err("この組み方に https は入っていません(feature \"ai\" なし)。\
         暗号なしでよければ宛先を http:// にしてください"
        .to_string())
}

/// https で POST して本文を返す(AI の宛先が外のときだけ使う)。
/// **鍵は呼ぶ側が渡す** — この関数はどこにも控えない
#[cfg(feature = "ai")]
pub fn post_https(
    host: &str,
    path: &str,
    body: &str,
    headers: &[(&str, &str)],
) -> Result<String, String> {
    use std::io::{BufRead as _, Read as _, Write as _};
    let sock = TcpStream::connect((host, 443))
        .map_err(|e| format!("繋がりません({host}): {e}"))?;
    sock.set_read_timeout(Some(Duration::from_secs(120))).ok();
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let cfg = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|_| format!("ホスト名が変です: {host}"))?;
    let conn = rustls::ClientConnection::new(std::sync::Arc::new(cfg), name)
        .map_err(|e| e.to_string())?;
    let mut st = rustls::StreamOwned::new(conn, sock);
    let mut req = format!(
        "POST {path} HTTP/1.0\r\nHost: {host}\r\nContent-Type: application/json\r\n"
    );
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ));
    st.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(st);
    let mut status = String::new();
    r.read_line(&mut status).map_err(|e| e.to_string())?;
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line).map_err(|e| e.to_string())? == 0 || line.trim().is_empty() {
            break;
        }
    }
    let mut out = String::new();
    r.read_to_string(&mut out).map_err(|e| e.to_string())?;
    if !status.contains(" 200") {
        return Err(format!("{}: {}", status.trim(), head(&out, 200)));
    }
    Ok(out)
}

/// 同じ名前の断り(feature "ai" を外した組み方 — スマホなど)。
/// **黙って空を返さない** — 「指摘なし」と読み違えられるため。
/// 形が本物と同じなので、呼ぶ側(ai.rs)は組み方を知らなくてよい
#[cfg(not(feature = "ai"))]
pub fn post_https(
    host: &str,
    _path: &str,
    _body: &str,
    _headers: &[(&str, &str)],
) -> Result<String, String> {
    Err(format!(
        "この組み方に https は入っていません(feature \"ai\" なし)。\
         外の宛先({host})へは出られません — ローカルのモデルは使えます"
    ))
}

// ---------- 道具つきの対話(エージェントパネルのループが使う) ----------

/// 道具の名乗り。`parameters` は JSON Schema の字をそのまま持つ
/// (書くのはこちらのコードなので、形の検査は呼ぶ側の試験で足りる)
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// 例: `{"type":"object","properties":{"a1":{"type":"string"}},"required":["a1"]}`
    pub parameters: String,
}

/// モデルからの道具呼び(OpenAI 互換の tool_calls の1つ)
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// 引数(JSON の字)。読むのは道具の側
    pub arguments: String,
}

/// 対話の1つの発言。履歴はこの並びで持ち、毎回まるごと送る
#[derive(Debug, Clone)]
pub enum Msg {
    System(String),
    User(String),
    Assistant(String),
    /// モデルが道具を呼んだ番。**そのまま履歴に残して次で送り返す**
    /// (送り返さないと、モデルは何を呼んだか思い出せない)
    AssistantCalls(Vec<ToolCall>),
    /// 道具の結果。`id` はどの呼びへの答えかを指す
    ToolResult { id: String, content: String },
}

/// 道具つきの1往復の答え。道具呼びが無ければ `content` が答えの字
#[derive(Debug, Clone, Default)]
pub struct ChatOut {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub elapsed_ms: u128,
}

fn msg_json(m: &Msg) -> String {
    match m {
        Msg::System(s) => format!(r#"{{"role":"system","content":"{}"}}"#, esc(s)),
        Msg::User(s) => format!(r#"{{"role":"user","content":"{}"}}"#, esc(s)),
        Msg::Assistant(s) => format!(r#"{{"role":"assistant","content":"{}"}}"#, esc(s)),
        Msg::AssistantCalls(calls) => {
            let cc: Vec<String> = calls
                .iter()
                .map(|c| {
                    format!(
                        r#"{{"id":"{}","type":"function","function":{{"name":"{}","arguments":"{}"}}}}"#,
                        esc(&c.id),
                        esc(&c.name),
                        esc(&c.arguments)
                    )
                })
                .collect();
            format!(r#"{{"role":"assistant","content":null,"tool_calls":[{}]}}"#, cc.join(","))
        }
        Msg::ToolResult { id, content } => format!(
            r#"{{"role":"tool","tool_call_id":"{}","content":"{}"}}"#,
            esc(id),
            esc(content)
        ),
    }
}

fn tools_json(tools: &[ToolDef]) -> String {
    if tools.is_empty() {
        return String::new();
    }
    let tt: Vec<String> = tools
        .iter()
        .map(|t| {
            format!(
                r#"{{"type":"function","function":{{"name":"{}","description":"{}","parameters":{}}}}}"#,
                esc(&t.name),
                esc(&t.description),
                t.parameters
            )
        })
        .collect();
    format!(r#","tools":[{}]"#, tt.join(","))
}

fn tools_body(ep: &Endpoint, msgs: &[Msg], tools: &[ToolDef], temperature: f32) -> String {
    let mm: Vec<String> = msgs.iter().map(msg_json).collect();
    format!(
        r#"{{"model":"{}","temperature":{},"messages":[{}]{}}}"#,
        esc(&ep.model),
        temperature,
        mm.join(","),
        tools_json(tools)
    )
}

/// `"鍵": [ … ]` の中身を切り出す(文字列の中の記号は数えない)。
/// 値が `null` や数値なら None — 「無い」と同じに扱う
fn array_after<'a>(raw: &'a str, key: &str) -> Option<&'a str> {
    let i = raw.find(&format!("\"{key}\""))?;
    let rest = &raw[i + key.len() + 2..];
    let c = rest.find(':')?;
    let after = rest[c + 1..].trim_start();
    if !after.starts_with('[') {
        return None;
    }
    let (mut depth, mut in_str, mut esc_next) = (0i32, false, false);
    for (j, ch) in after.char_indices() {
        if in_str {
            if esc_next {
                esc_next = false;
            } else if ch == '\\' {
                esc_next = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[1..j]);
                }
            }
            _ => {}
        }
    }
    None
}

/// `"鍵": "…"` の値を取り出す。値が `null` なら None
/// ([`extract_content`] は `null` の次の引用符へ食いつくので、
/// 道具呼びの応答にはこちらを使う)
fn quoted_after(raw: &str, key: &str) -> Option<String> {
    let i = raw.find(&format!("\"{key}\""))?;
    let rest = &raw[i + key.len() + 2..];
    let c = rest.find(':')?;
    let after = rest[c + 1..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut it = after[1..].chars();
    while let Some(ch) = it.next() {
        match ch {
            '"' => return Some(out),
            '\\' => match it.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => {}
                'u' => {
                    let h: String = it.by_ref().take(4).collect();
                    if let Some(ch) = u32::from_str_radix(&h, 16).ok().and_then(char::from_u32) {
                        out.push(ch);
                    }
                }
                o => out.push(o),
            },
            o => out.push(o),
        }
    }
    None
}

/// OpenAI 互換の応答から content と tool_calls を読む
fn parse_chat_out(raw: &str, elapsed_ms: u128) -> Result<ChatOut, String> {
    let tool_calls: Vec<ToolCall> = array_after(raw, "tool_calls")
        .map(|seg| {
            objects(seg)
                .iter()
                .map(|o| ToolCall {
                    id: field(o, "id").unwrap_or_default(),
                    name: field(o, "name").unwrap_or_default(),
                    arguments: field(o, "arguments").unwrap_or_else(|| "{}".into()),
                })
                .collect()
        })
        .unwrap_or_default();
    let content = quoted_after(raw, "content").unwrap_or_default();
    if content.is_empty() && tool_calls.is_empty() {
        return Err(format!("モデルの応答を読めません: {}", head(raw, 200)));
    }
    Ok(ChatOut {
        content,
        tool_calls,
        prompt_tokens: usage(raw, "prompt_tokens"),
        completion_tokens: usage(raw, "completion_tokens"),
        elapsed_ms,
    })
}

/// 履歴と道具の一覧を投げて、答えか道具呼びを受け取る。
/// **繋がらなければ Err**(chat と同じ理由 — 黙って空を返さない)
pub fn chat_tools(
    ep: &Endpoint,
    msgs: &[Msg],
    tools: &[ToolDef],
    temperature: f32,
) -> Result<ChatOut, String> {
    let body = tools_body(ep, msgs, tools, temperature);
    let t0 = std::time::Instant::now();
    let raw = post(ep, &body)?;
    parse_chat_out(&raw, t0.elapsed().as_millis())
}

/// JSON から `"鍵":"…"` の値を1つ取り出す(完全な処理系は持たない)。
/// Claude の応答は content[0].text なので、text を引けば足りる
pub fn extract_text_field(raw: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = raw.find(&pat)?;
    let rest = &raw[i + pat.len()..];
    let q = rest.find('"')?;
    let mut out = String::new();
    let mut it = rest[q + 1..].chars();
    while let Some(c) = it.next() {
        match c {
            '"' => return Some(out),
            '\\' => match it.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => {}
                'u' => {
                    let h: String = it.by_ref().take(4).collect();
                    if let Some(ch) =
                        u32::from_str_radix(&h, 16).ok().and_then(char::from_u32)
                    {
                        out.push(ch);
                    }
                }
                o => out.push(o),
            },
            o => out.push(o),
        }
    }
    None
}

/// OpenAI互換の応答から content を取り出す(JSON の完全な処理系は持たない)。
pub fn extract_content(raw: &str) -> Option<String> {
    let key = "\"content\"";
    let i = raw.find(key)?;
    let rest = &raw[i + key.len()..];
    let q = rest.find('"')?;
    let mut out = String::new();
    let mut it = rest[q + 1..].chars();
    while let Some(c) = it.next() {
        match c {
            '"' => return Some(out),
            '\\' => match it.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => {}
                'u' => {
                    let h: String = it.by_ref().take(4).collect();
                    if let Some(ch) = u32::from_str_radix(&h, 16).ok().and_then(char::from_u32) {
                        out.push(ch);
                    }
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

/// usage の数値を拾う。無ければ 0(速度の計算側で 0 除算しないこと)。
pub fn usage(raw: &str, field: &str) -> u64 {
    let k = format!("\"{field}\"");
    let Some(i) = raw.find(&k) else { return 0 };
    raw[i + k.len()..]
        .trim_start_matches(|c: char| c == ':' || c.is_whitespace())
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// JSON っぽい塊の中から、指定した名前の文字列を取り出す。
pub fn field(obj: &str, name: &str) -> Option<String> {
    let k = format!("\"{name}\"");
    let i = obj.find(&k)?;
    let rest = &obj[i + k.len()..];
    let c = rest.find(':')?;
    let q = rest[c..].find('"')? + c;
    let mut s = String::new();
    let mut it = rest[q + 1..].chars();
    while let Some(ch) = it.next() {
        match ch {
            '"' => return Some(s),
            '\\' => {
                if let Some(n) = it.next() {
                    s.push(if n == 'n' { '\n' } else { n })
                }
            }
            ch => s.push(ch),
        }
    }
    None
}

/// 最上位の `{...}` を順に切り出す(入れ子は数える)。
pub fn objects(content: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0usize, 0usize);
    let mut in_str = false;
    let mut esc_next = false;
    for (i, ch) in content.char_indices() {
        if in_str {
            if esc_next {
                esc_next = false;
            } else if ch == '\\' {
                esc_next = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            '{' => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    out.push(&content[start..=i]);
                }
            }
            _ => {}
        }
    }
    out
}

/// JSON の文字列配列 `["あ","い"]` を読む。順序を保つ — **候補は順序が意味を持つ**。
pub fn string_array(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if in_str {
            match c {
                '"' => {
                    out.push(std::mem::take(&mut cur));
                    in_str = false;
                }
                '\\' => {
                    if let Some(n) = it.next() {
                        cur.push(if n == 'n' { '\n' } else { n })
                    }
                }
                c => cur.push(c),
            }
        } else if c == '"' {
            in_str = true;
        } else if c == ']' {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_content_from_response() {
        let raw = r#"{"id":"x","choices":[{"message":{"role":"assistant","content":"[{\"found\":\"以外\"}]"}}]}"#;
        assert_eq!(extract_content(raw).unwrap(), r#"[{"found":"以外"}]"#);
    }

    #[test]
    fn picks_up_usage() {
        let raw = r#"{"usage":{"prompt_tokens":1234,"completion_tokens": 56}}"#;
        assert_eq!(usage(raw, "prompt_tokens"), 1234);
        assert_eq!(usage(raw, "completion_tokens"), 56);
        assert_eq!(usage(raw, "無い名前"), 0, "無ければ0(速度計算で落とさない)");
    }

    #[test]
    fn speed_survives_zero_elapsed() {
        let r = Reply { completion_tokens: 100, elapsed_ms: 0, ..Default::default() };
        assert_eq!(r.tokens_per_sec(), 0.0);
        let r = Reply { completion_tokens: 100, elapsed_ms: 1000, ..Default::default() };
        assert!((r.tokens_per_sec() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn not_fooled_by_braces_in_strings() {
        // 本文に { } が入っていても塊の切り出しが壊れない
        let c = r#"[{"base":"式","readings":["しき"],"note":"{ } を含む"}]"#;
        let o = objects(c);
        assert_eq!(o.len(), 1, "塊を1つに切り出せていない: {o:?}");
        assert_eq!(field(o[0], "base").unwrap(), "式");
    }

    #[test]
    fn candidate_array_keeps_order() {
        let v = string_array(r#"["のち","あと","うし","ご"]"#);
        assert_eq!(v, vec!["のち", "あと", "うし", "ご"], "候補は順序が意味を持つ");
    }

    #[test]
    fn the_tool_body_carries_history_and_tools() {
        let ep = Endpoint { model: "m".into(), ..Default::default() };
        let msgs = [
            Msg::System("s".into()),
            Msg::User("u".into()),
            Msg::AssistantCalls(vec![ToolCall {
                id: "c1".into(),
                name: "read_range".into(),
                arguments: r#"{"a1":"A1:B2"}"#.into(),
            }]),
            Msg::ToolResult { id: "c1".into(), content: "1\t2".into() },
        ];
        let tools = [ToolDef {
            name: "read_range".into(),
            description: "範囲を読む".into(),
            parameters: r#"{"type":"object","properties":{"a1":{"type":"string"}}}"#.into(),
        }];
        let b = tools_body(&ep, &msgs, &tools, 0.0);
        assert!(b.contains(r#""role":"tool","tool_call_id":"c1""#), "{b}");
        assert!(b.contains(r#""tool_calls":[{"id":"c1""#), "{b}");
        assert!(b.contains(r#""arguments":"{\"a1\":\"A1:B2\"}""#), "{b}");
        assert!(b.contains(r#""tools":[{"type":"function""#), "{b}");
    }

    #[test]
    fn a_tool_call_response_is_read_with_null_content() {
        // content が null でも、次の鍵の引用符に食いつかない
        let raw = r#"{"choices":[{"message":{"role":"assistant","content":null,
            "tool_calls":[{"id":"call_1","type":"function","function":
            {"name":"read_range","arguments":"{\"a1\":\"A1:B2\"}"}}]}}],
            "usage":{"prompt_tokens":10,"completion_tokens":5}}"#;
        let out = parse_chat_out(raw, 7).unwrap();
        assert_eq!(out.content, "");
        assert_eq!(out.tool_calls.len(), 1);
        assert_eq!(out.tool_calls[0].id, "call_1");
        assert_eq!(out.tool_calls[0].name, "read_range");
        assert_eq!(out.tool_calls[0].arguments, r#"{"a1":"A1:B2"}"#);
        assert_eq!(out.prompt_tokens, 10);
    }

    #[test]
    fn a_plain_answer_is_read_without_tool_calls() {
        let raw = r#"{"choices":[{"message":{"role":"assistant","content":"合計は 10 です"}}]}"#;
        let out = parse_chat_out(raw, 1).unwrap();
        assert_eq!(out.content, "合計は 10 です");
        assert!(out.tool_calls.is_empty());
    }

    #[test]
    fn an_unreadable_tool_response_is_an_error() {
        // 空の応答を「答えが空だった」と読み違えない
        assert!(parse_chat_out(r#"{"choices":[]}"#, 0).is_err());
    }

    #[test]
    fn connection_failure_returns_error() {
        // 使えないときに「指摘なし」を返さない
        let ep = Endpoint { port: 1, ..Default::default() };
        let e = chat(&ep, "s", "u", 0.0).unwrap_err();
        assert!(e.contains("繋がりません"), "{e}");
    }
}
