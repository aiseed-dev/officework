//! Read each ods in a folder, write it with `sheet::ods::write`, read the
//! written file back, and list what changed on the way: sheets, values,
//! formulas, merged cells, column widths, row heights and cell formatting.
//!
//!     cargo run -q -p sheet --example ods_round -- ~/ods-corpus OUT_DIR
//!
//! The written files are kept in OUT_DIR so LibreOffice can be asked to
//! open them (`tools/lo_pdf.py OUT_DIR/*.ods`) and their PDFs compared with
//! the originals' (`tools/ms_compare.py`).

use std::fs::File;
use std::path::Path;

use book::{Book, Value};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [src, out] = args.as_slice() else {
        eprintln!("usage: ods_round ODS_DIR OUT_DIR");
        std::process::exit(2);
    };
    std::fs::create_dir_all(out).expect("out folder");
    let mut files: Vec<_> = std::fs::read_dir(src)
        .expect("ods folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ods"))
        .collect();
    files.sort();
    let (mut same, mut total) = (0, 0);
    for f in files {
        let stem = f.file_stem().unwrap().to_string_lossy().to_string();
        let Ok((a, _)) = sheet::ods::read(File::open(&f).unwrap()) else {
            println!("{stem}: unreadable");
            continue;
        };
        total += 1;
        let (bytes, rep) = sheet::ods::write(&a);
        let dst = Path::new(out).join(format!("{stem}.ods"));
        std::fs::write(&dst, &bytes).unwrap();
        let b = match sheet::ods::read(std::io::Cursor::new(bytes)) {
            Ok((b, _)) => b,
            Err(e) => {
                println!("{stem}: written file unreadable: {e}");
                continue;
            }
        };
        let d = diff(&a, &b);
        if !rep.left_out.is_empty() {
            println!("{stem}: left out {:?}", rep.left_out);
        }
        if d.is_empty() {
            same += 1;
        } else {
            println!("{stem}: {} differences", d.len());
            for x in d.iter().take(6) {
                println!("  {x}");
            }
        }
    }
    println!("same {same} / {total}");
}

fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => (x - y).abs() <= 1e-12 * x.abs().max(1.0),
        _ => a == b,
    }
}

fn diff(a: &Book, b: &Book) -> Vec<String> {
    let mut out = Vec::new();
    let an: Vec<_> = a.sheets.iter().map(|s| (&s.name, s.hidden)).collect();
    let bn: Vec<_> = b.sheets.iter().map(|s| (&s.name, s.hidden)).collect();
    if an != bn {
        out.push(format!("sheets {an:?} vs {bn:?}"));
    }
    for (x, y) in a.sheets.iter().zip(&b.sheets) {
        for (p, c) in &x.cells {
            let d = y.get(*p).cloned().unwrap_or_default();
            if !close(&c.value, &d.value) {
                out.push(format!("{}!{} value {:?} vs {:?}", x.name, p.a1(), c.value, d.value));
            }
            if c.formula != d.formula {
                out.push(format!("{}!{} formula {:?} vs {:?}", x.name, p.a1(), c.formula, d.formula));
            }
            if c.fmt != d.fmt {
                out.push(format!("{}!{} format: {}", x.name, p.a1(), fmt_diff(&c.fmt, &d.fmt)));
            }
        }
        for p in y.cells.keys() {
            if x.get(*p).is_none() {
                out.push(format!("{}!{} only after writing", x.name, p.a1()));
            }
        }
        let mut xm = x.merges.clone();
        let mut ym = y.merges.clone();
        xm.sort();
        ym.sort();
        if xm != ym {
            out.push(format!("{} merges {} vs {}", x.name, xm.len(), ym.len()));
        }
        for (c, w) in &x.col_mm {
            let v = y.col_mm.get(c).copied().or(y.default_col_mm);
            if v.is_none_or(|v| (v - w).abs() > 0.01) {
                out.push(format!("{} column {c} width {w} vs {v:?}", x.name));
                break;
            }
        }
        for (r, h) in &x.row_height {
            let v = y.row_height.get(r).copied();
            if v.is_none_or(|v| (v - h).abs() > 0.05) {
                out.push(format!("{} row {r} height {h} vs {v:?}", x.name));
                break;
            }
        }
        if x.rich_runs != y.rich_runs {
            let k = x.rich_runs.keys().chain(y.rich_runs.keys()).find(|k| x.rich_runs.get(k) != y.rich_runs.get(k));
            out.push(format!(
                "{} rich text differs at {:?}: {:?} vs {:?}",
                x.name,
                k.map(|p| p.a1()),
                k.and_then(|k| x.rich_runs.get(k)),
                k.and_then(|k| y.rich_runs.get(k))
            ));
        }
        if x.row_hidden != y.row_hidden || x.col_hidden != y.col_hidden {
            out.push(format!("{} hidden rows or columns differ", x.name));
        }
    }
    out
}

/// The fields of two formats that differ, with both values
fn fmt_diff(a: &book::CellFormat, b: &book::CellFormat) -> String {
    let mut out = Vec::new();
    macro_rules! f {
        ($($x:ident),*) => {$(
            if a.$x != b.$x {
                out.push(format!("{} {:?} vs {:?}", stringify!($x), a.$x, b.$x));
            }
        )*};
    }
    f!(bold, italic, underline, strike, subscript, font, size_c, color, fill, borders, align, valign, wrap, shrink, rotation, indent, number_format);
    if out.is_empty() {
        out.push("other fields".into());
    }
    out.join("; ")
}
