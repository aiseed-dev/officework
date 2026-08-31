//! **WMF(Windows メタファイル)を紙の道に直す。**
//!
//! docx の図には、まだこの形が残っています。内閣府の面談記録の様式
//! (document_4.docx)は3枚のうち2枚が WMF です。絵ではなく**線と塗りの
//! 並び**なので、画素にせずそのまま [`crate::pdfw::Michi`] へ移します。
//! 拡大しても粗くなりません。
//!
//! 読むのは [`wmf_core`] です。あちらは `Player` という受け口を公開して
//! いて、必ず書くのは2つ、残り70の記録には既定の実装があります。ここでは
//! **線と塗りに関わるものだけ**を受けます。
//!
//! # 座標
//!
//! WMF は自前の升目(window)で座標を持ちます。置き場と大きさは
//! `SetWindowOrg` と `SetWindowExt` で決まり、無ければ見出しの外枠を
//! 使います。それを、docx が言う mm の枠へ引き伸ばします。
//!
//! # まだ受けないもの
//!
//! 字(`TextOut`)と絵の貼り付け(`BitBlt` の類)は受けていません。
//! **落としたことは数えます** — 黙って消しません。

use wmf_core::converter::{PlayError, Player};
use wmf_core::parser::*;

use crate::pdfw::{Michi, Suji};

/// WMF を読んで、道の並びにする係。
///
/// `x_mm`・`y_mm`(左下)と `w_mm`・`h_mm` が、紙の上の置き場です。
pub struct Wmf {
    /// 出来上がった道(紙の座標、mm)。
    /// **`wmf_core` は係を返してくれない**(`generate` の返りだけを渡す)
    /// ので、呼ぶ側と分け合う入れ物に積みます
    michi: std::rc::Rc<std::cell::RefCell<Vec<Michi>>>,
    /// 受けなかった記録の数
    nokoshi: std::rc::Rc<std::cell::Cell<usize>>,
    /// 置き場(紙の左下からの mm)
    waku: (f32, f32, f32, f32),
    /// WMF の升目(左, 上, 幅, 高さ)
    mado: (f32, f32, f32, f32),
    /// いまのペン(色, 太さ mm, 点線の刻み)。None なら線を引かない
    pen: Option<((f32, f32, f32), f32, Vec<f32>)>,
    /// いまの刷毛(塗りの色)。None なら塗らない
    hake: Option<(f32, f32, f32)>,
    /// 番号で覚えた道具。`SelectObject` がここから選びます
    dougu: Vec<Dougu>,
    /// `MoveTo` が置いた位置(WMF の升目)
    ima: (f32, f32),
}

/// 番号で覚える道具。WMF は作った順に番号が付きます
#[derive(Clone)]
enum Dougu {
    Pen(Option<((f32, f32, f32), f32, Vec<f32>)>),
    Hake(Option<(f32, f32, f32)>),
    /// 書体など、こちらが使わない道具
    Hoka,
}

impl Wmf {
    /// 置き場(紙の左下からの mm)を決めて作ります
    pub fn new(x_mm: f32, y_mm: f32, w_mm: f32, h_mm: f32) -> Self {
        Wmf {
            michi: Default::default(),
            nokoshi: Default::default(),
            waku: (x_mm, y_mm, w_mm, h_mm),
            mado: (0.0, 0.0, 1.0, 1.0),
            pen: Some(((0.0, 0.0, 0.0), 0.2, Vec::new())),
            hake: None,
            dougu: Vec::new(),
            ima: (0.0, 0.0),
        }
    }

    /// WMF の升目の点を、紙の mm に直します。
    /// **WMF の y は下向き**、紙の y は上向きなので、上下が入れ替わります
    fn ten(&self, x: f32, y: f32) -> (f32, f32) {
        let (ox, oy, ow, oh) = self.mado;
        let (px, py, pw, ph) = self.waku;
        let kx = if ow.abs() < 1e-6 { 0.0 } else { (x - ox) / ow };
        let ky = if oh.abs() < 1e-6 { 0.0 } else { (y - oy) / oh };
        (px + kx * pw, py + ph - ky * ph)
    }

    fn ten_s(&self, p: &PointS) -> (f32, f32) {
        self.ten(f32::from(p.x), f32::from(p.y))
    }

    /// いまのペンと刷毛で、道を1つ積みます
    fn tsumu(&mut self, suji: Vec<Suji>, tojiru: bool) {
        if suji.is_empty() {
            return;
        }
        let mut suji = suji;
        if tojiru {
            suji.push(Suji::Tojiru);
        }
        let (iro, futo, kizami) = match &self.pen {
            Some((c, w, d)) => (Some(*c), *w, d.clone()),
            None => (None, 0.0, Vec::new()),
        };
        // 塗りも線も無い道は積みません(何も出ないため)
        if iro.is_none() && self.hake.is_none() {
            return;
        }
        self.michi.borrow_mut().push(Michi {
            suji,
            fill: self.hake,
            stroke: iro,
            w_mm: futo.max(0.05),
            dash: kizami,
            ..Default::default()
        });
    }

    /// 点の並びから、直線でつないだ道を作ります
    fn tsuraneru(&self, ten: &[PointS]) -> Vec<Suji> {
        let mut v = Vec::with_capacity(ten.len());
        for (i, p) in ten.iter().enumerate() {
            let (x, y) = self.ten_s(p);
            v.push(if i == 0 { Suji::Ugoku(x, y) } else { Suji::Hiku(x, y) });
        }
        v
    }

    /// 楕円をベジェ曲線4本で作ります。円の 4分の1 を1本で表すときの
    /// 制御点の長さは `4/3 × tan(π/8) = 0.5523` です
    fn daen(&self, l: f32, t: f32, r: f32, b: f32) -> Vec<Suji> {
        const K: f32 = 0.552_284_8;
        let (cx, cy) = ((l + r) / 2.0, (t + b) / 2.0);
        let (rx, ry) = ((r - l) / 2.0, (b - t) / 2.0);
        // 右 → 下 → 左 → 上(WMF の升目のまま。`ten` が紙へ直します)
        let p = |x: f32, y: f32| self.ten(x, y);
        let (x0, y0) = p(cx + rx, cy);
        let mut v = vec![Suji::Ugoku(x0, y0)];
        for (a, b2, c) in [
            ((cx + rx, cy + ry * K), (cx + rx * K, cy + ry), (cx, cy + ry)),
            ((cx - rx * K, cy + ry), (cx - rx, cy + ry * K), (cx - rx, cy)),
            ((cx - rx, cy - ry * K), (cx - rx * K, cy - ry), (cx, cy - ry)),
            ((cx + rx * K, cy - ry), (cx + rx, cy - ry * K), (cx + rx, cy)),
        ] {
            let (ax, ay) = p(a.0, a.1);
            let (bx, by) = p(b2.0, b2.1);
            let (cx2, cy2) = p(c.0, c.1);
            v.push(Suji::Mageru(ax, ay, bx, by, cx2, cy2));
        }
        v.push(Suji::Tojiru);
        v
    }

    /// 番号の道具を選びます
    fn erabu(&mut self, i: usize) {
        match self.dougu.get(i).cloned() {
            Some(Dougu::Pen(p)) => self.pen = p,
            Some(Dougu::Hake(h)) => self.hake = h,
            _ => {}
        }
    }

    /// 作った道具を、空いている番号へ入れます
    fn shimau(&mut self, d: Dougu) {
        if let Some(k) = self.dougu.iter().position(|x| matches!(x, Dougu::Hoka)) {
            self.dougu[k] = d;
        } else {
            self.dougu.push(d);
        }
    }
}

fn iro(c: &ColorRef) -> (f32, f32, f32) {
    (f32::from(c.red) / 255.0, f32::from(c.green) / 255.0, f32::from(c.blue) / 255.0)
}

impl Player for Wmf {
    fn generate(self) -> Result<Vec<u8>, PlayError> {
        // 出すのは道で、バイト列ではありません。呼ぶ側が `michi` を取ります
        Ok(Vec::new())
    }

    fn header(mut self, _n: usize, h: MetafileHeader) -> Result<Self, PlayError> {
        // **置ける枠があればそれを升目の既定にします。** `SetWindowExt` が
        // 来れば上書きされます
        if let MetafileHeader::StartsWithPlaceable(p, _) = &h {
            let b = &p.bounding_box;
            let (w, hh) = (f32::from(b.right - b.left), f32::from(b.bottom - b.top));
            if w.abs() > 1e-6 && hh.abs() > 1e-6 {
                self.mado = (f32::from(b.left), f32::from(b.top), w, hh);
            }
        }
        Ok(self)
    }

    fn set_window_origin(mut self, _n: usize, r: META_SETWINDOWORG) -> Result<Self, PlayError> {
        self.mado.0 = f32::from(r.x);
        self.mado.1 = f32::from(r.y);
        Ok(self)
    }

    fn set_window_ext(mut self, _n: usize, r: META_SETWINDOWEXT) -> Result<Self, PlayError> {
        if r.x != 0 {
            self.mado.2 = f32::from(r.x);
        }
        if r.y != 0 {
            self.mado.3 = f32::from(r.y);
        }
        Ok(self)
    }

    fn create_pen_indirect(
        mut self,
        _n: usize,
        r: META_CREATEPENINDIRECT,
    ) -> Result<Self, PlayError> {
        let p = &r.pen;
        // 太さは升目の単位。横の縮尺で mm に直します
        let (_, _, ow, _) = self.mado;
        let (_, _, pw, _) = self.waku;
        let futo = if ow.abs() < 1e-6 {
            0.2
        } else {
            (f32::from(p.width.x).abs() / ow * pw).max(0.05)
        };
        // 点線の刻み。PS_DASH などは MS-WMF の PenStyle
        let kizami = match format!("{:?}", p.style.style).to_lowercase() {
            s if s.contains("null") => return Ok({ self.shimau(Dougu::Pen(None)); self }),
            s if s.contains("dashdotdot") => vec![futo * 6.0, futo * 2.0, futo, futo * 2.0],
            s if s.contains("dashdot") => vec![futo * 6.0, futo * 2.0, futo, futo * 2.0],
            s if s.contains("dash") => vec![futo * 6.0, futo * 3.0],
            s if s.contains("dot") => vec![futo, futo * 2.0],
            _ => Vec::new(),
        };
        self.shimau(Dougu::Pen(Some((iro(&p.color_ref), futo, kizami))));
        Ok(self)
    }

    fn create_brush_indirect(
        mut self,
        _n: usize,
        r: META_CREATEBRUSHINDIRECT,
    ) -> Result<Self, PlayError> {
        // **塗りつぶしと網掛けだけ受けます。** 絵の柄(DIB)は受けません。
        // 網掛けは線の並びですが、色1つに畳みます — 表計算の柄と同じ
        // 考え方です(線を引かず、色で表す)
        let hake = match &r.log_brush {
            LogBrush::Solid { color_ref } => Some(iro(color_ref)),
            LogBrush::Hatched { color_ref, .. } => Some(iro(color_ref)),
            _ => None,
        };
        self.shimau(Dougu::Hake(hake));
        Ok(self)
    }

    fn select_object(mut self, _n: usize, r: META_SELECTOBJECT) -> Result<Self, PlayError> {
        self.erabu(usize::from(r.object_index));
        Ok(self)
    }

    fn delete_object(mut self, _n: usize, r: META_DELETEOBJECT) -> Result<Self, PlayError> {
        let i = usize::from(r.object_index);
        if i < self.dougu.len() {
            self.dougu[i] = Dougu::Hoka;
        }
        Ok(self)
    }

    fn move_to(mut self, _n: usize, r: META_MOVETO) -> Result<Self, PlayError> {
        self.ima = (f32::from(r.x), f32::from(r.y));
        Ok(self)
    }

    fn line_to(mut self, _n: usize, r: META_LINETO) -> Result<Self, PlayError> {
        let (x0, y0) = self.ten(self.ima.0, self.ima.1);
        let (x1, y1) = self.ten(f32::from(r.x), f32::from(r.y));
        // 線だけを引きます(刷毛は使いません)
        let hake = self.hake.take();
        self.tsumu(vec![Suji::Ugoku(x0, y0), Suji::Hiku(x1, y1)], false);
        self.hake = hake;
        self.ima = (f32::from(r.x), f32::from(r.y));
        Ok(self)
    }

    fn polyline(mut self, _n: usize, r: META_POLYLINE) -> Result<Self, PlayError> {
        let suji = self.tsuraneru(&r.a_points);
        let hake = self.hake.take();
        self.tsumu(suji, false);
        self.hake = hake;
        Ok(self)
    }

    fn polygon(mut self, _n: usize, r: META_POLYGON) -> Result<Self, PlayError> {
        let suji = self.tsuraneru(&r.a_points);
        self.tsumu(suji, true);
        Ok(self)
    }

    fn poly_polygon(self, _n: usize, r: META_POLYPOLYGON) -> Result<Self, PlayError> {
        // **穴のある形。** 1つの道に全部の輪を入れ、偶奇で塗ります
        let mut suji = Vec::new();
        let mut atama = 0usize;
        for kazu in &r.poly_polygon.a_points_per_polygon {
            let n = usize::from(*kazu);
            let owari = (atama + n).min(r.poly_polygon.a_points.len());
            if atama < owari {
                suji.extend(self.tsuraneru(&r.poly_polygon.a_points[atama..owari]));
                suji.push(Suji::Tojiru);
            }
            atama = owari;
        }
        if suji.is_empty() {
            return Ok(self);
        }
        let (c, futo, kizami) = match &self.pen {
            Some((c, w, d)) => (Some(*c), *w, d.clone()),
            None => (None, 0.0, Vec::new()),
        };
        if c.is_none() && self.hake.is_none() {
            return Ok(self);
        }
        self.michi.borrow_mut().push(Michi {
            suji,
            fill: self.hake,
            fill_gusuu: true,
            stroke: c,
            w_mm: futo.max(0.05),
            dash: kizami,
            ..Default::default()
        });
        Ok(self)
    }

    fn rectangle(mut self, _n: usize, r: META_RECTANGLE) -> Result<Self, PlayError> {
        let (l, t) = (f32::from(r.left_rect), f32::from(r.top_rect));
        let (ri, b) = (f32::from(r.right_rect), f32::from(r.bottom_rect));
        let p = [(l, t), (ri, t), (ri, b), (l, b)];
        let suji = p
            .iter()
            .enumerate()
            .map(|(i, (x, y))| {
                let (mx, my) = self.ten(*x, *y);
                if i == 0 { Suji::Ugoku(mx, my) } else { Suji::Hiku(mx, my) }
            })
            .collect();
        self.tsumu(suji, true);
        Ok(self)
    }

    fn ellipse(mut self, _n: usize, r: META_ELLIPSE) -> Result<Self, PlayError> {
        let suji = self.daen(
            f32::from(r.left_rect),
            f32::from(r.top_rect),
            f32::from(r.right_rect),
            f32::from(r.bottom_rect),
        );
        self.tsumu(suji, false);
        Ok(self)
    }

    fn create_font_indirect(
        mut self,
        _n: usize,
        _r: META_CREATEFONTINDIRECT,
    ) -> Result<Self, PlayError> {
        // 書体は使いませんが、**番号は詰めます** — 詰めないと後の
        // `SelectObject` がずれます
        self.shimau(Dougu::Hoka);
        Ok(self)
    }

    fn text_out(self, _n: usize, _r: META_TEXTOUT) -> Result<Self, PlayError> {
        self.nokoshi.set(self.nokoshi.get() + 1);
        Ok(self)
    }

    fn ext_text_out(self, _n: usize, _r: META_EXTTEXTOUT) -> Result<Self, PlayError> {
        self.nokoshi.set(self.nokoshi.get() + 1);
        Ok(self)
    }
}

/// **WMF か。** 置ける形(`D7CDC69A`)と、素の形(`0100 0900`)の2つです
pub fn wmf_ka(data: &[u8]) -> bool {
    data.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) || data.starts_with(&[0x01, 0x00, 0x09, 0x00])
}

/// **WMF を読んで、紙の上の道にします。**
///
/// 置き場は紙の左下からの mm。読めなければ `None` を返します(呼ぶ側が
/// 数えて知らせます)。
pub fn michi_ni(
    data: &[u8],
    x_mm: f32,
    y_mm: f32,
    w_mm: f32,
    h_mm: f32,
) -> Option<(Vec<Michi>, usize)> {
    let p = Wmf::new(x_mm, y_mm, w_mm, h_mm);
    let (michi, nokoshi) = (p.michi.clone(), p.nokoshi.clone());
    wmf_core::converter::convert(data, p).ok()?;
    let v = michi.borrow().len();
    (v > 0).then(|| (michi.take(), nokoshi.get()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **置ける形の WMF を1つ作ります。** 四角を1つ描くだけの、いちばん
    /// 短いもの。中身は MS-WMF の並びそのままです
    fn wmf_shikaku() -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        let mut w = |x: u16| v.extend_from_slice(&x.to_le_bytes());
        // 置ける見出し(22 バイト)。外枠は 0,0 〜 100,100
        w(0xCDD7);
        w(0x9AC6);
        w(0); // hwmf
        w(0);
        w(0);
        w(100);
        w(100); // 外枠
        w(1440); // 1 インチあたりの升目
        w(0);
        w(0); // 予備
        w(0); // 検査の値(読み手は見ません)
        // 見出し(18 バイト = 9 語)
        w(1); // 種類(メタファイル)
        w(9); // 見出しの語数
        w(0x0300); // 版
        w(0);
        w(0); // 大きさ(語)
        w(2); // 覚えられる道具の数
        w(0);
        w(0); // いちばん大きい記録
        w(0); // 予備
        // SetWindowExt(100, 100)
        w(5);
        w(0);
        w(0x020C);
        w(100);
        w(100);
        // Rectangle(10, 10, 90, 90)
        w(7);
        w(0);
        w(0x041B);
        w(90);
        w(90);
        w(10);
        w(10);
        // 終わり
        w(3);
        w(0);
        w(0x0000);
        v
    }

    /// **WMF が道になる。**
    ///
    /// 内閣府の面談記録の様式(document_4.docx)は、3枚のうち2枚が WMF
    /// です。前は `image` クレートが読めず、絵が出ていませんでした
    /// (2026-08-31 発注者「document_4 では、絵が消えている」)。
    #[test]
    fn a_metafile_becomes_paths_on_the_paper() {
        let d = wmf_shikaku();
        assert!(wmf_ka(&d), "置ける形の印を見ていない");
        let (michi, nokoshi) = michi_ni(&d, 20.0, 50.0, 40.0, 40.0).expect("読めない");
        assert_eq!(nokoshi, 0, "受けなかった記録がある");
        assert_eq!(michi.len(), 1, "四角1つが道1本にならない");
        // 升目 100 のうち 10〜90 なので、40mm の枠では 4mm〜36mm。
        // **y は上下が入れ替わります** — WMF は下向き、紙は上向きです
        let x: Vec<f32> = michi[0]
            .suji
            .iter()
            .filter_map(|s| match s {
                Suji::Ugoku(x, _) | Suji::Hiku(x, _) => Some(*x),
                _ => None,
            })
            .collect();
        let hidari = x.iter().cloned().fold(f32::MAX, f32::min);
        let migi = x.iter().cloned().fold(f32::MIN, f32::max);
        assert!((hidari - 24.0).abs() < 0.1, "左が {hidari}mm(24mm のはず)");
        assert!((migi - 56.0).abs() < 0.1, "右が {migi}mm(56mm のはず)");
    }
}
