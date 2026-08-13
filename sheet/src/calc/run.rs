//! **再計算の駆動。** 何を、どの順で、何回計算するか。
//!
//! 依存の解決・循環の検出・スピルの跡・Python の UDF。

use std::collections::{HashMap, HashSet};

use crate::model::{Cell, Pos, Sheet, Value};

use super::funcs::*;
use super::parse::*;

/// PY セルの呼び出しを解く: (関数名, 引数)。引数は式をいま評価した値
/// (範囲は列数つきの2次元)。**Python は動かさない** — 材料を出すだけ。
pub enum PyArg {
    One(Value),
    /// (列数, 行優先の値)
    Rect(u32, Vec<Value>),
}

/// plugins にある UDF の名前(ASCII は大文字にして入れる)。
/// **sheet はファイルを覗かない** — calc が起動時と plugins が変わったときに
/// 名前だけ渡し、こちらは式の見立て(=集計(A1) は UDF か)に使う。
pub(super) static UDF_NAMES: std::sync::RwLock<Option<std::collections::HashSet<String>>> =
    std::sync::RwLock::new(None);

/// plugins の UDF の名前を入れ替える(calc から呼ぶ)。
pub fn set_udf_names<I: IntoIterator<Item = String>>(names: I) {
    let set: HashSet<String> = names.into_iter().map(|n| n.to_ascii_uppercase()).collect();
    if let Ok(mut g) = UDF_NAMES.write() {
        *g = Some(set);
    }
}

/// その名前は plugins の UDF か。字句解析は ASCII を大文字にするので、
/// 渡す名前も大文字で持っている(日本語の名前はそのまま)。
pub fn is_udf_name(n: &str) -> bool {
    UDF_NAMES
        .read()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.contains(n)))
        .unwrap_or(false)
}

pub fn eval_py_call(sheet: &Sheet, formula: &str) -> Option<(String, Vec<PyArg>)> {
    if !is_py_formula(formula) {
        return None;
    }
    let expanded = expand_names(formula, &sheet.names);
    let toks = lex(&expanded).ok()?;
    // PY ( の中の引数を、通常の引数解析(範囲は形つき)で読む
    let resolved = HashMap::new();
    // PY セルの引数評価では ROW()/COLUMN() の「いまのセル」は分からない — 原点で代える
    let mut p = P { t: &toks, i: 0, sheet, resolved: &resolved, at: Pos::new(0, 0), others: &[], sheet_at: 0, skip_hidden: Default::default(), lets: Vec::new(), book_path: "", date1904: false };
    // 素直な書き方 `=集計(A1:B9)` と、古い書き方 `=PY("集計", A1:B9)` の両方
    let bare = match (p.next(), p.next()) {
        (Some(Tok::Name(n)), Some(Tok::LParen)) if n == "PY" => None,
        (Some(Tok::Name(n)), Some(Tok::LParen)) if is_udf_name(&n) => Some(n),
        _ => return None,
    };
    let args = p.args().ok()?;
    let mut it = args.into_iter();
    let name = match &bare {
        Some(n) => n.clone(),
        // 古い書き方は1つ目が関数名の文字でなければならない
        None => match it.next()? {
            Arg::One(Value::Text(t)) => t,
            _ => return None,
        },
    };
    let rest = it
        .map(|a| match a {
            Arg::One(v) => PyArg::One(v),
            Arg::Rect(c, vs) => PyArg::Rect(c, vs),
        })
        .collect();
    Some((name, rest))
}

// ---------- 再計算 ----------

/// 式が参照しているセルを集める(依存関係)。トレース(参照元の可視化)にも使う。
pub fn deps(formula: &str) -> Vec<Pos> {
    let mut out = Vec::new();
    if let Ok(toks) = lex(formula) {
        for t in toks {
            match t {
                Tok::Ref(p) => out.push(p),
                Tok::Range(a, z) => {
                    for r in a.row.min(z.row)..=a.row.max(z.row) {
                        for c in a.col.min(z.col)..=a.col.max(z.col) {
                            out.push(Pos::new(r, c));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// 式の中の「名前」を参照に置き換える(=単価*2 → =A1*2)。
/// 文字列の中は触らない。名前の前後が識別子の続きなら置き換えない。
/// 長い名前から先に試す(「単価」と「単価計」を取り違えない)。
pub(super) fn expand_names(f: &str, names: &[crate::model::DefinedName]) -> String {
    if names.is_empty() {
        return f.to_string();
    }
    let mut sorted: Vec<&crate::model::DefinedName> = names.iter().collect();
    sorted.sort_by_key(|d| std::cmp::Reverse(d.name.chars().count()));
    let ch: Vec<char> = f.chars().collect();
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::new();
    let mut i = 0;
    while i < ch.len() {
        if ch[i] == '"' {
            out.push('"');
            i += 1;
            while i < ch.len() {
                out.push(ch[i]);
                if ch[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        // 識別子の途中からは始めない
        let prev_ident = i > 0 && ident(ch[i - 1]);
        if !prev_ident {
            let mut hit = None;
            for d in &sorted {
                let nc: Vec<char> = d.name.chars().collect();
                if !nc.is_empty() && ch[i..].starts_with(&nc[..]) {
                    let after = ch.get(i + nc.len()).copied();
                    if !after.map(ident).unwrap_or(false) {
                        hit = Some((nc.len(), d.range.clone()));
                        break;
                    }
                }
            }
            if let Some((len, r)) = hit {
                out.push_str(&r);
                i += len;
                continue;
            }
        }
        out.push(ch[i]);
        i += 1;
    }
    out
}

/// シート全体を再計算する。循環参照は #CIRC! にする(黙って0にしない)。
/// この式は UDF(plugins の関数)のセルか。`=集計(A1:B9)` が**単独で**
/// 立っていること(古い書き方の `=PY("集計", …)` も同じ扱い)。
/// UDF は普通の再計算では計算しない — 別スレッドでまとめて回し、
/// 答えが揃ってから1手で書き戻す(画面を止めないため)。
/// 「集計(…)+1」のような複合式は UDF のセルではない。
pub fn is_py_formula(f: &str) -> bool {
    let Ok(toks) = lex(f) else { return false };
    let mut it = toks.iter();
    if !matches!(it.next(), Some(Tok::Name(n)) if n == "PY" || is_udf_name(n)) {
        return false;
    }
    if !matches!(it.next(), Some(Tok::LParen)) {
        return false;
    }
    // 括弧の釣り合いが最後のトークンでちょうど閉じること
    let mut depth = 1i32;
    for (i, t) in it.enumerate() {
        match t {
            Tok::LParen => depth += 1,
            Tok::RParen => {
                depth -= 1;
                if depth == 0 {
                    return i + 3 == toks.len(); // これが末尾でなければ複合式
                }
            }
            _ => {}
        }
    }
    false
}

/// 式を1本、`at` の位置に置いたつもりで解く(表には**書かない**)。
///
/// 条件付き書式の `expression` が使う — 表のどのセルにも無い式を、
/// 「そこにあったら何になるか」で確かめるための入口。
/// 引数なしの `ROW()`/`COLUMN()` は `at` を答える。
///
/// **他のシートは引けない**(`others` が空)。`別表!A1` は #REF! になる。
/// 相対参照のずらしは呼ぶ側の仕事(`model::offset_refs`)
pub fn eval_once(sheet: &Sheet, at: Pos, formula: &str) -> Value {
    let f = formula.trim();
    let f = expand_names(f.strip_prefix('=').unwrap_or(f), &sheet.names);
    // 途中結果の控えは無い(この式は表の依存の輪に入っていない)。
    // セルの値は表に入っている確定値をそのまま読む
    let resolved: HashMap<Pos, Value> = HashMap::new();
    let Ok(toks) = lex(&f) else { return Value::Error("#ERROR!".into()) };
    let mut p = P {
        t: &toks,
        i: 0,
        sheet,
        resolved: &resolved,
        at,
        others: &[],
        sheet_at: 0,
        skip_hidden: Default::default(),
        lets: Vec::new(),
        book_path: "",
        date1904: false,
    };
    match p.expr() {
        Ok(v) if p.i == toks.len() => v,
        _ => Value::Error("#ERROR!".into()),
    }
}

/// 並びを返す関数(スピルする関数)。セル単独でも、四則・比較・& と
/// 組み合わせた配列数式でも使える。答えが2次元なら隣へあふれる
pub(super) const ARRAY_FNS: &[&str] = &[
    "FILTER", "SORT", "UNIQUE", "SEQUENCE", "TRANSPOSE", "TEXTSPLIT",
    "SORTBY", "RANDARRAY", "VSTACK", "HSTACK", "TAKE", "DROP", "TOCOL", "TOROW",
];

pub fn recalc(sheet: &mut Sheet) {
    // 1枚だけの計算にはブックが無い = 径路も無く、起点は 1899 の既定
    recalc_impl(sheet, &[], 0, "", false);
}

/// ブックの1枚を、**他のシートを見ながら**再計算する
/// (INDIRECT("別の表!A1") はこの道でだけ解ける)。
pub fn recalc_book(book: &mut crate::Book, target: usize) {
    if target >= book.sheets.len() {
        return;
    }
    let iter = book.calc_iter;
    // シートを借り分ける前に写しておく(借用が重なるため)
    let path = book.path.clone();
    let d1904 = book.date1904;
    let (left, rest) = book.sheets.split_at_mut(target);
    let (tgt, right) = rest.split_first_mut().expect("上で確かめた");
    let others: Vec<&Sheet> = left.iter().chain(right.iter()).collect();
    match iter {
        Some((count, delta)) => {
            // 反復計算: 循環は前回の値で埋めて、変化が delta 以下に
            // 落ち着くまで(上限 count 回)回す — Excel と同じ枠組み
            for _ in 0..count.max(1) {
                let (changed, maxd) = recalc_pass_iter(tgt, &others, target, true, &path, d1904);
                if !changed || maxd <= delta {
                    break;
                }
            }
            stamp_py(tgt);
        }
        None => recalc_impl(tgt, &others, target, &path, d1904),
    }
}

/// 全シートの再計算。別のシートへの間接参照があるときは、
/// 参照の先が新しくなるようもう1周する
pub fn recalc_all(book: &mut crate::Book) {
    // 直書きの `Sheet2!A1` も INDIRECT と同じく別シートを見るので、
    // `!` を含む式があれば2周する(参照の先が新しくなってから写すため)
    let cross = book.sheets.iter().any(|s| {
        s.cells.values().any(|c| {
            c.formula
                .as_ref()
                .map(|f| f.to_ascii_uppercase().contains("INDIRECT") || f.contains('!'))
                .unwrap_or(false)
        })
    });
    for _ in 0..if cross { 2 } else { 1 } {
        for i in 0..book.sheets.len() {
            recalc_book(book, i);
        }
    }
}

/// UDF のセルの「関数名+引数」の指紋を取り直す。**関数は回さない** —
/// これを見て calc が「計算し直しが要る」を判断する(引数が変われば変わる)。
/// UDF のセルが無ければ 0 で、そのときの費用はセルの走査1回だけ。
pub(super) fn stamp_py(sheet: &mut Sheet) {
    use std::hash::{Hash, Hasher};
    let py_cells: Vec<Pos> = sheet
        .cells
        .iter()
        .filter_map(|(p, c)| c.formula.as_ref().filter(|f| is_py_formula(f)).map(|_| *p))
        .collect();
    if py_cells.is_empty() {
        sheet.py_stamp = 0;
        return;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in py_cells {
        let Some(f) = sheet.cells.get(&p).and_then(|c| c.formula.clone()) else { continue };
        p.hash(&mut h);
        match eval_py_call(sheet, &f) {
            Some((name, args)) => {
                name.hash(&mut h);
                for a in &args {
                    match a {
                        PyArg::One(v) => v.display().hash(&mut h),
                        PyArg::Rect(c, vs) => {
                            c.hash(&mut h);
                            for v in vs {
                                v.display().hash(&mut h);
                            }
                        }
                    }
                }
            }
            // 引数が解けない式も、式そのものが変われば指紋が変わる
            None => f.hash(&mut h),
        }
    }
    // 0 は「UDF のセルが無い」の意味に取ってあるので避ける
    sheet.py_stamp = h.finish() | 1;
}

pub(super) fn recalc_impl(sheet: &mut Sheet, others: &[&Sheet], at: usize, book_path: &str, date1904: bool) {
    // OFFSET/INDIRECT(計算で決まる参照)とスピルは、1回の走査では依存の順が
    // 読めないことがある — そのときだけ、値が動かなくなるまで回す(上限つき。
    // RAND/NOW 入りの式は毎回変わるので、比較からは外している)
    let dynamic = !sheet.spills.is_empty()
        || sheet.cells.values().any(|c| {
            c.formula
                .as_ref()
                .map(|f| {
                    let u = f.to_ascii_uppercase();
                    // 構造化参照(`[`)も deps では位置に解けない —
                    // 依存の順が読めないので、値が動かなくなるまで回す組に入れる
                    u.contains("OFFSET") || u.contains("INDIRECT") || u.contains('[')
                        || ARRAY_FNS.iter().any(|n| u.contains(n))
                })
                .unwrap_or(false)
        });
    if !dynamic {
        recalc_pass(sheet, others, at, book_path, date1904);
        stamp_py(sheet);
        return;
    }
    for _ in 0..5 {
        if !recalc_pass(sheet, others, at, book_path, date1904) {
            break;
        }
    }
    stamp_py(sheet);
}

/// 再計算の1周。値が動いたら true(まだ安定していないかもしれない)
pub(super) fn recalc_pass(sheet: &mut Sheet, others: &[&Sheet], at: usize, book_path: &str, date1904: bool) -> bool {
    recalc_pass_iter(sheet, others, at, false, book_path, date1904).0
}

/// 再計算の1周(反復モードつき)。反復モードでは循環参照を #CIRC! に
/// せず**前回の値**で埋める。返りは (動いたか, 数の最大変化量)
pub(super) fn recalc_pass_iter(
    sheet: &mut Sheet,
    others: &[&Sheet],
    at: usize,
    iter_mode: bool,
    book_path: &str,
    date1904: bool,
) -> (bool, f64) {
    // PY セルはここでは計算しない(最後に計算した値を保つ)。
    // まだ一度も計算していなければ「#PY?」の印を置く(空白で誤魔化さない)
    let py_cells: Vec<Pos> = sheet
        .cells
        .iter()
        .filter_map(|(p, c)| {
            c.formula.as_ref().filter(|f| is_py_formula(f)).map(|_| *p)
        })
        .collect();
    for p in &py_cells {
        if let Some(c) = sheet.cells.get_mut(p) {
            if c.value.is_empty() {
                c.value = Value::Error("#PY?".into());
            }
        }
    }
    // 式を集める。あふれる関数の入った式は「配列数式」として別扱い
    let mut formulas: Vec<(Pos, String)> = Vec::new();
    let mut arrays: Vec<(Pos, String)> = Vec::new();
    let mut cse_list: Vec<(Pos, String, (u32, u32))> = Vec::new();
    for (p, c) in &sheet.cells {
        let Some(f) = c.formula.as_ref().filter(|f| !is_py_formula(f)) else { continue };
        let f = expand_names(f, &sheet.names);
        // **昔ながらの配列数式(CSE)は、中身に関わらず配列として計算する。**
        // =SUM(A1:A3*B1:B3) は普通に計算すると #VALUE! か1組だけの合計に
        // なってしまう — 古い帳票が静かに違う値になる。
        // ただし**覆う範囲は人が決めた大きさで固定**なので、あふれる
        // スピルとは別の列に積む
        if let Some(size) = sheet.cse.get(p) {
            cse_list.push((*p, f, *size));
        } else if is_array_formula(&f) {
            arrays.push((*p, f));
        } else {
            formulas.push((*p, f));
        }
    }
    // RAND/NOW/TODAY 入りの式は毎回値が変わる — 安定の判定から外す
    let volatile: HashSet<Pos> = formulas
        .iter()
        .chain(arrays.iter())
        .filter(|(_, f)| {
            let u = f.to_ascii_uppercase();
            u.contains("RAND") || u.contains("NOW") || u.contains("TODAY")
        })
        .map(|(p, _)| *p)
        .collect();
    // 前回のスピルの影(起点以外)。**ここではまだ消さない** — 先に消すと
    // 通常の式が「消された直後」を読んで、値が縮んだまま安定してしまう。
    // 影の席は「置き直してよい席」として覚えるだけ。掃除は置き場所が
    // 決まったあと(この関数の後半)
    let mut freed: HashSet<Pos> = HashSet::new();
    for (o, (h, w)) in sheet.spills.iter() {
        for r in o.row..o.row + h {
            for c in o.col..o.col + w {
                let p = Pos::new(r, c);
                if p != *o {
                    freed.insert(p);
                }
            }
        }
    }
    let mut changed = false;

    let mut resolved: HashMap<Pos, Value> = HashMap::new();
    let mut visiting: HashSet<Pos> = HashSet::new();

    pub(super) fn eval_at(
        p: Pos,
        map: &HashMap<Pos, String>,
        sheet: &Sheet,
        others: &[&Sheet],
        at: usize,
        book_path: &str,
    date1904: bool,
        resolved: &mut HashMap<Pos, Value>,
        visiting: &mut HashSet<Pos>,
        iter_mode: bool,
    ) -> Value {
        if let Some(v) = resolved.get(&p) {
            return v.clone();
        }
        let Some(f) = map.get(&p) else {
            return sheet.value(p);
        };
        if !visiting.insert(p) {
            if iter_mode {
                // 反復計算: 循環は**前回の値**で埋める。初回(空や #CIRC! の
                // 残骸)は 0 から始める — Excel と同じ起点
                return match sheet.value(p) {
                    Value::Number(n) => Value::Number(n),
                    _ => Value::Number(0.0),
                };
            }
            return Value::Error("#CIRC!".into());
        }
        // 先に依存を解く
        for d in deps(f) {
            if map.contains_key(&d) && !resolved.contains_key(&d) {
                let v = eval_at(d, map, sheet, others, at, book_path, date1904, resolved, visiting, iter_mode);
                resolved.insert(d, v);
            }
        }
        let v = match lex(f) {
            Ok(toks) => {
                let mut p2 = P { t: &toks, i: 0, sheet, resolved, at: p, others, sheet_at: at, skip_hidden: Default::default(), lets: Vec::new(), book_path, date1904 };
                match p2.expr() {
                    Ok(v) if p2.i == toks.len() => v,
                    Ok(_) => Value::Error("#ERROR!".into()),
                    Err(_) => Value::Error("#ERROR!".into()),
                }
            }
            Err(_) => Value::Error("#ERROR!".into()),
        };
        visiting.remove(&p);
        resolved.insert(p, v.clone());
        v
    }

    let map: HashMap<Pos, String> = formulas.iter().cloned().collect();
    for (p, _) in &formulas {
        let v = eval_at(*p, &map, sheet, others, at, book_path, date1904, &mut resolved, &mut visiting, iter_mode);
        resolved.insert(*p, v);
    }
    let mut max_delta = 0.0f64;
    for (p, v) in resolved {
        if let Some(c) = sheet.cells.get_mut(&p) {
            if c.formula.is_some() {
                if c.value != v && !volatile.contains(&p) {
                    changed = true;
                    if let (Value::Number(a), Value::Number(b)) = (&c.value, &v) {
                        max_delta = max_delta.max((a - b).abs());
                    } else {
                        max_delta = f64::INFINITY; // 数でない変化は「まだ大きい」
                    }
                }
                c.value = v;
            }
        }
    }

    // 配列の式(スピル)。通常の式のあとに評価し、置き先をまず全部決めてから
    // (掃除 → 書き込み)の順で反映する
    let mut new_spills: std::collections::BTreeMap<Pos, (u32, u32)> = Default::default();
    let mut writes: Vec<(Pos, Value)> = Vec::new();
    let mut written: HashSet<Pos> = HashSet::new();
    for (origin, f) in &arrays {
        let put_origin = |sheet: &mut Sheet, v: Value, changed: &mut bool| {
            if let Some(c) = sheet.cells.get_mut(origin) {
                if c.value != v && !volatile.contains(origin) {
                    *changed = true;
                }
                c.value = v;
            }
        };
        let rows = match eval_array(sheet, others, at, f, *origin, book_path, date1904) {
            Err(e) => {
                put_origin(sheet, e, &mut changed);
                continue;
            }
            Ok(r) => r,
        };
        let h = rows.len() as u32;
        let w = rows.iter().map(|r| r.len()).max().unwrap_or(0) as u32;
        if h == 0 || w == 0 || h.saturating_mul(w) > 200_000 {
            put_origin(sheet, Value::Error("#NUM!".into()), &mut changed);
            continue;
        }
        // 1×1 の答えは普通の値として置く(=SUM(FILTER(…))+1 のような集計)
        if h == 1 && w == 1 {
            let v = rows[0].first().cloned().unwrap_or(Value::Empty);
            put_origin(sheet, v, &mut changed);
            continue;
        }
        // 席の検査: 既に中身のあるセルへは**あふれない**(黙って潰さない)。
        // 前回の自分たちの影(freed)は空席と見る。同じ周の別のスピルとも争わない
        let mut blocked = false;
        'seek: for r in 0..h {
            for c in 0..w {
                let p = Pos::new(origin.row + r, origin.col + c);
                if p == *origin {
                    continue;
                }
                if written.contains(&p) {
                    blocked = true;
                    break 'seek;
                }
                if let Some(cell) = sheet.cells.get(&p) {
                    if cell.formula.is_some()
                        || (!cell.value.is_empty() && !freed.contains(&p))
                    {
                        blocked = true;
                        break 'seek;
                    }
                }
            }
        }
        if blocked {
            put_origin(sheet, Value::Error("#SPILL!".into()), &mut changed);
            continue;
        }
        for (r, row) in rows.iter().enumerate() {
            for c in 0..w as usize {
                let p = Pos::new(origin.row + r as u32, origin.col + c as u32);
                let v = row.get(c).cloned().unwrap_or(Value::Empty);
                if p == *origin {
                    put_origin(sheet, v, &mut changed);
                } else {
                    written.insert(p);
                    writes.push((p, v));
                }
            }
        }
        new_spills.insert(*origin, (h, w));
    }

    // **昔ながらの配列数式(CSE)。** 覆う範囲は人が決めた大きさで固定
    // なので、あふれ先を探さない・#SPILL! にもしない。答えがその範囲より
    // 小さければ足りない席は #N/A(Excel と同じ)、大きければ切る。
    // 1つの値しか返らない式は範囲いっぱいに配る(これも Excel と同じ)
    for (origin, f, (h, w)) in &cse_list {
        let rows = match eval_array(sheet, others, at, f, *origin, book_path, date1904) {
            Err(e) => {
                if let Some(c) = sheet.cells.get_mut(origin) {
                    if c.value != e && !volatile.contains(origin) {
                        changed = true;
                    }
                    c.value = e;
                }
                continue;
            }
            Ok(r) => r,
        };
        let one = if rows.len() == 1 && rows[0].len() == 1 { rows[0].first().cloned() } else { None };
        for r in 0..*h {
            for c in 0..*w {
                let p = Pos::new(origin.row + r, origin.col + c);
                let v = match &one {
                    Some(v) => v.clone(),
                    None => rows
                        .get(r as usize)
                        .and_then(|row| row.get(c as usize))
                        .cloned()
                        .unwrap_or_else(|| Value::Error("#N/A".into())),
                };
                if p == *origin {
                    if let Some(cell) = sheet.cells.get_mut(origin) {
                        if cell.value != v && !volatile.contains(origin) {
                            changed = true;
                        }
                        cell.value = v;
                    }
                } else {
                    written.insert(p);
                    writes.push((p, v));
                }
            }
        }
    }
    // 掃除: 前回の影のうち、今回書かない席だけ空にする(書式は残す)
    for p in &freed {
        if written.contains(p) {
            continue;
        }
        if let Some(cell) = sheet.cells.get_mut(p) {
            if cell.formula.is_none() && !cell.value.is_empty() {
                changed = true;
                cell.value = Value::Empty;
            }
        }
        if sheet
            .cells
            .get(p)
            .map(|c| c.formula.is_none() && c.value.is_empty() && c.fmt == Default::default())
            .unwrap_or(false)
        {
            sheet.cells.remove(p);
        }
    }
    // 書き込み
    for (p, v) in writes {
        match sheet.cells.get_mut(&p) {
            Some(cell) => {
                if cell.value != v {
                    changed = true;
                }
                cell.value = v;
            }
            None => {
                if !v.is_empty() {
                    changed = true;
                }
                sheet.cells.insert(p, Cell { formula: None, value: v, fmt: Default::default() });
            }
        }
    }
    if sheet.spills != new_spills {
        changed = true;
        sheet.spills = new_spills;
    }
    (changed, max_delta)
}

/// 配列数式か — あふれる関数(FILTER 等)が式のどこかに入っているか。
/// 文字列の中の "FILTER" を拾わないよう、字句にしてから見る
pub(super) fn is_array_formula(f: &str) -> bool {
    lex(f)
        .map(|toks| {
            toks.iter()
                .any(|t| matches!(t, Tok::Name(n) if ARRAY_FNS.contains(&n.as_str())))
        })
        .unwrap_or(false)
}

/// 配列数式を評価して、行ごとの値にする。
/// =SEQUENCE(3)+1 のように演算子と組み合わせた式も、要素ごとに計算される
pub(super) fn eval_array(
    sheet: &Sheet,
    others: &[&Sheet],
    sheet_at: usize,
    f: &str,
    at: Pos,
    book_path: &str,
    date1904: bool,
) -> Result<Vec<Vec<Value>>, Value> {
    let err = |s: &str| Value::Error(s.into());
    let toks = lex(f).map_err(|_| err("#ERROR!"))?;
    let resolved = HashMap::new();
    let mut p = P { t: &toks, i: 0, sheet, resolved: &resolved, at, others, sheet_at, skip_hidden: Default::default(), lets: Vec::new(), book_path, date1904 };
    let v = {
        let mut ap = AP { p: &mut p };
        ap.expr().map_err(|_| err("#ERROR!"))?
    };
    if p.i != toks.len() {
        return Err(err("#ERROR!"));
    }
    Ok(match v {
        AVal::One(x) => vec![vec![x]],
        AVal::Arr(rows) => rows,
    })
}