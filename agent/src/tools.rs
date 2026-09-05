//! **ops の語彙への結線**(v1 — MCP と同じ一覧)。
//!
//! エージェントは語彙を直に呼びます — ソケットも MCP も経由しません
//! (docs/sekkei/agent.ja.adoc「立ち位置」)。道具の名前と説明は MCP
//! (pysheet/officework/mcp.py)と同じにして、外の AI と中のエージェントが
//! 同じ言葉で表を触れるようにします。
//!
//! 呼び方は2つあります。
//!
//! - [`DirectHost`] — ops::Host を直に持つ(試験と、画面の無い口)
//! - [`QueueHost`] — rpc と同じ受け口(ops::Queue)へ流し、メインスレッドが
//!   捌くのを待つ。**ループを別スレッドで回すアプリはこちら** — gpui の
//!   状態はメインスレッドでしか触れない

use crate::{ToolDef, ToolHost};
use ops::{Host, J, Jobj};

/// v1 の道具の一覧(表)。名前・説明・引数の形は MCP と同じ
pub fn sheet_tools() -> Vec<ToolDef> {
    let t = |name: &str, description: &str, parameters: &str| ToolDef {
        name: name.into(),
        description: description.into(),
        parameters: parameters.into(),
    };
    // よく使う引数の形
    let a1_sheet = r#"{"type":"object","properties":{"a1":{"type":"string"},"sheet":{"type":"string"}},"required":["a1"]}"#;
    vec![
        t(
            "book_info",
            "いま開いているブックの様子(名前・径路・シートの一覧)。最初にこれを呼ぶ",
            r#"{"type":"object","properties":{}}"#,
        ),
        t(
            "used_range",
            "そのシートで使われている範囲の番地(例 A1:F42)。空なら A1",
            r#"{"type":"object","properties":{"sheet":{"type":"string"}}}"#,
        ),
        t(
            "read_range",
            "範囲の値を読む(2次元の並び)。a1 は A1 でも A1:C9 でもよい。大きすぎる範囲は避け、まず used_range で広さを見る",
            a1_sheet,
        ),
        t("read_formulas", "範囲の式を読む(値ではなく =SUM(...) の方)。無いセルは null", a1_sheet),
        t(
            "write_range",
            "範囲に値を書く。values は2次元の並び(1行でも [[...]])。= で始まる字は式として入る。書いた跡は Ctrl+Z で戻せる",
            r#"{"type":"object","properties":{"a1":{"type":"string"},"values":{"type":"array","items":{"type":"array"}},"sheet":{"type":"string"}},"required":["a1","values"]}"#,
        ),
        t(
            "set_format",
            "範囲に書式を掛ける。指定した物だけが変わる。number_format は #,##0 や yyyy/m/d など、fill は FFF2CC のような RRGGBB",
            r#"{"type":"object","properties":{"a1":{"type":"string"},"sheet":{"type":"string"},"bold":{"type":"boolean"},"italic":{"type":"boolean"},"number_format":{"type":"string"},"fill":{"type":"string"}},"required":["a1"]}"#,
        ),
        t("autofit", "列の幅を中身に合わせる(a1 の範囲。A1:C9 のようにセルまで書く)", a1_sheet),
        t(
            "save",
            "ブックを保存する。path を渡すとその名前で書き出す。人が保存を頼んだときだけ呼ぶ",
            r#"{"type":"object","properties":{"path":{"type":"string"}}}"#,
        ),
        // **大きな表は polars の物として**(2026-09-04)。セルで読ませない
        t(
            "table_schema",
            "名前の表(テーブル)の列の名前と型(number / text)と行の数。大きな表はまずこれ。表の名前は book_info か sheet_tables で",
            r#"{"type":"object","properties":{"table":{"type":"string"}},"required":["table"]}"#,
        ),
        t(
            "table_head",
            "名前の表の先頭 n 行(既定 5)を見出しつきで読む。様子を見る用",
            r#"{"type":"object","properties":{"table":{"type":"string"},"n":{"type":"integer"}},"required":["table"]}"#,
        ),
        t(
            "table_query",
            "名前の表を SQL で絞る・集計する(FROM には表の名前を書く。例: SELECT 品名, SUM(金額) AS 計 FROM 売上 GROUP BY 品名)。返りは見出しつきの小さな表と、絞った後の全行数 total。limit の既定は 200",
            r#"{"type":"object","properties":{"table":{"type":"string"},"sql":{"type":"string"},"limit":{"type":"integer"}},"required":["table","sql"]}"#,
        ),
    ]
}

/// read_range で一度に読ませる行の上限。越えたら断って table_* を案内する
/// (黙って切り詰めない。docs/sekkei/agent.ja.adoc「大きな calc の表は polars の物として」)
pub const READ_ROWS_MAX: u32 = 200;

/// `A1:C9` の行の数(読めなければ 1)
fn rows_of(a1: &str) -> u32 {
    let num = |s: &str| s.trim_start_matches(|c: char| c.is_ascii_alphabetic() || c == '$').parse::<u32>().unwrap_or(1);
    match a1.split_once(':') {
        Some((a, b)) => num(b).abs_diff(num(a)) + 1,
        None => 1,
    }
}

/// **文書の道具**(2026-09-04。docs/sekkei/agent.ja.adoc「writer にも同じパネル」)。
/// 名前・説明・引数は MCP(`doc_*`)と同じで、ブロックの番号で AsciiDoc の字を
/// 読み書きする。writer の受け口(rpc.rs)の動詞にそのまま対応する
pub fn doc_tools() -> Vec<ToolDef> {
    let t = |name: &str, description: &str, parameters: &str| ToolDef {
        name: name.into(),
        description: description.into(),
        parameters: parameters.into(),
    };
    let range = r#"{"type":"object","properties":{"start":{"type":"integer"},"end":{"type":"integer"}},"required":["start"]}"#;
    vec![
        t(
            "doc_outline",
            "文書の地図: 題と見出しの一覧(ブロックの番号つき)とブロックの数。長い文書はまずこれを呼ぶ。番号は 0 から",
            r#"{"type":"object","properties":{}}"#,
        ),
        t(
            "doc_read_blocks",
            "ブロック start〜end(両端を含む。end を省けば1つ)を AsciiDoc の字で読む。返りの stamp は書き替えの時に添える。一度に 30 個まで",
            range,
        ),
        t(
            "doc_replace_blocks",
            "ブロック start〜end を AsciiDoc の断片 adoc で書き替える(何ブロックでもよい)。stamps に読んだ時の照合の字を , で並べると、変わっていたら断る。書いた跡は Ctrl+Z で戻せる",
            r#"{"type":"object","properties":{"start":{"type":"integer"},"end":{"type":"integer"},"adoc":{"type":"string"},"stamps":{"type":"string","description":"読んだ時の stamp を , で並べる"}},"required":["start","end","adoc"]}"#,
        ),
        t(
            "doc_insert_blocks",
            "ブロック at の前に断片 adoc を差し込む。at がブロックの数と同じなら末尾",
            r#"{"type":"object","properties":{"at":{"type":"integer"},"adoc":{"type":"string"}},"required":["at","adoc"]}"#,
        ),
        t(
            "doc_delete_blocks",
            "ブロック start〜end を消す。stamps は doc_replace_blocks と同じ",
            r#"{"type":"object","properties":{"start":{"type":"integer"},"end":{"type":"integer"},"stamps":{"type":"string","description":"読んだ時の stamp を , で並べる"}},"required":["start"]}"#,
        ),
        t(
            "doc_find",
            "字を含むブロックを探す。返りは番号と前後の字",
            r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#,
        ),
        t(
            "doc_fill_fields",
            "名前の付いた記入欄にまとめて入れる。values は [[名前, 値], …]。無い名前は missing に返る",
            r#"{"type":"object","properties":{"values":{"type":"array","items":{"type":"array","items":{"type":"string"}}}},"required":["values"]}"#,
        ),
        t(
            "doc_to_pdf",
            "文書を PDF に書き出す(path は書き出し先)",
            r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#,
        ),
        t(
            "doc_save",
            "文書を保存する。path を渡すとその名前で。人が保存を頼んだときだけ呼ぶ",
            r#"{"type":"object","properties":{"path":{"type":"string"}}}"#,
        ),
    ]
}

/// **文書のマクロの道具**(2026-09-05)。表の run_macro と違い、**docx を通さず、
/// 文書の AsciiDoc の字を受け渡す**(決め: agent.ja.adoc「マクロは docx を通さない」)。
/// コードには `src`(文書の全部の AsciiDoc の字)が渡り、直した字を `out` に
/// 入れて返す。本体はそれを読み直して1手で入れる
pub fn doc_macro_tool() -> ToolDef {
    ToolDef {
        name: "run_macro".into(),
        description: "Python のマクロを書いて動かす(文書)。コードでは src(文書の全部の \
                      AsciiDoc の字。見出しは == 、段落は空行で区切り、表は |=== )が使える。\
                      直した字を out に入れると、本体がそれを読み直して1手で入れる(Ctrl+Z で \
                      戻せる)。print の出力が返る。一括の直し(見出しを一段下げる・語を \
                      全部置き替える など)はこれで解く。python-docx は使わない"
            .into(),
        parameters: r#"{"type":"object","properties":{"name":{"type":"string","description":"マクロの名前(ファイル名になる)"},"code":{"type":"string"}},"required":["code"]}"#
            .into(),
    }
}

/// 文書の道具呼びを writer の受け口の1行にする(名前は `doc_` を外した動詞)
pub fn doc_line_for(name: &str, o: &Jobj) -> Result<String, String> {
    let verb = name.strip_prefix("doc_").ok_or_else(|| format!("知らない道具です: {name}"))?;
    let mut parts: Vec<String> = vec![format!("\"cmd\":{}", J::S(verb.to_string()).to_json())];
    let num = |parts: &mut Vec<String>, key: &str, v: Option<f64>| {
        if let Some(v) = v {
            parts.push(format!("\"{key}\":{}", v as i64));
        }
    };
    let txt = |parts: &mut Vec<String>, key: &str, v: Option<String>| {
        if let Some(v) = v {
            parts.push(format!("\"{key}\":{}", J::S(v).to_json()));
        }
    };
    match verb {
        "outline" => {}
        "read_blocks" | "delete_blocks" | "replace_blocks" => {
            num(&mut parts, "from", Some(o.num("start").ok_or("start がありません")?));
            num(&mut parts, "to", o.num("end"));
            if verb == "replace_blocks" {
                txt(&mut parts, "adoc", Some(o.str("adoc").ok_or("adoc がありません")?));
            }
            txt(&mut parts, "stamps", o.str("stamps"));
        }
        "insert_blocks" => {
            num(&mut parts, "at", Some(o.num("at").ok_or("at がありません")?));
            txt(&mut parts, "adoc", Some(o.str("adoc").ok_or("adoc がありません")?));
        }
        "find" => txt(&mut parts, "text", Some(o.str("text").ok_or("text がありません")?)),
        "fill_fields" => {
            let grid = o.grid("values").ok_or("values がありません([[名前, 値], …])")?;
            parts.push(format!("\"values\":{}", J::A(grid.into_iter().map(J::A).collect()).to_json()));
        }
        "to_pdf" => txt(&mut parts, "path", Some(o.str("path").ok_or("path がありません")?)),
        "save" => txt(&mut parts, "path", o.str("path")),
        _ => return Err(format!("知らない道具です: {name}")),
    }
    Ok(format!("{{{}}}", parts.join(",")))
}

/// マクロの道具の名乗り。実行は ops の語彙でなく**サンドボックスの
/// Python** なので、結線はアプリの側(calc)が持つ。DirectHost /
/// QueueHost はこれを実行できない(知らない道具、と断る)ため、
/// 一覧に足すのは実行を持つアプリだけ
pub fn macro_tool() -> ToolDef {
    ToolDef {
        name: "run_macro".into(),
        description: "Python のマクロを書いて動かす。コードでは b(ブック)と \
                      s(いまのシート)が使える(openpyxl と同じ形)。\
                      1セルは s[\"A1\"].value = 5、範囲 s[\"A1:C9\"] は\
                      セルの行の組(それぞれの .value を読める)。\
                      print の出力が返り、表への変更は1手で入る。\
                      定型の道具に無い仕事はこれで解く"
            .into(),
        parameters: r#"{"type":"object","properties":{"name":{"type":"string","description":"マクロの名前(ファイル名になる)"},"code":{"type":"string"}},"required":["code"]}"#
            .into(),
    }
}

/// 道具呼びを ops の1行に組み替える。used_range だけは2段
/// (expand を呼んで番地に直す)なので、ここでは None を返す
fn line_for(name: &str, o: &Jobj) -> Result<String, String> {
    let mut parts: Vec<String> = Vec::new();
    let put_str = |parts: &mut Vec<String>, key: &str, from: Option<String>| {
        if let Some(v) = from {
            parts.push(format!("\"{key}\":{}", J::S(v).to_json()));
        }
    };
    match name {
        "book_info" => parts.push("\"cmd\":\"book_info\"".into()),
        "read_range" | "read_formulas" => {
            parts.push(format!(
                "\"cmd\":\"{}\"",
                if name == "read_range" { "get" } else { "get_formula" }
            ));
            let a1 = o.str("a1").ok_or("a1 がありません")?;
            let rows = rows_of(&a1);
            if rows > READ_ROWS_MAX {
                return Err(format!(
                    "{a1} は {rows} 行あります。一度に読めるのは {READ_ROWS_MAX} 行までです。大きな表は table_schema / table_head / table_query(SQL)で触ってください"
                ));
            }
            put_str(&mut parts, "a1", Some(a1));
            put_str(&mut parts, "sheet", o.str("sheet"));
        }
        "table_schema" | "table_head" | "table_query" => {
            parts.push(format!("\"cmd\":\"{name}\""));
            put_str(&mut parts, "table", Some(o.str("table").ok_or("table がありません")?));
            put_str(&mut parts, "sql", o.str("sql"));
            for k in ["n", "limit"] {
                if let Some(v) = o.num(k) {
                    parts.push(format!("\"{k}\":{}", v as i64));
                }
            }
        }
        "write_range" => {
            parts.push("\"cmd\":\"set\"".into());
            put_str(&mut parts, "a1", Some(o.str("a1").ok_or("a1 がありません")?));
            let grid = o.grid("values").ok_or("values がありません(2次元の並び)")?;
            let rows = J::A(grid.into_iter().map(J::A).collect());
            parts.push(format!("\"values\":{}", rows.to_json()));
            put_str(&mut parts, "sheet", o.str("sheet"));
        }
        "set_format" => {
            parts.push("\"cmd\":\"set_fmt\"".into());
            put_str(&mut parts, "a1", Some(o.str("a1").ok_or("a1 がありません")?));
            put_str(&mut parts, "sheet", o.str("sheet"));
            for k in ["bold", "italic"] {
                if let Some(b) = o.bool(k) {
                    parts.push(format!("\"{k}\":{b}"));
                }
            }
            put_str(&mut parts, "number_format", o.str("number_format"));
            put_str(&mut parts, "fill", o.str("fill"));
        }
        "autofit" => {
            parts.push("\"cmd\":\"autofit\"".into());
            put_str(&mut parts, "a1", Some(o.str("a1").ok_or("a1 がありません")?));
            put_str(&mut parts, "sheet", o.str("sheet"));
        }
        "save" => {
            parts.push("\"cmd\":\"save\"".into());
            put_str(&mut parts, "path", o.str("path"));
        }
        other => return Err(format!("知らない道具: {other}")),
    }
    Ok(format!("{{{}}}", parts.join(",")))
}

/// ops の返事を道具の結果にする。ok でなければ Err(理由)
fn result_for(reply: String) -> Result<String, String> {
    if reply.contains("\"ok\":true") {
        Ok(reply)
    } else {
        Err(lang::model::field(&reply, "error").unwrap_or(reply))
    }
}

/// 列の添字(0起点)→ 列の字(A・B・…・AA)
fn col_letters(mut c: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).expect("ASCII だけ")
}

/// 1つの道具呼びを、ops を呼ぶ関数の上で捌く(Direct と Queue の共通部)
fn call_with(
    ops_call: &mut dyn FnMut(&str) -> String,
    name: &str,
    arguments: &str,
) -> Result<String, String> {
    let args = if arguments.trim().is_empty() { "{}" } else { arguments };
    let o = Jobj::parse(args).ok_or("引数の JSON が読めません")?;
    if name == "used_range" {
        // A1 から地続きの表の大きさを訊いて、番地に直す
        let sheet = match o.str("sheet") {
            Some(s) => format!(",\"sheet\":{}", J::S(s).to_json()),
            None => String::new(),
        };
        let reply = result_for(ops_call(&format!(
            "{{\"cmd\":\"expand\",\"a1\":\"A1\"{sheet}}}"
        )))?;
        let rows = lang::model::usage(&reply, "rows").max(1);
        let cols = lang::model::usage(&reply, "cols").max(1);
        return Ok(format!(
            "{{\"ok\":true,\"a1\":\"A1:{}{}\"}}",
            col_letters(cols as u32 - 1),
            rows
        ));
    }
    result_for(ops_call(&line_for(name, &o)?))
}

/// ops::Host を直に持つ結線(試験と、画面の無い口)
pub struct DirectHost<'a, H: Host> {
    pub h: &'a mut H,
}

impl<H: Host> ToolHost for DirectHost<'_, H> {
    fn tools(&self) -> Vec<ToolDef> {
        sheet_tools()
    }
    fn call(&mut self, name: &str, arguments: &str) -> Result<String, String> {
        call_with(&mut |line| ops::handle(self.h, line), name, arguments)
    }
}

/// rpc と同じ受け口(ops::Queue)へ流す結線。ループを別スレッドで回し、
/// 状態はメインスレッドの汲み取りが捌く(rpc の 30ms の作法と同じ)
pub struct QueueHost {
    pub queue: ops::Queue,
    /// 捌かれるのを待つ長さ。汲み取りが止まっていたら諦めて Err
    pub timeout: std::time::Duration,
}

impl ToolHost for QueueHost {
    fn tools(&self) -> Vec<ToolDef> {
        sheet_tools()
    }
    fn call(&mut self, name: &str, arguments: &str) -> Result<String, String> {
        let queue = self.queue.clone();
        let timeout = self.timeout;
        call_with(
            &mut |line| {
                let (tx, rx) = std::sync::mpsc::channel();
                queue
                    .lock()
                    .expect("受け口の錠")
                    .push(ops::Req { line: line.to_string(), reply: tx });
                rx.recv_timeout(timeout)
                    .unwrap_or_else(|_| ops::err("アプリが応じません(汲み取りが止まっています)"))
            },
            name,
            arguments,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Agent, Model};

    #[test]
    fn big_ranges_are_refused_with_a_pointer_to_the_table_tools() {
        let o = Jobj::parse(r#"{"a1":"A1:D5000"}"#).unwrap();
        let e = line_for("read_range", &o).unwrap_err();
        assert!(e.contains("table_query") && e.contains("5000"), "{e}");
        let o = Jobj::parse(r#"{"a1":"A1:D200"}"#).unwrap();
        assert!(line_for("read_range", &o).is_ok(), "200 行までは読める");
        let o = Jobj::parse(r#"{"table":"売上","sql":"SELECT 1","limit":10}"#).unwrap();
        assert_eq!(line_for("table_query", &o).unwrap(), r#"{"cmd":"table_query","table":"売上","sql":"SELECT 1","limit":10}"#);
        let o = Jobj::parse(r#"{"table":"売上","n":3}"#).unwrap();
        assert_eq!(line_for("table_head", &o).unwrap(), r#"{"cmd":"table_head","table":"売上","n":3}"#);
        assert!(sheet_tools().iter().any(|t| t.name == "table_query"));
    }

    #[test]
    fn document_tools_map_onto_the_writer_socket_verbs() {
        let names: Vec<String> = doc_tools().iter().map(|t| t.name.clone()).collect();
        assert!(names.iter().all(|n| n.starts_with("doc_")), "{names:?}");
        let o = Jobj::parse(r#"{"start":2,"end":3,"adoc":"受注は4件。\n","stamps":"a1b2c3d4,e5f6a7b8"}"#).unwrap();
        let line = doc_line_for("doc_replace_blocks", &o).unwrap();
        assert_eq!(line, r#"{"cmd":"replace_blocks","from":2,"to":3,"adoc":"受注は4件。\n","stamps":"a1b2c3d4,e5f6a7b8"}"#);
        let o = Jobj::parse(r#"{"start":5}"#).unwrap();
        assert_eq!(doc_line_for("doc_read_blocks", &o).unwrap(), r#"{"cmd":"read_blocks","from":5}"#);
        let o = Jobj::parse(r#"{"values":[["氏名","山田"],["部署","総務"]]}"#).unwrap();
        assert_eq!(doc_line_for("doc_fill_fields", &o).unwrap(), r#"{"cmd":"fill_fields","values":[["氏名","山田"],["部署","総務"]]}"#);
        assert!(doc_line_for("doc_insert_blocks", &Jobj::parse(r#"{"adoc":"x"}"#).unwrap()).is_err());
        assert!(doc_line_for("read_range", &o).is_err());
    }
    use lang::model::{ChatOut, Msg, ToolCall};

    /// 画面の無い最小の口(ファイルの口と同じ立場)
    struct FileHost {
        book: book::Book,
    }

    impl Host for FileHost {
        fn app(&self) -> &'static str {
            "test"
        }
        fn book(&self) -> &book::Book {
            &self.book
        }
        fn book_mut(&mut self) -> &mut book::Book {
            &mut self.book
        }
        fn active(&self) -> usize {
            0
        }
        fn path(&self) -> Option<&std::path::Path> {
            None
        }
    }

    #[test]
    fn write_then_read_through_the_vocabulary() {
        let mut h = FileHost { book: book::Book::new() };
        let mut host = DirectHost { h: &mut h };
        let r = host
            .call("write_range", r#"{"a1":"A1","values":[[1,2],["=A1+B1",null]]}"#)
            .unwrap();
        assert!(r.contains("\"cells\":4"), "{r}");
        let r = host.call("read_range", r#"{"a1":"A1:B2"}"#).unwrap();
        assert!(r.contains("[[1,2],[3,null]]"), "式が計算されていない: {r}");
        let r = host.call("read_formulas", r#"{"a1":"A2"}"#).unwrap();
        assert!(r.contains("=A1+B1"), "{r}");
        let r = host.call("used_range", "{}").unwrap();
        assert!(r.contains("A1:B2"), "{r}");
        let r = host.call("book_info", "{}").unwrap();
        assert!(r.contains("\"ok\":true"), "{r}");
    }

    #[test]
    fn a_bad_call_comes_back_as_a_reason() {
        let mut h = FileHost { book: book::Book::new() };
        let mut host = DirectHost { h: &mut h };
        let e = host.call("read_range", "{}").unwrap_err();
        assert!(e.contains("a1"), "{e}");
        let e = host.call("save", "{}").unwrap_err();
        assert!(e.contains("保存先"), "{e}");
        let e = host.call("知らない", "{}").unwrap_err();
        assert!(e.contains("知らない道具"), "{e}");
    }

    #[test]
    fn column_letters_wrap_past_z() {
        assert_eq!(col_letters(0), "A");
        assert_eq!(col_letters(25), "Z");
        assert_eq!(col_letters(26), "AA");
        assert_eq!(col_letters(27), "AB");
    }

    /// 受け口ごしでも同じに動く — 別スレッドのループと、
    /// メインスレッドの汲み取りの形をそのまま試す
    #[test]
    fn the_queue_host_round_trips_through_a_pump() {
        let queue: ops::Queue = Default::default();
        let mut host =
            QueueHost { queue: queue.clone(), timeout: std::time::Duration::from_secs(5) };
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let pump = {
            let (queue, stop) = (queue.clone(), stop.clone());
            std::thread::spawn(move || {
                let mut h = FileHost { book: book::Book::new() };
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    let reqs: Vec<ops::Req> =
                        std::mem::take(&mut *queue.lock().expect("受け口の錠"));
                    for req in reqs {
                        let _ = req.reply.send(ops::handle(&mut h, &req.line));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            })
        };
        // 道具1つの往復をループごと通す(段階1の偽のモデル + 本物の結線)
        struct Scripted(Vec<ChatOut>);
        impl Model for Scripted {
            fn chat(&mut self, _m: &[Msg], _t: &[ToolDef]) -> Result<ChatOut, String> {
                Ok(self.0.remove(0))
            }
        }
        let mut model = Scripted(vec![
            ChatOut {
                tool_calls: vec![ToolCall {
                    id: "c1".into(),
                    name: "write_range".into(),
                    arguments: r#"{"a1":"A1","values":[[5,7]]}"#.into(),
                }],
                ..Default::default()
            },
            ChatOut { content: "書きました".into(), ..Default::default() },
        ]);
        let mut agent = Agent::new("s");
        let ans = agent.ask(&mut model, &mut host, "A1 に 5 と 7 を").unwrap();
        assert_eq!(ans, "書きました");
        let r = host.call("read_range", r#"{"a1":"A1:B1"}"#).unwrap();
        assert!(r.contains("[[5,7]]"), "{r}");
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        pump.join().expect("汲み取りの糸");
    }
}
