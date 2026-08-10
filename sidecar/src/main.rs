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
//!
//! **答えに自分の都合の欄を足さない。** 「埋めていない欄を名前で言う」つもりで
//! `xNotFilled` / `xUnsupported` を添えていたが、**本番のアプリが起動できなかった**
//! (2026-08-10)。`workbook:select` の schema は `strict()` で、知らない欄が
//! あれば ZodError で撥ねる。**試験の schema は `passthrough()` だったので
//! 通っていた** — 試験が緑でも本番が動かない、という形。
//! 埋めていない欄は `docs/sekkei/pyoffice.ja.md` に書く。

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufWriter, Read, Write};

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
                Ok((book, _unsupported, entry_count)) => {
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
                    let v = open_result(&id, &path, &book, &st.order, entry_count);
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

// ───────────────────────── ZIP の配管 ─────────────────────────
//
// 向こうの TypeScript が xlsx を組み立てる道。**こちらは中身を解釈しない** —
// 部品を数え、取り出し、探し、当てて書くだけ。xlsx の意味は上の組が持つ。

/// 要求の配列から名前を取り出す。`null` や欠けは空として扱う。
fn names(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// 部品ひとつの札。**向こうの `archiveEntrySchema` と同じ4欄。**
fn entry_value(f: &zip::read::ZipFile<'_>) -> Value {
    json!({
        "name": f.name(),
        "crc32": f.crc32(),
        "compressedSize": f.compressed_size(),
        "uncompressedSize": f.size(),
    })
}

fn open_zip(path: &str) -> Result<zip::ZipArchive<std::io::BufReader<std::fs::File>>, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{path}: 開けません: {e}"))?;
    zip::ZipArchive::new(std::io::BufReader::new(f))
        .map_err(|e| format!("{path}: ZIP として読めません: {e}"))
}

/// `archive_manifest` — 部品の一覧。**原本の並びのまま返す。**
fn archive_manifest(path: &str) -> Result<Vec<Value>, String> {
    let mut z = open_zip(path)?;
    (0..z.len())
        .map(|i| z.by_index(i).map(|f| entry_value(&f)).map_err(|e| format!("{path}: {e}")))
        .collect()
}

/// `read_entries` — 名前で指した部品を `output_dir` へ出し、置いた径路を返す。
///
/// **名前をそのまま径路にしない。** `xl/worksheets/sheet1.xml` の `/` で
/// 掘るのは呼ぶ側の想定ではないし、`..` を含む名前(zip slip)を渡されたら
/// 出力先の外へ書いてしまう。**平らな名前に潰して置く。**
fn read_entries(path: &str, want: &[String], output_dir: &str) -> Result<Vec<Value>, String> {
    let mut z = open_zip(path)?;
    std::fs::create_dir_all(output_dir).map_err(|e| format!("{output_dir}: 作れません: {e}"))?;
    let mut out = Vec::new();
    for (i, name) in want.iter().enumerate() {
        let mut f = match z.by_name(name) {
            Ok(f) => f,
            // **無い部品は黙って飛ばす。** 向こうは「あれば読む」で呼び、
            // 答えの数が減ったことで無かったと分かる作り
            Err(_) => continue,
        };
        let flat = format!("{i}-{}", name.replace(['/', '\\'], "_"));
        let dest = std::path::Path::new(output_dir).join(&flat);
        let mut w = std::fs::File::create(&dest)
            .map_err(|e| format!("{}: 作れません: {e}", dest.display()))?;
        std::io::copy(&mut f, &mut w).map_err(|e| format!("{name}: 出せません: {e}"))?;
        out.push(json!({ "name": name, "path": dest.display().to_string() }));
    }
    Ok(out)
}

/// `scan_entries` — 名前で指した部品に文字列が入っているか。
///
/// **解いた中身をそのまま見る。** UTF-8 として読めない部品もあるので、
/// 字ではなくバイトの並びで探す(xlsx の XML は UTF-8 なので同じことだが、
/// 画像などを渡されても落ちない)。
fn scan_entries(path: &str, want: &[String], needle: &str) -> Result<Vec<String>, String> {
    let mut z = open_zip(path)?;
    let pat = needle.as_bytes();
    let mut hit = Vec::new();
    for name in want {
        let Ok(mut f) = z.by_name(name) else { continue };
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| format!("{name}: 読めません: {e}"))?;
        if pat.is_empty() || buf.windows(pat.len()).any(|w| w == pat) {
            hit.push(name.clone());
        }
    }
    Ok(hit)
}

/// `save_archive` — 原本に差し替え・削除・追加を当てて、別名で書く。
///
/// **急所は「触っていない部品を解かずに写す」こと。** 向こうの TypeScript は
/// 保存のあと `assertManifestPreserved` で、触っていない部品の **crc32 と
/// 圧縮後の大きさ**が変わっていないことを確かめる。解いて詰め直すと、
/// deflate の水準が少し違うだけで圧縮後の大きさが変わり、**向こうが
/// 「部品が変わった」と言って保存を止める**。
///
/// `raw_copy_file` は圧縮済みの流れをそのまま写す。だから原本の部品は
/// **1バイトも変わらない** — 「触っていない所を壊さない」を字面どおりに守る。
fn save_archive(req: &Value) -> Result<Value, String> {
    let s = |k: &str| req.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let pairs = |k: &str| -> Vec<(String, String)> {
        req.get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        Some((
                            x.get("name")?.as_str()?.to_string(),
                            x.get("contentPath")?.as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let (source, target) = (s("sourcePath"), s("targetPath"));
    let replacements: BTreeMap<String, String> = pairs("replacements").into_iter().collect();
    let additions = pairs("additions");
    let removals: std::collections::BTreeSet<String> =
        names(req.get("removals")).into_iter().collect();

    let mut z = open_zip(&source)?;
    let before: Vec<Value> = (0..z.len())
        .map(|i| z.by_index(i).map(|f| entry_value(&f)).map_err(|e| format!("{source}: {e}")))
        .collect::<Result<_, _>>()?;

    let out = std::fs::File::create(&target).map_err(|e| format!("{target}: 作れません: {e}"))?;
    let mut w = zip::ZipWriter::new(std::io::BufWriter::new(out));
    let opts: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 原本の並びを保ったまま写す。差し替えはその場で、削除は飛ばす
    for i in 0..z.len() {
        let f = z.by_index(i).map_err(|e| format!("{source}: {e}"))?;
        let name = f.name().to_string();
        if removals.contains(&name) {
            continue;
        }
        match replacements.get(&name) {
            Some(src) => {
                drop(f);
                let body = std::fs::read(src).map_err(|e| format!("{src}: 読めません: {e}"))?;
                w.start_file(&name, opts).map_err(|e| format!("{name}: 書けません: {e}"))?;
                w.write_all(&body).map_err(|e| format!("{name}: 書けません: {e}"))?;
            }
            // **解かずに写す。** ここを `copy` にすると向こうの検査で落ちる
            None => w.raw_copy_file(f).map_err(|e| format!("{name}: 写せません: {e}"))?,
        }
    }
    // 追加は末尾へ。**原本に同じ名前があれば差し替えで済んでいる**ので、
    // ここで二重に書くと ZIP に同名の部品が2つ並ぶ
    let had: std::collections::BTreeSet<String> =
        before.iter().filter_map(|e| e.get("name")?.as_str().map(str::to_string)).collect();
    for (name, src) in &additions {
        if had.contains(name) {
            continue;
        }
        let body = std::fs::read(src).map_err(|e| format!("{src}: 読めません: {e}"))?;
        w.start_file(name, opts).map_err(|e| format!("{name}: 書けません: {e}"))?;
        w.write_all(&body).map_err(|e| format!("{name}: 書けません: {e}"))?;
    }
    w.finish().map_err(|e| format!("{target}: 閉じられません: {e}"))?;

    let mut z2 = open_zip(&target)?;
    let after: Vec<Value> = (0..z2.len())
        .map(|i| z2.by_index(i).map(|f| entry_value(&f)).map_err(|e| format!("{target}: {e}")))
        .collect::<Result<_, _>>()?;
    Ok(json!({ "beforeEntries": before, "afterEntries": after }))
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
    v.insert("visuals".into(), json!([]));
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
/// 色は `#RRGGBB`。
///
/// **飾りは三択を二択に畳む。** こちらの `CondLook` は「触らない(None)」と
/// 「外す(Some(false))」を分けて持つが、向こうの契約は素の bool しか無い。
/// どちらも `false` にする — 契約に無い区別を勝手に足さない
/// (2026-08-10。前は**いつも false** で、Excel で赤字・太字にした規則が
/// 太字と文字色を落として届いていた)。
/// `wrapText`・`diagonalUp/Down` はこちらが dxf から読んでいないので false
fn dxf_value(c: &sheet::model::CondRule) -> Value {
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

#[cfg(test)]
mod plumbing_tests {
    use super::*;

    /// 部品を3つ持つ ZIP を作る。
    ///
    /// **わざと違う圧縮の水準で詰める。** 最初はここを既定のままにしていて、
    /// 中の実装を「解いて詰め直す」形に替えても試験が**通ってしまった** —
    /// 同じライブラリの同じ水準で詰め直せば同じ大きさになるのは当たり前で、
    /// 何も見ていなかった。実物は Excel や JSZip が詰めた物で、水準は
    /// こちらと違う。**型紙を本番に似せないと、検査は空を打つ。**
    fn make_zip(path: &std::path::Path) {
        let f = std::fs::File::create(path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(2));
        // **繰り返しだけの中身では水準の差が出ない**(どの水準でも同じ所まで
        // 縮む)。実物の worksheet に似せて、変化のある中身にする
        let xmlish = |tag: &str| -> String {
            (0..400)
                .map(|i| format!(r#"<{tag} r="A{i}" s="{}"><v>{}</v></{tag}>"#, i % 7, i * 37 % 1009))
                .collect()
        };
        for (name, body) in [
            ("[Content_Types].xml", xmlish("Override")),
            ("xl/workbook.xml", xmlish("sheet")),
            ("xl/worksheets/sheet1.xml", format!("{}うたかたの泡", xmlish("c"))),
        ] {
            w.start_file(name, opts).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }

    fn manifest(path: &std::path::Path) -> Vec<(String, u32, u64)> {
        let mut z = open_zip(&path.display().to_string()).unwrap();
        (0..z.len())
            .map(|i| {
                let f = z.by_index(i).unwrap();
                (f.name().to_string(), f.crc32(), f.compressed_size())
            })
            .collect()
    }

    /// **触っていない部品は、解かずにそのまま写す。**
    ///
    /// 向こうの `assertManifestPreserved` は crc32 だけでなく**圧縮後の
    /// 大きさ**も見る。解いて詰め直すと deflate の水準の差で後者が動き、
    /// 保存が「部品が変わった」と止められる。`raw_copy_file` を
    /// `std::io::copy` に替えたらこの試験が落ちる。
    #[test]
    fn 触っていない部品は圧縮後の大きさまで変わらない() {
        let dir = std::env::temp_dir().join("ow-plumb-1");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (src, dst, body) = (dir.join("a.xlsx"), dir.join("b.xlsx"), dir.join("new.xml"));
        make_zip(&src);
        std::fs::write(&body, "<workbook/>").unwrap();

        let before = manifest(&src);
        let req = json!({
            "sourcePath": src.display().to_string(),
            "targetPath": dst.display().to_string(),
            "replacements": [{"name": "xl/workbook.xml", "contentPath": body.display().to_string()}],
            "removals": [],
            "additions": [],
        });
        let r = save_archive(&req).expect("保存できない");
        let after = manifest(&dst);

        assert_eq!(before.len(), after.len(), "部品の数が変わった");
        assert_eq!(
            before.iter().map(|e| &e.0).collect::<Vec<_>>(),
            after.iter().map(|e| &e.0).collect::<Vec<_>>(),
            "**並びが変わった** — 原本の順を保つこと"
        );
        for (b, a) in before.iter().zip(&after) {
            if b.0 == "xl/workbook.xml" {
                assert_ne!(b.1, a.1, "差し替えたのに中身が同じ");
                continue;
            }
            assert_eq!((b.1, b.2), (a.1, a.2), "{}: 触っていないのに変わった", b.0);
        }
        // 答えの形も向こうの schema どおりか
        assert!(r["beforeEntries"].is_array() && r["afterEntries"].is_array());
        assert_eq!(r["beforeEntries"][0]["name"], "[Content_Types].xml");
    }

    /// 削除と追加。**追加は原本に無い名前だけ** — 同じ名前を二重に書くと
    /// ZIP の中に同名の部品が2つ並び、読み手によって答えが変わる
    #[test]
    fn 削除と追加が効き同じ名前を二重に書かない() {
        let dir = std::env::temp_dir().join("ow-plumb-2");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (src, dst, body) = (dir.join("a.xlsx"), dir.join("b.xlsx"), dir.join("x.xml"));
        make_zip(&src);
        std::fs::write(&body, "<x/>").unwrap();

        let req = json!({
            "sourcePath": src.display().to_string(),
            "targetPath": dst.display().to_string(),
            // **原本にある名前を「追加」で渡す。** 差し替えとして扱われるべき
            "replacements": [{"name": "xl/workbook.xml", "contentPath": body.display().to_string()}],
            "removals": ["xl/worksheets/sheet1.xml"],
            "additions": [
                {"name": "xl/new.xml", "contentPath": body.display().to_string()},
                {"name": "xl/workbook.xml", "contentPath": body.display().to_string()},
            ],
        });
        save_archive(&req).expect("保存できない");
        let names: Vec<String> = manifest(&dst).into_iter().map(|e| e.0).collect();
        assert!(!names.contains(&"xl/worksheets/sheet1.xml".to_string()), "消えていない");
        assert!(names.contains(&"xl/new.xml".to_string()), "足されていない");
        assert_eq!(
            names.iter().filter(|n| *n == "xl/workbook.xml").count(),
            1,
            "**同じ名前が2つ並んだ** — 原本にある名前の追加は差し替えで済んでいる"
        );
    }

    /// `read_entries` は**平らな名前で置く**。`..` を含む名前を渡されても
    /// 出力先の外へ書かない(zip slip)
    #[test]
    fn 取り出しは出力先の外へ書かない() {
        let dir = std::env::temp_dir().join("ow-plumb-3");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (src, out) = (dir.join("a.xlsx"), dir.join("out"));
        make_zip(&src);
        let got = read_entries(
            &src.display().to_string(),
            &["xl/worksheets/sheet1.xml".into(), "無い部品.xml".into()],
            &out.display().to_string(),
        )
        .expect("取り出せない");
        assert_eq!(got.len(), 1, "**無い部品で落ちない・数で分かる**");
        let p = std::path::PathBuf::from(got[0]["path"].as_str().unwrap());
        assert_eq!(p.parent().unwrap(), out, "出力先の外に置いた");
        assert!(std::fs::read_to_string(&p).unwrap().contains("うたかたの泡"), "中身が違う");
    }

    #[test]
    fn 探しものは中身を見る() {
        let dir = std::env::temp_dir().join("ow-plumb-4");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.xlsx");
        make_zip(&src);
        let all: Vec<String> =
            ["[Content_Types].xml".into(), "xl/workbook.xml".into()].into_iter().collect();
        let s = src.display().to_string();
        assert_eq!(scan_entries(&s, &all, "<sheet r=\"A3\"").unwrap(), vec!["xl/workbook.xml".to_string()]);
        assert!(scan_entries(&s, &all, "無い字").unwrap().is_empty(), "無い字が見つかった");
    }
}
