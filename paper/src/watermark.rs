//! **Text watermarks** (Word's Design > Watermark).
//!
//! Word keeps a text watermark as a VML shape in the header: a `v:shape`
//! with a `v:textpath` (ECMA-376 Part 4, 19.1.2.23). The shape's `style`
//! gives its box (`width`, `height`), its place (`margin-left`,
//! `margin-top`, `mso-position-horizontal` and `-vertical` with their
//! `-relative`), and its turn (`rotation`, degrees clockwise); `fillcolor`
//! and `v:fill opacity` give the colour (Part 4, 19.1.2.19 and 19.1.2.5).
//! With `fitshape="t"` the text is stretched to the box (19.1.2.23).
//!
//! What Word draws, measured in Word for Mac's PDFs of such shapes
//! (2026-09-30; boxes of 424.5 by 141.5, 460 by 230 and 300 by 100 pt):
//!
//! * The letters' outlines are stretched, width and height separately, so
//!   their inked extent fills the box less 0.1 in at the left and right
//!   and 0.05 in at the top and bottom. Those are the default inner
//!   margins VML gives text (Part 4, 19.1.2.22, `inset`). The inked edges
//!   were 7.0 to 7.3 pt and 3.5 to 3.7 pt inside the box on all three.
//! * The box is turned about its centre.
//! * The fill is the shape's colour at the fill's opacity (`silver` came
//!   out as 192/255 with alpha 128 for `opacity=".5"`).
//! * It is drawn under the body text, also when its `z-index` is positive:
//!   a shape in the header is drawn before the body.

use crate::pdfw::rgb;

/// The inner margins of VML text (Part 4, 19.1.2.22), in mm
const INSET_X_MM: f32 = 0.1 * 25.4;
const INSET_Y_MM: f32 = 0.05 * 25.4;

/// Where a box goes along one direction (`mso-position-horizontal` or
/// `-vertical`)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align {
    /// `absolute` (the default): `margin-left` / `margin-top` from the
    /// start of the reference
    Absolute,
    /// `left` / `top` (and `inside`)
    Start,
    Center,
    /// `right` / `bottom` (and `outside`)
    End,
}

/// What a place counts from (`mso-position-horizontal-relative` or
/// `-vertical-relative`)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum From {
    /// `margin`
    Margin,
    /// `page`
    Page,
    /// `text` (the default), `char` or `line`: the anchoring paragraph.
    /// Across the page this is the text column (the margins); down the page
    /// it is the header's paragraph, which starts `w:pgMar w:header` below
    /// the top edge (ECMA-376 17.6.11)
    Text,
}

/// A watermark as the file describes it, before it is put on a page
#[derive(Debug, Clone, PartialEq)]
pub struct Vml {
    /// The box, in pt
    pub w_pt: f32,
    pub h_pt: f32,
    /// `margin-left` and `margin-top`, in pt
    pub left_pt: f32,
    pub top_pt: f32,
    pub h_align: Align,
    pub h_from: From,
    pub v_align: Align,
    pub v_from: From,
    /// Degrees, clockwise
    pub rotation: f32,
    pub rgb: (f32, f32, f32),
    /// The fill's opacity, 0 to 1
    pub a: f32,
}

impl Vml {
    /// The watermark this program writes into a docx (ooxml's
    /// `watermark_vml`): a 460 by 230 pt box turned 315 degrees, centred
    /// on the margins, `#d8d8d8` at half opacity. It is also what a
    /// watermark is drawn with when the file's own shape is not in the model
    pub fn ours() -> Vml {
        Vml {
            w_pt: 460.0,
            h_pt: 230.0,
            left_pt: 0.0,
            top_pt: 0.0,
            h_align: Align::Center,
            h_from: From::Margin,
            v_align: Align::Center,
            v_from: From::Margin,
            rotation: 315.0,
            rgb: rgb("d8d8d8"),
            a: 0.5,
        }
    }

    /// Reads the first `v:shape` in `xml` that holds a `v:textpath`. `None`
    /// when there is none, or when its box has no size
    pub fn parse(xml: &str) -> Option<Vml> {
        // The shape type before it (`v:shapetype`) holds a `v:textpath` too
        xml.match_indices("<v:shape ").find_map(|(start, _)| {
            let head_end = start + xml[start..].find('>')?;
            let end = xml[start..].find("</v:shape>").map(|e| start + e).unwrap_or(xml.len());
            let body = &xml[head_end..end];
            if !body.contains("<v:textpath") {
                return None;
            }
            Vml::parse_shape(&xml[start..head_end], body)
        })
    }

    /// One shape: its start tag and what it holds
    fn parse_shape(head: &str, body: &str) -> Option<Vml> {
        let style = attr(head, "style").unwrap_or_default();
        let prop = |name: &str| -> Option<String> {
            style.split(';').find_map(|kv| {
                let (k, v) = kv.split_once(':')?;
                (k.trim() == name).then(|| v.trim().to_string())
            })
        };
        let len = |name: &str| prop(name).and_then(|v| length_pt(&v));
        let w_pt = len("width")?;
        let h_pt = len("height")?;
        if w_pt <= 0.0 || h_pt <= 0.0 {
            return None;
        }
        let align = |v: Option<String>| match v.as_deref() {
            Some("left") | Some("top") | Some("inside") => Align::Start,
            Some("center") => Align::Center,
            Some("right") | Some("bottom") | Some("outside") => Align::End,
            _ => Align::Absolute,
        };
        let from = |v: Option<String>| match v.as_deref() {
            Some("margin") => From::Margin,
            Some("page") => From::Page,
            _ => From::Text,
        };
        // A shape with no fill colour is white (Part 4, 19.1.2.19)
        let fill = attr(head, "fillcolor").and_then(|c| colour(&c)).unwrap_or((1.0, 1.0, 1.0));
        let a = body
            .find("<v:fill")
            .and_then(|i| {
                let e = body[i..].find('>')?;
                attr(&body[i..i + e], "opacity")
            })
            .and_then(|v| opacity(&v))
            .unwrap_or(1.0);
        Some(Vml {
            w_pt,
            h_pt,
            left_pt: len("margin-left").or_else(|| len("left")).unwrap_or(0.0),
            top_pt: len("margin-top").or_else(|| len("top")).unwrap_or(0.0),
            h_align: align(prop("mso-position-horizontal")),
            h_from: from(prop("mso-position-horizontal-relative")),
            v_align: align(prop("mso-position-vertical")),
            v_from: from(prop("mso-position-vertical-relative")),
            rotation: prop("rotation").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0),
            rgb: fill,
            a,
        })
    }

    /// Puts the box on a page of this setup
    pub fn place(&self, page: &kumihan::PageSetup) -> Look {
        let pt = 25.4 / 72.0;
        let (w, h) = (self.w_pt * pt, self.h_pt * pt);
        let span = |from: From, down: bool| -> (f32, f32) {
            match (from, down) {
                (From::Page, false) => (0.0, page.w_mm),
                (From::Page, true) => (0.0, page.h_mm),
                (_, false) => (page.left_mm, page.w_mm - page.right_mm),
                (From::Text, true) => (page.header_mm, page.h_mm - page.bottom_mm),
                (_, true) => (page.top_mm, page.h_mm - page.bottom_mm),
            }
        };
        let put = |align: Align, from: From, off_pt: f32, size: f32, down: bool| -> f32 {
            let (a, b) = span(from, down);
            match align {
                Align::Absolute => a + off_pt * pt,
                Align::Start => a,
                Align::Center => a + (b - a - size) / 2.0,
                Align::End => b - size,
            }
        };
        let x = put(self.h_align, self.h_from, self.left_pt, w, false);
        let top = put(self.v_align, self.v_from, self.top_pt, h, true);
        Look {
            w_mm: w,
            h_mm: h,
            cx_mm: x + w / 2.0,
            cy_mm: page.h_mm - (top + h / 2.0),
            rotation: self.rotation,
            rgb: self.rgb,
            a: self.a,
        }
    }
}

/// A watermark put on a page: what the PDF, the page pictures and the draw
/// list draw ([`crate::pdfw::Leaf::watermark_look`])
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    /// The box, before it is turned
    pub w_mm: f32,
    pub h_mm: f32,
    /// The box's centre, in mm from the bottom left of the page like
    /// everything on a [`crate::pdfw::Leaf`]
    pub cx_mm: f32,
    pub cy_mm: f32,
    /// Degrees, clockwise (as VML's `rotation`)
    pub rotation: f32,
    pub rgb: (f32, f32, f32),
    /// The fill's opacity, 0 to 1
    pub a: f32,
}

impl Look {
    /// For a page whose margins are not known: [`Vml::ours`] centred on the
    /// page
    pub fn centred(w_mm: f32, h_mm: f32) -> Look {
        let v = Vml::ours();
        let pt = 25.4 / 72.0;
        Look {
            w_mm: v.w_pt * pt,
            h_mm: v.h_pt * pt,
            cx_mm: w_mm / 2.0,
            cy_mm: h_mm / 2.0,
            rotation: v.rotation,
            rgb: v.rgb,
            a: v.a,
        }
    }
}

/// How the text of a watermark is set so its inked extent fills the box
/// less the insets: the start of the baseline, the size, the horizontal
/// scale and the turn
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Fit {
    /// The start of the baseline, in mm from the bottom left of the page
    pub x_mm: f32,
    pub y_mm: f32,
    /// The size the height is drawn at
    pub size_pt: f32,
    /// The horizontal scale, in percent (as the PDF's `Tz`)
    pub tz: f32,
    /// Degrees, counterclockwise (as [`crate::pdfw::Piece::rotation`])
    pub angle: f32,
}

/// Works out [`Fit`] for `text` in `face`. The characters the face lacks are
/// left out, as the drawers leave them out. `None` when nothing inks
pub(crate) fn fit(look: &Look, text: &str, face: &ttf_parser::Face) -> Option<Fit> {
    let upem = face.units_per_em() as f32;
    let mut pen = 0.0f32;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for ch in text.chars() {
        let Some(g) = face.glyph_index(ch) else { continue };
        if let Some(b) = face.glyph_bounding_box(g) {
            x0 = x0.min(pen + b.x_min as f32);
            x1 = x1.max(pen + b.x_max as f32);
            y0 = y0.min(b.y_min as f32);
            y1 = y1.max(b.y_max as f32);
        }
        pen += face.glyph_hor_advance(g).unwrap_or(0) as f32;
    }
    if !(x1 > x0 && y1 > y0) || upem <= 0.0 {
        return None;
    }
    // The box less the insets; a box too small for them keeps a tenth
    let bw = (look.w_mm - 2.0 * INSET_X_MM).max(look.w_mm * 0.1);
    let bh = (look.h_mm - 2.0 * INSET_Y_MM).max(look.h_mm * 0.1);
    // mm per font unit, across and up
    let (sx, sy) = (bw / (x1 - x0), bh / (y1 - y0));
    // The start of the baseline, from the box's centre with y up
    let (ox, oy) = (-bw / 2.0 - x0 * sx, -bh / 2.0 - y0 * sy);
    // VML turns clockwise; y goes up here, so the angle is negated
    let angle = (-look.rotation).rem_euclid(360.0);
    let (s, c) = angle.to_radians().sin_cos();
    Some(Fit {
        x_mm: look.cx_mm + ox * c - oy * s,
        y_mm: look.cy_mm + ox * s + oy * c,
        size_pt: sy * upem * 72.0 / 25.4,
        tz: 100.0 * sx / sy,
        angle,
    })
}

/// Finds the watermark shape among the header's drawings kept in the model
/// (the default header's, then the first page's), and reads it. The docx
/// reader keeps the text in `Document::watermark` and the shape's own XML
/// among the header's drawings (`HeadFoot::anchors`); a document this
/// program made has no shape there, and [`Vml::ours`] is used
pub fn vml_of(doc: &kumihan::Document) -> Option<Vml> {
    let hfs = std::iter::once(&doc.header).chain(doc.first_header.as_ref());
    for hf in hfs {
        let raw = hf.anchors.iter().chain(hf.paragraphs.iter().flat_map(|p| p.anchors.iter()));
        if let Some(v) = raw.filter(|a| a.contains("v:textpath")).find_map(|a| Vml::parse(a)) {
            return Some(v);
        }
    }
    None
}

/// The value of `name="…"` in a start tag
fn attr(tag: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(i) = tag[from..].find(name) {
        let at = from + i;
        let before_ok = at == 0 || tag.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = &tag[at + name.len()..];
        if before_ok {
            if let Some(r) = rest.strip_prefix("=\"") {
                let e = r.find('"')?;
                return Some(unescape(&r[..e]));
            }
            if let Some(r) = rest.strip_prefix("='") {
                let e = r.find('\'')?;
                return Some(unescape(&r[..e]));
            }
        }
        from = at + name.len();
    }
    None
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// A CSS length in pt: `pt`, `in`, `cm`, `mm`, `pc`, `px` (1/96 in, also
/// the unit of a number without one, Part 4, 19.1.2.19 `width`), or `emu`
fn length_pt(v: &str) -> Option<f32> {
    let v = v.trim();
    let split = v.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+')).unwrap_or(v.len());
    let (n, unit) = v.split_at(split);
    let n: f32 = n.parse().ok()?;
    Some(match unit.trim() {
        "pt" => n,
        "in" => n * 72.0,
        "cm" => n * 72.0 / 2.54,
        "mm" => n * 72.0 / 25.4,
        "pc" => n * 12.0,
        "emu" => n / 12700.0,
        "px" | "" => n * 0.75,
        _ => return None,
    })
}

/// A VML colour: `#RRGGBB`, `#RGB`, or a name of the HTML colour table,
/// with Word's theme note (`#bfbfbf [2412]`) left off
fn colour(v: &str) -> Option<(f32, f32, f32)> {
    let v = v.split('[').next().unwrap_or("").trim();
    if let Some(h) = v.strip_prefix('#') {
        return match h.len() {
            6 if h.chars().all(|c| c.is_ascii_hexdigit()) => Some(rgb(h)),
            3 if h.chars().all(|c| c.is_ascii_hexdigit()) => {
                let d: String = h.chars().flat_map(|c| [c, c]).collect();
                Some(rgb(&d))
            }
            _ => None,
        };
    }
    let hex = match v.to_ascii_lowercase().as_str() {
        "black" => "000000",
        "silver" => "C0C0C0",
        "gray" | "grey" => "808080",
        "white" => "FFFFFF",
        "maroon" => "800000",
        "red" => "FF0000",
        "purple" => "800080",
        "fuchsia" => "FF00FF",
        "green" => "008000",
        "lime" => "00FF00",
        "olive" => "808000",
        "yellow" => "FFFF00",
        "navy" => "000080",
        "blue" => "0000FF",
        "teal" => "008080",
        "aqua" => "00FFFF",
        _ => return None,
    };
    Some(rgb(hex))
}

/// A VML opacity: a number, or 1/65536ths with a trailing `f` (Part 4,
/// 19.1.2.5, `opacity`)
fn opacity(v: &str) -> Option<f32> {
    let v = v.trim();
    let a = match v.strip_suffix('f') {
        Some(n) => n.parse::<f32>().ok()? / 65536.0,
        None => v.parse::<f32>().ok()?,
    };
    Some(a.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape Word writes for Design > Watermark (silver, half opacity)
    const WORD: &str = r##"<w:pict><v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:path textpathok="t" o:connecttype="custom"/><v:textpath on="t" fitshape="t"/></v:shapetype><v:shape id="PowerPlusWaterMarkObject1" o:spid="_x0000_s2049" type="#_x0000_t136" style="position:absolute;margin-left:0;margin-top:0;width:424.5pt;height:141.5pt;rotation:315;z-index:-251657216;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" o:allowincell="f" fillcolor="silver" stroked="f"><v:fill opacity=".5"/><v:textpath style="font-family:&quot;游明朝&quot;;font-size:1pt" string="社外秘"/></v:shape></w:pict>"##;

    fn a4() -> kumihan::PageSetup {
        kumihan::PageSetup {
            w_mm: 210.0,
            h_mm: 297.0,
            left_mm: 30.0,
            right_mm: 30.0,
            top_mm: 35.0,
            bottom_mm: 30.0,
            header_mm: 15.0,
            ..Default::default()
        }
    }

    #[test]
    fn word_s_watermark_is_read() {
        let v = Vml::parse(WORD).expect("not read");
        assert_eq!((v.w_pt, v.h_pt, v.rotation), (424.5, 141.5, 315.0));
        assert_eq!((v.h_align, v.h_from, v.v_align, v.v_from), (Align::Center, From::Margin, Align::Center, From::Margin));
        assert_eq!(v.rgb, rgb("C0C0C0"));
        assert_eq!(v.a, 0.5);
        // Centred on the margins
        let l = v.place(&a4());
        assert!((l.cx_mm - 105.0).abs() < 1e-3, "{l:?}");
        assert!((l.cy_mm - (297.0 - (35.0 + 267.0) / 2.0)).abs() < 1e-3, "{l:?}");
    }

    /// A box placed from the page's corner, not turned, opaque red
    #[test]
    fn a_placed_box_counts_from_what_it_names() {
        let xml = WORD
            .replace(
                "margin-left:0;margin-top:0;width:424.5pt;height:141.5pt;rotation:315;",
                "margin-left:36pt;margin-top:100pt;width:300pt;height:100pt;",
            )
            .replace(
                "mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin",
                "mso-position-horizontal-relative:page;mso-position-vertical-relative:page",
            )
            .replace("fillcolor=\"silver\"", "fillcolor=\"#FF0000\"")
            .replace("<v:fill opacity=\".5\"/>", "");
        let v = Vml::parse(&xml).expect("not read");
        assert_eq!((v.rotation, v.a, v.rgb), (0.0, 1.0, (1.0, 0.0, 0.0)));
        let l = v.place(&a4());
        let pt = 25.4 / 72.0;
        assert!((l.cx_mm - (36.0 + 150.0) * pt).abs() < 1e-3, "{l:?}");
        assert!((l.cy_mm - (297.0 - 150.0 * pt)).abs() < 1e-3, "{l:?}");
    }

    #[test]
    fn values_are_read_in_their_units() {
        assert_eq!(length_pt("1in"), Some(72.0));
        assert_eq!(length_pt("96px"), Some(72.0));
        assert_eq!(length_pt("96"), Some(72.0));
        assert!((length_pt("2.54cm").unwrap() - 72.0).abs() < 1e-3);
        assert_eq!(opacity("32768f"), Some(0.5));
        assert_eq!(opacity(".25"), Some(0.25));
        assert_eq!(colour("#bfbfbf [2412]"), Some(rgb("bfbfbf")));
        assert_eq!(colour("#f00"), Some((1.0, 0.0, 0.0)));
        assert_eq!(colour("Silver"), colour("#c0c0c0"));
    }

    /// The inked extent fills the box less 0.1 in and 0.05 in, as Word's
    /// PDF showed, and the text is turned about the box's centre
    #[test]
    fn the_text_fills_the_box_less_the_insets() {
        let (fam, _) = kumihan::font::for_text(None, "見本".chars()).expect("face");
        let data = kumihan::font::load(fam).expect("not loaded");
        let face = ttf_parser::Face::parse(&data, 0).expect("not a face");
        let mut look = Vml::ours().place(&a4());
        look.rotation = 0.0;
        let f = fit(&look, "見本", &face).expect("no fit");
        assert_eq!(f.angle, 0.0);
        // The ink of the two letters, at the fitted size and scale
        let em = f.size_pt * 25.4 / 72.0 / face.units_per_em() as f32;
        let (mut x0, mut x1, mut y0, mut y1, mut pen) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN, 0.0);
        for ch in "見本".chars() {
            let g = face.glyph_index(ch).unwrap();
            let b = face.glyph_bounding_box(g).unwrap();
            x0 = x0.min(pen + b.x_min as f32);
            x1 = x1.max(pen + b.x_max as f32);
            y0 = y0.min(b.y_min as f32);
            y1 = y1.max(b.y_max as f32);
            pen += face.glyph_hor_advance(g).unwrap() as f32;
        }
        let k = f.tz / 100.0;
        let (left, right) = (f.x_mm + x0 * em * k, f.x_mm + x1 * em * k);
        let (bottom, top) = (f.y_mm + y0 * em, f.y_mm + y1 * em);
        let (bx0, bx1) = (look.cx_mm - look.w_mm / 2.0, look.cx_mm + look.w_mm / 2.0);
        let (by0, by1) = (look.cy_mm - look.h_mm / 2.0, look.cy_mm + look.h_mm / 2.0);
        assert!((left - bx0 - 2.54).abs() < 0.01 && (bx1 - right - 2.54).abs() < 0.01, "{left} {right} in {bx0} {bx1}");
        assert!((bottom - by0 - 1.27).abs() < 0.01 && (by1 - top - 1.27).abs() < 0.01, "{bottom} {top} in {by0} {by1}");
        // Turned 315 degrees clockwise, about the centre
        look.rotation = 315.0;
        let g = fit(&look, "見本", &face).expect("no fit");
        assert!((g.angle - 45.0).abs() < 1e-3);
        let d0 = ((f.x_mm - look.cx_mm).powi(2) + (f.y_mm - look.cy_mm).powi(2)).sqrt();
        let d1 = ((g.x_mm - look.cx_mm).powi(2) + (g.y_mm - look.cy_mm).powi(2)).sqrt();
        assert!((d0 - d1).abs() < 1e-3, "not turned about the centre");
    }

    #[test]
    fn a_watermark_in_the_header_is_found() {
        let mut d = kumihan::Document::plain("本文");
        assert!(vml_of(&d).is_none());
        d.header.anchors.push(WORD.into());
        assert_eq!(vml_of(&d).map(|v| v.w_pt), Some(424.5));
    }
}
