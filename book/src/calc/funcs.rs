//! **関数の library。** `SUM` から動的配列まで、名前で引いて答える。
//!
//! 日付・文字・全角半角の道具もここに置く — 使うのが関数だけだから。

use std::collections::HashSet;

use crate::{format_value, Pos, Value};

use super::parse::*;
use super::run::*;

/// 配列数式の途中の値 — 1つの値か、2次元の並び。
pub(super) enum AVal {
    One(Value),
    Arr(Vec<Vec<Value>>),
}

impl AVal {
    pub(super) fn dims(&self) -> (usize, usize) {
        match self {
            AVal::One(_) => (1, 1),
            AVal::Arr(r) => (r.len(), r.iter().map(|x| x.len()).max().unwrap_or(0)),
        }
    }
    /// 要素を取る。1行・1列の側は引き伸ばす(Excel のブロードキャストと同じ)。
    /// 引き伸ばせない外側は #N/A(Excel の配列数式と同じ答え)
    pub(super) fn at(&self, r: usize, c: usize) -> Value {
        match self {
            AVal::One(v) => v.clone(),
            AVal::Arr(rows) => {
                let (h, w) = self.dims();
                let rr = if h == 1 { 0 } else { r };
                let cc = if w == 1 { 0 } else { c };
                if rr >= h || cc >= w {
                    Value::Error("#N/A".into())
                } else {
                    rows[rr].get(cc).cloned().unwrap_or(Value::Empty)
                }
            }
        }
    }
}

/// 要素ごとの2項演算。両方が1つの値なら普通の計算、
/// 並びが混ざれば大きい方の形に広げて1要素ずつ
pub(super) fn zip_aval(a: &AVal, b: &AVal, f: impl Fn(&Value, &Value) -> Value) -> AVal {
    if let (AVal::One(x), AVal::One(y)) = (a, b) {
        return AVal::One(f(x, y));
    }
    let (ha, wa) = a.dims();
    let (hb, wb) = b.dims();
    let (h, w) = (ha.max(hb), wa.max(wb));
    AVal::Arr(
        (0..h)
            .map(|r| {
                (0..w)
                    .map(|c| {
                        // エラーは伝播する(表計算の作法)
                        let x = a.at(r, c);
                        if let Value::Error(_) = x {
                            return x;
                        }
                        let y = b.at(r, c);
                        if let Value::Error(_) = y {
                            return y;
                        }
                        f(&x, &y)
                    })
                    .collect()
            })
            .collect(),
    )
}

/// 配列数式の評価器。文法は P と同じで、**演算が要素ごとに働く**。
/// =SEQUENCE(3)+1 のように、あふれる関数を四則・比較・& と組み合わせた
/// 式はこちらを通る(範囲もここでは並びとして扱う)
pub(super) struct AP<'a, 'b> {
    pub(super) p: &'b mut P<'a>,
}

impl AP<'_, '_> {
    pub(super) fn expr(&mut self) -> Result<AVal, String> {
        let lhs = self.add()?;
        if let Some(Tok::Cmp(op)) = self.p.peek().cloned() {
            self.p.next();
            let rhs = self.add()?;
            return Ok(zip_aval(&lhs, &rhs, |x, y| Value::Bool(cmp_values(&op, x, y))));
        }
        Ok(lhs)
    }

    pub(super) fn add(&mut self) -> Result<AVal, String> {
        let mut v = self.mul()?;
        while let Some(Tok::Op(o @ ('+' | '-' | '&'))) = self.p.peek().cloned() {
            self.p.next();
            let r = self.mul()?;
            v = zip_aval(&v, &r, |x, y| match o {
                '+' => Value::Number(x.as_number() + y.as_number()),
                '-' => Value::Number(x.as_number() - y.as_number()),
                _ => Value::Text(format!("{}{}", x.display(), y.display())),
            });
        }
        Ok(v)
    }

    pub(super) fn mul(&mut self) -> Result<AVal, String> {
        let mut v = self.pow()?;
        while let Some(Tok::Op(o @ ('*' | '/'))) = self.p.peek().cloned() {
            self.p.next();
            let r = self.pow()?;
            v = zip_aval(&v, &r, |x, y| {
                if o == '/' && y.as_number() == 0.0 {
                    Value::Error("#DIV/0!".into())
                } else if o == '*' {
                    Value::Number(x.as_number() * y.as_number())
                } else {
                    Value::Number(x.as_number() / y.as_number())
                }
            });
        }
        Ok(v)
    }

    pub(super) fn pow(&mut self) -> Result<AVal, String> {
        let v = self.unary()?;
        if let Some(Tok::Op('^')) = self.p.peek() {
            self.p.next();
            let r = self.pow()?;
            return Ok(zip_aval(&v, &r, |x, y| {
                Value::Number(x.as_number().powf(y.as_number()))
            }));
        }
        Ok(v)
    }

    pub(super) fn unary(&mut self) -> Result<AVal, String> {
        match self.p.peek().cloned() {
            Some(Tok::Op('-')) => {
                self.p.next();
                let v = self.unary()?;
                Ok(zip_aval(&v, &AVal::One(Value::Number(0.0)), |x, _| match x {
                    e @ Value::Error(_) => e.clone(),
                    v => Value::Number(-v.as_number()),
                }))
            }
            Some(Tok::Op('+')) => {
                self.p.next();
                self.unary()
            }
            _ => self.atom(),
        }
    }

    pub(super) fn atom(&mut self) -> Result<AVal, String> {
        match self.p.peek().cloned() {
            // 配列数式の中では、範囲は並びそのもの
            Some(Tok::Range(a, z)) => {
                self.p.next();
                let cols = (a.col.abs_diff(z.col) + 1) as usize;
                let vals = self.p.range_values(a, z);
                Ok(AVal::Arr(vals.chunks(cols.max(1)).map(|r| r.to_vec()).collect()))
            }
            // 串刺し集計も並びで渡す(=SUM(Sheet1:Sheet3!A1) が効く)
            Some(Tok::Sheet3(from, to, a, z)) => {
                self.p.next();
                Ok(match self.p.sheet3_ans(&from, &to, a, z) {
                    RefAns::Rect(cols, vals) => AVal::Arr(
                        vals.chunks((cols as usize).max(1)).map(|r| r.to_vec()).collect(),
                    ),
                    RefAns::Bad(v) => AVal::One(v),
                    RefAns::At(a, z) => {
                        let cols = (a.col.abs_diff(z.col) + 1) as usize;
                        let vals = self.p.range_values(a, z);
                        AVal::Arr(vals.chunks(cols.max(1)).map(|r| r.to_vec()).collect())
                    }
                })
            }
            // 構造化参照も並びで渡す(=SUM(Table1[金額]) が効く)
            Some(Tok::Table(tbl, col, this_row)) => {
                self.p.next();
                Ok(match self.p.table_range(tbl.as_deref(), &col, this_row) {
                    Some((a, z)) => {
                        let cols = (a.col.abs_diff(z.col) + 1) as usize;
                        let vals = self.p.range_values(a, z);
                        AVal::Arr(vals.chunks(cols.max(1)).map(|r| r.to_vec()).collect())
                    }
                    None => AVal::One(Value::Error("#REF!".into())),
                })
            }
            // 別シートの範囲も並びで渡す(=SUM(Sheet2!A1:A5) が効く)
            Some(Tok::Sheet(name, a, z)) => {
                self.p.next();
                Ok(match self.p.sheet_ans(&name, a, z) {
                    RefAns::At(a, z) => {
                        let cols = (a.col.abs_diff(z.col) + 1) as usize;
                        let vals = self.p.range_values(a, z);
                        AVal::Arr(vals.chunks(cols.max(1)).map(|r| r.to_vec()).collect())
                    }
                    RefAns::Rect(cols, vals) => AVal::Arr(
                        vals.chunks((cols as usize).max(1)).map(|r| r.to_vec()).collect(),
                    ),
                    RefAns::Bad(v) => AVal::One(v),
                })
            }
            Some(Tok::LParen) => {
                self.p.next();
                let v = self.expr()?;
                match self.p.next() {
                    Some(Tok::RParen) => Ok(v),
                    _ => Err("括弧が閉じていません".into()),
                }
            }
            // 配列定数はそのまま並びになる(=SUM({1,2,3}) が効く)
            Some(Tok::LBrace) => {
                self.p.next();
                Ok(AVal::Arr(self.p.array_const()?))
            }
            Some(Tok::Name(n)) if self.p.t.get(self.p.i + 1) == Some(&Tok::LParen) => {
                if ARRAY_FNS.contains(&n.as_str()) {
                    // あふれる関数の呼び出し — 並びのまま持つ
                    self.p.next();
                    self.p.next();
                    let args = self.p.args()?;
                    Ok(match array_call(&n, args) {
                        Ok(rows) => AVal::Arr(rows),
                        Err(e) => AVal::One(e),
                    })
                } else if matches!(n.as_str(), "OFFSET" | "INDIRECT") {
                    self.p.next();
                    self.p.next();
                    Ok(match self.p.ref_call(&n)? {
                        RefAns::At(a, z) => {
                            let cols = (a.col.abs_diff(z.col) + 1) as usize;
                            let vals = self.p.range_values(a, z);
                            AVal::Arr(
                                vals.chunks(cols.max(1)).map(|r| r.to_vec()).collect())
                        }
                        RefAns::Rect(cols, vals) => AVal::Arr(
                            vals.chunks((cols as usize).max(1)).map(|r| r.to_vec()).collect()),
                        RefAns::Bad(v) => AVal::One(v),
                    })
                } else {
                    // 普通の関数は1つの値(集計は中で並びを受けている)
                    Ok(AVal::One(self.p.atom()?))
                }
            }
            _ => Ok(AVal::One(self.p.atom()?)),
        }
    }
}

/// 比較の中身。文字同士は文字として、それ以外は数として比べる
/// (式の比較と、範囲の要素ごとの比較が同じ規則を通る)
/// **数の比べ方。6つの記号がみな、この1本で比べます**(2026-08-22)。
///
/// 二進の小数には刻みがあり、`0.1+0.2` は `0.3` にぴたりと一致しません
/// (差は 5.55e-17)。そこを甘く見ないと、事務の式が思ったとおりに
/// 動きません。
///
/// **前は `=` と `<>` だけが甘く、`<` `>` `<=` `>=` は厳密でした。**
/// そのため `=0.1+0.2=0.3` も `=(0.1+0.2)>0.3` も真という、同時には
/// 成り立たないはずの答えが出ていました。数を比べる道を1本にして
/// 揃えます。
///
/// **甘さは相対です。** 前は `f64::EPSILON`(約 2.2e-16)を差にそのまま
/// 当てていました。これは 1 のあたりでしか意味を持たず、
///
/// * 小さい数では甘すぎる — `1e-18` と `9e-18` が等しいと答えていました
///   (9倍違います)
/// * 大きい数では厳しすぎる — `1e10` のあたりでは刻みが 2e-6 もあるのに、
///   2.2e-16 しか許しません
///
/// 大きい方に合わせて 1 刻みぶんだけ許します。両方 0 なら差も 0 なので、
/// そのまま等しくなります。
pub(super) fn cmp_num(a: f64, b: f64) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if a.is_nan() || b.is_nan() {
        return Ordering::Equal; // 比べようがない。呼ぶ側が先に弾く
    }
    let allowance = f64::EPSILON * a.abs().max(b.abs());
    if (a - b).abs() <= allowance {
        return Ordering::Equal;
    }
    if a < b { Ordering::Less } else { Ordering::Greater }
}

/// 記号と並び順から真偽を出す。`cmp_num` と組で使います。
pub(super) fn ord_holds(op: &str, o: std::cmp::Ordering) -> bool {
    use std::cmp::Ordering::*;
    match op {
        "=" => o == Equal,
        "<>" => o != Equal,
        "<" => o == Less,
        ">" => o == Greater,
        "<=" => o != Greater,
        ">=" => o != Less,
        _ => false,
    }
}

pub(super) fn cmp_values(op: &str, lhs: &Value, rhs: &Value) -> bool {
    match (lhs, rhs) {
        (Value::Text(a), Value::Text(b)) => match op {
            "=" => a == b,
            "<>" => a != b,
            "<" => a < b,
            ">" => a > b,
            "<=" => a <= b,
            _ => a >= b,
        },
        // **数は1本の基準で比べる**(cmp_num)。ここだけ厳密にしていると、
        // 式の中の比較と関数の中の比較で答えが割れます
        _ => ord_holds(op, cmp_num(lhs.as_number(), rhs.as_number())),
    }
}

/// SUMIF / COUNTIF の条件合わせ。数は数として、文字は文字として比べる。
pub(super) fn matches_cond(v: &Value, cond: &Value) -> bool {
    match cond {
        Value::Number(n) => cmp_num(v.as_number(), *n) == std::cmp::Ordering::Equal,
        Value::Text(s) => {
            // ">100" のような書き方に応える
            let t = s.trim();
            // **記号の長い順に見る**(">=" を ">" と読み違えない)。
            // 比べ方は式の中と同じ1本(cmp_num)
            for op in [">=", "<=", "<>", ">", "<", "="] {
                if let Some(rest) = t.strip_prefix(op) {
                    if let Ok(n) = rest.trim().parse::<f64>() {
                        return !v.is_empty() && ord_holds(op, cmp_num(v.as_number(), n));
                    }
                }
            }
            v.display() == *s
        }
        _ => false,
    }
}

/// 暦(y,m,d)→ 1970-01-01 からの日数(Howard Hinnant の civil_from_days の逆)。
pub(super) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// 1970-01-01 からの日数 → 暦(y,m,d)。
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Excel の日付の通し番号(1899-12-30 起点)と 1970 起点の橋。
pub(crate) const EXCEL_EPOCH_DAYS: i64 = 25569;

/// 暦の日付 → Excel の通し番号。DATE 関数と pysheet(datetime の受け口)が
/// **同じ規約を通るための一本道** — 別々に持つと必ずずれる。
pub fn date_serial(y: i64, m: i64, d: i64) -> i64 {
    days_from_civil(y, m, d) + EXCEL_EPOCH_DAYS
}

/// 起点。1904 のブック(workbookPr の date1904)は 1904-01-01 が通し番号 0 —
/// 1899-12-30 との差は 1462 日(2026-08-13、起点の解釈をエンジンに)
pub fn excel_epoch(date1904: bool) -> i64 {
    if date1904 { EXCEL_EPOCH_DAYS - 1462 } else { EXCEL_EPOCH_DAYS }
}

/// 暦の日付 → 通し番号(起点つき)。1904 のブックの境目はこちらを通す
pub fn date_serial_at(y: i64, m: i64, d: i64, ep: i64) -> i64 {
    days_from_civil(y, m, d) + ep
}

/// 通し番号 → 曜日(0=日曜)。通し番号 1(1900-01-01)は月曜。
pub(crate) fn weekday0(serial: i64, ep: i64) -> i64 {
    // 1970-01-01(木)起点に直して数える
    ((serial - ep).rem_euclid(7) + 4).rem_euclid(7)
}

/// RAND 用の乱数(0.0 以上 1.0 未満)。暗号用ではない(表計算の RAND も同じ)。
/// 依存を増やさず xorshift64* を自前で持つ。種は最初の呼び出し時刻
pub(super) fn rand01() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEED: AtomicU64 = AtomicU64::new(0);
    let mut x = SEED.load(Ordering::Relaxed);
    if x == 0 {
        x = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| u64::from(d.subsec_nanos()) ^ d.as_secs())
            .unwrap_or(0x9E37_79B9_7F4A_7C15)
            | 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// いまの機械の暦での「今日」の通し番号と、時刻(日の割合)。
/// 時計は系の TZ 環境(日本なら JST)に従う — libc の localtime を使う
/// chrono に頼らず、TZ のずれは環境変数 JO_TZ_OFF_HOURS で補える(既定 +9)。
pub(super) fn today_serial(ep: i64) -> (f64, f64) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let off_h: i64 = std::env::var("JO_TZ_OFF_HOURS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9);
    let local = secs + off_h * 3600;
    let days = local.div_euclid(86400);
    let frac = local.rem_euclid(86400) as f64 / 86400.0;
    ((days + ep) as f64, frac)
}

/// LENB 系の1文字の「バイト」数。全角=2、半角(ASCII と半角カナ)=1
/// (Excel の日本語ロケールと同じ数え方。実際の UTF-8 の長さではない)
pub(super) fn jchar_width(c: char) -> usize {
    if c.is_ascii() || ('\u{FF61}'..='\u{FF9F}').contains(&c) {
        1
    } else {
        2
    }
}

/// 全角カタカナ ↔ 半角カナの対応表(並びを揃えてある)
pub(super) const KANA_Z: &str = "ァィゥェォャュョッーアイウエオカキクケコサシスセソタチツテト\
                      ナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン。「」、・";
pub(super) const KANA_H: &str = "ｧｨｩｪｫｬｭｮｯｰｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜｦﾝ｡｢｣､･";
/// 濁点つき(→ 半角では2文字になる)
pub(super) const DAKU_Z: &str = "ガギグゲゴザジズゼゾダヂヅデドバビブベボヴ";
pub(super) const DAKU_H: &str = "ｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾊﾋﾌﾍﾎｳ";
pub(super) const HANDAKU_Z: &str = "パピプペポ";
pub(super) const HANDAKU_H: &str = "ﾊﾋﾌﾍﾎ";

/// ASC — 全角を半角へ(英数記号・空白・カタカナ)
pub(super) fn asc_hankaku(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        // 全角英数記号(!〜~)は 0xFEE0 ずらすと半角
        if ('\u{FF01}'..='\u{FF5E}').contains(&c) {
            out.push(char::from_u32(c as u32 - 0xFEE0).unwrap_or(c));
        } else if c == '\u{3000}' {
            out.push(' ');
        } else if let Some(i) = DAKU_Z.chars().position(|z| z == c) {
            out.push(DAKU_H.chars().nth(i).unwrap());
            out.push('ﾞ');
        } else if let Some(i) = HANDAKU_Z.chars().position(|z| z == c) {
            out.push(HANDAKU_H.chars().nth(i).unwrap());
            out.push('ﾟ');
        } else if let Some(i) = KANA_Z.chars().position(|z| z == c) {
            out.push(KANA_H.chars().nth(i).unwrap());
        } else {
            out.push(c);
        }
    }
    out
}

/// JIS — 半角を全角へ(濁点は1文字に組む)
pub(super) fn jis_zenkaku(s: &str) -> String {
    let ch: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < ch.len() {
        let c = ch[i];
        let next = ch.get(i + 1).copied();
        // 濁点・半濁点は前の字と組んで1文字へ
        if next == Some('ﾞ') {
            if let Some(k) = DAKU_H.chars().position(|h| h == c) {
                out.push(DAKU_Z.chars().nth(k).unwrap());
                i += 2;
                continue;
            }
        }
        if next == Some('ﾟ') {
            if let Some(k) = HANDAKU_H.chars().position(|h| h == c) {
                out.push(HANDAKU_Z.chars().nth(k).unwrap());
                i += 2;
                continue;
            }
        }
        if c.is_ascii_graphic() {
            out.push(char::from_u32(c as u32 + 0xFEE0).unwrap_or(c));
        } else if c == ' ' {
            out.push('\u{3000}');
        } else if let Some(k) = KANA_H.chars().position(|h| h == c) {
            out.push(KANA_Z.chars().nth(k).unwrap());
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// 元号。通し番号 → (名前, ローマ字の頭文字, 和暦の年)。明治より前は None。
/// DATESTRING と表示形式(g・e)が**同じ表**を使う
pub(crate) fn era_of(serial: i64, ep: i64) -> Option<(&'static str, &'static str, i64)> {
    let (y, _, _) = civil_from_days(serial - ep);
    let eras: [(i64, &'static str, &'static str, i64); 5] = [
        (date_serial_at(2019, 5, 1, ep), "令和", "R", 2019),
        (date_serial_at(1989, 1, 8, ep), "平成", "H", 1989),
        (date_serial_at(1926, 12, 25, ep), "昭和", "S", 1926),
        (date_serial_at(1912, 7, 30, ep), "大正", "T", 1912),
        (date_serial_at(1868, 10, 23, ep), "明治", "M", 1868),
    ];
    for (start, name, initial, base) in eras {
        if serial >= start {
            return Some((name, initial, y - base + 1));
        }
    }
    None
}

/// 通し番号 → 和暦の文字(DATESTRING)。明治より前は西暦のまま
pub(super) fn wareki(serial: i64, ep: i64) -> String {
    let (y, m, d) = civil_from_days(serial - ep);
    match era_of(serial, ep) {
        Some((name, _, ey)) => format!("{name}{ey:02}年{m:02}月{d:02}日"),
        None => format!("{y}年{m:02}月{d:02}日"),
    }
}

/// 30/360(米国方式)の日数。DAYS360 と YEARFRAC が使う
pub(super) fn days360(s: i64, e: i64, ep: i64) -> i64 {
    let (sy, sm, mut sd) = civil_from_days(s - ep);
    let (ey, em, mut ed) = civil_from_days(e - ep);
    if sd == 31 {
        sd = 30;
    }
    if ed == 31 && sd == 30 {
        ed = 30;
    }
    (ey - sy) * 360 + (em - sm) * 30 + (ed - sd)
}

/// 符号の変わる区間を粗く探して挟み撃ち(IRR・RATE の反復解)。
/// 見つからなければ None(#NUM! — 黙って0を返さない)
pub(super) fn bisect(f: &dyn Fn(f64) -> f64, lo: f64, hi: f64) -> Option<f64> {
    let steps = 400;
    let mut px = lo;
    let mut py = f(lo);
    for i in 1..=steps {
        let x = lo + (hi - lo) * f64::from(i) / f64::from(steps);
        let y = f(x);
        if px.is_finite() && py.is_finite() && y.is_finite() && py * y <= 0.0 {
            let (mut a, mut b, mut fa) = (px, x, py);
            for _ in 0..200 {
                let m = (a + b) / 2.0;
                let fm = f(m);
                if fa * fm <= 0.0 {
                    b = m;
                } else {
                    a = m;
                    fa = fm;
                }
            }
            return Some((a + b) / 2.0);
        }
        px = x;
        py = y;
    }
    None
}

/// 関数の引数。ほとんどの関数は平らな値で足りるが、表を引く関数
/// (VLOOKUP・INDEX 等)は範囲の**形**(列数)が要る。
#[derive(Debug, Clone)]
pub(super) enum Arg {
    One(Value),
    /// (列数, 行優先の値)
    Rect(u32, Vec<Value>),
}

impl Arg {
    pub(super) fn values(&self) -> &[Value] {
        match self {
            Arg::One(v) => std::slice::from_ref(v),
            Arg::Rect(_, vs) => vs,
        }
    }
    pub(super) fn first(&self) -> Value {
        self.values().first().cloned().unwrap_or(Value::Empty)
    }
}

pub(super) fn call(name: &str, args: Vec<Arg>, date1904: bool) -> Result<Value, String> {
    // 起点(1899-12-30 か 1904-01-01)。日付の境目は全部これを通す
    let ep = excel_epoch(date1904);
    // 表を引く関数は形が要るので、平らにする前に受ける
    match name {
        "VLOOKUP" | "HLOOKUP" => {
            let key = args.first().map(|g| g.first()).unwrap_or(Value::Empty);
            let Some(Arg::Rect(cols, vals)) = args.get(1) else {
                return Ok(Value::Error("#VALUE!".into()));
            };
            let idx = args.get(2).map(|g| g.first().as_number()).unwrap_or(0.0) as usize;
            let (cols, vals) = (*cols as usize, vals);
            if cols == 0 || idx == 0 {
                return Ok(Value::Error("#VALUE!".into()));
            }
            let rows = vals.len() / cols;
            let same = |v: &Value| -> bool {
                match (v, &key) {
                    (Value::Number(x), Value::Number(y)) => (x - y).abs() < 1e-9,
                    _ => v.display() == key.display(),
                }
            };
            let hit = if name == "VLOOKUP" {
                // 1列目を上から探し、その行の idx 列目
                (0..rows)
                    .find(|r| same(&vals[r * cols]))
                    .and_then(|r| vals.get(r * cols + (idx - 1)))
            } else {
                // 1行目を左から探し、その列の idx 行目
                (0..cols)
                    .find(|c| same(&vals[*c]))
                    .and_then(|c| vals.get((idx - 1) * cols + c))
            };
            return Ok(hit.cloned().unwrap_or(Value::Error("#N/A".into())));
        }
        "INDEX" => {
            let Some(Arg::Rect(cols, vals)) = args.first() else {
                return Ok(Value::Error("#VALUE!".into()));
            };
            let cols = *cols as usize;
            let r = args.get(1).map(|g| g.first().as_number()).unwrap_or(0.0) as usize;
            let c = args.get(2).map(|g| g.first().as_number()).unwrap_or(1.0) as usize;
            if r == 0 || c == 0 {
                return Ok(Value::Error("#VALUE!".into()));
            }
            return Ok(vals
                .get((r - 1) * cols + (c - 1))
                .cloned()
                .unwrap_or(Value::Error("#REF!".into())));
        }
        "MATCH" => {
            let key = args.first().map(|g| g.first()).unwrap_or(Value::Empty);
            let hay = args.get(1).map(|g| g.values()).unwrap_or(&[]);
            // 照合の型は 0(完全一致)だけを受ける(それ以外は正直に断る)
            if let Some(t) = args.get(2) {
                if t.first().as_number() != 0.0 {
                    return Ok(Value::Error("#VALUE!".into()));
                }
            }
            return Ok(hay
                .iter()
                .position(|v| v.display() == key.display())
                .map(|i| Value::Number((i + 1) as f64))
                .unwrap_or(Value::Error("#N/A".into())));
        }
        "XMATCH" => {
            // XMATCH(探す値, 探す範囲, [照合の型], [検索の向き])
            // 完全一致(0)だけを受ける。**近似は断る** — 並びが揃っている
            // 前提を黙って敷くと、帳票が静かにずれた行を指す
            let key = args.first().map(|g| g.first()).unwrap_or(Value::Empty);
            let hay = args.get(1).map(|g| g.values()).unwrap_or(&[]);
            if let Some(t) = args.get(2) {
                if t.first().as_number() != 0.0 {
                    return Ok(Value::Error("#VALUE!".into()));
                }
            }
            // 検索の向き: -1 なら後ろから
            let back = args.get(3).map(|g| g.first().as_number() < 0.0).unwrap_or(false);
            let hit = if back {
                hay.iter().rposition(|v| v.display() == key.display())
            } else {
                hay.iter().position(|v| v.display() == key.display())
            };
            return Ok(hit
                .map(|i| Value::Number((i + 1) as f64))
                .unwrap_or(Value::Error("#N/A".into())));
        }
        // データベース関数(D 系)。DSUM(表, 列, 条件表)。
        // **条件表は「見出し + 条件の行」**という Excel の作法そのまま
        "DSUM" | "DAVERAGE" | "DCOUNT" | "DMAX" | "DMIN" | "DGET" => {
            let Some(Arg::Rect(w, vals)) = args.first() else {
                return Ok(Value::Error("#VALUE!".into()));
            };
            let (w, vals) = (*w as usize, vals.clone());
            if w == 0 || vals.len() < w {
                return Ok(Value::Error("#VALUE!".into()));
            }
            let heads: Vec<String> = vals[..w].iter().map(|v| v.display()).collect();
            // 取り出す列: 見出しの名前でも、左から何本目でも
            let field = args.get(1).map(|g| g.first()).unwrap_or(Value::Empty);
            let fi = match &field {
                Value::Number(n) => (*n as usize).checked_sub(1),
                v => heads.iter().position(|h| *h == v.display()),
            };
            let Some(fi) = fi.filter(|i| *i < w) else {
                return Ok(Value::Error("#VALUE!".into()));
            };
            // 条件表: 1行目が見出し、2行目以降が条件(同じ行は AND、行どうしは OR)
            let Some(Arg::Rect(cw, cvals)) = args.get(2) else {
                return Ok(Value::Error("#VALUE!".into()));
            };
            let (cw, cvals) = (*cw as usize, cvals.clone());
            if cw == 0 || cvals.len() < cw * 2 {
                return Ok(Value::Error("#VALUE!".into()));
            }
            let cheads: Vec<String> = cvals[..cw].iter().map(|v| v.display()).collect();
            let mut hits: Vec<Value> = Vec::new();
            for row in vals[w..].chunks(w) {
                let mut any = false;
                for crow in cvals[cw..].chunks(cw) {
                    let mut all = true;
                    let mut used = false;
                    for (k, cond) in crow.iter().enumerate() {
                        let c = cond.display();
                        if c.trim().is_empty() {
                            continue;
                        }
                        used = true;
                        let Some(ci) = cheads.get(k).and_then(|h| heads.iter().position(|x| x == h))
                        else {
                            all = false;
                            break;
                        };
                        let cell = row.get(ci).cloned().unwrap_or(Value::Empty);
                        if !matches_cond(&cell, &Value::Text(c.clone())) {
                            all = false;
                            break;
                        }
                    }
                    if all && used {
                        any = true;
                        break;
                    }
                }
                if any {
                    hits.push(row.get(fi).cloned().unwrap_or(Value::Empty));
                }
            }
            let nums: Vec<f64> = hits.iter().filter(|v| !v.is_empty()).map(|v| v.as_number()).collect();
            return Ok(match name {
                "DSUM" => Value::Number(nums.iter().sum()),
                "DAVERAGE" => {
                    if nums.is_empty() {
                        Value::Error("#DIV/0!".into())
                    } else {
                        Value::Number(nums.iter().sum::<f64>() / nums.len() as f64)
                    }
                }
                "DCOUNT" => Value::Number(nums.len() as f64),
                "DMAX" => nums.iter().cloned().fold(None::<f64>, |m, v| Some(m.map_or(v, |x: f64| x.max(v))))
                    .map(Value::Number).unwrap_or(Value::Number(0.0)),
                "DMIN" => nums.iter().cloned().fold(None::<f64>, |m, v| Some(m.map_or(v, |x: f64| x.min(v))))
                    .map(Value::Number).unwrap_or(Value::Number(0.0)),
                // DGET は**1件でなければ黙って返さない**(Excel と同じ)
                _ => match hits.len() {
                    0 => Value::Error("#VALUE!".into()),
                    1 => hits[0].clone(),
                    _ => Value::Error("#NUM!".into()),
                },
            });
        }
        "XLOOKUP" => {
            // XLOOKUP(探す値, 探す範囲, 返す範囲, [見つからないとき]) — 完全一致
            let key = args.first().map(|g| g.first()).unwrap_or(Value::Empty);
            let hay = args.get(1).map(|g| g.values()).unwrap_or(&[]);
            let ret = args.get(2).map(|g| g.values()).unwrap_or(&[]);
            if hay.is_empty() || hay.len() != ret.len() {
                return Ok(Value::Error("#VALUE!".into()));
            }
            let same = |v: &Value| match (v, &key) {
                (Value::Number(x), Value::Number(y)) => (x - y).abs() < 1e-9,
                _ => v.display() == key.display(),
            };
            return Ok(match hay.iter().position(same) {
                Some(i) => ret[i].clone(),
                None => args.get(3).map(|g| g.first()).unwrap_or(Value::Error("#N/A".into())),
            });
        }
        "COUNTIFS" | "SUMIFS" | "AVERAGEIFS" | "MINIFS" | "MAXIFS" => {
            // SUMIFS(合計範囲, 条件範囲1, 条件1, …) / COUNTIFS(条件範囲1, 条件1, …)
            // 条件は**行ごとに全部**合ったものだけ数える(範囲は同じ大きさ)
            let (vals, pairs) = if name == "COUNTIFS" {
                (None, &args[..])
            } else {
                (args.first(), args.get(1..).unwrap_or(&[]))
            };
            if pairs.is_empty() || pairs.len() % 2 != 0 {
                return Ok(Value::Error("#VALUE!".into()));
            }
            let n = pairs[0].values().len();
            if pairs.chunks(2).any(|c| c[0].values().len() != n)
                || vals.map(|v| v.values().len() != n).unwrap_or(false)
            {
                return Ok(Value::Error("#VALUE!".into()));
            }
            let hit = |i: usize| {
                pairs.chunks(2).all(|c| matches_cond(&c[0].values()[i], &c[1].first()))
            };
            let picked: Vec<f64> = (0..n)
                .filter(|i| hit(*i))
                .map(|i| vals.map(|v| v.values()[i].as_number()).unwrap_or(0.0))
                .collect();
            return Ok(match name {
                "COUNTIFS" => Value::Number(picked.len() as f64),
                "SUMIFS" => Value::Number(picked.iter().sum()),
                "AVERAGEIFS" => {
                    if picked.is_empty() {
                        Value::Error("#DIV/0!".into())
                    } else {
                        Value::Number(picked.iter().sum::<f64>() / picked.len() as f64)
                    }
                }
                // Excel の約束: 1件も合わなければ 0
                "MINIFS" => Value::Number(picked.iter().cloned().reduce(f64::min).unwrap_or(0.0)),
                _ => Value::Number(picked.iter().cloned().reduce(f64::max).unwrap_or(0.0)),
            });
        }
        "SUMIF" | "AVERAGEIF" => {
            // SUMIF(条件を見る範囲, 条件, [足す範囲])
            // AVERAGEIF(条件を見る範囲, 条件, [平均する範囲])
            // 3つ目を省いたら、条件を見た範囲そのものを足す(平均する)
            let rng = args.first().map(|g| g.values()).unwrap_or(&[]);
            let cond = args.get(1).map(|g| g.first()).unwrap_or(Value::Empty);
            let tgt = args.get(2).map(|g| g.values()).unwrap_or(rng);
            if tgt.len() != rng.len() {
                return Ok(Value::Error("#VALUE!".into()));
            }
            // 範囲にエラーがあればそれを返す(黙って0として数えない)
            if let Some(e) =
                rng.iter().chain(tgt).chain([&cond]).find(|v| matches!(v, Value::Error(_)))
            {
                return Ok(e.clone());
            }
            let picked: Vec<f64> = (0..rng.len())
                .filter(|i| matches_cond(&rng[*i], &cond))
                .map(|i| tgt[i].as_number())
                .collect();
            return Ok(if name == "SUMIF" {
                Value::Number(picked.iter().sum())
            } else if picked.is_empty() {
                Value::Error("#DIV/0!".into())
            } else {
                Value::Number(picked.iter().sum::<f64>() / picked.len() as f64)
            });
        }
        "SUMPRODUCT" => {
            // 同じ大きさの範囲を要素ごとに掛けて、全部足す
            let n = args.first().map(|g| g.values().len()).unwrap_or(0);
            if n == 0 || args.iter().any(|g| g.values().len() != n) {
                return Ok(Value::Error("#VALUE!".into()));
            }
            let mut total = 0.0;
            for i in 0..n {
                total += args.iter().map(|g| g.values()[i].as_number()).product::<f64>();
            }
            return Ok(Value::Number(total));
        }
        "LARGE" | "SMALL" => {
            // 大きい方(小さい方)から k 番目。数だけを見る
            let mut ns: Vec<f64> = args
                .first()
                .map(|g| {
                    g.values()
                        .iter()
                        .filter(|v| matches!(v, Value::Number(_)))
                        .map(|v| v.as_number())
                        .collect()
                })
                .unwrap_or_default();
            let k = args.get(1).map(|g| g.first().as_number()).unwrap_or(0.0) as usize;
            if k == 0 || k > ns.len() {
                return Ok(Value::Error("#NUM!".into()));
            }
            ns.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
            return Ok(Value::Number(if name == "LARGE" { ns[ns.len() - k] } else { ns[k - 1] }));
        }
        "PERCENTILE" | "PERCENTILE.INC" | "QUARTILE" | "QUARTILE.INC" => {
            // 百分位(直線補間 — Excel の PERCENTILE.INC と同じ)
            let mut ns: Vec<f64> = args
                .first()
                .map(|g| {
                    g.values()
                        .iter()
                        .filter(|v| matches!(v, Value::Number(_)))
                        .map(|v| v.as_number())
                        .collect()
                })
                .unwrap_or_default();
            let k = args.get(1).map(|g| g.first().as_number()).unwrap_or(f64::NAN);
            let k = if name.starts_with("QUARTILE") { k / 4.0 } else { k };
            if ns.is_empty() || !(0.0..=1.0).contains(&k) {
                return Ok(Value::Error("#NUM!".into()));
            }
            ns.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
            let pos = k * (ns.len() - 1) as f64;
            let (lo, frac) = (pos.floor() as usize, pos.fract());
            let hi = (lo + 1).min(ns.len() - 1);
            return Ok(Value::Number(ns[lo] + (ns[hi] - ns[lo]) * frac));
        }
        "CORREL" | "SLOPE" | "INTERCEPT" | "FORECAST" | "FORECAST.LINEAR" => {
            // 対で見る統計。両方が数の行だけを使う(Excel と同じ)
            let fc = name.starts_with("FORECAST");
            let (ys, xs) = if fc {
                (args.get(1), args.get(2))
            } else {
                (args.first(), args.get(1))
            };
            let (ys, xs) = (
                ys.map(|g| g.values()).unwrap_or(&[]),
                xs.map(|g| g.values()).unwrap_or(&[]),
            );
            if ys.len() != xs.len() {
                return Ok(Value::Error("#N/A".into()));
            }
            let pairs: Vec<(f64, f64)> = ys
                .iter()
                .zip(xs)
                .filter(|(y, x)| {
                    matches!(y, Value::Number(_)) && matches!(x, Value::Number(_))
                })
                .map(|(y, x)| (y.as_number(), x.as_number()))
                .collect();
            let n = pairs.len() as f64;
            if pairs.is_empty() {
                return Ok(Value::Error("#DIV/0!".into()));
            }
            let (my, mx) = (
                pairs.iter().map(|p| p.0).sum::<f64>() / n,
                pairs.iter().map(|p| p.1).sum::<f64>() / n,
            );
            let sxy: f64 = pairs.iter().map(|(y, x)| (x - mx) * (y - my)).sum();
            let sxx: f64 = pairs.iter().map(|(_, x)| (x - mx) * (x - mx)).sum();
            let syy: f64 = pairs.iter().map(|(y, _)| (y - my) * (y - my)).sum();
            return Ok(match name {
                "CORREL" => {
                    if sxx == 0.0 || syy == 0.0 {
                        Value::Error("#DIV/0!".into())
                    } else {
                        Value::Number(sxy / (sxx * syy).sqrt())
                    }
                }
                _ => {
                    if sxx == 0.0 {
                        Value::Error("#DIV/0!".into())
                    } else {
                        let slope = sxy / sxx;
                        match name {
                            "SLOPE" => Value::Number(slope),
                            "INTERCEPT" => Value::Number(my - slope * mx),
                            _ => {
                                let x = args.first().map(|g| g.first().as_number())
                                    .unwrap_or(0.0);
                                Value::Number(my - slope * mx + slope * x)
                            }
                        }
                    }
                }
            });
        }
        "RANK" | "RANK.EQ" | "RANK.AVG" => {
            // RANK(値, 範囲, [順序]) — 省略は大きい方が1位。
            // .EQ は同値同順位、.AVG は同値の順位の平均
            let x = args.first().map(|g| g.first().as_number()).unwrap_or(0.0);
            let ns: Vec<f64> = args
                .get(1)
                .map(|g| {
                    g.values()
                        .iter()
                        .filter(|v| matches!(v, Value::Number(_)))
                        .map(|v| v.as_number())
                        .collect()
                })
                .unwrap_or_default();
            let asc = args.get(2).map(|g| g.first().as_number() != 0.0).unwrap_or(false);
            let ties = ns.iter().filter(|v| (*v - x).abs() < 1e-9).count();
            if ties == 0 {
                return Ok(Value::Error("#N/A".into()));
            }
            let better =
                ns.iter().filter(|v| if asc { **v < x - 1e-9 } else { **v > x + 1e-9 }).count();
            return Ok(Value::Number(if name == "RANK.AVG" {
                // 同値が k 個なら (r + r+1 + … + r+k-1) / k
                (better + 1) as f64 + (ties - 1) as f64 / 2.0
            } else {
                (better + 1) as f64
            }));
        }
        "IRR" => {
            // IRR(額の並び, [推定値]) — 符号の変わる区間を探して挟み撃ち(反復解)
            let vals: Vec<f64> = args
                .first()
                .map(|g| {
                    g.values()
                        .iter()
                        .filter(|v| matches!(v, Value::Number(_)))
                        .map(|v| v.as_number())
                        .collect()
                })
                .unwrap_or_default();
            let f = |r: f64| -> f64 {
                vals.iter().enumerate().map(|(i, v)| v / (1.0 + r).powi(i as i32)).sum()
            };
            return Ok(match bisect(&f, -0.9999, 10.0) {
                Some(r) => Value::Number(r),
                None => Value::Error("#NUM!".into()),
            });
        }
        "LOOKUP" => {
            // LOOKUP(値, 探す並び, [結果の並び]) — 昇順前提の古典。
            // 「値以下でいちばん大きいもの」を選ぶ(Excel と同じ)
            let key = args.first().map(|g| g.first()).unwrap_or(Value::Empty);
            let hay = args.get(1).map(|g| g.values()).unwrap_or(&[]);
            let ret = args.get(2).map(|g| g.values()).unwrap_or(hay);
            if hay.is_empty() || hay.len() != ret.len() {
                return Ok(Value::Error("#N/A".into()));
            }
            let mut hit: Option<usize> = None;
            for (i, v) in hay.iter().enumerate() {
                let le = match (v, &key) {
                    (Value::Number(a), Value::Number(b)) => a <= b,
                    (Value::Text(a), Value::Text(b)) => a.as_str() <= b.as_str(),
                    _ => false,
                };
                if le {
                    hit = Some(i);
                }
            }
            return Ok(hit
                .and_then(|i| ret.get(i).cloned())
                .unwrap_or(Value::Error("#N/A".into())));
        }
        "SUBTOTAL" | "AGGREGATE" => {
            // SUBTOTAL(集計番号, 範囲…) — オートフィルターと「小計」が
            // 自動で埋め込む、実物のファイル頻出の関数。
            // **101〜111(手で隠した行を飛ばす)は呼ぶ側で範囲から抜いてある**
            // (P::atom の二度読み。ここへ来る値は既に飛ばした後)。
            // 絞り込みで隠れた行は app 側の状態なので、まだ数に入る(在庫)
            let f = args.first().map(|g| g.first().as_number()).unwrap_or(0.0) as i64;
            let f = if f > 100 { f - 100 } else { f };
            // AGGREGATE は第2引数が「無視の指定」— 読み飛ばす
            let skip = if name == "AGGREGATE" { 2 } else { 1 };
            let ns: Vec<f64> = args
                .get(skip..)
                .unwrap_or(&[])
                .iter()
                .flat_map(|g| g.values())
                .filter(|v| matches!(v, Value::Number(_)))
                .map(|v| v.as_number())
                .collect();
            let cnt_a = args
                .get(skip..)
                .unwrap_or(&[])
                .iter()
                .flat_map(|g| g.values())
                .filter(|v| !v.is_empty())
                .count();
            let n = ns.len() as f64;
            let mean = if ns.is_empty() { 0.0 } else { ns.iter().sum::<f64>() / n };
            let ss: f64 = ns.iter().map(|x| (x - mean) * (x - mean)).sum();
            return Ok(match f {
                1 if !ns.is_empty() => Value::Number(mean),
                2 => Value::Number(n),
                3 => Value::Number(cnt_a as f64),
                4 => Value::Number(ns.iter().cloned().reduce(f64::max).unwrap_or(0.0)),
                5 => Value::Number(ns.iter().cloned().reduce(f64::min).unwrap_or(0.0)),
                6 => Value::Number(ns.iter().product()),
                7 if ns.len() >= 2 => Value::Number((ss / (n - 1.0)).sqrt()),
                8 if !ns.is_empty() => Value::Number((ss / n).sqrt()),
                9 => Value::Number(ns.iter().sum()),
                10 if ns.len() >= 2 => Value::Number(ss / (n - 1.0)),
                11 if !ns.is_empty() => Value::Number(ss / n),
                1 | 7 | 8 | 10 | 11 => Value::Error("#DIV/0!".into()),
                _ => Value::Error("#VALUE!".into()),
            });
        }
        _ => {}
    }
    let a: Vec<Value> = args.iter().flat_map(|g| g.values().iter().cloned()).collect();
    // 引数にエラーがあればそれを返す(黙って0として数えない)。
    // ただしエラーを受けて働く関数(IFERROR・ISERROR・ISBLANK・IF)と、
    // 選ばなかった枝のエラーを踏んではいけない関数(IFS・SWITCH・CHOOSE)は素通しする
    if !matches!(
        name,
        "IFERROR" | "ISERROR" | "ISBLANK" | "IF" | "IFS" | "SWITCH" | "CHOOSE"
            | "ISNUMBER" | "ISTEXT" // エラーは数でも文字でもない → FALSE と答える
            | "IFNA" | "ISNA" | "ISERR" | "ISLOGICAL" | "ISNONTEXT" | "TYPE" | "N" | "T"
    ) {
        if let Some(e) = a.iter().find(|v| matches!(v, Value::Error(_))) {
            return Ok(e.clone());
        }
    }
    let nums = |a: &[Value]| -> Vec<f64> {
        a.iter().filter(|v| !v.is_empty()).map(|v| v.as_number()).collect()
    };
    Ok(match name {
        "SUM" => Value::Number(nums(&a).iter().sum()),
        "AVERAGE" => {
            let n = nums(&a);
            if n.is_empty() {
                Value::Error("#DIV/0!".into())
            } else {
                Value::Number(n.iter().sum::<f64>() / n.len() as f64)
            }
        }
        "COUNT" => Value::Number(
            a.iter().filter(|v| matches!(v, Value::Number(_))).count() as f64),
        "COUNTA" => Value::Number(a.iter().filter(|v| !v.is_empty()).count() as f64),
        "MIN" => nums(&a).into_iter().reduce(f64::min).map(Value::Number)
            .unwrap_or(Value::Number(0.0)),
        "MAX" => nums(&a).into_iter().reduce(f64::max).map(Value::Number)
            .unwrap_or(Value::Number(0.0)),
        // 事務でよく使うもの。無いと「関数が違う」で止まる
        "ROUNDDOWN" | "TRUNC" => {
            let n = nums(&a);
            let (v, d) = (n.first().copied().unwrap_or(0.0), n.get(1).copied().unwrap_or(0.0));
            let f = 10f64.powi(d as i32);
            Value::Number((v * f).trunc() / f)
        }
        "ROUNDUP" => {
            let n = nums(&a);
            let (v, d) = (n.first().copied().unwrap_or(0.0), n.get(1).copied().unwrap_or(0.0));
            let f = 10f64.powi(d as i32);
            // 0 から遠ざかる向きに上げる(負の数で符号が入れ替わらないように)
            Value::Number(if v < 0.0 { (v * f).floor() / f } else { (v * f).ceil() / f })
        }
        "COUNTIF" => {
            let cond = a.last().cloned().unwrap_or(Value::Empty);
            let n = a[..a.len().saturating_sub(1)].iter().filter(|v| matches_cond(v, &cond)).count();
            Value::Number(n as f64)
        }
        "PRODUCT" => Value::Number(nums(&a).iter().product()),
        "MOD" => {
            let n = nums(&a);
            let (x, y) = (n.first().copied().unwrap_or(0.0), n.get(1).copied().unwrap_or(0.0));
            if y == 0.0 {
                // 0 で割った答えは無い。黙って 0 を返さない
                Value::Error("#DIV/0!".into())
            } else {
                Value::Number(x - y * (x / y).floor())
            }
        }
        "POWER" => {
            let n = nums(&a);
            Value::Number(n.first().copied().unwrap_or(0.0)
                .powf(n.get(1).copied().unwrap_or(0.0)))
        }
        "SQRT" => {
            let v = nums(&a).first().copied().unwrap_or(0.0);
            if v < 0.0 { Value::Error("#NUM!".into()) } else { Value::Number(v.sqrt()) }
        }
        "LEFT" | "RIGHT" | "MID" => {
            let s = a.first().map(|v| v.display()).unwrap_or_default();
            let ch: Vec<char> = s.chars().collect();
            let n = |i: usize| a.get(i).map(|v| v.as_number() as usize).unwrap_or(0);
            Value::Text(match name {
                "LEFT" => ch.iter().take(n(1).min(ch.len())).collect(),
                "RIGHT" => ch.iter().skip(ch.len().saturating_sub(n(1))).collect(),
                // MID は1始まり(表計算の約束)
                _ => ch.iter().skip(n(1).saturating_sub(1)).take(n(2)).collect(),
            })
        }
        "TRIM" => Value::Text(a.first().map(|v| v.display()).unwrap_or_default().trim().to_string()),
        "UPPER" => Value::Text(a.first().map(|v| v.display()).unwrap_or_default().to_uppercase()),
        "LOWER" => Value::Text(a.first().map(|v| v.display()).unwrap_or_default().to_lowercase()),
        "ISBLANK" => Value::Bool(a.first().map(|v| v.is_empty()).unwrap_or(true)),
        "ISERROR" => Value::Bool(matches!(a.first(), Some(Value::Error(_)))),
        "IFERROR" => {
            // 第1引数がエラーなら第2引数(無ければ空)に落とす
            match a.first() {
                Some(Value::Error(_)) => a.get(1).cloned().unwrap_or(Value::Empty),
                v => v.cloned().unwrap_or(Value::Empty),
            }
        }
        "ABS" => Value::Number(a.first().map(|v| v.as_number().abs()).unwrap_or(0.0)),
        "ROUND" => {
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let d = a.get(1).map(|v| v.as_number()).unwrap_or(0.0) as i32;
            let f = 10f64.powi(d);
            Value::Number((x * f).round() / f)
        }
        "INT" => Value::Number(a.first().map(|v| v.as_number().floor()).unwrap_or(0.0)),
        "IF" => {
            // 条件のエラーは伝える。選ばなかった側のエラーは踏まない
            // (引数は先に評価済みなので、値の段階で無視するのが遅延評価の代わり)
            if let Some(e @ Value::Error(_)) = a.first() {
                return Ok(e.clone());
            }
            let c = matches!(a.first(), Some(Value::Bool(true)))
                || a.first().map(|v| v.as_number() != 0.0).unwrap_or(false);
            if c {
                a.get(1).cloned().unwrap_or(Value::Bool(true))
            } else {
                a.get(2).cloned().unwrap_or(Value::Bool(false))
            }
        }
        "AND" => Value::Bool(a.iter().all(|v| v.as_number() != 0.0
            || matches!(v, Value::Bool(true)))),
        "OR" => Value::Bool(a.iter().any(|v| v.as_number() != 0.0
            || matches!(v, Value::Bool(true)))),
        "NOT" => Value::Bool(!(a.first().map(|v| v.as_number() != 0.0).unwrap_or(false))),
        "CONCATENATE" | "CONCAT" => Value::Text(a.iter().map(|v| v.display()).collect()),
        // 引数つきの TRUE()/FALSE() も本物の Excel ファイルには出てくる
        "TRUE" => Value::Bool(true),
        "FALSE" => Value::Bool(false),
        // ---- 日付と時刻(値は Excel の通し番号 1899-12-30 起点)----
        "TODAY" => Value::Number(today_serial(ep).0),
        "NOW" => {
            let (d, f) = today_serial(ep);
            Value::Number(d + f)
        }
        "DATE" => {
            let g = |i: usize| a.get(i).map(|v| v.as_number() as i64).unwrap_or(0);
            Value::Number(date_serial_at(g(0), g(1), g(2), ep) as f64)
        }
        "YEAR" | "MONTH" | "DAY" => {
            let serial = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let (y, m, d) = civil_from_days(serial - ep);
            Value::Number(match name {
                "YEAR" => y,
                "MONTH" => m,
                _ => d,
            } as f64)
        }
        "WEEKDAY" => {
            // Excel の既定(1=日曜)。通し番号 1(1900-01-01)は月曜
            let serial = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            Value::Number(weekday0(serial, ep) as f64 + 1.0)
        }
        "TIME" => {
            let g = |i: usize| a.get(i).map(|v| v.as_number()).unwrap_or(0.0);
            let secs = g(0) * 3600.0 + g(1) * 60.0 + g(2);
            Value::Number(secs.rem_euclid(86400.0) / 86400.0)
        }
        "HOUR" | "MINUTE" | "SECOND" => {
            let serial = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let total = (serial.rem_euclid(1.0) * 86400.0).round() as i64;
            Value::Number(match name {
                "HOUR" => total / 3600 % 24,
                "MINUTE" => total / 60 % 60,
                _ => total % 60,
            } as f64)
        }
        "DATEVALUE" => {
            // "2026/8/5"・"2026-8-5"・"2026年8月5日" を通し番号に
            let s = a.first().map(|v| v.display()).unwrap_or_default();
            let t = s.trim().replace(['年', '月'], "/");
            let t = t.trim_end_matches('日');
            let parts: Vec<i64> =
                t.split(['/', '-']).filter_map(|p| p.trim().parse().ok()).collect();
            match parts.as_slice() {
                [y, m, d] => Value::Number(date_serial_at(*y, *m, *d, ep) as f64),
                _ => Value::Error("#VALUE!".into()),
            }
        }
        "EDATE" | "EOMONTH" => {
            // n ヶ月あと(前)。EDATE は同じ日(無ければ月末)、EOMONTH はその月末
            let serial = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let months = a.get(1).map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let (y, m, d) = civil_from_days(serial - ep);
            let total = y * 12 + (m - 1) + months;
            let (ny, nm) = (total.div_euclid(12), total.rem_euclid(12) + 1);
            let month_end = date_serial_at(ny, nm + 1, 1, ep) - 1; // 13月は翌年1月に正しく繰り上がる
            Value::Number(match name {
                "EOMONTH" => month_end,
                _ => date_serial_at(ny, nm, d, ep).min(month_end),
            } as f64)
        }
        "DATEDIF" => {
            // DATEDIF(始, 終, 単位) — 単位は Y/M/D/YM/MD/YD
            let s = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let e = a.get(1).map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let unit = a.get(2).map(|v| v.display().to_uppercase()).unwrap_or_default();
            if e < s {
                return Ok(Value::Error("#NUM!".into()));
            }
            let (sy, sm, sd) = civil_from_days(s - ep);
            let (ey, em, ed) = civil_from_days(e - ep);
            let borrow = (em, ed) < (sm, sd);
            let months = ey * 12 + em - (sy * 12 + sm) - i64::from(ed < sd);
            Value::Number(match unit.as_str() {
                "Y" => ey - sy - i64::from(borrow),
                "M" => months,
                "D" => e - s,
                "YM" => months.rem_euclid(12),
                "YD" => {
                    // 年を無視した日数: 始の年を終の直前まで進めて引く
                    let anchor = date_serial_at(ey - i64::from(borrow), sm, sd, ep);
                    e - anchor
                }
                "MD" => {
                    // 月を無視した日数: 始の「日」を終の月(足りなければ前月)に置いて引く
                    let (ay, am) = if ed >= sd {
                        (ey, em)
                    } else if em == 1 {
                        (ey - 1, 12)
                    } else {
                        (ey, em - 1)
                    };
                    e - date_serial_at(ay, am, sd, ep)
                }
                _ => return Ok(Value::Error("#VALUE!".into())),
            } as f64)
        }
        "WORKDAY" => {
            // WORKDAY(始, 日数, [休みの日…]) — 土日と休みを飛ばして数える
            let mut cur = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let days = a.get(1).map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let holidays: HashSet<i64> =
                a.get(2..).unwrap_or(&[]).iter().map(|v| v.as_number() as i64).collect();
            if days.abs() > 1_000_000 {
                return Ok(Value::Error("#NUM!".into()));
            }
            let step = if days < 0 { -1 } else { 1 };
            let mut left = days.abs();
            while left > 0 {
                cur += step;
                let w = weekday0(cur, ep);
                if w != 0 && w != 6 && !holidays.contains(&cur) {
                    left -= 1;
                }
            }
            Value::Number(cur as f64)
        }
        "NETWORKDAYS" => {
            // NETWORKDAYS(始, 終, [休みの日…]) — 両端を含む平日の数
            let s = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let e = a.get(1).map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let holidays: HashSet<i64> =
                a.get(2..).unwrap_or(&[]).iter().map(|v| v.as_number() as i64).collect();
            let (lo, hi) = (s.min(e), s.max(e));
            if hi - lo > 10_000_000 {
                return Ok(Value::Error("#NUM!".into()));
            }
            let n = (lo..=hi)
                .filter(|d| {
                    let w = weekday0(*d, ep);
                    w != 0 && w != 6 && !holidays.contains(d)
                })
                .count() as i64;
            Value::Number(if e < s { -n } else { n } as f64)
        }
        // ---- 財務(閉じた式で解けるものだけ。RATE のような反復解は持たない)----
        "PMT" | "PV" | "FV" | "NPER" => {
            let g = |i: usize| a.get(i).map(|v| v.as_number()).unwrap_or(0.0);
            let rate = g(0);
            match name {
                "PMT" => {
                    let (nper, pv, fv) = (g(1), g(2), g(3));
                    if nper == 0.0 {
                        Value::Error("#DIV/0!".into())
                    } else if rate == 0.0 {
                        Value::Number(-(pv + fv) / nper)
                    } else {
                        let k = (1.0 + rate).powf(nper);
                        Value::Number(-(pv * k + fv) * rate / (k - 1.0))
                    }
                }
                "PV" => {
                    let (nper, pmt, fv) = (g(1), g(2), g(3));
                    if rate == 0.0 {
                        Value::Number(-(pmt * nper + fv))
                    } else {
                        let k = (1.0 + rate).powf(nper);
                        Value::Number(-(pmt * (k - 1.0) / rate + fv) / k)
                    }
                }
                "FV" => {
                    let (nper, pmt, pv) = (g(1), g(2), g(3));
                    if rate == 0.0 {
                        Value::Number(-(pv + pmt * nper))
                    } else {
                        let k = (1.0 + rate).powf(nper);
                        Value::Number(-(pv * k + pmt * (k - 1.0) / rate))
                    }
                }
                _ => {
                    // NPER(rate, pmt, pv, [fv])
                    let (pmt, pv, fv) = (g(1), g(2), g(3));
                    if rate == 0.0 {
                        if pmt == 0.0 {
                            Value::Error("#DIV/0!".into())
                        } else {
                            Value::Number(-(pv + fv) / pmt)
                        }
                    } else {
                        let x = (pmt / rate - fv) / (pv + pmt / rate);
                        if x <= 0.0 {
                            Value::Error("#NUM!".into())
                        } else {
                            Value::Number(x.ln() / (1.0 + rate).ln())
                        }
                    }
                }
            }
        }
        "LEN" => Value::Number(a.first().map(|v| v.display().chars().count())
            .unwrap_or(0) as f64),
        // ---- 選ぶ関数(選ばなかった枝のエラーは踏まない — IF と同じ考え)----
        "IFS" => {
            // IFS(条件1, 値1, 条件2, 値2, …) — 最初に真になった対の値
            let mut out = Value::Error("#N/A".into());
            let mut i = 0;
            while let Some(c) = a.get(i) {
                if let Value::Error(_) = c {
                    out = c.clone();
                    break;
                }
                if c.as_number() != 0.0 {
                    out = a.get(i + 1).cloned().unwrap_or(Value::Empty);
                    break;
                }
                i += 2;
            }
            out
        }
        "SWITCH" => {
            // SWITCH(式, 候補1, 値1, …, [どれでもないとき])
            let key = a.first().cloned().unwrap_or(Value::Empty);
            if let Value::Error(_) = key {
                return Ok(key);
            }
            let rest = a.get(1..).unwrap_or(&[]);
            let mut out = if rest.len() % 2 == 1 {
                rest.last().cloned().unwrap_or(Value::Empty)
            } else {
                Value::Error("#N/A".into())
            };
            let mut i = 0;
            while i + 1 < rest.len() {
                if !matches!(rest[i], Value::Error(_)) && rest[i].display() == key.display() {
                    out = rest[i + 1].clone();
                    break;
                }
                i += 2;
            }
            out
        }
        "CHOOSE" => {
            // CHOOSE(番号, 値1, 値2, …) — 番号は1起点
            let idx = a.first().cloned().unwrap_or(Value::Empty);
            if let Value::Error(_) = idx {
                return Ok(idx);
            }
            let i = idx.as_number() as usize;
            if i == 0 || i >= a.len() {
                Value::Error("#VALUE!".into())
            } else {
                a[i].clone()
            }
        }
        // ---- 文字列 ----
        "TEXT" => {
            // TEXT(値, 表示形式) — セルの表示と同じ描き方で文字列にする
            let v = a.first().cloned().unwrap_or(Value::Empty);
            let code = a.get(1).map(|v| v.display()).unwrap_or_default();
            Value::Text(format_value(&v, Some(&code), date1904))
        }
        "REPLACE" => {
            // REPLACE(文字列, 開始位置, 文字数, 置く文字)。**位置は1から**、
            // 数え方は文字(バイトではない) — 日本語で崩れないように
            let src: Vec<char> = a.first().map(|v| v.display()).unwrap_or_default().chars().collect();
            let start = a.get(1).map(|v| v.as_number()).unwrap_or(1.0);
            let n = a.get(2).map(|v| v.as_number()).unwrap_or(0.0);
            let new = a.get(3).map(|v| v.display()).unwrap_or_default();
            if start < 1.0 || n < 0.0 {
                return Ok(Value::Error("#VALUE!".into()));
            }
            let i = ((start as usize) - 1).min(src.len());
            let j = (i + n as usize).min(src.len());
            let mut out: String = src[..i].iter().collect();
            out.push_str(&new);
            out.extend(&src[j..]);
            Value::Text(out)
        }
        "SUBSTITUTE" => {
            // SUBSTITUTE(文字列, 探す, 置く, [何個目だけ])
            let s = a.first().map(|v| v.display()).unwrap_or_default();
            let old = a.get(1).map(|v| v.display()).unwrap_or_default();
            let new = a.get(2).map(|v| v.display()).unwrap_or_default();
            if old.is_empty() {
                return Ok(Value::Text(s));
            }
            match a.get(3) {
                None => Value::Text(s.replace(&old, &new)),
                Some(nth) => {
                    let n = nth.as_number() as usize;
                    match s.match_indices(&old).nth(n.saturating_sub(1)) {
                        Some((i, _)) if n >= 1 => {
                            let mut t = s.clone();
                            t.replace_range(i..i + old.len(), &new);
                            Value::Text(t)
                        }
                        _ => Value::Text(s),
                    }
                }
            }
        }
        "FIND" | "SEARCH" => {
            // FIND(探す, 文字列, [開始]) — 1起点の文字番号。SEARCH は大文字小文字を見ない
            let (mut needle, mut hay) = (
                a.first().map(|v| v.display()).unwrap_or_default(),
                a.get(1).map(|v| v.display()).unwrap_or_default(),
            );
            if name == "SEARCH" {
                needle = needle.to_lowercase();
                hay = hay.to_lowercase();
            }
            let start = (a.get(2).map(|v| v.as_number()).unwrap_or(1.0) as usize).max(1);
            let ch: Vec<char> = hay.chars().collect();
            let from: String = ch.iter().skip(start - 1).collect();
            match from.find(&needle) {
                Some(b) => {
                    // バイト位置 → 文字番号(1起点、開始位置ぶんを足し戻す)
                    let chars_before = from[..b].chars().count();
                    Value::Number((start + chars_before) as f64)
                }
                None => Value::Error("#VALUE!".into()),
            }
        }
        "VALUE" => {
            // 「¥1,234」のような表示も数に戻す(記号と桁区切りを外して読む)
            let s = a.first().map(|v| v.display()).unwrap_or_default();
            let t: String =
                s.trim().chars().filter(|c| !matches!(c, ',' | '¥' | '\u{a0}' | ' ')).collect();
            match t.trim_end_matches('%').parse::<f64>() {
                Ok(n) if t.ends_with('%') => Value::Number(n / 100.0),
                Ok(n) => Value::Number(n),
                Err(_) => Value::Error("#VALUE!".into()),
            }
        }
        "TEXTJOIN" => {
            // TEXTJOIN(区切り, 空を飛ばすか, 値…)
            let delim = a.first().map(|v| v.display()).unwrap_or_default();
            let skip_empty = a.get(1).map(|v| v.as_number() != 0.0).unwrap_or(true);
            let parts: Vec<String> = a
                .get(2..)
                .unwrap_or(&[])
                .iter()
                .map(|v| v.display())
                .filter(|s| !(skip_empty && s.is_empty())) // 空文字も「空」と見る
                .collect();
            Value::Text(parts.join(&delim))
        }
        "TEXTBEFORE" | "TEXTAFTER" => {
            // TEXTBEFORE(文字, 区切り, [何番目], [見つからない時の値])
            // 何番目が負なら**後ろから**数える(Excel と同じ)。
            // 見つからなければ #N/A(4つ目を渡していればその値)
            let s = a.first().map(|v| v.display()).unwrap_or_default();
            let d = a.get(1).map(|v| v.display()).unwrap_or_default();
            let nth = a.get(2).map(|v| v.as_number() as i64).unwrap_or(1);
            let not_found = a.get(3).cloned();
            if d.is_empty() || nth == 0 {
                return Ok(Value::Error("#VALUE!".into()));
            }
            let hits: Vec<usize> = s.match_indices(d.as_str()).map(|(i, _)| i).collect();
            let idx = if nth > 0 {
                hits.get(nth as usize - 1).copied()
            } else {
                hits.iter().rev().nth((-nth) as usize - 1).copied()
            };
            match idx {
                Some(i) => Value::Text(if name == "TEXTBEFORE" {
                    s[..i].to_string()
                } else {
                    s[i + d.len()..].to_string()
                }),
                None => not_found.unwrap_or(Value::Error("#N/A".into())),
            }
        }
        "REPT" => {
            let s = a.first().map(|v| v.display()).unwrap_or_default();
            let n = a.get(1).map(|v| v.as_number()).unwrap_or(0.0);
            if n < 0.0 || s.chars().count() as f64 * n > 32767.0 {
                Value::Error("#VALUE!".into())
            } else {
                Value::Text(s.repeat(n as usize))
            }
        }
        "CHAR" | "UNICHAR" => {
            let n = a.first().map(|v| v.as_number()).unwrap_or(0.0) as u32;
            match char::from_u32(n) {
                Some(c) if n > 0 => Value::Text(c.to_string()),
                _ => Value::Error("#VALUE!".into()),
            }
        }
        "CODE" | "UNICODE" => {
            match a.first().map(|v| v.display()).unwrap_or_default().chars().next() {
                Some(c) => Value::Number(c as u32 as f64),
                None => Value::Error("#VALUE!".into()),
            }
        }
        // ---- 統計(第2段 2026-08-05)----
        "MEDIAN" => {
            let mut ns = nums(&a);
            if ns.is_empty() {
                Value::Error("#NUM!".into())
            } else {
                ns.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                let m = ns.len() / 2;
                Value::Number(if ns.len() % 2 == 1 { ns[m] } else { (ns[m - 1] + ns[m]) / 2.0 })
            }
        }
        "MODE" | "MODE.SNGL" => {
            // 最も多く現れる数。1度ずつしか現れないなら #N/A(Excel と同じ)
            let ns = nums(&a);
            let mut best: Option<(f64, usize)> = None;
            for (i, x) in ns.iter().enumerate() {
                let c = ns.iter().filter(|y| (*y - x).abs() < 1e-12).count();
                // 同数なら先に現れた方(Excel の MODE.SNGL の癖)
                let earlier = ns[..i].iter().any(|y| (y - x).abs() < 1e-12);
                if c > 1 && !earlier && best.map(|(_, bc)| c > bc).unwrap_or(true) {
                    best = Some((*x, c));
                }
            }
            best.map(|(x, _)| Value::Number(x)).unwrap_or(Value::Error("#N/A".into()))
        }
        "STDEV" | "STDEV.S" | "STDEVP" | "STDEV.P" | "VAR" | "VAR.S" | "VARP" | "VAR.P" => {
            let ns = nums(&a);
            let sample = matches!(name, "STDEV" | "STDEV.S" | "VAR" | "VAR.S");
            let need = if sample { 2 } else { 1 };
            if ns.len() < need {
                Value::Error("#DIV/0!".into())
            } else {
                let n = ns.len() as f64;
                let mean = ns.iter().sum::<f64>() / n;
                let ss: f64 = ns.iter().map(|x| (x - mean) * (x - mean)).sum();
                let var = ss / if sample { n - 1.0 } else { n };
                Value::Number(if name.starts_with("STDEV") { var.sqrt() } else { var })
            }
        }
        "COUNTBLANK" => Value::Number(a.iter().filter(|v| v.is_empty()).count() as f64),
        "SUMSQ" => Value::Number(nums(&a).iter().map(|x| x * x).sum()),
        // ---- 数学(第2段)----
        "FACT" => {
            let n = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            if !(0.0..=170.0).contains(&n) {
                Value::Error("#NUM!".into())
            } else {
                Value::Number((1..=n as i64).map(|i| i as f64).product())
            }
        }
        "COMBIN" | "PERMUT" => {
            let n = a.first().map(|v| v.as_number()).unwrap_or(0.0).floor();
            let k = a.get(1).map(|v| v.as_number()).unwrap_or(0.0).floor();
            if n < 0.0 || k < 0.0 || k > n || n > 1e15 {
                Value::Error("#NUM!".into())
            } else {
                let mut r = 1.0f64;
                for i in 0..k as i64 {
                    r *= n - i as f64;
                    if name == "COMBIN" {
                        r /= (i + 1) as f64;
                    }
                    if !r.is_finite() {
                        return Ok(Value::Error("#NUM!".into()));
                    }
                }
                Value::Number(r.round())
            }
        }
        "GCD" | "LCM" => {
            let ns: Vec<i64> = nums(&a).iter().map(|x| x.abs().floor() as i64).collect();
            if ns.is_empty() {
                return Ok(Value::Error("#VALUE!".into()));
            }
            fn gcd(a: i64, b: i64) -> i64 {
                if b == 0 { a } else { gcd(b, a % b) }
            }
            let mut acc: i64 = if name == "GCD" { 0 } else { 1 };
            for x in ns {
                if name == "GCD" {
                    acc = gcd(acc, x);
                } else {
                    let g = gcd(acc, x);
                    if g == 0 {
                        acc = 0;
                        continue;
                    }
                    match (acc / g).checked_mul(x) {
                        Some(v) => acc = v,
                        None => return Ok(Value::Error("#NUM!".into())),
                    }
                }
            }
            Value::Number(acc as f64)
        }
        "PI" => Value::Number(std::f64::consts::PI),
        "SIN" | "COS" | "TAN" | "SINH" | "COSH" | "TANH" | "EXP" | "DEGREES" | "RADIANS" => {
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            Value::Number(match name {
                "SIN" => x.sin(),
                "COS" => x.cos(),
                "TAN" => x.tan(),
                "SINH" => x.sinh(),
                "COSH" => x.cosh(),
                "TANH" => x.tanh(),
                "EXP" => x.exp(),
                "DEGREES" => x.to_degrees(),
                _ => x.to_radians(),
            })
        }
        "ASIN" | "ACOS" => {
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            if !(-1.0..=1.0).contains(&x) {
                Value::Error("#NUM!".into())
            } else {
                Value::Number(if name == "ASIN" { x.asin() } else { x.acos() })
            }
        }
        "ATAN" => Value::Number(a.first().map(|v| v.as_number()).unwrap_or(0.0).atan()),
        "ATAN2" => {
            // Excel の約束: ATAN2(x, y)(数学の atan2(y, x) と引数が逆順)
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let y = a.get(1).map(|v| v.as_number()).unwrap_or(0.0);
            if x == 0.0 && y == 0.0 {
                Value::Error("#DIV/0!".into())
            } else {
                Value::Number(y.atan2(x))
            }
        }
        "LN" | "LOG10" => {
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            if x <= 0.0 {
                Value::Error("#NUM!".into())
            } else {
                Value::Number(if name == "LN" { x.ln() } else { x.log10() })
            }
        }
        "LOG" => {
            // LOG(数, [底]) — 底の既定は 10
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let b = a.get(1).map(|v| v.as_number()).unwrap_or(10.0);
            if x <= 0.0 || b <= 0.0 || b == 1.0 {
                Value::Error("#NUM!".into())
            } else {
                Value::Number(x.log(b))
            }
        }
        "CEILING" | "FLOOR" => {
            // 基準値の倍数へ。符号が食い違う組は #NUM!(Excel の約束)
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let s = a.get(1).map(|v| v.as_number()).unwrap_or(1.0);
            if x > 0.0 && s < 0.0 {
                Value::Error("#NUM!".into())
            } else if s == 0.0 {
                if name == "CEILING" { Value::Number(0.0) } else { Value::Error("#DIV/0!".into()) }
            } else {
                let q = x / s;
                Value::Number(if name == "CEILING" { q.ceil() } else { q.floor() } * s)
            }
        }
        "MROUND" => {
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let m = a.get(1).map(|v| v.as_number()).unwrap_or(0.0);
            if m == 0.0 {
                Value::Number(0.0)
            } else if x.signum() * m.signum() < 0.0 {
                Value::Error("#NUM!".into())
            } else {
                Value::Number((x / m).round() * m)
            }
        }
        "EVEN" | "ODD" => {
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let sign = if x < 0.0 { -1.0 } else { 1.0 };
            let m = x.abs().ceil();
            let r = if name == "EVEN" {
                if m % 2.0 == 0.0 { m } else { m + 1.0 }
            } else if m % 2.0 == 1.0 {
                m
            } else {
                m + 1.0
            };
            Value::Number(sign * r)
        }
        "SIGN" => {
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            Value::Number(if x == 0.0 { 0.0 } else { x.signum() })
        }
        "RAND" => Value::Number(rand01()),
        "RANDBETWEEN" => {
            let lo = a.first().map(|v| v.as_number()).unwrap_or(0.0).ceil();
            let hi = a.get(1).map(|v| v.as_number()).unwrap_or(0.0).floor();
            if hi < lo {
                Value::Error("#NUM!".into())
            } else {
                Value::Number(lo + (rand01() * (hi - lo + 1.0)).floor().min(hi - lo))
            }
        }
        // ---- 情報(第2段)----
        "ISNUMBER" => Value::Bool(matches!(a.first(), Some(Value::Number(_)))),
        "ISTEXT" => Value::Bool(matches!(a.first(), Some(Value::Text(_)))),
        "ISEVEN" | "ISODD" => {
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0).abs().floor() as i64;
            Value::Bool((x % 2 == 0) == (name == "ISEVEN"))
        }
        // ---- Excel 互換の穴埋め(第4段 2026-08-05)----
        "IFNA" => match a.first() {
            Some(Value::Error(e)) if e == "#N/A" => a.get(1).cloned().unwrap_or(Value::Empty),
            v => v.cloned().unwrap_or(Value::Empty),
        },
        "NA" => Value::Error("#N/A".into()),
        "ISNA" => Value::Bool(matches!(a.first(), Some(Value::Error(e)) if e == "#N/A")),
        "ISERR" => Value::Bool(matches!(a.first(), Some(Value::Error(e)) if e != "#N/A")),
        "ISLOGICAL" => Value::Bool(matches!(a.first(), Some(Value::Bool(_)))),
        "ISNONTEXT" => Value::Bool(!matches!(a.first(), Some(Value::Text(_)))),
        "T" => match a.first() {
            Some(Value::Text(s)) => Value::Text(s.clone()),
            Some(e @ Value::Error(_)) => e.clone(),
            _ => Value::Text(String::new()),
        },
        "N" => match a.first() {
            Some(Value::Number(n)) => Value::Number(*n),
            Some(Value::Bool(b)) => Value::Number(*b as i32 as f64),
            Some(e @ Value::Error(_)) => e.clone(),
            _ => Value::Number(0.0),
        },
        "TYPE" => Value::Number(match a.first() {
            Some(Value::Text(_)) => 2.0,
            Some(Value::Bool(_)) => 4.0,
            Some(Value::Error(_)) => 16.0,
            _ => 1.0,
        }),
        "PROPER" => {
            let s = a.first().map(|v| v.display()).unwrap_or_default();
            let mut out = String::new();
            let mut head = true;
            for c in s.chars() {
                if c.is_alphabetic() {
                    out.extend(if head { c.to_uppercase().collect::<Vec<_>>() }
                               else { c.to_lowercase().collect() });
                    head = false;
                } else {
                    out.push(c);
                    head = true;
                }
            }
            Value::Text(out)
        }
        "EXACT" => Value::Bool(
            a.first().map(|v| v.display()) == a.get(1).map(|v| v.display())),
        "CLEAN" => Value::Text(
            a.first().map(|v| v.display()).unwrap_or_default()
                .chars().filter(|c| !c.is_control()).collect()),
        "FIXED" | "YEN" | "DOLLAR" => {
            // 数を文字にする(桁区切りつき)。YEN は ¥、DOLLAR は $ を頭に
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let dec = a.get(1).map(|v| v.as_number()).unwrap_or(2.0) as i32;
            let no_comma = name == "FIXED"
                && a.get(2).map(|v| v.as_number() != 0.0).unwrap_or(false);
            let code = match (name, no_comma) {
                (_, true) => format!("0.{}", "0".repeat(dec.max(0) as usize)),
                ("YEN", _) => format!("¥#,##0{}", if dec > 0 {
                    format!(".{}", "0".repeat(dec as usize)) } else { String::new() }),
                ("DOLLAR", _) => format!("$#,##0.{}", "0".repeat(dec.max(0) as usize)),
                _ => format!("#,##0{}", if dec > 0 {
                    format!(".{}", "0".repeat(dec as usize)) } else { String::new() }),
            };
            // 桁数が負なら、その桁で丸める(FIXED(-2) は百の位)
            let x = if dec < 0 {
                let f = 10f64.powi(-dec);
                (x / f).round() * f
            } else {
                x
            };
            Value::Text(format_value(&Value::Number(x), Some(&code), date1904))
        }
        "NUMBERVALUE" => {
            // NUMBERVALUE(文字列, [小数点], [桁区切り])
            let s = a.first().map(|v| v.display()).unwrap_or_default();
            let dec = a.get(1).map(|v| v.display()).unwrap_or_else(|| ".".into());
            let grp = a.get(2).map(|v| v.display()).unwrap_or_else(|| ",".into());
            let t: String = s
                .trim()
                .chars()
                .filter(|c| !grp.contains(*c) && !c.is_whitespace())
                .map(|c| if dec.contains(c) { '.' } else { c })
                .collect();
            match t.parse::<f64>() {
                Ok(n) => Value::Number(n),
                Err(_) => Value::Error("#VALUE!".into()),
            }
        }
        // バイト数の一族。**日本の古い帳票の定番** — 全角は2、半角(ASCII と
        // 半角カナ)は1と数える(Excel の日本語ロケールと同じ数え方)
        "LENB" => Value::Number(
            a.first().map(|v| v.display()).unwrap_or_default()
                .chars().map(jchar_width).sum::<usize>() as f64),
        "LEFTB" | "RIGHTB" | "MIDB" => {
            let s = a.first().map(|v| v.display()).unwrap_or_default();
            let ch: Vec<char> = s.chars().collect();
            let n = |i: usize| a.get(i).map(|v| v.as_number() as usize).unwrap_or(0);
            let take_bytes = |it: &mut dyn Iterator<Item = &char>, budget: usize| -> String {
                let mut used = 0;
                let mut out = String::new();
                for c in it {
                    let w = jchar_width(*c);
                    if used + w > budget {
                        break;
                    }
                    used += w;
                    out.push(*c);
                }
                out
            };
            Value::Text(match name {
                "LEFTB" => take_bytes(&mut ch.iter(), n(1)),
                "RIGHTB" => {
                    // 後ろから測って、前から出す
                    let total: usize = ch.iter().map(|c| jchar_width(*c)).sum();
                    let skip = total.saturating_sub(n(1));
                    let mut used = 0;
                    ch.iter()
                        .skip_while(|c| {
                            used += jchar_width(**c);
                            used <= skip
                        })
                        .collect()
                }
                _ => {
                    // MIDB(文字列, 始まり, 数)。始まりは1起点のバイト位置
                    let start = n(1).saturating_sub(1);
                    let mut used = 0;
                    let rest: Vec<&char> = ch
                        .iter()
                        .skip_while(|c| {
                            if used < start {
                                used += jchar_width(**c);
                                true
                            } else {
                                false
                            }
                        })
                        .collect();
                    take_bytes(&mut rest.into_iter(), n(2))
                }
            })
        }
        // 全角と半角(日本語一級の道具)
        "ASC" => Value::Text(asc_hankaku(&a.first().map(|v| v.display()).unwrap_or_default())),
        "JIS" | "DBCS" => {
            Value::Text(jis_zenkaku(&a.first().map(|v| v.display()).unwrap_or_default()))
        }
        "DATESTRING" => {
            // 通し番号 → 和暦の文字(令和08年08月05日)。明治より前は西暦で
            let serial = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            Value::Text(wareki(serial, ep))
        }
        "ADDRESS" => {
            // ADDRESS(行, 列, [形式]) — 1=絶対(既定) 4=相対
            let r = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let c = a.get(1).map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let abs = a.get(2).map(|v| v.as_number()).unwrap_or(1.0) as i64;
            if r < 1 || c < 1 {
                Value::Error("#VALUE!".into())
            } else {
                let cell = Pos::new(r as u32 - 1, c as u32 - 1).a1();
                Value::Text(match abs {
                    4 => cell,
                    _ => {
                        let split = cell.find(|ch: char| ch.is_ascii_digit()).unwrap_or(0);
                        format!("${}${}", &cell[..split], &cell[split..])
                    }
                })
            }
        }
        "HYPERLINK" => {
            // 表示は文字だけ(飛ぶ仕掛けはセルのリンクの仕事)
            a.get(1).or(a.first()).cloned().unwrap_or(Value::Empty)
        }
        "QUOTIENT" => {
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let y = a.get(1).map(|v| v.as_number()).unwrap_or(0.0);
            if y == 0.0 {
                Value::Error("#DIV/0!".into())
            } else {
                Value::Number((x / y).trunc())
            }
        }
        "CEILING.MATH" | "FLOOR.MATH" => {
            // 新しい既定の丸め。基準は絶対値で見る(符号の縛りが無い)
            let x = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let s = a.get(1).map(|v| v.as_number().abs()).unwrap_or(1.0);
            if s == 0.0 {
                Value::Number(0.0)
            } else {
                let q = x / s;
                Value::Number(if name == "CEILING.MATH" { q.ceil() } else { q.floor() } * s)
            }
        }
        "AVERAGEA" | "MAXA" | "MINA" => {
            // A 付き: 文字は0、TRUE は1 と数える(Excel の約束)
            let ns: Vec<f64> = a
                .iter()
                .filter(|v| !v.is_empty())
                .map(|v| match v {
                    Value::Number(n) => *n,
                    Value::Bool(b) => *b as i32 as f64,
                    _ => 0.0,
                })
                .collect();
            if ns.is_empty() {
                if name == "AVERAGEA" {
                    Value::Error("#DIV/0!".into())
                } else {
                    Value::Number(0.0)
                }
            } else {
                Value::Number(match name {
                    "AVERAGEA" => ns.iter().sum::<f64>() / ns.len() as f64,
                    "MAXA" => ns.iter().cloned().reduce(f64::max).unwrap(),
                    _ => ns.iter().cloned().reduce(f64::min).unwrap(),
                })
            }
        }
        "DAYS" => {
            let end = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            let start = a.get(1).map(|v| v.as_number()).unwrap_or(0.0);
            Value::Number(end - start)
        }
        "DAYS360" => {
            let s = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let e = a.get(1).map(|v| v.as_number()).unwrap_or(0.0) as i64;
            Value::Number(days360(s, e, ep) as f64)
        }
        "YEARFRAC" => {
            // 基準 0=30/360(既定) 1=実日数/年平均 2=/360 3=/365 4=欧州30/360
            let s = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let e = a.get(1).map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let basis = a.get(2).map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let days = (e - s) as f64;
            Value::Number(match basis {
                1 => days / 365.25, // 実際の暦の平均(Excel の厳密式の近似)
                2 => days / 360.0,
                3 => days / 365.0,
                4 => days360(s, e, ep) as f64 / 360.0,
                _ => days360(s, e, ep) as f64 / 360.0,
            })
        }
        "WEEKNUM" => {
            // 年の何週目か(1=日曜始まり(既定)、2=月曜始まり)
            let serial = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            let mon = a.get(1).map(|v| v.as_number()).unwrap_or(1.0) as i64 == 2;
            let (y, _, _) = civil_from_days(serial - ep);
            let jan1 = date_serial_at(y, 1, 1, ep);
            let head = if mon { (weekday0(jan1, ep) + 6) % 7 } else { weekday0(jan1, ep) };
            Value::Number(((serial - jan1 + head) / 7 + 1) as f64)
        }
        "ISOWEEKNUM" => {
            // ISO 8601: 木曜を含む週がその年の第1週
            let serial = a.first().map(|v| v.as_number()).unwrap_or(0.0) as i64;
            // その週の木曜へ動かして年内通算で数える
            let dow = (weekday0(serial, ep) + 6) % 7; // 0=月曜
            let thu = serial - dow + 3;
            let (y, _, _) = civil_from_days(thu - ep);
            Value::Number(((thu - date_serial_at(y, 1, 1, ep)) / 7 + 1) as f64)
        }
        "NPV" => {
            // NPV(利率, 額…) — 1期目の終わりから割り引く(Excel と同じ)
            let r = a.first().map(|v| v.as_number()).unwrap_or(0.0);
            if r <= -1.0 {
                Value::Error("#NUM!".into())
            } else {
                Value::Number(
                    a.get(1..)
                        .unwrap_or(&[])
                        .iter()
                        .filter(|v| !v.is_empty())
                        .enumerate()
                        .map(|(i, v)| v.as_number() / (1.0 + r).powi(i as i32 + 1))
                        .sum(),
                )
            }
        }
        "RATE" => {
            // RATE(回数, 定額, 現在価値, [将来価値]) — 反復解
            let g = |i: usize| a.get(i).map(|v| v.as_number()).unwrap_or(0.0);
            let (nper, pmt, pv, fv) = (g(0), g(1), g(2), g(3));
            let f = |r: f64| -> f64 {
                if r.abs() < 1e-12 {
                    pv + pmt * nper + fv
                } else {
                    let k = (1.0 + r).powf(nper);
                    pv * k + pmt * (k - 1.0) / r + fv
                }
            };
            match bisect(&f, -0.9999, 10.0) {
                Some(r) => Value::Number(r),
                None => Value::Error("#NUM!".into()),
            }
        }
        "PY" => Value::Error("#PY単独".into()), // =PY(…) はセル単独でだけ使える
        _ => Value::Error("#NAME?".into()),
    })
}

pub(super) fn array_call(name: &str, args: Vec<Arg>) -> Result<Vec<Vec<Value>>, Value> {
    let err = |s: &str| Value::Error(s.into());
    // 範囲を行ごとに割る
    let rows_of = |a: &Arg| -> Vec<Vec<Value>> {
        match a {
            Arg::One(v) => vec![vec![v.clone()]],
            Arg::Rect(w, vals) => {
                let w = (*w).max(1) as usize;
                vals.chunks(w).map(|r| r.to_vec()).collect()
            }
        }
    };
    match name {
        "SEQUENCE" => {
            // SEQUENCE(行, [列], [始まり], [間隔])
            let g = |i: usize| args.get(i).map(|a| a.first().as_number());
            let rows = g(0).unwrap_or(f64::NAN);
            let cols = g(1).unwrap_or(1.0);
            let start = g(2).unwrap_or(1.0);
            let step = g(3).unwrap_or(1.0);
            if !(1.0..=200_000.0).contains(&rows)
                || !(1.0..=200_000.0).contains(&cols)
                || rows * cols > 200_000.0
            {
                return Err(err("#NUM!"));
            }
            let (rows, cols) = (rows as usize, cols as usize);
            Ok((0..rows)
                .map(|r| {
                    (0..cols)
                        .map(|c| Value::Number(start + step * (r * cols + c) as f64))
                        .collect()
                })
                .collect())
        }
        "UNIQUE" => {
            let rows = rows_of(args.first().ok_or(err("#VALUE!"))?);
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for row in rows {
                let key: String =
                    row.iter().map(|v| v.display()).collect::<Vec<_>>().join("\u{1}");
                if seen.insert(key) {
                    out.push(row);
                }
            }
            Ok(out)
        }
        "SORT" => {
            // SORT(範囲, [鍵の列], [順序 1/-1])
            let mut rows = rows_of(args.first().ok_or(err("#VALUE!"))?);
            let w = rows.first().map(|r| r.len()).unwrap_or(0);
            let idx = args.get(1).map(|a| a.first().as_number() as usize).unwrap_or(1);
            let desc = args.get(2).map(|a| a.first().as_number() < 0.0).unwrap_or(false);
            if idx == 0 || idx > w {
                return Err(err("#VALUE!"));
            }
            rows.sort_by(|x, y| {
                let (a, b) = (&x[idx - 1], &y[idx - 1]);
                let o = match (a, b) {
                    (Value::Number(p), Value::Number(q)) => {
                        p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    _ => a.display().cmp(&b.display()),
                };
                if desc { o.reverse() } else { o }
            });
            Ok(rows)
        }
        "TEXTSPLIT" => {
            // TEXTSPLIT(文字, 列の区切り, [行の区切り], [空を飛ばす])
            // 区切りが空なら #VALUE!(黙って1個の塊を返さない)
            let s = args.first().ok_or(err("#VALUE!"))?.first().display();
            let col_d = args.get(1).map(|a| a.first().display()).unwrap_or_default();
            let row_d = args.get(2).map(|a| a.first().display()).unwrap_or_default();
            let skip_empty = args.get(3).is_some_and(|a| a.first().as_number() != 0.0);
            if col_d.is_empty() && row_d.is_empty() {
                return Err(err("#VALUE!"));
            }
            let lines: Vec<&str> = if row_d.is_empty() {
                vec![s.as_str()]
            } else {
                s.split(row_d.as_str()).collect()
            };
            let mut out: Vec<Vec<Value>> = Vec::new();
            for line in lines {
                let cells: Vec<&str> = if col_d.is_empty() {
                    vec![line]
                } else {
                    line.split(col_d.as_str()).collect()
                };
                let row: Vec<Value> = cells
                    .into_iter()
                    .filter(|c| !(skip_empty && c.is_empty()))
                    .map(|c| Value::Text(c.to_string()))
                    .collect();
                if !(skip_empty && row.is_empty()) {
                    out.push(row);
                }
            }
            if out.is_empty() {
                out.push(vec![Value::Text(String::new())]);
            }
            Ok(out)
        }
        // 拡張スピル。**縦横の向きを取り違えない**ことに注意して書く
        "SORTBY" => {
            // SORTBY(並べる範囲, 鍵の範囲, [1 昇順 / -1 降順])
            let rows = rows_of(args.first().ok_or(err("#VALUE!"))?);
            let keys: Vec<Value> = rows_of(args.get(1).ok_or(err("#VALUE!"))?)
                .into_iter()
                .map(|r| r.into_iter().next().unwrap_or(Value::Empty))
                .collect();
            if keys.len() != rows.len() {
                return Err(err("#VALUE!"));
            }
            let desc = args
                .get(2)
                .map(|g| g.first().as_number() < 0.0)
                .unwrap_or(false);
            let mut idx: Vec<usize> = (0..rows.len()).collect();
            idx.sort_by(|x, y| {
                let (p, q) = (&keys[*x], &keys[*y]);
                let o = match (p, q) {
                    (Value::Number(m), Value::Number(n)) => {
                        m.partial_cmp(n).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    _ => p.display().cmp(&q.display()),
                };
                if desc { o.reverse() } else { o }
            });
            Ok(idx.into_iter().map(|i| rows[i].clone()).collect())
        }
        "RANDARRAY" => {
            // RANDARRAY([行], [列], [最小], [最大], [整数か])
            let n = |i: usize, d: f64| args.get(i).map(|g| g.first().as_number()).unwrap_or(d);
            let (h, w) = (n(0, 1.0) as i64, n(1, 1.0) as i64);
            if h < 1 || w < 1 || h * w > 200_000 {
                return Err(err("#NUM!"));
            }
            let (lo, hi) = (n(2, 0.0), n(3, 1.0));
            let whole = args.get(4).map(|g| g.first().as_number() != 0.0).unwrap_or(false);
            let mut out = Vec::new();
            for _r in 0..h {
                let mut row = Vec::new();
                for _c in 0..w {
                    let v = lo + (hi - lo) * rand01();
                    row.push(Value::Number(if whole { v.round() } else { v }));
                }
                out.push(row);
            }
            Ok(out)
        }
        "VSTACK" | "HSTACK" => {
            // 縦(VSTACK)か横(HSTACK)に積む。足りない所は空
            let mut parts: Vec<Vec<Vec<Value>>> = Vec::new();
            for g in &args {
                parts.push(rows_of(g));
            }
            if name == "VSTACK" {
                let w = parts.iter().flatten().map(|r| r.len()).max().unwrap_or(0);
                let mut out = Vec::new();
                for p in parts {
                    for mut r in p {
                        r.resize(w, Value::Empty);
                        out.push(r);
                    }
                }
                Ok(out)
            } else {
                let h = parts.iter().map(|p| p.len()).max().unwrap_or(0);
                let mut out = vec![Vec::new(); h];
                for p in parts {
                    let w = p.iter().map(|r| r.len()).max().unwrap_or(0);
                    for (i, o) in out.iter_mut().enumerate() {
                        let mut r = p.get(i).cloned().unwrap_or_default();
                        r.resize(w, Value::Empty);
                        o.extend(r);
                    }
                }
                Ok(out)
            }
        }
        "TAKE" | "DROP" => {
            // TAKE(範囲, 行数, [列数]) / DROP(同じ)。負なら後ろから
            let rows = rows_of(args.first().ok_or(err("#VALUE!"))?);
            let nr = args.get(1).map(|g| g.first().as_number() as i64).unwrap_or(0);
            let nc = args.get(2).map(|g| g.first().as_number() as i64);
            let cut = |len: usize, n: i64, take: bool| -> (usize, usize) {
                let len = len as i64;
                let n = n.clamp(-len, len);
                match (take, n >= 0) {
                    (true, true) => (0, n as usize),
                    (true, false) => ((len + n) as usize, len as usize),
                    (false, true) => (n as usize, len as usize),
                    (false, false) => (0, (len + n) as usize),
                }
            };
            let take = name == "TAKE";
            let (a0, a1) = cut(rows.len(), nr, take);
            let mut out: Vec<Vec<Value>> = rows[a0.min(rows.len())..a1.min(rows.len())].to_vec();
            if let Some(nc) = nc {
                let w = out.iter().map(|r| r.len()).max().unwrap_or(0);
                let (b0, b1) = cut(w, nc, take);
                for r in out.iter_mut() {
                    r.resize(w, Value::Empty);
                    *r = r[b0.min(w)..b1.min(w)].to_vec();
                }
            }
            if out.is_empty() {
                return Err(err("#CALC!"));
            }
            Ok(out)
        }
        "TOCOL" | "TOROW" => {
            // 並びを1列(1行)に均す。**空は飛ばす**(Excel の既定は残すが、
            // 帳票では空の行が並ぶ方が困る…ので既定は残し、2つ目の引数で飛ばす)
            let rows = rows_of(args.first().ok_or(err("#VALUE!"))?);
            let skip_empty = args.get(1).map(|g| g.first().as_number() != 0.0).unwrap_or(false);
            let flat: Vec<Value> = rows
                .into_iter()
                .flatten()
                .filter(|v| !(skip_empty && v.is_empty()))
                .collect();
            if flat.is_empty() {
                return Err(err("#CALC!"));
            }
            Ok(if name == "TOCOL" {
                flat.into_iter().map(|v| vec![v]).collect()
            } else {
                vec![flat]
            })
        }
        "TRANSPOSE" => {
            let rows = rows_of(args.first().ok_or(err("#VALUE!"))?);
            let h = rows.len();
            let w = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            Ok((0..w)
                .map(|c| {
                    (0..h)
                        .map(|r| rows[r].get(c).cloned().unwrap_or(Value::Empty))
                        .collect()
                })
                .collect())
        }
        "FILTER" => {
            // FILTER(範囲, 条件の範囲, [空のとき])
            let rows = rows_of(args.first().ok_or(err("#VALUE!"))?);
            let inc: Vec<Value> =
                args.get(1).map(|a| a.values().to_vec()).unwrap_or_default();
            if inc.len() != rows.len() {
                return Err(err("#VALUE!"));
            }
            let out: Vec<Vec<Value>> = rows
                .into_iter()
                .zip(&inc)
                .filter(|(_, i)| i.as_number() != 0.0)
                .map(|(r, _)| r)
                .collect();
            if out.is_empty() {
                // 1件も無いときは第3引数(無ければ #CALC! — Excel と同じ)
                match args.get(2) {
                    Some(d) => Ok(vec![vec![d.first()]]),
                    None => Err(err("#CALC!")),
                }
            } else {
                Ok(out)
            }
        }
        _ => Err(err("#NAME?")),
    }
}