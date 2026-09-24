//! テーマ色(`xl/theme/theme1.xml` の色の組)。
//!
//! xlsx のセルは色を `rgb="FF4472C4"` と直に書くほかに、
//! **テーマの何番目** + **明るさの加減(tint)** で書ける
//! (`<color theme="4" tint="0.4"/>`)。これを読めないと、
//! 今どきの Excel で作った帳票の色が画面から消える — だから読む。
//!
//! 番号の並びは Excel の `theme=` の流儀:
//! 0=背景1 1=文字1 2=背景2 3=文字2 4〜9=アクセント1〜6 10=リンク 11=既読リンク。
//! (theme1.xml の中の並びは dk1,lt1,dk2,lt2,… なので読むときに入れ替える)

/// 既定(Office)の色の組。テーマの部品が無いブックはこれを使う
pub const OFFICE: [&str; 12] = [
    "FFFFFF", "000000", "E7E6E6", "44546A", "4472C4", "ED7D31", "A5A5A5", "FFC000",
    "5B9BD5", "70AD47", "0563C1", "954F72",
];

/// 名前つきの色の組(配色の変更で選ぶ)。名前は Euro-Office の言い方に寄せた
pub const SCHEMES: &[(&str, [&str; 12])] = &[
    ("Office", OFFICE),
    (
        "warm",
        [
            "FFFFFF", "000000", "F5EDE6", "6B4A2F", "C0504D", "E36C0A", "D99694", "F0A22E",
            "E8B04B", "9C6644", "9E3A26", "7F3F2E",
        ],
    ),
    (
        "cool",
        [
            "FFFFFF", "000000", "E8EEF4", "1F3864", "2E75B6", "41A5B5", "8FAADC", "70AD47",
            "4472C4", "255E91", "1F4E79", "3B5F8A",
        ],
    ),
    (
        "ink",
        [
            "FFFFFF", "000000", "EDEDED", "3B3B3B", "595959", "808080", "A6A6A6", "BFBFBF",
            "404040", "737373", "1B6E3C", "5A5A5A",
        ],
    ),
];

/// テーマの番号と明るさの加減から実際の色を出す。
/// tint は ECMA-376 の作法: 負なら暗く、正なら明るく(明度だけ動かす)。
pub fn resolve(colors: &[String], idx: u8, tint: f32) -> String {
    let base = colors
        .get(idx as usize)
        .cloned()
        .unwrap_or_else(|| OFFICE.get(idx as usize).unwrap_or(&"000000").to_string());
    if tint.abs() < 0.001 {
        return base;
    }
    let g = |i: usize| {
        u8::from_str_radix(base.get(i * 2..i * 2 + 2).unwrap_or("00"), 16).unwrap_or(0) as f32
            / 255.0
    };
    let (r, gg, b) = (g(0), g(1), g(2));
    let (h, s, l) = rgb_to_hsl(r, gg, b);
    let l2 = if tint < 0.0 {
        l * (1.0 + tint)
    } else {
        l * (1.0 - tint) + tint
    };
    let (r2, g2, b2) = hsl_to_rgb(h, s, l2.clamp(0.0, 1.0));
    format!(
        "{:02X}{:02X}{:02X}",
        (r2 * 255.0).round() as u8,
        (g2 * 255.0).round() as u8,
        (b2 * 255.0).round() as u8
    )
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < 1e-6 {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

/// **The theme index of a DrawingML scheme color** (ECMA-376 20.1.10.54
/// ST_SchemeColorVal), in the order of [`resolve`]. `bg1`/`tx1`/`bg2`/`tx2`
/// are the SpreadsheetML names for lt1/dk1/lt2/dk2. None for `phClr` (the
/// placeholder a style fills in) and unknown names.
pub fn scheme_index(val: &str) -> Option<u8> {
    Some(match val {
        "lt1" | "bg1" => 0,
        "dk1" | "tx1" => 1,
        "lt2" | "bg2" => 2,
        "dk2" | "tx2" => 3,
        "accent1" => 4,
        "accent2" => 5,
        "accent3" => 6,
        "accent4" => 7,
        "accent5" => 8,
        "accent6" => 9,
        "hlink" => 10,
        "folHlink" => 11,
        _ => return None,
    })
}

/// **A DrawingML color with its transforms applied** (ECMA-376 20.1.2.3),
/// `RRGGBB` in and out. `mods` are the child elements in order, with their
/// `val` as a fraction (50000 → 0.5).
///
/// - `shade` / `tint` mix with black / white in linear light: the spec's
///   examples give 00FF00 → 00BC00 for a 50% shade and BCFFBC for a 50% tint.
/// - `lumMod` / `lumOff` scale / shift the HSL luminance of the sRGB color:
///   the spec's example for a -20% lumOff (00FF00 → 009900) comes out this
///   way. Its example for a 50% lumMod (00FF00 → 007500) does not come out
///   of either reading (HSL gives 008000); HSL is used.
/// - Other transforms are left out.
pub fn dml_color(base: &str, mods: &[(&str, f32)]) -> String {
    let ch = |i: usize| {
        u8::from_str_radix(base.get(i * 2..i * 2 + 2).unwrap_or("00"), 16).unwrap_or(0) as f32 / 255.0
    };
    let (mut r, mut g, mut b) = (ch(0), ch(1), ch(2));
    let lin = |c: f32| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    let srgb = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
    };
    for (name, v) in mods {
        match *name {
            "shade" => {
                (r, g, b) = (srgb(lin(r) * v), srgb(lin(g) * v), srgb(lin(b) * v));
            }
            "tint" => {
                let t = |c: f32| srgb(lin(c) * v + (1.0 - v));
                (r, g, b) = (t(r), t(g), t(b));
            }
            "lumMod" | "lumOff" => {
                let (h, s, l) = rgb_to_hsl(r, g, b);
                let l = if *name == "lumMod" { l * v } else { l + v };
                (r, g, b) = hsl_to_rgb(h, s, l.clamp(0.0, 1.0));
            }
            _ => {}
        }
    }
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("{:02X}{:02X}{:02X}", byte(r), byte(g), byte(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawingml_transforms_follow_the_spec_examples() {
        // ECMA-376 20.1.2.3.31 shade, 20.1.2.3.34 tint, 20.1.2.3.21 lumOff
        assert_eq!(dml_color("00FF00", &[("shade", 0.5)]), "00BC00");
        assert_eq!(dml_color("00FF00", &[("tint", 0.5)]), "BCFFBC");
        assert_eq!(dml_color("00FF00", &[("lumOff", -0.2)]), "009900");
        // the photo box of the MHLW resume form: lt1 (white) with a 50% shade
        assert_eq!(dml_color("FFFFFF", &[("shade", 0.5)]), "BCBCBC");
        assert_eq!(scheme_index("lt1"), Some(0));
        assert_eq!(scheme_index("accent6"), Some(9));
        assert_eq!(scheme_index("phClr"), None);
    }

    #[test]
    fn the_brightness_tweak_applies() {
        let c: Vec<String> = OFFICE.iter().map(|s| s.to_string()).collect();
        assert_eq!(resolve(&c, 4, 0.0), "4472C4", "加減なしは素の色");
        let light = resolve(&c, 4, 0.6);
        let dark = resolve(&c, 4, -0.5);
        let lum = |h: &str| {
            (0..3)
                .map(|i| u32::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap())
                .sum::<u32>()
        };
        assert!(lum(&light) > lum("4472C4"), "明るくならない: {light}");
        assert!(lum(&dark) < lum("4472C4"), "暗くならない: {dark}");
        // 白と黒は加減しても振り切れない
        assert_eq!(resolve(&c, 0, 0.5).len(), 6);
    }

}
