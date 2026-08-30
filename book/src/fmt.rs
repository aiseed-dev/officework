//! **表示形式。** `#,##0` や `yyyy年m月d日` を値に当てて、画面に出す字にする。


use super::types::*;

/// 表示形式を当てて、画面に出す文字列にする。
///
/// **付けた書式が画面に出ないなら、それは飾りでしかない。**
/// 対応するのは実務で使う分だけ — 桁区切り・小数・パーセント・通貨。
/// 日付は別の話(連番の解釈が要る)なのでここでは扱わない。
/// 書式コードの `[…]` を読み分ける。返すのは
/// **(かっこを取り除いたコード, 記号, 経過時間の札)**。
///
/// Excel の書式コードは角かっこで4種類のことを言う
/// (**`text` の印が要る** — 字下げだけの塊は rustdoc が組み立てようとして
/// 落ちる。今日この罠を他人のコードで直したそばから自分で踏んだ):
///
/// ```text
/// [$¥-411]#,##0   記号つきの地域指定 — 通貨記号はこう書かれる
/// [$-409]mmmm     地域指定だけ — 月名を何語で出すか
/// [Red] [赤]      色。ここは字を作る所なので使わない
/// [h] [mm] [ss]   経過時間(24時をまたいでも巻き戻さない)
/// ```
///
/// **前はどれも読み飛ばしていなかった**ので、`[Red]#,##0` が
/// `[Red]46,240`、`[$-409]mmmm yyyy` が `[$-446240` と出ていた
/// (2026-08-10、実物26枚のうち4枚がこの形の書式を持っていた)。
///
/// 地域の番号は取り出すが**まだ使っていない** — 月名を言語で出すには
/// 描き手に月名の表を繋ぐ必要がある(`datetime_names` に材料はある)。
/// 取り出しておくのは、繋ぐときにここを触らずに済むようにするため。
pub(super) fn take_brackets(code: &str) -> (String, String, Option<char>, Option<u32>) {
    let mut out = String::new();
    let mut sym = String::new();
    let mut elapsed = None;
    let mut lcid = None;
    let mut it = code.chars().peekable();
    let mut quoted = false;
    while let Some(c) = it.next() {
        if c == '"' {
            quoted = !quoted;
            out.push(c);
            continue;
        }
        if quoted || c != '[' {
            out.push(c);
            continue;
        }
        let mut inner = String::new();
        for q in it.by_ref() {
            if q == ']' {
                break;
            }
            inner.push(q);
        }
        let low = inner.to_ascii_lowercase();
        if let Some(rest) = inner.strip_prefix('$') {
            // [$記号-地域] / [$-地域]。記号は字として出し、地域は覚える
            let mut parts = rest.splitn(2, '-');
            sym.push_str(parts.next().unwrap_or(""));
            // 地域は16進。`[$-411]` のほか `[$-F800]` のような形もあるが、
            // 読めないものは黙って無視する(言語が分からないだけ)
            if let Some(hex) = parts.next() {
                lcid = u32::from_str_radix(hex.trim_start_matches("0x"), 16).ok();
            }
        } else if low.chars().all(|c| c == 'h' || c == 'm' || c == 's') && !low.is_empty() {
            // 経過時間。札は1字で覚える(hh も h と同じ意味)
            elapsed = low.chars().next();
            out.push_str(&inner);
        }
        // 色([Red]・[赤]・[Color3])と条件([>100])は字を作らない — 落とす
    }
    (out, sym, elapsed, lcid)
}

pub fn format_value(v: &Value, code: Option<&str>, date1904: bool) -> String {
    // 起点(1899-12-30 か、1904 ブックの 1904-01-01)。日付の描きはこれを通す
    let ep = crate::calc::excel_epoch(date1904);
    let Value::Number(n) = v else { return v.display() };
    let Some(code) = code else { return v.display() };
    // 角かっこを先に読み分ける。**残すと画面にそのまま出る**
    let (stripped, bracket_sym, elapsed, lcid) = take_brackets(code);
    let code: &str = &stripped;

    // テキスト形式(@)は素のまま(新しく打つ分を文字として扱うのは Excel の話。
    // 表示は変えない)
    if code.trim() == "@" {
        return v.display();
    }

    // 指数(0.00E+00 の形)。仮数の小数桁は書式の `.00` から数える
    if let Some(epos) = code.to_uppercase().find("E+") {
        let dec = code[..epos]
            .rsplit_once('.')
            .map(|(_, d)| d.chars().take_while(|c| *c == '0' || *c == '#').count())
            .unwrap_or(0);
        if *n == 0.0 {
            return format!("{:.*}E+00", dec, 0.0);
        }
        let e = n.abs().log10().floor() as i32;
        let m = n / 10f64.powi(e);
        let sign = if e < 0 { '-' } else { '+' };
        return format!("{:.*}E{}{:02}", dec, m, sign, e.abs());
    }

    // 日付・時刻の書式なら、通し番号を暦に直して描く
    if let Some(s) = format_date(*n, code, elapsed, lcid, ep) {
        return s;
    }

    let percent = code.contains('%');
    let n = if percent { n * 100.0 } else { *n };
    let comma = code.contains(',');
    // 小数点以下の桁数は書式の `.000` から数える
    let dec = code
        .rsplit_once('.')
        .map(|(_, d)| d.chars().take_while(|c| *c == '0' || *c == '#').count())
        .unwrap_or(0);

    let s = format!("{:.*}", dec, n.abs());
    let (int, frac) = match s.split_once('.') {
        Some((i, f)) => (i.to_string(), format!(".{f}")),
        None => (s, String::new()),
    };
    // **整数側の `0` の数だけ頭を 0 で詰める。** `0000` で 1 → `0001`。
    // 品番・会員番号・郵便番号の定番の書式で、**入れていなかった**
    // (`#,##0.00` や `¥#,##0` は効いていたので気づきにくかった)。
    // 2026-08-15、種苗の会の注文書の見本を実機で見て見つけた —
    // 番号の欄が 0001 でなく 1 で並んでいた。詰めるのは**桁区切りの前**
    // (Excel も `00,000` で 1234 → `01,234`)
    let int = {
        let sect0 = code.split(';').next().unwrap_or(code);
        let intpat = sect0.split_once('.').map(|(i, _)| i).unwrap_or(sect0);
        let zeros = intpat.chars().filter(|c| *c == '0').count();
        if int.len() < zeros {
            format!("{int:0>zeros$}")
        } else {
            int
        }
    };
    let int = if comma { group(&int) } else { int };

    // **書式は `正;負;ゼロ;文字` の区画に分かれます**(2026-08-31)。
    // 負の区画があればそちらを使います。役所の表は「△ 5,148」と書き、
    // 前は区画を見ずに `-5,148` と出していました(国税庁の酒税の表)
    let sects: Vec<&str> = code.split(';').collect();
    let hu_sect = (n < 0.0).then(|| sects.get(1).copied()).flatten();
    let sect = hu_sect.unwrap_or_else(|| sects.first().copied().unwrap_or(code));
    let mut out = String::new();
    // 負の区画が無ければ、こちらで `-` を付けます。区画があるときは、
    // その区画の字(`△ ` や `(`)が符号の役をします
    if n < 0.0 && hu_sect.is_none() {
        out.push('-');
    }
    // 記号は数の**前にも後ろにも**付く。`"¥"#,##0` と `#,##0.00 "€"` は
    // どちらも実際の綴りで、**前しか読まないと独・仏・西・伊・葡・露・越の
    // 記号が落ちる**(14言語のうち7つがこの並び。2026-08-10 に踏んだ)。
    // 数の芯(# 0 ?)の前と後ろを、それぞれ字として出す
    let core = |c: char| c == '#' || c == '0' || c == '?';
    let (head, tail) = match (sect.find(core), sect.rfind(core)) {
        (Some(a), Some(b)) => (&sect[..a], &sect[b + 1..]),
        _ => (sect, ""),
    };
    out.push_str(&bracket_sym);
    out.push_str(&affix(head));
    out.push_str(&int);
    out.push_str(&frac);
    out.push_str(&affix(tail));
    out
}

/// 数の前後に付く字を出す。引用と `\` の逃げを読み、`_x`(x の幅だけ空ける)と
/// `*x`(x で埋める)は**何も出さない** — 幅の調整で、字ではない。
/// `,` と `.` は数の側が出すので落とす
pub(super) fn affix(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        match c {
            '"' => {
                for q in it.by_ref() {
                    if q == '"' {
                        break;
                    }
                    out.push(q);
                }
            }
            '\\' => {
                if let Some(q) = it.next() {
                    out.push(q);
                }
            }
            '_' | '*' => {
                it.next();
            }
            ',' | '.' => {}
            _ => out.push(c),
        }
    }
    out
}

/// 日付・時刻の表示形式なら描いて Some、数の形式なら None。
///
/// 見分け方: 引用部("…")を除いた地に y・d・h(または m と s の組)が
/// あれば日付・時刻。# や 0 が混ざるものは数の形式(例: `#,##0;[Red]…` の
/// Red の d を日付と見ない)。m は h・s の隣なら「分」、それ以外は「月」。
/// **和暦(g・e)は描ける。** `ggge"年"m"月"d"日"` → 令和8年8月6日、
/// `ge.m.d` → R8.8.6。元号は `calc::era_of` の表(DATESTRING と同じ道)。
/// ここの注釈は長く「まだ描けない」と嘘を書いていた(2026-08-10 に実測して
/// 気づいた)— **描ける物を描けないと書くと、次の人が二度作る**
///
/// 描けないのは**月名と曜日名**のほう。`m` は何文字並べても数字
/// (`mmmm` → `08`)で、`aaa`/`aaaa` は `YOBI` と「曜日」を日本語で返す。
/// 13言語ぶんの月名・曜日名は vendor/sdkjs/common/NumFormat.js の
/// cultureInfo にある(sekkei/calc.ja.md「書式の一覧」参照)
pub(super) fn format_date(n: f64, code: &str, elapsed: Option<char>, lcid: Option<u32>, ep: i64) -> Option<String> {
    // **最初の節だけを使う。** 書式は `正;負;ゼロ;文字` の4節に分かれ、
    // 日付の書式はたいてい `[$-409]mmmm yyyy;@` のように末尾に文字用の
    // 節を持つ。切らないと `;@` がそのまま画面に出る
    let code = {
        let (mut cut, mut q) = (code.len(), false);
        for (i, c) in code.char_indices() {
            match c {
                '"' => q = !q,
                ';' if !q => {
                    cut = i;
                    break;
                }
                _ => {}
            }
        }
        &code[..cut]
    };
    let mut bare = String::new();
    let mut quoted = false;
    for c in code.chars() {
        match c {
            '"' => quoted = !quoted,
            _ if !quoted => bare.push(c.to_ascii_lowercase()),
            _ => {}
        }
    }
    if bare.contains('#') || bare.contains('0') {
        return None;
    }
    let datey = bare.contains('y') || bare.contains('d') || bare.contains('h')
        || bare.contains('a') // 曜日(aaa)
        || bare.contains('e') // 和暦の年
        || bare.contains('g') // 元号
        // **月名は単独でも日付。** `mmmm` だけの書式は「8月」を出す
        // (`m` 1つは分とも取れるので、3つ以上並んだときだけ)
        || bare.contains("mmm")
        || (bare.contains('m') && bare.contains('s'));
    // 経過時間の札([mm] だけ、など)も時刻の書式
    if (!datey && elapsed.is_none()) || n < 0.0 {
        return None;
    }

    let days = n.floor() as i64;
    let (y, mo, d) = crate::calc::civil_from_days(days - ep);
    let total = ((n - days as f64) * 86400.0).round() as i64;
    let (hh, mi, ss) = (total / 3600 % 24, total / 60 % 60, total % 60);
    let wd = crate::calc::weekday0(days, ep) as usize; // 0=日曜

    // 字句: 引用は文字どおり、同じ字の連なりは1つの札
    #[derive(PartialEq)]
    enum T {
        Run(char, usize),
        Lit(String),
    }
    let mut toks: Vec<T> = Vec::new();
    let mut it = code.chars().peekable();
    while let Some(c) = it.next() {
        if c == '"' {
            let mut s = String::new();
            for q in it.by_ref() {
                if q == '"' {
                    break;
                }
                s.push(q);
            }
            toks.push(T::Lit(s));
        } else if c == '\\' {
            // `\ ` `\,` は次の1字を字として出す(Excel の逃げ)。
            // 読み飛ばさないと画面に `\` が出る
            if let Some(q) = it.next() {
                match toks.last_mut() {
                    Some(T::Lit(s)) => s.push(q),
                    _ => toks.push(T::Lit(q.to_string())),
                }
            }
        } else if c.is_ascii_alphabetic() {
            let lc = c.to_ascii_lowercase();
            let mut len = 1;
            while it.peek().map(|p| p.to_ascii_lowercase()) == Some(lc) {
                it.next();
                len += 1;
            }
            toks.push(T::Run(lc, len));
        } else {
            match toks.last_mut() {
                Some(T::Lit(s)) => s.push(c),
                _ => toks.push(T::Lit(c.to_string())),
            }
        }
    }

    // **月名・曜日名は書式コードが運ぶ地域で決まる。** 読む人の言語では
    // ない — その書式が何語で書かれたかの話で、`[$-407]` の入ったセルは
    // 日本語で開いても独語の月名で出る(「その帳票が独語で作られた」が
    // 残るだけ。docs/sekkei/calc.ja.md)。
    //
    // 指定が無ければ日本語。実物26枚では月名・曜日名を使う書式は
    // **2件とも地域指定を持っていた**(2026-08-10 に数えた)ので、
    // 指定なしは実質「こちらで作った書式」であり、素の言語でよい
    let names = crate::datetime_names::names(
        lcid.and_then(crate::datetime_names::lang_of_lcid).unwrap_or("ja"),
    );
    // **属格を使うか。** 露語などは「8月」と「8月の」で形が違い、
    // 日付の中(日と並ぶとき)は属格になる。`d` の札があれば属格
    let genitive = bare.contains('d');

    // 経過時間の総量(札の単位で数える)。24 時をまたいでも巻き戻さない
    let total = match elapsed {
        Some('h') => (n * 24.0).floor() as i64,
        Some('m') => (n * 24.0 * 60.0).floor() as i64,
        Some('s') => (n * 24.0 * 3600.0).round() as i64,
        _ => 0,
    };
    let mut out = String::new();
    let mut prev_hour = false; // 直前の字の札が h だったか(m の意味の判定)
    for (i, t) in toks.iter().enumerate() {
        match t {
            T::Lit(s) => out.push_str(s),
            T::Run(c, len) => {
                let pad = |v: i64, len: usize| {
                    if len >= 2 { format!("{v:02}") } else { v.to_string() }
                };
                match c {
                    'y' => out.push_str(&if *len >= 3 {
                        format!("{y:04}")
                    } else {
                        format!("{:02}", y.rem_euclid(100))
                    }),
                    'd' => match *len {
                        3 => out.push_str(names.days_abbr[wd]),
                        n if n >= 4 => out.push_str(names.days[wd]),
                        _ => out.push_str(&pad(d, *len)),
                    },
                    // **経過時間は巻き戻さない。** [h]:mm は 25:30 のように
                    // 24 時を超えて数える(勤怠表の合計がこれ)。札が立って
                    // いる字だけが通し、それ以外は普段どおりの時分秒
                    'h' if elapsed == Some('h') => out.push_str(&total.to_string()),
                    'm' if elapsed == Some('m') => out.push_str(&total.to_string()),
                    's' if elapsed == Some('s') => out.push_str(&total.to_string()),
                    'h' => out.push_str(&pad(hh, *len)),
                    's' => out.push_str(&pad(ss, *len)),
                    'm' => {
                        // 分: h の直後、または次の字の札が s のとき。それ以外は月
                        let next_s = toks[i + 1..]
                            .iter()
                            .find_map(|t| match t {
                                T::Run(c, _) => Some(*c == 's'),
                                _ => None,
                            })
                            .unwrap_or(false);
                        if prev_hour || next_s {
                            out.push_str(&pad(mi, *len));
                        } else {
                            let k = (mo as usize).saturating_sub(1).min(11);
                            match *len {
                                3 => out.push_str(names.months_abbr[k]),
                                4 => out.push_str(match names.months_genitive {
                                    Some(g) if genitive => g[k],
                                    _ => names.months[k],
                                }),
                                // mmmmm は頭文字1つ(J F M …)
                                n if n >= 5 => {
                                    let full = names.months[k];
                                    out.push(full.chars().next().unwrap_or('?'));
                                }
                                _ => out.push_str(&pad(mo, *len)),
                            }
                        }
                    }
                    'a' => {
                        // aaa=短い曜日、aaaa=「〜曜日」
                        // aaa=短い曜日、aaaa=完全な曜日。**表から引く** —
                        // 前は YOBI と「曜日」を日本語で焼き付けていて、
                        // どの言語で開いても「木曜日」が出ていた
                        out.push_str(if *len >= 4 { names.days[wd] } else { names.days_abbr[wd] });
                    }
                    // 和暦: g=R gg=令 ggg=令和 / e=年(ee=0詰め)。明治より前は西暦
                    'g' => if let Some((era, initial, _)) = crate::calc::era_of(days, ep) { out.push_str(match *len {
                        1 => initial,
                        2 => &era[..era.char_indices().nth(1).map(|(i, _)| i)
                            .unwrap_or(era.len())],
                        _ => era,
                    }) },
                    'e' => match crate::calc::era_of(days, ep) {
                        Some((_, _, ey)) => out.push_str(&pad(ey, *len)),
                        None => out.push_str(&y.to_string()),
                    },
                    _ => return None, // 知らない字は描けない — 黙って崩さない
                }
                if c.is_ascii_alphabetic() {
                    prev_hour = *c == 'h';
                }
            }
        }
    }
    Some(out)
}

/// 3桁ごとに区切る。
pub(super) fn group(s: &str) -> String {
    let b = s.as_bytes();
    let mut o = String::new();
    for (i, c) in b.iter().enumerate() {
        if i > 0 && (b.len() - i).is_multiple_of(3) {
            o.push(',');
        }
        o.push(*c as char);
    }
    o
}
