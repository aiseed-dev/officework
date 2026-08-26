//! **蒸留** — 互換の文書(docx 由来。直接書式の泥)から、意味だけの文書と
//! テンプレートを取り出す(段階D)。
//!
//! SEKKEI「本文とテンプレートを分ける」(2026-08-16 発注者)の最後の便。
//! 受け取った docx は直接書式そのものなので、意味だけの模型に載せるには
//! **蒸留(非可逆)しかない**。だから黙ってやらない — 明示の1手で呼ぶ。
//!
//! # やり方
//!
//! 1. 段落ごとに**見た目の鍵**([`Look`])を作る(大きさ・書体・太字・色・
//!    揃え・空き・行間)
//! 2. 同じ鍵の段落を1つのスタイルにまとめる。**いちばん多い鍵が「本文」**で、
//!    文書の既定になる
//! 3. 役割(見出し1〜3・引用)を持つ段落は、その名前を先に取る
//! 4. 本文からは見た目を落とし、`style_id` で名前を指す
//!
//! # 落ちる物は数える
//!
//! 段落の中で1つの run だけ色や大きさが違う、といった**段落の鍵に収まらない
//! 見た目**は落ちる。強調(太字・斜体)・上付き・下付き・リンク・ルビ・
//! 脚注・参照は**意味なので残る**。落とした数は [`Report`] で返す —
//! 「何も失っていない」と嘘をつかないため。

use std::collections::HashMap;

use crate::doc::{Align, Block, Document, Paragraph, ParaStyle};
use crate::theme::{StyleDef, Theme};

/// 段落1つの**見た目の鍵**。同じ鍵の段落は同じスタイルになる。
/// 大きさは pt×100 の整数で持つ(f32 は Eq にならない)
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
struct Look {
    size_c: Option<u32>,
    font: Option<String>,
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<String>,
    shade: Option<String>,
    align: Align,
    space_before_c: u32,
    space_after_c: u32,
    line_c: u32,
    /// 1行目の字下げ(twips)。原文の値をそのまま鍵にする
    first_twips: i32,
}

/// 数えるときの鍵。(大きさ, 書体, 太字, 斜体, 下線, スタイル)
/// — 段落の中でいちばん多い見た目を選ぶのに使います
type look_key = (Option<u32>, Option<String>, bool, bool, bool, Option<String>);

impl Look {
    /// 段落の見た目を読む。run は**いちばん多く使われている姿**を採る
    /// (先頭の run だと、頭に1字だけ違う書式があるときに引きずられる)
    fn of(p: &Paragraph) -> Look {
        let mut tally: HashMap<look_key, usize> = HashMap::new();
        for r in &p.runs {
            if r.text.is_empty() {
                continue; // 印だけの run(脚注)は見た目を持たない
            }
            let key = (
                r.size_pt.map(|s| (s * 100.0).round() as u32),
                r.font.clone(),
                r.fmt.bold,
                r.fmt.italic,
                r.fmt.underline,
                r.fmt.color.clone(),
            );
            *tally.entry(key).or_default() += r.text.chars().count();
        }
        let (size_c, font, bold, italic, underline, color) = tally
            .into_iter()
            .max_by_key(|(_, n)| *n)
            .map(|(k, _)| k)
            .unwrap_or((None, None, false, false, false, None));
        Look {
            size_c,
            font,
            bold,
            italic,
            underline,
            color,
            shade: p.shade.clone(),
            align: p.align,
            space_before_c: (p.space_before_pt * 100.0).round() as u32,
            space_after_c: (p.space_after_pt * 100.0).round() as u32,
            line_c: (p.line_spacing * 100.0).round() as u32,
            first_twips: p.first_line_twips,
        }
    }

    /// スタイルの定義へ。**本文との差だけ**を書く(base は文書の既定)
    fn to_style(&self, name: String, base: &Look) -> StyleDef {
        StyleDef {
            name,
            size_pt: (self.size_c != base.size_c).then_some(self.size_c).flatten().map(|c| c as f32 / 100.0),
            font: (self.font != base.font).then(|| self.font.clone()).flatten(),
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            color: self.color.clone(),
            shade: self.shade.clone(),
            align: (self.align != Align::Left).then_some(self.align),
            space_before_pt: self.space_before_c as f32 / 100.0,
            space_after_pt: self.space_after_c as f32 / 100.0,
            line_spacing: (self.line_c != 100).then_some(self.line_c as f32 / 100.0),
            // twips → 全角の文字数(その段落の字の大きさで割る)
            first_line_chars: (self.first_twips > 0).then(|| {
                let pt = self.size_c.or(base.size_c).unwrap_or(1050) as f32 / 100.0;
                (self.first_twips as f32 / 20.0 / pt * 10.0).round() / 10.0
            }),
        }
    }
}

/// ヘッダー・フッターの段落の並び → 1行の字(テンプレートに書く形)。
/// ページ番号の印は `{ページ}` `{ページ数}` に書き換える
fn single_line(hf: &crate::doc::HeadFoot) -> Option<String> {
    let s: String = hf
        .paragraphs
        .iter()
        .flat_map(|p| p.runs.iter())
        .map(|r| r.text.as_str())
        .collect::<Vec<_>>()
        .join("")
        .replace(crate::doc::PAGE_MARK, "{ページ}")
        .replace(crate::doc::PAGES_MARK, "{ページ数}");
    (!s.trim().is_empty()).then_some(s)
}

/// 蒸留の結果の報告。**落ちた物を数える** — 何も失っていないと嘘をつかない
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    /// 作ったスタイルの数
    pub styles: usize,
    /// 名前を付けた段落の数
    pub paragraphs: usize,
    /// **段落の見た目に収まらず落ちた run の数**(1字だけ色が違う、など)。
    /// 強調・上付き・下付き・リンク・ルビ・脚注・参照は意味なので数えない
    pub dropped: usize,
}

/// 互換の文書 → (意味だけの文書, テンプレート, 報告)。
///
/// 元の文書は触らない。返る文書は**見た目の欄が空**で、段落が
/// `style_id` でスタイルを指す。
pub fn distill(doc: &Document) -> (Document, Theme, Report) {
    // 1) 段落ごとの見た目を集め、いちばん多い物を本文にする
    let looks: Vec<Look> = doc.paragraphs().map(Look::of).collect();
    let mut count: HashMap<&Look, usize> = HashMap::new();
    for l in &looks {
        *count.entry(l).or_default() += 1;
    }
    let base = count
        .into_iter()
        .max_by_key(|(l, n)| (*n, std::cmp::Reverse((*l).clone().size_c)))
        .map(|(l, _)| l.clone())
        .unwrap_or_default();

    // 2) 見た目 → 名前。役割のある段落が固定名を先に取る
    let mut name_of: HashMap<Look, String> = HashMap::new();
    let mut used: Vec<String> = Vec::new();
    let mut fresh = 0usize;
    for (p, look) in doc.paragraphs().zip(&looks) {
        if name_of.contains_key(look) {
            continue;
        }
        let want = if *look == base {
            "本文".to_string()
        } else {
            match Theme::role_name(p.style) {
                Some(n) if p.style != ParaStyle::Body => n.to_string(),
                _ => {
                    fresh += 1;
                    format!("見た目{fresh}")
                }
            }
        };
        // 同じ名前を2つの見た目が欲しがったら、後から来た方に番号を足す
        let mut name = want.clone();
        let mut k = 2;
        while used.contains(&name) {
            name = format!("{want}の{k}");
            k += 1;
        }
        used.push(name.clone());
        name_of.insert(look.clone(), name);
    }

    // 3) テンプレートを組む
    let mut th = Theme {
        // 蒸留の元は紙の文書なので、組み方は紙のまま
        setting: Default::default(),
        font: base.font.clone(),
        size_pt: base.size_c.map(|c| c as f32 / 100.0),
        // 言語ごとの分は起こしません。**その docx が実際に使っていた
        // 書体と大きさ**が上に書いてあるので、言語で選び直すと元の紙面と
        // 変わってしまいます
        lang_docs: Vec::new(),
        page: doc.page,
        styles: Vec::new(),
        // 様式は docx から起こしません(枠は人が決める物です)
        forms: Vec::new(),
        submit: None,
        // **ページの飾りもテンプレートへ移す**(2026-08-18)。docx を分けた
        // ときに、透かしやページの色が消えないようにする。ヘッダーと
        // フッターは段落の並びなので、字だけを取って1行にする
        header: single_line(&doc.header),
        footer: single_line(&doc.footer),
        watermark: doc.watermark.clone(),
        page_color: doc.page_color.clone(),
        vertical: doc.vertical,
    };
    // 並びは文書に出てくる順(読む人が追える)
    let mut seen: Vec<&Look> = Vec::new();
    for l in &looks {
        if !seen.contains(&l) {
            seen.push(l);
        }
    }
    for l in seen {
        let name = name_of[l].clone();
        if *l == base {
            // 本文は文書の既定そのもの。空の定義は置かない
            continue;
        }
        th.styles.push(l.to_style(name, &base));
    }

    // 4) 本文から見た目を落とす
    let mut out = doc.clone();
    let mut rep = Report { styles: th.styles.len(), ..Default::default() };
    let mut li = 0usize;
    for b in &mut out.blocks {
        let Block::Para(p) = b else { continue };
        let look = &looks[li];
        li += 1;
        let name = name_of[look].clone();
        // 役割で出る名前(見出し1 など)は style_id を持たせない —
        // 役割そのものが指すので、二重に名乗らない
        let by_role = Theme::role_name(p.style) == Some(name.as_str());
        p.style_id = (!by_role && name != "本文").then_some(name);
        p.shade = None;
        p.space_before_pt = 0.0;
        p.space_after_pt = 0.0;
        p.line_spacing = 1.0;
        p.align = Align::Left;
        rep.paragraphs += 1;
        for r in &mut p.runs {
            if r.text.is_empty() {
                continue;
            }
            let traits = r.size_pt.map(|s| (s * 100.0).round() as u32) != look.size_c
                || r.font != look.font
                || r.fmt.color != look.color
                || r.fmt.highlight.is_some();
            if traits {
                rep.dropped += 1;
            }
            r.size_pt = None;
            r.font = None;
            r.fmt.color = None;
            r.fmt.highlight = None;
            // **強調は意味なので残す** — ただしスタイルが着ている分は落とす
            // (見出しの太字を run にも残すと、二重に太くなる)
            if look.bold {
                r.fmt.bold = false;
            }
            if look.italic {
                r.fmt.italic = false;
            }
            if look.underline {
                r.fmt.underline = false;
            }
        }
    }
    // ページはテンプレートが持つ(本文からは外す)
    out.page = None;
    out.font = None;
    out.size_pt = None;
    (out, th, rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{CharFormat, Run};

    fn para(text: &str, pt: Option<f32>, bold: bool, style: ParaStyle) -> Block {
        let fmt = CharFormat { bold, ..Default::default() };
        Block::Para(Paragraph {
            style,
            runs: vec![Run { text: text.into(), size_pt: pt, font: None, fmt }],
            ..Default::default()
        })
    }

    #[test]
    fn the_most_common_look_becomes_the_body_style() {
        let d = Document { blocks: vec![
            para("題", Some(16.0), true, ParaStyle::Heading(1)),
            para("ふつうの段落。", Some(10.5), false, ParaStyle::Body),
            para("もうひとつ。", Some(10.5), false, ParaStyle::Body),
            para("みっつめ。", Some(10.5), false, ParaStyle::Body),
        ], ..Default::default() };
        let (out, th, rep) = distill(&d);
        assert_eq!(th.size_pt, Some(10.5), "本文の大きさが文書の既定にならない");
        assert_eq!(rep.styles, 1, "見出しの1つだけがスタイルになる");
        assert_eq!(th.styles[0].name, "見出し1");
        assert_eq!(th.styles[0].size_pt, Some(16.0));
        assert!(th.styles[0].bold);
        // 本文からは見た目が落ちている
        for p in out.paragraphs() {
            for r in &p.runs {
                assert_eq!(r.size_pt, None, "大きさが残った");
                assert!(!r.fmt.bold, "スタイルが着ている太字が run に残った");
            }
        }
    }

    #[test]
    fn a_look_without_a_role_gets_a_made_up_name() {
        let d = Document { blocks: vec![
            para("ふつう。", Some(10.5), false, ParaStyle::Body),
            para("ふつう2。", Some(10.5), false, ParaStyle::Body),
            para("なぜか大きい。", Some(14.0), false, ParaStyle::Body),
        ], ..Default::default() };
        let (out, th, _) = distill(&d);
        assert_eq!(th.styles.len(), 1);
        assert_eq!(th.styles[0].name, "見た目1");
        let ps: Vec<&Paragraph> = out.paragraphs().collect();
        assert_eq!(ps[2].style_id.as_deref(), Some("見た目1"), "段落が名前を指さない");
        assert_eq!(ps[0].style_id, None, "本文は名指ししない");
    }

    #[test]
    fn emphasis_is_meaning_so_it_stays() {
        let mut d = Document::default();
        let plain = CharFormat::default();
        let strong = CharFormat { bold: true, ..Default::default() };
        let p = Paragraph {
            runs: vec![
                Run { text: "ここは".into(), size_pt: Some(10.5), font: None, fmt: plain.clone() },
                Run { text: "強い".into(), size_pt: Some(10.5), font: None, fmt: strong },
                Run { text: "です。".into(), size_pt: Some(10.5), font: None, fmt: plain },
            ],
            ..Default::default()
        };
        d.blocks = vec![Block::Para(p), para("ふつう。", Some(10.5), false, ParaStyle::Body)];
        let (out, _, _) = distill(&d);
        let ps: Vec<&Paragraph> = out.paragraphs().collect();
        assert!(ps[0].runs[1].fmt.bold, "文中の強調が消えた");
        assert!(!ps[0].runs[0].fmt.bold);
    }

    #[test]
    fn what_was_dropped_is_counted() {
        // 段落の見た目は 10.5pt。1つの run だけ 20pt = 段落の鍵に収まらない
        let mut d = Document::default();
        let p = Paragraph {
            runs: vec![
                Run { text: "ふつうの長い文。".into(), size_pt: Some(10.5), font: None, fmt: CharFormat::default() },
                Run { text: "大".into(), size_pt: Some(20.0), font: None, fmt: CharFormat::default() },
            ],
            ..Default::default()
        };
        d.blocks = vec![Block::Para(p)];
        let (_, _, rep) = distill(&d);
        assert_eq!(rep.dropped, 1, "落ちた run を数えていない");
    }

    #[test]
    fn distilled_output_composes_back_to_the_original_look() {
        // **蒸留 → 合成が恒等に近いこと**(門番)。見出しの 16pt 太字が、
        // テンプレート経由で戻る
        let d = Document { blocks: vec![
            para("題", Some(16.0), true, ParaStyle::Heading(1)),
            para("ふつう。", Some(10.5), false, ParaStyle::Body),
            para("ふつう2。", Some(10.5), false, ParaStyle::Body),
        ], ..Default::default() };
        let (out, th, _) = distill(&d);
        let back = crate::theme::compose(&out, &th);
        let ps: Vec<&Paragraph> = back.paragraphs().collect();
        assert_eq!(ps[0].runs[0].size_pt, Some(16.0), "見出しの大きさが戻らない");
        assert!(ps[0].runs[0].fmt.bold, "見出しの太字が戻らない");
        assert_eq!(back.size_pt, Some(10.5), "本文の既定が戻らない");
    }
}
