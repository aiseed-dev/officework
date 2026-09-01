//! 関数の答えの表(test/kansu_kotae.tsv)と、このエンジンの答えを
//! 突き合わせる。
//!
//! 表の答えは本家(いまは LibreOffice。Excel が使えるようになったら
//! 差し替え)が実際に計算した値で、正しさはこの表が決める。
//! 一致の基準: 数値は相対誤差 1e-10 以内、文字とエラーは字の一致。
//!
//!     cargo run -p sheet --example kansu_check -- test/kansu_kotae.tsv

use book::{Cell, Pos, Value};

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "test/kansu_kotae.tsv".into());
    let src = std::fs::read_to_string(&path).expect("答えの表が読めない");
    let mut ok = 0usize;
    let mut chigau: Vec<String> = Vec::new();
    let mut mijisso: Vec<&str> = Vec::new();
    let mut kesoku = 0usize;
    for line in src.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.splitn(3, '\t');
        let name = it.next().unwrap_or("");
        let formula = it.next().unwrap_or("");
        let kotae = it.next().unwrap_or("").trim();
        if kotae.is_empty() || kotae == "#NAME?" {
            // 正の側に答えが無い(本家に無い関数)。Excel が来たら埋まる
            kesoku += 1;
            continue;
        }
        let mut s = book::Sheet::new("検");
        s.set(Pos::new(0, 0), Cell::input(formula));
        book::calc::recalc(&mut s);
        let v = s.value(Pos::new(0, 0));
        let got = match &v {
            Value::Error(e) => e.clone(),
            Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            other => other.display(),
        };
        if got == "#NAME?" {
            mijisso.push(name);
            continue;
        }
        let hit = match (kotae.parse::<f64>(), &v) {
            (Ok(want), Value::Number(got_n)) => {
                (want - got_n).abs() <= want.abs().max(1.0) * 1e-10
            }
            _ => got == kotae,
        };
        if hit {
            ok += 1;
        } else {
            chigau.push(format!("  {name}: {formula} → 正 {kotae} / うち {got}"));
        }
    }
    mijisso.sort_unstable();
    mijisso.dedup();
    println!("一致 {ok} / 答えが違う {} / 未実装の関数 {} / 欠測 {kesoku}", chigau.len(), mijisso.len());
    if !chigau.is_empty() {
        println!("\n答えが違う:");
        for c in &chigau {
            println!("{c}");
        }
    }
    if !mijisso.is_empty() {
        println!("\n未実装: {}", mijisso.join(" "));
    }
    if !chigau.is_empty() {
        std::process::exit(1);
    }
}
