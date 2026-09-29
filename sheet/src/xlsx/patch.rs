//! **Filling an Excel template in place** (docs/sekkei/sashikomi.ja.adoc).
//!
//! A template made in Excel holds more than the model reads back, so the
//! filled file is not written again from the model: the cells that change
//! are written into the sheet's XML, and every other part of the package is
//! copied as it was, still compressed. A text value goes in as an inline
//! string (`t="inlineStr"`, ECMA-376 Part 1 18.3.1.4, 18.18.11); the cell
//! keeps its style index and its other attributes.
use book::{Book, Pos, Value};
use std::io::{Cursor, Read, Write};

/// A value to put in a cell of the sheet at `sheet` (its place in the
/// workbook, from 0).
#[derive(Debug, Clone, PartialEq)]
pub struct CellEdit {
    pub sheet: usize,
    pub at: Pos,
    pub value: Value,
}

/// **An Excel template filled from a data book**, as the bytes of the new
/// file, with notes of what could not be put in place (rows that need a
/// 別紙, pictures for picture fields): those need the model's writer.
pub fn fill_template(template: &[u8], data: &Book) -> Result<(Vec<u8>, Vec<String>), String> {
    let (form, _) = super::read(Cursor::new(template))?;
    let filled = book::form::fill(&form, data);
    let mut notes = Vec::new();
    if filled.sheets.len() > form.sheets.len() {
        notes.push(format!(
            "入りきらない行があります(別紙が要ります): {}",
            filled.sheets[form.sheets.len()..].iter().map(|s| s.name.clone()).collect::<Vec<_>>().join("・")
        ));
    }
    let mut edits = Vec::new();
    for (si, (a, b)) in form.sheets.iter().zip(&filled.sheets).enumerate() {
        if a.images.len() != b.images.len() {
            notes.push(format!("{}: 写真の欄には、この形では入れられません", a.name));
        }
        let mut at: Vec<Pos> = a.cells.keys().chain(b.cells.keys()).copied().collect();
        at.sort();
        at.dedup();
        for p in at {
            let (ca, cb) = (a.cells.get(&p), b.cells.get(&p));
            let before = ca.map(|c| (&c.formula, &c.value));
            let after = cb.map(|c| (&c.formula, &c.value));
            if before != after {
                edits.push(CellEdit {
                    sheet: si,
                    at: p,
                    value: cb.map(|c| c.value.clone()).unwrap_or_default(),
                });
            }
        }
    }
    Ok((patch_cells(template, &edits)?, notes))
}

/// **The package with the cells changed** and everything else as it was.
pub fn patch_cells(package: &[u8], edits: &[CellEdit]) -> Result<Vec<u8>, String> {
    let mut zin = zip::ZipArchive::new(Cursor::new(package)).map_err(|e| e.to_string())?;
    let text = |zin: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str| -> Result<String, String> {
        let mut s = String::new();
        zin.by_name(name).map_err(|e| format!("{name}: {e}"))?.read_to_string(&mut s).map_err(|e| e.to_string())?;
        Ok(s)
    };
    let paths = sheet_paths(&text(&mut zin, "xl/workbook.xml")?, &text(&mut zin, "xl/_rels/workbook.xml.rels")?);
    let mut changed: Vec<(String, String)> = Vec::new();
    let mut formula_gone = false;
    let mut sheets: Vec<usize> = edits.iter().map(|e| e.sheet).collect();
    sheets.sort();
    sheets.dedup();
    for si in sheets {
        let path = paths.get(si).ok_or_else(|| format!("シート {} がありません", si + 1))?;
        let mut xml = text(&mut zin, path)?;
        for e in edits.iter().filter(|e| e.sheet == si) {
            formula_gone |= set_cell(&mut xml, e.at, &e.value)?;
        }
        changed.push((path.clone(), xml));
    }
    // A formula replaced by a value leaves its entry in the calculation
    // chain; Excel asks to repair a chain that names a cell with no formula.
    // The chain is optional and Excel makes it again (18.6)
    let mut gone: Vec<String> = Vec::new();
    if formula_gone && zin.by_name("xl/calcChain.xml").is_ok() {
        gone.push("xl/calcChain.xml".into());
        let ct = text(&mut zin, "[Content_Types].xml")?;
        changed.push(("[Content_Types].xml".into(), remove_tag(&ct, "<Override ", "calcChain")));
        let rels = text(&mut zin, "xl/_rels/workbook.xml.rels")?;
        changed.push(("xl/_rels/workbook.xml.rels".into(), remove_tag(&rels, "<Relationship ", "calcChain")));
    }
    let mut out = Cursor::new(Vec::new());
    {
        let mut zout = zip::ZipWriter::new(&mut out);
        let opts: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for i in 0..zin.len() {
            let f = zin.by_index_raw(i).map_err(|e| e.to_string())?;
            let name = f.name().to_string();
            if gone.contains(&name) {
                continue;
            }
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
    Ok(out.into_inner())
}


/// The part of each sheet, in the workbook's order.
fn sheet_paths(workbook: &str, rels: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = workbook[at..].find("<sheet ") {
        let s = at + i;
        let end = workbook[s..].find('>').map(|e| s + e).unwrap_or(workbook.len());
        let tag = &workbook[s..end];
        at = end;
        let Some(rid) = attr(tag, "r:id") else { continue };
        let Some(target) = rel_target(rels, &rid) else { continue };
        out.push(match target.strip_prefix('/') {
            Some(abs) => abs.to_string(),
            None => format!("xl/{target}"),
        });
    }
    out
}

fn rel_target(rels: &str, id: &str) -> Option<String> {
    let mut at = 0;
    while let Some(i) = rels[at..].find("<Relationship ") {
        let s = at + i;
        let end = rels[s..].find('>').map(|e| s + e)?;
        let tag = &rels[s..end];
        at = end;
        if attr(tag, "Id").as_deref() == Some(id) {
            return attr(tag, "Target");
        }
    }
    None
}

/// The value of attribute `name` in a start tag (quotes either kind).
fn attr(tag: &str, name: &str) -> Option<String> {
    let mut at = 0;
    while let Some(i) = tag[at..].find(name) {
        let s = at + i;
        at = s + name.len();
        let before_ok = s == 0 || tag[..s].ends_with(char::is_whitespace);
        let rest = tag[at..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let rest = rest[1..].trim_start();
        let q = rest.chars().next()?;
        if q != '"' && q != '\'' {
            return None;
        }
        return rest[1..].split(q).next().map(str::to_string);
    }
    None
}

/// The text with every element that starts with `open` and holds `needle`
/// taken out (an element that closes itself with `/>`).
fn remove_tag(xml: &str, open: &str, needle: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut at = 0;
    while let Some(i) = xml[at..].find(open) {
        let s = at + i;
        let Some(e) = xml[s..].find("/>").map(|e| s + e + 2) else { break };
        out.push_str(&xml[at..s]);
        if !xml[s..e].contains(needle) {
            out.push_str(&xml[s..e]);
        }
        at = e;
    }
    out.push_str(&xml[at..]);
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The number as the file writes it: the shortest form that reads back
fn number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// The start tag of a cell with its value's type, the attributes that
/// belong to the old value (`t`, `vm`) taken off, and the children.
fn cell_xml(old_tag: &str, at: Pos, value: &Value) -> String {
    let mut attrs = String::new();
    // Keep the attributes the cell had, but those of its old value
    let body = old_tag.trim_start_matches("<c").trim_end_matches('>').trim_end_matches('/');
    let mut rest = body.trim();
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else { break };
        let name = rest[..eq].trim().to_string();
        let v = rest[eq + 1..].trim_start();
        let Some(q) = v.chars().next() else { break };
        let Some(close) = v[1..].find(q) else { break };
        let whole = &v[..close + 2];
        if !matches!(name.as_str(), "t" | "vm" | "r") {
            attrs.push_str(&format!(" {name}={whole}"));
        }
        rest = v[close + 2..].trim_start();
    }
    let r = at.a1();
    match value {
        Value::Empty => format!("<c r=\"{r}\"{attrs}/>"),
        Value::Number(n) => format!("<c r=\"{r}\"{attrs}><v>{}</v></c>", number(*n)),
        Value::Bool(b) => format!("<c r=\"{r}\"{attrs} t=\"b\"><v>{}</v></c>", u8::from(*b)),
        Value::Error(e) => format!("<c r=\"{r}\"{attrs} t=\"e\"><v>{}</v></c>", esc(e)),
        other => format!(
            "<c r=\"{r}\"{attrs} t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
            esc(&other.display())
        ),
    }
}

/// Put `value` in the cell at `at` of a sheet's XML, making its row and
/// the cell when they are not there. Returns whether a formula went.
fn set_cell(xml: &mut String, at: Pos, value: &Value) -> Result<bool, String> {
    let (row_no, a1) = (at.row + 1, at.a1());
    // The sheet data, made when the sheet has none
    if !xml.contains("<sheetData>") && !xml.contains("<sheetData ") {
        let Some(i) = xml.find("<sheetData/>") else { return Err("sheetData がありません".into()) };
        xml.replace_range(i..i + "<sheetData/>".len(), "<sheetData></sheetData>");
    }
    let data_start = xml.find("<sheetData").ok_or("sheetData がありません")?;
    let open_end = data_start + xml[data_start..].find('>').ok_or("sheetData が閉じていません")? + 1;
    let data_end = open_end + xml[open_end..].find("</sheetData>").ok_or("sheetData が閉じていません")?;
    // Find the row, or where it goes
    let mut at_row: Option<(usize, usize, bool)> = None; // (start, end of the whole row, self-closing)
    let mut insert_row_at = data_end;
    let mut i = open_end;
    while let Some(k) = xml[i..data_end].find("<row") {
        let s = i + k;
        let tag_end = s + xml[s..].find('>').ok_or("row が閉じていません")? + 1;
        let tag = &xml[s..tag_end];
        let self_closing = tag.ends_with("/>");
        let end = if self_closing {
            tag_end
        } else {
            tag_end + xml[tag_end..].find("</row>").ok_or("row が閉じていません")? + "</row>".len()
        };
        let r: u32 = attr(tag, "r").and_then(|v| v.parse().ok()).unwrap_or(0);
        if r == row_no {
            at_row = Some((s, end, self_closing));
            break;
        }
        if r > row_no {
            insert_row_at = s;
            break;
        }
        i = end;
    }
    let Some((rs, re, self_closing)) = at_row else {
        let new = format!("<row r=\"{row_no}\">{}</row>", cell_xml("<c>", at, value));
        xml.insert_str(insert_row_at, &new);
        return Ok(false);
    };
    if self_closing {
        let tag = xml[rs..re].trim_end_matches("/>").to_string();
        let new = format!("{tag}>{}</row>", cell_xml("<c>", at, value));
        xml.replace_range(rs..re, &new);
        return Ok(false);
    }
    // Find the cell in the row, or where it goes (cells are in column order)
    let body_start = rs + xml[rs..].find('>').unwrap_or(0) + 1;
    let body_end = re - "</row>".len();
    let mut j = body_start;
    let mut insert_at = body_end;
    while let Some(k) = xml[j..body_end].find("<c") {
        let s = j + k;
        // `<c` followed by a space or `>`: not `<col…`
        let next = xml[s + 2..].chars().next();
        if !matches!(next, Some(' ' | '>' | '/')) {
            j = s + 2;
            continue;
        }
        let tag_end = s + xml[s..].find('>').ok_or("c が閉じていません")? + 1;
        let tag = xml[s..tag_end].to_string();
        let end = if tag.ends_with("/>") {
            tag_end
        } else {
            tag_end + xml[tag_end..].find("</c>").ok_or("c が閉じていません")? + "</c>".len()
        };
        let r = attr(&tag, "r").unwrap_or_default();
        if r == a1 {
            let had_formula = xml[tag_end..end].contains("<f");
            let new = cell_xml(&tag, at, value);
            xml.replace_range(s..end, &new);
            return Ok(had_formula);
        }
        if let Some(p) = Pos::parse(&r) {
            if p.col > at.col {
                insert_at = s;
                break;
            }
        }
        j = end;
    }
    xml.insert_str(insert_at, &cell_xml("<c>", at, value));
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use book::{Cell, Sheet};

    fn template() -> Vec<u8> {
        let mut b = Book::new();
        b.sheets.clear();
        let mut s = Sheet::new("履歴書");
        let mut b7 = Cell::input("");
        b7.fmt.bold = true;
        s.set(Pos::parse("B7").unwrap(), b7);
        s.set(Pos::parse("A1").unwrap(), Cell::input("氏名: {氏名}"));
        s.set(Pos::parse("C3").unwrap(), Cell::input("=1+1"));
        s.set(Pos::parse("F12").unwrap(), Cell::input("見出し"));
        b.sheets.push(s);
        let mut out = Cursor::new(Vec::new());
        super::super::write(&b, &mut out).unwrap();
        out.into_inner()
    }

    fn parts(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        (0..z.len())
            .map(|i| {
                let mut f = z.by_index(i).unwrap();
                let mut v = Vec::new();
                f.read_to_end(&mut v).unwrap();
                (f.name().to_string(), v)
            })
            .collect()
    }

    /// **Only the sheet changes**: every other part is copied byte for
    /// byte, the filled cell keeps its style, and new cells and rows go in
    /// their places
    #[test]
    fn only_the_sheet_changes() {
        let src = template();
        let edits = vec![
            CellEdit { sheet: 0, at: Pos::parse("B7").unwrap(), value: Value::Text("山田 & 花子".into()) },
            CellEdit { sheet: 0, at: Pos::parse("D9").unwrap(), value: Value::Number(2010.0) },
            CellEdit { sheet: 0, at: Pos::parse("A12").unwrap(), value: Value::Text("前".into()) },
            CellEdit { sheet: 0, at: Pos::parse("C3").unwrap(), value: Value::Text("値".into()) },
        ];
        let out = patch_cells(&src, &edits).unwrap();
        for ((na, a), (nb, b)) in parts(&src).iter().zip(parts(&out).iter()) {
            assert_eq!(na, nb);
            if !na.contains("worksheets/sheet") {
                assert_eq!(a, b, "{na} changed");
            }
        }
        let (back, _) = super::super::read(Cursor::new(&out)).unwrap();
        let s = &back.sheets[0];
        let at = |a: &str| s.cells.get(&Pos::parse(a).unwrap()).cloned().unwrap_or_default();
        assert_eq!(at("B7").value, Value::Text("山田 & 花子".into()));
        assert!(at("B7").fmt.bold, "the cell lost its style");
        assert_eq!(at("D9").value, Value::Number(2010.0));
        assert_eq!(at("A12").value, Value::Text("前".into()));
        assert_eq!(at("F12").value, Value::Text("見出し".into()), "the cell after the new one went");
        assert_eq!(at("C3").formula, None, "the formula is left");
        assert_eq!(at("C3").value, Value::Text("値".into()));
    }

    /// **A template fills by marks, addresses and names in place**
    #[test]
    fn a_template_fills_in_place() {
        let src = template();
        let mut data = Book::new();
        data.sheets.clear();
        let mut kihon = Sheet::new("基本");
        for (r, (k, v)) in [("氏名", "山田 花子"), ("B7", "やまだ")].iter().enumerate() {
            kihon.set(Pos::new(r as u32, 0), Cell::input(k));
            kihon.set(Pos::new(r as u32, 1), Cell::input(v));
        }
        data.sheets.push(kihon);
        let (out, notes) = fill_template(&src, &data).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        let (back, _) = super::super::read(Cursor::new(&out)).unwrap();
        let s = &back.sheets[0];
        let at = |a: &str| s.cells.get(&Pos::parse(a).unwrap()).map(|c| c.value.display()).unwrap_or_default();
        assert_eq!(at("A1"), "氏名: 山田 花子");
        assert_eq!(at("B7"), "やまだ");
        assert_eq!(at("F12"), "見出し");
    }
}
