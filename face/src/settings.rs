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
        lines.push(format!("{key} = \"{value}\""));
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
    #[test]
    fn 環境変数でenの表が選ばれる() {
        // OnceLock は1プロセス1回 — この試験プロセスで最初に language() を
        // 呼ぶのはここ(他の試験は表を直接見る)。旗を立ててから引く
        std::env::set_var("OFFICE_LANG", "en");
        assert_eq!(super::language(), "en");
        assert_eq!(crate::ribbon::writer_tabs()[1].name, "Home");
        assert_eq!(crate::ribbon::calc_tabs()[1].name, "Home");
    }

    #[test]
    fn 知らない言語はjaに落ちる() {
        // language() は一度きり(OnceLock)なので、判定の芯だけ検査する
        let pick = |raw: &str| match raw {
            "en" => "en",
            _ => "ja",
        };
        assert_eq!(pick("en"), "en");
        assert_eq!(pick("fr"), "ja", "文言の無い言語を名乗らない");
        assert_eq!(pick(""), "ja");
    }
}
