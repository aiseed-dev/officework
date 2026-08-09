//! セルの文字列をマークダウンとして読む(2026-08-09 発注者確定)。
//!
//! **セルが持つのは平文のまま。** 書式は文字の中に記号として書かれていて、
//! 画面に描くときだけ解釈する。だから
//!
//! - xlsx へは**平文で入る** — 往復で絶対に化けない(リッチテキストの run に
//!   変換しない。Excel では記号がそのまま見える = 正直な劣化)
//! - **セルの中の一部だけを太字にする編集 UI が要らない**。文字を打つのと
//!   同じ操作で書式が付く
//! - 数式バーに出るのも、Python が読むのも、いつも同じ平文
//!
//! 数の入った表を壊さないよう、**書式と読めるものが1つも無ければ何も返さない**
//! (`parse` が None)。`2*3*4` のような掛け算が斜体に化けないように、
//! 記号で囲まれた中身が**数字だけのときは書式にしない**。

/// 一行の種類。セルは一行の入れ物なので、見出しと箇条書きだけを持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Para,
    /// `# ` `## ` `### ` の 1〜3
    Heading(u8),
    /// `- ` `* ` `+ `。深さは行頭の空白 2 つで 1 段
    Bullet(u8),
    /// `1. `。持っているのは書かれていた番号
    Ordered(u32),
}

/// 続きの文字と、そこに掛かっている書式。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    /// `` `等幅` ``
    pub mono: bool,
    /// `[文字](URL)` の URL
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub block: Block,
    pub spans: Vec<Span>,
}

/// マークダウンとして読む。**書式が1つも無ければ None**(そのときは
/// 今までどおり平文を1つ描くだけで済む — 普通のセルに費用を掛けない)。
pub fn parse(text: &str) -> Option<Vec<Line>> {
    if !text.contains(['*', '_', '`', '~', '#', '[', '-'])
        && !text.trim_start().starts_with(|c: char| c.is_ascii_digit())
    {
        return None;
    }
    let mut lines = Vec::new();
    let mut any = false;
    for raw in text.split('\n') {
        let (block, rest) = block_of(raw);
        if block != Block::Para {
            any = true;
        }
        let spans = inline(rest);
        if spans.iter().any(|s| s.bold || s.italic || s.strike || s.mono || s.link.is_some()) {
            any = true;
        }
        lines.push(Line { block, spans });
    }
    any.then_some(lines)
}

/// 見出しの書式。**正はブックの名前付きセルスタイル**(「見出し 1」など。
/// xlsx の `cellStyles` の builtinId 16/17/18)で、型紙(.xltx)に定義して
/// おけば、そこから作ったブック全部に効く(2026-08-09 発注者「テンプレートに
/// 設定できませんか?」)。Excel で「見出し 1」を編集しても追随する。
///
/// この定数は**型紙が何も持っていないときの既定**でしかない。
pub const DEFAULT_HEADINGS: [Heading; 3] = [
    Heading { scale: 1.60, bold: true },
    Heading { scale: 1.35, bold: true },
    Heading { scale: 1.15, bold: true },
];

/// 普通の文字の大きさ(pt)。名前付きスタイルの pt を比に直すときの分母。
pub const BASE_PT: f32 = 11.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heading {
    /// 普通の文字に対する大きさの比
    pub scale: f32,
    pub bold: bool,
}

/// `# ` の数(1〜3)から既定の書式を引く。
pub fn default_heading(level: u8) -> Option<Heading> {
    DEFAULT_HEADINGS.get(level.saturating_sub(1) as usize).copied()
}

/// ブックの名前付きスタイルから見出しの書式を引く。**型紙が正**で、
/// 見つからなければ既定に落ちる。
///
/// 見分け方は builtinId(Excel が「見出し 1/2/3」に振る 16/17/18)を第一に、
/// 無ければ名前の末尾の数字(「見出し 1」「Heading 1」どちらでも)。
pub fn heading_of(
    named: &[(String, Option<u32>, crate::model::CellFormat)],
    level: u8,
) -> Option<Heading> {
    let want_builtin = 15 + level as u32; // 見出し1 → 16
    let found = named.iter().find(|(name, b, _)| {
        *b == Some(want_builtin)
            || (b.is_none() && name.trim_end().ends_with(&level.to_string()))
    });
    match found {
        Some((_, _, f)) => Some(Heading {
            scale: f.size_c.map(|c| c as f32 / 100.0 / BASE_PT).unwrap_or(1.0).max(0.1),
            bold: f.bold,
        }),
        None => default_heading(level),
    }
}

/// この行の文字の大きさの比(見出しでなければ 1.0)。
pub fn line_scale(l: &Line, named: &[(String, Option<u32>, crate::model::CellFormat)]) -> f32 {
    match l.block {
        Block::Heading(n) => heading_of(named, n).map(|h| h.scale).unwrap_or(1.0),
        _ => 1.0,
    }
}

/// この中身を出すのに要る行の高さ(pt)。`base_pt` は普通の行の高さ
/// (xlsx の既定は 15pt)。**見出しがあれば、その比のぶんだけ高くする。**
/// 比は型紙の名前付きスタイルから引く(`named`)。
pub fn wanted_height_pt(
    lines: &[Line],
    base_pt: f32,
    named: &[(String, Option<u32>, crate::model::CellFormat)],
) -> f32 {
    lines.iter().map(|l| base_pt * line_scale(l, named)).sum()
}

/// 印を外した後の見た目の文字(幅の見積りに使う。箇条書きの中黒も数える)。
pub fn plain(lines: &[Line]) -> String {
    let mut out = String::new();
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match l.block {
            Block::Bullet(d) => {
                out.push_str(&"  ".repeat(d as usize));
                out.push('・');
            }
            Block::Ordered(n) => out.push_str(&format!("{n}. ")),
            _ => {}
        }
        for sp in &l.spans {
            out.push_str(&sp.text);
        }
    }
    out
}

/// 行頭の印を読んで、種類と残りの文字を返す。
fn block_of(raw: &str) -> (Block, &str) {
    let indent = raw.len() - raw.trim_start().len();
    let t = raw.trim_start();
    // 見出し: # の数。#### 以上は見出しにしない(セルは一行の入れ物)
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if (1..=3).contains(&hashes) {
        if let Some(r) = t[hashes..].strip_prefix(' ') {
            return (Block::Heading(hashes as u8), r);
        }
    }
    // 箇条書き。**印の後ろに空白が要る** — 「-100」は箇条書きではない
    for mark in ['-', '*', '+'] {
        if let Some(r) = t.strip_prefix(mark).and_then(|r| r.strip_prefix(' ')) {
            return (Block::Bullet((indent / 2).min(4) as u8), r);
        }
    }
    // 番号つき。「1. 」だけ — 「1.5」は数
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits <= 3 {
        if let Some(r) = t[digits..].strip_prefix(". ") {
            if let Ok(n) = t[..digits].parse::<u32>() {
                return (Block::Ordered(n), r);
            }
        }
    }
    (Block::Para, raw)
}

/// 行の中の印を読んで、続きの文字に割る。
fn inline(s: &str) -> Vec<Span> {
    let b: Vec<char> = s.chars().collect();
    let mut out: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < b.len() {
        // [文字](URL)
        if b[i] == '[' {
            if let Some((label, url, next)) = link_at(&b, i) {
                push(&mut out, &mut plain);
                out.push(Span { text: label, link: Some(url), ..Default::default() });
                i = next;
                continue;
            }
        }
        // **太字** / *斜体* / ~~取消線~~ / `等幅`
        let marks: [(&[char], fn(&mut Span)); 4] = [
            (&['*', '*'], (|s: &mut Span| s.bold = true) as fn(&mut Span)),
            (&['~', '~'], |s: &mut Span| s.strike = true),
            (&['`'], |s: &mut Span| s.mono = true),
            (&['*'], |s: &mut Span| s.italic = true),
        ];
        let mut hit = false;
        for (mark, set) in marks {
            if !b[i..].starts_with(mark) {
                continue;
            }
            let Some((inner, next)) = closed_at(&b, i, mark) else { continue };
            // `2*3*4` を斜体にしない — 囲みの中が数字だけなら書式にしない
            if inner.chars().all(|c| c.is_ascii_digit() || c == '.') {
                continue;
            }
            push(&mut out, &mut plain);
            let mut sp = Span { text: inner, ..Default::default() };
            set(&mut sp);
            out.push(sp);
            i = next;
            hit = true;
            break;
        }
        if hit {
            continue;
        }
        plain.push(b[i]);
        i += 1;
    }
    push(&mut out, &mut plain);
    if out.is_empty() {
        out.push(Span::default());
    }
    out
}

fn push(out: &mut Vec<Span>, plain: &mut String) {
    if !plain.is_empty() {
        out.push(Span { text: std::mem::take(plain), ..Default::default() });
    }
}

/// `mark` で始まって同じ `mark` で閉じているか。返りは (中身, 続きの位置)。
/// 中身が空のもの(`****`)は書式にしない。
fn closed_at(b: &[char], at: usize, mark: &[char]) -> Option<(String, usize)> {
    let from = at + mark.len();
    let mut j = from;
    while j + mark.len() <= b.len() {
        if b[j..].starts_with(mark) {
            if j == from {
                return None; // 中身が空
            }
            return Some((b[from..j].iter().collect(), j + mark.len()));
        }
        j += 1;
    }
    None
}

/// `[文字](URL)`。返りは (文字, URL, 続きの位置)。
fn link_at(b: &[char], at: usize) -> Option<(String, String, usize)> {
    let close = b[at + 1..].iter().position(|c| *c == ']')? + at + 1;
    if b.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = b[close + 2..].iter().position(|c| *c == ')')? + close + 2;
    let label: String = b[at + 1..close].iter().collect();
    let url: String = b[close + 2..end].iter().collect();
    if label.is_empty() || url.is_empty() {
        return None;
    }
    Some((label, url, end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(s: &str) -> Vec<Span> {
        parse(s).expect("書式として読めない").remove(0).spans
    }

    #[test]
    fn 普通の文字は素通しする() {
        // 書式が1つも無ければ None — 普通のセルに費用を掛けない
        assert!(parse("日本フネン株式会社").is_none());
        assert!(parse("").is_none());
        assert!(parse("2026-08-09").is_none(), "日付が箇条書きに化けた");
        assert!(parse("-100").is_none(), "負の数が箇条書きに化けた");
        assert!(parse("1.5").is_none(), "小数が番号つきに化けた");
        assert!(parse("A_1_B").is_none(), "下線つきの名前が化けた");
        assert!(parse("=2*3*4 の答え").is_none(), "掛け算が斜体に化けた");
        assert!(parse("商品名*").is_none(), "片方だけの印が化けた");
        assert!(parse("在庫#").is_none());
    }

    #[test]
    fn 行の中の印を読む() {
        let s = one("これは**太字**です");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].text, "これは");
        assert!(!s[0].bold);
        assert_eq!(s[1].text, "太字");
        assert!(s[1].bold, "日本語は前後に空白が無い — それでも太字になること");
        assert_eq!(s[2].text, "です");

        assert!(one("*斜体*")[0].italic);
        assert!(one("~~取消~~")[0].strike);
        assert!(one("`等幅`")[0].mono);
        // 太字が斜体に負けない(** を先に見る)
        let b = one("**強い**");
        assert!(b[0].bold && !b[0].italic);
    }

    #[test]
    fn リンクを読む() {
        let s = one("詳しくは[こちら](https://example.com/a)を見て");
        assert_eq!(s[1].text, "こちら");
        assert_eq!(s[1].link.as_deref(), Some("https://example.com/a"));
        assert_eq!(s[2].text, "を見て");
        // 形が崩れていれば平文のまま
        assert!(parse("[こちら]").is_none());
        assert!(parse("[](url)").is_none());
    }

    #[test]
    fn 見出しと箇条書き() {
        let l = parse("# 見出し").unwrap();
        assert_eq!(l[0].block, Block::Heading(1));
        assert_eq!(l[0].spans[0].text, "見出し");
        assert_eq!(parse("### 小見出し").unwrap()[0].block, Block::Heading(3));
        // #### 以上は見出しにしない(セルは一行の入れ物)
        assert!(parse("#### 深すぎ").is_none());
        // 空白が要る — 「#1」は見出しではない
        assert!(parse("#1 番").is_none());

        let l = parse("- 甲\n- 乙\n  - 丙").unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].block, Block::Bullet(0));
        assert_eq!(l[2].block, Block::Bullet(1), "字下げが段になっていない");
        assert_eq!(l[2].spans[0].text, "丙");

        let l = parse("1. 甲\n2. 乙").unwrap();
        assert_eq!(l[0].block, Block::Ordered(1));
        assert_eq!(l[1].block, Block::Ordered(2));
    }

    #[test]
    fn 混ぜても読める() {
        let l = parse("## 在庫\n- **甲**は 12 個\n- 乙は[表](u)へ").unwrap();
        assert_eq!(l[0].block, Block::Heading(2));
        assert_eq!(l[1].block, Block::Bullet(0));
        assert!(l[1].spans[0].bold);
        assert_eq!(l[2].spans[1].link.as_deref(), Some("u"));
    }
}
