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
    /// 書式 → 番号。`read_range` の `styleIndex` はこの番号を返す。
    ///
    /// **開いたときに一度だけ組む** — 範囲ごとに組み直すと同じ書式に違う番号が
    /// 付く。番号が指す先(`styles[]` の表そのもの)は `open` の答えに載せて
    /// 渡しきりで、ここには持たない — 表を返し直す命令は無いし、開いている
    /// 間ずっと書式の JSON を抱えることになる
    style_index: BTreeMap<String, usize>,
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

/// **ZIP の配管は向こうへ丸ごと転送する。**
///
/// genoffice の TypeScript は 12 コマンドを**1つの実行ファイル**に流す
/// (`apps/sheets/gateway/xlsx-package-io.ts` の保存も同じ client)。だから
/// `XLSX_SIDECAR_PATH` でこちらに差し替えると、**`save_archive` もこちらに来る**。
/// 設計の「ZIP の配管は触らない・向こうの実装を使う」は、実行ファイルを
/// 分けない限り自動では成らない — **差し替えた途端に保存が死ぬ。**
///
/// 解が転送。向こうのバイナリを子として持ち、**要求の行をそのまま渡して
/// 答えの行をそのまま返す**。CRC32 とマニフェスト照合つきの堅い実装が
/// そのまま効き、こちらは1文字も解釈しない。
struct Plumbing {
    path: String,
    child: Option<(std::process::Child, std::io::BufReader<std::process::ChildStdout>)>,
}

impl Plumbing {
    fn new() -> Plumbing {
        let path = std::env::var("GENOFFICE_SIDECAR").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/dev/genoffice/apps/sheets/native/xlsx-engine/target/release/xlsx-sidecar")
        });
        Plumbing { path, child: None }
    }

    /// 行を渡して行を受け取る。**中身は見ない。**
    fn forward(&mut self, line: &str) -> Result<String, String> {
        if self.child.is_none() {
            let mut c = std::process::Command::new(&self.path)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .map_err(|e| {
                    format!(
                        "向こうのサイドカーを起こせません({}): {e}。\
                         GENOFFICE_SIDECAR で場所を指してください",
                        self.path
                    )
                })?;
            let so = c.stdout.take().ok_or("向こうの stdout が取れません")?;
            self.child = Some((c, std::io::BufReader::new(so)));
        }
        let (c, rd) = self.child.as_mut().unwrap();
        let si = c.stdin.as_mut().ok_or("向こうの stdin が取れません")?;
        writeln!(si, "{line}").map_err(|e| format!("向こうへ書けません: {e}"))?;
        si.flush().map_err(|e| format!("向こうへ流せません: {e}"))?;
        let mut ans = String::new();
        match rd.read_line(&mut ans) {
            Ok(0) => {
                // **落ちたら黙って握りつぶさない。** 次の要求で起こし直す
                self.child = None;
                Err("向こうのサイドカーが答えずに終わりました".into())
            }
            Ok(_) => Ok(ans.trim_end().to_string()),
            Err(e) => {
                self.child = None;
                Err(format!("向こうから読めません: {e}"))
            }
        }
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut sessions: BTreeMap<String, Session> = BTreeMap::new();
    let mut plumbing = Plumbing::new();
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
        let res = handle(&line, &mut sessions, &mut plumbing, &mut seq);
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

/// 1行を捌いて、返す1行を作る。
///
/// **戻りが文字列なのは転送のため。** `Value` に一度入れ直すと、向こうの
/// 答えを組み直すことになる — 通信の言葉はこちらに直す権利がないので、
/// 配管の答えは**文字どおり素通し**にする。
fn handle(
    line: &str,
    sessions: &mut BTreeMap<String, Session>,
    plumbing: &mut Plumbing,
    seq: &mut u64,
) -> String {
    dispatch(line, sessions, plumbing, seq)
}

fn dispatch(
    line: &str,
    sessions: &mut BTreeMap<String, Session>,
    plumbing: &mut Plumbing,
    seq: &mut u64,
) -> String {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return fail("", "invalid_request", &format!("JSON が読めません: {e}")).to_string(),
    };
    let rid = req.get("requestId").and_then(Value::as_str).unwrap_or("").to_string();
    let cmd = match req.get("command").and_then(Value::as_str) {
        Some(c) => c,
        None => return fail(&rid, "invalid_request", "command がありません").to_string(),
    };
    let s = |k: &str| req.get(k).and_then(Value::as_str).unwrap_or("").to_string();

    match cmd {
        "open" => {
            let path = s("path");
            if path.is_empty() {
                return fail(&rid, "invalid_request", "path がありません").to_string();
            }
            match open_book(&path) {
                Ok((book, unsupported)) => {
                    *seq += 1;
                    let id = format!("ow-{seq}");
                    let mut st = Styles::new();
                    // **書式は全シートを通して1つの表にする**(向こうと同じ)
                    for sh in &book.sheets {
                        for c in sh.cells.values() {
                            st.intern(&c.fmt);
                        }
                    }
                    let v = open_result(&id, &path, &book, &unsupported, &st.order);
                    let stamp = stamp_of(&path);
                    // 表は上の答えに載せて渡しきり。**残すのは索引だけ**
                    let style_index = st.index;
                    sessions.insert(id, Session { path, stamp, book, style_index });
                    ok(&rid, v)
                }
                Err(e) => fail(&rid, "workbook_error", &e),
            }
        }
        "read_range" => {
            let sid = s("sessionId");
            let Some(sess) = sessions.get(&sid) else {
                return fail(&rid, "invalid_request", "そのセッションはありません").to_string();
            };
            if stamp_of(&sess.path) != sess.stamp {
                let p = sess.path.clone();
                sessions.remove(&sid);
                return fail(&rid, "workbook_error",
                    &format!("原本が変わったのでセッションを捨てました。開き直してください: {p}"))
                    .to_string();
            }
            let sess = &sessions[&sid];
            let sheet_id = s("sheetId");
            let Some(i) = sheet_index(&sess.book, &sheet_id) else {
                return fail(&rid, "invalid_request", &format!("シートがありません: {sheet_id}")).to_string();
            };
            let sh = &sess.book.sheets[i];
            let r = req.get("range").cloned().unwrap_or(Value::Null);
            let g = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
            let (r0, r1, c0, c1) = (g("startRow"), g("endRow"), g("startColumn"), g("endColumn"));
            let (rows, cols) = sh.extent();
            // **向こうと同じく、シートの外は断る。** 黙って丸めると、
            // 呼ぶ側は「そこは空だった」と受け取る
            if r1 >= rows.max(1) || c1 >= cols.max(1) {
                return fail(&rid, "invalid_request", "Range is outside the worksheet.").to_string();
            }
            ok(&rid, range_result(sh, r0, r1, c0, c1, &sess.style_index))
        }
        "close" => {
            sessions.remove(&s("sessionId"));
            ok(&rid, json!({}))
        }
        // **居座りを止めるだけ。** こちらは全部読んでから答えるので、
        // 途中で止める物が無い(設計「段階索引」の決め)
        "cancel" => ok(&rid, json!({})),
        // **ZIP の配管は向こうへ丸ごと転送する**(設計の「切る場所」)。
        // 解釈しない — 要求の行をそのまま渡し、答えの行をそのまま返す
        "archive_manifest" | "read_entries" | "scan_entries" | "save_archive"
        | "convert_workbook" => match plumbing.forward(line) {
            Ok(answer) => return answer,
            Err(e) => fail(&rid, "io_error", &format!("配管の転送に失敗: {e}")),
        },
        "read_formula_cells" => {
            let sid = s("sessionId");
            let Some(sess) = sessions.get(&sid) else {
                return fail(&rid, "invalid_request", "そのセッションはありません").to_string();
            };
            let sheet_id = s("sheetId");
            let Some(i) = sheet_index(&sess.book, &sheet_id) else {
                return fail(&rid, "invalid_request", &format!("シートがありません: {sheet_id}"))
                    .to_string();
            };
            ok(&rid, formula_cells_result(&sess.book.sheets[i]))
        }
        // **visuals を返していないので、この命令は来ないはず。** 向こうの
        // read_media は open で配った visualId を鍵に引く作りで、こちらは
        // まだ図形を返していない。来たなら想定違いなので、そう言って断る
        "read_media" => fail(
            &rid,
            "unsupported_command",
            "read_media はまだ。open が visuals を返していないので、この命令は来ない想定",
        ),
        "recalc_cells" => fail(
            &rid,
            "unsupported_command",
            "recalc_cells はまだ(設計の「進め方」の2)。読みの突き合わせが先",
        ),
        other => fail(&rid, "invalid_request", &format!("知らない命令: {other}")),
    }
    .to_string()
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

fn open_result(
    session_id: &str,
    path: &str,
    book: &Book,
    unsupported: &[String],
    styles: &[Value],
) -> Value {
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
    v.insert("styles".into(), json!(styles));
    v.insert("dxfStyles".into(), json!([]));
    v.insert("visuals".into(), json!([]));
    v.insert("definedNames".into(), json!(defined));
    v.insert(
        "xNotFilled".into(),
        json!([
            "dxfStyles(条件付き書式の見た目。規則は返しているが見た目の表はまだ)",
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

fn range_result(
    sh: &Sheet,
    r0: u32,
    r1: u32,
    c0: u32,
    c1: u32,
    style_index: &BTreeMap<String, usize>,
) -> Value {
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
            // **書式は open で配った表の番号で指す。** 表に無い(素の書式)なら付けない
            if let Some(v) = style_value(&cell.fmt) {
                if let Some(i) = style_index.get(&v.to_string()) {
                    o.insert("styleIndex".into(), json!(i));
                }
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
            "cells.rich(セルの中で書式が変わる run をモデルが持たない)",
            "autoFilter(モデルが持たない)",
            "conditionalRules の cfvos / colors / dxfIndex / rank / priority",
        ],
    })
}

/// 式のあるセルだけを返す。向こうの `read_formula_cells`。
///
/// 呼ぶ側は依存の連鎖を辿るのに使う。**全部読んでから答える**ので
/// `indexingComplete` は常に真、上限も設けていないので `truncated` は偽。
fn formula_cells_result(sh: &Sheet) -> Value {
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

/// 書式の索引表。**向こうは `styles[]` の索引を返し、セルは番号で指す。**
///
/// officework は `CellFormat` をセルに直に持つので、**同じ書式をまとめて
/// 番号を振り直す**。原本の `style_of`(読んだときの `<c s="…">`)は使わない —
/// あれは保存で原本の styles.xml を据え置くための控えで、番号の意味が
/// 向こうの索引と揃っている保証がない。
///
/// 返すのは**この範囲で実際に使われている書式だけ**。原本の styles.xml を
/// 丸ごと写すと、使っていない何百もの書式が付いてくる。
struct Styles {
    order: Vec<Value>,
    index: BTreeMap<String, usize>,
}

impl Styles {
    fn new() -> Styles {
        Styles { order: Vec::new(), index: BTreeMap::new() }
    }

    /// 書式を1つ入れて索引を返す。**素の書式(既定のまま)は番号を振らない** —
    /// 呼ぶ側は `styleIndex` が無ければ既定で描く
    fn intern(&mut self, f: &sheet::model::CellFormat) -> Option<usize> {
        let v = style_value(f)?;
        let key = v.to_string();
        if let Some(i) = self.index.get(&key) {
            return Some(*i);
        }
        let i = self.order.len();
        self.order.push(v);
        self.index.insert(key, i);
        Some(i)
    }
}

fn edge_value(e: &sheet::model::Edge) -> Option<Value> {
    if !e.on {
        return None;
    }
    let mut o = Map::new();
    o.insert("style".into(), json!(e.style.xlsx()));
    if let Some(c) = e.color {
        o.insert("color".into(), json!(format!("{:06X}", c & 0xFF_FFFF)));
    }
    Some(Value::Object(o))
}

/// `CellFormat` を向こうの `CellStyle` の形へ。**既定のままなら `None`。**
fn style_value(f: &sheet::model::CellFormat) -> Option<Value> {
    use sheet::model::VAlign;
    let mut o = Map::new();
    if let Some(n) = &f.font {
        o.insert("fontFamily".into(), json!(n));
    }
    if let Some(c) = f.size_c {
        o.insert("fontSize".into(), json!(f64::from(c) / 100.0));
    }
    for (k, v) in [
        ("bold", f.bold),
        ("italic", f.italic),
        ("underline", f.underline),
        ("strikethrough", f.strike),
        ("wrapText", f.wrap),
    ] {
        if v {
            o.insert(k.into(), json!(true));
        }
    }
    if let Some(c) = &f.color {
        o.insert("fontColor".into(), json!(c));
    }
    if let Some(c) = &f.fill {
        o.insert("fillColor".into(), json!(c));
    }
    // 揃えの名前は xlsx に書くときと同じ綴りで返す(突き合わせの相手は
    // 生の属性値を見せるので、こちらも畳まずにそのまま出す)
    if let Some(h) = f.align.as_xlsx() {
        o.insert("horizontalAlignment".into(), json!(h));
    }
    let v = match f.valign {
        VAlign::Top => Some("top"),
        VAlign::Middle => Some("center"),
        VAlign::Bottom => None, // xlsx の既定
    };
    if let Some(v) = v {
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
    if o.is_empty() { None } else { Some(Value::Object(o)) }
}
