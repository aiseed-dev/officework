//! **行の中の書き方。** 本家 asciidoctor の `QUOTE_SUBS`(`lib/asciidoctor.rb`)と
//! `extract_passthroughs`(`substitutors.rb`)を写したものです(2026-09-02)。
//!
//! 本家は段落の字をまるごと1つの文字列にして、決まった順に置き換えを
//! かけます。うちも同じ順で同じ条件を見ます。置き換えの結果は HTML では
//! なく、私用領域の字(U+E000 から)を印にした文字列にして、最後に
//! [`Run`] の並びへ直します。
//!
//! 突き合わせの表と、本家の約束を普通の言葉で書いたものは
//! `docs/sekkei/asciidoctor-tsukiawase.ja.adoc` にあります。
//!
//! # 読むときの順(本家と同じ)
//!
//! 1. passthrough(`+++字+++` `++字++` `$$字$$` `pass:[字]` `+字+`)を
//!    取り出して、印に置き換える。中の字は何があっても字のまま
//! 2. 太字(`**` `*`)、引用符(`"\`` `'\``)、等幅(```` `` ```` `` ` ``)、
//!    斜体(`__` `_`)、マーク(`##` `#`)、上付き(`^`)、下付き(`~`)の順に
//!    置き換える。二重の印は条件なし、一重の印は前後の字を見る
//! 3. 印つきの文字列を run の並びへ直す。ここでマクロ(`footnote:[]`
//!    `ruby:[]` `field:[]` リンク `<<>>`)も読む
//!
//! # 書くときの自己確認
//!
//! 本家の `\` は「この印は字です」と言うためのもので、印として読まれ
//! ない `*` の前に付けると、`\` がそのまま読者に見えます。だから書く側は
//! 「全部の `*` に `\` を付ける」ことができません。代わりに、書いた行を
//! 自分で読み直し、意図した run の並びと違うところだけに `\` を足します
//! ([`settle`])。

use crate::doc::{CharFormat, Document, Footnote, FootnoteRef, Paragraph, RefField, Run};

/// 等幅の字のスタイル名(`adoc.rs` の `MONO` と同じ物)
const MONO: &str = "等幅";

/// マークの色。Word の蛍光ペンの既定の色です
const MARK_COLOR: &str = "yellow";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Strong,
    Emph,
    Mono,
    Mark,
    Sup,
    Sub,
    DQuote,
    SQuote,
}

const KINDS: [Kind; 8] = [
    Kind::Strong,
    Kind::Emph,
    Kind::Mono,
    Kind::Mark,
    Kind::Sup,
    Kind::Sub,
    Kind::DQuote,
    Kind::SQuote,
];

// 私用領域の割り当て。**入力にある私用領域の字は、先に passthrough の
// 表へ逃がします**([`shield`])。なので本文の字と混ざりません
const OPEN0: u32 = 0xE000; // + 種類
const CLOSE0: u32 = 0xE010; // + 種類
const ATTR0: u32 = 0xE100; // + 属性の表の番号(開きの印の直後に置く)
const ATTR_MAX: u32 = 0xE800 - ATTR0;
const PASS0: u32 = 0xE800; // + passthrough の表の番号
const PASS_MAX: u32 = 0xF800 - PASS0;

/// 書く側が「字としての印」に付ける仮の字(`settle` が本物に戻します)
const TWIN0: u32 = 0xF800; // 私用領域の終わり近く(U+F8FF まで)
/// 字としても印としても意味を持ちうる字。並びは TWIN の番号です
pub(crate) const MARK_CHARS: &[char] = &['*', '_', '`', '#', '^', '~', '+', '\\', '[', '$'];

fn open(k: Kind) -> char {
    char::from_u32(OPEN0 + k as u32).expect("私用領域")
}
fn close(k: Kind) -> char {
    char::from_u32(CLOSE0 + k as u32).expect("私用領域")
}
fn kind_of(c: char, base: u32) -> Option<Kind> {
    let n = (c as u32).wrapping_sub(base);
    (n < 8).then(|| KINDS[n as usize])
}
fn attr_idx(c: char) -> Option<usize> {
    let n = (c as u32).wrapping_sub(ATTR0);
    (n < ATTR_MAX).then_some(n as usize)
}
fn pass_idx(c: char) -> Option<usize> {
    let n = (c as u32).wrapping_sub(PASS0);
    (n < PASS_MAX).then_some(n as usize)
}

/// 本家の `\p{Word}`。字か数字か `_`
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 役割と id(`[.役割#id]` の中身)。本家の `parse_quoted_text_attributes`
#[derive(Clone, Debug, Default, PartialEq)]
struct Attrs {
    /// 役割。複数なら空白で繋ぐ(本家と同じ)
    role: Option<String>,
    id: Option<String>,
}

fn parse_attrs(s: &str) -> Attrs {
    let s = s.split(',').next().unwrap_or("").trim();
    if s.is_empty() {
        return Attrs::default();
    }
    if s.starts_with('.') || s.starts_with('#') {
        let (before, after) = s.split_once('#').unwrap_or((s, ""));
        let mut a = Attrs::default();
        let roles_of = |t: &str| {
            let r = t.replace('.', " ");
            let r = r.trim_start().to_string();
            (!r.is_empty()).then_some(r)
        };
        if after.is_empty() {
            if before.len() > 1 {
                a.role = roles_of(before);
            }
        } else {
            let (id, roles) = after.split_once('.').unwrap_or((after, ""));
            if !id.is_empty() {
                a.id = Some(id.to_string());
            }
            if roles.is_empty() {
                if before.len() > 1 {
                    a.role = roles_of(before);
                }
            } else if before.len() > 1 {
                a.role = roles_of(&format!("{before}.{roles}"));
            } else {
                a.role = Some(roles.replace('.', " "));
            }
        }
        a
    } else {
        Attrs { role: Some(s.to_string()), id: None }
    }
}

/// 取り出した passthrough
#[derive(Clone, Debug)]
struct Pass {
    text: String,
    /// `[x-]+字+` の形(古い書き方の等幅)
    mono: bool,
    attrs: Option<Attrs>,
}

/// 印つきの文字列と、印が指す表
struct Marked {
    t: Vec<char>,
    attrs: Vec<Attrs>,
    passes: Vec<Pass>,
}

impl Marked {
    fn attr_char(&mut self, a: Attrs) -> Option<char> {
        if (self.attrs.len() as u32) >= ATTR_MAX {
            return None;
        }
        self.attrs.push(a);
        char::from_u32(ATTR0 + self.attrs.len() as u32 - 1)
    }
    fn pass_char(&mut self, p: Pass) -> Option<char> {
        if (self.passes.len() as u32) >= PASS_MAX {
            return None;
        }
        self.passes.push(p);
        char::from_u32(PASS0 + self.passes.len() as u32 - 1)
    }
}

fn starts_with(t: &[char], i: usize, s: &str) -> bool {
    for (j, c) in s.chars().enumerate() {
        if t.get(i + j) != Some(&c) {
            return false;
        }
    }
    true
}

/// `[` から始まる属性の並び(`[^\[\]]+` を `]` で閉じた形)。
/// 返すのは(中身, `]` の次の位置)
fn attrlist_at(t: &[char], i: usize) -> Option<(String, usize)> {
    if t.get(i) != Some(&'[') {
        return None;
    }
    let mut j = i + 1;
    while let Some(&c) = t.get(j) {
        if c == ']' {
            return (j > i + 1).then(|| (t[i + 1..j].iter().collect(), j + 1));
        }
        if c == '[' {
            return None;
        }
        j += 1;
    }
    None
}

fn line_start(t: &[char], i: usize) -> bool {
    i == 0 || t[i - 1] == '\n'
}

// ---------------------------------------------------------------- passthrough

/// 入力にある私用領域の字を表へ逃がす(印と混ざらないように)
fn shield(src: &str, m: &mut Marked) -> Vec<char> {
    src.chars()
        .map(|c| {
            if (0xE000..=0xF8FF).contains(&(c as u32)) {
                m.pass_char(Pass { text: c.to_string(), mono: false, attrs: None }).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// 本家の `InlinePassMacroRx`: `+++字+++` `++字++` `$$字$$` `pass:[字]`
fn pass_unconstrained(t: Vec<char>, m: &mut Marked) -> Vec<char> {
    let s: String = t.iter().collect();
    if !(s.contains("++") || s.contains("$$") || s.contains("ss:")) {
        return t;
    }
    let mut out = Vec::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        if let Some((end, rep)) = pass_macro_at(&t, i, m) {
            out.extend(rep);
            i = end;
        } else {
            out.push(t[i]);
            i += 1;
        }
    }
    out
}

fn pass_macro_at(t: &[char], i: usize, m: &mut Marked) -> Option<(usize, Vec<char>)> {
    // 形1: (\\?)[属性] \\{0,2} (+++|++|$$) 中身 同じ印
    let (esc_attr, attrlist, mut b) = if t.get(i) == Some(&'\\') && t.get(i + 1) == Some(&'[') {
        match attrlist_at(t, i + 1) {
            Some((a, n)) => (true, Some(a), n),
            None => (false, None, i),
        }
    } else if let Some((a, n)) = attrlist_at(t, i) {
        (false, Some(a), n)
    } else {
        (false, None, i)
    };
    if attrlist.is_none() {
        b = i;
    }
    let mut esc_n = 0;
    while esc_n < 2 && t.get(b + esc_n) == Some(&'\\') {
        esc_n += 1;
    }
    let bs = b + esc_n;
    let boundaries: &[&str] = if starts_with(t, bs, "+++") {
        &["+++", "++"]
    } else if starts_with(t, bs, "++") {
        &["++"]
    } else if starts_with(t, bs, "$$") {
        &["$$"]
    } else {
        &[]
    };
    for bd in boundaries {
        let c0 = bs + bd.len();
        let mut j = c0;
        let found = loop {
            if j > t.len() {
                break None;
            }
            if starts_with(t, j, bd) {
                break Some(j);
            }
            j += 1;
        };
        let Some(j) = found else { continue };
        let end = j + bd.len();
        let content: String = t[c0..j].iter().collect();
        let literal = |from: usize| -> Vec<char> { t[from..end].to_vec() };
        let rep: Vec<char> = if let Some(a) = &attrlist {
            if esc_n > 0 {
                // 印の逃がし。`\` を1つ減らして字のまま
                let mut v: Vec<char> = Vec::new();
                if esc_attr {
                    v.push('\\');
                }
                v.push('[');
                v.extend(a.chars());
                v.push(']');
                v.extend(std::iter::repeat_n('\\', esc_n - 1));
                v.extend(t[bs..end].iter());
                v
            } else if esc_attr {
                // 属性の逃がし。属性は字のまま、中身は passthrough
                let mut v: Vec<char> = "[".chars().chain(a.chars()).chain("]".chars()).collect();
                let p = Pass { text: content, mono: false, attrs: None };
                v.push(m.pass_char(p).unwrap_or('?'));
                v
            } else {
                let (mono, attrs) = if *bd == "++" && a == "x-" {
                    (true, Some(Attrs::default()))
                } else if *bd == "++" && a.ends_with(" x-") {
                    (true, Some(parse_attrs(&a[..a.len() - 3])))
                } else {
                    (false, Some(parse_attrs(a)))
                };
                let p = Pass { text: content, mono, attrs };
                vec![m.pass_char(p).unwrap_or('?')]
            }
        } else if esc_n > 0 {
            let mut v: Vec<char> = std::iter::repeat_n('\\', esc_n - 1).collect();
            v.extend(literal(bs));
            v
        } else {
            let p = Pass { text: content, mono: false, attrs: None };
            vec![m.pass_char(p).unwrap_or('?')]
        };
        return Some((end, rep));
    }
    // 形2: (\\?)pass:(subs)?[中身]
    let (esc, b) = if t.get(i) == Some(&'\\') { (true, i + 1) } else { (false, i) };
    if !starts_with(t, b, "pass:") {
        return None;
    }
    let mut k = b + 5;
    // subs の並び(`[a-z]+(?:,[a-z-]+)*`)。読み飛ばすだけ
    while t.get(k).is_some_and(|c| c.is_ascii_lowercase() || *c == ',' || *c == '-') {
        k += 1;
    }
    if t.get(k) != Some(&'[') {
        return None;
    }
    let c0 = k + 1;
    let mut j = c0;
    loop {
        match t.get(j) {
            None => return None,
            Some(']') if j == c0 || t[j - 1] != '\\' => break,
            _ => j += 1,
        }
    }
    let end = j + 1;
    if esc {
        return Some((end, t[i + 1..end].to_vec()));
    }
    let content: String = t[c0..j].iter().collect::<String>().replace("\\]", "]");
    let p = Pass { text: content, mono: false, attrs: None };
    Some((end, vec![m.pass_char(p).unwrap_or('?')]))
}

/// 本家の `InlinePassRx[false]`: 一重の `+字+`(前後の条件つき)
fn pass_constrained(t: Vec<char>, m: &mut Marked) -> Vec<char> {
    if !t.contains(&'+') {
        return t;
    }
    let mut out = Vec::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        if let Some((end, rep)) = pass_plus_at(&t, i, m) {
            out.extend(rep);
            i = end;
        } else {
            out.push(t[i]);
            i += 1;
        }
    }
    out
}

fn pass_plus_at(t: &[char], i: usize, m: &mut Marked) -> Option<(usize, Vec<char>)> {
    // 前置き(3つの形)。preceding は再び出力する字
    let mut tries: Vec<(Vec<char>, usize, bool)> = Vec::new(); // (preceding, 次の位置, `[` が続くか)
    let follows = |p: usize| t.get(p) == Some(&'[') || t.get(p) == Some(&'+');
    if line_start(t, i) && follows(i) {
        tries.push((vec![], i, t.get(i) == Some(&'[')));
    }
    if let Some(&c) = t.get(i) {
        if !is_word(c) && !matches!(c, ';' | ':' | '\\') && follows(i + 1) {
            tries.push((vec![c], i + 1, t.get(i + 1) == Some(&'[')));
        }
        if c == '\\' && t.get(i + 1) == Some(&'[') {
            tries.push((vec!['\\'], i + 1, false));
        }
        if c == '\\' && t.get(i + 1) == Some(&'+') {
            tries.push((vec![], i, false));
        }
    }
    for (preceding, b, bracket) in tries {
        // 属性: `[x-]` `[… x-]` か、普通の `[属性]`
        let mut attrlist: Option<String> = None;
        let mut forced = false;
        let mut p = b;
        if bracket {
            if let Some((a, n)) = attrlist_at(t, b) {
                if a == "x-" || a.ends_with(" x-") {
                    forced = true;
                }
                attrlist = Some(a);
                p = n;
            }
        } else if let Some((a, n)) = attrlist_at(t, b) {
            if a == "x-" || a.ends_with(" x-") {
                forced = true;
            }
            attrlist = Some(a);
            p = n;
        }
        // (\\)?\+ 中身 \+ (?!\w)
        let escaped = t.get(p) == Some(&'\\');
        let q = if escaped { p + 1 } else { p };
        if t.get(q) != Some(&'+') {
            continue;
        }
        let Some((j, content)) = constrained_content(t, q + 1, "+", |c| !is_word(c)) else {
            continue;
        };
        let end = j + 1;
        let mut rep: Vec<char> = preceding.clone();
        let mut attrs: Option<Attrs> = None;
        let mut mono = false;
        if let Some(a) = &attrlist {
            if escaped {
                rep.push('[');
                rep.extend(a.chars());
                rep.push(']');
                rep.extend(t[p + 1..end].iter());
                return Some((end, rep));
            } else if preceding == ['\\'] {
                rep.clear();
                rep.push('[');
                rep.extend(a.chars());
                rep.push(']');
            } else if forced {
                mono = true;
                attrs = Some(if a == "x-" { Attrs::default() } else { parse_attrs(&a[..a.len() - 3]) });
            } else {
                attrs = Some(parse_attrs(a));
            }
        } else if escaped {
            rep.extend(t[p + 1..end].iter());
            return Some((end, rep));
        }
        let pass = Pass { text: content, mono, attrs };
        rep.push(m.pass_char(pass).unwrap_or('?'));
        return Some((end, rep));
    }
    None
}

/// 一重の印の中身 `(\S|\S.*?\S)` と、閉じの印と、その後ろの条件。
/// 返すのは(閉じの印の位置, 中身)
fn constrained_content(
    t: &[char],
    c0: usize,
    mark: &str,
    after_ok: impl Fn(char) -> bool,
) -> Option<(usize, String)> {
    let first = *t.get(c0)?;
    if first.is_whitespace() {
        return None;
    }
    let ml = mark.chars().count();
    let mut j = c0 + 1;
    while j < t.len() {
        if starts_with(t, j, mark) && (j == c0 + 1 || !t[j - 1].is_whitespace()) {
            let after = j + ml;
            if after >= t.len() || after_ok(t[after]) {
                return Some((j, t[c0..j].iter().collect()));
            }
        }
        j += 1;
    }
    None
}

// ---------------------------------------------------------------- quotes

struct Rule {
    kind: Kind,
    mark: &'static str,
    /// 一重(前後の条件を見る)か
    constrained: bool,
    /// 閉じの印(開きと違うのは引用符だけ)
    close_mark: &'static str,
}

/// 本家の `QUOTE_SUBS[false]` と同じ並び
const RULES: &[Rule] = &[
    Rule { kind: Kind::Strong, mark: "**", constrained: false, close_mark: "**" },
    Rule { kind: Kind::Strong, mark: "*", constrained: true, close_mark: "*" },
    Rule { kind: Kind::DQuote, mark: "\"`", constrained: true, close_mark: "`\"" },
    Rule { kind: Kind::SQuote, mark: "'`", constrained: true, close_mark: "`'" },
    Rule { kind: Kind::Mono, mark: "``", constrained: false, close_mark: "``" },
    Rule { kind: Kind::Mono, mark: "`", constrained: true, close_mark: "`" },
    Rule { kind: Kind::Emph, mark: "__", constrained: false, close_mark: "__" },
    Rule { kind: Kind::Emph, mark: "_", constrained: true, close_mark: "_" },
    Rule { kind: Kind::Mark, mark: "##", constrained: false, close_mark: "##" },
    Rule { kind: Kind::Mark, mark: "#", constrained: true, close_mark: "#" },
    Rule { kind: Kind::Sup, mark: "^", constrained: false, close_mark: "^" },
    Rule { kind: Kind::Sub, mark: "~", constrained: false, close_mark: "~" },
];

/// 開きの直前に来られない字(`^` か、word でない字のうちこれ以外)
fn prev_ok(kind: Kind, c: char) -> bool {
    if is_word(c) || matches!(c, ';' | ':' | '}') {
        return false;
    }
    match kind {
        Kind::SQuote => c != '`',
        Kind::Mono => !matches!(c, '"' | '\'' | '`'),
        Kind::Mark => c != '&',
        _ => true,
    }
}

/// 閉じの直後に来られない字
fn after_ok(kind: Kind, c: char) -> bool {
    if is_word(c) {
        return false;
    }
    match kind {
        Kind::Mono => !matches!(c, '"' | '\'' | '`'),
        _ => true,
    }
}

fn quote_pass(t: Vec<char>, r: &Rule, m: &mut Marked) -> Vec<char> {
    let first = r.mark.chars().next().expect("印");
    if !t.contains(&first) {
        return t;
    }
    let mut out = Vec::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        let hit = if r.constrained {
            constrained_at(&t, i, r, m)
        } else {
            unconstrained_at(&t, i, r, m)
        };
        if let Some((end, rep)) = hit {
            out.extend(rep);
            i = end;
        } else {
            out.push(t[i]);
            i += 1;
        }
    }
    out
}

fn wrap(m: &mut Marked, kind: Kind, attrs: Option<Attrs>, content: &[char]) -> Vec<char> {
    let mut v = vec![open(kind)];
    if let Some(a) = attrs {
        if let Some(c) = m.attr_char(a) {
            v.push(c);
        }
    }
    v.extend(content.iter());
    v.push(close(kind));
    v
}

/// `(^|[^\w;:}])(?:\[属性\])?印(\S|\S.*?\S)印(?!\w)`
fn constrained_at(t: &[char], i: usize, r: &Rule, m: &mut Marked) -> Option<(usize, Vec<char>)> {
    let mut prefixes: Vec<usize> = Vec::new();
    if line_start(t, i) {
        prefixes.push(0);
    }
    if t.get(i).is_some_and(|&c| prev_ok(r.kind, c)) {
        prefixes.push(1);
    }
    for pl in prefixes {
        let b = i + pl;
        let (attrlist, p) = match attrlist_at(t, b) {
            Some((a, n)) => (Some(a), n),
            None => (None, b),
        };
        if !starts_with(t, p, r.mark) {
            continue;
        }
        let c0 = p + r.mark.chars().count();
        let kind = r.kind;
        let Some((j, _)) = constrained_content(t, c0, r.close_mark, |c| after_ok(kind, c)) else {
            continue;
        };
        let end = j + r.close_mark.chars().count();
        let escaped = pl == 1 && t[i] == '\\';
        let content = &t[c0..j];
        let mut rep: Vec<char> = Vec::new();
        if escaped {
            if let Some(a) = attrlist {
                // 属性の逃がし: 属性は字のまま、印は効く(本家の決め)
                rep.push('[');
                rep.extend(a.chars());
                rep.push(']');
                rep.extend(wrap(m, kind, None, content));
            } else {
                rep.extend(t[i + 1..end].iter());
            }
            return Some((end, rep));
        }
        if pl == 1 {
            rep.push(t[i]);
        }
        rep.extend(wrap(m, kind, attrlist.map(|a| parse_attrs(&a)), content));
        return Some((end, rep));
    }
    None
}

/// `\\?(?:\[属性\])?印印(.+?)印印`(上付き・下付きは中身に空白を許さない)
fn unconstrained_at(t: &[char], i: usize, r: &Rule, m: &mut Marked) -> Option<(usize, Vec<char>)> {
    let escaped = t.get(i) == Some(&'\\');
    let b = if escaped { i + 1 } else { i };
    let (attrlist, p) = match attrlist_at(t, b) {
        Some((a, n)) => (Some(a), n),
        None => (None, b),
    };
    if !starts_with(t, p, r.mark) {
        return None;
    }
    let ml = r.mark.chars().count();
    let c0 = p + ml;
    let tight = matches!(r.kind, Kind::Sup | Kind::Sub);
    if c0 >= t.len() {
        return None;
    }
    // 中身 `.+?`(上付き・下付きは `\S+?`)。一番近い閉じの印まで
    let mut j = c0 + 1;
    loop {
        if j > t.len() || (tight && t[j - 1].is_whitespace()) {
            return None;
        }
        if starts_with(t, j, r.mark) {
            break;
        }
        j += 1;
    }
    let end = j + ml;
    if escaped {
        return Some((end, t[i + 1..end].to_vec()));
    }
    let content = &t[c0..j];
    Some((end, wrap(m, r.kind, attrlist.map(|a| parse_attrs(&a)), content)))
}

// ---------------------------------------------------------------- run へ

/// 印つきの文字列を作る(読みの 1・2)
fn mark_up(src: &str) -> Marked {
    let mut m = Marked { t: Vec::new(), attrs: Vec::new(), passes: Vec::new() };
    let mut t = shield(src, &mut m);
    t = pass_unconstrained(t, &mut m);
    t = pass_constrained(t, &mut m);
    for r in RULES {
        t = quote_pass(t, r, &mut m);
    }
    m.t = t;
    m
}

/// 印を全部落として字だけにする(リンクの表示名などに使う)
fn plain(t: &[char], m: &Marked) -> String {
    let mut s = String::new();
    for &c in t {
        if kind_of(c, OPEN0).is_some() || kind_of(c, CLOSE0).is_some() || attr_idx(c).is_some() {
            continue;
        }
        if let Some(k) = pass_idx(c) {
            s.push_str(&m.passes[k].text);
            continue;
        }
        s.push(c);
    }
    s
}

/// 印を元の書き方に戻して字にする(リンクや参照の表示名に使う)。
///
/// run は1つに書式1つなので、リンクの字の中の書式は持てません。落とす
/// 代わりに `*字*` のような元の形で字に残し、書き戻しでも同じ形にします。
/// 本家はそれを再び書式として組むので、本家の出力は変わりません
fn remark(t: &[char], m: &Marked) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i < t.len() {
        let c = t[i];
        if let Some(k) = kind_of(c, OPEN0) {
            let attrs = t.get(i + 1).and_then(|&c| attr_idx(c)).and_then(|a| m.attrs.get(a));
            if let Some(a) = attrs {
                let mut inner = String::new();
                if let Some(id) = &a.id {
                    inner.push('#');
                    inner.push_str(id);
                }
                if let Some(r) = &a.role {
                    for x in r.split(' ') {
                        inner.push('.');
                        inner.push_str(x);
                    }
                }
                s.push('[');
                s.push_str(&inner);
                s.push(']');
                i += 1;
            }
            s.push_str(match k {
                Kind::Strong => "*",
                Kind::Emph => "_",
                Kind::Mono => "`",
                Kind::Mark => "#",
                Kind::Sup => "^",
                Kind::Sub => "~",
                Kind::DQuote => "\"`",
                Kind::SQuote => "'`",
            });
            i += 1;
            continue;
        }
        if let Some(k) = kind_of(c, CLOSE0) {
            s.push_str(match k {
                Kind::Strong => "*",
                Kind::Emph => "_",
                Kind::Mono => "`",
                Kind::Mark => "#",
                Kind::Sup => "^",
                Kind::Sub => "~",
                Kind::DQuote => "`\"",
                Kind::SQuote => "`'",
            });
            i += 1;
            continue;
        }
        if let Some(k) = pass_idx(c) {
            // passthrough は中身の字だけにします。書き手が `\` と `]` を
            // 包み直すので、往復で壊れません
            s.push_str(&m.passes[k].text);
            i += 1;
            continue;
        }
        s.push(c);
        i += 1;
    }
    s
}

#[derive(Default, Clone)]
struct State {
    depth: [u8; 8],
    /// 種類ごとに、開いた囲みが(属性を持っていたか, 役割を持っていたか)。
    /// 閉じるときに同じ物を外すために覚えます
    had_attrs: [Vec<(bool, bool)>; 8],
    roles: Vec<String>,
}

impl State {
    fn on(&self, k: Kind) -> bool {
        self.depth[k as usize] > 0
    }
    fn fmt(&self) -> CharFormat {
        let mut f = CharFormat {
            bold: self.on(Kind::Strong),
            italic: self.on(Kind::Emph),
            superscript: self.on(Kind::Sup),
            subscript: self.on(Kind::Sub),
            ..Default::default()
        };
        if self.on(Kind::Mark) {
            f.highlight = Some(MARK_COLOR.to_string());
        }
        // 役割が最後。等幅と役割が重なったら役割(テンプレートの表の方が細かい)
        if self.on(Kind::Mono) {
            f.style_id = Some(MONO.to_string());
        }
        if let Some(r) = self.roles.last() {
            f.style_id = Some(r.replace(' ', "."));
        }
        f
    }
}

struct Walker<'a> {
    m: &'a Marked,
    doc: &'a mut Document,
    fresh_note: &'a mut usize,
}

/// マクロの頭。`\` の次にこれが来たら、`\` は逃がしの印です
const MACRO_HEADS: &[&str] = &[
    "footnote:[", "field:", "ruby:", "https://", "http://", "file://", "ftp://", "irc://", "link:", "<<", "xref:",
];

impl Walker<'_> {
    fn walk(&mut self, t: &[char], st: &mut State) -> Result<Vec<Run>, String> {
        let mut runs: Vec<Run> = Vec::new();
        let mut cur = String::new();
        let flush = |runs: &mut Vec<Run>, cur: &mut String, st: &State| {
            if !cur.is_empty() {
                runs.push(Run { text: std::mem::take(cur), size_pt: None, font: None, fmt: st.fmt() });
            }
        };
        let mut i = 0;
        while i < t.len() {
            let c = t[i];
            if let Some(k) = kind_of(c, OPEN0) {
                flush(&mut runs, &mut cur, st);
                let mut attrs: Option<&Attrs> = None;
                if let Some(a) = t.get(i + 1).and_then(|&c| attr_idx(c)) {
                    attrs = self.m.attrs.get(a);
                    i += 1;
                }
                match k {
                    Kind::DQuote => cur.push_str("\"`"),
                    Kind::SQuote => cur.push_str("'`"),
                    _ => {}
                }
                let role = attrs.and_then(|a| a.role.clone());
                let has_attrs = attrs.is_some();
                if let Some(r) = role {
                    st.roles.push(r);
                }
                // 属性つきのマーク(`[.役割]#字#` `[#id]#字#`)は、マークでは
                // なく役割だけ(本家の `:unquoted`)
                if !(k == Kind::Mark && has_attrs) {
                    st.depth[k as usize] += 1;
                }
                st.had_attrs[k as usize].push((has_attrs, attrs.is_some_and(|a| a.role.is_some())));
                i += 1;
                continue;
            }
            if let Some(k) = kind_of(c, CLOSE0) {
                flush(&mut runs, &mut cur, st);
                let (had_attrs, had_role) = st.had_attrs[k as usize].pop().unwrap_or((false, false));
                if had_role {
                    st.roles.pop();
                }
                if !(k == Kind::Mark && had_attrs) {
                    st.depth[k as usize] = st.depth[k as usize].saturating_sub(1);
                }
                match k {
                    Kind::DQuote => cur.push_str("`\""),
                    Kind::SQuote => cur.push_str("`'"),
                    _ => {}
                }
                i += 1;
                continue;
            }
            if let Some(k) = pass_idx(c) {
                flush(&mut runs, &mut cur, st);
                let p = &self.m.passes[k];
                let mut fmt = st.fmt();
                if p.mono && fmt.style_id.is_none() {
                    fmt.style_id = Some(MONO.to_string());
                }
                if let Some(r) = p.attrs.as_ref().and_then(|a| a.role.as_ref()) {
                    fmt.style_id = Some(r.replace(' ', "."));
                }
                runs.push(Run { text: p.text.clone(), size_pt: None, font: None, fmt });
                i += 1;
                continue;
            }
            // `\` は、次がマクロのときだけ逃がしの印。それ以外は字
            if c == '\\' {
                if let Some(&n) = t.get(i + 1) {
                    if MACRO_HEADS.iter().any(|h| starts_with(t, i + 1, h)) {
                        cur.push(n);
                        i += 2;
                        continue;
                    }
                    // `\<https://…>` は `<` と URL の頭を字にして、リンクにしない
                    if n == '<' && scheme_len(t, i + 2).is_some() {
                        cur.push('<');
                        cur.push(t[i + 2]);
                        i += 3;
                        continue;
                    }
                }
                cur.push(c);
                i += 1;
                continue;
            }
            if let Some((end, mut run)) = self.macro_at(t, i, st)? {
                // 頭が `\u{0}` の印なら、直前に字にした `<` を外す(`<URL>` の形)
                if run.first().is_some_and(|r| r.text == "\u{0}") {
                    run.remove(0);
                    if cur.ends_with('<') {
                        cur.pop();
                    } else if let Some(last) = runs.last_mut() {
                        if last.text.ends_with('<') {
                            last.text.pop();
                        }
                    }
                }
                flush(&mut runs, &mut cur, st);
                runs.extend(run);
                i = end;
                continue;
            }
            cur.push(c);
            i += 1;
        }
        flush(&mut runs, &mut cur, st);
        Ok(runs)
    }

    /// `名前:対象[属性]` の形のマクロと、リンクと、`<<参照>>`
    fn macro_at(&mut self, t: &[char], i: usize, st: &State) -> Result<Option<(usize, Vec<Run>)>, String> {
        let m = self.m;
        let base = st.fmt();
        let run = |text: String, fmt: CharFormat| Run { text, size_pt: None, font: None, fmt };
        // `[` から `]` まで(`\]` は飛ばす)。返すのは `]` の位置
        let close_bracket = |from: usize| -> Option<usize> {
            let mut j = from;
            while j < t.len() {
                if t[j] == ']' && (j == from || t[j - 1] != '\\') {
                    return Some(j);
                }
                j += 1;
            }
            None
        };
        if starts_with(t, i, "footnote:[") {
            let c0 = i + "footnote:[".len();
            let j = close_bracket(c0).ok_or("footnote:[ が閉じていません")?;
            *self.fresh_note += 1;
            let id = format!("adoc{}", self.fresh_note);
            let mut inner = State::default();
            let mut note_runs = self.walk(&t[c0..j], &mut inner)?;
            for r in &mut note_runs {
                r.text = r.text.replace("\\]", "]");
            }
            let np = Paragraph { runs: note_runs, ..Default::default() };
            self.doc.footnotes.push(Footnote {
                id: id.clone(),
                endnote: false,
                paragraphs: vec![np],
                added: true,
            });
            let fmt = CharFormat {
                footnote: Some(FootnoteRef { id, endnote: false }),
                ..Default::default()
            };
            return Ok(Some((j + 1, vec![run(String::new(), fmt)])));
        }
        if starts_with(t, i, "field:") {
            let c0 = i + "field:".len();
            if let Some(open_at) = (c0..t.len()).find(|&k| t[k] == '[') {
                if let Some(j) = close_bracket(open_at + 1) {
                    let tag = plain(&t[c0..open_at], m);
                    let inner = plain(&t[open_at + 1..j], m);
                    let (alias, kind, items) = crate::adoc::parse_field(&inner);
                    let fmt = CharFormat {
                        sdt: Some(Box::new(crate::doc::Sdt { kind, alias, tag, items })),
                        ..Default::default()
                    };
                    return Ok(Some((j + 1, vec![run(String::new(), fmt)])));
                }
            }
        }
        if starts_with(t, i, "ruby:") {
            let c0 = i + "ruby:".len();
            if let Some(open_at) = (c0..t.len()).find(|&k| t[k] == '[') {
                if let Some(j) = close_bracket(open_at + 1) {
                    let fmt = CharFormat { ruby: Some(plain(&t[open_at + 1..j], m)), ..base };
                    return Ok(Some((j + 1, vec![run(plain(&t[c0..open_at], m), fmt)])));
                }
            }
        }
        // ---- 行の中の画像とアイコン(`image:対象[属性]` `icon:名前[属性]`)。
        // 模型に無いので字のまま持ちます。本家はリンクより先に読むので、
        // 属性の中の URL(`link="https://…"`)をリンクにしません
        for head in ["image:", "icon:"] {
            if starts_with(t, i, head) && !starts_with(t, i + head.len(), ":") {
                let c0 = i + head.len();
                if let Some(k) = (c0..t.len()).find(|&k| t[k] == '[' || t[k].is_whitespace()) {
                    if t[k] == '[' && k > c0 {
                        if let Some(j) = close_bracket(k + 1) {
                            return Ok(Some((j + 1, vec![run(remark(&t[i..j + 1], m), base)])));
                        }
                    }
                }
            }
        }
        // ---- リンク(本家の `InlineLinkMacroRx` と `InlineLinkRx`)。
        // 本家はマクロの段でリンクを先に、参照(`<<>>`)を後に読みます
        let link_run = |url: String, raw_text: &str, base: &CharFormat| -> Run {
            let (text, role) = link_text(raw_text, &url);
            let mut fmt = CharFormat { link: Some(url), ..base.clone() };
            if let Some(r) = role {
                fmt.style_id = Some(r);
            }
            run(text, fmt)
        };
        // 形1: `link:対象[字]`。対象は空でもよい。`link::` は違います
        if starts_with(t, i, "link:") && !starts_with(t, i, "link::") {
            let c0 = i + 5;
            let mut k = c0;
            while k < t.len() && !t[k].is_whitespace() && t[k] != '[' {
                k += 1;
            }
            let target_ok = k == c0 || t[c0] != ':';
            if target_ok && t.get(k) == Some(&'[') {
                if let Some(j) = close_bracket(k + 1) {
                    let url = plain(&t[c0..k], m);
                    return Ok(Some((j + 1, vec![link_run(url, &remark(&t[k + 1..j], m), &base)])));
                }
            }
            // `[` が無い `link:` は字のまま(この位置から URL も読みません)
            return Ok(None);
        }
        // 形2: URL。直前が行頭・空白・`<`・`>()[];"'`・印のどれかのときだけ
        if let Some(sl) = scheme_len(t, i) {
            let prev = if i == 0 { None } else { Some(t[i - 1]) };
            let prev_ok = match prev {
                None => true,
                Some(c) => {
                    c.is_whitespace()
                        || matches!(c, '<' | '>' | '(' | ')' | '[' | ']' | ';' | '"' | '\'')
                        || kind_of(c, OPEN0).is_some()
                        || kind_of(c, CLOSE0).is_some()
                        || attr_idx(c).is_some()
                }
            };
            if prev_ok {
                // `URL[字]` の形(`<` の後ろでも先に見ます)
                let mut k = i;
                while k < t.len() && !t[k].is_whitespace() && !is_sentinel(t[k]) && t[k] != '[' && t[k] != ']' {
                    k += 1;
                }
                if t.get(k) == Some(&'[') && k > i + sl {
                    if let Some(j) = close_bracket(k + 1) {
                        let url = plain(&t[i..k], m);
                        return Ok(Some((j + 1, vec![link_run(url, &remark(&t[k + 1..j], m), &base)])));
                    }
                }
                // `<URL>` の形。`>` で閉じる物だけがリンクで、閉じなければ何もしません
                if prev == Some('<') {
                    let mut k = i;
                    while k < t.len() && !t[k].is_whitespace() && !is_sentinel(t[k]) && t[k] != '>' {
                        k += 1;
                    }
                    if t.get(k) == Some(&'>') && k > i + sl {
                        let url = plain(&t[i..k], m);
                        // 直前の `<` はもう字に入っているので外す
                        return Ok(Some((k + 1, vec![Run { text: "\u{0}".into(), size_pt: None, font: None, fmt: CharFormat::default() }, link_run(url.clone(), "", &base)])));
                    }
                    return Ok(None);
                }
                // 裸の URL。末尾の `,.?!)` は URL に入れず、`;` `:`(とその前の `)`)も外に出します。
                // `<` は URL に入ります(本家は `&lt;` に変えてから見るので、切れ目にならない)
                let mut k = i;
                while k < t.len() && !t[k].is_whitespace() && !is_sentinel(t[k]) && !matches!(t[k], '[' | ']') {
                    k += 1;
                }
                while k > i && matches!(t[k - 1], ',' | '.' | '?' | '!' | ')') {
                    k -= 1;
                }
                if k > i && matches!(t[k - 1], ';' | ':') {
                    k -= 1;
                    if k > i && t[k - 1] == ')' {
                        k -= 1;
                    }
                }
                if k > i + sl {
                    let url = plain(&t[i..k], m);
                    return Ok(Some((k, vec![link_run(url, "", &base)])));
                }
            }
            return Ok(None);
        }
        // ---- 参照。`<<対象>>` `<<対象,字>>` `xref:対象[字]`
        if starts_with(t, i, "<<") {
            // 中が `<URL>`(閉じる物)か `link:` なら参照ではない(本家はリンクを先に読む)
            let closes = |from: usize| -> bool {
                let mut k = from;
                while k < t.len() && !t[k].is_whitespace() && !is_sentinel(t[k]) && !matches!(t[k], '>' | '[') {
                    k += 1;
                }
                matches!(t.get(k), Some('>') | Some('['))
            };
            let inner_link = t.get(i + 1) == Some(&'<')
                && (scheme_len(t, i + 2).is_some_and(|_| closes(i + 2)) || starts_with(t, i + 2, "link:"));
            let head_ok = t.get(i + 2).is_some_and(|&c| is_word(c) || matches!(c, '#' | '/' | '.' | ':' | '{'));
            if !inner_link && head_ok {
                if let Some(j) = (i + 2..t.len()).find(|&k| starts_with(t, k, ">>")) {
                    let inner = remark(&t[i + 2..j], m);
                    let (name, text) = match inner.split_once(',') {
                        Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
                        None => (inner.trim().to_string(), String::new()),
                    };
                    // 頭の `#` は「この文書の中」の印で、名前には入れません
                    let name = name.strip_prefix('#').unwrap_or(&name).to_string();
                    let text = if text.is_empty() { name.clone() } else { text };
                    let fmt = CharFormat { field: Some(RefField { name, page: false }), ..base };
                    return Ok(Some((j + 2, vec![run(text, fmt)])));
                }
            }
        }
        if starts_with(t, i, "xref:") {
            let c0 = i + 5;
            if t.get(c0).is_some_and(|&c| is_word(c) || matches!(c, '#' | '/' | '.' | ':' | '{')) {
                if let Some(k) = (c0..t.len()).find(|&k| t[k] == '[' || t[k].is_whitespace()) {
                    if t[k] == '[' {
                        if let Some(j) = close_bracket(k + 1) {
                            let name = plain(&t[c0..k], m);
                            let name = name.strip_prefix('#').unwrap_or(&name).to_string();
                            let text = remark(&t[k + 1..j], m).replace("\\]", "]");
                            let text = if text.is_empty() { name.clone() } else { text };
                            let fmt = CharFormat { field: Some(RefField { name, page: false }), ..base };
                            return Ok(Some((j + 1, vec![run(text, fmt)])));
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}

/// 印の字(私用領域)か。URL や字の並びはここで切れます
fn is_sentinel(c: char) -> bool {
    (0xE000..=0xF8FF).contains(&(c as u32))
}

/// URL の頭(`https://` など)の長さ。本家が自動でリンクにする5つ
fn scheme_len(t: &[char], i: usize) -> Option<usize> {
    ["https://", "http://", "file://", "ftp://", "irc://"]
        .iter()
        .find(|s| starts_with(t, i, s))
        .map(|s| s.len())
}

/// リンクの `[…]` の中身 → (表示する字, 役割)。
///
/// `=` があれば属性の並びとして読みます(本家と同じ)。最初の名前無しの
/// 項目が字で、`role=` だけを持ち越します。字が空なら URL を字にします。
/// 末尾の `^`(別の窓で開く印)は模型に無いので、字に残します
fn link_text(raw: &str, url: &str) -> (String, Option<String>) {
    let raw = raw.replace("\\]", "]");
    if !raw.contains('=') {
        return (if raw.is_empty() { url.to_string() } else { raw }, None);
    }
    // `,` で切る(引用符の中は切らない)
    let mut items: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in raw.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ',' => items.push(std::mem::take(&mut cur)),
            None => cur.push(c),
        }
    }
    items.push(cur);
    let is_name = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let named: Vec<(String, String)> = items
        .iter()
        .filter_map(|it| it.split_once('=').filter(|(k, _)| is_name(k.trim())).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
        .collect();
    if named.is_empty() {
        // 属性は無い。ただし引用符で囲んだ字は、引用符を外した物が字
        let first = items.first().map(|s| s.trim()).unwrap_or("");
        let quoted = raw.trim().starts_with(['"', '\'']) && items.len() == 1;
        return (if quoted { first.to_string() } else { raw }, None);
    }
    let text = items
        .first()
        .filter(|it| !it.split_once('=').is_some_and(|(k, _)| is_name(k.trim())))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let role = named.iter().find(|(k, _)| k == "role").map(|(_, v)| v.replace(' ', "."));
    (if text.is_empty() { url.to_string() } else { text }, role)
}

/// 段落の字(行は継いだ後)を run の並びへ
pub(crate) fn parse(src: &str, doc: &mut Document, fresh_note: &mut usize) -> Result<Vec<Run>, String> {
    let m = mark_up(src);
    let mut w = Walker { m: &m, doc, fresh_note };
    let mut st = State::default();
    w.walk(&m.t, &mut st)
}

// ---------------------------------------------------------------- 書く側の自己確認

/// 書く側が、字としての印を仮の字に置き換える([`settle`] が戻します)
pub(crate) fn twin(c: char) -> char {
    match MARK_CHARS.iter().position(|&m| m == c) {
        Some(k) => char::from_u32(TWIN0 + k as u32).expect("私用領域"),
        None => c,
    }
}

fn untwin(c: char) -> char {
    let n = (c as u32).wrapping_sub(TWIN0);
    if (n as usize) < MARK_CHARS.len() {
        MARK_CHARS[n as usize]
    } else {
        c
    }
}

/// 読み直して比べるための形。adoc が運ぶ欄だけを見ます
fn shape(runs: &[Run]) -> Vec<(char, CharFormat)> {
    let mut v = Vec::new();
    for r in runs {
        let mut f = r.fmt.clone();
        f.underline = false;
        f.strike = false;
        f.color = None;
        if f.highlight.is_some() {
            f.highlight = Some(MARK_COLOR.to_string());
        }
        if f.footnote.is_some() {
            f.footnote = Some(FootnoteRef { id: String::new(), endnote: false });
        }
        if r.text.is_empty() && (f.footnote.is_some() || f.sdt.is_some()) {
            v.push(('\0', f));
            continue;
        }
        for c in r.text.chars() {
            v.push((c, f.clone()));
        }
    }
    v
}

fn distance(a: &[(char, CharFormat)], b: &[(char, CharFormat)]) -> usize {
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    a.len() + b.len() - 2 * (pre + suf)
}

/// 書いた1行を読み直し、意図した run の並びと違うところに `\` を足す。
///
/// `line` には、字としての印が [`twin`] の仮の字で入っています。
/// 全部本物に戻してから読み、違えば仮の字だった所へ左から順に `\` を
/// 試し、違いが減る所だけ採ります。
pub(crate) fn settle(line: &str, intended: &[Run]) -> String {
    let want = shape(intended);
    let mut cur: Vec<char> = Vec::with_capacity(line.len());
    let mut spots: Vec<usize> = Vec::new();
    for c in line.chars() {
        let u = untwin(c);
        if u != c {
            spots.push(cur.len());
        }
        cur.push(u);
    }
    // マクロの頭(URL など)も、`\` が要るかもしれない場所です
    for k in 0..cur.len() {
        if MACRO_HEADS.iter().any(|h| starts_with(&cur, k, h))
            || (cur[k] == '<' && scheme_len(&cur, k + 1).is_some())
        {
            spots.push(k);
        }
    }
    spots.sort_unstable();
    spots.dedup();
    if spots.is_empty() {
        return cur.into_iter().collect();
    }
    let score = |v: &[char]| -> usize {
        let s: String = v.iter().collect();
        let mut scratch = Document::default();
        let mut n = 0usize;
        match parse(&s, &mut scratch, &mut n) {
            Ok(runs) => distance(&shape(&runs), &want),
            Err(_) => usize::MAX,
        }
    };
    let mut best = score(&cur);
    if best == 0 {
        return cur.into_iter().collect();
    }
    // 左から順に、`\` を1つ、だめなら2つ試す(`\\**字**` のように
    // 二重の印には2つ要ることがある)。違いが減る所だけ採ります
    let mut shift = 0usize;
    let mut added: Vec<usize> = Vec::new();
    for at in spots {
        for n in 1..=2 {
            let mut trial = cur.clone();
            for _ in 0..n {
                trial.insert(at + shift, '\\');
            }
            let s = score(&trial);
            if s < best {
                cur = trial;
                best = s;
                for k in 0..n {
                    added.push(at + shift + k);
                }
                shift += n;
                break;
            }
        }
        if best == 0 {
            break;
        }
    }
    // 途中で足した `\` のうち、外しても読みが悪くならない物は外します
    // (`[input]\`字`` に `\[input]` まで付いてしまうのを防ぐ)
    if added.len() > 1 {
        for &at in added.iter().rev() {
            let mut trial = cur.clone();
            trial.remove(at);
            let s = score(&trial);
            if s <= best {
                cur = trial;
                best = s;
            }
        }
    }
    cur.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(src: &str) -> Vec<Run> {
        let mut d = Document::default();
        let mut n = 0;
        parse(src, &mut d, &mut n).expect("読めない")
    }

    #[test]
    fn a_single_mark_needs_a_boundary() {
        let r = runs("*a few strong words*");
        assert_eq!(r.len(), 1);
        assert!(r[0].fmt.bold);
        assert_eq!(r[0].text, "a few strong words");
        // 前が字なら印ではない
        let r = runs("README-de_DE and _x_");
        assert_eq!(r[0].text, "README-de_DE and ");
        assert!(!r[0].fmt.italic);
        assert!(r[1].fmt.italic);
    }

    #[test]
    fn a_double_mark_works_anywhere() {
        let r = runs("**Git**Hub");
        assert_eq!((r[0].text.as_str(), r[0].fmt.bold), ("Git", true));
        assert_eq!((r[1].text.as_str(), r[1].fmt.bold), ("Hub", false));
    }

    #[test]
    fn a_backslash_stays_unless_it_escapes_a_match() {
        let r = runs("5 \\* 3");
        assert_eq!(r[0].text, "5 \\* 3");
        let r = runs("\\*a*");
        assert_eq!(r[0].text, "*a*");
        assert!(!r[0].fmt.bold);
    }

    #[test]
    fn roles_and_marks() {
        let r = runs("[.white.red-background]#alert#");
        assert_eq!(r[0].fmt.style_id.as_deref(), Some("white.red-background"));
        assert!(r[0].fmt.highlight.is_none());
        let r = runs("#a few words#");
        assert_eq!(r[0].fmt.highlight.as_deref(), Some(MARK_COLOR));
    }

    #[test]
    fn passthroughs_keep_the_text() {
        let r = runs("+++*x*+++ and +*y*+ and pass:[_z_]");
        let text: String = r.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(text, "*x* and *y* and _z_");
        assert!(r.iter().all(|r| !r.fmt.bold && !r.fmt.italic));
        let r = runs("`+lit *x*+`");
        assert_eq!(r[0].fmt.style_id.as_deref(), Some(MONO));
        assert_eq!(r[0].text, "lit *x*");
    }

    #[test]
    fn settle_adds_a_backslash_only_where_needed() {
        let lit = |s: &str| -> String { s.chars().map(twin).collect() };
        let plain = |t: &str| Run { text: t.to_string(), size_pt: None, font: None, fmt: CharFormat::default() };
        assert_eq!(settle(&lit("5 * 3 * 2"), &[plain("5 * 3 * 2")]), "5 * 3 * 2");
        assert_eq!(settle(&lit("x *y* z"), &[plain("x *y* z")]), "x \\*y* z");
    }
}
