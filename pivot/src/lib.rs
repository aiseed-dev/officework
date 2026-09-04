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

/// 大きな表を polars の物として扱う道具(table_schema / table_head / table_query)
pub mod table;

/// 集計の指図。[`book::PivotDef`] から写した物です
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
    /// 小計(行の見出しが2つ以上のとき、1つ目の区切りごと)
    pub subtotals: bool,
    /// 空行(1つ目の区切りごとに1行空ける)
    pub blank_rows: bool,
    /// コンパクト形式(繰り返しの見出しを空欄に)
    pub compact: bool,
    /// 絞り込み: (見出し, 隠す値の並び)
    pub hide: Vec<(String, Vec<String>)>,
    /// グループ化: (見出し, 単位)。単位は months / quarters / years / 幅:N
    pub group_by: Vec<(String, String)>,
    /// 値のフィルター: (比較 ">" ">=" "<" "<=" "=", しきい値)
    pub vfilter: Option<(String, f64)>,
    /// 計算の種類。"" / total(総計に対する%)/ running_total / difference
    pub show_as: String,
    /// 並べ替え。"" / labels_z / labels_z_2 / largest_value_first /
    /// smallest_value_first
    pub sort: String,
    /// 小計の行の札。`{}` が区切りの名前に変わります
    pub subtotal_label: String,
    /// 総計の行の札
    pub grand_label: String,
    /// 値の欄の札(「合計 / 金額」の「合計」)。空なら `agg` の綴りのまま
    pub agg_label: String,
}

/// **行の種類。** 置くときの見た目(帯・太字)を決めます。
///
/// Python の台本と同じ記号です(`h` 見出し / `d` 明細 / `s` 小計 /
/// `b` 空行 / `t` 総計)。
pub const KIND_HEAD: char = 'h';
pub const KIND_DATA: char = 'd';
pub const KIND_TOTAL: char = 't';
pub const KIND_SUB: char = 's';
pub const KIND_BLANK: char = 'b';
/// 列に広げたときの**1行目の札**(集計の名前と、広げた見出し)
pub const KIND_LABEL: char = 'l';

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
    // ① 絞り込み(見出しの ▼)。隠す値を先に落としてから集計します
    let body = shibori(head, body, &spec.hide);
    // ② グループ化(日付を 月/四半期/年 に、数を幅Nの帯に)
    let body = matome(head, &body, &spec.group_by);

    let df = to_frame(head, &body)?;
    let g = atsumeru(&df, spec, &spec.rows, &spec.cols)?;
    let mut kumi = if spec.cols.is_empty() { tate(&g, spec) } else { yoko(&g, spec) };

    // ③ 値のフィルター(集計した後の行に掛ける)
    if let Some((op, th)) = &spec.vfilter {
        let n = spec.rows.len();
        kumi.rows.retain(|r| {
            let v = r.last().and_then(|s| s.trim().parse::<f64>().ok());
            let _ = n;
            match v {
                Some(v) => match op.as_str() {
                    ">" => v > *th,
                    ">=" => v >= *th,
                    "<" => v < *th,
                    "<=" => v <= *th,
                    "=" => (v - *th).abs() < f64::EPSILON,
                    _ => true,
                },
                None => false,
            }
        });
    }

    // ④ 並べ替え。**小計と空行を出しているときは掛けません** —
    // 区切りの塊が崩れるためです
    if !spec.sort.is_empty() && !spec.subtotals && !spec.blank_rows {
        narabe(&mut kumi, spec);
    }

    // ⑤ 小計・空行・総計を挟みながら、字の表に組む
    let (rows, kinds) = kumitate(&df, &g, kumi, spec)?;
    Ok(Grid { rows, kinds })
}

/// 値の欄の見出し。**「集計の仕方 / 見出し」**の形です(Excel と同じ)
fn atai_midashi(spec: &Spec) -> String {
    let na = if spec.agg_label.is_empty() { &spec.agg } else { &spec.agg_label };
    format!("{na} / {}", spec.value)
}

/// 集計した組。見出しの並び・行の並び・列の見出し(広げたとき)
struct Kumi {
    /// 見出しの行
    head: Vec<String>,
    /// 明細の行(見出しの欄 + 値の欄)
    rows: Vec<Vec<String>>,
    /// 列に広げたときの列の組。広げていなければ空
    #[allow(dead_code)]
    retsu: Vec<Vec<String>>,
}

/// 列に広げない形(行の見出し + 値の1欄)
fn tate(g: &DataFrame, spec: &Spec) -> Kumi {
    let mut rows: Vec<Vec<String>> = (0..g.height())
        .map(|i| {
            let mut r: Vec<String> = spec.rows.iter().map(|c| cell(g, c, i)).collect();
            r.push(cell(g, &spec.value, i));
            r
        })
        .collect();
    rows.sort_by(|a, b| a[..spec.rows.len()].cmp(&b[..spec.rows.len()]));
    let mut head = spec.rows.clone();
    head.push(atai_midashi(spec));
    Kumi { head, rows, retsu: Vec::new() }
}

/// 列に広げる形。**並べ直しはこちらで組みます**(polars に横の pivot が無い)
fn yoko(g: &DataFrame, spec: &Spec) -> Kumi {
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
    let mut head: Vec<String> = spec.rows.clone();
    head.extend(retsu.iter().map(|c| c.join(" / ")));
    let rows: Vec<Vec<String>> = hako
        .into_iter()
        .map(|(r, naka)| {
            let mut gyou = r;
            for c in &retsu {
                // **合計は、組み合わせが無くても 0**(空の合計)。平均などは
                // 空欄のままです(数が1つも無いので平均が決まらない)
                gyou.push(naka.get(c).cloned().unwrap_or_else(|| {
                    if spec.agg == "sum" || spec.agg.is_empty() || spec.agg == "count" {
                        "0".into()
                    } else {
                        String::new()
                    }
                }));
            }
            gyou
        })
        .collect();
    Kumi { head, rows, retsu }
}

/// **絞り込み。** 隠す値の行を落とします
fn shibori(
    head: &[String],
    body: &[Vec<String>],
    hide: &[(String, Vec<String>)],
) -> Vec<Vec<String>> {
    if hide.iter().all(|(_, v)| v.is_empty()) {
        return body.to_vec();
    }
    let kumi: Vec<(usize, &Vec<String>)> = hide
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .filter_map(|(f, v)| head.iter().position(|h| h == f).map(|i| (i, v)))
        .collect();
    body.iter()
        .filter(|r| {
            !kumi.iter().any(|(i, kakusu)| {
                r.get(*i).map(|v| kakusu.contains(v)).unwrap_or(false)
            })
        })
        .cloned()
        .collect()
}

/// **グループ化。** 日付を 月/四半期/年 の札に、数を幅Nの帯に置き換えます。
///
/// 読めない値はそのまま残します(黙って落としません)。
fn matome(
    head: &[String],
    body: &[Vec<String>],
    group: &[(String, String)],
) -> Vec<Vec<String>> {
    if group.is_empty() {
        return body.to_vec();
    }
    let mut out = body.to_vec();
    for (f, unit) in group {
        let Some(i) = head.iter().position(|h| h == f) else { continue };
        if let Some(w) = unit.strip_prefix("幅:").and_then(|s| s.trim().parse::<f64>().ok()) {
            if w <= 0.0 {
                continue;
            }
            // 帯の札の幅をそろえる(字の順でも 0〜49 < 50〜99 < 100〜149)
            let mx = out
                .iter()
                .filter_map(|r| r.get(i).and_then(|s| s.trim().parse::<f64>().ok()))
                .fold(f64::NEG_INFINITY, f64::max);
            let hasu = w.fract() != 0.0;
            let hiku = if hasu { 0.0 } else { 1.0 };
            let keta = if mx.is_finite() {
                kazu((mx / w).floor() * w + w - hiku).chars().count()
            } else {
                0
            };
            for r in out.iter_mut() {
                let Some(v) = r.get(i).and_then(|s| s.trim().parse::<f64>().ok()) else {
                    continue;
                };
                let lo = (v / w).floor() * w;
                r[i] = format!(
                    "{}〜{}",
                    migi(&kazu(lo), keta),
                    migi(&kazu(lo + w - hiku), keta)
                );
            }
        } else {
            for r in out.iter_mut() {
                let Some(d) = r.get(i).and_then(|s| hiduke(s)) else { continue };
                let (y, m) = d;
                r[i] = match unit.as_str() {
                    "years" => format!("{y}年"),
                    "quarters" => format!("{y}年Q{}", m.div_ceil(3)),
                    _ => format!("{y}-{m:02}"),
                };
            }
        }
    }
    out
}

/// `2026-08-05` と `2026/08/05` を (年, 月) に。読めなければ None
fn hiduke(s: &str) -> Option<(i32, u32)> {
    let t = s.trim();
    let mut it = t.split(['-', '/']);
    let y: i32 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let _d: u32 = it.next()?.trim().parse().ok()?;
    (1..=12).contains(&m).then_some((y, m))
}

/// 右詰め(帯の札の桁をそろえる)
fn migi(s: &str, keta: usize) -> String {
    let n = s.chars().count();
    if n >= keta {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(keta - n))
    }
}

/// **並べ替え。** 見出しの順か、値の大きさ順
fn narabe(kumi: &mut Kumi, spec: &Spec) {
    let n = spec.rows.len();
    let atai = |r: &Vec<String>| -> f64 {
        r.get(n).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(f64::NEG_INFINITY)
    };
    match spec.sort.as_str() {
        // `labels_z` が A→Z(昇順)、`labels_z_2` が Z→A(降順)です
        "labels_z" => kumi.rows.sort_by(|a, b| a[..n].cmp(&b[..n])),
        "labels_z_2" => kumi.rows.sort_by(|a, b| b[..n].cmp(&a[..n])),
        "largest_value_first" => {
            kumi.rows.sort_by(|a, b| atai(b).partial_cmp(&atai(a)).unwrap_or(std::cmp::Ordering::Equal))
        }
        "smallest_value_first" => {
            kumi.rows.sort_by(|a, b| atai(a).partial_cmp(&atai(b)).unwrap_or(std::cmp::Ordering::Equal))
        }
        _ => {}
    }
}

/// polars で集計する(行の見出し × 列の見出しで束ねる)
fn atsumeru(
    df: &DataFrame,
    spec: &Spec,
    rows: &[String],
    cols: &[String],
) -> Result<DataFrame, String> {
    let key: Vec<Expr> = rows.iter().chain(cols.iter()).map(|c| col(c.as_str())).collect();
    df.clone()
        .lazy()
        .group_by(key)
        .agg([agg_expr(&spec.agg, &spec.value)?])
        .collect()
        .map_err(|e| format!("集計できません: {e}"))
}

/// **字の表に組む。** 小計・空行・総計をここで挟み、計算の種類を掛けます
fn kumitate(
    df: &DataFrame,
    _g: &DataFrame,
    kumi: Kumi,
    spec: &Spec,
) -> Result<(Vec<Vec<String>>, Vec<char>), String> {
    let n = spec.rows.len();
    let tot_col = spec.totals && !spec.cols.is_empty();

    // 行ごとの総計(列に広げた分をまとめた値)
    let mut kumi = kumi;
    if tot_col {
        kumi.head.push(spec.grand_label.clone());
        for r in kumi.rows.iter_mut() {
            let wa = tashi(r[n..].iter().map(|s| s.as_str()));
            r.push(wa);
        }
    }

    // 計算の種類(比率・累計・差)。**明細の値の欄だけ**を置き換えます
    if !spec.show_as.is_empty() {
        keisan(&mut kumi.rows, n, spec, df)?;
    }

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut kinds: Vec<char> = Vec::new();
    // **列に広げたときは見出しが2行**です(1行目に「何を集計したか」と
    // 「どの見出しで広げたか」、2行目に列の名前)。Python の台本と同じ形
    if !spec.cols.is_empty() {
        // **列の見出しは、行の見出しの右隣**に置きます(Excel と同じ形)。
        // 行の見出しが2つなら1つ空けてから
        let mut ue = vec![atai_midashi(spec)];
        ue.extend(std::iter::repeat_n(String::new(), n.saturating_sub(1)));
        ue.push(spec.cols.join(" / "));
        while ue.len() < kumi.head.len() {
            ue.push(String::new());
        }
        rows.push(ue);
        kinds.push(KIND_LABEL);
    }
    rows.push(kumi.head.clone());
    kinds.push(KIND_HEAD);

    // 1つ目の見出しで束ねながら出す(小計と空行はその区切りごと)
    let mut i = 0usize;
    while i < kumi.rows.len() {
        let g = kumi.rows[i].first().cloned().unwrap_or_default();
        let mut katamari: Vec<Vec<String>> = Vec::new();
        while i < kumi.rows.len() && kumi.rows[i].first() == Some(&g) {
            katamari.push(kumi.rows[i].clone());
            i += 1;
        }
        let mut mae: Option<Vec<String>> = None;
        for r in &katamari {
            let mut cells = r.clone();
            // コンパクト形式(繰り返しの見出しを空欄に)
            if spec.compact {
                if let Some(m) = &mae {
                    for k in 0..n {
                        if cells[k] == m[k] {
                            cells[k] = String::new();
                        } else {
                            break;
                        }
                    }
                }
            }
            rows.push(cells);
            kinds.push(KIND_DATA);
            mae = Some(r.clone());
        }
        // 小計。**行の見出しが2つ以上のときだけ**(1つなら明細と同じ)
        if spec.subtotals && n >= 2 {
            let mut cells = vec![spec.subtotal_label.replace("{}", &g)];
            cells.extend(std::iter::repeat_n(String::new(), n - 1));
            for j in n..kumi.head.len() {
                cells.push(tashi(katamari.iter().map(|r| r[j].as_str())));
            }
            rows.push(cells);
            kinds.push(KIND_SUB);
        }
        if spec.blank_rows && n >= 2 {
            rows.push(vec![String::new(); kumi.head.len()]);
            kinds.push(KIND_BLANK);
        }
    }

    // 総計
    if spec.totals && !kumi.rows.is_empty() {
        let mut cells = vec![spec.grand_label.clone()];
        cells.extend(std::iter::repeat_n(String::new(), n.saturating_sub(1)));
        for j in n..kumi.head.len() {
            cells.push(tashi(kumi.rows.iter().map(|r| r[j].as_str())));
        }
        rows.push(cells);
        kinds.push(KIND_TOTAL);
    }
    Ok((rows, kinds))
}

/// 計算の種類。比率は総計を 100%、累計は積み上げ、差は前の行との差
fn keisan(
    rows: &mut [Vec<String>],
    n: usize,
    spec: &Spec,
    df: &DataFrame,
) -> Result<(), String> {
    match spec.show_as.as_str() {
        "total" => {
            let sou: f64 = rows
                .iter()
                .filter_map(|r| r.get(n).and_then(|s| s.trim().parse::<f64>().ok()))
                .sum();
            let _ = df;
            if sou == 0.0 {
                return Ok(());
            }
            for r in rows.iter_mut() {
                for j in n..r.len() {
                    r[j] = match r[j].trim().parse::<f64>() {
                        Ok(v) => format!("{:.1}%", v / sou * 100.0),
                        Err(_) => String::new(),
                    };
                }
            }
        }
        "running_total" | "difference" => {
            let haba = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            let mut mae: Vec<Option<f64>> = vec![None; haba];
            for r in rows.iter_mut() {
                for j in n..r.len() {
                    let v = r[j].trim().parse::<f64>().ok();
                    match v {
                        Some(v) if spec.show_as == "running_total" => {
                            let atarashii = v + mae[j].unwrap_or(0.0);
                            r[j] = kazu(atarashii);
                            mae[j] = Some(atarashii);
                        }
                        Some(v) => {
                            r[j] = match mae[j] {
                                Some(p) => kazu(v - p),
                                None => String::new(),
                            };
                            mae[j] = Some(v);
                        }
                        None => r[j] = String::new(),
                    }
                }
            }
        }
        other => {
            return Err(format!(
                "計算の種類に「{other}」はありません。使えるのは \
                 total / running_total / difference"
            ))
        }
    }
    Ok(())
}

/// **ブックの中で集計して、置く。**
///
/// 指図([`book::PivotDef`])のとおりに元の表を読み、集計して、
/// `dest` へ字で置きます。返りは置いた広さ(行数, 列数)です。
///
/// アプリが動いていなくても回ります。`pip install officework` した人が
/// ピボットを使えるようにするための入り口です(2026-08-29)。
pub fn apply(book: &mut book::Book, def: &mut book::PivotDef) -> Result<(u32, u32), String> {
    let spec = from_def(def);
    apply_with(book, def, &spec)
}

/// [`apply`] と同じですが、**札を呼ぶ側が決められます**(画面の言語で
/// 「総計」と書きたいときに使います)
pub fn apply_with(
    book: &mut book::Book,
    def: &mut book::PivotDef,
    spec: &Spec,
) -> Result<(u32, u32), String> {
    let (head, body) = read_src(book, def)?;
    let g = run(&head, &body, spec)?;
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

/// 指図([`book::PivotDef`])を集計の指図に写す。
///
/// **札は呼ぶ側が入れます。** 画面の言語で「小計」「総計」と書きたいので、
/// ここは英語の既定を置くだけです([`Spec::subtotal_label`])。
pub fn from_def(def: &book::PivotDef) -> Spec {
    Spec {
        rows: def.rows_sel.clone(),
        cols: def.cols_sel.clone(),
        value: def.value.clone(),
        agg: def.agg.clone(),
        totals: def.totals,
        subtotals: def.subtotals,
        blank_rows: def.blank_rows,
        compact: def.compact,
        hide: def.hide.clone(),
        group_by: def.group_by.clone(),
        vfilter: def.vfilter.clone(),
        show_as: def.show_as.clone(),
        sort: def.sort.clone(),
        subtotal_label: "{} subtotal".into(),
        grand_label: "Grand totals".into(),
        agg_label: String::new(),
    }
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
pub(crate) fn to_frame(head: &[String], body: &[Vec<String>]) -> Result<DataFrame, String> {
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
    DataFrame::new(body.len(), cols).map_err(|e| format!("表として読めません: {e}"))
}

fn agg_expr(agg: &str, value: &str) -> Result<Expr, String> {
    let c = col(value);
    Ok(match agg {
        "sum" | "" => c.sum(),
        "count" => c.count(),
        "mean" | "avg" | "average" => c.mean(),
        "min" | "minimum" => c.min(),
        "max" | "maximum" => c.max(),
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
