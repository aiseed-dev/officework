//! pyoffice のサイドカー — **genoffice と同じ言葉を喋る**橋。
//!
//! ここは庫(lib)。stdio の入り口は main.rs、関数(FFI)の口は [`ffi`] —
//! 言葉はどの運び方でも同じ [`Bridge`] が捌く。
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
//!
//! **答えに自分の都合の欄を足さない。** 「埋めていない欄を名前で言う」つもりで
//! `xNotFilled` / `xUnsupported` を添えていたが、**本番のアプリが起動できなかった**
//! (2026-08-10)。`workbook:select` の schema は `strict()` で、知らない欄が
//! あれば ZodError で撥ねる。**試験の schema は `passthrough()` だったので
//! 通っていた** — 試験が緑でも本番が動かない、という形。
//! 埋めていない欄は `docs/sekkei/pyoffice.ja.md` に書く。

use std::collections::BTreeMap;
// stdin/stdout は入り口(main.rs)の持ち物。ここは Plumbing(子プロセスの
// 読み書き)が trait だけ使う
use std::io::{BufRead, Write};

use serde_json::{Map, Value, json};
use sheet::model::Pos;
use sheet::Book;

mod archive;
mod value;
mod visuals;

use archive::{archive_manifest, names, open_zip, read_entries, save_archive, scan_entries};
use value::{formula_cells_result, open_result, range_result, style_table};
use visuals::{media_index, read_media, sheet_parts, visuals_of};


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
    /// 図形と画像の id → (原本の中の径路, 絵の種類)。`read_media` が引く。
    ///
    /// **絵の中身はここに持たない。** 何十 MB にもなるし、向こうは要る物だけ
    /// 1つずつ聞きに来る作り。聞かれたときに原本から出す
    media: BTreeMap<String, (String, String)>,
    // **書式の番号は持たない。** 原本の `<c s="…">`(`Sheet::style_of`)を
    // そのまま返すので、こちらで採番する物が無くなった(2026-08-10)
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
    path: Option<String>,
    child: Option<(std::process::Child, std::io::BufReader<std::process::ChildStdout>)>,
}

impl Plumbing {
    /// **指されていなければ、探しに行かない。**
    ///
    /// 前は `~/dev/genoffice/…` を既定として当て推量していた。ZIP の配管を
    /// 5つとも転送していた頃はそれで助かったが、いま転送するのは
    /// `convert_workbook`(`.xls`)だけで、しかも**独自路線**と決めた
    /// (2026-08-10)。他人の木を黙って探して黙って使うのは、その決めと
    /// 噛み合わない。**指されたときだけ使う。**
    fn new() -> Plumbing {
        Plumbing { path: std::env::var("GENOFFICE_SIDECAR").ok(), child: None }
    }

    /// 行を渡して行を受け取る。**中身は見ない。**
    fn forward(&mut self, line: &str) -> Result<String, String> {
        let Some(path) = self.path.clone() else {
            return Err("GENOFFICE_SIDECAR が指されていません".into());
        };
        if self.child.is_none() {
            let mut c = std::process::Command::new(&path)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .map_err(|e| {
                    format!("向こうのサイドカーを起こせません({path}): {e}")
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


fn ok(request_id: &str, result: Value) -> Value {
    json!({"version": PROTOCOL_VERSION, "requestId": request_id, "ok": true, "result": result})
}

fn fail(request_id: &str, code: &str, message: &str) -> Value {
    json!({"version": PROTOCOL_VERSION, "requestId": request_id, "ok": false,
           "error": {"code": code, "message": message}})
}

/// 橋の本体 — 開いたブックの居座りを抱え、JSON 1行を捌いて 1行返す。
///
/// **運び方は3つ、言葉は1つ。** stdio(main.rs — genoffice のデスクトップ)、
/// FFI([`ffi`] — スマホ。iOS はサブプロセスを起こせないので、アプリに
/// 焼き込んで関数で呼ぶ)、そして試験。どれも同じ [`Bridge::handle`] を通る。
pub struct Bridge {
    sessions: BTreeMap<String, Session>,
    plumbing: Plumbing,
    resident: BTreeMap<String, Resident>,
    seq: u64,
}

impl Bridge {
    pub fn new() -> Bridge {
        Bridge {
            sessions: BTreeMap::new(),
            plumbing: Plumbing::new(),
            resident: BTreeMap::new(),
            seq: 0,
        }
    }

    /// 1行を捌いて、返す1行を作る。
    ///
    /// **戻りが文字列なのは転送のため。** `Value` に一度入れ直すと、向こうの
    /// 答えを組み直すことになる — 通信の言葉はこちらに直す権利がないので、
    /// 配管の答えは**文字どおり素通し**にする。
    pub fn handle(&mut self, line: &str) -> String {
        dispatch(line, &mut self.sessions, &mut self.plumbing, &mut self.resident, &mut self.seq)
    }
}

impl Default for Bridge {
    fn default() -> Self {
        Bridge::new()
    }
}

/// stdio の入り口(main.rs)が入出力の故障を報告する形。言葉は dispatch と同じ
pub fn io_error_line(message: &str) -> String {
    fail("", "io_error", message).to_string()
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
                    let styles = style_table(&book);
                    // **図形と画像は原本から読む**(モデルは原本どおりの
                    // アンカーを持たない)。読めなかった物は `skipped` に溜める
                    let mut skipped = Vec::new();
                    let visuals = match open_zip(&path) {
                        Ok(mut z) => {
                            let parts = sheet_parts(&mut z);
                            book.sheets
                                .iter()
                                .enumerate()
                                .flat_map(|(i, sh)| {
                                    let part = parts.get(i).map(String::as_str).unwrap_or("");
                                    visuals_of(
                                        &mut z,
                                        part,
                                        &format!("sheet-{}", i + 1),
                                        sh,
                                        &mut skipped,
                                    )
                                })
                                .collect()
                        }
                        Err(e) => {
                            skipped.push(format!("原本を開き直せない: {e}"));
                            Vec::new()
                        }
                    };
                    // **読めなかった物を黙って捨てない。** 契約は `strict()` で
                    // 欄を足せないので、**標準エラーへ言う**(向こうの log に出る)。
                    // 黙って空を返すのが、この家でいちばん嫌う形
                    for w in unsupported.iter().chain(skipped.iter()) {
                        eprintln!("[officework] 読めなかった: {w}({path})");
                    }
                    let v = open_result(&id, &path, &book, &styles, entry_count, visuals);
                    let stamp = stamp_of(&path);
                    let media = media_index(&v);
                    sessions.insert(id, Session { path, stamp, book, media });
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
            ok(&rid, range_result(sh, r0, r1, c0, c1))
        }
        "close" => {
            sessions.remove(&s("sessionId"));
            ok(&rid, json!({}))
        }
        // **居座りを止めるだけ。** こちらは全部読んでから答えるので、
        // 途中で止める物が無い(設計「段階索引」の決め)
        "cancel" => ok(&rid, json!({})),
        // **ZIP の配管も自前で持つ**(2026-08-10 発注者確定「独自路線」)。
        // 前は5つとも向こうへ転送していたが、**連絡を取らないと決めた相手の
        // ビルド生成物に実行時に依るのは独立ではない**。4つを自分でやる
        "archive_manifest" => match archive_manifest(&s("path")) {
            Ok(entries) => ok(&rid, json!({ "entries": entries })),
            Err(e) => fail(&rid, "io_error", &e),
        },
        "read_entries" => {
            match read_entries(&s("path"), &names(req.get("entries")), &s("outputDir")) {
                Ok(entries) => ok(&rid, json!({ "entries": entries })),
                Err(e) => fail(&rid, "io_error", &e),
            }
        }
        "scan_entries" => {
            match scan_entries(&s("path"), &names(req.get("entries")), &s("needle")) {
                Ok(matches) => ok(&rid, json!({ "matches": matches })),
                Err(e) => fail(&rid, "io_error", &e),
            }
        }
        "save_archive" => match save_archive(&req) {
            Ok(v) => ok(&rid, v),
            Err(e) => fail(&rid, "io_error", &e),
        },
        // **`.xls` はまだ読めない。** BIFF8 は xlsx とは別の形式で、
        // `sheet` は持っていない。控えがあるうちは向こうへ転送し、
        // 無ければ**そう言って止める** — 黙って空の帳面を開かない
        "convert_workbook" => match plumbing.forward(line) {
            Ok(answer) => return answer,
            Err(e) => fail(
                &rid,
                "unsupported_command",
                &format!(
                    ".xls はまだ読めません(古い binary の形式で、officework は xlsx と \
                     docx を読みます)。xlsx に変換してから開いてください。{e}"
                ),
            ),
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
        // 絵の中身。**open で配った id を鍵に、原本から出す**
        "read_media" => {
            let sid = s("sessionId");
            let Some(sess) = sessions.get(&sid) else {
                return fail(&rid, "invalid_request", "そのセッションはありません").to_string();
            };
            match read_media(&sess.path, &sess.media, &s("visualId")) {
                Ok(v) => ok(&rid, v),
                Err(e) => fail(&rid, "io_error", &e),
            }
        }
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
    // **出どころを教える。** CELL("filename") が `径路[名前]シート名` を
    // 返すのに要る。ファイルには入っていない情報なので、開いた側が入れる
    book.path = std::fs::canonicalize(path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.to_string());
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
                    json!(sheet::model::format_value(
                        &v,
                        c.fmt.number_format.as_deref(),
                        r.book.date1904
                    )),
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

/// C の口 — スマホ(genoffice の Web 画面の下)へ橋を焼き込むための関数。
///
/// **iOS はアプリからのサブプロセス起動を禁じている**ので、stdio の形は
/// 使えない。同じ JSON の言葉を、関数の呼び出しで運ぶ(2026-08-13 発注者
/// 「ネイティブの庫(FFI)を優先」)。組み方: この crate は staticlib /
/// cdylib too(Cargo.toml)— iOS は .a を焼き込み、Android は .so を積む。
pub mod ffi {
    use std::ffi::{c_char, CStr, CString};

    /// 橋を作る。使い終わったら [`officework_bridge_free`] に返すこと。
    #[no_mangle]
    pub extern "C" fn officework_bridge_new() -> *mut crate::Bridge {
        Box::into_raw(Box::new(crate::Bridge::new()))
    }

    /// JSON 1行 → JSON 1行。stdio 版と1文字も違わない言葉。
    /// 返った文字列は [`officework_string_free`] に返すこと。
    ///
    /// # Safety
    /// `bridge` は [`officework_bridge_new`] の返り(まだ free していない物)、
    /// `req` は NUL 終端の UTF-8。守られない呼び方にも落ちずに
    /// JSON の断り(`ok:false`)で答える — 橋の向こうは他人の言語なので、
    /// こちらの都合(panic)を輸出しない
    #[no_mangle]
    pub unsafe extern "C" fn officework_bridge_handle(
        bridge: *mut crate::Bridge,
        req: *const c_char,
    ) -> *mut c_char {
        let out = if bridge.is_null() || req.is_null() {
            crate::fail("", "invalid_request", "null が渡されました").to_string()
        } else {
            match CStr::from_ptr(req).to_str() {
                Ok(line) => (*bridge).handle(line),
                Err(_) => crate::fail("", "invalid_request", "UTF-8 ではありません").to_string(),
            }
        };
        // JSON の文字列に生の NUL は入らない(serde_json は \u0000 に逃がす)が、
        // 万一入っても黙って空を返さない — 断りの定型で答える
        match CString::new(out) {
            Ok(c) => c.into_raw(),
            Err(_) => CString::new(
                crate::fail("", "io_error", "答えに NUL が混ざりました").to_string(),
            )
            .expect("定型の断りに NUL は無い")
            .into_raw(),
        }
    }

    /// [`officework_bridge_handle`] が返した文字列を返す。
    ///
    /// # Safety
    /// `s` は handle の返り(まだ返していない物)。null は何もしない
    #[no_mangle]
    pub unsafe extern "C" fn officework_string_free(s: *mut c_char) {
        if !s.is_null() {
            drop(CString::from_raw(s));
        }
    }

    /// 橋を畳む(開いていたブックの居座りも消える)。
    ///
    /// # Safety
    /// `b` は [`officework_bridge_new`] の返り(まだ free していない物)。
    /// null は何もしない
    #[no_mangle]
    pub unsafe extern "C" fn officework_bridge_free(b: *mut crate::Bridge) {
        if !b.is_null() {
            drop(Box::from_raw(b));
        }
    }
}

#[cfg(test)]
mod ffi_tests {
    use std::ffi::{CStr, CString};

    use super::ffi::*;

    /// C の呼び方そのままで一往復する(作る → 聞く → 返す → 畳む)
    fn ask(b: *mut crate::Bridge, req: &str) -> serde_json::Value {
        let c = CString::new(req).unwrap();
        let ans = unsafe { officework_bridge_handle(b, c.as_ptr()) };
        let s = unsafe { CStr::from_ptr(ans) }.to_str().unwrap().to_string();
        unsafe { officework_string_free(ans) };
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn 関数の橋でも同じ言葉を喋る() {
        let b = officework_bridge_new();
        let v = ask(b, r#"{"version":1,"requestId":"r1","command":"cancel"}"#);
        assert_eq!(v["ok"], serde_json::json!(true));
        assert_eq!(v["requestId"], serde_json::json!("r1"));
        // 実物も開ける(stdio の入り口と同じ dispatch を通っている証拠)
        let sample = concat!(env!("CARGO_MANIFEST_DIR"), "/../sample/見積書.xlsx");
        let v = ask(
            b,
            &format!(r#"{{"version":1,"requestId":"r2","command":"open","path":{}}}"#,
                serde_json::json!(sample)),
        );
        assert_eq!(v["ok"], serde_json::json!(true), "{v}");
        assert!(v["result"]["sessionId"].is_string());
        unsafe { officework_bridge_free(b) };
    }

    #[test]
    fn 壊れた呼び方にも_json_の断りで答える() {
        let b = officework_bridge_new();
        let v = ask(b, "これは JSON ではない");
        assert_eq!(v["ok"], serde_json::json!(false));
        // null の橋・null の要求でも落ちない(他人の言語に panic を輸出しない)
        let c = CString::new("{}").unwrap();
        let ans = unsafe { officework_bridge_handle(std::ptr::null_mut(), c.as_ptr()) };
        let s = unsafe { CStr::from_ptr(ans) }.to_str().unwrap().to_string();
        unsafe { officework_string_free(ans) };
        assert!(s.contains("null"), "{s}");
        unsafe { officework_string_free(std::ptr::null_mut()) }; // null は何もしない
        unsafe { officework_bridge_free(b) };
    }
}
