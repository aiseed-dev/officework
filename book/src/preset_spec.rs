//! OOXML の図形の定義([`crate::preset_gen`])を**点の列**にする解釈器。
//!
//! 定義データには、形ごとに調整値の既定(adj)・座標の計算式(gd)・
//! 線の引き方(cmds)が入っている。ここでやるのはその3段の解釈だけで、
//! 形そのものの知識は持たない — 形の正体は vendor の定義データ1箇所。
//!
//! # 式
//!
//! `"*/ w adj1 100000"` のような前置きの字句。演算は17種で、名前は
//! 計算済みの式 → 調整値 → 組み込み(`l` `r` `hc` `wd2` `cd4` など)→
//! 数字、の順に引く。角度は 1/60000 度で持つ(OOXML の決まり)。
//!
//! # 座標
//!
//! path が自分の座標系(`w` `h`)を言うときは、その座標系から実寸へ
//! 倍率を掛ける(この形の path は数字の座標しか持たないことを
//! 生成時に確かめてある)。言わないときは式の値がそのまま実寸。

use crate::preset_gen::{SpecShape, SHAPES};

/// 1本の輪郭。紙(PDF)と画面(SVG)が共通に使う
#[derive(Debug, Clone)]
pub struct Poly {
    pub pts: Vec<(f32, f32)>,
    pub closed: bool,
    /// この輪郭を塗ってよいか(定義が `fill="none"` と言う輪郭は塗らない)
    pub fill: bool,
    /// この輪郭の線を引くか(`stroke="false"` は塗りだけ)
    pub stroke: bool,
}

fn find(kind: &str) -> Option<&'static SpecShape> {
    // 187個の並びの線形探索。形1つの描画で1回だけなので足りる
    SHAPES.iter().find(|s| s.name == kind)
}

/// その名前が定義データにあるか
pub fn spec_has(kind: &str) -> bool {
    find(kind).is_some()
}

/// 定義データにある名前の一覧(定義順)。一覧画面と確認の絵が使う
pub fn spec_names() -> Vec<&'static str> {
    SHAPES.iter().map(|s| s.name).collect()
}

/// 1/60000 度 → ラジアン
fn rad(v: f64) -> f64 {
    (v / 60000.0).to_radians()
}

/// 0 割りを避ける(定義データに 0 で割る式は無いが、調整値は人が書く)
fn nz(v: f64) -> f64 {
    if v == 0.0 {
        1e-9
    } else {
        v
    }
}

/// 名前 → 値。計算済み → 組み込み → 数字の順
fn lookup(name: &str, env: &[(&str, f64)], w: f64, h: f64) -> f64 {
    if let Some((_, v)) = env.iter().rev().find(|(k, _)| *k == name) {
        return *v;
    }
    let ss = w.min(h);
    let frac = |head: &str, base: f64| -> Option<f64> {
        name.strip_prefix(head)
            .and_then(|n| n.parse::<f64>().ok())
            .map(|n| base / n)
    };
    match name {
        "w" => w,
        "h" => h,
        "l" | "t" => 0.0,
        "r" => w,
        "b" => h,
        "hc" => w / 2.0,
        "vc" => h / 2.0,
        "ss" => ss,
        "ls" => w.max(h),
        // 円の割り前(1/60000 度)。cd2 = 半周 = 180度
        "cd2" => 10_800_000.0,
        "cd4" => 5_400_000.0,
        "cd8" => 2_700_000.0,
        "3cd4" => 16_200_000.0,
        "3cd8" => 8_100_000.0,
        "5cd8" => 13_500_000.0,
        "7cd8" => 18_900_000.0,
        _ => frac("wd", w)
            .or_else(|| frac("hd", h))
            .or_else(|| frac("ssd", ss))
            .or_else(|| name.parse::<f64>().ok())
            .unwrap_or(0.0),
    }
}

/// 1つの式を解く
fn eval(fmla: &str, env: &[(&str, f64)], w: f64, h: f64) -> f64 {
    let t: Vec<&str> = fmla.split_whitespace().collect();
    let a = |i: usize| t.get(i).map(|n| lookup(n, env, w, h)).unwrap_or(0.0);
    match t.first().copied().unwrap_or("") {
        "val" => a(1),
        "*/" => a(1) * a(2) / nz(a(3)),
        "+-" => a(1) + a(2) - a(3),
        "+/" => (a(1) + a(2)) / nz(a(3)),
        "?:" => {
            if a(1) > 0.0 {
                a(2)
            } else {
                a(3)
            }
        }
        "abs" => a(1).abs(),
        // at2 x y = atan2(y, x)(答えは 1/60000 度)
        "at2" => a(2).atan2(a(1)).to_degrees() * 60000.0,
        "cat2" => a(1) * a(3).atan2(a(2)).cos(),
        "sat2" => a(1) * a(3).atan2(a(2)).sin(),
        "cos" => a(1) * rad(a(2)).cos(),
        "sin" => a(1) * rad(a(2)).sin(),
        "tan" => a(1) * rad(a(2)).tan(),
        "max" => a(1).max(a(2)),
        "min" => a(1).min(a(2)),
        // mod x y z = 3つ組の長さ
        "mod" => (a(1) * a(1) + a(2) * a(2) + a(3) * a(3)).sqrt(),
        // pin x y z = y を [x, z] に収める
        "pin" => {
            let (lo, v, hi) = (a(1), a(2), a(3));
            if v < lo {
                lo
            } else if v > hi {
                hi
            } else {
                v
            }
        }
        "sqrt" => a(1).max(0.0).sqrt(),
        _ => 0.0,
    }
}

/// 形を点の列にする。箱 (x0,y0)-(x1,y1) に収め、調整値の上書きを受ける。
///
/// 曲線と弧は 12〜24 に刻む(紙の他の曲線と同じで、見た目に区別が
/// 付かない細かさ)。知らない名前は `None`。
pub fn spec_polys(
    kind: &str,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    adj: &[(String, f32)],
) -> Option<Vec<Poly>> {
    let sh = find(kind)?;
    let (w, h) = ((x1 - x0) as f64, (y1 - y0) as f64);
    // 名前の環境。調整値(上書きがあればそちら)→ 式を定義順に
    let mut env: Vec<(&str, f64)> = Vec::with_capacity(sh.adj.len() + sh.gd.len());
    for (k, v) in sh.adj {
        let v = adj
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, o)| *o as f64)
            .unwrap_or(*v as f64);
        env.push((k, v));
    }
    for (k, f) in sh.gd {
        let v = eval(f, &env, w, h);
        env.push((k, v));
    }
    let mut out: Vec<Poly> = Vec::new();
    for p in sh.paths {
        // path が自分の座標系を言うときの倍率
        let (sx, sy) = (
            if p.w > 0.0 { w / p.w as f64 } else { 1.0 },
            if p.h > 0.0 { h / p.h as f64 } else { 1.0 },
        );
        let t: Vec<&str> = p.cmds.split_whitespace().collect();
        let v = |tok: &str, s: f64| (lookup(tok, &env, w, h) * s) as f32;
        let mut acc: Vec<(f32, f32)> = Vec::new();
        let mut start: Option<(f32, f32)> = None;
        let mut i = 0;
        let mut flush = |acc: &mut Vec<(f32, f32)>, closed: bool| {
            if acc.len() > 1 {
                out.push(Poly { pts: std::mem::take(acc), closed, fill: p.fill, stroke: p.stroke });
            } else {
                acc.clear();
            }
        };
        while i < t.len() {
            match t[i] {
                "M" => {
                    flush(&mut acc, false);
                    let pt = (v(t[i + 1], sx), v(t[i + 2], sy));
                    acc.push(pt);
                    start = Some(pt);
                    i += 3;
                }
                "L" => {
                    acc.push((v(t[i + 1], sx), v(t[i + 2], sy)));
                    i += 3;
                }
                "A" => {
                    // 弧: いまの点が (stAng) の位置に乗る楕円を描く
                    let (wr, hr) = (v(t[i + 1], sx) as f64, v(t[i + 2], sy) as f64);
                    let st = rad(lookup(t[i + 3], &env, w, h));
                    let sw = rad(lookup(t[i + 4], &env, w, h));
                    let cur = *acc.last().unwrap_or(&(0.0, 0.0));
                    let (cx, cy) = (
                        cur.0 as f64 - wr * st.cos(),
                        cur.1 as f64 - hr * st.sin(),
                    );
                    let n = ((sw.abs().to_degrees() / 7.5).ceil() as usize).max(4);
                    for k in 1..=n {
                        let a = st + sw * k as f64 / n as f64;
                        acc.push(((cx + wr * a.cos()) as f32, (cy + hr * a.sin()) as f32));
                    }
                    i += 5;
                }
                "C" | "Q" => {
                    let cubic = t[i] == "C";
                    let p0 = *acc.last().unwrap_or(&(0.0, 0.0));
                    let (c1, c2, pe);
                    if cubic {
                        c1 = (v(t[i + 1], sx), v(t[i + 2], sy));
                        c2 = (v(t[i + 3], sx), v(t[i + 4], sy));
                        pe = (v(t[i + 5], sx), v(t[i + 6], sy));
                        i += 7;
                    } else {
                        let c = (v(t[i + 1], sx), v(t[i + 2], sy));
                        pe = (v(t[i + 3], sx), v(t[i + 4], sy));
                        // 2次を3次に上げる(答えは同じ曲線)
                        c1 = (
                            p0.0 + 2.0 / 3.0 * (c.0 - p0.0),
                            p0.1 + 2.0 / 3.0 * (c.1 - p0.1),
                        );
                        c2 = (
                            pe.0 + 2.0 / 3.0 * (c.0 - pe.0),
                            pe.1 + 2.0 / 3.0 * (c.1 - pe.1),
                        );
                        i += 5;
                    }
                    for k in 1..=12 {
                        let s = k as f32 / 12.0;
                        let u = 1.0 - s;
                        acc.push((
                            u * u * u * p0.0
                                + 3.0 * u * u * s * c1.0
                                + 3.0 * u * s * s * c2.0
                                + s * s * s * pe.0,
                            u * u * u * p0.1
                                + 3.0 * u * u * s * c1.1
                                + 3.0 * u * s * s * c2.1
                                + s * s * s * pe.1,
                        ));
                    }
                }
                "Z" => {
                    flush(&mut acc, true);
                    // 閉じた後の続きは、その輪郭の始点から
                    if let Some(s) = start {
                        acc.push(s);
                    }
                    i += 1;
                }
                _ => i += 1,
            }
        }
        flush(&mut acc, false);
    }
    // 箱の位置へ
    for poly in &mut out {
        for pt in &mut poly.pts {
            pt.0 += x0;
            pt.1 += y0;
        }
    }
    (!out.is_empty()).then_some(out)
}
