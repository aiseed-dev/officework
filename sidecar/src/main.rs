//! stdio の入り口 — genoffice が `XLSX_SIDECAR_PATH` でこの実行ファイルを指す。
//!
//! 言葉の中身は庫([`sidecar::Bridge`])にある。ここは**プロセスで包む係**
//! だけ — スマホは同じ橋を関数(`sidecar::ffi`)で呼ぶ(iOS はサブプロセスを
//! 起こせないため)。この入り口が薄いのは意図: 運び方が増えても言葉は1つ。
use std::io::{self, BufRead, BufWriter, Write};

use sidecar::Bridge;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut bridge = Bridge::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                let _ = writeln!(out, "{}", sidecar::io_error_line(&e.to_string()));
                let _ = out.flush();
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let res = bridge.handle(&line);
        let _ = writeln!(out, "{res}");
        let _ = out.flush();
    }
}
