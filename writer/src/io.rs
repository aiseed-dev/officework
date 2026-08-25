//! writer の外との出入り(main.rs から純移動 2026-08-08。部屋割りの4歩目)。
//! 排他ロック・署名の鍵・網(HTTP)・URL の組み立てと名乗り。**純移動**

// 排他ロック・署名の鍵・16進は ops(calc と writer で1本。2026-08-12 段A)。
// 訳の要る文言だけ、ここで包む(calc 側と同文 — ずれてよいのは言葉だけ)
// 16進と .sig の道の組み立ては、署名の中身ごと ops へ移りました
// (2026-08-21)。ここから使う物だけ残します
pub(crate) use ops::{lock_identity, lock_path_for};

/// 先客のロックを読む(あれば名乗りを返す)。自分自身のロックは先客と見ない。
pub(crate) fn foreign_lock(p: &std::path::Path) -> Option<String> {
    ops::foreign_lock(p, ui::t!("someone"))
}

/// **自動復旧の控え**(2026-08-21 の B-3)。
///
/// 表にしかありませんでした。落ちたとき・電源が切れたときに失う分を
/// 減らすための別の控えで、**原本は上書きしません**。無事に保存できたら
/// 消します。置き場と道の作り方は `ops` の1本で、表と共通です。
///
/// 中身は adoc です — 文章の原本と同じ形なので、復旧はただ開くだけです。
pub(crate) fn 控えの道(orig: Option<&std::path::Path>) -> std::path::PathBuf {
    ops::recover_path_for(orig, "adoc", "未保存の文書")
}

/// 鍵が用意できなかった理由を、その言語の文で言う(本体は ops)。
///
/// **鍵を読む所そのものは ops の1本**です(2026-08-21 に署名の中身も
/// そちらへ移しました)。ここに残るのは訳の要る文言だけで、置き場を
/// アプリ側にするのは、訳の走査が `calc/src` `writer/src` `ui/src` しか
/// 見ないからです。
pub(crate) fn key_err_msg(e: ops::KeyErr) -> String {
    match e {
        ops::KeyErr::Corrupt => ui::t!("The key file is damaged (~/.config/officework/sign.key)").to_string(),
        ops::KeyErr::NoRandom(e) => ui::tf!("Can't get random numbers: {}", e).to_string(),
        ops::KeyErr::CantStore(e) => ui::tf!("Can't store the key: {}", e).to_string(),
    }
}

/// 小さな HTTP(http と https。公開 Web も見える — 発注者 2026-08-04)。
/// HTTP/1.0 で頼む(chunked を受けない素直な形)。転送(3xx)は5回まで
/// 追いかける。GET は body=None、POST は form の urlencoded。
/// 返り値は (中身, 最終 URL)
pub(crate) fn http_fetch(url: &str, body: Option<&str>) -> Result<(Vec<u8>, String), String> {
    use std::io::{BufRead as _, Read as _, Write as _};
    let mut url = url.to_string();
    for _ in 0..5 {
        let (https, rest) = if let Some(r) = url.strip_prefix("https://") {
            (true, r)
        } else if let Some(r) = url.strip_prefix("http://") {
            (false, r)
        } else {
            return Err(ui::t!("Use an http:// or https:// URL").into());
        };
        let (hostport, path) = match rest.split_once('/') {
            Some((h, p)) => (h.to_string(), format!("/{p}")),
            None => (rest.to_string(), "/".to_string()),
        };
        let host = hostport.split(':').next().unwrap_or(&hostport).to_string();
        let addr = if hostport.contains(':') {
            hostport.clone()
        } else {
            format!("{hostport}:{}", if https { 443 } else { 80 })
        };
        let sock = std::net::TcpStream::connect(&addr)
            .map_err(|e| ui::tf!("Can't connect ({}): {}", addr, e))?;
        sock.set_read_timeout(Some(std::time::Duration::from_secs(15))).ok();
        let req = match body {
            Some(b) => format!(
                "POST {path} HTTP/1.0\r\nHost: {hostport}\r\n                 User-Agent: aiseed-writer\r\n                 Content-Type: application/x-www-form-urlencoded\r\n                 Content-Length: {}\r\nConnection: close\r\n\r\n{b}",
                b.len()
            ),
            None => format!(
                "GET {path} HTTP/1.0\r\nHost: {hostport}\r\n                 User-Agent: aiseed-writer\r\nConnection: close\r\n\r\n"
            ),
        };
        // http と https を同じ道で読むための入れ物
        let mut stream: Box<dyn ReadWrite> = if https {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let cfg = rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            let name = rustls::pki_types::ServerName::try_from(host.clone())
                .map_err(|_| ui::tf!("Bad host name: {}", host))?;
            let conn = rustls::ClientConnection::new(std::sync::Arc::new(cfg), name)
                .map_err(|e| e.to_string())?;
            Box::new(rustls::StreamOwned::new(conn, sock))
        } else {
            Box::new(sock)
        };
        stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
        let mut r = std::io::BufReader::new(stream);
        let mut status = String::new();
        r.read_line(&mut status).map_err(|e| e.to_string())?;
        let mut location: Option<String> = None;
        let mut line = String::new();
        loop {
            line.clear();
            if r.read_line(&mut line).map_err(|e| e.to_string())? == 0
                || line.trim().is_empty()
            {
                break;
            }
            if line.to_ascii_lowercase().starts_with("location:") {
                location = Some(line[9..].trim().to_string());
            }
        }
        if status.contains(" 30") {
            if let Some(loc) = location {
                url = resolve_url(&url, &loc);
                continue;
            }
        }
        let mut out = Vec::new();
        r.read_to_end(&mut out).map_err(|e| e.to_string())?;
        if !status.contains(" 200") {
            return Err(ui::tf!("Server response: {}", status.trim()));
        }
        return Ok((out, url));
    }
    Err(ui::t!("Too many redirects (5 max)").into())
}

trait ReadWrite: std::io::Read + std::io::Write {}
impl<T: std::io::Read + std::io::Write> ReadWrite for T {}

/// 相対の URL を今の場所から解く(部分集合)
pub(crate) fn resolve_url(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    let scheme_end = base.find("://").map(|i| i + 3).unwrap_or(0);
    let host_end = base[scheme_end..]
        .find('/')
        .map(|i| scheme_end + i)
        .unwrap_or(base.len());
    if let Some(rest) = href.strip_prefix("//") {
        return format!("{}{rest}", &base[..scheme_end]);
    }
    if href.starts_with('/') {
        return format!("{}{href}", &base[..host_end]);
    }
    match base.rfind('/').filter(|i| *i > host_end) {
        Some(i) => format!("{}{href}", &base[..i + 1]),
        None => format!("{}/{href}", &base[..host_end]),
    }
}

pub(crate) fn urlenc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                o.push(b as char)
            }
            b' ' => o.push('+'),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

