//! **シートの、格子に載らない意味を表で持つ。**
//!
//! `.sheet.adoc` は「表1つ = シート1枚」です。名前の定義や入力規則は
//! 升目ではないので、そのままでは書けません。`[.names]` のような**役割の
//! 印**を付けた表にして、シートの表と見分けます。
//!
//! ....
//! [.names]
//! .売上台帳
//! |===
//! |name |range |scoped
//!
//! |税率 |$A$1 |true
//! |===
//! ....
//!
//! # 役割と列の見出しは英語
//!
//! **書式の一部であって、画面に出る字ではない**からです(2026-08-26
//! 発注者)。テンプレート(`.tmpl.adoc`)は人が読んで直す物なので各国語
//! ですが、こちらは利用者が直接書く物ではありません。
//!
//! 題は**シートの名前**です。どのシートの物かを言うために使います。

/// 図形を (名前, 項目, 値) の表で持つ
pub mod shape;

use crate::{Block, Cellbox, Document, Table};
use book::{
    CondKind, CondLook, CondOp, CondRule, DefinedName, Pos, Scenario, Sheet, TableDef, Validation,
};

/// 役割の印。**足したらここに書く** — `every_role_is_handled` が
/// 読み手と揃っているか確かめます。
pub const ROLES: &[&str] = &[
    "hidden", "tables", "names", "links", "conditional-format", "validations",
    "scenarios", "print-areas", "phonetics", "shapes", "images",
];

/// シート1枚ぶんの、格子に載らない意味を表にする。
pub fn tables_of(s: &Sheet) -> Vec<Table> {
    let mut out = Vec::new();
    let push = |out: &mut Vec<Table>, role: &str, head: &[&str], rows: Vec<Vec<String>>| {
        if !rows.is_empty() {
            out.push(table(role, &s.name, head, rows));
        }
    };

    // 隠した行と列。**絞り込みと違って保存に残る**ので意味の側です
    let mut hidden: Vec<Vec<String>> = Vec::new();
    for r in &s.row_hidden {
        hidden.push(vec!["row".into(), (r + 1).to_string()]);
    }
    for c in &s.col_hidden {
        hidden.push(vec!["column".into(), col_name(*c)]);
    }
    push(&mut out, "hidden", &["kind", "at"], hidden);

    push(&mut out, "tables", &["name", "range", "style", "header", "totals",
        "banded-rows", "banded-cols", "first-col", "last-col", "filter"],
        s.tables.iter().map(|t| vec![
            t.name.clone(), span(t.a, t.b), t.style.clone().unwrap_or_default(),
            yes(t.header), yes(t.totals), yes(t.banded_rows), yes(t.banded_cols),
            yes(t.first_col), yes(t.last_col), yes(t.filter),
        ]).collect());

    push(&mut out, "names", &["name", "range", "scoped"],
        s.names.iter().map(|n| vec![n.name.clone(), n.range.clone(), yes(n.scoped)]).collect());

    push(&mut out, "links", &["at", "target"],
        s.links.iter().map(|(p, u)| vec![p.a1(), u.clone()]).collect());

    push(&mut out, "conditional-format", &["range", "rule", "look"],
        s.cond.iter().map(|r| vec![span(r.range.0, r.range.1), cond_kind(&r.kind), cond_look(&r.look)]).collect());

    push(&mut out, "validations", &["range", "kind", "op", "formula", "formula2",
        "input-title", "input-body", "error-style", "error-title", "error-body",
        "allow-blank", "hide-arrow"],
        s.validations.iter().map(|v| {
            let (it, ib) = v.input_msg.clone().unwrap_or_default();
            let (es, et, eb) = v.error_msg.clone().unwrap_or_default();
            vec![span(v.range.0, v.range.1), v.kind.clone(), v.op.clone(),
                 v.formula.clone(), v.formula2.clone(), it, ib, es, et, eb,
                 yes(v.allow_blank), yes(v.hide_arrow)]
        }).collect());

    push(&mut out, "scenarios", &["name", "comment", "cells"],
        s.scenarios.iter().map(|sc| vec![sc.name.clone(), sc.comment.clone(),
            sc.cells.iter().map(|(p, v)| format!("{}={v}", p.a1())).collect::<Vec<_>>().join(" ")]).collect());

    push(&mut out, "print-areas", &["range"],
        s.print_areas.iter().map(|(a, b)| vec![span(*a, *b)]).collect());

    push(&mut out, "phonetics", &["at", "reading"],
        s.phonetics.iter().map(|(p, r)| vec![p.a1(), r.clone()]).collect());

    // **図形は縦長の (名前, 項目, 値)。** 持ち物が 19 あるので横には並べません。
    // 名前は場所(`D5`)です — 図形そのものに名前が無いので、置き場で呼びます
    let mut shapes: Vec<Vec<String>> = Vec::new();
    for sp in &s.shapes {
        let at = sp.at.a1();
        for (item, v) in shape::to_rows(sp) {
            shapes.push(vec![at.clone(), item.to_string(), v]);
        }
    }
    push(&mut out, "shapes", &["shape", "item", "value"], shapes);

    // **画像は実体を隣のファイルに出します。** binary は adoc に入りません。
    // 名前はシート名と番号から決まるので、模型に径路の欄を足さずに済みます
    push(&mut out, "images", &["file", "at", "dx", "dy", "width", "height"],
        s.images.iter().enumerate().map(|(i, im)| vec![
            image_file(&s.name, i, &im.data), im.at.a1(),
            n2(im.dx_px), n2(im.dy_px), n2(im.width_px), n2(im.height_px),
        ]).collect());

    out
}

/// **画像の置き場。** `images/<シート名>-<番号>.<形式>`。
///
/// 名前が中身から決まるので、同じブックを2度書き出しても同じ名前です。
pub fn image_file(sheet: &str, nth: usize, data: &[u8]) -> String {
    format!("images/{}-{}.{}", safe_name(sheet), nth + 1, image_ext(data))
}

/// ファイル名に使えない字を `_` にする(どの OS でも通る形)
fn safe_name(s: &str) -> String {
    s.chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect()
}

/// 中身の頭から形式を見る。分からなければ `png`
fn image_ext(data: &[u8]) -> &'static str {
    match data {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, ..] => "jpg",
        [b'G', b'I', b'F', ..] => "gif",
        [b'<', ..] => "svg",
        _ => "png",
    }
}

fn n2(v: f32) -> String {
    if (v - v.round()).abs() < 0.0005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// **書き出す画像の実体。**(径路, 中身)を返します。
///
/// `.sheet.adoc` を保存する側が、隣にこのファイルを置きます
/// (writer の [`crate::adoc::assign_image_paths`] と同じ作法)。
pub fn image_files(book: &book::Book) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for s in &book.sheets {
        for (i, im) in s.images.iter().enumerate() {
            if !im.data.is_empty() {
                out.push((image_file(&s.name, i, &im.data), im.data.clone()));
            }
        }
    }
    out
}

/// 役割の印の付いた表をシートへ入れる。**知らない役割は黙って飛ばします**
/// (先の版が足した印を、古い版が落とさないためです)。
pub fn take(role: &str, sheet_name: &str, rows: &[Vec<String>], s: &mut Sheet) {
    if s.name != sheet_name {
        return;
    }
    fn g(r: &[String], i: usize) -> &str {
        r.get(i).map(|x| x.trim()).unwrap_or("")
    }
    match role {
        "hidden" => {
            for r in rows {
                match g(r, 0) {
                    "row" => {
                        if let Ok(n) = g(r, 1).parse::<u32>() {
                            if n > 0 {
                                s.row_hidden.insert(n - 1);
                            }
                        }
                    }
                    "column" => {
                        if let Some(c) = col_index(g(r, 1)) {
                            s.col_hidden.insert(c);
                        }
                    }
                    _ => {}
                }
            }
        }
        "tables" => {
            // **明示の指定が正です。** シートの表に見出しの行があると
            // `to_sheet` が定義を1つ作ります。こちらの表があるときは
            // そちらが正しいので、作った分を捨てます(2026-08-26)
            s.tables.clear();
            for r in rows {
                let Some((a, b)) = read_span(g(r, 1)) else { continue };
                s.tables.push(TableDef {
                    name: g(r, 0).into(), a, b,
                    style: (!g(r, 2).is_empty()).then(|| g(r, 2).to_string()),
                    header: on(g(r, 3)), totals: on(g(r, 4)),
                    banded_rows: on(g(r, 5)), banded_cols: on(g(r, 6)),
                    first_col: on(g(r, 7)), last_col: on(g(r, 8)), filter: on(g(r, 9)),
                });
            }
        }
        "names" => {
            for r in rows {
                s.names.push(DefinedName {
                    name: g(r, 0).into(), range: g(r, 1).into(), scoped: on(g(r, 2)),
                });
            }
        }
        "links" => {
            for r in rows {
                if let Some(p) = Pos::parse(g(r, 0)) {
                    s.links.insert(p, g(r, 1).to_string());
                }
            }
        }
        "conditional-format" => {
            for r in rows {
                let Some(range) = read_span(g(r, 0)) else { continue };
                let Some(kind) = read_cond_kind(g(r, 1)) else { continue };
                s.cond.push(CondRule { range, kind, look: read_cond_look(g(r, 2)) });
            }
        }
        "validations" => {
            for r in rows {
                let Some(range) = read_span(g(r, 0)) else { continue };
                let input = (!g(r, 5).is_empty() || !g(r, 6).is_empty())
                    .then(|| (g(r, 5).to_string(), g(r, 6).to_string()));
                let error = (!g(r, 7).is_empty() || !g(r, 8).is_empty() || !g(r, 9).is_empty())
                    .then(|| (g(r, 7).to_string(), g(r, 8).to_string(), g(r, 9).to_string()));
                s.validations.push(Validation {
                    range, kind: g(r, 1).into(), op: g(r, 2).into(),
                    formula: g(r, 3).into(), formula2: g(r, 4).into(),
                    input_msg: input, error_msg: error,
                    allow_blank: on(g(r, 10)), hide_arrow: on(g(r, 11)),
                });
            }
        }
        "scenarios" => {
            for r in rows {
                let cells = g(r, 2)
                    .split_whitespace()
                    .filter_map(|x| {
                        let (at, v) = x.split_once('=')?;
                        Some((Pos::parse(at)?, v.to_string()))
                    })
                    .collect();
                s.scenarios.push(Scenario { name: g(r, 0).into(), comment: g(r, 1).into(), cells });
            }
        }
        "print-areas" => {
            for r in rows {
                if let Some(sp) = read_span(g(r, 0)) {
                    s.print_areas.push(sp);
                }
            }
        }
        "phonetics" => {
            for r in rows {
                if let Some(p) = Pos::parse(g(r, 0)) {
                    s.phonetics.insert(p, g(r, 1).to_string());
                }
            }
        }
        "shapes" => {
            // 同じ名前の行をまとめてから1つの図形にします
            let mut by_name: Vec<(String, Vec<(String, String)>)> = Vec::new();
            for r in rows {
                let name = g(r, 0).to_string();
                if name.is_empty() {
                    continue;
                }
                let item = (g(r, 1).to_string(), g(r, 2).to_string());
                match by_name.iter_mut().find(|(n, _)| *n == name) {
                    Some((_, v)) => v.push(item),
                    None => by_name.push((name, vec![item])),
                }
            }
            for (_, items) in by_name {
                s.shapes.push(shape::from_rows(&items));
            }
        }
        "images" => {
            // **中身は隣のファイルにあります。** ここで持つのは置き場と
            // 大きさだけで、実体は開く側が径路から読みます
            for r in rows {
                let Some(at) = Pos::parse(g(r, 1)) else { continue };
                s.images.push(book::SheetImage {
                    at,
                    dx_px: g(r, 2).parse().unwrap_or(0.0),
                    dy_px: g(r, 3).parse().unwrap_or(0.0),
                    width_px: g(r, 4).parse().unwrap_or(0.0),
                    height_px: g(r, 5).parse().unwrap_or(0.0),
                    data: Vec::new(),
                });
            }
        }
        _ => {}
    }
}

/// 文書から、役割の印の付いた表を取り出してシートへ入れる。
pub fn take_all(doc: &Document, sheets: &mut [Sheet]) {
    for b in &doc.blocks {
        let Block::Table(t) = b else { continue };
        let Some(role) = t.role.as_deref() else { continue };
        let Some(name) = t.title.as_deref() else { continue };
        let rows = t.text_rows();
        let body = if t.header_row && !rows.is_empty() { &rows[1..] } else { &rows[..] };
        for s in sheets.iter_mut() {
            take(role, name, body, s);
        }
    }
}

// ---------- 字にする / 字から読む ----------

fn table(role: &str, title: &str, head: &[&str], rows: Vec<Vec<String>>) -> Table {
    let cell = |s: &str| Cellbox {
        paragraphs: Document::plain(s).paragraphs().cloned().collect(),
        ..Default::default()
    };
    let mut t = Table {
        role: Some(role.to_string()),
        title: Some(title.to_string()),
        header_row: true,
        rows: vec![head.iter().map(|h| cell(h)).collect()],
        ..Default::default()
    };
    for r in rows {
        t.rows.push(r.iter().map(|x| cell(x)).collect());
    }
    t
}

fn yes(b: bool) -> String {
    b.to_string()
}

fn on(s: &str) -> bool {
    s.eq_ignore_ascii_case("true")
}

fn span(a: Pos, b: Pos) -> String {
    if a == b { a.a1() } else { format!("{}:{}", a.a1(), b.a1()) }
}

fn read_span(s: &str) -> Option<(Pos, Pos)> {
    match s.split_once(':') {
        Some((a, b)) => Some((Pos::parse(a.trim())?, Pos::parse(b.trim())?)),
        None => {
            let p = Pos::parse(s.trim())?;
            Some((p, p))
        }
    }
}

fn col_name(c: u32) -> String {
    let a1 = Pos::new(0, c).a1();
    a1.trim_end_matches(|ch: char| ch.is_ascii_digit()).to_string()
}

fn col_index(s: &str) -> Option<u32> {
    let s = s.trim();
    (!s.is_empty() && s.chars().all(|c| c.is_ascii_alphabetic()))
        .then(|| Pos::parse(&format!("{s}1")).map(|p| p.col))
        .flatten()
}

/// 条件の種類。**原文をそのまま持てる形**にします — 知らない種類を
/// 黙って落とさないためです。
fn cond_kind(k: &CondKind) -> String {
    match k {
        CondKind::Cmp(op, v) => format!("cmp {} {v}", cond_op(*op)),
        CondKind::Between(a, b, inside) => format!("between {a} {b} {inside}"),
        CondKind::Text(s) => format!("text {s}"),
        CondKind::Dup(b) => format!("dup {b}"),
        CondKind::Top(n, bottom) => format!("top {n} {bottom}"),
        CondKind::Avg(above) => format!("avg {above}"),
        CondKind::Bar(s) => format!("bar {s}"),
        CondKind::Scale(a, b, c) => {
            format!("scale {a} {} {c}", b.clone().unwrap_or_else(|| "-".into()))
        }
        CondKind::Icons(s) => format!("icons {s}"),
        CondKind::Formula(s) => format!("formula {s}"),
    }
}

fn read_cond_kind(s: &str) -> Option<CondKind> {
    let mut it = s.splitn(2, ' ');
    let head = it.next()?;
    let rest = it.next().unwrap_or("");
    let w: Vec<&str> = rest.split_whitespace().collect();
    Some(match head {
        "cmp" => CondKind::Cmp(read_cond_op(w.first()?), w.get(1)?.parse().ok()?),
        "between" => CondKind::Between(
            w.first()?.parse().ok()?, w.get(1)?.parse().ok()?, on(w.get(2).copied().unwrap_or("true"))),
        "text" => CondKind::Text(rest.to_string()),
        "dup" => CondKind::Dup(on(rest.trim())),
        "top" => CondKind::Top(w.first()?.parse().ok()?, on(w.get(1).copied().unwrap_or("false"))),
        "avg" => CondKind::Avg(on(rest.trim())),
        "bar" => CondKind::Bar(rest.to_string()),
        "scale" => CondKind::Scale(
            w.first()?.to_string(),
            match w.get(1) {
                Some(&"-") | None => None,
                Some(x) => Some(x.to_string()),
            },
            w.get(2)?.to_string()),
        "icons" => CondKind::Icons(rest.to_string()),
        "formula" => CondKind::Formula(rest.to_string()),
        _ => return None,
    })
}

const COND_OPS: &[(CondOp, &str)] = &[
    (CondOp::Gt, "gt"), (CondOp::Lt, "lt"), (CondOp::Eq, "eq"),
    (CondOp::Ge, "ge"), (CondOp::Le, "le"), (CondOp::Ne, "ne"),
];

fn cond_op(o: CondOp) -> &'static str {
    COND_OPS.iter().find(|(k, _)| *k == o).map(|(_, v)| *v).unwrap_or("eq")
}

fn read_cond_op(s: &str) -> CondOp {
    COND_OPS.iter().find(|(_, v)| *v == s).map(|(k, _)| *k).unwrap_or(CondOp::Eq)
}

/// 掛ける見た目。**三択(入・切・言わない)**なので、言わない物は書きません
fn cond_look(l: &CondLook) -> String {
    let mut out: Vec<String> = Vec::new();
    for (name, v) in [("bold", l.bold), ("italic", l.italic), ("strike", l.strike)] {
        if let Some(b) = v {
            out.push(format!("{name}={b}"));
        }
    }
    for (name, v) in [("color", &l.color), ("fill", &l.fill)] {
        if let Some(x) = v {
            out.push(format!("{name}={x}"));
        }
    }
    out.join(" ")
}

fn read_cond_look(s: &str) -> CondLook {
    let mut l = CondLook::default();
    for part in s.split_whitespace() {
        let Some((k, v)) = part.split_once('=') else { continue };
        match k {
            "bold" => l.bold = Some(on(v)),
            "italic" => l.italic = Some(on(v)),
            "strike" => l.strike = Some(on(v)),
            "color" => l.color = Some(v.to_string()),
            "fill" => l.fill = Some(v.to_string()),
            _ => {}
        }
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holes::filled_sheet;

    /// **役割の印と読み手が揃っているか。**
    ///
    /// [`ROLES`] に足して [`take`] に枝を足し忘れると、書いた物が読めません。
    /// 逆に枝だけ足すと、書き手が作らないので死んだ枝になります。
    #[test]
    fn every_role_is_handled() {
        let s = filled_sheet("売上台帳");
        let 書いた: Vec<String> =
            tables_of(&s).iter().filter_map(|t| t.role.clone()).collect();
        for r in ROLES {
            assert!(
                書いた.iter().any(|x| x == r),
                "役割「{r}」を ROLES に書いたのに、書き手が作りません"
            );
        }
        for r in &書いた {
            assert!(ROLES.contains(&r.as_str()), "書き手が作る役割「{r}」が ROLES に無い");
        }
    }

    /// **役割の名前は英語の識別子。** 書式の一部なので訳しません
    /// (2026-08-26 発注者)。
    #[test]
    fn the_role_names_are_ascii() {
        for r in ROLES {
            assert!(
                r.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "役割「{r}」に英小文字とハイフン以外が入っています"
            );
        }
    }

    /// 列の見出しも英語です。**利用者が直接書く物ではない**ためです。
    #[test]
    fn the_column_headings_are_ascii() {
        let s = filled_sheet("売上台帳");
        for t in tables_of(&s) {
            for c in &t.rows[0] {
                let h = crate::paras_text(&c.paragraphs);
                assert!(
                    h.is_ascii(),
                    "役割 {:?} の見出し「{h}」が英語ではありません",
                    t.role
                );
            }
        }
    }
}
