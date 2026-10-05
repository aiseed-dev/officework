//! Comments, pictures and shapes in an ods.
//!
//! In a LibreOffice ods these sit inside the cell they are anchored to
//! (`table:table-cell` holds an `office:annotation`, a `draw:frame` with a
//! `draw:image`, or a `draw:custom-shape`), with `svg:x` / `svg:y` measured
//! from that cell's corner. Ones anchored to the page sit in `table:shapes`
//! with positions from the sheet's corner. A shape's colours and lines are
//! in a graphic style (`gr1`), on top of the default graphic style.
//!
//! The model counts positions in pixels at 96 dpi, as the xlsx reader does
//! (EMU / 9525).

use std::collections::HashMap;
use std::fmt::Write as _;

use book::{CommentEntry, CommentThread, Pos, SheetImage, SheetShape};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::read::{attr, length_mm};

pub(super) fn mm_to_px(mm: f32) -> f32 {
    mm * 96.0 / 25.4
}

pub(super) fn px_to_mm(px: f32) -> f32 {
    px * 25.4 / 96.0
}

/// The look of a shape from its graphic style. None = not set here
#[derive(Debug, Clone, Default)]
struct GProps {
    parent: Option<String>,
    /// Some(None) = no fill
    fill: Option<Option<String>>,
    /// Some(None) = no line
    line: Option<Option<String>>,
    line_w_pt: Option<f32>,
    dash: Option<Option<String>>,
    shadow: Option<bool>,
    alpha: Option<f32>,
    anchor: Option<book::TextAnchor>,
    ins: Option<(f32, f32, f32, f32)>,
}

/// The look of a shape's text: from a paragraph style (`P2`) or a text
/// style (`T1`)
#[derive(Debug, Clone, Default)]
struct TProps {
    align: Option<book::HAlign>,
    size_pt: Option<f32>,
    font: Option<String>,
}

/// Graphic styles of both parts, by name
#[derive(Default)]
pub(super) struct GraphicStyles {
    default: GProps,
    styles: HashMap<String, GProps>,
    /// Paragraph and text styles, by name
    texts: HashMap<String, TProps>,
    /// font-face name → family
    faces: HashMap<String, String>,
}

impl GraphicStyles {
    pub(super) fn parse(styles_xml: &str, content_xml: &str) -> GraphicStyles {
        let mut g = GraphicStyles::default();
        for xml in [styles_xml, content_xml] {
            let mut r = Reader::from_str(xml);
            // (name, is the default style)
            let mut cur: Option<(String, bool)> = None;
            let mut props = GProps::default();
            // A paragraph or text style being read
            let mut tcur: Option<(String, TProps)> = None;
            loop {
                let ev = r.read_event();
                match ev {
                    Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                        let empty = matches!(ev, Ok(Event::Empty(_)));
                        match e.name().as_ref() {
                            b"style:default-style" if attr(e, "style:family").as_deref() == Some("graphic") => {
                                props = GProps::default();
                                cur = (!empty).then(|| (String::new(), true));
                            }
                            b"style:style" if attr(e, "style:family").as_deref() == Some("graphic") => {
                                props = GProps { parent: attr(e, "style:parent-style-name"), ..GProps::default() };
                                let name = attr(e, "style:name").unwrap_or_default();
                                if empty {
                                    g.styles.insert(name, std::mem::take(&mut props));
                                } else {
                                    cur = Some((name, false));
                                }
                            }
                            b"style:graphic-properties" if cur.is_some() => graphic_props(e, &mut props),
                            b"style:style" if matches!(attr(e, "style:family").as_deref(), Some("paragraph") | Some("text")) => {
                                let name = attr(e, "style:name").unwrap_or_default();
                                if !empty {
                                    tcur = Some((name, TProps::default()));
                                }
                            }
                            b"style:paragraph-properties" => {
                                if let Some((_, t)) = tcur.as_mut() {
                                    t.align = attr(e, "fo:text-align").map(|a| match a.as_str() {
                                        "center" => book::HAlign::Center,
                                        "end" | "right" => book::HAlign::Right,
                                        "justify" => book::HAlign::Justify,
                                        _ => book::HAlign::Left,
                                    });
                                }
                            }
                            b"style:text-properties" => {
                                if let Some((_, t)) = tcur.as_mut() {
                                    if let Some(v) = attr(e, "fo:font-size").and_then(|v| length_mm(&v)) {
                                        t.size_pt = Some(v / 25.4 * 72.0);
                                    }
                                    t.font = attr(e, "style:font-name-asian")
                                        .or_else(|| attr(e, "style:font-name"))
                                        .or_else(|| attr(e, "style:font-family-asian"))
                                        .or_else(|| attr(e, "fo:font-family"))
                                        .map(|v| v.trim_matches(|c| c == '\'' || c == '"').to_string())
                                        .or(t.font.take());
                                }
                            }
                            b"style:font-face" => {
                                if let (Some(n), Some(f)) = (attr(e, "style:name"), attr(e, "svg:font-family")) {
                                    g.faces.insert(n, f.trim_matches(|c| c == '\'' || c == '"').to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(Event::End(ref e)) if matches!(e.name().as_ref(), b"style:style" | b"style:default-style") => {
                        if let Some((name, t)) = tcur.take() {
                            g.texts.insert(name, t);
                        }
                        if let Some((name, is_default)) = cur.take() {
                            if is_default {
                                g.default = std::mem::take(&mut props);
                            } else {
                                g.styles.insert(name, std::mem::take(&mut props));
                            }
                        }
                    }
                    Ok(Event::Eof) | Err(_) => break,
                    _ => {}
                }
            }
        }
        g
    }

    /// The look of a style, the default and the parents applied
    fn resolve(&self, name: Option<&str>) -> GProps {
        let mut chain = Vec::new();
        let mut at = name.map(str::to_string);
        while let Some(n) = at {
            if chain.len() > 16 || chain.contains(&n) {
                break;
            }
            let Some(p) = self.styles.get(&n) else { break };
            at = p.parent.clone();
            chain.push(n);
        }
        let mut out = self.default.clone();
        for n in chain.iter().rev() {
            let p = &self.styles[n];
            macro_rules! take {
                ($f:ident) => {
                    if p.$f.is_some() {
                        out.$f = p.$f.clone();
                    }
                };
            }
            take!(fill);
            take!(line);
            take!(line_w_pt);
            take!(dash);
            take!(shadow);
            take!(alpha);
            take!(anchor);
            take!(ins);
        }
        out
    }
}

fn color(v: &str) -> Option<String> {
    let h = v.trim().strip_prefix('#')?;
    (h.len() == 6).then(|| h.to_ascii_uppercase())
}

fn graphic_props(e: &BytesStart, p: &mut GProps) {
    match attr(e, "draw:fill").as_deref() {
        Some("none") => p.fill = Some(None),
        Some(_) => p.fill = Some(attr(e, "draw:fill-color").and_then(|c| color(&c))),
        None => {
            if let Some(c) = attr(e, "draw:fill-color") {
                p.fill = Some(color(&c));
            }
        }
    }
    match attr(e, "draw:stroke").as_deref() {
        Some("none") => p.line = Some(None),
        Some(kind) => {
            p.line = Some(attr(e, "svg:stroke-color").and_then(|c| color(&c)).or(Some("000000".into())));
            p.dash = Some((kind == "dash").then(|| "dash".to_string()));
        }
        None => {
            if let Some(c) = attr(e, "svg:stroke-color") {
                p.line = Some(color(&c));
            }
        }
    }
    if let Some(w) = attr(e, "svg:stroke-width").and_then(|v| length_mm(&v)) {
        p.line_w_pt = Some(w / 25.4 * 72.0);
    }
    if let Some(s) = attr(e, "draw:shadow") {
        p.shadow = Some(s == "visible");
    }
    if let Some(o) = attr(e, "draw:opacity").and_then(|v| v.trim_end_matches('%').parse::<f32>().ok()) {
        p.alpha = Some(o / 100.0);
    }
    let pad = |k: &str| attr(e, k).and_then(|v| length_mm(&v));
    if let (Some(l), Some(r), Some(t), Some(b)) =
        (pad("fo:padding-left"), pad("fo:padding-right"), pad("fo:padding-top"), pad("fo:padding-bottom"))
    {
        p.ins = Some((l, r, t, b));
    }
    if let Some(v) = attr(e, "draw:textarea-vertical-align") {
        p.anchor = Some(match v.as_str() {
            "middle" => book::TextAnchor::Middle,
            "bottom" => book::TextAnchor::Bottom,
            _ => book::TextAnchor::Top,
        });
    }
}

/// One drawing object as it is being read, from its first element to its
/// end
#[derive(Debug, Default)]
pub(super) struct Capture {
    /// The element that started it (`draw:frame`, `office:annotation`, …)
    pub(super) kind: String,
    attrs: HashMap<String, String>,
    depth: usize,
    image: Option<String>,
    chart: bool,
    /// The sub-document of an embedded object (`./Object 1`)
    pub(super) object: Option<String>,
    /// Where the object starts in content.xml, to keep it as written
    pub(super) raw_start: usize,
    geometry: Option<String>,
    mirror_h: bool,
    mirror_v: bool,
    paras: Vec<String>,
    in_p: usize,
    /// The first paragraph's style and the first text style in it
    p_style: Option<String>,
    t_style: Option<String>,
    in_geometry: bool,
    who: String,
    when: String,
    in_meta: Option<&'static str>,
}

/// What a finished capture becomes
pub(super) enum Drawn {
    Comment(CommentThread),
    Image(SheetImage),
    Shape(Box<SheetShape>),
    /// Something counted in the report and not kept (charts, OLE objects)
    Other(String),
}

impl Capture {
    /// Begin with the element `e`. Returns None for elements that do not
    /// start an object (groups pass through: their children are read one
    /// by one)
    pub(super) fn start(e: &BytesStart) -> Option<Capture> {
        let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
        // Groups and links hold objects; their children are read one by one
        if name != "office:annotation" && (!name.starts_with("draw:") || matches!(name.as_str(), "draw:g" | "draw:a")) {
            return None;
        }
        let attrs = e
            .attributes()
            .flatten()
            .map(|a| {
                (
                    String::from_utf8_lossy(a.key.as_ref()).to_string(),
                    a.unescape_value().map(|v| v.to_string()).unwrap_or_default(),
                )
            })
            .collect();
        Some(Capture { kind: name, attrs, depth: 1, ..Capture::default() })
    }

    /// Feed one event from inside the object. True when the object ended
    pub(super) fn feed(&mut self, ev: &Event) -> bool {
        match ev {
            Event::Start(e) | Event::Empty(e) => {
                let empty = matches!(ev, Event::Empty(_));
                if !empty {
                    self.depth += 1;
                }
                match e.name().as_ref() {
                    b"draw:image" => {
                        if self.image.is_none() {
                            self.image = attr(e, "xlink:href");
                        }
                    }
                    b"draw:object" | b"draw:object-ole" => {
                        self.chart = true;
                        if self.object.is_none() {
                            self.object = attr(e, "xlink:href");
                        }
                    }
                    b"draw:enhanced-geometry" => {
                        self.geometry = attr(e, "draw:type");
                        self.mirror_h = attr(e, "draw:mirror-horizontal").as_deref() == Some("true");
                        self.mirror_v = attr(e, "draw:mirror-vertical").as_deref() == Some("true");
                        if !empty {
                            self.in_geometry = true;
                        }
                    }
                    b"text:p" | b"text:h" if !self.in_geometry => {
                        if self.paras.is_empty() {
                            self.p_style = attr(e, "text:style-name");
                        }
                        self.paras.push(String::new());
                        if !empty {
                            self.in_p += 1;
                        }
                    }
                    b"text:span" if self.in_p > 0 && self.t_style.is_none() => self.t_style = attr(e, "text:style-name"),
                    b"text:s" if self.in_p > 0 => {
                        let n = attr(e, "text:c").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
                        if let Some(p) = self.paras.last_mut() {
                            p.extend(std::iter::repeat_n(' ', n));
                        }
                    }
                    b"text:line-break" if self.in_p > 0 => {
                        if let Some(p) = self.paras.last_mut() {
                            p.push('\n');
                        }
                    }
                    b"text:tab" if self.in_p > 0 => {
                        if let Some(p) = self.paras.last_mut() {
                            p.push('\t');
                        }
                    }
                    b"dc:creator" if !empty => self.in_meta = Some("who"),
                    b"dc:date" if !empty => self.in_meta = Some("when"),
                    _ => {}
                }
                false
            }
            Event::Text(t) => {
                let t = t.unescape().unwrap_or_default();
                match self.in_meta {
                    Some("who") => self.who.push_str(&t),
                    Some(_) => self.when.push_str(&t),
                    None => {
                        if self.in_p > 0 {
                            if let Some(p) = self.paras.last_mut() {
                                p.push_str(&t);
                            }
                        }
                    }
                }
                false
            }
            Event::End(e) => {
                match e.name().as_ref() {
                    b"text:p" | b"text:h" if self.in_p > 0 => self.in_p -= 1,
                    b"draw:enhanced-geometry" => self.in_geometry = false,
                    b"dc:creator" | b"dc:date" => self.in_meta = None,
                    _ => {}
                }
                self.depth = self.depth.saturating_sub(1);
                self.depth == 0
            }
            _ => false,
        }
    }

    fn len(&self, key: &str) -> f32 {
        self.attrs.get(key).and_then(|v| length_mm(v)).unwrap_or(0.0)
    }

    /// The package paths an embedded object uses: its folder and the
    /// picture standing in for it, without a leading `./`
    pub(super) fn object_paths(&self) -> (Option<String>, Option<String>) {
        let clean = |h: &String| h.trim_start_matches("./").to_string();
        (self.object.as_ref().map(clean), self.image.as_ref().map(clean))
    }

    /// Offset and size in mm: (x, y, width, height)
    pub(super) fn rect_mm(&self) -> (f32, f32, f32, f32) {
        if self.kind == "draw:line" {
            let (x1, y1, x2, y2) = (self.len("svg:x1"), self.len("svg:y1"), self.len("svg:x2"), self.len("svg:y2"));
            return (x1.min(x2), y1.min(y2), (x2 - x1).abs(), (y2 - y1).abs());
        }
        (self.len("svg:x"), self.len("svg:y"), self.len("svg:width"), self.len("svg:height"))
    }

    /// The cell at the far corner when the object resizes with the cells
    /// (`table:end-cell-address`, with `table:end-x` / `table:end-y`)
    pub(super) fn end(&self) -> Option<(Pos, f32, f32)> {
        let a = self.attrs.get("table:end-cell-address")?;
        let a1 = super::formula::to_a1(&format!("of:=[{a}]"))?;
        let cell = a1.rsplit('!').next()?.replace('$', "");
        let p = Pos::parse(&cell)?;
        Some((p, mm_to_px(self.len("table:end-x")), mm_to_px(self.len("table:end-y"))))
    }

    /// Turn the object into the model's piece, anchored at `at` with its
    /// offset already in mm from that cell's corner
    /// `offset` replaces the object's own `svg:x` / `svg:y` (mm), for an
    /// object anchored to the page and placed on a cell afterwards
    pub(super) fn finish(
        self,
        at: Pos,
        offset: Option<(f32, f32)>,
        pictures: &HashMap<String, Vec<u8>>,
        gs: &GraphicStyles,
        z: u32,
    ) -> Drawn {
        let (x, y, w, h) = self.rect_mm();
        let (x, y) = offset.unwrap_or((x, y));
        if self.kind == "office:annotation" {
            let text = self.paras.join("\n");
            let entry = CommentEntry { who: self.who.trim().to_string(), when: self.when.trim().to_string(), text };
            return Drawn::Comment(CommentThread { done: false, entries: vec![entry] });
        }
        if self.chart {
            return Drawn::Other("chart".into());
        }
        if self.kind == "draw:frame" {
            return match self.image.as_deref().map(|h| h.trim_start_matches("./")).and_then(|h| pictures.get(h)) {
                Some(data) => Drawn::Image(SheetImage {
                    at,
                    dx_px: mm_to_px(x),
                    dy_px: mm_to_px(y),
                    width_px: mm_to_px(w),
                    height_px: mm_to_px(h),
                    data: data.clone(),
                    z,
                }),
                None => Drawn::Other("draw:frame".into()),
            };
        }
        let kind = match (self.kind.as_str(), self.geometry.as_deref()) {
            ("draw:line", _) => "line",
            ("draw:rect", _) => "rect",
            ("draw:ellipse", _) => "ellipse",
            ("draw:custom-shape", Some(g)) => match g.trim_start_matches("ooxml-") {
                "rectangle" | "rect" => "rect",
                "round-rectangle" | "roundRect" => "roundRect",
                "ellipse" => "ellipse",
                "right-arrow" | "rightArrow" => "rightArrow",
                "diamond" => "diamond",
                "line" => "line",
                other => return Drawn::Other(format!("draw:custom-shape {other}")),
            },
            (other, _) => return Drawn::Other(other.to_string()),
        };
        let g = gs.resolve(self.attrs.get("draw:style-name").map(String::as_str));
        let text = Some(self.paras.join("\n")).filter(|t| !t.trim().is_empty());
        let mut s = SheetShape {
            at,
            width_px: mm_to_px(w),
            height_px: mm_to_px(h),
            kind: kind.to_string(),
            fill: g.fill.clone().flatten(),
            line: g.line.clone().flatten(),
            text,
            dx_px: mm_to_px(x),
            dy_px: mm_to_px(y),
            flip_h: self.mirror_h,
            flip_v: self.mirror_v,
            dash: g.dash.clone().flatten(),
            shadow: g.shadow.unwrap_or(false),
            alpha: g.alpha.unwrap_or(1.0),
            to: self.end(),
            name: self.attrs.get("draw:name").cloned(),
            z,
            ..SheetShape::default()
        };
        let tp = |n: &Option<String>| n.as_ref().and_then(|n| gs.texts.get(n)).cloned().unwrap_or_default();
        let (pp, tt) = (tp(&self.p_style), tp(&self.t_style));
        s.text_fmt.align = pp.align.unwrap_or_default();
        s.text_fmt.anchor = g.anchor.unwrap_or_default();
        if let Some(ins) = g.ins {
            s.text_fmt.ins_mm = ins;
        }
        s.text_fmt.size_pt = tt.size_pt.or(pp.size_pt);
        s.text_fmt.font = tt.font.or(pp.font).map(|n| gs.faces.get(&n).cloned().unwrap_or(n));
        if let Some(w) = g.line_w_pt {
            // A hairline (0) is drawn one pixel wide
            s.line_w = w.max(0.75);
        }
        if kind == "line" {
            // A line falling to the left is a mirrored box
            let (x1, y1, x2, y2) = (self.len("svg:x1"), self.len("svg:y1"), self.len("svg:x2"), self.len("svg:y2"));
            s.flip_h = (x2 < x1) != (y2 < y1);
        }
        Drawn::Shape(Box::new(s))
    }
}

/// Which cell holds the point `(x, y)` mm from the sheet's corner, and the
/// offset from that cell's corner, for objects anchored to the page
pub(super) fn cell_at(sh: &book::Sheet, basis: &book::ColBasis, x: f32, y: f32) -> (Pos, f32, f32) {
    let mut c = 0u32;
    let mut left = 0.0f32;
    while c < 16383 {
        let w = sh.col_haba_mm(c, basis);
        if left + w > x {
            break;
        }
        left += w;
        c += 1;
    }
    let mut r = 0u32;
    let mut top = 0.0f32;
    while r < 1_048_575 {
        let h = sh.row_height.get(&r).copied().or(sh.default_row_height).unwrap_or(book::DEFAULT_ROW_PT) * 25.4 / 72.0;
        if top + h > y {
            break;
        }
        top += h;
        r += 1;
    }
    (Pos::new(r, c), x - left, y - top)
}

/// A comment as an `office:annotation`. A thread with replies becomes one
/// note: ODF keeps one author and one date per note
pub(super) fn comment_xml(t: &CommentThread) -> String {
    let first = t.entries.first();
    let mut s = String::from("<office:annotation>");
    if let Some(e) = first {
        if !e.who.is_empty() {
            let _ = write!(s, "<dc:creator>{}</dc:creator>", super::write::esc(&e.who));
        }
        if !e.when.is_empty() {
            let _ = write!(s, "<dc:date>{}</dc:date>", super::write::esc(&e.when));
        }
    }
    let mut lines: Vec<String> = Vec::new();
    for (i, e) in t.entries.iter().enumerate() {
        for (k, line) in e.text.split('\n').enumerate() {
            if i > 0 && k == 0 && !e.who.is_empty() {
                lines.push(format!("{}: {line}", e.who));
            } else {
                lines.push(line.to_string());
            }
        }
    }
    for l in lines {
        let _ = write!(s, "<text:p>{}</text:p>", super::write::esc(&l));
    }
    s.push_str("</office:annotation>");
    s
}

/// A picture's file type by its first bytes: (extension, media type)
pub(super) fn picture_type(data: &[u8]) -> (&'static str, &'static str) {
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        ("png", "image/png")
    } else if data.starts_with(&[0xFF, 0xD8]) {
        ("jpg", "image/jpeg")
    } else if data.starts_with(b"GIF8") {
        ("gif", "image/gif")
    } else if data.starts_with(b"BM") {
        ("bmp", "image/bmp")
    } else if data.windows(4).take(256).any(|w| w == b"<svg") {
        ("svg", "image/svg+xml")
    } else {
        ("png", "image/png")
    }
}

/// The ODF shape type for the model's kind, None for kinds this does not
/// write yet
pub(super) fn shape_type(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "rect" => "rectangle",
        "roundRect" => "round-rectangle",
        "ellipse" => "ellipse",
        "rightArrow" => "right-arrow",
        "diamond" => "diamond",
        "line" => "line",
        _ => return None,
    })
}

/// The graphic properties of a shape, the inside of `style:graphic-properties`
pub(super) fn shape_graphic(s: &SheetShape) -> String {
    let mut a = String::new();
    match &s.fill {
        Some(c) => {
            let _ = write!(a, r##"draw:fill="solid" draw:fill-color="#{}""##, c.to_ascii_lowercase());
        }
        None => a.push_str(r#"draw:fill="none""#),
    }
    match &s.line {
        Some(c) => {
            let kind = if s.dash.is_some() { "dash" } else { "solid" };
            let _ = write!(
                a,
                r##" draw:stroke="{kind}" svg:stroke-color="#{}" svg:stroke-width="{:.4}cm""##,
                c.to_ascii_lowercase(),
                s.line_w * 2.54 / 72.0
            );
        }
        None => a.push_str(r#" draw:stroke="none""#),
    }
    if s.shadow {
        a.push_str(r#" draw:shadow="visible" draw:shadow-offset-x="0.1cm" draw:shadow-offset-y="0.1cm""#);
    }
    if s.alpha < 1.0 {
        let _ = write!(a, r#" draw:opacity="{}%""#, (s.alpha * 100.0).round());
    }
    // The text box spans the shape and the paragraph style aligns the
    // lines, as LibreOffice's xlsx import does
    let v = match s.text_fmt.anchor {
        book::TextAnchor::Top => "top",
        book::TextAnchor::Middle => "middle",
        book::TextAnchor::Bottom => "bottom",
    };
    let _ = write!(a, r#" draw:textarea-horizontal-align="justify" draw:textarea-vertical-align="{v}""#);
    // The text wraps inside the box, with the box's inner margins
    let (l, r, t, b) = s.text_fmt.ins_mm;
    let _ = write!(
        a,
        r#" fo:wrap-option="wrap" draw:auto-grow-height="false" draw:auto-grow-width="false" fo:padding-left="{}" fo:padding-right="{}" fo:padding-top="{}" fo:padding-bottom="{}""#,
        cm(l),
        cm(r),
        cm(t),
        cm(b)
    );
    a
}

/// The paragraph style of a shape's text, the inside of a paragraph style
pub(super) fn shape_paragraph(s: &SheetShape) -> String {
    let al = match s.text_fmt.align {
        book::HAlign::Center | book::HAlign::CenterContinuous => "center",
        book::HAlign::Right => "end",
        book::HAlign::Justify | book::HAlign::Distribute => "justify",
        _ => "start",
    };
    let mut x = format!(r#"<style:paragraph-properties fo:text-align="{al}"/>"#);
    let mut tp = String::new();
    if let Some(pt) = s.text_fmt.size_pt {
        let pt = (pt * 100.0).round() / 100.0;
        let _ = write!(tp, r#" fo:font-size="{pt}pt" style:font-size-asian="{pt}pt""#);
    }
    if let Some(f) = &s.text_fmt.font {
        let f = super::write::esc(f);
        let _ = write!(tp, r#" style:font-name="{f}" style:font-name-asian="{f}""#);
    }
    if !tp.is_empty() {
        let _ = write!(x, "<style:text-properties{tp}/>");
    }
    x
}

/// Lengths in cm, as LibreOffice writes them
pub(super) fn cm(mm: f32) -> String {
    let v = format!("{:.4}", mm / 10.0);
    format!("{}cm", v.trim_end_matches('0').trim_end_matches('.'))
}

/// `table:end-cell-address` and its offsets, for an object that resizes
/// with the cells
pub(super) fn end_attrs(sheet: &str, to: Option<(Pos, f32, f32)>) -> String {
    match to {
        Some((p, ex, ey)) => format!(
            r#" table:end-cell-address="{}.{}" table:end-x="{}" table:end-y="{}""#,
            super::write::esc(&format!("${}", super::write::quote_sheet(sheet))),
            p.a1(),
            cm(px_to_mm(ex)),
            cm(px_to_mm(ey))
        ),
        None => String::new(),
    }
}

/// A shape as ODF, with its graphic style already named. None for shapes
/// this does not write yet
pub(super) fn shape_xml(s: &SheetShape, sheet: &str, style: &str, para: &str) -> Option<String> {
    let ty = shape_type(&s.kind)?;
    let (x, y, w, h) = (px_to_mm(s.dx_px), px_to_mm(s.dy_px), px_to_mm(s.width_px), px_to_mm(s.height_px));
    let name = s.name.as_ref().map(|n| format!(r#" draw:name="{}""#, super::write::esc(n))).unwrap_or_default();
    let end = end_attrs(sheet, s.to);
    let z = s.z;
    if ty == "line" {
        let (x1, x2) = if s.flip_h { (x + w, x) } else { (x, x + w) };
        let (y1, y2) = if s.flip_v { (y + h, y) } else { (y, y + h) };
        return Some(format!(
            r#"<draw:line draw:z-index="{z}"{name} draw:style-name="{style}" svg:x1="{}" svg:y1="{}" svg:x2="{}" svg:y2="{}"{end}><text:p/></draw:line>"#,
            cm(x1),
            cm(y1),
            cm(x2),
            cm(y2)
        ));
    }
    let text: String = match &s.text {
        Some(t) => t.split('\n').map(|l| format!(r#"<text:p text:style-name="{para}">{}</text:p>"#, super::write::esc(l))).collect(),
        None => String::new(),
    };
    let mirror = format!(
        "{}{}",
        if s.flip_h { r#" draw:mirror-horizontal="true""# } else { "" },
        if s.flip_v { r#" draw:mirror-vertical="true""# } else { "" }
    );
    Some(format!(
        r#"<draw:custom-shape draw:z-index="{z}"{name} draw:style-name="{style}" svg:width="{}" svg:height="{}" svg:x="{}" svg:y="{}"{end}>{text}<draw:enhanced-geometry svg:viewBox="0 0 21600 21600" draw:type="{ty}"{mirror}/></draw:custom-shape>"#,
        cm(w),
        cm(h),
        cm(x),
        cm(y)
    ))
}

/// A picture as an ODF frame, its file already named in the package
pub(super) fn image_xml(i: &SheetImage, href: &str, style: &str) -> String {
    format!(
        r#"<draw:frame draw:z-index="{}" draw:style-name="{style}" svg:width="{}" svg:height="{}" svg:x="{}" svg:y="{}"><draw:image xlink:href="{href}" xlink:type="simple" xlink:show="embed" xlink:actuate="onLoad"><text:p/></draw:image></draw:frame>"#,
        i.z,
        cm(px_to_mm(i.width_px)),
        cm(px_to_mm(i.height_px)),
        cm(px_to_mm(i.dx_px)),
        cm(px_to_mm(i.dy_px))
    )
}
