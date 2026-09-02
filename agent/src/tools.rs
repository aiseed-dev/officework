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
    ]
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
            put_str(&mut parts, "a1", Some(o.str("a1").ok_or("a1 がありません")?));
            put_str(&mut parts, "sheet", o.str("sheet"));
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
