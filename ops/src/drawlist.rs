//! **The draw list of a book or a filled form** (docs/sekkei/drawlist.ja.adoc).
//!
//! The same layout as the PDF ([`crate::pdf::book`]), handed over as data:
//! pages of items in points from the top left, the fonts by file, and for
//! a form the fields to edit with their places on the pages.

use paper::drawlist::serde_json::{json, Value};
use paper::grid::Wanted;

/// The fonts of a book as the draw list takes them, in the order the
/// layout numbers them.
fn font_files(b: &book::Book) -> Result<Vec<(String, paper::drawlist::FontFile)>, String> {
    Ok(crate::pdf::book_fonts(b)?
        .into_iter()
        .filter_map(|(na, fam)| {
            let data = kumihan::font::load(fam).ok()?;
            Some((na, paper::drawlist::FontFile {
                name: fam.name.clone(),
                file: fam.path.display().to_string(),
                index: fam.index,
                data,
            }))
        })
        .collect())
}

fn lay_out(b: &book::Book, wanted: &[Wanted]) -> Result<Value, String> {
    let sheets = crate::pdf::printed_sheets(b)?;
    let files = font_files(b)?;
    let fonts: Vec<(String, Vec<u8>)> = files.iter().map(|(na, f)| (na.clone(), f.data.clone())).collect();
    let leaves = paper::grid::book_leaves_fonts(&sheets, &fonts, wanted)?;
    let first = sheets[0].1;
    let ff: Vec<paper::drawlist::FontFile> = files.into_iter().map(|(_, f)| f).collect();
    Ok(paper::drawlist::pages(&leaves, (first.width_mm, first.height_mm), &ff))
}

/// **A book's pages as a draw list**, with no fields.
pub fn book(b: &book::Book) -> Result<Value, String> {
    let mut v = lay_out(b, &[])?;
    for p in v["pages"].as_array_mut().into_iter().flatten() {
        let o = p.as_object_mut().expect("a page");
        o.remove("spots");
        o.insert("fields".into(), json!([]));
    }
    Ok(v)
}

/// **A form filled from data, as a draw list with its fields.** `filled`
/// is the form with the data put in ([`book::form::fill_in`], recalculated).
/// Each field gets one rectangle per page it shows on: the smallest one
/// holding all its cells (merged ranges whole) or its shape.
pub fn form(form: &book::Book, data: &book::Book, filled: &book::Book) -> Result<Value, String> {
    let groups = book::form::groups(form, data);
    // One request per printed sheet, in the order the layout takes them
    let printed: Vec<usize> = (0..filled.sheets.len()).filter(|i| !filled.sheets[*i].hidden).collect();
    let mut wanted: Vec<Wanted> = vec![Wanted::default(); printed.len()];
    for (gi, g) in groups.iter().enumerate() {
        let key = gi.to_string();
        for (si, at) in &g.cells {
            let Some(w) = printed.iter().position(|p| p == si) else { continue };
            let s = &filled.sheets[*si];
            let (a, z) = s
                .merges
                .iter()
                .find(|(a, z)| (a.row..=z.row).contains(&at.row) && (a.col..=z.col).contains(&at.col))
                .copied()
                .unwrap_or((*at, *at));
            for r in a.row..=z.row {
                for c in a.col..=z.col {
                    wanted[w].cells.insert(book::Pos::new(r, c), key.clone());
                }
            }
        }
        if let Some((si, k)) = g.shape {
            if let Some(w) = printed.iter().position(|p| *p == si) {
                wanted[w].shapes.insert(k, key.clone());
            }
        }
    }
    let mut v = lay_out(filled, &wanted)?;
    for p in v["pages"].as_array_mut().into_iter().flatten() {
        // Spots filed under a group's number → that group's rectangle here
        let mut boxes: Vec<(usize, [f64; 4])> = Vec::new();
        for s in p["spots"].as_array().into_iter().flatten() {
            let Some(gi) = s["key"].as_str().and_then(|k| k.parse::<usize>().ok()) else { continue };
            let r: Vec<f64> = s["rect"].as_array().into_iter().flatten().filter_map(|x| x.as_f64()).collect();
            let [x, y, w, h] = r[..] else { continue };
            match boxes.iter_mut().find(|(g, _)| *g == gi) {
                Some((_, b)) => {
                    let (x1, y1) = ((b[0] + b[2]).max(x + w), (b[1] + b[3]).max(y + h));
                    b[0] = b[0].min(x);
                    b[1] = b[1].min(y);
                    b[2] = x1 - b[0];
                    b[3] = y1 - b[1];
                }
                None => boxes.push((gi, [x, y, w, h])),
            }
        }
        boxes.sort_by_key(|(g, _)| *g);
        let fields: Vec<Value> = boxes
            .iter()
            .map(|(gi, b)| {
                let g = &groups[*gi];
                let r = |v: f64| (v * 100.0).round() / 100.0;
                json!({"name": g.name, "kind": g.kind.name(),
                       "rect": [r(b[0]), r(b[1]), r(b[2]), r(b[3])], "value": g.value})
            })
            .collect();
        let o = p.as_object_mut().expect("a page");
        o.remove("spots");
        o.insert("fields".into(), Value::Array(fields));
    }
    // The names the data lacks altogether, for the drawer to point out
    v["missing"] = json!(book::form::missing(form, data));
    Ok(v)
}
