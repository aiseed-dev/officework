//! **図形の塗りと線の色を、どこから取るか。**
//!
//! DrawingML は色を2か所に書けます。図形自身(`wps:spPr`)と、テーマの
//! 書式を指す参照(`wps:style` の `a:fillRef` / `a:lnRef`)です。
//! **図形自身の指定が強い** — 規格でもそうですし、Word もそう描きます。
//!
//! 2026-09-03。内閣府の面談の記録(document_4.docx)の枠は
//! `<a:noFill />` と言っているのに塗り潰され、紙が1枚まるごと濃い青に
//! なっていました。

/// テーマの配色(12色)。accent1 だけ見分けが付けばよいので、順番だけ
/// 本物に合わせます(dk1, lt1, dk2, lt2, accent1..6, hlink, folHlink)
fn irodana() -> Vec<String> {
    ["000000", "FFFFFF", "44546A", "E7E6E6", "4472C4", "ED7D31", "A5A5A5", "FFC000",
     "5B9BD5", "70AD47", "0563C1", "954F72"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn zukei(sppr: &str, style: &str) -> book::SheetShape {
    let a = format!(
        r#"<wp:anchor xmlns:wp="x" xmlns:a="y" xmlns:wps="z">
<wp:extent cx="1828800" cy="914400"/>
<wps:wsp><wps:spPr>{sppr}</wps:spPr>{style}</wps:wsp></wp:anchor>"#
    );
    ooxml::foreign_shape_with(&a, &irodana(), &[]).expect("図形が読めない").look
}

const KATACHI: &str = r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#;
const SANSHOU: &str = r#"<wps:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef></wps:style>"#;

#[test]
fn a_shape_that_says_no_fill_is_not_filled_even_with_a_style() {
    let sp = zukei(&format!("{KATACHI}<a:noFill />"), SANSHOU);
    assert_eq!(sp.fill, None, "塗らないと言っている図形を塗っている");
}

#[test]
fn a_shape_that_says_nothing_takes_the_fill_from_its_style() {
    let sp = zukei(KATACHI, SANSHOU);
    assert_eq!(
        sp.fill.as_deref(),
        Some("4472C4"),
        "スタイルの参照(a:fillRef の accent1)から塗りが取れていない"
    );
}

#[test]
fn a_style_that_points_at_nothing_gives_no_fill() {
    let nashi = r#"<wps:style><a:fillRef idx="0"><a:schemeClr val="accent1"/></a:fillRef></wps:style>"#;
    let sp = zukei(KATACHI, nashi);
    assert_eq!(sp.fill, None, "idx=\"0\"(書式なし)を塗りとして読んでいる");
}

#[test]
fn the_line_colour_falls_back_to_the_style_too() {
    // `<a:ln>` を持たない図形。線の色は `a:lnRef` から取ります
    let sp = zukei(KATACHI, SANSHOU);
    assert!(sp.line.is_some(), "スタイルの参照から線の色が取れていない");
}

#[test]
fn a_colour_inside_the_line_is_not_the_fill() {
    // 線だけを色で言っている図形。その色を塗りとして読むと、枠が塗り潰されます
    let sppr = format!(
        r#"{KATACHI}<a:noFill /><a:ln w="38100"><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:ln>"#
    );
    let sp = zukei(&sppr, SANSHOU);
    assert_eq!(sp.fill, None, "線の色で塗り潰している");
    assert_eq!(sp.line.as_deref(), Some("4472C4"), "線の色が読めていない");
}

/// A line whose `a:ln` gives only its colour takes its width from the theme
/// entry `a:lnRef` names (ECMA-376 20.1.4.1.20). The fold lines of Word's
/// engagement card name `idx="1"` (0.5pt) and were drawn at 1.5pt
#[test]
fn a_line_that_gives_only_its_colour_takes_the_width_from_its_style() {
    let a = format!(
        r#"<wp:anchor xmlns:wp="x" xmlns:a="y" xmlns:wps="z">
<wp:extent cx="1828800" cy="0"/>
<wps:wsp><wps:spPr><a:prstGeom prst="line"><a:avLst/></a:prstGeom><a:ln><a:solidFill><a:srgbClr val="D9D9D9"/></a:solidFill></a:ln></wps:spPr>{}</wps:wsp></wp:anchor>"#,
        SANSHOU.replace("idx=\"2\"", "idx=\"1\"")
    );
    let sp = ooxml::foreign_shape_with(&a, &irodana(), &[0.5, 1.0, 1.5]).expect("not read").look;
    assert_eq!(sp.line.as_deref(), Some("D9D9D9"), "the shape's own colour was lost");
    assert!((sp.line_w - 0.5).abs() < 0.01, "width {}", sp.line_w);
}
