//! Read an xlsx and the ods LibreOffice made from it, and list where the two
//! workbooks differ: sheets, cell values, formulas and merged cells.
//!
//!     cargo run -q -p sheet --example ods_vs_xlsx -- ~/xlsx-corpus ~/ods-corpus
//!
//! Every `<stem>.ods` in the second folder is paired with `<stem>.xlsx` in the
//! first. The ods comes from `tools/lo_pdf.py --to ods`, so a difference is
//! either this reader or LibreOffice's own conversion; the list says which
//! cells to look at, not who is right.

use std::fs::File;
use std::path::Path;

use book::{Book, Value};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        let diffs = compare(&x, &o);
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
