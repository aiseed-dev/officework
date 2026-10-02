//! Cell styles of an ods as the engine's [`CellFormat`].
//!
//! A cell names an automatic style in content.xml (`ce4`), which names a
//! parent among the named styles of styles.xml (`Default`), and everything
//! sits on the default cell style (`style:default-style`). Properties are
//! taken from the default first, then down the parents, then the cell's own
//! style, so the nearest one wins.
//!
//! Border widths are the ones LibreOffice 24.2 writes for Excel's line
//! weights, confirmed on the xlsx corpus converted to ods (2026-10-02):
//! hair 0.06pt, thin 0.74pt, medium 1.76pt, thick 2.49pt, double as
//! `double-thin`. The limits between them are the midpoints.

use std::collections::HashMap;

use book::{BStyle, Borders, CellFormat, Edge, HAlign, VAlign};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::read::attr;

/// The properties one style sets. None = not set here, look further up
#[derive(Debug, Clone, Default)]
struct Props {
    parent: Option<String>,
    data_style: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strike: Option<bool>,
    subscript: Option<bool>,
    font: Option<String>,
    font_asian: Option<String>,
    size_c: Option<u32>,
    color: Option<Option<String>>,
    fill: Option<Option<String>>,
    top: Option<Edge>,
    bottom: Option<Edge>,
    left: Option<Edge>,
    right: Option<Edge>,
    diag_down: Option<Edge>,
    diag_up: Option<Edge>,
    align: Option<HAlign>,
    valign: Option<VAlign>,
    wrap: Option<bool>,
    shrink: Option<bool>,
    rotation: Option<Option<i32>>,
}

/// The cell styles of both parts and the fonts they name
#[derive(Default)]
pub(super) struct CellStyles {
    default: Props,
    styles: HashMap<String, Props>,
    /// font-face name → family as it should be looked up
    faces: HashMap<String, String>,
    /// data style name → format code
    codes: HashMap<String, String>,
    cache: std::cell::RefCell<HashMap<String, CellFormat>>,
}

impl CellStyles {
    /// Read styles.xml first, then content.xml: an automatic style may name
    /// a parent from styles.xml, never the other way round
    pub(super) fn parse(styles_xml: &str, content_xml: &str) -> CellStyles {
        let mut cs = CellStyles::default();
        for xml in [styles_xml, content_xml] {
            cs.read_part(xml);
            cs.codes.extend(super::numfmt::parse_data_styles(xml));
        }
        cs
    }

    fn read_part(&mut self, xml: &str) {
        let mut r = Reader::from_str(xml);
        // (name, is the default style)
        let mut cur: Option<(String, bool)> = None;
        let mut props = Props::default();
        loop {
            let ev = r.read_event();
            match ev {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let empty = matches!(ev, Ok(Event::Empty(_)));
                    match e.name().as_ref() {
                        b"style:font-face" => {
                            if let (Some(n), Some(f)) = (attr(e, "style:name"), attr(e, "svg:font-family")) {
                                self.faces.insert(n, f.trim_matches(|c| c == '\'' || c == '"').to_string());
                            }
                        }
                        b"style:default-style" if attr(e, "style:family").as_deref() == Some("table-cell") => {
                            cur = Some((String::new(), true));
                            props = Props::default();
                            if empty {
                                cur = None;
                            }
                        }
                        b"style:style" if attr(e, "style:family").as_deref() == Some("table-cell") => {
                            props = Props {
                                parent: attr(e, "style:parent-style-name"),
                                data_style: attr(e, "style:data-style-name"),
                                ..Props::default()
                            };
                            let name = attr(e, "style:name").unwrap_or_default();
                            if empty {
                                self.styles.insert(name, std::mem::take(&mut props));
                            } else {
                                cur = Some((name, false));
                            }
                        }
                        b"style:table-cell-properties" if cur.is_some() => cell_props(e, &mut props),
                        b"style:paragraph-properties" if cur.is_some() => para_props(e, &mut props),
                        b"style:text-properties" if cur.is_some() => text_props(e, &mut props),
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    if matches!(e.name().as_ref(), b"style:style" | b"style:default-style") {
                        if let Some((name, is_default)) = cur.take() {
                            if is_default {
                                self.default = std::mem::take(&mut props);
                            } else {
                                self.styles.insert(name, std::mem::take(&mut props));
                            }
                        }
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }

    /// The format of a cell style, the default and the parents applied
    /// The font and size of the workbook's default are left unset, as the
    /// xlsx reader leaves them for cells in the default font: LibreOffice
    /// writes them into every automatic style
    pub(super) fn format(&self, name: &str) -> CellFormat {
        if let Some(f) = self.cache.borrow().get(name) {
            return f.clone();
        }
        let mut f = self.raw_format(name);
        if let Some((font, size)) = self.default_font() {
            if f.font.as_deref() == Some(font.as_str()) {
                f.font = None;
            }
            if f.size_c == Some((size * 100.0).round() as u32) {
                f.size_c = None;
            }
        }
        self.cache.borrow_mut().insert(name.to_string(), f.clone());
        f
    }

    fn raw_format(&self, name: &str) -> CellFormat {
        let mut chain = Vec::new();
        let mut at = Some(name.to_string());
        while let Some(n) = at {
            if chain.len() > 16 || chain.contains(&n) {
                break;
            }
            let Some(p) = self.styles.get(&n) else { break };
            at = p.parent.clone();
            chain.push(n);
        }
        let mut layers: Vec<&Props> = vec![&self.default];
        layers.extend(chain.iter().rev().filter_map(|n| self.styles.get(n)));
        self.flatten(&layers)
    }

    /// The default cell style's font, for the workbook's default font
    pub(super) fn default_font(&self) -> Option<(String, f32)> {
        let f = self.raw_format("Default");
        Some((f.font?, f.size_c? as f32 / 100.0))
    }

    fn flatten(&self, layers: &[&Props]) -> CellFormat {
        let mut f = CellFormat::default();
        let mut b = Borders::default();
        let (mut font, mut font_asian) = (None, None);
        for p in layers {
            macro_rules! take {
                ($field:ident, $dst:expr) => {
                    if let Some(v) = p.$field.clone() {
                        $dst = v;
                    }
                };
            }
            take!(bold, f.bold);
            take!(italic, f.italic);
            take!(underline, f.underline);
            take!(strike, f.strike);
            take!(subscript, f.subscript);
            take!(color, f.color);
            take!(fill, f.fill);
            take!(top, b.top);
            take!(bottom, b.bottom);
            take!(left, b.left);
            take!(right, b.right);
            take!(align, f.align);
            take!(valign, f.valign);
            take!(wrap, f.wrap);
            take!(shrink, f.shrink);
            take!(rotation, f.rotation);
            if let Some(s) = p.size_c {
                f.size_c = Some(s);
            }
            if let Some(n) = &p.font {
                font = Some(n.clone());
            }
            if let Some(n) = &p.font_asian {
                font_asian = Some(n.clone());
            }
            if let Some(e) = p.diag_down {
                b.diag = e;
                b.diag_down = e.on;
            }
            if let Some(e) = p.diag_up {
                if e.on {
                    b.diag = e;
                }
                b.diag_up = e.on;
            }
            if let Some(d) = &p.data_style {
                f.number_format = self.codes.get(d).cloned().filter(|c| c != "General");
            }
        }
        f.borders = b;
        // The western font name is the one an xlsx carries; the asian one is
        // used only when the style names no other
        let face = font.or(font_asian);
        f.font = face.map(|n| self.faces.get(&n).cloned().unwrap_or(n));
        f
    }
}

fn cell_props(e: &BytesStart, p: &mut Props) {
    if let Some(c) = attr(e, "fo:background-color") {
        p.fill = Some(color(&c));
    }
    if let Some(v) = attr(e, "fo:border") {
        let edge = border(&v);
        p.top = Some(edge);
        p.bottom = Some(edge);
        p.left = Some(edge);
        p.right = Some(edge);
    }
    for (key, slot) in [
        ("fo:border-top", &mut p.top),
        ("fo:border-bottom", &mut p.bottom),
        ("fo:border-left", &mut p.left),
        ("fo:border-right", &mut p.right),
        ("style:diagonal-tl-br", &mut p.diag_down),
        ("style:diagonal-bl-tr", &mut p.diag_up),
    ] {
        if let Some(v) = attr(e, key) {
            *slot = Some(border(&v));
        }
    }
    if let Some(v) = attr(e, "style:vertical-align") {
        p.valign = Some(match v.as_str() {
            "top" => VAlign::Top,
            "middle" => VAlign::Middle,
            _ => VAlign::Bottom,
        });
    }
    // LibreOffice keeps Excel's distributed vertical alignment in its own
    // attribute next to `style:vertical-align` (seen on the corpus)
    if attr(e, "loext:vertical-justify").as_deref() == Some("distribute") {
        p.valign = Some(VAlign::Distribute);
    }
    if let Some(v) = attr(e, "fo:wrap-option") {
        p.wrap = Some(v == "wrap");
    }
    if let Some(v) = attr(e, "style:shrink-to-fit") {
        p.shrink = Some(v == "true");
    }
    if let Some(v) = attr(e, "style:rotation-angle") {
        // ODF counts counterclockwise from 0 to 360; the engine keeps
        // Excel's -90..90 (positive = counterclockwise)
        let deg = v.trim_end_matches("deg").parse::<f32>().ok().map(|d| d.round() as i32 % 360);
        p.rotation = Some(match deg {
            Some(0) | None => None,
            Some(d) if d <= 90 => Some(d),
            Some(d) if d >= 270 => Some(d - 360),
            Some(d) => Some(d - 180),
        });
    }
}

fn para_props(e: &BytesStart, p: &mut Props) {
    if let Some(v) = attr(e, "fo:text-align") {
        p.align = Some(match v.as_str() {
            "center" => HAlign::Center,
            "end" | "right" => HAlign::Right,
            // `css3t:text-justify="distribute"` turns justify into Excel's
            // distributed alignment (seen on the corpus)
            "justify" if attr(e, "css3t:text-justify").as_deref() == Some("distribute") => HAlign::Distribute,
            "justify" => HAlign::Justify,
            "start" | "left" => HAlign::Left,
            _ => HAlign::General,
        });
    }
}

fn text_props(e: &BytesStart, p: &mut Props) {
    if let Some(v) = attr(e, "fo:font-weight") {
        p.bold = Some(v == "bold" || v.parse::<u32>().is_ok_and(|w| w >= 600));
    }
    if let Some(v) = attr(e, "fo:font-style") {
        p.italic = Some(v == "italic" || v == "oblique");
    }
    if let Some(v) = attr(e, "style:text-underline-style") {
        p.underline = Some(v != "none");
    }
    if let Some(v) = attr(e, "style:text-line-through-style") {
        p.strike = Some(v != "none");
    }
    if let Some(v) = attr(e, "style:text-position") {
        p.subscript = Some(v.starts_with("sub") || v.starts_with('-'));
    }
    if let Some(v) = attr(e, "style:font-name") {
        p.font = Some(v);
    }
    if let Some(v) = attr(e, "style:font-name-asian") {
        p.font_asian = Some(v);
    }
    if let Some(v) = attr(e, "fo:font-size").and_then(|s| super::read::length_mm(&s)) {
        p.size_c = Some((v / 25.4 * 72.0 * 100.0).round() as u32);
    }
    if let Some(v) = attr(e, "fo:color") {
        p.color = Some(color(&v));
    }
    if attr(e, "style:use-window-font-color").as_deref() == Some("true") {
        p.color = Some(None);
    }
}

/// `#dce6f1` → `DCE6F1`; `transparent` → no colour
fn color(v: &str) -> Option<String> {
    let h = v.trim().strip_prefix('#')?;
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_ascii_uppercase())
}

/// `0.74pt solid #000000` → a thin black edge; `none` → no edge
fn border(v: &str) -> Edge {
    let v = v.trim();
    if v.is_empty() || v == "none" || v == "hidden" {
        return Edge::OFF;
    }
    let mut width_pt = 0.74f32;
    let mut line = "solid";
    let mut col = None;
    for part in v.split_whitespace() {
        if part.starts_with('#') {
            // Black is what LibreOffice writes for Excel's automatic colour
            col = color(part).and_then(|h| u32::from_str_radix(&h, 16).ok()).filter(|c| *c != 0);
        } else if let Some(mm) = super::read::length_mm(part) {
            width_pt = mm / 25.4 * 72.0;
        } else {
            line = match part {
                "thin" => {
                    width_pt = 0.74;
                    line
                }
                "medium" => {
                    width_pt = 1.76;
                    line
                }
                "thick" => {
                    width_pt = 2.49;
                    line
                }
                other => other,
            };
        }
    }
    if line == "none" || line == "hidden" {
        return Edge::OFF;
    }
    let heavy = width_pt > 1.25;
    let style = match line {
        "double" | "double-thin" | "thin-thick" | "thick-thin" => BStyle::Double,
        "dashed" | "fine-dashed" | "dash" if heavy => BStyle::MediumDashed,
        "dashed" | "fine-dashed" | "dash" => BStyle::Dashed,
        "dotted" | "dot" => BStyle::Dotted,
        "dot-dash" | "dash-dot" if heavy => BStyle::MediumDashDot,
        "dot-dash" | "dash-dot" => BStyle::DashDot,
        "dot-dot-dash" | "dash-dot-dot" if heavy => BStyle::MediumDashDotDot,
        "dot-dot-dash" | "dash-dot-dot" => BStyle::DashDotDot,
        _ if width_pt < 0.4 => BStyle::Hair,
        _ if width_pt <= 1.25 => BStyle::Thin,
        _ if width_pt <= 2.1 => BStyle::Medium,
        _ => BStyle::Thick,
    };
    Edge::line(style, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_widths_libreoffice_writes_map_back_to_excel_weights() {
        assert_eq!(border("0.06pt solid #000000").style, BStyle::Hair);
        assert_eq!(border("0.74pt solid #000000").style, BStyle::Thin);
        assert_eq!(border("1.76pt solid #000000").style, BStyle::Medium);
        assert_eq!(border("2.49pt solid #000000").style, BStyle::Thick);
        assert_eq!(border("1.76pt double-thin #000000").style, BStyle::Double);
        assert_eq!(border("0.74pt solid #ff0000").color, Some(0xFF0000));
        assert!(!border("none").on);
    }

    #[test]
    fn styles_inherit_from_the_default_and_their_parents() {
        let styles = r#"<office:document-styles><office:font-face-decls><style:font-face style:name="Mincho" svg:font-family="'ＭＳ 明朝'"/></office:font-face-decls><office:styles>
<style:default-style style:family="table-cell"><style:text-properties style:font-name="Mincho" fo:font-size="10.5pt"/></style:default-style>
<style:style style:name="Heading" style:family="table-cell"><style:text-properties fo:font-weight="bold" fo:font-size="14pt"/></style:style>
</office:styles></office:document-styles>"#;
        let content = r##"<office:document-content><office:automatic-styles>
<style:style style:name="ce1" style:family="table-cell" style:parent-style-name="Heading"><style:table-cell-properties fo:background-color="#dce6f1" fo:border="0.74pt solid #000000" style:vertical-align="middle" fo:wrap-option="wrap"/><style:paragraph-properties fo:text-align="center"/></style:style>
</office:automatic-styles></office:document-content>"##;
        let cs = CellStyles::parse(styles, content);
        let f = cs.format("ce1");
        assert!(f.bold);
        assert_eq!(f.size_c, Some(1400));
        // The default font is not repeated on each cell
        assert_eq!(cs.format("ce1").font, None);
        assert_eq!(f.fill.as_deref(), Some("DCE6F1"));
        assert_eq!(f.borders.top.style, BStyle::Thin);
        assert!(f.borders.left.on);
        assert_eq!(f.align, HAlign::Center);
        assert_eq!(f.valign, VAlign::Middle);
        assert!(f.wrap);
        assert_eq!(cs.default_font(), Some(("ＭＳ 明朝".to_string(), 10.5)));
    }
}
