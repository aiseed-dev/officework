//! `xl/theme/theme1.xml` の読み書き。
//!
//! 色の組そのもの(既定の色・名前つきの配色・番号と明るさから色を解く)は
//! **模型の側**([`kumihan::book::theme`])にあります。ここに置くのは
//! xlsx の XML との出し入れだけです(2026-08-26。SEKKEI「エンジンは
//! 3つに分ける」)。
//!
//! 番号の並びは Excel の `theme=` の流儀:
//! 0=背景1 1=文字1 2=背景2 3=文字2 4〜9=アクセント1〜6 10=リンク 11=既読リンク。
//! (theme1.xml の中の並びは dk1,lt1,dk2,lt2,… なので読むときに入れ替える)

use book::theme::OFFICE;

/// `theme1.xml` から色の組を読む。読めない部分は Office の色で埋める。
pub fn parse(xml: &str) -> Vec<String> {
    // clrScheme の中を順に拾う: dk1, lt1, dk2, lt2, accent1..6, hlink, folHlink
    let body = match (xml.find("<a:clrScheme"), xml.find("</a:clrScheme>")) {
        (Some(i), Some(j)) if j > i => &xml[i..j],
        _ => return OFFICE.iter().map(|s| s.to_string()).collect(),
    };
    let one = |tag: &str| -> Option<String> {
        let i = body.find(&format!("<a:{tag}>"))?;
        let rest = &body[i..];
        let end = rest.find(&format!("</a:{tag}>"))?;
        let seg = &rest[..end];
        // srgbClr val="RRGGBB" か、sysClr lastClr="RRGGBB"
        for key in ["srgbClr val=\"", "sysClr lastClr=\"", "lastClr=\""] {
            if let Some(a) = seg.find(key) {
                let a = a + key.len();
                if let Some(b) = seg[a..].find('"') {
                    let v = &seg[a..a + b];
                    if v.len() == 6 {
                        return Some(v.to_uppercase());
                    }
                }
            }
        }
        None
    };
    let g = |tag: &str, dflt: &str| one(tag).unwrap_or_else(|| dflt.to_string());
    // theme= の番号の並びへ入れ替える(背景1=lt1 が 0 番)
    vec![
        g("lt1", OFFICE[0]),
        g("dk1", OFFICE[1]),
        g("lt2", OFFICE[2]),
        g("dk2", OFFICE[3]),
        g("accent1", OFFICE[4]),
        g("accent2", OFFICE[5]),
        g("accent3", OFFICE[6]),
        g("accent4", OFFICE[7]),
        g("accent5", OFFICE[8]),
        g("accent6", OFFICE[9]),
        g("hlink", OFFICE[10]),
        g("folHlink", OFFICE[11]),
    ]
}

/// テーマの部品(書き出し用)。読んだ色の組をそのまま返す最小の形。
pub fn to_xml(colors: &[String]) -> String {
    let c = |i: usize| colors.get(i).map(|s| s.as_str()).unwrap_or(OFFICE[i]);
    // theme1.xml の中の並びは dk1, lt1, dk2, lt2, accent…(番号とは逆)
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office">"#,
            r#"<a:themeElements><a:clrScheme name="Office">"#,
            r#"<a:dk1><a:srgbClr val="{dk1}"/></a:dk1><a:lt1><a:srgbClr val="{lt1}"/></a:lt1>"#,
            r#"<a:dk2><a:srgbClr val="{dk2}"/></a:dk2><a:lt2><a:srgbClr val="{lt2}"/></a:lt2>"#,
            r#"<a:accent1><a:srgbClr val="{a1}"/></a:accent1><a:accent2><a:srgbClr val="{a2}"/></a:accent2>"#,
            r#"<a:accent3><a:srgbClr val="{a3}"/></a:accent3><a:accent4><a:srgbClr val="{a4}"/></a:accent4>"#,
            r#"<a:accent5><a:srgbClr val="{a5}"/></a:accent5><a:accent6><a:srgbClr val="{a6}"/></a:accent6>"#,
            r#"<a:hlink><a:srgbClr val="{hl}"/></a:hlink><a:folHlink><a:srgbClr val="{fl}"/></a:folHlink>"#,
            r#"</a:clrScheme><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri"/>"#,
            r#"<a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/>"#,
            r#"<a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme>"#,
            r#"<a:fmtScheme name="Office"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
            r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>"#,
            r#"<a:lnStyleLst><a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>"#,
            r#"<a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>"#,
            r#"<a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>"#,
            r#"<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle>"#,
            r#"<a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>"#,
            r#"<a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
            r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst>"#,
            r#"</a:fmtScheme></a:themeElements></a:theme>"#
        ),
        dk1 = c(1),
        lt1 = c(0),
        dk2 = c(3),
        lt2 = c(2),
        a1 = c(4),
        a2 = c(5),
        a3 = c(6),
        a4 = c(7),
        a5 = c(8),
        a6 = c(9),
        hl = c(10),
        fl = c(11),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_read_from_the_theme_part() {
        let xml = r#"<a:theme><a:themeElements><a:clrScheme name="x">
            <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
            <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
            <a:dk2><a:srgbClr val="44546A"/></a:dk2>
            <a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>
            <a:accent1><a:srgbClr val="4472C4"/></a:accent1>
            <a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
            </a:clrScheme></a:themeElements></a:theme>"#;
        let c = parse(xml);
        assert_eq!(c[0], "FFFFFF", "背景1(lt1)が 0 番でない");
        assert_eq!(c[1], "000000", "文字1(dk1)が 1 番でない");
        assert_eq!(c[4], "4472C4", "アクセント1が 4 番でない");
    }

    #[test]
    fn with_no_part_the_default_set_is_used() {
        let c = parse("<a:theme/>");
        assert_eq!(c.len(), 12);
        assert_eq!(c[4], OFFICE[4]);
    }

    #[test]
    fn writing_then_reading_gives_the_same_set() {
        let mut c: Vec<String> = OFFICE.iter().map(|s| s.to_string()).collect();
        c[4] = "C0504D".into();
        let back = parse(&to_xml(&c));
        assert_eq!(back, c, "テーマの往復が壊れている");
    }
}
