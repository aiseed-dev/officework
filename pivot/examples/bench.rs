//! Rust の polars と、いままでの Python の別プロセスを、同じ表で比べる。
//!
//!     cargo run -q -p pivot --release --example bench -- 10000
use std::time::Instant;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(10_000);
    let mise = ["東京", "大阪", "名古屋", "札幌", "福岡"];
    let tsuki = ["4月", "5月", "6月", "7月"];
    let head: Vec<String> =
        ["支店", "月", "品名", "金額"].iter().map(|s| s.to_string()).collect();
    let body: Vec<Vec<String>> = (0..n)
        .map(|i| {
            vec![
                mise[i % mise.len()].to_string(),
                tsuki[i % tsuki.len()].to_string(),
                format!("品{}", i % 20),
                ((i * 37) % 5000).to_string(),
            ]
        })
        .collect();
    let spec = pivot::Spec {
        rows: vec!["支店".into(), "品名".into()],
        cols: vec!["月".into()],
        value: "金額".into(),
        agg: "sum".into(),
        totals: true,
    };

    // 1度暖める(初回は割り当てが入る)
    let _ = pivot::run(&head, &body, &spec);
    let t = Instant::now();
    let out = pivot::run(&head, &body, &spec).expect("集計");
    let rust = t.elapsed();
    println!("{n} 行");
    println!("  Rust の polars : {:?}  ({} 行 × {} 列)", rust, out.rows.len(), out.rows[0].len());

    // Python の別プロセス(いままでの道)
    let spec_json = serde_like(&head, &body);
    let dir = std::env::temp_dir().join("pivot-bench");
    std::fs::create_dir_all(&dir).unwrap();
    let sp = dir.join("spec.json");
    std::fs::write(&sp, spec_json).unwrap();
    let py = dir.join("pivot.py");
    std::fs::write(&py, pyrun::PIVOT_PY).unwrap();
    let t = Instant::now();
    let o = std::process::Command::new("/home/dev/dev/officework/.venv/bin/python")
        .arg(&py)
        .arg(&sp)
        .output();
    let python = t.elapsed();
    match o {
        Ok(o) if o.status.success() => {
            let gyou = o.stdout.iter().filter(|b| **b == 0x1e).count();
            println!("  Python の別プロセス: {python:?}  ({gyou} 行)");
            println!("  **{:.0} 倍**速い", python.as_secs_f64() / rust.as_secs_f64());
        }
        Ok(o) => println!("  Python: 落ちた: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("")),
        Err(e) => println!("  Python: 起動できない: {e}"),
    }
}

/// 指図の JSON を手で組む(この見本のためだけ)
fn serde_like(head: &[String], body: &[Vec<String>]) -> String {
    let q = |s: &str| format!("{:?}", s);
    let h: Vec<String> = head.iter().map(|s| q(s)).collect();
    let r: Vec<String> = body
        .iter()
        .map(|row| format!("[{}]", row.iter().map(|s| q(s)).collect::<Vec<_>>().join(",")))
        .collect();
    format!(
        r#"{{"headers":[{}],"rows":[{}],"index":["支店","品名"],"columns":["月"],"value":"金額","agg":"sum","totals":true,"subtotals":false,"blank_rows":false,"compact":true,"hide":[],"group_by":[],"show_as":"","sort":"","vfilter":null}}"#,
        h.join(","),
        r.join(",")
    )
}
