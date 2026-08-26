//! **図形を表で持つ。**
//!
//! 図形の持ち物は 19 あります(`kind` `points` `text_fmt` `spark_marks` …)。
//! 横に 19 列並べると読めないので、書式と同じ**縦長の (名前, 項目, 値)**
//! にします。設定した項目だけが1行ずつ出ます。
//!
//! # SVG にはしません
//!
//! 絵にしてしまうと、頂点も色も文字も**模型が失われて編集に戻せません**。
//! 図形は構造を持ったデータなので、構造のまま書きます。
//!
//! # 見ていない欄は言う
//!
//! [`FIELDS`] に載っていない欄は往復しません。`every_shape_field_is_carried`
//! が `types.rs` と突き合わせます。

use book::{HAlign, PathPoint, Pos, SheetShape, TextAnchor};

/// **図形の欄と、表に書くときの項目の名前。** 名前は英語の識別子です
/// — 書式の一部であって、画面に出る字ではありません。
pub const FIELDS: &[(&str, &str)] = &[
    ("at", "at"),
    ("kind", "kind"),
    ("width_px", "width"),
    ("height_px", "height"),
    ("dx_px", "dx"),
    ("dy_px", "dy"),
    ("fill", "fill"),
    ("line", "line"),
    ("line_w", "line-width"),
    ("alpha", "alpha"),
    ("shadow", "shadow"),
    ("rot", "rotation"),
    ("flip_h", "flip-h"),
    ("flip_v", "flip-v"),
    ("base", "base"),
    ("text", "text"),
    ("text_fmt", "text-format"),
    ("spark_marks", "spark-marks"),
    ("points", "points"),
];

/// 1つの図形を (項目, 値) の並びにする。**既定のままの欄は出しません。**
pub fn to_rows(s: &SheetShape) -> Vec<(&'static str, String)> {
    let d = SheetShape::default();
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let mut put = |key: &str, v: String| {
        if let Some((_, label)) = FIELDS.iter().find(|(k, _)| *k == key) {
            out.push((*label, v));
        }
    };
    // 場所と大きさと種類は既定でも必ず書きます — 無いと図形になりません
    put("at", s.at.a1());
    put("kind", s.kind.clone());
    put("width_px", num(s.width_px));
    put("height_px", num(s.height_px));

    for (k, v, dv) in [
        ("dx_px", s.dx_px, d.dx_px),
        ("dy_px", s.dy_px, d.dy_px),
        ("line_w", s.line_w, d.line_w),
        ("alpha", s.alpha, d.alpha),
        ("rot", s.rot, d.rot),
        ("base", s.base, d.base),
    ] {
        if (v - dv).abs() > f32::EPSILON {
            put(k, num(v));
        }
    }
    for (k, v, dv) in [
        ("shadow", s.shadow, d.shadow),
        ("flip_h", s.flip_h, d.flip_h),
        ("flip_v", s.flip_v, d.flip_v),
    ] {
        if v != dv {
            put(k, v.to_string());
        }
    }
    for (k, v) in [("fill", &s.fill), ("line", &s.line), ("text", &s.text)] {
        if let Some(x) = v {
            put(k, x.clone());
        }
    }
    if s.text_fmt != d.text_fmt {
        put("text_fmt", text_fmt(&s.text_fmt));
    }
    if s.spark_marks != d.spark_marks {
        put("spark_marks", spark(&s.spark_marks));
    }
    if !s.points.is_empty() {
        put("points", points(&s.points));
    }
    out
}

/// (項目, 値) の並びから図形を組み立てる。**知らない項目は飛ばします。**
pub fn from_rows(rows: &[(String, String)]) -> SheetShape {
    let mut s = SheetShape::default();
    for (label, v) in rows {
        let Some((key, _)) = FIELDS.iter().find(|(_, l)| l == label) else { continue };
        let yes = v.eq_ignore_ascii_case("true");
        match *key {
            "at" => {
                if let Some(p) = Pos::parse(v) {
                    s.at = p;
                }
            }
            "kind" => s.kind = v.clone(),
            "width_px" => s.width_px = v.parse().unwrap_or(s.width_px),
            "height_px" => s.height_px = v.parse().unwrap_or(s.height_px),
            "dx_px" => s.dx_px = v.parse().unwrap_or(s.dx_px),
            "dy_px" => s.dy_px = v.parse().unwrap_or(s.dy_px),
            "line_w" => s.line_w = v.parse().unwrap_or(s.line_w),
            "alpha" => s.alpha = v.parse().unwrap_or(s.alpha),
            "rot" => s.rot = v.parse().unwrap_or(s.rot),
            "base" => s.base = v.parse().unwrap_or(s.base),
            "shadow" => s.shadow = yes,
            "flip_h" => s.flip_h = yes,
            "flip_v" => s.flip_v = yes,
            "fill" => s.fill = Some(v.clone()),
            "line" => s.line = Some(v.clone()),
            "text" => s.text = Some(v.clone()),
            "text_fmt" => s.text_fmt = read_text_fmt(v),
            "spark_marks" => s.spark_marks = read_spark(v),
            "points" => s.points = read_points(v),
            _ => {}
        }
    }
    s
}

const ANCHORS: &[(TextAnchor, &str)] =
    &[(TextAnchor::Top, "top"), (TextAnchor::Middle, "middle"), (TextAnchor::Bottom, "bottom")];

const ALIGNS: &[(HAlign, &str)] = &[
    (HAlign::General, "general"), (HAlign::Left, "left"), (HAlign::Center, "center"),
    (HAlign::Right, "right"), (HAlign::Justify, "justify"),
    (HAlign::CenterContinuous, "center-across"), (HAlign::Distribute, "distributed"),
];

/// 文字の組み方。`align=center anchor=middle vertical=true` の形
fn text_fmt(f: &book::TextFmt) -> String {
    let d = book::TextFmt::default();
    let mut out: Vec<String> = Vec::new();
    if f.align != d.align {
        out.push(format!("align={}", pick(ALIGNS, f.align, "general")));
    }
    if f.anchor != d.anchor {
        out.push(format!("anchor={}", pick(ANCHORS, f.anchor, "top")));
    }
    for (k, v, dv) in [
        ("vertical", f.vertical, d.vertical),
        ("strike", f.strike, d.strike),
        ("sup", f.sup, d.sup),
        ("sub", f.sub, d.sub),
    ] {
        if v != dv {
            out.push(format!("{k}={v}"));
        }
    }
    if let Some(b) = f.bullet {
        out.push(format!("bullet={b}"));
    }
    out.join(" ")
}

fn read_text_fmt(s: &str) -> book::TextFmt {
    let mut f = book::TextFmt::default();
    for part in s.split_whitespace() {
        let Some((k, v)) = part.split_once('=') else { continue };
        let yes = v.eq_ignore_ascii_case("true");
        match k {
            "align" => f.align = find(ALIGNS, v, HAlign::General),
            "anchor" => f.anchor = find(ANCHORS, v, TextAnchor::Top),
            "vertical" => f.vertical = yes,
            "strike" => f.strike = yes,
            "sup" => f.sup = yes,
            "sub" => f.sub = yes,
            "bullet" => f.bullet = Some(yes),
            _ => {}
        }
    }
    f
}

/// スパークラインの点の印。入っている物の名前を並べます
fn spark(m: &book::SparkMarks) -> String {
    [("high", m.high), ("low", m.low), ("first", m.first), ("last", m.last),
     ("negative", m.negative)]
        .iter()
        .filter(|(_, on)| *on)
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_spark(s: &str) -> book::SparkMarks {
    let mut m = book::SparkMarks::default();
    for w in s.split_whitespace() {
        match w {
            "high" => m.high = true,
            "low" => m.low = true,
            "first" => m.first = true,
            "last" => m.last = true,
            "negative" => m.negative = true,
            _ => {}
        }
    }
    m
}

/// 折れ線と曲線の頂点。`M12,34` が始まり、`12,34` が続き、
/// 制御点は `<` が前、`>` が後ろです
fn points(v: &[PathPoint]) -> String {
    v.iter()
        .map(|p| {
            let mut s = String::new();
            if p.start {
                s.push('M');
            }
            s.push_str(&format!("{},{}", num(p.at.0), num(p.at.1)));
            if let Some(c) = p.c_in {
                s.push_str(&format!("<{},{}", num(c.0), num(c.1)));
            }
            if let Some(c) = p.c_out {
                s.push_str(&format!(">{},{}", num(c.0), num(c.1)));
            }
            s
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_points(s: &str) -> Vec<PathPoint> {
    s.split_whitespace()
        .filter_map(|w| {
            let (start, rest) = match w.strip_prefix('M') {
                Some(r) => (true, r),
                None => (false, w),
            };
            let (head, c_out) = split_at_mark(rest, '>');
            let (at, c_in) = split_at_mark(head, '<');
            Some(PathPoint {
                at: pair(at)?,
                start,
                c_in: c_in.and_then(pair),
                c_out: c_out.and_then(pair),
            })
        })
        .collect()
}

fn split_at_mark(s: &str, m: char) -> (&str, Option<&str>) {
    match s.split_once(m) {
        Some((a, b)) => (a, Some(b)),
        None => (s, None),
    }
}

fn pair(s: &str) -> Option<(f32, f32)> {
    let (a, b) = s.split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

fn pick<T: PartialEq + Copy>(table: &[(T, &'static str)], v: T, dv: &'static str) -> &'static str {
    table.iter().find(|(k, _)| *k == v).map(|(_, s)| *s).unwrap_or(dv)
}

fn find<T: Copy>(table: &[(T, &'static str)], s: &str, dv: T) -> T {
    table.iter().find(|(_, n)| *n == s).map(|(k, _)| *k).unwrap_or(dv)
}

/// 数を字にする(整数はそのまま、小数は要るぶんだけ)
fn num(v: f32) -> String {
    if (v - v.round()).abs() < 0.0005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **図形の欄が全部運べるか。** `types.rs` と突き合わせます。
    #[test]
    fn every_shape_field_is_carried() {
        let src = include_str!("../../../book/src/types.rs");
        let head = "pub struct SheetShape {";
        let from = src.find(head).expect("SheetShape が無い");
        let body = &src[from + head.len()..];
        let to = body.find("\n}").expect("終わりが無い");
        let fields: Vec<&str> = body[..to]
            .lines()
            .filter_map(|l| l.trim().strip_prefix("pub "))
            .filter_map(|l| l.split(':').next())
            .map(|s| s.trim())
            .collect();
        for f in &fields {
            assert!(
                FIELDS.iter().any(|(k, _)| k == f),
                "SheetShape の欄「{f}」が shape::FIELDS に無い。往復しません"
            );
        }
        for (k, _) in FIELDS {
            assert!(fields.contains(k), "shape::FIELDS の「{k}」が SheetShape に無い");
        }
    }

    #[test]
    fn the_item_names_are_ascii_and_unique() {
        for (i, (_, a)) in FIELDS.iter().enumerate() {
            assert!(a.is_ascii(), "項目「{a}」が英語ではありません");
            for (_, b) in &FIELDS[i + 1..] {
                assert_ne!(a, b, "同じ項目の名前が2つある: 「{a}」");
            }
        }
    }
}
