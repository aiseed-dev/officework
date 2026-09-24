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

/// **A form filled from data, with the chosen options circled.**
///
/// The options of a choice mark (`{性別:男・女}`) are written out, and the
/// one the data names gets an ellipse around it. Where it is comes from the
/// page itself: the pages are laid out once, the layout reports the box its
/// chars were set in, and the ellipse is anchored to the cell at that
/// offset, so it shows in the PDF, the draw list and the xlsx alike.
pub fn fill_form(
    form: &book::Book,
    data: &book::Book,
    dir: Option<&std::path::Path>,
) -> Result<book::Book, String> {
    let (mut filled, choices) = book::form::fill_choices(form, data, dir);
    if choices.is_empty() {
        return Ok(filled);
    }
    let printed: Vec<usize> = (0..filled.sheets.len()).filter(|i| !filled.sheets[*i].hidden).collect();
    let mut wanted: Vec<Wanted> = vec![Wanted::default(); printed.len()];
    for (i, c) in choices.iter().enumerate() {
        let Some(w) = printed.iter().position(|p| *p == c.sheet) else { continue };
        wanted[w].text.entry(c.at).or_default().push((format!("t{i}"), c.start, c.len));
        wanted[w].cells.insert(c.at, format!("c{i}"));
    }
    let sheets = crate::pdf::printed_sheets(&filled)?;
    let fonts: Vec<(String, Vec<u8>)> = font_files(&filled)?
        .into_iter()
        .map(|(na, f)| (na, f.data))
        .collect();
    let leaves = paper::grid::book_leaves_fonts(&sheets, &fonts, &wanted)?;
    drop(sheets);
    let px = |mm: f32| mm * 96.0 / 25.4;
    for (i, c) in choices.iter().enumerate() {
        let find = |key: &str| {
            leaves.iter().find_map(|l| l.spots.iter().find(|s| s.key == key).cloned())
        };
        let (Some(t), Some(cell)) = (find(&format!("t{i}")), find(&format!("c{i}"))) else { continue };
        let s = &filled.sheets[c.sheet];
        // The print scale: the cell as laid out against its width in the book
        let own = s.col_haba_mm(c.at.col, &filled.col_basis);
        let scale = if own > 0.0 { cell.w_mm / own } else { 1.0 };
        // A little room round the chars (the em box)
        let (pad_x, pad_y) = (t.h_mm * 0.3, t.h_mm * 0.15);
        let left = t.x_mm - pad_x - cell.x_mm;
        let top = (cell.y_mm + cell.h_mm) - (t.y_mm + t.h_mm) - pad_y;
        let sp = book::SheetShape {
            at: c.at,
            dx_px: px(left / scale).max(0.0),
            dy_px: px(top / scale).max(0.0),
            width_px: px((t.w_mm + 2.0 * pad_x) / scale),
            height_px: px((t.h_mm + 2.0 * pad_y) / scale),
            kind: "ellipse".into(),
            line: Some("000000".into()),
            line_w: 0.75,
            ..Default::default()
        };
        filled.sheets[c.sheet].shapes_new.push(sp);
    }
    Ok(filled)
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
                let mut f = json!({"name": g.name, "kind": g.kind.name(),
                       "rect": [r(b[0]), r(b[1]), r(b[2]), r(b[3])], "value": g.value});
                if !g.options.is_empty() {
                    f["options"] = json!(g.options);
                }
                f
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
