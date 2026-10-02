//! ODF data styles (`number:number-style`, `number:date-style`, …) as the
//! format codes the engine and xlsx use (`#,##0`, `yyyy/m/d`).
//!
//! The shapes handled here were checked against what LibreOffice 24.2 writes
//! when it converts xlsx files to ods (the corpus in `~/ods-corpus` and a
//! few hundred codes written for the purpose). Where a rule comes from those
//! files it says so.
//!
//! One Excel code can become several ODF styles. LibreOffice writes the last
//! section as the style itself and the earlier sections as separate styles
//! (`N104P0`, `N104P1`, hidden with `style:volatile`) that the main style
//! reaches through `style:map`. Those maps are put back into one code here.

use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// Every data style in one XML part (content.xml or styles.xml), by name,
/// as a format code. Styles this cannot express are left out
pub fn parse_data_styles(xml: &str) -> HashMap<String, String> {
    let styles = scan(xml);
    let by_name: HashMap<&str, &Style> = styles.iter().map(|s| (s.name.as_str(), s)).collect();
    let mut out = HashMap::new();
    for s in &styles {
        if s.name.is_empty() {
            continue;
        }
        if let Some(code) = code_of(s, &by_name) {
            out.insert(s.name.clone(), code);
        }
    }
    out
}

// ---------------------------------------------------------------- reading

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Number,
    Percentage,
    Currency,
    Date,
    Time,
    Boolean,
    Text,
}

/// The attributes of one element, by qualified name (`number:style`)
#[derive(Default, Clone, Debug)]
struct Attrs(Vec<(String, String)>);

impl Attrs {
    fn of(e: &BytesStart) -> Attrs {
        Attrs(
            e.attributes()
                .flatten()
                .map(|a| {
                    (
                        String::from_utf8_lossy(a.key.as_ref()).into_owned(),
                        a.unescape_value().map(|v| v.into_owned()).unwrap_or_default(),
                    )
                })
                .collect(),
        )
    }

    fn get(&self, k: &str) -> Option<&str> {
        self.0.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
    }

    fn num(&self, k: &str) -> Option<usize> {
        self.get(k).and_then(|v| v.trim().parse().ok())
    }

    fn flag(&self, k: &str) -> bool {
        self.get(k) == Some("true")
    }
}

/// Text put inside the integer digits of a number (`000-0000`).
/// (position counted in digits from the right, text, blank-width char)
type Embedded = (usize, String, Option<Blank>);

/// `loext:blank-width-char`: a character and the index of the text's
/// character that takes that character's width instead of showing
/// (Excel `_)`). LibreOffice writes `)` for the first character and `-1`
/// for the second character of `- ` (read off converted accounting formats)
type Blank = (char, usize);

#[derive(Debug)]
enum Item {
    /// `number:text`. `blank` is `loext:blank-width-char`: the text takes
    /// the width of that character instead of showing (Excel `_)`)
    Text { s: String, blank: Option<Blank> },
    /// `number:fill-character` (Excel `*x`)
    Fill(char),
    Number { a: Attrs, embedded: Vec<Embedded> },
    Scientific(Attrs),
    Fraction(Attrs),
    Currency { sym: String, lang: Option<String>, country: Option<String> },
    /// `number:text-content`: the cell's own text (Excel `@`)
    TextContent,
    /// year, month, day, day-of-week, era, quarter, week-of-year, hours,
    /// minutes, seconds
    Part { name: String, a: Attrs },
    AmPm,
    Boolean,
}

#[derive(Debug)]
struct Style {
    kind: Kind,
    name: String,
    a: Attrs,
    /// `fo:color` of `style:text-properties`, as written (`#ff0000`)
    color: Option<String>,
    items: Vec<Item>,
    /// (`style:condition`, `style:apply-style-name`)
    maps: Vec<(String, String)>,
}

fn style_kind(name: &str) -> Option<Kind> {
    Some(match name {
        "number:number-style" => Kind::Number,
        "number:percentage-style" => Kind::Percentage,
        "number:currency-style" => Kind::Currency,
        "number:date-style" => Kind::Date,
        "number:time-style" => Kind::Time,
        "number:boolean-style" => Kind::Boolean,
        "number:text-style" => Kind::Text,
        _ => return None,
    })
}

/// What the text inside the element being read is for
enum Target {
    Text(Option<Blank>),
    Embedded(usize, Option<Blank>),
    Symbol(Option<String>, Option<String>),
    Fill,
}

#[derive(Default)]
struct State {
    out: Vec<Style>,
    cur: Option<Style>,
    target: Option<Target>,
    buf: String,
}

fn blank_char(a: &Attrs) -> Option<Blank> {
    let v = a.get("loext:blank-width-char")?;
    let mut it = v.chars();
    let c = it.next()?;
    let rest: String = it.collect();
    let idx = if rest.is_empty() { 0 } else { rest.parse().ok()? };
    Some((c, idx))
}

fn start(e: &BytesStart, empty: bool, st: &mut State) {
    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    if let Some(kind) = style_kind(&name) {
        let a = Attrs::of(e);
        let style = Style {
            kind,
            name: a.get("style:name").unwrap_or_default().to_string(),
            a,
            color: None,
            items: Vec::new(),
            maps: Vec::new(),
        };
        if empty {
            st.out.push(style);
        } else {
            st.cur = Some(style);
        }
        return;
    }
    let Some(cur) = st.cur.as_mut() else { return };
    let a = Attrs::of(e);
    match name.as_str() {
        "style:text-properties" => {
            if let Some(c) = a.get("fo:color") {
                cur.color = Some(c.to_ascii_lowercase());
            }
        }
        "style:map" => {
            if let (Some(c), Some(n)) = (a.get("style:condition"), a.get("style:apply-style-name")) {
                cur.maps.push((c.to_string(), n.to_string()));
            }
        }
        "number:text" => {
            let blank = blank_char(&a);
            if empty {
                cur.items.push(Item::Text { s: String::new(), blank });
            } else {
                st.target = Some(Target::Text(blank));
                st.buf.clear();
            }
        }
        "number:number" => cur.items.push(Item::Number { a, embedded: Vec::new() }),
        "number:embedded-text" => {
            let pos = a.num("number:position").unwrap_or(0);
            let blank = blank_char(&a);
            if empty {
                if let Some(Item::Number { embedded, .. }) = cur.items.last_mut() {
                    embedded.push((pos, String::new(), blank));
                }
            } else {
                st.target = Some(Target::Embedded(pos, blank));
                st.buf.clear();
            }
        }
        "number:scientific-number" => cur.items.push(Item::Scientific(a)),
        "number:fraction" => cur.items.push(Item::Fraction(a)),
        "number:currency-symbol" => {
            let lang = a.get("number:language").map(str::to_string);
            let country = a.get("number:country").map(str::to_string);
            if empty {
                cur.items.push(Item::Currency { sym: String::new(), lang, country });
            } else {
                st.target = Some(Target::Symbol(lang, country));
                st.buf.clear();
            }
        }
        "number:fill-character" => {
            if empty {
                cur.items.push(Item::Fill(' '));
            } else {
                st.target = Some(Target::Fill);
                st.buf.clear();
            }
        }
        "number:text-content" => cur.items.push(Item::TextContent),
        "number:am-pm" => cur.items.push(Item::AmPm),
        "number:boolean" => cur.items.push(Item::Boolean),
        "number:year" | "number:month" | "number:day" | "number:day-of-week" | "number:era"
        | "number:quarter" | "number:week-of-year" | "number:hours" | "number:minutes"
        | "number:seconds" => cur.items.push(Item::Part {
            name: name["number:".len()..].to_string(),
            a,
        }),
        _ => {}
    }
}

fn end(name: &str, st: &mut State) {
    if style_kind(name).is_some() {
        st.target = None;
        if let Some(s) = st.cur.take() {
            st.out.push(s);
        }
        return;
    }
    let Some(cur) = st.cur.as_mut() else { return };
    let text = std::mem::take(&mut st.buf);
    match (name, st.target.take()) {
        ("number:text", Some(Target::Text(blank))) => cur.items.push(Item::Text { s: text, blank }),
        ("number:embedded-text", Some(Target::Embedded(pos, blank))) => {
            if let Some(Item::Number { embedded, .. }) = cur.items.last_mut() {
                embedded.push((pos, text, blank));
            }
        }
        ("number:currency-symbol", Some(Target::Symbol(lang, country))) => {
            cur.items.push(Item::Currency { sym: text, lang, country })
        }
        ("number:fill-character", Some(Target::Fill)) => {
            cur.items.push(Item::Fill(text.chars().next().unwrap_or(' ')))
        }
        (_, t) => st.target = t,
    }
}

fn scan(xml: &str) -> Vec<Style> {
    let mut st = State::default();
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event() {
            // Data styles come before the body in content.xml
            Ok(Event::Start(e)) if e.name().as_ref() == b"office:body" => break,
            Ok(Event::Start(e)) => start(&e, false, &mut st),
            Ok(Event::Empty(e)) => start(&e, true, &mut st),
            Ok(Event::Text(t)) => {
                if st.target.is_some() {
                    st.buf.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => end(&String::from_utf8_lossy(e.name().as_ref()), &mut st),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    st.out
}

// ---------------------------------------------------------------- writing

/// One section of an Excel code, before the sections are joined
struct Section {
    color: String,
    /// Locale tag and digits and text, everything after color and condition
    body: String,
}

impl Section {
    fn of(s: &Style) -> Option<Section> {
        Some(Section { color: color_tag(s.color.as_deref()), body: body_of(s)? })
    }

    fn with_condition(&self, cond: &str) -> String {
        format!("{}{}{}", self.color, cond, self.body)
    }
}

/// The condition of a `style:map` as Excel writes it (`>=0`), or `None`
/// when it is about anything but the value
fn excel_condition(c: &str) -> Option<String> {
    let c: String = c.chars().filter(|c| !c.is_whitespace()).collect();
    let rest = c.strip_prefix("value()")?;
    Some(rest.replacen("!=", "<>", 1))
}

/// Whether the number is the biggest double: LibreOffice writes this
/// condition to say "every number" for the number part of `0.00;@`
/// (confirmed on files converted from xlsx)
fn is_any_number(cond: &str) -> bool {
    cond.starts_with("<=1.79769313486231") && cond.contains("E+308")
}

fn code_of(s: &Style, by_name: &HashMap<&str, &Style>) -> Option<String> {
    if s.kind == Kind::Boolean {
        // Excel has no format for logical values; they show as TRUE or FALSE
        return Some("General".to_string());
    }
    let main = Section::of(s)?;
    if s.maps.is_empty() {
        return Some(main.with_condition(""));
    }
    let mut conds = Vec::new();
    let mut parts = Vec::new();
    for (c, n) in &s.maps {
        conds.push(excel_condition(c)?);
        parts.push(Section::of(by_name.get(n.as_str())?)?);
    }
    let text = s.kind == Kind::Text;
    // The condition sets Excel implies by the number of sections (confirmed
    // on files converted from xlsx): `pos;neg` is `>=0` and the rest,
    // `pos;neg;zero` is `>0`, `<0` and the rest, and a text style adds the
    // fourth section for text
    let cs: Vec<&str> = conds.iter().map(String::as_str).collect();
    let implied = match (text, cs.as_slice()) {
        (false, [">=0"]) | (false, [">0", "<0"]) => true,
        (true, [">0", "<0", "=0"]) | (true, [">=0", "<0"]) => true,
        (true, [c]) => is_any_number(c),
        _ => false,
    };
    let mut secs: Vec<String> = Vec::new();
    if implied {
        secs.extend(parts.iter().map(|p| p.with_condition("")));
        secs.push(main.with_condition(""));
    } else {
        // A text style cannot say which numbers go where, and Excel takes
        // at most two conditions
        if text || parts.len() > 2 {
            return None;
        }
        for (p, c) in parts.iter().zip(&conds) {
            secs.push(p.with_condition(&format!("[{c}]")));
        }
        // LibreOffice adds an "everything else" section in General when
        // the conditions cover the numbers (`[>100]0;[<=100]0.0`)
        let general_rest = main.body == "General" && main.color.is_empty();
        if !(general_rest && !parts.is_empty()) {
            secs.push(main.with_condition(""));
        }
    }
    Some(secs.join(";"))
}

/// Excel's names for the eight colors, then `[ColorN]` for LibreOffice's
/// palette. Colors that are in neither are dropped: they only tint the text
fn color_tag(c: Option<&str>) -> String {
    let Some(c) = c else { return String::new() };
    let named = match c {
        "#000000" => Some("Black"),
        "#0000ff" => Some("Blue"),
        "#00ffff" => Some("Cyan"),
        "#00ff00" => Some("Green"),
        "#ff00ff" => Some("Magenta"),
        "#ff0000" => Some("Red"),
        "#ffffff" => Some("White"),
        "#ffff00" => Some("Yellow"),
        _ => None,
    };
    if let Some(n) = named {
        return format!("[{n}]");
    }
    // LibreOffice reads `[ColorN]` from this palette (confirmed by writing
    // all 56 codes in an xlsx and converting)
    let hex = c.trim_start_matches('#');
    match LO_PALETTE.iter().position(|p| *p == hex) {
        Some(i) => format!("[Color{}]", i + 1),
        None => String::new(),
    }
}

const LO_PALETTE: [&str; 56] = [
    "000000", "111111", "1c1c1c", "333333", "666666", "808080", "999999", "b2b2b2", "cccccc",
    "dddddd", "eeeeee", "ffffff", "ffff00", "ffbf00", "ff8000", "ff4000", "ff0000", "bf0041",
    "800080", "55308d", "2a6099", "158466", "00a933", "81d41a", "ffffd7", "fff5ce", "ffdbb6",
    "ffd8ce", "ffd7d7", "f7d1d5", "e0c2cd", "dedce6", "dee6ef", "dee7e5", "dde8cb", "f6f9d4",
    "ffffa6", "ffe994", "ffb66c", "ffaa95", "ffa6a6", "ec9ba4", "bf819e", "b7b3ca", "b4c7dc",
    "b3cac7", "afd095", "e8f2a1", "ffff6d", "ffde59", "ff972f", "ff7b59", "ff6d6d", "e16173",
    "a1467e", "8e86ae",
];

/// How a literal is written into the code
#[derive(Clone, Copy, PartialEq)]
enum Ctx {
    Number,
    Percentage,
    /// Dates and times take their separators as they are
    DateTime,
}

/// CJK characters, which are never code letters and stay unquoted
fn wide(c: char) -> bool {
    matches!(c, '\u{3000}'..='\u{9fff}' | '\u{ac00}'..='\u{d7af}' | '\u{ff00}'..='\u{ffef}')
}

/// Literal text as an Excel code writes it. Currency signs and CJK
/// characters stay as they are, punctuation Excel allows without quotes
/// gets a backslash, anything else goes in double quotes. Date and time
/// codes leave the common separators (`/ - : . , ( )` and the space) bare,
/// as Excel's own date formats do. (Both spellings were found in the
/// xlsx files of the corpus for the same ODF style, so this picks the
/// usual Excel one: `\-` in numbers, `-` in dates.)
fn lit(s: &str, ctx: Ctx) -> String {
    // Text with words or digits in it is quoted whole (`"Total: "`), as
    // Excel writes it
    if s.chars().any(|c| c.is_alphanumeric() && !wide(c)) && !s.contains(['"', '\\']) {
        return format!("\"{s}\"");
    }
    let mut out = String::new();
    let mut run = String::new();
    let flush = |out: &mut String, run: &mut String| {
        if !run.is_empty() {
            out.push('"');
            out.push_str(run);
            out.push('"');
            run.clear();
        }
    };
    for c in s.chars() {
        let bare = c == '$'
            || "¥£€¢₩₽₹".contains(c)
            || wide(c)
            || (ctx == Ctx::Percentage && c == '%')
            || (ctx == Ctx::DateTime && " -/:.,()'".contains(c));
        let escaped = " -+()/:!^&'~{}<>=,.%".contains(c);
        if c == '"' || c == '\\' {
            flush(&mut out, &mut run);
            out.push('\\');
            out.push(c);
        } else if bare {
            flush(&mut out, &mut run);
            out.push(c);
        } else if escaped {
            flush(&mut out, &mut run);
            out.push('\\');
            out.push(c);
        } else {
            run.push(c);
        }
    }
    flush(&mut out, &mut run);
    out
}

/// A `number:text` with its blank-width character: one character of the
/// text is replaced by the space of that character (`_)`)
fn text_item(s: &str, blank: Option<Blank>, ctx: Ctx) -> String {
    let Some((b, idx)) = blank else { return lit(s, ctx) };
    let chars: Vec<char> = s.chars().collect();
    if idx >= chars.len() {
        return lit(s, ctx);
    }
    let before: String = chars[..idx].iter().collect();
    let after: String = chars[idx + 1..].iter().collect();
    format!("{}_{}{}", lit(&before, ctx), b, lit(&after, ctx))
}

/// Whether a `number:number` is the General format: no decimal places are
/// given, so as many as the number needs are shown
fn is_general(a: &Attrs, embedded: &[Embedded]) -> bool {
    a.get("number:decimal-places").is_none()
        && !a.flag("number:grouping")
        && a.num("number:min-integer-digits").unwrap_or(1) <= 1
        && a.get("number:display-factor").is_none()
        && a.get("loext:max-blank-integer-digits").is_none()
        && embedded.is_empty()
}

/// `.00`, `.0#`, `.##`: the decimals of a number
fn decimals(a: &Attrs) -> String {
    let dp = a.num("number:decimal-places").unwrap_or(0);
    if dp == 0 {
        return String::new();
    }
    let mdp = a.num("number:min-decimal-places").unwrap_or(dp).min(dp);
    // A blank (` `) replacement for the optional places is Excel's `?`
    let opt = if a.get("number:decimal-replacement") == Some(" ") { '?' } else { '#' };
    let mut s = String::from(".");
    s.push_str(&"0".repeat(mdp));
    s.extend(std::iter::repeat_n(opt, dp - mdp));
    s
}

/// The digits of a `number:number`: `#,##0.00`, `000-0000`, `0.0,`
fn number_pattern(a: &Attrs, embedded: &[Embedded], ctx: Ctx) -> String {
    let blank = a.num("loext:max-blank-integer-digits").unwrap_or(0);
    let mid = a.num("number:min-integer-digits").unwrap_or(if blank > 0 { 0 } else { 1 });
    let grouping = a.flag("number:grouping");
    let dec = decimals(a);
    // Digits to the left of the text that sits furthest left
    let left = embedded.iter().map(|e| e.0 + 1).max().unwrap_or(0);
    let mut width = mid.max(left).max(blank);
    if grouping {
        width = width.max(4);
    }
    // No integer digits and no decimals: `##`, as in the files
    if width == 0 {
        width = if dec.is_empty() { 2 } else { 1 };
    }
    let mut int = String::new();
    for i in (0..width).rev() {
        if let Some((_, t, b)) = embedded.iter().find(|e| e.0 == i + 1) {
            int.push_str(&text_item(t, *b, ctx));
        }
        int.push(if i < blank {
            '?'
        } else if i < mid {
            '0'
        } else {
            '#'
        });
        if grouping && i > 0 && i % 3 == 0 {
            int.push(',');
        }
    }
    // Embedded text to the right of every digit (position 0)
    if let Some((_, t, b)) = embedded.iter().find(|e| e.0 == 0) {
        int.push_str(&text_item(t, *b, ctx));
    }
    let mut out = int;
    out.push_str(&dec);
    // Each `,` after the digits divides by 1000
    let factor = a.get("number:display-factor").and_then(|v| v.parse::<f64>().ok()).unwrap_or(1.0);
    let mut f = factor;
    while f >= 1000.0 {
        out.push(',');
        f /= 1000.0;
    }
    out
}

fn scientific_pattern(a: &Attrs) -> String {
    let mid = a.num("number:min-integer-digits").unwrap_or(1);
    let interval = a.num("number:exponent-interval").unwrap_or(1);
    let mut s = if interval > 1 {
        format!("{}0", "#".repeat(interval - 1))
    } else {
        "0".repeat(mid.max(1))
    };
    s.push_str(&decimals(a));
    s.push('E');
    s.push(if a.get("number:forced-exponent-sign") == Some("false") { '-' } else { '+' });
    s.push_str(&"0".repeat(a.num("number:min-exponent-digits").unwrap_or(1).max(1)));
    s
}

fn digit_count(n: usize) -> usize {
    n.to_string().len()
}

fn fraction_pattern(a: &Attrs) -> String {
    let pad = |width: usize, zeros: usize| {
        let zeros = zeros.min(width);
        format!("{}{}", "?".repeat(width - zeros), "0".repeat(zeros))
    };
    let mut s = String::new();
    // An integer part is there when `min-integer-digits` is (0 means `#`)
    if let Some(m) = a.num("number:min-integer-digits") {
        s.push_str(&if m == 0 { "#".to_string() } else { "0".repeat(m) });
        s.push(' ');
    }
    let nw = a
        .num("number:min-numerator-digits")
        .unwrap_or(1)
        .max(a.num("loext:max-numerator-digits").unwrap_or(1));
    s.push_str(&pad(nw, a.num("loext:zeros-numerator-digits").unwrap_or(0)));
    s.push('/');
    if let Some(d) = a.num("number:denominator-value") {
        s.push_str(&d.to_string());
    } else {
        let dw = a
            .num("number:min-denominator-digits")
            .unwrap_or(1)
            .max(digit_count(a.num("number:max-denominator-value").unwrap_or(9)));
        s.push_str(&pad(dw, a.num("loext:zeros-denominator-digits").unwrap_or(0)));
    }
    s
}

/// Windows locale ids of the language and country in ODF, as Excel writes
/// them in `[$-411]`. A language alone takes its usual country
fn lcid(lang: &str, country: Option<&str>) -> Option<u32> {
    let lang = lang.to_ascii_lowercase();
    let country = country.map(|c| c.to_ascii_uppercase());
    let by_country = match (lang.as_str(), country.as_deref()) {
        ("ja", Some("JP")) => 0x411,
        ("en", Some("US")) => 0x409,
        ("en", Some("GB")) => 0x809,
        ("en", Some("AU")) => 0xC09,
        ("en", Some("CA")) => 0x1009,
        ("en", Some("NZ")) => 0x1409,
        ("en", Some("IE")) => 0x1809,
        ("en", Some("ZA")) => 0x1C09,
        ("en", Some("IN")) => 0x4009,
        ("de", Some("DE")) => 0x407,
        ("de", Some("CH")) => 0x807,
        ("de", Some("AT")) => 0xC07,
        ("fr", Some("FR")) => 0x40C,
        ("fr", Some("BE")) => 0x80C,
        ("fr", Some("CA")) => 0xC0C,
        ("fr", Some("CH")) => 0x100C,
        ("es", Some("ES")) => 0x40A,
        ("es", Some("MX")) => 0x80A,
        ("es", Some("CO")) => 0x240A,
        ("es", Some("AR")) => 0x2C0A,
        ("es", Some("CL")) => 0x340A,
        ("it", Some("IT")) => 0x410,
        ("it", Some("CH")) => 0x810,
        ("pt", Some("BR")) => 0x416,
        ("pt", Some("PT")) => 0x816,
        ("ru", Some("RU")) => 0x419,
        ("tr", Some("TR")) => 0x41F,
        ("vi", Some("VN")) => 0x42A,
        ("id", Some("ID")) => 0x421,
        ("ko", Some("KR")) => 0x412,
        ("zh", Some("CN")) => 0x804,
        ("zh", Some("TW")) => 0x404,
        ("zh", Some("HK")) => 0xC04,
        ("zh", Some("SG")) => 0x1004,
        ("nl", Some("NL")) => 0x413,
        ("nl", Some("BE")) => 0x813,
        ("sv", Some("SE")) => 0x41D,
        ("pl", Some("PL")) => 0x415,
        ("da", Some("DK")) => 0x406,
        ("fi", Some("FI")) => 0x40B,
        ("nb", Some("NO")) | ("no", Some("NO")) => 0x414,
        ("cs", Some("CZ")) => 0x405,
        ("hu", Some("HU")) => 0x40E,
        ("el", Some("GR")) => 0x408,
        ("he", Some("IL")) => 0x40D,
        ("th", Some("TH")) => 0x41E,
        ("uk", Some("UA")) => 0x422,
        ("ar", Some("SA")) => 0x401,
        ("hi", Some("IN")) => 0x439,
        ("ro", Some("RO")) => 0x418,
        ("bg", Some("BG")) => 0x402,
        _ => 0,
    };
    if by_country != 0 {
        return Some(by_country);
    }
    if country.is_some() {
        return None;
    }
    // LibreOffice writes only the language for a tag like `[$€-1]`: the
    // neutral Arabic id, which it reads back as plain "ar"
    Some(match lang.as_str() {
        "ja" => 0x411,
        "en" => 0x409,
        "de" => 0x407,
        "fr" => 0x40C,
        "es" => 0x40A,
        "it" => 0x410,
        "pt" => 0x416,
        "ru" => 0x419,
        "tr" => 0x41F,
        "vi" => 0x42A,
        "id" => 0x421,
        "ko" => 0x412,
        "zh" => 0x804,
        "ar" => 0x1,
        _ => return None,
    })
}

/// `[$¥-411]`: a currency sign with its locale, as Excel writes it
fn currency_tag(sym: &str, lang: Option<&str>, country: Option<&str>) -> String {
    let sym = sym.replace(']', "");
    match lang.and_then(|l| lcid(l, country)) {
        Some(id) => format!("[${sym}-{id:X}]"),
        None => format!("[${sym}]"),
    }
}

fn part_code(name: &str, a: &Attrs) -> String {
    let long = a.get("number:style") == Some("long");
    let gengou = a.get("number:calendar") == Some("gengou");
    let pick = |s: &str, l: &str| if long { l } else { s }.to_string();
    match name {
        "year" if gengou => pick("e", "ee"),
        "year" => pick("yy", "yyyy"),
        "month" if a.flag("number:textual") => pick("mmm", "mmmm"),
        "month" => pick("m", "mm"),
        "day" => pick("d", "dd"),
        // `aaa` is the weekday in the Japanese calendar's locale
        "day-of-week" if gengou => pick("aaa", "aaaa"),
        "day-of-week" => pick("ddd", "dddd"),
        "era" => pick("gg", "ggg"),
        "quarter" => "Q".to_string(),
        "week-of-year" => "ww".to_string(),
        "hours" => pick("h", "hh"),
        "minutes" => pick("m", "mm"),
        "seconds" => {
            let mut s = pick("s", "ss");
            let dp = a.num("number:decimal-places").unwrap_or(0);
            if dp > 0 {
                s.push('.');
                s.push_str(&"0".repeat(dp));
            }
            s
        }
        _ => String::new(),
    }
}

/// The code of one style without its color and conditions
fn body_of(s: &Style) -> Option<String> {
    let ctx = match s.kind {
        Kind::Percentage => Ctx::Percentage,
        Kind::Date | Kind::Time => Ctx::DateTime,
        _ => Ctx::Number,
    };
    let mut out = String::new();
    // `number:truncate-on-overflow="false"`: the first of hours, minutes
    // and seconds counts past its range (`[h]`)
    let mut elapsed = s.a.get("number:truncate-on-overflow") == Some("false");
    // A date or time with names or era in it carries its locale
    // (`[$-409]mmmm yyyy`; confirmed on converted files)
    if matches!(s.kind, Kind::Date | Kind::Time) {
        if let Some(lang) = s.a.get("number:language") {
            if let Some(id) = lcid(lang, s.a.get("number:country")) {
                out.push_str(&format!("[$-{id:X}]"));
            }
        }
    }
    for it in &s.items {
        match it {
            Item::Text { s, blank } => out.push_str(&text_item(s, *blank, ctx)),
            Item::Fill(c) => {
                out.push('*');
                out.push(*c);
            }
            Item::Number { a, embedded } => {
                if is_general(a, embedded) {
                    out.push_str(if s.kind == Kind::Percentage { "0" } else { "General" });
                } else {
                    out.push_str(&number_pattern(a, embedded, ctx));
                }
            }
            Item::Scientific(a) => out.push_str(&scientific_pattern(a)),
            Item::Fraction(a) => out.push_str(&fraction_pattern(a)),
            Item::Currency { sym, lang, country } => {
                out.push_str(&currency_tag(sym, lang.as_deref(), country.as_deref()))
            }
            Item::TextContent => out.push('@'),
            Item::Part { name, a } => {
                let code = part_code(name, a);
                if elapsed && matches!(name.as_str(), "hours" | "minutes" | "seconds") {
                    elapsed = false;
                    out.push('[');
                    out.push_str(&code);
                    out.push(']');
                } else {
                    out.push_str(&code);
                }
            }
            Item::AmPm => out.push_str("AM/PM"),
            Item::Boolean => {}
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The code of the style `name` in a snippet of data styles
    fn code(xml: &str, name: &str) -> String {
        let doc = format!("<office:document-content>{xml}</office:document-content>");
        parse_data_styles(&doc).get(name).cloned().unwrap_or_else(|| "<none>".to_string())
    }

    // The snippets below are copied from ods files LibreOffice 24.2 made
    // from xlsx files. The comment on each is the code of the xlsx cell.

    #[test]
    fn plain_numbers() {
        let n = |dp: u8, grouping: bool| {
            format!(
                r#"<number:number-style style:name="S"><number:number number:decimal-places="{dp}" number:min-decimal-places="{dp}" number:min-integer-digits="1"{}/></number:number-style>"#,
                if grouping { r#" number:grouping="true""# } else { "" }
            )
        };
        assert_eq!(code(&n(0, false), "S"), "0");
        assert_eq!(code(&n(2, false), "S"), "0.00");
        assert_eq!(code(&n(0, true), "S"), "#,##0");
        assert_eq!(code(&n(2, true), "S"), "#,##0.00");
        // 0000
        let x = r#"<number:number-style style:name="S"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="4"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "0000");
        // ##
        let x = r#"<number:number-style style:name="S"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="0"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "##");
        // #.## and 0.0# and .00
        let x = r#"<number:number-style style:name="S"><number:number number:decimal-places="2" number:min-decimal-places="0" number:decimal-replacement="" number:min-integer-digits="0"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "#.##");
        let x = r#"<number:number-style style:name="S"><number:number number:decimal-places="2" number:min-decimal-places="1" number:decimal-replacement=""/></number:number-style>"#;
        assert_eq!(code(x, "S"), "0.0#");
        // #,##0,, and 0.0,
        let x = r#"<number:number-style style:name="S"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true" number:display-factor="1000000"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "#,##0,,");
        // General with text
        let x = r#"<number:number-style style:name="S"><number:number/><number:text> yen</number:text></number:number-style>"#;
        assert_eq!(code(x, "S"), "General\" yen\"");
        // N0: a number with no decimal places is General
        let x = r#"<number:number-style style:name="S"><number:number number:min-integer-digits="1"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "General");
    }

    #[test]
    fn embedded_text() {
        // 000-0000
        let x = r#"<number:number-style style:name="S"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="7"><number:embedded-text number:position="4">-</number:embedded-text></number:number></number:number-style>"#;
        assert_eq!(code(x, "S"), "000\\-0000");
        // 00-00-00
        let x = r#"<number:number-style style:name="S"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="6"><number:embedded-text number:position="4">-</number:embedded-text><number:embedded-text number:position="2">-</number:embedded-text></number:number></number:number-style>"#;
        assert_eq!(code(x, "S"), "00\\-00\\-00");
        // 0000"年"00"月"
        let x = r#"<number:number-style style:name="S"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="6"><number:embedded-text number:position="2">年</number:embedded-text></number:number><number:text>月</number:text></number:number-style>"#;
        assert_eq!(code(x, "S"), "0000年00月");
    }

    #[test]
    fn percent_scientific_fraction() {
        // 0.00%
        let x = r#"<number:percentage-style style:name="S"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1"/><number:text>%</number:text></number:percentage-style>"#;
        assert_eq!(code(x, "S"), "0.00%");
        // 0.00E+00
        let x = r#"<number:number-style style:name="S"><number:scientific-number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:min-exponent-digits="2" number:exponent-interval="1" number:forced-exponent-sign="true"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "0.00E+00");
        // ##0.0E+0
        let x = r#"<number:number-style style:name="S"><number:scientific-number number:decimal-places="1" number:min-decimal-places="1" number:min-exponent-digits="1" number:exponent-interval="3" number:forced-exponent-sign="true"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "##0.0E+0");
        // # ?/?
        let x = r#"<number:number-style style:name="S"><number:fraction number:min-integer-digits="0" number:min-numerator-digits="1" loext:max-numerator-digits="1" number:min-denominator-digits="1" number:max-denominator-value="9"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "# ?/?");
        // # ?/8
        let x = r#"<number:number-style style:name="S"><number:fraction number:min-integer-digits="0" number:min-numerator-digits="1" loext:max-numerator-digits="1" number:denominator-value="8"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "# ?/8");
    }

    #[test]
    fn literals() {
        // ¥#,##0
        let x = r#"<number:number-style style:name="S"><number:text>¥</number:text><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "¥#,##0");
        // \(##\)
        let x = r#"<number:number-style style:name="S"><number:text>(</number:text><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="0"/><number:text>)</number:text></number:number-style>"#;
        assert_eq!(code(x, "S"), "\\(##\\)");
        // "*"\ \ 0.0
        let x = r#"<number:number-style style:name="S"><number:text>*  </number:text><number:number number:decimal-places="1" number:min-decimal-places="1" number:min-integer-digits="1"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "\"*\"\\ \\ 0.0");
        // #,##0_  (the space only takes a width)
        let x = r#"<number:number-style style:name="S"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/><number:text loext:blank-width-char=" "> </number:text></number:number-style>"#;
        assert_eq!(code(x, "S"), "#,##0_ ");
        // "abc"@ and @
        let x = r#"<number:text-style style:name="S"><number:text>abc</number:text><number:text-content/></number:text-style>"#;
        assert_eq!(code(x, "S"), "\"abc\"@");
        let x = r#"<number:text-style style:name="S"><number:text-content/></number:text-style>"#;
        assert_eq!(code(x, "S"), "@");
        // *-0
        let x = r#"<number:number-style style:name="S"><number:fill-character>-</number:fill-character><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "*-0");
    }

    #[test]
    fn currency() {
        // [$¥-411]#,##0
        let x = r#"<number:currency-style style:name="S"><number:currency-symbol number:language="ja" number:country="JP">¥</number:currency-symbol><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:currency-style>"#;
        assert_eq!(code(x, "S"), "[$¥-411]#,##0");
        // #,##0.00\ [$€-40C]
        let x = r#"<number:currency-style style:name="S"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/><number:text> </number:text><number:currency-symbol number:language="fr" number:country="FR">€</number:currency-symbol></number:currency-style>"#;
        assert_eq!(code(x, "S"), "#,##0.00\\ [$€-40C]");
        // [$CHF] #,##0.00
        let x = r#"<number:currency-style style:name="S"><number:currency-symbol>CHF</number:currency-symbol><number:text> </number:text><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/></number:currency-style>"#;
        assert_eq!(code(x, "S"), "[$CHF]\\ #,##0.00");
    }

    #[test]
    fn dates_and_times() {
        // yyyy/m/d
        let x = r#"<number:date-style style:name="S"><number:year number:style="long"/><number:text>/</number:text><number:month/><number:text>/</number:text><number:day/></number:date-style>"#;
        assert_eq!(code(x, "S"), "yyyy/m/d");
        // yyyy年m月d日
        let x = r#"<number:date-style style:name="S"><number:year number:style="long"/><number:text>年</number:text><number:month/><number:text>月</number:text><number:day/><number:text>日</number:text></number:date-style>"#;
        assert_eq!(code(x, "S"), "yyyy年m月d日");
        // yyyy/m/d(aaa)
        let x = r#"<number:date-style style:name="S"><number:year number:style="long"/><number:text>/</number:text><number:month/><number:text>/</number:text><number:day/><number:text>(</number:text><number:day-of-week number:calendar="gengou"/><number:text>)</number:text></number:date-style>"#;
        assert_eq!(code(x, "S"), "yyyy/m/d(aaa)");
        // ggge"年"m"月"d"日"
        let x = r#"<number:date-style style:name="S"><number:era number:calendar="gengou" number:style="long"/><number:year number:calendar="gengou"/><number:text>年</number:text><number:month number:calendar="gengou"/><number:text>月</number:text><number:day number:calendar="gengou"/><number:text>日</number:text></number:date-style>"#;
        assert_eq!(code(x, "S"), "ggge年m月d日");
        // yyyy/m/d h:mm
        let x = r#"<number:date-style style:name="S" number:automatic-order="true"><number:year number:style="long"/><number:text>/</number:text><number:month/><number:text>/</number:text><number:day/><number:text> </number:text><number:hours/><number:text>:</number:text><number:minutes number:style="long"/></number:date-style>"#;
        assert_eq!(code(x, "S"), "yyyy/m/d h:mm");
        // h:mm:ss.00 and h:mm AM/PM
        let x = r#"<number:time-style style:name="S"><number:hours number:style="long"/><number:text>:</number:text><number:minutes number:style="long"/><number:text>:</number:text><number:seconds number:style="long" number:decimal-places="2"/></number:time-style>"#;
        assert_eq!(code(x, "S"), "hh:mm:ss.00");
        let x = r#"<number:time-style style:name="S"><number:hours/><number:text>:</number:text><number:minutes number:style="long"/><number:text> </number:text><number:am-pm/></number:time-style>"#;
        assert_eq!(code(x, "S"), "h:mm AM/PM");
        // [h]:mm and [mm]:ss
        let x = r#"<number:time-style style:name="S" number:truncate-on-overflow="false"><number:hours/><number:text>:</number:text><number:minutes number:style="long"/></number:time-style>"#;
        assert_eq!(code(x, "S"), "[h]:mm");
        let x = r#"<number:time-style style:name="S" number:truncate-on-overflow="false"><number:minutes number:style="long"/><number:text>:</number:text><number:seconds number:style="long"/></number:time-style>"#;
        assert_eq!(code(x, "S"), "[mm]:ss");
        // h"時"mm"分"
        let x = r#"<number:time-style style:name="S"><number:hours/><number:text>時</number:text><number:minutes number:style="long"/><number:text>分</number:text></number:time-style>"#;
        assert_eq!(code(x, "S"), "h時mm分");
        // [$-409]mmmm yyyy
        let x = r#"<number:date-style style:name="S" number:language="en" number:country="US"><number:month number:style="long" number:textual="true"/><number:text> </number:text><number:year number:style="long"/></number:date-style>"#;
        assert_eq!(code(x, "S"), "[$-409]mmmm yyyy");
        // d-mmm-yy
        let x = r#"<number:date-style style:name="S"><number:day/><number:text>-</number:text><number:month number:textual="true"/><number:text>-</number:text><number:year/></number:date-style>"#;
        assert_eq!(code(x, "S"), "d-mmm-yy");
    }

    #[test]
    fn colors_and_maps() {
        // #,##0;[Red]-#,##0
        let x = r##"<number:number-style style:name="N132"><style:text-properties fo:color="#ff0000"/><number:text>-</number:text><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/><style:map style:condition="value()&gt;=0" style:apply-style-name="N132P0"/></number:number-style>
<number:number-style style:name="N132P0" style:volatile="true"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>"##;
        assert_eq!(code(x, "N132"), "#,##0;[Red]\\-#,##0");
        // the part by itself is its own code
        assert_eq!(code(x, "N132P0"), "#,##0");
        // [Red]#,##0
        let x = r##"<number:number-style style:name="S"><style:text-properties fo:color="#ff0000"/><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>"##;
        assert_eq!(code(x, "S"), "[Red]#,##0");
        // [Color10]0.00
        let x = r##"<number:number-style style:name="S"><style:text-properties fo:color="#dddddd"/><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1"/></number:number-style>"##;
        assert_eq!(code(x, "S"), "[Color10]0.00");
        // [Red]#,##0;[Blue]-#,##0;[Green]"zero"
        let x = r##"<number:number-style style:name="N208"><style:text-properties fo:color="#00ff00"/><number:text>zero</number:text><style:map style:condition="value()&gt;0" style:apply-style-name="N208P0"/><style:map style:condition="value()&lt;0" style:apply-style-name="N208P1"/></number:number-style>
<number:number-style style:name="N208P0" style:volatile="true"><style:text-properties fo:color="#ff0000"/><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>
<number:number-style style:name="N208P1" style:volatile="true"><style:text-properties fo:color="#0000ff"/><number:text>-</number:text><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>"##;
        assert_eq!(code(x, "N208"), "[Red]#,##0;[Blue]\\-#,##0;[Green]\"zero\"");
    }

    #[test]
    fn words_and_accounting() {
        // "Total: "#,##0.00" yen"
        let x = r#"<number:number-style style:name="S"><number:text>Total: </number:text><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/><number:text> yen</number:text></number:number-style>"#;
        assert_eq!(code(x, "S"), "\"Total: \"#,##0.00\" yen\"");
        // _-* #,##0_-;\-* #,##0_-;_-* "-"_-;_-@_- (the zero section's text is
        // `- ` with the width of `-` for its second character)
        let x = r#"<number:text-style style:name="N135"><number:text loext:blank-width-char="-"> </number:text><number:text-content/><number:text loext:blank-width-char="-"> </number:text><style:map style:condition="value()&gt;0" style:apply-style-name="N135P0"/><style:map style:condition="value()&lt;0" style:apply-style-name="N135P1"/><style:map style:condition="value()=0" style:apply-style-name="N135P2"/></number:text-style>
<number:number-style style:name="N135P0" style:volatile="true"><number:text loext:blank-width-char="-"> </number:text><number:fill-character> </number:fill-character><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/><number:text loext:blank-width-char="-"> </number:text></number:number-style>
<number:number-style style:name="N135P1" style:volatile="true"><number:text>-</number:text><number:fill-character> </number:fill-character><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/><number:text loext:blank-width-char="-"> </number:text></number:number-style>
<number:number-style style:name="N135P2" style:volatile="true"><number:text loext:blank-width-char="-"> </number:text><number:fill-character> </number:fill-character><number:text loext:blank-width-char="-1">- </number:text></number:number-style>"#;
        assert_eq!(code(x, "N135"), "_-* #,##0_-;\\-* #,##0_-;_-* \\-_-;_-@_-");
    }

    #[test]
    fn four_sections_and_text() {
        // #,##0;\-#,##0;;
        let x = r#"<number:text-style style:name="N145"><number:text/><style:map style:condition="value()&gt;0" style:apply-style-name="N145P0"/><style:map style:condition="value()&lt;0" style:apply-style-name="N145P1"/><style:map style:condition="value()=0" style:apply-style-name="N145P2"/></number:text-style>
<number:number-style style:name="N145P0" style:volatile="true"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>
<number:number-style style:name="N145P1" style:volatile="true"><number:text>-</number:text><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>
<number:number-style style:name="N145P2" style:volatile="true"><number:text/></number:number-style>"#;
        assert_eq!(code(x, "N145"), "#,##0;\\-#,##0;;");
        // #,##0;-#,##0;  (three sections, the zero section is empty)
        let x = r#"<number:number-style style:name="N145"><number:text/><style:map style:condition="value()&gt;0" style:apply-style-name="N145P0"/><style:map style:condition="value()&lt;0" style:apply-style-name="N145P1"/></number:number-style>
<number:number-style style:name="N145P0" style:volatile="true"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>
<number:number-style style:name="N145P1" style:volatile="true"><number:text>-</number:text><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>"#;
        assert_eq!(code(x, "N145"), "#,##0;\\-#,##0;");
        // [$-409]mmmm\ yyyy;@
        let x = r#"<number:text-style style:name="N20129" number:language="en" number:country="US"><number:text-content/><style:map style:condition="value()&lt;=1.7976931348623157E+308" style:apply-style-name="N20129P0"/></number:text-style>
<number:date-style style:name="N20129P0" style:volatile="true" number:language="en" number:country="US"><number:month number:style="long" number:textual="true"/><number:text> </number:text><number:year number:style="long"/></number:date-style>"#;
        assert_eq!(code(x, "N20129"), "[$-409]mmmm yyyy;@");
    }

    #[test]
    fn explicit_conditions() {
        // [>100]#,##0;[<=100]0.0 (LibreOffice adds a General style for the rest)
        let x = r#"<number:number-style style:name="N217"><number:number/><style:map style:condition="value()&gt;100" style:apply-style-name="N217P0"/><style:map style:condition="value()&lt;=100" style:apply-style-name="N217P1"/></number:number-style>
<number:number-style style:name="N217P0" style:volatile="true"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1" number:grouping="true"/></number:number-style>
<number:number-style style:name="N217P1" style:volatile="true"><number:number number:decimal-places="1" number:min-decimal-places="1" number:min-integer-digits="1"/></number:number-style>"#;
        assert_eq!(code(x, "N217"), "[>100]#,##0;[<=100]0.0");
        // [=1]"one";[=2]"two";0
        let x = r#"<number:number-style style:name="N220"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1"/><style:map style:condition="value()=1" style:apply-style-name="N220P0"/><style:map style:condition="value()=2" style:apply-style-name="N220P1"/></number:number-style>
<number:number-style style:name="N220P0" style:volatile="true"><number:text>one</number:text></number:number-style>
<number:number-style style:name="N220P1" style:volatile="true"><number:text>two</number:text></number:number-style>"#;
        assert_eq!(code(x, "N220"), "[=1]\"one\";[=2]\"two\";0");
        // [<>0]0;"zero"
        let x = r#"<number:number-style style:name="N221"><number:text>zero</number:text><style:map style:condition="value()!=0" style:apply-style-name="N221P0"/></number:number-style>
<number:number-style style:name="N221P0" style:volatile="true"><number:number number:decimal-places="0" number:min-decimal-places="0" number:min-integer-digits="1"/></number:number-style>"#;
        assert_eq!(code(x, "N221"), "[<>0]0;\"zero\"");
    }

    #[test]
    fn styles_part_and_odd_input() {
        // Both parts of an ods hold styles; garbage is not a style
        assert!(parse_data_styles("").is_empty());
        assert!(parse_data_styles("<a><b>").is_empty());
        let x = r#"<number:boolean-style style:name="B"><number:boolean/></number:boolean-style>"#;
        assert_eq!(code(x, "B"), "General");
        // a map that names a style which is not there cannot be put together
        let x = r#"<number:number-style style:name="S"><number:number number:decimal-places="0"/><style:map style:condition="value()&gt;=0" style:apply-style-name="X"/></number:number-style>"#;
        assert_eq!(code(x, "S"), "<none>");
    }
}
