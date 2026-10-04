//! Print an ods (or an xlsx) to PDF the way officework does, to compare
//! with LibreOffice's or Excel's PDF.
//!
//!     cargo run -q -p ops --example ods_pdf -- book.ods out.pdf

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [src, out] = args.as_slice() else {
        eprintln!("usage: ods_pdf BOOK OUT.pdf");
        std::process::exit(2);
    };
    let f = std::fs::File::open(src).expect("open");
    let (mut b, _) = if src.ends_with(".ods") {
        sheet::ods::read(f).expect("read ods")
    } else {
        sheet::xlsx::read(f).expect("read xlsx")
    };
    book::calc::recalc_all(&mut b);
    // The result is how many columns were cut at the paper's edge
    let cut = ops::pdf::book(&b, std::path::Path::new(out)).expect("pdf");
    if cut > 0 {
        println!("{cut} columns cut at the paper's edge");
    }
}
