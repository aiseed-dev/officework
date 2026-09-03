//! **式を読む。** 字句に割り、構文の木を辿りながらその場で畳む。
//!
//! 参照(`A1` `Sheet2!B3` `表[列]`)の解決もここ。

use std::collections::HashMap;

use crate::grid::Grid;
use crate::{Pos, Value};

use super::funcs::*;

// ---------- 字句 ----------

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Tok {
    Num(f64),
    Str(String),
    Ref(Pos),
    Range(Pos, Pos),
    /// 別のシートの参照(シート名, 始点, 終点)。1セルなら始点=終点。
    /// 値は**その時の値を写す**(位置では持ち帰れない — RefAns::Rect と同じ理屈)
    Sheet(String, Pos, Pos),
    /// 串刺し集計 `Sheet1:Sheet3!A1`(始めのシート, 終わりのシート, 始点, 終点)。
    /// **ブックの並び順**で2枚の間にある全シートの同じ場所を集める
    Sheet3(String, String, Pos, Pos),
    /// 構造化参照(表の名前 — `[@列]` のときは None, 列の名前, この行だけか)。
    /// `Table1[金額]` = その列のデータ本体、`[@金額]` = いまの行の同じ列
    Table(Option<String>, String, bool),
    Name(String),
    Op(char),
    Cmp(String),
    LParen,
    RParen,
    Comma,
    /// 配列定数 `{1,2;3,4}` の括りと行の区切り
    LBrace,
    RBrace,
    Semi,
}

/// 構造化参照 `Table1[金額]` / `Table1[@金額]` / `[@金額]` / `[金額]` を読む。
/// 読めたら (表の名前, 列の名前, この行だけか, 次の位置)。
/// `[[#見出し],[列]]` のような入れ子の形は**受けない**(None を返して
/// 式のエラーにする — 黙って違う範囲を読むより正直)
#[allow(clippy::type_complexity)]
pub(super) fn lex_table_ref(b: &[char], i: usize) -> Option<(Option<String>, String, bool, usize)> {
    let (tbl, open) = if b.get(i) == Some(&'[') {
        (None, i) // 表の名前を省いた形 — いまのセルが入っている表
    } else {
        let mut j = i;
        let mut s = String::new();
        while j < b.len() && (b[j].is_alphanumeric() || b[j] == '_' || b[j] == '.') {
            s.push(b[j]);
            j += 1;
        }
        if s.is_empty() || b.get(j) != Some(&'[') {
            return None;
        }
        (Some(s), j)
    };
    // 中身は `]` まで。入れ子の `[` があれば受けない
    let mut k = open + 1;
    let mut inner = String::new();
    while k < b.len() && b[k] != ']' {
        if b[k] == '[' {
            return None;
        }
        inner.push(b[k]);
        k += 1;
    }
    if k >= b.len() {
        return None; // 閉じていない
    }
    let this_row = inner.starts_with('@');
    let col = inner.trim_start_matches('@').trim().to_string();
    // `#見出し` などの特別な名前は受けない(上と同じ理由)
    if col.is_empty() || col.starts_with('#') {
        return None;
    }
    Some((tbl, col, this_row, k + 1))
}

/// `Sheet2!A1` / `売上!A1:B3` / `'4月 実績'!B2` を読む。
/// 読めたら (シート名, 始点, 終点, 次の位置)。**後ろに `!` があるときだけ**
/// 当たるので、既存の字句の読み方は変わらない(当たらなければ None で素通し)。
/// 和文のシート名(売上・4月)も通すため、名前の走査は Unicode の英数字で見る
#[allow(clippy::type_complexity)]
pub(super) fn lex_sheet_ref(
    b: &[char],
    i: usize,
) -> Option<(String, Option<String>, Pos, Pos, usize)> {
    let (name, after) = if b.get(i) == Some(&'\'') {
        // 引用符つき = 空白や記号を含む名前('4月 実績'!B2)
        let mut j = i + 1;
        let mut s = String::new();
        while j < b.len() && b[j] != '\'' {
            s.push(b[j]);
            j += 1;
        }
        if j >= b.len() {
            return None; // 閉じていない — 既存の枝に任せる
        }
        (s, j + 1)
    } else {
        let mut j = i;
        let mut s = String::new();
        while j < b.len() && (b[j].is_alphanumeric() || b[j] == '_' || b[j] == '.') {
            s.push(b[j]);
            j += 1;
        }
        (s, j)
    };
    if name.is_empty() {
        return None;
    }
    // `Sheet1:Sheet3!A1` — 2枚目の名前(串刺し集計)
    let (name2, after) = if b.get(after) == Some(&':') {
        let mut j = after + 1;
        let mut s2 = String::new();
        if b.get(j) == Some(&'\'') {
            j += 1;
            while j < b.len() && b[j] != '\'' {
                s2.push(b[j]);
                j += 1;
            }
            if j >= b.len() {
                return None;
            }
            j += 1;
        } else {
            while j < b.len() && (b[j].is_alphanumeric() || b[j] == '_' || b[j] == '.') {
                s2.push(b[j]);
                j += 1;
            }
        }
        if s2.is_empty() {
            return None;
        }
        (Some(s2), j)
    } else {
        (None, after)
    };
    if b.get(after) != Some(&'!') {
        return None;
    }
    // `!` の後ろの A1 か A1:B3
    let one = |from: usize| -> (Option<Pos>, usize) {
        let mut j = from;
        while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == '$') {
            j += 1;
        }
        (Pos::parse(&b[from..j].iter().collect::<String>()), j)
    };
    let (a, j) = one(after + 1);
    let a = a?;
    if b.get(j) == Some(&':') {
        let (z, k) = one(j + 1);
        if let Some(z) = z {
            return Some((name, name2, a, z, k));
        }
    }
    Some((name, name2, a, a, j))
}

pub(super) fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '"' {
            let mut s = String::new();
            i += 1;
            while i < b.len() && b[i] != '"' {
                s.push(b[i]);
                i += 1;
            }
            if i >= b.len() {
                return Err("文字列が閉じていません".into());
            }
            i += 1;
            out.push(Tok::Str(s));
            continue;
        }
        // 別シートの参照は**数の枝より先**に見る(`4月!B2` のように
        // 数字で始まる名前があるため)。`!` が無ければ素通しする
        if let Some((name, name2, a, z, k)) = lex_sheet_ref(&b, i) {
            out.push(match name2 {
                Some(n2) => Tok::Sheet3(name, n2, a, z),
                None => Tok::Sheet(name, a, z),
            });
            i = k;
            continue;
        }
        // 構造化参照(表の列)。`[` が要るので、これも当たらなければ素通し
        if let Some((tbl, col, this_row, k)) = lex_table_ref(&b, i) {
            out.push(Tok::Table(tbl, col, this_row));
            i = k;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            let st = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == '.') {
                i += 1;
            }
            // **指数の書き方**(`1E15`・`1.5E-14`)。Excel も LibreOffice も
            // 数として読みます。数の直後にセル参照は来られないので、
            // `1E15` を「1 と セル E15」と取り違えることはありません。
            // 後ろに数字が続くときだけ取ります(`1E` は数ではありません)
            if i < b.len() && (b[i] == 'e' || b[i] == 'E') {
                let mut j = i + 1;
                if j < b.len() && (b[j] == '+' || b[j] == '-') {
                    j += 1;
                }
                if j < b.len() && b[j].is_ascii_digit() {
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let s: String = b[st..i].iter().collect();
            out.push(Tok::Num(s.parse().map_err(|_| format!("数値として読めません: {s}"))?));
            continue;
        }
        // 名前の頭は **ASCII に限らない** — plugins の関数は日本語で名づける
        // (`=集計(A1:B9)`)。セル参照は ASCII なので取り違えは起きない
        if c.is_alphabetic() || c == '$' || c == '_' {
            let st = i;
            while i < b.len() && (b[i].is_alphanumeric() || b[i] == '$' || b[i] == '_' || b[i] == '.') {
                i += 1;
            }
            let word: String = b[st..i].iter().collect();
            // A1:B3 の範囲
            if i < b.len() && b[i] == ':' {
                let st2 = i + 1;
                let mut j = st2;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == '$') {
                    j += 1;
                }
                let word2: String = b[st2..j].iter().collect();
                if let (Some(a), Some(z)) = (Pos::parse(&word), Pos::parse(&word2)) {
                    out.push(Tok::Range(a, z));
                    i = j;
                    continue;
                }
            }
            // ATAN2 や LOG10 は「ATAN 列の 2 行目」とも読めてしまう —
            // **直後が ( なら関数名**(セル参照を関数のようには呼べない)
            let called = b.get(i).copied() == Some('(');
            match Pos::parse(&word) {
                Some(p) if !called => out.push(Tok::Ref(p)),
                _ => out.push(Tok::Name(word.to_ascii_uppercase())),
            }
            continue;
        }
        // 比較演算子
        if "<>=".contains(c) {
            let two: String = b[i..(i + 2).min(b.len())].iter().collect();
            if ["<=", ">=", "<>"].contains(&two.as_str()) {
                out.push(Tok::Cmp(two));
                i += 2;
                continue;
            }
            out.push(Tok::Cmp(c.to_string()));
            i += 1;
            continue;
        }
        match c {
            '+' | '-' | '*' | '/' | '^' | '&' => out.push(Tok::Op(c)),
            '(' => out.push(Tok::LParen),
            ')' => out.push(Tok::RParen),
            ',' => out.push(Tok::Comma),
            '{' => out.push(Tok::LBrace),
            '}' => out.push(Tok::RBrace),
            ';' => out.push(Tok::Semi),
            _ => return Err(format!("読めない文字: {c}")),
        }
        i += 1;
    }
    Ok(out)
}

// ---------- 構文と評価(再帰下降) ----------

pub(super) struct P<'a> {
    pub(super) t: &'a [Tok],
    pub(super) i: usize,
    pub(super) sheet: &'a dyn Grid,
    pub(super) resolved: &'a HashMap<Pos, Value>,
    /// いま計算しているセル。ROW()/COLUMN()(引数なし)が使う
    pub(super) at: Pos,
    /// 同じブックの他のシート(読み取りだけ)。INDIRECT("別の表!A1") が引く。
    /// recalc(1枚だけ)では空 — そのときの別シート参照は #REF!
    pub(super) others: &'a [&'a dyn Grid],
    /// 自分がブックの何枚目か。**others は「自分より前」+「自分より後」の
    /// 並びなので、これが無いとブックの並び順を復元できない**(串刺し集計が使う)
    pub(super) sheet_at: usize,
    /// 範囲を読むとき、隠れた行を飛ばす。0 = 飛ばさない、
    /// 1 = 絞り込みで隠れた行(filter_hidden)を飛ばす、
    /// 2 = 手で隠した行(row_hidden)も飛ばす。
    /// SUBTOTAL/AGGREGATE の引数を読む間だけ立つ(Excel の約束)
    pub(super) skip_hidden: std::cell::Cell<u8>,
    /// LET が束ねた名前(大文字で持つ)。**後ろが勝ち**= 入れ子や
    /// 同じ名前の付け直しで内側が外側を隠す
    pub(super) lets: Vec<(String, Value)>,
    /// ブックの出どころ(絶対の径路)。`CELL("filename")` だけが使う。
    /// **空 = まだ保存していない**(Excel も空文字を返す)
    pub(super) book_path: &'a str,
    /// 1904 起点のブックか(日付の関数と表示の境目が使う)
    pub(super) date1904: bool,
}

/// 参照を計算する関数(OFFSET/INDIRECT)の答え。
/// 別のシートの中身は「位置」では持ち帰れないので、値の形も持つ
pub(super) enum RefAns {
    /// このシートの範囲
    At(Pos, Pos),
    /// 別のシートから写した値(列数, 行優先)
    Rect(u32, Vec<Value>),
    /// 参照として成り立たない(エラー値をそのまま出す)
    Bad(Value),
}

/// **四則のための数。** `as_number` と違い、**文字を 0 にしない。**
///
/// 表計算には数の取り方が2つある。混ぜると黙って違う答えが出る:
///
/// - **集計**(`SUM`・`AVERAGE`)は文字を**飛ばす**。`nums()` の担当
/// - **四則**(`+ - * / ^`)は文字が混じったら **`#VALUE!`**。ここの担当
///
/// `="あ"+1` が `1` になっていた(2026-08-10 に ironcalc と突き合わせて判明)。
/// 文字の混じった列の合計が**それらしい数**として出るので、台帳でいちばん困る。
///
/// 数字だけの文字列(`"5"`)は読む — Excel も `="5"+1` は 6 にする。
/// 真偽は 1/0(`=TRUE+1` は 2)。空欄は 0。
pub(super) fn arith(v: &Value) -> Result<f64, Value> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Bool(b) => Ok(*b as i32 as f64),
        Value::Empty => Ok(0.0),
        Value::Text(t) => t.trim().parse().map_err(|_| Value::Error("#VALUE!".into())),
        Value::Error(e) => Err(Value::Error(e.clone())),
    }
}

/// 2つの値を四則の数にする。**どちらかが文字なら `#VALUE!`。**
pub(super) fn arith2(a: &Value, b: &Value) -> Result<(f64, f64), Value> {
    Ok((arith(a)?, arith(b)?))
}

impl<'a> P<'a> {
    pub(super) fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    pub(super) fn next(&mut self) -> Option<Tok> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }

    pub(super) fn cell(&self, p: Pos) -> Value {
        self.resolved.get(&p).cloned().unwrap_or_else(|| self.sheet.value(p))
    }

    pub(super) fn range_values(&self, a: Pos, z: Pos) -> Vec<Value> {
        let (r0, r1) = (a.row.min(z.row), a.row.max(z.row));
        let (c0, c1) = (a.col.min(z.col), a.col.max(z.col));
        let skip = self.skip_hidden.get();
        let mut v = Vec::new();
        for r in r0..=r1 {
            // SUBTOTAL/AGGREGATE のときだけ、隠れた行は数に入れない
            if (skip >= 1 && self.sheet.row_filtered(r))
                || (skip >= 2 && self.sheet.row_hidden(r))
            {
                continue;
            }
            for c in c0..=c1 {
                v.push(self.cell(Pos::new(r, c)));
            }
        }
        v
    }

    // 比較 < 加減 < 乗除 < 冪 < 単項 < 原子
    pub(super) fn expr(&mut self) -> Result<Value, String> {
        let lhs = self.add()?;
        if let Some(Tok::Cmp(op)) = self.peek().cloned() {
            self.next();
            let rhs = self.add()?;
            let r = match (&lhs, &rhs) {
                (Value::Text(a), Value::Text(b)) => match op.as_str() {
                    "=" => a == b,
                    "<>" => a != b,
                    "<" => a < b,
                    ">" => a > b,
                    "<=" => a <= b,
                    ">=" => a >= b,
                    _ => return Err(format!("比較演算子が不正: {op}")),
                },
                // **数は1本の基準で比べる**(2026-08-22)。前はここで
                // `=` と `<>` だけを甘くしていたので、`=0.1+0.2=0.3` と
                // `=(0.1+0.2)>0.3` が同時に真になっていました
                _ => {
                    if !matches!(op.as_str(), "=" | "<>" | "<" | ">" | "<=" | ">=") {
                        return Err(format!("比較演算子が不正: {op}"));
                    }
                    ord_holds(op.as_str(), cmp_num(lhs.as_number(), rhs.as_number()))
                }
            };
            return Ok(Value::Bool(r));
        }
        Ok(lhs)
    }

    pub(super) fn add(&mut self) -> Result<Value, String> {
        let mut v = self.mul()?;
        while let Some(Tok::Op(o @ ('+' | '-' | '&'))) = self.peek().cloned() {
            self.next();
            let r = self.mul()?;
            // エラーは伝播する(表計算の作法)。ここで消すと循環参照が0になって隠れる
            if let Value::Error(_) = v { continue }
            if let Value::Error(_) = r { v = r; continue }
            v = match o {
                // & は文字列連結なので数にしない(表計算の作法)
                '&' => Value::Text(format!("{}{}", v.display(), r.display())),
                // **打ち消し合ったら 0 にします**(LibreOffice と同じ)。
                // 式の評価はここと funcs.rs の2本あるので、両方に入れます
                _ => match arith2(&v, &r) {
                    Err(e) => e,
                    Ok((x, y)) => Value::Number(if o == '+' {
                        super::funcs::chikai_tashizan(x, y)
                    } else {
                        super::funcs::chikai_hikizan(x, y)
                    }),
                },
            };
        }
        Ok(v)
    }


    pub(super) fn mul(&mut self) -> Result<Value, String> {
        let mut v = self.pow()?;
        while let Some(Tok::Op(o @ ('*' | '/'))) = self.peek().cloned() {
            self.next();
            let r = self.pow()?;
            if let Value::Error(_) = v { continue }
            if let Value::Error(_) = r { v = r; continue }
            // **型の誤りが零除算より先に立つ。** `="あ"/0` は Excel でも
            // #VALUE!(#DIV/0! ではない)
            let (x, y) = match arith2(&v, &r) {
                Err(e) => {
                    v = e;
                    continue;
                }
                Ok(p) => p,
            };
            if o == '/' && y == 0.0 {
                return Ok(Value::Error("#DIV/0!".into()));
            }
            v = Value::Number(if o == '*' { x * y } else { x / y });
        }
        Ok(v)
    }

    pub(super) fn pow(&mut self) -> Result<Value, String> {
        let v = self.unary()?;
        if let Some(Tok::Op('^')) = self.peek() {
            self.next();
            let r = self.pow()?;
            return Ok(match arith2(&v, &r) {
                Err(e) => e,
                Ok((x, y)) => Value::Number(x.powf(y)),
            });
        }
        Ok(v)
    }

    pub(super) fn unary(&mut self) -> Result<Value, String> {
        match self.peek().cloned() {
            Some(Tok::Op('-')) => {
                self.next();
                match self.unary()? {
                    e @ Value::Error(_) => Ok(e),
                    v => Ok(match arith(&v) {
                        Err(e) => e,
                        Ok(x) => Value::Number(-x),
                    }),
                }
            }
            Some(Tok::Op('+')) => {
                self.next();
                self.unary()
            }
            _ => self.atom(),
        }
    }

    pub(super) fn args(&mut self) -> Result<Vec<Arg>, String> {
        // 関数の引数は**配列数式として**読む — 範囲は形(列数)を保ち、
        // あふれる関数(FILTER 等)や要素ごとの演算(C1:C9>100、
        // SEQUENCE(3)*2)もそのまま並びとして渡る。
        // 1つの値に落ちるものは従来どおり1つの値
        let mut out = Vec::new();
        if let Some(Tok::RParen) = self.peek() {
            self.next();
            return Ok(out);
        }
        loop {
            let v = {
                let mut ap = AP { p: self };
                ap.expr()?
            };
            // 2次元の並びを(列数, 行優先の値)に直す
            let flat = |rows: Vec<Vec<Value>>| -> (u32, Vec<Value>) {
                let w = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
                let mut vals = Vec::new();
                for row in &rows {
                    for c in 0..w {
                        vals.push(row.get(c).cloned().unwrap_or(Value::Empty));
                    }
                }
                (w as u32, vals)
            };
            out.push(match v {
                AVal::One(x) => Arg::One(x),
                AVal::Arr(rows) => {
                    let (w, vals) = flat(rows);
                    Arg::Rect(w, vals)
                }
                // 参照の和は領域の形を控えて、値は続けて並べる
                AVal::Union(areas) => {
                    let mut shape = Vec::new();
                    let mut vals = Vec::new();
                    for rows in areas {
                        let (w, vs) = flat(rows);
                        shape.push((w, vs.len()));
                        vals.extend(vs);
                    }
                    Arg::Union(shape, vals)
                }
            });
            match self.next() {
                Some(Tok::Comma) => continue,
                Some(Tok::RParen) => break,
                _ => return Err("引数の括弧が閉じていません".into()),
            }
        }
        Ok(out)
    }

    /// 配列定数 `{1,2;3,4}` の中身を読む(`{` は読んだ後に呼ぶ)。
    /// 中身は定数だけ — 数(負の符号つきも)・文字・TRUE/FALSE。
    /// `,` が列の区切り、`;` が行の区切り(Excel と同じ書き方)
    pub(super) fn array_const(&mut self) -> Result<Vec<Vec<Value>>, String> {
        let mut rows = vec![Vec::new()];
        loop {
            let v = match self.next() {
                Some(Tok::Num(n)) => Value::Number(n),
                Some(Tok::Str(s)) => Value::Text(s),
                Some(Tok::Op(sign @ ('-' | '+'))) => match self.next() {
                    Some(Tok::Num(n)) => Value::Number(if sign == '-' { -n } else { n }),
                    _ => return Err("配列定数の符号の後ろが数ではありません".into()),
                },
                Some(Tok::Name(n)) if n == "TRUE" => Value::Bool(true),
                Some(Tok::Name(n)) if n == "FALSE" => Value::Bool(false),
                _ => return Err("配列定数の中身が読めません".into()),
            };
            rows.last_mut().expect("1行は必ずある").push(v);
            match self.next() {
                Some(Tok::Comma) => {}
                Some(Tok::Semi) => rows.push(Vec::new()),
                Some(Tok::RBrace) => return Ok(rows),
                _ => return Err("配列定数が閉じていません".into()),
            }
        }
    }

    /// 別のシートの範囲を答える。直書きの `Sheet2!A1` と
    /// `INDIRECT("Sheet2!A1")` の両方がここを通る(道を1本にする)。
    /// 自分のシート名なら普通の範囲、知らない名前と1枚だけの計算では #REF!
    pub(super) fn sheet_ans(&self, name: &str, a: Pos, z: Pos) -> RefAns {
        if name == self.sheet.name() {
            return RefAns::At(a, z);
        }
        match self.others.iter().find(|s| s.name() == name) {
            // 別のシートの値は**その時の値**を写す(位置では持ち帰れない)
            Some(other) => {
                let cols = a.col.abs_diff(z.col) + 1;
                let mut vals = Vec::new();
                for r in a.row.min(z.row)..=a.row.max(z.row) {
                    for c in a.col.min(z.col)..=a.col.max(z.col) {
                        vals.push(other.value(Pos::new(r, c)));
                    }
                }
                RefAns::Rect(cols, vals)
            }
            // 知らない名前・1枚だけの計算では #REF!(黙って自シートと読まない)
            None => RefAns::Bad(Value::Error("#REF!".into())),
        }
    }

    /// LET の中身。`(` の次から読み、閉じ括弧まで。
    /// 「名前 , 値」の組が続く限り束ね、組でなくなった所が本体の式
    pub(super) fn let_body(&mut self) -> Result<Value, String> {
        loop {
            // 束縛の名前か? — **次が `,` のときだけ**名前として取る。
            // 本体が名前1つだけ(LET(x,1,x))なら次は `)` なので本体に回る
            let pair = matches!(
                (self.t.get(self.i), self.t.get(self.i + 1)),
                (Some(Tok::Name(_)), Some(Tok::Comma))
            );
            if pair {
                let Some(Tok::Name(n)) = self.t.get(self.i).cloned() else {
                    unreachable!("上で確かめた")
                };
                self.i += 2; // 名前と `,` を飛ばす
                let v = self.expr()?;
                self.lets.push((n, v));
                match self.next() {
                    Some(Tok::Comma) => continue,
                    // 名前と値で終わった = 本体の式が無い
                    _ => return Ok(Value::Error("#VALUE!".into())),
                }
            }
            let body = self.expr()?;
            return match self.next() {
                Some(Tok::RParen) => Ok(body),
                _ => Err("引数の括弧が閉じていません".into()),
            };
        }
    }

    /// 構造化参照を範囲に直す。表の名前(省いたらいまのセルが入っている表)と
    /// **見出しの字**で列を引く。見出し行の無い表からは引けない(None)
    pub(super) fn table_range(&self, tbl: Option<&str>, col: &str, this_row: bool) -> Option<(Pos, Pos)> {
        let inside = |t: &crate::TableDef, p: Pos| {
            p.row >= t.a.row && p.row <= t.b.row && p.col >= t.a.col && p.col <= t.b.col
        };
        let t = match tbl {
            Some(n) => self.sheet.tables().iter().find(|t| t.name == n)?,
            None => self.sheet.tables().iter().find(|t| inside(t, self.at))?,
        };
        if !t.header {
            return None; // 見出しが無ければ列を名前で引けない
        }
        // 見出しの字で列を引く。**中身の無いセルは空文字**なので、
        // `value` の答えをそのまま比べれば足ります(`Value::Empty` の
        // `display` は空文字です)
        let c = (t.a.col..=t.b.col).find(|c| self.sheet.value(Pos::new(t.a.row, *c)).display() == col)?;
        if this_row {
            // いまの行の同じ列。表の外や見出しの行なら引けない
            if self.at.row <= t.a.row || self.at.row > t.b.row {
                return None;
            }
            return Some((Pos::new(self.at.row, c), Pos::new(self.at.row, c)));
        }
        // データ本体(見出しと合計行は外す)
        let r0 = t.a.row + 1;
        let r1 = if t.totals { t.b.row.checked_sub(1)? } else { t.b.row };
        (r0 <= r1).then_some((Pos::new(r0, c), Pos::new(r1, c)))
    }

    /// ブックの並び順のシート一覧(自分を含む)。串刺し集計が使う —
    /// others は「自分より前」+「自分より後」の並びなので、
    /// sheet_at の所へ自分を挿し戻せば元の順になる
    pub(super) fn sheets_in_order(&self) -> Vec<&'a dyn Grid> {
        let k = self.sheet_at.min(self.others.len());
        let mut v: Vec<&'a dyn Grid> = Vec::with_capacity(self.others.len() + 1);
        v.extend(self.others[..k].iter().copied());
        v.push(self.sheet);
        v.extend(self.others[k..].iter().copied());
        v
    }

    /// 串刺し集計 `Sheet1:Sheet3!A1` — 並び順で2枚の間にある全シートの
    /// 同じ場所を集めて1つの並びにする。どちらかの名前が無ければ #REF!
    pub(super) fn sheet3_ans(&self, from: &str, to: &str, a: Pos, z: Pos) -> RefAns {
        let all = self.sheets_in_order();
        let (Some(i), Some(j)) = (
            all.iter().position(|s| s.name() == from),
            all.iter().position(|s| s.name() == to),
        ) else {
            return RefAns::Bad(Value::Error("#REF!".into()));
        };
        let (i, j) = (i.min(j), i.max(j));
        let cols = a.col.abs_diff(z.col) + 1;
        let mut vals = Vec::new();
        for sh in &all[i..=j] {
            for r in a.row.min(z.row)..=a.row.max(z.row) {
                for c in a.col.min(z.col)..=a.col.max(z.col) {
                    let p = Pos::new(r, c);
                    // 自分のシートは計算途中の値(resolved)を見る
                    vals.push(if sh.name() == self.sheet.name() {
                        self.cell(p)
                    } else {
                        sh.value(p)
                    });
                }
            }
        }
        RefAns::Rect(cols, vals)
    }

    /// OFFSET / INDIRECT — **計算して決まる参照**。
    /// Ok(RefAns) / Err(構文エラー) の2層
    pub(super) fn ref_call(&mut self, name: &str) -> Result<RefAns, String> {
        if name == "INDIRECT" {
            // INDIRECT(文字列, [参照形式]) — "A1"・"A1:B2"・"別の表!A1" を受ける。
            // 参照形式が FALSE なら R1C1 形式("R2C3"・"R[1]C[-1]")として読む
            let v = self.expr()?;
            let a1 = match self.next() {
                Some(Tok::RParen) => true,
                Some(Tok::Comma) => {
                    let f = self.expr()?;
                    match self.next() {
                        Some(Tok::RParen) => {}
                        _ => return Err("引数の括弧が閉じていません".into()),
                    }
                    f.as_number() != 0.0
                }
                _ => return Err("引数の括弧が閉じていません".into()),
            };
            if let Value::Error(_) = v {
                return Ok(RefAns::Bad(v));
            }
            let s = v.display();
            let (sheet_name, rest) = match s.split_once('!') {
                Some((n, r)) => (Some(n.trim_matches('\'').to_string()), r.to_string()),
                None => (None, s),
            };
            // R1C1 は A1 に直してから同じ道を通す(R[1] はいまのセルから数える)
            let rest = if a1 { rest } else { crate::refs::formula_from_r1c1(&rest, self.at) };
            let range = match rest.split_once(':') {
                Some((a, z)) => Pos::parse(a).zip(Pos::parse(z)),
                None => Pos::parse(&rest).map(|p| (p, p)),
            };
            let Some((a, z)) = range else {
                return Ok(RefAns::Bad(Value::Error("#REF!".into())));
            };
            return Ok(match sheet_name {
                None => RefAns::At(a, z),
                Some(n) => self.sheet_ans(&n, a, z),
            });
        }
        // OFFSET(基準, 行, 列, [高さ], [幅])
        let (a, z) = match self.next() {
            Some(Tok::Ref(p)) => (p, p),
            Some(Tok::Range(a, z)) => (a, z),
            _ => return Ok(RefAns::Bad(Value::Error("#VALUE!".into()))),
        };
        let mut vals = Vec::new();
        loop {
            match self.next() {
                Some(Tok::Comma) => vals.push(self.expr()?.as_number()),
                Some(Tok::RParen) => break,
                _ => return Err("引数の括弧が閉じていません".into()),
            }
        }
        if !(2..=4).contains(&vals.len()) {
            return Ok(RefAns::Bad(Value::Error("#VALUE!".into())));
        }
        let (r0, c0) = (a.row.min(z.row) as i64, a.col.min(z.col) as i64);
        let (dr, dc) = (vals[0] as i64, vals[1] as i64);
        let h = vals.get(2).map(|v| *v as i64).unwrap_or(i64::from(a.row.abs_diff(z.row)) + 1);
        let w = vals.get(3).map(|v| *v as i64).unwrap_or(i64::from(a.col.abs_diff(z.col)) + 1);
        let (nr, nc) = (r0 + dr, c0 + dc);
        // 表の外に出た参照は #REF!(Excel と同じ数え方の上限)
        if nr < 0 || nc < 0 || h < 1 || w < 1 || nr + h > 1_048_576 || nc + w > 16_384 {
            return Ok(RefAns::Bad(Value::Error("#REF!".into())));
        }
        Ok(RefAns::At(
            Pos::new(nr as u32, nc as u32),
            Pos::new((nr + h - 1) as u32, (nc + w - 1) as u32),
        ))
    }

    /// ROW / COLUMN / ROWS / COLUMNS — 参照の位置と大きさを答える。
    /// 値ではなく**参照そのもの**が要るので、args() で崩す前に読む。
    /// 引数なしの ROW()/COLUMN() は、いま計算しているセルの位置
    pub(super) fn pos_fn(&mut self, name: &str) -> Result<Value, String> {
        let (a, z) = match self.peek().cloned() {
            Some(Tok::RParen) => (self.at, self.at),
            Some(Tok::Ref(p)) => {
                self.next();
                (p, p)
            }
            Some(Tok::Range(a, z)) => {
                self.next();
                (a, z)
            }
            _ => return Ok(Value::Error("#VALUE!".into())),
        };
        match self.next() {
            Some(Tok::RParen) => {}
            _ => return Err("引数の括弧が閉じていません".into()),
        }
        Ok(Value::Number(match name {
            "ROW" => (a.row.min(z.row) + 1) as f64,
            "COLUMN" => (a.col.min(z.col) + 1) as f64,
            "ROWS" => (a.row.abs_diff(z.row) + 1) as f64,
            _ => (a.col.abs_diff(z.col) + 1) as f64,
        }))
    }

    pub(super) fn atom(&mut self) -> Result<Value, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Value::Number(n)),
            Some(Tok::Str(s)) => Ok(Value::Text(s)),
            Some(Tok::Ref(p)) => Ok(self.cell(p)),
            Some(Tok::Range(a, z)) => {
                // 単独の範囲は先頭セルの値(関数の外では範囲は使えない)
                Ok(self.range_values(a, z).into_iter().next().unwrap_or(Value::Empty))
            }
            Some(Tok::Sheet(name, a, z)) => Ok(match self.sheet_ans(&name, a, z) {
                RefAns::At(a, z) => {
                    self.range_values(a, z).into_iter().next().unwrap_or(Value::Empty)
                }
                RefAns::Rect(_, vals) => vals.into_iter().next().unwrap_or(Value::Empty),
                RefAns::Bad(v) => v,
            }),
            Some(Tok::Sheet3(from, to, a, z)) => {
                Ok(match self.sheet3_ans(&from, &to, a, z) {
                    RefAns::Rect(_, vals) => vals.into_iter().next().unwrap_or(Value::Empty),
                    RefAns::Bad(v) => v,
                    RefAns::At(a, z) => {
                        self.range_values(a, z).into_iter().next().unwrap_or(Value::Empty)
                    }
                })
            }
            Some(Tok::Table(tbl, col, this_row)) => {
                Ok(match self.table_range(tbl.as_deref(), &col, this_row) {
                    // 単独なら先頭の値([@列] は1セルなのでその値)
                    Some((a, z)) => {
                        self.range_values(a, z).into_iter().next().unwrap_or(Value::Empty)
                    }
                    None => Value::Error("#REF!".into()),
                })
            }
            Some(Tok::LParen) => {
                let v = self.expr()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(v),
                    // `(A1:B2,C3:D4)` の参照の和。1つの値が要る場面では
                    // 使えない(Excel も #VALUE!)。残りは読み飛ばす
                    Some(Tok::Comma) => {
                        loop {
                            self.expr()?;
                            match self.next() {
                                Some(Tok::Comma) => continue,
                                Some(Tok::RParen) => break,
                                _ => return Err("括弧が閉じていません".into()),
                            }
                        }
                        Ok(Value::Error("#VALUE!".into()))
                    }
                    _ => Err("括弧が閉じていません".into()),
                }
            }
            // 配列定数。1つの値が要る場面では左上の値を使う
            // (並びのまま要る場面は配列の読み手 AP が受ける)
            Some(Tok::LBrace) => {
                let rows = self.array_const()?;
                Ok(rows
                    .into_iter()
                    .next()
                    .and_then(|r| r.into_iter().next())
                    .unwrap_or(Value::Empty))
            }
            Some(Tok::Name(name)) => {
                match self.peek() {
                    Some(Tok::LParen) => {
                        self.next();
                        // 参照の「位置」を答える関数は、値に崩す前に受ける
                        if matches!(name.as_str(), "ROW" | "COLUMN" | "ROWS" | "COLUMNS") {
                            return self.pos_fn(&name);
                        }
                        // 計算して決まる参照。式の中では1セルの値として使う
                        if matches!(name.as_str(), "OFFSET" | "INDIRECT") {
                            return Ok(match self.ref_call(&name)? {
                                RefAns::At(a, z) if a == z => self.cell(a),
                                RefAns::Rect(_, vals) if vals.len() == 1 => {
                                    vals.into_iter().next().unwrap()
                                }
                                RefAns::Bad(v) => v,
                                _ => Value::Error("#VALUE!".into()),
                            });
                        }
                        // ふりがな。シートが持つ読み(xlsx の rPh)を引く。
                        // 読みが無ければセルの字そのもの(Excel と同じ約束)
                        if name == "PHONETIC" {
                            let (a, z) = match self.next() {
                                Some(Tok::Ref(p)) => (p, p),
                                Some(Tok::Range(a, z)) => (a, z),
                                _ => return Ok(Value::Error("#VALUE!".into())),
                            };
                            match self.next() {
                                Some(Tok::RParen) => {}
                                _ => return Err("引数の括弧が閉じていません".into()),
                            }
                            let mut out = String::new();
                            for r in a.row.min(z.row)..=a.row.max(z.row) {
                                for c in a.col.min(z.col)..=a.col.max(z.col) {
                                    let p = Pos::new(r, c);
                                    match self.sheet.phonetic(p) {
                                        Some(ruby) => out.push_str(ruby),
                                        None => out.push_str(&self.cell(p).display()),
                                    }
                                }
                            }
                            return Ok(Value::Text(out));
                        }
                        // CELL("filename") — **`径路[ファイル名]シート名`**。
                        // 実物では `]` の後ろを取ってシート名にする常套句と
                        // して使われる(=MID(CELL("filename",A1),
                        // FIND("]",…)+1, 31))。**この形しか実装しない** —
                        // "address"・"row"・"width" などは今までどおり
                        // #NAME? で、要ると分かってから足す。
                        //
                        // 第2引数は参照だが、同じブックなら答えは変わらない
                        // ので受け取って捨てる。保存前は空文字(Excel と同じ。
                        // #NAME? のままにはしない — 実装できる物を
                        // 誤りにして回避させない)
                        if name == "CELL" {
                            let args = self.args()?;
                            let kind = args
                                .first()
                                .map(|g| g.first().display().to_ascii_lowercase())
                                .unwrap_or_default();
                            if kind != "filename" {
                                return Ok(Value::Error("#NAME?".into()));
                            }
                            return Ok(Value::Text(cell_filename(self.book_path, self.sheet.name())));
                        }
                        // LET(名前, 値, [名前, 値]…, 式) — 名前を束ねてから
                        // 最後の式を計算する。値は**先に計算して**束ねるので、
                        // 後の束縛や本体から前の名前が見える(Excel と同じ)
                        if name == "LET" {
                            let depth = self.lets.len();
                            let r = self.let_body();
                            self.lets.truncate(depth); // 束縛は LET の中だけ
                            return r;
                        }
                        // SUBTOTAL は絞り込みで隠れた行をいつも飛ばし、
                        // 101〜111 は手で隠した行も飛ばす。AGGREGATE は
                        // 第2引数のオプション 1・3・5・7 のとき隠れた行を
                        // 飛ばす(Excel と同じ)。番号は引数を**読んでみてから**
                        // 分かるので、飛ばす行があれば読み直す(字句は残って
                        // いるので安い)
                        if matches!(name.as_str(), "SUBTOTAL" | "AGGREGATE") {
                            let save = self.i;
                            let args = self.args()?;
                            let n = args.first().map(|g| g.first().as_number()).unwrap_or(0.0);
                            let mode: u8 = if name == "SUBTOTAL" {
                                if n > 100.0 { 2 } else { 1 }
                            } else {
                                let opt = args.get(1).map(|g| g.first().as_number()).unwrap_or(0.0);
                                if matches!(opt as i64, 1 | 3 | 5 | 7) { 2 } else { 0 }
                            };
                            let again = (mode >= 1 && self.sheet.any_row_filtered())
                                || (mode >= 2 && self.sheet.any_row_hidden());
                            if again {
                                self.i = save;
                                self.skip_hidden.set(mode);
                                let again = self.args();
                                self.skip_hidden.set(0);
                                return call(&name, again?, self.date1904);
                            }
                            return call(&name, args, self.date1904);
                        }
                        let args = self.args()?;
                        call(&name, args, self.date1904)
                    }
                    _ => match name.as_str() {
                        "TRUE" => Ok(Value::Bool(true)),
                        "FALSE" => Ok(Value::Bool(false)),
                        // LET が束ねた名前(後ろが勝ち = 内側が外側を隠す)
                        _ => match self.lets.iter().rev().find(|(n, _)| *n == name) {
                            Some((_, v)) => Ok(v.clone()),
                            None => Ok(Value::Error("#NAME?".into())),
                        },
                    },
                }
            }
            other => Err(format!("式が途中で終わっています: {other:?}")),
        }
    }
}

/// `CELL("filename")` の答え — **`径路[ファイル名]シート名`**。
///
/// 径路が空(まだ保存していない)なら空文字。Excel と同じで、
/// このとき `FIND("]",…)` は #VALUE! になる — それが本家の姿。
///
/// 径路の区切りは OS のものをそのまま使う(Windows なら `\`)。
/// Excel も同じで、式が拾うのは `]` の後ろだけなので影響しない
pub fn cell_filename(book_path: &str, sheet_name: &str) -> String {
    if book_path.is_empty() {
        return String::new();
    }
    let p = std::path::Path::new(book_path);
    let file = p.file_name().map(|s| s.to_string_lossy()).unwrap_or_default();
    let dir = p.parent().map(|s| s.to_string_lossy()).unwrap_or_default();
    // Excel は径路の末尾に区切りを付ける(`C:\帳票\[売上.xlsx]4月`)
    let sep = std::path::MAIN_SEPARATOR;
    if dir.is_empty() {
        format!("[{file}]{sheet_name}")
    } else {
        format!("{dir}{sep}[{file}]{sheet_name}")
    }
}
