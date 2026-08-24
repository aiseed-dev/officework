//! セルの文字列を **AsciiDoc として読む**(2026-08-18 発注者確定。
//! 2026-08-19 にマークダウンから移した)。
//!
//! *writer と2つの書き方を持つ理由がありません。* 文書の本文が AsciiDoc に
//! なったので、セルの中も同じ書き方にしました。**印が変わるだけで、
//! できることは変わりません。**
//!
//! [cols="1,1"]
//! |===
//! |できること |書き方
//!
//! |太字 |`**字**`
//! |斜体 |`__字__`
//! |取り消し線 |`[.line-through]##字##`
//! |等幅 |```` ``字`` ````
//! |リンク |`URL[字]`(前後に空白)
//! |見出し 1〜3 |`= ` `== ` `=== `
//! |箇条書き |`* `
//! |番号つき |`. `(番号は書かない)
//! |===
//!
//! **二重の印はどこでも効きます。** 一重(`*字*`)は本家と同じく
//! 「語の外」だけで効くので、語の間に空白の無い日本語の文中では
//! 二重が要ります(2026-08-19 に本家へ通して確かめました)。
//! `A_1_B` という名前や `2*3*4` という掛け算は書式に化けません。
//! 二重でも、中身が数字だけ(`2**3**4`)なら書式にしません。
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
//! (`parse` が None)。

/// 一行の種類。セルは一行の入れ物なので、見出しと箇条書きだけを持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Para,
    /// `= ` `== ` `=== ` の 1〜3
    Heading(u8),
    /// `* `。深さは印の数(`**`)か、行頭の空白 2 つで 1 段
    Bullet(u8),
    /// `. `。**本家は番号を書かない**ので、続きぐあいで振った番号を持つ
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
    /// `URL[字]` の URL
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub block: Block,
    pub spans: Vec<Span>,
}

/// AsciiDoc として読む。**書式が1つも無ければ None**(そのときは
/// 今までどおり平文を1つ描くだけで済む — 普通のセルに費用を掛けない)。
/// 印の1つ。字と、その字が囲んだ所に付ける印。
type 印を付ける = fn(&mut Span);

pub fn parse(text: &str) -> Option<Vec<Line>> {
    // 印が1つも無ければ、読むまでもない(普通のセルに費用を掛けない)
    if !text.contains(['*', '_', '`', '[', '=', '.']) {
        return None;
    }
    let mut lines = Vec::new();
    let mut any = false;
    // 番号つきの続きぐあい。**AsciiDoc は番号を書かない**ので、
    // 続いている間だけ 1 から数えます(間に別の行が入れば振り出し)
    let mut 番号 = 0u32;
    for raw in text.split('\n') {
        let (mut block, rest) = block_of(raw);
        if let Block::Ordered(_) = block {
            番号 += 1;
            block = Block::Ordered(番号);
        } else {
            番号 = 0;
        }
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

/// **選んだ字を印で囲む(もう囲んであれば外す)。**
///
/// リボンの太字・斜体・取り消しのボタンが、セルの編集中に使います
/// (2026-08-19 発注者「セルの中の一部を選択してリボンのボタンをつかえば
/// いいのでは」)。書き方を覚えなくても、選んで押せば印が入ります。
///
/// 返りは(置き換える範囲, 置き換え後の字, 置き換え後に選び直す範囲)。
/// 範囲はバイト位置で、選択は Editor の選択(文字の境目に揃っている)を
/// そのまま受け取ります。
pub fn toggle_wrap(
    text: &str,
    sel: std::ops::Range<usize>,
    open: &str,
    close: &str,
) -> (std::ops::Range<usize>, String, std::ops::Range<usize>) {
    let inner = &text[sel.clone()];
    // すでに囲んであれば外す(同じボタンで行き来できるように)
    if text[..sel.start].ends_with(open) && text[sel.end..].starts_with(close) {
        let from = sel.start - open.len();
        let to = sel.end + close.len();
        return (from..to, inner.to_string(), from..from + inner.len());
    }
    // 囲む。選び直すのは中身(続けて別のボタンも押せるように)
    let rep = format!("{open}{inner}{close}");
    let s = sel.start + open.len();
    (sel.clone(), rep, s..s + inner.len())
}

/// 行頭の印を読んで、種類と残りの文字を返す。
///
/// 番号つきの番号はここでは決めません(AsciiDoc は番号を書かないので、
/// [`parse`] が続きぐあいを見て振ります)。
fn block_of(raw: &str) -> (Block, &str) {
    let indent = raw.len() - raw.trim_start().len();
    let t = raw.trim_start();

    // 見出し: `=` の数(1〜3)。**後ろの空白が要る** — `=SUM(A1)` は式で、
    // 見出しではありません(`kumihan::adoc::is_formula_cell` と同じ決め)
    let 等号 = t.chars().take_while(|c| *c == '=').count();
    if (1..=3).contains(&等号) {
        if let Some(r) = t[等号..].strip_prefix(' ') {
            return (Block::Heading(等号 as u8), r);
        }
    }

    // 箇条書き `* `。**深さは `*` の数**(AsciiDoc の作法)で、字下げでも
    // 数えます。印の後ろに空白が要るので `*太字*` とは紛れません
    let 星 = t.chars().take_while(|c| *c == '*').count();
    if (1..=5).contains(&星) {
        if let Some(r) = t[星..].strip_prefix(' ') {
            let 深さ = if 星 > 1 { 星 - 1 } else { indent / 2 };
            return (Block::Bullet(深さ.min(4) as u8), r);
        }
    }

    // 番号つき `. `。深さは `.` の数。番号は parse が振ります
    let 点 = t.chars().take_while(|c| *c == '.').count();
    if (1..=5).contains(&点) {
        if let Some(r) = t[点..].strip_prefix(' ') {
            return (Block::Ordered(0), r);
        }
    }

    (Block::Para, raw)
}

/// 取り消し線の書き方(本家には専用の印が無く、役割で書きます)。
const 取り消し線: &str = "[.line-through]#";

/// 行の中の印を読んで、続きの文字に割る。
fn inline(s: &str) -> Vec<Span> {
    let b: Vec<char> = s.chars().collect();
    let mut out: Vec<Span> = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < b.len() {
        // 取り消し線 `[.line-through]##字##`(二重 — 日本語の文中で効く形)と
        // `[.line-through]#字#`(一重 — 語の外だけ。英語向け)
        if b[i] == '[' && b[i..].starts_with(&取り消し線.chars().collect::<Vec<_>>()[..]) {
            let mut from = i + 取り消し線.chars().count();
            let 二重 = b.get(from) == Some(&'#');
            if 二重 {
                from += 1;
            }
            let 閉じ = if 二重 {
                b[from..].windows(2).position(|w| w == ['#', '#'])
            } else {
                b[from..].iter().position(|c| *c == '#')
            };
            if let Some(end) = 閉じ {
                if end > 0 {
                    push(&mut out, &mut plain);
                    let text: String = b[from..from + end].iter().collect();
                    out.push(Span { text, strike: true, ..Default::default() });
                    i = from + end + if 二重 { 2 } else { 1 };
                    continue;
                }
            }
        }
        // **二重の印**(`**太字**` `__斜体__` ``` ``等幅`` ```)。
        // 本家では一重は「語の外」だけで効き、日本語には語の間の空白が
        // 無いので、**文中では二重が要ります**(2026-08-19 発注者の指摘で
        // 本家に通して確かめた — 一重は字のまま、二重だけ効いた)
        let 二重印: [(char, 印を付ける); 3] = [
            ('*', (|s: &mut Span| s.bold = true) as fn(&mut Span)),
            ('_', |s: &mut Span| s.italic = true),
            ('`', |s: &mut Span| s.mono = true),
        ];
        let mut hit2 = false;
        for (m, set) in 二重印 {
            if !(b[i] == m && b.get(i + 1) == Some(&m)) {
                continue;
            }
            let from = i + 2;
            let Some(end) = b[from..].windows(2).position(|w| w == [m, m]) else { continue };
            if end == 0 {
                continue; // 中身が空(`****`)
            }
            let inner: String = b[from..from + end].iter().collect();
            // **数字だけなら書式にしない**(表を壊さない線引き)。
            // `2**3**4` を太字にすると、Python 流の冪の字が黙って化ける
            if inner.chars().all(|c| c.is_ascii_digit() || c == '.') {
                continue;
            }
            push(&mut out, &mut plain);
            let mut sp = Span { text: inner, ..Default::default() };
            set(&mut sp);
            out.push(sp);
            i = from + end + 2;
            hit2 = true;
            break;
        }
        if hit2 {
            continue;
        }
        // リンク `URL[字]`
        if let Some((label, url, next)) = link_at(&b, i) {
            push(&mut out, &mut plain);
            out.push(Span { text: label, link: Some(url), ..Default::default() });
            i = next;
            continue;
        }
        // *太字* / _斜体_ / `等幅`
        let marks: [(char, 印を付ける); 3] = [
            ('*', (|s: &mut Span| s.bold = true) as fn(&mut Span)),
            ('_', |s: &mut Span| s.italic = true),
            ('`', |s: &mut Span| s.mono = true),
        ];
        let mut hit = false;
        for (mark, set) in marks {
            if b[i] != mark {
                continue;
            }
            let Some((inner, next)) = 囲みを読む(&b, i, mark) else { continue };
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

/// 字の区切りか(語の中か外かを見る)。**本家の「囲みの印は語の外だけ」**
/// を判じるのに使います。
///
/// *「語の中」と見るのは英数字と `_` だけ*です。日本語は語の間に空白を
/// 置かないので、かなや漢字まで語の中と見ると `これは*太字*です` が
/// 書式になりません。これで `ABC_001` や `2*3*4` は今までどおり素通しし、
/// 日本語の中の印は効きます。
///
/// *残る穴。* `売上_合計_表` のように**日本語の間に `_` を挟んだ名前**は
/// 斜体になります。英数字の名前(`ABC_001`)は安全です。
fn 区切り(c: Option<&char>) -> bool {
    match c {
        None => true,
        Some(c) => !c.is_ascii_alphanumeric() && *c != '_',
    }
}

/// `印`で囲まれた所を読む。返りは (中身, 続きの位置)。
///
/// **本家と同じ「語の外だけ」の決まり**(constrained formatting)で読みます。
/// 印の前が語の中(字か数字)なら書式にしません。これで
/// `A_1_B`(名前)や `2*3*4`(掛け算)が斜体や太字に化けません。
fn 囲みを読む(b: &[char], at: usize, mark: char) -> Option<(String, usize)> {
    // 開きの印: 前が語の外で、後ろが空白でない
    if !区切り(at.checked_sub(1).map(|k| &b[k])) {
        return None;
    }
    let from = at + 1;
    if b.get(from).is_none_or(|c| c.is_whitespace()) {
        return None;
    }
    let mut j = from;
    while j < b.len() {
        if b[j] == mark {
            // 閉じの印: 前が空白でなく、後ろが語の外
            if j > from && !b[j - 1].is_whitespace() && 区切り(b.get(j + 1)) {
                let inner: String = b[from..j].iter().collect();
                // **数字だけなら書式にしない**(表を壊さない線引き)。
                // `2**3**4` の開きの2つ目の `*` が、一重として `*3*` を
                // 掴んでいた(2026-08-19 に試験で見つけた)
                if inner.chars().all(|c| c.is_ascii_digit() || c == '.') {
                    return None;
                }
                return Some((inner, j + 1));
            }
        }
        j += 1;
    }
    None
}

/// `URL[字]`(本家の書き方)。返りは (字, URL, 続きの位置)。
///
/// URL の頭は行頭か空白の後ろだけを見ます。`https://` `http://` `mailto:` の
/// 3つに絞るのは、`[` の前の字を何でも URL と読むと普通の字が化けるためです。
fn link_at(b: &[char], at: usize) -> Option<(String, String, usize)> {
    if !区切り(at.checked_sub(1).map(|k| &b[k])) {
        return None;
    }
    let 頭: [&str; 3] = ["https://", "http://", "mailto:"];
    let 残り: String = b[at..].iter().collect();
    if !頭.iter().any(|h| 残り.starts_with(h)) {
        return None;
    }
    // URL は `[` まで(空白が来たらリンクではない)
    let mut j = at;
    while j < b.len() && b[j] != '[' {
        if b[j].is_whitespace() {
            return None;
        }
        j += 1;
    }
    let close = b[j..].iter().position(|c| *c == ']')? + j;
    let url: String = b[at..j].iter().collect();
    let label: String = b[j + 1..close].iter().collect();
    if url.is_empty() || label.is_empty() {
        return None;
    }
    Some((label, url, close + 1))
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
        assert!(parse("商品名*").is_none(), "片方だけの印が化けた");
        assert!(parse("在庫#").is_none());
    }

    /// **囲みの印は語の外だけ**(本家の決まり)。名前や式が化けない
    #[test]
    fn 語の中の印は書式にしない() {
        assert!(parse("A_1_B").is_none(), "下線つきの名前が斜体に化けた");
        assert!(parse("=2*3*4 の答え").is_none(), "掛け算が太字に化けた");
        assert!(parse("ABC_001_X").is_none(), "英数字の名前が斜体に化けた");
        assert!(parse("2*3").is_none());
        // **日本語は語の間に空白が無い**ので、かなや漢字は語の外と見る。
        // これで日本語の中の印が効く(下の試験)。裏返しに、日本語の間に
        // `_` を挟んだ名前は斜体になる — 分かっていて残す穴
        assert!(parse("売上_合計_表").is_some(), "この穴は分かっている");
    }

    /// **日本語の文中は二重の印**(2026-08-19 発注者の指摘)。
    /// 一重は本家で字のまま出るので、確実なのは二重
    #[test]
    fn 二重の印は文中で効く() {
        let s = one("これは**太字**です");
        assert_eq!(s.len(), 3);
        assert_eq!(s[1].text, "太字");
        assert!(s[1].bold);
        assert!(one("これは__斜体__です")[1].italic);
        assert!(one("値は``x``です")[1].mono);
        let t = one("これは[.line-through]##取消##です");
        assert!(t[1].strike, "{t:?}");
        assert_eq!(t[1].text, "取消");
    }

    /// 二重でも、中身が数字だけなら書式にしない(`2**3**4` は冪の字)
    #[test]
    fn 数字だけの二重は書式にしない() {
        assert!(parse("2**3**4").is_none(), "冪の字が太字に化けた");
        assert!(parse("x = 2**10").is_none(), "閉じの無い二重が化けた");
    }

    #[test]
    fn 行の中の印を読む() {
        let s = one("これは*太字*です");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].text, "これは");
        assert!(!s[0].bold);
        assert_eq!(s[1].text, "太字");
        assert!(s[1].bold, "日本語は前後に空白が無い — それでも太字になること");
        assert_eq!(s[2].text, "です");

        assert!(one("_斜体_")[0].italic);
        assert!(one("`等幅`")[0].mono);
        assert!(one("[.line-through]#取消#")[0].strike);
    }

    /// 本家の書き方は `URL[字]`
    #[test]
    fn リンクを読む() {
        let s = one("詳しくは https://example.com/a[こちら] を見て");
        assert_eq!(s[1].text, "こちら");
        assert_eq!(s[1].link.as_deref(), Some("https://example.com/a"));
        assert!(s[2].text.contains("を見て"));

        assert!(one("mailto:a@example.jp[問い合わせ]")[0].link.is_some());
        // 形が崩れていれば平文のまま
        assert!(parse("https://example.com").is_none(), "字だけの URL を印にした");
        assert!(parse("https://example.com[]").is_none());
    }

    #[test]
    fn 見出しと箇条書き() {
        let l = parse("= 見出し").unwrap();
        assert_eq!(l[0].block, Block::Heading(1));
        assert_eq!(l[0].spans[0].text, "見出し");
        assert_eq!(parse("=== 小見出し").unwrap()[0].block, Block::Heading(3));
        // ==== 以上は見出しにしない(セルは一行の入れ物)
        assert!(parse("==== 深すぎ").is_none());
        // **空白が要る** — `=SUM(A1)` は式であって見出しではない
        assert!(parse("=SUM(A1)").is_none(), "式を見出しにした");
        assert!(parse("=A1*B1").is_none(), "式を書式にした");

        let l = parse("* 甲\n* 乙\n** 丙").unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].block, Block::Bullet(0));
        assert_eq!(l[2].block, Block::Bullet(1), "印の数が段になっていない");
        assert_eq!(l[2].spans[0].text, "丙");
        // 字下げでも段になる
        assert_eq!(parse("* 甲\n  * 丙").unwrap()[1].block, Block::Bullet(1));

        // 番号は本家どおり書かない。続いている間だけ数える
        let l = parse(". 甲\n. 乙").unwrap();
        assert_eq!(l[0].block, Block::Ordered(1));
        assert_eq!(l[1].block, Block::Ordered(2));
    }

    #[test]
    fn 混ぜても読める() {
        let l = parse("== 在庫\n* *甲*は 12 個\n* 乙は https://x[表] へ").unwrap();
        assert_eq!(l[0].block, Block::Heading(2));
        assert_eq!(l[1].block, Block::Bullet(0));
        assert!(l[1].spans[0].bold);
        assert_eq!(l[2].spans[1].link.as_deref(), Some("https://x"));
    }

    /// **選んで押すと印が入り、もう一度で外れる**(リボンのボタンの中身)
    #[test]
    fn 選んだ字を囲んで外せる() {
        let t = "これは太字です";
        let sel = 9..15; // 「太字」
        let (r, rep, s2) = toggle_wrap(t, sel, "**", "**");
        assert_eq!(r, 9..15);
        assert_eq!(rep, "**太字**");
        let t2 = format!("{}{}{}", &t[..r.start], rep, &t[r.end..]);
        assert_eq!(t2, "これは**太字**です");
        assert_eq!(&t2[s2.clone()], "太字", "選び直しがずれた");

        // もう一度 → 外れる
        let (r3, rep3, s3) = toggle_wrap(&t2, s2, "**", "**");
        let t3 = format!("{}{}{}", &t2[..r3.start], rep3, &t2[r3.end..]);
        assert_eq!(t3, "これは太字です");
        assert_eq!(&t3[s3], "太字");
    }

    /// 取り消し線のように開きと閉じが違う印でも往復できる
    #[test]
    fn 開きと閉じが違う印も往復する() {
        let t = "予定は中止です";
        let (r, rep, s2) = toggle_wrap(t, 9..15, "[.line-through]##", "##");
        let t2 = format!("{}{}{}", &t[..r.start], rep, &t[r.end..]);
        assert_eq!(t2, "予定は[.line-through]##中止##です");
        let (r3, rep3, _) = toggle_wrap(&t2, s2, "[.line-through]##", "##");
        let t3 = format!("{}{}{}", &t2[..r3.start], rep3, &t2[r3.end..]);
        assert_eq!(t3, "予定は中止です");
    }

    /// 印を外した見た目の字(幅の見積りに使う)
    #[test]
    fn 印を外した字が出る() {
        let l = parse("* 甲\n. 乙").unwrap();
        assert_eq!(plain(&l), "・甲 1. 乙");
    }
}
