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
use quick_xml::events::Event;
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
    /// master page name → header and footer as xlsx header text
    /// (`&L…&C…&R…` with `&A` for the sheet name, `&P` for the page)
    texts: HashMap<String, (Option<String>, Option<String>)>,
    layouts: HashMap<String, Page>,
}

impl Pages {
    pub(super) fn parse(styles_xml: &str) -> Pages {
        let mut p = Pages::default();
        let mut r = Reader::from_str(styles_xml);
        let mut layout: Option<String> = None;
        // in a header-style (true) or footer-style (false)
        let mut hf: Option<bool> = None;
        let mut master: Option<(String, String, bool, bool)> = None;
        // Inside a shown header (true) or footer (false) of a master page:
        // the text of its regions so far
        let mut hf_text: Option<(bool, HfText)> = None;
        let mut texts: (Option<String>, Option<String>) = (None, None);
        loop {
            let ev = r.read_event();
            if let Some((_, t)) = hf_text.as_mut() {
                match &ev {
                    Ok(Event::Start(e)) | Ok(Event::Empty(e)) => t.element(e.name().as_ref(), matches!(ev, Ok(Event::Empty(_)))),
                    Ok(Event::Text(x)) => t.text(&x.unescape().unwrap_or_default()),
                    Ok(Event::End(e)) if matches!(e.name().as_ref(), b"style:header" | b"style:footer") => {
                        if let Some((is_header, t)) = hf_text.take() {
                            let code = t.code();
                            if is_header {
                                texts.0 = Some(code);
                            } else {
                                texts.1 = Some(code);
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
                                hf_text = Some((is_header, HfText::default()));
                            }
                        }
                    }
                    _ => {}
                },
                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"style:page-layout" => layout = None,
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
        if let Some((h, f)) = self.texts.get(master) {
            sh.header = h.clone().filter(|_| *header).filter(|t| !t.is_empty());
            sh.footer = f.clone().filter(|_| *footer).filter(|t| !t.is_empty());
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

/// The text of a header or footer as it is read: one string per region
/// (`L`, `C`, `R`); text outside regions counts as the centre
#[derive(Default)]
struct HfText {
    regions: Vec<(char, String)>,
    cur: Option<char>,
    paras: usize,
    in_field: bool,
}

impl HfText {
    fn slot(&mut self) -> &mut String {
        let k = self.cur.unwrap_or('C');
        if let Some(i) = self.regions.iter().position(|(c, _)| *c == k) {
            return &mut self.regions[i].1;
        }
        self.regions.push((k, String::new()));
        &mut self.regions.last_mut().unwrap().1
    }
    fn element(&mut self, name: &[u8], empty: bool) {
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
                    self.slot().push('\n');
                }
                self.paras += 1;
                let _ = empty;
            }
            b"text:sheet-name" => self.slot().push_str("&A"),
            b"text:page-number" => self.slot().push_str("&P"),
            b"text:page-count" => self.slot().push_str("&N"),
            b"text:date" => self.slot().push_str("&D"),
            b"text:time" => self.slot().push_str("&T"),
            b"text:title" | b"text:file-name" => self.slot().push_str("&F"),
            b"text:s" => self.slot().push(' '),
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
        if matches!(name, b"style:region-left" | b"style:region-center" | b"style:region-right") {
            self.cur = None;
        }
    }
    fn text(&mut self, t: &str) {
        if !self.in_field {
            let t = t.replace('&', "&&");
            self.slot().push_str(&t);
        }
    }
    /// As xlsx header text: `&L…&C…&R…`, empty regions left out
    fn code(&self) -> String {
        let mut out = String::new();
        for k in ['L', 'C', 'R'] {
            if let Some((_, t)) = self.regions.iter().find(|(c, _)| *c == k) {
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
        let p = Pages::parse(xml);
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
