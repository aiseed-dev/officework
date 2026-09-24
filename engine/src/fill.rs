//! **Merge** — pours data into a template (the heart of forms).
//!
//! The owner asked for a form builder on 2026-08-17. In an invoice or a delivery
//! note the top and the bottom are fixed, and **only the number of detail rows
//! comes from the data**. Filling in that part is what this module does.
//!
//! ## How a template is written
//!
//! ```text
//! 請求先: {宛名} 様
//!
//! |===
//! | 品名 | 数量 | 金額
//! | {明細.品名} | {明細.数量} | {明細.金額}
//! |===
//!
//! 合計 {合計} 円
//! ```
//!
//! `{member}` is replaced as it stands (the same notation as an AsciiDoc attribute
//! reference; the earlier `{{member}}` is still accepted). **A table row that
//! contains `{群.項目}` is repeated once for each row of that group.** We do not ask
//! for a separate marker for the repeat, so that the writer has less to learn.
//!
//! ## Not written once per output format
//!
//! Merging is done **on the document model**, and the finished document is then
//! turned into PDF, HTML or docx. If merging were written per format, the same
//! template would give different results in different formats.
//!
//! ## An unknown name is not silently blanked
//!
//! A name that is not in the data is left as written and listed in [`Report`].
//! Blanking it would quietly produce an invoice with an empty amount.

use crate::doc::{Block, Document, Paragraph, Run, Table};
use std::collections::BTreeMap;

/// 流し込むデータ。
#[derive(Debug, Clone, Default)]
pub struct Data {
    /// 1つだけの値(`{宛名}`)
    pub values: BTreeMap<String, String>,
    /// 繰り返す値(`{明細.品名}`)。群の名前 → 行の並び
    pub rows: BTreeMap<String, Vec<BTreeMap<String, String>>>,
}

impl Data {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&mut self, name: &str, value: impl Into<String>) -> &mut Self {
        self.values.insert(name.into(), value.into());
        self
    }
    /// 群に1行足します。
    pub fn push_row(&mut self, group: &str, row: BTreeMap<String, String>) -> &mut Self {
        self.rows.entry(group.into()).or_default().push(row);
        self
    }
}

/// 差し込みの結果の報告。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    /// データに無かった名前(出てきた順、重複なし)
    pub unknown: Vec<String>,
    /// 増やした行の数(群の名前 → 行数)
    pub expanded: BTreeMap<String, usize>,
}

impl Report {
    /// 人に見せる1行。**分からない名前があればそれを先に言います。**
    pub fn summary(&self) -> String {
        if self.unknown.is_empty() {
            let n: usize = self.expanded.values().sum();
            format!("差し込みました(明細 {n} 行)")
        } else {
            format!(
                "データに無い名前が {} 個あります: {}",
                self.unknown.len(),
                self.unknown.join(" / ")
            )
        }
    }
}

/// CSV(1行目が見出し)を読んで [`Data`] にします。
///
/// **1枚で足ります。** 見出しが `{member}` と同じなら、その値は**1行目**から
/// 取ります(宛名や合計のように1つだけの値)。表の繰り返しには全部の行を
/// 使います。2枚に分けさせないのは、書く人の手間を増やさないためです。
///
/// 区切りはカンマ、囲みは `"` です。改行を含む欄も読めます。
pub fn from_csv(src: &str, group: &str) -> Data {
    let rows = read_csv(src);
    let mut d = Data::new();
    let Some(head) = rows.first() else { return d };
    for r in rows.iter().skip(1) {
        let mut one = BTreeMap::new();
        for (i, h) in head.iter().enumerate() {
            one.insert(h.clone(), r.get(i).cloned().unwrap_or_default());
        }
        // 1つだけの値は1行目から
        if d.values.is_empty() {
            for (k, v) in &one {
                d.values.insert(k.clone(), v.clone());
            }
        }
        d.rows.entry(group.to_string()).or_default().push(one);
    }
    d
}

/// CSV を桁の並びに。囲みの中の改行とカンマ、`""` の逃がしを見ます。
fn read_csv(src: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut it = src.chars().peekable();
    while let Some(c) = it.next() {
        if quoted {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    it.next();
                    cell.push('"');
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
            continue;
        }
        match c {
            '"' if cell.is_empty() => quoted = true,
            ',' => row.push(std::mem::take(&mut cell)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            _ => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

/// この文書が名指している群(`{群.項目}` の群)。1つも無ければ None。
///
/// 2つ以上あれば、どれに流すかは人が決めることなので **None を返さず全部**
/// 返します。呼ぶ側が「1つでなければ断る」と決められます。
pub fn groups(doc: &Document) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let mut see = |p: &Paragraph| {
        if let Some(g) = group_of(p) {
            if !v.contains(&g) {
                v.push(g);
            }
        }
    };
    for b in &doc.blocks {
        match b {
            Block::Para(p) => see(p),
            Block::Table(t) => {
                for p in t.all_paragraphs() {
                    see(p);
                }
            }
        }
    }
    v
}

/// 文字列の中の `{…}`(と `{{…}}`)を探して、名前を順に返します。
fn names(s: &str) -> Vec<(usize, usize, String)> {
    let mut v = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'{' {
            // **本家の書き方 `{member}` を正とします**(2026-08-18 発注者
            // 「AsciiDoc とは何かを考えていけば理解できてくるのでは」)。
            // AsciiDoc は属性の参照を `{member}` と書きます。うちの差し込みは
            // `{{member}}` という別の書き方を作っていたので、本家に寄せました。
            // **`{{member}}` も今までどおり受けます** — 手引きと見本が
            // その書き方で出ているためです
            let double = b.get(i + 1) == Some(&b'{');
            let head = if double { i + 2 } else { i + 1 };
            let closing = if double { "}}" } else { "}" };
            if let Some(rel) = s[head..].find(closing) {
                let name = s[head..head + rel].trim().to_string();
                // 名前に空白や改行が混ざる物は差し込みの穴ではない
                // (普通の文の中括弧を巻き込まないため)
                if !name.is_empty() && !name.contains(char::is_whitespace) {
                    let tail = head + rel + closing.len();
                    v.push((i, tail, name));
                    i = tail;
                    continue;
                }
            }
        }
        i += 1;
    }
    v
}

/// その段落が名指している群(`{群.項目}` の群)。複数あれば最初のもの。
fn group_of(p: &Paragraph) -> Option<String> {
    for r in &p.runs {
        for (_, _, n) in names(&r.text) {
            if let Some((g, _)) = n.split_once('.') {
                return Some(g.to_string());
            }
        }
    }
    None
}

/// run の字を、辞書で置き換えます。無い名前はそのまま残して報告します。
fn subst(text: &str, look: &dyn Fn(&str) -> Option<String>, unknown: &mut Vec<String>) -> String {
    let ns = names(text);
    if ns.is_empty() {
        return text.to_string();
    }
    let mut o = String::new();
    let mut at = 0usize;
    for (s, e, name) in ns {
        o.push_str(&text[at..s]);
        match look(&name) {
            Some(v) => o.push_str(&v),
            None => {
                if !unknown.contains(&name) {
                    unknown.push(name.clone());
                }
                o.push_str(&text[s..e]); // そのまま残す
            }
        }
        at = e;
    }
    o.push_str(&text[at..]);
    o
}

fn fill_runs(
    runs: &[Run],
    look: &dyn Fn(&str) -> Option<String>,
    unknown: &mut Vec<String>,
) -> Vec<Run> {
    runs.iter()
        .map(|r| Run { text: subst(&r.text, look, unknown), ..r.clone() })
        .collect()
}

/// 表の行を、群のデータの数だけ増やします。
fn fill_table(t: &Table, d: &Data, rep: &mut Report) -> Table {
    let mut out = t.clone();
    out.rows.clear();
    for row in &t.rows {
        // この行が名指している群(セルの中の段落を見ます)
        let g = row
            .iter()
            .flat_map(|c| c.paragraphs.iter())
            .find_map(group_of);
        let Some(g) = g else {
            // 普通の行。1つだけの値だけ差し込みます
            let look = |n: &str| d.values.get(n).cloned();
            out.rows.push(
                row.iter()
                    .map(|c| {
                        let mut c2 = c.clone();
                        for p in &mut c2.paragraphs {
                            p.runs = fill_runs(&p.runs, &look, &mut rep.unknown);
                        }
                        c2
                    })
                    .collect(),
            );
            continue;
        };
        let data = d.rows.get(&g).cloned().unwrap_or_default();
        if data.is_empty() && !d.rows.contains_key(&g) && !rep.unknown.contains(&g) {
            rep.unknown.push(g.clone());
        }
        rep.expanded.insert(g.clone(), data.len());
        for one in &data {
            let look = |n: &str| match n.split_once('.') {
                Some((gg, item)) if gg == g => one.get(item).cloned(),
                _ => d.values.get(n).cloned(),
            };
            out.rows.push(
                row.iter()
                    .map(|c| {
                        let mut c2 = c.clone();
                        for p in &mut c2.paragraphs {
                            p.runs = fill_runs(&p.runs, &look, &mut rep.unknown);
                        }
                        c2
                    })
                    .collect(),
            );
        }
    }
    out
}

/// Template + data → the merged document and a report.
///
/// **The original is not touched.** A copy is returned, so the template can be used
/// any number of times.
pub fn fill(doc: &Document, d: &Data) -> (Document, Report) {
    let mut out = doc.clone();
    let mut rep = Report::default();
    let look = |n: &str| d.values.get(n).cloned();
    for b in &mut out.blocks {
        match b {
            Block::Para(p) => p.runs = fill_runs(&p.runs, &look, &mut rep.unknown),
            Block::Table(t) => *t = fill_table(t, d, &mut rep),
        }
    }
    (out, rep)
}

/// Where a stretch of a filled document form stands: the paragraph (its
/// place in the blocks, and for a table the row, cell and paragraph in the
/// cell) and the bytes in the paragraph's text.
#[derive(Debug, Clone, PartialEq)]
pub struct DocAt {
    pub block: usize,
    /// (row, cell, paragraph) inside a table block
    pub cell: Option<(usize, usize, usize)>,
    pub from: usize,
    pub to: usize,
}

/// A document form filled from a data book ([`fill_form`]).
#[derive(Debug, Clone)]
pub struct FormFill {
    pub doc: Document,
    /// The names the data lacks altogether
    pub missing: Vec<String>,
    /// Each mark and where its answer went
    pub marks: Vec<(String, DocAt)>,
    /// Where the chosen options of choice marks went, to be circled
    pub chosen: Vec<DocAt>,
}

/// A mark cut by the run boundaries (Word splits typed text into runs) is
/// joined into the run it starts in, so each run holds whole marks.
fn join_marks(runs: &mut Vec<Run>) {
    let unclosed = |t: &str| t.rfind('{').is_some_and(|a| !t[a..].contains('}'));
    let mut emptied = vec![false; runs.len()];
    let mut i = 0;
    while i < runs.len() {
        if unclosed(&runs[i].text) {
            let mut j = i + 1;
            while j < runs.len() {
                let t = std::mem::take(&mut runs[j].text);
                match t.find('}') {
                    Some(k) => {
                        runs[i].text.push_str(&t[..=k]);
                        runs[j].text = t[k + 1..].to_string();
                        emptied[j] = runs[j].text.is_empty();
                        break;
                    }
                    None => {
                        runs[i].text.push_str(&t);
                        emptied[j] = true;
                        j += 1;
                    }
                }
            }
            if unclosed(&runs[i].text) && j >= runs.len() {
                break;
            }
            // the joined run may end in another mark
            continue;
        }
        i += 1;
    }
    let mut k = 0;
    runs.retain(|_| {
        let keep = !emptied[k];
        k += 1;
        keep
    });
}

/// The width of a text in half-width units: a full-width char is 2
fn width(t: &str) -> usize {
    t.chars().map(|c| if c.is_ascii() || ('\u{FF61}'..='\u{FF9F}').contains(&c) { 1 } else { 2 }).sum()
}

/// **Takes `need` half-width units of spaces** from the blank after byte
/// `at` of run `ri`: the spaces there, going on into the runs after while
/// they are underlined too. A space taken is overwritten with NUL bytes
/// of its own length, so byte places stay put until they are counted again.
fn take_spaces(runs: &mut [Run], ri: usize, at: usize, need: usize) {
    let mut left = need;
    let mut ri = ri;
    let mut at = at;
    while left > 0 && ri < runs.len() {
        let t = &mut runs[ri].text;
        let mut pos = at;
        while left > 0 && pos < t.len() {
            let c = t[pos..].chars().next().expect("a char");
            if c == '\0' {
                pos += 1;
                continue;
            }
            if c != ' ' && c != '\u{3000}' {
                return;
            }
            let n = c.len_utf8();
            t.replace_range(pos..pos + n, &"\0".repeat(n));
            left = left.saturating_sub(if c == ' ' { 1 } else { 2 });
            pos += n;
        }
        // on into the next run only if it goes on with the underline
        ri += 1;
        at = 0;
        if ri < runs.len() && !runs[ri].fmt.underline {
            return;
        }
    }
}

/// **A document form filled from a data book**, with the marks of a sheet
/// form ([`book::form`]): `{氏名}`, `{日付.和暦年}`, `{送達場所=住所}`,
/// `{性別:男・女}` and the rest. Unlike [`fill`], an unanswered mark
/// becomes empty (a form's blanks stay blank); the names the data lacks
/// altogether are returned for the caller to report, with where the
/// chosen options of choice marks stand.
pub fn fill_form(doc: &Document, data: &book::Book) -> FormFill {
    let answers = book::form::Data::new(data);
    let mut out = doc.clone();
    let mut names: Vec<String> = Vec::new();
    let mut marks: Vec<(String, DocAt)> = Vec::new();
    let mut chosen: Vec<DocAt> = Vec::new();
    let mut para = |p: &mut Paragraph, block: usize, cell: Option<(usize, usize, usize)>| {
        join_marks(&mut p.runs);
        // 1. Answer the marks, keeping where each answer went in its run
        let mut found_marks: Vec<(String, usize, usize, usize)> = Vec::new();
        let mut found_chosen: Vec<(usize, usize, usize)> = Vec::new();
        for (ri, r) in p.runs.iter_mut().enumerate() {
            let found = book::form::marks(&r.text);
            if found.is_empty() {
                continue;
            }
            names.extend(found);
            let f = book::form::fill_text_ranges(&r.text, &answers);
            found_marks.extend(f.marks.iter().map(|(m, a, b)| (m.clone(), ri, *a, *b)));
            found_chosen.extend(f.chosen.iter().map(|(a, b)| (ri, *a, *b)));
            r.text = f.text;
        }
        // 2. A blank of underlined spaces keeps its length
        for (_, ri, a, b) in found_marks.iter().rev() {
            if p.runs[*ri].fmt.underline {
                let need = width(&p.runs[*ri].text[*a..*b]);
                take_spaces(&mut p.runs, *ri, *b, need);
            }
        }
        // 3. Count the places again without the spaces taken
        let gone = |t: &str, upto: usize| t[..upto].bytes().filter(|b| *b == 0).count();
        let starts: Vec<usize> = p
            .runs
            .iter()
            .scan(0usize, |acc, r| {
                let s = *acc;
                *acc += r.text.len() - gone(&r.text, r.text.len());
                Some(s)
            })
            .collect();
        let at = |ri: usize, a: usize, b: usize| {
            let t = &p.runs[ri].text;
            DocAt { block, cell, from: starts[ri] + a - gone(t, a), to: starts[ri] + b - gone(t, b) }
        };
        marks.extend(found_marks.iter().map(|(m, ri, a, b)| (m.clone(), at(*ri, *a, *b))));
        chosen.extend(found_chosen.iter().map(|(ri, a, b)| at(*ri, *a, *b)));
        for r in &mut p.runs {
            if r.text.contains('\0') {
                r.text.retain(|c| c != '\0');
            }
        }
    };
    for (bi, b) in out.blocks.iter_mut().enumerate() {
        match b {
            Block::Para(p) => para(p, bi, None),
            Block::Table(t) => {
                for (ri, row) in t.rows.iter_mut().enumerate() {
                    for (ci, c) in row.iter_mut().enumerate() {
                        for (pi, p) in c.paragraphs.iter_mut().enumerate() {
                            para(p, bi, Some((ri, ci, pi)));
                        }
                    }
                }
            }
        }
    }
    // Text boxes carry their text as one string
    for sh in &mut out.shapes {
        if let Some(t) = &sh.look.text {
            let found = book::form::marks(t);
            if !found.is_empty() {
                names.extend(found);
                sh.look.text = Some(book::form::fill_text(t, &answers, &mut Vec::new()));
            }
        }
    }
    let missing = book::form::missing_of(names, data);
    FormFill { doc: out, missing, marks, chosen }
}

#[cfg(test)]
mod form_tests {
    use super::*;

    fn run(t: &str) -> Run {
        Run { text: t.into(), size_pt: None, font: None, fmt: Default::default() }
    }

    #[test]
    fn an_underlined_blank_keeps_its_length() {
        let (data, _) = crate::book_adoc::parse("= データ\n\n.基本\n|===\n|氏名 |山田 太郎\n|===\n").unwrap();
        let mut under = run("");
        under.fmt.underline = true;
        let blank = |t: &str| Run { text: t.into(), ..under.clone() };
        let mut doc = Document::default();
        let mut p = Paragraph::default();
        // 氏名 then an underlined blank of 8 full-width spaces cut in two runs
        p.runs = vec![run("氏名"), blank("{氏名}　　　　"), blank("　　　　"), run("印")];
        doc.blocks.push(Block::Para(p));
        let f = fill_form(&doc, &data);
        let Block::Para(p) = &f.doc.blocks[0] else { panic!() };
        let blanks: String = p.runs.iter().filter(|r| r.fmt.underline).map(|r| r.text.as_str()).collect();
        // 山田 太郎 is 9 half-width units: 5 full-width spaces (10) go
        assert_eq!(blanks, "山田 太郎　　　");
        assert_eq!(super::width(&blanks), 15, "about the blank's 16 units");
        let text: String = p.runs.iter().map(|r| r.text.as_str()).collect();
        let (_, at) = &f.marks[0];
        assert_eq!(&text[at.from..at.to], "山田 太郎");
        assert!(text.ends_with("印"));
        // not underlined: the spaces stay
        let mut doc = Document::default();
        let mut p = Paragraph::default();
        p.runs = vec![run("{氏名}　　様")];
        doc.blocks.push(Block::Para(p));
        let f = fill_form(&doc, &data);
        let Block::Para(p) = &f.doc.blocks[0] else { panic!() };
        assert_eq!(p.runs[0].text, "山田 太郎　　様");
    }

    #[test]
    fn a_document_form_is_filled_from_a_data_book() {
        let (data, _) = crate::book_adoc::parse(
            "= データ\n\n.基本\n|===\n|届出日 |2026-09-25\n|氏名 |山田 太郎\n|送達場所 |住所\n|===\n",
        )
        .unwrap();
        let mut doc = Document::default();
        let mut p = Paragraph::default();
        // the date's mark is cut across runs, as Word splits typed text
        p.runs = vec![run("令和{届出日.和"), run("暦年}年{届出日.月}月　氏名 {氏名}")];
        doc.blocks.push(Block::Para(p));
        let mut p = Paragraph::default();
        p.runs = vec![run("{送達場所=住所}住所 {送達場所=勤務先}勤務先 {電話}")];
        doc.blocks.push(Block::Para(p));
        let f = fill_form(&doc, &data);
        let (out, missing) = (&f.doc, f.missing.clone());
        let text = |i: usize| match &out.blocks[i] {
            Block::Para(p) => p.runs.iter().map(|r| r.text.as_str()).collect::<String>(),
            _ => String::new(),
        };
        assert_eq!(text(0), "令和8年9月　氏名 山田 太郎");
        assert_eq!(text(1), "■住所 □勤務先 ");
        assert_eq!(missing, ["電話"]);
        // 山田 太郎 went to bytes 37.. of the first paragraph
        let (m, at) = &f.marks[2];
        assert_eq!(m, "氏名");
        assert_eq!(&text(0)[at.from..at.to], "山田 太郎");
        assert_eq!(f.chosen.len(), 0);
    }
}
