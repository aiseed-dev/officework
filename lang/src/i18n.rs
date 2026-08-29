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
use std::sync::OnceLock;



/// いまの言語。**いつでも変えられます**(2026-08-19 発注者「言語は設定で
/// いつでも変更できるようにして」)。
///
/// 前は1プロセス1回で固まっていて、設定で選んでも*次の起動まで効きません*
/// でした。読み書きできる錠に替えたので、選んだその場で変わります。
static LANG: std::sync::RwLock<Option<&'static str>> = std::sync::RwLock::new(None);

/// 画面の言語。**文言が揃った言語だけ**を受ける(登録簿 i18n_tables が正)。
///
/// 優先順: [`set_language`] の注入 > 環境変数 OFFICE_LANG > settings.toml >
/// **OS の言語設定** > 既定 en(2026-08-30 発注者。前は ja でした)
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
    // **決め方は book に1本**(環境変数 → 設定 → OS → en。2026-08-30)。
    // エンジンと画面で別々に決めていたので、Python から使うと設定を
    // 見ませんでした。ここは book が返した札を、文言の揃った言語に
    // 丸めるだけです
    let l = quiet_tag(&book::lang::decide(None)).unwrap_or(book::lang::FALLBACK);
    *LANG.write().expect("言語の錠") = Some(l);
    l
}



/// 札を、表に載っている `&'static str` に直す。無ければ `None`。
///
/// **en を名指しで受けます。** 鍵が英語なので en は対訳表を持ちません
/// (2026-08-26)。表の登録簿だけを見ると en が落ちてしまいます。
fn quiet_tag(tag: &str) -> Option<&'static str> {
    if tag == "en" {
        return Some("en");
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
/// **試験のときだけの錠。** 画面の言語は処理系に1つしかないので、
/// 替える試験と読む試験が並ぶと崩れます。替える側がこの錠を取り、
/// 読む側で崩れて困る物も取ります(2026-08-29。engine の
/// `font::lang_lock` と同じ形)。
/// **`#[cfg(test)]` にはできません。** それは crate の中だけの印で、
/// 使うのは別の crate(face)の試験だからです
pub static LANG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 錠を取ります。前の試験が落ちて錠が壊れていても、そのまま使います
pub fn lang_lock() -> std::sync::MutexGuard<'static, ()> {
    LANG_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_language(tag: &str) -> bool {
    let Some(l) = quiet_tag(tag) else { return false };
    *LANG.write().expect("言語の錠") = Some(l);
    true
}

/// 選べる言語(表の揃った言語 + en)。設定ページの巡回もこれを見る。
///
/// **並びは辞書順**にします。前は ja を頭に置いていましたが、鍵が英語に
/// なって ja も表を持つ言語の1つになったので、特別扱いをやめました。
pub fn languages() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = crate::i18n_tables::LANGS.to_vec();
    if !v.contains(&"en") {
        v.push("en");
        v.sort_unstable();
    }
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
    type Table = HashMap<&'static str, &'static str>;
    static MADE: OnceLock<std::sync::Mutex<HashMap<&'static str, Option<&'static Table>>>> =
        OnceLock::new();
    let box_of = MADE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let l = language();
    let mut box_of = box_of.lock().expect("対訳表の錠");
    *box_of.entry(l).or_insert_with(|| {
        crate::i18n_tables::table(l)
            .map(|t| &*Box::leak(Box::new(t.iter().copied().collect::<Table>())))
    })
}

/// 鍵をいまの言語の文に。
///
/// **鍵は記号です**(2026-08-26 発注者「キーは英語の省略形で適当に
/// 決めればいい」)。`save` のような字で、画面には出ません。
///
/// その言語に訳が無ければ**英語**に落ちます。記号をそのまま画面に出すと
/// 「save」と表示されてしまうので、英語を最後の砦にします。英語にも無い
/// (= 鍵の書き間違い)ときだけ、鍵がそのまま出ます — そこは見張りが
/// 止めるところです。
pub fn tr(key: &'static str) -> &'static str {
    if let Some(m) = lang_map() {
        if let Some(v) = m.get(key) {
            return v;
        }
    }
    english_table().get(key).copied().unwrap_or(key)
}

/// 英語の表。**最後の砦**なので、言語に関わらずこれだけは持っておきます。
fn english_table() -> &'static HashMap<&'static str, &'static str> {
    static MADE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MADE.get_or_init(|| {
        crate::i18n_tables::table("en")
            .map(|t| t.iter().copied().collect())
            .unwrap_or_default()
    })
}

/// **実行時に決まる鍵**を引く。表に無ければ鍵をそのまま返す。
///
/// [`tr`] は `&'static str` しか受けません(鍵はソースに書いた文なので
/// 普通はそれで足ります)。集計の種類のように、鍵が変数に入っている所
/// だけこちらを使ってください(2026-08-26)。
pub fn tr_dyn(key: &str) -> String {
    match lang_map() {
        Some(m) => m.get(key).map(|s| s.to_string()).unwrap_or_else(|| key.to_string()),
        None => key.to_string(),
    }
}

/// 穴埋めつきの文。雛形を tr で引いてから、実行時に埋める
pub fn trf(key: &'static str, args: &[&dyn Display]) -> String {
    fill(tr(key), args)
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
    fn placeholders_fill_in_order() {
        assert_eq!(fill("{} 列で {} 件", &[&"B", &3]), "B 列で 3 件");
        assert_eq!(fill("縮尺 {:.0}%", &[&99.6]), "縮尺 100%");
        assert_eq!(fill("{{リテラル}} と {}", &[&"値"]), "{リテラル} と 値");
    }

    #[test]
    fn untranslated_key_shows_english() {
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
mod how_language_is_chosen {
    use super::*;

    /// **この節の試験は直列に回します。** 言語の控えも `OFFICE_LANG` も
    /// プロセスで1つなので、同時に走ると取り合って落ちます
    /// (2026-08-21 に実際に落ちました)。錠を試験の頭で取り、
    /// 終わりまで持ちます。
    ///
    /// 毒された錠は中身を取り出して使います — 1本落ちたせいで
    /// 残りが「錠が毒された」で落ちると、本当の原因が見えなくなります。
    static LOCK_OF: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serially() -> std::sync::MutexGuard<'static, ()> {
        LOCK_OF.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// OS の言語設定を見る環境変数。試験では**全部押さえます** —
    /// 1つでも残っていると、回す機械の言語で答えが変わります
    const LOCALE_ENV_VARS: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];

    /// 控えを空にしてから、環境変数を立てて引き直す。
    /// **必ず元に戻します**(呼ぶ側が錠を持っている前提)
    ///
    /// OS のロケールは `"C"` に伏せます。伏せないと、`OFFICE_LANG` も
    /// 設定も無いときの答えが**回す機械の言語で変わり**、CI と手元で
    /// 違う結果になります(2026-08-26 に OS のロケールを見るようにした)。
    fn resolve_again(raw: Option<&str>) -> &'static str {
        resolve_with_locale(raw, Some("C"))
    }

    /// `引き直す` の、OS のロケールも決められる版。
    fn resolve_with_locale(raw: Option<&str>, locale: Option<&str>) -> &'static str {
        let old_env = std::env::var_os("OFFICE_LANG");
        let old_locale: Vec<_> = LOCALE_ENV_VARS
            .iter()
            .map(|k| (*k, std::env::var_os(k)))
            .collect();
        let old_lang = *LANG.read().expect("言語の錠");
        unsafe {
            match raw {
                Some(v) => std::env::set_var("OFFICE_LANG", v),
                None => std::env::remove_var("OFFICE_LANG"),
            }
            for k in LOCALE_ENV_VARS {
                std::env::remove_var(k);
            }
            // 立てるのは LANG だけ。LC_ALL / LC_MESSAGES を消してあるので、
            // 見る順(LC_ALL → LC_MESSAGES → LANG)の一番下に届きます
            if let Some(v) = locale {
                std::env::set_var("LANG", v);
            }
        }
        *LANG.write().expect("言語の錠") = None;
        let got = language();
        *LANG.write().expect("言語の錠") = old_lang;
        unsafe {
            match old_env {
                Some(v) => std::env::set_var("OFFICE_LANG", v),
                None => std::env::remove_var("OFFICE_LANG"),
            }
            for (k, v) in old_locale {
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
    fn environment_variable_selects_language() {
        let _lock = serially();
        assert_eq!(resolve_again(Some("en")), "en");
        assert_eq!(resolve_again(Some("fr")), "fr", "fr は文言が揃っています");
        assert_eq!(resolve_again(Some("ja")), "ja");
    }

    /// **知らない札は en に落ちる。** 文言の無い言語を名乗りません。
    ///
    /// 落ち先は 2026-08-30 に発注者が ja から en へ変えました。
    ///
    /// 前はこの試験が自分で書いた `match` を検べていて、本物を1度も
    /// 通していませんでした。おまけに「fr は文言が無い」と書いてあり、
    /// *主張そのものが古く*なっていました(いまは揃っています)。
    #[test]
    fn unknown_tag_falls_back_to_english() {
        let _lock = serially();
        assert_eq!(resolve_again(Some("xx")), "en");
        assert_eq!(resolve_again(Some("")), "en");
        assert_eq!(resolve_again(None), "en", "何も無ければ既定は en");
    }

    /// 揃っている言語は全部受ける(表と食い違わない)
    #[test]
    fn accepts_every_complete_language() {
        let _lock = serially();
        for l in crate::i18n_tables::LANGS {
            assert_eq!(resolve_again(Some(l)), *l, "{l} が受けられない");
        }
    }

    /// **OS の言語設定に従う**(2026-08-26)。
    ///
    /// 鍵を英語に裏返すと「何も書いていなければ英語」になります。設定を
    /// 書いていない人の画面がある日いきなり英語にならないよう、その人の
    /// 機械の言語で出します。
    #[test]
    fn with_no_setting_uses_os_language() {
        let _lock = serially();
        assert_eq!(resolve_with_locale(None, Some("ja_JP.UTF-8")), "ja");
        assert_eq!(resolve_with_locale(None, Some("en_US.UTF-8")), "en");
        assert_eq!(resolve_with_locale(None, Some("de_DE.UTF-8")), "de");
        // 飾りが無い形も読む
        assert_eq!(resolve_with_locale(None, Some("fr_FR")), "fr");
        assert_eq!(resolve_with_locale(None, Some("ko")), "ko");
    }

    /// **国で分けている札は、国まで見る**(pt-br と zh-tw)。
    ///
    /// 国を落とすと、台湾の人に簡体字が、ブラジルの人に欧州の
    /// ポルトガル語が出ます。
    #[test]
    fn region_tags_match_on_region_too() {
        let _lock = serially();
        assert_eq!(resolve_with_locale(None, Some("pt_BR.UTF-8")), "pt-br");
        assert_eq!(resolve_with_locale(None, Some("pt_PT.UTF-8")), "pt");
        assert_eq!(resolve_with_locale(None, Some("zh_TW.UTF-8")), "zh-tw");
        assert_eq!(resolve_with_locale(None, Some("zh_CN.UTF-8")), "zh");
        // 香港は繁体。表に zh-hk が無いので zh-tw へ寄せる
        assert_eq!(resolve_with_locale(None, Some("zh_HK.UTF-8")), "zh-tw");
    }

    /// **書いてある設定が OS より強い。** 機械が英語でも、選んだ言語で出ます
    #[test]
    fn written_setting_beats_os_locale() {
        let _lock = serially();
        assert_eq!(resolve_with_locale(Some("ja"), Some("en_US.UTF-8")), "ja");
        assert_eq!(resolve_with_locale(Some("de"), Some("ja_JP.UTF-8")), "de");
    }

    /// OS の言語も読めなければ en(2026-08-30 発注者。前は ja でした)
    #[test]
    fn falls_back_to_english_when_os_locale_unreadable() {
        let _lock = serially();
        for l in ["C", "POSIX", "", "xx_YY.UTF-8"] {
            assert_eq!(resolve_with_locale(None, Some(l)), "en", "{l:?} で en に落ちない");
        }
        assert_eq!(resolve_with_locale(None, None), "en", "環境変数が無いとき");
    }

    /// [`set_language`] はいつ呼んでも効く(2026-08-19 の決め)
    #[test]
    fn injected_language_takes_effect_any_time() {
        let _lock = serially();
        let from = *LANG.read().expect("言語の錠");
        assert!(set_language("de"));
        assert_eq!(language(), "de");
        assert!(set_language("ja"));
        assert_eq!(language(), "ja");
        assert!(!set_language("xx"), "知らない札は断る");
        *LANG.write().expect("言語の錠") = from;
    }
}
