//! **ピボットの集計。** polars(Rust)で回します。
//!
//! 2026-08-29 発注者「ピボットの処理は polars をつかって。polars が
//! rust のライブラリーもあるのでは」。
//!
//! いままでは Python の polars を**別プロセスで**呼んでいました
//! (`pyrun::PIVOT_PY`)。動いているアプリからしか使えず、
//! `pip install officework` した人は集計できません。ここに移すと、
//! エンジンだけで回ります。
//!
//! # 何を受けて何を返すか
//!
//! 受けるのは**字の表**(見出しの行 + 中身の行)と指図です。返すのも
//! 字の表で、そのままシートに貼れます。セルの型を跨がないので、
//! 呼ぶ側は xlsx でも adoc でも同じです。
//!
//! # 列に広げる所は自分で組みます
//!
//! polars 0.55 の `pivot` の機能は縦にする方(`unpivot`)だけで、
//! 横に広げる関数は入っていません。行と列の両方で `group_by` してから、
//! こちらで並べ直します。

use polars::prelude::*;
use std::collections::BTreeMap;

/// 集計の指図。[`book::PivotDef`] から要る所だけを写した物です
#[derive(Debug, Clone, Default)]
pub struct Spec {
    /// 行に並べる見出し
    pub rows: Vec<String>,
    /// 列に広げる見出し(空なら広げない)
    pub cols: Vec<String>,
    /// 集計する値の見出し
    pub value: String,
    /// 集計の仕方。sum / count / mean / min / max / median
    pub agg: String,
    /// 総計を出すか
    pub totals: bool,
}

/// **行の種類。** 置くときの見た目(帯・太字)を決めます。
///
/// Python の台本と同じ記号です(`h` 見出し / `d` 明細 / `s` 小計 /
/// `b` 空行 / `t` 総計)。
pub const KIND_HEAD: char = 'h';
pub const KIND_DATA: char = 'd';
pub const KIND_TOTAL: char = 't';

/// 集計の答え。字の表と、行ごとの種類です
#[derive(Debug, Clone, Default)]
pub struct Grid {
    pub rows: Vec<Vec<String>>,
    /// `rows` と同じ長さ。[`KIND_HEAD`] などが入ります
    pub kinds: Vec<char>,
}

/// **字の表を集計する。**
///
/// `head` は見出しの行、`body` は中身の行です。返りは見出しを含む
/// 字の表で、そのまま貼れます。
pub fn run(head: &[String], body: &[Vec<String>], spec: &Spec) -> Result<Grid, String> {
    if spec.rows.is_empty() {
        return Err("行に並べる見出しがありません".into());
    }
    for name in spec.rows.iter().chain(spec.cols.iter()).chain([&spec.value]) {
        if !head.contains(name) {
            return Err(format!("「{name}」という見出しが表にありません"));
        }
    }
    let df = to_frame(head, body)?;
    let agg = agg_expr(&spec.agg, &spec.value)?;
    let key: Vec<Expr> = spec
        .rows
        .iter()
        .chain(spec.cols.iter())
        .map(|c| col(c.as_str()))
        .collect();
    let g = df
        .lazy()
        .group_by(key)
        .agg([agg])
        .collect()
        .map_err(|e| format!("集計できません: {e}"))?;
    let rows = if spec.cols.is_empty() { tate(&g, spec) } else { yoko(&g, spec) };
    // 1行目が見出し、最後が総計(出していれば)、あいだが明細
    let mut kinds = vec![KIND_DATA; rows.len()];
    if !kinds.is_empty() {
        kinds[0] = KIND_HEAD;
    }
    if spec.totals && kinds.len() > 1 {
        let last = kinds.len() - 1;
        kinds[last] = KIND_TOTAL;
    }
    Ok(Grid { rows, kinds })
}

/// 列に広げない形(行の見出し + 値の1欄)
fn tate(g: &DataFrame, spec: &Spec) -> Vec<Vec<String>> {
    let mut kumi: Vec<(Vec<String>, String)> = Vec::with_capacity(g.height());
    for i in 0..g.height() {
        let k: Vec<String> = spec.rows.iter().map(|c| cell(g, c, i)).collect();
        kumi.push((k, cell(g, &spec.value, i)));
    }
    kumi.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = vec![spec
        .rows
        .iter()
        .cloned()
        .chain([spec.value.clone()])
        .collect::<Vec<_>>()];
    for (k, v) in &kumi {
        out.push(k.iter().cloned().chain([v.clone()]).collect());
    }
    if spec.totals {
        let mut gyou = vec![String::new(); spec.rows.len()];
        gyou[0] = "総計".into();
        gyou.push(tashi(kumi.iter().map(|(_, v)| v.as_str())));
        out.push(gyou);
    }
    out
}

/// 列に広げる形。**並べ直しはこちらで組みます**(polars に横の pivot が無い)
fn yoko(g: &DataFrame, spec: &Spec) -> Vec<Vec<String>> {
    // (行の組, 列の組) → 値
    let mut hako: BTreeMap<Vec<String>, BTreeMap<Vec<String>, String>> = BTreeMap::new();
    let mut retsu: Vec<Vec<String>> = Vec::new();
    for i in 0..g.height() {
        let r: Vec<String> = spec.rows.iter().map(|c| cell(g, c, i)).collect();
        let c: Vec<String> = spec.cols.iter().map(|c| cell(g, c, i)).collect();
        if !retsu.contains(&c) {
            retsu.push(c.clone());
        }
        hako.entry(r).or_default().insert(c, cell(g, &spec.value, i));
    }
    retsu.sort();

    let mut atama: Vec<String> = spec.rows.clone();
    atama.extend(retsu.iter().map(|c| c.join(" / ")));
    if spec.totals {
        atama.push("総計".into());
    }
    let mut out = vec![atama];
    for (r, naka) in &hako {
        let mut gyou = r.clone();
        for c in &retsu {
            gyou.push(naka.get(c).cloned().unwrap_or_default());
        }
        if spec.totals {
            gyou.push(tashi(retsu.iter().filter_map(|c| naka.get(c).map(|s| s.as_str()))));
        }
        out.push(gyou);
    }
    if spec.totals {
        let mut gyou = vec![String::new(); spec.rows.len()];
        gyou[0] = "総計".into();
        for c in &retsu {
            gyou.push(tashi(hako.values().filter_map(|n| n.get(c).map(|s| s.as_str()))));
        }
        gyou.push(tashi(
            hako.values().flat_map(|n| n.values().map(|s| s.as_str())),
        ));
        out.push(gyou);
    }
    out
}

/// 字の数を足す。**数でない物は飛ばします**(count の答えも数です)
fn tashi<'a>(it: impl Iterator<Item = &'a str>) -> String {
    let mut wa = 0.0f64;
    let mut atta = false;
    for s in it {
        if let Ok(v) = s.trim().parse::<f64>() {
            wa += v;
            atta = true;
        }
    }
    if atta {
        kazu(wa)
    } else {
        String::new()
    }
}

/// 数を字に。**整数は小数点を付けません**(1 を 1.0 と刷らない)
fn kazu(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn cell(df: &DataFrame, name: &str, i: usize) -> String {
    match df.column(name).and_then(|c| c.get(i)) {
        Ok(AnyValue::Null) => String::new(),
        Ok(AnyValue::Float64(v)) => kazu(v),
        Ok(AnyValue::Float32(v)) => kazu(v as f64),
        Ok(v) => v.to_string().trim_matches('"').to_string(),
        Err(_) => String::new(),
    }
}

/// 字の表を DataFrame に。**数に見える欄は数として読みます** —
/// 合計や平均は数でないと出せません
fn to_frame(head: &[String], body: &[Vec<String>]) -> Result<DataFrame, String> {
    let mut cols: Vec<Column> = Vec::with_capacity(head.len());
    for (i, name) in head.iter().enumerate() {
        let moji: Vec<&str> =
            body.iter().map(|r| r.get(i).map(|s| s.as_str()).unwrap_or("")).collect();
        let kazu: Option<Vec<Option<f64>>> = moji
            .iter()
            .map(|s| {
                let t = s.trim();
                if t.is_empty() {
                    Some(None)
                } else {
                    t.parse::<f64>().ok().map(Some)
                }
            })
            .collect();
        cols.push(match kazu {
            Some(v) if moji.iter().any(|s| !s.trim().is_empty()) => {
                Column::new(name.as_str().into(), v)
            }
            _ => Column::new(name.as_str().into(), moji),
        });
    }
    // **行数を先に渡します**(0.55 で変わりました)。列が1本も無い表でも
    // 行数が決まるようにするためです
    DataFrame::new(body.len(), cols).map_err(|e| format!("表として読めません: {e}"))
}

fn agg_expr(agg: &str, value: &str) -> Result<Expr, String> {
    let c = col(value);
    Ok(match agg {
        "sum" | "" => c.sum(),
        "count" => c.count(),
        "mean" | "avg" => c.mean(),
        "min" => c.min(),
        "max" => c.max(),
        "median" => c.median(),
        other => {
            return Err(format!(
                "集計の仕方に「{other}」はありません。使えるのは \
                 sum / count / mean / min / max / median"
            ))
        }
    }
    .alias(value))
}

/// **ブックの中で集計して、置く。**
///
/// 指図([`book::PivotDef`])のとおりに元の表を読み、集計して、
/// `dest` へ字で置きます。返りは置いた広さ(行数, 列数)です。
///
/// アプリが動いていなくても回ります。`pip install officework` した人が
/// ピボットを使えるようにするための入り口です(2026-08-29)。
pub fn apply(book: &mut book::Book, def: &mut book::PivotDef) -> Result<(u32, u32), String> {
    let (head, body) = read_src(book, def)?;
    let spec = Spec {
        rows: def.rows_sel.clone(),
        cols: def.cols_sel.clone(),
        value: def.value.clone(),
        agg: def.agg.clone(),
        totals: def.totals,
    };
    let g = run(&head, &body, &spec)?;
    let si = book
        .sheets
        .iter()
        .position(|s| s.name == def.sheet)
        .ok_or_else(|| format!("シート「{}」がありません", def.sheet))?;
    // **前に置いた面を消してから置きます。** 残すと、小さくなったときに
    // 古い数が右や下に残ります
    for r in 0..def.size.0 {
        for c in 0..def.size.1 {
            book.sheets[si]
                .cells
                .remove(&book::Pos::new(def.dest.row + r, def.dest.col + c));
        }
    }
    let h = g.rows.len() as u32;
    let w = g.rows.iter().map(|r| r.len()).max().unwrap_or(1) as u32;
    for (dr, row) in g.rows.iter().enumerate() {
        for (dc, text) in row.iter().enumerate() {
            let p = book::Pos::new(def.dest.row + dr as u32, def.dest.col + dc as u32);
            let mut cell = book::Cell::input(text);
            // 見出しと総計は太字。**帯の色は画面の受け持ち**なので付けません
            if matches!(g.kinds.get(dr), Some(&KIND_HEAD) | Some(&KIND_TOTAL)) {
                cell.fmt.bold = true;
            }
            book.sheets[si].set(p, cell);
        }
    }
    def.size = (h, w);
    Ok((h, w))
}

/// 元の表を字で読む。1行目が見出しです
fn read_src(
    book: &book::Book,
    def: &book::PivotDef,
) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let s = book
        .sheets
        .iter()
        .find(|s| s.name == def.sheet)
        .ok_or_else(|| format!("元の表のシート「{}」がありません", def.sheet))?;
    let (a, b) = def.src;
    if b.row <= a.row {
        return Err("元の表に中身の行がありません(1行目は見出しです)".into());
    }
    let moji = |p: book::Pos| -> String {
        s.get(p)
            .map(|c| book::format_value(&c.value, c.fmt.number_format.as_deref(), false))
            .unwrap_or_default()
    };
    let head: Vec<String> =
        (a.col..=b.col).map(|c| moji(book::Pos::new(a.row, c))).collect();
    let body: Vec<Vec<String>> = (a.row + 1..=b.row)
        .map(|r| (a.col..=b.col).map(|c| moji(book::Pos::new(r, c))).collect())
        .collect();
    Ok((head, body))
}
