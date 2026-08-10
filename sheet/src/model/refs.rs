//! **式の中の参照を動かす。** 行や列を入れたときの付け替え、R1C1 との
//! 行き来、コピーのときのずらし。


use super::types::*;

/// 式の中の A1 参照を、行・列の出し入れに合わせてずらす。
///
/// **これをやらないと、行を挿しただけで式が別のセルを指す。**
/// 「動かない」ではなく「**間違った答えを黙って出す**」側の欠陥なので、
/// 帳票では致命的になる。
///
/// 絶対参照(`$C$5`)の `$` は形として残す — 利用者が書いたものを勝手に消さない。
/// 参照先が消えたときは `#REF!` にする(黙って別のセルを指すより良い)。
pub fn shift_refs(formula: &str, at: u32, delta: i64, is_row: bool) -> String {
    let ch: Vec<char> = formula.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < ch.len() {
        // 文字列の中の A1 らしきものは触らない
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
        // 参照の形: [$]英字+[$]数字+
        let start = i;
        let mut j = i;
        let abs_col = j < ch.len() && ch[j] == '$';
        if abs_col {
            j += 1;
        }
        let letters = j;
        while j < ch.len() && ch[j].is_ascii_alphabetic() {
            j += 1;
        }
        if j == letters {
            out.push(ch[i]);
            i += 1;
            continue;
        }
        let abs_row = j < ch.len() && ch[j] == '$';
        if abs_row {
            j += 1;
        }
        let digits = j;
        while j < ch.len() && ch[j].is_ascii_digit() {
            j += 1;
        }
        if j == digits {
            // 英字だけ = 関数名。触らない
            out.extend(&ch[start..j]);
            i = j;
            continue;
        }
        let raw: String = ch[start..j].iter().collect();
        out.push_str(&shift_one(&raw, at, delta, is_row, abs_col, abs_row));
        i = j;
    }
    out
}

pub(super) fn shift_one(raw: &str, at: u32, delta: i64, is_row: bool, abs_col: bool, abs_row: bool) -> String {
    let Some(p) = Pos::parse(raw) else { return raw.to_string() };
    let target = if is_row { p.row } else { p.col };
    // 挿した/抜いた場所より手前は動かない
    if target < at {
        return raw.to_string();
    }
    // 抜いた行そのものを指していたら、指す先が無い
    if delta < 0 && target == at {
        return "#REF!".to_string();
    }
    let moved = (target as i64 + delta).max(0) as u32;
    let np = if is_row { Pos { row: moved, col: p.col } } else { Pos { row: p.row, col: moved } };
    // $ の形を戻す
    let a1 = np.a1();
    let split = a1.find(|c: char| c.is_ascii_digit()).unwrap_or(a1.len());
    let (c, r) = a1.split_at(split);
    format!("{}{c}{}{r}", if abs_col { "$" } else { "" }, if abs_row { "$" } else { "" })
}

/// 参照の引き直しの結果。
/// 式の A1 参照を R1C1 の書き方に(`at` = 式のあるセル)。表示用。
/// 文字列の中は触らない。関数名(後ろが `(`)とシート名(後ろが `!`)も触らない
pub fn formula_to_r1c1(formula: &str, at: Pos) -> String {
    let ch: Vec<char> = formula.chars().collect();
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
        let start = i;
        let mut j = i;
        let abs_col = j < ch.len() && ch[j] == '$';
        if abs_col {
            j += 1;
        }
        let letters = j;
        while j < ch.len() && ch[j].is_ascii_alphabetic() {
            j += 1;
        }
        if j == letters {
            out.push(ch[i]);
            i += 1;
            continue;
        }
        let abs_row = j < ch.len() && ch[j] == '$';
        if abs_row {
            j += 1;
        }
        let digits = j;
        while j < ch.len() && ch[j].is_ascii_digit() {
            j += 1;
        }
        if j == digits {
            out.extend(&ch[start..j]);
            i = j;
            continue;
        }
        let raw: String = ch[start..j].iter().collect();
        // LOG10( のような関数名、ABC1! のようなシート名は参照ではない
        let next = ch.get(j).copied();
        if next == Some('(') || next == Some('!') {
            out.push_str(&raw);
            i = j;
            continue;
        }
        match Pos::parse(&raw) {
            Some(p) => {
                let r = if abs_row {
                    format!("R{}", p.row + 1)
                } else if p.row == at.row {
                    "R".into()
                } else {
                    format!("R[{}]", p.row as i64 - at.row as i64)
                };
                let c = if abs_col {
                    format!("C{}", p.col + 1)
                } else if p.col == at.col {
                    "C".into()
                } else {
                    format!("C[{}]", p.col as i64 - at.col as i64)
                };
                out.push_str(&r);
                out.push_str(&c);
            }
            None => out.push_str(&raw),
        }
        i = j;
    }
    out
}

/// R1C1 の書き方の参照を A1 に戻す(`at` = 式を打ったセル)。
/// 範囲の外に出る相対参照(R[-9]C を1行目で 等)は #REF! にする
pub fn formula_from_r1c1(formula: &str, at: Pos) -> String {
    let ch: Vec<char> = formula.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    // R / C の後ろの「[数]」「数」「無し」を読む。返りは (絶対か, 番地, 進んだ先)
    fn part(ch: &[char], mut j: usize, base: u32) -> Option<(bool, i64, usize)> {
        if ch.get(j) == Some(&'[') {
            let mut k = j + 1;
            let neg = ch.get(k) == Some(&'-');
            if neg {
                k += 1;
            }
            let d0 = k;
            while k < ch.len() && ch[k].is_ascii_digit() {
                k += 1;
            }
            if k == d0 || ch.get(k) != Some(&']') {
                return None;
            }
            let n: i64 = ch[d0..k].iter().collect::<String>().parse().ok()?;
            Some((false, base as i64 + if neg { -n } else { n }, k + 1))
        } else {
            let d0 = j;
            while j < ch.len() && ch[j].is_ascii_digit() {
                j += 1;
            }
            if j == d0 {
                // 数が無い = 自分の行/列
                Some((false, base as i64, j))
            } else {
                let n: i64 = ch[d0..j].iter().collect::<String>().parse().ok()?;
                Some((true, n - 1, j))
            }
        }
    }
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
        // 語の途中(英字・数字・_ の続き)から R を拾わない
        let prev_word = i > 0 && (ch[i - 1].is_ascii_alphanumeric() || ch[i - 1] == '_');
        if !prev_word && (ch[i] == 'R' || ch[i] == 'r') {
            if let Some((abs_r, row, jc)) = part(&ch, i + 1, at.row) {
                if (ch.get(jc) == Some(&'C') || ch.get(jc) == Some(&'c'))
                    && ch.get(jc + 1) != Some(&'(')
                {
                    if let Some((abs_c, col, jend)) = part(&ch, jc + 1, at.col) {
                        // 後ろに英字が続くなら参照ではない(RC1A のような語)
                        let tail_word = ch
                            .get(jend)
                            .map(|c| c.is_ascii_alphabetic() || *c == '_' || *c == '(')
                            .unwrap_or(false);
                        if !tail_word {
                            if row < 0 || col < 0 {
                                out.push_str("#REF!");
                            } else {
                                let p = Pos::new(row as u32, col as u32);
                                let a1 = p.a1();
                                let split = a1
                                    .find(|c: char| c.is_ascii_digit())
                                    .unwrap_or(a1.len());
                                let (cs, rs) = a1.split_at(split);
                                out.push_str(&format!(
                                    "{}{cs}{}{rs}",
                                    if abs_c { "$" } else { "" },
                                    if abs_r { "$" } else { "" }
                                ));
                            }
                            i = jend;
                            continue;
                        }
                    }
                }
            }
        }
        out.push(ch[i]);
        i += 1;
    }
    out
}

pub enum MapRef {
    /// そのまま
    Keep,
    /// 参照先が動いた(一緒に動かす)
    To(Pos),
    /// 参照先が消えた(#REF! にする — 黙って別のセルを指すより良い)
    Broken,
}

/// 式の中の A1 参照を、写像 `f` で引き直す。
/// 文字列の中・関数名は触らない。`$` の形は保つ。
pub fn map_refs(formula: &str, f: impl Fn(Pos) -> MapRef) -> String {
    let ch: Vec<char> = formula.chars().collect();
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
        let start = i;
        let mut j = i;
        let abs_col = j < ch.len() && ch[j] == '$';
        if abs_col {
            j += 1;
        }
        let letters = j;
        while j < ch.len() && ch[j].is_ascii_alphabetic() {
            j += 1;
        }
        if j == letters {
            out.push(ch[i]);
            i += 1;
            continue;
        }
        let abs_row = j < ch.len() && ch[j] == '$';
        if abs_row {
            j += 1;
        }
        let digits = j;
        while j < ch.len() && ch[j].is_ascii_digit() {
            j += 1;
        }
        if j == digits {
            out.extend(&ch[start..j]);
            i = j;
            continue;
        }
        let raw: String = ch[start..j].iter().collect();
        match Pos::parse(&raw) {
            Some(p) => match f(p) {
                MapRef::Keep => out.push_str(&raw),
                MapRef::Broken => out.push_str("#REF!"),
                MapRef::To(np) => {
                    let a1 = np.a1();
                    let split = a1.find(|c: char| c.is_ascii_digit()).unwrap_or(a1.len());
                    let (c, r) = a1.split_at(split);
                    out.push_str(&format!(
                        "{}{c}{}{r}",
                        if abs_col { "$" } else { "" },
                        if abs_row { "$" } else { "" }
                    ));
                }
            },
            None => out.push_str(&raw),
        }
        i = j;
    }
    out
}

/// 式の中の相対参照を (dr, dc) だけずらす。**コピーの規則**。
///
/// 行の出し入れ(`shift_refs`)とは別物 — コピーでは位置に関係なく
/// **相対参照が全部ずれ、`$` の付いた側だけ止まる**。
/// 紙の外(負の位置)を指すことになったら `#REF!`。
pub fn offset_refs(formula: &str, dr: i64, dc: i64) -> String {
    let ch: Vec<char> = formula.chars().collect();
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
        let start = i;
        let mut j = i;
        let abs_col = j < ch.len() && ch[j] == '$';
        if abs_col {
            j += 1;
        }
        let letters = j;
        while j < ch.len() && ch[j].is_ascii_alphabetic() {
            j += 1;
        }
        if j == letters {
            out.push(ch[i]);
            i += 1;
            continue;
        }
        let abs_row = j < ch.len() && ch[j] == '$';
        if abs_row {
            j += 1;
        }
        let digits = j;
        while j < ch.len() && ch[j].is_ascii_digit() {
            j += 1;
        }
        if j == digits {
            out.extend(&ch[start..j]);
            i = j;
            continue;
        }
        let raw: String = ch[start..j].iter().collect();
        match Pos::parse(&raw) {
            Some(p) => {
                let nr = if abs_row { p.row as i64 } else { p.row as i64 + dr };
                let nc = if abs_col { p.col as i64 } else { p.col as i64 + dc };
                if nr < 0 || nc < 0 {
                    out.push_str("#REF!");
                } else {
                    let a1 = Pos { row: nr as u32, col: nc as u32 }.a1();
                    let split = a1.find(|c: char| c.is_ascii_digit()).unwrap_or(a1.len());
                    let (c, r) = a1.split_at(split);
                    out.push_str(&format!(
                        "{}{c}{}{r}",
                        if abs_col { "$" } else { "" },
                        if abs_row { "$" } else { "" }
                    ));
                }
            }
            None => out.push_str(&raw),
        }
        i = j;
    }
    out
}
