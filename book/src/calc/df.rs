//! **`=df(...)` — 列の定義。** 数式を1つのセルではなく、表の列に属させます。
//!
//! 書き方は `=df(売上[金額] = 売上[単価] * 売上[数量], 税率 = 0.1, ...)` です。
//! 左辺が `表[列]` なら、その列の各行をその数式で埋めます。列が無ければ
//! 表の右端に足します。左辺が名前だけなら、その df の中で使える定数です。
//!
//! 定義はシートのどのセルに書いても構いません。再計算のたびに、まず
//! シート中の df を集め、依存の順に列を埋めてから、普通の数式を計算します
//! (`run.rs` の `recalc_pass_iter` の最初で呼びます)。
//!
//! セルに見せる物は、定義した列の名前です(`売上[金額], 売上[税額]`)。
//! 値は表の側に入ります。
//!
//! 手引きは docs/ja/df-manual.adoc です。今回は断る物が2つあります。
//! 右辺の集計(`SUM(売上[金額])` のような累計)と、別の表の列の引き当てです。

use std::collections::{HashMap, HashSet};

use crate::grid::Grid;
use crate::{Cell, Pos, Sheet, Value};

use super::parse::*;

/// 定義の左辺。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Lhs {
    /// `表[列]`
    Column { table: String, col: String },
    /// 名前だけ(その df の中の定数)
    Name(String),
}

impl Lhs {
    fn text(&self) -> String {
        match self {
            Lhs::Column { table, col } => format!("{table}[{col}]"),
            Lhs::Name(n) => n.clone(),
        }
    }
}

/// 定義1つ。
struct Def {
    /// この定義を書いたセル
    at: Pos,
    lhs: Lhs,
    /// 右辺の字句
    rhs: Vec<Tok>,
}

/// この式は `=df(...)` のセルか。`df(` が単独で立っていて、括弧が
/// 最後で閉じることを見ます(`=df(...)+1` のような複合式は df ではありません)。
pub fn is_df_formula(f: &str) -> bool {
    let Ok(toks) = lex(f) else { return false };
    if !matches!(toks.first(), Some(Tok::Name(n)) if n == "DF") {
        return false;
    }
    if !matches!(toks.get(1), Some(Tok::LParen)) {
        return false;
    }
    let mut depth = 0i32;
    for (i, t) in toks.iter().enumerate().skip(1) {
        match t {
            Tok::LParen => depth += 1,
            Tok::RParen => {
                depth -= 1;
                if depth == 0 {
                    return i + 1 == toks.len();
                }
            }
            _ => {}
        }
    }
    false
}

/// 右辺に書けない集計の関数。行ごとの値でなく列の全部を見る物です。
/// 「df の中の集計」はまだ決まっていないので、今回は断ります。
const AGGREGATES: &[&str] = &[
    "SUM", "AVERAGE", "COUNT", "COUNTA", "COUNTBLANK", "MIN", "MAX", "MEDIAN", "MODE",
    "PRODUCT", "SUMPRODUCT", "SUMIF", "SUMIFS", "COUNTIF", "COUNTIFS", "AVERAGEIF",
    "AVERAGEIFS", "MINIFS", "MAXIFS", "LARGE", "SMALL", "RANK", "RANK.EQ", "RANK.AVG",
    "STDEV", "STDEVP", "STDEV.S", "STDEV.P", "VAR", "VARP", "VAR.S", "VAR.P",
    "SUBTOTAL", "AGGREGATE", "PERCENTILE", "QUARTILE", "SUMSQ", "AVERAGEA", "MAXA", "MINA",
];

/// `=df(...)` の中身を定義に割る。字句の並びは `DF ( … )` で、
/// 定義は一番外の `,` で区切ります。
fn parse_defs(at: Pos, toks: &[Tok]) -> Result<Vec<Def>, String> {
    // `DF (` と最後の `)` を外す
    let inner = &toks[2..toks.len() - 1];
    let mut pieces: Vec<&[Tok]> = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, t) in inner.iter().enumerate() {
        match t {
            Tok::LParen | Tok::LBrace => depth += 1,
            Tok::RParen | Tok::RBrace => depth -= 1,
            Tok::Comma if depth == 0 => {
                pieces.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    pieces.push(&inner[start..]);
    let mut defs = Vec::new();
    for piece in pieces {
        let lhs = match piece.first() {
            Some(Tok::Table(Some(t), c, false)) => Lhs::Column { table: t.clone(), col: c.clone() },
            Some(Tok::Table(None, _, _)) => {
                return Err("df の左辺には表の名前が要ります(売上[金額] のように書きます)".into())
            }
            Some(Tok::Table(Some(_), _, true)) => {
                return Err("df の左辺に @ は書けません(売上[金額] のように書きます)".into())
            }
            Some(Tok::Name(n)) if n != "TRUE" && n != "FALSE" => Lhs::Name(n.clone()),
            _ => return Err("df の定義は「表[列] = 数式」か「名前 = 数式」の形で書きます".into()),
        };
        if !matches!(piece.get(1), Some(Tok::Cmp(op)) if op == "=") {
            return Err(format!("{} の後ろに = が要ります", lhs.text()));
        }
        let rhs = &piece[2..];
        if rhs.is_empty() {
            return Err(format!("{} の右辺が空です", lhs.text()));
        }
        defs.push(Def { at, lhs, rhs: rhs.to_vec() });
    }
    Ok(defs)
}

/// 右辺が呼んでいる関数の名前(`名前(` の形の物だけ)。
fn called_names(rhs: &[Tok]) -> Vec<&str> {
    rhs.windows(2)
        .filter_map(|w| match (&w[0], &w[1]) {
            (Tok::Name(n), Tok::LParen) => Some(n.as_str()),
            _ => None,
        })
        .collect()
}

/// 右辺に出てくる名前(関数の呼び出しでない物)。
fn used_names(rhs: &[Tok]) -> Vec<&str> {
    (0..rhs.len())
        .filter_map(|i| match &rhs[i] {
            Tok::Name(n) if !matches!(rhs.get(i + 1), Some(Tok::LParen)) => Some(n.as_str()),
            _ => None,
        })
        .collect()
}

/// シートの名前の定義(`単価` = `B2` のような物)を、右辺の字句に置き換える。
/// df の中で定義した名前は置き換えない(df の名前が先)。
fn expand_sheet_names(rhs: &mut [Tok], sheet: &Sheet, df_names: &HashSet<String>) {
    if sheet.names.is_empty() {
        return;
    }
    for i in 0..rhs.len() {
        let Tok::Name(n) = &rhs[i] else { continue };
        if matches!(rhs.get(i + 1), Some(Tok::LParen)) || df_names.contains(n) {
            continue;
        }
        let Some(d) = sheet.names.iter().find(|d| d.name.to_ascii_uppercase() == *n) else { continue };
        let tok = match d.range.split_once(':') {
            Some((a, z)) => Pos::parse(a).zip(Pos::parse(z)).map(|(a, z)| Tok::Range(a, z)),
            None => Pos::parse(&d.range).map(Tok::Ref),
        };
        if let Some(t) = tok {
            rhs[i] = t;
        }
    }
}

/// 表の見出しの列を引く(見出しの字で)。
fn header_col(sheet: &Sheet, t: &crate::TableDef, col: &str) -> Option<u32> {
    (t.a.col..=t.b.col).find(|c| sheet.value(Pos::new(t.a.row, *c)).display() == col)
}

/// シートの df を全部集めて、依存の順に列を埋める。
/// 返りは「値が動いたか」。df のセルそのものの値もここで置きます。
pub(super) fn apply(
    sheet: &mut Sheet,
    others: &[&dyn Grid],
    sheet_at: usize,
    book_path: &str,
    date1904: bool,
) -> bool {
    // df のセルを集める(位置の順で — 文言に出す場所が安定するように)
    let mut cells: Vec<(Pos, String)> = sheet
        .cells
        .iter()
        .filter_map(|(p, c)| c.formula.as_ref().filter(|f| is_df_formula(f)).map(|f| (*p, f.clone())))
        .collect();
    cells.sort_by_key(|(p, _)| (p.row, p.col));
    if cells.is_empty() {
        return false;
    }

    // セルごとのエラー(最初の1つをセルに見せる)
    let mut errors: HashMap<Pos, String> = HashMap::new();
    let note = |errors: &mut HashMap<Pos, String>, at: Pos, msg: String| {
        errors.entry(at).or_insert(msg);
    };

    // 1. 読む
    let mut defs: Vec<Def> = Vec::new();
    for (p, f) in &cells {
        let toks = match lex(f) {
            Ok(t) => t,
            Err(e) => {
                note(&mut errors, *p, e);
                continue;
            }
        };
        match parse_defs(*p, &toks) {
            Ok(ds) => defs.extend(ds),
            Err(e) => note(&mut errors, *p, e),
        }
    }

    // 2. 二重の定義。列はシート全体で1つ、名前は同じ df の中で1つ
    let mut seen: HashMap<(Option<Pos>, Lhs), Pos> = HashMap::new();
    let mut dead: HashSet<usize> = HashSet::new();
    for (i, d) in defs.iter().enumerate() {
        let scope = match &d.lhs {
            Lhs::Column { .. } => None,
            Lhs::Name(_) => Some(d.at),
        };
        match seen.get(&(scope, d.lhs.clone())) {
            Some(first) => {
                let msg = format!("{} を2回定義しています({} と {})", d.lhs.text(), first.a1(), d.at.a1());
                note(&mut errors, *first, msg.clone());
                note(&mut errors, d.at, msg);
                dead.insert(i);
                // 先の方も使わない
                if let Some(j) = defs.iter().position(|e| e.at == *first && e.lhs == d.lhs) {
                    dead.insert(j);
                }
            }
            None => {
                seen.insert((scope, d.lhs.clone()), d.at);
            }
        }
    }

    // 3. 右辺を確かめる(表と列があるか、断る物を書いていないか)
    let defined_cols: HashSet<(String, String)> = defs
        .iter()
        .filter_map(|d| match &d.lhs {
            Lhs::Column { table, col } => Some((table.clone(), col.clone())),
            Lhs::Name(_) => None,
        })
        .collect();
    let df_names: HashMap<Pos, HashSet<String>> = defs.iter().fold(HashMap::new(), |mut m, d| {
        if let Lhs::Name(n) = &d.lhs {
            m.entry(d.at).or_default().insert(n.clone());
        }
        m
    });
    let empty = HashSet::new();
    for (i, d) in defs.iter_mut().enumerate() {
        if dead.contains(&i) {
            continue;
        }
        expand_sheet_names(&mut d.rhs, sheet, df_names.get(&d.at).unwrap_or(&empty));
        let mut fail = |msg: String| {
            note(&mut errors, d.at, msg);
            dead.insert(i);
        };
        if let Some(agg) = called_names(&d.rhs).iter().find(|n| AGGREGATES.contains(n)) {
            fail(format!("df の中の集計はまだ使えません({} の {})", d.lhs.text(), agg));
            continue;
        }
        match &d.lhs {
            Lhs::Column { table, col: _ } => {
                let Some(t) = sheet.tables.iter().find(|t| t.name == *table) else {
                    fail(format!("表 {table} が見つかりません"));
                    continue;
                };
                if !t.header {
                    fail(format!("表 {table} に見出しの行が無いので、列を名前で引けません"));
                    continue;
                }
                let mut bad = None;
                for tok in &d.rhs {
                    let Tok::Table(tb, c, _) = tok else { continue };
                    let tb = tb.as_deref().unwrap_or(table);
                    if tb != table {
                        bad = Some(format!("別の表の列はまだ引けません({}[{}])", tb, c));
                        break;
                    }
                    if header_col(sheet, t, c).is_none() && !defined_cols.contains(&(tb.to_string(), c.clone())) {
                        bad = Some(format!("{}[{}] が見つかりません", tb, c));
                        break;
                    }
                }
                if let Some(msg) = bad {
                    fail(msg);
                }
            }
            Lhs::Name(n) => {
                if d.rhs.iter().any(|t| matches!(t, Tok::Table(..))) {
                    fail(format!("名前 {n} の右辺には列を書けません(列の定義は 表[列] = 数式 と書きます)"));
                }
            }
        }
    }

    // 4. 依存の順。列は (表, 列) で、名前は (セル, 名前) で引く
    let key_of = |d: &Def| -> (Option<Pos>, Lhs) {
        match &d.lhs {
            Lhs::Column { .. } => (None, d.lhs.clone()),
            Lhs::Name(_) => (Some(d.at), d.lhs.clone()),
        }
    };
    // エラーになった定義も引けるようにしておく(それを使う定義を
    // 5 で見つけて断るため)
    let index: HashMap<(Option<Pos>, Lhs), usize> =
        defs.iter().enumerate().map(|(i, d)| (key_of(d), i)).collect();
    let deps_of = |d: &Def| -> Vec<usize> {
        let mut out = Vec::new();
        let own_table = match &d.lhs {
            Lhs::Column { table, .. } => Some(table.as_str()),
            Lhs::Name(_) => None,
        };
        for tok in &d.rhs {
            if let Tok::Table(tb, c, _) = tok {
                let tb = tb.as_deref().or(own_table).unwrap_or_default();
                let k = (None, Lhs::Column { table: tb.to_string(), col: c.clone() });
                if let Some(j) = index.get(&k) {
                    out.push(*j);
                }
            }
        }
        for n in used_names(&d.rhs) {
            if let Some(j) = index.get(&(Some(d.at), Lhs::Name(n.to_string()))) {
                out.push(*j);
            }
        }
        out
    };
    // 深さ優先で並べる。0 = 未訪問、1 = 訪問中(ここに戻れば循環)、2 = 済み
    let mut state = vec![0u8; defs.len()];
    let mut order: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    // 引数はまとめない(何を渡したかを呼ぶ側から見えるようにする)
    #[allow(clippy::too_many_arguments)]
    fn visit(
        i: usize,
        defs: &[Def],
        deps_of: &dyn Fn(&Def) -> Vec<usize>,
        state: &mut [u8],
        order: &mut Vec<usize>,
        stack: &mut Vec<usize>,
        dead: &mut HashSet<usize>,
        errors: &mut HashMap<Pos, String>,
    ) {
        if state[i] == 2 || dead.contains(&i) {
            return;
        }
        if state[i] == 1 {
            // 循環。stack の i から後ろが循環の並び
            let from = stack.iter().position(|s| *s == i).unwrap_or(0);
            let ring: Vec<usize> = stack[from..].to_vec();
            let mut names: Vec<String> = ring.iter().map(|j| defs[*j].lhs.text()).collect();
            names.push(defs[i].lhs.text());
            let msg = format!("定義が循環しています({})", names.join(" → "));
            for j in ring {
                errors.entry(defs[j].at).or_insert(msg.clone());
                dead.insert(j);
            }
            return;
        }
        state[i] = 1;
        stack.push(i);
        for j in deps_of(&defs[i]) {
            visit(j, defs, deps_of, state, order, stack, dead, errors);
        }
        stack.pop();
        state[i] = 2;
        if !dead.contains(&i) {
            order.push(i);
        }
    }
    for i in 0..defs.len() {
        visit(i, &defs, &deps_of, &mut state, &mut order, &mut stack, &mut dead, &mut errors);
    }

    // 5. 順に埋める。エラーになった定義を使う定義も使わない
    let mut changed = false;
    let mut names: HashMap<Pos, Vec<(String, Value)>> = HashMap::new();
    for i in order {
        let d = &defs[i];
        if dead.contains(&i) {
            continue;
        }
        if let Some(j) = deps_of(d).into_iter().find(|j| dead.contains(j)) {
            note(&mut errors, d.at, format!("{} は、エラーになった {} を使っています", d.lhs.text(), defs[j].lhs.text()));
            dead.insert(i);
            continue;
        }
        match &d.lhs {
            Lhs::Name(n) => {
                let lets = names.get(&d.at).cloned().unwrap_or_default();
                let v = eval_row(sheet, others, sheet_at, book_path, date1904, &d.rhs, d.at, lets, None);
                names.entry(d.at).or_default().push((n.clone(), v));
            }
            Lhs::Column { table, col } => {
                let ti = sheet.tables.iter().position(|t| t.name == *table).expect("3 で確かめた");
                let t = sheet.tables[ti].clone();
                // 列が無ければ右端に足す。足す先が空いていなければ断る
                let c = match header_col(sheet, &t, col) {
                    Some(c) => c,
                    None => {
                        let c = t.b.col + 1;
                        let taken = (t.a.row..=t.b.row).any(|r| {
                            sheet.cells.get(&Pos::new(r, c)).map(|x| x.formula.is_some() || !x.value.is_empty()).unwrap_or(false)
                        });
                        if taken {
                            // 列の名前(A・B・…)は番地から数字を外して作る
                            let letters: String = Pos::new(0, c).a1().chars().filter(|ch| ch.is_ascii_alphabetic()).collect();
                            note(&mut errors, d.at, format!("{} を足す場所({letters} 列)が空いていません", d.lhs.text()));
                            dead.insert(i);
                            continue;
                        }
                        let head = Pos::new(t.a.row, c);
                        let fmt = sheet.cells.get(&head).map(|x| x.fmt.clone()).unwrap_or_default();
                        sheet.set(head, Cell { formula: None, value: Value::Text(col.clone()), fmt });
                        sheet.tables[ti].b.col = c;
                        changed = true;
                        c
                    }
                };
                let r0 = t.a.row + 1;
                let r1 = if t.totals { t.b.row.saturating_sub(1) } else { t.b.row };
                let lets = names.get(&d.at).cloned().unwrap_or_default();
                let mut out: Vec<(Pos, Value)> = Vec::new();
                for r in r0..=r1 {
                    let at = Pos::new(r, c);
                    let v = eval_row(sheet, others, sheet_at, book_path, date1904, &d.rhs, at, lets.clone(), Some(table));
                    out.push((at, v));
                }
                for (p, v) in out {
                    let old = sheet.cells.get(&p);
                    if old.map(|x| x.formula.is_none() && x.value == v).unwrap_or(v.is_empty()) {
                        continue;
                    }
                    let fmt = old.map(|x| x.fmt.clone()).unwrap_or_default();
                    sheet.set(p, Cell { formula: None, value: v, fmt });
                    changed = true;
                }
            }
        }
    }

    // 6. df のセルに見せる物。エラーがあればその文言、無ければ定義した列の名前
    for (p, _) in &cells {
        let v = match errors.get(p) {
            Some(e) => Value::Error(e.clone()),
            None => {
                let cols: Vec<String> = defs
                    .iter()
                    .filter(|d| d.at == *p && matches!(d.lhs, Lhs::Column { .. }))
                    .map(|d| d.lhs.text())
                    .collect();
                let shown = if cols.is_empty() {
                    defs.iter().filter(|d| d.at == *p).map(|d| d.lhs.text()).collect::<Vec<_>>()
                } else {
                    cols
                };
                Value::Text(shown.join(", "))
            }
        };
        if let Some(c) = sheet.cells.get_mut(p) {
            if c.value != v {
                c.value = v;
                changed = true;
            }
        }
    }
    changed
}

/// 右辺を1行ぶん計算する。`own_table` があれば、その表の列の参照は
/// 「いまの行の同じ列」として読む(`売上[単価]` が行ごとの値になる)。
#[allow(clippy::too_many_arguments)]
fn eval_row(
    sheet: &Sheet,
    others: &[&dyn Grid],
    sheet_at: usize,
    book_path: &str,
    date1904: bool,
    rhs: &[Tok],
    at: Pos,
    lets: Vec<(String, Value)>,
    own_table: Option<&str>,
) -> Value {
    let toks: Vec<Tok> = rhs
        .iter()
        .map(|t| match (t, own_table) {
            (Tok::Table(_, c, _), Some(tb)) => Tok::Table(Some(tb.to_string()), c.clone(), true),
            _ => t.clone(),
        })
        .collect();
    let resolved = HashMap::new();
    let mut p = P {
        t: &toks,
        i: 0,
        sheet,
        resolved: &resolved,
        at,
        others,
        sheet_at,
        skip_hidden: Default::default(),
        lets,
        book_path,
        date1904,
    };
    match p.expr() {
        Ok(v) if p.i == toks.len() => v,
        _ => Value::Error("#ERROR!".into()),
    }
}
