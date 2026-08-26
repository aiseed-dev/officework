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
//! [`Host::extra`] に残す。ソケットを開く・スレッド・30ms の汲み取りはアプリ側
//! (calc/src/rpc.rs)のまま — ここは意味だけ。

use std::path::PathBuf;

/// 文書の中の表で、セル関数を使う(SEKKEI「エンジンの統一」3段目)
/// 上書きの前の控え(バージョン履歴)。writer と calc で1本
pub mod history;
pub mod table;

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
    fn book(&self) -> &kumihan::book::Book;
    fn book_mut(&mut self) -> &mut kumihan::book::Book;
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
    fn after_write(&mut self, _si: usize, _written: &[kumihan::book::Pos]) {}
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
    /// ブックを閉じる(新しい空のブックに戻る)。未保存の確認は口の側で済み
    fn close_book(&mut self) -> Result<(), String> {
        Err("この口では close はできません".into())
    }

    /// アプリの版(ping の返事に載せる)。ファイルの口は名乗らなくてよい
    fn version(&self) -> &'static str {
        ""
    }
    /// いま選んでいる範囲(シート, 左上, 右下)。画面のあるアプリだけが持つ
    fn selection(&self) -> Option<(usize, kumihan::book::Pos, kumihan::book::Pos)> {
        None
    }
    /// 選択を動かして見せる。画面のあるアプリだけ
    fn select(&mut self, _si: usize, _a: kumihan::book::Pos, _b: kumihan::book::Pos) -> Result<(), String> {
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
    /// **ブック全体**を1つの PDF に(頁番号はブック通し)
    fn book_to_pdf(&mut self, _p: &std::path::Path) -> Result<String, String> {
        Err("この口では to_pdf はできません".into())
    }
    /// シートの複製(タブのメニューと同じ作法)。返りは写しの名前
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
        _a: kumihan::book::Pos,
        _b: kumihan::book::Pos,
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
                "{{\"ok\":true,\"path\":{},\"sheets\":{},\"active\":{},\"read_only_rec\":{}}}",
                match h.path() {
                    Some(p) => J::S(p.display().to_string()).to_json(),
                    None => "null".into(),
                },
                sheets.to_json(),
                h.active(),
                h.book().read_only_rec
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
                    cols.push(match sh.value(kumihan::book::Pos::new(r, c)) {
                        kumihan::book::Value::Number(n) => J::N(n),
                        kumihan::book::Value::Bool(x) => J::B(x),
                        kumihan::book::Value::Empty => J::Null,
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
                        match sh.get(kumihan::book::Pos::new(r, c)).and_then(|x| x.formula.clone()) {
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
            let Some(origin) = kumihan::book::Pos::parse(a1.split(':').next().unwrap_or(&a1)) else {
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
            let mut written: Vec<kumihan::book::Pos> = Vec::new();
            for (dr, row) in grid.iter().enumerate() {
                for (dc, v) in row.iter().enumerate() {
                    let p = kumihan::book::Pos::new(origin.row + dr as u32, origin.col + dc as u32);
                    let sh = &mut h.book_mut().sheets[si];
                    let fmt = sh.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
                    let mut cell = match v {
                        J::Null => kumihan::book::Cell::input(""),
                        J::N(x) => kumihan::book::Cell::input(&J::N(*x).to_json()),
                        J::B(x) => kumihan::book::Cell::input(if *x { "TRUE" } else { "FALSE" }),
                        J::S(s) => kumihan::book::Cell::input(s),
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
            kumihan::calc::recalc_book(h.book_mut(), si);
            h.mark_dirty();
            h.wrote(n);
            format!("{{\"ok\":true,\"cells\":{n}}}")
        }
        // 左上から地続きの表の大きさ(xlwings の expand='table')
        "expand" => {
            let Some(a1) = o.str("a1") else { return err("a1 がありません") };
            let Some(p) = kumihan::book::Pos::parse(&a1) else { return err("a1 が読めません") };
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let sh = &h.book().sheets[si];
            let filled = |r: u32, c: u32| !sh.value(kumihan::book::Pos::new(r, c)).is_empty();
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
            kumihan::calc::recalc_book(h.book_mut(), si);
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
        // シートを PDF に(印刷設定に従う。効かせた物は note で言う)。
        // whole=true で**ブック全体**を1つに束ねる(頁番号はブック通し)
        "to_pdf" => {
            let Some(p) = o.str("path") else { return err("path がありません") };
            if o.bool("whole").unwrap_or(false) {
                h.settle();
                return match h.book_to_pdf(std::path::Path::new(&p)) {
                    Ok(note) => format!("{{\"ok\":true,\"note\":{}}}", J::S(note).to_json()),
                    Err(e) => err(&e),
                };
            }
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
        // ブックを閉じる(未保存があれば断る — new / open と同じ作法)
        "close" => {
            h.settle();
            if h.dirty() {
                return err("未保存の変更があります(保存するか、捨ててから)");
            }
            match h.close_book() {
                Ok(()) => "{\"ok\":true}".into(),
                Err(e) => err(&e),
            }
        }
        // シートの表(テーブル)[[名前, 範囲], …]
        "sheet_tables" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let items: Vec<J> = h.book().sheets[si]
                .tables
                .iter()
                .map(|t| {
                    J::A(vec![
                        J::S(t.name.clone()),
                        J::S(format!("{}:{}", t.a.a1(), t.b.a1())),
                    ])
                })
                .collect();
            format!("{{\"ok\":true,\"tables\":{}}}", J::A(items).to_json())
        }
        // 表のデザイン(見出し行の色・縞模様・最初と最後の列)。**画面のボタンと
        // 同じ実装**(sheet::tabledesign)を呼ぶ — 記録した行がそのまま走る
        "table_style" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let Some(what) = o.str("what").and_then(|s| sheet::tabledesign::Deco::from_name(&s))
            else {
                return err("what は header / band_row / band_col / first_col / last_col のどれか");
            };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            let on = o.bool("on").unwrap_or(true);
            h.settle();
            h.mark_once();
            let n = sheet::tabledesign::deco(&mut h.book_mut().sheets[si], a, b, what, on);
            h.mark_dirty();
            h.wrote(n);
            format!("{{\"ok\":true,\"cells\":{n}}}")
        }
        // 合計行(選択の下に =SUM を足す)。下に中身があれば断る
        "table_total" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            if h.book().sheets[si].protected {
                return err("シートが保護されています");
            }
            if sheet::tabledesign::below_used(&h.book().sheets[si], a, b) {
                return err("すぐ下の行に中身があります(空けてから — 黙って上書きしません)");
            }
            h.settle();
            h.mark_once();
            let n = sheet::tabledesign::add_total_row(&mut h.book_mut().sheets[si], a, b);
            h.mark_dirty();
            h.wrote(n);
            format!("{{\"ok\":true,\"cells\":{n}}}")
        }
        // 表オブジェクトを外して普通の範囲に戻す(書式と式は残る)
        "table_to_range" => {
            let (si, a, _b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            h.settle();
            h.mark_once();
            match sheet::tabledesign::to_range(&mut h.book_mut().sheets[si], a) {
                None => err("そこに表はありません"),
                Some(name) => {
                    h.mark_dirty();
                    format!("{{\"ok\":true,\"name\":{}}}", J::S(name).to_json())
                }
            }
        }
        // 開いた人に「見るだけ」を勧める旗(鍵ではない)
        "read_only_rec" => {
            let on = o.bool("on").unwrap_or(true);
            h.settle();
            h.mark_once();
            h.book_mut().read_only_rec = on;
            h.mark_dirty();
            format!("{{\"ok\":true,\"on\":{on}}}")
        }
        // 印刷の設定(紙・向き・余白 mm・印刷範囲)
        "page_setup" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            let sh = &h.book().sheets[si];
            let margins = match sh.margins_mm {
                Some((l, r, t, b)) => {
                    format!("[{l},{r},{t},{b}]")
                }
                None => "null".into(),
            };
            let areas = J::A(
                sh.print_areas
                    .iter()
                    .map(|(a, b)| J::S(format!("{}:{}", a.a1(), b.a1())))
                    .collect(),
            );
            // タイトル列は列の名前で返す(openpyxl と同じ "A:B" の形)
            let letter = |c: u32| {
                let a1 = kumihan::book::Pos::new(0, c).a1();
                a1.trim_end_matches(|ch: char| ch.is_ascii_digit()).to_string()
            };
            format!(
                "{{\"ok\":true,\"paper\":{},\"landscape\":{},\"margins_mm\":{},\"print_area\":{},\"title_rows\":{},\"title_cols\":{}}}",
                match sh.paper_size {
                    Some(c) => J::N(f64::from(c)).to_json(),
                    None => "null".into(),
                },
                sh.landscape,
                margins,
                areas.to_json(),
                match sh.print_title_rows {
                    Some((a, b)) => J::S(format!("{}:{}", a + 1, b + 1)).to_json(),
                    None => "null".into(),
                },
                match sh.print_title_cols {
                    Some((a, b)) => J::S(format!("{}:{}", letter(a), letter(b))).to_json(),
                    None => "null".into(),
                }
            )
        }
        // 印刷の設定を**書く**(2026-08-16。読むだけだったので、レイアウトタブの
        // 操作が記録しても走らなかった)。渡した項目だけ変わる
        "set_page_setup" => {
            let si = match sheet_index(h, &o) {
                Ok(i) => i,
                Err(e) => return e,
            };
            h.settle();
            h.mark_once();
            let sh = &mut h.book_mut().sheets[si];
            if o.has("paper") {
                sh.paper_size = o.num("paper").map(|x| x as u32);
            }
            if o.has("landscape") {
                sh.landscape = o.bool("landscape").unwrap_or(false);
            }
            if o.has("scale") {
                sh.print_scale = o.num("scale").map(|x| x.clamp(10.0, 400.0) as u32);
                if sh.print_scale.is_some() {
                    // 「ページに合わせる」と倍率は両立しない(xlsx も片方だけ)
                    sh.fit_to_w = None;
                    sh.fit_to_h = None;
                }
            }
            if o.has("fit_to_w") {
                sh.fit_to_w = o.num("fit_to_w").map(|x| x as u32);
            }
            if o.has("fit_to_h") {
                sh.fit_to_h = o.num("fit_to_h").map(|x| x as u32);
            }
            if o.has("print_gridlines") {
                sh.print_gridlines = o.bool("print_gridlines").unwrap_or(false);
            }
            if o.has("print_headings") {
                sh.print_headings = o.bool("print_headings").unwrap_or(false);
            }
            // 余白は4つの鍵で受ける(**浅い JSON の読み手に配列は無い** —
            // 依存を増やさない流儀のまま、mm を1つずつ)
            if ["margin_l", "margin_r", "margin_t", "margin_b"].iter().any(|k| o.has(k)) {
                let (l0, r0, t0, b0) = sh.margins_mm.unwrap_or((20.0, 20.0, 20.0, 20.0));
                let mm = |k: &str, now: f32| o.num(k).map(|x| x as f32).unwrap_or(now);
                sh.margins_mm = Some((
                    mm("margin_l", l0),
                    mm("margin_r", r0),
                    mm("margin_t", t0),
                    mm("margin_b", b0),
                ));
            }
            h.mark_dirty();
            "{\"ok\":true}".into()
        }
        // セルのコメント(xlwings の note)。text を渡せば置く、null で消す
        "note" => {
            let (si, a, _) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            if !o.has("text") {
                let t = h.book().sheets[si].comments.get(&a).cloned();
                return format!(
                    "{{\"ok\":true,\"text\":{}}}",
                    match t {
                        Some(v) => J::S(v.flatten()).to_json(),
                        None => "null".into(),
                    }
                );
            }
            h.settle();
            h.mark_once();
            let sh = &mut h.book_mut().sheets[si];
            match o.str("text").filter(|v| !v.is_empty()) {
                Some(v) => {
                    sh.comments.insert(a, v.into());
                }
                None => {
                    sh.comments.remove(&a);
                }
            }
            h.mark_dirty();
            "{\"ok\":true}".into()
        }
        // セルのリンク。url を渡せば置く、null で消す
        "hyperlink" => {
            let (si, a, _) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            if !o.has("url") {
                let t = h.book().sheets[si].links.get(&a).cloned();
                return format!(
                    "{{\"ok\":true,\"url\":{}}}",
                    match t {
                        Some(v) => J::S(v).to_json(),
                        None => "null".into(),
                    }
                );
            }
            h.settle();
            h.mark_once();
            let sh = &mut h.book_mut().sheets[si];
            match o.str("url").filter(|v| !v.is_empty()) {
                Some(v) => {
                    sh.links.insert(a, v);
                }
                None => {
                    sh.links.remove(&a);
                }
            }
            h.mark_dirty();
            "{\"ok\":true}".into()
        }
        // 行・列のグループ化。level を渡せば掛ける(0 で外す)、hidden で畳む
        "group" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let rows = o.str("axis").as_deref() != Some("columns");
            let level = o.num("level").unwrap_or(1.0).clamp(0.0, 7.0) as u8;
            let hidden = o.bool("hidden").unwrap_or(false);
            h.settle();
            h.mark_once();
            let sh = &mut h.book_mut().sheets[si];
            let range: Vec<u32> = if rows {
                (a.row..=b.row).collect()
            } else {
                (a.col..=b.col).collect()
            };
            for k in range {
                let (outline, hid) = if rows {
                    (&mut sh.row_outline, &mut sh.row_hidden)
                } else {
                    (&mut sh.col_outline, &mut sh.col_hidden)
                };
                if level == 0 {
                    outline.remove(&k);
                    hid.remove(&k);
                } else {
                    outline.insert(k, level);
                    if hidden {
                        hid.insert(k);
                    } else {
                        hid.remove(&k);
                    }
                }
            }
            h.mark_dirty();
            "{\"ok\":true}".into()
        }
        // 配列式(スピル)か。左上のセルで見る
        "array_info" => {
            let (si, a, _) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let sh = &h.book().sheets[si];
            match sh.cse.get(&a) {
                Some((rows, cols)) => {
                    let f = sh
                        .get(a)
                        .and_then(|c| c.formula.clone())
                        .map(|f| format!("={f}"))
                        .unwrap_or_default();
                    format!(
                        "{{\"ok\":true,\"has_array\":true,\"formula\":{},\"rows\":{rows},\"cols\":{cols}}}",
                        J::S(f).to_json()
                    )
                }
                None => "{\"ok\":true,\"has_array\":false}".into(),
            }
        }
        // 範囲の紙の上の場所と大きさ(ポイント)。列幅・行高から測る —
        // 画面の画素ではなくモデルの寸法(画像・図形の置き場所の計算に使う)
        "layout" => {
            let (si, a, b) = match target(h, &o) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let sh = &h.book().sheets[si];
            // xlsx の列幅は字数。字数 → px は 7倍+5(Excel の換算)、px → pt は 72/96
            let col_pt = |c: u32| -> f64 {
                let chars = sh
                    .col_width
                    .get(&c)
                    .copied()
                    .or(sh.default_col_width)
                    .unwrap_or(8.43);
                (f64::from(chars) * 7.0 + 5.0) * 72.0 / 96.0
            };
            let row_pt = |r: u32| -> f64 {
                f64::from(sh.row_height.get(&r).copied().or(sh.default_row_height).unwrap_or(15.0))
            };
            let left: f64 = (0..a.col).map(col_pt).sum();
            let top: f64 = (0..a.row).map(row_pt).sum();
            let width: f64 = (a.col..=b.col).map(col_pt).sum();
            let height: f64 = (a.row..=b.row).map(row_pt).sum();
            format!(
                "{{\"ok\":true,\"left\":{left},\"top\":{top},\"width\":{width},\"height\":{height}}}"
            )
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
            kumihan::calc::recalc_book(h.book_mut(), si);
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
                    _ => match kumihan::book::Pos::parse(&at) {
                        Some(p) => p.row,
                        None => return err(&format!("行の指し方が読めません: {at:?}")),
                    },
                }
            } else {
                match kumihan::book::Pos::parse(&format!("{}1", at.trim())) {
                    Some(p) => p.col,
                    None => match kumihan::book::Pos::parse(&at) {
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
            kumihan::calc::recalc_book(h.book_mut(), si);
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
            h.book_mut().sheets[si].images_new.push(kumihan::book::SheetImage {
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
                    s.names.iter().map(|d| {
                        let (n, r) = (&d.name, &d.range);
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
                s.names.retain(|d| d.name != name);
            }
            h.book_mut().sheets[si].names.push(kumihan::book::DefinedName::new(name, reference));
            kumihan::calc::recalc_book(h.book_mut(), si);
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
                s.names.retain(|d| d.name != name);
                removed |= s.names.len() != before;
            }
            if removed {
                let si = h.active();
                kumihan::calc::recalc_book(h.book_mut(), si);
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
        // 結合する(家の作法 — アプリの結合と同じ kumihan::book の merge)
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
            kumihan::calc::recalc_book(h.book_mut(), si);
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
                    let a = match it.next().and_then(kumihan::book::Pos::parse) {
                        Some(p) => p,
                        None => return err("a1 が読めません"),
                    };
                    let b = match it.next() {
                        Some(t) => match kumihan::book::Pos::parse(t) {
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
            let keys: Vec<kumihan::book::Pos> = sh
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
                    let mut cell = kumihan::book::Cell::input("");
                    cell.fmt = fmt;
                    sh.set(p, cell);
                }
            }
            kumihan::calc::recalc_book(h.book_mut(), si);
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
                    ("subscript", f.subscript),
                    ("rtl_text", f.rtl_text),
                    ("formula_hidden", f.formula_hidden),
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
                if f.indent > 0 {
                    out.push(format!("\"indent\":{}", f.indent));
                }
                if let Some(v) = f.rotation {
                    out.push(format!("\"rotation\":{v}"));
                }
                // ロックは**既定が真**なので、外れているときだけ言う
                if f.unlocked {
                    out.push("\"locked\":false".into());
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
                    let p = kumihan::book::Pos::new(r, c);
                    let mut cell = sh.get(p).cloned().unwrap_or_else(|| kumihan::book::Cell::input(""));
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
                            .map(|x| kumihan::book::HAlign::from_xlsx(&x))
                            .unwrap_or_default();
                    }
                    if o.has("vertical") {
                        f.valign = o
                            .str("vertical")
                            .map(|x| kumihan::book::VAlign::from_xlsx(&x))
                            .unwrap_or_default();
                    }
                    // **記録した操作が走るために足した**(2026-08-16)。
                    // 下付き・右横書き・字下げ・文字の向き・ロック・式を隠す
                    if o.has("subscript") {
                        f.subscript = o.bool("subscript").unwrap_or(false);
                    }
                    if o.has("rtl_text") {
                        f.rtl_text = o.bool("rtl_text").unwrap_or(false);
                    }
                    if o.has("indent") {
                        f.indent = o.num("indent").unwrap_or(0.0).clamp(0.0, 250.0) as u8;
                    }
                    if o.has("rotation") {
                        f.rotation = o.num("rotation").map(|x| x as i32);
                    }
                    if o.has("locked") {
                        f.unlocked = !o.bool("locked").unwrap_or(true);
                    }
                    if o.has("formula_hidden") {
                        f.formula_hidden = o.bool("formula_hidden").unwrap_or(false);
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
                let a = it.next().and_then(kumihan::book::Pos::parse)?;
                Some((a, it.next().and_then(kumihan::book::Pos::parse).unwrap_or(a)))
            });
            let keys: Vec<kumihan::book::Pos> = sh
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
                inside(r, c) && !sh.value(kumihan::book::Pos::new(r as u32, c as u32)).is_empty()
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
            let p = kumihan::book::Pos::new(r.max(0) as u32, c.max(0) as u32);
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

/// 署名の鍵の置き場。calc と writer で共通の ~/.config/officework/sign.key
/// (秘密鍵の種 32 バイト)
pub fn sign_key_path() -> PathBuf {
    pyrun::config_dir().join("sign.key")
}

/// 署名の添え書きの置き場。ファイルの隣の 名前.xlsx.sig / 名前.docx.sig
pub fn sig_path_for(p: &std::path::Path) -> PathBuf {
    let mut os = p.as_os_str().to_owned();
    os.push(".sig");
    PathBuf::from(os)
}

/// 鍵が用意できなかった理由。**文言はここでは作らない**(RunErr と同じ型)
#[derive(Debug)]
pub enum KeyErr {
    /// 鍵ファイルが壊れている(~/.config/officework/sign.key)
    Corrupt,
    /// OS の乱数が取れない
    NoRandom(String),
    /// 鍵ファイルが置けない
    CantStore(std::io::Error),
}

/// 署名の鍵を読む。無ければ作る(OS の乱数を種にする)。
///
/// **どの機械でも同じ強さの鍵**を作る。種は `getrandom`(unix は
/// getrandom(2)/urandom、Windows は BCryptGenRandom)— 2026-08-17 に
/// calc/writer を Windows の的に足すまで、ここは `/dev/urandom` 直読みの
/// `#[cfg(unix)]` で、**Windows では calc も writer も組めなかった**。
/// 時刻や pid で代用しないのは、これが署名の鍵だから。
///
/// 置き方だけは的で違う: unix は 0600 で作る。Windows に mode は無く、
/// 置き場(利用者の profile の下)が既に本人だけの物なので `create_new` のみ。
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
    getrandom::fill(&mut seed).map_err(|e| KeyErr::NoRandom(e.to_string()))?;
    if let Some(dir) = kp.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    use std::io::Write as _;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        o.mode(0o600);
    }
    o.open(&kp)
        .and_then(|mut f| f.write_all(&seed))
        .map_err(KeyErr::CantStore)?;
    Ok(ed25519_dalek::SigningKey::from_bytes(&seed))
}

/// **自動復旧の控えの置き場。** `~/.config/officework/recover/`。
///
/// 本家(ローカルの Excel / Word)と同じ考え方で、**開いているファイルを
/// 勝手に上書きしない**。落ちたとき・電源が切れたときに失う分を減らす
/// ための別の控えで、無事に保存できたら消します。上書きしてしまうと
/// 「保存していないつもりの変更」が原本に入り、Ctrl+Z でも戻せません
/// — 帳票では取り返しがつきません。
///
/// **表と文章で同じ置き場です**(2026-08-21)。前は表にしか無く、
/// 道の作り方も表の中に閉じていました。文章にも要るので出しました。
pub fn recover_dir() -> PathBuf {
    pyrun::config_dir().join("recover")
}

/// いまの文書・ブックの控えの道。名前は**元の道から作る**ので、同じ
/// ファイルを開き直したときに同じ控えを指します。
///
/// `ext` は控えの拡張子です(表は `xlsx`、文章は `adoc`)。同じ場所に
/// 両方が並ぶので、種類は拡張子で見分けます。
pub fn recover_path_for(orig: Option<&std::path::Path>, ext: &str, untitled: &str) -> PathBuf {
    let key = match orig {
        Some(p) => {
            // 道をそのまま名前にはできないので、道の hash と見える名前
            let mut h: u64 = 1469598103934665603;
            for b in p.to_string_lossy().as_bytes() {
                h ^= *b as u64;
                h = h.wrapping_mul(1099511628211);
            }
            let stem = p
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "doc".into());
            format!("{stem}-{h:016x}")
        }
        None => untitled.to_string(),
    };
    recover_dir().join(format!("{key}.{ext}"))
}

/// 無事に保存できたら控えは要りません(消し忘れると次の起動で
/// 「落ちた後です」と嘘を言います)
pub fn drop_recover(orig: Option<&std::path::Path>, ext: &str, untitled: &str) {
    let p = recover_path_for(orig, ext, untitled);
    let _ = std::fs::remove_file(&p);
    let _ = std::fs::remove_file(p.with_extension("path"));
}

/// 起動のときに残っている控え(前回落ちた跡)。`(見える名前, 控えの道)`。
///
/// `ext` で種類を絞ります — 表の画面に文章の控えを出しても開けません。
pub fn stale_recovers(ext: &str) -> Vec<(String, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(recover_dir()) else { return Vec::new() };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some(ext) {
            continue;
        }
        let orig = std::fs::read_to_string(p.with_extension("path")).ok();
        let name = orig.unwrap_or_else(|| {
            p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
        });
        out.push((name, p));
    }
    out.sort();
    out
}

/// 控えの隣に「元はどのファイルか」を書き添えます。復旧のときに
/// 「どのファイルの控えか」を言うためです
pub fn note_recover_origin(record: &std::path::Path, orig: Option<&std::path::Path>) {
    if let Some(o) = orig {
        let _ = std::fs::write(record.with_extension("path"), o.to_string_lossy().as_bytes());
    }
}

/// 署名を検めた/添えた結果。**文言はここでは作らない**([`KeyErr`] と同じ型)
#[derive(Debug)]
pub enum Signed {
    /// 既にある署名が有効だった。中身は署名した人の名乗り
    Verified(String),
    /// 署名して添えた。中身は添えたファイルの名前
    Wrote(String),
}

/// 署名がうまくいかなかった理由。**文言はここでは作らない**
#[derive(Debug)]
pub enum SignErr {
    /// 署名するファイルが読めない
    Read(std::io::Error),
    /// 添え書きが置けない
    Write(std::io::Error),
    /// 鍵が用意できない
    Key(KeyErr),
}

/// **隣の `.sig` で署名する・検める。**
///
/// 押すたびにこの1本を通ります。既にある署名が有効なら検めた結果を返し、
/// 無い・壊れている・中身が変わっているなら署名し直して添えます。
///
/// # なぜここにあるのか(2026-08-21)
///
/// 前は calc と writer が**同じ 40 行を別々に持って**いました。写しは
/// ずれます — 実際、表の側だけ2つの文言が `ui::tf!` ではなく `format!` に
/// なっていて、**14 言語で日本語がそのまま出て**いました。設計が
/// `ai-where` で挙げたのと同じ形です。
///
/// 中身をここに1本にして、**文言だけをアプリに残しました**。文言を
/// 残すのは、訳の走査(`ui/gen_i18n.py`)が `calc/src` `writer/src`
/// `ui/src` しか見ないからです。ここに置くと生きている訳が
/// 「使われていない」と数えられます。
pub fn sign_or_verify(path: &std::path::Path) -> Result<Signed, SignErr> {
    use ed25519_dalek::{Signer as _, Verifier as _};
    let bytes = std::fs::read(path).map_err(SignErr::Read)?;
    let sp = sig_path_for(path);
    // 既にある署名を検める
    if let Ok(txt) = std::fs::read_to_string(&sp) {
        let field = |k: &str| -> Option<String> {
            txt.lines().find(|l| l.starts_with(k)).map(|l| l[k.len()..].trim().to_string())
        };
        let ok = (|| -> Option<(String, bool)> {
            let signer = field("signer:")?;
            let vk: [u8; 32] = unhex(&field("pubkey:")?)?.try_into().ok()?;
            let sg: [u8; 64] = unhex(&field("sig:")?)?.try_into().ok()?;
            let vk = ed25519_dalek::VerifyingKey::from_bytes(&vk).ok()?;
            let sig = ed25519_dalek::Signature::from_bytes(&sg);
            Some((signer, vk.verify(&bytes, &sig).is_ok()))
        })();
        if let Some((signer, true)) = ok {
            return Ok(Signed::Verified(signer));
        }
    }
    // 無い・壊れている・中身が変わった → 署名し(直し)て添える
    let key = load_or_make_key().map_err(SignErr::Key)?;
    let sig = key.sign(&bytes);
    let txt = format!(
        "office-sign v1\nsigner: {}\npubkey: {}\nsig: {}\n",
        lock_identity(),
        to_hex(key.verifying_key().as_bytes()),
        to_hex(&sig.to_bytes())
    );
    std::fs::write(&sp, txt).map_err(SignErr::Write)?;
    Ok(Signed::Wrote(sp.file_name().unwrap_or_default().to_string_lossy().into_owned()))
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
fn target(h: &impl Host, o: &Jobj) -> Result<(usize, kumihan::book::Pos, kumihan::book::Pos), String> {
    let si = sheet_index(h, o)?;
    let a1 = o.str("a1").ok_or_else(|| err("a1 がありません"))?;
    let mut it = a1.split(':');
    let a = it.next().and_then(kumihan::book::Pos::parse).ok_or_else(|| err("a1 が読めません"))?;
    let b = match it.next() {
        Some(t) => kumihan::book::Pos::parse(t).ok_or_else(|| err("a1 が読めません"))?,
        None => a,
    };
    Ok((si, a, b))
}

// ---- 受け口(AF_UNIX)の世話 ------------------------------------------------
//
// **ソケットを開いて行を積む所は、どのアプリでも同じ**なので1本にします
// (2026-08-19 発注者「calc が rpc に対応しているのであれば、writer でも
// 使えるようにして」)。捌くのはアプリの主スレッドの仕事なので、そちらは
// 各アプリに残します — gpui の型を持ち込まないためです。

/// 受け口に来た1要求。答えは `reply` へ返します。
pub struct Req {
    pub line: String,
    pub reply: std::sync::mpsc::Sender<String>,
}

/// 溜まった要求。アプリの主スレッドが取り出して捌きます。
pub type Queue = std::sync::Arc<std::sync::Mutex<Vec<Req>>>;

/// **すでに動いている本体に話しかける**(2026-08-20)。
///
/// 返事の1行を返します。誰も居なければ `None` です。
///
/// # なぜ要るか — ファイルの関連付け
///
/// 統合してからは「タブ1つ = ファイル1つ」です。ファイルの管理画面で
/// 2枚目を開いたときに**窓がもう1つ立つ**と、その形が崩れます。
/// 開いている本体に渡して、タブとして開くのが正しい姿です。
///
/// 待つのは 500 ミリ秒までです。相手が居るのに黙っている(固まっている)
/// ときに、こちらまで止まらないためです。
#[cfg(unix)]
pub fn ask(app: &str, line: &str) -> Option<String> {
    use std::io::{BufRead as _, Write as _};
    let mut c = std::os::unix::net::UnixStream::connect(sock_path(app)).ok()?;
    let wait_for = Some(std::time::Duration::from_millis(500));
    let _ = c.set_read_timeout(wait_for);
    let _ = c.set_write_timeout(wait_for);
    c.write_all(line.as_bytes()).ok()?;
    c.write_all(b"\n").ok()?;
    c.flush().ok()?;
    let mut reply = String::new();
    std::io::BufReader::new(c).read_line(&mut reply).ok()?;
    let reply = reply.trim().to_string();
    if reply.is_empty() {
        return None;
    }
    Some(reply)
}

/// **受け口を開く。** 開けたら真。
///
/// 開けなくてもアプリは動きます(黙らず標準エラーにだけ言います)。
/// `app` は名乗りで、`$XDG_RUNTIME_DIR/officework/<app>.sock` になります。
///
/// **Windows では作りません(2026-08-20 発注者)。** `#[cfg(unix)]` は
/// 移植待ちではなく決めです — 聞き続ける物を、使い道を決めないまま
/// 増やしません。
#[cfg(unix)]
pub fn listen(app: &'static str, queue: Queue) -> bool {
    use std::io::{BufRead as _, Write as _};
    let path = sock_path(app);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // **動いている本体の受け口を取り上げません**(2026-08-20 に見つけた)。
    // 前は残骸かどうかを見ずに消していたので、2つ目を起こすと1つ目の
    // 受け口が黙って自分に移り、道具の話し相手が入れ替わっていました。
    // *話しかけてみて、返事があれば生きている*ので、そのままにします
    if path.exists() {
        if ask(app, "{\"cmd\":\"ping\"}").is_some() {
            eprintln!("officework はすでに動いています({app})。受け口はそのままにします");
            return false;
        }
        let _ = std::fs::remove_file(&path); // 返事が無い = 前回の残骸
    }
    let listener = match std::os::unix::net::UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("officework の受け口が開けません({app}): {e}");
            return false;
        }
    };
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { continue };
            let q = queue.clone();
            std::thread::spawn(move || {
                let Ok(mut w) = conn.try_clone() else { return };
                let r = std::io::BufReader::new(conn);
                for line in r.lines() {
                    let Ok(line) = line else { break };
                    if line.trim().is_empty() {
                        continue;
                    }
                    let (tx, rx) = std::sync::mpsc::channel();
                    q.lock().expect("受け口の錠").push(Req { line, reply: tx });
                    // 主スレッドが捌くのを待つ(5秒で諦める)
                    let resp = rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap_or_else(|_| {
                            format!("{{\"err\":\"{app} が応じません(忙しいか、閉じかけ)\"}}")
                        });
                    if w.write_all(resp.as_bytes()).is_err() {
                        break;
                    }
                    let _ = w.write_all(b"\n");
                }
            });
        }
    });
    true
}

/// **`HOME` を触る試験を直列に回すための錠**(2026-08-21)。
///
/// 鍵の置き場も控えの置き場も `HOME` の下です。同時に走ると別の試験が
/// 立てた `HOME` を見て落ちます(実際に落ちました)。毒された錠は中身を
/// 取り出して使います — 1本落ちたせいで残りが「錠が毒された」で落ちると、
/// 本当の原因が見えなくなります。
#[cfg(test)]
static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
fn own_home() -> std::sync::MutexGuard<'static, ()> {
    HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod sign_tests {
    use super::*;

    /// 署名して、もう一度押したら検められること。**中身を書き替えたら
    /// 検めが落ちる**ことも見る(落ちなければ署名の意味がありません)
    #[test]
    fn sign_then_verify() {
        let _home = own_home();
        // 鍵の置き場は HOME の下。試験どうしがぶつからないよう別の家にする
        let home = std::env::temp_dir().join(format!("ops-sign-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let from = std::env::var_os("HOME");
        unsafe { std::env::set_var("HOME", &home) };

        let f = home.join("報告書.adoc");
        std::fs::write(&f, "= 報告書\n\n本文です。\n").unwrap();

        // 1回目 — 署名を添える
        match sign_or_verify(&f) {
            Ok(Signed::Wrote(name)) => assert!(name.ends_with(".sig"), "添え書きの名前: {name}"),
            other_of => panic!("1回目は署名するはず: {other_of:?}"),
        }
        assert!(sig_path_for(&f).exists(), "隣に .sig が置かれる");

        // 2回目 — 中身が同じなら検められる
        match sign_or_verify(&f) {
            Ok(Signed::Verified(who)) => assert_eq!(who, lock_identity()),
            other_of => panic!("2回目は検めるはず: {other_of:?}"),
        }

        // 中身を書き替えたら検めが落ち、署名し直す
        std::fs::write(&f, "= 報告書\n\n書き替えました。\n").unwrap();
        match sign_or_verify(&f) {
            Ok(Signed::Wrote(_)) => {}
            other_of => panic!("中身が変わったら署名し直すはず: {other_of:?}"),
        }

        // 無いファイルは読めないと言う(黙って成功にしない)
        match sign_or_verify(&home.join("ありません.adoc")) {
            Err(SignErr::Read(_)) => {}
            other_of => panic!("読めないと言うはず: {other_of:?}"),
        }

        match from {
            Some(v) => unsafe { std::env::set_var("HOME", v) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        let _ = std::fs::remove_dir_all(&home);
    }
}

#[cfg(test)]
mod recover_tests {
    use super::*;

    /// **控えの道は元のファイルごとに決まる。** 同じファイルを開き直したら
    /// 同じ控えを指し、別のファイルなら別の控えになります。
    ///
    /// 種類は拡張子で見分けます — 同じ置き場に表と文章の控えが並ぶので、
    /// 混ざると表の画面に文書の控えが出て、開けません。
    #[test]
    fn backup_path_and_listing() {
        let _home = own_home();
        let home = std::env::temp_dir().join(format!("ops-rec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let from = std::env::var_os("HOME");
        unsafe { std::env::set_var("HOME", &home) };

        let a = std::path::Path::new("/tmp/報告書.adoc");
        let b = std::path::Path::new("/tmp/別の報告書.adoc");
        assert_eq!(recover_path_for(Some(a), "adoc", "無題"), recover_path_for(Some(a), "adoc", "無題"));
        assert_ne!(recover_path_for(Some(a), "adoc", "無題"), recover_path_for(Some(b), "adoc", "無題"));
        // 名前の無い文書は決まった名前(開き直しても同じ控えを指す)
        assert!(recover_path_for(None, "adoc", "未保存の文書")
            .to_string_lossy()
            .contains("未保存の文書"));

        // 控えを2つ置く(文章と表を1つずつ)
        let sentence = recover_path_for(Some(a), "adoc", "無題");
        std::fs::create_dir_all(sentence.parent().unwrap()).unwrap();
        std::fs::write(&sentence, "= 報告書\n").unwrap();
        note_recover_origin(&sentence, Some(a));
        let table = recover_path_for(Some(a), "xlsx", "無題");
        std::fs::write(&table, b"PK").unwrap();

        // **拡張子で分かれる**(表の画面に文書の控えを出さない)
        let sentences = stale_recovers("adoc");
        assert_eq!(sentences.len(), 1, "{sentences:?}");
        assert_eq!(sentences[0].0, a.to_string_lossy(), "元の道を添えて見せる");
        assert_eq!(stale_recovers("xlsx").len(), 1);

        // 保存できたら消す。**添え書きも消える**(消し忘れると次の起動で
        // 「落ちた後です」と嘘を言う)
        drop_recover(Some(a), "adoc", "無題");
        assert!(!sentence.exists(), "控えが残っている");
        assert!(!sentence.with_extension("path").exists(), "添え書きが残っている");
        assert_eq!(stale_recovers("adoc").len(), 0);
        assert_eq!(stale_recovers("xlsx").len(), 1, "表の控えは消さない");

        match from {
            Some(v) => unsafe { std::env::set_var("HOME", v) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        let _ = std::fs::remove_dir_all(&home);
    }
}
