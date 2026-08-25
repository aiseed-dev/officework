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
///
/// 優先順: [`set_language`] の注入 > 環境変数 OFFICE_LANG > settings.toml >
/// **OS の言語設定** > 既定 ja
///
/// OS の言語設定を見るようになったのは 2026-08-26 です。それまでは
/// 「何も書いていなければ日本語」でした。鍵を英語に裏返すと「何も
/// 書いていなければ英語」になってしまい、設定を書いていない人の画面が
/// ある日いきなり英語になります。**設定を書いていない人には、その人の
/// 機械の言語で出す**のが正しい既定です。
pub fn language() -> &'static str {
    if let Some(l) = *LANG.read().expect("言語の錠") {
        return l;
    }
    let l = std::env::var("OFFICE_LANG")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| from_file("language"))
        .and_then(|raw| 静かな札(&raw))
        .or_else(os_language)
        .unwrap_or("ja");
    *LANG.write().expect("言語の錠") = Some(l);
    l
}

/// OS の言語設定。無い・読めない・表に無いなら `None`。
///
/// 見る順は `LC_ALL` → `LC_MESSAGES` → `LANG`。POSIX の決まりの順です。
/// 値は `ja_JP.UTF-8` や `pt_BR.UTF-8` のような形なので、[`札に直す`] で
/// うちの札に直します。
///
/// Windows と Mac もこの3つを見ます。どちらも本来は OS の API で聞く物
/// ですが、**うちの3つのアプリはどれも `main` で言語を注げる**ので、
/// そちらの殻から [`set_language`] で渡してください。ここはその注ぎが
/// 無かったときの下敷きです。
fn os_language() -> Option<&'static str> {
    for k in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Ok(v) = std::env::var(k) else { continue };
        if v.is_empty() || v == "C" || v == "POSIX" {
            continue;
        }
        if let Some(l) = 札に直す(&v) {
            return Some(l);
        }
    }
    None
}

/// POSIX のロケールの字を、うちの札に直す。
///
/// `ja_JP.UTF-8` → `ja`、`pt_BR.UTF-8` → `pt-br`、`zh_TW` → `zh-tw`。
/// **国まで見るのは、うちが国で分けている札だけ**です(`pt`/`pt-br` と
/// `zh`/`zh-tw`)。それ以外は言語だけで引きます。
///
/// 中国語は `zh_CN` が簡体、`zh_TW` と `zh_HK` が繁体です。国を落として
/// `zh` にしてしまうと、台湾の人に簡体字が出ます。
fn 札に直す(ロケール: &str) -> Option<&'static str> {
    // `.UTF-8` や `@euro` のような飾りを落とす
    let 芯 = ロケール
        .split(['.', '@'])
        .next()
        .unwrap_or(ロケール)
        .replace('_', "-");
    if 芯.is_empty() {
        return None;
    }
    // 国まで込みで引く(pt-br / zh-tw)
    let 小文字 = 芯.to_ascii_lowercase();
    if let Some(l) = 静かな札(&小文字) {
        return Some(l);
    }
    let 言語 = 小文字.split('-').next().unwrap_or(&小文字);
    // zh_HK は繁体。表に zh-hk は無いので zh-tw へ寄せる
    if 言語 == "zh" && 小文字 == "zh-hk" {
        return 静かな札("zh-tw");
    }
    静かな札(言語)
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

/// 言語の決め方の試験。**`LANG` の控えを触るのでここに置きます**
/// (2026-08-21)。
///
/// 前は `face/src/settings.rs` に「環境変数で en になる」の試験があり、
/// *その試験プロセスで最初に `language()` を呼ぶのがそこ*であることに
/// 頼っていました。だから `face/src/tabs.rs` の4本は `#[ignore]` を付けて
/// 単独で回していました — 先に `language()` を呼ぶと控えが埋まって、
/// あちらが落ちるからです。
///
/// 控えを直に触れるここへ移したので、順番の縛りが要らなくなりました。
#[cfg(test)]
mod 言語の決め方 {
    use super::*;

    /// **この節の試験は直列に回します。** 言語の控えも `OFFICE_LANG` も
    /// プロセスで1つなので、同時に走ると取り合って落ちます
    /// (2026-08-21 に実際に落ちました)。錠を試験の頭で取り、
    /// 終わりまで持ちます。
    ///
    /// 毒された錠は中身を取り出して使います — 1本落ちたせいで
    /// 残りが「錠が毒された」で落ちると、本当の原因が見えなくなります。
    static 錠: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn 順番に() -> std::sync::MutexGuard<'static, ()> {
        錠.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// OS の言語設定を見る環境変数。試験では**全部押さえます** —
    /// 1つでも残っていると、回す機械の言語で答えが変わります
    const ロケールの環境変数: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];

    /// 控えを空にしてから、環境変数を立てて引き直す。
    /// **必ず元に戻します**(呼ぶ側が錠を持っている前提)
    ///
    /// OS のロケールは `"C"` に伏せます。伏せないと、`OFFICE_LANG` も
    /// 設定も無いときの答えが**回す機械の言語で変わり**、CI と手元で
    /// 違う結果になります(2026-08-26 に OS のロケールを見るようにした)。
    fn 引き直す(raw: Option<&str>) -> &'static str {
        引き直す_ロケール(raw, Some("C"))
    }

    /// `引き直す` の、OS のロケールも決められる版。
    fn 引き直す_ロケール(raw: Option<&str>, ロケール: Option<&str>) -> &'static str {
        let 元env = std::env::var_os("OFFICE_LANG");
        let 元ロケール: Vec<_> = ロケールの環境変数
            .iter()
            .map(|k| (*k, std::env::var_os(k)))
            .collect();
        let 元lang = *LANG.read().expect("言語の錠");
        unsafe {
            match raw {
                Some(v) => std::env::set_var("OFFICE_LANG", v),
                None => std::env::remove_var("OFFICE_LANG"),
            }
            for k in ロケールの環境変数 {
                std::env::remove_var(k);
            }
            // 立てるのは LANG だけ。LC_ALL / LC_MESSAGES を消してあるので、
            // 見る順(LC_ALL → LC_MESSAGES → LANG)の一番下に届きます
            if let Some(v) = ロケール {
                std::env::set_var("LANG", v);
            }
        }
        *LANG.write().expect("言語の錠") = None;
        let got = language();
        *LANG.write().expect("言語の錠") = 元lang;
        unsafe {
            match 元env {
                Some(v) => std::env::set_var("OFFICE_LANG", v),
                None => std::env::remove_var("OFFICE_LANG"),
            }
            for (k, v) in 元ロケール {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
        got
    }

    /// 環境変数で選べる
    #[test]
    fn 環境変数で選べる() {
        let _錠 = 順番に();
        assert_eq!(引き直す(Some("en")), "en");
        assert_eq!(引き直す(Some("fr")), "fr", "fr は文言が揃っています");
        assert_eq!(引き直す(Some("ja")), "ja");
    }

    /// **知らない札は ja に落ちる。** 文言の無い言語を名乗りません。
    ///
    /// 前はこの試験が自分で書いた `match` を検べていて、本物を1度も
    /// 通していませんでした。おまけに「fr は文言が無い」と書いてあり、
    /// *主張そのものが古く*なっていました(いまは揃っています)。
    #[test]
    fn 知らない札はjaに落ちる() {
        let _錠 = 順番に();
        assert_eq!(引き直す(Some("xx")), "ja");
        assert_eq!(引き直す(Some("")), "ja");
        assert_eq!(引き直す(None), "ja", "何も無ければ既定は ja");
    }

    /// 揃っている言語は全部受ける(表と食い違わない)
    #[test]
    fn 揃っている言語は全部受ける() {
        let _錠 = 順番に();
        for l in crate::i18n_tables::LANGS {
            assert_eq!(引き直す(Some(l)), *l, "{l} が受けられない");
        }
    }

    /// **OS の言語設定に従う**(2026-08-26)。
    ///
    /// 鍵を英語に裏返すと「何も書いていなければ英語」になります。設定を
    /// 書いていない人の画面がある日いきなり英語にならないよう、その人の
    /// 機械の言語で出します。
    #[test]
    fn 設定が無ければosの言語で出る() {
        let _錠 = 順番に();
        assert_eq!(引き直す_ロケール(None, Some("ja_JP.UTF-8")), "ja");
        assert_eq!(引き直す_ロケール(None, Some("en_US.UTF-8")), "en");
        assert_eq!(引き直す_ロケール(None, Some("de_DE.UTF-8")), "de");
        // 飾りが無い形も読む
        assert_eq!(引き直す_ロケール(None, Some("fr_FR")), "fr");
        assert_eq!(引き直す_ロケール(None, Some("ko")), "ko");
    }

    /// **国で分けている札は、国まで見る**(pt-br と zh-tw)。
    ///
    /// 国を落とすと、台湾の人に簡体字が、ブラジルの人に欧州の
    /// ポルトガル語が出ます。
    #[test]
    fn 国で分けている札は国まで見る() {
        let _錠 = 順番に();
        assert_eq!(引き直す_ロケール(None, Some("pt_BR.UTF-8")), "pt-br");
        assert_eq!(引き直す_ロケール(None, Some("pt_PT.UTF-8")), "pt");
        assert_eq!(引き直す_ロケール(None, Some("zh_TW.UTF-8")), "zh-tw");
        assert_eq!(引き直す_ロケール(None, Some("zh_CN.UTF-8")), "zh");
        // 香港は繁体。表に zh-hk が無いので zh-tw へ寄せる
        assert_eq!(引き直す_ロケール(None, Some("zh_HK.UTF-8")), "zh-tw");
    }

    /// **書いてある設定が OS より強い。** 機械が英語でも、選んだ言語で出ます
    #[test]
    fn 書いた設定がosより強い() {
        let _錠 = 順番に();
        assert_eq!(引き直す_ロケール(Some("ja"), Some("en_US.UTF-8")), "ja");
        assert_eq!(引き直す_ロケール(Some("de"), Some("ja_JP.UTF-8")), "de");
    }

    /// OS の言語も読めなければ ja(いままでどおり)
    #[test]
    fn osの言語も読めなければja() {
        let _錠 = 順番に();
        for l in ["C", "POSIX", "", "xx_YY.UTF-8"] {
            assert_eq!(引き直す_ロケール(None, Some(l)), "ja", "{l:?} で ja に落ちない");
        }
        assert_eq!(引き直す_ロケール(None, None), "ja", "環境変数が無いとき");
    }

    /// [`set_language`] はいつ呼んでも効く(2026-08-19 の決め)
    #[test]
    fn 注いだ言語はいつでも効く() {
        let _錠 = 順番に();
        let 元 = *LANG.read().expect("言語の錠");
        assert!(set_language("de"));
        assert_eq!(language(), "de");
        assert!(set_language("ja"));
        assert_eq!(language(), "ja");
        assert!(!set_language("xx"), "知らない札は断る");
        *LANG.write().expect("言語の錠") = 元;
    }
}
