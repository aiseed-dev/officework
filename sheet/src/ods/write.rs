//! Write a workbook as an ods.
//!
//! First step (2026-10-04, SEKKEI "決め: ODF を先にする"): sheets, values,
//! formulas, merged cells, cell formatting with number formats, column
//! widths, row heights, hidden rows, columns and sheets, frozen panes are
//! not yet, and the page setup (paper, orientation, margins, scale, print
//! ranges, title rows). The layout of the files follows what LibreOffice
//! 24.2 writes (`xmloff/source/style`, `sc/source/filter/xml/xmlexprt.cxx`),
//! and every file this writes is checked by reading it back and by having
//! LibreOffice open it (`tools/lo_pdf.py`).
//!
//! What the model holds but this does not write yet is counted in the
//! [`WriteReport`], so saving can say what was left out.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use book::{BStyle, Book, Cell, CellFormat, Edge, HAlign, Pos, Sheet, VAlign, Value};

/// What was not written, by name and count (the same shape as the
/// reader's report)
#[derive(Debug, Default, Clone)]
pub struct WriteReport {
    pub left_out: Vec<(String, usize)>,
}

impl WriteReport {
    fn note(&mut self, what: &str, n: usize) {
        if n == 0 {
            return;
        }
        match self.left_out.iter_mut().find(|(x, _)| x == what) {
            Some(e) => e.1 += n,
            None => self.left_out.push((what.to_string(), n)),
        }
    }
    pub fn is_lossless(&self) -> bool {
        self.left_out.is_empty()
    }
}

const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" xmlns:of="urn:oasis:names:tc:opendocument:xmlns:of:1.2" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:css3t="http://www.w3.org/TR/css3-text/" xmlns:calcext="urn:org:documentfoundation:names:experimental:calc:xmlns:calcext:1.0" xmlns:loext="urn:org:documentfoundation:names:experimental:office:xmlns:loext:1.0" office:version="1.3""#;

/// Columns and rows past the last used one are written as one repeated
/// element up to the sheet's size, as LibreOffice does
const MAX_COLS: u32 = 16384;
const MAX_ROWS: u32 = 1_048_576;

pub fn write(book: &Book) -> (Vec<u8>, WriteReport) {
    let mut rep = WriteReport::default();
    let mut st = Styles::default();
    let mut body = String::new();
    for sh in &book.sheets {
        sheet_body(sh, book, &mut st, &mut body, &mut rep);
        left_out(sh, &mut rep);
    }
    // Names for the whole workbook come after the sheets
    let global: Vec<String> = book
        .sheets
        .iter()
        .flat_map(|sh| sh.names.iter().filter(|n| !n.scoped).map(move |n| named_range_xml(&sh.name, n)))
        .collect();
    if !global.is_empty() {
        let _ = write!(body, "<table:named-expressions>{}</table:named-expressions>", global.concat());
    }
    let content = content_xml(book, &st, &body);
    let styles = styles_xml(book, &st);
    let bytes = zip_parts(&content, &styles).unwrap_or_default();
    (bytes, rep)
}

/// Parts of a sheet this step does not write
fn left_out(sh: &Sheet, rep: &mut WriteReport) {
    rep.note("comments", sh.comments.len());
    rep.note("hyperlinks", sh.links.len());
    rep.note("conditional formats", sh.cond.len());
    rep.note("data validation", sh.validations.len());
    rep.note("shapes and pictures", sh.shapes.len() + sh.shapes_new.len());
    rep.note("tables", sh.tables.len());
    rep.note("scenarios", sh.scenarios.len());
    rep.note("frozen panes", usize::from(sh.freeze.is_some()));
    rep.note(
        "different headers on first or even pages",
        [&sh.header_even, &sh.footer_even, &sh.header_first, &sh.footer_first].iter().filter(|h| h.is_some()).count(),
    );
    rep.note("page breaks", sh.row_breaks.len() + sh.col_breaks.len());
    rep.note("sheet protection", usize::from(sh.protected));
}

/// The automatic styles collected while the sheets are written
#[derive(Default)]
struct Styles {
    cols: Vec<String>,
    rows: Vec<(String, bool)>,
    cells: Vec<CellFormat>,
    cell_index: BTreeMap<CellFormat, usize>,
    codes: Vec<String>,
    /// page setups; each sheet's table style names one master page
    pages: Vec<String>,
    tables: Vec<(bool, usize)>,
    fonts: Vec<String>,
}

impl Styles {
    fn col(&mut self, mm: f32) -> String {
        let w = length(mm);
        let i = self.cols.iter().position(|x| *x == w).unwrap_or_else(|| {
            self.cols.push(w);
            self.cols.len() - 1
        });
        format!("co{}", i + 1)
    }
    fn row(&mut self, pt: f32, auto: bool) -> String {
        let h = length(pt * 25.4 / 72.0);
        let key = (h, auto);
        let i = self.rows.iter().position(|x| *x == key).unwrap_or_else(|| {
            self.rows.push(key);
            self.rows.len() - 1
        });
        format!("ro{}", i + 1)
    }
    fn cell(&mut self, f: &CellFormat) -> Option<String> {
        if f.is_plain() {
            return None;
        }
        let i = match self.cell_index.get(f) {
            Some(i) => *i,
            None => {
                self.cells.push(f.clone());
                self.cell_index.insert(f.clone(), self.cells.len() - 1);
                if let Some(font) = &f.font {
                    if !self.fonts.contains(font) {
                        self.fonts.push(font.clone());
                    }
                }
                if let Some(code) = &f.number_format {
                    if !self.codes.contains(code) {
                        self.codes.push(code.clone());
                    }
                }
                self.cells.len() - 1
            }
        };
        Some(format!("ce{}", i + 1))
    }
    fn table(&mut self, hidden: bool, page: String) -> String {
        let p = self.pages.iter().position(|x| *x == page).unwrap_or_else(|| {
            self.pages.push(page);
            self.pages.len() - 1
        });
        let key = (hidden, p);
        let i = self.tables.iter().position(|x| *x == key).unwrap_or_else(|| {
            self.tables.push(key);
            self.tables.len() - 1
        });
        format!("ta{}", i + 1)
    }
}

/// Millimetres as an ODF length in centimetres, as LibreOffice writes them
fn length(mm: f32) -> String {
    let cm = format!("{:.4}", mm / 10.0);
    let cm = cm.trim_end_matches('0').trim_end_matches('.');
    format!("{cm}cm")
}

pub(super) fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            '\t' | '\n' | '\r' => o.push(c),
            c if (c as u32) < 0x20 => {}
            c => o.push(c),
        }
    }
    o
}

fn sheet_body(sh: &Sheet, book: &Book, st: &mut Styles, out: &mut String, rep: &mut WriteReport) {
    let page = page_layout(sh, book);
    let ta = st.table(sh.hidden, page);
    let _ = write!(out, r#"<table:table table:name="{}" table:style-name="{ta}""#, esc(&sh.name));
    if !sh.print_areas.is_empty() {
        let ranges: Vec<String> = sh
            .print_areas
            .iter()
            .map(|(a, b)| format!("{}.{}:{}.{}", quote_sheet(&sh.name), a.a1(), quote_sheet(&sh.name), b.a1()))
            .collect();
        let _ = write!(out, r#" table:print-ranges="{}""#, esc(&ranges.join(" ")));
    }
    out.push('>');

    // How far the sheet is used
    let mut last_row = 0u32;
    let mut last_col = 0u32;
    for p in sh.cells.keys() {
        last_row = last_row.max(p.row + 1);
        last_col = last_col.max(p.col + 1);
    }
    for (_, b) in &sh.merges {
        last_row = last_row.max(b.row + 1);
        last_col = last_col.max(b.col + 1);
    }
    for c in sh.col_mm.keys().chain(sh.col_hidden.iter()) {
        last_col = last_col.max(c + 1);
    }
    for r in sh.row_height.keys().chain(sh.row_hidden.iter()) {
        last_row = last_row.max(r + 1);
    }
    let last_col = last_col.min(MAX_COLS);
    let last_row = last_row.min(MAX_ROWS);

    // Columns: runs of the same width and visibility become one element
    let default_mm = sh.col_haba_mm(u32::MAX, &book.col_basis);
    let col_key = |c: u32| (sh.col_haba_mm(c, &book.col_basis), sh.col_hidden.contains(&c));
    let mut c = 0;
    while c < last_col {
        let k = col_key(c);
        let mut n = 1;
        while c + n < last_col && col_key(c + n) == k {
            n += 1;
        }
        column(out, st, k.0, k.1, n);
        c += n;
    }
    if last_col < MAX_COLS {
        column(out, st, default_mm, false, MAX_COLS - last_col);
    }

    // Title rows repeat at the top of each printed page
    let titles = sh.print_title_rows;
    let covered = covered_cells(sh);
    let starts: HashMap<Pos, (u32, u32)> = sh
        .merges
        .iter()
        .map(|(a, b)| (*a, (b.row - a.row + 1, b.col - a.col + 1)))
        .collect();
    let mut r = 0;
    while r < last_row {
        if titles.is_some_and(|(a, _)| a == r) {
            out.push_str("<table:table-header-rows>");
        }
        let row_xml = row_cells(sh, r, last_col, &covered, &starts, st);
        // Empty rows in a run are written once with a repeat count
        let mut n = 1;
        let in_titles = |x: u32| titles.is_some_and(|(a, b)| x >= a && x <= b);
        if row_xml.is_none() {
            while r + n < last_row
                && row_attrs(sh, r + n, st) == row_attrs(sh, r, st)
                && in_titles(r + n) == in_titles(r)
                && row_cells(sh, r + n, last_col, &covered, &starts, st).is_none()
                && !titles.is_some_and(|(a, _)| a == r + n)
            {
                n += 1;
            }
        }
        let attrs = row_attrs(sh, r, st);
        let rep_attr = if n > 1 { format!(r#" table:number-rows-repeated="{n}""#) } else { String::new() };
        let _ = write!(out, "<table:table-row{attrs}{rep_attr}>");
        match row_xml {
            Some(x) => out.push_str(&x),
            None => {
                let _ = write!(out, r#"<table:table-cell table:number-columns-repeated="{MAX_COLS}"/>"#);
            }
        }
        out.push_str("</table:table-row>");
        if titles.is_some_and(|(_, b)| b == r + n - 1) {
            out.push_str("</table:table-header-rows>");
        }
        r += n;
    }
    if last_row < MAX_ROWS {
        let _ = write!(
            out,
            r#"<table:table-row table:number-rows-repeated="{}"><table:table-cell table:number-columns-repeated="{MAX_COLS}"/></table:table-row>"#,
            MAX_ROWS - last_row
        );
    }
    // Names that only this sheet uses
    let local: Vec<String> = sh.names.iter().filter(|n| n.scoped).map(|n| named_range_xml(&sh.name, n)).collect();
    if !local.is_empty() {
        let _ = write!(out, "<table:named-expressions>{}</table:named-expressions>", local.concat());
    }
    out.push_str("</table:table>");
    if titles.is_some_and(|(a, b)| a >= last_row || b >= last_row) {
        rep.note("title rows past the used range", 1);
    }
}

/// A name for a cell or a block of cells on `sheet`, as LibreOffice writes it
fn named_range_xml(sheet: &str, n: &book::DefinedName) -> String {
    let s = format!("${}", quote_sheet(sheet));
    let abs = |a: &str| -> String {
        let a = a.replace('$', "");
        let split = a.find(|c: char| c.is_ascii_digit()).unwrap_or(a.len());
        let (c, r) = a.split_at(split);
        format!("${c}${r}")
    };
    let (first, range) = match n.range.split_once(':') {
        Some((a, b)) => (abs(a), format!("{s}.{}:.{}", abs(a), abs(b))),
        None => (abs(&n.range), format!("{s}.{}", abs(&n.range))),
    };
    format!(
        r#"<table:named-range table:name="{}" table:base-cell-address="{s}.{first}" table:cell-range-address="{}"/>"#,
        esc(&n.name),
        esc(&range)
    )
}

fn column(out: &mut String, st: &mut Styles, mm: f32, hidden: bool, n: u32) {
    let co = st.col(mm);
    let _ = write!(out, r#"<table:table-column table:style-name="{co}""#);
    if n > 1 {
        let _ = write!(out, r#" table:number-columns-repeated="{n}""#);
    }
    if hidden {
        out.push_str(r#" table:visibility="collapse""#);
    }
    out.push_str(r#" table:default-cell-style-name="Default"/>"#);
}

fn row_attrs(sh: &Sheet, r: u32, st: &mut Styles) -> String {
    let mut a = String::new();
    let h = sh.row_height.get(&r).copied().or(sh.default_row_height).unwrap_or(book::DEFAULT_ROW_PT);
    let auto = sh.row_height_auto.contains_key(&r) || !sh.row_height.contains_key(&r);
    let _ = write!(a, r#" table:style-name="{}""#, st.row(h, auto));
    if sh.row_hidden.contains(&r) {
        a.push_str(r#" table:visibility="collapse""#);
    } else if sh.filter_hidden.contains(&r) {
        a.push_str(r#" table:visibility="filter""#);
    }
    a
}

fn covered_cells(sh: &Sheet) -> std::collections::HashSet<Pos> {
    let mut s = std::collections::HashSet::new();
    for (a, b) in &sh.merges {
        for r in a.row..=b.row {
            for c in a.col..=b.col {
                if (r, c) != (a.row, a.col) {
                    s.insert(Pos::new(r, c));
                }
            }
        }
    }
    s
}

/// The cells of one row, or None when the row has nothing to write
fn row_cells(
    sh: &Sheet,
    r: u32,
    last_col: u32,
    covered: &std::collections::HashSet<Pos>,
    starts: &HashMap<Pos, (u32, u32)>,
    st: &mut Styles,
) -> Option<String> {
    let used: BTreeMap<u32, &Cell> = sh.cells.range(Pos::new(r, 0)..Pos::new(r + 1, 0)).map(|(p, c)| (p.col, c)).collect();
    let any_cover = covered.iter().any(|p| p.row == r);
    if used.is_empty() && !any_cover {
        return None;
    }
    let mut out = String::new();
    let mut c = 0;
    let mut blank = 0u32;
    let flush = |out: &mut String, blank: &mut u32| {
        if *blank > 0 {
            if *blank > 1 {
                let _ = write!(out, r#"<table:table-cell table:number-columns-repeated="{blank}"/>"#);
            } else {
                out.push_str("<table:table-cell/>");
            }
            *blank = 0;
        }
    };
    while c < last_col {
        let p = Pos::new(r, c);
        if covered.contains(&p) {
            flush(&mut out, &mut blank);
            let style = used.get(&c).and_then(|cl| st.cell(&cl.fmt));
            match style {
                Some(s) => {
                    let _ = write!(out, r#"<table:covered-table-cell table:style-name="{s}"/>"#);
                }
                None => out.push_str("<table:covered-table-cell/>"),
            }
            c += 1;
            continue;
        }
        match used.get(&c) {
            Some(cl) => {
                flush(&mut out, &mut blank);
                cell_xml(&mut out, cl, starts.get(&p).copied(), st);
            }
            None => blank += 1,
        }
        c += 1;
    }
    let rest = MAX_COLS - last_col + blank;
    if rest > 0 {
        let _ = write!(out, r#"<table:table-cell table:number-columns-repeated="{rest}"/>"#);
    }
    Some(out)
}

fn cell_xml(out: &mut String, cl: &Cell, span: Option<(u32, u32)>, st: &mut Styles) {
    out.push_str("<table:table-cell");
    if let Some(s) = st.cell(&cl.fmt) {
        let _ = write!(out, r#" table:style-name="{s}""#);
    }
    if let Some(f) = &cl.formula {
        let _ = write!(out, r#" table:formula="{}""#, esc(&super::formula::to_of(f)));
    }
    let text = match &cl.value {
        Value::Empty => None,
        Value::Number(n) => {
            let _ = write!(out, r#" office:value-type="float" office:value="{n}" calcext:value-type="float""#);
            Some(cl.value.display())
        }
        Value::Bool(b) => {
            let _ = write!(out, r#" office:value-type="boolean" office:boolean-value="{b}" calcext:value-type="boolean""#);
            Some(if *b { "TRUE".to_string() } else { "FALSE".to_string() })
        }
        Value::Text(t) => {
            out.push_str(r#" office:value-type="string" calcext:value-type="string""#);
            Some(t.clone())
        }
        Value::Error(e) => {
            if cl.formula.is_some() {
                out.push_str(r#" office:value-type="string" office:string-value="" calcext:value-type="error""#);
            } else {
                out.push_str(r#" office:value-type="string" calcext:value-type="string""#);
            }
            Some(e.clone())
        }
        // A moment in a time zone: written as its serial number
        other => {
            let n = other.as_number();
            let _ = write!(out, r#" office:value-type="float" office:value="{n}" calcext:value-type="float""#);
            Some(other.display())
        }
    };
    if let Some((rows, cols)) = span {
        let _ = write!(out, r#" table:number-columns-spanned="{cols}" table:number-rows-spanned="{rows}""#);
    }
    match text {
        Some(t) if !t.is_empty() => {
            out.push('>');
            for line in t.split('\n') {
                out.push_str("<text:p>");
                out.push_str(&para(line));
                out.push_str("</text:p>");
            }
            out.push_str("</table:table-cell>");
        }
        _ => out.push_str("/>"),
    }
}

/// One line of text: runs of spaces after the first, and leading spaces,
/// become `<text:s/>`, since XML would fold them; tabs become `<text:tab/>`
fn para(line: &str) -> String {
    let mut o = String::new();
    let mut spaces = 0usize;
    let mut at_start = true;
    let flush = |o: &mut String, spaces: &mut usize, at_start: bool| {
        if *spaces == 0 {
            return;
        }
        let (plain, kept) = if at_start { (0, *spaces) } else { (1, *spaces - 1) };
        if plain == 1 {
            o.push(' ');
        }
        if kept == 1 {
            o.push_str("<text:s/>");
        } else if kept > 1 {
            let _ = write!(o, r#"<text:s text:c="{kept}"/>"#);
        }
        *spaces = 0;
    };
    for ch in line.chars() {
        match ch {
            ' ' => spaces += 1,
            '\t' => {
                flush(&mut o, &mut spaces, at_start);
                o.push_str("<text:tab/>");
                at_start = false;
            }
            c => {
                flush(&mut o, &mut spaces, at_start);
                o.push_str(&esc(&c.to_string()));
                at_start = false;
            }
        }
    }
    // Trailing spaces would also be folded away
    if spaces > 0 {
        let kept = spaces;
        if kept == 1 {
            o.push_str("<text:s/>");
        } else {
            let _ = write!(o, r#"<text:s text:c="{kept}"/>"#);
        }
    }
    o
}

fn quote_sheet(name: &str) -> String {
    if name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        name.to_string()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

/// The page layout properties of a sheet, as the inside of a
/// `style:page-layout-properties` element
fn page_layout(sh: &Sheet, book: &Book) -> String {
    let (mut w, mut h) = sh.paper_size.and_then(paper_mm).unwrap_or((210.0, 297.0));
    if sh.landscape {
        std::mem::swap(&mut w, &mut h);
    }
    // Excel's defaults when the file names none: 0.7in left and right,
    // 0.75in top and bottom (ECMA-376 18.3.1.62 lists the attributes; these
    // are the values Excel writes for a new sheet)
    let (l, r, t, b) = sh.margins_mm.unwrap_or((17.78, 17.78, 19.05, 19.05));
    // A header sits inside the top margin in xlsx but above it in ODF, so
    // the margin is split into the page edge and the header's height
    let (he, fe) = sh.hf_margins_mm.unwrap_or((7.62, 7.62));
    let default_pt = book.default_font.as_ref().map(|(_, pt)| *pt).filter(|pt| *pt > 0.0).unwrap_or(book::DEFAULT_CELL_PT);
    let (t, header) = split_margin(t, he, sh.header.as_deref(), default_pt);
    let (b, footer) = split_margin(b, fe, sh.footer.as_deref(), default_pt);
    let mut a = format!(
        r#"fo:page-width="{}" fo:page-height="{}" style:print-orientation="{}" fo:margin-top="{}" fo:margin-bottom="{}" fo:margin-left="{}" fo:margin-right="{}" style:print-page-order="ttb" style:writing-mode="lr-tb""#,
        length(w),
        length(h),
        if sh.landscape { "landscape" } else { "portrait" },
        length(t),
        length(b),
        length(l),
        length(r),
    );
    match (sh.fit_to_w, sh.fit_to_h) {
        (None, None) => {
            let _ = write!(a, r#" style:scale-to="{}%""#, sh.print_scale.unwrap_or(100));
        }
        (fw, fh) => {
            let _ = write!(a, r#" style:scale-to-X="{}" style:scale-to-Y="{}""#, fw.unwrap_or(0), fh.unwrap_or(0));
        }
    }
    let mut print = vec!["charts", "drawings", "objects", "zero-values"];
    if sh.print_gridlines {
        print.push("grid");
    }
    if sh.print_headings {
        print.push("headers");
    }
    let _ = write!(a, r#" style:print="{}""#, print.join(" "));
    // The header and footer go with the layout key, after separators that
    // `styles_xml` splits again
    let hf = |h: &Option<String>, size: Option<HfSize>| match (h, size) {
        // The height LibreOffice writes includes the spacing
        (Some(code), Some(HfSize::Grows { min, gap })) => format!("min|{}|{}|{}", length(min + gap), length(gap), code),
        (Some(code), Some(HfSize::Fixed(height))) => format!("fix|{}|0cm|{}", length(height), code),
        _ => String::new(),
    };
    if sh.h_centered || sh.v_centered {
        let v = match (sh.h_centered, sh.v_centered) {
            (true, true) => "both",
            (true, false) => "horizontal",
            _ => "vertical",
        };
        let _ = write!(a, r#" style:table-centering="{v}""#);
    }
    let _ = write!(a, "\u{1}{}\u{1}{}", hf(&sh.header, header), hf(&sh.footer, footer));
    a
}

/// How tall a header or footer is in ODF
#[derive(Debug, Clone, Copy, PartialEq)]
enum HfSize {
    /// At least `min` high, `gap` away from the cells
    Grows { min: f32, gap: f32 },
    /// Exactly this high: the text does not fit between the page edge and
    /// the cells, so it is cut
    Fixed(f32),
}

/// A top or bottom margin with a header or footer in it, the way
/// LibreOffice's xlsx import splits it (`PageSettingsConverter::
/// convertHeaderFooterData` in sc/source/filter/oox/pagesettings.cxx): the
/// page margin becomes the distance to the header (`edge`, Excel's 0.3in
/// unless the sheet says), the header is as tall as its text, and the rest
/// is the spacing to the cells. If the text is taller than the room, the
/// header gets the room as a fixed height
fn split_margin(m: f32, edge: f32, code: Option<&str>, default_pt: f32) -> (f32, Option<HfSize>) {
    let Some(code) = code else { return (m, None) };
    let edge = edge.min(m);
    let total = m - edge;
    let text = hf_text_height_pt(code, default_pt) * 25.4 / 72.0;
    let gap = total - text;
    if gap >= 0.0 {
        (edge, Some(HfSize::Grows { min: text, gap }))
    } else {
        (edge, Some(HfSize::Fixed(total)))
    }
}

/// The height of a header's text in points, as LibreOffice's header parser
/// counts it: each line is as tall as the largest font size on it (the
/// workbook's default size until an `&NN` code changes it), the lines of a
/// region add up, and the tallest region counts. Every region has at least
/// one line
fn hf_text_height_pt(code: &str, default_pt: f32) -> f32 {
    let mut totals = [0f32; 3];
    let mut line_max = [0f32; 3];
    let mut size = default_pt;
    let mut region = 1usize;
    let ch: Vec<char> = code.chars().collect();
    let mut i = 0;
    let line_height = |m: f32, size: f32| if m == 0.0 { size } else { m };
    while i < ch.len() {
        match ch[i] {
            '\n' => {
                totals[region] += line_height(line_max[region], size);
                line_max[region] = 0.0;
                i += 1;
            }
            '&' => {
                let n = ch.get(i + 1).copied().unwrap_or(' ');
                i += 2;
                match n {
                    'L' => region = 0,
                    'C' => region = 1,
                    'R' => region = 2,
                    '"' => {
                        while i < ch.len() && ch[i] != '"' {
                            i += 1;
                        }
                        i += 1;
                    }
                    d if d.is_ascii_digit() => {
                        let mut v = d.to_digit(10).unwrap_or(0);
                        while i < ch.len() && ch[i].is_ascii_digit() {
                            v = v * 10 + ch[i].to_digit(10).unwrap_or(0);
                            i += 1;
                        }
                        if v > 0 {
                            size = v as f32;
                            line_max[region] = line_max[region].max(size);
                        }
                    }
                    _ => line_max[region] = line_max[region].max(size),
                }
            }
            _ => {
                line_max[region] = line_max[region].max(size);
                i += 1;
            }
        }
    }
    for r in 0..3 {
        totals[r] += line_height(line_max[r], size);
    }
    totals.into_iter().fold(0.0, f32::max)
}

/// xlsx header text (`&L…&C…&R…`, `&P` and the other codes) as the
/// regions of an ODF header or footer
fn hf_xml(code: &str) -> String {
    let mut regions: Vec<(char, String)> = Vec::new();
    let mut cur = 'C';
    let mut text = String::new();
    let mut paras: Vec<String> = Vec::new();
    let flush = |regions: &mut Vec<(char, String)>, cur: char, paras: &mut Vec<String>, text: &mut String| {
        paras.push(std::mem::take(text));
        let body: String = paras.drain(..).map(|p| format!("<text:p>{p}</text:p>")).collect();
        if body != "<text:p></text:p>" {
            regions.push((cur, body));
        }
    };
    let ch: Vec<char> = code.chars().collect();
    let mut i = 0;
    while i < ch.len() {
        let c = ch[i];
        if c == '\n' {
            paras.push(std::mem::take(&mut text));
            i += 1;
            continue;
        }
        if c != '&' {
            text.push_str(&esc(&c.to_string()));
            i += 1;
            continue;
        }
        let Some(&n) = ch.get(i + 1) else { break };
        i += 2;
        match n {
            'L' | 'C' | 'R' => {
                if !text.is_empty() || !paras.is_empty() {
                    flush(&mut regions, cur, &mut paras, &mut text);
                }
                cur = n;
            }
            'A' => text.push_str("<text:sheet-name>???</text:sheet-name>"),
            'P' => text.push_str("<text:page-number>1</text:page-number>"),
            'N' => text.push_str("<text:page-count>99</text:page-count>"),
            'D' => text.push_str("<text:date/>"),
            'T' => text.push_str("<text:time/>"),
            'F' => text.push_str("<text:title>???</text:title>"),
            '&' => text.push_str("&amp;"),
            // A font name in quotes, or a size in digits: formatting, skipped
            '"' => {
                while i < ch.len() && ch[i] != '"' {
                    i += 1;
                }
                i += 1;
            }
            d if d.is_ascii_digit() => {
                while i < ch.len() && ch[i].is_ascii_digit() {
                    i += 1;
                }
            }
            _ => {}
        }
    }
    if !text.is_empty() || !paras.is_empty() {
        flush(&mut regions, cur, &mut paras, &mut text);
    }
    let mut out = String::new();
    for k in ['L', 'C', 'R'] {
        if let Some((_, body)) = regions.iter().find(|(c, _)| *c == k) {
            let tag = match k {
                'L' => "region-left",
                'C' => "region-center",
                _ => "region-right",
            };
            let _ = write!(out, "<style:{tag}>{body}</style:{tag}>");
        }
    }
    out
}

/// xlsx paper codes in millimetres (portrait). B sizes are JIS, as in calc
pub(super) fn paper_mm(code: u32) -> Option<(f32, f32)> {
    Some(match code {
        1 => (215.9, 279.4),
        5 => (215.9, 355.6),
        8 => (297.0, 420.0),
        9 => (210.0, 297.0),
        11 => (148.0, 210.0),
        12 => (257.0, 364.0),
        13 => (182.0, 257.0),
        _ => return None,
    })
}

fn content_xml(book: &Book, st: &Styles, body: &str) -> String {
    let mut s = String::new();
    let _ = write!(s, r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NS}>"#);
    s.push_str(&font_decls(book, st));
    s.push_str("<office:automatic-styles>");
    for (i, w) in st.cols.iter().enumerate() {
        let _ = write!(
            s,
            r#"<style:style style:name="co{}" style:family="table-column"><style:table-column-properties fo:break-before="auto" style:column-width="{w}"/></style:style>"#,
            i + 1
        );
    }
    for (i, (h, auto)) in st.rows.iter().enumerate() {
        let _ = write!(
            s,
            r#"<style:style style:name="ro{}" style:family="table-row"><style:table-row-properties style:row-height="{h}" fo:break-before="auto" style:use-optimal-row-height="{auto}"/></style:style>"#,
            i + 1
        );
    }
    for (i, (hidden, page)) in st.tables.iter().enumerate() {
        let _ = write!(
            s,
            r#"<style:style style:name="ta{}" style:family="table" style:master-page-name="PageStyle_{}"><style:table-properties table:display="{}" style:writing-mode="lr-tb"/></style:style>"#,
            i + 1,
            page + 1,
            !hidden
        );
    }
    for (i, code) in st.codes.iter().enumerate() {
        if let Some(x) = super::numfmt_write::data_style(&format!("N{}", i + 100), code) {
            s.push_str(&x);
        }
    }
    for (i, f) in st.cells.iter().enumerate() {
        let data = f
            .number_format
            .as_ref()
            .and_then(|c| st.codes.iter().position(|x| x == c))
            .filter(|k| super::numfmt_write::data_style("N", &st.codes[*k]).is_some())
            .map(|k| format!(r#" style:data-style-name="N{}""#, k + 100))
            .unwrap_or_default();
        let _ = write!(
            s,
            r#"<style:style style:name="ce{}" style:family="table-cell" style:parent-style-name="Default"{data}>{}</style:style>"#,
            i + 1,
            cell_style(f)
        );
    }
    s.push_str("</office:automatic-styles><office:body><office:spreadsheet>");
    if book.date1904 {
        s.push_str(r#"<table:calculation-settings><table:null-date table:date-value="1904-01-01"/></table:calculation-settings>"#);
    }
    s.push_str(body);
    s.push_str("</office:spreadsheet></office:body></office:document-content>");
    s
}

fn font_decls(book: &Book, st: &Styles) -> String {
    let mut names: Vec<&String> = st.fonts.iter().collect();
    if let Some((f, _)) = &book.default_font {
        if !names.contains(&f) {
            names.push(f);
        }
    }
    let mut s = String::from("<office:font-face-decls>");
    for n in names {
        let _ = write!(s, r#"<style:font-face style:name="{0}" svg:font-family="&apos;{0}&apos;"/>"#, esc(n));
    }
    s.push_str("</office:font-face-decls>");
    s
}

/// The font attributes of a text style. As LibreOffice's xlsx import does
/// (`sc/source/filter/oox/stylesbuffer.cxx`, `Font::finalizeImport`), the
/// font becomes the Asian font too only when it has CJK glyphs; otherwise
/// Japanese text keeps the default Asian font
fn font_names(font: &str) -> String {
    let n = esc(font);
    let mut a = format!(r#" style:font-name="{n}""#);
    if has_cjk(font) {
        let _ = write!(a, r#" style:font-name-asian="{n}""#);
    }
    a
}

/// Whether a font has CJK glyphs: from the installed font when there is
/// one, otherwise from its name (a font named in CJK characters is taken
/// to have them, as the system's substitute for it would)
fn has_cjk(font: &str) -> bool {
    match kumihan::font::resolve(font) {
        Some(f) => f.japanese || f.han || f.hangul,
        None => !font.is_ascii(),
    }
}

fn border_value(e: &Edge) -> String {
    if !e.on {
        return "none".into();
    }
    let (w, line) = match e.style {
        BStyle::Hair => ("0.06pt", "solid"),
        BStyle::Thin => ("0.74pt", "solid"),
        BStyle::Medium => ("1.76pt", "solid"),
        BStyle::Thick => ("2.49pt", "solid"),
        BStyle::Double => ("1.76pt", "double-thin"),
        BStyle::Dotted => ("0.74pt", "dotted"),
        BStyle::Dashed => ("0.74pt", "dashed"),
        BStyle::DashDot => ("0.74pt", "dash-dot"),
        BStyle::DashDotDot => ("0.74pt", "dash-dot-dot"),
        BStyle::MediumDashed => ("1.76pt", "dashed"),
        BStyle::MediumDashDot | BStyle::SlantDashDot => ("1.76pt", "dash-dot"),
        BStyle::MediumDashDotDot => ("1.76pt", "dash-dot-dot"),
    };
    let col = e.color.unwrap_or(0);
    format!("{w} {line} #{col:06x}")
}

fn cell_style(f: &CellFormat) -> String {
    let mut cp = String::new();
    if let Some(fill) = &f.fill {
        let _ = write!(cp, r##" fo:background-color="#{}""##, fill.to_ascii_lowercase());
    }
    let b = &f.borders;
    if b.top == b.bottom && b.top == b.left && b.top == b.right {
        if b.top.on {
            let _ = write!(cp, r#" fo:border="{}""#, border_value(&b.top));
        }
    } else {
        for (k, e) in [("top", &b.top), ("bottom", &b.bottom), ("left", &b.left), ("right", &b.right)] {
            let _ = write!(cp, r#" fo:border-{k}="{}""#, border_value(e));
        }
    }
    if b.diag_down {
        let _ = write!(cp, r#" style:diagonal-tl-br="{}""#, border_value(&b.diag));
    }
    if b.diag_up {
        let _ = write!(cp, r#" style:diagonal-bl-tr="{}""#, border_value(&b.diag));
    }
    match f.valign {
        VAlign::Top => cp.push_str(r#" style:vertical-align="top""#),
        VAlign::Middle => cp.push_str(r#" style:vertical-align="middle""#),
        VAlign::Distribute => cp.push_str(r#" style:vertical-align="middle" loext:vertical-justify="distribute""#),
        _ => cp.push_str(r#" style:vertical-align="bottom""#),
    }
    if f.wrap {
        cp.push_str(r#" fo:wrap-option="wrap""#);
    }
    if f.shrink {
        cp.push_str(r#" style:shrink-to-fit="true""#);
    }
    // The model keeps xlsx's textRotation: 1-90 up, 91-180 down by v - 90,
    // 255 for characters stacked one below the other
    match f.rotation {
        Some(255) => cp.push_str(r#" style:direction="ttb""#),
        Some(r) if (1..=90).contains(&r) => {
            let _ = write!(cp, r#" style:rotation-angle="{r}" style:rotation-align="none""#);
        }
        Some(r) if (91..=180).contains(&r) => {
            let _ = write!(cp, r#" style:rotation-angle="{}" style:rotation-align="none""#, 360 - (r - 90));
        }
        _ => {}
    }
    if f.align != HAlign::General {
        cp.push_str(r#" style:text-align-source="fix""#);
    }
    let mut out = String::new();
    if !cp.is_empty() {
        let _ = write!(out, "<style:table-cell-properties{cp}/>");
    }
    let pp = match f.align {
        HAlign::General => "",
        HAlign::Left => r#" fo:text-align="start""#,
        HAlign::Center | HAlign::CenterContinuous => r#" fo:text-align="center""#,
        HAlign::Right => r#" fo:text-align="end""#,
        HAlign::Justify => r#" fo:text-align="justify""#,
        HAlign::Distribute => r#" fo:text-align="justify" css3t:text-justify="distribute""#,
    };
    if !pp.is_empty() {
        let _ = write!(out, "<style:paragraph-properties{pp}/>");
    }
    let mut tp = String::new();
    if let Some(font) = &f.font {
        tp.push_str(&font_names(font));
    }
    if let Some(sz) = f.size_c {
        let pt = sz as f32 / 100.0;
        let _ = write!(tp, r#" fo:font-size="{pt}pt" style:font-size-asian="{pt}pt" style:font-size-complex="{pt}pt""#);
    }
    if f.bold {
        tp.push_str(r#" fo:font-weight="bold" style:font-weight-asian="bold" style:font-weight-complex="bold""#);
    }
    if f.italic {
        tp.push_str(r#" fo:font-style="italic" style:font-style-asian="italic" style:font-style-complex="italic""#);
    }
    if f.underline {
        tp.push_str(r#" style:text-underline-style="solid" style:text-underline-width="auto" style:text-underline-color="font-color""#);
    }
    if f.strike {
        tp.push_str(r#" style:text-line-through-style="solid" style:text-line-through-type="single""#);
    }
    if f.subscript {
        tp.push_str(r#" style:text-position="sub 58%""#);
    }
    if let Some(c) = &f.color {
        let _ = write!(tp, r##" fo:color="#{}""##, c.to_ascii_lowercase());
    }
    if !tp.is_empty() {
        let _ = write!(out, "<style:text-properties{tp}/>");
    }
    out
}

fn styles_xml(book: &Book, st: &Styles) -> String {
    let mut s = String::new();
    let _ = write!(s, r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NS}>"#);
    s.push_str(&font_decls(book, st));
    s.push_str("<office:styles>");
    let (font, size) = book.default_font.clone().unwrap_or_else(|| (String::new(), book::DEFAULT_CELL_PT));
    let mut tp = String::new();
    if !font.is_empty() {
        tp.push_str(&font_names(&font));
    }
    let _ = write!(tp, r#" fo:font-size="{size}pt" style:font-size-asian="{size}pt" style:font-size-complex="{size}pt""#);
    let _ = write!(
        s,
        r#"<style:default-style style:family="table-cell"><style:text-properties{tp}/></style:default-style><style:style style:name="Default" style:family="table-cell"/>"#
    );
    s.push_str("</office:styles><office:automatic-styles>");
    let parts: Vec<Vec<&str>> = st.pages.iter().map(|p| p.split('\u{1}').collect()).collect();
    for (i, p) in parts.iter().enumerate() {
        let _ = write!(s, r#"<style:page-layout style:name="pm{}"><style:page-layout-properties {}/>"#, i + 1, p[0]);
        for (k, tag, gap) in [(1, "header-style", "margin-bottom"), (2, "footer-style", "margin-top")] {
            let f: Vec<&str> = p.get(k).copied().unwrap_or("").splitn(4, '|').collect();
            if f.len() == 4 {
                let height = if f[0] == "fix" { "svg:height" } else { "fo:min-height" };
                let _ = write!(
                    s,
                    r#"<style:{tag}><style:header-footer-properties {height}="{}" fo:margin-left="0cm" fo:margin-right="0cm" fo:{gap}="{}"/></style:{tag}>"#,
                    f[1], f[2]
                );
            }
        }
        s.push_str("</style:page-layout>");
    }
    s.push_str("</office:automatic-styles><office:master-styles>");
    for (i, p) in parts.iter().enumerate() {
        // A sheet without a header or footer gets none: LibreOffice's own
        // default page style would print the sheet name and the page number
        let hf = |k: usize, tag: &str| -> String {
            let f: Vec<&str> = p.get(k).copied().unwrap_or("").splitn(4, '|').collect();
            match f.get(3) {
                Some(code) => format!("<style:{tag}>{}</style:{tag}>", hf_xml(code)),
                None => format!(r#"<style:{tag} style:display="false"/>"#),
            }
        };
        let _ = write!(
            s,
            r#"<style:master-page style:name="PageStyle_{0}" style:page-layout-name="pm{0}">{1}{2}</style:master-page>"#,
            i + 1,
            hf(1, "header"),
            hf(2, "footer")
        );
    }
    s.push_str("</office:master-styles></office:document-styles>");
    s
}

fn zip_parts(content: &str, styles: &str) -> zip::result::ZipResult<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        // The mimetype comes first and unpacked, so the file type can be
        // read from a fixed place (ODF 1.3 Part 2, 3.3)
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let packed = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        z.start_file("mimetype", stored)?;
        z.write_all(b"application/vnd.oasis.opendocument.spreadsheet")?;
        z.start_file("content.xml", packed)?;
        z.write_all(content.as_bytes())?;
        z.start_file("styles.xml", packed)?;
        z.write_all(styles.as_bytes())?;
        z.start_file("meta.xml", packed)?;
        let _ = write!(
            z,
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-meta {NS}><office:meta><meta:generator>officework/{}</meta:generator></office:meta></office:document-meta>"#,
            env!("CARGO_PKG_VERSION")
        );
        z.start_file("META-INF/manifest.xml", packed)?;
        z.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3"><manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/></manifest:manifest>"#,
        )?;
        z.finish()?;
    }
    Ok(buf.into_inner())
}
