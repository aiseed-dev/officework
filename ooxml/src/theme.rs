//! **`word/theme/theme1.xml` — 役ごとの書体。**
//!
//! docx は書体を2つの役で持ちます(2026-08-26 発注者、教師の実測どおり)。
//!
//! - `major` — **タイトルと見出し。ゴシック**
//! - `minor` — **本文。明朝**
//!
//! 等幅はここには入りません。コードの段落はスタイル(`等幅`・`塊の中`)が
//! 書体を名指しします — 役が3つあるのではなく、**2つの役+スタイルの
//! 名指し**という形です。
//!
//! # なぜ直に書かずテーマにするか
//!
//! 一度は `docDefaults` に書体の名前を直に書きました。**それでは役ごとに
//! 変えられません** — タイトルも本文も同じ書体になります。Word も
//! OnlyOffice もテーマ参照で書いていて、そちらが正しい形でした
//! (2026-08-26 発注者「これは、書くべきです」)。
//!
//! # 構成だけ借ります
//!
//! 教師(Word の空)の XML は写しません。`fontScheme` の形と、
//! `majorHAnsi` / `minorEastAsia` という参照の名前だけを借ります。

use kumihan::font::{default_language, script_of, Generic, Script};

/// テーマの名前。Word の既定は "Office"
const NAME: &str = "officework";

/// **役ごとの書体を決める。**(ラテン文字, 日本語などの字)を major・minor で。
///
/// 日本語は BIZ UD 系です(2026-08-26 発注者)。他の言語は
/// `kumihan::font` の候補の先頭を使います — 「標準フォントは OS と言語で
/// 変える」と1本の道です。
pub fn fonts() -> (String, String) {
    let lang = default_language();
    if script_of(&lang) == Script::Japanese {
        // タイトルはゴシック、本文は明朝
        return ("BIZ UDPゴシック".into(), "BIZ UDP明朝".into());
    }
    let pick = |g: Generic| {
        kumihan::font::default_generic(&lang, g)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| match g {
                Generic::SansSerif => "Arial".into(),
                Generic::Serif => "Times New Roman".into(),
            })
    };
    (pick(Generic::SansSerif), pick(Generic::Serif))
}

/// `theme1.xml` の字。
pub fn xml() -> String {
    let (major, minor) = fonts();
    let scheme = |role: &str, font: &str| {
        format!(
            r#"<a:{role}Font><a:latin typeface="{f}"/><a:ea typeface="{f}"/><a:cs typeface=""/>
<a:font script="Jpan" typeface="{f}"/><a:font script="Hang" typeface="{f}"/>
<a:font script="Hans" typeface="{f}"/><a:font script="Hant" typeface="{f}"/></a:{role}Font>"#,
            role = role,
            f = esc(font)
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="{name}"><a:themeElements><a:clrScheme name="{name}"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="{name}">{major}{minor}</a:fontScheme><a:fmtScheme name="{name}"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#,
        name = NAME,
        major = scheme("major", &major),
        minor = scheme("minor", &minor),
    )
}

/// **本文(minor)を指す参照。** `docDefaults` が使います
pub const MINOR_REF: &str = r#"<w:rFonts w:asciiTheme="minorHAnsi" w:hAnsiTheme="minorHAnsi" w:eastAsiaTheme="minorEastAsia" w:cstheme="minorBidi"/>"#;

/// **タイトルと見出し(major)を指す参照。** そのスタイルが使います
pub const MAJOR_REF: &str = r#"<w:rFonts w:asciiTheme="majorHAnsi" w:hAnsiTheme="majorHAnsi" w:eastAsiaTheme="majorEastAsia" w:cstheme="majorBidi"/>"#;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_is_gothic_and_the_body_is_mincho() {
        kumihan::font::set_default_language("ja");
        let (major, minor) = fonts();
        assert!(major.contains("ゴシック"), "タイトルがゴシックでない: {major}");
        assert!(minor.contains("明朝"), "本文が明朝でない: {minor}");
    }

    #[test]
    fn the_theme_names_both_roles() {
        kumihan::font::set_default_language("ja");
        let x = xml();
        assert!(x.contains("<a:majorFont>"), "major が無い");
        assert!(x.contains("<a:minorFont>"), "minor が無い");
        assert!(x.contains("BIZ UDPゴシック"), "ゴシックが入らない");
        assert!(x.contains("BIZ UDP明朝"), "明朝が入らない");
    }
}
