//! Read an xlsx and the ods LibreOffice made from it, and list where the two
//! workbooks differ: sheets, cell values, formulas and merged cells.
//!
//!     cargo run -q -p sheet --example ods_vs_xlsx -- ~/xlsx-corpus ~/ods-corpus
//!
//! With `--fmt` it also counts, per property, the cells whose formatting
//! differs (bold, fill, font, size, borders, alignment, wrapping, number
//! format), with one example each.
//!
//! Every `<stem>.ods` in the second folder is paired with `<stem>.xlsx` in the
//! first. The ods comes from `tools/lo_pdf.py --to ods`, so a difference is
//! either this reader or LibreOffice's own conversion; the list says which
//! cells to look at, not who is right.

use std::fs::File;
use std::path::Path;

use book::{Book, Value};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let fmt = args.iter().any(|a| a == "--fmt");
    args.retain(|a| a != "--fmt");
    let [xdir, odir] = args.as_slice() else {
        eprintln!("usage: ods_vs_xlsx XLSX_DIR ODS_DIR");
        std::process::exit(2);
    };
    let mut files: Vec<_> = std::fs::read_dir(odir)
        .expect("ods folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ods"))
        .collect();
    files.sort();
    let (mut same, mut total) = (0, 0);
    for ods in files {
        let stem = ods.file_stem().unwrap().to_string_lossy().to_string();
        let xlsx = Path::new(xdir).join(format!("{stem}.xlsx"));
        if !xlsx.exists() {
            continue;
        }
        total += 1;
        let o = match sheet::ods::read(File::open(&ods).unwrap()) {
            Ok((b, rep)) => {
                if !rep.unsupported.is_empty() {
                    println!("{stem}: ods report {:?}", rep.unsupported);
                }
                b
            }
            Err(e) => {
                println!("{stem}: ods FAILED {e}");
                continue;
            }
        };
        let Ok((x, _)) = sheet::xlsx::read(File::open(&xlsx).unwrap()) else {
            println!("{stem}: xlsx unreadable");
            continue;
        };
        let diffs = if fmt { compare_fmt(&x, &o) } else { compare(&x, &o) };
        if diffs.is_empty() {
            same += 1;
        } else {
            println!("{stem}: {} differences", diffs.len());
            for d in diffs.iter().take(8) {
                println!("  {d}");
            }
        }
    }
    println!("same {same} / {total}");
}

fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => (x - y).abs() <= 1e-9 * x.abs().max(1.0),
        _ => a == b,
    }
}

/// Where a shape is, how big, and what it shows
type ShapeKey = (book::Pos, String, i32, i32, Option<String>, Option<String>, Option<String>);

fn compare(x: &Book, o: &Book) -> Vec<String> {
    let mut out = Vec::new();
    let xn: Vec<_> = x.sheets.iter().map(|s| s.name.as_str()).collect();
    let on: Vec<_> = o.sheets.iter().map(|s| s.name.as_str()).collect();
    if xn != on {
        out.push(format!("sheets {xn:?} vs {on:?}"));
    }
    for (xs, os) in x.sheets.iter().zip(&o.sheets) {
        for (p, c) in &xs.cells {
            let oc = os.get(*p);
            let ov = oc.map(|c| c.value.clone()).unwrap_or(Value::Empty);
            if !close(&c.value, &ov) {
                out.push(format!("{}!{:?} value {:?} vs {:?}", xs.name, p, c.value, ov));
            }
            let of = oc.and_then(|c| c.formula.clone());
            if c.formula != of {
                out.push(format!("{}!{:?} formula {:?} vs {:?}", xs.name, p, c.formula, of));
            }
        }
        for (p, c) in &os.cells {
            if xs.get(*p).is_none() && !c.value.is_empty() {
                out.push(format!("{}!{:?} only in ods {:?}", os.name, p, c.value));
            }
        }
        // Comments, pictures and shapes: where they are anchored
        let xc: Vec<_> = xs.comments.iter().map(|(p, t)| (*p, t.text().to_string())).collect();
        let oc: Vec<_> = os.comments.iter().map(|(p, t)| (*p, t.text().to_string())).collect();
        if xc != oc {
            out.push(format!("{} comments {xc:?} vs {oc:?}", xs.name));
        }
        let place = |v: &[book::SheetImage]| -> Vec<(book::Pos, i32, i32, i32, i32)> {
            let mut v: Vec<_> = v.iter().map(|i| (i.at, i.dx_px.round() as i32, i.dy_px.round() as i32, i.width_px.round() as i32, i.height_px.round() as i32)).collect();
            v.sort();
            v
        };
        if place(&xs.images) != place(&os.images) {
            out.push(format!("{} pictures {:?} vs {:?}", xs.name, place(&xs.images), place(&os.images)));
        }
        let shp = |v: &[book::SheetShape]| -> Vec<ShapeKey> {
            let mut v: Vec<_> = v.iter().map(|s| (s.at, s.kind.clone(), s.width_px.round() as i32, s.height_px.round() as i32, s.fill.clone(), s.line.clone(), s.text.clone())).collect();
            v.sort();
            v
        };
        if shp(&xs.shapes) != shp(&os.shapes) {
            out.push(format!("{} shapes {:?} vs {:?}", xs.name, shp(&xs.shapes), shp(&os.shapes)));
        }
        let vals = |v: &[book::Validation]| -> Vec<String> {
            let mut out: Vec<String> = v.iter().map(|x| format!("{:?}", x)).collect();
            out.sort();
            out
        };
        if vals(&xs.validations) != vals(&os.validations) {
            out.push(format!("{} validations {:?} vs {:?}", xs.name, vals(&xs.validations), vals(&os.validations)));
        }
        let mut xm = xs.merges.clone();
        let mut om = os.merges.clone();
        xm.sort();
        om.sort();
        if xm != om {
            let only_x: Vec<_> = xm.iter().filter(|m| !om.contains(m)).collect();
            let only_o: Vec<_> = om.iter().filter(|m| !xm.contains(m)).collect();
            out.push(format!("{} merges only in xlsx {only_x:?}, only in ods {only_o:?}", xs.name));
        }
    }
    out
}

fn compare_fmt(x: &Book, o: &Book) -> Vec<String> {
    use std::collections::BTreeMap;
    let mut count: BTreeMap<&str, (usize, String)> = BTreeMap::new();
    for (xs, os) in x.sheets.iter().zip(&o.sheets) {
        for (p, c) in &xs.cells {
            let a = &c.fmt;
            let d = os.get(*p).map(|c| c.fmt.clone()).unwrap_or_default();
            let mut note = |k: &'static str, l: String| {
                let e = count.entry(k).or_insert((0, String::new()));
                if e.0 == 0 {
                    e.1 = format!("{}!{:?} {l}", xs.name, p);
                }
                e.0 += 1;
            };
            if a.bold != d.bold { note("bold", format!("{} vs {}", a.bold, d.bold)); }
            if a.italic != d.italic { note("italic", format!("{} vs {}", a.italic, d.italic)); }
            if a.fill != d.fill { note("fill", format!("{:?} vs {:?}", a.fill, d.fill)); }
            if a.font != d.font { note("font", format!("{:?} vs {:?}", a.font, d.font)); }
            if a.size_c != d.size_c { note("size", format!("{:?} vs {:?}", a.size_c, d.size_c)); }
            if a.borders != d.borders { note("borders", format!("{:?} vs {:?}", a.borders.top, d.borders.top)); }
            if a.align != d.align { note("align", format!("{:?} vs {:?}", a.align, d.align)); }
            if a.valign != d.valign { note("valign", format!("{:?} vs {:?}", a.valign, d.valign)); }
            if a.wrap != d.wrap { note("wrap", format!("{} vs {}", a.wrap, d.wrap)); }
            if a.number_format != d.number_format { note("number_format", format!("{:?} vs {:?}", a.number_format, d.number_format)); }
        }
    }
    count.into_iter().map(|(k, (n, ex))| format!("{k} {n}: {ex}")).collect()
}
