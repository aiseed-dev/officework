//! Read an ods into the engine's workbook.
//!
//! First step (2026-10-02, SEKKEI "決め: ODF を先にする"): sheets, values,
//! formulas, merged cells, column widths, row heights and hidden rows and
//! columns. Cell formatting (fonts, fills, borders, number formats) comes
//! next; until then date and time cells get a plain date or time format so
//! they are not shown as serial numbers. Everything else in the file is
//! counted in the [`Report`] by element name, never dropped in silence.

use std::collections::HashMap;
use std::io::{Read, Seek};

use book::{Book, Cell, CellFormat, Pos, Sheet, Value};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::styles::CellStyles;
use crate::xlsx::Report;

/// Rows and columns past this are not spread out one by one. LibreOffice
/// writes the empty rest of a sheet as one repeated row or column
/// (`number-rows-repeated="1048000"`)
const SPREAD_LIMIT: u32 = 4096;

/// Empty cells that only carry a style are spread out up to this many
const STYLE_SPREAD_LIMIT: u32 = 1024;

pub fn read<R: Read + Seek>(src: R) -> Result<(Book, Report), String> {
    let mut zip = zip::ZipArchive::new(src).map_err(|e| format!("zipを開けません: {e}"))?;
    let mut mimetype = String::new();
    if let Ok(mut f) = zip.by_name("mimetype") {
        let _ = f.read_to_string(&mut mimetype);
    }
    if !mimetype.is_empty() && mimetype.trim() != "application/vnd.oasis.opendocument.spreadsheet" {
        return Err(format!("表計算の文書ではありません({})", mimetype.trim()));
    }
    let mut content = String::new();
    zip.by_name("content.xml")
        .map_err(|_| "content.xml がありません".to_string())?
        .read_to_string(&mut content)
        .map_err(|e| format!("content.xml を読めません: {e}"))?;
    let mut styles_xml = String::new();
    if let Ok(mut f) = zip.by_name("styles.xml") {
        let _ = f.read_to_string(&mut styles_xml);
    }
    let mut rep = Report::default();
    let styles = Styles::parse(&content);
    let cells = CellStyles::parse(&styles_xml, &content);
    let pages = super::page::Pages::parse(&styles_xml);
    let graphics = super::drawing::GraphicStyles::parse(&styles_xml, &content);
    // Pictures by their path in the package (`Pictures/1000….png`)
    let names: Vec<String> = zip.file_names().filter(|n| n.starts_with("Pictures/")).map(str::to_string).collect();
    let mut pictures: HashMap<String, Vec<u8>> = HashMap::new();
    for n in names {
        if let Ok(mut f) = zip.by_name(&n) {
            let mut data = Vec::new();
            if f.read_to_end(&mut data).is_ok() {
                pictures.insert(n, data);
            }
        }
    }
    // Embedded objects (charts): their folders and stand-in pictures, with
    // the media types the manifest gives them
    let names: Vec<String> =
        zip.file_names().filter(|n| n.starts_with("Object") || n.starts_with("ObjectReplacements/")).map(str::to_string).collect();
    let mut objects: HashMap<String, Vec<u8>> = HashMap::new();
    for n in names {
        if let Ok(mut f) = zip.by_name(&n) {
            let mut data = Vec::new();
            if f.read_to_end(&mut data).is_ok() {
                objects.insert(n, data);
            }
        }
    }
    let mut manifest = String::new();
    if let Ok(mut f) = zip.by_name("META-INF/manifest.xml") {
        let _ = f.read_to_string(&mut manifest);
    }
    let media = manifest_types(&manifest);
    let drawing = Drawing { pictures: &pictures, graphics: &graphics, objects: &objects, media: &media };
    let mut book = Book::new();
    book.sheets.clear();
    book.default_font = cells.default_font();
    parse_body(&content, &styles, &cells, &pages, &drawing, &mut book, &mut rep);
    if book.sheets.is_empty() {
        book.sheets.push(Sheet::new("Sheet1"));
    }
    rep.sheets = book.sheets.len();
    rep.cells = book.sheets.iter().map(|s| s.cells.len()).sum();
    Ok((book, rep))
}

/// An attribute by its qualified name (`table:name`). ODF files always use
/// these prefixes for these namespaces, and LibreOffice writes them so
pub(super) fn attr(e: &BytesStart, want: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (a.key.as_ref() == want.as_bytes())
            .then(|| a.unescape_value().map(|v| v.to_string()).unwrap_or_default())
    })
}

/// The automatic styles of content.xml that this step needs: widths of
/// column styles, heights of row styles, and whether a table style hides
/// the sheet
#[derive(Default)]
struct Styles {
    col_mm: HashMap<String, f32>,
    /// (height in pt, optimal = the height follows the content)
    row_pt: HashMap<String, (f32, bool)>,
    hidden_tables: Vec<String>,
    /// Table styles that lay the sheet out right to left
    rtl_tables: Vec<String>,
    /// table style → the master page that holds its page settings
    table_master: HashMap<String, String>,
}

impl Styles {
    fn parse(xml: &str) -> Styles {
        let mut s = Styles::default();
        let mut r = Reader::from_str(xml);
        let mut cur: Option<(String, String)> = None; // (name, family)
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                    b"style:style" => {
                        let name = attr(&e, "style:name").unwrap_or_default();
                        if let Some(m) = attr(&e, "style:master-page-name") {
                            s.table_master.insert(name.clone(), m);
                        }
                        cur = Some((name, attr(&e, "style:family").unwrap_or_default()));
                    }
                    b"style:table-column-properties" => {
                        if let (Some((name, _)), Some(w)) = (&cur, attr(&e, "style:column-width")) {
                            if let Some(mm) = length_mm(&w) {
                                s.col_mm.insert(name.clone(), mm);
                            }
                        }
                    }
                    b"style:table-row-properties" => {
                        if let (Some((name, _)), Some(h)) = (&cur, attr(&e, "style:row-height")) {
                            if let Some(mm) = length_mm(&h) {
                                let optimal = attr(&e, "style:use-optimal-row-height").as_deref() == Some("true");
                                s.row_pt.insert(name.clone(), (mm / 25.4 * 72.0, optimal));
                            }
                        }
                    }
                    b"style:table-properties" => {
                        if let Some((name, _)) = &cur {
                            if attr(&e, "table:display").as_deref() == Some("false") {
                                s.hidden_tables.push(name.clone());
                            }
                            if attr(&e, "style:writing-mode").is_some_and(|w| w.starts_with("rl")) {
                                s.rtl_tables.push(name.clone());
                            }
                        }
                    }
                    b"office:body" => break,
                    _ => {}
                },
                Ok(Event::End(e)) if e.name().as_ref() == b"style:style" => cur = None,
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        s
    }
}

/// An ODF length (`2.258cm`, `0.1665in`, `12pt`, `5mm`) in millimetres
pub(super) fn length_mm(v: &str) -> Option<f32> {
    let v = v.trim();
    let split = v.find(|c: char| c.is_ascii_alphabetic())?;
    let n: f32 = v[..split].parse().ok()?;
    let mm = match &v[split..] {
        "mm" => n,
        "cm" => n * 10.0,
        "in" => n * 25.4,
        "pt" => n * 25.4 / 72.0,
        "pc" => n * 25.4 / 6.0,
        "px" => n * 25.4 / 96.0,
        _ => return None,
    };
    Some(mm)
}

/// One cell as it is being read: the cell, and how far it spreads
struct Pending {
    cell: Cell,
    repeat: u32,
    span: (u32, u32),
    has_content: bool,
    /// Formatted differently from a plain cell: kept even when empty
    styled: bool,
    /// Runs with their own look inside the cell (`text:span`), when any
    runs: Option<Vec<book::RichRun>>,
}

fn same_look(a: &book::RichRun, b: &book::RichRun) -> bool {
    (&a.font, a.size_pt, a.bold, a.italic, &a.color, a.vert) == (&b.font, b.size_pt, b.bold, b.italic, &b.color, b.vert)
}

/// Text added to the cell, also kept as runs under the span style it sits in
fn run_push(runs: &mut Vec<(Option<String>, String)>, spans: &[String], s: &str) {
    let style = spans.last().filter(|x| !x.is_empty()).cloned();
    match runs.last_mut() {
        Some((st, t)) if *st == style => t.push_str(s),
        _ => runs.push((style, s.to_string())),
    }
}

/// What drawing objects need while they are read
struct Drawing<'a> {
    pictures: &'a HashMap<String, Vec<u8>>,
    graphics: &'a super::drawing::GraphicStyles,
    objects: &'a HashMap<String, Vec<u8>>,
    /// package path → media type, from the manifest
    media: &'a HashMap<String, String>,
}

/// The manifest's media types by path
fn manifest_types(xml: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"manifest:file-entry" => {
                if let (Some(p), Some(t)) = (attr(&e, "manifest:full-path"), attr(&e, "manifest:media-type")) {
                    out.insert(p, t);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

impl Drawing<'_> {
    /// An embedded object (a chart) kept as written, with its files
    fn keep(&self, c: &super::drawing::Capture, raw: &str, at: Option<Pos>) -> book::KeptObject {
        let (folder, stand_in) = c.object_paths();
        let mut files = Vec::new();
        if let Some(folder) = &folder {
            let dir = format!("{folder}/");
            if let Some(t) = self.media.get(&dir) {
                files.push((dir.clone(), Vec::new(), t.clone()));
            }
            let mut inside: Vec<&String> = self.objects.keys().filter(|k| k.starts_with(&dir)).collect();
            inside.sort();
            for k in inside {
                let t = self.media.get(k).cloned().unwrap_or_else(|| "text/xml".into());
                files.push((k.clone(), self.objects[k].clone(), t));
            }
        }
        if let Some(img) = stand_in.filter(|p| self.objects.contains_key(p)) {
            let t = self.media.get(&img).cloned().unwrap_or_default();
            files.push((img.clone(), self.objects[&img].clone(), t));
        }
        book::KeptObject { at, xml: raw.to_string(), files }
    }

    /// Put a finished object on its sheet
    fn place(&self, c: super::drawing::Capture, at: Pos, offset: Option<(f32, f32)>, sh: &mut Sheet, z: &mut u32, rep: &mut Report) {
        use super::drawing::Drawn;
        *z += 1;
        match c.finish(at, offset, self.pictures, self.graphics, *z) {
            Drawn::Comment(t) => {
                sh.comments.insert(at, t);
            }
            Drawn::Image(i) => sh.images.push(i),
            Drawn::Shape(s) => sh.shapes.push(*s),
            Drawn::Other(what) => rep_note(rep, &what),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_body(
    xml: &str,
    styles: &Styles,
    cells: &CellStyles,
    pages: &super::page::Pages,
    drawing: &Drawing,
    book: &mut Book,
    rep: &mut Report,
) {
    // The first row inside `table:table-header-rows`: the print title rows
    let mut header_rows_from: Option<u32> = None;
    // The same for `table:table-header-columns`: the print title columns
    let mut header_cols_from: Option<u32> = None;
    // Names for the whole workbook, put on the sheet they point at once all
    // sheets are read
    let mut pending_names: Vec<(String, book::DefinedName)> = Vec::new();
    // The look of a cell with no style of its own: empty cells in this look
    // are not kept
    let plain = cells.format("Default");
    // A column's default cell style, for cells that name none
    let mut col_style: HashMap<u32, String> = HashMap::new();
    let mut rest_style: Option<String> = None;
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);
    let mut in_body = false;
    let mut sheet: Option<Sheet> = None;
    let (mut row, mut col) = (0u32, 0u32);
    // The row being read: its repeat count and the cells it got, so a
    // repeated row with content can be copied
    let mut row_repeat = 1u32;
    let mut row_cells: Vec<(u32, Cell)> = Vec::new();
    let mut cell: Option<Pending> = None;
    // Text of the cell: paragraphs joined with line breaks
    let mut text = String::new();
    let mut paras = 0usize;
    // The cell's text as runs (span style, text), and the spans open now
    let mut runs: Vec<(Option<String>, String)> = Vec::new();
    let mut spans: Vec<String> = Vec::new();
    let mut depth_p = 0usize;
    // A comment, picture or shape being read, and the cell it is anchored
    // to (None: the page, inside `table:shapes`). Its paragraphs are its
    // own, not the cell's
    let mut cap: Option<(super::drawing::Capture, Option<Pos>)> = None;
    let mut in_shapes = false;
    // The ranges of the conditional format being read, and the colours of
    // a colour scale in it
    let mut cf_ranges: Vec<(Pos, Pos)> = Vec::new();
    let mut scale: Vec<String> = Vec::new();
    // Objects anchored to the page, placed on a cell once the sheet's
    // columns and rows are known
    let mut on_page: Vec<super::drawing::Capture> = Vec::new();
    // Stacking order of the sheet's objects, the first at the bottom
    let mut z = 0u32;
    let mut null_date: Option<(i64, i64, i64)> = None;

    loop {
        let before = r.buffer_position() as usize;
        let ev = r.read_event();
        if let Some((c, _)) = cap.as_mut() {
            let done = match &ev {
                Ok(e) => c.feed(e),
                Err(_) => true,
            };
            if done {
                let (c, at) = cap.take().expect("capturing");
                // An embedded object (a chart) is kept as written
                if c.object.is_some() {
                    if let Some(sh) = sheet.as_mut() {
                        let raw = &xml[c.raw_start..r.buffer_position() as usize];
                        sh.kept_objects.push(drawing.keep(&c, raw, at));
                    }
                    continue;
                }
                match (at, sheet.as_mut()) {
                    (Some(at), Some(sh)) => drawing.place(c, at, None, sh, &mut z, rep),
                    _ => on_page.push(c),
                }
            }
            if !matches!(ev, Ok(Event::Eof) | Err(_)) {
                continue;
            }
        }
        match ev {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let empty = matches!(ev, Ok(Event::Empty(_)));
                let name = e.name();
                let name = name.as_ref();
                if !in_body {
                    if name == b"office:spreadsheet" {
                        in_body = true;
                    }
                    continue;
                }
                match name {
                    b"table:null-date" => {
                        null_date = attr(e, "table:date-value").and_then(|d| ymd(&d));
                    }
                    b"table:table" => {
                        let mut sh = Sheet::new(&attr(e, "table:name").unwrap_or_default());
                        if let Some(st) = attr(e, "table:style-name") {
                            sh.hidden = styles.hidden_tables.contains(&st);
                            sh.rtl = styles.rtl_tables.contains(&st);
                            if let Some(m) = styles.table_master.get(&st) {
                                pages.apply(m, &mut sh);
                            }
                        }
                        if let Some(v) = attr(e, "table:print-ranges") {
                            sh.print_areas = super::page::print_ranges(&v);
                        }
                        sheet = Some(sh);
                        row = 0;
                        col = 0;
                        col_style.clear();
                        rest_style = None;
                    }
                    b"table:table-column" => {
                        if let Some(sh) = sheet.as_mut() {
                            let n = repeat(e, "table:number-columns-repeated");
                            let w = attr(e, "table:style-name").and_then(|s| styles.col_mm.get(&s).copied());
                            let hidden = attr(e, "table:visibility").is_some_and(|v| v != "visible");
                            let dstyle = attr(e, "table:default-cell-style-name");
                            if n > SPREAD_LIMIT {
                                rest_style = dstyle.clone();
                                if let Some(w) = w {
                                    sh.default_col_mm.get_or_insert(w);
                                }
                            } else {
                                for c in col..col + n {
                                    if let Some(d) = &dstyle {
                                        col_style.insert(c, d.clone());
                                    }
                                    if let Some(w) = w {
                                        sh.col_mm.insert(c, w);
                                    }
                                    if hidden {
                                        sh.col_hidden.insert(c);
                                    }
                                }
                            }
                            col = col.saturating_add(n);
                        }
                    }
                    b"table:table-header-rows" if !empty => header_rows_from = Some(row),
                    b"table:table-header-columns" if !empty => header_cols_from = Some(col),
                    b"table:table-row" => {
                        col = 0;
                        row_repeat = repeat(e, "table:number-rows-repeated");
                        row_cells.clear();
                        if let Some(sh) = sheet.as_mut() {
                            let h = attr(e, "table:style-name").and_then(|s| styles.row_pt.get(&s).copied());
                            let vis = attr(e, "table:visibility");
                            if row_repeat <= SPREAD_LIMIT {
                                for rr in row..row + row_repeat {
                                    if let Some((pt, optimal)) = h {
                                        sh.row_height.insert(rr, pt);
                                        if optimal {
                                            sh.row_height_auto.insert(rr, pt);
                                        }
                                    }
                                    match vis.as_deref() {
                                        Some("collapse") => {
                                            sh.row_hidden.insert(rr);
                                        }
                                        Some("filter") => {
                                            sh.filter_hidden.insert(rr);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        if empty {
                            row = row.saturating_add(row_repeat);
                        }
                    }
                    b"table:table-cell" | b"table:covered-table-cell" => {
                        let covered = name == b"table:covered-table-cell";
                        let style = attr(e, "table:style-name")
                            .or_else(|| col_style.get(&col).cloned())
                            .or_else(|| rest_style.clone().filter(|_| !col_style.contains_key(&col)));
                        let fmt = style.map(|s| cells.format(&s));
                        let mut p = start_cell(e, covered, null_date, rep);
                        if let Some(f) = fmt {
                            if p.cell.fmt.number_format.is_some() && f.number_format.is_none() {
                                // Keep the placeholder date or time format
                                let keep = p.cell.fmt.number_format.take();
                                p.cell.fmt = CellFormat { number_format: keep, ..f };
                            } else {
                                p.cell.fmt = f;
                            }
                            if p.cell.fmt != plain {
                                p.styled = true;
                            }
                        }
                        if empty {
                            finish_cell(p, row, &mut col, &mut row_cells, sheet.as_mut());
                        } else {
                            cell = Some(p);
                            text.clear();
                            runs.clear();
                            spans.clear();
                            paras = 0;
                        }
                    }
                    b"text:p" | b"text:h" if cell.is_some() => {
                        if paras > 0 {
                            text.push('\n');
                            run_push(&mut runs, &[], "\n");
                        }
                        paras += 1;
                        if !empty {
                            depth_p += 1;
                        }
                    }
                    b"text:s" if depth_p > 0 => {
                        let n = attr(e, "text:c").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
                        let sp: String = std::iter::repeat_n(' ', n).collect();
                        text.push_str(&sp);
                        run_push(&mut runs, &spans, &sp);
                    }
                    b"text:tab" if depth_p > 0 => {
                        text.push('\t');
                        run_push(&mut runs, &spans, "\t");
                    }
                    b"text:line-break" if depth_p > 0 => {
                        text.push('\n');
                        run_push(&mut runs, &spans, "\n");
                    }
                    b"text:span" if depth_p > 0 && !empty => spans.push(attr(e, "text:style-name").unwrap_or_default()),
                    b"table:shapes" if !empty => in_shapes = true,
                    // Comments, pictures, charts and shapes: read on their own,
                    // their paragraphs kept out of the cell
                    n if n == b"office:annotation" || n.starts_with(b"draw:") => {
                        if let Some(mut c) = super::drawing::Capture::start(e) {
                            c.raw_start = before;
                            let at = (!in_shapes).then(|| Pos::new(row, col));
                            if empty {
                                match (at, sheet.as_mut()) {
                                    (Some(at), Some(sh)) => drawing.place(c, at, None, sh, &mut z, rep),
                                    _ => on_page.push(c),
                                }
                            } else {
                                cap = Some((c, at));
                            }
                        }
                    }
                    b"table:named-range" => {
                        let nm = attr(e, "table:name").unwrap_or_default();
                        let addr = attr(e, "table:cell-range-address").unwrap_or_default();
                        // Print areas and title rows come from their own
                        // attributes; LibreOffice also writes them as names
                        if !nm.starts_with("_xlnm.") {
                            match named_range(&addr) {
                                Some((on, range)) => {
                                    let n = book::DefinedName { name: nm, range, scoped: sheet.is_some() };
                                    match sheet.as_mut() {
                                        Some(sh) => sh.names.push(n),
                                        None => pending_names.push((on, n)),
                                    }
                                }
                                None => rep_note(rep, "table:named-range"),
                            }
                        }
                    }
                    b"table:named-expression" => rep_note(rep, "table:named-expression"),
                    // Conditional formats (LibreOffice's calcext extension)
                    b"calcext:conditional-format" => {
                        cf_ranges = attr(e, "calcext:target-range-address").map(|v| super::page::print_ranges(&v)).unwrap_or_default();
                    }
                    b"calcext:condition" => {
                        let at = cf_ranges.first().map(|r| r.0).unwrap_or_default();
                        let kind = attr(e, "calcext:value").and_then(|v| super::cond::kind_of(&v, at));
                        let look = attr(e, "calcext:apply-style-name").map(|n| cells.cond_look(&n)).unwrap_or_default();
                        match (kind, sheet.as_mut()) {
                            (Some(kind), Some(sh)) => {
                                for r in &cf_ranges {
                                    sh.cond.push(book::CondRule { range: *r, kind: kind.clone(), look: look.clone() });
                                }
                            }
                            _ => rep_note(rep, "calcext:condition"),
                        }
                    }
                    b"calcext:data-bar" => {
                        let color = attr(e, "calcext:positive-color").and_then(|c| c.strip_prefix('#').map(|h| h.to_ascii_uppercase())).unwrap_or_else(|| "638EC6".into());
                        if let Some(sh) = sheet.as_mut() {
                            for r in &cf_ranges {
                                sh.cond.push(book::CondRule { range: *r, kind: book::CondKind::Bar(color.clone()), look: Default::default() });
                            }
                        }
                    }
                    b"calcext:color-scale" if !empty => scale.clear(),
                    b"calcext:color-scale-entry" => {
                        if let Some(c) = attr(e, "calcext:color").and_then(|c| c.strip_prefix('#').map(|h| h.to_ascii_uppercase())) {
                            scale.push(c);
                        }
                    }
                    b"calcext:icon-set" => {
                        let name = attr(e, "calcext:icon-set-type").unwrap_or_else(|| "3Arrows".into());
                        if let Some(sh) = sheet.as_mut() {
                            for r in &cf_ranges {
                                sh.cond.push(book::CondRule { range: *r, kind: book::CondKind::Icons(name.clone()), look: Default::default() });
                            }
                        }
                    }
                    b"calcext:date-is" => rep_note(rep, "calcext:date-is"),
                    b"table:database-ranges"
                    | b"table:content-validations"
                    | b"table:data-pilot-tables"
                    | b"table:table-row-group"
                    | b"table:table-column-group" => {
                        rep_note(rep, std::str::from_utf8(name).unwrap_or("?"));
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if depth_p > 0 {
                    let t = t.unescape().unwrap_or_default();
                    text.push_str(&t);
                    run_push(&mut runs, &spans, &t);
                }
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"table:shapes" => in_shapes = false,
                    b"calcext:color-scale" => {
                        let kind = match scale.as_slice() {
                            [a, b] => Some(book::CondKind::Scale(a.clone(), None, b.clone())),
                            [a, m, b] => Some(book::CondKind::Scale(a.clone(), Some(m.clone()), b.clone())),
                            _ => None,
                        };
                        match (kind, sheet.as_mut()) {
                            (Some(kind), Some(sh)) => {
                                for r in &cf_ranges {
                                    sh.cond.push(book::CondRule { range: *r, kind: kind.clone(), look: Default::default() });
                                }
                            }
                            _ => rep_note(rep, "calcext:color-scale"),
                        }
                    }
                    b"text:p" | b"text:h" if depth_p > 0 => depth_p -= 1,
                    b"text:span" if depth_p > 0 => {
                        spans.pop();
                    }
                    b"table:table-cell" | b"table:covered-table-cell" => {
                        if let Some(mut p) = cell.take() {
                            if paras > 0 {
                                fill_text(&mut p, &text);
                            }
                            if runs.iter().any(|(st, _)| st.is_some()) {
                                // Neighbouring runs that look the same are one run:
                                // two text styles may say the same thing
                                let mut merged: Vec<book::RichRun> = Vec::new();
                                for (st, t) in runs.drain(..) {
                                    let r = match st {
                                        Some(st) => cells.run(&st, t),
                                        None => book::RichRun { text: t, ..Default::default() },
                                    };
                                    // A line break has no look of its own: it goes
                                    // with the run before it
                                    let only_breaks = r.text.chars().all(|c| c == '\n');
                                    match merged.last_mut() {
                                        Some(last) if only_breaks || same_look(last, &r) => last.text.push_str(&r.text),
                                        _ => merged.push(r),
                                    }
                                }
                                if merged.iter().any(|r| !same_look(r, &book::RichRun::default())) {
                                    p.runs = Some(merged);
                                }
                            }
                            finish_cell(p, row, &mut col, &mut row_cells, sheet.as_mut());
                        }
                    }
                    b"table:table-row" => {
                        // A repeated row with content is copied to every row it covers
                        if let Some(sh) = sheet.as_mut() {
                            // Rows that only carry styles are not copied past
                            // the limit for styled cells, for the same reason
                            let content = row_cells.iter().any(|(_, c)| c.formula.is_some() || !c.value.is_empty());
                            let copies = if content {
                                row_repeat.min(SPREAD_LIMIT)
                            } else if row_repeat > STYLE_SPREAD_LIMIT {
                                1
                            } else {
                                row_repeat
                            };
                            for k in 1..copies {
                                for (c, cl) in &row_cells {
                                    sh.set(Pos::new(row + k, *c), cl.clone());
                                }
                            }
                        }
                        row = row.saturating_add(row_repeat);
                    }
                    b"table:table-header-columns" => {
                        if let (Some(from), Some(sh)) = (header_cols_from.take(), sheet.as_mut()) {
                            if col > from {
                                sh.print_title_cols = Some((from, col - 1));
                            }
                        }
                    }
                    b"table:table-header-rows" => {
                        if let (Some(from), Some(sh)) = (header_rows_from.take(), sheet.as_mut()) {
                            if row > from {
                                sh.print_title_rows = Some((from, row - 1));
                            }
                        }
                    }
                    b"table:table" => {
                        if let Some(sh) = sheet.as_mut() {
                            for c in on_page.drain(..) {
                                let (x, y, _, _) = c.rect_mm();
                                let (at, ox, oy) = super::drawing::cell_at(sh, &book.col_basis, x, y);
                                drawing.place(c, at, Some((ox, oy)), sh, &mut z, rep);
                            }
                        }
                        z = 0;
                        if let Some(sh) = sheet.take() {
                            book.sheets.push(sh);
                        }
                    }
                    b"office:spreadsheet" => break,
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    for (on, n) in pending_names {
        match book.sheets.iter_mut().find(|s| s.name == on) {
            Some(sh) => sh.names.push(n),
            None => rep_note(rep, "table:named-range"),
        }
    }
    if null_date == Some((1904, 1, 1)) {
        book.date1904 = true;
    }
}

/// `$出納帳.$A$1:.$F$19` → (sheet, `A1:F19`). Only one cell or one block
/// of cells, as the model's names hold
fn named_range(addr: &str) -> Option<(String, String)> {
    let a1 = super::formula::to_a1(&format!("of:=[{addr}]"))?;
    let (sheet, r) = a1.rsplit_once('!')?;
    let sheet = match sheet.strip_prefix('\'').and_then(|q| q.strip_suffix('\'')) {
        Some(q) => q.replace("''", "'"),
        None => sheet.to_string(),
    };
    let plain: String = r.chars().filter(|c| *c != '$').collect();
    let ok = match plain.split_once(':') {
        Some((a, b)) => Pos::parse(a).is_some() && Pos::parse(b).is_some(),
        None => Pos::parse(&plain).is_some(),
    };
    ok.then_some((sheet, plain))
}

fn repeat(e: &BytesStart, key: &str) -> u32 {
    attr(e, key).and_then(|v| v.parse::<u32>().ok()).filter(|n| *n > 0).unwrap_or(1)
}

fn rep_note(rep: &mut Report, n: &str) {
    match rep.unsupported.iter_mut().find(|(x, _)| x == n) {
        Some(e) => e.1 += 1,
        None => rep.unsupported.push((n.to_string(), 1)),
    }
}

fn start_cell(e: &BytesStart, covered: bool, null_date: Option<(i64, i64, i64)>, rep: &mut Report) -> Pending {
    let repeat = repeat(e, "table:number-columns-repeated");
    let span = (
        attr(e, "table:number-rows-spanned").and_then(|v| v.parse().ok()).unwrap_or(1u32),
        attr(e, "table:number-columns-spanned").and_then(|v| v.parse().ok()).unwrap_or(1u32),
    );
    let mut cell = Cell::default();
    let mut has_content = false;
    if covered {
        return Pending { cell, repeat, span: (1, 1), has_content, styled: false, runs: None };
    }
    if let Some(f) = attr(e, "table:formula") {
        match super::formula::to_a1(&f) {
            Some(a1) => {
                cell.formula = Some(a1);
                has_content = true;
            }
            None => rep_note(rep, "table:formula(読めない書き方)"),
        }
    }
    let vt = attr(e, "office:value-type");
    let ext = attr(e, "calcext:value-type");
    let mut fmt = CellFormat::default();
    match vt.as_deref() {
        Some("float") | Some("percentage") | Some("currency") => {
            if let Some(v) = attr(e, "office:value").and_then(|v| v.parse::<f64>().ok()) {
                cell.value = Value::Number(v);
                has_content = true;
            }
        }
        Some("date") => {
            if let Some((serial, has_time)) = attr(e, "office:date-value").and_then(|d| date_serial(&d, null_date)) {
                cell.value = Value::Number(serial);
                // Placeholder until number styles are read
                fmt.number_format = Some(if has_time { "yyyy/m/d h:mm" } else { "yyyy/m/d" }.into());
                has_content = true;
            }
        }
        Some("time") => {
            if let Some(d) = attr(e, "office:time-value").and_then(|t| duration_days(&t)) {
                cell.value = Value::Number(d);
                fmt.number_format = Some("[h]:mm:ss".into());
                has_content = true;
            }
        }
        Some("boolean") => {
            if let Some(b) = attr(e, "office:boolean-value") {
                cell.value = Value::Bool(b == "true");
                has_content = true;
            }
        }
        Some("string") => {
            // The text is usually in the paragraphs, filled in later; a
            // formula's empty string has no paragraph and stays ""
            cell.value = Value::Text(attr(e, "office:string-value").unwrap_or_default());
            has_content = true;
        }
        _ => {}
    }
    if ext.as_deref() == Some("error") {
        // Filled from the cell's text when the paragraphs arrive
        cell.value = Value::Error(String::new());
        has_content = true;
    }
    cell.fmt = fmt;
    Pending { cell, repeat, span, has_content, styled: false, runs: None }
}

/// The paragraphs of a cell: the value of a string cell, or of an error
fn fill_text(p: &mut Pending, text: &str) {
    match &p.cell.value {
        Value::Error(_) => p.cell.value = Value::Error(text.to_string()),
        Value::Text(t) if t.is_empty() => p.cell.value = Value::Text(text.to_string()),
        Value::Text(_) => {}
        // A string cell keeps its text only in the paragraphs
        Value::Empty if !text.is_empty() => {
            p.cell.value = Value::Text(text.to_string());
            p.has_content = true;
        }
        _ => {}
    }
}

fn finish_cell(p: Pending, row: u32, col: &mut u32, row_cells: &mut Vec<(u32, Cell)>, sheet: Option<&mut Sheet>) {
    let Some(sh) = sheet else {
        *col = col.saturating_add(p.repeat);
        return;
    };
    if p.span.0 > 1 || p.span.1 > 1 {
        sh.merges.push((Pos::new(row, *col), Pos::new(row + p.span.0 - 1, *col + p.span.1 - 1)));
    }
    // A styled empty cell repeated this far is the style of the rest of the
    // row, not cells someone formatted; it is not spread out
    if p.has_content || (p.styled && p.repeat <= STYLE_SPREAD_LIMIT) {
        for k in 0..p.repeat.min(SPREAD_LIMIT) {
            sh.set(Pos::new(row, *col + k), p.cell.clone());
            row_cells.push((*col + k, p.cell.clone()));
            if let Some(r) = &p.runs {
                sh.rich_runs.insert(Pos::new(row, *col + k), r.clone());
            }
        }
    }
    *col = col.saturating_add(p.repeat);
}

fn ymd(d: &str) -> Option<(i64, i64, i64)> {
    let date = d.split('T').next()?;
    let mut it = date.split('-');
    let y = it.next()?.parse().ok()?;
    let m = it.next()?.parse().ok()?;
    let dd = it.next()?.parse().ok()?;
    Some((y, m, dd))
}

/// `2026-08-04` or `2026-08-04T13:30:00.5` as a serial number counted from
/// the workbook's null date (1899-12-30 when the file names none). The flag
/// says whether there was a time part
fn date_serial(d: &str, null_date: Option<(i64, i64, i64)>) -> Option<(f64, bool)> {
    let (y, m, dd) = ymd(d)?;
    let (ny, nm, nd) = null_date.unwrap_or((1899, 12, 30));
    let days = book::calc::date_serial(y, m, dd) - book::calc::date_serial(ny, nm, nd);
    let mut serial = days as f64;
    let mut has_time = false;
    if let Some(t) = d.split('T').nth(1) {
        let mut it = t.split(':');
        let h: f64 = it.next()?.parse().ok()?;
        let mi: f64 = it.next().unwrap_or("0").parse().ok()?;
        let s: f64 = it.next().unwrap_or("0").trim_end_matches('Z').parse().ok()?;
        let frac = (h * 3600.0 + mi * 60.0 + s) / 86400.0;
        has_time = frac > 0.0;
        serial += frac;
    }
    Some((serial, has_time))
}

/// An ISO 8601 duration (`PT12H30M00S`, `P1DT2H`) in days
fn duration_days(t: &str) -> Option<f64> {
    let neg = t.starts_with('-');
    let t = t.trim_start_matches('-').strip_prefix('P')?;
    let (date, time) = match t.split_once('T') {
        Some((d, tm)) => (d, tm),
        None => (t, ""),
    };
    let mut secs = 0.0f64;
    let mut num = String::new();
    for c in date.chars() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
        } else {
            let n: f64 = num.parse().ok()?;
            num.clear();
            match c {
                'D' => secs += n * 86400.0,
                'W' => secs += n * 7.0 * 86400.0,
                _ => return None,
            }
        }
    }
    for c in time.chars() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
        } else {
            let n: f64 = num.parse().ok()?;
            num.clear();
            match c {
                'H' => secs += n * 3600.0,
                'M' => secs += n * 60.0,
                'S' => secs += n,
                _ => return None,
            }
        }
    }
    let d = secs / 86400.0;
    Some(if neg { -d } else { d })
}
