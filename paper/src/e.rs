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
use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, Point, Rect, Stroke};
use vello_cpu::peniko::color::{AlphaColor, Srgb};
use vello_cpu::{Pixmap, RenderContext, Resources};

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
/// **字はまだ描きません。** 罫線・塗り・好きな形の塗り・紙の色を描きます。
///
/// 字を置くには書体の実体(`vello_cpu::peniko::FontData`)と、字から
/// 字形の番号への対応が要ります。対応は [`crate::pdfw`] が PDF を書く
/// ときに作っている物と同じなので、そこを分けて両方から使う形にします
/// (次の区切り)。**先に「絵が出る」所まで通して、順に足します。**
pub fn egaku(leaf: &Leaf, w_mm: f32, h_mm: f32, bai: f32) -> E {
    let (w, h) = ((w_mm * bai) as u16, (h_mm * bai) as u16);
    let (w, h) = (w.max(1), h.max(1));
    let mut cx = RenderContext::new(w, h);
    // mm を画素に。**y はそのまま下向き**です — Leaf は左下からの y で
    // 持っているので、写すときに紙の高さから引きます
    let mm = bai as f64;

    if let Some(c) = leaf.bg {
        cx.set_paint(iro(c, 1.0));
        cx.fill_rect(&Rect::new(0.0, 0.0, w as f64, h as f64));
    }

    // **塗りが先、罫線が後、絵はその間。** pdfw と同じ順です —
    // 順が違うと線が塗りに隠れます
    for f in &leaf.fills {
        cx.set_paint(iro(f.rgb, 1.0));
        let y = (h_mm - f.y_mm - f.h_mm) as f64 * mm;
        cx.fill_rect(&Rect::new(
            f.x_mm as f64 * mm,
            y,
            (f.x_mm + f.w_mm) as f64 * mm,
            y + f.h_mm as f64 * mm,
        ));
    }

    for p in &leaf.polys {
        if p.points.len() < 3 {
            continue;
        }
        let mut path = BezPath::new();
        let ten = |(x, y): (f32, f32)| {
            Point::new(x as f64 * mm, (h_mm - y) as f64 * mm)
        };
        path.move_to(ten(p.points[0]));
        for q in &p.points[1..] {
            path.line_to(ten(*q));
        }
        path.close_path();
        cx.set_paint(iro(p.rgb, 1.0));
        cx.fill_path(&path);
    }

    for r in &leaf.rules {
        let mut path = BezPath::new();
        path.move_to(Point::new(r.x1_mm as f64 * mm, (h_mm - r.y1_mm) as f64 * mm));
        path.line_to(Point::new(r.x2_mm as f64 * mm, (h_mm - r.y2_mm) as f64 * mm));
        cx.set_stroke(Stroke {
            width: (r.w_mm.max(0.05) as f64 * mm).max(0.5),
            join: Join::Miter,
            start_cap: Cap::Butt,
            end_cap: Cap::Butt,
            ..Default::default()
        });
        cx.set_paint(iro(r.rgb, 1.0));
        cx.stroke_path(&path);
    }

    cx.set_transform(Affine::IDENTITY);
    cx.flush();
    let mut pix = Pixmap::new(w, h);
    let mut res = Resources::new();
    cx.render(&mut pix, &mut res);
    E { w: w as u32, h: h as u32, rgba: pix.data_as_u8_slice().to_vec() }
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
            }],
            rules: vec![Rule {
                x1_mm: 10.0, y1_mm: 10.0, x2_mm: 40.0, y2_mm: 10.0,
                w_mm: 0.3, rgb: (0.2, 0.4, 0.7),
            }],
            polys: vec![Poly {
                points: vec![(50.0, 10.0), (70.0, 10.0), (60.0, 30.0)],
                rgb: (0.9, 0.5, 0.2),
            }],
            ..Default::default()
        }
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
