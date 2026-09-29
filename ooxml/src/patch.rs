//! **Filling a Word template in place** (docs/sekkei/sashikomi.ja.adoc).
//!
//! A template made in Word holds more than the model reads back, so the
//! filled document is not written again from the model. In the body, the
//! headers and the footers, only the text of the marks (`{氏名}`) and the
//! mail merge fields (`MERGEFIELD 氏名`, ECMA-376 Part 1 17.16.5.35) is
//! changed; every other part, and everything else in those parts, stays as
//! it was.
//!
//! - A mark is looked for in the text of its paragraph, so a mark Word has
//!   split over several runs (it does, when spelling or editing marks part
//!   of it) is found. The answer goes into the run where the mark starts,
//!   in that run's format, and the rest of the mark leaves the later runs.
//! - A mail merge field, from its `begin` to its `end` (or a `w:fldSimple`),
//!   becomes one run holding the value, in the format of the text the field
//!   showed. The filled document holds the value as plain text, as the
//!   document Word's mail merge makes.
//! - The answers are those of a form (`book::form`): `{氏名}`, `{日付.和暦年}`,
//!   `{学歴・職歴.3.内容}` and the rest. A mark or field the data does not
//!   answer becomes empty; names the data lacks altogether are returned.
use book::form::{fill_text_ranges, missing_of, Data};
use std::io::{Cursor, Read, Write};

/// **A Word template filled from a data book**: the bytes of the new file,
/// and the names the data lacks altogether (filled with nothing).
pub fn fill_template(template: &[u8], data: &book::Book) -> Result<(Vec<u8>, Vec<String>), String> {
    let mut zin = zip::ZipArchive::new(Cursor::new(template)).map_err(|e| e.to_string())?;
    let answers = Data::new(data);
    let names: Vec<String> = zin.file_names().map(str::to_string).collect();
    let parts: Vec<String> = names
        .iter()
        .filter(|n| {
            *n == "word/document.xml"
                || (n.starts_with("word/header") || n.starts_with("word/footer")) && n.ends_with(".xml")
        })
        .cloned()
        .collect();
    let mut changed: Vec<(String, String)> = Vec::new();
    let mut asked: Vec<String> = Vec::new();
    for p in &parts {
        let mut xml = String::new();
        zin.by_name(p).map_err(|e| e.to_string())?.read_to_string(&mut xml).map_err(|e| e.to_string())?;
        let filled = fill_part(&xml, &answers, &mut asked);
        if filled != xml {
            changed.push((p.clone(), filled));
        }
    }
    asked.sort();
    asked.dedup();
    let missing = missing_of(asked, data);
    let mut out = Cursor::new(Vec::new());
    {
        let mut zout = zip::ZipWriter::new(&mut out);
        let opts: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for i in 0..zin.len() {
            let f = zin.by_index_raw(i).map_err(|e| e.to_string())?;
            let name = f.name().to_string();
            match changed.iter().find(|(n, _)| *n == name) {
                Some((_, xml)) => {
                    drop(f);
                    zout.start_file(name, opts).map_err(|e| e.to_string())?;
                    zout.write_all(xml.as_bytes()).map_err(|e| e.to_string())?;
                }
                None => zout.raw_copy_file(f).map_err(|e| e.to_string())?,
            }
        }
        zout.finish().map_err(|e| e.to_string())?;
    }
    Ok((out.into_inner(), missing))
}

/// One part (body, header or footer) with its fields and marks filled.
/// The names asked for are added to `asked`.
pub(crate) fn fill_part(xml: &str, answers: &Data, asked: &mut Vec<String>) -> String {
    let xml = fill_simple_fields(xml, answers, asked);
    let xml = fill_complex_fields(&xml, answers, asked);
    fill_marks(&xml, answers, asked)
}

/// The value of a data item, as a mark `{name}` gives it
fn answer(name: &str, answers: &Data, asked: &mut Vec<String>) -> String {
    asked.push(name.to_string());
    fill_text_ranges(&format!("{{{name}}}"), answers).text
}

fn unesc(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(j) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let ent = &tail[1..j];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=j]),
        }
        rest = &tail[j + 1..];
    }
    out.push_str(rest);
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The value of attribute `name` in a start tag
fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=");
    let i = tag.find(&key)? + key.len();
    let q = tag[i..].chars().next()?;
    tag[i + 1..].split(q).next().map(unesc)
}

/// The run properties (`<w:rPr>…</w:rPr>`) in a stretch of XML, or nothing
fn run_props(xml: &str) -> &str {
    match (xml.find("<w:rPr>"), xml.find("</w:rPr>")) {
        (Some(a), Some(b)) if a < b => &xml[a..b + "</w:rPr>".len()],
        _ => "",
    }
}

fn value_run(props: &str, value: &str) -> String {
    format!("<w:r>{props}<w:t xml:space=\"preserve\">{}</w:t></w:r>", esc(value))
}

/// `<w:fldSimple w:instr="MERGEFIELD 氏名">…</w:fldSimple>` → the value
fn fill_simple_fields(xml: &str, answers: &Data, asked: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(i) = rest.find("<w:fldSimple") {
        let tag_end = match rest[i..].find('>') {
            Some(e) => i + e + 1,
            None => break,
        };
        let tag = &rest[i..tag_end];
        let self_closing = tag.ends_with("/>");
        let end = if self_closing {
            tag_end
        } else {
            match rest[tag_end..].find("</w:fldSimple>") {
                Some(e) => tag_end + e + "</w:fldSimple>".len(),
                None => break,
            }
        };
        let name = attr(tag, "w:instr").and_then(|s| crate::read::merge_instr(&s));
        out.push_str(&rest[..i]);
        match name {
            Some(name) => {
                let v = answer(&name, answers, asked);
                out.push_str(&value_run(run_props(&rest[tag_end..end]), &v));
            }
            None => out.push_str(&rest[i..end]),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The start of the run (`<w:r>` or `<w:r …>`) that holds the byte `at`
fn run_start(xml: &str, at: usize) -> Option<usize> {
    let mut i = at;
    loop {
        let s = xml[..i].rfind("<w:r")?;
        let next = xml[s + 4..].chars().next();
        if matches!(next, Some('>' | ' ')) {
            return Some(s);
        }
        i = s;
    }
}

/// A complex mail merge field (`w:fldChar` begin … instrText … separate …
/// result … end) → one run holding the value, in the result's format
fn fill_complex_fields(xml: &str, answers: &Data, asked: &mut Vec<String>) -> String {
    const BEGIN: &str = "w:fldCharType=\"begin\"";
    let mut out = String::with_capacity(xml.len());
    let mut done = 0usize;
    let mut from = 0usize;
    while let Some(k) = xml[from..].find(BEGIN) {
        let b = from + k;
        from = b + BEGIN.len();
        // The matching end, counting fields inside this one
        let mut depth = 1;
        let mut j = from;
        let mut sep: Option<usize> = None;
        let mut end: Option<usize> = None;
        while let Some(t) = xml[j..].find("w:fldCharType=\"") {
            let at = j + t;
            let kind: String = xml[at + 15..].chars().take_while(|c| *c != '"').collect();
            match kind.as_str() {
                "begin" => depth += 1,
                "separate" if depth == 1 => sep = Some(at),
                "end" => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(at);
                        break;
                    }
                }
                _ => {}
            }
            j = at + 15;
        }
        let Some(end) = end else { break };
        // The instruction: the instrText between begin and separate (or end)
        let head_end = sep.unwrap_or(end);
        let mut instr = String::new();
        let mut t = b;
        while let Some(x) = xml[t..head_end].find("<w:instrText") {
            let s = t + x;
            let Some(gt) = xml[s..].find('>') else { break };
            let Some(close) = xml[s..].find("</w:instrText>") else { break };
            instr.push_str(&unesc(&xml[s + gt + 1..s + close]));
            t = s + close;
        }
        let Some(name) = crate::read::merge_instr(&instr) else { continue };
        let (Some(rs), Some(re)) = (run_start(xml, b), xml[end..].find("</w:r>").map(|e| end + e + "</w:r>".len())) else {
            continue;
        };
        if rs < done {
            continue;
        }
        let v = answer(&name, answers, asked);
        // The format of the text the field showed, else of the field's first run
        let props = match sep {
            Some(s) => {
                let shown = run_props(&xml[s..end]);
                if shown.is_empty() { run_props(&xml[rs..b]) } else { shown }
            }
            None => run_props(&xml[rs..b]),
        };
        out.push_str(&xml[done..rs]);
        out.push_str(&value_run(props, &v));
        done = re;
        from = re;
    }
    out.push_str(&xml[done..]);
    out
}

/// A text node: where its text starts and ends in the part, which
/// paragraph it is in, and its text
struct TextNode {
    open: usize,
    end: usize,
    para: usize,
    text: String,
}

/// The marks `{…}` answered, looked for in the text of each paragraph
fn fill_marks(xml: &str, answers: &Data, asked: &mut Vec<String>) -> String {
    // The text nodes, with the innermost paragraph each is in
    let mut nodes: Vec<TextNode> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut paras = 0usize;
    let mut i = 0usize;
    while let Some(k) = xml[i..].find('<') {
        let s = i + k;
        let Some(e) = xml[s..].find('>').map(|e| s + e + 1) else { break };
        let tag = &xml[s..e];
        let name: String = tag[1..].chars().take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/').collect();
        match name.as_str() {
            "w:p" if !tag.ends_with("/>") => {
                stack.push(paras);
                paras += 1;
            }
            "/w:p" => {
                stack.pop();
            }
            "w:t" if !tag.ends_with("/>") => {
                if let Some(close) = xml[e..].find("</w:t>") {
                    nodes.push(TextNode {
                        open: s,
                        end: e + close,
                        para: stack.last().copied().unwrap_or(usize::MAX),
                        text: unesc(&xml[e..e + close]),
                    });
                    i = e + close;
                    continue;
                }
            }
            _ => {}
        }
        i = e;
    }
    // Answer the marks paragraph by paragraph
    let mut texts: Vec<Option<String>> = vec![None; nodes.len()];
    let mut idx = 0;
    while idx < nodes.len() {
        let para = nodes[idx].para;
        let group: Vec<usize> = (idx..nodes.len()).take_while(|&n| nodes[n].para == para).collect();
        idx += group.len();
        let joined: String = group.iter().map(|&n| nodes[n].text.as_str()).collect();
        if !joined.contains('{') {
            continue;
        }
        // Where each node's text starts in the joined text
        let mut offs = Vec::with_capacity(group.len());
        let mut acc = 0;
        for &n in &group {
            offs.push(acc);
            acc += nodes[n].text.len();
        }
        let mut cur: Vec<String> = group.iter().map(|&n| nodes[n].text.clone()).collect();
        // Marks from the last, so earlier byte places stay right
        let mut spans: Vec<(usize, usize)> = Vec::new();
        let mut from = 0;
        while let Some(a) = joined[from..].find('{') {
            let a = from + a;
            let Some(b) = joined[a..].find('}') else { break };
            let b = a + b + 1;
            if !book::form::marks(&joined[a..b]).is_empty() && !joined[a + 1..b - 1].contains('{') {
                spans.push((a, b));
            }
            from = a + 1;
        }
        for &(a, b) in spans.iter().rev() {
            let mark = &joined[a..b];
            for m in book::form::marks(mark) {
                asked.push(m);
            }
            let filled = fill_text_ranges(mark, answers).text;
            // The node where the mark starts and where it ends
            let first = offs.iter().rposition(|&o| o <= a).unwrap_or(0);
            let last = offs.iter().rposition(|&o| o < b).unwrap_or(first);
            let (fa, lb) = (a - offs[first], b - offs[last]);
            if first == last {
                cur[first].replace_range(fa..lb, &filled);
            } else {
                cur[first].replace_range(fa.., &filled);
                for c in cur.iter_mut().take(last).skip(first + 1) {
                    c.clear();
                }
                cur[last].replace_range(..lb, "");
            }
        }
        for (gi, &n) in group.iter().enumerate() {
            if cur[gi] != nodes[n].text {
                texts[n] = Some(cur[gi].clone());
            }
        }
    }
    // Write the changed nodes back, keeping their spaces
    let mut out = String::with_capacity(xml.len());
    let mut done = 0;
    for (n, t) in nodes.iter().zip(&texts) {
        let Some(t) = t else { continue };
        out.push_str(&xml[done..n.open]);
        out.push_str("<w:t xml:space=\"preserve\">");
        out.push_str(&esc(t));
        done = n.end;
    }
    out.push_str(&xml[done..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use book::{Book, Cell, Pos, Sheet};

    fn data() -> Book {
        let mut b = Book::new();
        b.sheets.clear();
        let mut kihon = Sheet::new("基本");
        for (r, (k, v)) in [("氏名", "山田 花子"), ("宛名", "千代田商事 御中"), ("日付", "2026-09-29")].iter().enumerate() {
            kihon.set(Pos::new(r as u32, 0), Cell::input(k));
            kihon.set(Pos::new(r as u32, 1), Cell::input(v));
        }
        b.sheets.push(kihon);
        b
    }

    fn fill(xml: &str) -> (String, Vec<String>) {
        let d = data();
        let mut asked = Vec::new();
        let out = fill_part(xml, &Data::new(&d), &mut asked);
        (out, asked)
    }

    /// **A mark Word split over runs is found**, and the answer takes the
    /// format of the run the mark starts in; the other runs keep theirs
    #[test]
    fn a_mark_split_over_runs_is_filled() {
        let xml = r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>氏名: {氏</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>名} 様</w:t></w:r></w:p>"#;
        let (out, asked) = fill(xml);
        assert_eq!(out, r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">氏名: 山田 花子</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t xml:space="preserve"> 様</w:t></w:r></w:p>"#);
        assert_eq!(asked, vec!["氏名".to_string()]);
    }

    /// **Marks in two paragraphs stay apart**, a brace that is not a mark
    /// stays, and a mark with the form's parts (`{日付.年}`) is answered
    #[test]
    fn marks_stay_in_their_paragraphs() {
        let xml = r#"<w:p><w:r><w:t>{日付.年}年 {</w:t></w:r></w:p><w:p><w:r><w:t>宛名} &amp; {宛名}</w:t></w:r></w:p>"#;
        let (out, _) = fill(xml);
        assert!(out.contains(">2026年 {</w:t>"), "{out}");
        assert!(out.contains(">宛名} &amp; 千代田商事 御中</w:t>"), "{out}");
    }

    /// **A mail merge field becomes one run with the value**, in the format
    /// of the text it showed; a field of another kind stays
    #[test]
    fn mail_merge_fields_become_their_values() {
        let xml = concat!(
            r#"<w:p><w:r><w:t>宛先:</w:t></w:r>"#,
            r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r>"#,
            r#"<w:r><w:instrText xml:space="preserve"> MERGEFIELD 宛名 \* MERGEFORMAT </w:instrText></w:r>"#,
            r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r>"#,
            r#"<w:r><w:rPr><w:sz w:val="28"/></w:rPr><w:t>«宛名»</w:t></w:r>"#,
            r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#,
            r#"<w:fldSimple w:instr=" MERGEFIELD 氏名 "><w:r><w:rPr><w:b/></w:rPr><w:t>«氏名»</w:t></w:r></w:fldSimple>"#,
            r#"<w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p>"#,
        );
        let (out, asked) = fill(xml);
        assert!(out.contains(r#"<w:r><w:rPr><w:sz w:val="28"/></w:rPr><w:t xml:space="preserve">千代田商事 御中</w:t></w:r>"#), "{out}");
        assert!(out.contains(r#"<w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">山田 花子</w:t></w:r>"#), "{out}");
        assert!(!out.contains("MERGEFIELD"), "{out}");
        assert!(out.contains(r#"w:instr=" PAGE ""#), "the page field went: {out}");
        assert_eq!(asked, vec!["氏名".to_string(), "宛名".to_string()]);
    }

    /// **Only the parts with marks change**; the rest of the package is
    /// copied as it was, and names the data lacks are said
    #[test]
    fn only_the_parts_with_marks_change() {
        let mut d = kumihan::Document::plain("氏名: {氏名}\n電話: {電話}");
        d.header.paragraphs = vec![kumihan::Paragraph {
            runs: vec![kumihan::Run { text: "社外秘".into(), size_pt: None, font: None, fmt: Default::default() }],
            ..Default::default()
        }];
        let mut src = Cursor::new(Vec::new());
        crate::write(&d, &mut src).unwrap();
        let src = src.into_inner();
        let (out, missing) = fill_template(&src, &data()).unwrap();
        assert_eq!(missing, vec!["電話".to_string()]);
        let parts = |b: &[u8]| -> Vec<(String, Vec<u8>)> {
            let mut z = zip::ZipArchive::new(Cursor::new(b)).unwrap();
            (0..z.len())
                .map(|i| {
                    let mut f = z.by_index(i).unwrap();
                    let mut v = Vec::new();
                    f.read_to_end(&mut v).unwrap();
                    (f.name().to_string(), v)
                })
                .collect()
        };
        for ((na, a), (nb, b)) in parts(&src).iter().zip(parts(&out).iter()) {
            assert_eq!(na, nb);
            if na != "word/document.xml" {
                assert_eq!(a, b, "{na} changed");
            }
        }
        let (back, _) = crate::read(Cursor::new(&out)).unwrap();
        assert_eq!(back.body_text(), "氏名: 山田 花子\n電話: ");
    }
}
