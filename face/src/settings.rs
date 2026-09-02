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

impl AiDest {
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
    let mut close = |cur: &mut Option<AiDest>, out: &mut Vec<AiDest>| {
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

/// 宛先の一覧。`[[ai]]` が無ければ、今までの `ai_url` / `ai_model` を
/// 1行にして返す(後方互換 — 鍵も従来どおり OFFICE_API_KEY)。
/// それも無ければ空 = パネルの「未設定」
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
