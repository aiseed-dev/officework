//! 画面の文言の言語 — **日本語の文がそのまま鍵**。
//!
//! 設計(SEKKEI「設定 — 器と言語」段階③):
//! - 呼び出しは `t!("…")`(そのままの文)と `tf!("…{}…", 値)`(穴埋め)。
//!   ja では鍵をそのまま返すので、**日本語の挙動は1バイトも変わらない**
//! - en は [`i18n_en`](crate::i18n_en) の対訳表で引く。**表に無い文は
//!   ja のまま出る**(嘘の英語を作らない)— 表の完全性は
//!   `python3 ui/gen_i18n.py` が検査する(未訳があれば止まる)
//! - 穴埋めは実行時の簡易整形(format! は雛形がコンパイル時定数でないと
//!   使えないため)。対応する書式は `{}`・`{:.0}`・`{:?}` — このアプリの
//!   文言が実際に使う3種だけ(増やすときはここに足す)

use std::collections::HashMap;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::OnceLock;

fn settings_path() -> PathBuf {
    pyrun::config_dir().join("settings.toml")
}

/// settings.toml から素朴に1つの鍵を読む(`key = "value"` の行)
fn from_file(key: &str) -> Option<String> {
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

/// いまの言語。**いつでも変えられます**(2026-08-19 発注者「言語は設定で
/// いつでも変更できるようにして」)。
///
/// 前は1プロセス1回で固まっていて、設定で選んでも*次の起動まで効きません*
/// でした。読み書きできる錠に替えたので、選んだその場で変わります。
static LANG: std::sync::RwLock<Option<&'static str>> = std::sync::RwLock::new(None);

/// 画面の言語。**文言が揃った言語だけ**を受ける(登録簿 i18n_tables が正)。
/// 優先順: [`set_language`] の注入 > 環境変数 OFFICE_LANG > settings.toml > 既定 ja
pub fn language() -> &'static str {
    if let Some(l) = *LANG.read().expect("言語の錠") {
        return l;
    }
    let raw = std::env::var("OFFICE_LANG")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| from_file("language"))
        .unwrap_or_default();
    let l = 静かな札(&raw).unwrap_or("ja");
    *LANG.write().expect("言語の錠") = Some(l);
    l
}

/// 札を、表に載っている `&'static str` に直す。無ければ `None`。
fn 静かな札(tag: &str) -> Option<&'static str> {
    if tag == "ja" {
        return Some("ja");
    }
    crate::i18n_tables::LANGS.iter().find(|x| **x == tag).copied()
}

/// 言語を外から注ぐ — settings.toml を持たない的(スマホの Swift / Kotlin の
/// 画面、WASM)のため。ファイルを読む代わりに、アプリが OS の言語設定を
/// ここへ渡す。
///
/// **いつ呼んでも効きます**(2026-08-19)。設定で選び直したときもここを
/// 通します。知らない札は false — 黙って ja に落としません。
/// 呼ばなければ今までどおり(環境変数 → settings.toml → ja)です。
pub fn set_language(tag: &str) -> bool {
    let Some(l) = 静かな札(tag) else { return false };
    *LANG.write().expect("言語の錠") = Some(l);
    true
}

/// 選べる言語(ja + 表の揃った言語)。設定ページの巡回もこれを見る
pub fn languages() -> Vec<&'static str> {
    let mut v = vec!["ja"];
    v.extend_from_slice(crate::i18n_tables::LANGS);
    v
}

/// 言語の札を、**その言語の人が読める名前**にする。
///
/// 設定ページは長らく `de` `fr` `zh-tw` と札のまま並べていた。それで
/// 済んでいたのは1つの言語に札が1つだったからで、**ポルトガル語を
/// `pt`(欧州)と `pt-br`(ブラジル)に分けた時点で済まなくなった** —
/// リスボンの人に `pt` と `pt-br` を見せても、どちらが自分のものか
/// 読み取れない(2026-08-11)。
///
/// 名前は英語ではなく**その言語自身の綴り**で持つ。自分の言語を探す人が
/// 読むのは、その言語の字だから。名前を書いていない札はそのまま返す
/// (黙って消さない)。
pub fn language_label(tag: &str) -> &str {
    match tag {
        "ja" => "日本語",
        "de" => "Deutsch",
        "en" => "English",
        "es" => "Español",
        "fr" => "Français",
        "id" => "Bahasa Indonesia",
        "it" => "Italiano",
        "ko" => "한국어",
        "pt" => "Português (Portugal)",
        "pt-br" => "Português (Brasil)",
        "ru" => "Русский",
        "tr" => "Türkçe",
        "vi" => "Tiếng Việt",
        "zh" => "简体中文",
        "zh-tw" => "繁體中文",
        other => other,
    }
}

/// いまの言語の対訳表。**言語ごとに作って取っておきます。**
///
/// 前は1つだけ作って固めていたので、言語を変えても前の表を見ていました。
/// 言語は 14 個で頭打ちなので、作った表はそのまま置いておきます
/// (`Box::leak`)。
fn lang_map() -> Option<&'static HashMap<&'static str, &'static str>> {
    type 表 = HashMap<&'static str, &'static str>;
    static 作った: OnceLock<std::sync::Mutex<HashMap<&'static str, Option<&'static 表>>>> =
        OnceLock::new();
    let 箱 = 作った.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let l = language();
    let mut 箱 = 箱.lock().expect("対訳表の錠");
    *箱.entry(l).or_insert_with(|| {
        crate::i18n_tables::table(l)
            .map(|t| &*Box::leak(Box::new(t.iter().copied().collect::<表>())))
    })
}

/// 文をいまの言語で。表に無い文は ja のまま(嘘の翻訳を作らない)
pub fn tr(ja: &'static str) -> &'static str {
    match lang_map() {
        Some(m) => m.get(ja).copied().unwrap_or(ja),
        None => ja,
    }
}

/// 穴埋めつきの文。雛形を tr で引いてから、実行時に埋める
pub fn trf(ja: &'static str, args: &[&dyn Display]) -> String {
    fill(tr(ja), args)
}

/// 簡易整形: `{}`・`{:.0}`・`{:?}` を左から順に埋める。
/// `{{`・`}}` は文字どおりの括弧
fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut it = template.chars().peekable();
    let mut n = 0usize;
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                out.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                out.push('}');
            }
            '{' => {
                let mut spec = String::new();
                for d in it.by_ref() {
                    if d == '}' {
                        break;
                    }
                    spec.push(d);
                }
                let s = args.get(n).map(|a| a.to_string()).unwrap_or_default();
                n += 1;
                match spec.as_str() {
                    ":.0" => match s.parse::<f64>() {
                        Ok(v) => out.push_str(&format!("{v:.0}")),
                        Err(_) => out.push_str(&s),
                    },
                    // {:?} は呼び側が表示済みの文字を渡す約束(そのまま)
                    _ => out.push_str(&s),
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 穴埋めが順に入る() {
        assert_eq!(fill("{} 列で {} 件", &[&"B", &3]), "B 列で 3 件");
        assert_eq!(fill("縮尺 {:.0}%", &[&99.6]), "縮尺 100%");
        assert_eq!(fill("{{リテラル}} と {}", &[&"値"]), "{リテラル} と 値");
    }

    #[test]
    fn 表に無い文はそのまま() {
        // ja では常に鍵のまま。en で表に無くても鍵のまま(黙って英語を作らない)
        assert_eq!(tr("この文は表に無い(試験用)"), "この文は表に無い(試験用)");
    }
}
