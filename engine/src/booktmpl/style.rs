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

use super::words;
use book::{BStyle, Borders, CellFormat, Edge, HAlign, VAlign};

/// 罫線を引く場所の記号
pub const EDGES: &[&str] = &["edge_top", "edge_bottom", "edge_left", "edge_right"];

/// この枚が使う記号を全部並べる(見張りが言葉の表と突き合わせます)
pub fn symbols() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = FIELDS.iter().map(|(_, s)| *s).collect();
    v.extend(EDGES);
    v.extend(BSTYLES.iter().map(|(_, s)| *s));
    v.extend(HALIGNS.iter().map(|(_, s)| *s));
    v.extend(VALIGNS.iter().map(|(_, s)| *s));
    v
}

/// **書式の欄と、表に書くときの項目の名前。**
///
/// 名前は Excel の「セルの書式設定」の言い方に寄せてあります。
pub const FIELDS: &[(&str, &str)] = &[
    ("bold", "bold"),
    ("italic", "italic"),
    ("underline", "underline"),
    ("strike", "strikethrough"),
    ("subscript", "subscript"),
    ("borders", "tmpl_borders"),
    ("align", "halign"),
    ("valign", "valign"),
    ("fill", "fill_color"),
    ("fill_bg", "fill_bg"),
    ("fill_pattern", "fill_pattern"),
    ("fill_grad", "gradient_2"),
    ("fill_theme", "fill_theme"),
    ("color", "font_color"),
    ("color_theme", "color_theme"),
    ("font", "tmpl_font"),
    ("size_c", "size"),
    ("rotation", "rotation_2"),
    ("rtl_text", "rtl"),
    ("wrap", "wrap"),
    ("shrink", "shrink"),
    ("indent", "indent_3"),
    ("number_format", "number_format"),
    ("unlocked", "unlocked"),
    ("formula_hidden", "hide_formula"),
];

/// 項目の名前。**組になる物は1語ずつ繋ぎます。**
///
/// 「塗りのテーマ色」のような複合語は、どの製品も1語では持たないので
/// 訳が引けません。塗り(どこに)とテーマ色(何を)は別々に訳があるので、
/// カンマで繋いで書きます(2026-08-28 発注者「塗りのテーマ色で何の問題が
/// あるの」— 問題はありませんでした。線種と同じ手で書けます)。
fn item_text(sym: &str) -> String {
    match sym {
        "fill_theme" => {
            format!("{},{}", words::text("fill_color"), words::text("theme_colors"))
        }
        "color_theme" => {
            format!("{},{}", words::text("font_color"), words::text("theme_colors"))
        }
        _ => words::text(sym).to_string(),
    }
}

/// 項目の名前(組も1語も)を記号に戻す
fn item_sym(label: &str) -> Option<&'static str> {
    if let Some((doko, nani)) = label.split_once(',') {
        if words::is("theme_colors", nani.trim()) {
            if words::is("fill_color", doko.trim()) {
                return Some("fill_theme");
            }
            if words::is("font_color", doko.trim()) {
                return Some("color_theme");
            }
        }
    }
    FIELDS.iter().find(|(_, sym)| words::is(sym, label)).map(|(k, _)| *k)
}

/// 1つの書式を (項目, 値) の並びにする。**既定のままの欄は出しません。**
pub fn to_rows(f: &CellFormat) -> Vec<(String, String)> {
    let d = CellFormat::default();
    let mut out: Vec<(String, String)> = Vec::new();
    // 項目の名前は**画面の言語**で書きます。値の中の語(線種・揃え)も同じ
    let mut put = |key: &str, v: String| {
        if let Some((_, sym)) = FIELDS.iter().find(|(k, _)| *k == key) {
            out.push((item_text(sym), v));
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
        put("align", halign_text(f.align));
    }
    if f.valign != d.valign {
        put("valign", words::text(valign_text(f.valign)).into());
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
        let Some(key) = item_sym(label) else { continue };
        let yes = v.eq_ignore_ascii_case("true");
        match key {
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
    for (label, e) in [("edge_top", &b.top), ("edge_bottom", &b.bottom), ("edge_left", &b.left), ("edge_right", &b.right)] {
        if !e.on {
            continue;
        }
        let mut s = format!("{}:{}", words::text(label), bstyle_text(e.style));
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
        #[allow(clippy::match_single_binding)]
        match () {
            _ => match words::which(EDGES, label) {
                Some("edge_top") => b.top = e,
                Some("edge_bottom") => b.bottom = e,
                Some("edge_left") => b.left = e,
                Some("edge_right") => b.right = e,
                _ => {}
            },
        }
    }
    b
}

/// 線種の名前。Excel の「線のスタイル」の言い方に寄せる
const BSTYLES: &[(BStyle, &str)] = &[
    (BStyle::Hair, "hairline"),
    (BStyle::Dotted, "dotted"),
    (BStyle::DashDotDot, "dash_dot_dot"),
    (BStyle::DashDot, "dash_dot"),
    (BStyle::Dashed, "dashed"),
    (BStyle::Thin, "thin"),
    (BStyle::MediumDashDotDot, "medium_dash_dot_dot"),
    (BStyle::MediumDashDot, "medium_dash_dot"),
    (BStyle::MediumDashed, "medium_dashed"),
    (BStyle::Medium, "medium"),
    (BStyle::Thick, "thick"),
    (BStyle::Double, "double"),
    (BStyle::SlantDashDot, "slant_dash_dot"),
];

/// 線種を**柄と修飾に分けて**書く。
///
/// 発注者「斜め一点鎖線・選択範囲内で中央、これに訳語をつけるのが
/// おかしくないですか。斜め線(方向, 線種, 配置)、こういうマクロでは
/// ないの」(2026-08-28)。そのとおりで、`slantDashDot` は Excel の
/// 型スタンプの名前です。**罫線は場所×ペンの直交モデル**という決めが
/// あるのに、ここだけ型の名前を語彙にしていました。
///
/// 柄(一点鎖線)に、修飾(斜め)をカンマで足す形にします。どちらも
/// 1語なので、どの言語にも訳があります。
fn bstyle_text(s: BStyle) -> String {
    let (gara, kazari) = bstyle_parts(s);
    match kazari {
        Some(k) => format!("{},{}", words::text(gara), words::text(k)),
        None => words::text(gara).to_string(),
    }
}

/// 線種 → (柄, 修飾)。修飾は方向(斜め)だけです。
///
/// 太さの付いた物(`medium_dashed` など)は**1語のまま**にします。
/// 出どころ(本家・LibreOffice)がその形で訳を持っていて、
/// 分けると逆に元の言い方から離れます。
fn bstyle_parts(s: BStyle) -> (&'static str, Option<&'static str>) {
    match s {
        BStyle::SlantDashDot => ("dash_dot", Some("diagonal")),
        _ => (
            BSTYLES.iter().find(|(k, _)| *k == s).map(|(_, v)| *v).unwrap_or("thin"),
            None,
        ),
    }
}

fn read_bstyle(s: &str) -> BStyle {
    // **柄と修飾に分かれた形を先に見ます。** 分かれていなければ、
    // 今までどおり1語として引きます(古いテンプレートもこれで読めます)
    if let Some((gara, kazari)) = s.split_once(',') {
        let gara = read_bstyle_1(gara.trim());
        if words::is("diagonal", kazari.trim()) && gara == BStyle::DashDot {
            return BStyle::SlantDashDot;
        }
        return gara;
    }
    read_bstyle_1(s)
}

fn read_bstyle_1(s: &str) -> BStyle {
    BSTYLES.iter().find(|(_, sym)| words::is(sym, s)).map(|(k, _)| *k).unwrap_or(BStyle::Thin)
}

const HALIGNS: &[(HAlign, &str)] = &[
    (HAlign::General, "align_general"),
    (HAlign::Left, "left"),
    (HAlign::Center, "center"),
    (HAlign::Right, "right"),
    (HAlign::Justify, "justify"),
    (HAlign::CenterContinuous, "center_across"),
    (HAlign::Distribute, "distributed"),
];

/// 横位置。**「選択範囲内で中央」も分けて書きます** — 中央(揃え)に
/// 範囲(効く先)をカンマで足した形です。線種と同じ考えです
fn halign_text(a: HAlign) -> String {
    match a {
        HAlign::CenterContinuous => {
            format!("{},{}", words::text("center"), words::text("selection"))
        }
        _ => words::text(
            HALIGNS.iter().find(|(k, _)| *k == a).map(|(_, v)| *v).unwrap_or("align_general"),
        )
        .to_string(),
    }
}

fn read_halign(s: &str) -> HAlign {
    if let Some((yose, saki)) = s.split_once(',') {
        let yose = read_halign_1(yose.trim());
        if words::is("selection", saki.trim()) && yose == HAlign::Center {
            return HAlign::CenterContinuous;
        }
        return yose;
    }
    read_halign_1(s)
}

fn read_halign_1(s: &str) -> HAlign {
    HALIGNS.iter().find(|(_, sym)| words::is(sym, s)).map(|(k, _)| *k).unwrap_or(HAlign::General)
}

const VALIGNS: &[(VAlign, &str)] =
    &[(VAlign::Top, "top"), (VAlign::Middle, "center"), (VAlign::Bottom, "bottom"), (VAlign::Distribute, "distributed")];

fn valign_text(a: VAlign) -> &'static str {
    VALIGNS.iter().find(|(k, _)| *k == a).map(|(_, v)| *v).unwrap_or("bottom")
}

fn read_valign(s: &str) -> VAlign {
    VALIGNS.iter().find(|(_, sym)| words::is(sym, s)).map(|(k, _)| *k).unwrap_or(VAlign::Bottom)
}

/// テーマ色は `番号,明るさの加減` で書きます(`4,400` = アクセント1 を +0.4)
fn read_theme_color(s: &str) -> Option<(u8, i32)> {
    let (i, tint) = s.split_once(',')?;
    Some((i.trim().parse().ok()?, tint.trim().parse().ok()?))
}

/// グラデーションは `[道] 角度 位置:色 位置:色 …`
fn read_grad(s: &str) -> Option<book::Gradient> {
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
    Some(book::Gradient {
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
        let src = include_str!("../../../book/src/types.rs");
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

#[cfg(test)]
mod chokkou_tests {
    use super::{bstyle_text, halign_text, read_bstyle, read_halign};
    use book::{BStyle, HAlign};

    /// **型スタンプの名前を語彙にしない。**
    ///
    /// 発注者「斜め線(方向, 線種, 配置)、こういうマクロではないの」
    /// (2026-08-28)。`slantDashDot` は Excel の型の名前で、意味は
    /// 「一点鎖線を斜めに」です。1語ずつの組で書き、読み返せることを見ます。
    #[test]
    fn a_slanted_line_is_written_as_a_pattern_and_a_direction() {
        let _lang = crate::font::lang_lock();
        crate::font::set_default_language("ja");
        let t = bstyle_text(BStyle::SlantDashDot);
        assert_eq!(t, "一点鎖線,斜め", "型の名前のままです: {t}");
        assert_eq!(read_bstyle(&t), BStyle::SlantDashDot, "読み返せない");
    }

    #[test]
    fn centre_across_a_selection_is_written_the_same_way() {
        let _lang = crate::font::lang_lock();
        crate::font::set_default_language("ja");
        let t = halign_text(HAlign::CenterContinuous);
        assert_eq!(t, "中央,選択範囲", "{t}");
        assert_eq!(read_halign(&t), HAlign::CenterContinuous, "読み返せない");
    }

    /// 修飾の付かない線種と揃えは、今までどおり1語です
    #[test]
    fn a_plain_pattern_stays_one_word() {
        let _lang = crate::font::lang_lock();
        crate::font::set_default_language("ja");
        assert_eq!(bstyle_text(BStyle::DashDot), "一点鎖線");
        assert_eq!(read_bstyle("一点鎖線"), BStyle::DashDot);
        assert_eq!(halign_text(HAlign::Center), "中央");
    }

    /// **古いテンプレート(1語の型の名前)も読めます**
    #[test]
    fn the_old_stamp_names_still_read() {
        let _lang = crate::font::lang_lock();
        crate::font::set_default_language("ja");
        assert_eq!(read_bstyle("斜め一点鎖線"), BStyle::SlantDashDot);
        assert_eq!(read_halign("選択範囲内で中央"), HAlign::CenterContinuous);
    }

    /// **テーマ色も組で書きます。** 「塗りのテーマ色」は1語では
    /// 引けませんが、塗りとテーマ色は別々に訳があります
    #[test]
    fn a_theme_colour_is_written_as_where_and_what() {
        let _lang = crate::font::lang_lock();
        crate::font::set_default_language("ja");
        let mut f = book::CellFormat::default();
        f.fill_theme = Some((4, 200));
        f.color_theme = Some((1, 0));
        let rows = super::to_rows(&f);
        let items: Vec<&str> = rows.iter().map(|(k, _)| k.as_str()).collect();
        assert!(items.contains(&"塗り,テーマ色"), "{items:?}");
        assert!(items.contains(&"文字色,テーマ色"), "{items:?}");
        // 読み返せる
        let back = super::from_rows(&rows);
        assert_eq!(back.fill_theme, Some((4, 200)), "塗りのテーマ色が戻らない");
        assert_eq!(back.color_theme, Some((1, 0)), "文字のテーマ色が戻らない");
    }

    /// **色とテーマ由来は両方持てます。** 行が別なので、片方が消えません
    #[test]
    fn a_colour_and_its_theme_origin_both_survive() {
        let _lang = crate::font::lang_lock();
        crate::font::set_default_language("ja");
        let mut f = book::CellFormat::default();
        f.fill = Some("FFF2CC".into());
        f.fill_theme = Some((4, 200));
        let back = super::from_rows(&super::to_rows(&f));
        assert_eq!(back.fill.as_deref(), Some("FFF2CC"), "塗りの色が消えた");
        assert_eq!(back.fill_theme, Some((4, 200)), "テーマ由来が消えた");
    }

    /// 古いテンプレート(1語の「塗りのテーマ色」)も読めます
    #[test]
    fn the_old_one_word_theme_colour_still_reads() {
        let _lang = crate::font::lang_lock();
        crate::font::set_default_language("ja");
        let rows = vec![("塗りのテーマ色".to_string(), "4,200".to_string())];
        assert_eq!(super::from_rows(&rows).fill_theme, Some((4, 200)));
    }

    /// どの言語でも組で書ける(訳の無い語に落ちない)
    #[test]
    fn every_language_writes_both_parts() {
        let _lang = crate::font::lang_lock();
        for l in crate::booktmpl::words::LANGS {
            crate::font::set_default_language(l);
            let t = bstyle_text(BStyle::SlantDashDot);
            assert!(t.contains(','), "{l}: 組になっていない: {t}");
            assert_eq!(read_bstyle(&t), BStyle::SlantDashDot, "{l}: 読み返せない: {t}");
            let a = halign_text(HAlign::CenterContinuous);
            assert_eq!(read_halign(&a), HAlign::CenterContinuous, "{l}: 読み返せない: {a}");
            let mut f = book::CellFormat::default();
            f.fill_theme = Some((4, 200));
            let rows = super::to_rows(&f);
            assert!(rows[0].0.contains(','), "{l}: テーマ色が組になっていない: {:?}", rows[0].0);
            assert_eq!(super::from_rows(&rows).fill_theme, Some((4, 200)), "{l}: 読み返せない");
        }
        crate::font::set_default_language("ja");
    }
}
