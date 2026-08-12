//! ブックの操作の言葉 — **誰が呼んでも同じ意味**(SEKKEI「操作の言葉を
//! 1本に」段A。2026-08-12 に calc/src/rpc.rs から純移動)。
//!
//! JSON 1行の命令(ping / book_info / new / open / save / get /
//! get_formula / set / expand と、橋の背骨 2026-08-12: calculate /
//! selection / select / activate_sheet / status / to_pdf / copy_sheet /
//! delete_sheet / merges / merge / unmerge / merge_area / clear /
//! clear_contents / end)をここで捌く。**動いているアプリの都合**
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
/// `/tmp/officework-UID/<app>.sock` へ落とす(Python 側も同じ規則)。
/// **unix だけ** — 橋は「この機械の unix ソケット」が設計で、Windows の
/// wheel(エンジンだけを配る)はこれを使わない。0.2.0 のタグで Windows の
/// wheel がここで組めなくなって気づいた(2026-08-12 — publish の門は効き、
/// PyPI には何も出ていない)
#[cfg(unix)]
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
    /// 鍵があるか(値が null でも真)。「渡した項目だけ変える」書式の口が使う
    pub fn has(&self, key: &str) -> bool {
        self.value_start(key).is_some()
    }

    /// 値が null か(鍵が無いなら偽)
    pub fn is_null(&self, key: &str) -> bool {
        self.value_start(key).map(|at| self.src[at..].starts_with("null")).unwrap_or(false)
    }

    /// 数(整数も小数も)。無い・数でないなら None
    pub fn num(&self, key: &str) -> Option<f64> {
        let at = self.value_start(key)?;
        let v = &self.src[at..];
        let end = v
            .find(|c: char| c == ',' || c == '}' || c == ']' || c.is_whitespace())
            .unwrap_or(v.len());
        v[..end].parse().ok()
    }

    /// 真偽。無い・真偽でないなら None
    pub fn bool(&self, key: &str) -> Option<bool> {
        let at = self.value_start(key)?;
        let v = &self.src[at..];
        if v.starts_with("true") {
            Some(true)
        } else if v.starts_with("false") {
            Some(false)
        } else {
            None
        }
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

    /// アプリの版(ping の返事に載せる)。ファイルの口は名乗らなくてよい
    fn version(&self) -> &'static str {
        ""
    }
    /// いま選んでいる範囲(シート, 左上, 右下)。画面のあるアプリだけが持つ
    fn selection(&self) -> Option<(usize, sheet::Pos, sheet::Pos)> {
        None
    }
    /// 選択を動かして見せる。画面のあるアプリだけ
    fn select(&mut self, _si: usize, _a: sheet::Pos, _b: sheet::Pos) -> Result<(), String> {
        Err("この口では select はできません".into())
    }
    /// 画面のシートを切り替える。画面のあるアプリだけ
    fn activate_sheet(&mut self, _si: usize) -> Result<(), String> {
        Err("この口では activate はできません".into())
    }
    /// 状態行に文言を出す(長い処理の進み具合を見せる)。画面のあるアプリだけ
    fn set_status(&mut self, _text: &str) -> Result<(), String> {
        Err("この口では status はできません".into())
    }
    /// シートを PDF に。返りは報告の文言(効かせた印刷設定など)
    fn to_pdf(&mut self, _si: usize, _p: &std::path::Path) -> Result<String, String> {
        Err("この口では to_pdf はできません".into())
    }
    /// シートの複製(耳のメニューと同じ作法)。返りは写しの名前
    fn copy_sheet(&mut self, _si: usize, _name: Option<&str>) -> Result<String, String> {
        Err("この口では copy_sheet はできません".into())
    }
    /// シートの削除(同じく)。返りは消した名前
    fn delete_sheet(&mut self, _si: usize) -> Result<String, String> {
        Err("この口では delete_sheet はできません".into())
    }
    /// ウィンドウ枠の固定(いま画面が持っている値。保存で xlsx に載る)
    fn get_freeze(&mut self, _si: usize) -> Result<(u32, u32), String> {
        Err("この口では freeze はできません".into())
    }
    /// ウィンドウ枠の固定を置く。(0, 0) は解除
    fn set_freeze(&mut self, _si: usize, _rows: u32, _cols: u32) -> Result<(), String> {
        Err("この口では freeze はできません".into())
    }
    /// シートを隠す・戻す(最後の見えている1枚は断る、はアプリの作法)
    fn set_sheet_hidden(&mut self, _si: usize, _hidden: bool) -> Result<(), String> {
        Err("この口では visible はできません".into())
    }
    /// 中身に合わせて列幅(col=true)・行高を決める。返りは合わせた本数。
    /// **文字の測りはアプリが持っている**ので、画面のあるアプリだけ
    fn autofit(
        &mut self,
        _si: usize,
        _a: sheet::Pos,
        _b: sheet::Pos,
        _col: bool,
    ) -> Result<usize, String> {
        Err("この口では autofit はできません(文字の測りが要ります)".into())
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
        "ping" => format!(
            "{{\"ok\":true,\"app\":\"{}\",\"version\":{}}}",
            h.app(),
            J::S(h.version().to_string()).to_json()
        ),
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
        // 全再計算(xlwings の App.calculate)
        "calculate" => {
            h.settle();
            let si = h.active();
            sheet::recalc_book(h.book_mut(), si);
            "{\"ok\":true}".into()
        }
        // いま選んでいる範囲(「選んで、Jupyter で加工」の入り方)
        "selection" => match h.selection() {
            Some((si, a, b)) => {
                let name = h.book().sheets[si].name.clone();
                let a1 =
                    if a == b { a.a1() } else { format!("{}:{}", a.a1(), b.a1()) };
                format!(
                    "{{\"ok\":true,\"sheet\":{},\"a1\":{}}}",
                    J::S(name).to_json(),
                    J::S(a1).to_json()
                )
            }
            None => err("この口では selection はできません"),
        },
        // 選択を動かして見せる
        "select" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            match h.select(si, a, b) {
                Ok(()) => "{\"ok\":true}".into(),
                Err(e) => err(&e),
            }
        }
        // 画面のシートを切り替える
        "activate_sheet" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            match h.activate_sheet(si) {
                Ok(()) => "{\"ok\":true}".into(),
                Err(e) => err(&e),
            }
        }
        // 状態行に文言を出す(長い処理の進み具合)
        "status" => {
            let Some(text) = o.str("text") else { return err("text がありません") };
            match h.set_status(&text) {
                Ok(()) => "{\"ok\":true}".into(),
                Err(e) => err(&e),
            }
        }
        // シートを PDF に(印刷設定に従う。効かせた物は note で言う)
        "to_pdf" => {
            let Some(p) = o.str("path") else { return err("path がありません") };
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            h.settle();
            match h.to_pdf(si, std::path::Path::new(&p)) {
                Ok(note) => format!("{{\"ok\":true,\"note\":{}}}", J::S(note).to_json()),
                Err(e) => err(&e),
            }
        }
        // シートの複製(写しは元の右隣・そこへ移る。名前は省略で「名前 (2)」)
        "copy_sheet" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let name = o.str("new_name");
            match h.copy_sheet(si, name.as_deref()) {
                Ok(n) => format!("{{\"ok\":true,\"name\":{}}}", J::S(n).to_json()),
                Err(e) => err(&e),
            }
        }
        // シートの削除(最後の1枚は断る。undo は消える — アプリの削除と同じ)
        "delete_sheet" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            match h.delete_sheet(si) {
                Ok(n) => format!("{{\"ok\":true,\"name\":{}}}", J::S(n).to_json()),
                Err(e) => err(&e),
            }
        }
        // ウィンドウ枠の固定。rows / cols を渡せば置く((0,0) は解除)、
        // 渡さなければ今の値を返す
        "freeze" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let (rows, cols) = (o.num("rows"), o.num("cols"));
            if rows.is_none() && cols.is_none() {
                match h.get_freeze(si) {
                    Ok((r, c)) => format!("{{\"ok\":true,\"rows\":{r},\"cols\":{c}}}"),
                    Err(e) => err(&e),
                }
            } else {
                match h.set_freeze(si, rows.unwrap_or(0.0) as u32, cols.unwrap_or(0.0) as u32)
                {
                    Ok(()) => "{\"ok\":true}".into(),
                    Err(e) => err(&e),
                }
            }
        }
        // シートの表示・非表示。value を渡せば置く、渡さなければ今の値を返す
        "sheet_visible" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            match o.bool("value") {
                None => {
                    format!("{{\"ok\":true,\"visible\":{}}}", !h.book().sheets[si].hidden)
                }
                Some(v) => match h.set_sheet_hidden(si, !v) {
                    Ok(()) => "{\"ok\":true}".into(),
                    Err(e) => err(&e),
                },
            }
        }
        // 範囲を動かす(切り取って貼る)。**外から指す式は付いて動く**
        "move_range" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            let dr = o.num("rows").unwrap_or(0.0) as i64;
            let dc = o.num("cols").unwrap_or(0.0) as i64;
            if a.row as i64 + dr < 0 || a.col as i64 + dc < 0 {
                return err("紙の外(0行・0列より上)へは動かせません");
            }
            let translate = o.bool("translate").unwrap_or(false);
            h.settle();
            h.mark_once();
            let n = h.book_mut().sheets[si].move_range(a, b, dr, dc, translate);
            sheet::recalc_book(h.book_mut(), si);
            h.mark_dirty();
            format!("{{\"ok\":true,\"cells\":{n}}}")
        }
        // 中身に合わせて列幅・行高を決める(リボンの「自動調整」と同じ測り)
        "autofit" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let col = o.str("axis").as_deref() != Some("rows");
            h.settle();
            h.mark_once();
            match h.autofit(si, a, b, col) {
                Ok(n) => {
                    if n > 0 {
                        h.mark_dirty();
                    }
                    format!("{{\"ok\":true,\"count\":{n}}}")
                }
                Err(e) => err(&e),
            }
        }
        // 行・列を挿す/抜く(丸ごと)。**残った式の参照が付いて動く** —
        // 明細の行を増やす操作そのもの。count は枚数(既定 1)
        "insert_rows" | "delete_rows" | "insert_cols" | "delete_cols" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            // at は行なら "3"(1起点)か "A3"、列なら "C" か "C3"
            let Some(at) = o.str("at") else { return err("at がありません") };
            let rows = cmd.ends_with("rows");
            let idx = if rows {
                match at.trim().parse::<u32>() {
                    Ok(n) if n >= 1 => n - 1,
                    _ => match sheet::Pos::parse(&at) {
                        Some(p) => p.row,
                        None => return err(&format!("行の指し方が読めません: {at:?}")),
                    },
                }
            } else {
                match sheet::Pos::parse(&format!("{}1", at.trim())) {
                    Some(p) => p.col,
                    None => match sheet::Pos::parse(&at) {
                        Some(p) => p.col,
                        None => return err(&format!("列の指し方が読めません: {at:?}")),
                    },
                }
            };
            let count = o.num("count").unwrap_or(1.0).max(1.0) as u32;
            h.settle();
            h.mark_once();
            let sh = &mut h.book_mut().sheets[si];
            for _ in 0..count {
                match cmd.as_str() {
                    "insert_rows" => sh.insert_row(idx),
                    "delete_rows" => sh.remove_row(idx),
                    "insert_cols" => sh.insert_col(idx),
                    _ => sh.remove_col(idx),
                }
            }
            sheet::recalc_book(h.book_mut(), si);
            h.mark_dirty();
            format!("{{\"ok\":true,\"count\":{count}}}")
        }
        // シートの画像の一覧 [[留めたセル, 幅px, 高さpx], …]
        // (開いた帳票にあった物と、Python が貼った物の両方)
        "pictures" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let sh = &h.book().sheets[si];
            let items: Vec<J> = sh
                .images
                .iter()
                .chain(sh.images_new.iter())
                .map(|im| {
                    J::A(vec![
                        J::S(im.at.a1()),
                        J::N(f64::from(im.width_px)),
                        J::N(f64::from(im.height_px)),
                    ])
                })
                .collect();
            format!("{{\"ok\":true,\"pictures\":{}}}", J::A(items).to_json())
        }
        // 画像(PNG / JPEG)をシートに浮かべる。data は16進の bytes。
        // アプリの「挿入 > グラフ」と同じ道 — matplotlib の絵が Python から
        // 実機のシートに浮かぶ(SEKKEI「calc の分業」の筋)
        "add_image" => {
            let Some(hexdata) = o.str("data") else { return err("data がありません") };
            let Some(data) = unhex(&hexdata) else {
                return err("data が16進として読めません");
            };
            let Some((w, hh)) = image_px(&data) else {
                return err("PNG / JPEG として読めない(大きさが測れない)");
            };
            let (si, a, _) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            // 実寸を既定に、片方だけ渡されたら縦横比を保って合わせる
            let (w0, h0) = (w as f32, hh as f32);
            let (width, height) = match (o.num("width_px"), o.num("height_px")) {
                (Some(wq), Some(hq)) => (wq as f32, hq as f32),
                (Some(wq), None) => (wq as f32, wq as f32 * h0 / w0),
                (None, Some(hq)) => (hq as f32 * w0 / h0, hq as f32),
                (None, None) => (w0, h0),
            };
            h.settle();
            h.mark_once();
            h.book_mut().sheets[si].images_new.push(sheet::model::SheetImage {
                at: a,
                dx_px: 0.0,
                dy_px: 0.0,
                width_px: width,
                height_px: height,
                data,
            });
            h.mark_dirty();
            format!("{{\"ok\":true,\"width_px\":{width},\"height_px\":{height}}}")
        }
        // 名前付き範囲の一覧 [[シート, 名前, 参照], …](全シート)
        "names" => {
            let items: Vec<J> = h
                .book()
                .sheets
                .iter()
                .flat_map(|s| {
                    s.names.iter().map(|(n, r)| {
                        J::A(vec![J::S(s.name.clone()), J::S(n.clone()), J::S(r.clone())])
                    })
                })
                .collect();
            format!("{{\"ok\":true,\"names\":{}}}", J::A(items).to_json())
        }
        // 名前を定義する(同じ名前はどのシートの物でも置き換え)。式が追随する
        "define_name" => {
            let Some(name) = o.str("name") else { return err("name がありません") };
            if name.is_empty() || name.contains([' ', '!', ':']) {
                return err(&format!("名前に空白・! ・: は使えない: {name:?}"));
            }
            let (si, a, b_) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let reference =
                if a == b_ { a.a1() } else { format!("{}:{}", a.a1(), b_.a1()) };
            h.settle();
            h.mark_once();
            for s in h.book_mut().sheets.iter_mut() {
                s.names.retain(|(n, _)| *n != name);
            }
            h.book_mut().sheets[si].names.push((name, reference));
            sheet::recalc_book(h.book_mut(), si);
            h.mark_dirty();
            "{\"ok\":true}".into()
        }
        // 名前を消す(どのシートの物でも)。式は #NAME? になる — 黙って残さない
        "delete_name" => {
            let Some(name) = o.str("name") else { return err("name がありません") };
            h.settle();
            h.mark_once();
            let mut removed = false;
            for s in h.book_mut().sheets.iter_mut() {
                let before = s.names.len();
                s.names.retain(|(n, _)| *n != name);
                removed |= s.names.len() != before;
            }
            if removed {
                let si = h.active();
                sheet::recalc_book(h.book_mut(), si);
                h.mark_dirty();
            }
            format!("{{\"ok\":true,\"removed\":{removed}}}")
        }
        // 結合の一覧(["B2","C3"] の対の並び — pysheet の merges と同じ形)
        "merges" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let pairs = J::A(
                h.book().sheets[si]
                    .merges
                    .iter()
                    .map(|(a, b)| J::A(vec![J::S(a.a1()), J::S(b.a1())]))
                    .collect(),
            );
            format!("{{\"ok\":true,\"merges\":{}}}", pairs.to_json())
        }
        // 結合する(家の作法 — アプリの結合と同じ sheet::model の merge)
        "merge" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            h.settle();
            h.mark_once();
            let promoted = h.book_mut().sheets[si].merge(a, b);
            sheet::recalc_book(h.book_mut(), si);
            h.mark_dirty();
            format!("{{\"ok\":true,\"promoted\":{promoted}}}")
        }
        // 範囲に掛かる結合を解く(xlwings の unmerge と同じ「掛かる物は全部」)
        "unmerge" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            h.settle();
            h.mark_once();
            let n = h.book_mut().sheets[si].unmerge(a, b);
            if n > 0 {
                h.mark_dirty();
            }
            format!("{{\"ok\":true,\"removed\":{n}}}")
        }
        // セルを含む結合の範囲(無ければセル自身 — xlwings の merge_area)
        "merge_area" => {
            let (si, a, _) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let sh = &h.book().sheets[si];
            let a1 = match sh.merges.iter().find(|(x, y)| {
                (x.row..=y.row).contains(&a.row) && (x.col..=y.col).contains(&a.col)
            }) {
                Some((x, y)) => format!("{}:{}", x.a1(), y.a1()),
                None => a.a1(),
            };
            format!("{{\"ok\":true,\"a1\":{}}}", J::S(a1).to_json())
        }
        // 中身を消す。clear_contents は値と式だけ(書式は据え置き — set の
        // Null と同じ道)、clear は書式ごと。a1 省略はシート全部。
        // 結合は消さない(結合を解くのは unmerge の仕事)
        "clear" | "clear_contents" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            let span = match o.str("a1") {
                Some(a1) => {
                    let mut it = a1.split(':');
                    let a = match it.next().and_then(sheet::Pos::parse) {
                        Some(p) => p,
                        None => return err("a1 が読めません"),
                    };
                    let b = match it.next() {
                        Some(t) => match sheet::Pos::parse(t) {
                            Some(p) => p,
                            None => return err("a1 が読めません"),
                        },
                        None => a,
                    };
                    Some((a, b))
                }
                None => None,
            };
            h.settle();
            h.mark_once();
            let everything = cmd == "clear";
            let sh = &mut h.book_mut().sheets[si];
            let keys: Vec<sheet::Pos> = sh
                .cells
                .keys()
                .filter(|p| match span {
                    Some((a, b)) => {
                        (a.row..=b.row).contains(&p.row) && (a.col..=b.col).contains(&p.col)
                    }
                    None => true,
                })
                .cloned()
                .collect();
            let n = keys.len();
            for p in keys {
                if everything {
                    sh.cells.remove(&p);
                } else {
                    // 書式は据え置きで、値と式だけ消す(set の Null と同じ)
                    let fmt = sh.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
                    let mut cell = sheet::Cell::input("");
                    cell.fmt = fmt;
                    sh.set(p, cell);
                }
            }
            sheet::recalc_book(h.book_mut(), si);
            h.mark_dirty();
            h.wrote(n);
            format!("{{\"ok\":true,\"cells\":{n}}}")
        }
        // 書式を読む(左上のセル。持っている項目だけ返る — pysheet の fmt と同じ鍵)
        "get_fmt" => {
            let (si, a, _) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let sh = &h.book().sheets[si];
            let mut out: Vec<String> = vec!["\"ok\":true".into()];
            if let Some(c) = sh.get(a) {
                let f = &c.fmt;
                for (k, on) in [
                    ("bold", f.bold),
                    ("italic", f.italic),
                    ("underline", f.underline),
                    ("strike", f.strike),
                    ("wrap", f.wrap),
                    ("shrink", f.shrink),
                ] {
                    if on {
                        out.push(format!("\"{k}\":true"));
                    }
                }
                if let Some(v) = &f.font {
                    out.push(format!("\"font\":{}", J::S(v.clone()).to_json()));
                }
                if let Some(sc) = f.size_c {
                    out.push(format!("\"size\":{}", sc as f64 / 100.0));
                }
                if let Some(v) = &f.color {
                    out.push(format!("\"color\":{}", J::S(v.clone()).to_json()));
                }
                if let Some(v) = &f.fill {
                    out.push(format!("\"fill\":{}", J::S(v.clone()).to_json()));
                }
                if let Some(v) = &f.number_format {
                    out.push(format!("\"number_format\":{}", J::S(v.clone()).to_json()));
                }
                if let Some(v) = f.align.as_xlsx() {
                    out.push(format!("\"horizontal\":\"{v}\""));
                }
                if let Some(v) = f.valign.as_xlsx() {
                    out.push(format!("\"vertical\":\"{v}\""));
                }
            }
            format!("{{{}}}", out.join(","))
        }
        // 書式を書く(範囲の全セル。**渡した項目だけ**変わり、null は消す)
        "set_fmt" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            h.settle();
            h.mark_once();
            let sh = &mut h.book_mut().sheets[si];
            for r in a.row..=b.row {
                for c in a.col..=b.col {
                    let p = sheet::Pos::new(r, c);
                    let mut cell = sh.get(p).cloned().unwrap_or_else(|| sheet::Cell::input(""));
                    let f = &mut cell.fmt;
                    if o.has("bold") {
                        f.bold = o.bool("bold").unwrap_or(false);
                    }
                    if o.has("italic") {
                        f.italic = o.bool("italic").unwrap_or(false);
                    }
                    if o.has("underline") {
                        f.underline = o.bool("underline").unwrap_or(false);
                    }
                    if o.has("strike") {
                        f.strike = o.bool("strike").unwrap_or(false);
                    }
                    if o.has("wrap") {
                        f.wrap = o.bool("wrap").unwrap_or(false);
                    }
                    if o.has("shrink") {
                        f.shrink = o.bool("shrink").unwrap_or(false);
                    }
                    if o.has("font") {
                        f.font = o.str("font");
                    }
                    if o.has("size") {
                        f.size_c = o.num("size").map(|x| (x * 100.0).round() as u32);
                    }
                    if o.has("color") {
                        f.color = o.str("color");
                        f.color_theme = None;
                    }
                    if o.has("fill") {
                        f.fill = o.str("fill");
                        f.fill_theme = None;
                    }
                    if o.has("number_format") {
                        f.number_format = o.str("number_format");
                    }
                    if o.has("horizontal") {
                        f.align = o
                            .str("horizontal")
                            .map(|x| sheet::model::HAlign::from_xlsx(&x))
                            .unwrap_or_default();
                    }
                    if o.has("vertical") {
                        f.valign = o
                            .str("vertical")
                            .map(|x| sheet::model::VAlign::from_xlsx(&x))
                            .unwrap_or_default();
                    }
                    sh.set(p, cell);
                }
            }
            h.mark_dirty();
            "{\"ok\":true}".into()
        }
        // 書式を消す(値は残る)。a1 省略はシート全部
        "clear_formats" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            h.settle();
            h.mark_once();
            let sh = &mut h.book_mut().sheets[si];
            let span = o.str("a1").and_then(|a1| {
                let mut it = a1.split(':');
                let a = it.next().and_then(sheet::Pos::parse)?;
                Some((a, it.next().and_then(sheet::Pos::parse).unwrap_or(a)))
            });
            let keys: Vec<sheet::Pos> = sh
                .cells
                .keys()
                .filter(|p| match span {
                    Some((a, b)) => {
                        (a.row..=b.row).contains(&p.row) && (a.col..=b.col).contains(&p.col)
                    }
                    None => true,
                })
                .cloned()
                .collect();
            let n = keys.len();
            for p in keys {
                let mut cell = sh.get(p).cloned().unwrap_or_default();
                cell.fmt = Default::default();
                sh.set(p, cell);
            }
            h.mark_dirty();
            format!("{{\"ok\":true,\"cells\":{n}}}")
        }
        // 列幅(字数)・行高(ポイント)。value を渡せば範囲の列・行に置く、
        // 渡さなければ「全部同じならその値、まちまちなら null」(xlwings と同じ)
        "col_width" | "row_height" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let cols = cmd == "col_width";
            match o.num("value") {
                Some(v) => {
                    h.settle();
                    h.mark_once();
                    let sh = &mut h.book_mut().sheets[si];
                    if cols {
                        for c in a.col..=b.col {
                            sh.col_width.insert(c, v as f32);
                        }
                    } else {
                        for r in a.row..=b.row {
                            sh.row_height.insert(r, v as f32);
                        }
                    }
                    h.mark_dirty();
                    "{\"ok\":true}".into()
                }
                None => {
                    let sh = &h.book().sheets[si];
                    let vals: Vec<Option<f32>> = if cols {
                        (a.col..=b.col)
                            .map(|c| sh.col_width.get(&c).copied().or(sh.default_col_width))
                            .collect()
                    } else {
                        (a.row..=b.row)
                            .map(|r| sh.row_height.get(&r).copied().or(sh.default_row_height))
                            .collect()
                    };
                    let same = vals.windows(2).all(|w| w[0] == w[1]);
                    match (same, vals.first().copied().flatten()) {
                        (true, Some(v)) => format!("{{\"ok\":true,\"value\":{v}}}"),
                        _ => "{\"ok\":true,\"value\":null}".into(),
                    }
                }
            }
        }
        // Ctrl+矢印相当(xlwings の end)。端は使っている範囲まで —
        // Excel の 1048576 行目には飛ばない(そこに用は無い)
        "end" => {
            let (si, a, _) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let Some(dir) = o.str("direction") else { return err("direction がありません") };
            let (dr, dc): (i64, i64) = match dir.as_str() {
                "up" => (-1, 0),
                "down" => (1, 0),
                "left" => (0, -1),
                "right" => (0, 1),
                _ => return err("direction は up / down / left / right"),
            };
            let sh = &h.book().sheets[si];
            let (rows, cols) = sh.extent();
            let (lim_r, lim_c) = (rows.max(1) as i64 - 1, cols.max(1) as i64 - 1);
            let inside = |r: i64, c: i64| r >= 0 && c >= 0 && r <= lim_r && c <= lim_c;
            let filled = |r: i64, c: i64| {
                inside(r, c) && !sh.value(sheet::Pos::new(r as u32, c as u32)).is_empty()
            };
            let (mut r, mut c) = (a.row as i64, a.col as i64);
            if filled(r, c) && filled(r + dr, c + dc) {
                // 地続きの最後まで
                while filled(r + dr, c + dc) {
                    r += dr;
                    c += dc;
                }
            } else {
                // 次の埋まったセルへ。無ければ使っている範囲の端
                loop {
                    if !inside(r + dr, c + dc) {
                        break;
                    }
                    r += dr;
                    c += dc;
                    if filled(r, c) {
                        break;
                    }
                }
            }
            let p = sheet::Pos::new(r.max(0) as u32, c.max(0) as u32);
            format!("{{\"ok\":true,\"a1\":{}}}", J::S(p.a1()).to_json())
        }
        other => match h.extra(other, &o) {
            Some(resp) => resp,
            None => err(&format!("知らない cmd: {other}")),
        },
    }
}

// ---- 共有の道具(calc と writer の写経を1本に。2026-08-12 段A) ----------
//
// 同名の関数が両アプリに写されていて、5つは既にずれていた(訳の抜け・
// 直しの入り忘れ)。**本体はここ、訳(ui::t!)と gpui は各アプリの包み**
// — run_with_timeout と同じ型。

/// 本文のフォント。**同梱せず、システムから探す**
/// (埋め込むと実行ファイルがフォントを配ることになり、免許の表示義務も付く)。
///
/// 起動時に一度だけ読み、以後は借りて使う。
/// 見つからなければ**その場で止める** — 日本語が豆腐になった画面を
/// 「動いている」と見せない。
pub fn font_data() -> &'static [u8] {
    static FONT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    FONT.get_or_init(|| {
        {
            // 文書が書体を指定していればそれを、無ければ機械にある日本語フォントを
            let (fam, _) = kumihan::font::for_document(None).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });
            kumihan::font::load(fam).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            })
        }
    })
}

/// PNG / JPEG の画素数 (幅, 高さ)。読めなければ None。
/// 中身は復号しない — 大きさを知るだけなら頭を見れば足りる。
pub fn image_px(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        // 署名8 + 長さ4 + "IHDR"4 の後に、幅・高さが BE で並ぶ
        let w = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
        return Some((w, h));
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2usize;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                return None;
            }
            let marker = bytes[i + 1];
            // 単独の印(長さ無し)は飛ばす
            if marker == 0xFF || (0xD0..=0xD9).contains(&marker) || marker == 0x01 {
                i += 2;
                continue;
            }
            let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            // SOF0〜3 に高さ・幅
            if matches!(marker, 0xC0..=0xC3) {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
                return Some((w, h));
            }
            i += 2 + len;
        }
        return None;
    }
    None
}

/// `RRGGBB` の1成分を 0.0〜1.0 で返す。読めない色は黒として扱う。
/// (calc の「文字列 → gpui::Rgba」はこの上の包み — gpui はここに入れない)
pub fn hex(s: &str, i: usize) -> f32 {
    s.get(i * 2..i * 2 + 2)
        .and_then(|h| u8::from_str_radix(h, 16).ok())
        .map(|v| v as f32 / 255.0)
        .unwrap_or(0.0)
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

/// 自分の名乗り(誰が開いているか)。user@host。
pub fn lock_identity() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "?".into());
    let host = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "?".into());
    format!("{user}@{host}")
}

/// 排他ロックの置き場(LibreOffice と同じ `.~lock.<名前>#`)。
/// ファイルサーバーの共有フォルダで「同時に開いて後勝ちで潰す」を防ぐ。
pub fn lock_path_for(p: &std::path::Path) -> PathBuf {
    let name = p.file_name().unwrap_or_default().to_string_lossy();
    p.with_file_name(format!(".~lock.{name}#"))
}

/// 先客のロックを読む(あれば名乗りを返す)。自分自身のロックは先客と見ない。
/// fallback は名乗りの読めないロックの呼び名(「誰か」の訳 — 言葉はアプリの領分)
pub fn foreign_lock(p: &std::path::Path, fallback: &str) -> Option<String> {
    let lp = lock_path_for(p);
    let raw = std::fs::read_to_string(lp).ok()?;
    let who = raw
        .split(',')
        .map(str::trim)
        .find(|t| !t.is_empty())
        .unwrap_or(fallback)
        .to_string();
    (who != lock_identity()).then_some(who)
}

/// 署名の鍵の置き場。calc と writer で共通の ~/.config/office/sign.key
/// (秘密鍵の種 32 バイト)
pub fn sign_key_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".config/office/sign.key")
}

/// 署名の添え書きの置き場。ファイルの隣の 名前.xlsx.sig / 名前.docx.sig
pub fn sig_path_for(p: &std::path::Path) -> PathBuf {
    let mut os = p.as_os_str().to_owned();
    os.push(".sig");
    PathBuf::from(os)
}

/// 鍵が用意できなかった理由。**文言はここでは作らない**(RunErr と同じ型)
#[cfg(unix)]
#[derive(Debug)]
pub enum KeyErr {
    /// 鍵ファイルが壊れている(~/.config/office/sign.key)
    Corrupt,
    /// /dev/urandom が読めない
    NoRandom(std::io::Error),
    /// 鍵ファイルが置けない
    CantStore(std::io::Error),
}

/// 署名の鍵を読む。無ければ作る(/dev/urandom の種。0600 で置く)。
/// **unix だけ**(/dev/urandom と 0600 は unix の作法。使うのは calc/writer)
#[cfg(unix)]
pub fn load_or_make_key() -> Result<ed25519_dalek::SigningKey, KeyErr> {
    let kp = sign_key_path();
    if let Ok(bytes) = std::fs::read(&kp) {
        let seed: [u8; 32] = bytes
            .get(..32)
            .and_then(|b| b.try_into().ok())
            .ok_or(KeyErr::Corrupt)?;
        return Ok(ed25519_dalek::SigningKey::from_bytes(&seed));
    }
    let mut seed = [0u8; 32];
    use std::io::Read as _;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut seed))
        .map_err(KeyErr::NoRandom)?;
    if let Some(dir) = kp.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&kp)
        .and_then(|mut f| f.write_all(&seed))
        .map_err(KeyErr::CantStore)?;
    Ok(ed25519_dalek::SigningKey::from_bytes(&seed))
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
