//! **差し込み** — 雛形にデータを流し込みます(帳票の芯)。
//!
//! 発注者 2026-08-17「帳票ビルダー」。請求書や納品書は、頭と足は決まっていて
//! **明細の行数だけがデータで決まります**。そこを埋めるのがここです。
//!
//! ## 書き方
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
//! `{member}` はそのまま置き換えます(AsciiDoc の属性の参照と同じ書き方です。
//! 前からの `{{member}}` も受けます)。`{群.項目}` を含む**表の行は、その群の
//! データの数だけ増えます**。増やす印を別に書かせないのは、書く人が覚える
//! ことを増やさないためです。
//!
//! ## 出力形式ごとに作らない
//!
//! 差し込みは**文書の模型の上**で行い、出来上がった文書を PDF にも HTML にも
//! docx にもします。形式ごとに差し込みを書くと、同じ雛形が形式によって違う
//! 結果になります。
//!
//! ## 分からない名前は黙って空にしない
//!
//! データに無い名前は書いたまま残し、[`Report`] に挙げます。空にすると
//! 「金額が空欄の請求書」が黙って出来上がります。

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
                for c in t.rows.iter().flat_map(|r| r.iter()) {
                    for p in &c.paragraphs {
                        see(p);
                    }
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
        let データ = d.rows.get(&g).cloned().unwrap_or_default();
        if データ.is_empty() && !d.rows.contains_key(&g) && !rep.unknown.contains(&g) {
            rep.unknown.push(g.clone());
        }
        rep.expanded.insert(g.clone(), データ.len());
        for one in &データ {
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

/// 雛形 + データ → 差し込み済みの文書と報告。
///
/// **原本は触りません。** 写しを返すので、雛形は何度でも使えます。
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
