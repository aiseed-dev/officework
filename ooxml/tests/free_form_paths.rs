//! Points of a free-form shape (`a:custGeom`, ECMA-376 20.1.9.8).
//!
//! The path list (`a:pathLst`, 20.1.9.16) holds one or more paths. Each path
//! has its own coordinate space `w` x `h` (20.1.9.15), and the points are read
//! as fractions of it so that the shape box scales them.

fn shape(geom: &str) -> book::SheetShape {
    let a = format!(
        r#"<wp:anchor xmlns:wp="x" xmlns:a="y" xmlns:wps="z">
<wp:extent cx="1828800" cy="914400"/>
<wps:wsp><wps:spPr>{geom}<a:noFill/></wps:spPr></wps:wsp></wp:anchor>"#
    );
    ooxml::foreign_shape_with(&a, &[], &[]).expect("shape not read").look
}

fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
}

#[test]
fn lines_and_curves_become_points() {
    let sp = shape(
        r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="r" b="b"/>
<a:pathLst><a:path w="100" h="200">
<a:moveTo><a:pt x="0" y="0"/></a:moveTo>
<a:lnTo><a:pt x="100" y="0"/></a:lnTo>
<a:cubicBezTo><a:pt x="100" y="50"/><a:pt x="50" y="100"/><a:pt x="0" y="200"/></a:cubicBezTo>
<a:close/>
</a:path></a:pathLst></a:custGeom>"#,
    );
    assert_eq!(sp.kind, "path");
    let p = &sp.points;
    assert!(p.len() >= 3, "points not read: {p:?}");
    assert!(close(p[0].at, (0.0, 0.0)));
    assert!(close(p[1].at, (1.0, 0.0)));
    assert!(close(p[1].c_out.expect("no control point out"), (1.0, 0.25)));
    assert!(close(p[2].at, (0.0, 1.0)));
    assert!(close(p[2].c_in.expect("no control point in"), (0.5, 0.5)));
    // close draws back to the start of the outline
    assert!(close(p.last().unwrap().at, (0.0, 0.0)), "close did not return: {p:?}");
}

#[test]
fn a_second_move_starts_a_new_outline_and_each_path_has_its_own_size() {
    let sp = shape(
        r#"<a:custGeom><a:pathLst>
<a:path w="10" h="10"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="10" y="10"/></a:lnTo></a:path>
<a:path w="20" h="20"><a:moveTo><a:pt x="10" y="0"/></a:moveTo><a:lnTo><a:pt x="20" y="20"/></a:lnTo></a:path>
</a:pathLst></a:custGeom>"#,
    );
    let p = &sp.points;
    assert_eq!(p.len(), 4, "{p:?}");
    assert!(close(p[1].at, (1.0, 1.0)));
    assert!(p[2].start, "second path does not start a new outline");
    assert!(close(p[2].at, (0.5, 0.0)));
    assert!(close(p[3].at, (1.0, 1.0)));
}

#[test]
fn a_quadratic_curve_becomes_a_cubic_one() {
    let sp = shape(
        r#"<a:custGeom><a:pathLst><a:path w="30" h="30">
<a:moveTo><a:pt x="0" y="0"/></a:moveTo>
<a:quadBezTo><a:pt x="30" y="0"/><a:pt x="30" y="30"/></a:quadBezTo>
</a:path></a:pathLst></a:custGeom>"#,
    );
    let p = &sp.points;
    assert_eq!(p.len(), 2, "{p:?}");
    // Control points of the same curve: start + 2/3 (q - start), end + 2/3 (q - end)
    assert!(close(p[0].c_out.unwrap(), (2.0 / 3.0, 0.0)));
    assert!(close(p[1].c_in.unwrap(), (1.0, 1.0 / 3.0)));
}
