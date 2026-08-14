//! 関数の言葉の登録簿。**このファイルは calc/gen_funcs.py が生成する。**
//! 手で書かない — 言語を足すときは gen_funcs.py --all を回す。

use super::funcs::FnText;

pub fn text(lang: &str) -> Option<&'static [FnText]> {
    match lang {
        "de" => Some(crate::funcs_de::TEXT),
        "en" => Some(crate::funcs_en::TEXT),
        "es" => Some(crate::funcs_es::TEXT),
        "fr" => Some(crate::funcs_fr::TEXT),
        "id" => Some(crate::funcs_id::TEXT),
        "it" => Some(crate::funcs_it::TEXT),
        "ko" => Some(crate::funcs_ko::TEXT),
        "pt" => Some(crate::funcs_pt::TEXT),
        "pt-br" => Some(crate::funcs_pt_br::TEXT),
        "ru" => Some(crate::funcs_ru::TEXT),
        "tr" => Some(crate::funcs_tr::TEXT),
        "vi" => Some(crate::funcs_vi::TEXT),
        "zh" => Some(crate::funcs_zh::TEXT),
        "zh-tw" => Some(crate::funcs_zh_tw::TEXT),
        _ => None,
    }
}
