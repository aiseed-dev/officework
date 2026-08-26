//! 図形どうしの足し引き(結合・交差・減算・切り分け)。
//!
//! **曲線はいったん折れ線に割ってから計算する。** 曲線どうしの交点を厳密に
//! 解くのは別の大仕事で、そこまでの精度は帳票の図に要らない。割りの細かさは
//! 区間あたり 16 — 画面でも紙でも折れ目が見えない細かさ(PDF の描画が 12 で
//! 足りているので、その上を取った)。
//!
//! 計算は**格子に載せた偶奇判定**で行う。交点をつないで輪郭を追う方式
//! (Greiner-Hormann など)は、重なった辺・接する頂点・自己交差でどれも
//! 別の場合分けが要り、帳票の図の品質に対して割に合わない。ここは
//!
//!   1. 両方の形を細かい格子で「中か外か」に落とす
//!   2. 求める演算で中外を組み直す
//!   3. 境目をたどって輪郭に戻す(輪郭追跡)
//!
//! という道を取る。**近似であることは画面でも言う** — 呼んだ側が
//! 「元の形はもう戻らない」と分かるように。

use super::types::PathPoint;

/// 足し引きの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    /// 結合(どちらかに入っていれば残す)
    Union,
    /// 交差(両方に入っている所だけ)
    Intersect,
    /// 減算(1つ目から2つ目を抜く)
    Subtract,
}

/// 曲線を折れ線に割る細かさ(区間あたり)
const FLATTEN: usize = 16;

/// 格子の目の数(片側)。**細かすぎると重く、粗いと角が丸まる。**
/// 256 でセル数個ぶんの図なら 1px 以下の狂いに収まる
const GRID: usize = 256;

/// 点の列(切れ目つき)を、輪郭ごとの折れ線に割る。
///
/// 曲線は `FLATTEN` に刻む。返す座標は元と同じ 0..1 の目盛り。
pub fn flatten(points: &[PathPoint]) -> Vec<Vec<(f32, f32)>> {
    let mut out: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut cur: Vec<(f32, f32)> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        if i == 0 || p.start {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(p.at);
            continue;
        }
        let prev = &points[i - 1];
        match (prev.c_out, p.c_in) {
            (None, None) => cur.push(p.at),
            (co, ci) => {
                let p0 = prev.at;
                let c1 = co.unwrap_or(prev.at);
                let c2 = ci.unwrap_or(p.at);
                let p3 = p.at;
                for k in 1..=FLATTEN {
                    let t = k as f32 / FLATTEN as f32;
                    let u = 1.0 - t;
                    cur.push((
                        u * u * u * p0.0 + 3.0 * u * u * t * c1.0 + 3.0 * u * t * t * c2.0
                            + t * t * t * p3.0,
                        u * u * u * p0.1 + 3.0 * u * u * t * c1.1 + 3.0 * u * t * t * c2.1
                            + t * t * t * p3.1,
                    ));
                }
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// その点が形の中か(偶奇。輪郭は何本でもよい)。
fn inside(contours: &[Vec<(f32, f32)>], x: f32, y: f32) -> bool {
    let mut n = 0;
    for c in contours {
        if c.len() < 3 {
            continue;
        }
        let mut j = c.len() - 1;
        for i in 0..c.len() {
            let (xi, yi) = c[i];
            let (xj, yj) = c[j];
            if (yi > y) != (yj > y) {
                let t = (y - yi) / (yj - yi);
                if x < xi + t * (xj - xi) {
                    n += 1;
                }
            }
            j = i;
        }
    }
    n % 2 == 1
}

/// 2つの形を足し引きして、輪郭の列を返す。
///
/// 座標は**1つ目の形の枠**(0..1)で数える。2つ目は、枠のずれを
/// 呼ぶ側が直してから渡す。
pub fn combine(
    a: &[Vec<(f32, f32)>],
    b: &[Vec<(f32, f32)>],
    op: BoolOp,
) -> Vec<Vec<(f32, f32)>> {
    // 両方を包む枠(はみ出した分も拾う)
    let (mut lo, mut hi) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for c in a.iter().chain(b.iter()) {
        for &(x, y) in c {
            lo = (lo.0.min(x), lo.1.min(y));
            hi = (hi.0.max(x), hi.1.max(y));
        }
    }
    if lo.0 > hi.0 {
        return Vec::new();
    }
    // 枠を少し広げる(境目が格子の縁に乗ると輪郭が閉じない)
    let pad = 0.02;
    let (lo, hi) = ((lo.0 - pad, lo.1 - pad), (hi.0 + pad, hi.1 + pad));
    let (w, h) = ((hi.0 - lo.0).max(1e-6), (hi.1 - lo.1).max(1e-6));
    let at = |i: usize, j: usize| {
        (
            lo.0 + (i as f32 + 0.5) / GRID as f32 * w,
            lo.1 + (j as f32 + 0.5) / GRID as f32 * h,
        )
    };
    // 目ごとに中か外か
    let mut m = vec![false; GRID * GRID];
    for j in 0..GRID {
        for i in 0..GRID {
            let (x, y) = at(i, j);
            let ia = inside(a, x, y);
            let ib = inside(b, x, y);
            m[j * GRID + i] = match op {
                BoolOp::Union => ia || ib,
                BoolOp::Intersect => ia && ib,
                BoolOp::Subtract => ia && !ib,
            };
        }
    }
    trace(&m, lo, (w, h))
}

/// 中外の格子から輪郭をたどる(**辺をつないで輪をつくる**)。
///
/// 中の目と外の目の境目の辺を全部あげ、端点でつないで輪にする。
/// 輪の向きは問わない — 塗りは偶奇で決めるので、外側と穴が
/// 入れ子になっていれば正しく抜ける。
fn trace(m: &[bool], lo: (f32, f32), size: (f32, f32)) -> Vec<Vec<(f32, f32)>> {
    let g = GRID;
    let inm = |i: isize, j: isize| -> bool {
        i >= 0 && j >= 0 && (i as usize) < g && (j as usize) < g && m[j as usize * g + i as usize]
    };
    // 格子の交点の座標(目の角)
    let pt = |i: isize, j: isize| {
        (
            lo.0 + i as f32 / g as f32 * size.0,
            lo.1 + j as f32 / g as f32 * size.1,
        )
    };
    // 境目の辺: (始点の格子座標, 終点の格子座標)
    let mut edges: Vec<((isize, isize), (isize, isize))> = Vec::new();
    for j in 0..g as isize {
        for i in 0..g as isize {
            if !inm(i, j) {
                continue;
            }
            // 中の目の四辺のうち、外に面している辺だけが境目
            if !inm(i, j - 1) {
                edges.push(((i, j), (i + 1, j)));
            }
            if !inm(i + 1, j) {
                edges.push(((i + 1, j), (i + 1, j + 1)));
            }
            if !inm(i, j + 1) {
                edges.push(((i + 1, j + 1), (i, j + 1)));
            }
            if !inm(i - 1, j) {
                edges.push(((i, j + 1), (i, j)));
            }
        }
    }
    // 始点 → 辺 の索引でつなぐ
    let mut from: std::collections::HashMap<(isize, isize), Vec<(isize, isize)>> =
        std::collections::HashMap::new();
    for (s, e) in &edges {
        from.entry(*s).or_default().push(*e);
    }
    let mut out: Vec<Vec<(f32, f32)>> = Vec::new();
    while let Some((&s0, _)) = from.iter().find(|(_, v)| !v.is_empty()) {
        let mut ring: Vec<(isize, isize)> = vec![s0];
        let mut cur = s0;
        while let Some(next) = from.get_mut(&cur).and_then(|v| v.pop()) {
            if next == s0 {
                break;
            }
            ring.push(next);
            cur = next;
            if ring.len() > g * g * 4 {
                break; // 念のための止め(輪にならない形)
            }
        }
        // 使い切った始点は表から外す(次の輪を探すため)
        from.retain(|_, v| !v.is_empty());
        if ring.len() >= 4 {
            out.push(simplify(ring.iter().map(|&(i, j)| pt(i, j)).collect()));
        }
    }
    out
}

/// 一直線に並んだ点をまとめる。**格子をたどると点が莫大になる** —
/// 1辺ごとに1点あると、四角1つで 1000 点を超える
fn simplify(pts: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    if pts.len() < 3 {
        return pts;
    }
    let mut out: Vec<(f32, f32)> = Vec::new();
    for (i, &p) in pts.iter().enumerate() {
        let a = pts[(i + pts.len() - 1) % pts.len()];
        let b = pts[(i + 1) % pts.len()];
        // a→p と p→b が同じ向きなら p は要らない
        let (v1x, v1y) = (p.0 - a.0, p.1 - a.1);
        let (v2x, v2y) = (b.0 - p.0, b.1 - p.1);
        let cross = v1x * v2y - v1y * v2x;
        if cross.abs() > 1e-9 {
            out.push(p);
        }
    }
    if out.len() < 3 {
        pts
    } else {
        out
    }
}

/// 輪郭の列を、切れ目つきの点の列へ戻す。
pub fn to_points(contours: &[Vec<(f32, f32)>]) -> Vec<PathPoint> {
    let mut out = Vec::new();
    for (k, c) in contours.iter().enumerate() {
        for (i, &(x, y)) in c.iter().enumerate() {
            if i == 0 && k > 0 {
                out.push(PathPoint::start_at(x, y));
            } else {
                out.push(PathPoint::at(x, y));
            }
        }
    }
    out
}

/// 図形の輪郭を 0..1 の点で取る(足し引きの相手にするため)。
///
/// **点で作る形(`path`/`ink`/`spark`)はその点をそのまま**、
/// prstGeom の形は名前ごとに輪郭を組む。丸みのある形(楕円・角丸)は
/// 32 に刻んで近似する。`None` は「輪郭を出せない形」— 呼んだ側は
/// 足し引きを断る(**黙って四角にしない**)。
pub fn outline(kind: &str, points: &[PathPoint]) -> Option<Vec<Vec<(f32, f32)>>> {
    if !points.is_empty() {
        return Some(flatten(points));
    }
    let n = |k: usize, f: &dyn Fn(f32) -> (f32, f32)| -> Vec<(f32, f32)> {
        (0..k).map(|i| f(i as f32 / k as f32)).collect()
    };
    let tau = std::f32::consts::TAU;
    let poly = |v: Vec<(f32, f32)>| Some(vec![v]);
    // 星(外周と内周を交互に)
    let star = |k: usize, inner: f32| {
        let step = std::f32::consts::PI / k as f32;
        poly(
            (0..k * 2)
                .map(|i| {
                    let r = if i % 2 == 0 { 1.0 } else { inner };
                    let a = -std::f32::consts::FRAC_PI_2 + step * i as f32;
                    (0.5 + a.cos() * 0.5 * r, 0.5 + a.sin() * 0.5 * r)
                })
                .collect(),
        )
    };
    let ngon = |k: usize| {
        poly(
            (0..k)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + tau * i as f32 / k as f32;
                    (0.5 + a.cos() * 0.5, 0.5 + a.sin() * 0.5)
                })
                .collect(),
        )
    };
    match kind {
        // 角丸は丸みを無視して四角で足し引きする(丸みは辺の 0.15 で、
        // 足し引きの結果に見えるほどの差にならない)
        "rect" | "roundRect" | "flowChartProcess" => {
            poly(vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
        }
        "ellipse" | "flowChartConnector" => poly(n(32, &|t| {
            let a = t * tau;
            (0.5 + a.cos() * 0.5, 0.5 + a.sin() * 0.5)
        })),
        "triangle" => poly(vec![(0.5, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "rtTriangle" => poly(vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)]),
        "parallelogram" | "flowChartInputOutput" => {
            poly(vec![(0.25, 0.0), (1.0, 0.0), (0.75, 1.0), (0.0, 1.0)])
        }
        "trapezoid" => poly(vec![(0.25, 0.0), (0.75, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "diamond" | "flowChartDecision" => {
            poly(vec![(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)])
        }
        "pentagon" => ngon(5),
        "hexagon" => poly(vec![
            (0.25, 0.0), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.25, 1.0), (0.0, 0.5),
        ]),
        "octagon" => poly(vec![
            (0.29, 0.0), (0.71, 0.0), (1.0, 0.29), (1.0, 0.71),
            (0.71, 1.0), (0.29, 1.0), (0.0, 0.71), (0.0, 0.29),
        ]),
        "plus" | "mathPlus" => {
            let t = if kind == "plus" { 0.25 } else { 0.1175 };
            poly(vec![
                (0.5 - t, 0.0), (0.5 + t, 0.0), (0.5 + t, 0.5 - t), (1.0, 0.5 - t),
                (1.0, 0.5 + t), (0.5 + t, 0.5 + t), (0.5 + t, 1.0), (0.5 - t, 1.0),
                (0.5 - t, 0.5 + t), (0.0, 0.5 + t), (0.0, 0.5 - t), (0.5 - t, 0.5 - t),
            ])
        }
        "star4" => star(4, 0.35),
        "star5" => star(5, 0.382),
        "star6" => star(6, 0.577),
        "star8" => star(8, 0.707),
        "rightArrow" => poly(vec![
            (0.0, 0.25), (0.65, 0.25), (0.65, 0.0), (1.0, 0.5),
            (0.65, 1.0), (0.65, 0.75), (0.0, 0.75),
        ]),
        "leftArrow" => poly(vec![
            (1.0, 0.25), (0.35, 0.25), (0.35, 0.0), (0.0, 0.5),
            (0.35, 1.0), (0.35, 0.75), (1.0, 0.75),
        ]),
        "upArrow" => poly(vec![
            (0.25, 1.0), (0.25, 0.35), (0.0, 0.35), (0.5, 0.0),
            (1.0, 0.35), (0.75, 0.35), (0.75, 1.0),
        ]),
        "downArrow" => poly(vec![
            (0.25, 0.0), (0.25, 0.65), (0.0, 0.65), (0.5, 1.0),
            (1.0, 0.65), (0.75, 0.65), (0.75, 0.0),
        ]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Vec<(f32, f32)>> {
        vec![vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]]
    }

    /// 輪郭が囲む面積(向きは問わない)
    fn area(c: &[(f32, f32)]) -> f32 {
        let mut s = 0.0;
        for i in 0..c.len() {
            let (x0, y0) = c[i];
            let (x1, y1) = c[(i + 1) % c.len()];
            s += x0 * y1 - x1 * y0;
        }
        (s / 2.0).abs()
    }

    #[test]
    fn two_separate_unions_make_two_rings() {
        let r = combine(&rect(0.0, 0.0, 0.2, 0.2), &rect(0.6, 0.6, 0.8, 0.8), BoolOp::Union);
        assert_eq!(r.len(), 2, "離れた形が1つに繋がった: {r:?}");
    }

    #[test]
    fn two_overlapping_unions_have_area_minus_the_overlap() {
        let a = rect(0.0, 0.0, 0.4, 0.4);
        let b = rect(0.2, 0.2, 0.6, 0.6);
        let r = combine(&a, &b, BoolOp::Union);
        assert_eq!(r.len(), 1, "1つの輪にならない");
        let got: f32 = r.iter().map(|c| area(c)).sum();
        // 0.16 + 0.16 - 0.04 = 0.28
        assert!((got - 0.28).abs() < 0.01, "面積が合わない: {got}");
    }

    #[test]
    fn intersection_keeps_only_the_overlap() {
        let r = combine(
            &rect(0.0, 0.0, 0.4, 0.4),
            &rect(0.2, 0.2, 0.6, 0.6),
            BoolOp::Intersect,
        );
        let got: f32 = r.iter().map(|c| area(c)).sum();
        assert!((got - 0.04).abs() < 0.005, "面積が合わない: {got}");
    }

    #[test]
    fn cutting_the_inside_leaves_a_hole() {
        // **これが輪郭を2本持てないと表せない形**
        let r = combine(
            &rect(0.0, 0.0, 0.8, 0.8),
            &rect(0.3, 0.3, 0.5, 0.5),
            BoolOp::Subtract,
        );
        assert_eq!(r.len(), 2, "外側と穴の2本にならない: {}", r.len());
        let got: f32 = r.iter().map(|c| area(c)).sum::<f32>();
        // 外 0.64 と 穴 0.04 が別々に出る(面積は足して 0.68)
        assert!((got - 0.68).abs() < 0.02, "面積が合わない: {got}");
    }

    #[test]
    fn curves_are_flattened_before_computing() {
        use super::super::types::PathPoint as P;
        let pts = vec![
            P::at(0.0, 0.5),
            P { at: (0.5, 0.5), start: false, c_in: Some((0.1, 0.0)), c_out: Some((0.9, 0.0)) },
            P::at(1.0, 0.5),
        ];
        let f = flatten(&pts);
        assert_eq!(f.len(), 1, "輪郭が割れた");
        assert!(f[0].len() > 10, "曲線を刻んでいない: {}", f[0].len());
    }

    #[test]
    fn a_gap_splits_the_outline() {
        use super::super::types::PathPoint as P;
        let pts = vec![
            P::at(0.0, 0.0),
            P::at(1.0, 0.0),
            P::start_at(0.2, 0.2),
            P::at(0.4, 0.2),
        ];
        assert_eq!(flatten(&pts).len(), 2, "切れ目で分かれていない");
    }
}

