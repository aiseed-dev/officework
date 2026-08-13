//! **xlsx を書く。** こちらが作り直す部品以外は、原本から持ち越す。
//!
//! 図形・テーマ・印刷設定・文書情報は原本のまま写す。**理解しない部品を
//! 壊さない**ための作りで、`write_with` に原本を渡すとそれが効く。

use std::io::{Cursor, Read, Seek, Write};

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};

use crate::model::{Book, Cell, Pos, Sheet, Value};

use super::read::{attr, local, parse_rels, resolve_book_target, resolve_target, sheet_part_no, sheet_parts};

/// localSheetId 属性の頭(引用符の入れ子を避けるため定数で持つ)
pub(super) const SID_ATTR: &str = "localSheetId=\"";

// ---------- 書く ----------

pub(super) const CT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>__SHEETS__<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;

pub(super) const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;

pub(super) const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
/// スレッドのコメントの名前空間(2018。Excel 365 以降がこちらを見る)
pub(super) const TCNS: &str =
    "http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments";

/// 番号から決まった形の GUID を作る。
///
/// **乱数を使わない。** 同じブックを2回書いたら同じ物が出てほしい —
/// 差分が読めなくなるし、試験も書けない。Excel は中身を見ず、
/// 一意であることしか要求しない
pub(super) fn guid(n: usize) -> String {
    format!("{{00000000-0000-0000-0000-{n:012X}}}")
}
pub(super) const RNS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

pub(super) fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
     .replace('"', "&quot;")
}

/// definedName の中身を (シート名, "A1" か "A1:B2") に分ける。
/// 'Sheet 1'!$A$1 の引用も解く。理解できない形なら None(原文で持ち越す側)。
pub(super) fn split_defined(target: &str) -> Option<(String, String)> {
    let (sheet, r) = target.split_once('!')?;
    let sheet = sheet.trim();
    let sheet = if let Some(q) = sheet.strip_prefix('\'') {
        q.strip_suffix('\'')?.replace("''", "'")
    } else {
        sheet.to_string()
    };
    let plain: String = r.chars().filter(|c| *c != '$').collect();
    // A1 か A1:B2 の形だけ。複数範囲(カンマ)や行・列全体は理解しない
    let ok = match plain.split_once(':') {
        Some((a, b)) => Pos::parse(a).is_some() && Pos::parse(b).is_some(),
        None => Pos::parse(&plain).is_some(),
    };
    ok.then_some((sheet, plain))
}

/// "A1" / "A1:B2" → "$A$1" / "$A$1:$B$2"
pub(super) fn dollars(r: &str) -> String {
    let one = |s: &str| -> String {
        let split = s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len());
        let (c, n) = s.split_at(split);
        format!("${c}${n}")
    };
    match r.split_once(':') {
        Some((a, b)) => format!("{}:{}", one(a), one(b)),
        None => one(r),
    }
}

/// 原本の workbook.xml の definedNames を、こちらの塊に置き換える。
/// 原文の workbook.xml の <sheet> に state="hidden" を差し替える。
/// **知らない属性は残す** — 名前・sheetId・r:id はそのまま。
/// 原本の workbook.xml の計算方法(calcPr calcMode)をこちらのモデルに合わせる。
/// 他の属性(calcId 等)は据え置く。calcPr が無い原本に手動を書くときは
/// definedNames の後(スキーマの順)に差し込む
/// calcPr の1本(新規保存)。手動と反復のどちらも無ければ空
pub(super) fn calc_pr_xml(book: &Book) -> String {
    let mut attrs = String::new();
    if book.calc_manual {
        attrs.push_str(r#" calcMode="manual""#);
    }
    if let Some((n, d)) = book.calc_iter {
        attrs.push_str(&format!(r#" iterate="1" iterateCount="{n}" iterateDelta="{d}""#));
    }
    if book.r1c1 {
        attrs.push_str(r#" refMode="R1C1""#);
    }
    if attrs.is_empty() { String::new() } else { format!("<calcPr{attrs}/>") }
}

/// calcPr の iterate 系3属性を差し替える(付ける/外す)。
/// calcPr の refMode 属性を差し替える(付ける/外す)。
pub(super) fn patch_refmode(tag: &str, r1c1: bool) -> String {
    let mut t = tag.to_string();
    while let Some(a) = t.find(" refMode=\"") {
        let vstart = a + " refMode=\"".len();
        let Some(vend) = t[vstart..].find('"') else { break };
        t.replace_range(a..vstart + vend + 1, "");
    }
    if r1c1 {
        if let Some(stripped) = t.strip_suffix("/>") {
            t = format!("{stripped} refMode=\"R1C1\"/>");
        } else if let Some(stripped) = t.strip_suffix('>') {
            t = format!("{stripped} refMode=\"R1C1\">");
        }
    }
    t
}

pub(super) fn patch_iterate(tag: &str, iter: Option<(u32, f64)>) -> String {
    let mut t = tag.to_string();
    for name in ["iterate", "iterateCount", "iterateDelta"] {
        while let Some(a) = t.find(&format!(" {name}=\"")) {
            let vstart = a + name.len() + 3;
            let Some(vend) = t[vstart..].find('"') else { break };
            t.replace_range(a..vstart + vend + 1, "");
        }
    }
    if let Some((n, d)) = iter {
        let ins = format!(r#" iterate="1" iterateCount="{n}" iterateDelta="{d}""#);
        if let Some(stripped) = t.strip_suffix("/>") {
            t = format!("{stripped}{ins}/>");
        } else if let Some(stripped) = t.strip_suffix('>') {
            t = format!("{stripped}{ins}>");
        }
    }
    t
}

pub(super) fn patch_calc_pr(workbook: &str, manual: bool) -> String {
    let mode = if manual { "manual" } else { "auto" };
    if let Some(start) = workbook.find("<calcPr") {
        let Some(len) = workbook[start..].find('>') else { return workbook.into() };
        let tag = &workbook[start..start + len + 1];
        let new_tag = if let Some(a) = tag.find("calcMode=\"") {
            // 既にある calcMode の値だけ差し替える
            let vstart = a + "calcMode=\"".len();
            match tag[vstart..].find('"') {
                Some(vend) => format!("{}{}{}", &tag[..vstart], mode, &tag[vstart + vend..]),
                None => return workbook.into(),
            }
        } else if manual {
            tag.replacen("<calcPr", r#"<calcPr calcMode="manual""#, 1)
        } else {
            return workbook.into(); // calcMode 無し=自動。触らない
        };
        format!("{}{}{}", &workbook[..start], new_tag, &workbook[start + len + 1..])
    } else if manual {
        // calcPr が無い原本。definedNames の後(無ければ sheets の後)に差し込む
        let ins = r#"<calcPr calcMode="manual"/>"#;
        if let Some(p) = workbook.find("</definedNames>") {
            let at = p + "</definedNames>".len();
            format!("{}{}{}", &workbook[..at], ins, &workbook[at..])
        } else if let Some(p) = workbook.find("</sheets>") {
            let at = p + "</sheets>".len();
            format!("{}{}{}", &workbook[..at], ins, &workbook[at..])
        } else {
            workbook.into()
        }
    } else {
        workbook.into()
    }
}

/// workbook.xml の `<sheet>` から `r:id` を**並び順に**拾う。
/// 書き出しで「どの部品がどのシートの物か」を読みと同じ道理で解くのに使う
pub(super) fn sheet_rids(xml: &str) -> Vec<Option<String>> {
    let mut r = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sheet" => {
                out.push(attr(&e, "id"));
            }
            _ => {}
        }
        buf.clear();
    }
    out
}

/// 原本のブックの rels を、書き出す番号へ向け直す。
///
/// 本体は `<sheet>` の並び順に `sheet1..N` と振り直して書くので、
/// **原本の的を持ち越すと `<sheet>` が別の部品を指す** — 消した跡や
/// 並べ替えで、rId の順と部品の番号は離れているため。
/// `rids` は原本の `<sheet>` の並び順の `r:id`
pub(super) fn patch_book_rels(rels: &str, rids: &[Option<String>], n_sheets: usize) -> String {
    let mut inner = String::new();
    for (id, ty, target, ext) in parse_rels(rels) {
        let at = rids.iter().take(n_sheets).position(|r| r.as_deref() == Some(id.as_str()));
        let (ty, target) = match at {
            // 書き出す並びの番号へ。中身も worksheet として書くので型も揃える
            Some(k) => (format!("{RNS}/worksheet"), format!("worksheets/sheet{}.xml", k + 1)),
            // `<sheet>` から指されていないシートの項は落とす。
            // 書き出す部品に無いものを残すと Excel が「修復」に入る
            None if ty.ends_with("/worksheet") || ty.ends_with("/chartsheet") => continue,
            None => (ty, target),
        };
        inner.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}"{}/>"#,
            esc(&id),
            esc(&ty),
            esc(&target),
            if ext { r#" TargetMode="External""# } else { "" }
        ));
    }
    // **共有文字列の関係は必ず要る。** こちらは必ず xl/sharedStrings.xml を
    // 書き、セルは t="s" の索引で字を指す — 関係が無いと、厳密な読み手
    // (openpyxl / lxml)は文字列の表を見つけられず添字が外れる。
    // openpyxl が作った原本には**この関係が無い**ので、持ち越しだけでは
    // 落ちる(2026-08-13、テーブルの検分で踏んだ)
    if !inner.contains("/sharedStrings\"") {
        let mut id = "rIdSS".to_string();
        let mut n = 2;
        while inner.contains(&format!("Id=\"{id}\"")) {
            id = format!("rIdSS{n}");
            n += 1;
        }
        inner.push_str(&format!(
            r#"<Relationship Id="{id}" Type="{RNS}/sharedStrings" Target="sharedStrings.xml"/>"#
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         {inner}</Relationships>"
    )
}

pub(super) fn patch_sheet_states(workbook: &str, book: &Book) -> String {
    let mut out = String::new();
    let mut rest = workbook;
    let mut i = 0usize;
    while let Some(p) = rest.find("<sheet ") {
        let Some(e) = rest[p..].find('>') else { break };
        let tag = &rest[p..p + e + 1];
        out.push_str(&rest[..p]);
        // 既存の state= を落として、必要なら付け直す
        let mut t = tag.to_string();
        while let Some(a) = t.find(" state=\"") {
            if let Some(b) = t[a + 8..].find('"') {
                t.replace_range(a..a + 8 + b + 1, "");
            } else {
                break;
            }
        }
        if book.sheets.get(i).map(|s| s.hidden).unwrap_or(false) {
            let cut = t.len() - if t.ends_with("/>") { 2 } else { 1 };
            t.insert_str(cut, " state=\"hidden\"");
        }
        out.push_str(&t);
        rest = &rest[p + e + 1..];
        i += 1;
    }
    out.push_str(rest);
    out
}

/// 読み取り専用のお願いを workbook.xml に織り込む(無ければ足し、
/// 外したら消す)。**鍵ではないので password は書かない** — 掛けた振りをしない
pub(super) fn patch_read_only(workbook: &str, on: bool) -> String {
    let mut s = workbook.to_string();
    // 既存の workbookProtection は取り除いてから置き直す
    if let Some(i) = s.find("<workbookProtection") {
        if let Some(j) = s[i..].find("/>") {
            s.replace_range(i..i + j + 2, "");
        } else if let Some(j) = s[i..].find("</workbookProtection>") {
            s.replace_range(i..i + j + "</workbookProtection>".len(), "");
        }
    }
    if !on {
        return s;
    }
    // 位置は fileVersion/workbookPr の後・bookViews の前(スキーマの並び)。
    // 手近な目印として <sheets> の前に置く
    match s.find("<bookViews").or_else(|| s.find("<sheets")) {
        Some(i) => {
            s.insert_str(i, r#"<workbookProtection readOnlyRecommended="1"/>"#);
            s
        }
        None => s,
    }
}

pub(super) fn patch_defined_names(workbook: &str, block: &str) -> String {
    let mut s = workbook.to_string();
    if let Some(i) = s.find("<definedNames>") {
        if let Some(j) = s[i..].find("</definedNames>") {
            s.replace_range(i..i + j + "</definedNames>".len(), "");
        }
    } else if let Some(i) = s.find("<definedNames/>") {
        s.replace_range(i..i + "<definedNames/>".len(), "");
    }
    if block.is_empty() {
        return s;
    }
    // 位置は sheets の直後(スキーマの並び)
    match s.find("</sheets>") {
        Some(i) => {
            let at = i + "</sheets>".len();
            s.insert_str(at, block);
            s
        }
        None => s,
    }
}

/// A1 を絶対参照($A$1)にする。Print_Area は絶対参照で書くのが通り相場。
pub(super) fn abs_a1(p: Pos) -> String {
    let a1 = p.a1();
    let split = a1.find(|c: char| c.is_ascii_digit()).unwrap_or(0);
    format!("${}${}", &a1[..split], &a1[split..])
}

/// 全シートの名前の定義 + 印刷範囲 + 理解しなかった原文を definedNames の塊にする。
pub(super) fn defined_names_xml(book: &Book) -> String {
    let mut inner = String::new();
    for raw in &book.names_raw {
        inner.push_str(raw);
    }
    // タイトル行・列(モデルが正)。両方あれば **, で並べる**(Excel の形。
    // 列が先 — Excel が書く順)
    for (i, sh) in book.sheets.iter().enumerate() {
        let name = sh.name.replace('\'', "''");
        let mut parts: Vec<String> = Vec::new();
        if let Some((a, b)) = sh.print_title_cols {
            // 列は字で($A:$B)。Pos の a1 から行番号を落として使う
            let letter = |c: u32| {
                let a1 = Pos::new(0, c).a1();
                a1.trim_end_matches(|ch: char| ch.is_ascii_digit()).to_string()
            };
            parts.push(format!("'{name}'!${}:${}", letter(a), letter(b)));
        }
        if let Some((a, b)) = sh.print_title_rows {
            parts.push(format!("'{name}'!${}:${}", a + 1, b + 1));
        }
        if !parts.is_empty() {
            inner.push_str(&format!(
                "<definedName name=\"_xlnm.Print_Titles\" localSheetId=\"{i}\">{}</definedName>",
                esc(&parts.join(","))
            ));
        }
    }
    // 印刷範囲(モデルが正)。シート名は常に引用符で包む(空白・記号に安全)
    for (i, sh) in book.sheets.iter().enumerate() {
        if sh.print_areas.is_empty() {
            continue;
        }
        let refs: Vec<String> = sh
            .print_areas
            .iter()
            .map(|(a, b)| {
                format!("'{}'!{}:{}", sh.name.replace('\'', "''"), abs_a1(*a), abs_a1(*b))
            })
            .collect();
        inner.push_str(&format!(
            "<definedName name=\"_xlnm.Print_Area\" localSheetId=\"{i}\">{}</definedName>",
            esc(&refs.join(","))
        ));
    }
    // 人が付けた名前。**同じ名前が2枚以上のシートにあるときだけ
    // localSheetId を付ける。**
    //
    // こちらのモデルは名前を「指す先のシート」に持たせていて、Excel の
    // 「適用範囲」(ブック全体 / このシートだけ)は持っていない。全部に
    // localSheetId を付けるとブック全体の名前がシート限定に落ちて、他の
    // シートの式が壊れる。逆に一つも付けないと、同じ名前が2枚にあるとき
    // **ブック全体の名前が2つ**になって開けないファイルになる。
    // 重なったときだけシート限定にするのが、どちらも壊さない線。
    let mut seen: std::collections::HashMap<&str, usize> = Default::default();
    for s in &book.sheets {
        for (n, _) in &s.names {
            *seen.entry(n.as_str()).or_insert(0) += 1;
        }
    }
    for (i, s) in book.sheets.iter().enumerate() {
        for (n, r) in &s.names {
            let scoped = seen.get(n.as_str()).copied().unwrap_or(0) > 1;
            let sid = if scoped { format!(" localSheetId=\"{i}\"") } else { String::new() };
            inner.push_str(&format!(
                "<definedName name=\"{}\"{}>'{}'!{}</definedName>",
                esc(n),
                sid,
                s.name.replace('\'', "''"),
                dollars(r)
            ));
        }
    }
    if inner.is_empty() {
        String::new()
    } else {
        format!("<definedNames>{inner}</definedNames>")
    }
}

/// 自己閉じ要素の属性を差し替える(無ければ足す)。他の属性は触らない。
pub(super) fn set_attr(el: &str, name: &str, value: &str) -> String {
    let pat = format!("{name}=\"");
    if let Some(i) = el.find(&pat) {
        let vstart = i + pat.len();
        if let Some(vend) = el[vstart..].find('"') {
            let mut out = String::with_capacity(el.len() + value.len());
            out.push_str(&el[..vstart]);
            out.push_str(value);
            out.push_str(&el[vstart + vend..]);
            return out;
        }
    }
    el.replacen("/>", &format!(" {name}=\"{value}\"/>"), 1)
}

/// 印刷まわりの塊(pageMargins → pageSetup → drawing の順 = schema の順)。
/// 原文があれば**属性だけ差し替える**(拡大縮小など知らない属性を残す)。
/// 無ければモデルの値から最小の要素を作る。
pub(super) fn print_extra_xml(orig: &str, sh: &Sheet) -> String {
    let take = |pat: &str| -> Option<String> {
        let i = orig.find(pat)?;
        let j = orig[i..].find("/>")? + i + 2;
        Some(orig[i..j].to_string())
    };
    let inch = |mm: f32| format!("{:.5}", mm / 25.4);
    // printOptions(枠線・見出しの印刷)。モデルの真偽を原文へ織り込む
    let popts = {
        let el = take("<printOptions").unwrap_or_else(|| "<printOptions/>".to_string());
        let el = set_attr(&el, "gridLines", if sh.print_gridlines { "1" } else { "0" });
        let el = set_attr(&el, "headings", if sh.print_headings { "1" } else { "0" });
        if !sh.print_gridlines && !sh.print_headings && !orig.contains("<printOptions") {
            None
        } else {
            Some(el)
        }
    };
    let margins = match (sh.margins_mm, take("<pageMargins")) {
        (Some((l, r, t, b)), Some(el)) => {
            let el = set_attr(&el, "left", &inch(l));
            let el = set_attr(&el, "right", &inch(r));
            let el = set_attr(&el, "top", &inch(t));
            Some(set_attr(&el, "bottom", &inch(b)))
        }
        (Some((l, r, t, b)), None) => Some(format!(
            "<pageMargins left=\"{}\" right=\"{}\" top=\"{}\" bottom=\"{}\" header=\"0.3\" footer=\"0.3\"/>",
            inch(l), inch(r), inch(t), inch(b)
        )),
        (None, el) => el,
    };
    let setup = {
        let orig_el = take("<pageSetup");
        if !sh.landscape && sh.paper_size.is_none() && sh.print_scale.is_none()
            && sh.fit_to_w.is_none() && sh.fit_to_h.is_none()
            && orig_el.is_none()
        {
            None
        } else {
            let el = orig_el.unwrap_or_else(|| "<pageSetup/>".to_string());
            let el = set_attr(
                &el,
                "orientation",
                if sh.landscape { "landscape" } else { "portrait" },
            );
            let el = match sh.paper_size {
                Some(c) => set_attr(&el, "paperSize", &c.to_string()),
                None => el,
            };
            let el = match sh.print_scale {
                Some(sc) => set_attr(&el, "scale", &sc.to_string()),
                None => el,
            };
            // 紙 N 枚に収める。**片方だけ指定でも両方書く**(0 = 合わせない)
            // — 書かないと読み手の既定(1)が効いて意図しない縮小になる
            let el = if sh.fit_to_w.is_some() || sh.fit_to_h.is_some() {
                let el = set_attr(&el, "fitToWidth", &sh.fit_to_w.unwrap_or(0).to_string());
                set_attr(&el, "fitToHeight", &sh.fit_to_h.unwrap_or(0).to_string())
            } else {
                el
            };
            Some(el)
        }
    };
    let mut out = String::new();
    if let Some(po) = popts {
        out.push_str(&po);
    }
    if let Some(m) = margins {
        out.push_str(&m);
    }
    if let Some(su) = setup {
        out.push_str(&su);
    }
    // 印刷のヘッダー/フッター(schema では pageSetup の後・rowBreaks の前)。
    // **奇数・偶数・先頭頁の別も書く** — 持たずに落としていたころは、
    // 左右で綴じる帳票を開いて保存すると偶数頁の組みが消えていた(2026-08-13)
    if sh.header.is_some()
        || sh.footer.is_some()
        || sh.header_even.is_some()
        || sh.footer_even.is_some()
        || sh.header_first.is_some()
        || sh.footer_first.is_some()
    {
        let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;");
        let mut attrs = String::new();
        if sh.hf_diff_odd_even {
            attrs.push_str(" differentOddEven=\"1\"");
        }
        if sh.hf_diff_first {
            attrs.push_str(" differentFirst=\"1\"");
        }
        out.push_str(&format!("<headerFooter{attrs}>"));
        // 並びはスキーマ(CT_HeaderFooter)の順: odd → even → first
        for (tag, v) in [
            ("oddHeader", &sh.header),
            ("oddFooter", &sh.footer),
            ("evenHeader", &sh.header_even),
            ("evenFooter", &sh.footer_even),
            ("firstHeader", &sh.header_first),
            ("firstFooter", &sh.footer_first),
        ] {
            if let Some(t) = v {
                out.push_str(&format!("<{tag}>{}</{tag}>", esc(t)));
            }
        }
        out.push_str("</headerFooter>");
    }
    // 改ページ(モデルが正。原文の rowBreaks は読みでモデルへ入っている)
    if !sh.row_breaks.is_empty() {
        let mut sorted = sh.row_breaks.clone();
        sorted.sort_unstable();
        sorted.dedup();
        out.push_str(&format!(
            r#"<rowBreaks count="{}" manualBreakCount="{}">"#,
            sorted.len(),
            sorted.len()
        ));
        for r in sorted {
            out.push_str(&format!(r#"<brk id="{r}" max="16383" man="1"/>"#));
        }
        out.push_str("</rowBreaks>");
    }
    // 縦の改ページ(schema では rowBreaks の後)
    if !sh.col_breaks.is_empty() {
        let mut sorted = sh.col_breaks.clone();
        sorted.sort_unstable();
        sorted.dedup();
        out.push_str(&format!(
            r#"<colBreaks count="{}" manualBreakCount="{}">"#,
            sorted.len(),
            sorted.len()
        ));
        for c in sorted {
            out.push_str(&format!(r#"<brk id="{c}" max="1048575" man="1"/>"#));
        }
        out.push_str("</colBreaks>");
    }
    if let Some(d) = take("<drawing") {
        out.push_str(&d);
    }
    out
}

pub(super) const CORE_REL: &str = r#"<Relationship Id="rIdCore" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>"#;

pub(super) const CORE_XML_EMPTY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"></cp:coreProperties>";

/// core.xml の1つのタグを差し替える(無ければ足す)。原文の他の欄は残す。
///
/// **元の開きタグの属性は保つ。** openpyxl は `xmlns:dc` を根ではなく
/// **要素自身に**宣言する(`<dc:creator xmlns:dc="…">`)— 属性ごと
/// 作り直すと接頭辞の宣言が消え、厳密な読み手(lxml)が開けない
/// 壊れた XML になる(2026-08-13、1904 の適合検査で発覚)。
pub(super) fn set_core_tag(s: &str, tag: &str, val: &str) -> String {
    let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    if let Some(i) = s.find(&open) {
        let rest = &s[i..];
        let Some(gt) = rest.find('>') else { return s.to_string() };
        let selfclosed = gt > 0 && rest.as_bytes()[gt - 1] == b'/';
        let attrs_end = if selfclosed { gt - 1 } else { gt };
        let attrs = rest[open.len()..attrs_end].trim_end();
        let sep = if attrs.is_empty() { "" } else { " " };
        let repl = if val.is_empty() {
            format!("<{tag}{sep}{attrs}/>")
        } else {
            format!("<{tag}{sep}{attrs}>{}</{tag}>", esc(val))
        };
        if selfclosed {
            return format!("{}{}{}", &s[..i], repl, &rest[gt + 1..]);
        }
        if let Some(c) = rest.find(&close) {
            return format!("{}{}{}", &s[..i], repl, &rest[c + close.len()..]);
        }
        s.to_string()
    } else if let Some(i) = s.rfind("</cp:coreProperties>") {
        // 足すとき: 根に接頭辞の宣言が無い core.xml(openpyxl 産)もあるので、
        // dc: の欄は要素自身に宣言を付けて足す(openpyxl と同じ流儀)
        let decl = if tag.starts_with("dc:") {
            r#" xmlns:dc="http://purl.org/dc/elements/1.1/""#
        } else {
            ""
        };
        let repl = if val.is_empty() {
            format!("<{tag}{decl}/>")
        } else {
            format!("<{tag}{decl}>{}</{tag}>", esc(val))
        };
        format!("{}{}{}", &s[..i], repl, &s[i..])
    } else {
        s.to_string()
    }
}

pub(super) const CUSTOM_REL: &str = r#"<Relationship Id="rIdCustom" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties" Target="docProps/custom.xml"/>"#;

pub(super) const CUSTOM_CT: &str = r#"<Override PartName="/docProps/custom.xml" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/>"#;

/// `.rels` から、その先を指す `<Relationship …/>` を取り除く。
/// 部品を消したのに関係が残ると、包みが「無い先」を指して壊れる。
pub(super) fn drop_rel_to(rels: &str, target: &str) -> String {
    let mut s = rels.to_string();
    loop {
        let Some(hit) = s.find(&format!("Target=\"{target}\"")) else { return s };
        // その属性を抱えている <Relationship … /> の頭と尻
        let Some(a) = s[..hit].rfind("<Relationship") else { return s };
        let Some(b) = s[hit..].find("/>").map(|k| hit + k + 2) else { return s };
        s.replace_range(a..b, "");
    }
}

/// カスタムプロパティを1つの文字列に繋ぐ(`dc:creator` の中身)。
///
/// **Excel は `;` で継ぐ。** 名前そのものに `;` が入っていたら区切りと
/// 見分けが付かないので、繋ぐ前に落とす — 開き直したときに1人が2人に
/// 化ける方が、記号が1つ消えるより悪い。
pub(super) fn join_creators(v: &[String]) -> String {
    v.iter()
        .map(|s| s.replace(';', " ").trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// `docProps/custom.xml` をモデルから組む。
///
/// `fmtid` はこの部品に規格が定めた1つの値。`pid` は **2から連番** —
/// 0と1は予約で、飛ばすと読み手が拒む。`linkTarget` は原本のまま返す。
pub(super) fn custom_props_xml(props: &[crate::model::CustomProp]) -> String {
    use crate::model::CustomVal;
    let esc = |t: &str| {
        t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    };
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/custom-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\">",
    );
    for (i, p) in props.iter().enumerate() {
        let (tag, val) = match &p.value {
            CustomVal::Text(t) => ("lpwstr".to_string(), esc(t)),
            // 数は**丸めずそのまま**。整数は小数点を付けない綴りで出す
            CustomVal::Number(n) => ("r8".to_string(), format!("{n}")),
            CustomVal::Date(d) => ("filetime".to_string(), esc(d)),
            CustomVal::Bool(b) => ("bool".to_string(), if *b { "true" } else { "false" }.into()),
            CustomVal::Other(t, v) => (t.clone(), esc(v)),
        };
        let link = match &p.link {
            Some(t) => format!(r#" linkTarget="{}""#, esc(t)),
            None => String::new(),
        };
        s.push_str(&format!(
            r#"<property fmtid="{{D5CDD505-2E9C-101B-9397-08002B2CF9AE}}" pid="{}" name="{}"{link}><vt:{tag}>{val}</vt:{tag}></property>"#,
            i + 2,
            esc(&p.name)
        ));
    }
    s.push_str("</Properties>");
    s
}

/// docProps/core.xml をブックの情報で差し替える。
pub(super) fn patch_core_props(orig: &str, p: &crate::model::BookProps) -> String {
    let creator = join_creators(&p.creators);
    let mut s = orig.to_string();
    for (tag, v) in [
        ("dc:creator", &creator),
        ("dc:title", &p.title),
        ("dc:subject", &p.subject),
        ("cp:keywords", &p.keywords),
        ("dc:description", &p.description),
    ] {
        s = set_core_tag(&s, tag, v);
    }
    s
}

/// **書いた xlsx を型紙(XLTX)に仕立て直す。**
///
/// 中身は xlsx と同じで、違うのは `[Content_Types].xml` の宣言ひとつだけ
/// (`...spreadsheetml.sheet.main+xml` → `...template.main+xml`)。
/// 開くと「この型紙から新しいブック」になる。
///
/// 書き手を二重に持たないよう、**出来上がった zip を作り直す**形にした。
pub fn to_template(bytes: &[u8]) -> Result<Vec<u8>, String> {
    const FROM: &str = "spreadsheetml.sheet.main+xml";
    const TO: &str = "spreadsheetml.template.main+xml";
    let mut zin = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut zout = zip::ZipWriter::new(&mut out);
        let opts: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut swapped = false;
        for i in 0..zin.len() {
            let mut f = zin.by_index(i).map_err(|e| e.to_string())?;
            let name = f.name().to_string();
            let mut buf = Vec::new();
            std::io::copy(&mut f, &mut buf).map_err(|e| e.to_string())?;
            if name == "[Content_Types].xml" {
                let t = String::from_utf8_lossy(&buf).to_string();
                if t.contains(FROM) {
                    swapped = true;
                }
                buf = t.replace(FROM, TO).into_bytes();
            }
            zout.start_file(name, opts).map_err(|e| e.to_string())?;
            zout.write_all(&buf).map_err(|e| e.to_string())?;
        }
        if !swapped {
            // **黙って xlsx のまま出さない。** 宣言を書き換えられなければ
            // 型紙ではないので、そう言って止める
            return Err("型紙の宣言が見つかりません(xlsx の作りが変わった?)".into());
        }
        zout.finish().map_err(|e| e.to_string())?;
    }
    Ok(out.into_inner())
}

pub fn write<W: Write + Seek>(book: &Book, dst: W) -> Result<(), String> {
    write_with(book, None::<std::io::Cursor<Vec<u8>>>, dst)
}

/// 保存する。`original` に開いた元のファイルを渡すと、こちらが作り直す部品
/// (シート・共有文字列・書式)以外 — **図形・テーマ・印刷設定・文書情報** —
/// を原本から持ち越す。渡さないと消える。
///
/// calcChain.xml だけは意図して捨てる(位置が古いままだと Excel が
/// 誤った計算順で開くことがある。無ければ Excel が作り直す)。
pub fn write_with<R: Read + Seek, W: Write + Seek>(
    book: &Book,
    original: Option<R>,
    dst: W,
) -> Result<(), String> {
    // 原本の部品と、各シートの引き継ぎ要素(印刷まわり・図形)を先に読む
    let mut carried: Vec<(String, Vec<u8>)> = Vec::new();
    let mut sheet_extras: Vec<String> = Vec::new();
    // [Content_Types] とシートの rels は「そのまま」ではなく、
    // リンク・コメントのぶんを織り込んで作り直す
    let mut orig_ct: Option<String> = None;
    let mut orig_sheet_rels: Vec<Option<String>> = Vec::new();
    let mut orig_styles: Option<String> = None;
    if let Some(src) = original {
        if let Ok(mut z) = zip::ZipArchive::new(src) {
            // **どの部品がどのシートの物かは r:id で解く**(読みと同じ道理)。
            // 部品の番号は `<sheet>` の並びとは一致しない。ここを部品の番号で
            // 数えていたので、引き継ぐ印刷設定と図形が別のシートへ付いていた
            let mut orig_wb = String::new();
            if let Ok(mut f) = z.by_name("xl/workbook.xml") {
                let _ = f.read_to_string(&mut orig_wb);
            }
            let mut orig_wb_rels = String::new();
            if let Ok(mut f) = z.by_name("xl/_rels/workbook.xml.rels") {
                let _ = f.read_to_string(&mut orig_wb_rels);
            }
            let orig_rids = sheet_rids(&orig_wb);
            let orig_parts: Vec<String> = {
                let entries: Vec<String> = (0..z.len())
                    .filter_map(|i| z.by_index(i).ok().map(|f| f.name().to_string()))
                    .collect();
                let mut parts: Vec<String> = entries
                    .iter()
                    .filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
                    .cloned()
                    .collect();
                parts.sort_by(|a, b| {
                    sheet_part_no(a).cmp(&sheet_part_no(b)).then_with(|| a.cmp(b))
                });
                let rels: Vec<(String, String)> = parse_rels(&orig_wb_rels)
                    .into_iter()
                    .filter(|(id, _, _, ext)| !id.is_empty() && !ext)
                    .map(|(id, _, target, _)| (id, resolve_book_target(&target)))
                    .collect();
                sheet_parts(&orig_rids, &rels, &entries, &parts)
            };
            // 部品名 → そのシートの並び順
            let book_at = |part: &str| orig_parts.iter().position(|p| p == part);
            for i in 0..z.len() {
                let Ok(mut f) = z.by_index(i) else { continue };
                let name = f.name().to_string();
                let regenerated = name.starts_with("xl/worksheets/sheet")
                    && name.ends_with(".xml")
                    || name == "xl/sharedStrings.xml"
                    || name == "xl/styles.xml"
                    || name == "xl/calcChain.xml"
                    // コメントの部品はこちらが作り直す
                    || name.starts_with("xl/comments")
                    // スレッドと人の一覧もモデルが正。**古い物を残すと
                    // 新しい写しと食い違う**(この節が直したかった穴そのもの)
                    || name.starts_with("xl/threadedComments/")
                    || name.starts_with("xl/persons/")
                    || name.starts_with("xl/drawings/vmlDrawing");
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_err() {
                    continue;
                }
                if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
                    // シート本体は作り直すが、印刷まわりと図形の参照は引き継ぐ
                    let s = String::from_utf8_lossy(&buf);
                    let mut extra = String::new();
                    for pat in ["<printOptions", "<pageMargins", "<pageSetup", "<drawing"] {
                        if let Some(i) = s.find(pat) {
                            if let Some(j) = s[i..].find("/>") {
                                extra.push_str(&s[i..i + j + 2]);
                            }
                        }
                    }
                    // 部品の番号ではなく**このシートの並び順**へ入れる
                    if let Some(k) = book_at(&name) {
                        while sheet_extras.len() <= k {
                            sheet_extras.push(String::new());
                        }
                        sheet_extras[k] = extra;
                    }
                }
                if name == "xl/workbook.xml" {
                    // 名前の定義はこちらの帳簿(モデル+原文持ち越し)が正。
                    // 原本の definedNames を置き換えて持ち越す
                    let s = String::from_utf8_lossy(&buf).to_string();
                    let patched = patch_defined_names(&s, &defined_names_xml(book));
                    // 隠しシートの state はこちらのモデルが正(原文へ属性差し替え)
                    let patched = patch_sheet_states(&patched, book);
                    // 計算方法もこちらが正(F9 で手動にしたら残す)
                    let patched = patch_calc_pr(&patched, book.calc_manual);
                    // 読み取り専用のお願い(鍵ではない)
                    let patched = patch_read_only(&patched, book.read_only_rec);
                    // 反復計算も原本の calcPr に織り込む(無ければ足し、切っていれば外す)
                    let patched = if let Some(start) = patched.find("<calcPr") {
                        match patched[start..].find('>') {
                            Some(len) => {
                                let tag = &patched[start..start + len + 1];
                                format!(
                                    "{}{}{}",
                                    &patched[..start],
                                    patch_refmode(&patch_iterate(tag, book.calc_iter), book.r1c1),
                                    &patched[start + len + 1..]
                                )
                            }
                            None => patched,
                        }
                    } else if book.calc_iter.is_some() || book.r1c1 {
                        // calcPr が無い原本に反復だけ足す(manual と同じ差し込み場所)
                        let ins = calc_pr_xml(book);
                        if let Some(p) = patched.find("</definedNames>") {
                            let at = p + "</definedNames>".len();
                            format!("{}{}{}", &patched[..at], ins, &patched[at..])
                        } else if let Some(p) = patched.find("</sheets>") {
                            let at = p + "</sheets>".len();
                            format!("{}{}{}", &patched[..at], ins, &patched[at..])
                        } else {
                            patched
                        }
                    } else {
                        patched
                    };
                    carried.push((name, patched.into_bytes()));
                    continue;
                }
                if name == "xl/theme/theme1.xml" {
                    continue; // テーマの色はモデルが正(配色の変更が効く)
                }
                if name == "xl/styles.xml" {
                    // 原本の書式表は**据え置き合成の土台**として持つ
                    // (作り直すと、読みで拾えない書式が消える — 発注者 2026-08-06)
                    orig_styles = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                if name.starts_with("xl/tables/") {
                    continue; // 表オブジェクトはモデルから作り直す
                }
                if name == "docProps/core.xml" {
                    // ブックの情報はこちらのモデルが正。原文の他の欄は残す
                    let s = String::from_utf8_lossy(&buf).to_string();
                    carried.push((name, patch_core_props(&s, &book.props).into_bytes()));
                    continue;
                }
                if name == "docProps/custom.xml" {
                    continue; // カスタムプロパティはモデルから作り直す(下)
                }
                if name == "[Content_Types].xml" {
                    orig_ct = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                if let Some(base) = name
                    .strip_prefix("xl/worksheets/_rels/")
                    .and_then(|r| r.strip_suffix(".rels"))
                {
                    // この rels の持ち主の部品を、並び順へ直して置く
                    if let Some(k) = book_at(&format!("xl/worksheets/{base}")) {
                        while orig_sheet_rels.len() <= k {
                            orig_sheet_rels.push(None);
                        }
                        orig_sheet_rels[k] = Some(String::from_utf8_lossy(&buf).to_string());
                    }
                    continue;
                }
                if name == "xl/_rels/workbook.xml.rels" {
                    // 本体は並び順の番号で書き出すので、的もそこへ向け直す
                    let s = String::from_utf8_lossy(&buf).to_string();
                    let fixed = patch_book_rels(&s, &orig_rids, book.sheets.len());
                    carried.push((name, fixed.into_bytes()));
                    continue;
                }
                if !regenerated {
                    carried.push((name, buf));
                }
            }
        }
    }

    let mut zip = zip::ZipWriter::new(dst);
    let o: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 共有文字列を集める(ふりがなも添える — 落とすと日本語の宝が消える)。
    // 同じ字で違う読みの2セルは、先に出た読みで代表(表は字で引くため)
    let mut shared: Vec<String> = Vec::new();
    let mut shared_ruby: Vec<Option<String>> = Vec::new();
    let mut idx = std::collections::HashMap::new();
    for sh in &book.sheets {
        for (p, c) in &sh.cells {
            if let Value::Text(t) = &c.value {
                let ruby = sh.phonetics.get(p);
                match idx.get(t) {
                    None => {
                        idx.insert(t.clone(), shared.len());
                        shared.push(t.clone());
                        shared_ruby.push(ruby.cloned());
                    }
                    Some(&i) => {
                        if shared_ruby[i].is_none() {
                            shared_ruby[i] = ruby.cloned();
                        }
                    }
                }
            }
        }
    }

    // 原本の書式表を読み戻す(据え置き合成の照合用)。
    // 読みと同じ関数で解くので、読みで拾えない書式も**同じように**落ち、
    // 「触っていないセル」は必ず一致して原本の索引のまま書き戻る
    let orig_fmts: Option<Vec<crate::model::CellFormat>> =
        orig_styles.as_ref().map(|xml| crate::styles::parse(xml, &book.theme));
    // このセルは原本の書式のままか(なら索引ごと据え置く)
    let kept_style = |sh: &Sheet, p: &Pos, fmt: &crate::model::CellFormat| -> Option<u32> {
        let fmts = orig_fmts.as_ref()?;
        let i = *sh.style_of.get(p)?;
        (fmts.get(i as usize)? == fmt).then_some(i)
    };
    // 使われている書式を集めて表にする(据え置きのセルは除く)。
    // 索引を <c s="…"> に配る
    let used: Vec<crate::model::CellFormat> = {
        let mut v = Vec::new();
        for sh in &book.sheets {
            for (p, c) in &sh.cells {
                if kept_style(sh, p, &c.fmt).is_some() {
                    continue;
                }
                if !c.fmt.is_plain() && !v.contains(&c.fmt) {
                    v.push(c.fmt.clone());
                }
            }
        }
        v
    };
    let (styles_xml, style_idx) = match orig_styles
        .as_ref()
        .and_then(|xml| crate::styles::append_to(xml, &used, &book.named_styles_new))
    {
        Some(r) => r,
        // 原本が無い(新規)か、節の見つからない styles.xml なら作り直し
        None => crate::styles::build(&used, &book.named_styles_new),
    };
    // 条件付き書式の見た目(dxfs)。全シートの規則から集めて番号を振る
    let dxf_list: Vec<crate::model::CondLook> = {
        let mut v: Vec<crate::model::CondLook> = Vec::new();
        for sh in &book.sheets {
            for r in &sh.cond {
                if !v.contains(&r.look) {
                    v.push(r.look.clone());
                }
            }
        }
        v
    };
    let styles_xml = if dxf_list.is_empty() {
        styles_xml
    } else {
        let mut dx = format!("<dxfs count=\"{}\">", dxf_list.len());
        for look in &dxf_list {
            dx.push_str("<dxf>");
            // font の中身は**順番が決まっている**(b, i, strike, u, color)。
            // 並べ替えると Excel が styles.xml ごと撥ねる
            let f = |on: Option<bool>, tag: &str| match on {
                Some(true) => format!("<{tag}/>"),
                Some(false) => format!("<{tag} val=\"0\"/>"),
                None => String::new(),
            };
            let font = format!(
                "{}{}{}{}{}",
                f(look.bold, "b"),
                f(look.italic, "i"),
                f(look.strike, "strike"),
                match look.underline {
                    Some(true) => "<u/>".to_string(),
                    Some(false) => "<u val=\"none\"/>".to_string(),
                    None => String::new(),
                },
                look.color
                    .as_ref()
                    .map(|c| format!("<color rgb=\"FF{c}\"/>"))
                    .unwrap_or_default(),
            );
            if !font.is_empty() {
                dx.push_str(&format!("<font>{font}</font>"));
            }
            if let Some(f) = &look.fill {
                dx.push_str(&format!(
                    "<fill><patternFill><bgColor rgb=\"FF{f}\"/></patternFill></fill>"
                ));
            }
            dx.push_str("</dxf>");
        }
        dx.push_str("</dxfs>");
        let mut s = styles_xml;
        // 原本に dxfs の節があれば置き換える(二重の節は不正)。無ければ挿す
        if let Some(i) = s.find("<dxfs") {
            let end = match s[i..].find("</dxfs>") {
                Some(j) => i + j + "</dxfs>".len(),
                // <dxfs count="0"/> の自己完結形
                None => i + s[i..].find("/>").map(|j| j + 2).unwrap_or(0),
            };
            if end > i {
                s.replace_range(i..end, &dx);
            }
        } else if let Some(p) = s.rfind("</styleSheet>") {
            s.insert_str(p, &dx);
        }
        s
    };

    let overrides: String = (1..=book.sheets.len())
        .map(|i| format!(r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#))
        .collect();
    // このアプリで挿した画像(グラフ)の部品。原本に drawing のあるシートは
    // **その部品の中へアンカーと rels を継ぎ足す**(drawing は1シート1部品の決まり)。
    // 無いシートは drawingC{N}.xml を新しく作る
    let mut media_out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut fresh_parts: Vec<(String, String)> = Vec::new();
    // 連番は原本にある imageC の続きから(前の保存で足した分と衝突しない)
    let mut media_n = 0usize;
    for (name, _) in &carried {
        if let Some(rest) = name.strip_prefix("xl/media/imageC") {
            if let Some(num) = rest.split('.').next().and_then(|v| v.parse::<usize>().ok()) {
                media_n = media_n.max(num);
            }
        }
    }
    for (i, sh) in book.sheets.iter().enumerate() {
        if sh.images_new.is_empty() && sh.shapes_new.is_empty() {
            continue;
        }
        let mut anchors = String::new();
        let mut rels_add = String::new();
        for (k, spn) in sh.shapes_new.iter().enumerate() {
            anchors.push_str(&shape_anchor_xml(spn, (i as u32) * 100 + k as u32 + 50));
        }
        for (k, im) in sh.images_new.iter().enumerate() {
            media_n += 1;
            let ext = if im.data.starts_with(&[0xFF, 0xD8]) {
                "jpeg"
            } else if im.data.starts_with(b"GIF8") {
                "gif"
            } else if im.data.starts_with(b"BM") {
                "bmp"
            } else {
                "png"
            };
            let _ = k;
            let rid = format!("rIdC{media_n}");
            media_out.push((format!("xl/media/imageC{media_n}.{ext}"), im.data.clone()));
            rels_add.push_str(&format!(
                r#"<Relationship Id="{rid}" Type="{RNS}/image" Target="../media/imageC{media_n}.{ext}"/>"#
            ));
            anchors.push_str(&image_anchor_xml(im, &rid, (i as u32) * 100 + k as u32 + 2));
        }
        let orig_target = orig_sheet_rels.get(i).cloned().flatten().and_then(|onr| {
            parse_rels(&onr)
                .into_iter()
                .find(|(_, ty, _, _)| ty.ends_with("/drawing"))
                .map(|(_, _, t, _)| resolve_target(&t))
        });
        match orig_target {
            Some(dpath) => {
                for (name, buf) in carried.iter_mut() {
                    if *name == dpath {
                        let mut xml = String::from_utf8_lossy(buf).to_string();
                        if let Some(p) = xml.rfind("</xdr:wsDr>") {
                            xml.insert_str(p, &anchors);
                            *buf = xml.into_bytes();
                        }
                    }
                }
                let drels = {
                    let (dir, base) = dpath.rsplit_once('/').unwrap_or(("xl/drawings", &dpath));
                    format!("{dir}/_rels/{base}.rels")
                };
                let mut found = false;
                for (name, buf) in carried.iter_mut() {
                    if *name == drels {
                        let mut xml = String::from_utf8_lossy(buf).to_string();
                        if let Some(p) = xml.rfind("</Relationships>") {
                            xml.insert_str(p, &rels_add);
                            *buf = xml.into_bytes();
                        }
                        found = true;
                    }
                }
                if !found {
                    fresh_parts.push((drels, format!(
                        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels_add}</Relationships>"
                    )));
                }
            }
            None => {
                fresh_parts.push((
                    format!("xl/drawings/drawingC{}.xml", i + 1),
                    format!(
                        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">{anchors}</xdr:wsDr>"
                    ),
                ));
                fresh_parts.push((
                    format!("xl/drawings/_rels/drawingC{}.xml.rels", i + 1),
                    format!(
                        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels_add}</Relationships>"
                    ),
                ));
            }
        }
    }

    // ブックに載せた Python・ピボット・スピルの記録はモデルが正(古い部品は写さない)
    carried.retain(|(name, _)| {
        name != "xl/joPython.xml" && name != "xl/joPivot.xml" && name != "xl/joSpill.xml"
        && name != "xl/joChanges.xml"
    });
    let carry = !carried.is_empty();
    // ブックの情報。原本に core.xml が無い・新規ブックでも、書いた情報は残す
    let pr = &book.props;
    let props_any = !(pr.creators.is_empty()
        && pr.title.is_empty()
        && pr.subject.is_empty()
        && pr.keywords.is_empty()
        && pr.description.is_empty());
    let had_core = carried.iter().any(|(n, _)| n == "docProps/core.xml");
    let core_fresh = !had_core && props_any;
    if core_fresh {
        carried.push((
            "docProps/core.xml".to_string(),
            patch_core_props(CORE_XML_EMPTY, pr).into_bytes(),
        ));
        // 持ち越した .rels に core の関係が無ければ足す
        if let Some((_, buf)) = carried.iter_mut().find(|(n, _)| n == "_rels/.rels") {
            let s = String::from_utf8_lossy(buf).to_string();
            if !s.contains("core-properties") {
                if let Some(i) = s.rfind("</Relationships>") {
                    let mut s2 = s.clone();
                    s2.insert_str(i, CORE_REL);
                    *buf = s2.into_bytes();
                }
            }
        }
    }
    // カスタムプロパティ(docProps/custom.xml)。**部品・宣言・関係の3つで
    // ひと組** — どれか1つでも欠けると、Excel は「修復しました」と言って
    // 開く(関係だけ残って部品が無い方が重い。無い先を指すので壊れた包み)。
    // 中身は上の持ち越しで一度落としてあるので、ここが唯一の書き手
    let custom_any = !book.props.custom.is_empty();
    if custom_any {
        carried.push((
            "docProps/custom.xml".to_string(),
            custom_props_xml(&book.props.custom).into_bytes(),
        ));
    }
    if let Some((_, buf)) = carried.iter_mut().find(|(n, _)| n == "_rels/.rels") {
        let mut s = String::from_utf8_lossy(buf).to_string();
        let had = s.contains("docProps/custom.xml");
        if custom_any && !had {
            if let Some(i) = s.rfind("</Relationships>") {
                s.insert_str(i, CUSTOM_REL);
            }
        } else if !custom_any && had {
            // 全部消したら関係も畳む。**空の部品を置いて誤魔化さない**
            s = drop_rel_to(&s, "docProps/custom.xml");
        }
        *buf = s.into_bytes();
    }
    for (name, buf) in &carried {
        zip.start_file(name.as_str(), o).map_err(|e| e.to_string())?;
        zip.write_all(buf).map_err(|e| e.to_string())?;
    }
    for (name, buf) in &media_out {
        zip.start_file(name.as_str(), o).map_err(|e| e.to_string())?;
        zip.write_all(buf).map_err(|e| e.to_string())?;
    }
    let mut put = |name: &str, data: &str| -> Result<(), String> {
        zip.start_file(name, o).map_err(|e| e.to_string())?;
        zip.write_all(data.as_bytes()).map_err(|e| e.to_string())
    };
    // [Content_Types]。コメントの部品を持つときは、その宣言も要る
    {
        let mut ct = match &orig_ct {
            Some(s) => s.clone(),
            None => CT.replace("__SHEETS__", &overrides),
        };
        // 表オブジェクトの宣言は作り直す(減ったときに空の宣言を残さない)
        while let Some(i) = ct.find(r#"<Override PartName="/xl/tables/"#) {
            if let Some(j) = ct[i..].find("/>") {
                ct.replace_range(i..i + j + 2, "");
            } else {
                break;
            }
        }
        // シート本体の宣言も作り直す。**原本の部品の番号は飛んでいることが
        // ある**(消した跡)が、こちらは並び順に sheet1..N と書き出すので、
        // 原本の宣言をそのまま持ち越すと有る部品が漏れ、無い部品を宣言する
        if orig_ct.is_some() {
            for pat in [
                r#"<Override PartName="/xl/worksheets/"#,
                r#"<Override PartName="/xl/chartsheets/"#,
            ] {
                while let Some(i) = ct.find(pat) {
                    match ct[i..].find("/>") {
                        Some(j) => ct.replace_range(i..i + j + 2, ""),
                        None => break,
                    }
                }
            }
            // 宣言の並び順は問われないので最後へ足す
            if let Some(p) = ct.rfind("</Types>") {
                ct.insert_str(p, &overrides);
            }
        }
        let n_tables: usize = book.sheets.iter().map(|s| s.tables.len()).sum();
        let has_comments = book.sheets.iter().any(|s| !s.comments.is_empty());
        let mut add = String::new();
        // 共有文字列の宣言。**必ず書く部品なので必ず宣言する** —
        // 原本(openpyxl 産など)に無いことがあり、持ち越しだけでは漏れる
        // (関係の側と対。2026-08-13)
        if !ct.contains("/xl/sharedStrings.xml") {
            add.push_str(r#"<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>"#);
        }
        for n in 1..=n_tables {
            add.push_str(&format!(
                r#"<Override PartName="/xl/tables/table{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>"#
            ));
        }
        if has_comments && !ct.contains("Extension=\"vml\"") {
            add.push_str(r#"<Default Extension="vml" ContentType="application/vnd.openxmlformats-officedocument.vmlDrawing"/>"#);
        }
        for (i, sh) in book.sheets.iter().enumerate() {
            let part = format!("/xl/comments{}.xml", i + 1);
            if !sh.comments.is_empty() && !ct.contains(&part) {
                add.push_str(&format!(r#"<Override PartName="{part}" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml"/>"#));
            }
            // スレッドの本体。**古い写しと同じ回で宣言する**
            let tpart = format!("/xl/threadedComments/threadedComment{}.xml", i + 1);
            if !sh.comments.is_empty() && !ct.contains(&tpart) {
                add.push_str(&format!(r#"<Override PartName="{tpart}" ContentType="application/vnd.ms-excel.threadedcomments+xml"/>"#));
            }
        }
        // 挿した画像の部品の宣言(絵の拡張子と、新しく作った drawing)
        if media_out.iter().any(|(n, _)| n.ends_with(".png")) && !ct.contains("Extension=\"png\"") {
            add.push_str(r#"<Default Extension="png" ContentType="image/png"/>"#);
        }
        if media_out.iter().any(|(n, _)| n.ends_with(".jpeg")) && !ct.contains("Extension=\"jpeg\"") {
            add.push_str(r#"<Default Extension="jpeg" ContentType="image/jpeg"/>"#);
        }
        if media_out.iter().any(|(n, _)| n.ends_with(".gif")) && !ct.contains("Extension=\"gif\"") {
            add.push_str(r#"<Default Extension="gif" ContentType="image/gif"/>"#);
        }
        if media_out.iter().any(|(n, _)| n.ends_with(".bmp")) && !ct.contains("Extension=\"bmp\"") {
            add.push_str(r#"<Default Extension="bmp" ContentType="image/bmp"/>"#);
        }
        for (name, _) in &fresh_parts {
            if name.starts_with("xl/drawings/drawingC") && name.ends_with(".xml") {
                add.push_str(&format!(
                    r#"<Override PartName="/{name}" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/>"#
                ));
            }
        }
        if !book.theme.is_empty() && !ct.contains("/xl/theme/theme1.xml") {
            add.push_str(r#"<Override PartName="/xl/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>"#);
        }
        if has_comments && !ct.contains("/xl/persons/person.xml") {
            add.push_str(r#"<Override PartName="/xl/persons/person.xml" ContentType="application/vnd.ms-excel.person+xml"/>"#);
        }
        if core_fresh && !ct.contains("core-properties") {
            add.push_str(r#"<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>"#);
        }
        // カスタムプロパティの宣言。**消えたときは宣言も畳む**(部品の無い
        // 宣言は Excel の修復を呼ぶ)。持ち越しでない CT にも同じ手で足りる
        let ct_has_custom = ct.contains("/docProps/custom.xml");
        if custom_any && !ct_has_custom {
            add.push_str(CUSTOM_CT);
        } else if !custom_any && ct_has_custom {
            if let Some(i) = ct.find(r#"<Override PartName="/docProps/custom.xml""#) {
                if let Some(j) = ct[i..].find("/>") {
                    ct.replace_range(i..i + j + 2, "");
                }
            }
        }
        if !add.is_empty() {
            if let Some(p) = ct.rfind("</Types>") {
                ct.insert_str(p, &add);
            }
        }
        put("[Content_Types].xml", &ct)?;
    }
    for (name, xml) in &fresh_parts {
        put(name, xml)?;
    }
    // **ブックには Python を一切書かない**(発注者確定 2026-08-09:
    // データとプログラムを1つのファイルにしない — xlsm の逆)。
    // 関数(UDF)も手続きも plugins の .py にある。古いブックから読んだ
    // コードは保存で消える(開くときの報告でそう言う。取り出しは @export)
    // 変更履歴(独自部品 xl/joChanges.xml)。Excel は読まない — 正直な劣化
    if !book.changes.is_empty() {
        let mut cx = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<joChanges>",
        );
        for c in &book.changes {
            cx.push_str(&format!(
                "<c who=\"{}\" when=\"{}\" sheet=\"{}\" at=\"{}\" before=\"{}\" after=\"{}\"/>",
                esc(&c.who), esc(&c.when), esc(&c.sheet), c.at.a1(),
                esc(&c.before), esc(&c.after)
            ));
        }
        cx.push_str("</joChanges>");
        put("xl/joChanges.xml", &cx)?;
    }
    if !book.pivots.is_empty() {
        let mut px = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<joPivot>",
        );
        for d in &book.pivots {
            px.push_str(&format!(
                "<pivot sheet=\"{}\" src=\"{}:{}\" dest=\"{}\" h=\"{}\" w=\"{}\" value=\"{}\" agg=\"{}\" totals=\"{}\" subtotals=\"{}\" blank=\"{}\" compact=\"{}\" style=\"{}\" name=\"{}\">",
                esc(&d.sheet),
                d.src.0.a1(),
                d.src.1.a1(),
                d.dest.a1(),
                d.size.0,
                d.size.1,
                esc(&d.value),
                esc(&d.agg),
                d.totals as u8,
                d.subtotals as u8,
                d.blank_rows as u8,
                d.compact as u8,
                esc(&d.style),
                esc(&d.name),
            ));
            for r in &d.rows_sel {
                px.push_str(&format!("<r>{}</r>", esc(r)));
            }
            for c in &d.cols_sel {
                px.push_str(&format!("<c>{}</c>", esc(c)));
            }
            for (f, vs) in &d.hide {
                px.push_str(&format!("<f name=\"{}\">", esc(f)));
                for v in vs {
                    px.push_str(&format!("<v>{}</v>", esc(v)));
                }
                px.push_str("</f>");
            }
            // 値のフィルターとグループ化(第2版)
            if let Some((op, th)) = &d.vfilter {
                px.push_str(&format!("<vf op=\"{}\" v=\"{}\"/>", esc(op), th));
            }
            for (f, unit) in &d.group_by {
                px.push_str(&format!("<g name=\"{}\" unit=\"{}\"/>", esc(f), esc(unit)));
            }
            if !d.sort.is_empty() {
                px.push_str(&format!("<so v=\"{}\"/>", esc(&d.sort)));
            }
            if !d.show_as.is_empty() {
                px.push_str(&format!("<sa v=\"{}\"/>", esc(&d.show_as)));
            }
            px.push_str("</pivot>");
        }
        px.push_str("</joPivot>");
        put("xl/joPivot.xml", &px)?;
    }
    if book.sheets.iter().any(|s| !s.spills.is_empty()) {
        let mut sx = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<joSpill>",
        );
        for s in &book.sheets {
            for (at, (h, w)) in &s.spills {
                sx.push_str(&format!(
                    "<s sheet=\"{}\" at=\"{}\" h=\"{h}\" w=\"{w}\"/>",
                    esc(&s.name),
                    at.a1()
                ));
            }
        }
        sx.push_str("</joSpill>");
        put("xl/joSpill.xml", &sx)?;
    }
    if !carry {
        let mut add = String::new();
        if core_fresh {
            add.push_str(CORE_REL);
        }
        if custom_any {
            add.push_str(CUSTOM_REL);
        }
        put("_rels/.rels", &RELS.replace("</Relationships>", &format!("{add}</Relationships>")))?;
    }

    let sheets_xml: String = book.sheets.iter().enumerate()
        .map(|(i, s)| format!(r#"<sheet name="{}" sheetId="{}"{} r:id="rId{}"/>"#,
                              esc(&s.name), i + 1,
                              if s.hidden { r#" state="hidden""# } else { "" },
                              i + 1))
        .collect();
    if !carry {
    put("xl/workbook.xml", &format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="{NS}" xmlns:r="{RNS}">{}<sheets>{sheets_xml}</sheets>{}{}</workbook>"#,
        // 読み取り専用のお願い(スキーマでは sheets の前)
        if book.read_only_rec { r#"<workbookProtection readOnlyRecommended="1"/>"# } else { "" },
        defined_names_xml(book),
        // 手動計算をファイルに残す(自動は既定なので書かない)
        calc_pr_xml(book).as_str()))?;

    let wrels: String = (1..=book.sheets.len())
        .map(|i| format!(r#"<Relationship Id="rId{i}" Type="{RNS}/worksheet" Target="worksheets/sheet{i}.xml"/>"#))
        .collect();
    put("xl/_rels/workbook.xml.rels", &format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{wrels}<Relationship Id="rIdSS" Type="{RNS}/sharedStrings" Target="sharedStrings.xml"/><Relationship Id="rIdST" Type="{RNS}/styles" Target="styles.xml"/><Relationship Id="rIdTH" Type="{RNS}/theme" Target="theme/theme1.xml"/></Relationships>"#))?;
    }

    // テーマの色。読んだものをそのまま返し、配色を変えたときは新しい組を書く
    if !book.theme.is_empty() {
        put("xl/theme/theme1.xml", &crate::theme::to_xml(&book.theme))?;
    }
    put("xl/styles.xml", &styles_xml)?;

    let si: String = shared
        .iter()
        .zip(&shared_ruby)
        .map(|(s, ruby)| match ruby {
            Some(r) => format!(
                "<si><t xml:space=\"preserve\">{}</t>\
                 <rPh sb=\"0\" eb=\"{}\"><t>{}</t></rPh>\
                 <phoneticPr fontId=\"0\"/></si>",
                esc(s),
                s.chars().count(),
                esc(r)
            ),
            None => format!("<si><t xml:space=\"preserve\">{}</t></si>", esc(s)),
        })
        .collect();
    put("xl/sharedStrings.xml", &format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="{NS}" count="{}" uniqueCount="{}">{si}</sst>"#, shared.len(), shared.len()))?;

    // コメントを書いた人の一覧。**ブックに1つ**(xl/persons/person.xml)。
    // シートを回る前に集めておく — 同じ人が複数のシートに書いていても1件
    let book_persons: Vec<String> = {
        let mut v: Vec<String> = Vec::new();
        for s in &book.sheets {
            for th in s.comments.values() {
                for e in &th.entries {
                    if !v.contains(&e.who) {
                        v.push(e.who.clone());
                    }
                }
            }
        }
        v
    };
    if !book_persons.is_empty() {
        let list: String = book_persons
            .iter()
            .enumerate()
            .map(|(k, name)| {
                format!(
                    r#"<person displayName="{}" id="{}" userId="{}" providerId="None"/>"#,
                    esc(name),
                    guid(900_000 + k),
                    esc(name)
                )
            })
            .collect();
        put("xl/persons/person.xml", &format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<personList xmlns="{TCNS}" xmlns:x="{NS}">{list}</personList>"#))?;
    }

    for (i, sh) in book.sheets.iter().enumerate() {
        let mut w = Writer::new(Cursor::new(Vec::new()));
        let mut ws = BytesStart::new("worksheet");
        ws.push_attribute(("xmlns", NS));
        ws.push_attribute(("xmlns:r", RNS));
        w.write_event(Event::Start(ws)).unwrap();
        // 耳(タブ)の色。schema では worksheet の先頭(sheetPr)
        if let Some(c) = &sh.tab_color {
            w.write_event(Event::Start(BytesStart::new("sheetPr"))).unwrap();
            let mut tc = BytesStart::new("tabColor");
            tc.push_attribute(("rgb", c.as_str()));
            w.write_event(Event::Empty(tc)).unwrap();
            w.write_event(Event::End(BytesEnd::new("sheetPr"))).unwrap();
        }
        // 画面の見え方(schema では sheetPr の次)。右から左・固定枠・格子線・
        // 倍率がここに集まる。**rtl のときだけ書いていたので固定枠を置く場所が
        // なかった** — 常に書き、中身は持っているものだけを載せる
        {
            w.write_event(Event::Start(BytesStart::new("sheetViews"))).unwrap();
            let mut sv = BytesStart::new("sheetView");
            if sh.rtl {
                sv.push_attribute(("rightToLeft", "1"));
            }
            // 読んだときだけ返す(None は原文に無かった = Excel の既定)。
            // 既定を書き足さないので、触っていない帳票の差分が増えない
            let b10 = |v: bool| if v { "1" } else { "0" };
            if let Some(v) = sh.show_formulas {
                sv.push_attribute(("showFormulas", b10(v)));
            }
            if let Some(v) = sh.show_gridlines {
                sv.push_attribute(("showGridLines", b10(v)));
            }
            if let Some(z) = sh.zoom_scale {
                sv.push_attribute(("zoomScale", z.to_string().as_str()));
            }
            sv.push_attribute(("workbookViewId", "0"));
            match sh.freeze {
                None => w.write_event(Event::Empty(sv)).unwrap(),
                Some(f) => {
                    w.write_event(Event::Start(sv)).unwrap();
                    let mut p = BytesStart::new("pane");
                    // **xSplit が列、ySplit が行。** 0 の側は書かない
                    // (Excel は書かない。書くと「0列を固定」の余計な指定になる)
                    if f.frozen_columns > 0 {
                        p.push_attribute(("xSplit", f.frozen_columns.to_string().as_str()));
                    }
                    if f.frozen_rows > 0 {
                        p.push_attribute(("ySplit", f.frozen_rows.to_string().as_str()));
                    }
                    // 止めた枠のすぐ右下のセル = 繰る側の左上
                    let tl = Pos::new(f.frozen_rows, f.frozen_columns);
                    p.push_attribute(("topLeftCell", tl.a1().as_str()));
                    // 動く側の枠。行だけ止めれば下、列だけ止めれば右、両方なら右下
                    p.push_attribute((
                        "activePane",
                        match (f.frozen_rows > 0, f.frozen_columns > 0) {
                            (true, true) => "bottomRight",
                            (true, false) => "bottomLeft",
                            _ => "topRight",
                        },
                    ));
                    p.push_attribute(("state", "frozen"));
                    w.write_event(Event::Empty(p)).unwrap();
                    w.write_event(Event::End(BytesEnd::new("sheetView"))).unwrap();
                }
            }
            w.write_event(Event::End(BytesEnd::new("sheetViews"))).unwrap();
        }
        // グループ化があるときは sheetFormatPr に深さの最大を書く
        // (Excel のアウトライン欄の 1 2 3 ボタンがこれを見る)。cols より前が作法
        if !sh.row_outline.is_empty() || !sh.col_outline.is_empty() {
            let mut fp = BytesStart::new("sheetFormatPr");
            fp.push_attribute(("defaultRowHeight", "15"));
            if let Some(m) = sh.row_outline.values().max() {
                fp.push_attribute(("outlineLevelRow", m.to_string().as_str()));
            }
            if let Some(m) = sh.col_outline.values().max() {
                fp.push_attribute(("outlineLevelCol", m.to_string().as_str()));
            }
            w.write_event(Event::Empty(fp)).unwrap();
        }
        // 列幅・列のグループ化。読んだものを返す(捨てると帳票の形が変わる)。
        // 同じ指定が並ぶ区間は1つの col にまとめる
        if !sh.col_width.is_empty()
            || sh.default_col_width.is_some()
            || !sh.col_outline.is_empty()
            || !sh.col_hidden.is_empty()
        {
            w.write_event(Event::Start(BytesStart::new("cols"))).unwrap();
            if let Some(dw) = sh.default_col_width {
                let mut e = BytesStart::new("col");
                e.push_attribute(("min", "1"));
                e.push_attribute(("max", "16384"));
                e.push_attribute(("width", dw.to_string().as_str()));
                w.write_event(Event::Empty(e)).unwrap();
            }
            // 列ごとの指定(幅・深さ・畳み)をひとつの走査にまとめる
            let mut marks: std::collections::BTreeSet<u32> =
                sh.col_width.keys().copied().collect();
            marks.extend(sh.col_outline.keys().copied());
            marks.extend(sh.col_hidden.iter().copied());
            let spec = |c: u32| {
                (
                    sh.col_width.get(&c).copied(),
                    sh.col_outline.get(&c).copied(),
                    sh.col_hidden.contains(&c),
                )
            };
            let same = |a: &(Option<f32>, Option<u8>, bool), b: &(Option<f32>, Option<u8>, bool)| {
                a.1 == b.1
                    && a.2 == b.2
                    && match (a.0, b.0) {
                        (Some(x), Some(y)) => (x - y).abs() < 1e-6,
                        (None, None) => true,
                        _ => false,
                    }
            };
            let cols: Vec<u32> = marks.into_iter().collect();
            let mut i = 0;
            while i < cols.len() {
                let c0 = cols[i];
                let sp = spec(c0);
                let mut c1 = c0;
                while i + 1 < cols.len() && cols[i + 1] == c1 + 1 && same(&spec(cols[i + 1]), &sp)
                {
                    c1 = cols[i + 1];
                    i += 1;
                }
                let mut e = BytesStart::new("col");
                e.push_attribute(("min", (c0 + 1).to_string().as_str()));
                e.push_attribute(("max", (c1 + 1).to_string().as_str()));
                if let Some(wd) = sp.0 {
                    e.push_attribute(("width", wd.to_string().as_str()));
                    e.push_attribute(("customWidth", "1"));
                }
                if let Some(l) = sp.1 {
                    e.push_attribute(("outlineLevel", l.to_string().as_str()));
                }
                if sp.2 {
                    e.push_attribute(("hidden", "1"));
                }
                w.write_event(Event::Empty(e)).unwrap();
                i += 1;
            }
            w.write_event(Event::End(BytesEnd::new("cols"))).unwrap();
        }
        w.write_event(Event::Start(BytesStart::new("sheetData"))).unwrap();

        let mut rows: std::collections::BTreeMap<u32, Vec<(&Pos, &Cell)>> = Default::default();
        for (p, c) in &sh.cells { rows.entry(p.row).or_default().push((p, c)); }
        // 中身が無くてもグループ化・畳みのある行は <row> を出す(捨てない)
        for r in sh.row_outline.keys().chain(sh.row_hidden.iter()) {
            rows.entry(*r).or_default();
        }
        for (r, cells) in rows {
            let mut row = BytesStart::new("row");
            row.push_attribute(("r", (r + 1).to_string().as_str()));
            if let Some(h) = sh.row_height.get(&r) {
                row.push_attribute(("ht", h.to_string().as_str()));
                row.push_attribute(("customHeight", "1"));
            }
            if let Some(l) = sh.row_outline.get(&r) {
                row.push_attribute(("outlineLevel", l.to_string().as_str()));
            }
            if sh.row_hidden.contains(&r) {
                row.push_attribute(("hidden", "1"));
            }
            w.write_event(Event::Start(row)).unwrap();
            for (p, c) in cells {
                let mut ce = BytesStart::new("c");
                ce.push_attribute(("r", p.a1().as_str()));
                let (ty, text) = match &c.value {
                    Value::Text(t) => ("s", idx[t].to_string()),
                    Value::Number(n) => ("", n.to_string()),
                    Value::Bool(b) => ("b", (*b as u8).to_string()),
                    Value::Error(e) => ("e", e.clone()),
                    Value::Empty => ("", String::new()),
                };
                if !ty.is_empty() { ce.push_attribute(("t", ty)); }
                // 書式は styles.xml 側にあり、ここは索引だけ。
                // 触っていないセルは**原本の索引のまま**(書式の据え置き)
                if let Some(i) = kept_style(sh, p, &c.fmt) {
                    if i > 0 {
                        ce.push_attribute(("s", i.to_string().as_str()));
                    }
                } else if let Some(s) = style_idx.get(&c.fmt).filter(|i| **i > 0) {
                    ce.push_attribute(("s", s.to_string().as_str()));
                }
                w.write_event(Event::Start(ce)).unwrap();
                if let Some(f) = &c.formula {
                    let mut fe = BytesStart::new("f");
                    // 昔ながらの配列数式は t="array" と覆う範囲を添えて返す。
                    // **返さないと、開いて保存しただけで普通の式に落ちる**
                    if let Some((h, wd)) = sh.cse.get(p) {
                        let end = Pos::new(p.row + h - 1, p.col + wd - 1);
                        fe.push_attribute(("t", "array"));
                        fe.push_attribute((
                            "ref",
                            format!("{}:{}", p.a1(), end.a1()).as_str(),
                        ));
                    }
                    w.write_event(Event::Start(fe)).unwrap();
                    w.write_event(Event::Text(BytesText::new(f))).unwrap();
                    w.write_event(Event::End(BytesEnd::new("f"))).unwrap();
                }
                if !text.is_empty() {
                    w.write_event(Event::Start(BytesStart::new("v"))).unwrap();
                    w.write_event(Event::Text(BytesText::new(&text))).unwrap();
                    w.write_event(Event::End(BytesEnd::new("v"))).unwrap();
                }
                w.write_event(Event::End(BytesEnd::new("c"))).unwrap();
            }
            w.write_event(Event::End(BytesEnd::new("row"))).unwrap();
        }
        w.write_event(Event::End(BytesEnd::new("sheetData"))).unwrap();
        // シートの保護(パスワード無し。効き目はアプリが守る)。
        // 作法どおり sheetData の直後・mergeCells の前
        if sh.protected {
            let mut pr = BytesStart::new("sheetProtection");
            pr.push_attribute(("sheet", "1"));
            pr.push_attribute(("scenarios", "1"));
            // **既定に頼らず全部書く。** 属性ごとに既定の向きが違うので、
            // 省くと読み手によって解釈が割れる
            let a = &sh.protect_allow;
            let d = |allow: bool| if allow { "0" } else { "1" };
            for (k, v) in [
                ("selectLockedCells", a.select_locked),
                ("selectUnlockedCells", a.select_unlocked),
                ("formatCells", a.format_cells),
                ("formatColumns", a.format_cols),
                ("formatRows", a.format_rows),
                ("insertColumns", a.insert_cols),
                ("insertRows", a.insert_rows),
                ("insertHyperlinks", a.insert_links),
                ("deleteColumns", a.delete_cols),
                ("deleteRows", a.delete_rows),
                ("sort", a.sort),
                ("autoFilter", a.autofilter),
                ("pivotTables", a.pivot),
                ("objects", a.objects),
            ] {
                pr.push_attribute((k, d(v)));
            }
            w.write_event(Event::Empty(pr)).unwrap();
        }
        // 結合を返す。読めたのに書かないと、開いて保存しただけで帳票が壊れる
        if !sh.merges.is_empty() {
            let mut mc = BytesStart::new("mergeCells");
            mc.push_attribute(("count", sh.merges.len().to_string().as_str()));
            w.write_event(Event::Start(mc)).unwrap();
            for (a, b) in &sh.merges {
                let mut m = BytesStart::new("mergeCell");
                m.push_attribute(("ref", format!("{}:{}", a.a1(), b.a1()).as_str()));
                w.write_event(Event::Empty(m)).unwrap();
            }
            w.write_event(Event::End(BytesEnd::new("mergeCells"))).unwrap();
        }
        w.write_event(Event::End(BytesEnd::new("worksheet"))).unwrap();
        let mut body = String::from_utf8(w.into_inner().into_inner()).unwrap();
        // 条件付き書式(schema では mergeCells の後・hyperlinks の前)
        if !sh.cond.is_empty() {
            let mut cf = String::new();
            for (n, r) in sh.cond.iter().enumerate() {
                let dxf = dxf_list
                    .iter()
                    .position(|p| *p == r.look)
                    .unwrap_or(0);
                let (a, b) = r.range;
                let sq = if a == b {
                    a.a1()
                } else {
                    format!("{}:{}", a.a1(), b.a1())
                };
                use crate::model::CondKind;
                let inner = match &r.kind {
                    CondKind::Cmp(op, v) => format!(
                        r#"<cfRule type="cellIs" dxfId="{dxf}" priority="{}" operator="{}"><formula>{v}</formula></cfRule>"#,
                        n + 1, op.as_xlsx()
                    ),
                    CondKind::Between(lo, hi, out) => format!(
                        r#"<cfRule type="cellIs" dxfId="{dxf}" priority="{}" operator="{}"><formula>{lo}</formula><formula>{hi}</formula></cfRule>"#,
                        n + 1, if *out { "notBetween" } else { "between" }
                    ),
                    CondKind::Text(t) => format!(
                        r#"<cfRule type="containsText" dxfId="{dxf}" priority="{}" operator="containsText" text="{}"/>"#,
                        n + 1,
                        t.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
                    ),
                    CondKind::Dup(false) => format!(
                        r#"<cfRule type="duplicateValues" dxfId="{dxf}" priority="{}"/>"#,
                        n + 1
                    ),
                    CondKind::Dup(true) => format!(
                        r#"<cfRule type="uniqueValues" dxfId="{dxf}" priority="{}"/>"#,
                        n + 1
                    ),
                    CondKind::Top(k, bottom) => format!(
                        r#"<cfRule type="top10" dxfId="{dxf}" priority="{}" rank="{k}"{}/>"#,
                        n + 1,
                        if *bottom { r#" bottom="1""# } else { "" }
                    ),
                    CondKind::Avg(below) => format!(
                        r#"<cfRule type="aboveAverage" dxfId="{dxf}" priority="{}"{}/>"#,
                        n + 1,
                        if *below { r#" aboveAverage="0""# } else { "" }
                    ),
                    // バー/スケール/アイコンは dxf を使わない(色は中身に持つ)
                    CondKind::Bar(color) => format!(
                        r#"<cfRule type="dataBar" priority="{}"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF{color}"/></dataBar></cfRule>"#,
                        n + 1
                    ),
                    CondKind::Scale(lo, mid, hi) => {
                        let (vo, cols) = match mid {
                            Some(m) => (
                                r#"<cfvo type="min"/><cfvo type="percentile" val="50"/><cfvo type="max"/>"#.to_string(),
                                format!(r#"<color rgb="FF{lo}"/><color rgb="FF{m}"/><color rgb="FF{hi}"/>"#),
                            ),
                            None => (
                                r#"<cfvo type="min"/><cfvo type="max"/>"#.to_string(),
                                format!(r#"<color rgb="FF{lo}"/><color rgb="FF{hi}"/>"#),
                            ),
                        };
                        format!(
                            r#"<cfRule type="colorScale" priority="{}"><colorScale>{vo}{cols}</colorScale></cfRule>"#,
                            n + 1
                        )
                    }
                    CondKind::Icons(name) => {
                        // 区切りはアイコン数で等分(3つなら 0/33/67%)
                        let k: u32 = name
                            .chars()
                            .next()
                            .and_then(|c| c.to_digit(10))
                            .unwrap_or(3)
                            .max(2);
                        let vo: String = (0..k)
                            .map(|i| {
                                format!(r#"<cfvo type="percent" val="{}"/>"#, i * 100 / k)
                            })
                            .collect();
                        format!(
                            r#"<cfRule type="iconSet" priority="{}"><iconSet iconSet="{name}">{vo}</iconSet></cfRule>"#,
                            n + 1
                        )
                    }
                    // 数式で指定。**持っている原文をそのまま返す** —
                    // 画面で解くときにずらした式は、間違っても書かない
                    CondKind::Formula(f) => format!(
                        r#"<cfRule type="expression" dxfId="{dxf}" priority="{}"><formula>{}</formula></cfRule>"#,
                        n + 1,
                        f.replace('&', "&amp;").replace('<', "&lt;")
                    ),
                };
                cf.push_str(&format!(
                    r#"<conditionalFormatting sqref="{sq}">{inner}</conditionalFormatting>"#
                ));
            }
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, &cf);
            }
        }
        // データの入力規則(schema では conditionalFormatting の後・hyperlinks の前)
        if !sh.validations.is_empty() {
            // 属性は " も守る(文言が入るため)。本文は & と < だけ
            let ea = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
            let et = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;");
            let mut dv = format!(r#"<dataValidations count="{}">"#, sh.validations.len());
            for v in &sh.validations {
                let (a, b) = v.range;
                let sq = if a == b { a.a1() } else { format!("{}:{}", a.a1(), b.a1()) };
                let mut attrs = String::new();
                if !v.kind.is_empty() {
                    attrs.push_str(&format!(r#" type="{}""#, ea(&v.kind)));
                }
                if !v.op.is_empty() {
                    attrs.push_str(&format!(r#" operator="{}""#, ea(&v.op)));
                }
                if let Some((style, t, m)) = &v.error_msg {
                    attrs.push_str(&format!(
                        r#" errorStyle="{}" errorTitle="{}" error="{}""#,
                        ea(style), ea(t), ea(m)
                    ));
                }
                if let Some((t, m)) = &v.input_msg {
                    attrs.push_str(&format!(
                        r#" promptTitle="{}" prompt="{}""#,
                        ea(t), ea(m)
                    ));
                }
                if v.hide_arrow {
                    attrs.push_str(r#" showDropDown="1""#);
                }
                let mut fs = String::new();
                if !v.formula.is_empty() {
                    fs.push_str(&format!("<formula1>{}</formula1>", et(&v.formula)));
                }
                if !v.formula2.is_empty() {
                    fs.push_str(&format!("<formula2>{}</formula2>", et(&v.formula2)));
                }
                dv.push_str(&format!(
                    r#"<dataValidation{attrs} allowBlank="{}" showInputMessage="1" showErrorMessage="1" sqref="{sq}">{fs}</dataValidation>"#,
                    if v.allow_blank { "1" } else { "0" },
                ));
            }
            dv.push_str("</dataValidations>");
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, &dv);
            }
        }
        // ハイパーリンク(schema では mergeCells の後・印刷まわりの前)
        if !sh.links.is_empty() {
            let mut hl = String::from("<hyperlinks>");
            for (n, (p, url)) in sh.links.iter().enumerate() {
                if let Some(loc) = url.strip_prefix('#') {
                    hl.push_str(&format!(
                        r#"<hyperlink ref="{}" location="{}"/>"#, p.a1(), esc(loc)));
                } else {
                    hl.push_str(&format!(
                        r#"<hyperlink ref="{}" r:id="rIdHL{}"/>"#, p.a1(), n + 1));
                }
            }
            hl.push_str("</hyperlinks>");
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, &hl);
            }
        }
        // 印刷まわり(原本の原文にモデルの向き・用紙・余白を織り込む)と
        // 図形の参照を、schema の位置(hyperlinks の後)へ
        {
            let orig = sheet_extras.get(i).map(|s| s.as_str()).unwrap_or("");
            let extra = print_extra_xml(orig, sh);
            if !extra.is_empty() {
                if let Some(pos) = body.rfind("</worksheet>") {
                    body.insert_str(pos, &extra);
                }
            }
        }
        // このアプリで挿した画像。原本に drawing が無ければ新しい部品への参照を足す
        // (原本に有るときは、その部品の中へアンカーを継ぎ足す — 部品は1シート1つの決まり)
        if (!sh.images_new.is_empty() || !sh.shapes_new.is_empty())
            && !body.contains("<drawing ")
        {
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, r#"<drawing r:id="rIdDRW"/>"#);
            }
        }
        // 表オブジェクトへの参照(schema では最後の方)
        if !sh.tables.is_empty() {
            let base: usize = book.sheets[..i].iter().map(|s| s.tables.len()).sum();
            let mut tp = format!(r#"<tableParts count="{}">"#, sh.tables.len());
            for k in 0..sh.tables.len() {
                tp.push_str(&format!(r#"<tablePart r:id="rIdTBL{}"/>"#, base + k + 1));
            }
            tp.push_str("</tableParts>");
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, &tp);
            }
        }
        // コメントの図形(VML)への参照は一番後ろ
        if !sh.comments.is_empty() {
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, r#"<legacyDrawing r:id="rIdVML"/>"#);
            }
        }
        put(&format!("xl/worksheets/sheet{}.xml", i + 1),
            &format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n{body}"))?;

        // このシートの rels。原本のもの(図形など)は残し、
        // リンク・コメントのぶんはこちらが作り直す
        let orig = orig_sheet_rels.get(i).cloned().flatten();
        if !sh.links.is_empty() || !sh.comments.is_empty() || orig.is_some()
            || !sh.images_new.is_empty() || !sh.shapes_new.is_empty()
            || !sh.tables.is_empty()
        {
            let mut inner = String::new();
            if let Some(o) = &orig {
                for (id, ty, target, ext) in parse_rels(o) {
                    if ty.ends_with("/hyperlink")
                        || ty.ends_with("/comments")
                        || ty.ends_with("/vmlDrawing")
                        || ty.ends_with("/table")
                    {
                        continue;
                    }
                    inner.push_str(&format!(
                        r#"<Relationship Id="{}" Type="{}" Target="{}"{}/>"#,
                        esc(&id), esc(&ty), esc(&target),
                        if ext { r#" TargetMode="External""# } else { "" }
                    ));
                }
            }
            for (n, (_, url)) in sh.links.iter().enumerate() {
                if url.starts_with('#') {
                    continue; // 帳面の中の場所は location 属性だけで足りる
                }
                inner.push_str(&format!(
                    r#"<Relationship Id="rIdHL{}" Type="{RNS}/hyperlink" Target="{}" TargetMode="External"/>"#,
                    n + 1, esc(url)
                ));
            }
            let had_drawing = orig.as_deref().is_some_and(|o| o.contains("/drawing\""));
            if (!sh.images_new.is_empty() || !sh.shapes_new.is_empty()) && !had_drawing {
                inner.push_str(&format!(
                    r#"<Relationship Id="rIdDRW" Type="{RNS}/drawing" Target="../drawings/drawingC{}.xml"/>"#,
                    i + 1
                ));
            }
            {
                let base: usize = book.sheets[..i].iter().map(|s| s.tables.len()).sum();
                for k in 0..sh.tables.len() {
                    let n = base + k + 1;
                    inner.push_str(&format!(
                        r#"<Relationship Id="rIdTBL{n}" Type="{RNS}/table" Target="../tables/table{n}.xml"/>"#
                    ));
                }
            }
            if !sh.comments.is_empty() {
                inner.push_str(&format!(
                    r#"<Relationship Id="rIdCM" Type="{RNS}/comments" Target="../comments{}.xml"/>"#,
                    i + 1
                ));
                inner.push_str(&format!(
                    r#"<Relationship Id="rIdVML" Type="{RNS}/vmlDrawing" Target="../drawings/vmlDrawing{}.vml"/>"#,
                    i + 1
                ));
                // スレッドの本体への道。**この関係が無いと Excel は
                // 古い写しだけを見る** = 返信も解決も無かったことになる
                inner.push_str(&format!(
                    r#"<Relationship Id="rIdTC" Type="http://schemas.microsoft.com/office/2017/10/relationships/threadedComment" Target="../threadedComments/threadedComment{}.xml"/>"#,
                    i + 1
                ));
            }
            put(&format!("xl/worksheets/_rels/sheet{}.xml.rels", i + 1), &format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{inner}</Relationships>"))?;
        }
        // 表オブジェクトの部品
        {
            let base: usize = book.sheets[..i].iter().map(|s| s.tables.len()).sum();
            for (k, t) in sh.tables.iter().enumerate() {
                let n = base + k + 1;
                let r = if t.a == t.b {
                    t.a.a1()
                } else {
                    format!("{}:{}", t.a.a1(), t.b.a1())
                };
                // 列の名前は見出し行から。空なら「列N」(Excel は空名を嫌う)
                let mut cols = String::new();
                for (ci, c) in (t.a.col..=t.b.col).enumerate() {
                    let nm = if t.header {
                        sh.get(Pos::new(t.a.row, c))
                            .map(|x| x.value.display())
                            .filter(|v| !v.is_empty())
                            .unwrap_or_else(|| format!("列{}", ci + 1))
                    } else {
                        format!("列{}", ci + 1)
                    };
                    cols.push_str(&format!(
                        r#"<tableColumn id="{}" name="{}"/>"#,
                        ci + 1,
                        esc(&nm)
                    ));
                }
                let b01 = |v: bool| if v { "1" } else { "0" };
                let xml = format!(
                    concat!(
                        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
                        r#"<table xmlns="{ns}" id="{n}" name="{nm}" displayName="{nm}" ref="{r}""#,
                        r#" headerRowCount="{hdr}" totalsRowCount="{tot}">"#,
                        r#"{af}<tableColumns count="{cnt}">{cols}</tableColumns>"#,
                        r#"<tableStyleInfo name="{sty}" showFirstColumn="{fc}""#,
                        r#" showLastColumn="{lc}" showRowStripes="{rs}" showColumnStripes="{cs}"/>"#,
                        r#"</table>"#
                    ),
                    ns = NS,
                    n = n,
                    nm = esc(&t.name),
                    r = r,
                    hdr = if t.header { 1 } else { 0 },
                    tot = if t.totals { 1 } else { 0 },
                    af = if t.filter {
                        format!(r#"<autoFilter ref="{r}"/>"#)
                    } else {
                        String::new()
                    },
                    cnt = (t.b.col - t.a.col + 1),
                    cols = cols,
                    // **原本の様式を据え置く。** ここを決め打ちにしていたので、
                    // `TableStyleLight9` の帳票を開いて保存すると青くなっていた
                    sty = esc(t.style.as_deref().unwrap_or("TableStyleMedium2")),
                    fc = b01(t.first_col),
                    lc = b01(t.last_col),
                    rs = b01(t.banded_rows),
                    cs = b01(t.banded_cols),
                );
                put(&format!("xl/tables/table{n}.xml"), &xml)?;
            }
        }
        // コメントの本体と、Excel がコメントに使う最小の VML 図形
        if !sh.comments.is_empty() {
            let persons = &book_persons;
            // 著者の一覧(重複は畳む)。**古い写しにも名前を残す**
            let mut authors: Vec<String> = Vec::new();
            for th in sh.comments.values() {
                for e in &th.entries {
                    if !authors.contains(&e.who) {
                        authors.push(e.who.clone());
                    }
                }
            }
            if authors.is_empty() {
                authors.push(String::new());
            }
            let mut cl = String::new();
            for (p, th) in &sh.comments {
                let aid = th
                    .entries
                    .first()
                    .and_then(|e| authors.iter().position(|a| *a == e.who))
                    .unwrap_or(0);
                // **写しには筋を一続きにして書く。** 頭だけ書くと、古い
                // 読み手には返信が無かったことになる
                cl.push_str(&format!(
                    r#"<comment ref="{}" authorId="{aid}"><text><r><t xml:space="preserve">{}</t></r></text></comment>"#,
                    p.a1(),
                    esc(&th.flatten())
                ));
            }
            let al: String =
                authors.iter().map(|a| format!("<author>{}</author>", esc(a))).collect();
            put(&format!("xl/comments{}.xml", i + 1), &format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<comments xmlns="{NS}"><authors>{al}</authors><commentList>{cl}</commentList></comments>"#))?;
            let mut shapes = String::new();
            for (n, (p, _)) in sh.comments.iter().enumerate() {
                shapes.push_str(&format!(
                    r##"<v:shape id="_x0000_s{}" type="#_x0000_t202" style="position:absolute;margin-left:80pt;margin-top:2pt;width:120pt;height:60pt;z-index:{};visibility:hidden" fillcolor="#ffffe1" o:insetmode="auto"><v:fill color2="#ffffe1"/><x:ClientData ObjectType="Note"><x:MoveWithCells/><x:SizeWithCells/><x:AutoFill>False</x:AutoFill><x:Row>{}</x:Row><x:Column>{}</x:Column></x:ClientData></v:shape>"##,
                    1025 + n, n + 1, p.row, p.col
                ));
            }
            put(&format!("xl/drawings/vmlDrawing{}.vml", i + 1), &format!(
                r#"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel"><o:shapelayout v:ext="edit"><o:idmap v:ext="edit" data="1"/></o:shapelayout><v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe"><v:stroke joinstyle="miter"/><v:path gradientshapeok="t" o:connecttype="rect"/></v:shapetype>{shapes}</xml>"#))?;
            // **スレッドの本体。** 近代の Excel はこちらを見るので、
            // 古い写しと**同じ回で必ず一緒に書く** — 片方だけ書き換えると、
            // 直したつもりの文が Excel に映らない(2026-08-13 に実測した穴)
            let mut tc = String::new();
            let mut n = 0usize;
            for (p, th) in &sh.comments {
                let mut parent: Option<String> = None;
                let done = if th.done { r#" done="1""# } else { r#" done="0""# };
                for e in &th.entries {
                    n += 1;
                    let id = guid(i * 1000 + n);
                    let pid = match &parent {
                        Some(x) => format!(r#" parentId="{x}""#),
                        None => String::new(),
                    };
                    let person = match persons.iter().position(|a| *a == e.who) {
                        Some(k) => guid(900_000 + k),
                        None => guid(900_000),
                    };
                    // 日付は綴りのまま返す。無ければ書かない(嘘の日付を作らない)
                    let dt = if e.when.is_empty() {
                        String::new()
                    } else {
                        format!(r#" dT="{}""#, esc(&e.when))
                    };
                    tc.push_str(&format!(
                        r#"<threadedComment ref="{}"{dt} personId="{person}" id="{id}"{pid}{}><text>{}</text></threadedComment>"#,
                        p.a1(),
                        if parent.is_none() { done } else { "" },
                        esc(&e.text)
                    ));
                    if parent.is_none() {
                        parent = Some(id);
                    }
                }
            }
            put(&format!("xl/threadedComments/threadedComment{}.xml", i + 1), &format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<ThreadedComments xmlns="{TCNS}">{tc}</ThreadedComments>"#))?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// 挿した図形1枚のアンカー(oneCellAnchor の xdr:sp)。Excel でも図形として開ける。
pub(super) fn shape_anchor_xml(sp: &crate::model::SheetShape, id: u32) -> String {
    let (cx, cy) = ((sp.width_px * 9525.0) as i64, (sp.height_px * 9525.0) as i64);
    // 不透明度は srgbClr の子 a:alpha(10万分率)。1.0 なら書かない
    let alpha = if sp.alpha < 0.999 {
        format!(
            "<a:alpha val=\"{}\"/>",
            (sp.alpha.clamp(0.0, 1.0) * 100_000.0) as i64
        )
    } else {
        String::new()
    };
    let fill = match &sp.fill {
        Some(c) => format!("<a:solidFill><a:srgbClr val=\"{c}\">{alpha}</a:srgbClr></a:solidFill>"),
        None => "<a:noFill/>".to_string(),
    };
    let line = match &sp.line {
        Some(c) => format!(
            "<a:ln w=\"{w}\"><a:solidFill><a:srgbClr val=\"{c}\">{alpha}</a:srgbClr></a:solidFill></a:ln>",
            w = (sp.line_w.max(0.1) * 12700.0) as i64
        ),
        None => String::new(),
    };
    // 影(右下への落ち影)。色は固定の灰 — 画面の描き方と揃える
    let effect = if sp.shadow {
        concat!(
            "<a:effectLst><a:outerShdw blurRad=\"50800\" dist=\"50800\" ",
            "dir=\"2700000\" algn=\"tl\" rotWithShape=\"0\">",
            "<a:srgbClr val=\"9E9E9E\"><a:alpha val=\"35000\"/></a:srgbClr>",
            "</a:outerShdw></a:effectLst>"
        )
    } else {
        ""
    };
    // 回転(6万分の1度)と反転は xfrm の属性
    let mut xfrm_attrs = String::new();
    let rot = sp.rot.rem_euclid(360.0);
    if rot != 0.0 {
        xfrm_attrs.push_str(&format!(" rot=\"{}\"", (rot * 60000.0) as i64));
    }
    if sp.flip_h {
        xfrm_attrs.push_str(" flipH=\"1\"");
    }
    if sp.flip_v {
        xfrm_attrs.push_str(" flipV=\"1\"");
    }
    // 形: 折れ線(spark)は custGeom、他は prstGeom。
    // 縦棒・勝ち負けも custGeom(棒ごとに4点の閉じた小道 — Excel でも棒に見える)
    let bars = matches!(sp.kind.as_str(), "spark-col" | "spark-wl");
    let poly = matches!(sp.kind.as_str(), "spark" | "ink" | "marker");
    let geom = if bars && !sp.points.is_empty() {
        let n = sp.points.len().max(1) as f32;
        let bw = (10000.0 / n * 0.7).max(120.0);
        let base = (sp.base * 10000.0) as i64;
        let mut path = String::new();
        for (cx_, ty) in &sp.points {
            let (l, r) = (
                ((cx_ * 10000.0) - bw / 2.0) as i64,
                ((cx_ * 10000.0) + bw / 2.0) as i64,
            );
            let t = (ty * 10000.0) as i64;
            path.push_str(&format!(
                concat!(
                    "<a:moveTo><a:pt x=\"{l}\" y=\"{t}\"/></a:moveTo>",
                    "<a:lnTo><a:pt x=\"{r}\" y=\"{t}\"/></a:lnTo>",
                    "<a:lnTo><a:pt x=\"{r}\" y=\"{b}\"/></a:lnTo>",
                    "<a:lnTo><a:pt x=\"{l}\" y=\"{b}\"/></a:lnTo>",
                    "<a:close/>"
                ),
                l = l, r = r, t = t, b = base
            ));
        }
        format!(
            concat!(
                "<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>",
                "<a:rect l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>",
                "<a:pathLst><a:path w=\"10000\" h=\"10000\">{}</a:path></a:pathLst>",
                "</a:custGeom>"
            ),
            path
        )
    } else if poly && !sp.points.is_empty() {
        let mut path = String::new();
        for (i, (x, y)) in sp.points.iter().enumerate() {
            let (px_, py_) = ((x * 10000.0) as i64, (y * 10000.0) as i64);
            if i == 0 {
                path.push_str(&format!(
                    "<a:moveTo><a:pt x=\"{px_}\" y=\"{py_}\"/></a:moveTo>"
                ));
            } else {
                path.push_str(&format!(
                    "<a:lnTo><a:pt x=\"{px_}\" y=\"{py_}\"/></a:lnTo>"
                ));
            }
        }
        format!(
            concat!(
                "<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>",
                "<a:rect l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>",
                "<a:pathLst><a:path w=\"10000\" h=\"10000\" fill=\"none\">{}</a:path></a:pathLst>",
                "</a:custGeom>"
            ),
            path
        )
    } else {
        format!("<a:prstGeom prst=\"{}\"><a:avLst/></a:prstGeom>", sp.kind)
    };
    // 中の文字(テキストボックス)。組み方は sp.text_fmt から。
    // **既定のときは属性を書かない** — 書かないことが既定を表す(xlsx の作法)
    let txt = match &sp.text {
        Some(t) => {
            let tf = &sp.text_fmt;
            let anchor = match tf.anchor {
                crate::model::TextAnchor::Top => "",
                crate::model::TextAnchor::Middle => r#" anchor="ctr""#,
                crate::model::TextAnchor::Bottom => r#" anchor="b""#,
            };
            // 縦書きは日本語の縦組み(eaVert = 東アジアの縦。字は回さない)
            let vert = if tf.vertical { r#" vert="eaVert""# } else { "" };
            let algn = match tf.align {
                crate::model::HAlign::Center => r#" algn="ctr""#,
                crate::model::HAlign::Right => r#" algn="r""#,
                crate::model::HAlign::Justify => r#" algn="just""#,
                _ => "",
            };
            // 箇条書き。中黒は buChar、番号は buAutoNum(算用数字+ピリオド)
            let bullet = match tf.bullet {
                Some(true) => r#"<a:buFont typeface="+mj-lt"/><a:buAutoNum type="arabicPeriod"/>"#,
                Some(false) => r#"<a:buFont typeface="Arial"/><a:buChar char="・"/>"#,
                None => "",
            };
            let ppr = if algn.is_empty() && bullet.is_empty() {
                String::new()
            } else {
                format!("<a:pPr{algn}>{bullet}</a:pPr>")
            };
            let strike = if tf.strike { r#" strike="sngStrike""# } else { "" };
            // 上付き・下付きは baseline の千分率(Office の既定に合わせる)
            let base = if tf.sup {
                r#" baseline="30000""#
            } else if tf.sub {
                r#" baseline="-25000""#
            } else {
                ""
            };
            format!(
                concat!(
                    "<xdr:txBody><a:bodyPr wrap=\"square\"{anchor}{vert}/><a:lstStyle/>",
                    "<a:p>{ppr}<a:r><a:rPr lang=\"ja-JP\" sz=\"1100\"{strike}{base}/>",
                    "<a:t>{t}</a:t></a:r></a:p>",
                    "</xdr:txBody>"
                ),
                anchor = anchor,
                vert = vert,
                ppr = ppr,
                strike = strike,
                base = base,
                t = esc(t)
            )
        }
        None => String::new(),
    };
    format!(
        concat!(
            "<xdr:oneCellAnchor>",
            "<xdr:from><xdr:col>{col}</xdr:col><xdr:colOff>{dx}</xdr:colOff>",
            "<xdr:row>{row}</xdr:row><xdr:rowOff>{dy}</xdr:rowOff></xdr:from>",
            "<xdr:ext cx=\"{cx}\" cy=\"{cy}\"/>",
            "<xdr:sp macro=\"\" textlink=\"\">",
            "<xdr:nvSpPr><xdr:cNvPr id=\"{id}\" name=\"{name}\"/><xdr:cNvSpPr/></xdr:nvSpPr>",
            "<xdr:spPr><a:xfrm{xfrm}><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>",
            "{geom}{fill}{line}{effect}</xdr:spPr>{txt}",
            "</xdr:sp><xdr:clientData/></xdr:oneCellAnchor>"
        ),
        col = sp.at.col,
        row = sp.at.row,
        dx = (sp.dx_px * 9525.0) as i64,
        dy = (sp.dy_px * 9525.0) as i64,
        cx = cx,
        cy = cy,
        id = id,
        // 折れ線ものは name に自作の札を残す — 開き直しで組み直せる。
        // **印は3つ目の欄**(無ければ空。古い札とも読み合える)
        name = if matches!(sp.kind.as_str(), "spark-col" | "spark-wl") {
            format!("jo:{}:{:.4}:{}", sp.kind, sp.base, sp.spark_marks.tag())
        } else if sp.kind == "spark" && sp.spark_marks != crate::model::SparkMarks::default() {
            format!("jo:spark:0:{}", sp.spark_marks.tag())
        } else {
            format!("図形 {id}")
        },
        geom = geom,
        fill = fill,
        line = line,
        effect = effect,
        xfrm = xfrm_attrs,
        txt = txt
    )
}

/// 挿した画像1枚のアンカー(oneCellAnchor)。大きさは px → EMU(9525 EMU = 1px)。
pub(super) fn image_anchor_xml(im: &crate::model::SheetImage, rid: &str, id: u32) -> String {
    let (cx, cy) = ((im.width_px * 9525.0) as i64, (im.height_px * 9525.0) as i64);
    format!(
        concat!(
            "<xdr:oneCellAnchor>",
            "<xdr:from><xdr:col>{col}</xdr:col><xdr:colOff>{cox}</xdr:colOff>",
            "<xdr:row>{row}</xdr:row><xdr:rowOff>{roy}</xdr:rowOff></xdr:from>",
            "<xdr:ext cx=\"{cx}\" cy=\"{cy}\"/>",
            "<xdr:pic><xdr:nvPicPr><xdr:cNvPr id=\"{id}\" name=\"画像 {id}\"/><xdr:cNvPicPr/></xdr:nvPicPr>",
            "<xdr:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill>",
            "<xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>",
            "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic>",
            "<xdr:clientData/></xdr:oneCellAnchor>"
        ),
        col = im.at.col,
        row = im.at.row,
        cox = (im.dx_px * 9525.0) as i64,
        roy = (im.dy_px * 9525.0) as i64,
        cx = cx,
        cy = cy,
        id = id,
        rid = rid
    )
}
