//! Format codes as ODF data styles (the inverse of `numfmt`).
//!
//! The elements, their attributes and their order follow what LibreOffice
//! 24.2 writes for the same format (`number:number` with
//! `number:decimal-places`, `number:min-decimal-places`, `number:grouping`
//! and so on). One Excel code can become several styles: the sections before
//! the last one are written as `<name>P0`, `<name>P1`, ... with
//! `style:volatile="true"`, and the last section is the style itself, which
//! reaches the others through `style:map` conditions.
//!
//! Codes that cannot be written (unknown tags, text inside decimals, a
//! condition on the first of three sections, ...) give `None`, and the cell
//! is then written without a data style.

use std::fmt::Write;

/// The ODF data style elements for one format code, named `name`. None for
/// General and for codes that cannot be written
pub fn data_style(name: &str, code: &str) -> Option<String> {
    let trimmed = code.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("general") {
        return None;
    }
    let secs: Vec<Sec> = split_sections(code)?.into_iter().map(parse_section).collect::<Option<_>>()?;
    if secs.len() > 4 {
        return None;
    }
    // LibreOffice puts the language of the whole format on every style
    let lang = secs.iter().find_map(|s| s.locale).and_then(lang_of);
    let plan = plan(&secs)?;
    let mut out = String::new();
    for (i, (sec, kind)) in plan.parts.iter().enumerate() {
        out.push_str(&style_xml(&format!("{name}P{i}"), true, *kind, *sec, lang, &[])?);
        out.push('\n');
    }
    let maps: Vec<String> = plan
        .maps
        .iter()
        .enumerate()
        .map(|(i, c)| {
            format!(
                "<style:map style:condition=\"{}\" style:apply-style-name=\"{}\"/>",
                esc(c),
                esc(&format!("{name}P{i}"))
            )
        })
        .collect();
    out.push_str(&style_xml(name, false, plan.main.1, plan.main.0, lang, &maps)?);
    Some(out)
}

// ---------------------------------------------------------------- sections

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Number,
    Percentage,
    Currency,
    Date,
    Time,
    Text,
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Lit(String),
    /// `_x`: the width of `x`, shown as blank
    Blank(char),
    /// `*x`: repeat `x` to fill the cell
    Fill(char),
    /// One of `0`, `#`, `?`
    Digit(char),
    Point,
    Comma,
    Percent,
    Slash,
    At,
    General,
    Exp { lower: bool, plus: bool },
    Currency { sym: String, lcid: Option<u32> },
    Year(usize),
    /// A run of `m` before it is known to be the month or the minutes
    M(usize),
    Month(usize),
    Minute(usize),
    Day(usize),
    Hour(usize),
    Second(usize),
    /// `e`, `ee`: the year in the era (Japanese calendar)
    EraYear(usize),
    /// `g`, `gg`, `ggg`: the era name
    Era(usize),
    /// `aaa`, `aaaa`: the weekday in the Japanese calendar's locale
    WeekdayJp(usize),
    Quarter(usize),
    Week,
    AmPm,
    /// `[h]`, `[mm]`, `[ss]`: the first unit counts past its range
    Elapsed(char, usize),
}

/// One section of a code: the tags in brackets and the body after them
#[derive(Default, Debug)]
struct Sec {
    /// `[Red]` as the hex color LibreOffice writes
    color: Option<&'static str>,
    /// `[>100]`: the ODF operator and the number as written
    cond: Option<(&'static str, String)>,
    /// `[$-411]`
    locale: Option<u32>,
    /// `[DBNum1]`
    dbnum: Option<u8>,
    toks: Vec<Tok>,
}

/// The sections of a code, split at `;` outside quotes, escapes and brackets
fn split_sections(code: &str) -> Option<Vec<&str>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut bracket = false;
    let mut skip = false;
    for (i, c) in code.char_indices() {
        if skip {
            skip = false;
        } else if quoted {
            quoted = c != '"';
        } else if bracket {
            bracket = c != ']';
        } else {
            match c {
                '"' => quoted = true,
                '[' => bracket = true,
                '\\' | '_' | '*' => skip = true,
                ';' => {
                    out.push(&code[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
    }
    if quoted || bracket {
        return None;
    }
    out.push(&code[start..]);
    Some(out)
}

fn ci_prefix(chars: &[char], word: &str) -> bool {
    let w: Vec<char> = word.chars().collect();
    chars.len() >= w.len() && chars.iter().zip(&w).all(|(a, b)| a.to_ascii_lowercase() == *b)
}

fn run_of(chars: &[char], c: char) -> usize {
    chars.iter().take_while(|x| x.eq_ignore_ascii_case(&c)).count()
}

fn parse_section(src: &str) -> Option<Sec> {
    let chars: Vec<char> = src.chars().collect();
    let mut sec = Sec::default();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let rest = &chars[i..];
        match c {
            '"' => {
                let close = rest[1..].iter().position(|x| *x == '"')?;
                sec.toks.push(Tok::Lit(rest[1..=close].iter().collect()));
                i += close + 2;
            }
            '\\' => {
                sec.toks.push(Tok::Lit(rest.get(1)?.to_string()));
                i += 2;
            }
            '_' => {
                sec.toks.push(Tok::Blank(*rest.get(1)?));
                i += 2;
            }
            '*' => {
                sec.toks.push(Tok::Fill(*rest.get(1)?));
                i += 2;
            }
            '[' => {
                let close = rest.iter().position(|x| *x == ']')?;
                let inner: String = rest[1..close].iter().collect();
                bracket(&inner, &mut sec)?;
                i += close + 1;
            }
            '0' | '#' | '?' => {
                sec.toks.push(Tok::Digit(c));
                i += 1;
            }
            '.' => {
                sec.toks.push(Tok::Point);
                i += 1;
            }
            ',' => {
                sec.toks.push(Tok::Comma);
                i += 1;
            }
            '%' => {
                sec.toks.push(Tok::Percent);
                i += 1;
            }
            '/' => {
                sec.toks.push(Tok::Slash);
                i += 1;
            }
            '@' => {
                sec.toks.push(Tok::At);
                i += 1;
            }
            c if c.is_ascii_alphabetic() => i += letters(rest, &mut sec)?,
            c => {
                sec.toks.push(Tok::Lit(c.to_string()));
                i += 1;
            }
        }
    }
    resolve_minutes(&mut sec.toks);
    Some(sec)
}

/// A keyword made of letters, and how many characters it took
fn letters(rest: &[char], sec: &mut Sec) -> Option<usize> {
    let c = rest[0].to_ascii_lowercase();
    if ci_prefix(rest, "general") {
        sec.toks.push(Tok::General);
        return Some(7);
    }
    if ci_prefix(rest, "am/pm") {
        sec.toks.push(Tok::AmPm);
        return Some(5);
    }
    if ci_prefix(rest, "a/p") {
        sec.toks.push(Tok::AmPm);
        return Some(3);
    }
    // `E+` and `E-` after digits
    if c == 'e' && matches!(rest.get(1), Some('+') | Some('-')) {
        if let Some(Tok::Digit(_) | Tok::Point) = sec.toks.last() {
            sec.toks.push(Tok::Exp { lower: rest[0] == 'e', plus: rest[1] == '+' });
            return Some(2);
        }
    }
    let n = run_of(rest, c);
    let tok = match (c, n) {
        ('y', _) => Tok::Year(n),
        ('m', 1..=5) => Tok::M(n),
        ('d', 1..=4) => Tok::Day(n),
        ('h', 1..=2) => Tok::Hour(n),
        ('s', 1..=2) => Tok::Second(n),
        ('e', 1..=2) => Tok::EraYear(n),
        ('g', 1..=3) => Tok::Era(n),
        ('a', 3..=4) => Tok::WeekdayJp(n),
        ('q', 1..=2) => Tok::Quarter(n),
        ('w', 2) => Tok::Week,
        _ => return None,
    };
    sec.toks.push(tok);
    Some(n)
}

/// The names of the eight colors Excel has, and `[ColorN]`
fn color_of(name: &str) -> Option<&'static str> {
    let low = name.to_ascii_lowercase();
    Some(match low.as_str() {
        "black" => "#000000",
        "blue" => "#0000ff",
        "cyan" => "#00ffff",
        "green" => "#00ff00",
        "magenta" => "#ff00ff",
        "red" => "#ff0000",
        "white" => "#ffffff",
        "yellow" => "#ffff00",
        _ => {
            let n: usize = low.strip_prefix("color")?.parse().ok()?;
            return LO_PALETTE.get(n.checked_sub(1)?).copied();
        }
    })
}

/// Palette LibreOffice reads `[ColorN]` from, as the colors it writes
const LO_PALETTE: [&str; 56] = [
    "#000000", "#111111", "#1c1c1c", "#333333", "#666666", "#808080", "#999999", "#b2b2b2",
    "#cccccc", "#dddddd", "#eeeeee", "#ffffff", "#ffff00", "#ffbf00", "#ff8000", "#ff4000",
    "#ff0000", "#bf0041", "#800080", "#55308d", "#2a6099", "#158466", "#00a933", "#81d41a",
    "#ffffd7", "#fff5ce", "#ffdbb6", "#ffd8ce", "#ffd7d7", "#f7d1d5", "#e0c2cd", "#dedce6",
    "#dee6ef", "#dee7e5", "#dde8cb", "#f6f9d4", "#ffffa6", "#ffe994", "#ffb66c", "#ffaa95",
    "#ffa6a6", "#ec9ba4", "#bf819e", "#b7b3ca", "#b4c7dc", "#b3cac7", "#afd095", "#e8f2a1",
    "#ffff6d", "#ffde59", "#ff972f", "#ff7b59", "#ff6d6d", "#e16173", "#a1467e", "#8e86ae",
];

/// What one `[...]` tag in a section means
fn bracket(inner: &str, sec: &mut Sec) -> Option<()> {
    if let Some(rest) = inner.strip_prefix('$') {
        let (sym, id) = match rest.split_once('-') {
            Some((s, i)) => (s, u32::from_str_radix(i, 16).ok()),
            None => (rest, None),
        };
        if sym.is_empty() {
            // `[$-411]`: the locale of the format. A tag whose id is not
            // a number (`[$-ja-JP-x-calendar]`) or not a language
            // (`[$-F800]`) is left out
            sec.locale = id.filter(|i| lang_of(*i).is_some()).or(sec.locale);
        } else {
            sec.toks.push(Tok::Currency { sym: sym.to_string(), lcid: id });
        }
        return Some(());
    }
    if let Some(c) = color_of(inner) {
        sec.color = Some(c);
        return Some(());
    }
    let low = inner.to_ascii_lowercase();
    if let Some(n) = low.strip_prefix("dbnum") {
        sec.dbnum = Some(n.parse().ok().filter(|n| (1..=3).contains(n))?);
        return Some(());
    }
    let first = low.chars().next()?;
    if matches!(first, 'h' | 'm' | 's') && low.chars().all(|c| c == first) {
        sec.toks.push(Tok::Elapsed(first, low.len()));
        return Some(());
    }
    // `[>100]`
    let op_len = low.chars().take_while(|c| matches!(c, '<' | '>' | '=')).count();
    let (op, num) = low.split_at(op_len);
    let op = match op {
        "=" => "=",
        "<>" => "!=",
        "<" => "<",
        "<=" => "<=",
        ">" => ">",
        ">=" => ">=",
        _ => return None,
    };
    let v: f64 = num.trim().parse().ok()?;
    sec.cond = Some((op, fmt_num(v)));
    Some(())
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// `m` is the minutes right after hours or right before seconds, the month
/// anywhere else. Literals between them do not matter
/// (`h"時"mm"分"` has minutes)
fn resolve_minutes(toks: &mut [Tok]) {
    let plain = |t: &Tok| matches!(t, Tok::Lit(_) | Tok::Blank(_) | Tok::Fill(_));
    for i in 0..toks.len() {
        let Tok::M(n) = toks[i] else { continue };
        let minute = n <= 2 && {
            let prev = toks[..i].iter().rev().find(|t| !plain(t));
            let next = toks[i + 1..].iter().find(|t| !plain(t));
            matches!(prev, Some(Tok::Hour(_)) | Some(Tok::Elapsed('h', _)))
                || matches!(next, Some(Tok::Second(_)) | Some(Tok::Elapsed('s', _)))
        };
        toks[i] = if minute { Tok::Minute(n) } else { Tok::Month(n) };
    }
}

fn kind_of(sec: &Sec) -> Kind {
    let any = |f: fn(&Tok) -> bool| sec.toks.iter().any(f);
    if any(|t| matches!(t, Tok::At)) {
        Kind::Text
    } else if any(|t| {
        matches!(
            t,
            Tok::Year(_)
                | Tok::Month(_)
                | Tok::Day(_)
                | Tok::Quarter(_)
                | Tok::Week
                | Tok::EraYear(_)
                | Tok::Era(_)
                | Tok::WeekdayJp(_)
        )
    }) {
        Kind::Date
    } else if any(|t| {
        matches!(t, Tok::Hour(_) | Tok::Minute(_) | Tok::Second(_) | Tok::AmPm | Tok::Elapsed(..))
    }) {
        Kind::Time
    } else if any(|t| matches!(t, Tok::Currency { .. })) {
        Kind::Currency
    } else if any(|t| matches!(t, Tok::Percent)) {
        Kind::Percentage
    } else {
        Kind::Number
    }
}

// ---------------------------------------------------------------- plan

/// A style to write: a section (None is General) and its element type
type Part<'a> = (Option<&'a Sec>, Kind);

/// The styles one code becomes: the earlier sections, the conditions that
/// reach them, and the style that stands for the last section
struct Plan<'a> {
    parts: Vec<Part<'a>>,
    maps: Vec<String>,
    main: Part<'a>,
}

/// The upper limit LibreOffice writes for "every number"
const ANY_NUMBER: &str = "value()<=1.7976931348623157E+308";

fn plan(secs: &[Sec]) -> Option<Plan<'_>> {
    let cond = |s: &Sec| s.cond.as_ref().map(|(op, v)| format!("value(){op}{v}"));
    fn part(s: &Sec) -> Part<'_> {
        (Some(s), kind_of(s))
    }
    let cs: Vec<Option<String>> = secs.iter().map(cond).collect();
    let (parts, maps, main): (Vec<Part>, Vec<String>, Part) = match secs {
        [a] => {
            if cs[0].is_some() {
                return None;
            }
            (vec![], vec![], part(a))
        }
        [a, b] => match (&cs[0], &cs[1]) {
            // Both sections have conditions: LibreOffice adds a General
            // style for everything else
            (Some(c1), Some(c2)) => (
                vec![part(a), part(b)],
                vec![c1.clone(), c2.clone()],
                (None, Kind::Number),
            ),
            (Some(c1), None) => (vec![part(a)], vec![c1.clone()], part(b)),
            (None, Some(_)) => return None,
            (None, None) if kind_of(b) == Kind::Text => {
                (vec![part(a)], vec![ANY_NUMBER.to_string()], part(b))
            }
            (None, None) => (vec![part(a)], vec!["value()>=0".to_string()], part(b)),
        },
        [a, b, c] => match (&cs[0], &cs[1]) {
            (Some(c1), Some(c2)) => {
                if kind_of(c) == Kind::Text {
                    return None;
                }
                (vec![part(a), part(b)], vec![c1.clone(), c2.clone()], part(c))
            }
            (None, None) if kind_of(c) == Kind::Text => (
                vec![part(a), part(b)],
                vec!["value()>=0".to_string(), "value()<0".to_string()],
                part(c),
            ),
            (None, None) => (
                vec![part(a), part(b)],
                vec!["value()>0".to_string(), "value()<0".to_string()],
                part(c),
            ),
            _ => return None,
        },
        [a, b, c, d] => {
            if cs.iter().any(Option::is_some) {
                return None;
            }
            (
                vec![part(a), part(b), part(c)],
                vec!["value()>0".to_string(), "value()<0".to_string(), "value()=0".to_string()],
                (Some(d), Kind::Text),
            )
        }
        _ => return None,
    };
    Some(Plan { parts, maps, main })
}

// ---------------------------------------------------------------- locales

/// Windows locale ids of the languages and countries LibreOffice and Excel
/// both know, as `numfmt` reads them
const LOCALES: [(u32, &str, Option<&str>); 60] = [
    (0x1, "ar", None),
    (0x411, "ja", Some("JP")),
    (0x409, "en", Some("US")),
    (0x809, "en", Some("GB")),
    (0xC09, "en", Some("AU")),
    (0x1009, "en", Some("CA")),
    (0x1409, "en", Some("NZ")),
    (0x1809, "en", Some("IE")),
    (0x1C09, "en", Some("ZA")),
    (0x4009, "en", Some("IN")),
    (0x407, "de", Some("DE")),
    (0x807, "de", Some("CH")),
    (0xC07, "de", Some("AT")),
    (0x40C, "fr", Some("FR")),
    (0x80C, "fr", Some("BE")),
    (0xC0C, "fr", Some("CA")),
    (0x100C, "fr", Some("CH")),
    (0x40A, "es", Some("ES")),
    (0x80A, "es", Some("MX")),
    (0x240A, "es", Some("CO")),
    (0x2C0A, "es", Some("AR")),
    (0x340A, "es", Some("CL")),
    (0x410, "it", Some("IT")),
    (0x810, "it", Some("CH")),
    (0x416, "pt", Some("BR")),
    (0x816, "pt", Some("PT")),
    (0x419, "ru", Some("RU")),
    (0x41F, "tr", Some("TR")),
    (0x42A, "vi", Some("VN")),
    (0x421, "id", Some("ID")),
    (0x412, "ko", Some("KR")),
    (0x804, "zh", Some("CN")),
    (0x404, "zh", Some("TW")),
    (0xC04, "zh", Some("HK")),
    (0x1004, "zh", Some("SG")),
    (0x413, "nl", Some("NL")),
    (0x813, "nl", Some("BE")),
    (0x41D, "sv", Some("SE")),
    (0x415, "pl", Some("PL")),
    (0x406, "da", Some("DK")),
    (0x40B, "fi", Some("FI")),
    (0x414, "nb", Some("NO")),
    (0x405, "cs", Some("CZ")),
    (0x40E, "hu", Some("HU")),
    (0x408, "el", Some("GR")),
    (0x40D, "he", Some("IL")),
    (0x41E, "th", Some("TH")),
    (0x422, "uk", Some("UA")),
    (0x401, "ar", Some("SA")),
    (0x439, "hi", Some("IN")),
    (0x418, "ro", Some("RO")),
    (0x402, "bg", Some("BG")),
    (0x40F, "is", Some("IS")),
    (0x41B, "sk", Some("SK")),
    (0x424, "sl", Some("SI")),
    (0x41A, "hr", Some("HR")),
    (0x425, "et", Some("EE")),
    (0x426, "lv", Some("LV")),
    (0x427, "lt", Some("LT")),
    (0x429, "fa", Some("IR")),
];

/// Language and country of a Windows locale id. The upper bits (the
/// calendar and the sort order in `[$-1010411]`) are ignored
fn lang_of(id: u32) -> Option<(&'static str, Option<&'static str>)> {
    let id = id & 0xFFFF;
    LOCALES.iter().find(|l| l.0 == id).map(|l| (l.1, l.2))
}

fn lang_attrs(lang: Option<(&str, Option<&str>)>) -> String {
    let mut s = String::new();
    if let Some((l, c)) = lang {
        let _ = write!(s, " number:language=\"{l}\"");
        if let Some(c) = c {
            let _ = write!(s, " number:country=\"{c}\"");
        }
    }
    s
}

// ---------------------------------------------------------------- writing

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c => o.push(c),
        }
    }
    o
}

/// Collects the elements of one style. Literal text is held back until the
/// next element so that neighbors become one `number:text`
#[derive(Default)]
struct W {
    out: String,
    text: String,
    has_text: bool,
    /// Currency symbol already written (only one is allowed)
    currency: bool,
}

impl W {
    fn lit(&mut self, s: &str) {
        self.text.push_str(s);
        self.has_text = true;
    }

    /// `_x`: a blank as wide as `x`. LibreOffice reads the position of a
    /// blank inside a longer text wrongly when that text starts with an
    /// unquoted separator (`- ` with the blank at 1 shows `- _-`), so every
    /// blank gets a text element of its own, where it is always at 0
    fn blank(&mut self, c: char) {
        self.flush();
        let _ = write!(self.out, "<number:text loext:blank-width-char=\"{}\"> </number:text>", esc(&c.to_string()));
    }

    fn flush(&mut self) {
        if !self.has_text {
            return;
        }
        if self.text.is_empty() {
            self.out.push_str("<number:text/>");
        } else {
            let _ = write!(self.out, "<number:text>{}</number:text>", esc(&self.text));
        }
        self.text.clear();
        self.has_text = false;
    }

    fn element(&mut self, xml: &str) {
        self.flush();
        self.out.push_str(xml);
    }

    /// A token that is not a digit, as text
    fn plain(&mut self, t: &Tok) -> Option<()> {
        match t {
            Tok::Lit(s) => self.lit(s),
            Tok::Blank(c) => self.blank(*c),
            Tok::Fill(c) => self.element(&format!("<number:fill-character>{}</number:fill-character>", esc(&c.to_string()))),
            Tok::Percent => self.lit("%"),
            Tok::Comma => self.lit(","),
            Tok::Point => self.lit("."),
            Tok::Slash => self.lit("/"),
            Tok::At => self.element("<number:text-content/>"),
            Tok::Currency { sym, lcid } => {
                if self.currency {
                    self.lit(sym);
                } else {
                    self.currency = true;
                    let lang = lcid.and_then(lang_of);
                    self.element(&format!(
                        "<number:currency-symbol{}>{}</number:currency-symbol>",
                        lang_attrs(lang),
                        esc(sym)
                    ));
                }
            }
            _ => return None,
        }
        Some(())
    }

    fn finish(mut self) -> String {
        self.flush();
        self.out
    }
}

fn is_digit(t: &Tok) -> bool {
    matches!(t, Tok::Digit(_))
}

/// The body of a numeric section: text and at most one number, scientific
/// number or fraction
fn numeric(toks: &[Tok], w: &mut W) -> Option<()> {
    if toks.iter().any(|t| matches!(t, Tok::Exp { .. })) {
        return scientific(toks, w);
    }
    let slash = toks.iter().position(|t| *t == Tok::Slash);
    if let Some(s) = slash {
        if s > 0 && is_digit(&toks[s - 1]) {
            return fraction(toks, s, w);
        }
    }
    if toks.contains(&Tok::General) {
        if toks.iter().any(is_digit) {
            return None;
        }
        for t in toks {
            match t {
                Tok::General => w.element("<number:number number:min-integer-digits=\"1\"/>"),
                t => w.plain(t)?,
            }
        }
        return Some(());
    }
    let digits: Vec<usize> = (0..toks.len()).filter(|i| is_digit(&toks[*i])).collect();
    if digits.is_empty() {
        return toks.iter().try_for_each(|t| w.plain(t));
    }
    // A `.` followed by a digit starts the decimals
    let point = (0..toks.len()).find(|&i| toks[i] == Tok::Point && toks.get(i + 1).is_some_and(is_digit));
    let int_end = point.unwrap_or(toks.len());
    let ints: Vec<usize> = digits.iter().copied().filter(|&i| i < int_end).collect();
    let dec_end = point.map(|p| {
        let mut e = p + 1;
        while toks.get(e).is_some_and(is_digit) {
            e += 1;
        }
        e
    });
    // Where the number starts and where its integer digits end
    let start = ints.first().copied().or(point)?;
    let after = match dec_end {
        Some(e) => e,
        None => ints.last()? + 1,
    };
    // No digits after the number
    if digits.iter().any(|&i| i >= after) {
        return None;
    }
    let mut end = after;
    while toks.get(end) == Some(&Tok::Comma) {
        end += 1;
    }
    let scale = end - after;

    // Integer part: digits, grouping commas, text between the digits
    let mut grouping = false;
    let mut emb: Vec<(usize, &Tok)> = Vec::new();
    if let (Some(&first), Some(&last_int)) = (ints.first(), ints.last()) {
        for (j, t) in toks.iter().enumerate().take(last_int).skip(first) {
            match t {
                Tok::Digit(_) => {}
                Tok::Comma => grouping = true,
                t @ (Tok::Lit(_) | Tok::Blank(_) | Tok::Percent) => {
                    emb.push((ints.iter().filter(|&&i| i > j).count(), t))
                }
                _ => return None,
            }
        }
        // Text between the last integer digit and the decimal point
        if let Some(p) = point {
            for t in &toks[last_int + 1..p] {
                match t {
                    Tok::Lit(_) | Tok::Blank(_) | Tok::Percent => emb.push((0, t)),
                    _ => return None,
                }
            }
        }
    }
    let chars = |range: &[usize]| -> Vec<char> {
        range
            .iter()
            .map(|&i| match toks[i] {
                Tok::Digit(c) => c,
                _ => '0',
            })
            .collect()
    };
    let int_chars = chars(&ints);
    let dec_chars: Vec<char> = match (point, dec_end) {
        (Some(p), Some(e)) => chars(&(p + 1..e).collect::<Vec<_>>()),
        _ => vec![],
    };
    let blank_int = int_chars.iter().filter(|c| **c == '?').count();
    let min_int = int_chars.iter().filter(|c| matches!(**c, '0' | '?')).count();
    let dp = dec_chars.len();
    let optional = dec_chars.iter().rev().take_while(|c| matches!(**c, '#' | '?')).count();
    let align = dec_chars.iter().rev().take_while(|c| matches!(**c, '#' | '?')).any(|c| *c == '?');

    let mut a = format!(" number:decimal-places=\"{dp}\" number:min-decimal-places=\"{}\"", dp - optional);
    if align && dp > 0 {
        a.push_str(" number:decimal-replacement=\" \"");
    } else if optional > 0 {
        a.push_str(" number:decimal-replacement=\"\"");
    }
    let _ = write!(a, " number:min-integer-digits=\"{min_int}\"");
    if blank_int > 0 {
        let _ = write!(a, " loext:max-blank-integer-digits=\"{blank_int}\"");
    }
    if grouping {
        a.push_str(" number:grouping=\"true\"");
    }
    if scale > 0 {
        let _ = write!(a, " number:display-factor=\"1{}\"", "000".repeat(scale));
    }
    let number = embedded(&a, "number", &emb);

    for t in &toks[..start] {
        w.plain(t)?;
    }
    w.element(&number);
    for t in &toks[end..] {
        w.plain(t)?;
    }
    Some(())
}

/// `<number:number ...>` with the text that sits between its digits
fn embedded(attrs: &str, tag: &str, emb: &[(usize, &Tok)]) -> String {
    if emb.is_empty() {
        return format!("<number:{tag}{attrs}/>");
    }
    let mut out = format!("<number:{tag}{attrs}>");
    let mut i = 0;
    while i < emb.len() {
        let pos = emb[i].0;
        let mut content = String::new();
        let mut blank = String::new();
        while i < emb.len() && emb[i].0 == pos {
            match emb[i].1 {
                Tok::Lit(s) => content.push_str(s),
                Tok::Percent => content.push('%'),
                Tok::Blank(c) => {
                    // The way LibreOffice writes several blanks in one text
                    if blank.is_empty() {
                        blank.push(*c);
                        if !content.is_empty() {
                            let _ = write!(blank, "{}", content.chars().count());
                        }
                    } else {
                        let _ = write!(blank, "_{c}{}", content.chars().count());
                    }
                    content.push(' ');
                }
                _ => {}
            }
            i += 1;
        }
        let b = if blank.is_empty() { String::new() } else { format!(" loext:blank-width-char=\"{}\"", esc(&blank)) };
        let _ = write!(out, "<number:embedded-text number:position=\"{pos}\"{b}>{}</number:embedded-text>", esc(&content));
    }
    let _ = write!(out, "</number:{tag}>");
    out
}

/// `0.00E+00`, `##0.0E+0`
fn scientific(toks: &[Tok], w: &mut W) -> Option<()> {
    let e = toks.iter().position(|t| matches!(t, Tok::Exp { .. }))?;
    let Tok::Exp { lower, plus } = toks[e] else { return None };
    let first = toks[..e].iter().position(is_digit)?;
    let mut ints = Vec::new();
    let mut decs = Vec::new();
    let mut in_dec = false;
    for t in &toks[first..e] {
        match t {
            Tok::Digit(c) if in_dec => decs.push(*c),
            Tok::Digit(c) => ints.push(*c),
            Tok::Point if !in_dec => in_dec = true,
            _ => return None,
        }
    }
    let mut x = e + 1;
    let mut exp_digits = 0;
    while let Some(Tok::Digit(c)) = toks.get(x) {
        if *c != '0' {
            return None;
        }
        exp_digits += 1;
        x += 1;
    }
    if exp_digits == 0 || toks[x..].iter().any(is_digit) {
        return None;
    }
    let dp = decs.len();
    let optional = decs.iter().rev().take_while(|c| matches!(**c, '#' | '?')).count();
    let mut a = format!(
        " number:decimal-places=\"{dp}\" number:min-decimal-places=\"{}\" number:min-integer-digits=\"{}\"",
        dp - optional,
        ints.iter().filter(|c| matches!(**c, '0' | '?')).count()
    );
    let blank = ints.iter().filter(|c| **c == '?').count();
    if blank > 0 {
        let _ = write!(a, " loext:max-blank-integer-digits=\"{blank}\"");
    }
    let _ = write!(
        a,
        " number:min-exponent-digits=\"{exp_digits}\" number:exponent-interval=\"{}\" number:forced-exponent-sign=\"{plus}\"",
        ints.len()
    );
    if lower {
        a.push_str(" loext:exponent-lowercase=\"true\"");
    }
    for t in &toks[..first] {
        w.plain(t)?;
    }
    w.element(&format!("<number:scientific-number{a}/>"));
    for t in &toks[x..] {
        w.plain(t)?;
    }
    Some(())
}

/// `# ?/?`, `0 ??/??`, `?/8`. `slash` is the index of the `/`
fn fraction(toks: &[Tok], slash: usize, w: &mut W) -> Option<()> {
    let digit_chars = |from: usize, to: usize| -> String {
        toks[from..to]
            .iter()
            .map(|t| match t {
                Tok::Digit(c) => *c,
                _ => ' ',
            })
            .collect()
    };
    let mut ns = slash;
    while ns > 0 && is_digit(&toks[ns - 1]) {
        ns -= 1;
    }
    let numerator = digit_chars(ns, slash);
    // The denominator is digits (`??`) or a number (`8`)
    let mut de = slash + 1;
    let mut fixed = String::new();
    while let Some(t) = toks.get(de) {
        match t {
            Tok::Lit(s) if fixed.is_empty() || fixed.chars().all(|c| c.is_ascii_digit()) => {
                if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
                    break;
                }
                fixed.push_str(s);
            }
            _ => break,
        }
        de += 1;
    }
    let mut denominator = String::new();
    if fixed.is_empty() {
        while toks.get(de).is_some_and(is_digit) {
            de += 1;
        }
        denominator = digit_chars(slash + 1, de);
        if denominator.is_empty() {
            return None;
        }
    }
    // An integer part: digits, then what separates it from the numerator
    let mut k = ns;
    while k > 0 && matches!(toks[k - 1], Tok::Lit(_) | Tok::Blank(_)) {
        k -= 1;
    }
    let mut a = String::new();
    let mut pre_end = ns;
    let mut delimiter = String::new();
    if k > 0 && matches!(toks[k - 1], Tok::Digit(_) | Tok::Comma) {
        let mut is = k;
        while is > 0 && matches!(toks[is - 1], Tok::Digit(_) | Tok::Comma) {
            is -= 1;
        }
        let ints: Vec<char> = toks[is..k]
            .iter()
            .filter_map(|t| if let Tok::Digit(c) = t { Some(*c) } else { None })
            .collect();
        for t in &toks[k..ns] {
            match t {
                Tok::Lit(s) => delimiter.push_str(s),
                _ => return None,
            }
        }
        let _ = write!(a, " number:min-integer-digits=\"{}\"", ints.iter().filter(|c| matches!(**c, '0' | '?')).count());
        let blank = ints.iter().filter(|c| **c == '?').count();
        if blank > 0 {
            let _ = write!(a, " loext:max-blank-integer-digits=\"{blank}\"");
        }
        if toks[is..k].contains(&Tok::Comma) {
            a.push_str(" number:grouping=\"true\"");
        }
        pre_end = is;
    }
    if !delimiter.is_empty() && delimiter != " " {
        let _ = write!(a, " loext:integer-fraction-delimiter=\"{}\"", esc(&delimiter));
    }
    // LibreOffice counts a `0` as a `?` for the minimum and notes the zeros
    let spread = |s: &str| -> (usize, usize, usize) {
        let n = s.chars().count();
        let min = s.replace('0', "?").find('?').map_or(0, |i| n - i);
        let zeros = s.find('0').map_or(0, |i| n - i);
        (n, min.max(1), zeros)
    };
    let (nn, nmin, nzero) = spread(&numerator);
    let _ = write!(a, " number:min-numerator-digits=\"{nmin}\" loext:max-numerator-digits=\"{nn}\"");
    if nzero > 0 {
        let _ = write!(a, " loext:zeros-numerator-digits=\"{nzero}\"");
    }
    if !fixed.is_empty() {
        let v: u64 = fixed.parse().ok().filter(|v| *v > 0)?;
        let _ = write!(a, " number:denominator-value=\"{v}\"");
    } else {
        let (dn, dmin, dzero) = spread(&denominator);
        let _ = write!(
            a,
            " number:min-denominator-digits=\"{dmin}\" number:max-denominator-value=\"{}\"",
            "9".repeat(dn)
        );
        if dzero > 0 {
            let _ = write!(a, " loext:zeros-denominator-digits=\"{dzero}\"");
        }
    }
    for t in &toks[..pre_end] {
        w.plain(t)?;
    }
    w.element(&format!("<number:fraction{a}/>"));
    for t in &toks[de..] {
        w.plain(t)?;
    }
    Some(())
}

/// The body of a date or time section
fn date_time(sec: &Sec, w: &mut W, style_attrs: &mut String) -> Option<()> {
    let toks = &sec.toks;
    // `e` and `ee` bring in the Japanese calendar for the whole section
    let gengou = toks.iter().any(|t| matches!(t, Tok::EraYear(_)));
    let cal = |c: Option<&str>| c.map(|c| format!(" number:calendar=\"{c}\"")).unwrap_or_default();
    let section_cal = cal(gengou.then_some("gengou"));
    let long = |b: bool| if b { " number:style=\"long\"" } else { "" };
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        i += 1;
        let xml = match t {
            Tok::Year(n) => format!("<number:year{}{}/>", cal(gengou.then_some("gregorian")), long(*n >= 3)),
            Tok::Month(n) => format!(
                "<number:month{}{}{}/>",
                section_cal,
                long(matches!(*n, 2 | 4)),
                if *n >= 3 { " number:textual=\"true\"" } else { "" }
            ),
            Tok::Day(n) if *n <= 2 => format!("<number:day{}{}/>", section_cal, long(*n == 2)),
            Tok::Day(n) => format!("<number:day-of-week{}{}/>", section_cal, long(*n == 4)),
            Tok::WeekdayJp(n) => format!("<number:day-of-week{}{}/>", cal(Some("gengou")), long(*n == 4)),
            Tok::EraYear(n) => format!("<number:year{}{}/>", cal(Some("gengou")), long(*n == 2)),
            Tok::Era(n) => format!("<number:era{}{}/>", section_cal, long(*n == 3)),
            Tok::Quarter(n) => format!("<number:quarter{}{}/>", section_cal, long(*n == 2)),
            Tok::Week => format!("<number:week-of-year{section_cal}/>"),
            Tok::Hour(n) => format!("<number:hours{}/>", long(*n == 2)),
            Tok::Minute(n) => format!("<number:minutes{}/>", long(*n == 2)),
            Tok::AmPm => "<number:am-pm/>".to_string(),
            Tok::Second(_) | Tok::Elapsed(..) => {
                let (unit, n) = match t {
                    Tok::Second(n) => ('s', *n),
                    Tok::Elapsed(c, n) => (*c, *n),
                    _ => return None,
                };
                if matches!(t, Tok::Elapsed(..)) {
                    *style_attrs = " number:truncate-on-overflow=\"false\"".to_string();
                }
                match unit {
                    'h' => format!("<number:hours{}/>", long(n == 2)),
                    'm' => format!("<number:minutes{}/>", long(n == 2)),
                    _ => {
                        // `.00` right after the seconds is their decimals
                        let mut dec = 0;
                        if toks.get(i) == Some(&Tok::Point) {
                            while toks.get(i + 1 + dec) == Some(&Tok::Digit('0')) {
                                dec += 1;
                            }
                            if dec > 0 {
                                i += 1 + dec;
                            }
                        }
                        let d = if dec > 0 { format!(" number:decimal-places=\"{dec}\"") } else { String::new() };
                        format!("<number:seconds{}{d}/>", long(n == 2))
                    }
                }
            }
            Tok::Digit(_) | Tok::General | Tok::Exp { .. } | Tok::M(_) | Tok::At | Tok::Currency { .. } => return None,
            t => {
                w.plain(t)?;
                continue;
            }
        };
        w.element(&xml);
    }
    Some(())
}

fn style_xml(
    name: &str,
    volatile: bool,
    kind: Kind,
    sec: Option<&Sec>,
    lang: Option<(&str, Option<&str>)>,
    maps: &[String],
) -> Option<String> {
    let mut w = W::default();
    let mut extra = String::new();
    let mut color = None;
    let mut dbnum = None;
    match sec {
        None => w.element("<number:number number:min-integer-digits=\"1\"/>"),
        Some(sec) => {
            color = sec.color;
            dbnum = sec.dbnum;
            match kind {
                Kind::Date | Kind::Time => date_time(sec, &mut w, &mut extra)?,
                Kind::Text => {
                    // A text section has no digits
                    if sec.toks.iter().any(|t| is_digit(t) || matches!(t, Tok::General | Tok::Exp { .. })) {
                        return None;
                    }
                    sec.toks.iter().try_for_each(|t| w.plain(t))?
                }
                Kind::Currency if sec.toks.contains(&Tok::Percent) => return None,
                _ => numeric(&sec.toks, &mut w)?,
            }
        }
    }
    let mut body = w.finish();
    if body.is_empty() {
        body.push_str("<number:text/>");
    }
    let tag = match kind {
        Kind::Number => "number-style",
        Kind::Percentage => "percentage-style",
        Kind::Currency => "currency-style",
        Kind::Date => "date-style",
        Kind::Time => "time-style",
        Kind::Text => "text-style",
    };
    let mut out = format!("<number:{tag} style:name=\"{}\"", esc(name));
    if volatile {
        out.push_str(" style:volatile=\"true\"");
    }
    out.push_str(&lang_attrs(lang));
    out.push_str(&extra);
    if let Some(n) = dbnum {
        // Japanese numerals: the kanji and the way they are written
        let (format, style) = match (n, matches!(kind, Kind::Date | Kind::Time)) {
            (1, false) => ("一", "long"),
            (1, true) => ("一", "short"),
            (2, _) => ("壱", "long"),
            _ => ("１", "short"),
        };
        let _ = write!(
            out,
            " number:transliteration-format=\"{format}\" number:transliteration-language=\"ja\" number:transliteration-country=\"JP\" number:transliteration-style=\"{style}\""
        );
    }
    out.push('>');
    if let Some(c) = color {
        let _ = write!(out, "<style:text-properties fo:color=\"{c}\"/>");
    }
    out.push_str(&body);
    for m in maps {
        out.push_str(m);
    }
    let _ = write!(out, "</number:{tag}>");
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ods::numfmt::parse_data_styles;

    /// Style names are `N1` here; the styles sit in a document element
    fn wrap(xml: &str) -> String {
        format!("<office:document-content>{xml}</office:document-content>")
    }

    /// The code `numfmt` reads back from the styles written for `code`
    fn back(code: &str) -> Option<String> {
        let xml = data_style("N1", code)?;
        parse_data_styles(&wrap(&xml)).get("N1").cloned()
    }

    /// Format codes found in xlsx files (the corpus and codes written for
    /// the purpose), the ones LibreOffice 24.2 made styles from
    const CODES: &[&str] = &[
    r###"¥#,##0"###,
    r###" 0"###,
    r###""¥"#,##0"###,
    r###""¥"#,##0_);[Red]\("¥"#,##0\)"###,
    r###""*"\ \ #,##0.0;"*"\ \-#,##0.0"###,
    r###""*"\ \ 0.0"###,
    r###""*"\ \ 0.0;"*"\ \-0.0"###,
    r###""*"\ \ \ #,##0.0;"*"\ \ \-#,##0.0"###,
    r###""*"\ \ \ 0.0;"*"\ \ \-0.0"###,
    r###""**"#,##0;"**"\-#,##0"###,
    r###""**"\ #,##0;"**"\-#,##0"###,
    r###""**"\ \ 0.00;"**"\ \-0.00"###,
    r###""**"\ \ 0.0;"**"\ \-0.0"###,
    r###""Rp"#,##0"###,
    r###""Total: "#,##0"###,
    r###""Total: "#,##0.00" yen""###,
    r###""True";"True";"False""###,
    r###""abc"@"###,
    r###""△"#,##0;"▲"#,##0"###,
    r###""はい";"はい";"いいえ""###,
    r###"#"###,
    r###"# "x" #"###,
    r###"# ?/8"###,
    r###"# ?/?"###,
    r###"# ??/??"###,
    r###"# ???/???"###,
    r###"##"###,
    r###"##0.0E+0"###,
    r###"#,#"###,
    r###"#,##"###,
    r###"#,###"###,
    r###"#,##0"###,
    r###"#,##0 "x""###,
    r###"#,##0" pcs""###,
    r###"#,##0"\""###,
    r###"#,##0"円""###,
    r###"#,##0%"###,
    r###"#,##0,"###,
    r###"#,##0,,"###,
    r###"#,##0-"###,
    r###"#,##0.0"###,
    r###"#,##0.0##"###,
    r###"#,##0.00"###,
    r###"#,##0.00;[Red]#,##0.00"###,
    r###"#,##0.00;[Red]-#,##0.00"###,
    r###"#,##0.00;[Red]\-#,##0.00"###,
    r###"#,##0.00\ [$€-1]"###,
    r###"#,##0.00\ [$€-407]"###,
    r###"#,##0.00\ [$€-40C]"###,
    r###"#,##0.00\ \€"###,
    r###"#,##0.00_);[Red](#,##0.00)"###,
    r###"#,##0;"△"#,##0"###,
    r###"#,##0;-#,##0"###,
    r###"#,##0;-#,##0;"###,
    r###"#,##0;-#,##0;"-""###,
    r###"#,##0;-#,##0;0;@"###,
    r###"#,##0;-#,##0;;"###,
    r###"#,##0;[Blue]-#,##0"###,
    r###"#,##0;[Red]#,##0"###,
    r###"#,##0;[Red]-#,##0"###,
    r###"#,##0;[Red]-#,##0;[Blue]0;[Green]@"###,
    r###"#,##0;[赤]-#,##0"###,
    r###"#,##0;\-#,##0;;"###,
    r###"#,##0;△#,##0"###,
    r###"#,##0\ "x""###,
    r###"#,##0\ \ "###,
    r###"#,##0\円"###,
    r###"#,##0_ "###,
    r###"#,##0_);(#,##0)"###,
    r###"#,##0_);[Red](#,##0)"###,
    r###"#,##0_);\(#,##0\)"###,
    r###"#,##0_;\-#,##0"###,
    r###"#.##"###,
    r###"#\ ##0"###,
    r###"$#,##0.00"###,
    r###"$#,##0.00_);($#,##0.00)"###,
    r###"$#,##0.00_);[Red]($#,##0.00)"###,
    r###"(#,##0)"###,
    r###"(aaa)"###,
    r###"* #,##0"###,
    r###"*-0"###,
    r###"+#,##0;-#,##0;0"###,
    r###"+0;-0;0"###,
    r###",0"###,
    r###"-#,##0"###,
    r###".00"###,
    r###"0"###,
    r###"0 "###,
    r###"0 "x""###,
    r###"0 "円""###,
    r###"0 0/0"###,
    r###"0 \ 0"###,
    r###"0"%""###,
    r###"0"円""###,
    r###"0%"###,
    r###"0,0"###,
    r###"0.#"###,
    r###"0.###"###,
    r###"0.0"###,
    r###"0.0" kg""###,
    r###"0.0"+""###,
    r###"0.0#"###,
    r###"0.0%"###,
    r###"0.0%;"▲"0.0%"###,
    r###"0.0,"###,
    r###"0.00"###,
    r###"0.00%"###,
    r###"0.00%;[Red]-0.00%"###,
    r###"0.00,"###,
    r###"0.00-"###,
    r###"0.0000"×10""###,
    r###"0.00;@"###,
    r###"0.00;[Red]-0.00;"-""###,
    r###"0.00E+00"###,
    r###"0.00E-00"###,
    r###"0.0E+00"###,
    r###"0.0\ \%"###,
    r###"0.0\%"###,
    r###"000"-"0000"###,
    r###"000-0000"###,
    r###"000.00"###,
    r###"0000"###,
    r###"0000"年"00"月""###,
    r###"00000"###,
    r###"00\-00\-00"###,
    r###"0;"neg";"zero""###,
    r###"0;-0;0;"t"@"###,
    r###"0;-0;;@"###,
    r###"0;-0;@"###,
    r###"0;-0;@;@"###,
    r###"0;;0"###,
    r###"0;@"###,
    r###"0;[Red]0"###,
    r###"0E+0"###,
    r###"0\ ?/?"###,
    r###"0_)"###,
    r###"0_)%"###,
    r###"0_);[Red]\(0\)"###,
    r###";;0"###,
    r###";;;"###,
    r###"?.??"###,
    r###"?/?"###,
    r###"@"###,
    r###"@" san""###,
    r###"@_)"###,
    r###"General" yen""###,
    r###"General;-General;"zero""###,
    r###"General;[Red]General"###,
    r###"[$€-2] #,##0.00"###,
    r###"[$₹-4009] #,##0.00"###,
    r###"[$€-407] #,##0.00"###,
    r###"[$₩-412]#,##0"###,
    r###"[$₽-419] #,##0.00"###,
    r###"[$¥-411]#,##0"###,
    r###"[$¥-411]#,##0;[Red]-[$¥-411]#,##0"###,
    r###"[$¥-411]#,##0;[Red]\-[$¥-411]#,##0"###,
    r###"[$¥-804]#,##0.00"###,
    r###"[$¥-ja-JP]#,##0"###,
    r###"[$$-409]#,##0.00"###,
    r###"[$$-C09]#,##0.00"###,
    r###"[$-1010411]yyyy/m/d"###,
    r###"[$-404]yyyy"年"m"月"d"日""###,
    r###"[$-404]yyyy/m/d"###,
    r###"[$-407]d. mmmm yyyy"###,
    r###"[$-407]dd/mm/yyyy"###,
    r###"[$-409]d-mmm-yy"###,
    r###"[$-409]dddd"###,
    r###"[$-409]h:mm AM/PM"###,
    r###"[$-409]h:mm:ss AM/PM"###,
    r###"[$-409]m/d/yyyy"###,
    r###"[$-409]mmmm\ yyyy;@"###,
    r###"[$-409]mmmm\-yy;@"###,
    r###"[$-409]yyyy-mm-dd"###,
    r###"[$-40A]d "de" mmmm "de" yyyy"###,
    r###"[$-40C]d mmmm yyyy"###,
    r###"[$-410]d mmmm yyyy"###,
    r###"[$-411]#,##0"###,
    r###"[$-411][DBNum1]0"###,
    r###"[$-411]aaa"###,
    r###"[$-411]aaaa"###,
    r###"[$-411]dddd"###,
    r###"[$-411]ggge"年""###,
    r###"[$-411]ggge"年"m"月"d"日""###,
    r###"[$-411]h"時"mm"分""###,
    r###"[$-411]h:mm"###,
    r###"[$-411]m"月"d"日""###,
    r###"[$-411]yyyy"年"m"月"d"日""###,
    r###"[$-411]yyyy/m/d"###,
    r###"[$-412]yyyy-mm-dd"###,
    r###"[$-416]d "de" mmmm "de" yyyy"###,
    r###"[$-419]d mmmm yyyy"###,
    r###"[$-41F]d mmmm yyyy"###,
    r###"[$-421]d mmmm yyyy"###,
    r###"[$-42A]d mmmm yyyy"###,
    r###"[$-61]yyyy"###,
    r###"[$-804]yyyy"年"m"月"d"日""###,
    r###"[$-809]dd/mm/yyyy"###,
    r###"[$-F400]h:mm:ss\ AM/PM"###,
    r###"[$-F800]dddd\,\ mmmm\ d\,\ yyyy"###,
    r###"[$-F800]dddd\,\ mmmm\ dd\,\ yyyy"###,
    r###"[$-ja-JP-x-calendar]yyyy"###,
    r###"[$CHF] #,##0.00"###,
    r###"[$USD] #,##0.00"###,
    r###"[$£-809]#,##0.00"###,
    r###"[$円-411]#,##0"###,
    r###"[$円]#,##0"###,
    r###"[<0][Red]0;0"###,
    r###"[<1000]0;[<1000000]0.0,"K";0.0,,"M""###,
    r###"[<1]0.00;[>=1]0"###,
    r###"[<=9999999]###-####;(###) ###-####"###,
    r###"[<>0]0;"zero""###,
    r###"[=1]"one";[=2]"two";0"###,
    r###"[>1000]#,##0,"K";0"###,
    r###"[>100]#,##0;[<=100]0.0"###,
    r###"[>=1000000]0.0,," M";[>=1000]0.0," K";0"###,
    r###"[>=100]#,##0;[<100]0.0;"x""###,
    r###"[Black]0"###,
    r###"[Blue]0.00"###,
    r###"[Color10]0"###,
    r###"[Color10]0.00"###,
    r###"[Color11]0"###,
    r###"[Color12]0"###,
    r###"[Color13]0"###,
    r###"[Color14]0"###,
    r###"[Color15]0"###,
    r###"[Color16]0"###,
    r###"[Color17]0"###,
    r###"[Color18]0"###,
    r###"[Color19]0"###,
    r###"[Color1]0"###,
    r###"[Color20]0"###,
    r###"[Color21]0"###,
    r###"[Color22]0"###,
    r###"[Color23]0"###,
    r###"[Color24]0"###,
    r###"[Color25]0"###,
    r###"[Color26]0"###,
    r###"[Color27]0"###,
    r###"[Color28]0"###,
    r###"[Color29]0"###,
    r###"[Color2]0"###,
    r###"[Color30]0"###,
    r###"[Color31]0"###,
    r###"[Color32]0"###,
    r###"[Color33]0"###,
    r###"[Color34]0"###,
    r###"[Color35]0"###,
    r###"[Color36]0"###,
    r###"[Color37]0"###,
    r###"[Color38]0"###,
    r###"[Color39]0"###,
    r###"[Color3]0"###,
    r###"[Color40]0"###,
    r###"[Color41]0"###,
    r###"[Color42]0"###,
    r###"[Color43]0"###,
    r###"[Color44]0"###,
    r###"[Color45]0"###,
    r###"[Color46]0"###,
    r###"[Color47]0"###,
    r###"[Color48]0"###,
    r###"[Color49]0"###,
    r###"[Color4]0"###,
    r###"[Color50]0"###,
    r###"[Color51]0"###,
    r###"[Color52]0"###,
    r###"[Color53]0"###,
    r###"[Color54]0"###,
    r###"[Color55]0"###,
    r###"[Color56]0"###,
    r###"[Color5]0"###,
    r###"[Color6]0"###,
    r###"[Color7]0"###,
    r###"[Color8]0"###,
    r###"[Color9]0"###,
    r###"[Cyan]0"###,
    r###"[DBNum1]#,##0"###,
    r###"[DBNum1]General"###,
    r###"[DBNum1][$-411]yyyy"年"m"月"d"日""###,
    r###"[DBNum2]0"###,
    r###"[DBNum3]0"###,
    r###"[Green]0"###,
    r###"[Magenta]0"###,
    r###"[Red]#,##0"###,
    r###"[Red]#,##0;[Blue]-#,##0;[Green]"zero""###,
    r###"[Red]-0"###,
    r###"[Red]0;[Blue]-0;[Green]0;[Yellow]@"###,
    r###"[Red]0;[Blue]0"###,
    r###"[Red][<0]0;0"###,
    r###"[White]0"###,
    r###"[Yellow]0"###,
    r###"[h]"時間"mm"分""###,
    r###"[h]:mm"###,
    r###"[h]:mm:ss"###,
    r###"[h]:mm:ss.00"###,
    r###"[hh]:mm"###,
    r###"[m]:ss"###,
    r###"[mm]:ss"###,
    r###"[s]"###,
    r###"[ss]"###,
    r###"\¥#,##0"###,
    r###"\$#,##0.00"###,
    r###"\(##\)"###,
    r###"\+0"###,
    r###"\-#,##0"###,
    r###"\Y\e\a\r\ yyyy"###,
    r###"_ * #,##0_ ;_ * -#,##0_ ;_ * "-"_ ;_ @_ "###,
    r###"_("$"* #,##0_);_("$"* \(#,##0\);_("$"* "-"_);_(@_)"###,
    r###"_(* #,##0.00_);_(* \(#,##0.00\);_(* "-"??_);_(@_)"###,
    r###"_(* #,##0_);_(* \(#,##0\);_(* "-"_);_(@_)"###,
    r###"_-* #,##0_-;\-* #,##0_-;_-* "-"_-;_-@_-"###,
    r###"aaa"###,
    r###"aaaa"###,
    r###"d"###,
    r###"d mmm"###,
    r###"d" "mmmm" "yyyy"###,
    r###"d-mmm"###,
    r###"d-mmm-yy"###,
    r###"d.m.yyyy"###,
    r###"d/m/yyyy"###,
    r###"dd"###,
    r###"dd-mmm-yyyy"###,
    r###"dd/mm/yyyy"###,
    r###"ddd"###,
    r###"dddd"###,
    r###"dddd, mmmm dd, yyyy"###,
    r###"ge.m.d"###,
    r###"gge"年""###,
    r###"ggge"年"m"月"d"日""###,
    r###"h"時"mm"分""###,
    r###"h"時"mm"分"ss"秒""###,
    r###"h:mm"###,
    r###"h:mm A/P"###,
    r###"h:mm AM/PM"###,
    r###"h:mm:ss"###,
    r###"h:mm:ss AM/PM"###,
    r###"h:mm:ss.000"###,
    r###"hh:mm"###,
    r###"hh:mm AM/PM"###,
    r###"hh:mm:ss"###,
    r###"hh:mm:ss.00"###,
    r###"m"###,
    r###"m"月"d"日""###,
    r###"m/d"###,
    r###"m/d/yy h:mm"###,
    r###"m/d/yyyy"###,
    r###"m/d/yyyy h:mm AM/PM"###,
    r###"mm"###,
    r###"mm-dd-yy"###,
    r###"mm/dd"###,
    r###"mm:ss"###,
    r###"mm:ss.0"###,
    r###"mmm"###,
    r###"mmm d, yyyy"###,
    r###"mmm-yy"###,
    r###"mmmm"###,
    r###"mmmm d, yyyy"###,
    r###"mmmm\ d\,\ yyyy"###,
    r###"mmmmm"###,
    r###"q"###,
    r###"ww"###,
    r###"y"###,
    r###"yy"###,
    r###"yy/m/d"###,
    r###"yyy"###,
    r###"yyyy"Q"Q"###,
    r###"yyyy"年"m"月""###,
    r###"yyyy"年"m"月"d"日""###,
    r###"yyyy-mm-dd"###,
    r###"yyyy-mm-dd"T"hh:mm:ss"###,
    r###"yyyy.mm.dd"###,
    r###"yyyy/m/d"###,
    r###"yyyy/m/d h:mm"###,
    r###"yyyy/m/d hh:mm:ss"###,
    r###"yyyy/m/d(aaa)"###,
    r###"yyyy/m/d\(aaa\)"###,
    r###"yyyy/mm/dd"###,
    r###"yyyy/mm/dd hh:mm"###,
    r###"yyyy\年m\月d\日"###,
    r###"yyyymmdd"###,
    r###"yyyy年m月d日"###,
    ];

    /// Values that show what a code does to a number, a negative number,
    /// zero, a fraction and a date
    const SAMPLES: [f64; 8] = [0.0, 1.0, -1.0, 1234.567, -1234.567, 0.5, 45123.75, 25569.0];

    fn render(code: &str) -> Vec<String> {
        SAMPLES
            .iter()
            .map(|v| book::format_value(&book::Value::Number(*v), Some(code), false))
            .collect()
    }

    /// Codes that are written but read back spelled differently on purpose,
    /// with the reason
    const SPELLED_DIFFERENTLY: &[&str] = &[
        // `[$-F800]` and `[$-F400]` name the system format, which has no language
        r"[$-F800]dddd\,\ mmmm\ d\,\ yyyy",
        r"[$-F800]dddd\,\ mmmm\ dd\,\ yyyy",
        // one era letter, one `AM/PM` spelling, one `q`, `y`, `yyy`, `mmmmm`
        "ge.m.d",
        "h:mm A/P",
        "mmmmm",
        "q",
        "y",
        "yyy",
    ];

    /// Codes the writer gives up on, with the reason
    const NOT_WRITTEN: &[&str] = &[
        // the Japanese name of a color
        "#,##0;[赤]-#,##0",
    ];

    /// A code with every quote and backslash taken out: the same text can
    /// be written with either
    fn bare(code: &str) -> String {
        code.chars().filter(|c| !matches!(c, '"' | '\\')).collect()
    }

    #[test]
    fn codes_read_back() {
        let (mut exact, mut same, mut spelled, mut none) = (0, 0, 0, 0);
        let mut differ = Vec::new();
        for code in CODES {
            if data_style("N1", code).is_none() {
                none += 1;
                assert!(NOT_WRITTEN.contains(code), "not written: {code:?}");
                continue;
            }
            let got = back(code).unwrap_or_default();
            if got == *code {
                exact += 1;
            } else if bare(&got) == bare(code) || render(&got) == render(code) {
                same += 1;
            } else if SPELLED_DIFFERENTLY.contains(code) {
                spelled += 1;
            } else {
                println!("DIFFER {code:?} -> {got:?}");
                differ.push(*code);
            }
        }
        println!("exact {exact} same {same} spelled {spelled} not written {none} differ {}", differ.len());
        assert!(differ.is_empty(), "codes that read back differently: {differ:?}");
    }

    /// The styles for `code` named `S`, the way they are written
    fn xml(code: &str) -> String {
        data_style("S", code).unwrap_or_else(|| "<none>".to_string())
    }

    #[test]
    fn allowed_differences_are_real() {
        for code in SPELLED_DIFFERENTLY {
            let got = back(code).unwrap_or_default();
            assert!(
                got != *code && bare(&got) != bare(code) && render(&got) != render(code),
                "{code:?} reads back as {got:?}"
            );
        }
    }

    #[test]
    fn not_written() {
        assert_eq!(data_style("S", ""), None);
        assert_eq!(data_style("S", "General"), None);
        assert_eq!(data_style("S", "general"), None);
        // an unknown tag, an unknown letter, an unclosed quote
        assert_eq!(data_style("S", "[Foo]0"), None);
        assert_eq!(data_style("S", "0 x"), None);
        assert_eq!(data_style("S", "0\\"), None);
        assert_eq!(data_style("S", "\\"), None);
        assert_eq!(data_style("S", "\"0"), None);
        // a condition on a single section, or on the first of three
        assert_eq!(data_style("S", "[>100]0"), None);
        assert_eq!(data_style("S", "[>100]0;0.0;0.00"), None);
        // five sections
        assert_eq!(data_style("S", "0;0;0;0;0"), None);
        // digits inside a date
        assert_eq!(data_style("S", "yyyy 0"), None);
    }

    // The expected text below is what LibreOffice 24.2 writes for the same
    // codes (read off ods files it made from xlsx files), apart from the
    // style names.

    #[test]
    fn numbers() {
        assert_eq!(
            xml("#,##0.00"),
            r#"<number:number-style style:name="S"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/></number:number-style>"#
        );
        assert_eq!(
            xml("?.??"),
            r#"<number:number-style style:name="S"><number:number number:decimal-places="2" number:min-decimal-places="0" number:decimal-replacement=" " number:min-integer-digits="1" loext:max-blank-integer-digits="1"/></number:number-style>"#
        );
        assert_eq!(
            xml("#,##0,,"),
            r#"<number:number-style style:name="S"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true" number:display-factor="1000000"/></number:number-style>"#
        );
        assert_eq!(
            xml(".00"),
            r#"<number:number-style style:name="S"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="0"/></number:number-style>"#
        );
        assert_eq!(
            xml("General\" yen\""),
            r#"<number:number-style style:name="S"><number:number number:min-integer-digits="1"/><number:text> yen</number:text></number:number-style>"#
        );
        assert_eq!(
            xml("000-0000"),
            r#"<number:number-style style:name="S"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="7"><number:embedded-text number:position="4">-</number:embedded-text></number:number></number:number-style>"#
        );
        assert_eq!(
            xml("0.00%"),
            r#"<number:percentage-style style:name="S"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1"/><number:text>%</number:text></number:percentage-style>"#
        );
    }

    #[test]
    fn scientific_and_fractions() {
        assert_eq!(
            xml("##0.0E+0"),
            r#"<number:number-style style:name="S"><number:scientific-number number:decimal-places="1" number:min-decimal-places="1" number:min-integer-digits="1" number:min-exponent-digits="1" number:exponent-interval="3" number:forced-exponent-sign="true"/></number:number-style>"#
        );
        assert_eq!(
            xml("0.00E-00"),
            r#"<number:number-style style:name="S"><number:scientific-number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:min-exponent-digits="2" number:exponent-interval="1" number:forced-exponent-sign="false"/></number:number-style>"#
        );
        assert_eq!(
            xml("# ?/?"),
            r#"<number:number-style style:name="S"><number:fraction number:min-integer-digits="0" number:min-numerator-digits="1" loext:max-numerator-digits="1" number:min-denominator-digits="1" number:max-denominator-value="9"/></number:number-style>"#
        );
        assert_eq!(
            xml("# ?/8"),
            r#"<number:number-style style:name="S"><number:fraction number:min-integer-digits="0" number:min-numerator-digits="1" loext:max-numerator-digits="1" number:denominator-value="8"/></number:number-style>"#
        );
    }

    #[test]
    fn dates_and_times() {
        assert_eq!(
            xml("yyyy/m/d h:mm"),
            r#"<number:date-style style:name="S"><number:year number:style="long"/><number:text>/</number:text><number:month/><number:text>/</number:text><number:day/><number:text> </number:text><number:hours/><number:text>:</number:text><number:minutes number:style="long"/></number:date-style>"#
        );
        assert_eq!(
            xml("[$-411]ggge\"年\"m\"月\"d\"日\""),
            r#"<number:date-style style:name="S" number:language="ja" number:country="JP"><number:era number:calendar="gengou" number:style="long"/><number:year number:calendar="gengou"/><number:text>年</number:text><number:month number:calendar="gengou"/><number:text>月</number:text><number:day number:calendar="gengou"/><number:text>日</number:text></number:date-style>"#
        );
        assert_eq!(
            xml("[h]:mm:ss.00"),
            r#"<number:time-style style:name="S" number:truncate-on-overflow="false"><number:hours/><number:text>:</number:text><number:minutes number:style="long"/><number:text>:</number:text><number:seconds number:style="long" number:decimal-places="2"/></number:time-style>"#
        );
        assert_eq!(
            xml("h:mm AM/PM"),
            r#"<number:time-style style:name="S"><number:hours/><number:text>:</number:text><number:minutes number:style="long"/><number:text> </number:text><number:am-pm/></number:time-style>"#
        );
        assert_eq!(
            xml("[$-409]dddd"),
            r#"<number:date-style style:name="S" number:language="en" number:country="US"><number:day-of-week number:style="long"/></number:date-style>"#
        );
        // `m` is the month except next to hours or seconds
        assert_eq!(
            xml("m/d h\"時\"m"),
            r#"<number:date-style style:name="S"><number:month/><number:text>/</number:text><number:day/><number:text> </number:text><number:hours/><number:text>時</number:text><number:minutes/></number:date-style>"#
        );
    }

    #[test]
    fn currency() {
        assert_eq!(
            xml("[$¥-411]#,##0"),
            r#"<number:currency-style style:name="S"><number:currency-symbol number:language="ja" number:country="JP">¥</number:currency-symbol><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:currency-style>"#
        );
        assert_eq!(
            xml("[$CHF] #,##0.00"),
            r#"<number:currency-style style:name="S"><number:currency-symbol>CHF</number:currency-symbol><number:text> </number:text><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/></number:currency-style>"#
        );
    }

    #[test]
    fn sections() {
        // two sections: the first is reached with `>=0`, the second is the style
        assert_eq!(
            xml("#,##0.00;[Red]-#,##0.00"),
            concat!(
                r#"<number:number-style style:name="SP0" style:volatile="true"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/></number:number-style>"#,
                "\n",
                r##"<number:number-style style:name="S"><style:text-properties fo:color="#ff0000"/><number:text>-</number:text><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/><style:map style:condition="value()&gt;=0" style:apply-style-name="SP0"/></number:number-style>"##
            )
        );
        // four sections: the style is the text section
        assert_eq!(
            xml("0;-0;;@"),
            concat!(
                r#"<number:number-style style:name="SP0" style:volatile="true"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1"/></number:number-style>"#,
                "\n",
                r#"<number:number-style style:name="SP1" style:volatile="true"><number:text>-</number:text><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1"/></number:number-style>"#,
                "\n",
                r#"<number:number-style style:name="SP2" style:volatile="true"><number:text/></number:number-style>"#,
                "\n",
                r#"<number:text-style style:name="S"><number:text-content/><style:map style:condition="value()&gt;0" style:apply-style-name="SP0"/><style:map style:condition="value()&lt;0" style:apply-style-name="SP1"/><style:map style:condition="value()=0" style:apply-style-name="SP2"/></number:text-style>"#
            )
        );
        // `0;@`: every number goes to the first section
        assert!(xml("0;@").contains(r#"style:condition="value()&lt;=1.7976931348623157E+308""#));
        // two conditions: LibreOffice adds a General style for the rest
        let x = xml("[<1]0.00;[>=1]0");
        assert!(x.ends_with(r#"<number:number-style style:name="S"><number:number number:min-integer-digits="1"/><style:map style:condition="value()&lt;1" style:apply-style-name="SP0"/><style:map style:condition="value()&gt;=1" style:apply-style-name="SP1"/></number:number-style>"#));
        // a condition and `<>`
        assert!(xml("[<>0]0;\"zero\"").contains(r#"style:condition="value()!=0""#));
    }

    #[test]
    fn blanks_and_fill() {
        // each blank is a text element of its own
        assert_eq!(
            xml("0_)%"),
            r#"<number:percentage-style style:name="S"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1"/><number:text loext:blank-width-char=")"> </number:text><number:text>%</number:text></number:percentage-style>"#
        );
        assert_eq!(
            xml("_-* #,##0"),
            r#"<number:number-style style:name="S"><number:text loext:blank-width-char="-"> </number:text><number:fill-character> </number:fill-character><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>"#
        );
        // the names and the texts are escaped
        assert_eq!(
            xml("\"a<b&c\""),
            r#"<number:number-style style:name="S"><number:text>a&lt;b&amp;c</number:text></number:number-style>"#
        );
        assert!(data_style("A\"B", "0").unwrap().contains(r#"style:name="A&quot;B""#));
    }
}
