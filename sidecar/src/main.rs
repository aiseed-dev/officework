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
    // 計算のための居座り(径路 → 開いたブック)。向こうも Model を residents させる
    let mut resident: BTreeMap<String, Resident> = BTreeMap::new();
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
        let res = handle(&line, &mut sessions, &mut plumbing, &mut resident, &mut seq);
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
    resident: &mut BTreeMap<String, Resident>,
    seq: &mut u64,
) -> String {
    dispatch(line, sessions, plumbing, resident, seq)
}

fn dispatch(
    line: &str,
    sessions: &mut BTreeMap<String, Session>,
    plumbing: &mut Plumbing,
    resident: &mut BTreeMap<String, Resident>,
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
                Ok((book, unsupported, entry_count)) => {
                    *seq += 1;
                    // **セッションの id は UUID。** `ow-1` のような自前の
                    // 名前にしていたが、向こうは `z.uuid()` で検査していて
                    // 撥ねられる(2026-08-10、向こうの試験で判明)。
                    // 中身は問われないと踏んでいたが、**形は問われていた**
                    let id = uuid_v4(*seq);
                    let mut st = Styles::new();
                    // **書式は全シートを通して1つの表にする**(向こうと同じ)
                    for sh in &book.sheets {
                        for c in sh.cells.values() {
                            st.intern(&c.fmt);
                        }
                    }
                    let v = open_result(&id, &path, &book, &unsupported, &st.order, entry_count);
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
            // **見せる大きさは `size()`。** 申告(`<dimension>`)と実際の大きいほう
            // (2026-08-10 発注者確定)。`extent()` だと末尾の空行の高さや
            // 罫線だけの枠が範囲の外に落ちて、呼ぶ側に一度も届かない
            let (rows, cols) = sh.size();
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
        // **計算**(設計の「進め方」の2)。ironcalc の代わりに sheet::calc が答える。
        // セッションではなく**径路**で来る(向こうもそう作られている)ので、
        // 開き直しを避けるために計算用の居座りを別に持つ
        "recalc_cells" => {
            let path = s("path");
            if path.is_empty() {
                return fail(&rid, "invalid_request", "path がありません").to_string();
            }
            let edits = req.get("edits").and_then(Value::as_array).cloned().unwrap_or_default();
            let reads = req.get("reads").and_then(Value::as_array).cloned().unwrap_or_default();
            match recalc(resident, &path, &edits, &reads) {
                Ok(v) => ok(&rid, v),
                Err(e) => fail(&rid, "workbook_error", &e),
            }
        }
        other => fail(&rid, "invalid_request", &format!("知らない命令: {other}")),
    }
    .to_string()
}

/// xlsx を読んで、**開いた時点で計算し直す**(pysheet の `Book.open` と同じ作法)。
/// 原本にキャッシュ値の無い式でも答えを返せる — 向こうは空を返す所(設計参照)。
fn open_book(path: &str) -> Result<(Book, Vec<String>, usize), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: 読めない: {e}"))?;
    // 原本の部品の数(向こうの `entryCount`)。読むついでに数える
    let entry_count = zip::ZipArchive::new(std::io::Cursor::new(&bytes))
        .map(|z| z.len())
        .unwrap_or(0);
    let (mut book, rep) = sheet::xlsx::read(std::io::Cursor::new(&bytes))
        .map_err(|e| format!("{path}: xlsx として読めない: {e}"))?;
    sheet::recalc_all(&mut book);
    // **読めなかった物は名前で言う。** 黙って落とすのが一番悪い
    Ok((
        book,
        rep.unsupported.into_iter().map(|(w, n)| format!("{w} ×{n}")).collect(),
        entry_count,
    ))
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
    entry_count: usize,
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
    v.insert("visuals".into(), json!([]));
    v.insert("definedNames".into(), json!(defined));
    v.insert(
        "xNotFilled".into(),
        json!([
            "dxfIndex はシートを跨ぐと番号がずれる(1枚の帳票では正しい)",
            "visuals(図形・画像・グラフ)",
            "sparklines / pivotTables / pivotRanges",
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
        // **鍵は掛けない・掛けた振りもしない**(SEKKEI writer の保護と同じ作法)
        "sheetProtection": json!({"protected": sh.protected, "hasPassword": false}),
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
        o.insert("color".into(), json!(hex_color(&format!("{:06X}", c & 0xFF_FFFF))));
    }
    Some(Value::Object(o))
}

/// `CellFormat` を向こうの `CellStyle` の形へ。**既定のままなら `None`。**
fn style_value(f: &sheet::model::CellFormat) -> Option<Value> {
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

/// 計算のために居座らせたブック。**径路で引く。**
///
/// 向こうは ironcalc の Model をセッションに残す。こちらも同じく、毎回
/// ZIP を開き直さない。原本が変われば捨てる(`open` の居座りと同じ作法)。
struct Resident {
    stamp: (u64, u64),
    book: Book,
    /// 前回この居座りに当てた編集(**セルごと**に、打った字を覚える)。
    ///
    /// 呼ぶ側は「いま画面にある編集の**全部**」を毎回渡してくる作りで、
    /// 減ることがある(取り消し)。当てっぱなしにすると**取り消したはずの値が
    /// 残る** — 実際に残っていた(2026-08-10、向こうの試験
    /// `serves repeat requests ... rebuilds after a revert` が捕まえた)。
    ///
    /// - **同じセルを打ち直しただけ**なら当て直せばよい(居座りが効く)
    /// - **前に当てたセルが今回の一覧から消えていたら**取り消しなので、
    ///   ファイルから組み直す(そのセルの元の中身はファイルしか知らない)
    applied: BTreeMap<(String, u32, u32), String>,
}

/// `recalc_cells` — **打った字を入れて、計算して、頼まれた範囲を返す。**
///
/// 向こうの答えの形に揃える: `formatted`(表示形式を当てた文字列)・
/// `number`(数なら生の値)・`isFormula`。`cached` は居座りが効いたか。
fn recalc(
    resident: &mut BTreeMap<String, Resident>,
    path: &str,
    edits: &[Value],
    reads: &[Value],
) -> Result<Value, String> {
    let stamp = stamp_of(path);
    // 今回の編集を、セルごとの表にする
    let want: BTreeMap<(String, u32, u32), String> = edits
        .iter()
        .map(|e| {
            let g = |k: &str| e.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
            (
                (
                    e.get("sheet").and_then(Value::as_str).unwrap_or_default().to_string(),
                    g("row"),
                    g("column"),
                ),
                e.get("input").and_then(Value::as_str).unwrap_or_default().to_string(),
            )
        })
        .collect();

    // **居座りが効くのは、原本が変わっておらず、前に当てたセルが1つも
    // 消えていないとき。** 値が変わっただけなら当て直せば済む
    let usable = match resident.get(path) {
        Some(r) => r.stamp == stamp && r.applied.keys().all(|k| want.contains_key(k)),
        None => false,
    };
    if !usable {
        let (book, _, _) = open_book(path)?;
        resident.insert(path.to_string(), Resident { stamp, book, applied: BTreeMap::new() });
    }
    let cached = usable;
    let r = resident.get_mut(path).ok_or("居座りが作れません")?;

    // **中身の変わったセルだけ当てる。** 同じ字なら触らない
    let fresh: Vec<_> = want
        .iter()
        .filter(|(k, v)| r.applied.get(*k) != Some(*v))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    r.applied = want;

    for ((name, row, col), input) in &fresh {
        let Some(sh) = r.book.sheets.iter_mut().find(|s| &s.name == name) else {
            // **知らないシートは黙って飛ばさない。** 呼ぶ側の思い違いが隠れる
            return Err(format!("シートがありません: {name}"));
        };
        let p = Pos { row: *row, col: *col };
        // **表示形式は据え置く。** 打ち直しで書式を落とすのは calc の掟に反する
        let mut fmt = sh.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
        let mut cell = if input.is_empty() {
            sheet::model::Cell::default()
        } else {
            sheet::model::Cell::input(input)
        };
        // **日付を返す式には日付の形式を薦める** — 無いと画面に通し番号
        // (46244)が出る。**元の形式があるときは触らない**(Excel と同じ)
        if fmt.number_format.is_none() {
            if let Some(f) = cell.formula.as_deref().and_then(sheet::model::Cell::date_format_of) {
                fmt.number_format = Some(f.into());
            }
        }
        cell.fmt = fmt;
        sh.set(p, cell);
    }
    if !fresh.is_empty() {
        sheet::recalc_all(&mut r.book);
    }

    let mut cells = Vec::new();
    for rd in reads {
        let name = rd.get("sheet").and_then(Value::as_str).unwrap_or_default();
        let Some(sh) = r.book.sheets.iter().find(|s| s.name == name) else {
            return Err(format!("シートがありません: {name}"));
        };
        let g = |k: &str| rd.get("range").and_then(|x| x.get(k)).and_then(Value::as_u64).unwrap_or(0) as u32;
        for row in g("startRow")..=g("endRow") {
            for col in g("startColumn")..=g("endColumn") {
                let p = Pos { row, col };
                let Some(c) = sh.get(p) else { continue };
                let v = sh.value(p);
                let mut o = Map::new();
                o.insert("sheet".into(), json!(name));
                o.insert("row".into(), json!(row));
                o.insert("column".into(), json!(col));
                o.insert(
                    "formatted".into(),
                    json!(sheet::model::format_value(&v, c.fmt.number_format.as_deref())),
                );
                // **数でなければ `number` を省く。** `null` を入れると
                // `z.number()` に撥ねられる(向こうは `skip_serializing_if`)
                if let sheet::Value::Number(n) = v {
                    o.insert("number".into(), json!(n));
                }
                o.insert("isFormula".into(), json!(c.formula.is_some()));
                cells.push(Value::Object(o));
            }
        }
    }
    Ok(json!({"cells": cells, "cached": cached}))
}

/// セッションの id に使う UUID v4 の形。**外から見える形だけを揃える。**
///
/// 向こうは `z.uuid()` で検査しており、`ow-1` のような自前の名前は撥ねられる。
/// 乱数の質は問われない(こちらの中で一意ならよい)ので、番号と時刻から組む —
/// **`uuid` クレートを足すほどの話ではない。**
fn uuid_v4(seq: u64) -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let a = t ^ (seq << 32);
    let b = t.rotate_left(17) ^ seq.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        (a >> 16) as u16,
        (a & 0x0FFF) as u16,
        // 変種の桁は 8/9/a/b のいずれかでなければならない
        0x8000u16 | ((b >> 48) as u16 & 0x3FFF),
        b & 0xFFFF_FFFF_FFFF
    )
}

/// 列の幅ひとまとまりを、向こうの `ColumnWidth` の形へ。
///
/// **`width` と `outlineLevel` は無ければ省き、`startColumn`/`endColumn`/
/// `hidden` は必ず出す**(向こうの `skip_serializing_if` の付き方)。
fn col_value(a: u32, z: u32, (w, hidden, lvl, collapsed): (Option<f32>, bool, u8, bool)) -> Value {
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
fn hex_color(c: &str) -> String {
    let h = c.trim_start_matches('#');
    let h = if h.len() == 8 { &h[2..] } else { h };
    format!("#{}", h.to_uppercase())
}

/// `A1:B2` を `$A$1:$B$2` に。**名前の定義は絶対参照で書かれる**のが常で、
/// 向こうもその綴りで返す。`sheet` は解いて持っているので戻す。
fn absolute(r: &str) -> String {
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
/// 色は `#RRGGBB`。持っているのは文字色と塗りだけなので、他は既定のまま。
fn dxf_value(c: &sheet::model::CondRule) -> Value {
    let mut o = Map::new();
    for k in ["bold", "italic", "underline", "strikethrough", "wrapText", "diagonalUp", "diagonalDown"] {
        o.insert(k.into(), json!(false));
    }
    if let Some(x) = &c.color {
        o.insert("fontColor".into(), json!(hex_color(x)));
    }
    if let Some(x) = &c.fill {
        o.insert("fillColor".into(), json!(hex_color(x)));
    }
    Value::Object(o)
}
