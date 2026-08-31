//! リボンの表の登録簿。**このファイルは ui/gen_lang.py が生成する。**
//! 手で書かない — 言語を足すときは gen_lang.py を回す。

use super::ribbon::Tab;

pub fn tabs(lang: &str) -> Option<(&'static [Tab], &'static [Tab])> {
    // 言語のファイルは語の対だけ。表は localized が骨組みへ差し込んで組む
    let (lang, words, by_id) = match lang {
        "de" => ("de", crate::ribbon_de::WORDS, crate::ribbon_de::WORDS_BY_ID),
        "en" => ("en", crate::ribbon_en::WORDS, crate::ribbon_en::WORDS_BY_ID),
        "es" => ("es", crate::ribbon_es::WORDS, crate::ribbon_es::WORDS_BY_ID),
        "fr" => ("fr", crate::ribbon_fr::WORDS, crate::ribbon_fr::WORDS_BY_ID),
        "id" => ("id", crate::ribbon_id::WORDS, crate::ribbon_id::WORDS_BY_ID),
        "it" => ("it", crate::ribbon_it::WORDS, crate::ribbon_it::WORDS_BY_ID),
        "ja" => ("ja", crate::ribbon_ja::WORDS, crate::ribbon_ja::WORDS_BY_ID),
        "ko" => ("ko", crate::ribbon_ko::WORDS, crate::ribbon_ko::WORDS_BY_ID),
        "pt" => ("pt", crate::ribbon_pt::WORDS, crate::ribbon_pt::WORDS_BY_ID),
        "pt-br" => ("pt-br", crate::ribbon_pt_br::WORDS, crate::ribbon_pt_br::WORDS_BY_ID),
        "ru" => ("ru", crate::ribbon_ru::WORDS, crate::ribbon_ru::WORDS_BY_ID),
        "tr" => ("tr", crate::ribbon_tr::WORDS, crate::ribbon_tr::WORDS_BY_ID),
        "vi" => ("vi", crate::ribbon_vi::WORDS, crate::ribbon_vi::WORDS_BY_ID),
        "zh" => ("zh", crate::ribbon_zh::WORDS, crate::ribbon_zh::WORDS_BY_ID),
        "zh-tw" => ("zh-tw", crate::ribbon_zh_tw::WORDS, crate::ribbon_zh_tw::WORDS_BY_ID),
        _ => return None,
    };
    Some(super::ribbon::localized(lang, words, by_id))
}
