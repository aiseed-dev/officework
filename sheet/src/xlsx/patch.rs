//! **Filling an Excel template in place** (docs/sekkei/sashikomi.ja.adoc).
//!
//! A template made in Excel holds more than the model reads back, so the
//! filled file is not written again from the model: the cells that change
//! are written into the sheet's XML, and every other part of the package is
//! copied as it was, still compressed. A text value goes in as an inline
//! string (`t="inlineStr"`, ECMA-376 Part 1 18.3.1.4, 18.18.11); the cell
//! keeps its style index and its other attributes.
use book::{Book, Pos, SheetImage, Value};
use std::io::{Cursor, Read, Write};

/// A value to put in a cell of the sheet at `sheet` (its place in the
/// workbook, from 0).
#[derive(Debug, Clone, PartialEq)]
pub struct CellEdit {
    pub sheet: usize,
    pub at: Pos,
    pub value: Value,
}

/// A picture put in the shape named `shape` of the sheet at `sheet`.
#[derive(Debug, Clone)]
pub struct PicEdit {
    pub sheet: usize,
    pub shape: String,
    pub image: SheetImage,
}

/// The items of the data's two-column tables (name, value).
fn items(data: &Book) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for d in &data.sheets {
        let (rows, cols) = d.extent();
        if cols != 2 {
            continue;
        }
        for r in 0..rows {
            let k = d.value(Pos::new(r, 0)).display().trim().to_string();
            if !k.is_empty() {
                out.push((k, d.value(Pos::new(r, 1)).display().trim().to_string()));
            }
        }
    }
    out
}

/// PNG or JPEG, the pictures put in as they are
fn picture_kind(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG") {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8]) {
        Some("jpeg")
    } else {
        None
    }
}

/// **An Excel template filled from a data book**, as the bytes of the new
/// file, with notes of what could not be put in place (rows that need a
/// 別紙, pictures of a kind not taken yet).
///
/// A shape the template names as a data item (a photo box named `写真` in
/// Excel's name box) takes the picture file the data gives for it, looked
/// for in `dir` (the data file's folder): inside the shape, keeping its
/// proportions, centred, and the shape's instructions give way, as in an
/// adoc form (`book::form`). PNG and JPEG go in as they are.
pub fn fill_template(template: &[u8], data: &Book, dir: Option<&std::path::Path>) -> Result<(Vec<u8>, Vec<String>), String> {
    let (mut form, _) = super::read(Cursor::new(template))?;
    let mut notes = Vec::new();
    // The shapes named as data items become picture fields, when the data
    // gives a picture of a kind taken
    let items = items(data);
    let mut named: Vec<(usize, String)> = Vec::new();
    for (si, s) in form.sheets.iter_mut().enumerate() {
        for sp in s.shapes.iter_mut().chain(s.shapes_new.iter_mut()) {
            let Some(name) = sp.name.clone() else { continue };
            let Some((_, file)) = items.iter().find(|(k, v)| *k == name && !v.is_empty()) else { continue };
            let path = dir.map(|d| d.join(file)).unwrap_or_else(|| std::path::PathBuf::from(file));
            let Ok(bytes) = std::fs::read(&path) else {
                notes.push(format!("{name}: ファイルが見つかりません: {}", path.display()));
                continue;
            };
            if picture_kind(&bytes).is_none() {
                notes.push(format!("{name}: PNG か JPEG にしてください: {file}"));
                continue;
            }
            sp.field = Some(name.clone());
            named.push((si, name));
        }
    }
    let filled = book::form::fill_in(&form, data, dir);
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
    // The pictures, in the order the fill put them: the named shapes, sheet
    // by sheet
    let mut pics = Vec::new();
    for (si, s) in filled.sheets.iter().enumerate().take(form.sheets.len()) {
        let before = form.sheets[si].images_new.len();
        let names = named.iter().filter(|(i, _)| *i == si).map(|(_, n)| n.clone());
        for (name, im) in names.zip(s.images_new.iter().skip(before)) {
            pics.push(PicEdit { sheet: si, shape: name, image: im.clone() });
        }
    }
    Ok((patch(template, &edits, &pics)?, notes))
}

/// **The package with the cells changed** and everything else as it was.
pub fn patch_cells(package: &[u8], edits: &[CellEdit]) -> Result<Vec<u8>, String> {
    patch(package, edits, &[])
}

/// The part a relationship of `from` points at, as a package path.
fn resolve(from: &str, target: &str) -> String {
    if let Some(abs) = target.strip_prefix('/') {
        return abs.to_string();
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for seg in target.split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// The rels part of a part (`xl/drawings/drawing1.xml` →
/// `xl/drawings/_rels/drawing1.xml.rels`)
fn rels_of(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

/// The package with the cells changed and the pictures put in; everything
/// else as it was.
fn patch(package: &[u8], edits: &[CellEdit], pics: &[PicEdit]) -> Result<Vec<u8>, String> {
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
    // The pictures: each into the drawing part of its sheet, with its own
    // media part and relationship
    let mut added: Vec<(String, Vec<u8>)> = Vec::new();
    let names: Vec<String> = zin.file_names().map(str::to_string).collect();
    let take = |zin: &mut zip::ZipArchive<Cursor<&[u8]>>, changed: &mut Vec<(String, String)>, name: &str| -> Option<String> {
        if let Some((_, x)) = changed.iter().find(|(n, _)| n == name) {
            return Some(x.clone());
        }
        let mut s = String::new();
        zin.by_name(name).ok()?.read_to_string(&mut s).ok()?;
        Some(s)
    };
    let put = |changed: &mut Vec<(String, String)>, name: &str, xml: String| {
        match changed.iter_mut().find(|(n, _)| n == name) {
            Some((_, x)) => *x = xml,
            None => changed.push((name.to_string(), xml)),
        }
    };
    for (k, pic) in pics.iter().enumerate() {
        let sheet_path = paths.get(pic.sheet).ok_or("シートがありません")?.clone();
        let sheet_xml = take(&mut zin, &mut changed, &sheet_path).ok_or("シートが読めません")?;
        let rid = sheet_xml
            .find("<drawing ")
            .and_then(|i| attr(&sheet_xml[i..i + sheet_xml[i..].find('>').unwrap_or(0)], "r:id"))
            .ok_or("図形の部品がありません")?;
        let sheet_rels = take(&mut zin, &mut changed, &rels_of(&sheet_path)).ok_or("シートの関係がありません")?;
        let drawing = resolve(&sheet_path, &rel_target(&sheet_rels, &rid).ok_or("図形の部品がありません")?);
        let mut dxml = take(&mut zin, &mut changed, &drawing).ok_or("図形の部品が読めません")?;
        let drels_name = rels_of(&drawing);
        let mut drels = take(&mut zin, &mut changed, &drels_name).unwrap_or_else(|| {
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>".to_string()
        });
        // The media part, named so it meets no part of the template
        let ext = match picture_kind(&pic.image.data) { Some("png") => "png", _ => "jpeg" };
        let mut n = k + 1;
        let media = loop {
            let m = format!("xl/media/owimage{n}.{ext}");
            if !names.contains(&m) && !added.iter().any(|(a, _)| *a == m) {
                break m;
            }
            n += 1;
        };
        added.push((media.clone(), pic.image.data.clone()));
        let prid = format!("rIdOw{n}");
        let rel = format!(
            "<Relationship Id=\"{prid}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{}\"/>",
            media.rsplit('/').next().unwrap_or("")
        );
        let at = drels.rfind("</Relationships>").ok_or("関係が閉じていません")?;
        drels.insert_str(at, &rel);
        // A shape id above every id in the drawing
        let mut id = 1u32;
        let mut i = 0;
        while let Some(j) = dxml[i..].find("cNvPr id=\"") {
            let s0 = i + j + "cNvPr id=\"".len();
            let v: String = dxml[s0..].chars().take_while(|c| c.is_ascii_digit()).collect();
            id = id.max(v.parse::<u32>().unwrap_or(0) + 1);
            i = s0;
        }
        // The anchor as the writer makes it; the drawing root need not
        // declare the relationships namespace, so the picture declares it
        let anchor = super::write::image_anchor_xml(&pic.image, &prid, id).replace(
            "<a:blip r:embed",
            "<a:blip xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:embed",
        );
        // The instructions in the named shape give way to the picture
        if let Some(nm) = dxml.find(&format!("name=\"{}\"", super::write::esc(&pic.shape))) {
            let start = dxml[..nm].rfind("Anchor>").unwrap_or(0);
            let end = nm + dxml[nm..].find("Anchor>").unwrap_or(dxml.len() - nm);
            if let (Some(a), Some(b)) = (dxml[start..end].find("<xdr:txBody>"), dxml[start..end].find("</xdr:txBody>")) {
                dxml.replace_range(start + a..start + b + "</xdr:txBody>".len(), "");
            }
        }
        let close = dxml.rfind("</xdr:wsDr>").ok_or("図形の部品が閉じていません")?;
        dxml.insert_str(close, &anchor);
        put(&mut changed, &drawing, dxml);
        put(&mut changed, &drels_name, drels);
        let mut ct = take(&mut zin, &mut changed, "[Content_Types].xml").ok_or("[Content_Types].xml がありません")?;
        let ctype = if ext == "png" { "image/png" } else { "image/jpeg" };
        if !ct.contains(&format!("Extension=\"{ext}\"")) {
            let at = ct.rfind("</Types>").ok_or("[Content_Types].xml が閉じていません")?;
            ct.insert_str(at, &format!("<Default Extension=\"{ext}\" ContentType=\"{ctype}\"/>"));
            put(&mut changed, "[Content_Types].xml", ct);
        }
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
        for (name, xml) in changed.iter().filter(|(n, _)| !names_in(&zin, n)) {
            zout.start_file(name.clone(), opts).map_err(|e| e.to_string())?;
            zout.write_all(xml.as_bytes()).map_err(|e| e.to_string())?;
        }
        for (name, bytes) in &added {
            zout.start_file(name.clone(), opts).map_err(|e| e.to_string())?;
            zout.write_all(bytes).map_err(|e| e.to_string())?;
        }
        zout.finish().map_err(|e| e.to_string())?;
    }
    Ok(out.into_inner())
}

fn names_in(zin: &zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> bool {
    zin.file_names().any(|n| n == name)
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
        let (out, notes) = fill_template(&src, &data, None).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        let (back, _) = super::super::read(Cursor::new(&out)).unwrap();
        let s = &back.sheets[0];
        let at = |a: &str| s.cells.get(&Pos::parse(a).unwrap()).map(|c| c.value.display()).unwrap_or_default();
        assert_eq!(at("A1"), "氏名: 山田 花子");
        assert_eq!(at("B7"), "やまだ");
        assert_eq!(at("F12"), "見出し");
    }

    /// **A picture goes into the shape the template names after the data
    /// item**: a new picture in the sheet's drawing part and a new media
    /// part, the shape's instructions gone; every other part as it was
    #[test]
    fn a_picture_goes_into_the_named_shape() {
        let dir = std::env::temp_dir().join(format!("ow-patch-pic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&300u32.to_be_bytes());
        png.extend_from_slice(&400u32.to_be_bytes());
        std::fs::write(dir.join("p.png"), &png).unwrap();
        let mut b = Book::new();
        b.sheets.clear();
        let mut s = Sheet::new("履歴書");
        s.set(Pos::parse("A1").unwrap(), Cell::input("氏名"));
        s.shapes_new.push(book::SheetShape {
            name: Some("写真".into()),
            kind: "rect".into(),
            width_px: 120.0,
            height_px: 200.0,
            text: Some("写真をはる位置".into()),
            ..Default::default()
        });
        b.sheets.push(s);
        let mut src = Cursor::new(Vec::new());
        super::super::write(&b, &mut src).unwrap();
        let src = src.into_inner();
        let mut data = Book::new();
        data.sheets.clear();
        let mut kihon = Sheet::new("基本");
        kihon.set(Pos::new(0, 0), Cell::input("写真"));
        kihon.set(Pos::new(0, 1), Cell::input("p.png"));
        data.sheets.push(kihon);
        let (out, notes) = fill_template(&src, &data, Some(&dir)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(notes.is_empty(), "{notes:?}");
        let before = parts(&src);
        let after = parts(&out);
        let media: Vec<&String> = after.iter().map(|(n, _)| n).filter(|n| n.starts_with("xl/media/")).collect();
        assert_eq!(media.len(), 1, "{media:?}");
        for (n, v) in &before {
            let now = after.iter().find(|(m, _)| m == n).map(|(_, v)| v).expect(n);
            let may_change = n.contains("drawings/") || n == "[Content_Types].xml";
            if !may_change {
                assert_eq!(v, now, "{n} changed");
            }
        }
        let (back, _) = super::super::read(Cursor::new(&out)).unwrap();
        let sh = &back.sheets[0];
        assert_eq!(sh.images.len(), 1, "the picture is not read back");
        let im = &sh.images[0];
        assert_eq!((im.width_px.round(), im.height_px.round()), (120.0, 160.0));
        let box_text = sh.shapes.iter().find(|sp| sp.name.as_deref() == Some("写真")).and_then(|sp| sp.text.clone());
        assert_eq!(box_text, None, "the instructions stay over the picture");
    }
}
