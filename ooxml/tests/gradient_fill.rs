//! A shape filled with a gradient (`a:gradFill`, ECMA-376 20.1.8.33).
//!
//! The stops (`a:gs`, 20.1.8.36) are kept with their places, and a linear
//! gradient (`a:lin`, 20.1.8.41) keeps its angle. `fill` gets one colour for
//! anything that does not draw a gradient.

fn palette() -> Vec<String> {
    ["000000", "FFFFFF", "44546A", "E7E6E6", "4472C4", "ED7D31", "A5A5A5", "FFC000",
     "5B9BD5", "70AD47", "0563C1", "954F72"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn shape(sppr: &str) -> book::SheetShape {
    let a = format!(
        r#"<wp:anchor xmlns:wp="x" xmlns:a="y" xmlns:wps="z">
<wp:extent cx="1828800" cy="914400"/>
<wps:wsp><wps:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{sppr}</wps:spPr>
<wps:style><a:fillRef idx="1"><a:schemeClr val="accent2"/></a:fillRef></wps:style></wps:wsp></wp:anchor>"#
    );
    ooxml::foreign_shape_with(&a, &palette(), &[]).expect("shape not read").look
}

#[test]
fn a_linear_gradient_keeps_its_stops_and_angle() {
    let sp = shape(
        r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:schemeClr val="accent1"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="1"/></a:gradFill><a:ln><a:noFill/></a:ln>"#,
    );
    let g = sp.fill_grad.expect("gradient not read");
    assert_eq!(g.degree_c, 9000);
    assert_eq!(g.stops, vec![(0, "FF0000".to_string()), (1000, "4472C4".to_string())]);
    assert_eq!(g.path, None);
    // One colour for what cannot draw the gradient, not the style's colour
    assert_ne!(sp.fill.as_deref(), Some("ED7D31"));
    assert!(sp.fill.is_some());
}

#[test]
fn a_solid_fill_has_no_gradient() {
    let sp = shape(r#"<a:solidFill><a:srgbClr val="00FF00"/></a:solidFill>"#);
    assert_eq!(sp.fill_grad, None);
    assert_eq!(sp.fill.as_deref(), Some("00FF00"));
}
