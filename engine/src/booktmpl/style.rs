//! **セルの書式をテンプレートの表で持つ。**
//!
//! `.sheet.adoc` は意味だけを持ちます。太字も塗りも罫線も見た目なので、
//! 隣の `.tmpl.adoc` の持ち場です(2026-08-18 発注者)。
//!
//! # 型スタンプを作らない
//!
//! セル1つに1行を書く形にはしません。**同じ書式は1つにまとめて名前を付け、
//! 範囲でセルに当てます**(SEKKEI「書式は数でなく条件で止める」)。実物の
//! ブックは書式の種類が数十で、当たっているセルが数千という形をしています。
//!
//! # 縦長の (名前, 項目, 値)
//!
//! [`CellFormat`] の欄は 25 あります。横に 25 列並べると読めないので、
//! **設定した項目だけを1行ずつ**書きます。
//!
//! ....
//! .書式
//! |===
//! |名前 |項目 |値
//!
//! |見出し |太字 |true
//! |見出し |塗り |4472C4
//! |金額 |表示形式 |#,##0
//! |===
//! ....
//!
//! # 見ていない欄は言う
//!
//! [`FIELDS`] に載っていない欄は往復しません。**載せ忘れが黙って落ちない
//! ように**、`every_format_field_is_carried` が `types.rs` と突き合わせます。

use crate::book::{BStyle, Borders, CellFormat, Edge, HAlign, VAlign};

/// **書式の欄と、表に書くときの項目の名前。**
///
/// 名前は Excel の「セルの書式設定」の言い方に寄せてあります。
pub const FIELDS: &[(&str, &str)] = &[
    ("bold", "太字"),
    ("italic", "斜体"),
    ("underline", "下線"),
    ("strike", "取り消し線"),
    ("subscript", "下付き"),
    ("borders", "罫線"),
    ("align", "横位置"),
    ("valign", "縦位置"),
    ("fill", "塗り"),
    ("fill_bg", "塗りの地"),
    ("fill_pattern", "塗りの柄"),
    ("fill_grad", "グラデーション"),
    ("fill_theme", "塗りのテーマ色"),
    ("color", "文字色"),
    ("color_theme", "文字のテーマ色"),
    ("font", "書体"),
    ("size_c", "大きさ"),
    ("rotation", "回転"),
    ("rtl_text", "右横書き"),
    ("wrap", "折り返し"),
    ("shrink", "縮小"),
    ("indent", "字下げ"),
    ("number_format", "表示形式"),
    ("unlocked", "ロック解除"),
    ("formula_hidden", "式を隠す"),
];

/// 1つの書式を (項目, 値) の並びにする。**既定のままの欄は出しません。**
pub fn to_rows(f: &CellFormat) -> Vec<(&'static str, String)> {
    let d = CellFormat::default();
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let mut put = |key: &str, v: String| {
        if let Some((_, label)) = FIELDS.iter().find(|(k, _)| *k == key) {
            out.push((label, v));
        }
    };
    if f.bold != d.bold {
        put("bold", f.bold.to_string());
    }
    if f.italic != d.italic {
        put("italic", f.italic.to_string());
    }
    if f.underline != d.underline {
        put("underline", f.underline.to_string());
    }
    if f.strike != d.strike {
        put("strike", f.strike.to_string());
    }
    if f.subscript != d.subscript {
        put("subscript", f.subscript.to_string());
    }
    if f.borders != d.borders {
        put("borders", borders_text(&f.borders));
    }
    if f.align != d.align {
        put("align", halign_text(f.align).into());
    }
    if f.valign != d.valign {
        put("valign", valign_text(f.valign).into());
    }
    for (key, v) in [
        ("fill", &f.fill),
        ("fill_bg", &f.fill_bg),
        ("fill_pattern", &f.fill_pattern),
        ("color", &f.color),
        ("font", &f.font),
        ("number_format", &f.number_format),
    ] {
        if let Some(x) = v {
            put(key, x.clone());
        }
    }
    for (key, v) in [("fill_theme", f.fill_theme), ("color_theme", f.color_theme)] {
        if let Some((i, tint)) = v {
            put(key, format!("{i},{tint}"));
        }
    }
    if let Some(g) = &f.fill_grad {
        let stops: Vec<String> =
            g.stops.iter().map(|(at, c)| format!("{}:{c}", *at as f32 / 1000.0)).collect();
        let mut s = format!("{} {}", g.degree_c as f32 / 60000.0, stops.join(" "));
        if let Some(p) = &g.path {
            s = format!("{p} {s}");
        }
        put("fill_grad", s);
    }
    if let Some(v) = f.size_c {
        put("size_c", numbers(v as f32 / 100.0));
    }
    if let Some(v) = f.rotation {
        put("rotation", v.to_string());
    }
    if f.rtl_text != d.rtl_text {
        put("rtl_text", f.rtl_text.to_string());
    }
    if f.wrap != d.wrap {
        put("wrap", f.wrap.to_string());
    }
    if f.shrink != d.shrink {
        put("shrink", f.shrink.to_string());
    }
    if f.indent != d.indent {
        put("indent", f.indent.to_string());
    }
    if f.unlocked != d.unlocked {
        put("unlocked", f.unlocked.to_string());
    }
    if f.formula_hidden != d.formula_hidden {
        put("formula_hidden", f.formula_hidden.to_string());
    }
    out
}

/// (項目, 値) の並びから書式を組み立てる。**知らない項目は飛ばします。**
pub fn from_rows(rows: &[(String, String)]) -> CellFormat {
    let mut f = CellFormat::default();
    for (label, v) in rows {
        let Some((key, _)) = FIELDS.iter().find(|(_, l)| l == label) else { continue };
        let yes = v.eq_ignore_ascii_case("true");
        match *key {
            "bold" => f.bold = yes,
            "italic" => f.italic = yes,
            "underline" => f.underline = yes,
            "strike" => f.strike = yes,
            "subscript" => f.subscript = yes,
            "rtl_text" => f.rtl_text = yes,
            "wrap" => f.wrap = yes,
            "shrink" => f.shrink = yes,
            "unlocked" => f.unlocked = yes,
            "formula_hidden" => f.formula_hidden = yes,
            "borders" => f.borders = read_borders(v),
            "align" => f.align = read_halign(v),
            "valign" => f.valign = read_valign(v),
            "fill" => f.fill = Some(v.clone()),
            "fill_bg" => f.fill_bg = Some(v.clone()),
            "fill_pattern" => f.fill_pattern = Some(v.clone()),
            "color" => f.color = Some(v.clone()),
            "font" => f.font = Some(v.clone()),
            "number_format" => f.number_format = Some(v.clone()),
            "fill_theme" => f.fill_theme = read_theme_color(v),
            "color_theme" => f.color_theme = read_theme_color(v),
            "fill_grad" => f.fill_grad = read_grad(v),
            "size_c" => {
                if let Ok(pt) = v.parse::<f32>() {
                    f.size_c = Some((pt * 100.0).round() as u32);
                }
            }
            "rotation" => f.rotation = v.parse().ok(),
            "indent" => f.indent = v.parse().unwrap_or(0),
            _ => {}
        }
    }
    f
}

/// 罫線。**場所×ペン**で書きます(`上:細 下:太`)。
/// 型スタンプ(「格子」「外枠」)は作りません — 場所と線種の直交が家の決めです。
fn borders_text(b: &Borders) -> String {
    let mut out: Vec<String> = Vec::new();
    for (label, e) in [("上", &b.top), ("下", &b.bottom), ("左", &b.left), ("右", &b.right)] {
        if !e.on {
            continue;
        }
        let mut s = format!("{label}:{}", bstyle_text(e.style));
        if let Some(c) = e.color {
            s.push_str(&format!(":{c:06X}"));
        }
        out.push(s);
    }
    out.join(" ")
}

fn read_borders(s: &str) -> Borders {
    let mut b = Borders::default();
    for part in s.split_whitespace() {
        let mut it = part.split(':');
        let Some(label) = it.next() else { continue };
        let style = it.next().map(read_bstyle).unwrap_or(BStyle::Thin);
        let color = it.next().and_then(|c| u32::from_str_radix(c, 16).ok());
        let e = Edge { on: true, style, color };
        match label {
            "上" => b.top = e,
            "下" => b.bottom = e,
            "左" => b.left = e,
            "右" => b.right = e,
            _ => {}
        }
    }
    b
}

/// 線種の名前。Excel の「線のスタイル」の言い方に寄せる
const BSTYLES: &[(BStyle, &str)] = &[
    (BStyle::Hair, "極細"),
    (BStyle::Dotted, "点線"),
    (BStyle::DashDotDot, "一点二鎖線"),
    (BStyle::DashDot, "一点鎖線"),
    (BStyle::Dashed, "破線"),
    (BStyle::Thin, "細"),
    (BStyle::MediumDashDotDot, "中一点二鎖線"),
    (BStyle::MediumDashDot, "中一点鎖線"),
    (BStyle::MediumDashed, "中破線"),
    (BStyle::Medium, "中"),
    (BStyle::Thick, "太"),
    (BStyle::Double, "二重"),
    (BStyle::SlantDashDot, "斜め一点鎖線"),
];

fn bstyle_text(s: BStyle) -> &'static str {
    BSTYLES.iter().find(|(k, _)| *k == s).map(|(_, v)| *v).unwrap_or("細")
}

fn read_bstyle(s: &str) -> BStyle {
    BSTYLES.iter().find(|(_, v)| *v == s).map(|(k, _)| *k).unwrap_or(BStyle::Thin)
}

const HALIGNS: &[(HAlign, &str)] = &[
    (HAlign::General, "標準"),
    (HAlign::Left, "左"),
    (HAlign::Center, "中央"),
    (HAlign::Right, "右"),
    (HAlign::Justify, "両端"),
    (HAlign::CenterContinuous, "選択範囲内で中央"),
    (HAlign::Distribute, "均等割付"),
];

fn halign_text(a: HAlign) -> &'static str {
    HALIGNS.iter().find(|(k, _)| *k == a).map(|(_, v)| *v).unwrap_or("標準")
}

fn read_halign(s: &str) -> HAlign {
    HALIGNS.iter().find(|(_, v)| *v == s).map(|(k, _)| *k).unwrap_or(HAlign::General)
}

const VALIGNS: &[(VAlign, &str)] =
    &[(VAlign::Top, "上"), (VAlign::Middle, "中央"), (VAlign::Bottom, "下"), (VAlign::Distribute, "均等割付")];

fn valign_text(a: VAlign) -> &'static str {
    VALIGNS.iter().find(|(k, _)| *k == a).map(|(_, v)| *v).unwrap_or("下")
}

fn read_valign(s: &str) -> VAlign {
    VALIGNS.iter().find(|(_, v)| *v == s).map(|(k, _)| *k).unwrap_or(VAlign::Bottom)
}

/// テーマ色は `番号,明るさの加減` で書きます(`4,400` = アクセント1 を +0.4)
fn read_theme_color(s: &str) -> Option<(u8, i32)> {
    let (i, tint) = s.split_once(',')?;
    Some((i.trim().parse().ok()?, tint.trim().parse().ok()?))
}

/// グラデーションは `[道] 角度 位置:色 位置:色 …`
fn read_grad(s: &str) -> Option<crate::book::Gradient> {
    let mut it = s.split_whitespace().peekable();
    let mut path = None;
    if it.peek().is_some_and(|x| x.parse::<f32>().is_err()) {
        path = it.next().map(|x| x.to_string());
    }
    let degree: f32 = it.next()?.parse().ok()?;
    let stops: Vec<(u32, String)> = it
        .filter_map(|x| {
            let (at, c) = x.split_once(':')?;
            let at: f32 = at.parse().ok()?;
            Some(((at * 1000.0).round() as u32, c.to_string()))
        })
        .collect();
    Some(crate::book::Gradient {
        degree_c: (degree * 60000.0).round() as i32,
        stops,
        path,
    })
}

/// 数を字にする(整数はそのまま、小数は要るぶんだけ)
fn numbers(v: f32) -> String {
    if (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **書式の欄が全部運べるか。** `types.rs` と突き合わせます。
    /// 欄を足して [`FIELDS`] に足し忘れると、その書式は往復しません。
    #[test]
    fn every_format_field_is_carried() {
        let src = include_str!("../book/types.rs");
        let head = "pub struct CellFormat {";
        let from = src.find(head).expect("CellFormat が無い");
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
                "CellFormat の欄「{f}」が style::FIELDS に無い。\
                 足すとテンプレートで往復しません"
            );
        }
        for (k, _) in FIELDS {
            assert!(fields.contains(k), "style::FIELDS の「{k}」が CellFormat に無い");
        }
    }

    #[test]
    fn the_item_names_do_not_repeat() {
        for (i, (_, a)) in FIELDS.iter().enumerate() {
            for (_, b) in &FIELDS[i + 1..] {
                assert_ne!(a, b, "同じ項目の名前が2つある: 「{a}」");
            }
        }
    }
}
