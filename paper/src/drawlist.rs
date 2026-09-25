//! **The draw list as data** for drawers outside this program, such as the
//! Flet component (docs/sekkei/drawlist.ja.adoc).
//!
//! The pages ([`Leaf`]) are turned into plain values: points (pt) from the
//! top left of the page with y growing downwards, one list of items in the
//! order the PDF draws them, and the fonts by name and file. The drawer only
//! draws what it is told where it is told; every position is worked out
//! here.

use crate::pdfw::{face_for, Leaf, Michi, Suji};
pub use serde_json;
use serde_json::{json, Value};

/// The shape of the draw list. It is 0.x and may change.
pub const VERSION: &str = "0.1";

/// A font handed to the draw list: the name cells use, where its file is,
/// and its bytes (to measure it and to pick the face a text falls back to).
pub struct FontFile {
    pub name: String,
    pub file: String,
    /// The face inside a collection (.ttc); 0 for a single font
    pub index: u32,
    pub data: Vec<u8>,
}

fn pt(mm: f32) -> f32 {
    mm * 72.0 / 25.4
}

/// Two decimals are well under a printer dot and keep the list small
fn r2(v: f32) -> f64 {
    ((v as f64) * 100.0).round() / 100.0
}

fn hex(c: (f32, f32, f32)) -> String {
    hex_str(Some(&crate::pdfw::to_hex(c)))
}

/// The colour as written on a piece (RRGGBB, with or without `#`)
fn hex_str(s: Option<&str>) -> String {
    match s {
        Some(h) => format!("#{}", h.trim_start_matches('#').to_uppercase()),
        None => "#000000".to_string(),
    }
}

/// Standard base64 (RFC 4648) for the picture bytes
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn mime(data: &[u8]) -> &'static str {
    if data.starts_with(&[0xFF, 0xD8]) { "image/jpeg" } else { "image/png" }
}

/// **The pages as a draw list.** `size_mm` is the page size for a page
/// that does not carry its own. Only the fonts some text is set in are
/// listed; a text's `font` is the index in `fonts`.
pub fn pages(pages: &[Leaf], size_mm: (f32, f32), fonts: &[FontFile]) -> Value {
    let faces: Vec<ttf_parser::Face> = fonts
        .iter()
        .filter_map(|f| ttf_parser::Face::parse(&f.data, f.index).ok())
        .collect();
    let usable = faces.len() == fonts.len() && !faces.is_empty();
    let face_of = |text: &str, want: u8| if usable { face_for(text, want, &faces) } else { 0 };
    let ascent = |i: usize| -> (f32, f32) {
        faces.get(i).map(|f| {
            let em = f.units_per_em() as f32;
            (f.ascender() as f32 / em, -(f.descender() as f32) / em)
        }).unwrap_or((0.88, 0.12))
    };
    let mut in_use = vec![false; fonts.len().max(1)];
    let mut out_pages = Vec::new();
    for page in pages {
        let (wmm, hmm) = page.size_mm.unwrap_or(size_mm);
        let h = pt(hmm);
        let y = |mm: f32| r2(h - pt(mm));
        let rect = |x: f32, y0: f32, w: f32, hh: f32| json!([r2(pt(x)), r2(h - pt(y0 + hh)), r2(pt(w)), r2(pt(hh))]);
        let mut items: Vec<Value> = Vec::new();
        if let Some(c) = page.bg {
            items.push(json!({"type": "fill", "rect": [0, 0, r2(pt(wmm)), r2(h)], "color": hex(c), "alpha": 1}));
        }
        for f in &page.fills {
            items.push(json!({"type": "fill", "rect": rect(f.x_mm, f.y_mm, f.w_mm, f.h_mm),
                              "color": hex(f.rgb), "alpha": r2(f.a)}));
        }
        // Shapes, paths and pictures in the order of their z, as the PDF does
        let mut order: Vec<(i32, u8, usize)> = Vec::new();
        order.extend(page.polys.iter().enumerate().map(|(k, g)| (g.z, 0u8, k)));
        order.extend(page.paths.iter().enumerate().map(|(k, m)| (m.z, 1u8, k)));
        order.extend(page.images.iter().enumerate().map(|(k, im)| (im.z, 2u8, k)));
        order.sort_unstable();
        let d_of = |suji: &[Suji]| -> Value {
            Value::Array(suji.iter().map(|s| match *s {
                Suji::Ugoku(a, b) => json!(["M", r2(pt(a)), y(b)]),
                Suji::Hiku(a, b) => json!(["L", r2(pt(a)), y(b)]),
                Suji::Mageru(a, b, c, d, e, f) => json!(["C", r2(pt(a)), y(b), r2(pt(c)), y(d), r2(pt(e)), y(f)]),
                Suji::Tojiru => json!(["Z"]),
            }).collect())
        };
        for (_, kind, k) in order {
            match kind {
                0 => {
                    let g = &page.polys[k];
                    if g.points.len() < 3 {
                        continue;
                    }
                    let mut d: Vec<Value> = Vec::new();
                    for (i, (a, b)) in g.points.iter().enumerate() {
                        d.push(json!([if i == 0 { "M" } else { "L" }, r2(pt(*a)), y(*b)]));
                    }
                    d.push(json!(["Z"]));
                    items.push(json!({"type": "path", "d": d, "fill": hex(g.rgb), "even_odd": false,
                                      "stroke": null, "width": 0, "dash": [], "alpha": r2(g.a), "clip": []}));
                }
                1 => {
                    let m: &Michi = &page.paths[k];
                    if m.suji.is_empty() {
                        continue;
                    }
                    items.push(json!({
                        "type": "path", "d": d_of(&m.suji),
                        "fill": m.fill.map(hex), "even_odd": m.fill_gusuu,
                        "stroke": m.stroke.map(hex),
                        "width": if m.stroke.is_some() { r2(pt(m.w_mm)) } else { 0.0 },
                        "dash": m.dash.iter().map(|v| r2(pt(*v))).collect::<Vec<_>>(),
                        "alpha": r2(m.a), "clip": d_of(&m.clip),
                    }));
                }
                _ => {
                    let im = &page.images[k];
                    let mut it = json!({"type": "image", "rect": rect(im.x_mm, im.y_mm, im.w_mm, im.h_mm),
                                        "mime": mime(&im.data), "base64": base64(&im.data)});
                    // The box to cut a stretched picture to (a:fillRect)
                    if let Some([cx0, cy0, cw, ch]) = im.clip {
                        it["clip"] = rect(cx0, cy0, cw, ch);
                    }
                    items.push(it);
                }
            }
        }
        let line = |r: &crate::pdfw::Rule| json!({
            "type": "line", "from": [r2(pt(r.x1_mm)), y(r.y1_mm)], "to": [r2(pt(r.x2_mm)), y(r.y2_mm)],
            "width": r2(pt(r.w_mm)), "color": hex(r.rgb), "alpha": r2(r.a),
            "dash": r.dash.as_deref().unwrap_or(&[]).iter().map(|v| r2(pt(*v))).collect::<Vec<_>>(),
        });
        items.extend(page.rules.iter().map(line));
        for p in &page.pieces {
            if let Some(hl) = &p.highlight {
                let h_mm = p.size_pt * 25.4 / 72.0;
                items.push(json!({"type": "fill", "rect": rect(p.x_mm, p.y_mm - h_mm * 0.22, p.w_mm, h_mm),
                                  "color": hex_str(Some(hl)), "alpha": 1}));
            }
        }
        for p in &page.pieces {
            let fi = face_of(&p.text, p.font);
            if let Some(u) = in_use.get_mut(fi) {
                *u = true;
            }
            let (asc, _) = ascent(fi);
            let base = h - pt(p.y_mm);
            let mut t = json!({
                "type": "text", "x": r2(pt(p.x_mm)), "baseline": r2(base),
                "top": r2(base - asc * p.size_pt), "text": p.text, "font": fi,
                "size": r2(p.size_pt), "width": r2(pt(p.w_mm)),
                "color": hex_str(p.color.as_deref()), "bold": p.bold,
            });
            let o = t.as_object_mut().expect("an object");
            if p.italic {
                o.insert("italic".into(), json!(true));
            }
            if p.rotation.abs() > 0.001 {
                // Degrees, turning left (counterclockwise) about (x, baseline)
                o.insert("rotation".into(), json!(r2(p.rotation)));
            }
            if p.tc_pt.abs() > 0.001 {
                o.insert("letter_spacing".into(), json!(r2(p.tc_pt)));
            }
            if p.tz > 0.0 && (p.tz - 100.0).abs() > 0.001 {
                o.insert("scale_x".into(), json!(r2(p.tz / 100.0)));
            }
            items.push(t);
            // Underline and strike-through as lines, placed as the PDF does
            for (on, at) in [(p.underline, kumihan::UNDERLINE_EM), (p.strike, kumihan::STRIKE_EM)] {
                if !on || p.w_mm <= 0.0 {
                    continue;
                }
                let h_mm = p.size_pt * 25.4 / 72.0;
                let yy = y(p.y_mm + h_mm * at);
                items.push(json!({"type": "line", "from": [r2(pt(p.x_mm)), yy], "to": [r2(pt(p.x_mm + p.w_mm)), yy],
                                  "width": r2(pt(h_mm * 0.05).max(0.3)), "color": hex_str(p.color.as_deref()),
                                  "alpha": 1, "dash": []}));
            }
        }
        items.extend(page.rules_top.iter().map(line));
        if let Some(w) = &page.watermark {
            in_use[0] = true;
            let (asc, _) = ascent(0);
            let base = h - pt(hmm) * 0.3;
            items.push(json!({"type": "text", "x": r2(pt(wmm) * 0.2), "baseline": r2(base),
                              "top": r2(base - asc * 60.0), "text": w, "font": 0, "size": 60,
                              "width": 0, "color": "#D9D9D9", "bold": false, "rotation": 45}));
        }
        let spots: Vec<Value> = page.spots.iter().map(|s| json!({
            "key": s.key, "rect": rect(s.x_mm, s.y_mm, s.w_mm, s.h_mm),
        })).collect();
        out_pages.push(json!({"size": [r2(pt(wmm)), r2(h)], "items": items, "spots": spots}));
    }
    let fonts_out: Vec<Value> = fonts
        .iter()
        .enumerate()
        .filter(|(i, _)| in_use.get(*i).copied().unwrap_or(false))
        .map(|(i, f)| {
            let (a, d) = ascent(i);
            json!({"id": i, "name": f.name, "file": f.file, "index": f.index,
                   "ascent": r2(a * 1000.0) / 1000.0, "descent": r2(d * 1000.0) / 1000.0})
        })
        .collect();
    json!({"version": VERSION, "fonts": fonts_out, "pages": out_pages})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfw::{Fill, Piece, Rule, Spot};

    /// Points from the top left, in the PDF's order, with the spots
    #[test]
    fn a_page_becomes_items_from_the_top_left() {
        let page = Leaf {
            size_mm: Some((210.0, 297.0)),
            // added last but drawn first, as in the PDF
            pieces: vec![Piece { x_mm: 25.4, y_mm: 297.0 - 25.4, size_pt: 10.0, text: "字".into(),
                                 w_mm: 10.0 * 25.4 / 72.0, ..Default::default() }],
            rules: vec![Rule { x1_mm: 0.0, y1_mm: 297.0, x2_mm: 25.4, y2_mm: 297.0, w_mm: 25.4 / 72.0,
                               dash: Some(vec![25.4 / 72.0 * 3.0, 25.4 / 72.0]), ..Default::default() }],
            fills: vec![Fill { x_mm: 0.0, y_mm: 297.0 - 25.4, w_mm: 25.4, h_mm: 25.4,
                               rgb: (1.0, 0.0, 0.0), ..Default::default() }],
            spots: vec![Spot { key: "0".into(), x_mm: 25.4, y_mm: 297.0 - 50.8, w_mm: 25.4, h_mm: 25.4 }],
            ..Default::default()
        };
        let v = pages(&[page], (210.0, 297.0), &[]);
        let p = &v["pages"][0];
        assert_eq!(p["size"], json!([595.28, 841.89]));
        let kinds: Vec<&str> = p["items"].as_array().unwrap().iter().map(|i| i["type"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["fill", "line", "text"]);
        assert_eq!(p["items"][0]["rect"], json!([0.0, 0.0, 72.0, 72.0]));
        assert_eq!(p["items"][0]["color"], "#FF0000");
        assert_eq!(p["items"][1]["from"], json!([0.0, 0.0]));
        assert_eq!(p["items"][1]["dash"], json!([3.0, 1.0]));
        let t = &p["items"][2];
        assert_eq!((t["x"].as_f64(), t["baseline"].as_f64()), (Some(72.0), Some(72.0)));
        assert!(t["top"].as_f64().unwrap() < 72.0, "the top is above the baseline");
        assert_eq!(t["width"].as_f64(), Some(10.0));
        assert_eq!(p["spots"][0]["rect"], json!([72.0, 72.0, 72.0, 72.0]));
    }

    #[test]
    fn base64_is_the_standard_one() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}
