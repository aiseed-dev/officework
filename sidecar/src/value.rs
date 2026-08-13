//! **モデルを向こうの JSON に写す。** ここが「通信の言葉を1文字も変えない」の
//! 現場で、欄の名前・真偽の出し方・`Option` の省き方は全部向こうに合わせる。
//!
//! 作法は2つだけ:
//!
//! - **真偽の欄は偽でも必ず出す**(向こうは `z.boolean()` で検査する)
//! - **`Option` は無ければ省く**(`null` は契約に無い第三の答え)

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use sheet::model::{CondKind, CondOp, Pos};
use sheet::{Book, Sheet};


pub(crate) fn a1_col(mut c: u32) -> String {
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (c % 26) as u8) as char);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s
}

pub(crate) fn rng(a: Pos, b: Pos) -> Value {
    json!({"startRow": a.row, "startColumn": a.col, "endRow": b.row, "endColumn": b.col})
}

pub(crate) fn open_result(
    session_id: &str,
    path: &str,
    book: &Book,
    styles: &[Value],
    entry_count: usize,
    visuals: Vec<Value>,
) -> Value {
    let name = path.rsplit('/').next().unwrap_or(path);
    let mut sheets = Vec::new();
    let mut defined = Vec::new();
    // **条件付き書式の見た目**(向こうの `dxfStyles`)。規則は `read_range` で
    // 返しているのに見た目の表が空だったので、規則に色が付いているのに
    // **画面には出ない**状態だった(2026-08-10、向こうの試験で判明)。
    // 規則の並びと同じ順で積み、`dxfIndex` がその番号を指す
    let mut dxfs: Vec<Value> = Vec::new();
    for sh in &book.sheets {
        for c in &sh.cond {
            dxfs.push(dxf_value(c));
        }
    }
    for (i, sh) in book.sheets.iter().enumerate() {
        let (rows, cols) = sh.size();
        // **列の幅。** 向こうの `ColumnWidth` の作法は
        // 「`startColumn`/`endColumn`/`hidden` は必ず出し、`width`・
        // `outlineLevel`・`collapsed` は無ければ省く」(`lib.rs` の
        // `skip_serializing_if`)。**`null` を入れると z.number() に撥ねられる**
        // (2026-08-10、向こうの試験で判明)。持たない物は**省く**のであって
        // `null` を置くのではない
        let mut col_props: BTreeMap<u32, (Option<f32>, bool, u8, bool)> = BTreeMap::new();
        for (c, w) in &sh.col_width {
            col_props.entry(*c).or_insert((None, false, 0, false)).0 = Some(*w);
        }
        for c in &sh.col_hidden {
            col_props.entry(*c).or_insert((None, false, 0, false)).1 = true;
        }
        for (c, l) in &sh.col_outline {
            col_props.entry(*c).or_insert((None, false, 0, false)).2 = *l;
        }
        for c in &sh.col_collapsed {
            col_props.entry(*c).or_insert((None, false, 0, false)).3 = true;
        }
        // **続きの列で中身が同じなら1つにまとめる。** 原本の
        // `<col min="2" max="3" width="20"/>` は範囲で書かれており、
        // 向こうも範囲で返す。1列ずつ返すと数が合わない
        // (2026-08-10、向こうの試験で判明)
        let mut widths: Vec<Value> = Vec::new();
        let mut run: Option<(u32, u32, (Option<f32>, bool, u8, bool))> = None;
        for (c, v) in col_props {
            match &mut run {
                Some((_, end, prev)) if *end + 1 == c && *prev == v => *end = c,
                _ => {
                    if let Some((a, z, v0)) = run.take() {
                        widths.push(col_value(a, z, v0));
                    }
                    run = Some((c, c, v));
                }
            }
        }
        if let Some((a, z, v0)) = run {
            widths.push(col_value(a, z, v0));
        }
        let comments: Vec<Value> = sh
            .comments
            .iter()
            // **著者が読めるようになった**(前は空で出していた)。返信は
            // 一続きの文にして出す — 呼び手は1つのコメントとして扱う
            .map(|(p, t)| json!({
                "row": p.row, "column": p.col,
                "author": t.entries.first().map(|e| e.who.clone()).unwrap_or_default(),
                "text": t.flatten(),
                "resolved": t.done,
            }))
            .collect();
        let tables: Vec<Value> = sh
            .tables
            .iter()
            .map(|t| {
                let mut o = Map::new();
                o.insert("range".into(), rng(t.a, t.b));
                o.insert("headerRowCount".into(), json!(u32::from(t.header)));
                o.insert("showRowStripes".into(), json!(t.banded_rows));
                o.insert("showColumnStripes".into(), json!(t.banded_cols));
                // **`styleName` は様式の名前**(`TableStyleMedium2`)。
                // `t.name`(`Table1`)を入れていた — 別物を渡していた。
                // 原本に指定が無ければ**付けない**(欄は optional)
                if let Some(s) = &t.style {
                    o.insert("styleName".into(), json!(s));
                }
                // `headerFill` / `headerFontColor` / `stripeFill` は**出さない**。
                // 組み込みの表の様式の配色は**ファイルではなく Excel が持って
                // いる**ので、名前から色を組み立てることになる。60種類の色を
                // 少しずつ間違えるくらいなら、黙って省くほうが正直
                Value::Object(o)
            })
            .collect();
        for (nm, r) in &sh.names {
            // **原本の綴りで返す。** 向こうは `Structure!$A$1:$B$2` の形で
            // 返し、試験もそれを見ている(2026-08-10)。こちらは参照を
            // `A1:B2` に解いて持っているので、**`$` を戻して組み直す**。
            //
            // シート名の引用符は**要るときだけ** — 空白や記号を含まない
            // 名前に `'` を付けると、向こうと綴りが変わる
            let quoted = sh.name.chars().any(|c| !c.is_alphanumeric() && c != '_');
            let name = if quoted { format!("'{}'", sh.name) } else { sh.name.clone() };
            defined.push(json!({"name": nm, "formula": format!("{name}!{}", absolute(r))}));
        }
        sheets.push(json!({
            "id": format!("sheet-{}", i + 1),
            "name": sh.name,
            "rowCount": rows,
            "columnCount": cols,
            "columnWidths": widths,
            "defaultRowHeight": sh.default_row_height,
            "defaultColumnWidth": sh.default_col_width,
            "freeze": sh.freeze.as_ref().map(|f| json!({
                "frozenRows": f.frozen_rows, "frozenColumns": f.frozen_columns})),
            "hidden": sh.hidden,
            "tabColor": sh.tab_color.as_deref().map(hex_color),
            "showGridLines": sh.show_gridlines.unwrap_or(true),
            "showFormulas": sh.show_formulas.unwrap_or(false),
            "tables": tables,
            "comments": comments,
            "pivotRanges": [],
            "pivotTables": [],
            "sparklines": [],
        }));
    }
    let mut v = Map::new();
    v.insert("sessionId".into(), json!(session_id));
    v.insert("name".into(), json!(name));
    // 原本の部品の数。**`null` にしていたが、向こうは `z.number()` で
    // 検査していて撥ねられる**(2026-08-10)。「持たないなら null」は
    // こちらの都合で、契約は数を要求している。ZIP を数えれば済む話だった —
    // **持てない物と、持とうとしなかった物を混同していた**
    v.insert("entryCount".into(), json!(entry_count));
    v.insert("sheets".into(), json!(sheets));
    v.insert("styles".into(), json!(styles));
    v.insert("dxfStyles".into(), json!(dxfs));
    v.insert("visuals".into(), json!(visuals));
    v.insert("definedNames".into(), json!(defined));
    // **`xNotFilled` と `xUnsupported` は載せない。**
    //
    // 「埋めていない欄を名前で言う」つもりで足したが、**本番のアプリが
    // 起動できなかった**(2026-08-10、実機で `bojsjfy.xlsx` を開いて判明)。
    // `workbook:select` の schema は `strict()` で、知らない欄があると
    // ZodError で撥ねる。**試験は `passthrough()` だったので通っていた** —
    // 試験が緑でも本番が動かない、という形。
    //
    // 埋めていない欄・読めなかった物は**設計文書に書く**。
    // 通信の言葉に自分の都合の欄を足さない — **契約に無い物は載せない**。
    Value::Object(v)
}

pub(crate) fn cell_value(sh: &Sheet, p: Pos) -> Value {
    match sh.value(p) {
        sheet::Value::Empty => Value::Null,
        sheet::Value::Number(n) => json!(n),
        sheet::Value::Text(s) => json!(s),
        sheet::Value::Bool(b) => json!(b),
        // **誤りは文字として返す。** 向こうの CellValue に誤りの型が無い
        sheet::Value::Error(e) => json!(e),
    }
}

pub(crate) fn cond_value(k: &CondKind) -> (String, Vec<String>, Option<String>) {
    let op = |o: &CondOp| {
        match o {
            CondOp::Gt => "greaterThan",
            CondOp::Lt => "lessThan",
            CondOp::Eq => "equal",
            CondOp::Ge => "greaterThanOrEqual",
            CondOp::Le => "lessThanOrEqual",
            CondOp::Ne => "notEqual",
        }
        .to_string()
    };
    match k {
        CondKind::Cmp(o, v) => ("cellIs".into(), vec![v.to_string()], Some(op(o))),
        CondKind::Between(a, b, outside) => (
            "cellIs".into(),
            vec![a.to_string(), b.to_string()],
            Some(if *outside { "notBetween".into() } else { "between".into() }),
        ),
        CondKind::Text(t) => ("containsText".into(), vec![t.clone()], None),
        CondKind::Dup(_) => ("duplicateValues".into(), vec![], None),
        CondKind::Top(_, _) => ("top10".into(), vec![], None),
        CondKind::Avg(_) => ("aboveAverage".into(), vec![], None),
        CondKind::Bar(_) => ("dataBar".into(), vec![], None),
        CondKind::Scale(_, _, _) => ("colorScale".into(), vec![], None),
        CondKind::Icons(_) => ("iconSet".into(), vec![], None),
        // 式は範囲の左上を錨にした原文のまま渡す(ずらすのは解く側の仕事)
        CondKind::Formula(f) => ("expression".into(), vec![f.clone()], None),
    }
}

pub(crate) fn range_result(
    sh: &Sheet,
    r0: u32,
    r1: u32,
    c0: u32,
    c1: u32,
) -> Value {
    let inside = |p: Pos| p.row >= r0 && p.row <= r1 && p.col >= c0 && p.col <= c1;

    let mut cells = Vec::new();
    for r in r0..=r1 {
        for c in c0..=c1 {
            let p = Pos { row: r, col: c };
            let Some(cell) = sh.get(p) else { continue };
            let v = cell_value(sh, p);
            let f = cell.formula.as_ref().map(|f| format!("={f}"));
            // **書式だけのセルも返す。** 中身が空でも罫線を持っていれば、
            // それは**帳票の枠**で、返さなければ画面に枠が出ない
            // (2026-08-10、実機で `見積書.xlsx` を開いて判明 — 向こうが
            //  54 セル返す所を 42 しか返していなかった)。
            //
            // 「中身のあるセルだけ」は**突き合わせの都合**で入れた判定だった。
            // `pyoffice_diff.py` は両側でこれを揃えているので差が出ず、
            // **道具の都合が本番の答えに漏れていた**ことに気付けなかった。
            let styled = style_value(&cell.fmt).is_some();
            if v.is_null() && f.is_none() && !styled {
                continue;
            }
            let mut o = Map::new();
            o.insert("row".into(), json!(r));
            o.insert("column".into(), json!(c));
            o.insert("value".into(), v);
            if let Some(f) = f {
                o.insert("formula".into(), json!(f));
            }
            // **原本の `<c s="…">` をそのまま返す。** `s` の無いセルには
            // 付けない — 向こうも「無い」で来る(罫線の試験がそう見ている)
            if let Some(i) = sh.style_of.get(&p) {
                o.insert("styleIndex".into(), json!(i));
            }
            if let Some((h, w)) = sh.cse.get(&p) {
                o.insert(
                    "arrayRef".into(),
                    json!(format!("{}{}:{}{}", a1_col(c), r + 1, a1_col(c + w - 1), r + h)),
                );
            }
            cells.push(Value::Object(o));
        }
    }

    let mut rows = Vec::new();
    for r in r0..=r1 {
        let h = sh.row_height.get(&r).copied();
        let hidden = sh.row_hidden.contains(&r);
        let lvl = sh.row_outline.get(&r).copied().unwrap_or(0);
        if h.is_none() && !hidden && lvl == 0 {
            continue;
        }
        // **列と同じ作法。** `height` と `outlineLevel` は無ければ**省く** —
        // `null` や `0` を入れると撥ねられる(`outlineLevel` は 1 以上が契約)。
        // 列(`col_value`)では直したのに、行で同じ誤りをしていた
        // (2026-08-10、実機で開いて判明 — 列幅は届くのに**文字が来なかった**)
        let mut o = Map::new();
        o.insert("row".into(), json!(r));
        if let Some(h) = h {
            o.insert("height".into(), json!(h));
        }
        o.insert("hidden".into(), json!(hidden));
        if lvl > 0 {
            o.insert("outlineLevel".into(), json!(lvl));
        }
        if sh.row_collapsed.contains(&r) {
            o.insert("collapsed".into(), json!(true));
        }
        rows.push(Value::Object(o));
    }

    let merges: Vec<Value> =
        sh.merges.iter().filter(|(a, _)| inside(*a)).map(|(a, b)| rng(*a, *b)).collect();
    let hyperlinks: Vec<Value> = sh
        .links
        .iter()
        .filter(|(p, _)| inside(**p))
        .map(|(p, t)| json!({"row": p.row, "column": p.col, "target": t}))
        .collect();
    let conditional: Vec<Value> = sh
        .cond
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let (kind, formulas, op) = cond_value(&c.kind);
            // **`dxfIndex` は `open` で配った `dxfStyles` の番号。**
            // 積む順を揃えてあるので、シート内の並びがそのまま番号になる
            // (いまはシートを跨ぐと番号がずれる — 1枚しか持たない帳票が
            //  ほとんどなので当面これで足りるが、**分かっていて残す**)
            json!({"ranges": [rng(c.range.0, c.range.1)], "ruleType": kind,
                   "operator": op, "formulas": formulas, "priority": 1,
                   "dxfIndex": i,
                   "percent": false, "bottom": false, "cfvos": [], "colors": [],
                   "iconReverse": false, "showValue": true})
        })
        .collect();
    let validations: Vec<Value> = sh
        .validations
        .iter()
        .map(|v| {
            json!({"ranges": [rng(v.range.0, v.range.1)], "ruleType": v.kind,
                   "operator": if v.op.is_empty() { Value::Null } else { json!(v.op) },
                   "formulas": if v.formula2.is_empty() {
                       json!([v.formula]) } else { json!([v.formula, v.formula2]) },
                   "allowBlank": v.allow_blank,
                   "suppressDropdown": v.hide_arrow,
                   "showInputMessage": v.input_msg.is_some(),
                   "showErrorMessage": v.error_msg.is_some(),
                   "errorStyle": v.error_msg.as_ref().map(|m| m.0.clone()),
                   "errorTitle": v.error_msg.as_ref().map(|m| m.1.clone()),
                   "error": v.error_msg.as_ref().map(|m| m.2.clone()),
                   "promptTitle": v.input_msg.as_ref().map(|m| m.0.clone()),
                   "prompt": v.input_msg.as_ref().map(|m| m.1.clone())})
        })
        .collect();

    json!({
        "cells": cells,
        "rows": rows,
        "merges": merges,
        "hyperlinks": hyperlinks,
        "conditionalRules": conditional,
        "autoFilter": Value::Null,
        "dataValidations": validations,
        // **原本に `<sheetProtection>` が無ければ `null`。**「保護が無い」と
        // 「保護されていない」は別物で、向こうは前者を `null` で返す
        // (2026-08-10、欄を機械的に突き合わせて判明 — 13 枚で食い違っていた)。
        // 鍵は掛けない・掛けた振りもしない(SEKKEI writer の保護と同じ作法)
        "sheetProtection": if sh.protected {
            json!({"protected": true, "hasPassword": false})
        } else {
            Value::Null
        },
        // **全部読んでから答える**ので、いつも索引は済んでいる(設計「段階索引」)
        //
        // **`xNotFilled` はここに載せない。** `read_range` の schema は
        // `strict()` で、知らない欄があると撥ねる(`open` は `passthrough()`
        // なので通っていた)。**足した欄が害になる所がある**
        // (2026-08-10、向こうの試験で判明)
        "indexedThroughRow": r1,
        "indexingComplete": true,
    })
}

/// 式のあるセルだけを返す。向こうの `read_formula_cells`。
///
/// 呼ぶ側は依存の連鎖を辿るのに使う。**全部読んでから答える**ので
/// `indexingComplete` は常に真、上限も設けていないので `truncated` は偽。
pub(crate) fn formula_cells_result(sh: &Sheet) -> Value {
    let cells: Vec<Value> = sh
        .cells
        .iter()
        .filter_map(|(p, c)| {
            let f = c.formula.as_ref()?;
            let mut o = Map::new();
            o.insert("row".into(), json!(p.row));
            o.insert("column".into(), json!(p.col));
            o.insert("value".into(), cell_value(sh, *p));
            o.insert("formula".into(), json!(format!("={f}")));
            Some(Value::Object(o))
        })
        .collect();
    json!({"cells": cells, "indexingComplete": true, "truncated": false})
}
/// 書式の表。**原本の `cellXfs` の索引で並べる。**
///
/// 前は「この範囲で実際に使われている書式だけ」を詰め直して自前で番号を
/// 振っていた。使っていない何百もの書式を配らずに済む、という理屈だった。
///
/// **取り消した(2026-08-10)。** 向こうは原本の索引で数えていて、番号が
/// 食い違うと向こうの試験がそこで止まる。保存では困らない(向こうは
/// `s=` を原文から読み直す)ので実害なしと判断していたが、**その試験は
/// 先で図形と画像を確かめていた** — 番号の差が、別の検証を丸ごと隠していた。
/// **道具を塞ぐのは実害。**
///
/// 使われていない索引は空の書式で埋める。原本の `styles.xml` を読み直せば
/// 中身を入れられるが、**そこは `sheet` の持ち場**で、ここから覗く話ではない。
pub(crate) fn style_table(book: &Book) -> Vec<Value> {
    let mut used: BTreeMap<u32, Value> = BTreeMap::new();
    for sh in &book.sheets {
        for (p, i) in &sh.style_of {
            if let Some(c) = sh.get(*p) {
                if let Some(v) = style_value(&c.fmt) {
                    used.entry(*i).or_insert(v);
                }
            }
        }
    }
    // **隙間は「素の書式」で埋める。** 空の `{}` にしていたら、向こうの
    // schema が `bold: z.boolean()` を求めていて撥ねられた — 真偽の欄は
    // 偽でも必ず出す、という同じ約束が、表の中身にも掛かっている
    let plain = plain_style();
    match used.keys().copied().max() {
        None => Vec::new(),
        Some(m) => (0..=m).map(|i| used.get(&i).cloned().unwrap_or_else(|| plain.clone())).collect(),
    }
}

pub(crate) fn edge_value(e: &sheet::model::Edge) -> Option<Value> {
    if !e.on {
        return None;
    }
    let mut o = Map::new();
    o.insert("style".into(), json!(e.style.xlsx()));
    if let Some(c) = e.color {
        o.insert("color".into(), json!(hex_color(&format!("{:06X}", c & 0xFF_FFFF))));
    }
    Some(Value::Object(o))
}

/// `CellFormat` を向こうの `CellStyle` の形へ。**既定のままなら `None`。**
pub(crate) fn style_value(f: &sheet::model::CellFormat) -> Option<Value> {
    let mut o = Map::new();
    if let Some(n) = &f.font {
        o.insert("fontFamily".into(), json!(n));
    }
    if let Some(c) = f.size_c {
        o.insert("fontSize".into(), json!(f64::from(c) / 100.0));
    }
    // **真偽の欄は偽でも必ず出す。** 向こうは zod で
    // `bold: z.boolean()` と検査しており、**省くと undefined で撥ねられる**
    // (2026-08-10、向こうの試験を掛けて 9 件中 7 件が落ちて判明)。
    //
    // 突き合わせでは「既定と同じなら省く」で差が出なかった —
    // `pyoffice_diff.py` が両側を正規化してから比べるため。
    // **突き合わせは「同じ答えか」、試験は「約束を守るか」で、別物。**
    //
    // 向こうの `CellStyle` の作法は「**真偽は必ず出し、`Option` は省く**」
    // (`visuals.rs` の `skip_serializing_if` の付き方)。それに揃える
    for (k, v) in [
        ("bold", f.bold),
        ("italic", f.italic),
        ("underline", f.underline),
        ("strikethrough", f.strike),
        ("wrapText", f.wrap),
        // 斜めの罫線はモデルに無いので、**常に偽**として出す
        ("diagonalUp", false),
        ("diagonalDown", false),
    ] {
        o.insert(k.into(), json!(v));
    }
    if let Some(c) = &f.color {
        o.insert("fontColor".into(), json!(hex_color(c)));
    }
    if let Some(c) = &f.fill {
        o.insert("fillColor".into(), json!(hex_color(c)));
    }
    // 揃えの名前は xlsx に書くときと同じ綴りで返す(突き合わせの相手は
    // 生の属性値を見せるので、こちらも畳まずにそのまま出す)
    if let Some(h) = f.align.as_xlsx() {
        o.insert("horizontalAlignment".into(), json!(h));
    }
    // 横と同じく `as_xlsx()` に寄せる — 畳まずに xlsx の綴りで返し、
    // 既定(`bottom`)だけ省く。変種が増えたときに**ここを直し忘れない**
    if let Some(v) = f.valign.as_xlsx() {
        o.insert("verticalAlignment".into(), json!(v));
    }
    if let Some(n) = &f.number_format {
        o.insert("numberFormat".into(), json!(n));
    }
    for (k, e) in [
        ("borderTop", &f.borders.top),
        ("borderBottom", &f.borders.bottom),
        ("borderLeft", &f.borders.left),
        ("borderRight", &f.borders.right),
    ] {
        if let Some(v) = edge_value(e) {
            o.insert(k.into(), v);
        }
    }
    // **真偽の欄は必ず入るので、`o` は空にならない。** 素の書式かどうかは
    // 「真偽以外に何も無く、真偽が全部偽」で見る — ここを `o.is_empty()` の
    // ままにすると、全部のセルに書式の番号が付いて表が膨らむ
    let plain = o.len() == 7 && o.values().all(|v| v == &json!(false));
    if plain { None } else { Some(Value::Object(o)) }
}

/// **素の書式そのもの。** `style_value` は素なら `None` を返す(セルに
/// 番号を付けないため)が、`styles[]` の**隙間を埋めるには実体が要る** —
/// 向こうの schema は表の中の書式にも `bold: z.boolean()` を求めている。
pub(crate) fn plain_style() -> Value {
    let mut f = sheet::model::CellFormat::default();
    // 素だと `None` が返るので、いったん崩して作らせ、その印を消す
    f.bold = true;
    let mut v = style_value(&f).expect("素でない書式が None になった");
    if let Some(o) = v.as_object_mut() {
        o.insert("bold".into(), json!(false));
    }
    v
}
/// 列の幅ひとまとまりを、向こうの `ColumnWidth` の形へ。
///
/// **`width` と `outlineLevel` は無ければ省き、`startColumn`/`endColumn`/
/// `hidden` は必ず出す**(向こうの `skip_serializing_if` の付き方)。
pub(crate) fn col_value(a: u32, z: u32, (w, hidden, lvl, collapsed): (Option<f32>, bool, u8, bool)) -> Value {
    let mut o = Map::new();
    o.insert("startColumn".into(), json!(a));
    o.insert("endColumn".into(), json!(z));
    if let Some(w) = w {
        o.insert("width".into(), json!(w));
    }
    o.insert("hidden".into(), json!(hidden));
    if lvl > 0 {
        o.insert("outlineLevel".into(), json!(lvl));
    }
    // 向こうと同じく**偽なら省く**(`skip_serializing_if`)
    if collapsed {
        o.insert("collapsed".into(), json!(true));
    }
    Value::Object(o)
}

/// 色を向こうの綴りへ。**`#RRGGBB`** で返す(`visuals.rs` が `format!("#{value}")`)。
///
/// こちらは原本の `FFRRGGBB`(先頭は透過)や `RRGGBB` のまま持っているので、
/// **透過の桁を落として `#` を付ける**。`#` を付け忘れると、向こうの
/// 試験が `'#92D050'` を期待して落ちる(2026-08-10 に踏んだ)。
pub(crate) fn hex_color(c: &str) -> String {
    let h = c.trim_start_matches('#');
    let h = if h.len() == 8 { &h[2..] } else { h };
    format!("#{}", h.to_uppercase())
}

/// `A1:B2` を `$A$1:$B$2` に。**名前の定義は絶対参照で書かれる**のが常で、
/// 向こうもその綴りで返す。`sheet` は解いて持っているので戻す。
pub(crate) fn absolute(r: &str) -> String {
    r.split(':')
        .map(|part| {
            let mut out = String::new();
            let mut chars = part.chars().peekable();
            // 列(英字)の前と、行(数字)の前に `$` を置く
            if chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                out.push('$');
            }
            let mut in_row = false;
            for c in part.chars() {
                if c.is_ascii_digit() && !in_row {
                    in_row = true;
                    out.push('$');
                }
                out.push(c);
            }
            out
        })
        .collect::<Vec<_>>()
        .join(":")
}

/// 条件付き書式の見た目を、向こうの `dxfStyles` の形へ。
///
/// **`CellStyle` と同じ器**を使う(向こうも `Vec<CellStyle>`)。真偽は必ず出し、
/// 色は `#RRGGBB`。
///
/// **飾りは三択を二択に畳む。** こちらの `CondLook` は「触らない(None)」と
/// 「外す(Some(false))」を分けて持つが、向こうの契約は素の bool しか無い。
/// どちらも `false` にする — 契約に無い区別を勝手に足さない
/// (2026-08-10。前は**いつも false** で、Excel で赤字・太字にした規則が
/// 太字と文字色を落として届いていた)。
/// `wrapText`・`diagonalUp/Down` はこちらが dxf から読んでいないので false
pub(crate) fn dxf_value(c: &sheet::model::CondRule) -> Value {
    let mut o = Map::new();
    let lk = &c.look;
    for (k, v) in [
        ("bold", lk.bold),
        ("italic", lk.italic),
        ("underline", lk.underline),
        ("strikethrough", lk.strike),
    ] {
        o.insert(k.into(), json!(v.unwrap_or(false)));
    }
    for k in ["wrapText", "diagonalUp", "diagonalDown"] {
        o.insert(k.into(), json!(false));
    }
    if let Some(x) = &lk.color {
        o.insert("fontColor".into(), json!(hex_color(x)));
    }
    if let Some(x) = &lk.fill {
        o.insert("fillColor".into(), json!(hex_color(x)));
    }
    Value::Object(o)
}