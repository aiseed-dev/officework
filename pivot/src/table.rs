//! **大きな表は polars の物として**(docs/sekkei/agent.ja.adoc「大きな calc の表は
//! polars の物として」。2026-09-04 発注者「大きな calc の表は、polars オブジェクトで
//! 対応」)。
//!
//! 数万行の表をセルで読ませるとトークンが尽きるので、エージェント・Python・MCP は
//! ここの3つで触ります。受けるのは pivot と同じ**字の表**(見出し + 中身)で、
//! 返すのも字の表です。
//!
//! 問い合わせの言葉は **SQL**(polars の sql)。設計の表は「polars の式」と書いたが、
//! 式の字を Rust で読む部品は無く、SQL は polars に用意があり、次期の DuckDB とも
//! 揃うので SQL にした(2026-09-04)。表は `FROM 名前` で指す。

use polars::prelude::*;
use polars::sql::SQLContext;

/// 列の名前と型(`number` / `text`。API の語は英語)と行の数
#[derive(Debug)]
pub struct Schema {
    pub cols: Vec<(String, &'static str)>,
    pub rows: usize,
}

/// 型を見る(数として読めた列は `number`、それ以外は `text`)
pub fn schema(head: &[String], body: &[Vec<String>]) -> Result<Schema, String> {
    let df = super::to_frame(head, body)?;
    let cols = df
        .columns()
        .iter()
        .map(|c| (c.name().to_string(), if c.dtype().is_numeric() { "number" } else { "text" }))
        .collect();
    Ok(Schema { cols, rows: body.len() })
}

/// 先頭 n 行(見出しつき)
pub fn head(head: &[String], body: &[Vec<String>], n: usize) -> Vec<Vec<String>> {
    let mut out = vec![head.to_vec()];
    out.extend(body.iter().take(n).cloned());
    out
}

/// 問い合わせの答え。`total` は絞った後の全行数で、`rows` は `limit` まで
#[derive(Debug)]
pub struct Answer {
    pub cols: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total: usize,
}

/// SQL で絞る・集計する。表は `name` で `FROM` に書く
pub fn query(
    name: &str,
    head: &[String],
    body: &[Vec<String>],
    sql: &str,
    limit: usize,
) -> Result<Answer, String> {
    let df = super::to_frame(head, body)?;
    let mut ctx = SQLContext::new();
    ctx.register(name, df.lazy());
    let out = ctx
        .execute(sql)
        .map_err(|e| format!("SQL が読めません: {e}"))?
        .collect()
        .map_err(|e| format!("問い合わせに失敗: {e}"))?;
    let total = out.height();
    let cols: Vec<String> = out.get_column_names().iter().map(|s| s.to_string()).collect();
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(total.min(limit));
    for i in 0..total.min(limit) {
        let mut r = Vec::with_capacity(cols.len());
        for c in out.columns() {
            let v = c.get(i).map_err(|e| e.to_string())?;
            r.push(cell_text(&v));
        }
        rows.push(r);
    }
    Ok(Answer { cols, rows, total })
}

/// セルの字(polars の値をそのままの字に。数は余分な小数を付けない)
fn cell_text(v: &AnyValue) -> String {
    match v {
        AnyValue::Null => String::new(),
        AnyValue::String(s) => s.to_string(),
        AnyValue::StringOwned(s) => s.to_string(),
        AnyValue::Float64(f) => trim_num(*f),
        AnyValue::Float32(f) => trim_num(*f as f64),
        // A moment with a zone is written the way the cell shows it
        AnyValue::Datetime(v, tu, Some(tz)) => moment_text(*v, *tu, tz.as_str()),
        AnyValue::DatetimeOwned(v, tu, Some(tz)) => moment_text(*v, *tu, tz.as_str()),
        other => other.to_string(),
    }
}

fn moment_text(v: i64, tu: TimeUnit, zone: &str) -> String {
    let secs = match tu {
        TimeUnit::Nanoseconds => v as f64 / 1e9,
        TimeUnit::Microseconds => v as f64 / 1e6,
        TimeUnit::Milliseconds => v as f64 / 1e3,
    };
    book::tz::show(secs, zone)
}

fn trim_num(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn sample() -> (Vec<String>, Vec<Vec<String>>) {
        let head = s(&["品名", "地区", "金額"]);
        let body = vec![
            s(&["鉛筆", "東", "100"]),
            s(&["ノート", "東", "250"]),
            s(&["鉛筆", "西", "300"]),
            s(&["消しゴム", "西", ""]),
        ];
        (head, body)
    }

    // moments with a zone, as ops passes them (2026-09-24)
    fn m(unix: i64, zone: &str) -> String {
        format!("\u{1d}{unix}[{zone}]")
    }

    #[test]
    fn a_column_of_one_zone_keeps_it_and_mixed_zones_become_utc() {
        // 2026-10-01 01:00 UTC (10:00 Tokyo) and 11:30 UTC (04:30 LA)
        let head = s(&["便", "出発", "到着"]);
        let body = vec![
            vec!["NH6".into(), m(1_790_816_400, "Asia/Tokyo"), m(1_790_854_200, "America/Los_Angeles")],
            vec!["NH5".into(), m(1_790_902_800, "Asia/Tokyo"), m(1_790_816_400, "Asia/Tokyo")],
        ];
        let df = crate::to_frame(&head, &body).unwrap();
        let dt = |c: &str| df.column(c).unwrap().dtype().clone();
        assert_eq!(dt("出発").to_string(), "datetime[μs, Asia/Tokyo]");
        assert_eq!(dt("到着").to_string(), "datetime[μs, UTC]", "mixed zones are UTC");
        // SQL sorts by the moment, and the answer is written as the cell shows it
        let a = query("旅程", &head, &body, "SELECT 便, 到着 FROM 旅程 ORDER BY 到着", 10).unwrap();
        assert_eq!(a.rows[0], s(&["NH5", "2026-10-01 01:00 UTC"]));
        assert_eq!(a.rows[1], s(&["NH6", "2026-10-01 11:30 UTC"]));
        let a = query("旅程", &head, &body, "SELECT 出発 FROM 旅程 WHERE 便 = 'NH6'", 10).unwrap();
        assert_eq!(a.rows[0], s(&["2026-10-01 10:00 Asia/Tokyo"]));
    }

    #[test]
    fn the_schema_tells_numbers_from_text_and_counts_rows() {
        let (h, b) = sample();
        let sc = schema(&h, &b).unwrap();
        assert_eq!(sc.rows, 4);
        assert_eq!(sc.cols, vec![("品名".to_string(), "text"), ("地区".to_string(), "text"), ("金額".to_string(), "number")]);
        assert_eq!(head(&h, &b, 2).len(), 3, "見出し + 2 行");
    }

    #[test]
    fn sql_groups_and_filters_the_table_and_reports_the_total() {
        let (h, b) = sample();
        let a = query("売上", &h, &b, "SELECT 品名, SUM(金額) AS 合計 FROM 売上 GROUP BY 品名 ORDER BY 合計 DESC", 200).unwrap();
        assert_eq!(a.cols, vec!["品名", "合計"]);
        assert_eq!(a.total, 3);
        // 空(null)の並ぶ位置は polars の流儀に任せ、中身だけ見る
        assert!(a.rows.contains(&vec!["鉛筆".to_string(), "400".to_string()]), "{:?}", a.rows);
        assert!(a.rows.contains(&vec!["ノート".to_string(), "250".to_string()]));
        assert!(a.rows.contains(&vec!["消しゴム".to_string(), String::new()]), "空は空のまま");
        let a = query("売上", &h, &b, "SELECT * FROM 売上 WHERE 地区 = '西'", 1).unwrap();
        assert_eq!(a.total, 2, "絞った後の全行数");
        assert_eq!(a.rows.len(), 1, "limit まで");
        assert!(query("売上", &h, &b, "SELEC x", 5).unwrap_err().contains("SQL"));
    }
}
