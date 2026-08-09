//! pyoffice のサイドカー — **genoffice と同じ言葉を喋る**実行ファイル。
//!
//! 設計は `docs/sekkei/pyoffice.ja.md`。要点だけ:
//!
//! - stdin/stdout に **JSON を1行ずつ**。要求 `{version, requestId, command, …}`、
//!   答え `{version, requestId, ok, result|error}`
//! - **通信の言葉は1文字も変えない。** 同じ形で答えれば、genoffice 側は
//!   `XLSX_SIDECAR_PATH` でこの実行ファイルを指すだけでよく、TypeScript は
//!   何も知らなくていい。戻すのも環境変数1つ
//! - 12 のコマンドのうち、**エンジンの仕事は5つ**(`open` `read_range`
//!   `read_formula_cells` `read_media` `recalc_cells`)。ZIP の配管
//!   (`save_archive` など)は向こうの実装が堅いので触らない
//!
//! **まだ全部は喋れない。** 喋れないコマンドは
//! `unsupported_command` で**断る** — それらしい空の答えを返さない。
//! 埋まっていない欄は、答えに `xNotFilled` を添えて名前で言う
//! (向こうに無い欄だが、知らない欄は無視される作りなので邪魔にならない)。

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufWriter, Write};

use serde_json::{Map, Value, json};
use sheet::model::{CondKind, CondOp, Pos};
use sheet::{Book, Sheet};

const PROTOCOL_VERSION: u8 = 1;

/// 開いたブックの居座り。向こうと同じく id で持ち、`close` で捨てる。
///
/// **原本が変わったら捨てる。** 向こうは mtime と大きさで見張っている
/// (設計「埋まっていない穴」の3)。同じ見張りをする — 古い中身を返し続けるのは
/// 「読めない」より悪い。**黙って読み直しもしない** — 呼ぶ側に開き直させる
/// (こちらで読み直すと、その間の編集がどちらの物か分からなくなる)。
struct Session {
    path: String,
    stamp: (u64, u64),
    book: Book,
}

/// ファイルの印(更新時刻, 大きさ)。読めなければ 0 — 消えたことも変化と見る。
fn stamp_of(path: &str) -> (u64, u64) {
    match std::fs::metadata(path) {
        Ok(m) => {
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (t, m.len())
        }
        Err(_) => (0, 0),
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut sessions: BTreeMap<String, Session> = BTreeMap::new();
    let mut seq: u64 = 0;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                let _ = writeln!(out, "{}", fail("", "io_error", &e.to_string()));
                let _ = out.flush();
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let res = handle(&line, &mut sessions, &mut seq);
        let _ = writeln!(out, "{res}");
        let _ = out.flush();
    }
}

fn ok(request_id: &str, result: Value) -> Value {
    json!({"version": PROTOCOL_VERSION, "requestId": request_id, "ok": true, "result": result})
}

fn fail(request_id: &str, code: &str, message: &str) -> Value {
    json!({"version": PROTOCOL_VERSION, "requestId": request_id, "ok": false,
           "error": {"code": code, "message": message}})
}

fn handle(line: &str, sessions: &mut BTreeMap<String, Session>, seq: &mut u64) -> Value {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return fail("", "invalid_request", &format!("JSON が読めません: {e}")),
    };
    let rid = req.get("requestId").and_then(Value::as_str).unwrap_or("").to_string();
    let cmd = match req.get("command").and_then(Value::as_str) {
        Some(c) => c,
        None => return fail(&rid, "invalid_request", "command がありません"),
    };
    let s = |k: &str| req.get(k).and_then(Value::as_str).unwrap_or("").to_string();

    match cmd {
        "open" => {
            let path = s("path");
            if path.is_empty() {
                return fail(&rid, "invalid_request", "path がありません");
            }
            match open_book(&path) {
                Ok((book, unsupported)) => {
                    *seq += 1;
                    let id = format!("ow-{seq}");
                    let v = open_result(&id, &path, &book, &unsupported);
                    let stamp = stamp_of(&path);
                    sessions.insert(id, Session { path, stamp, book });
                    ok(&rid, v)
                }
                Err(e) => fail(&rid, "workbook_error", &e),
            }
        }
        "read_range" => {
            let sid = s("sessionId");
            let Some(sess) = sessions.get(&sid) else {
                return fail(&rid, "invalid_request", "そのセッションはありません");
            };
            if stamp_of(&sess.path) != sess.stamp {
                let p = sess.path.clone();
                sessions.remove(&sid);
                return fail(&rid, "workbook_error",
                    &format!("原本が変わったのでセッションを捨てました。開き直してください: {p}"));
            }
            let sess = &sessions[&sid];
            let sheet_id = s("sheetId");
            let Some(i) = sheet_index(&sess.book, &sheet_id) else {
                return fail(&rid, "invalid_request", &format!("シートがありません: {sheet_id}"));
            };
            let sh = &sess.book.sheets[i];
            let r = req.get("range").cloned().unwrap_or(Value::Null);
            let g = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
            let (r0, r1, c0, c1) = (g("startRow"), g("endRow"), g("startColumn"), g("endColumn"));
            let (rows, cols) = sh.extent();
            // **向こうと同じく、シートの外は断る。** 黙って丸めると、
            // 呼ぶ側は「そこは空だった」と受け取る
            if r1 >= rows.max(1) || c1 >= cols.max(1) {
                return fail(&rid, "invalid_request", "Range is outside the worksheet.");
            }
            ok(&rid, range_result(sh, r0, r1, c0, c1))
        }
        "close" => {
            sessions.remove(&s("sessionId"));
            ok(&rid, json!({}))
        }
        // **居座りを止めるだけ。** こちらは全部読んでから答えるので、
        // 途中で止める物が無い(設計「段階索引」の決め)
        "cancel" => ok(&rid, json!({})),
        // ZIP の配管。**向こうの実装を使う** — ここに来た時点で繋ぎ方が違う
        "archive_manifest" | "read_entries" | "scan_entries" | "save_archive"
        | "convert_workbook" => fail(
            &rid,
            "unsupported_command",
            &format!("{cmd} は ZIP の配管。pyoffice では向こうの実装を使う(設計の「切る場所」)"),
        ),
        "read_formula_cells" | "read_media" | "recalc_cells" => fail(
            &rid,
            "unsupported_command",
            &format!("{cmd} はまだ。読み(open/read_range)の突き合わせが先(設計の「進め方」)"),
        ),
        other => fail(&rid, "invalid_request", &format!("知らない命令: {other}")),
    }
}

/// xlsx を読んで、**開いた時点で計算し直す**(pysheet の `Book.open` と同じ作法)。
/// 原本にキャッシュ値の無い式でも答えを返せる — 向こうは空を返す所(設計参照)。
fn open_book(path: &str) -> Result<(Book, Vec<String>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: 読めない: {e}"))?;
    let (mut book, rep) = sheet::xlsx::read(std::io::Cursor::new(&bytes))
        .map_err(|e| format!("{path}: xlsx として読めない: {e}"))?;
    sheet::recalc_all(&mut book);
    // **読めなかった物は名前で言う。** 黙って落とすのが一番悪い
    Ok((book, rep.unsupported.into_iter().map(|(w, n)| format!("{w} ×{n}")).collect()))
}

fn sheet_index(book: &Book, sheet_id: &str) -> Option<usize> {
    // 向こうの id は `sheet-{workbook.xml の sheetId}`。**こちらは番号で作る** —
    // モデルが sheetId を持たないため。呼ぶ側は open で受けた物を返すだけなので
    // 中身は問われないが、**向こうの id とは一致しない**(突き合わせは名前で見る)
    sheet_id
        .strip_prefix("sheet-")
        .and_then(|n| n.parse::<usize>().ok())
        .filter(|n| *n >= 1 && *n <= book.sheets.len())
        .map(|n| n - 1)
}

fn a1_col(mut c: u32) -> String {
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

fn rng(a: Pos, b: Pos) -> Value {
    json!({"startRow": a.row, "startColumn": a.col, "endRow": b.row, "endColumn": b.col})
}

fn open_result(session_id: &str, path: &str, book: &Book, unsupported: &[String]) -> Value {
    let name = path.rsplit('/').next().unwrap_or(path);
    let mut sheets = Vec::new();
    let mut defined = Vec::new();
    for (i, sh) in book.sheets.iter().enumerate() {
        let (rows, cols) = sh.extent();
        let mut widths: Vec<Value> = Vec::new();
        for (c, w) in &sh.col_width {
            widths.push(json!({"startColumn": c, "endColumn": c, "width": w,
                               "hidden": sh.col_hidden.contains(c),
                               "outlineLevel": sh.col_outline.get(c).copied().unwrap_or(0),
                               "collapsed": false}));
        }
        // 幅は既定のまま隠しただけ・畳んだだけの列も落とさない
        for c in sh.col_hidden.iter().chain(sh.col_outline.keys()) {
            if !sh.col_width.contains_key(c) {
                widths.push(json!({"startColumn": c, "endColumn": c, "width": Value::Null,
                                   "hidden": sh.col_hidden.contains(c),
                                   "outlineLevel": sh.col_outline.get(c).copied().unwrap_or(0),
                                   "collapsed": false}));
            }
        }
        let comments: Vec<Value> = sh
            .comments
            .iter()
            .map(|(p, t)| json!({"row": p.row, "column": p.col, "author": "", "text": t}))
            .collect();
        let tables: Vec<Value> = sh
            .tables
            .iter()
            .map(|t| {
                json!({"range": rng(t.a, t.b),
                       "headerRowCount": u32::from(t.header),
                       "showRowStripes": t.banded_rows,
                       "showColumnStripes": t.banded_cols,
                       "styleName": t.name})
            })
            .collect();
        for (nm, r) in &sh.names {
            defined.push(json!({"name": nm, "formula": format!("'{}'!{}", sh.name, r)}));
        }
        sheets.push(json!({
            "id": format!("sheet-{}", i + 1),
            "name": sh.name,
            "rowCount": rows,
            "columnCount": cols,
            "columnWidths": widths,
            "defaultRowHeight": Value::Null,
            "defaultColumnWidth": sh.default_col_width,
            "freeze": sh.freeze.as_ref().map(|f| json!({
                "frozenRows": f.frozen_rows, "frozenColumns": f.frozen_columns})),
            "hidden": sh.hidden,
            "tabColor": sh.tab_color,
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
    // 部品の数は ZIP を開き直さないと分からない。**0 で誤魔化さず** null
    v.insert("entryCount".into(), Value::Null);
    v.insert("sheets".into(), json!(sheets));
    v.insert("styles".into(), json!([]));
    v.insert("dxfStyles".into(), json!([]));
    v.insert("visuals".into(), json!([]));
    v.insert("definedNames".into(), json!(defined));
    v.insert(
        "xNotFilled".into(),
        json!([
            "styles / dxfStyles(書式の索引表。CellFormat から組み直す — 次の便)",
            "visuals(図形・画像・グラフ)",
            "sparklines / pivotTables / pivotRanges",
            "entryCount",
            "defaultRowHeight",
            "comments.author(モデルが持たない)",
            "definedNames.sheetIndex(モデルは参照先のシートに載せている)",
            "rowCount/columnCount は <dimension> ではなく**実際に中身のある範囲**",
        ]),
    );
    // **読めなかった物を答えに載せる。** 呼ぶ側が知らない欄なので邪魔にならず、
    // 突き合わせでは「何を落としたか」がそのまま出る
    v.insert("xUnsupported".into(), json!(unsupported));
    Value::Object(v)
}

fn cell_value(sh: &Sheet, p: Pos) -> Value {
    match sh.value(p) {
        sheet::Value::Empty => Value::Null,
        sheet::Value::Number(n) => json!(n),
        sheet::Value::Text(s) => json!(s),
        sheet::Value::Bool(b) => json!(b),
        // **誤りは文字として返す。** 向こうの CellValue に誤りの型が無い
        sheet::Value::Error(e) => json!(e),
    }
}

fn cond_value(k: &CondKind) -> (String, Vec<String>, Option<String>) {
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

fn range_result(sh: &Sheet, r0: u32, r1: u32, c0: u32, c1: u32) -> Value {
    let inside = |p: Pos| p.row >= r0 && p.row <= r1 && p.col >= c0 && p.col <= c1;

    let mut cells = Vec::new();
    for r in r0..=r1 {
        for c in c0..=c1 {
            let p = Pos { row: r, col: c };
            let Some(cell) = sh.get(p) else { continue };
            let v = cell_value(sh, p);
            let f = cell.formula.as_ref().map(|f| format!("={f}"));
            if v.is_null() && f.is_none() {
                continue;
            }
            let mut o = Map::new();
            o.insert("row".into(), json!(r));
            o.insert("column".into(), json!(c));
            o.insert("value".into(), v);
            if let Some(f) = f {
                o.insert("formula".into(), json!(f));
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
        rows.push(json!({"row": r, "height": h, "hidden": hidden, "outlineLevel": lvl}));
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
        .map(|c| {
            let (kind, formulas, op) = cond_value(&c.kind);
            json!({"ranges": [rng(c.range.0, c.range.1)], "ruleType": kind,
                   "operator": op, "formulas": formulas, "priority": 1,
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
        // **鍵は掛けない・掛けた振りもしない**(SEKKEI writer の保護と同じ作法)
        "sheetProtection": json!({"protected": sh.protected, "hasPassword": false}),
        // **全部読んでから答える**ので、いつも索引は済んでいる(設計「段階索引」)
        "indexedThroughRow": r1,
        "indexingComplete": true,
        "xNotFilled": [
            "cells.styleIndex(styles の索引表がまだ)",
            "cells.rich(セルの中で書式が変わる run をモデルが持たない)",
            "autoFilter(モデルが持たない)",
            "conditionalRules の cfvos / colors / dxfIndex / rank / priority",
        ],
    })
}
