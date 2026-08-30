//! **どの言語で組むかを決める、1本の規則。**
//!
//! 2026-08-30 発注者「どれも無ければ en です」。
//!
//! 決め方は上から順です。
//!
//! . 呼ぶ側が明に渡した言語(`Doc(lang=)` など)
//! . 環境変数 `OFFICE_LANG`
//! . 設定ファイル `~/.config/officework/settings.toml` の `language`
//! . OS の言語設定(`LC_ALL` → `LC_MESSAGES` → `LANG`)
//! . どれも無ければ `en-us`
//!
//! **ここに置いたのは、決め方が2つに分かれていたからです。** 画面
//! (`lang::i18n`)は上の順で決めていましたが、エンジン
//! (`kumihan::font`)は設定も OS も見ずに `ja` の決め打ちでした。
//! Python から使うと必ず日本語の既定になり、ドイツ語の設定にしてある
//! 機械でも日本語で組まれます。
//!
//! `book` は依存を1つも持たないので、画面もエンジンもここを呼べます。

/// 設定ファイルの置き場(`~/.config/officework/settings.toml`)。
///
/// **`XDG_CONFIG_HOME` は見ません。** マクロの置き場(`pyrun::config_dir`)が
/// `HOME` だけを見ているので、ここだけ XDG を見ると、設定と
/// マクロが別の場所に分かれます(2026-08-30)。
pub fn settings_path() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".config/officework/settings.toml")
}

/// 設定ファイルから1つ読む。無ければ `None`
pub fn setting(key: &str) -> Option<String> {
    let s = std::fs::read_to_string(settings_path()).ok()?;
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

/// **どれも無かったときの言語。**
///
/// 2026-08-30 発注者。前は `ja` でした。英語を主にしたので、手がかりが
/// 1つも無いときは英語で出します。日本語の機械では OS の言語が先に
/// 当たるので、この行まで落ちません。
///
/// 手がかりが1つも無いのは素の `en` と同じことなので、[`to_tag`] が
/// `en` に返すのと同じ `en-us` にします。
pub const FALLBACK: &str = "en-us";

/// OS の言語設定を札に直す。無い・読めないなら `None`。
///
/// 見る順は `LC_ALL` → `LC_MESSAGES` → `LANG`(POSIX の決まりの順)。
pub fn os_language() -> Option<String> {
    for k in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Ok(v) = std::env::var(k) else { continue };
        if v.is_empty() || v == "C" || v == "POSIX" {
            continue;
        }
        if let Some(t) = to_tag(&v) {
            return Some(t);
        }
    }
    None
}

/// POSIX のロケールの字を札に直す。`ja_JP.UTF-8` → `ja`、`zh_TW` → `zh-tw`。
///
/// **国まで見るのは、国で分けている札だけ**です(`pt`/`pt-br`、
/// `zh`/`zh-tw`、`en-us`/`en-gb`)。台湾の人に簡体字を出さないためです。
///
/// **英語は日付の並びが国で割れています。** `m/d/yyyy` と `dd/mm/yyyy` の
/// どちらで出すかを決めるため、`en-us` と `en-gb` に分けています
/// (2026-08-30 発注者)。
///
/// 素の `en` は `en-us` です。国を名乗っていて、それが米国でないときだけ
/// `en-gb` にします。`en_GB` も `en_AU` も `en_IE` も `en-gb` です。
/// 豪・NZ・愛・南ア・印が `dd/mm/yyyy` を使うからで、2026-08-11 の
/// 判じ方をそのまま使っています。
pub fn to_tag(locale: &str) -> Option<String> {
    let core = locale.split(['.', '@']).next().unwrap_or(locale).replace('_', "-");
    if core.is_empty() {
        return None;
    }
    let lower = core.to_lowercase();
    if lower == "en" || lower.starts_with("en-") {
        let us = lower == "en" || lower.starts_with("en-us");
        return Some(if us { "en-us" } else { "en-gb" }.into());
    }
    for kuni in ["pt-br", "zh-tw", "zh-hk"] {
        if lower.starts_with(kuni) {
            return Some(if kuni == "zh-hk" { "zh-tw".into() } else { kuni.into() });
        }
    }
    let go = lower.split('-').next().unwrap_or(&lower);
    (!go.is_empty()).then(|| go.to_string())
}

/// **明の指定が無いときの言語。** 上の順で決めます
pub fn decide(akashi: Option<&str>) -> String {
    akashi
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| std::env::var("OFFICE_LANG").ok().filter(|s| !s.is_empty()))
        .or_else(|| setting("language"))
        .and_then(|raw| to_tag(&raw))
        .or_else(os_language)
        .unwrap_or_else(|| FALLBACK.to_string())
}

#[cfg(test)]
mod tests {
    /// **札に直す。** 国で分けている物だけ国まで見ます
    #[test]
    fn a_posix_locale_becomes_our_tag() {
        for (from, want) in [
            ("ja_JP.UTF-8", "ja"),
            // 英語は国まで名乗ります(下の試験で細かく見ます)
            ("en_US.UTF-8", "en-us"),
            ("en", "en-us"),
            ("en_GB.UTF-8", "en-gb"),
            ("pt_BR.UTF-8", "pt-br"),
            ("pt_PT.UTF-8", "pt"),
            ("zh_TW", "zh-tw"),
            ("zh_HK.UTF-8", "zh-tw"),
            ("zh_CN.UTF-8", "zh"),
            ("de", "de"),
        ] {
            assert_eq!(super::to_tag(from).as_deref(), Some(want), "{from}");
        }
        assert_eq!(super::to_tag(""), None);
    }

    /// **明の指定がいちばん強い。**
    #[test]
    fn an_explicit_language_wins() {
        assert_eq!(super::decide(Some("de")), "de");
        assert_eq!(super::decide(Some("pt_BR.UTF-8")), "pt-br");
    }

    /// **どれも無ければ英語。**(2026-08-30 発注者。前は ja でした)
    #[test]
    fn nothing_set_means_english() {
        // 環境変数を空にして呼びます。設定ファイルは機械のものを見るので、
        // ここでは「空の指定が明の指定として通らない」ことだけ見ます
        assert_eq!(super::decide(Some("")), super::decide(None));
        assert_eq!(super::FALLBACK, "en-us");
    }

}
