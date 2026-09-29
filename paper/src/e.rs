//! **紙面を絵にする。**
//!
//! 2026-08-29 の依頼(SEKKEI「チャートと vello の受け持ち」)。
//!
//! # 受け持ちの線
//!
//! **チャートの元は図形の模型のまま**です。ここは「出来上がった紙面
//! ([`crate::pdfw::Leaf`])を画素にする」だけを受け持ちます。
//!
//! * PDF は [`crate::pdfw`] がベクトルのまま書きます
//! * xlsx は本物の図形(prstGeom / custGeom)のまま書きます
//! * ここが作るのは**画素だけ** — 画面の下絵、回帰検査、PNG 書き出し
//!
//! チャートの元を絵にしてしまうと、PDF と xlsx のチャートまで画素に
//! 落ちます。だから絵にするのは末端の1箇所に閉じます。
//!
//! # 呼ぶ側は vello を知りません
//!
//! 裏は `vello_cpu` ですが、この層の外へ型は出しません。alpha の API が
//! 動いても、直す場所はこのファイルだけです。
//!
//! ```no_run
//! let leaf = paper::pdfw::Leaf::default();
//! let e = paper::e::egaku(&leaf, 210.0, 297.0, 4.0);
//! std::fs::write("紙.png", e.png().unwrap()).unwrap();
//! ```

use crate::pdfw::Leaf;
use std::sync::Arc;
use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, Point, Rect, Stroke};
use vello_cpu::peniko::color::{AlphaColor, Srgb};
use vello_cpu::peniko::{Blob, FontData, Gradient, ImageBrush, ImageQuality, ImageSampler};
use vello_cpu::{Glyph, ImageSource, Pixmap, RenderContext, Resources};

/// 出来上がった絵。**画素の並びと大きさ**だけを持ちます
pub struct E {
    /// 横の画素数
    pub w: u32,
    /// 縦の画素数
    pub h: u32,
    /// RGBA(透明度を掛けた形。1画素4バイト)
    pub rgba: Vec<u8>,
}

impl E {
    /// PNG にする。**書き出しと回帰検査**に使います
    pub fn png(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, self.w, self.h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&self.rgba).map_err(|e| e.to_string())?;
        w.finish().map_err(|e| e.to_string())?;
        Ok(out)
    }

    /// **絵の指紋。** 回帰検査で「同じ入力なら同じ絵」を見るための物です。
    ///
    /// 画素を全部比べる代わりにこれを比べます。違えば画素を見に行きます。
    pub fn yubi(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self.rgba.iter().chain(&self.w.to_le_bytes()).chain(&self.h.to_le_bytes()) {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{h:016x}")
    }
}

/// **紙面を絵にする。**
///
/// `w_mm` × `h_mm` が紙の大きさ、`bai` が 1mm 何画素か(4.0 なら
/// A4 が 840×1188 画素)。
///
/// **字は描きません。** 罫線・塗り・好きな形の塗り・紙の色だけです。
/// 字も描くなら [`egaku_with`] に書体を渡します。
pub fn egaku(leaf: &Leaf, w_mm: f32, h_mm: f32, bai: f32) -> E {
    egaku_with(leaf, w_mm, h_mm, bai, None)
}

/// **書体を渡して、字も描く。**
///
/// `font` は書体の実体(TTF / OTF / TTC のバイト列)です。
/// [`crate::pdfw`] が PDF に埋めるのと**同じ物**を渡してください —
/// 同じ書体で組んだ紙面を、同じ書体で描くためです。
///
/// 字形の番号は**その場で引きます**([`ttf_parser`])。pdfw が作る
/// サブセット後の番号とは別で、こちらは元の書体の番号です。絵は
/// 書体を丸ごと持っているので、番号を詰め直す必要がありません。
pub fn egaku_with(leaf: &Leaf, w_mm: f32, h_mm: f32, bai: f32, font: Option<&[u8]>) -> E {
    let fonts: Vec<&[u8]> = font.into_iter().collect();
    egaku_fonts(leaf, w_mm, h_mm, bai, &fonts)
}

/// **書体を何本でも持つ形**(2026-09-08。PDF の `write_pages_fonts` と対)。
/// `Piece::font` がどれで描くかを指します。1本目が既定
pub fn egaku_fonts(leaf: &Leaf, w_mm: f32, h_mm: f32, bai: f32, fonts: &[&[u8]]) -> E {
    // **四捨五入します。** 切り捨てると A4 の 150 dpi が 1754 でなく 1753
    // 画素になり、1画素足りません(297mm × 150 ÷ 25.4 = 1753.94)
    let (w, h) = ((w_mm * bai).round() as u16, (h_mm * bai).round() as u16);
    let (w, h) = (w.max(1), h.max(1));
    let mut cx = RenderContext::new(w, h);
    // mm を画素に。**y はそのまま下向き**です — Leaf は左下からの y で
    // 持っているので、写すときに紙の高さから引きます
    let mm = bai as f64;

    // **紙は必ず敷きます。** 敷かないと透明のままで、開く道具によっては
    // 黒く見えます(2026-08-29 に PDF と並べて気づきました)。
    // `bg` が無ければ白 — 紙は白い物です
    cx.set_paint(iro(leaf.bg.unwrap_or((1.0, 1.0, 1.0)), 1.0));
    cx.fill_rect(&Rect::new(0.0, 0.0, w as f64, h as f64));

    // The same order as the PDF writer (`pdfw::write_pages_fonts`), so the
    // screen's pages and the PDF stack things alike: the paper colour, the
    // plain fills, then free shapes, paths and pictures by `(z, kind,
    // index)`, then the rules, the highlights, the text and the pen strokes
    for f in &leaf.fills {
        cx.set_paint(iro(f.rgb, f.a));
        let y = (h_mm - f.y_mm - f.h_mm) as f64 * mm;
        cx.fill_rect(&Rect::new(
            f.x_mm as f64 * mm,
            y,
            (f.x_mm + f.w_mm) as f64 * mm,
            y + f.h_mm as f64 * mm,
        ));
    }

    let mut order: Vec<(i32, u8, usize)> = Vec::new();
    order.extend(leaf.polys.iter().enumerate().map(|(k, g)| (g.z, 0u8, k)));
    order.extend(leaf.paths.iter().enumerate().map(|(k, m)| (m.z, 1u8, k)));
    order.extend(leaf.images.iter().enumerate().map(|(k, im)| (im.z, 2u8, k)));
    order.sort_unstable();
    for (_, kind, k) in order {
        match kind {
            0 => {
                let p = &leaf.polys[k];
                if p.points.len() < 3 {
                    continue;
                }
                let mut path = BezPath::new();
                let ten = |(x, y): (f32, f32)| Point::new(x as f64 * mm, (h_mm - y) as f64 * mm);
                path.move_to(ten(p.points[0]));
                for q in &p.points[1..] {
                    path.line_to(ten(*q));
                }
                path.close_path();
                // A gradient piece: the same stops as the PDF's shading, and
                // the end colours go on beyond the two ends (pad)
                if let Some(g) = &p.grad {
                    let stops: Vec<(f32, AlphaColor<Srgb>)> = crate::pdfw::grad_stops(g)
                        .into_iter()
                        .map(|(t, c)| (t, iro(c, p.a)))
                        .collect();
                    cx.set_paint(Gradient::new_linear(ten(g.from), ten(g.to)).with_stops(&stops[..]));
                } else {
                    cx.set_paint(iro(p.rgb, p.a));
                }
                cx.fill_path(&path);
            }
            1 => michi(&mut cx, &leaf.paths[k], h_mm, mm),
            _ => e_hameru(&mut cx, &leaf.images[k], h_mm, mm),
        }
    }

    for r in leaf.rules.iter() {
        let mut path = BezPath::new();
        path.move_to(Point::new(r.x1_mm as f64 * mm, (h_mm - r.y1_mm) as f64 * mm));
        path.line_to(Point::new(r.x2_mm as f64 * mm, (h_mm - r.y2_mm) as f64 * mm));
        let mut stroke = Stroke {
            width: (r.w_mm.max(0.05) as f64 * mm).max(0.5),
            join: Join::Miter,
            start_cap: Cap::Butt,
            end_cap: Cap::Butt,
            ..Default::default()
        };
        // The dash pattern (mm), as the PDF draws it; the screen's shapes are
        // drawn through here, so a dashed frame was solid on screen
        if let Some(d) = &r.dash {
            stroke = stroke.with_dashes(0.0, d.iter().map(|v| *v as f64 * mm));
        }
        cx.set_stroke(stroke);
        cx.set_paint(iro(r.rgb, r.a));
        cx.stroke_path(&path);
    }

    // Highlighted text: the box the PDF fills, under the letters
    for p in &leaf.pieces {
        if let Some(c) = p.highlight.as_deref().and_then(crate::pdfw::highlight_rgb) {
            let hh = p.size_pt * 25.4 / 72.0;
            let y0 = p.y_mm - hh * 0.22;
            cx.set_paint(iro(c, 1.0));
            cx.fill_rect(&Rect::new(
                p.x_mm as f64 * mm,
                (h_mm - y0 - hh) as f64 * mm,
                (p.x_mm + p.w_mm) as f64 * mm,
                (h_mm - y0) as f64 * mm,
            ));
        }
    }

    let mut res = Resources::new();
    // **字はいちばん上。** 塗りと罫線の後に置きます
    if !fonts.is_empty() {
        moji(&mut cx, &mut res, leaf, h_mm, mm, fonts);
        // **透かしも字です。** 敷いた後の紙に薄く斜めで重ねます
        if let Some(s) = &leaf.watermark {
            sukashi(&mut cx, &mut res, s, w_mm, h_mm, mm, fonts[0]);
        }
    }
    // **字の上に引く線**(手描きのペン)。字を書いた後に引きます
    for r in &leaf.rules_top {
        let mut path = BezPath::new();
        path.move_to(Point::new(r.x1_mm as f64 * mm, (h_mm - r.y1_mm) as f64 * mm));
        path.line_to(Point::new(r.x2_mm as f64 * mm, (h_mm - r.y2_mm) as f64 * mm));
        cx.set_stroke(Stroke {
            width: (r.w_mm.max(0.05) as f64 * mm).max(0.5),
            join: Join::Round,
            start_cap: Cap::Round,
            end_cap: Cap::Round,
            ..Default::default()
        });
        cx.set_paint(iro(r.rgb, r.a));
        cx.stroke_path(&path);
    }
    cx.set_transform(Affine::IDENTITY);
    cx.flush();
    let mut pix = Pixmap::new(w, h);
    cx.render(&mut pix, &mut res);
    E { w: w as u32, h: h as u32, rgba: pix.data_as_u8_slice().to_vec() }
}

/// 字を置く。**1つの `Piece` が1つの run** です(同じ大きさ・同じ色)
fn moji(
    cx: &mut RenderContext,
    res: &mut Resources,
    leaf: &Leaf,
    h_mm: f32,
    mm: f64,
    fonts: &[&[u8]],
) {
    // The letters are drawn the way the PDF writer writes them
    // (`pdfw::write_pages_fonts`): the face that has every letter of the
    // piece (`face_for`), character spacing (Tc) and horizontal scale (Tz)
    // on the advances, rotation, italic shear for a face that is not
    // italic, bold stroked only on a face that is not bold, and the
    // underline and strikethrough lines. The pictures drew none of these,
    // and a run with spacing drifted off the caret (2026-09-29 review)
    let faces: Vec<Option<ttf_parser::Face<'_>>> =
        fonts.iter().map(|data| ttf_parser::Face::parse(data, 0).ok()).collect();
    let all: Option<Vec<ttf_parser::Face<'_>>> = faces.iter().cloned().collect();
    let mut datas: Vec<Option<FontData>> = vec![None; fonts.len()];
    let pt_px = 25.4 / 72.0 * mm;
    for p in &leaf.pieces {
        if p.text.is_empty() {
            continue;
        }
        let want = (p.font as usize).min(faces.len().saturating_sub(1));
        let fi = match &all {
            Some(ok) => crate::pdfw::face_for(&p.text, p.font, ok),
            None => want,
        };
        let Some(face) = faces.get(fi).and_then(|f| f.as_ref()) else { continue };
        let fd = datas[fi]
            .get_or_insert_with(|| FontData::new(Blob::new(Arc::new(fonts[fi].to_vec())), 0))
            .clone();
        let em = face.units_per_em() as f64;
        let size = p.size_pt as f64 * pt_px;
        let th = if p.tz > 0.0 { p.tz as f64 / 100.0 } else { 1.0 };
        let tc = p.tc_pt as f64 * pt_px;
        let mut okuri = 0.0f64;
        let mut gs: Vec<Glyph> = Vec::with_capacity(p.text.chars().count());
        for ch in p.text.chars() {
            let Some(gid) = face.glyph_index(ch) else { continue };
            gs.push(Glyph { id: gid.0 as u32, x: okuri as f32, y: 0.0 });
            let adv = face.glyph_hor_advance(gid).unwrap_or(0) as f64;
            // PDF: tx = (w0 * Tfs + Tc) * Th
            okuri += (adv / em * size + tc) * th;
        }
        let (r, g, b) = p.color.as_deref().map(crate::pdfw::rgb).unwrap_or((0.0, 0.0, 0.0));
        if !gs.is_empty() {
            // `y_mm` is the baseline, from the bottom of the paper; the
            // rotation turns left (y goes down here, so it is negated)
            let at = Affine::translate((p.x_mm as f64 * mm, (h_mm - p.y_mm) as f64 * mm))
                * Affine::rotate(-(p.rotation as f64).to_radians());
            let sh = if p.italic && !face.is_italic() {
                -(face.italic_angle() as f64).to_radians().tan()
            } else {
                0.0
            };
            let per_glyph = Affine::new([th, 0.0, -sh, 1.0, 0.0, 0.0]);
            cx.set_transform(at);
            cx.set_paint(iro((r, g, b), 1.0));
            cx.glyph_run(res, &fd)
                .font_size(size as f32)
                .glyph_transform(per_glyph)
                .hint(false)
                .fill_glyphs(gs.clone().into_iter());
            // Bold on a face that is not bold: the outline stroked at a
            // thirtieth of the size, as the PDF's `Tr 2`
            if p.bold && !face.is_bold() {
                cx.set_stroke(Stroke { width: p.size_pt as f64 / 30.0 * pt_px, ..Default::default() });
                cx.glyph_run(res, &fd)
                    .font_size(size as f32)
                    .glyph_transform(per_glyph)
                    .hint(false)
                    .stroke_glyphs(gs.into_iter());
            }
            cx.set_transform(Affine::IDENTITY);
        }
        // Underline and strikethrough, the width of the piece
        let (ul_at, ul_w) = crate::pdfw::underline_em(faces.get(fi).and_then(|f| f.as_ref()));
        for (on, at, w) in [(p.underline, ul_at, ul_w), (p.strike, kumihan::STRIKE_EM, 0.05)] {
            if !on || p.w_mm <= 0.0 {
                continue;
            }
            let hh = p.size_pt * 25.4 / 72.0;
            let y = (h_mm - (p.y_mm + hh * at)) as f64 * mm;
            let mut path = BezPath::new();
            path.move_to(Point::new(p.x_mm as f64 * mm, y));
            path.line_to(Point::new((p.x_mm + p.w_mm) as f64 * mm, y));
            let w_mm = (hh as f64 * w as f64).max(0.3 * 25.4 / 72.0);
            cx.set_stroke(Stroke { width: w_mm * mm, join: Join::Miter, start_cap: Cap::Butt, end_cap: Cap::Butt, ..Default::default() });
            cx.set_paint(iro((r, g, b), 1.0));
            cx.stroke_path(&path);
        }
    }
}

/// **透かしを重ねる。** 薄い灰で 45 度に倒します([`crate::pdfw`] と同じ見え方)
fn sukashi(
    cx: &mut RenderContext,
    res: &mut Resources,
    s: &str,
    w_mm: f32,
    h_mm: f32,
    mm: f64,
    data: &[u8],
) {
    let Ok(face) = ttf_parser::Face::parse(data, 0) else { return };
    let fd = FontData::new(Blob::new(Arc::new(data.to_vec())), 0);
    let em = face.units_per_em() as f64;
    // pdfw と同じ 60pt・紙の左から2割・下から3割
    let size = 60.0 * 25.4 / 72.0 * mm;
    let (x0, y0) = (w_mm as f64 * 0.2 * mm, (h_mm as f64 * 0.7) * mm);
    let mut okuri = 0.0f64;
    let mut gs: Vec<Glyph> = Vec::with_capacity(s.chars().count());
    for ch in s.chars() {
        let Some(gid) = face.glyph_index(ch) else { continue };
        // **倒すのは字の並びの方**です。紙ごと回すと他の物まで回ります
        gs.push(Glyph { id: gid.0 as u32, x: okuri as f32, y: 0.0 });
        okuri += face.glyph_hor_advance(gid).unwrap_or(0) as f64 / em * size;
    }
    if gs.is_empty() {
        return;
    }
    let c = std::f64::consts::FRAC_1_SQRT_2;
    // y が下向きなので、上へ上がる向きに倒すには sin の符号を返します
    cx.set_transform(Affine::new([c, -c, c, c, x0, y0]));
    cx.set_paint(iro((0.85, 0.85, 0.85), 1.0));
    cx.glyph_run(res, &fd).font_size(size as f32).hint(false).fill_glyphs(gs.into_iter());
    cx.set_transform(Affine::IDENTITY);
}

/// **絵を1枚はめる。** PNG / JPEG を解いて画素にし、紙の場所へ引き伸ばします
fn e_hameru(cx: &mut RenderContext, im: &crate::pdfw::Image, h_mm: f32, mm: f64) {
    // **読めない絵は黙って飛ばします。** pdfw も同じで、数えて返す側が
    // 「読めない画像 N 件」と言います
    let Ok(dec) = image::load_from_memory(&im.data) else { return };
    let rgba = dec.to_rgba8();
    let (iw, ih) = (rgba.width(), rgba.height());
    if iw == 0 || ih == 0 || iw > u16::MAX as u32 || ih > u16::MAX as u32 {
        return;
    }
    // vello は**掛けた後の**画素(premultiplied)で持ちます
    let mut pm = Pixmap::new(iw as u16, ih as u16);
    {
        let buf = pm.data_as_u8_slice_mut();
        for (i, p) in rgba.pixels().enumerate() {
            let a = p.0[3] as u32;
            let k = i * 4;
            for j in 0..3 {
                buf[k + j] = ((p.0[j] as u32 * a + 127) / 255) as u8;
            }
            buf[k + 3] = p.0[3];
        }
    }
    // 紙の上の四角(左下からの mm を、上からの画素に直します)
    let x0 = im.x_mm as f64 * mm;
    let y0 = (h_mm - im.y_mm - im.h_mm) as f64 * mm;
    let (w, h) = (im.w_mm as f64 * mm, im.h_mm as f64 * mm);
    if !(w > 0.0 && h > 0.0) {
        return;
    }
    // 絵の画素の座標から紙の座標へ。**引き伸ばしは筆の側**で掛けます
    cx.set_paint_transform(
        Affine::translate((x0, y0)) * Affine::scale_non_uniform(w / iw as f64, h / ih as f64),
    );
    cx.set_paint(ImageBrush {
        image: ImageSource::Pixmap(Arc::new(pm)),
        sampler: ImageSampler { quality: ImageQuality::High, ..Default::default() },
    });
    // A picture stretched past its shape (a:fillRect) fills only the part
    // inside the shape's box; the brush keeps the whole picture's placement
    let mut r = Rect::new(x0, y0, x0 + w, y0 + h);
    if let Some([cx0, cy0, cw, ch]) = im.clip {
        let c = Rect::new(
            cx0 as f64 * mm,
            (h_mm - cy0 - ch) as f64 * mm,
            (cx0 + cw) as f64 * mm,
            (h_mm - cy0) as f64 * mm,
        );
        r = r.intersect(c);
        if r.width() <= 0.0 || r.height() <= 0.0 {
            cx.reset_paint_transform();
            return;
        }
    }
    cx.fill_rect(&r);
    cx.reset_paint_transform();
}

/// A path of lines and curves (`pdfw::Michi`), filled and stroked as
/// `pdfw::michi_kaku` writes it, cut to its clip shape when it has one
fn michi(cx: &mut RenderContext, m: &crate::pdfw::Michi, h_mm: f32, mm: f64) {
    use crate::pdfw::Suji;
    use vello_cpu::peniko::Fill;
    if m.suji.is_empty() {
        return;
    }
    let ten = |x: f32, y: f32| Point::new(x as f64 * mm, (h_mm - y) as f64 * mm);
    let bez = |suji: &[Suji]| {
        let mut path = BezPath::new();
        for s in suji {
            match *s {
                Suji::Ugoku(x, y) => path.move_to(ten(x, y)),
                Suji::Hiku(x, y) => path.line_to(ten(x, y)),
                Suji::Mageru(x1, y1, x2, y2, x, y) => path.curve_to(ten(x1, y1), ten(x2, y2), ten(x, y)),
                Suji::Tojiru => path.close_path(),
            }
        }
        path
    };
    let clipped = !m.clip.is_empty();
    if clipped {
        cx.set_fill_rule(Fill::NonZero);
        cx.push_clip_layer(&bez(&m.clip));
    }
    let path = bez(&m.suji);
    if let Some(c) = m.fill {
        cx.set_fill_rule(if m.fill_gusuu { Fill::EvenOdd } else { Fill::NonZero });
        cx.set_paint(iro(c, m.a));
        cx.fill_path(&path);
        cx.set_fill_rule(Fill::NonZero);
    }
    if let Some(c) = m.stroke {
        // The PDF's defaults: mitred joins and butt ends
        let mut stroke = Stroke {
            width: (m.w_mm.max(0.05) as f64 * mm).max(0.5),
            join: Join::Miter,
            start_cap: Cap::Butt,
            end_cap: Cap::Butt,
            ..Default::default()
        };
        if !m.dash.is_empty() {
            stroke = stroke.with_dashes(0.0, m.dash.iter().map(|v| *v as f64 * mm));
        }
        cx.set_stroke(stroke);
        cx.set_paint(iro(c, m.a));
        cx.stroke_path(&path);
    }
    if clipped {
        cx.pop_layer();
    }
}

/// 0〜1 の三つ組を色に
fn iro(c: (f32, f32, f32), a: f32) -> AlphaColor<Srgb> {
    AlphaColor::new([c.0, c.1, c.2, a])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdfw::{Fill, Poly, Rule};

    fn hako() -> Leaf {
        Leaf {
            bg: Some((1.0, 1.0, 1.0)),
            fills: vec![Fill {
                x_mm: 10.0, y_mm: 10.0, w_mm: 30.0, h_mm: 20.0,
                rgb: (0.87, 0.92, 0.98),
                ..Default::default()
            }],
            rules: vec![Rule {
                x1_mm: 10.0, y1_mm: 10.0, x2_mm: 40.0, y2_mm: 10.0,
                w_mm: 0.3, rgb: (0.2, 0.4, 0.7),
                ..Default::default()
            }],
            polys: vec![Poly {
                points: vec![(50.0, 10.0), (70.0, 10.0), (60.0, 30.0)],
                rgb: (0.9, 0.5, 0.2),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    // A dashed rule is dashed in the picture too, as in the PDF (2026-09-24:
    // the screen drew the dashed frame of a text box as a solid line)
    #[test]
    fn a_dashed_rule_has_gaps() {
        let leaf = Leaf {
            bg: Some((1.0, 1.0, 1.0)),
            rules: vec![Rule {
                x1_mm: 0.0, y1_mm: 5.0, x2_mm: 40.0, y2_mm: 5.0,
                w_mm: 1.0, rgb: (0.0, 0.0, 0.0),
                // sysDash: 3 widths on, 1 off
                dash: Some(vec![3.0, 1.0]),
                ..Default::default()
            }],
            ..Default::default()
        };
        let e = egaku(&leaf, 40.0, 10.0, 4.0);
        let y = e.h / 2;
        let dark: Vec<bool> = (0..e.w)
            .map(|x| e.rgba[((y * e.w + x) * 4) as usize] < 128)
            .collect();
        let runs = dark.windows(2).filter(|w| w[0] && !w[1]).count();
        // 40mm of a 4mm pattern: about 10 dashes
        assert!((9..=11).contains(&runs), "{runs} dashes");
    }

    /// A gradient piece changes colour from its top to its bottom, as the
    /// PDF's shading does
    #[test]
    fn a_gradient_piece_changes_colour_along_its_line() {
        let leaf = Leaf {
            bg: Some((1.0, 1.0, 1.0)),
            polys: vec![Poly {
                points: vec![(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)],
                rgb: (0.5, 0.5, 0.5),
                grad: Some(crate::pdfw::Grad {
                    from: (20.0, 40.0),
                    to: (20.0, 0.0),
                    stops: vec![(0.0, (1.0, 0.0, 0.0)), (1.0, (0.0, 0.0, 1.0))],
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let e = egaku(&leaf, 40.0, 40.0, 4.0);
        let px = |y: u32| {
            let i = ((y * e.w + e.w / 2) * 4) as usize;
            (e.rgba[i], e.rgba[i + 1], e.rgba[i + 2])
        };
        let (top, bottom) = (px(1), px(e.h - 2));
        assert!(top.0 > 240 && top.2 < 15, "top is not red: {top:?}");
        assert!(bottom.2 > 240 && bottom.0 < 15, "bottom is not blue: {bottom:?}");
        let mid = px(e.h / 2);
        assert!((100..160).contains(&mid.0) && (100..160).contains(&mid.2), "middle is not mixed: {mid:?}");
    }

    /// 見本の絵。左半分が赤、右半分が青の PNG
    fn futairo_png(w: u32, h: u32) -> Vec<u8> {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..h {
            for x in 0..w {
                let c: [u8; 4] =
                    if x < w / 2 { [230, 40, 40, 255] } else { [40, 40, 230, 255] };
                rgba.extend_from_slice(&c);
            }
        }
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().expect("頭");
        wr.write_image_data(&rgba).expect("中身");
        wr.finish().expect("締め");
        out
    }

    /// **絵が紙の場所に出る。** 長く描かれておらず、紙には出るのに絵からは
    /// 黙って消えていました(2026-08-29)
    #[test]
    fn an_image_lands_where_the_paper_puts_it() {
        let leaf = Leaf {
            images: vec![crate::pdfw::Image {
                x_mm: 10.0,
                y_mm: 10.0,
                w_mm: 40.0,
                h_mm: 20.0,
                data: std::sync::Arc::new(futairo_png(20, 10)),
                z: 0,
                clip: None,
            }],
            ..Default::default()
        };
        let e = egaku(&leaf, 80.0, 40.0, 4.0);
        let iro = |x_mm: f32, y_mm: f32| -> (u8, u8, u8) {
            let (px, py) = ((x_mm * 4.0) as usize, ((40.0 - y_mm) * 4.0) as usize);
            let i = (py * 320 + px) * 4;
            (e.rgba[i], e.rgba[i + 1], e.rgba[i + 2])
        };
        // 絵の左寄り(15mm, 20mm)は赤、右寄り(45mm, 20mm)は青
        let (r, _, b) = iro(15.0, 20.0);
        assert!(r > 200 && b < 100, "絵の左が出ていない: {r},{b}");
        let (r, _, b) = iro(45.0, 20.0);
        assert!(b > 200 && r < 100, "絵の右が出ていない: {r},{b}");
        // 絵の外(60mm, 20mm)は白のまま — **引き伸ばしがはみ出していない**
        let (r, g, b) = iro(60.0, 20.0);
        assert!(r > 250 && g > 250 && b > 250, "絵が枠からはみ出した: {r},{g},{b}");
    }

    /// **Shapes, paths and pictures stack by `z`, as the PDF writes them.**
    /// The restaurant brochure's dark box with the logo sits above the
    /// photo; the screen drew every picture last and hid the box, while the
    /// PDF showed it as Word does (2026-09-27)
    #[test]
    fn things_stack_in_the_pdfs_order() {
        use crate::pdfw::{Michi, Suji};
        let leaf = Leaf {
            images: vec![crate::pdfw::Image {
                x_mm: 10.0,
                y_mm: 10.0,
                w_mm: 40.0,
                h_mm: 20.0,
                data: std::sync::Arc::new(futairo_png(20, 10)),
                z: 1,
                clip: None,
            }],
            // A dark box drawn after the picture, and a path under it
            polys: vec![Poly {
                points: vec![(12.0, 12.0), (20.0, 12.0), (20.0, 18.0), (12.0, 18.0)],
                rgb: (0.1, 0.1, 0.1),
                z: 2,
                ..Default::default()
            }],
            paths: vec![Michi {
                suji: vec![
                    Suji::Ugoku(60.0, 10.0),
                    Suji::Hiku(70.0, 10.0),
                    Suji::Hiku(70.0, 20.0),
                    Suji::Tojiru,
                ],
                fill: Some((0.0, 0.6, 0.0)),
                z: 0,
                ..Default::default()
            }],
            ..Default::default()
        };
        let e = egaku(&leaf, 80.0, 40.0, 4.0);
        let iro = |x_mm: f32, y_mm: f32| -> (u8, u8, u8) {
            let (px, py) = ((x_mm * 4.0) as usize, ((40.0 - y_mm) * 4.0) as usize);
            let i = (py * 320 + px) * 4;
            (e.rgba[i], e.rgba[i + 1], e.rgba[i + 2])
        };
        let (r, g, b) = iro(15.0, 15.0);
        assert!(r < 60 && g < 60 && b < 60, "the box went under the picture: {r},{g},{b}");
        let (r, g, b) = iro(68.0, 12.0);
        assert!(g > 120 && r < 60 && b < 60, "the path is not drawn: {r},{g},{b}");
    }

    /// **A highlight named by Word's colour name is drawn in that colour**
    /// (`w:highlight w:val="yellow"`, ECMA-376 17.18.40). The name was read
    /// as hexadecimal and came out black, hiding the letters (2026-09-29)
    #[test]
    fn a_named_highlight_has_its_colour() {
        let leaf = Leaf {
            pieces: vec![crate::pdfw::Piece {
                x_mm: 10.0,
                y_mm: 20.0,
                size_pt: 20.0,
                w_mm: 30.0,
                text: "a".into(),
                highlight: Some("yellow".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let e = egaku(&leaf, 80.0, 40.0, 4.0);
        let (px, py) = (20 * 4, ((40.0 - 22.0) * 4.0) as usize);
        let i = (py * 320 + px) * 4;
        let (r, g, b) = (e.rgba[i], e.rgba[i + 1], e.rgba[i + 2]);
        assert!(r > 240 && g > 240 && b < 30, "not yellow: {r},{g},{b}");
    }

    /// **読めない絵は落とすが、他の物は描く。** 1枚のせいで紙面ごと消えない
    #[test]
    fn an_unreadable_image_does_not_take_the_page_with_it() {
        let mut leaf = hako();
        leaf.images = vec![crate::pdfw::Image {
            x_mm: 10.0,
            y_mm: 10.0,
            w_mm: 20.0,
            h_mm: 10.0,
            data: std::sync::Arc::new("これは PNG ではありません".as_bytes().to_vec()),
            z: 0,
            clip: None,
        }];
        let e = egaku(&leaf, 80.0, 40.0, 4.0);
        assert_eq!((e.w, e.h), (320, 160));
        // 三角の塗りは残っている
        let i = ((40.0 - 15.0) as usize * 4 * 320 + 60 * 4) * 4;
        assert!(e.rgba[i] > 200, "他の物まで消えた");
    }

    /// **透かしが出る。** 書体を渡したときだけです
    #[test]
    fn a_watermark_appears_only_with_a_font() {
        let leaf = Leaf { watermark: Some("見本".into()), ..Default::default() };
        let nashi = egaku(&leaf, 210.0, 297.0, 2.0);
        // **透かしの字が組める書体を選びます。** 既定の言語は機械の設定に
        // よるので(2026-08-30 から en が落ち先)、`for_document(None)` だと
        // 言語を設定していない機械で仮名の無い書体が返り、何も描かれません
        let (fam, _) = kumihan::font::for_text(None, "見本".chars()).expect("書体");
        let data = kumihan::font::load(fam).expect("読めない");
        let ari = egaku_with(&leaf, 210.0, 297.0, 2.0, Some(&data));
        assert_ne!(nashi.yubi(), ari.yubi(), "透かしが描かれていない");
        // 書体が無ければ紙は白のまま
        assert!(nashi.rgba.iter().all(|b| *b == 255), "字を描かないのに何か出た");
    }

    /// **絵が出る。** 紙の色で塗りつぶされ、置いた物の色が画素に現れます
    #[test]
    fn a_leaf_becomes_pixels() {
        let e = egaku(&hako(), 80.0, 40.0, 4.0);
        assert_eq!((e.w, e.h), (320, 160), "大きさが違う");
        assert_eq!(e.rgba.len(), 320 * 160 * 4, "画素の数が合わない");
        // 塗りの真ん中(25mm, 20mm)は青みがかっている
        let iro = |x_mm: f32, y_mm: f32| -> (u8, u8, u8) {
            let (px, py) = ((x_mm * 4.0) as usize, ((40.0 - y_mm) * 4.0) as usize);
            let i = (py * 320 + px) * 4;
            (e.rgba[i], e.rgba[i + 1], e.rgba[i + 2])
        };
        let (r, g, b) = iro(25.0, 20.0);
        assert!(b > r && b > 200, "塗りが出ていない: {r},{g},{b}");
        // 三角の真ん中(60mm, 15mm)は橙
        let (r, g, b) = iro(60.0, 15.0);
        assert!(r > 200 && b < 100, "好きな形の塗りが出ていない: {r},{g},{b}");
        // 何も置いていない所は白
        let (r, g, b) = iro(78.0, 38.0);
        assert!(r > 250 && g > 250 && b > 250, "紙の色が出ていない: {r},{g},{b}");
    }

    /// **同じ入力なら同じ絵。** 回帰検査はこれに拠ります
    #[test]
    fn the_same_input_gives_the_same_picture() {
        let a = egaku(&hako(), 80.0, 40.0, 4.0);
        let b = egaku(&hako(), 80.0, 40.0, 4.0);
        assert_eq!(a.yubi(), b.yubi(), "同じ入力で絵が変わった");
        // 違う入力なら指紋も違う
        let mut c = hako();
        c.fills[0].rgb = (0.2, 0.8, 0.3);
        assert_ne!(a.yubi(), egaku(&c, 80.0, 40.0, 4.0).yubi(), "色を変えても同じ指紋");
    }

    /// **字が出る。** 書体を渡したときだけです
    #[test]
    fn glyphs_appear_only_when_a_font_is_given() {
        // 出す字が組める書体を選びます(上の透かしの試験と同じ理由)
        let (fam, _) = kumihan::font::for_text(None, "あ".chars()).expect("書体");
        let data = kumihan::font::load(fam).expect("読めない");
        let mut leaf = Leaf { bg: Some((1.0, 1.0, 1.0)), ..Default::default() };
        leaf.pieces.push(crate::pdfw::Piece {
            x_mm: 5.0,
            y_mm: 10.0,
            size_pt: 20.0,
            text: "あ".into(),
            ..Default::default()
        });
        // 字の墨が乗った画素を数えます(白でない物)
        let sumi = |e: &E| -> usize {
            e.rgba.chunks(4).filter(|p| p[0] < 200 && p[3] > 0).count()
        };
        let nashi = egaku(&leaf, 40.0, 20.0, 4.0);
        let ari = egaku_with(&leaf, 40.0, 20.0, 4.0, Some(&data));
        assert_eq!(sumi(&nashi), 0, "書体を渡していないのに字が出ている");
        assert!(sumi(&ari) > 50, "字が出ていない: 墨の画素 {}", sumi(&ari));
    }

    fn ja_font() -> Vec<u8> {
        let (fam, _) = kumihan::font::for_text(None, "あ".chars()).expect("書体");
        kumihan::font::load(fam).expect("読めない")
    }

    /// Dark pixels in a box of the picture (mm from the top left)
    fn ink_in(e: &E, bai: f32, x0: f32, x1: f32, y0: f32, y1: f32) -> usize {
        let w = e.w as usize;
        let mut n = 0;
        for y in (y0 * bai) as usize..(y1 * bai) as usize {
            for x in (x0 * bai) as usize..(x1 * bai) as usize {
                let i = (y * w + x) * 4;
                if e.rgba[i] < 128 {
                    n += 1;
                }
            }
        }
        n
    }

    /// **Underline and strikethrough are drawn as in the PDF**: a line the
    /// width of the piece. The pictures drew neither (2026-09-29 review)
    #[test]
    fn underline_and_strike_are_drawn() {
        let data = ja_font();
        let piece = |under: bool, strike: bool| Leaf {
            pieces: vec![crate::pdfw::Piece {
                x_mm: 5.0, y_mm: 10.0, size_pt: 20.0, w_mm: 30.0,
                text: "あ".into(), underline: under, strike, ..Default::default()
            }],
            ..Default::default()
        };
        // Past the letter (7mm wide), where only a line can be
        let (x0, x1) = (20.0, 34.0);
        let h = 20.0 * 25.4 / 72.0;
        let under_y = 20.0 - (10.0 + h * kumihan::UNDERLINE_EM);
        let strike_y = 20.0 - (10.0 + h * kumihan::STRIKE_EM);
        let plain = egaku_with(&piece(false, false), 40.0, 20.0, 4.0, Some(&data));
        let lined = egaku_with(&piece(true, true), 40.0, 20.0, 4.0, Some(&data));
        assert_eq!(ink_in(&plain, 4.0, x0, x1, 0.0, 20.0), 0);
        assert!(ink_in(&lined, 4.0, x0, x1, under_y - 0.5, under_y + 0.5) > 20, "no underline");
        assert!(ink_in(&lined, 4.0, x0, x1, strike_y - 0.5, strike_y + 0.5) > 20, "no strikethrough");
    }

    /// **Character spacing (`Tc`) moves the letters after each one**, as
    /// the layout placed them. It was ignored, so the letters drifted off
    /// the caret and selection
    #[test]
    fn character_spacing_moves_the_letters() {
        let data = ja_font();
        let piece = |tc: f32| Leaf {
            pieces: vec![crate::pdfw::Piece {
                x_mm: 2.0, y_mm: 10.0, size_pt: 20.0, w_mm: 30.0,
                text: "ああ".into(), tc_pt: tc, ..Default::default()
            }],
            ..Default::default()
        };
        let right_edge = |e: &E| -> usize {
            let w = e.w as usize;
            (0..w).rev().find(|&x| (0..e.h as usize).any(|y| e.rgba[(y * w + x) * 4] < 128)).unwrap_or(0)
        };
        let tight = egaku_with(&piece(0.0), 60.0, 20.0, 4.0, Some(&data));
        let wide = egaku_with(&piece(20.0), 60.0, 20.0, 4.0, Some(&data));
        // The second letter moves by 20pt = 7.06mm = 28px at 4px/mm
        let moved = right_edge(&wide) as i64 - right_edge(&tight) as i64;
        assert!((moved - 28).abs() <= 2, "moved {moved}px");
    }

    /// **A letter the named face lacks is drawn in the first face**, as the
    /// PDF writer does (`face_for`); it went missing on the pictures
    #[test]
    fn a_letter_the_face_lacks_comes_from_the_first_face() {
        let ja = ja_font();
        // A face on this machine without kana; none found, nothing to test
        let Some(latin) = kumihan::font::list().iter().find_map(|f| {
            let d = kumihan::font::load(f).ok()?;
            let face = ttf_parser::Face::parse(&d, 0).ok()?;
            (face.glyph_index('A').is_some() && face.glyph_index('あ').is_none()).then_some(d)
        }) else {
            return;
        };
        let leaf = Leaf {
            pieces: vec![crate::pdfw::Piece {
                x_mm: 5.0, y_mm: 10.0, size_pt: 20.0, w_mm: 8.0,
                text: "あ".into(), font: 1, ..Default::default()
            }],
            ..Default::default()
        };
        let e = egaku_fonts(&leaf, 40.0, 20.0, 4.0, &[&ja, &latin]);
        assert!(ink_in(&e, 4.0, 4.0, 14.0, 2.0, 12.0) > 30, "the letter is missing");
    }

    /// **紙は必ず白く敷きます。** 敷かないと透明のままで、開く道具に
    /// よっては黒く見えます(2026-08-29 に PDF と並べて気づきました)
    #[test]
    fn the_paper_is_always_opaque() {
        let e = egaku(&Leaf::default(), 20.0, 10.0, 4.0);
        let sumi = e.rgba.chunks(4).find(|p| p[3] != 255);
        assert!(sumi.is_none(), "透けている画素がある: {sumi:?}");
        assert_eq!(&e.rgba[..4], &[255, 255, 255, 255], "紙が白くない");
    }

    /// PNG として読める物が出る
    #[test]
    fn it_writes_a_png() {
        let png = egaku(&hako(), 40.0, 20.0, 4.0).png().expect("PNG");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']), "PNG の頭が違う");
        let (w, h) = ops_image_px(&png).expect("大きさが読めない");
        assert_eq!((w, h), (160, 80), "PNG の大きさが違う");
    }

    /// PNG の頭から大きさを読む(試験のためだけ)
    fn ops_image_px(b: &[u8]) -> Option<(u32, u32)> {
        if b.len() < 24 {
            return None;
        }
        let n = |i: usize| u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        Some((n(16), n(20)))
    }
}
