//! **文書をブロックの番号で読み書きする。**
//!
//! エージェント・Python・MCP の3つの入り口が同じ言葉で文書を触るための
//! 語彙です(docs/sekkei/agent.ja.adoc「writer にも同じパネル」。2026-09-04)。
//! 本文の丸ごとではなく、`Document.blocks` の添字で範囲を指し、中身は
//! AsciiDoc の字で受け渡します。長い文書でも、触る所だけ読めば済みます。
//!
//! 読んだ時の字には**照合の字**(短いハッシュ)を付けます。書き替えの時に
//! それを添えれば、読んだ後に文書が変わっていた時に断れます(表のセルの
//! 控えと同じ考え)。

use crate::{adoc, Block, Document, ParaStyle};
use std::hash::{DefaultHasher, Hash, Hasher};

/// 見出しの1行(文書の地図)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// ブロックの番号(`Document.blocks` の添字)
    pub index: usize,
    /// 題は 0、`== 見出し` は 1、`=== ` は 2 …
    pub level: u8,
    pub text: String,
}

/// 読んだブロック1つ
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    pub index: usize,
    /// 照合の字(8桁の16進)。書き替えの時に添える
    pub stamp: String,
    /// そのブロックの AsciiDoc の字(末尾は改行1つ)
    pub adoc: String,
}

/// 見出しの一覧。長い文書はまずこれで地図を見る
pub fn outline(doc: &Document) -> Vec<Heading> {
    doc.blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| match b {
            Block::Para(p) => {
                let level = match p.style {
                    ParaStyle::Title => 0,
                    ParaStyle::Heading(n) => n,
                    _ => return None,
                };
                Some(Heading { index: i, level, text: para_text(p) })
            }
            Block::Table(_) => None,
        })
        .collect()
}

/// ブロックの字(印なし)。段落は run の字、表はセルの字を空白と改行で繋ぐ
pub fn text_of(block: &Block) -> String {
    match block {
        Block::Para(p) => para_text(p),
        Block::Table(t) => t
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|c| c.paragraphs.iter().map(para_text).collect::<Vec<_>>().join(" "))
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn para_text(p: &crate::Paragraph) -> String {
    p.runs.iter().map(|r| r.text.as_str()).collect()
}

/// 範囲(両端を含む)のブロックを AsciiDoc の字で書く。頭(題名や属性)は書かない
pub fn write_range(doc: &Document, from: usize, to: usize) -> Result<String, String> {
    check_range(doc, from, to)?;
    // 頭を空にした写しに、その範囲だけを入れて書く。書き方は文書全体と同じ
    // 関数なので、表の控えや節の用紙もそのまま効く
    let mut part = doc.clone();
    part.blocks = doc.blocks[from..=to].to_vec();
    part.attrs.clear();
    part.template = None;
    part.props.title.clear();
    Ok(adoc::write(&part))
}

/// 範囲(両端を含む)のブロックを1つずつ読む。番号と照合の字つき
pub fn read(doc: &Document, from: usize, to: usize) -> Result<Vec<Read>, String> {
    check_range(doc, from, to)?;
    (from..=to)
        .map(|i| {
            let adoc = write_range(doc, i, i)?;
            Ok(Read { index: i, stamp: stamp(&adoc), adoc })
        })
        .collect()
}

/// 照合の字。同じ字なら同じ8桁
pub fn stamp(adoc: &str) -> String {
    let mut h = DefaultHasher::new();
    adoc.hash(&mut h);
    format!("{:08x}", h.finish() as u32)
}

/// AsciiDoc の断片をブロックの並びに読む。頭の属性(`:名前: 値`)は断片には
/// 書けない(文書の頭に属する物なので)
pub fn parse_fragment(src: &str) -> Result<Vec<Block>, String> {
    let (d, _notes) = adoc::parse_full(src)?;
    if !d.attrs.is_empty() {
        return Err("断片に属性の行(:名前: 値)は書けません。文書の頭の物です".into());
    }
    Ok(d.blocks)
}

/// 範囲(両端を含む)を断片で書き替える。`expect` に読んだ時の照合の字を渡すと、
/// その間に文書が変わっていたら断る。返りは入れたブロックの数
pub fn replace(
    doc: &mut Document,
    from: usize,
    to: usize,
    src: &str,
    expect: Option<&[String]>,
) -> Result<usize, String> {
    check_range(doc, from, to)?;
    check_stamps(doc, from, to, expect)?;
    let new = parse_fragment(src)?;
    let n = new.len();
    doc.blocks.splice(from..=to, new);
    Ok(n)
}

/// `at` の前に断片を差し込む。`at` がブロックの数と同じなら末尾に足す
pub fn insert(doc: &mut Document, at: usize, src: &str) -> Result<usize, String> {
    if at > doc.blocks.len() {
        return Err(format!("at={at} は範囲の外です(ブロックは {} 個。末尾に足すなら {})", doc.blocks.len(), doc.blocks.len()));
    }
    let new = parse_fragment(src)?;
    let n = new.len();
    doc.blocks.splice(at..at, new);
    Ok(n)
}

/// 範囲(両端を含む)を消す。返りは消した数
pub fn delete(doc: &mut Document, from: usize, to: usize, expect: Option<&[String]>) -> Result<usize, String> {
    check_range(doc, from, to)?;
    check_stamps(doc, from, to, expect)?;
    doc.blocks.drain(from..=to);
    Ok(to - from + 1)
}

/// 字を含むブロックを探す。返りは番号と、その字の周り(前後 20 字)
pub fn find(doc: &Document, needle: &str) -> Vec<(usize, String)> {
    if needle.is_empty() {
        return Vec::new();
    }
    doc.blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let t = text_of(b);
            let at = t.find(needle)?;
            let chars: Vec<char> = t.chars().collect();
            let pos = t[..at].chars().count();
            let s = pos.saturating_sub(20);
            let e = (pos + needle.chars().count() + 20).min(chars.len());
            Some((i, chars[s..e].iter().collect()))
        })
        .collect()
}

fn check_range(doc: &Document, from: usize, to: usize) -> Result<(), String> {
    let n = doc.blocks.len();
    if n == 0 {
        return Err("文書にブロックがありません".into());
    }
    if from > to {
        return Err(format!("from={from} が to={to} より後です"));
    }
    if to >= n {
        return Err(format!("to={to} は範囲の外です(ブロックは 0〜{})", n - 1));
    }
    Ok(())
}

fn check_stamps(doc: &Document, from: usize, to: usize, expect: Option<&[String]>) -> Result<(), String> {
    let Some(expect) = expect else { return Ok(()) };
    if expect.len() != to - from + 1 {
        return Err(format!("照合の字が {} 個ですが、範囲は {} 個です", expect.len(), to - from + 1));
    }
    for (k, i) in (from..=to).enumerate() {
        let now = stamp(&write_range(doc, i, i)?);
        if now != expect[k] {
            return Err(format!("ブロック {i} は読んだ後に変わっています。読み直してください"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "= 報告\n\n== 概況\n\n受注は3件。\n見積は5件。\n\n|===\n|件名 |金額\n|外壁 |640,200\n|===\n\n== 予定\n\n8月に着手。\n";

    #[test]
    fn the_outline_lists_the_title_and_headings_with_their_index() {
        let d = adoc::parse(SRC).unwrap();
        let o = outline(&d);
        let got: Vec<(usize, u8, &str)> = o.iter().map(|h| (h.index, h.level, h.text.as_str())).collect();
        assert_eq!(got, vec![(0, 0, "報告"), (1, 1, "概況"), (4, 1, "予定")]);
    }

    #[test]
    fn reading_a_range_gives_each_block_as_asciidoc_with_a_stamp() {
        let d = adoc::parse(SRC).unwrap();
        let r = read(&d, 2, 3).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].adoc, "受注は3件。\n見積は5件。\n");
        assert_eq!(r[1].adoc, "|===\n|件名 |金額\n|外壁 |640,200\n|===\n");
        assert_eq!(r[0].stamp.len(), 8);
        assert_ne!(r[0].stamp, r[1].stamp);
        // 頭(題名)は範囲の字に入らない。題そのものを読めば `= ` で返る
        assert_eq!(read(&d, 0, 0).unwrap()[0].adoc, "= 報告\n");
        assert!(read(&d, 3, 9).is_err(), "範囲の外を断らない");
    }

    #[test]
    fn replacing_a_range_splices_the_fragment_and_the_whole_document_still_writes() {
        let mut d = adoc::parse(SRC).unwrap();
        let n = replace(&mut d, 2, 2, "受注は4件。\n\n* 外壁\n* 屋根\n", None).unwrap();
        assert_eq!(n, 3);
        let back = adoc::write(&d);
        assert_eq!(back, "= 報告\n\n== 概況\n\n受注は4件。\n\n* 外壁\n* 屋根\n\n|===\n|件名 |金額\n|外壁 |640,200\n|===\n\n== 予定\n\n8月に着手。\n");
    }

    #[test]
    fn a_stale_stamp_is_refused_and_a_fresh_one_passes() {
        let mut d = adoc::parse(SRC).unwrap();
        let st = read(&d, 2, 2).unwrap()[0].stamp.clone();
        replace(&mut d, 2, 2, "受注は4件。\n", None).unwrap();
        let e = replace(&mut d, 2, 2, "受注は5件。\n", Some(&[st])).unwrap_err();
        assert!(e.contains("変わっています"), "{e}");
        let st2 = read(&d, 2, 2).unwrap()[0].stamp.clone();
        replace(&mut d, 2, 2, "受注は5件。\n", Some(&[st2])).unwrap();
        assert_eq!(text_of(&d.blocks[2]), "受注は5件。");
    }

    #[test]
    fn insert_and_delete_move_the_neighbours() {
        let mut d = adoc::parse(SRC).unwrap();
        insert(&mut d, 5, "NOTE: 早めに。\n").unwrap();
        assert_eq!(outline(&d)[2].index, 4, "見出しの番号がずれた");
        assert_eq!(read(&d, 5, 5).unwrap()[0].adoc, "NOTE: 早めに。\n");
        let end = d.blocks.len();
        insert(&mut d, end, "終わり。\n").unwrap();
        assert_eq!(text_of(d.blocks.last().unwrap()), "終わり。");
        delete(&mut d, 1, 3, None).unwrap();
        assert_eq!(outline(&d).iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), vec!["報告", "予定"]);
        assert!(insert(&mut d, 99, "x\n").is_err());
    }

    #[test]
    fn a_fragment_may_not_carry_head_attributes() {
        let mut d = adoc::parse(SRC).unwrap();
        let e = replace(&mut d, 2, 2, ":author: 誰か\n本文。\n", None).unwrap_err();
        assert!(e.contains("属性"), "{e}");
    }

    #[test]
    fn find_returns_block_numbers_with_context() {
        let d = adoc::parse(SRC).unwrap();
        let hits = find(&d, "640,200");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 3);
        assert!(hits[0].1.contains("外壁"));
        assert!(find(&d, "").is_empty());
        assert_eq!(find(&d, "件").len(), 2, "段落と表の両方に「件」がある");
    }
}
