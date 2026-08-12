//! ブックの操作の言葉 — **誰が呼んでも同じ意味**(SEKKEI「操作の言葉を
//! 1本に」段A。2026-08-12 に calc/src/rpc.rs から純移動)。
//!
//! JSON 1行の9命令(ping / book_info / new / open / save / get /
//! get_formula / set / expand)をここで捌く。**動いているアプリの都合**
//! (未保存の確認・undo の節目・状態行・画面の通知)は [`Host`] の向こう —
//! calc が実装すれば生きた表への口、pysheet が実装すればファイルへの口に
//! なる。**この口に無い動詞は既定で断る**(「できないものを、できるように
//! 見せない」を型で言う)。
//!
//! アプリ固有の命令(calc の ribbon / ui_state — 点検の道具用)は
//! [`Host::extra`] に残す。ソケットを開く・スレッド・泵はアプリ側
//! (calc/src/rpc.rs)のまま — ここは意味だけ。

use std::path::PathBuf;

/// ソケットの置き場所。`$XDG_RUNTIME_DIR/officework/<app>.sock`。
/// AF_UNIX の径路は 108 字までなので、長すぎるときは
/// `/tmp/officework-UID/<app>.sock` へ落とす(Python 側も同じ規則)
pub fn sock_path(app: &str) -> PathBuf {
    if let Some(base) = std::env::var_os("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(&base).join("officework").join(format!("{app}.sock"));
        if p.as_os_str().len() <= 90 {
            return p;
        }
    }
    let uid = std::fs::metadata("/proc/self")
        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
        .unwrap_or(0);
    std::env::temp_dir().join(format!("officework-{uid}")).join(format!("{app}.sock"))
}

// ---- JSON の小さな読み書き(依存を増やさない — ooxml/crypt と同じ流儀) ----

/// ごく浅い JSON の値。この口の語彙(文字列・数・真偽・null・2次元の並び)だけ
#[derive(Debug, Clone, PartialEq)]
pub enum J {
    S(String),
    N(f64),
    B(bool),
    Null,
    A(Vec<J>),
}

pub fn jesc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

impl J {
    pub fn to_json(&self) -> String {
        match self {
            J::S(s) => format!("\"{}\"", jesc(s)),
            J::N(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            J::B(b) => b.to_string(),
            J::Null => "null".into(),
            J::A(xs) => {
                format!("[{}]", xs.iter().map(|x| x.to_json()).collect::<Vec<_>>().join(","))
            }
        }
    }
}

/// 1行の JSON オブジェクトから鍵を引く(浅い読み。入れ子の object は読まない)
pub struct Jobj<'a> {
    src: &'a str,
}

impl<'a> Jobj<'a> {
    pub fn parse(src: &'a str) -> Option<Jobj<'a>> {
        let t = src.trim();
        if t.starts_with('{') && t.ends_with('}') {
            Some(Jobj { src: t })
        } else {
            None
        }
    }
    /// "key" の値の始まりの位置(値の1文字目)
    fn value_start(&self, key: &str) -> Option<usize> {
        let pat = format!("\"{}\"", jesc(key));
        let mut from = 0;
        loop {
            let i = self.src[from..].find(&pat)? + from;
            // 鍵の後ろに : が来るか(文字列の中の偶然の一致は雑に弾く)
            let after = &self.src[i + pat.len()..];
            let colon = after.find(|c: char| !c.is_whitespace())?;
            if after[colon..].starts_with(':') {
                let v = &after[colon + 1..];
                let off = v.find(|c: char| !c.is_whitespace())?;
                return Some(i + pat.len() + colon + 1 + off);
            }
            from = i + pat.len();
        }
    }
    pub fn str(&self, key: &str) -> Option<String> {
        let at = self.value_start(key)?;
        let v = &self.src[at..];
        if !v.starts_with('"') {
            return None;
        }
        let mut o = String::new();
        let mut it = v[1..].chars();
        while let Some(c) = it.next() {
            match c {
                '"' => return Some(o),
                '\\' => match it.next()? {
                    'n' => o.push('\n'),
                    'r' => o.push('\r'),
                    't' => o.push('\t'),
                    'u' => {
                        let h: String = (0..4).filter_map(|_| it.next()).collect();
                        if let Ok(cp) = u32::from_str_radix(&h, 16) {
                            if let Some(ch) = char::from_u32(cp) {
                                o.push(ch);
                            }
                        }
                    }
                    c => o.push(c),
                },
                c => o.push(c),
            }
        }
        None
    }
    /// 2次元の並び(値は 文字列/数/真偽/null)。"values": [[...],[...]]
    pub fn grid(&self, key: &str) -> Option<Vec<Vec<J>>> {
        let at = self.value_start(key)?;
        let v = &self.src[at..];
        if !v.starts_with('[') {
            return None;
        }
        let mut rows: Vec<Vec<J>> = Vec::new();
        let mut row: Vec<J> = Vec::new();
        let mut depth = 0usize;
        let mut i = 0usize;
        let b = v.as_bytes();
        while i < v.len() {
            let c = b[i] as char;
            match c {
                '[' => {
                    depth += 1;
                    if depth == 2 {
                        row = Vec::new();
                    }
                    i += 1;
                }
                ']' => {
                    if depth == 2 {
                        rows.push(std::mem::take(&mut row));
                    }
                    depth -= 1;
                    i += 1;
                    if depth == 0 {
                        return Some(rows);
                    }
                }
                '"' => {
                    // 文字列
                    let mut o = String::new();
                    i += 1;
                    while i < v.len() {
                        let c = b[i] as char;
                        if c == '\\' {
                            // 逃げ
                            let n = v[i + 1..].chars().next()?;
                            match n {
                                'n' => o.push('\n'),
                                'r' => o.push('\r'),
                                't' => o.push('\t'),
                                'u' => {
                                    let h = v.get(i + 2..i + 6)?;
                                    if let Ok(cp) = u32::from_str_radix(h, 16) {
                                        if let Some(ch) = char::from_u32(cp) {
                                            o.push(ch);
                                        }
                                    }
                                    i += 4;
                                }
                                c => o.push(c),
                            }
                            i += 1 + n.len_utf8();
                        } else if c == '"' {
                            i += 1;
                            break;
                        } else {
                            let ch = v[i..].chars().next()?;
                            o.push(ch);
                            i += ch.len_utf8();
                        }
                    }
                    if depth == 2 {
                        row.push(J::S(o));
                    }
                }
                ',' | ' ' | '\t' | '\n' | '\r' => i += 1,
                _ => {
                    // 数・真偽・null
                    let end = v[i..]
                        .find(|c: char| c == ',' || c == ']' || c.is_whitespace())
                        .map(|e| i + e)
                        .unwrap_or(v.len());
                    let tok = &v[i..end];
                    let j = match tok {
                        "true" => J::B(true),
                        "false" => J::B(false),
                        "null" => J::Null,
                        t => J::N(t.parse().ok()?),
                    };
                    if depth == 2 {
                        row.push(j);
                    }
                    i = end;
                }
            }
        }
        None
    }
}

pub fn err(msg: &str) -> String {
    format!("{{\"err\":\"{}\"}}", jesc(msg))
}

// ---- 口の向こう(アプリが実装する) ----------------------------------------

/// 口の向こうの実装。**gpui を知らない** — 「動いているアプリの都合」を
/// 名前のある数個のメソッドに閉じ込める(切れない部分が名前で見える形)。
/// 既定の実装は「何もしない/断る」— この口に無い動詞は黙って動かない
pub trait Host {
    /// ping の名乗り("calc" 等)
    fn app(&self) -> &'static str;
    fn book(&self) -> &sheet::Book;
    fn book_mut(&mut self) -> &mut sheet::Book;
    /// いま出ているシートの添字(ファイルの口なら 0 でよい)
    fn active(&self) -> usize;
    fn path(&self) -> Option<&std::path::Path>;

    /// 打ちかけの確定(calc の commit)。ファイルの口では何もしない
    fn settle(&mut self) {}
    /// 未保存の変更があるか(new/open が黙って捨てないための確認)
    fn dirty(&self) -> bool {
        false
    }
    /// データを変えた印
    fn mark_dirty(&mut self) {}
    /// undo の節目を(要るなら)置く。手続きの最中は置かない、はアプリの判断
    fn mark_once(&mut self) {}
    /// 書き込みの後片づけ(行の高さ合わせ等)。written はセルの並び
    fn after_write(&mut self, _si: usize, _written: &[sheet::Pos]) {}
    /// 書き込みの報告(状態行に「{n} セル」等)。ファイルの口では黙ってよい
    fn wrote(&mut self, _n: usize) {}

    /// この口に無い動詞は既定で断る
    fn new_book(&mut self) -> Result<(), String> {
        Err("この口では new はできません".into())
    }
    fn open(&mut self, _p: &std::path::Path) -> Result<(), String> {
        Err("この口では open はできません".into())
    }
    fn save(&mut self, _p: PathBuf) -> Result<(), String> {
        Err("この口では save はできません".into())
    }

    /// アプリにしか無い命令(calc の ribbon / ui_state)。既定は「知らない」
    fn extra(&mut self, _cmd: &str, _o: &Jobj) -> Option<String> {
        None
    }
}

// ---- 9命令 --------------------------------------------------------------

/// 1要求を捌く。答えは JSON 1行。
pub fn handle(h: &mut impl Host, line: &str) -> String {
    let Some(o) = Jobj::parse(line) else {
        return err("JSON が読めません");
    };
    let Some(cmd) = o.str("cmd") else {
        return err("cmd がありません");
    };
    match cmd.as_str() {
        "ping" => format!("{{\"ok\":true,\"app\":\"{}\"}}", h.app()),
        // ブックの情報(名前・道・シートの一覧・いまのシート)
        "book_info" => {
            let sheets = J::A(h.book().sheets.iter().map(|s| J::S(s.name.clone())).collect());
            format!(
                "{{\"ok\":true,\"path\":{},\"sheets\":{},\"active\":{}}}",
                match h.path() {
                    Some(p) => J::S(p.display().to_string()).to_json(),
                    None => "null".into(),
                },
                sheets.to_json(),
                h.active()
            )
        }
        // 新しいブック(未保存の変更があれば断る — 黙って捨てない)
        "new" => {
            h.settle();
            if h.dirty() {
                return err("calc に未保存の変更があります(保存するか、捨ててから)");
            }
            match h.new_book() {
                Ok(()) => "{\"ok\":true}".into(),
                Err(e) => err(&e),
            }
        }
        // ファイルを開く(同じく、未保存があれば断る)
        "open" => {
            let Some(p) = o.str("path") else { return err("path がありません") };
            h.settle();
            if h.dirty() {
                return err("calc に未保存の変更があります(保存するか、捨ててから)");
            }
            match h.open(std::path::Path::new(&p)) {
                Ok(()) => "{\"ok\":true}".into(),
                Err(e) => err(&e),
            }
        }
        // 保存(path 省略はいまの場所へ)
        "save" => {
            h.settle();
            match o.str("path").map(PathBuf::from).or_else(|| h.path().map(|p| p.to_path_buf())) {
                Some(p) => match h.save(p) {
                    Ok(()) => "{\"ok\":true}".into(),
                    Err(e) => err(&e),
                },
                None => err("保存先がありません(path を渡してください)"),
            }
        }
        // 範囲の値を読む(計算済みの値。数は数・文字は文字・空は null)
        "get" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let sh = &h.book().sheets[si];
            let mut rows: Vec<J> = Vec::new();
            for r in a.row..=b.row {
                let mut cols: Vec<J> = Vec::new();
                for c in a.col..=b.col {
                    cols.push(match sh.value(sheet::Pos::new(r, c)) {
                        sheet::Value::Number(n) => J::N(n),
                        sheet::Value::Bool(x) => J::B(x),
                        sheet::Value::Empty => J::Null,
                        v => J::S(v.display()),
                    });
                }
                rows.push(J::A(cols));
            }
            format!("{{\"ok\":true,\"values\":{}}}", J::A(rows).to_json())
        }
        // 式を読む(無ければ null)
        "get_formula" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let sh = &h.book().sheets[si];
            let mut rows: Vec<J> = Vec::new();
            for r in a.row..=b.row {
                let mut cols: Vec<J> = Vec::new();
                for c in a.col..=b.col {
                    cols.push(
                        match sh.get(sheet::Pos::new(r, c)).and_then(|x| x.formula.clone()) {
                            Some(f) => J::S(format!("={f}")),
                            None => J::Null,
                        },
                    );
                }
                rows.push(J::A(cols));
            }
            format!("{{\"ok\":true,\"formulas\":{}}}", J::A(rows).to_json())
        }
        // 範囲へ書く(origin の a1 から values の形ぶん)。
        // 文字列は Cell::input と同じ扱い(=から始まれば式)。1回=1手(Ctrl+Z)
        "set" => {
            let Some(a1) = o.str("a1") else { return err("a1 がありません") };
            let Some(origin) = sheet::Pos::parse(a1.split(':').next().unwrap_or(&a1)) else {
                return err("a1 が読めません");
            };
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let Some(grid) = o.grid("values") else { return err("values がありません") };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            h.settle();
            h.mark_once();
            let mut n = 0usize;
            let mut written: Vec<sheet::Pos> = Vec::new();
            for (dr, row) in grid.iter().enumerate() {
                for (dc, v) in row.iter().enumerate() {
                    let p = sheet::Pos::new(origin.row + dr as u32, origin.col + dc as u32);
                    let sh = &mut h.book_mut().sheets[si];
                    let fmt = sh.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
                    let mut cell = match v {
                        J::Null => sheet::Cell::input(""),
                        J::N(x) => sheet::Cell::input(&J::N(*x).to_json()),
                        J::B(x) => sheet::Cell::input(if *x { "TRUE" } else { "FALSE" }),
                        J::S(s) => sheet::Cell::input(s),
                        J::A(_) => return err("values の入れ子が深すぎます"),
                    };
                    cell.fmt = fmt; // 書式は据え置く(打ち直しと同じ作法)
                    sh.set(p, cell);
                    written.push(p);
                    n += 1;
                }
            }
            // 見出しを書いたら行を広げる(手で打ったときと同じ扱い)は
            // アプリの後片づけ — いま出ているシートのときだけ、等の判断ごと
            h.after_write(si, &written);
            sheet::recalc_book(h.book_mut(), si);
            h.mark_dirty();
            h.wrote(n);
            format!("{{\"ok\":true,\"cells\":{n}}}")
        }
        // 左上から地続きの表の大きさ(xlwings の expand='table')
        "expand" => {
            let Some(a1) = o.str("a1") else { return err("a1 がありません") };
            let Some(p) = sheet::Pos::parse(&a1) else { return err("a1 が読めません") };
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let sh = &h.book().sheets[si];
            let filled = |r: u32, c: u32| !sh.value(sheet::Pos::new(r, c)).is_empty();
            let mut hh = 0u32;
            while filled(p.row + hh, p.col) {
                hh += 1;
            }
            let mut w = 0u32;
            while filled(p.row, p.col + w) {
                w += 1;
            }
            format!("{{\"ok\":true,\"rows\":{},\"cols\":{}}}", hh.max(1), w.max(1))
        }
        other => match h.extra(other, &o) {
            Some(resp) => resp,
            None => err(&format!("知らない cmd: {other}")),
        },
    }
}

/// 要求のシート(名前。省略はいまのシート)を添字に。
fn sheet_index(h: &impl Host, o: &Jobj) -> Result<usize, String> {
    match o.str("sheet") {
        None => Ok(h.active()),
        Some(name) => h
            .book()
            .sheets
            .iter()
            .position(|s| s.name == name)
            .ok_or_else(|| err(&format!("シート「{name}」がありません"))),
    }
}

/// 要求の範囲(a1 は "B2" か "B2:D4")を (シート, 左上, 右下) に。
fn target(h: &impl Host, o: &Jobj) -> Result<(usize, sheet::Pos, sheet::Pos), String> {
    let si = sheet_index(h, o)?;
    let a1 = o.str("a1").ok_or_else(|| err("a1 がありません"))?;
    let mut it = a1.split(':');
    let a = it.next().and_then(sheet::Pos::parse).ok_or_else(|| err("a1 が読めません"))?;
    let b = match it.next() {
        Some(t) => sheet::Pos::parse(t).ok_or_else(|| err("a1 が読めません"))?,
        None => a,
    };
    Ok((si, a, b))
}
