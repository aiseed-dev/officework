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

/// Where a stretch of a filled document stands, in the counting the layout
/// uses: a body paragraph by its place in the body text, a table cell by
/// its table number and the bytes of the cell's text.
fn text_at(doc: &kumihan::Document, at: &kumihan::fill::DocAt) -> Option<paper::pdfw::TextAt> {
    use kumihan::Block;
    let para_len = |p: &kumihan::Paragraph| p.runs.iter().map(|r| r.text.len()).sum::<usize>();
    match at.cell {
        None => {
            let para0: usize = doc.blocks[..at.block]
                .iter()
                .filter_map(|b| match b {
                    Block::Para(p) => Some(para_len(p) + 1),
                    _ => None,
                })
                .sum();
            Some(paper::pdfw::TextAt::Body { para0, from: at.from, to: at.to })
        }
        Some((row, col, pi)) => {
            let table = doc.blocks[..at.block].iter().filter(|b| matches!(b, Block::Table(_))).count();
            let Block::Table(t) = &doc.blocks[at.block] else { return None };
            let cell = t.rows.get(row)?.get(col)?;
            let before: usize = cell.paragraphs[..pi].iter().map(|p| para_len(p) + 1).sum();
            Some(paper::pdfw::TextAt::Cell { table, row, col, from: before + at.from, to: before + at.to })
        }
    }
}

/// **A document form filled from data, with the chosen options circled**
/// (the document side of [`fill_form`]). The circles are shapes that stick
/// to their page, placed where the layout set the chosen chars.
pub fn fill_doc_form(form: &kumihan::Document, data: &book::Book) -> Result<kumihan::fill::FormFill, String> {
    let mut f = kumihan::fill::fill_form(form, data);
    if f.chosen.is_empty() {
        return Ok(f);
    }
    let pages = paper::doc_pages(&f.doc, None)?;
    let wants: Vec<(String, paper::pdfw::TextAt)> = f
        .chosen
        .iter()
        .enumerate()
        .filter_map(|(i, at)| text_at(&f.doc, at).map(|t| (i.to_string(), t)))
        .collect();
    for (page, s) in paper::pdfw::text_spots(&pages.sheet, pages.paper, &wants) {
        let h = pages.leaves.get(page).and_then(|l| l.size_mm).map(|(_, h)| h).unwrap_or(pages.paper.height_mm);
        let (pad_x, pad_y) = (s.h_mm * 0.3, s.h_mm * 0.15);
        f.doc.shapes.push(kumihan::DocShape {
            page,
            x_mm: s.x_mm - pad_x,
            y_mm: h - (s.y_mm + s.h_mm) - pad_y,
            w_mm: s.w_mm + 2.0 * pad_x,
            h_mm: s.h_mm + 2.0 * pad_y,
            look: book::SheetShape {
                kind: "ellipse".into(),
                line: Some("000000".into()),
                line_w: 0.75,
                ..Default::default()
            },
            z: 0,
        });
    }
    Ok(f)
}

/// The fonts of a laid-out document for the draw list: each name the text
/// uses, found on this machine for its file.
fn doc_font_files(fonts: &[(String, Vec<u8>)]) -> Vec<paper::drawlist::FontFile> {
    fonts
        .iter()
        .map(|(na, data)| {
            let fam = kumihan::font::for_document(Some(na)).ok().map(|(f, _)| f);
            paper::drawlist::FontFile {
                name: fam.map(|f| f.name.clone()).unwrap_or_else(|| na.clone()),
                file: fam.map(|f| f.path.display().to_string()).unwrap_or_default(),
                index: fam.map(|f| f.index).unwrap_or(0),
                data: data.clone(),
            }
        })
        .collect()
}

/// **A document's pages as a draw list**, with no fields.
pub fn doc(d: &kumihan::Document) -> Result<Value, String> {
    let p = paper::doc_pages(d, None)?;
    let mut v = paper::drawlist::pages(&p.leaves, (p.paper.width_mm, p.paper.height_mm), &doc_font_files(&p.fonts));
    for page in v["pages"].as_array_mut().into_iter().flatten() {
        let o = page.as_object_mut().expect("a page");
        o.remove("spots");
        o.insert("fields".into(), json!([]));
    }
    Ok(v)
}

/// **A filled document form as a draw list with its fields**: one field per
/// data item, its rectangle the box of the answers' chars on each page (an
/// empty answer, the char after it). `filled` is what [`fill_doc_form`]
/// made from `form` and `data`.
pub fn doc_form(form: &kumihan::Document, data: &book::Book, filled: &kumihan::Document) -> Result<Value, String> {
    let f = kumihan::fill::fill_form(form, data);
    let answers = book::form::Data::new(data);
    // The data items, in the order the form first names them
    let mut items: Vec<(String, book::form::Kind, Vec<String>)> = Vec::new();
    let mut wants: Vec<(String, paper::pdfw::TextAt)> = Vec::new();
    for (mark, at) in &f.marks {
        let Some((name, kind)) = book::form::item_of(mark) else { continue };
        let i = match items.iter().position(|(n, _, _)| *n == name) {
            Some(i) => i,
            None => {
                items.push((name, kind, Vec::new()));
                items.len() - 1
            }
        };
        for o in book::form::options_of(mark) {
            if !items[i].2.contains(&o) {
                items[i].2.push(o);
            }
        }
        if let Some(t) = text_at(filled, at) {
            wants.push((i.to_string(), t));
        }
    }
    let p = paper::doc_pages(filled, None)?;
    let spots = paper::pdfw::text_spots(&p.sheet, p.paper, &wants);
    let mut v = paper::drawlist::pages(&p.leaves, (p.paper.width_mm, p.paper.height_mm), &doc_font_files(&p.fonts));
    let pt = |mm: f32| ((mm * 72.0 / 25.4) as f64 * 100.0).round() / 100.0;
    for (k, page) in v["pages"].as_array_mut().into_iter().flatten().enumerate() {
        let h = p.leaves.get(k).and_then(|l| l.size_mm).map(|(_, h)| h).unwrap_or(p.paper.height_mm);
        let mut fields: Vec<(usize, Value)> = spots
            .iter()
            .filter(|(pk, _)| *pk == k)
            .filter_map(|(_, s)| {
                let i: usize = s.key.parse().ok()?;
                let (name, kind, options) = &items[i];
                let mut fv = json!({
                    "name": name, "kind": kind.name(),
                    "rect": [pt(s.x_mm), pt(h - s.y_mm - s.h_mm), pt(s.w_mm), pt(s.h_mm)],
                    "value": answers.raw(name).unwrap_or_default(),
                });
                if !options.is_empty() {
                    fv["options"] = json!(options);
                }
                Some((i, fv))
            })
            .collect();
        fields.sort_by_key(|(i, _)| *i);
        let o = page.as_object_mut().expect("a page");
        o.remove("spots");
        o.insert("fields".into(), Value::Array(fields.into_iter().map(|(_, f)| f).collect()));
    }
    v["missing"] = json!(f.missing);
    Ok(v)
}
