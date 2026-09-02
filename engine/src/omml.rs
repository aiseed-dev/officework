//! **OMML を LaTeX に変える。**
//!
//! docx と xlsx の数式は OMML(`m:oMath`)で書いてあります。こちらは
//! LaTeX を組める([`kumihan::suushiki`])ので、読むときに LaTeX へ直せば
//! 数式が絵として出て、開き直しても直せます(2026-09-02 の決め。
//! SEKKEI「数式は Rust で組み、OMML も読む」)。
//!
//! 扱う要素は LibreOffice の読み手(`starmath/source/ooxmlimport.cxx` の
//! `SmOoxmlImport`)に合わせました。あちらは OMML を StarMath に変えます。
//! 同じ19要素を LaTeX に変えます。
//!
//! **試験は実物で**。LibreOffice に StarMath の式を docx へ書かせた物を
//! 使います(手書きの OMML では、属性の付き方も入れ子も本物と違います)。

use quick_xml::events::Event;
use quick_xml::Reader;

/// 要素1つ。属性は名前の後ろだけを鍵にします(`m:val` は `val`)。
#[derive(Debug, Clone)]
struct Ki {
    na: String,
    zoku: Vec<(String, String)>,
    ko: Vec<Ki>,
    ji: String,
}

impl Ki {
    fn zoku(&self, k: &str) -> Option<&str> {
        self.zoku.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
    }

    /// 名前が合う子を全部
    fn kora<'a>(&'a self, na: &'a str) -> impl Iterator<Item = &'a Ki> {
        self.ko.iter().filter(move |k| k.na == na)
    }

    /// 名前が合う最初の子
    fn ko1(&self, na: &str) -> Option<&Ki> {
        self.ko.iter().find(|k| k.na == na)
    }
}

/// 接頭辞を落とした名前(`m:oMath` → `oMath`)
fn na_of(b: &[u8]) -> String {
    let s = String::from_utf8_lossy(b);
    match s.rfind(':') {
        Some(i) => s[i + 1..].to_string(),
        None => s.to_string(),
    }
}

/// XML を木にします。数式の外側の要素(`w:rPr` など)も入りますが、
/// 変換の側で見なければ害はありません
fn ki_ni(xml: &str) -> Option<Ki> {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);
    let mut tumi: Vec<Ki> = Vec::new();
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let zoku = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        (na_of(a.key.as_ref()),
                         String::from_utf8_lossy(&a.value).to_string())
                    })
                    .collect();
                tumi.push(Ki { na: na_of(e.name().as_ref()), zoku, ko: Vec::new(), ji: String::new() });
            }
            Ok(Event::Empty(e)) => {
                let zoku = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        (na_of(a.key.as_ref()),
                         String::from_utf8_lossy(&a.value).to_string())
                    })
                    .collect();
                let k = Ki { na: na_of(e.name().as_ref()), zoku, ko: Vec::new(), ji: String::new() };
                match tumi.last_mut() {
                    Some(oya) => oya.ko.push(k),
                    None => return Some(k),
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(oya) = tumi.last_mut() {
                    oya.ji.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(_)) => {
                let k = tumi.pop()?;
                match tumi.last_mut() {
                    Some(oya) => oya.ko.push(k),
                    None => return Some(k),
                }
            }
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
}

/// **OMML の断片を LaTeX に。** 読めなければ `None`。
///
/// `xml` は `<m:oMath>` か `<m:oMathPara>` から始まる原文です。
pub fn to_latex(xml: &str) -> Option<String> {
    let ki = ki_ni(xml)?;
    let s = naka(&ki, false);
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// 子を順に変えて繋ぎます。
///
/// `kan` は「いま `m:fName` の中に居る」印です。関数の名前は `m:r` の字
/// として来ますが、`\lim` のように上下付きの中へ入っていることもあるので、
/// 入れ子の底まで持って行きます(LibreOffice の `SmOoxmlImport` も
/// `m:fName` を再帰で読んでから関数として扱います)。
fn naka(k: &Ki, kan: bool) -> String {
    k.ko.iter().map(|c| hitotsu(c, kan)).collect::<Vec<_>>().join("")
}

/// 中括弧で包みます。1文字なら包みません(`x^2` を `x^{2}` にしない)。
///
/// `\lim` のような命令1つだけのときも包みません。包むと LaTeX が
/// 上下付きの置き場所を変えてしまい、`lim` の下に来るべき物が右へ回ります。
fn tsutsumu(s: &str) -> String {
    let inochi = s.starts_with('\\') && s[1..].chars().all(|c| c.is_ascii_alphabetic());
    if inochi || (s.chars().count() == 1 && !s.starts_with('\\')) {
        s.to_string()
    } else {
        format!("{{{s}}}")
    }
}

fn hitotsu(k: &Ki, kan: bool) -> String {
    match k.na.as_str() {
        // 数式の入れ物。中を繋ぐだけ
        "oMath" | "oMathPara" | "e" | "num" | "den" | "sub" | "sup" | "deg"
        | "lim" | "fName" | "mr" => naka(k, kan),
        // 分数。`m:type` は bar(既定)/ lin(斜め)/ noBar(棒なし)
        "f" => {
            let n = k.ko1("num").map(|x| naka(x, kan)).unwrap_or_default();
            let d = k.ko1("den").map(|x| naka(x, kan)).unwrap_or_default();
            match k.ko1("fPr").and_then(|p| p.ko1("type")).and_then(|t| t.zoku("val")) {
                Some("lin") => format!("{}/{}", tsutsumu(&n), tsutsumu(&d)),
                Some("noBar") => format!("{{{{{n}}} \\atop {{{d}}}}}"),
                _ => format!("\\frac{{{n}}}{{{d}}}"),
            }
        }
        // 根号。`m:degHide` が立っていれば平方根
        "rad" => {
            let e = k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default();
            let kakusu = k
                .ko1("radPr")
                .and_then(|p| p.ko1("degHide"))
                .is_some_and(|d| !matches!(d.zoku("val"), Some("0") | Some("false")));
            let deg = k.ko1("deg").map(|x| naka(x, kan)).unwrap_or_default();
            if kakusu || deg.trim().is_empty() {
                format!("\\sqrt{{{e}}}")
            } else {
                format!("\\sqrt[{deg}]{{{e}}}")
            }
        }
        "sSup" => format!("{}^{}",
            tsutsumu(&k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default()),
            tsutsumu(&k.ko1("sup").map(|x| naka(x, kan)).unwrap_or_default())),
        "sSub" => format!("{}_{}",
            tsutsumu(&k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default()),
            tsutsumu(&k.ko1("sub").map(|x| naka(x, kan)).unwrap_or_default())),
        "sSubSup" => format!("{}_{}^{}",
            tsutsumu(&k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default()),
            tsutsumu(&k.ko1("sub").map(|x| naka(x, kan)).unwrap_or_default()),
            tsutsumu(&k.ko1("sup").map(|x| naka(x, kan)).unwrap_or_default())),
        // 前置き(左に付く上下付き)
        "sPre" => format!("{{}}_{}^{}{}",
            tsutsumu(&k.ko1("sub").map(|x| naka(x, kan)).unwrap_or_default()),
            tsutsumu(&k.ko1("sup").map(|x| naka(x, kan)).unwrap_or_default()),
            tsutsumu(&k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default())),
        // 総和・積分など。演算子は `m:chr`(既定は総和)
        "nary" => {
            let pr = k.ko1("naryPr");
            let chr = pr.and_then(|p| p.ko1("chr")).and_then(|c| c.zoku("val")).unwrap_or("∑");
            let kakusu = |na: &str| {
                pr.and_then(|p| p.ko1(na))
                    .is_some_and(|d| !matches!(d.zoku("val"), Some("0") | Some("false")))
            };
            let mut s = enzan(chr);
            if !kakusu("subHide") {
                let v = k.ko1("sub").map(|x| naka(x, kan)).unwrap_or_default();
                if !v.trim().is_empty() {
                    s.push('_');
                    s.push_str(&tsutsumu(&v));
                }
            }
            if !kakusu("supHide") {
                let v = k.ko1("sup").map(|x| naka(x, kan)).unwrap_or_default();
                if !v.trim().is_empty() {
                    s.push('^');
                    s.push_str(&tsutsumu(&v));
                }
            }
            s.push_str(&k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default());
            s
        }
        // 括弧。開き・閉じ・区切りは属性で来ます(無ければ丸括弧)
        "d" => {
            let pr = k.ko1("dPr");
            let g = |na: &str, kitei: &str| {
                pr.and_then(|p| p.ko1(na))
                    .and_then(|c| c.zoku("val"))
                    .unwrap_or(kitei)
                    .to_string()
            };
            let (b, e2) = (g("begChr", "("), g("endChr", ")"));
            let sep = g("sepChr", ",");
            let naka_ra: Vec<String> = k.kora("e").map(|x| naka(x, kan)).collect();
            format!("\\left{} {} \\right{}", kakko(&b), naka_ra.join(&format!(" {sep} ")), kakko(&e2))
        }
        // 行列
        "m" => {
            let gyou: Vec<String> = k
                .kora("mr")
                .map(|r| r.kora("e").map(|x| naka(x, kan)).collect::<Vec<_>>().join(" & "))
                .collect();
            format!("\\begin{{matrix}} {} \\end{{matrix}}", gyou.join(" \\\\ "))
        }
        // 式の並び
        "eqArr" => {
            let gyou: Vec<String> = k.kora("e").map(|x| naka(x, kan)).collect();
            format!("\\begin{{array}}{{l}} {} \\end{{array}}", gyou.join(" \\\\ "))
        }
        // 関数(sin など)。名前が知っている物なら制御綴りにします
        "func" => {
            // 名前は中を読んで作ります。知っている名前かどうかは `m:r` の
            // 所で見るので、ここでは包み直しません
            let na = k.ko1("fName").map(|x| naka(x, true)).unwrap_or_default();
            let e = k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default();
            format!("{} {}", na.trim_end(), e)
        }
        // 極限。下は `\lim_{…}`、上は `^{…}`
        "limLow" => format!("{}_{}",
            tsutsumu(&k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default()),
            tsutsumu(&k.ko1("lim").map(|x| naka(x, kan)).unwrap_or_default())),
        "limUpp" => format!("{}^{}",
            tsutsumu(&k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default()),
            tsutsumu(&k.ko1("lim").map(|x| naka(x, kan)).unwrap_or_default())),
        // アクセント。`m:chr` の既定は U+0302(ハット)
        "acc" => {
            let chr = k
                .ko1("accPr")
                .and_then(|p| p.ko1("chr"))
                .and_then(|c| c.zoku("val"))
                .unwrap_or("\u{0302}");
            format!("{}{{{}}}", akusento(chr), k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default())
        }
        // 上線・下線
        "bar" => {
            let sita = k
                .ko1("barPr")
                .and_then(|p| p.ko1("pos"))
                .and_then(|c| c.zoku("val"))
                == Some("bot");
            let e = k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default();
            if sita { format!("\\underline{{{e}}}") } else { format!("\\overline{{{e}}}") }
        }
        // 中括弧などで括る
        "groupChr" => {
            let pr = k.ko1("groupChrPr");
            let chr = pr.and_then(|p| p.ko1("chr")).and_then(|c| c.zoku("val")).unwrap_or("⏟");
            let sita = pr.and_then(|p| p.ko1("pos")).and_then(|c| c.zoku("val")) == Some("bot");
            let e = k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default();
            match (chr, sita) {
                ("⏞", _) => format!("\\overbrace{{{e}}}"),
                ("⏟", _) => format!("\\underbrace{{{e}}}"),
                (_, true) => format!("\\underbrace{{{e}}}"),
                _ => format!("\\overbrace{{{e}}}"),
            }
        }
        // 囲み。`m:box` は中身だけ、`m:borderBox` は枠
        "box" => k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default(),
        "borderBox" => format!("\\boxed{{{}}}", k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default()),
        // 見えない箱。場所だけ取ります
        "phant" => format!("\\phantom{{{}}}", k.ko1("e").map(|x| naka(x, kan)).unwrap_or_default()),
        // 字。`m:nor`(普通の字)なら \text で囲みます
        "r" => {
            let s: String = k.kora("t").map(|t| t.ji.clone()).collect();
            if s.is_empty() {
                return String::new();
            }
            let futsuu = k
                .ko1("rPr")
                .is_some_and(|p| p.ko1("nor").is_some() || p.ko1("lit").is_some());
            if futsuu {
                format!("\\text{{{s}}}")
            } else if kan {
                kansuu(s.trim())
            } else {
                ji_no_kae(&s)
            }
        }
        // 数式の外の物(w:rPr など)は見ません
        _ => String::new(),
    }
}

/// 総和・積分などの演算子。名前が無ければそのままの字
fn enzan(c: &str) -> String {
    match c {
        "∑" => "\\sum".into(),
        "∏" => "\\prod".into(),
        "∐" => "\\coprod".into(),
        "∫" => "\\int".into(),
        "∬" => "\\iint".into(),
        "∭" => "\\iiint".into(),
        "∮" => "\\oint".into(),
        "⋃" => "\\bigcup".into(),
        "⋂" => "\\bigcap".into(),
        "⋁" => "\\bigvee".into(),
        "⋀" => "\\bigwedge".into(),
        _ => ji_no_kae(c),
    }
}

/// アクセントの字 → LaTeX の命令
fn akusento(c: &str) -> String {
    match c {
        "\u{0302}" | "^" => "\\hat".into(),
        "\u{0303}" | "~" => "\\tilde".into(),
        "\u{0304}" | "¯" => "\\bar".into(),
        "\u{0306}" => "\\breve".into(),
        "\u{0307}" | "˙" => "\\dot".into(),
        "\u{0308}" | "¨" => "\\ddot".into(),
        "\u{030C}" | "ˇ" => "\\check".into(),
        "\u{20D7}" | "→" => "\\vec".into(),
        _ => "\\hat".into(),
    }
}

/// 関数の名前。知っている物は制御綴りに
fn kansuu(na: &str) -> String {
    const SHIRU: &[&str] = &[
        "sin", "cos", "tan", "sec", "csc", "cot", "sinh", "cosh", "tanh",
        "arcsin", "arccos", "arctan", "log", "ln", "lg", "exp", "det", "dim",
        "gcd", "hom", "ker", "max", "min", "sup", "inf", "lim", "deg", "arg",
    ];
    let t = na.trim();
    if SHIRU.contains(&t) {
        format!("\\{t}")
    } else {
        ji_no_kae(t)
    }
}

/// 字の置き替え。LaTeX で意味を持つ字を逃がし、よく出る記号を命令にします
fn ji_no_kae(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    // 直前の字も `\text{}` に入れた物か
    let mut tugi = false;
    for c in s.chars() {
        match c {
            // LaTeX の特別な字
            '\\' => out.push_str("\\backslash "),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '#' | '$' | '%' | '&' | '_' => {
                out.push('\\');
                out.push(c);
            }
            // ギリシャ文字
            'α' => out.push_str("\\alpha "),
            'β' => out.push_str("\\beta "),
            'γ' => out.push_str("\\gamma "),
            'δ' => out.push_str("\\delta "),
            'ε' => out.push_str("\\epsilon "),
            'θ' => out.push_str("\\theta "),
            'λ' => out.push_str("\\lambda "),
            'μ' => out.push_str("\\mu "),
            'π' => out.push_str("\\pi "),
            'ρ' => out.push_str("\\rho "),
            'σ' => out.push_str("\\sigma "),
            'τ' => out.push_str("\\tau "),
            'φ' => out.push_str("\\phi "),
            'ω' => out.push_str("\\omega "),
            'Γ' => out.push_str("\\Gamma "),
            'Δ' => out.push_str("\\Delta "),
            'Θ' => out.push_str("\\Theta "),
            'Λ' => out.push_str("\\Lambda "),
            'Σ' => out.push_str("\\Sigma "),
            'Φ' => out.push_str("\\Phi "),
            'Ω' => out.push_str("\\Omega "),
            // よく出る記号
            '≤' => out.push_str("\\leq "),
            '≥' => out.push_str("\\geq "),
            '≠' => out.push_str("\\neq "),
            '≈' => out.push_str("\\approx "),
            '×' => out.push_str("\\times "),
            '÷' => out.push_str("\\div "),
            '±' => out.push_str("\\pm "),
            '∞' => out.push_str("\\infty "),
            '→' => out.push_str("\\to "),
            '∈' => out.push_str("\\in "),
            '∀' => out.push_str("\\forall "),
            '∃' => out.push_str("\\exists "),
            '∂' => out.push_str("\\partial "),
            '∇' => out.push_str("\\nabla "),
            '⋯' | '…' => out.push_str("\\cdots "),
            '⋅' => out.push_str("\\cdot "),
            // **日本語はそのままでは組めません。** `\text{}` に入れます。
            // 続く分はまとめて1つに入れます(1字ずつだと字間が開きます)
            c if !c.is_ascii() && !c.is_whitespace() => {
                if out.ends_with('}') && tugi {
                    out.pop();
                    out.push(c);
                    out.push('}');
                } else {
                    out.push_str(&format!("\\text{{{c}}}"));
                }
                tugi = true;
                continue;
            }
            c => out.push(c),
        }
        tugi = false;
    }
    out
}

/// 括弧の字 → LaTeX。`\left` `\right` の後ろに置ける形にします
fn kakko(c: &str) -> String {
    match c {
        "(" | ")" | "[" | "]" | "/" | "|" => c.to_string(),
        "{" => "\\{".into(),
        "}" => "\\}".into(),
        "⟨" | "〈" => "\\langle".into(),
        "⟩" | "〉" => "\\rangle".into(),
        "‖" => "\\|".into(),
        "" => ".".into(),
        _ => ".".into(),
    }
}
