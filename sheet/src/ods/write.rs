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

const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" xmlns:of="urn:oasis:names:tc:opendocument:xmlns:of:1.2" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:css3t="http://www.w3.org/TR/css3-text/" xmlns:calcext="urn:org:documentfoundation:names:experimental:calc:xmlns:calcext:1.0" xmlns:loext="urn:org:documentfoundation:names:experimental:office:xmlns:loext:1.0" office:version="1.3""#;

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
    let settings = super::settings::write(book);
    let bytes = zip_parts(&content, &styles, settings.as_deref(), &st.pictures, &st.objects).unwrap_or_default();
    (bytes, rep)
}

/// Parts of a sheet this step does not write
fn left_out(sh: &Sheet, rep: &mut WriteReport) {
    // Stable ids: the app turns them into words on the screen
    // A link in ODF is text; a link on an empty cell has nothing to sit on
    rep.note("hyperlink", sh.links.keys().filter(|p| sh.value(**p).display().is_empty()).count());
    rep.note("table", sh.tables.len());
    rep.note("scenario", sh.scenarios.len());
    // ODF keeps six of the things a protected sheet allows (selecting
    // locked and unlocked cells, inserting and deleting rows and columns)
    let a = &sh.protect_allow;
    let more = [a.format_cells, a.format_cols, a.format_rows, a.insert_links, a.sort, a.autofilter, a.pivot, a.objects];
    rep.note("sheet_protection", usize::from(sh.protected && more.iter().any(|x| *x)));
}

/// The automatic styles collected while the sheets are written
#[derive(Default)]
struct Styles {
    /// Column styles: (width, starts a page)
    cols: Vec<(String, bool)>,
    /// Row styles: (height, optimal height, starts a page)
    rows: Vec<(String, bool, bool)>,
    cells: Vec<CellFormat>,
    cell_index: BTreeMap<CellFormat, usize>,
    codes: Vec<String>,
    /// page setups; each sheet's table style names one master page
    pages: Vec<String>,
    tables: Vec<(bool, bool, usize)>,
    fonts: Vec<String>,
    /// Looks of runs inside cells, as the text styles `T1`, `T2`, …
    texts: Vec<String>,
    /// Looks of shapes and pictures, as the graphic styles `gr1`, …
    graphics: Vec<String>,
    /// Looks of conditional formats, as the named cell styles
    /// `ConditionalStyle_1`, … in styles.xml
    conds: Vec<book::CondLook>,
    /// Data validation rules: (the rule as written, its name `val1`, …)
    vals: Vec<(String, String)>,
    /// Paragraph styles of shape text, `P1`, …
    paras: Vec<String>,
    /// Pictures in the package: (path, bytes, media type)
    pictures: Vec<(String, Vec<u8>, &'static str)>,
    /// Files of kept objects (charts): (path, bytes, media type); a path
    /// ending in `/` is a folder, listed in the manifest only
    objects: Vec<(String, Vec<u8>, String)>,
    /// Kept objects written so far, to number their folders
    object_count: usize,
}

impl Styles {
    fn col(&mut self, mm: f32, brk: bool) -> String {
        let w = (length(mm), brk);
        let i = self.cols.iter().position(|x| *x == w).unwrap_or_else(|| {
            self.cols.push(w);
            self.cols.len() - 1
        });
        format!("co{}", i + 1)
    }
    fn row(&mut self, pt: f32, auto: bool, brk: bool) -> String {
        let h = length(pt * 25.4 / 72.0);
        let key = (h, auto, brk);
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
    /// A kept object's XML with its folder renamed to the next free
    /// number, its files added to the package under the new name
    fn keep(&mut self, k: &book::KeptObject) -> String {
        self.object_count += 1;
        let new = format!("Object {}", self.object_count);
        let mut xml = k.xml.clone();
        // The folder is the first file's top folder (`Object 1/`)
        let old = k.files.iter().find_map(|(p, _, _)| p.split_once('/').map(|(a, _)| a.to_string())).filter(|a| a != "ObjectReplacements");
        for (path, data, media) in &k.files {
            let renamed = match &old {
                Some(o) if path.starts_with(&format!("{o}/")) => format!("{new}/{}", &path[o.len() + 1..]),
                Some(o) if path == &format!("ObjectReplacements/{o}") => format!("ObjectReplacements/{new}"),
                _ => path.clone(),
            };
            self.objects.push((renamed, data.clone(), media.clone()));
        }
        if let Some(o) = &old {
            for (from, to) in [
                (format!("\"./{o}\""), format!("\"./{new}\"")),
                (format!("\"{o}\""), format!("\"{new}\"")),
                (format!("\"./ObjectReplacements/{o}\""), format!("\"./ObjectReplacements/{new}\"")),
                (format!("\"ObjectReplacements/{o}\""), format!("\"ObjectReplacements/{new}\"")),
            ] {
                xml = xml.replace(&from, &to);
            }
        }
        xml
    }

    /// The name of a data validation rule, None for a kind ODF has no
    /// condition for. Rules with the same everything share one name
    fn validation(&mut self, sheet: &str, v: &book::Validation) -> Option<String> {
        let cond = super::valid::condition(v)?;
        let base = format!("{}.{}", quote_sheet(sheet), v.range.0.a1());
        let mut x = format!(
            r#" table:condition="{}" table:allow-empty-cell="{}""#,
            esc(&cond),
            v.allow_blank
        );
        if v.kind == "list" {
            let _ = write!(x, r#" table:display-list="{}""#, if v.hide_arrow { "none" } else { "unsorted" });
        }
        let _ = write!(x, r#" table:base-cell-address="{}">"#, esc(&base));
        if let Some((t, b)) = &v.input_msg {
            let _ = write!(x, r#"<table:help-message table:title="{}" table:display="true">{}</table:help-message>"#, esc(t), paras(b));
        }
        match &v.error_msg {
            Some((kind, t, b)) => {
                let _ = write!(
                    x,
                    r#"<table:error-message table:message-type="{}" table:title="{}" table:display="true">{}</table:error-message>"#,
                    esc(kind),
                    esc(t),
                    paras(b)
                );
            }
            // A rule without words still refuses what does not match
            None => x.push_str(r#"<table:error-message table:message-type="stop" table:display="true"/>"#),
        }
        if let Some((_, n)) = self.vals.iter().find(|(r, _)| *r == x) {
            return Some(n.clone());
        }
        let name = format!("val{}", self.vals.len() + 1);
        self.vals.push((x, name.clone()));
        Some(name)
    }

    fn cond_style(&mut self, look: &book::CondLook) -> String {
        let i = self.conds.iter().position(|x| x == look).unwrap_or_else(|| {
            self.conds.push(look.clone());
            self.conds.len() - 1
        });
        format!("ConditionalStyle_{}", i + 1)
    }

    fn paragraph(&mut self, inner: String) -> String {
        let i = self.paras.iter().position(|x| *x == inner).unwrap_or_else(|| {
            self.paras.push(inner);
            self.paras.len() - 1
        });
        format!("P{}", i + 1)
    }

    fn graphic(&mut self, props: String) -> String {
        let i = self.graphics.iter().position(|x| *x == props).unwrap_or_else(|| {
            self.graphics.push(props);
            self.graphics.len() - 1
        });
        format!("gr{}", i + 1)
    }

    /// The package path of a picture, the same bytes stored once
    fn picture(&mut self, data: &[u8]) -> String {
        if let Some((p, _, _)) = self.pictures.iter().find(|(_, d, _)| d == data) {
            return p.clone();
        }
        let (ext, mime) = super::drawing::picture_type(data);
        let path = format!("Pictures/image{}.{ext}", self.pictures.len() + 1);
        self.pictures.push((path.clone(), data.to_vec(), mime));
        path
    }

    /// The text style for a run's own look, or None for a plain run
    fn text(&mut self, r: &book::RichRun) -> Option<String> {
        let mut tp = String::new();
        if let Some(f) = &r.font {
            tp.push_str(&font_names(f));
            if !self.fonts.contains(f) {
                self.fonts.push(f.clone());
            }
        }
        if let Some(pt) = r.size_pt {
            let _ = write!(tp, r#" fo:font-size="{pt}pt" style:font-size-asian="{pt}pt" style:font-size-complex="{pt}pt""#);
        }
        if let Some(b) = r.bold {
            let w = if b { "bold" } else { "normal" };
            let _ = write!(tp, r#" fo:font-weight="{w}" style:font-weight-asian="{w}" style:font-weight-complex="{w}""#);
        }
        if let Some(i) = r.italic {
            let st = if i { "italic" } else { "normal" };
            let _ = write!(tp, r#" fo:font-style="{st}" style:font-style-asian="{st}" style:font-style-complex="{st}""#);
        }
        if let Some(c) = &r.color {
            let _ = write!(tp, r##" fo:color="#{}""##, c.to_ascii_lowercase());
        }
        // As LibreOffice writes Excel's vertAlign
        match r.vert {
            Some(book::RunPosition::Superscript) => tp.push_str(r#" style:text-position="super 58%""#),
            Some(book::RunPosition::Subscript) => tp.push_str(r#" style:text-position="sub 58%""#),
            Some(book::RunPosition::Baseline) => tp.push_str(r#" style:text-position="0% 100%""#),
            None => {}
        }
        if tp.is_empty() {
            return None;
        }
        let i = self.texts.iter().position(|x| *x == tp).unwrap_or_else(|| {
            self.texts.push(tp);
            self.texts.len() - 1
        });
        Some(format!("T{}", i + 1))
    }

    fn table(&mut self, hidden: bool, rtl: bool, page: String) -> String {
        let p = self.pages.iter().position(|x| *x == page).unwrap_or_else(|| {
            self.pages.push(page);
            self.pages.len() - 1
        });
        let key = (hidden, rtl, p);
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

/// Text as paragraphs, one per line
fn paras(t: &str) -> String {
    if t.is_empty() {
        return String::new();
    }
    t.split('\n').map(|l| format!("<text:p>{}</text:p>", para(l))).collect()
}

fn sheet_body(sh: &Sheet, book: &Book, st: &mut Styles, out: &mut String, rep: &mut WriteReport) {
    // Data validation: each range and the name of its rule; the cells in
    // the range carry the name
    let mut vals: Vec<((Pos, Pos), String)> = Vec::new();
    for v in &sh.validations {
        match st.validation(&sh.name, v) {
            Some(n) => vals.push((v.range, n)),
            None => rep.note("data_validation", 1),
        }
    }
    let page = page_layout(sh, book);
    let ta = st.table(sh.hidden, sh.rtl, page);
    let _ = write!(out, r#"<table:table table:name="{}" table:style-name="{ta}""#, esc(&sh.name));
    if !sh.print_areas.is_empty() {
        let ranges: Vec<String> = sh
            .print_areas
            .iter()
            .map(|(a, b)| format!("{}.{}:{}.{}", quote_sheet(&sh.name), a.a1(), quote_sheet(&sh.name), b.a1()))
            .collect();
        let _ = write!(out, r#" table:print-ranges="{}""#, esc(&ranges.join(" ")));
    }
    // A protected sheet without a password, and what it allows, in
    // LibreOffice's extension (sc/source/filter/xml/xmlexprt.cxx)
    if sh.protected {
        out.push_str(r#" table:protected="true""#);
    }
    out.push('>');
    if sh.protected {
        let a = &sh.protect_allow;
        out.push_str("<loext:table-protection");
        for (on, key) in [
            (a.select_locked, "select-protected-cells"),
            (a.select_unlocked, "select-unprotected-cells"),
            (a.insert_cols, "insert-columns"),
            (a.insert_rows, "insert-rows"),
            (a.delete_cols, "delete-columns"),
            (a.delete_rows, "delete-rows"),
        ] {
            if on {
                let _ = write!(out, r#" loext:{key}="true""#);
            }
        }
        out.push_str("/>");
    }

    // Comments, pictures and shapes go inside the cell they are anchored
    // to: (comment, drawing objects) by cell
    let mut extras: HashMap<Pos, (String, String)> = HashMap::new();
    for (p, t) in &sh.comments {
        extras.entry(*p).or_default().0 = super::drawing::comment_xml(t);
    }
    for i in sh.images.iter().chain(&sh.images_new) {
        let href = st.picture(&i.data);
        let style = st.graphic(r#"draw:stroke="none" draw:fill="none""#.to_string());
        extras.entry(i.at).or_default().1.push_str(&super::drawing::image_xml(i, &href, &style));
    }
    for shp in sh.shapes.iter().chain(&sh.shapes_new) {
        if super::drawing::shape_type(&shp.kind).is_none() {
            rep.note("shape", 1);
            continue;
        }
        if shp.rot != 0.0 {
            rep.note("shape_rotation", 1);
        }
        let style = st.graphic(super::drawing::shape_graphic(shp));
        let para = st.paragraph(super::drawing::shape_paragraph(shp));
        if let Some(f) = &shp.text_fmt.font {
            if !st.fonts.contains(f) {
                st.fonts.push(f.clone());
            }
        }
        if let Some(x) = super::drawing::shape_xml(shp, &sh.name, &style, &para) {
            extras.entry(shp.at).or_default().1.push_str(&x);
        }
    }

    // Kept objects (charts) get folders numbered across the workbook, so
    // two never share a name; the page-anchored ones go in table:shapes,
    // which comes before the columns
    let mut on_page = String::new();
    for k in &sh.kept_objects {
        let xml = st.keep(k);
        match k.at {
            Some(at) => extras.entry(at).or_default().1.push_str(&xml),
            None => on_page.push_str(&xml),
        }
    }
    if !on_page.is_empty() {
        let _ = write!(out, "<table:shapes>{on_page}</table:shapes>");
    }

    // How far the sheet is used
    let mut last_row = 0u32;
    let mut last_col = 0u32;
    for p in extras.keys() {
        last_row = last_row.max(p.row + 1);
        last_col = last_col.max(p.col + 1);
    }
    for ((_, b), _) in &vals {
        last_row = last_row.max(b.row + 1);
        last_col = last_col.max(b.col + 1);
    }
    for p in sh.cells.keys() {
        last_row = last_row.max(p.row + 1);
        last_col = last_col.max(p.col + 1);
    }
    for (_, b) in &sh.merges {
        last_row = last_row.max(b.row + 1);
        last_col = last_col.max(b.col + 1);
    }
    for c in sh.col_mm.keys().chain(sh.col_hidden.iter()).chain(sh.col_breaks.iter()) {
        last_col = last_col.max(c + 1);
    }
    for r in sh.row_height.keys().chain(sh.row_hidden.iter()).chain(sh.row_breaks.iter()) {
        last_row = last_row.max(r + 1);
    }
    if let Some((_, b)) = sh.print_title_cols {
        last_col = last_col.max(b + 1);
    }
    if let Some((_, b)) = sh.print_title_rows {
        last_row = last_row.max(b + 1);
    }
    let last_col = last_col.min(MAX_COLS);
    let last_row = last_row.min(MAX_ROWS);

    // Columns: runs of the same width and visibility become one element
    let default_mm = sh.col_haba_mm(u32::MAX, &book.col_basis);
    let col_key = |c: u32| (sh.col_haba_mm(c, &book.col_basis), sh.col_hidden.contains(&c), sh.col_breaks.contains(&c));
    // Title columns repeat at the left of each printed page; runs do not
    // cross their edges
    let tcols = sh.print_title_cols;
    let in_tcols = |x: u32| tcols.is_some_and(|(a, b)| x >= a && x <= b);
    let mut c = 0;
    while c < last_col {
        let k = col_key(c);
        let mut n = 1;
        while c + n < last_col && col_key(c + n) == k && in_tcols(c + n) == in_tcols(c) && !tcols.is_some_and(|(a, _)| a == c + n) {
            n += 1;
        }
        if tcols.is_some_and(|(a, _)| a == c) {
            out.push_str("<table:table-header-columns>");
        }
        column(out, st, k.0, k.1, k.2, n);
        if tcols.is_some_and(|(_, b)| b == c + n - 1) {
            out.push_str("</table:table-header-columns>");
        }
        c += n;
    }
    if last_col < MAX_COLS {
        column(out, st, default_mm, false, false, MAX_COLS - last_col);
    }

    // Title rows repeat at the top of each printed page
    let titles = sh.print_title_rows;
    let covered = covered_cells(sh);
    let starts: HashMap<Pos, (u32, u32)> = sh
        .merges
        .iter()
        .map(|(a, b)| (*a, (b.row - a.row + 1, b.col - a.col + 1)))
        .collect();
    // The rows where something starts or stops. A row that is not one of
    // them is written the same as the row before it
    let mut events = std::collections::BTreeSet::new();
    let per_row = sh
        .cells
        .keys()
        .map(|p| p.row)
        .chain(covered.iter().map(|p| p.row))
        .chain(extras.keys().map(|p| p.row))
        .chain(sh.row_height.keys().copied())
        .chain(sh.row_height_auto.keys().copied())
        .chain(sh.row_hidden.iter().copied())
        .chain(sh.filter_hidden.iter().copied())
        .chain(sh.row_breaks.iter().copied());
    for x in per_row {
        events.extend([x, x + 1]);
    }
    for ((a, b), _) in &vals {
        events.extend([a.row, b.row + 1]);
    }
    if let Some((a, b)) = titles {
        events.extend([a, b + 1]);
    }
    let mut r = 0;
    while r < last_row {
        if titles.is_some_and(|(a, _)| a == r) {
            out.push_str("<table:table-header-rows>");
        }
        let row_xml = row_cells(sh, r, last_col, &covered, &starts, &extras, &vals, st);
        // Rows with no cells of their own, written the same (empty, or only
        // under a validation rule), are written once with a repeat count
        let mut n = 1;
        let in_titles = |x: u32| titles.is_some_and(|(a, b)| x >= a && x <= b);
        let no_cells = |x: u32| sh.cells.range(Pos::new(x, 0)..Pos::new(x + 1, 0)).next().is_none();
        if no_cells(r) {
            while r + n < last_row {
                let x = r + n;
                if !events.contains(&x) {
                    n = events.range(x..).next().copied().unwrap_or(last_row).min(last_row) - r;
                    continue;
                }
                if !(no_cells(x)
                    && row_attrs(sh, x, st) == row_attrs(sh, r, st)
                    && in_titles(x) == in_titles(r)
                    && row_cells(sh, x, last_col, &covered, &starts, &extras, &vals, st) == row_xml
                    && !titles.is_some_and(|(a, _)| a == x))
                {
                    break;
                }
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
        // The rows to the end take the sheet's default height when it has one
        let style = match sh.default_row_height {
            Some(h) => format!(r#" table:style-name="{}""#, st.row(h, true, false)),
            None => String::new(),
        };
        let _ = write!(
            out,
            r#"<table:table-row{style} table:number-rows-repeated="{}"><table:table-cell table:number-columns-repeated="{MAX_COLS}"/></table:table-row>"#,
            MAX_ROWS - last_row
        );
    }
    // Conditional formats, in LibreOffice's calcext extension: one
    // conditional-format per range, in the order of the rules
    if !sh.cond.is_empty() {
        out.push_str("<calcext:conditional-formats>");
        let q = quote_sheet(&sh.name);
        // Neighbouring rules on the same range share one conditional-format:
        // LibreOffice draws a range's rules together only then
        for (i, rule) in sh.cond.iter().enumerate() {
            let (a, b) = rule.range;
            let first = i == 0 || sh.cond[i - 1].range != rule.range;
            let last = sh.cond.get(i + 1).is_none_or(|n| n.range != rule.range);
            if first {
                let _ = write!(
                    out,
                    r#"<calcext:conditional-format calcext:target-range-address="{}">"#,
                    esc(&format!("{q}.{}:{q}.{}", a.a1(), b.a1()))
                );
            }
            let base = esc(&format!("{q}.{}", a.a1()));
            match &rule.kind {
                book::CondKind::Bar(c) => {
                    let c = c.to_ascii_lowercase();
                    let _ = write!(
                        out,
                        r##"<calcext:data-bar calcext:min-length="10" calcext:max-length="90" calcext:negative-color="#{c}" calcext:axis-position="none" calcext:positive-color="#{c}" calcext:axis-color="#000000"><calcext:formatting-entry calcext:value="0" calcext:type="minimum"/><calcext:formatting-entry calcext:value="0" calcext:type="maximum"/></calcext:data-bar>"##
                    );
                }
                book::CondKind::Scale(lo, mid, hi) => {
                    out.push_str("<calcext:color-scale>");
                    let _ = write!(out, r##"<calcext:color-scale-entry calcext:value="0" calcext:type="minimum" calcext:color="#{}"/>"##, lo.to_ascii_lowercase());
                    if let Some(m) = mid {
                        let _ = write!(out, r##"<calcext:color-scale-entry calcext:value="50" calcext:type="percentile" calcext:color="#{}"/>"##, m.to_ascii_lowercase());
                    }
                    let _ = write!(out, r##"<calcext:color-scale-entry calcext:value="0" calcext:type="maximum" calcext:color="#{}"/></calcext:color-scale>"##, hi.to_ascii_lowercase());
                }
                book::CondKind::Icons(name) => {
                    // Thresholds at even shares, as Excel's defaults
                    let n = name.chars().next().and_then(|c| c.to_digit(10)).unwrap_or(3);
                    let _ = write!(out, r#"<calcext:icon-set calcext:icon-set-type="{}">"#, esc(name));
                    for k in 0..n {
                        let share = (k as f32 * 100.0 / n as f32).round();
                        let _ = write!(out, r#"<calcext:formatting-entry calcext:value="{share}" calcext:type="percent"/>"#);
                    }
                    out.push_str("</calcext:icon-set>");
                }
                kind => {
                    if let Some(v) = super::cond::value_of(kind) {
                        let style = st.cond_style(&rule.look);
                        let _ = write!(
                            out,
                            r#"<calcext:condition calcext:apply-style-name="{style}" calcext:value="{}" calcext:base-cell-address="{base}"/>"#,
                            esc(&v)
                        );
                    }
                }
            }
            if last {
                out.push_str("</calcext:conditional-format>");
            }
        }
        out.push_str("</calcext:conditional-formats>");
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

fn column(out: &mut String, st: &mut Styles, mm: f32, hidden: bool, brk: bool, n: u32) {
    let co = st.col(mm, brk);
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
    let _ = write!(a, r#" table:style-name="{}""#, st.row(h, auto, sh.row_breaks.contains(&r)));
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
#[allow(clippy::too_many_arguments)]
fn row_cells(
    sh: &Sheet,
    r: u32,
    last_col: u32,
    covered: &std::collections::HashSet<Pos>,
    starts: &HashMap<Pos, (u32, u32)>,
    extras: &HashMap<Pos, (String, String)>,
    vals: &[((Pos, Pos), String)],
    st: &mut Styles,
) -> Option<String> {
    let val_at = |c: u32| {
        vals.iter()
            .find(|((a, b), _)| r >= a.row && r <= b.row && c >= a.col && c <= b.col)
            .map(|(_, n)| n.as_str())
    };
    let any_val = vals.iter().any(|((a, b), _)| r >= a.row && r <= b.row);
    let used: BTreeMap<u32, &Cell> = sh.cells.range(Pos::new(r, 0)..Pos::new(r + 1, 0)).map(|(p, c)| (p.col, c)).collect();
    let any_cover = covered.iter().any(|p| p.row == r);
    let any_extra = extras.keys().any(|p| p.row == r);
    if used.is_empty() && !any_cover && !any_extra && !any_val {
        return None;
    }
    let empty_cell = Cell::default();
    let mut out = String::new();
    let mut c = 0;
    // A run of empty cells, and the validation rule they are under
    let mut blank = 0u32;
    let mut blank_val: Option<&str> = None;
    let flush = |out: &mut String, blank: &mut u32, val: Option<&str>| {
        if *blank > 0 {
            out.push_str("<table:table-cell");
            if let Some(v) = val {
                let _ = write!(out, r#" table:content-validation-name="{v}""#);
            }
            if *blank > 1 {
                let _ = write!(out, r#" table:number-columns-repeated="{blank}""#);
            }
            out.push_str("/>");
            *blank = 0;
        }
    };
    while c < last_col {
        let p = Pos::new(r, c);
        if covered.contains(&p) {
            flush(&mut out, &mut blank, blank_val);
            let style = used.get(&c).and_then(|cl| st.cell(&cl.fmt));
            out.push_str("<table:covered-table-cell");
            if let Some(v) = val_at(c) {
                let _ = write!(out, r#" table:content-validation-name="{v}""#);
            }
            if let Some(s) = style {
                let _ = write!(out, r#" table:style-name="{s}""#);
            }
            match extras.get(&p) {
                Some((note, objects)) => {
                    let _ = write!(out, ">{note}{objects}</table:covered-table-cell>");
                }
                None => out.push_str("/>"),
            }
            c += 1;
            continue;
        }
        let extra = extras.get(&p);
        let v = val_at(c);
        match used.get(&c).copied().or(extra.map(|_| &empty_cell)) {
            Some(cl) => {
                flush(&mut out, &mut blank, blank_val);
                let link = sh.links.get(&p).map(|l| link_href(l));
                cell_xml(&mut out, cl, starts.get(&p).copied(), sh.rich_runs.get(&p), extra, v, link.as_deref(), st);
            }
            None => {
                if blank > 0 && blank_val != v {
                    flush(&mut out, &mut blank, blank_val);
                }
                blank_val = v;
                blank += 1;
            }
        }
        c += 1;
    }
    // The empty cells at the end join the rest of the row unless they are
    // under a rule
    if blank_val.is_some() {
        flush(&mut out, &mut blank, blank_val);
    }
    let rest = MAX_COLS - last_col + blank;
    if rest > 0 {
        let _ = write!(out, r#"<table:table-cell table:number-columns-repeated="{rest}"/>"#);
    }
    Some(out)
}

/// One cell. `extra` is its comment and the drawing objects anchored to it:
/// the comment comes before the cell's paragraphs, the objects after them,
/// as LibreOffice writes them
#[allow(clippy::too_many_arguments)]
fn cell_xml(
    out: &mut String,
    cl: &Cell,
    span: Option<(u32, u32)>,
    runs: Option<&Vec<book::RichRun>>,
    extra: Option<&(String, String)>,
    validation: Option<&str>,
    link: Option<&str>,
    st: &mut Styles,
) {
    out.push_str("<table:table-cell");
    if let Some(v) = validation {
        let _ = write!(out, r#" table:content-validation-name="{v}""#);
    }
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
            out.push_str(r#" office:value-type="string""#);
            // A formula's text result is read from this attribute, not from
            // the paragraph (LibreOffice showed 0 without it)
            if cl.formula.is_some() {
                let _ = write!(out, r#" office:string-value="{}""#, esc(t));
            }
            out.push_str(r#" calcext:value-type="string""#);
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
    // Runs are used only while they still spell the cell's text
    let runs = runs.filter(|r| text.as_ref().is_some_and(|t| r.iter().map(|x| x.text.as_str()).collect::<String>() == *t));
    let mut inner = String::new();
    match (text, runs) {
        (Some(t), Some(runs)) if !t.is_empty() => {
            inner.push_str("<text:p>");
            for r in runs {
                let style = st.text(r);
                for (k, seg) in r.text.split('\n').enumerate() {
                    if k > 0 {
                        inner.push_str("</text:p><text:p>");
                    }
                    if seg.is_empty() {
                        continue;
                    }
                    match &style {
                        Some(s) => {
                            let _ = write!(inner, r#"<text:span text:style-name="{s}">{}</text:span>"#, para(seg));
                        }
                        None => inner.push_str(&para(seg)),
                    }
                }
            }
            inner.push_str("</text:p>");
        }
        (Some(t), _) if !t.is_empty() => {
            for line in t.split('\n') {
                inner.push_str("<text:p>");
                inner.push_str(&para(line));
                inner.push_str("</text:p>");
            }
        }
        _ => {}
    }
    // A link covers the text of each paragraph, as LibreOffice writes it
    if let Some(href) = link.filter(|_| !inner.is_empty()) {
        let a = format!(r#"<text:p><text:a xlink:href="{}" xlink:type="simple">"#, esc(href));
        inner = inner.replace("<text:p>", &a).replace("</text:p>", "</text:a></text:p>");
    }
    let (note, objects) = extra.map(|(a, b)| (a.as_str(), b.as_str())).unwrap_or(("", ""));
    if inner.is_empty() && note.is_empty() && objects.is_empty() {
        out.push_str("/>");
    } else {
        let _ = write!(out, ">{note}{inner}{objects}</table:table-cell>");
    }
}

/// A link of the model as an ODF href. A place in the workbook is
/// `#Sheet!A1` in the model (as an xlsx location) and `#Sheet.A1` in ODF
fn link_href(l: &str) -> String {
    match l.strip_prefix('#') {
        Some(loc) => match split_place(loc, '!') {
            Some((sheet, cell)) => format!("#{sheet}.{cell}"),
            None => l.to_string(),
        },
        None => l.to_string(),
    }
}

/// `Sheet!A1` → (`Sheet`, `A1`), and `Sheet!A1:Sheet!B2` or `Sheet!A1:B2`
/// → (`Sheet`, `A1:B2`), splitting at the last separator outside quotes.
/// None when what follows is not a cell or a range (a name, say)
pub(super) fn split_place(loc: &str, sep: char) -> Option<(String, String)> {
    let last_sep = |s: &str| {
        let mut quoted = false;
        let mut at = None;
        for (i, c) in s.char_indices() {
            match c {
                '\'' => quoted = !quoted,
                c if c == sep && !quoted => at = Some(i),
                _ => {}
            }
        }
        at
    };
    let is_ref = |s: &str| Pos::parse(&s.replace('$', "")).is_some();
    let (first, second) = match loc.rsplit_once(':') {
        Some((a, b)) if !b.contains('\'') => (a, Some(b)),
        _ => (loc, None),
    };
    let i = last_sep(first)?;
    let (sheet, a) = (&first[..i], &first[i + 1..]);
    if sheet.is_empty() || !is_ref(a) {
        return None;
    }
    let cell = match second {
        Some(b) => {
            let b = last_sep(b).map_or(b, |j| &b[j + 1..]);
            if !is_ref(b) {
                return None;
            }
            format!("{a}:{b}")
        }
        None => a.to_string(),
    };
    Some((sheet.to_string(), cell))
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

pub(super) fn quote_sheet(name: &str) -> String {
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
    // Headers and footers of even (left) and first pages: `1|code` when
    // shown, empty when the pages use the plain ones
    let other = |on: bool, h: &Option<String>| if on { format!("1|{}", h.as_deref().unwrap_or("")) } else { String::new() };
    let _ = write!(
        a,
        "\u{1}{}\u{1}{}\u{1}{}\u{1}{}",
        other(sh.hf_diff_odd_even, &sh.header_even),
        other(sh.hf_diff_odd_even, &sh.footer_even),
        other(sh.hf_diff_first, &sh.header_first),
        other(sh.hf_diff_first, &sh.footer_first)
    );
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
/// regions of an ODF header or footer. Changes of look (`&"Font,Bold"`,
/// `&16`, `&B`, …) become text spans whose styles are added to `looks`
/// `base` is the look each region starts in: the workbook's default font,
/// as LibreOffice gives header text when it reads an xlsx
fn hf_xml(code: &str, looks: &mut Vec<String>, base: &super::page::HfLook) -> String {
    use super::page::HfLook;
    /// A piece of a region: text or a field in a look, or a new paragraph
    enum Bit {
        Text(HfLook, String),
        Field(HfLook, &'static str),
        Para,
    }
    let mut regions: Vec<(char, Vec<Bit>)> = Vec::new();
    let mut cur = 'C';
    let mut bits: Vec<Bit> = Vec::new();
    let mut look = base.clone();
    let ch: Vec<char> = code.chars().collect();
    let mut i = 0;
    let push_text = |bits: &mut Vec<Bit>, look: &HfLook, c: char| match bits.last_mut() {
        Some(Bit::Text(l, t)) if l == look => t.push(c),
        _ => bits.push(Bit::Text(look.clone(), c.to_string())),
    };
    while i < ch.len() {
        let c = ch[i];
        if c == '\n' {
            bits.push(Bit::Para);
            i += 1;
            continue;
        }
        if c != '&' {
            push_text(&mut bits, &look, c);
            i += 1;
            continue;
        }
        let Some(&n) = ch.get(i + 1) else { break };
        i += 2;
        match n {
            // Each section starts in the plain look
            'L' | 'C' | 'R' => {
                if !bits.is_empty() {
                    regions.push((cur, std::mem::take(&mut bits)));
                }
                cur = n;
                look = base.clone();
            }
            'A' => bits.push(Bit::Field(look.clone(), "<text:sheet-name>???</text:sheet-name>")),
            'P' => bits.push(Bit::Field(look.clone(), "<text:page-number>1</text:page-number>")),
            'N' => bits.push(Bit::Field(look.clone(), "<text:page-count>99</text:page-count>")),
            'D' => bits.push(Bit::Field(look.clone(), "<text:date/>")),
            'T' => bits.push(Bit::Field(look.clone(), "<text:time/>")),
            'F' => bits.push(Bit::Field(look.clone(), "<text:title>???</text:title>")),
            '&' => push_text(&mut bits, &look, '&'),
            // `&"Font,Style"`: `-` keeps the font; the style sets bold and
            // italic (Excel writes its own language's words for them)
            '"' => {
                let start = i;
                while i < ch.len() && ch[i] != '"' {
                    i += 1;
                }
                let spec: String = ch[start..i.min(ch.len())].iter().collect();
                i += 1;
                let (font, style) = spec.split_once(',').unwrap_or((spec.as_str(), ""));
                if font != "-" && !font.is_empty() {
                    look.font = Some(font.to_string());
                }
                if !style.is_empty() {
                    let st = style.to_lowercase();
                    look.bold = st.contains("bold") || style.contains("太字");
                    look.italic = st.contains("italic") || st.contains("oblique") || style.contains("斜体");
                }
            }
            d if d.is_ascii_digit() => {
                let mut v = d.to_digit(10).unwrap_or(0);
                while i < ch.len() && ch[i].is_ascii_digit() {
                    v = v * 10 + ch[i].to_digit(10).unwrap_or(0);
                    i += 1;
                }
                if v > 0 {
                    look.size = Some(v);
                }
            }
            'B' => look.bold = !look.bold,
            'I' => look.italic = !look.italic,
            'U' | 'E' => look.underline = !look.underline,
            'S' => look.strike = !look.strike,
            // A colour: six hex digits, or a theme colour with a tint
            // (`&K01+000`); not kept
            'K' => i = (i + 6).min(ch.len()),
            _ => {}
        }
    }
    if !bits.is_empty() {
        regions.push((cur, bits));
    }
    let mut style_of = |l: &HfLook| -> Option<String> {
        if *l == HfLook::default() {
            return None;
        }
        let mut tp = String::new();
        if let Some(f) = &l.font {
            let f = esc(f);
            let _ = write!(tp, r#" fo:font-family="{f}" style:font-family-asian="{f}" style:font-family-complex="{f}""#);
        }
        if let Some(n) = l.size {
            let _ = write!(tp, r#" fo:font-size="{n}pt" style:font-size-asian="{n}pt" style:font-size-complex="{n}pt""#);
        }
        if l.bold {
            tp.push_str(r#" fo:font-weight="bold" style:font-weight-asian="bold" style:font-weight-complex="bold""#);
        }
        if l.italic {
            tp.push_str(r#" fo:font-style="italic" style:font-style-asian="italic" style:font-style-complex="italic""#);
        }
        if l.underline {
            tp.push_str(r#" style:text-underline-style="solid" style:text-underline-width="auto" style:text-underline-color="font-color""#);
        }
        if l.strike {
            tp.push_str(r#" style:text-line-through-style="solid""#);
        }
        let i = looks.iter().position(|x| *x == tp).unwrap_or_else(|| {
            looks.push(tp);
            looks.len() - 1
        });
        Some(format!("MT{}", i + 1))
    };
    let mut out = String::new();
    for k in ['L', 'C', 'R'] {
        let Some((_, bits)) = regions.iter().find(|(c, _)| *c == k) else { continue };
        let mut body = String::from("<text:p>");
        for b in bits {
            let (l, x) = match b {
                Bit::Para => {
                    body.push_str("</text:p><text:p>");
                    continue;
                }
                Bit::Text(l, t) => (l, para(t)),
                Bit::Field(l, f) => (l, f.to_string()),
            };
            match style_of(l) {
                Some(s) => {
                    let _ = write!(body, r#"<text:span text:style-name="{s}">{x}</text:span>"#);
                }
                None => body.push_str(&x),
            }
        }
        body.push_str("</text:p>");
        if body == "<text:p></text:p>" {
            continue;
        }
        let tag = match k {
            'L' => "region-left",
            'C' => "region-center",
            _ => "region-right",
        };
        let _ = write!(out, "<style:{tag}>{body}</style:{tag}>");
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
    for (i, (w, brk)) in st.cols.iter().enumerate() {
        let _ = write!(
            s,
            r#"<style:style style:name="co{}" style:family="table-column"><style:table-column-properties fo:break-before="{}" style:column-width="{w}"/></style:style>"#,
            i + 1,
            if *brk { "page" } else { "auto" }
        );
    }
    for (i, (h, auto, brk)) in st.rows.iter().enumerate() {
        let _ = write!(
            s,
            r#"<style:style style:name="ro{}" style:family="table-row"><style:table-row-properties style:row-height="{h}" fo:break-before="{}" style:use-optimal-row-height="{auto}"/></style:style>"#,
            i + 1,
            if *brk { "page" } else { "auto" }
        );
    }
    for (i, (hidden, rtl, page)) in st.tables.iter().enumerate() {
        let _ = write!(
            s,
            r#"<style:style style:name="ta{}" style:family="table" style:master-page-name="PageStyle_{}"><style:table-properties table:display="{}" style:writing-mode="{}"/></style:style>"#,
            i + 1,
            page + 1,
            !hidden,
            if *rtl { "rl-tb" } else { "lr-tb" }
        );
    }
    for (i, code) in st.codes.iter().enumerate() {
        if let Some(x) = super::numfmt_write::data_style(&format!("N{}", i + 100), code) {
            s.push_str(&x);
        }
    }
    for (i, p) in st.paras.iter().enumerate() {
        let _ = write!(s, r#"<style:style style:name="P{}" style:family="paragraph">{p}</style:style>"#, i + 1);
    }
    for (i, g) in st.graphics.iter().enumerate() {
        let _ = write!(s, r#"<style:style style:name="gr{}" style:family="graphic"><style:graphic-properties {g}/></style:style>"#, i + 1);
    }
    for (i, tp) in st.texts.iter().enumerate() {
        let _ = write!(s, r#"<style:style style:name="T{}" style:family="text"><style:text-properties{tp}/></style:style>"#, i + 1);
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
    // Data validation rules come before the sheets
    if !st.vals.is_empty() {
        s.push_str("<table:content-validations>");
        for (x, n) in &st.vals {
            let _ = write!(s, r#"<table:content-validation table:name="{n}"{x}</table:content-validation>"#);
        }
        s.push_str("</table:content-validations>");
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
    // Cells are locked unless a style says otherwise, as in an xlsx
    match (f.unlocked, f.formula_hidden) {
        (true, false) => cp.push_str(r#" style:cell-protect="none""#),
        (true, true) => cp.push_str(r#" style:cell-protect="formula-hidden""#),
        (false, true) => cp.push_str(r#" style:cell-protect="protected formula-hidden""#),
        (false, false) => {}
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
    // The looks of conditional formats, as named cell styles
    for (i, look) in st.conds.iter().enumerate() {
        let mut cp = String::new();
        if let Some(f) = &look.fill {
            let _ = write!(cp, r##"<style:table-cell-properties fo:background-color="#{}"/>"##, f.to_ascii_lowercase());
        }
        let mut tp = String::new();
        if let Some(c) = &look.color {
            let _ = write!(tp, r##" fo:color="#{}""##, c.to_ascii_lowercase());
        }
        if let Some(b) = look.bold {
            let w = if b { "bold" } else { "normal" };
            let _ = write!(tp, r#" fo:font-weight="{w}" style:font-weight-asian="{w}" style:font-weight-complex="{w}""#);
        }
        if let Some(it) = look.italic {
            let v = if it { "italic" } else { "normal" };
            let _ = write!(tp, r#" fo:font-style="{v}" style:font-style-asian="{v}" style:font-style-complex="{v}""#);
        }
        if let Some(u) = look.underline {
            let _ = write!(tp, r#" style:text-underline-style="{}""#, if u { "solid" } else { "none" });
        }
        if let Some(k) = look.strike {
            let _ = write!(tp, r#" style:text-line-through-style="{}""#, if k { "solid" } else { "none" });
        }
        if !tp.is_empty() {
            let _ = write!(cp, "<style:text-properties{tp}/>");
        }
        let _ = write!(
            s,
            r#"<style:style style:name="ConditionalStyle_{}" style:family="table-cell" style:parent-style-name="Default">{cp}</style:style>"#,
            i + 1
        );
    }
    s.push_str("</office:styles><office:automatic-styles>");
    let parts: Vec<Vec<&str>> = st.pages.iter().map(|p| p.split('\u{1}').collect()).collect();
    // The master pages first, since their text adds text styles
    let mut looks: Vec<String> = Vec::new();
    let mut masters = String::new();
    let base = super::page::HfLook {
        font: book.default_font.as_ref().map(|(f, _)| f.clone()),
        size: book.default_font.as_ref().map(|(_, pt)| pt.round() as u32).filter(|n| *n > 0),
        ..Default::default()
    };
    for (i, p) in parts.iter().enumerate() {
        // A sheet without a header or footer gets none: LibreOffice's own
        // default page style would print the sheet name and the page number
        let mut hf = |k: usize, tag: &str| -> String {
            let f: Vec<&str> = p.get(k).copied().unwrap_or("").splitn(4, '|').collect();
            match f.get(3) {
                Some(code) => format!("<style:{tag}>{}</style:{tag}>", hf_xml(code, &mut looks, &base)),
                None => format!(r#"<style:{tag} style:display="false"/>"#),
            }
        };
        let (header, footer) = (hf(1, "header"), hf(2, "footer"));
        let mut other = |k: usize, tag: &str| -> String {
            match p.get(k).and_then(|x| x.strip_prefix("1|")) {
                Some(code) => format!("<style:{tag}>{}</style:{tag}>", hf_xml(code, &mut looks, &base)),
                None => format!(r#"<style:{tag} style:display="false"/>"#),
            }
        };
        let (hl, fl, hfst, ffst) = (other(3, "header-left"), other(4, "footer-left"), other(5, "header-first"), other(6, "footer-first"));
        let _ = write!(
            masters,
            r#"<style:master-page style:name="PageStyle_{0}" style:page-layout-name="pm{0}">{header}{hl}{hfst}{footer}{fl}{ffst}</style:master-page>"#,
            i + 1
        );
    }
    for (i, tp) in looks.iter().enumerate() {
        let _ = write!(s, r#"<style:style style:name="MT{}" style:family="text"><style:text-properties{tp}/></style:style>"#, i + 1);
    }
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
    s.push_str(&masters);
    s.push_str("</office:master-styles></office:document-styles>");
    s
}

fn zip_parts(
    content: &str,
    styles: &str,
    settings: Option<&str>,
    pictures: &[(String, Vec<u8>, &'static str)],
    objects: &[(String, Vec<u8>, String)],
) -> zip::result::ZipResult<Vec<u8>> {
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
        if let Some(x) = settings {
            z.start_file("settings.xml", packed)?;
            z.write_all(x.as_bytes())?;
        }
        z.start_file("meta.xml", packed)?;
        let _ = write!(
            z,
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-meta {NS}><office:meta><meta:generator>officework</meta:generator></office:meta></office:document-meta>"#
        );
        // Pictures are already compressed
        for (path, data, _) in pictures {
            z.start_file(path.as_str(), stored)?;
            z.write_all(data)?;
        }
        for (path, data, _) in objects.iter().filter(|(p, _, _)| !p.ends_with('/')) {
            z.start_file(path.as_str(), packed)?;
            z.write_all(data)?;
        }
        z.start_file("META-INF/manifest.xml", packed)?;
        let mut m = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3"><manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>"#,
        );
        if settings.is_some() {
            m.push_str(r#"<manifest:file-entry manifest:full-path="settings.xml" manifest:media-type="text/xml"/>"#);
        }
        for (path, _, mime) in pictures {
            let _ = write!(m, r#"<manifest:file-entry manifest:full-path="{path}" manifest:media-type="{mime}"/>"#);
        }
        for (path, _, mime) in objects {
            let _ = write!(m, r#"<manifest:file-entry manifest:full-path="{}" manifest:media-type="{}"/>"#, esc(path), esc(mime));
        }
        m.push_str("</manifest:manifest>");
        z.write_all(m.as_bytes())?;
        z.finish()?;
    }
    Ok(buf.into_inner())
}
