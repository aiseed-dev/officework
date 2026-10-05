//! Page settings of an ods: each sheet's table style names a master page in
//! styles.xml, which names a page layout with the paper, orientation,
//! margins and scaling.
//!
//! The engine keeps margins the way an xlsx does: the distance from the
//! paper's edge to where the cells start. ODF measures `fo:margin-top` to
//! the header, and the header's height and spacing come on top, so a shown
//! header or footer is added to the margin here.

use std::collections::HashMap;

use book::Sheet;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::read::{attr, length_mm};

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Page {
    width_mm: Option<f32>,
    height_mm: Option<f32>,
    landscape: bool,
    margins: [Option<f32>; 4], // left, right, top, bottom
    header_mm: f32,
    footer_mm: f32,
    scale: Option<u32>,
    fit: Option<(u32, u32)>,
    grid: bool,
    headings: bool,
    centering: Option<String>,
}

#[derive(Default)]
pub(super) struct Pages {
    /// master page name → page layout name, header shown, footer shown
    masters: HashMap<String, (String, bool, bool)>,
    /// master page name → its headers and footers as xlsx header text
    /// (`&L…&C…&R…` with `&A` for the sheet name, `&P` for the page)
    texts: HashMap<String, HfTexts>,
    layouts: HashMap<String, Page>,
}

/// The headers and footers of a master page. Each is None when it is not
/// shown. The left (even) and first page ones exist only when shown
#[derive(Default)]
struct HfTexts {
    header: Option<String>,
    footer: Option<String>,
    header_left: Option<String>,
    footer_left: Option<String>,
    header_first: Option<String>,
    footer_first: Option<String>,
}

/// The look of text in a header or footer, the part that xlsx header codes
/// can carry
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct HfLook {
    pub(super) font: Option<String>,
    pub(super) size: Option<u32>,
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) underline: bool,
    pub(super) strike: bool,
}

/// The codes that change text from look `from` to look `to`
/// (`&"Font,Bold"`, `&16`, `&B`, `&I`, `&U`, `&S`). A font is named with
/// its style; without a new font, bold and italic are switched
pub(super) fn look_codes(from: &HfLook, to: &HfLook) -> String {
    let mut s = String::new();
    match &to.font {
        Some(f) if to.font != from.font => {
            let style = match (to.bold, to.italic) {
                (true, true) => "Bold Italic",
                (true, false) => "Bold",
                (false, true) => "Italic",
                (false, false) => "Regular",
            };
            s.push_str(&format!("&\"{f},{style}\""));
        }
        _ => {
            if to.bold != from.bold {
                s.push_str("&B");
            }
            if to.italic != from.italic {
                s.push_str("&I");
            }
        }
    }
    if let Some(n) = to.size.filter(|_| to.size != from.size) {
        s.push_str(&format!("&{n}"));
    }
    if to.underline != from.underline {
        s.push_str("&U");
    }
    if to.strike != from.strike {
        s.push_str("&S");
    }
    s
}

impl Pages {
    /// `base` is the look header text starts in: the workbook's default
    /// font, as LibreOffice starts header text when it reads an xlsx
    /// (sc/source/filter/oox/pagesettings.cxx). A span in that look adds
    /// no codes
    pub(super) fn parse(styles_xml: &str, base: HfLook) -> Pages {
        let mut p = Pages::default();
        let mut r = Reader::from_str(styles_xml);
        let mut layout: Option<String> = None;
        // in a header-style (true) or footer-style (false)
        let mut hf: Option<bool> = None;
        let mut master: Option<(String, String, bool, bool)> = None;
        // Inside a shown header (true) or footer (false) of a master page:
        // the text of its regions so far
        let mut hf_text: Option<(Vec<u8>, HfText)> = None;
        let mut texts = HfTexts::default();
        // Text styles (`MT1`, …) and font faces, for the look of header text
        let mut looks: HashMap<String, HfLook> = HashMap::new();
        let mut faces: HashMap<String, String> = HashMap::new();
        let mut text_style: Option<String> = None;
        loop {
            let ev = r.read_event();
            if let Some((tag, t)) = hf_text.as_mut() {
                match &ev {
                    Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                        t.element(e, matches!(ev, Ok(Event::Empty(_))), &looks)
                    }
                    Ok(Event::Text(x)) => t.text(&x.unescape().unwrap_or_default()),
                    Ok(Event::End(e)) if e.name().as_ref() == tag.as_slice() => {
                        if let Some((tag, t)) = hf_text.take() {
                            let code = Some(t.code());
                            match tag.as_slice() {
                                b"style:header" => texts.header = code,
                                b"style:footer" => texts.footer = code,
                                b"style:header-left" => texts.header_left = code,
                                b"style:footer-left" => texts.footer_left = code,
                                b"style:header-first" => texts.header_first = code,
                                _ => texts.footer_first = code,
                            }
                        }
                        continue;
                    }
                    Ok(Event::End(e)) => t.end(e.name().as_ref()),
                    _ => {}
                }
                continue;
            }
            let empty = matches!(ev, Ok(Event::Empty(_)));
            match ev {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                    b"style:page-layout" => {
                        let n = attr(&e, "style:name").unwrap_or_default();
                        p.layouts.insert(n.clone(), Page::default());
                        layout = Some(n);
                    }
                    b"style:page-layout-properties" => {
                        if let Some(pg) = layout.as_ref().and_then(|n| p.layouts.get_mut(n)) {
                            pg.width_mm = attr(&e, "fo:page-width").and_then(|v| length_mm(&v));
                            pg.height_mm = attr(&e, "fo:page-height").and_then(|v| length_mm(&v));
                            pg.landscape = attr(&e, "style:print-orientation").as_deref() == Some("landscape");
                            for (i, k) in ["fo:margin-left", "fo:margin-right", "fo:margin-top", "fo:margin-bottom"].iter().enumerate() {
                                pg.margins[i] = attr(&e, k).and_then(|v| length_mm(&v));
                            }
                            pg.scale = attr(&e, "style:scale-to").and_then(|v| v.trim_end_matches('%').parse::<f32>().ok()).map(|v| v.round() as u32);
                            let x = attr(&e, "style:scale-to-X").and_then(|v| v.parse().ok());
                            let y = attr(&e, "style:scale-to-Y").and_then(|v| v.parse().ok());
                            if x.is_some() || y.is_some() {
                                pg.fit = Some((x.unwrap_or(0), y.unwrap_or(0)));
                            }
                            // `style:scale-to-pages` fits the whole sheet on that many pages
                            if let Some(n) = attr(&e, "style:scale-to-pages").and_then(|v| v.parse::<u32>().ok()) {
                                pg.fit = Some((n, n));
                            }
                            let print = attr(&e, "style:print").unwrap_or_default();
                            pg.grid = print.split_whitespace().any(|w| w == "grid");
                            pg.headings = print.split_whitespace().any(|w| w == "headers");
                            pg.centering = attr(&e, "style:table-centering").filter(|v| v != "none");
                        }
                    }
                    b"style:font-face" => {
                        if let (Some(n), Some(f)) = (attr(&e, "style:name"), attr(&e, "svg:font-family")) {
                            faces.insert(n, f.trim_matches(|c| c == '\'' || c == '"').to_string());
                        }
                    }
                    b"style:style" if attr(&e, "style:family").as_deref() == Some("text") => {
                        text_style = attr(&e, "style:name");
                    }
                    b"style:text-properties" => {
                        if let Some(n) = &text_style {
                            looks.insert(n.clone(), text_look(&e, &faces));
                        }
                    }
                    b"style:header-style" => hf = Some(true),
                    b"style:footer-style" => hf = Some(false),
                    b"style:header-footer-properties" => {
                        // LibreOffice writes the header's whole height, the
                        // spacing to the cells included, as `fo:min-height`
                        // (a fixed one as `svg:height`); `fo:margin-bottom`
                        // is the part of it that is spacing. Checked on the
                        // corpus: min-height 1.101cm = text 0.388cm (11pt)
                        // + margin 0.713cm
                        if let (Some(is_header), Some(pg)) = (hf, layout.as_ref().and_then(|n| p.layouts.get_mut(n))) {
                            let h = attr(&e, "fo:min-height").or_else(|| attr(&e, "svg:height")).and_then(|v| length_mm(&v)).unwrap_or(0.0);
                            if is_header {
                                pg.header_mm = h;
                            } else {
                                pg.footer_mm = h;
                            }
                        }
                    }
                    b"style:master-page" => {
                        master = Some((
                            attr(&e, "style:name").unwrap_or_default(),
                            attr(&e, "style:page-layout-name").unwrap_or_default(),
                            false,
                            false,
                        ));
                    }
                    // A header or footer is shown unless it says display="false"
                    b"style:header" | b"style:footer" => {
                        let is_header = e.name().as_ref() == b"style:header";
                        let shown = attr(&e, "style:display").as_deref() != Some("false");
                        if let Some(m) = master.as_mut() {
                            if is_header {
                                m.2 = shown;
                            } else {
                                m.3 = shown;
                            }
                            // An empty element is a shown header with nothing in it
                            if shown && !empty {
                                hf_text = Some((e.name().as_ref().to_vec(), HfText::new(base.clone())));
                            }
                        }
                    }
                    // Headers and footers of left (even) and first pages, used
                    // only when shown
                    b"style:header-left" | b"style:footer-left" | b"style:header-first" | b"style:footer-first"
                        if master.is_some() && attr(&e, "style:display").as_deref() != Some("false") =>
                    {
                        let code = (!empty).then(|| (e.name().as_ref().to_vec(), HfText::new(base.clone())));
                        match code {
                            Some(c) => hf_text = Some(c),
                            None => match e.name().as_ref() {
                                b"style:header-left" => texts.header_left = Some(String::new()),
                                b"style:footer-left" => texts.footer_left = Some(String::new()),
                                b"style:header-first" => texts.header_first = Some(String::new()),
                                _ => texts.footer_first = Some(String::new()),
                            },
                        }
                    }
                    _ => {}
                },
                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"style:page-layout" => layout = None,
                    b"style:style" => text_style = None,
                    b"style:header-style" | b"style:footer-style" => hf = None,
                    b"style:master-page" => {
                        if let Some((n, l, h, f)) = master.take() {
                            p.texts.insert(n.clone(), std::mem::take(&mut texts));
                            p.masters.insert(n, (l, h, f));
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        p
    }

    /// Put the page settings of master page `master` on the sheet
    pub(super) fn apply(&self, master: &str, sh: &mut Sheet) {
        let Some((layout, header, footer)) = self.masters.get(master) else { return };
        let Some(pg) = self.layouts.get(layout) else { return };
        if let Some(t) = self.texts.get(master) {
            let some = |x: &Option<String>| x.clone().filter(|t| !t.is_empty());
            sh.header = some(&t.header).filter(|_| *header);
            sh.footer = some(&t.footer).filter(|_| *footer);
            sh.hf_diff_odd_even = t.header_left.is_some() || t.footer_left.is_some();
            sh.header_even = some(&t.header_left);
            sh.footer_even = some(&t.footer_left);
            sh.hf_diff_first = t.header_first.is_some() || t.footer_first.is_some();
            sh.header_first = some(&t.header_first);
            sh.footer_first = some(&t.footer_first);
        }
        sh.landscape = pg.landscape;
        if let (Some(w), Some(h)) = (pg.width_mm, pg.height_mm) {
            let (short, long) = if w < h { (w, h) } else { (h, w) };
            sh.paper_size = [1u32, 5, 8, 9, 11, 12, 13].into_iter().find(|code| {
                super::write::paper_mm(*code).is_some_and(|(pw, ph)| (pw - short).abs() < 1.0 && (ph - long).abs() < 1.0)
            });
        }
        if let [Some(l), Some(r), Some(t), Some(b)] = pg.margins {
            if *header || *footer {
                sh.hf_margins_mm = Some((if *header { t } else { 7.62 }, if *footer { b } else { 7.62 }));
            }
            let t = t + if *header { pg.header_mm } else { 0.0 };
            let b = b + if *footer { pg.footer_mm } else { 0.0 };
            sh.margins_mm = Some((l, r, t, b));
        }
        sh.print_scale = pg.scale.filter(|s| *s != 100);
        if let Some((w, h)) = pg.fit {
            sh.fit_to_w = Some(w);
            sh.fit_to_h = Some(h);
        }
        sh.print_gridlines = pg.grid;
        sh.print_headings = pg.headings;
        match pg.centering.as_deref() {
            Some("both") => (sh.h_centered, sh.v_centered) = (true, true),
            Some("horizontal") => sh.h_centered = true,
            Some("vertical") => sh.v_centered = true,
            _ => {}
        }
    }
}

/// The look a text style gives: font, size, bold, italic, underline, strike
fn text_look(e: &BytesStart, faces: &HashMap<String, String>) -> HfLook {
    let font = attr(e, "style:font-name")
        .map(|n| faces.get(&n).cloned().unwrap_or(n))
        .or_else(|| attr(e, "fo:font-family").map(|f| f.trim_matches(|c| c == '\'' || c == '"').to_string()));
    let size = attr(e, "fo:font-size").and_then(|v| v.strip_suffix("pt").and_then(|n| n.parse::<f32>().ok())).map(|n| n.round() as u32);
    let on = |k: &str, off: &str| attr(e, k).is_some_and(|v| v != off);
    HfLook {
        font,
        size,
        bold: attr(e, "fo:font-weight").is_some_and(|w| w == "bold" || w.parse::<u32>().is_ok_and(|n| n >= 600)),
        italic: on("fo:font-style", "normal"),
        underline: on("style:text-underline-style", "none"),
        strike: on("style:text-line-through-style", "none"),
    }
}

/// The text of a header or footer as it is read: one string per region
/// (`L`, `C`, `R`); text outside regions counts as the centre. A change of
/// look on the way becomes codes, as in an xlsx
struct HfText {
    /// The look every region starts in
    base: HfLook,
    /// Each region's text and the look it has reached
    regions: Vec<(char, String, HfLook)>,
    cur: Option<char>,
    paras: usize,
    in_field: bool,
    /// The looks of the spans open now (None for a style without one)
    spans: Vec<Option<HfLook>>,
}

impl HfText {
    fn new(base: HfLook) -> HfText {
        HfText { base, regions: Vec::new(), cur: None, paras: 0, in_field: false, spans: Vec::new() }
    }
    fn slot(&mut self) -> (&mut String, &mut HfLook) {
        let k = self.cur.unwrap_or('C');
        let i = match self.regions.iter().position(|(c, _, _)| *c == k) {
            Some(i) => i,
            None => {
                self.regions.push((k, String::new(), self.base.clone()));
                self.regions.len() - 1
            }
        };
        let (_, t, l) = &mut self.regions[i];
        (t, l)
    }
    /// Text or a code in the look of the spans open now
    fn put(&mut self, s: &str) {
        let want = self.spans.iter().rev().find_map(|l| l.clone()).unwrap_or_else(|| self.base.clone());
        let (t, have) = self.slot();
        // A look cannot go back to "no font" or "no size" in codes: those stay
        let want = HfLook { font: want.font.or(have.font.clone()), size: want.size.or(have.size), ..want };
        let codes = look_codes(have, &want);
        t.push_str(&codes);
        // A size code runs into digits after it (`&11` and `1` read as
        // `&111`); bold switched on and off again keeps them apart
        if codes.chars().last().is_some_and(|c| c.is_ascii_digit()) && s.starts_with(|c: char| c.is_ascii_digit()) {
            t.push_str("&B&B");
        }
        *have = want;
        t.push_str(s);
    }
    fn element(&mut self, e: &BytesStart, empty: bool, looks: &HashMap<String, HfLook>) {
        let name = e.name();
        let name = name.as_ref();
        match name {
            b"style:region-left" => {
                self.cur = Some('L');
                self.paras = 0;
            }
            b"style:region-center" => {
                self.cur = Some('C');
                self.paras = 0;
            }
            b"style:region-right" => {
                self.cur = Some('R');
                self.paras = 0;
            }
            b"text:p" => {
                if self.paras > 0 {
                    self.slot().0.push('\n');
                }
                self.paras += 1;
            }
            b"text:span" if !empty => self.spans.push(attr(e, "text:style-name").and_then(|n| looks.get(&n).cloned())),
            b"text:sheet-name" => self.put("&A"),
            b"text:page-number" => self.put("&P"),
            b"text:page-count" => self.put("&N"),
            b"text:date" => self.put("&D"),
            b"text:time" => self.put("&T"),
            b"text:title" | b"text:file-name" => self.put("&F"),
            b"text:s" => {
                let n = attr(e, "text:c").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
                self.put(&" ".repeat(n));
            }
            b"text:tab" => self.put("\t"),
            _ => {}
        }
        // A field's own text (`???`, `1`) is LibreOffice's preview of it
        if matches!(name, b"text:sheet-name" | b"text:page-number" | b"text:page-count" | b"text:date" | b"text:time" | b"text:title" | b"text:file-name") && !empty {
            self.in_field = true;
        }
    }
    fn end(&mut self, name: &[u8]) {
        if matches!(name, b"text:sheet-name" | b"text:page-number" | b"text:page-count" | b"text:date" | b"text:time" | b"text:title" | b"text:file-name") {
            self.in_field = false;
        }
        if name == b"text:span" {
            self.spans.pop();
        }
        if matches!(name, b"style:region-left" | b"style:region-center" | b"style:region-right") {
            self.cur = None;
        }
    }
    fn text(&mut self, t: &str) {
        if !self.in_field {
            self.put(&t.replace('&', "&&"));
        }
    }
    /// As xlsx header text: `&L…&C…&R…`, empty regions left out
    fn code(&self) -> String {
        let mut out = String::new();
        for k in ['L', 'C', 'R'] {
            if let Some((_, t, _)) = self.regions.iter().find(|(c, _, _)| *c == k) {
                if !t.is_empty() {
                    out.push('&');
                    out.push(k);
                    out.push_str(t);
                }
            }
        }
        out
    }
}

/// `見積書.A1:見積書.F19 $'My sheet'.$A$1:.$B$2` → the ranges on `sheet`
pub(super) fn print_ranges(v: &str) -> Vec<(book::Pos, book::Pos)> {
    let mut out = Vec::new();
    // Ranges are separated by spaces, but a quoted sheet name may hold one
    let mut parts = Vec::new();
    let (mut cur, mut quoted) = (String::new(), false);
    for c in v.chars() {
        match c {
            '\'' => {
                quoted = !quoted;
                cur.push(c);
            }
            ' ' if !quoted => parts.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    parts.push(cur);
    for part in parts.iter().filter(|p| !p.is_empty()) {
        let a1 = super::formula::to_a1(&format!("of:=[{part}]")).unwrap_or_default();
        let cells = a1.rsplit('!').next().unwrap_or("").replace('$', "");
        let (a, b) = cells.split_once(':').unwrap_or((cells.as_str(), cells.as_str()));
        if let (Some(a), Some(b)) = (book::Pos::parse(a), book::Pos::parse(b)) {
            out.push((a, b));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shown_header_counts_toward_the_top_margin() {
        let xml = r#"<office:document-styles><office:automatic-styles>
<style:page-layout style:name="pm1"><style:page-layout-properties fo:page-width="29.7cm" fo:page-height="21cm" style:print-orientation="landscape" fo:margin-top="1cm" fo:margin-bottom="2cm" fo:margin-left="1.5cm" fo:margin-right="1.5cm" style:scale-to="80%" style:print="grid zero-values"/>
<style:header-style><style:header-footer-properties fo:min-height="1cm" fo:margin-bottom="0.25cm"/></style:header-style>
<style:footer-style><style:header-footer-properties fo:min-height="1cm" fo:margin-top="0.25cm"/></style:footer-style></style:page-layout>
</office:automatic-styles><office:master-styles>
<style:master-page style:name="P1" style:page-layout-name="pm1"><style:header><text:p>x</text:p></style:header><style:footer style:display="false"/></style:master-page>
</office:master-styles></office:document-styles>"#;
        let p = Pages::parse(xml, HfLook::default());
        let mut sh = Sheet::new("S");
        p.apply("P1", &mut sh);
        assert!(sh.landscape);
        assert_eq!(sh.paper_size, Some(9));
        let (l, r, t, b) = sh.margins_mm.unwrap();
        assert!((l - 15.0).abs() < 0.01 && (r - 15.0).abs() < 0.01);
        assert!((t - 20.0).abs() < 0.01, "{t}");
        assert!((b - 20.0).abs() < 0.01, "{b}");
        assert_eq!(sh.print_scale, Some(80));
        assert!(sh.print_gridlines);
    }

    #[test]
    fn print_ranges_are_read() {
        assert_eq!(
            print_ranges("見積書.A1:見積書.F19"),
            vec![(book::Pos::new(0, 0), book::Pos::new(18, 5))]
        );
        assert_eq!(print_ranges("$'My s'.$B$2:.$C$3").len(), 1);
    }
}
