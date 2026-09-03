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

/// **DrawingML の色を RGB に解く。**
///
/// `<a:schemeClr val="accent1"><a:lumMod val="75000"/></a:schemeClr>` のような
/// 書き方を、文書のテーマの配色と、濃さの修飾から実際の色にします。
///
/// 式は LibreOffice の `oox/source/drawingml/color.cxx` と同じです。
///
/// * `lumMod` / `lumOff` — HSL に直して、明るさ(L)に掛ける・足す
/// * `shade` / `tint` — **ガンマを外した RGB** に直して掛ける
///   (`DEC_GAMMA = 2.3`)。`shade` は 0% が黒、`tint` は 0% が白
///
/// 前は Office の既定の配色を焼き込み、修飾を1つも見ていませんでした。
/// 内閣府の面談の記録の飾り枠は `accent1` に `lumMod 75%` で、この文書の
/// テーマの `accent1` は 418AB3 です。元は #316685 で出るのに、うちは
/// 既定の #4472C4 で出ていました(2026-09-03 発注者)。
pub fn dml_iro(seg: &str, palette: &[String]) -> Option<String> {
    // 色そのもの。`srgbClr` が先、無ければテーマの色
    let mut rgb = if let Some(i) = seg.find("<a:srgbClr val=\"") {
        let j = i + 16;
        let e = seg[j..].find('"')? + j;
        u32::from_str_radix(&seg[j..e], 16).ok()?
    } else if let Some(i) = seg.find("<a:sysClr ") {
        let e = seg[i..].find('>').map(|e| i + e)?;
        let t = &seg[i..e];
        let k = t.find("lastClr=\"")? + 9;
        let ee = t[k..].find('"')? + k;
        u32::from_str_radix(&t[k..ee], 16).ok()?
    } else {
        let i = seg.find("<a:schemeClr val=\"")? + 18;
        let e = seg[i..].find('"')? + i;
        theme_iro(&seg[i..e], palette)?
    };
    // 濃さの修飾。書いてある順に効かせます
    let mut at = 0usize;
    while let Some(k) = seg[at..].find("<a:") {
        let i = at + k;
        let e = match seg[i..].find('>') {
            Some(e) => i + e,
            None => break,
        };
        let tag = &seg[i..e];
        let na = tag[3..].split([' ', '/', '>']).next().unwrap_or("");
        let v = tag
            .find("val=\"")
            .and_then(|p| {
                let s2 = i + p + 5;
                seg[s2..].find('"').and_then(|q| seg[s2..s2 + q].parse::<f64>().ok())
            })
            .map(|v| v / 100_000.0);
        if let Some(v) = v {
            rgb = match na {
                "lumMod" => lum(rgb, v, true),
                "lumOff" => lum(rgb, v, false),
                "shade" => senkei(rgb, |c| c * v),
                "tint" => senkei(rgb, |c| 1.0 - (1.0 - c) * v),
                _ => rgb,
            };
        }
        at = e + 1;
    }
    Some(format!("{:06X}", rgb))
}

/// テーマの配色を名前で引く。並びは `dk1 lt1 dk2 lt2 accent1..6 hlink folHlink`
fn theme_iro(na: &str, palette: &[String]) -> Option<u32> {
    let k = match na {
        "dk1" | "tx1" => 0,
        "lt1" | "bg1" => 1,
        "dk2" | "tx2" => 2,
        "lt2" | "bg2" => 3,
        "accent1" => 4,
        "accent2" => 5,
        "accent3" => 6,
        "accent4" => 7,
        "accent5" => 8,
        "accent6" => 9,
        "hlink" => 10,
        "folHlink" => 11,
        "phClr" => return None,
        _ => return None,
    };
    // 文書のテーマが引ければそれ。無ければ Office の既定
    let kitei = [
        "000000", "FFFFFF", "44546A", "E7E6E6", "4472C4", "ED7D31", "A5A5A5", "FFC000",
        "5B9BD5", "70AD47", "0563C1", "954F72",
    ];
    let s = palette.get(k).map(|s| s.as_str()).filter(|s| s.len() == 6).unwrap_or(kitei[k]);
    u32::from_str_radix(s, 16).ok()
}

/// HSL の明るさに掛ける(`lumMod`)か足す(`lumOff`)
fn lum(rgb: u32, v: f64, kakeru: bool) -> u32 {
    let (r, g, b) = (
        ((rgb >> 16) & 255) as f64 / 255.0,
        ((rgb >> 8) & 255) as f64 / 255.0,
        (rgb & 255) as f64 / 255.0,
    );
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    let s = if d == 0.0 {
        0.0
    } else if l < 0.5 {
        d / (mx + mn)
    } else {
        d / (2.0 - mx - mn)
    };
    let h = if d == 0.0 {
        0.0
    } else if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    let mut l2 = if kakeru { l * v } else { l + v };
    l2 = l2.clamp(0.0, 1.0);
    // 白か黒になったら彩度は落ちます(LibreOffice も同じ)
    let s2 = if l2 <= 0.0 || l2 >= 1.0 { 0.0 } else { s };
    let c = (1.0 - (2.0 * l2 - 1.0).abs()) * s2;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l2 - c / 2.0;
    let (r2, g2, b2) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let f = |v: f64| (((v + m) * 255.0).round().clamp(0.0, 255.0)) as u32;
    (f(r2) << 16) | (f(g2) << 8) | f(b2)
}

/// ガンマを外した RGB で計算します(`shade` / `tint`)。LibreOffice の
/// `toCrgb` と同じく、ガンマは 2.3 です
fn senkei(rgb: u32, f: impl Fn(f64) -> f64) -> u32 {
    const G: f64 = 2.3;
    let one = |v: u32| {
        let c = (v as f64 / 255.0).powf(G);
        let c2 = f(c).clamp(0.0, 1.0);
        (c2.powf(1.0 / G) * 255.0).round() as u32
    };
    (one((rgb >> 16) & 255) << 16) | (one((rgb >> 8) & 255) << 8) | one(rgb & 255)
}

/// **テーマの配色を並びで取り出す**(`a:clrScheme`)。
///
/// 並びは docx の書き順で `dk1 lt1 dk2 lt2 accent1..6 hlink folHlink` の12色。
/// `sysClr` は `lastClr` を使います。読めなければ空を返し、
/// [`dml_iro`] が Office の既定に落とします。
pub fn clr_scheme(xml: &str) -> Vec<String> {
    let Some(i) = xml.find("<a:clrScheme") else { return Vec::new() };
    let e = xml[i..].find("</a:clrScheme>").map(|e| i + e).unwrap_or(xml.len());
    let naka = &xml[i..e];
    let mut out = Vec::new();
    for na in [
        "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4",
        "accent5", "accent6", "hlink", "folHlink",
    ] {
        let pat = format!("<a:{na}>");
        let iro = naka.find(&pat).and_then(|k| {
            let owari = naka[k..].find(&format!("</a:{na}>")).map(|e| k + e)?;
            let seg = &naka[k..owari];
            for key in ["<a:srgbClr val=\"", "lastClr=\""] {
                if let Some(p) = seg.find(key) {
                    let s2 = p + key.len();
                    if let Some(q) = seg[s2..].find('"') {
                        return Some(seg[s2..s2 + q].to_string());
                    }
                }
            }
            None
        });
        out.push(iro.unwrap_or_default());
    }
    out
}
