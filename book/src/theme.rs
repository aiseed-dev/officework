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

#[cfg(test)]
mod tests {
    use super::*;

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
