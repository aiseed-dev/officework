//! **差し込み** — 雛形にデータを流し込みます(帳票の芯)。
//!
//! 発注者 2026-08-17「帳票ビルダー」。請求書や納品書は、頭と足は決まっていて
//! **明細の行数だけがデータで決まります**。そこを埋めるのがここです。
//!
//! ## 書き方
//!
//! ```text
//! 請求先: {{宛名}} 様
//!
//! |===
//! | 品名 | 数量 | 金額
//! | {{明細.品名}} | {{明細.数量}} | {{明細.金額}}
//! |===
//!
//! 合計 {{合計}} 円
//! ```
//!
//! `{{名前}}` はそのまま置き換えます。`{{群.項目}}` を含む**表の行は、その群の
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
//! データに無い名前は `{{名前}}` のまま残し、[`Report`] に挙げます。空にすると
//! 「金額が空欄の請求書」が黙って出来上がります。

use crate::doc::{Block, Document, Paragraph, Run, Table};
use std::collections::BTreeMap;

/// 流し込むデータ。
#[derive(Debug, Clone, Default)]
pub struct Data {
    /// 1つだけの値(`{{宛名}}`)
    pub values: BTreeMap<String, String>,
    /// 繰り返す値(`{{明細.品名}}`)。群の名前 → 行の並び
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

/// 文字列の中の `{{…}}` を探して、名前を順に返します。
fn names(s: &str) -> Vec<(usize, usize, String)> {
    let mut v = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'{' && b[i + 1] == b'{' {
            if let Some(rel) = s[i + 2..].find("}}") {
                let name = s[i + 2..i + 2 + rel].trim().to_string();
                v.push((i, i + 2 + rel + 2, name));
                i = i + 2 + rel + 2;
                continue;
            }
        }
        i += 1;
    }
    v
}

/// その段落が名指している群(`{{群.項目}}` の群)。複数あれば最初のもの。
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
