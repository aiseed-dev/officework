//! **型そのもの。** セル・シート・ブックと、それに直に付く実装。

use std::collections::BTreeMap;

use super::refs::*;

/// A1 形式のセル位置。0起点の (行, 列) で持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub row: u32,
    pub col: u32,
}

impl Pos {
    pub fn new(row: u32, col: u32) -> Pos {
        Pos { row, col }
    }

    /// "B3" → Pos{row:2, col:1}。列は A..Z, AA.. の26進(1起点)。
    pub fn parse(s: &str) -> Option<Pos> {
        // 絶対参照の $ は位置に影響しないので先に全部落とす($C$5 も C5 と同じ)
        let s: String = s.trim().chars().filter(|c| *c != '$').collect();
        let split = s.find(|c: char| c.is_ascii_digit())?;
        let (col_s, row_s) = s.split_at(split);
        if col_s.is_empty() || !col_s.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let mut col = 0u32;
        for c in col_s.chars() {
            col = col * 26 + (c.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        }
        let row: u32 = row_s.parse().ok()?;
        if row == 0 {
            return None;
        }
        Some(Pos { row: row - 1, col: col - 1 })
    }

    pub fn a1(&self) -> String {
        let mut n = self.col + 1;
        let mut s = String::new();
        while n > 0 {
            let r = ((n - 1) % 26) as u8;
            s.insert(0, (b'A' + r) as char);
            n = (n - 1) / 26;
        }
        format!("{s}{}", self.row + 1)
    }
}

/// セルの値(計算の結果)。
#[derive(Debug, Clone, PartialEq)]
#[derive(Default)]
pub enum Value {
    #[default]
    Empty,
    Number(f64),
    Text(String),
    Bool(bool),
    /// #DIV/0! のようなエラー。文字列で持つ(表計算の作法)
    Error(String),
}

impl Value {
    pub fn as_number(&self) -> f64 {
        match self {
            Value::Number(n) => *n,
            Value::Bool(b) => *b as i32 as f64,
            // 表計算の慣習: 文字列は数値として0。ただし数字だけの文字列は読む
            Value::Text(s) => s.trim().parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }
    pub fn display(&self) -> String {
        match self {
            Value::Empty => String::new(),
            Value::Number(n) => {
                if (n.fract()).abs() < 1e-10 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            Value::Text(s) => s.clone(),
            Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
            Value::Error(e) => e.clone(),
        }
    }
    pub fn is_empty(&self) -> bool {
        matches!(self, Value::Empty)
    }
}

/// ヘッダー/フッターの文字列(&L 左 &C 中 &R 右)を3つに割る。
/// 区分の印より前の文字は中(xlsx の慣わし)
pub fn hf_split(s: &str) -> (String, String, String) {
    let (mut l, mut c, mut r) = (String::new(), String::new(), String::new());
    let mut cur = 1u8;
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        if ch == '&' {
            match it.peek() {
                Some('L') => { it.next(); cur = 0; continue }
                Some('C') => { it.next(); cur = 1; continue }
                Some('R') => { it.next(); cur = 2; continue }
                _ => {}
            }
        }
        match cur { 0 => l.push(ch), 1 => c.push(ch), _ => r.push(ch) }
    }
    (l, c, r)
}

/// 3つの区分をヘッダー/フッターの文字列に組む。全部空なら空文字
pub fn hf_join(l: &str, c: &str, r: &str) -> String {
    let mut out = String::new();
    if !l.is_empty() { out.push_str("&L"); out.push_str(l); }
    if !c.is_empty() { out.push_str("&C"); out.push_str(c); }
    if !r.is_empty() { out.push_str("&R"); out.push_str(r); }
    out
}

/// 罫線の線種(xlsx の style 属性と対)。並びは細→太のおおよそ。
/// 知らない線種は Thin に落とすが、**書きでは読んだ線種を返す**(往復で保つ)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum BStyle {
    Hair,
    Dotted,
    DashDotDot,
    DashDot,
    Dashed,
    #[default]
    Thin,
    MediumDashDotDot,
    MediumDashDot,
    MediumDashed,
    Medium,
    Thick,
    Double,
    SlantDashDot,
}

impl BStyle {
    pub fn xlsx(self) -> &'static str {
        match self {
            BStyle::Hair => "hair",
            BStyle::Dotted => "dotted",
            BStyle::DashDotDot => "dashDotDot",
            BStyle::DashDot => "dashDot",
            BStyle::Dashed => "dashed",
            BStyle::Thin => "thin",
            BStyle::MediumDashDotDot => "mediumDashDotDot",
            BStyle::MediumDashDot => "mediumDashDot",
            BStyle::MediumDashed => "mediumDashed",
            BStyle::Medium => "medium",
            BStyle::Thick => "thick",
            BStyle::Double => "double",
            BStyle::SlantDashDot => "slantDashDot",
        }
    }
    pub fn from_xlsx(s: &str) -> BStyle {
        match s {
            "hair" => BStyle::Hair,
            "dotted" => BStyle::Dotted,
            "dashDotDot" => BStyle::DashDotDot,
            "dashDot" => BStyle::DashDot,
            "dashed" => BStyle::Dashed,
            "mediumDashDotDot" => BStyle::MediumDashDotDot,
            "mediumDashDot" => BStyle::MediumDashDot,
            "mediumDashed" => BStyle::MediumDashed,
            "medium" => BStyle::Medium,
            "thick" => BStyle::Thick,
            "double" => BStyle::Double,
            "slantDashDot" => BStyle::SlantDashDot,
            _ => BStyle::Thin, // 知らない線種は細実線で描く(消しはしない)
        }
    }
    /// 画面と PDF の線の太さ(px)。二重線は2本描きの合計ではなく1本ぶん
    pub fn px(self) -> f32 {
        match self {
            BStyle::Hair => 0.5,
            BStyle::Medium | BStyle::MediumDashed | BStyle::MediumDashDot
            | BStyle::MediumDashDotDot | BStyle::SlantDashDot => 2.0,
            BStyle::Thick => 3.0,
            _ => 1.0,
        }
    }
    /// 破線系か(画面では dashed 近似で描く。点線の刻みまでは分けない)
    pub fn dashed(self) -> bool {
        matches!(
            self,
            BStyle::Dotted | BStyle::Dashed | BStyle::DashDot | BStyle::DashDotDot
                | BStyle::MediumDashed | BStyle::MediumDashDot | BStyle::MediumDashDotDot
                | BStyle::SlantDashDot
        )
    }
}

/// 罫線1辺 — 有無・線種・色(RRGGBB。None=自動の黒)。
/// 2026-08-07 拡張: 前は有無(bool)だけだった
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub on: bool,
    pub style: BStyle,
    pub color: Option<u32>,
}

impl Edge {
    pub const THIN: Edge = Edge { on: true, style: BStyle::Thin, color: None };
    pub const OFF: Edge = Edge { on: false, style: BStyle::Thin, color: None };
    pub fn line(style: BStyle, color: Option<u32>) -> Edge {
        Edge { on: true, style, color }
    }
}

/// 罫線の引き方。**日本の帳票は罫線で出来ている**ので、ここは飾りではない。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Borders {
    pub top: Edge,
    pub bottom: Edge,
    pub left: Edge,
    pub right: Edge,
}

impl Borders {
    pub const ALL: Borders =
        Borders { top: Edge::THIN, bottom: Edge::THIN, left: Edge::THIN, right: Edge::THIN };
    pub const NONE: Borders =
        Borders { top: Edge::OFF, bottom: Edge::OFF, left: Edge::OFF, right: Edge::OFF };

    pub fn any(self) -> bool {
        self.top.on || self.bottom.on || self.left.on || self.right.on
    }
}

/// セルの横の揃え。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum HAlign {
    /// 指定なし — **数は右、文字は左**という表計算の既定に従う
    #[default]
    General,
    Left,
    Center,
    Right,
    /// 両端揃え(折り返した行を左右いっぱいに伸ばす)
    Justify,
    /// **選択範囲内で中央**(xlsx の `centerContinuous`)。結合はせずに、
    /// 右へ続く空セルをまたいで文字を中央に置く — 表の題を欄をまたいで
    /// 中央に据える書き方で、日銀の統計表がこれを使う。
    ///
    /// **持つ値は正しいが、描くのは今のところ普通の中央揃え。** どこまで
    /// 跨るか(右へ続く、空で同じ揃えのセルの並び)を描画側が数えねばならず、
    /// そこは未着手 — 自分のセルの中で中央に寄る。値は畳まないので、
    /// 開いて保存しても `centerContinuous` のまま返る
    CenterContinuous,
    /// 均等割付(xlsx の `distributed`)。文字をセルの幅いっぱいに
    /// 散らして並べる。**日本語の帳票では姓名や項目名の幅を揃えるのに使う**
    /// (nihongo.ja.md の K4)。文書側の `engine::Align::Distribute` と同じ考え
    Distribute,
}

impl HAlign {
    pub fn as_xlsx(self) -> Option<&'static str> {
        match self {
            HAlign::General => None,
            HAlign::Left => Some("left"),
            HAlign::Center => Some("center"),
            HAlign::Right => Some("right"),
            HAlign::Justify => Some("justify"),
            HAlign::CenterContinuous => Some("centerContinuous"),
            HAlign::Distribute => Some("distributed"),
        }
    }
    pub fn from_xlsx(v: &str) -> HAlign {
        match v {
            "left" => HAlign::Left,
            "center" => HAlign::Center,
            "right" => HAlign::Right,
            "justify" => HAlign::Justify,
            "centerContinuous" => HAlign::CenterContinuous,
            "distributed" => HAlign::Distribute,
            _ => HAlign::General,
        }
    }
}

/// セルの縦の揃え。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum VAlign {
    Top,
    Middle,
    /// xlsx の既定は下揃え
    #[default]
    Bottom,
    /// 上下いっぱいに散らす(xlsx の `distributed`)。折り返した行を
    /// **セルの高さいっぱいに均等に配る** — 横の均等割付の縦版で、
    /// 日銀の統計表が使っている(実物の corpus に 5 箇所)。
    ///
    /// **持つ値は正しいが、描くのは今のところ上揃え。** 行の間隔を高さから
    /// 割り出す所が未着手 — `HAlign::CenterContinuous` と同じ扱いで、
    /// **`Bottom` へ畳み戻さない**(畳むと保存で消える)。
    ///
    /// `justify` も xlsx にはあるが、実物 31 枚に 1 度も出ない。
    /// **出てから足す** — 使われていない物を先に作らない
    Distribute,
}

impl VAlign {
    pub fn as_xlsx(self) -> Option<&'static str> {
        match self {
            VAlign::Top => Some("top"),
            VAlign::Middle => Some("center"),
            VAlign::Bottom => None,
            VAlign::Distribute => Some("distributed"),
        }
    }
    pub fn from_xlsx(v: &str) -> VAlign {
        match v {
            "top" => VAlign::Top,
            "center" => VAlign::Middle,
            "distributed" => VAlign::Distribute,
            // **`justify` はまだ Bottom に落ちる。** 実物に出ないので変種を
            // 作っていない(2026-08-10)。出たら足す
            _ => VAlign::Bottom,
        }
    }
}

/// グラデーションの塗り(xlsx の `gradientFill`)。
///
/// **数はすべて整数で持つ。** xlsx の degree と position は小数だが、
/// `CellFormat` は `Ord` を導出していて `f64` には付かない — 文字の
/// 大きさ(`size_c`)と同じ手で、角度は度×100、位置は千分率に落とす。
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Gradient {
    /// 角度(度×100)。0=左から右。`path` のときは意味を持たない
    pub degree_c: i32,
    /// 途中の色(位置 0..=1000 の千分率, 色 `RRGGBB`)。**位置の昇順**。
    /// 2つ未満のグラデーションは xlsx にもならないので持たない
    pub stops: Vec<(u32, String)>,
    /// 放射(`gradientFill@type="path"`)なら Some にその型名。
    /// 線形は None。**知らない型もそのまま抱える** — 落として線形に
    /// 均すと、円形の塗りが黙って横縞になる
    pub path: Option<String>,
}

/// セルの書式。xlsx の `styles.xml`(xf / font / fill / border)に対応する。
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellFormat {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub borders: Borders,
    pub align: HAlign,
    /// 塗りつぶしの色 `RRGGBB`
    pub fill: Option<String>,
    /// 文字色 `RRGGBB`
    pub color: Option<String>,
    /// 文字色がテーマ由来なら(番号, 明るさの加減×1000)。
    /// **配色を変えると色が追従する**ため、由来を覚えておく
    pub color_theme: Option<(u8, i32)>,
    /// 塗りがテーマ由来なら(番号, 明るさの加減×1000)
    pub fill_theme: Option<(u8, i32)>,
    /// 塗りの**柄**(xlsx の `patternFill@patternType`)。`lightGrid` など。
    ///
    /// **べた塗りと塗り無しはここに入れない** — べた塗りは `fill` が色を
    /// 持つこと、塗り無しは `fill` が `None` であることで表す。柄のときの
    /// `fill` は**前景色**(fgColor)の意味になる
    pub fill_pattern: Option<String>,
    /// 柄の**地の色** `RRGGBB`(`patternFill` の bgColor)。
    /// 柄があるときだけ意味を持つ
    pub fill_bg: Option<String>,
    /// グラデーション。**柄とは排他** — xlsx も塗りの要素は一つで、
    /// `patternFill` と `gradientFill` は並べられない
    pub fill_grad: Option<Gradient>,
    /// 書体の名前(xlsx の `<font><name>`)。文書の設定
    pub font: Option<String>,
    /// 文字の大きさ(pt×100 で持つ。f32 だと Ord が付かない)
    pub size_c: Option<u32>,
    pub strike: bool,
    /// 下付き(xlsx の vertAlign subscript)。上付きは未実装
    pub subscript: bool,
    /// 文字の回転(xlsx の alignment textRotation。度。90=縦向き)
    pub rotation: Option<i32>,
    /// 右横書き(xlsx の alignment readingOrder="2")。
    /// **日本語の右横書き** — 昔の看板や横額の書き方。1字ずつ右から並べる
    pub rtl_text: bool,
    pub valign: VAlign,
    /// 折り返して全体を表示
    pub wrap: bool,
    /// 縮小して全体を表示(xlsx の alignment shrinkToFit)
    pub shrink: bool,
    /// 字下げの段数(xlsx の alignment indent。1段 = 全角約1字)。
    /// **日本の帳票は項目の階層を字下げで見せる** — 持たないと、
    /// 書式を1つ触っただけで階層が潰れる(2026-08-13 に踏んで足した)
    pub indent: u8,
    /// 表示形式(`#,##0` `0.00%` など)。xlsx の numFmt
    pub number_format: Option<String>,
    /// **保護中でも書き換えられる**セル(xlsx の `<protection locked="0"/>`)。
    ///
    /// xlsx は「ロックされている」を既定にして `locked="1"` を省く。こちらも
    /// **裏返して「ロックを外した」で持つ** — そうすれば `Default` が
    /// 素直に derive でき、`is_plain()`(=書式なし)の判定も狂わない。
    /// 手書きの `Default` は取りこぼす(図形の line_w で踏んだ)。
    ///
    /// シートを保護していないときは意味を持たない。保護すると、ここが
    /// false のセルだけが堰き止められる — 帳票の「記入欄だけ開ける」作法
    pub unlocked: bool,
    /// **式を隠す**(xlsx の `<protection hidden="1"/>`)。
    ///
    /// シートを保護している間、このセルの**式が数式バーに出ない**
    /// (値は見える)。単価表の掛け率を伏せる、といった使い方。
    /// ロックと同じく**シートを保護して初めて効く** — 保護していない
    /// ブックでこれを立てても何も起きない
    pub formula_hidden: bool,
}

impl CellFormat {
    pub fn is_plain(&self) -> bool {
        *self == CellFormat::default()
    }
}

/// 保護中でも許す操作。**`Default` は Excel が保護をかけたときの初期値**
/// (選択だけ許し、他は禁じる)。`derive` だと全部 false になり
/// 「選択すらできない」になってしまうので手で書く。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectAllow {
    /// ロックされたセルを選べる
    pub select_locked: bool,
    /// ロックを外したセルを選べる
    pub select_unlocked: bool,
    /// セルの書式を変えられる
    pub format_cells: bool,
    /// 列の幅・非表示を変えられる
    pub format_cols: bool,
    /// 行の高さ・非表示を変えられる
    pub format_rows: bool,
    pub insert_cols: bool,
    pub insert_rows: bool,
    /// ハイパーリンクを入れられる
    pub insert_links: bool,
    pub delete_cols: bool,
    pub delete_rows: bool,
    pub sort: bool,
    /// オートフィルターを操作できる
    pub autofilter: bool,
    /// ピボットテーブルを操作できる
    pub pivot: bool,
    /// 図形・画像を選んで動かせる(xlsx の `objects`)。
    /// **既定は禁止** — 保護は「うっかり動かす」を止めるためのもので、
    /// 図形はセルより簡単にずれる(2026-08-13、台帳「図形ロック」)
    pub objects: bool,
}

impl Default for ProtectAllow {
    fn default() -> Self {
        Self {
            select_locked: true,
            select_unlocked: true,
            format_cells: false,
            format_cols: false,
            format_rows: false,
            insert_cols: false,
            insert_rows: false,
            insert_links: false,
            delete_cols: false,
            delete_rows: false,
            sort: false,
            autofilter: false,
            pivot: false,
            objects: false,
        }
    }
}

impl ProtectAllow {
    /// 画面に出す並び(名前, 読む, 書く)。チェックの一覧と往復で使う
    pub fn items(&self) -> [(&'static str, bool); 14] {
        [
            ("図形・画像の操作", self.objects),
            ("ロックされたセルの選択", self.select_locked),
            ("ロックされていないセルの選択", self.select_unlocked),
            ("セルの書式設定", self.format_cells),
            ("列の書式設定", self.format_cols),
            ("行の書式設定", self.format_rows),
            ("列の挿入", self.insert_cols),
            ("行の挿入", self.insert_rows),
            ("ハイパーリンクの挿入", self.insert_links),
            ("列の削除", self.delete_cols),
            ("行の削除", self.delete_rows),
            ("並べ替え", self.sort),
            ("オートフィルターの使用", self.autofilter),
            ("ピボットテーブルの使用", self.pivot),
        ]
    }

    /// 名前で入切する(一覧を押したときの受け)
    pub fn toggle(&mut self, name: &str) {
        let f = match name {
            "ロックされたセルの選択" => &mut self.select_locked,
            "ロックされていないセルの選択" => &mut self.select_unlocked,
            "セルの書式設定" => &mut self.format_cells,
            "列の書式設定" => &mut self.format_cols,
            "行の書式設定" => &mut self.format_rows,
            "列の挿入" => &mut self.insert_cols,
            "行の挿入" => &mut self.insert_rows,
            "ハイパーリンクの挿入" => &mut self.insert_links,
            "列の削除" => &mut self.delete_cols,
            "行の削除" => &mut self.delete_rows,
            "並べ替え" => &mut self.sort,
            "オートフィルターの使用" => &mut self.autofilter,
            "ピボットテーブルの使用" => &mut self.pivot,
            _ => return,
        };
        *f = !*f;
    }
}

/// 1つのセル。入力(式か値)と、計算後の値と、見た目。
#[derive(Debug, Clone, Default)]
pub struct Cell {
    /// 式("=" で始まる)。無ければ None
    pub formula: Option<String>,
    /// 計算後の値(式が無ければ入力そのもの)
    pub value: Value,
    /// 見た目。**罫線はここ**
    pub fmt: CellFormat,
}


impl Cell {
    /// 利用者が入力した文字列を、式か値として解釈する。
    /// **式が日付や時刻を返すなら、その表示形式を薦める。**
    ///
    /// `=DATE(2026,8,10)` の答えは通し番号 `46244` で、表示形式が無いと
    /// **画面にも 46244 と出る**。Excel は General のセルに日付を返す式を
    /// 入れると、セルの表示形式を日付に変える。同じ手当てが要る
    /// (2026-08-10 に ironcalc と突き合わせて判明)。
    ///
    /// **薦めるだけで、押し付けない。** 既に表示形式のあるセルは触らない —
    /// 呼ぶ側が「元の書式が無いときだけ使う」。pysheet の `__setitem__` が
    /// Python の `date` に対して既にその作法(`date_fmt`)。
    ///
    /// **外側の関数だけを見る。** `=TODAY()+1` は日付だが `=YEAR(TODAY())`
    /// は数(年)。中まで辿ると当たらなくなるので、**先頭の名前だけ**で決める。
    /// `=A1+1`(A1 が日付)のような書式の伝播はしない — それは別の話。
    pub fn date_format_of(formula: &str) -> Option<&'static str> {
        let f = formula.trim_start_matches('=').trim_start();
        let name: String = f
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '.')
            .collect::<String>()
            .to_ascii_uppercase();
        // 括弧が続かないなら関数ではない(`=DATE` のような名前だけの参照)
        if !f[name.len()..].trim_start().starts_with('(') {
            return None;
        }
        match name.as_str() {
            // 日付を返す
            "DATE" | "TODAY" | "DATEVALUE" | "EDATE" | "EOMONTH" | "WORKDAY" => {
                Some("yyyy/m/d")
            }
            // 日付と時刻の両方
            "NOW" => Some("yyyy/m/d h:mm"),
            // 時刻だけ
            "TIME" | "TIMEVALUE" => Some("h:mm"),
            // **YEAR / MONTH / DAY / WEEKDAY / DATEDIF / NETWORKDAYS は数を返す。**
            // 日付の関数だからと形式を付けると、年 2026 が 1905年7月18日 になる
            _ => None,
        }
    }

    /// 打ち込まれた字をセルにする。
    ///
    /// **`=` の後ろに空白があれば式にしません**(2026-08-19 発注者確定)。
    /// `= 大` はセルの見出し(`cellmark`)、`=SUM(A1)` は式です。
    /// 見分ける決めは `kumihan::adoc::is_formula_cell` の1つだけを見ます —
    /// 打ち込みと `.adoc` の読み書きで**同じ字が同じ意味**になります。
    ///
    /// *失う物。* Excel 風の `= 1+2`(`=` の後ろに空白を置いた式)は
    /// 式ではなく字になります。
    pub fn input(s: &str) -> Cell {
        let t = s.trim();
        if kumihan::adoc::is_formula_cell(t) {
            let f = t.strip_prefix('=').unwrap_or(t);
            return Cell { formula: Some(f.to_string()), value: Value::Empty, fmt: Default::default() };
        }
        if t.is_empty() {
            return Cell::default();
        }
        if let Ok(n) = t.parse::<f64>() {
            return Cell { formula: None, value: Value::Number(n), fmt: Default::default() };
        }
        match t.to_ascii_uppercase().as_str() {
            "TRUE" => Cell { formula: None, value: Value::Bool(true), fmt: Default::default() },
            "FALSE" => Cell { formula: None, value: Value::Bool(false), fmt: Default::default() },
            _ => Cell { formula: None, value: Value::Text(t.to_string()), fmt: Default::default() },
        }
    }

    /// 編集欄に出す文字列(式ならその式、値ならその表示)。
    pub fn editable(&self) -> String {
        match &self.formula {
            Some(f) => format!("={f}"),
            None => self.value.display(),
        }
    }
}

/// 固定枠(ウィンドウ枠の固定)。**止める行数・列数**で持つ —
/// Excel の「先頭の行を固定」と同じ数え方で、`frozen_rows: 1` なら
/// 見出しの1行だけが止まる。xlsx の `sheetView > pane` と往復する。
///
/// genoffice のサイドカーが返す `FreezePane` と同じ形にしてある
/// (突き合わせのときに欄を並べ替えずに済む)。
///
/// xlsx の `pane` は掴んで動かす**分割**(`state="split"`)も同じ形で書くが、
/// あれは固定ではないので持たない(読みで捨てる)。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FreezePane {
    /// 上で止める行数(0 = 行は止めない)
    pub frozen_rows: u32,
    /// 左で止める列数(0 = 列は止めない)
    pub frozen_columns: u32,
}

/// 1枚のシート。疎な表なので BTreeMap で持つ(空セルは持たない)。
#[derive(Debug, Clone, Default)]
pub struct Sheet {
    pub name: String,
    pub cells: BTreeMap<Pos, Cell>,
    /// セル結合(左上, 右下)。**日本の帳票は結合で見出しを作る**ので、
    /// 読み飛ばして保存すると枠組みが壊れる
    pub merges: Vec<(Pos, Pos)>,
    /// 列幅(xlsx の単位 = 標準フォントの「0」何個ぶん)。無い列は既定幅。
    /// これも読み飛ばして保存すると帳票の形が変わる
    pub col_width: BTreeMap<u32, f32>,
    /// 全列の既定幅。`<col min="1" max="16384">` を1列ずつ展開しない
    /// (展開すると保存が 16,384 個の col で肥大する)
    pub default_col_width: Option<f32>,
    /// 全行の既定の高さ(pt)。`<sheetFormatPr defaultRowHeight="15"/>`。
    /// **書いてはいたが読んでいなかった**(2026-08-10、向こうの試験で判明)。
    /// 無い行はこの高さで描くので、落とすと行間が変わる
    pub default_row_height: Option<f32>,
    /// 畳んである行・列(xlsx の `collapsed`)。アウトラインの「−」を押した状態。
    /// **`hidden` とは別** — 畳んだ親の行そのものは見えている
    pub row_collapsed: std::collections::BTreeSet<u32>,
    pub col_collapsed: std::collections::BTreeSet<u32>,
    /// 行の高さ(pt)。無い行は既定。列幅と同じ構図
    pub row_height: BTreeMap<u32, f32>,
    /// 行のグループ化(アウトライン)の深さ 1〜7(xlsx の outlineLevel)。
    /// 載っていない行は 0。列も同じ構図
    pub row_outline: BTreeMap<u32, u8>,
    pub col_outline: BTreeMap<u32, u8>,
    /// 畳んで見えなくした行・列(xlsx の hidden)。**絞り込みと違って
    /// 保存に残る** — 畳んだ台帳は畳んだまま次の人に渡る
    pub row_hidden: std::collections::BTreeSet<u32>,
    pub col_hidden: std::collections::BTreeSet<u32>,
    /// この表にある表オブジェクト(xlsx の table)
    pub tables: Vec<TableDef>,
    /// 読み込んだ xlsx でのセルの書式索引(`<c s="…">`)。
    /// **保存で原本の styles.xml を据え置く**ための控え — 書式を触って
    /// いないセルは同じ索引で書き戻す(勝手な書式設定をしないの方針)。
    /// 行や列を動かして古くなっても、保存時に中身を照合するので誤用はない
    pub style_of: BTreeMap<Pos, u32>,
    /// 右から左へ並べる(xlsx の sheetView rightToLeft)。
    /// **日本語も右から書くことがある**(右横書き)— 発注者 2026-08-04
    pub rtl: bool,
    /// 固定枠(xlsx の sheetView > pane)。None = 固定していない。
    /// **台帳と様式では見出しの固定は既定に近い** — 読み書きしないと、
    /// Excel で固定した台帳を開いて保存するだけで黙って消える
    pub freeze: Option<FreezePane>,
    /// 画面の格子線を描く(sheetView showGridLines)。
    /// **None は「原文に書かれていなかった」** = Excel の既定(描く)。
    /// 読んだ値だけを返し、こちらから既定を書き足さない — 以下2つも同じ作法
    pub show_gridlines: Option<bool>,
    /// 値でなく式を並べて見せる(showFormulas)。None = 既定(値を見せる)
    pub show_formulas: Option<bool>,
    /// 表示倍率 %(zoomScale)。None = 既定(100)
    pub zoom_scale: Option<u32>,
    /// 隠しシート(xlsx の workbook.xml の sheet state="hidden")。
    /// 隠しても中身も式も生きている — 見えなくなるだけ
    pub hidden: bool,
    /// 耳(タブ)の色(xlsx の sheetPr > tabColor の rgb="FFRRGGBB")。
    /// 読んだ値をそのまま持ち、保存でそのまま返す。theme 指定の色は
    /// 拾えない(そのときは色なし)
    pub tab_color: Option<String>,
    /// シートの保護(xlsx の sheetProtection)。**パスワードは掛けない** —
    /// 掛けた振りもしない(writer の保護と同じ正直な作法)。
    /// 効き目はアプリが守る: 保護中は編集を堰き止める
    pub protected: bool,
    /// **保護中でも許す操作。** xlsx の sheetProtection は「禁じる」向きで
    /// 書くが(`formatCells="1"` = 書式を禁じる)、こちらは画面の
    /// チェックボックスと同じ**「許す」向き**で持ち、読み書きの所だけで
    /// 裏返す。向きを混ぜると必ずどこかで逆になる
    pub protect_allow: ProtectAllow,
    /// 名前の定義。式の中で名前が使える。
    /// workbook.xml の definedNames と往復する
    pub names: Vec<DefinedName>,
    /// セルのハイパーリンク(外部URL)。sheet.xml の hyperlinks と往復する
    pub links: BTreeMap<Pos, String>,
    /// セルのコメント。**話の筋(スレッド)**で持つ。
    ///
    /// 近代の Excel はコメントを `xl/threadedComments/` に置き、
    /// `xl/comments1.xml` はその**古い読み手向けの写し**になっている。
    /// 前は写しのほうだけを読み書きしていたので、スレッドを持つブックを
    /// 開いて直すと**写しだけが変わり、Excel が見る本体は元のまま**だった
    /// (2026-08-13 に実測。返信もこちらから見えていなかった)
    pub comments: BTreeMap<Pos, CommentThread>,
    /// 条件付き書式(cellIs だけ)。xlsx の conditionalFormatting と往復する
    pub cond: Vec<CondRule>,
    /// データの入力規則(list だけ)。xlsx の dataValidations と往復する
    pub validations: Vec<Validation>,
    /// 印刷の向き(xlsx の pageSetup orientation="landscape")。
    /// **読むだけ** — 保存は原文持ち越しが正。PDF がこれに従う
    pub landscape: bool,
    /// 用紙(xlsx の pageSetup paperSize。9=A4, 8=A3, 11=A5, 12=B4, 13=B5)
    pub paper_size: Option<u32>,
    /// 印刷の余白 mm(左, 右, 上, 下)。xlsx の pageMargins(インチ)から換算
    pub margins_mm: Option<(f32, f32, f32, f32)>,
    /// 印刷範囲(definedName _xlnm.Print_Area)。編集の対象なのでモデルで持つ
    /// (xlsx との往復は読み書きが解く)。複数の域も持てる
    pub print_areas: Vec<(Pos, Pos)>,
    /// 拡大縮小印刷(pageSetup scale、%)。無ければ 100
    pub print_scale: Option<u32>,
    /// **紙 N 枚に収める**(pageSetup の fitToWidth / fitToHeight)。
    /// 0 は「その向きは合わせない」。どちらかが立っていれば `print_scale` は
    /// 使わず、**中身が収まるまで縮める**(Excel と同じく縮めるだけ、
    /// 拡大はしない)。両方 None なら今までどおり print_scale
    pub fit_to_w: Option<u32>,
    pub fit_to_h: Option<u32>,
    /// 改ページ(このモデルでは「新しい紙をここから始める行」0起点。
    /// xlsx の rowBreaks/brk@id と同じ数え方)
    pub row_breaks: Vec<u32>,
    /// **縦の改ページ**(この列から新しい紙。xlsx の colBreaks)。
    /// 読み書きしていなかったので、Excel で入れた区切りが消えていた
    pub col_breaks: Vec<u32>,
    /// 枠線・見出し(行番号と列名)も印刷する(printOptions)
    pub print_gridlines: bool,
    pub print_headings: bool,
    /// タイトル行(各ページの頭で繰り返す行の範囲。Print_Titles の行の部)
    pub print_title_rows: Option<(u32, u32)>,
    /// タイトル列(各ページの左で繰り返す列の範囲。Print_Titles の列の部
    /// `$A:$B`)。**横に長い台帳で品名の列を毎ページ出す**ための物
    /// (2026-08-13。行だけ持っていて、列は原文で持ち越すだけだった)
    pub print_title_cols: Option<(u32, u32)>,
    /// 印刷のヘッダー(xlsx の oddHeader の生の文字列。&L/&C/&R が区分、
    /// &P=頁 &N=総頁。紙(PDF)に出る — 画面の格子には出ない)
    pub header: Option<String>,
    /// 印刷のフッター(oddFooter)。作法は header と同じ
    pub footer: Option<String>,
    /// 偶数頁・先頭頁だけのヘッダー/フッター(xlsx の evenHeader /
    /// evenFooter / firstHeader / firstFooter)と、それを使うかの旗。
    /// **持たないと保存で消えていた**(2026-08-13 に踏んで足した) —
    /// 左右で綴じる帳票は偶数頁を別に組むので、消すと様式が崩れる
    pub header_even: Option<String>,
    pub footer_even: Option<String>,
    pub header_first: Option<String>,
    pub footer_first: Option<String>,
    /// 奇数頁と偶数頁で分ける(headerFooter differentOddEven)
    pub hf_diff_odd_even: bool,
    /// 先頭頁だけ分ける(headerFooter differentFirst)
    pub hf_diff_first: bool,
    /// 読んだ xlsx の図形(**表示だけ**。保存は原文の持ち越しが担う)
    pub shapes: Vec<SheetShape>,
    /// **このアプリで挿した**図形。保存でこちらが DrawingML として書き出す
    pub shapes_new: Vec<SheetShape>,
    /// 読んだ xlsx の画像(**表示だけ**。保存は原文の drawing 持ち越しが担う —
    /// 図形など理解しない部品を壊さないため、読んだ絵はこちらで書き直さない)
    pub images: Vec<SheetImage>,
    /// **このアプリで挿した**画像(グラフもこれ)。保存でこちらが部品
    /// (drawing・rels・media)ごと書き出す。読んだ画像と持ち場を分ける —
    /// 混ぜると保存で二重になる(writer と同じ構図)
    pub images_new: Vec<SheetImage>,
    /// UDF(plugins の関数)のセルの引数の指紋。再計算のたびに計算し直す。
    /// **ファイルには入らない** — アプリが「引数が変わったから計算し直す」を
    /// 見分けるためだけの控え(0 = UDF のセルが無い)
    pub py_stamp: u64,
    /// セルのふりがな(xlsx の rPh)。**日本語の xlsx の宝** — 欧米の実装が
    /// 落としがちなので、読んで持ち、保存で書き戻す。PHONETIC 関数が読む
    pub phonetics: BTreeMap<Pos, String>,
    /// 動的配列のスピル(起点 → 高さ, 幅)。=FILTER 等があふれた先の記録。
    /// 再計算はここを見て前回の影を消してから置き直す(残骸を残さない)。
    /// xlsx へは独自部品 xl/joSpill.xml で往復(joPivot と同じ作法) —
    /// これが無いと、開き直したとき自分のスピル跡を他人のデータと
    /// 見分けられず、偽の #SPILL! になる
    pub spills: std::collections::BTreeMap<Pos, (u32, u32)>,
    /// 原本の `<dimension ref="A1:CN46"/>` が言っていた大きさ(行数, 列数)。
    /// **書き手の申告であって、実際に中身がある範囲とは限らない。**
    /// 空でも罫線や高さを持つ行・列が末尾にあると、申告のほうが大きくなる。
    /// 逆に申告が `A1` だけの手抜きな書き手もいるので、**単独では信じない** —
    /// 使うときは [`Sheet::size`] で実際と大きいほうを採る(2026-08-10 発注者確定)
    pub dim: Option<(u32, u32)>,
    /// **原本に `<c>` が置かれていた一番大きい所**(行数, 列数)。
    ///
    /// `<c r="D1" s="0"/>` — 要素はあるが値も書式も持たないセル。中身が
    /// 無いので [`Sheet::cells`] には入れない(入れると「中身のある範囲」を
    /// 意味する [`Sheet::extent`] が 38 箇所で狂う)。**それでも、書き手が
    /// そこまで書いたという事実は残す** — `<dimension>` を書かない書き手の
    /// とき、これが「シートはここまである」と言える唯一の手掛かりになる。
    ///
    /// 読みで落とすと、呼ぶ側が正しく要求した範囲を「シートの外」と
    /// 断ることになる(2026-08-10、向こうの試験が教えてくれた)
    pub seen: Option<(u32, u32)>,
    /// **昔ながらの配列数式(CSE)。** 起点 → 覆う大きさ(行, 列)。
    ///
    /// Excel で範囲を選んで Ctrl+Shift+Enter で入れたもの。xlsx では
    /// `<f t="array" ref="A1:C3">` と書かれ、**範囲の大きさは式が決めるの
    /// ではなく人が決める**(スピルとはそこが違う)。
    ///
    /// 読めないと `=SUM(A1:A3*B1:B3)` のような式が普通の式として計算され、
    /// **黙って違う値になる**(#VALUE! か、掛け算の1組だけの合計)。
    /// 古い帳票にはよく入っているので、読めることが乗り換えの条件になる。
    pub cse: std::collections::BTreeMap<Pos, (u32, u32)>,
}

impl Sheet {
    /// この席が昔ながらの配列数式の中なら、その起点を返す。
    /// **配列の一部だけを書き換えさせない**ための見張りに使う
    pub fn cse_anchor(&self, p: Pos) -> Option<Pos> {
        self.cse.iter().find_map(|(o, (h, w))| {
            (p.row >= o.row && p.row < o.row + h && p.col >= o.col && p.col < o.col + w)
                .then_some(*o)
        })
    }
}

/// シートに浮かぶ図形。**中身はベクタ**(発注者案 2026-08-04: SVG で作る —
/// 拡大縮小で崩れない)。画面へは to_svg が SVG を作り、xlsx へは DrawingML の
/// 名前の定義(「売上範囲」= Sheet1!B2:D9 のような)。
///
/// **どのシートに持たせるかと、どこまで効くかは別の話。** この列は
/// 「指す先のシート」に持たせてあるが、名前が効く範囲(適用範囲)は
/// `scoped` が決める — ブック全体なら他のシートの式からも引ける。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DefinedName {
    pub name: String,
    /// 参照 "A1" か "A1:B2"
    pub range: String,
    /// **このシートだけで使える名前**(xlsx の `localSheetId`)。
    /// `false` はブック全体。同じ名前を各シートに置きたいときに立てる
    pub scoped: bool,
}

impl DefinedName {
    pub fn new(name: impl Into<String>, range: impl Into<String>) -> Self {
        Self { name: name.into(), range: range.into(), scoped: false }
    }
}

/// 形をつくる1点(0..1 に正規化)。
///
/// **制御点は「この点へ入る側」と「この点から出る側」の2つ。**
/// どちらも `None` なら、その区間は直線 — スパークラインやペンの線は
/// この姿のままで、曲線を足しても作りが分かれない
/// (点の列と制御点の列を別々に持つと、片方だけ足したり消したりできて
/// しまう。**同じことを2箇所で言わない**)。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PathPoint {
    pub at: (f32, f32),
    /// **ここから新しい輪郭が始まる**(SVG の `M`、xlsx の `moveTo`)。
    ///
    /// 穴のある形は、外側の輪郭のあとに内側の輪郭を続けて持つ。
    /// 輪郭を別の列に分けて持つ案は採らなかった — 書式そのものが
    /// 「moveTo で切れ目を入れた1本の列」なので、同じ形にしておく方が
    /// 読み書きで取り違えない(先頭の点は立てなくてよい)
    pub start: bool,
    /// 手前の点からこの点へ来る曲線の制御点
    pub c_in: Option<(f32, f32)>,
    /// この点から次の点へ出る曲線の制御点
    pub c_out: Option<(f32, f32)>,
}

impl PathPoint {
    /// 角の点(曲がらない)
    pub fn at(x: f32, y: f32) -> Self {
        Self { at: (x, y), start: false, c_in: None, c_out: None }
    }

    /// 新しい輪郭の始まりの点(穴の1点目など)
    pub fn start_at(x: f32, y: f32) -> Self {
        Self { at: (x, y), start: true, c_in: None, c_out: None }
    }
    pub fn x(&self) -> f32 {
        self.at.0
    }
    pub fn y(&self) -> f32 {
        self.at.1
    }
}

/// スパークラインの点に付ける印。
///
/// **元データには届かない。** この線は挿したときの数を焼き付けた折れ線で、
/// 元の範囲を覚えていない(台帳の割り切り)。だから「空セルの扱い」や
/// 「縦軸のそろえ」のような**引き直しが要る設定は持たない** — 持っても
/// 効かない。印は焼き付けた点だけで決められるので持てる。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SparkMarks {
    /// いちばん高い点
    pub high: bool,
    /// いちばん低い点
    pub low: bool,
    pub first: bool,
    pub last: bool,
    /// 負の点(勝ち負け・縦棒で底より下)
    pub negative: bool,
}

impl SparkMarks {
    /// 札の綴り(`hlfen` のうち立っている字だけ)。**順は固定** —
    /// 綴りが揺れると往復の突き合わせができない
    pub fn tag(&self) -> String {
        let mut s = String::new();
        for (on, c) in [
            (self.high, 'h'), (self.low, 'l'), (self.first, 'f'),
            (self.last, 'e'), (self.negative, 'n'),
        ] {
            if on {
                s.push(c);
            }
        }
        s
    }

    /// 札の綴りから戻す。知らない字は黙って捨てる(前に進める)
    pub fn parse(t: &str) -> Self {
        Self {
            high: t.contains('h'),
            low: t.contains('l'),
            first: t.contains('f'),
            last: t.contains('e'),
            negative: t.contains('n'),
        }
    }
}

/// セルに付いた**話の筋**(コメントのスレッド)。
///
/// 先頭が元のコメント、あとが返信。`xl/threadedComments/` と往復し、
/// `xl/comments1.xml` へは**古い読み手のために写しを書く**(本体はスレッド側)。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommentThread {
    /// 解決済み(`threadedComment@done`)。**筋ごと**に立つ — 返信1つだけを
    /// 解決済みにはできない(Excel も同じ)
    pub done: bool,
    /// 発言。**空にはしない** — 空の筋は「コメントが無い」と同じなので消す
    pub entries: Vec<CommentEntry>,
}

/// 筋の中の1発言。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommentEntry {
    /// 書いた人。古いブックでは `<authors>` の名前、スレッドでは person の
    /// 表示名。**分からなければ空** — 「不明」と作らない
    pub who: String,
    /// いつ(`dT`)。**綴りのまま持つ**(ISO8601)。暦の計算はしない —
    /// 往復で崩さないことが先
    pub when: String,
    pub text: String,
}

/// 文字1本から筋を作れるようにする。**打ったばかりのコメントは
/// 「名無しの1発言の筋」**で、これが一番多い作り方
impl From<&str> for CommentThread {
    fn from(t: &str) -> Self {
        Self::new(t, "")
    }
}
impl From<String> for CommentThread {
    fn from(t: String) -> Self {
        Self::new(t, "")
    }
}

impl CommentThread {
    /// 文字1本から筋を作る(打ったばかりのコメント)
    pub fn new(text: impl Into<String>, who: impl Into<String>) -> Self {
        Self {
            done: false,
            entries: vec![CommentEntry {
                who: who.into(),
                when: String::new(),
                text: text.into(),
            }],
        }
    }

    /// 筋の頭の文字(古い呼び出し側が欲しいのはたいていこれ)
    pub fn text(&self) -> &str {
        self.entries.first().map(|e| e.text.as_str()).unwrap_or("")
    }

    /// 画面や古い写しに出す一続きの文。**返信も解決も見えるように綴る** —
    /// 頭だけ見せると、返信が付いていることが分からない
    pub fn flatten(&self) -> String {
        let mut s = String::new();
        for (i, e) in self.entries.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            if !e.who.is_empty() {
                s.push_str(&e.who);
                s.push_str(": ");
            }
            s.push_str(&e.text);
        }
        s
    }
}

/// テキストボックスの中の文字の組み方(xlsx の `txBody`)。
///
/// **画面で見せられることだけを持つ。** タブ位置(`a:tabLst`)は、文字を
/// 重ね描きの div 1枚で出している以上、位置を守って描けない — 持たない。
/// 持てば「設定はあるのに効かない」欄が1つ増える。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextFmt {
    /// 横の揃え(`a:pPr@algn`)。`General` は指定なし=左
    pub align: HAlign,
    /// 縦の揃え(`a:bodyPr@anchor`)。**既定は上** — セルの既定(下)とは違う
    pub anchor: TextAnchor,
    /// 縦書き(`a:bodyPr@vert="eaVert"`)。日本語の縦組み
    pub vertical: bool,
    /// 箇条書き。`None`=無し / `Some(false)`=中黒 / `Some(true)`=番号
    pub bullet: Option<bool>,
    /// 取り消し線(`a:rPr@strike="sngStrike"`)
    pub strike: bool,
    /// 上付き・下付き(`a:rPr@baseline`)。**両方は立たない**
    pub sup: bool,
    pub sub: bool,
}

/// テキストボックスの縦の揃え。セルの [`VAlign`] とは**既定が違う**
/// (セルは下、図形の中は上)ので別の型にした
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextAnchor {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// 図形(prstGeom)として書く — Excel でも図形として開ける。
#[derive(Debug, Clone, PartialEq)]
pub struct SheetShape {
    /// 左上を留めるセル
    pub at: Pos,
    pub width_px: f32,
    pub height_px: f32,
    /// 図形の種類(xlsx の prstGeom の名前):
    /// rect / roundRect / ellipse / rightArrow / diamond / line。
    /// "spark"(スパークライン)・"ink"(ペン)・"marker"(蛍光ペン)は
    /// 折れ線(points を使う。xlsx へは custGeom で書く = Excel でも線に見える)
    pub kind: String,
    /// 塗り RRGGBB(無ければ塗らない)
    pub fill: Option<String>,
    /// 線 RRGGBB(無ければ引かない)
    pub line: Option<String>,
    /// 図形の中の文字(テキストボックス)。xlsx の txBody と往復する。
    /// 画面へは SVG でなく重ね描き(組版の質と日本語のため)
    pub text: Option<String>,
    /// その文字の組み方(揃え・縦書き・箇条書き・文字効果)。
    /// `text` が無いときは意味を持たない
    pub text_fmt: TextFmt,
    /// スパークラインの**点の印**(高点・低点・最初・最後・負)。
    /// `kind` が `spark` 系のときだけ意味を持つ
    pub spark_marks: SparkMarks,
    /// 形をつくる点(0..1 に正規化)。折れ線もの(`spark` / `ink` / `marker`)と
    /// **自由な形**(`path`)が使う。
    /// `spark-col`/`spark-wl`(縦棒・勝ち負け)では (棒の中心x, 棒の先端y)
    pub points: Vec<PathPoint>,
    /// 棒の底(0..1 の y)。"spark-col"/"spark-wl" が使う(他は 0 のまま)
    pub base: f32,
    /// アンカーのセルからの右・下へのずらし(px)。SmartArt のような
    /// 図形の集まりを、セルの粗さに縛られずに組むための細かい座標
    pub dx_px: f32,
    pub dy_px: f32,
    /// 回転(度・時計回り)。xlsx の xfrm rot(6万分の1度)と往復
    pub rot: f32,
    /// 左右・上下の反転(xlsx の flipH / flipV)
    pub flip_h: bool,
    pub flip_v: bool,
    /// 線の太さ(pt)。既定 1.5pt = 従来の 2px。xlsx の a:ln の w と往復
    pub line_w: f32,
    /// 不透明度(0〜1、1=不透明)。塗りと線の色に掛かる。
    /// xlsx へは srgbClr の子 a:alpha として書く
    pub alpha: f32,
    /// 影(右下への落ち影)。xlsx の a:outerShdw と往復。
    /// 紙(PDF)は輪郭だけの方針なので影は画面と xlsx だけ
    pub shadow: bool,
}

impl Default for SheetShape {
    fn default() -> Self {
        SheetShape {
            at: Pos::default(),
            width_px: 0.0,
            height_px: 0.0,
            kind: String::new(),
            fill: None,
            line: None,
            text: None,
            text_fmt: TextFmt::default(),
            spark_marks: SparkMarks::default(),
            points: Vec::new(),
            base: 0.0,
            dx_px: 0.0,
            dy_px: 0.0,
            rot: 0.0,
            flip_h: false,
            flip_v: false,
            line_w: 1.5,
            alpha: 1.0,
            shadow: false,
        }
    }
}

impl Default for Pos {
    fn default() -> Self {
        Pos::new(0, 0)
    }
}

impl SheetShape {
    /// 折れ線もの(スパークライン・ペン)か。回転・反転・影は掛けない
    fn is_poly(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "spark" | "spark-col" | "spark-wl" | "ink" | "marker"
        )
    }

    /// 影と回転のはみ出しぶんの余白(px)。SVG のキャンバスは論理の
    /// 大きさより四方にこれだけ広い — 貼る側は左上から差し引く
    pub fn pad(&self) -> f32 {
        if self.is_poly() {
            return 0.0;
        }
        let (w, h) = (self.width_px.max(4.0), self.height_px.max(4.0));
        let mut p = 0.0f32;
        if self.rot.rem_euclid(360.0) != 0.0 {
            let t = self.rot.to_radians();
            let bw = w * t.cos().abs() + h * t.sin().abs();
            let bh = w * t.sin().abs() + h * t.cos().abs();
            p = ((bw - w).max(bh - h) / 2.0).max(0.0);
        }
        if self.shadow {
            p += 6.0;
        }
        p.ceil()
    }

    /// 画面用の SVG。**大きさを width/height に織り込む**ので、
    /// 描画側がその都度ラスタ化すれば、どの大きさでも輪郭が鮮明に出る。
    pub fn to_svg(&self) -> String {
        let (w, h) = (self.width_px.max(4.0), self.height_px.max(4.0));
        let fill = self
            .fill
            .as_deref()
            .map(|c| format!("#{c}"))
            .unwrap_or_else(|| "none".into());
        let line = self
            .line
            .as_deref()
            .map(|c| format!("#{c}"))
            .unwrap_or_else(|| "none".into());
        // 線の太さ pt → px(96/72)
        let sw = (self.line_w * 4.0 / 3.0).max(0.5);
        let p = self.pad();
        let (cw, ch) = (w + p * 2.0, h + p * 2.0);
        // 反転→回転の順(DrawingML の xfrm と同じ見た目)。中心まわり
        let mut tf = format!("translate({:.2} {:.2})", p + w / 2.0, p + h / 2.0);
        if self.rot.rem_euclid(360.0) != 0.0 && !self.is_poly() {
            tf.push_str(&format!(" rotate({:.2})", self.rot));
        }
        if (self.flip_h || self.flip_v) && !self.is_poly() {
            tf.push_str(&format!(
                " scale({} {})",
                if self.flip_h { -1 } else { 1 },
                if self.flip_v { -1 } else { 1 }
            ));
        }
        tf.push_str(&format!(" translate({:.2} {:.2})", -w / 2.0, -h / 2.0));
        // 影: 同じ形を灰色で右下にずらして先に(=下に)描く。
        // ずらしは回転の外(紙に落ちる向きが傾かない)
        let shadow = if self.shadow && !self.is_poly() {
            let sf = if self.fill.is_some() { "#9E9E9E" } else { "none" };
            let sl = if self.line.is_some() { "#9E9E9E" } else { "none" };
            format!(
                r#"<g transform="translate(4 4)"><g transform="{tf}">{}</g></g>"#,
                self.body_svg(sf, sl, sw, 0.35)
            )
        } else {
            String::new()
        };
        let body = format!(
            r#"<g transform="{tf}">{}</g>"#,
            self.body_svg(&fill, &line, sw, self.alpha.clamp(0.0, 1.0))
        );
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}" viewBox="0 0 {cw} {ch}">{shadow}{body}</svg>"#
        )
    }

    /// 形の本体(色と線幅と不透明度を差し替えられる — 影が同じ形を灰色で使う)
    fn body_svg(&self, fill: &str, line: &str, sw: f32, op: f32) -> String {
        let (w, h) = (self.width_px.max(4.0), self.height_px.max(4.0));
        let op_attr = if op < 0.999 {
            format!(r#" opacity="{op:.3}""#)
        } else {
            String::new()
        };
        let style = format!(r#"fill="{fill}" stroke="{line}" stroke-width="{sw:.2}"{op_attr}"#);
        // 線の太さの半分だけ内側に(縁が切れないように)
        let inset = (sw / 2.0).max(1.0);
        let (x0, y0, x1, y1) = (inset, inset, w - inset, h - inset);
        match self.kind.as_str() {
            "roundRect" => format!(
                r#"<rect x="{x0}" y="{y0}" width="{}" height="{}" rx="{r}" ry="{r}" {style}/>"#,
                x1 - x0,
                y1 - y0,
                r = ((x1 - x0).min(y1 - y0) * 0.15).max(4.0)
            ),
            "ellipse" => format!(
                r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" {style}/>"#,
                w / 2.0,
                h / 2.0,
                (x1 - x0) / 2.0,
                (y1 - y0) / 2.0
            ),
            "rightArrow" => {
                // 胴と鏃。高さの半分が鏃(prstGeom の既定に寄せる)
                let neck = h * 0.25;
                let head = (w * 0.35).min(h);
                format!(
                    r#"<polygon points="{x0},{ty} {bx},{ty} {bx},{y0} {x1},{my} {bx},{y1} {bx},{by} {x0},{by}" {style}/>"#,
                    ty = y0 + neck,
                    by = y1 - neck,
                    bx = x1 - head,
                    my = h / 2.0
                )
            }
            "diamond" => format!(
                r#"<polygon points="{},{y0} {x1},{} {},{y1} {x0},{}" {style}/>"#,
                w / 2.0,
                h / 2.0,
                w / 2.0,
                h / 2.0
            ),
            "line" => format!(r#"<line x1="{x0}" y1="{y0}" x2="{x1}" y2="{y1}" {style}/>"#),
            // 手描きの線(ペン=細い / 蛍光ペン=太くて薄い)も同じ折れ線
            // 縦棒・勝ち負けのスパークライン: base(底)から先端まで棒を立てる。
            // 勝ち負けは負(先端が底より下)を赤に
            "spark-col" | "spark-wl" => {
                let n = self.points.len().max(1) as f32;
                let bw = ((x1 - x0) / n * 0.7).max(1.5);
                let base_y = y0 + self.base * (y1 - y0);
                let mut bars = String::new();
                for p in &self.points {
                    let (px_, py_) = p.at;
                    let cx_ = x0 + px_ * (x1 - x0);
                    let top = y0 + py_ * (y1 - y0);
                    let neg = py_ > self.base + 1e-6;
                    let col = if self.kind == "spark-wl" && neg {
                        "#C0504D"
                    } else {
                        line
                    };
                    let (ry, rh) = if neg {
                        (base_y, (top - base_y).max(1.0))
                    } else {
                        (top, (base_y - top).max(1.0))
                    };
                    bars.push_str(&format!(
                        r#"<rect x="{:.1}" y="{ry:.1}" width="{bw:.1}" height="{rh:.1}" fill="{col}"/>"#,
                        cx_ - bw / 2.0
                    ));
                }
                bars
            }
            "spark" | "ink" | "marker" | "path" => {
                // 正規化した点を大きさに展開する。**制御点があれば曲線**
                let ex = |p: (f32, f32)| (x0 + p.0 * (x1 - x0), y0 + p.1 * (y1 - y0));
                let d = {
                    let mut d = String::new();
                    for (i, p) in self.points.iter().enumerate() {
                        let (ax, ay) = ex(p.at);
                        // **輪郭の切れ目で M を打つ。** 穴のある形は、
                        // 外側の輪郭のあとに内側が続く(SVG は evenodd で抜く)
                        if i == 0 || p.start {
                            d.push_str(&format!("{}M{ax:.1},{ay:.1}", if i == 0 { "" } else { "Z " }));
                            continue;
                        }
                        let prev = &self.points[i - 1];
                        match (prev.c_out, p.c_in) {
                            (None, None) => d.push_str(&format!(" L{ax:.1},{ay:.1}")),
                            (co, ci) => {
                                // 片方しか無ければ、その点自身を控えに使う
                                let (c1x, c1y) = ex(co.unwrap_or(prev.at));
                                let (c2x, c2y) = ex(ci.unwrap_or(p.at));
                                d.push_str(&format!(
                                    " C{c1x:.1},{c1y:.1} {c2x:.1},{c2y:.1} {ax:.1},{ay:.1}"
                                ));
                            }
                        }
                    }
                    d
                };
                let _ = &d;
                let (w_, o_) = match self.kind.as_str() {
                    "marker" => (9.0, 0.45), // 蛍光ペンは太く薄く
                    "ink" => (2.2, 1.0),
                    "path" => (sw, 1.0),
                    _ => (1.5, 1.0),
                };
                // 点の印(高点・低点・最初・最後)。**焼き付けた点だけで決まる**ので
                // 元データを引き直さずに出せる
                let m = self.spark_marks;
                let dots = if self.kind == "spark" && m != SparkMarks::default() {
                    let key = |i: &(usize, &PathPoint)| i.1.at.1;
                    let hi = self.points.iter().enumerate().min_by(|a, b| key(a).total_cmp(&key(b))).map(|(i, _)| i);
                    let lo = self.points.iter().enumerate().max_by(|a, b| key(a).total_cmp(&key(b))).map(|(i, _)| i);
                    let last = self.points.len().checked_sub(1);
                    let mut s = String::new();
                    for (i, pp) in self.points.iter().enumerate() {
                        let (px_, py_) = pp.at;
                        // 高点=赤 / 低点=青 / 最初と最後=線と同じ色。
                        // y は下向きなので**最小が高点**
                        let col = if m.high && Some(i) == hi {
                            Some("#C0504D")
                        } else if m.low && Some(i) == lo {
                            Some("#4472C4")
                        } else if (m.first && i == 0) || (m.last && Some(i) == last) {
                            Some(line)
                        } else {
                            None
                        };
                        if let Some(c) = col {
                            s.push_str(&format!(
                                r#"<circle cx="{:.1}" cy="{:.1}" r="2.2" fill="{c}"/>"#,
                                x0 + px_ * (x1 - x0),
                                y0 + py_ * (y1 - y0)
                            ));
                        }
                    }
                    s
                } else {
                    String::new()
                };
                // 自由な形(path)は塗る。折れ線ものは塗らない
                let f_ = if self.kind == "path" { fill } else { "none" };
                // 穴は even-odd で抜く(内側の輪郭が塗りを打ち消す)
                let close = if self.kind == "path" { "Z" } else { "" };
                format!(
                    r#"<path d="{d}{close}" fill="{f_}" fill-rule="evenodd" stroke="{line}" stroke-width="{w_}" stroke-opacity="{o_}" stroke-linecap="round" stroke-linejoin="round"/>{dots}"#
                )
            }
            // 本家の図形ギャラリーの品揃え。**xlsx の prstGeom の名前を
            // そのまま種類に使う**ので、Excel で作った帳票の形もここに来る
            k => match preset_svg(k, x0, y0, x1, y1, &style) {
                Some(s) => s,
                // 知らない形は四角で描く。**それを黙ってやらない** —
                // 読みの Report が「描けない図形」として一件ずつ数える
                None => format!(
                    r#"<rect x="{x0}" y="{y0}" width="{}" height="{}" {style}/>"#,
                    x1 - x0,
                    y1 - y0
                ),
            },
        }
    }
}

/// その形を**その形として描けるか**。
///
/// 描けない名前は四角で見せることになるので、読みはこれを見て
/// 「描けない図形」として数える。**判断はここ1箇所** — 描く側と
/// 数える側で名前の表が割れると、黙って四角に化けたものが数から漏れる。
pub fn can_draw(kind: &str) -> bool {
    matches!(
        kind,
        // body_svg が直に持っている形
        "rect" | "roundRect" | "ellipse" | "rightArrow" | "diamond" | "line"
        // 点で形を作るもの(このアプリが作る。points で描く)。
        // `path` は**自由な形** — 頂点と制御点を人がつまんで決める
        | "spark" | "spark-col" | "spark-wl" | "ink" | "marker" | "path"
    ) || preset_svg(kind, 0.0, 0.0, 1.0, 1.0, "").is_some()
}

/// prstGeom の名前 → 形の SVG。**描ける形だけ** Some を返す。
///
/// 形は箱(x0,y0)-(x1,y1)に収める。Office の既定の調整値(adj)に寄せた
/// 比で作る — 一致させることが目的ではなく、**その形に見えること**が目的。
/// 知らない名前は None で、呼んだ側が四角に落として読みが数える。
pub fn preset_svg(kind: &str, x0: f32, y0: f32, x1: f32, y1: f32, style: &str) -> Option<String> {
    let (w, h) = (x1 - x0, y1 - y0);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let m = w.min(h);
    let poly = |pts: Vec<(f32, f32)>| {
        let s: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect();
        format!(r#"<polygon points="{}" {style}/>"#, s.join(" "))
    };
    // 星形(外周と内周を交互に。箱に合わせて楕円へ乗せる)
    let star = |n: usize, inner: f32| {
        let (rx, ry) = (w / 2.0, h / 2.0);
        let step = std::f32::consts::PI / n as f32;
        let pts = (0..n * 2)
            .map(|i| {
                let r = if i % 2 == 0 { 1.0 } else { inner };
                let a = -std::f32::consts::FRAC_PI_2 + step * i as f32;
                (cx + a.cos() * rx * r, cy + a.sin() * ry * r)
            })
            .collect();
        poly(pts)
    };
    // 正多角形(頂点を上から時計回り)
    let ngon = |n: usize| {
        let (rx, ry) = (w / 2.0, h / 2.0);
        let step = std::f32::consts::TAU / n as f32;
        let pts = (0..n)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + step * i as f32;
                (cx + a.cos() * rx, cy + a.sin() * ry)
            })
            .collect();
        poly(pts)
    };
    // 十字(腕の太さ t は短辺の割合)
    let cross = |t: f32| {
        let (ax, ay) = (w * t / 2.0, h * t / 2.0);
        poly(vec![
            (cx - ax, y0), (cx + ax, y0), (cx + ax, cy - ay), (x1, cy - ay),
            (x1, cy + ay), (cx + ax, cy + ay), (cx + ax, y1), (cx - ax, y1),
            (cx - ax, cy + ay), (x0, cy + ay), (x0, cy - ay), (cx - ax, cy - ay),
        ])
    };
    // 横棒(数式の記号に使う。t=高さの割合)
    let bar = |t: f32, yc: f32| {
        let th = h * t / 2.0;
        format!(
            r#"<rect x="{x0:.1}" y="{:.1}" width="{w:.1}" height="{:.1}" {style}/>"#,
            yc - th,
            th * 2.0
        )
    };
    let s = match kind {
        // ---- 基本図形 ----
        "triangle" => poly(vec![(cx, y0), (x1, y1), (x0, y1)]),
        "rtTriangle" => poly(vec![(x0, y0), (x0, y1), (x1, y1)]),
        "parallelogram" => {
            let a = w * 0.25;
            poly(vec![(x0 + a, y0), (x1, y0), (x1 - a, y1), (x0, y1)])
        }
        "trapezoid" => {
            let a = w * 0.25;
            poly(vec![(x0 + a, y0), (x1 - a, y0), (x1, y1), (x0, y1)])
        }
        "pentagon" => ngon(5),
        "hexagon" => {
            let a = m * 0.25;
            poly(vec![
                (x0 + a, y0), (x1 - a, y0), (x1, cy), (x1 - a, y1), (x0 + a, y1), (x0, cy),
            ])
        }
        "octagon" => {
            let a = m * 0.293;
            poly(vec![
                (x0 + a, y0), (x1 - a, y0), (x1, y0 + a), (x1, y1 - a),
                (x1 - a, y1), (x0 + a, y1), (x0, y1 - a), (x0, y0 + a),
            ])
        }
        "plus" => cross(0.5),
        // 角を落とした四角(flowChartTerminator と同じ姿=丸みの強い角丸)
        "flowChartTerminator" | "stadium" => format!(
            r#"<rect x="{x0:.1}" y="{y0:.1}" width="{w:.1}" height="{h:.1}" rx="{r:.1}" ry="{r:.1}" {style}/>"#,
            r = h / 2.0
        ),
        // ---- 星とリボン ----
        "star4" => star(4, 0.35),
        "star5" => star(5, 0.382),
        "star6" => star(6, 0.577),
        "star8" => star(8, 0.707),
        // ---- 矢印(右向きは既存。残りは同じ形の向き違い)----
        "leftArrow" | "upArrow" | "downArrow" => {
            let neck = if kind == "leftArrow" { h * 0.25 } else { w * 0.25 };
            match kind {
                "leftArrow" => {
                    let head = (w * 0.35).min(h);
                    poly(vec![
                        (x1, cy - neck), (x0 + head, cy - neck), (x0 + head, y0),
                        (x0, cy), (x0 + head, y1), (x0 + head, cy + neck), (x1, cy + neck),
                    ])
                }
                "upArrow" => {
                    let head = (h * 0.35).min(w);
                    poly(vec![
                        (cx - neck, y1), (cx - neck, y0 + head), (x0, y0 + head),
                        (cx, y0), (x1, y0 + head), (cx + neck, y0 + head), (cx + neck, y1),
                    ])
                }
                _ => {
                    let head = (h * 0.35).min(w);
                    poly(vec![
                        (cx - neck, y0), (cx - neck, y1 - head), (x0, y1 - head),
                        (cx, y1), (x1, y1 - head), (cx + neck, y1 - head), (cx + neck, y0),
                    ])
                }
            }
        }
        "leftRightArrow" => {
            let (neck, head) = (h * 0.25, (w * 0.25).min(h));
            poly(vec![
                (x0, cy), (x0 + head, y0), (x0 + head, cy - neck), (x1 - head, cy - neck),
                (x1 - head, y0), (x1, cy), (x1 - head, y1), (x1 - head, cy + neck),
                (x0 + head, cy + neck), (x0 + head, y1),
            ])
        }
        "upDownArrow" => {
            let (neck, head) = (w * 0.25, (h * 0.25).min(w));
            poly(vec![
                (cx, y0), (x1, y0 + head), (cx + neck, y0 + head), (cx + neck, y1 - head),
                (x1, y1 - head), (cx, y1), (x0, y1 - head), (cx - neck, y1 - head),
                (cx - neck, y0 + head), (x0, y0 + head),
            ])
        }
        // ---- 数式の記号 ----
        "mathPlus" => cross(0.235),
        "mathMinus" => bar(0.235, cy),
        "mathEqual" => {
            let g = h * 0.19;
            format!("{}{}", bar(0.14, cy - g), bar(0.14, cy + g))
        }
        "mathNotEqual" => {
            let g = h * 0.19;
            let sl = w * 0.12;
            format!(
                "{}{}{}",
                bar(0.14, cy - g),
                bar(0.14, cy + g),
                poly(vec![
                    (cx + sl, y0), (cx + sl * 2.0, y0), (cx - sl, y1), (cx - sl * 2.0, y1),
                ])
            )
        }
        "mathMultiply" => {
            // 斜めの十字。腕の太さは短辺の 0.2
            let t = m * 0.1;
            let (dx, dy) = (t, t);
            poly(vec![
                (x0 + dx, y0), (cx, cy - dy * 1.4), (x1 - dx, y0), (x1, y0 + dy),
                (cx + dx * 1.4, cy), (x1, y1 - dy), (x1 - dx, y1), (cx, cy + dy * 1.4),
                (x0 + dx, y1), (x0, y1 - dy), (cx - dx * 1.4, cy), (x0, y0 + dy),
            ])
        }
        // ---- フローチャート(姿が同じものは同じ形で描く)----
        "flowChartProcess" => format!(
            r#"<rect x="{x0:.1}" y="{y0:.1}" width="{w:.1}" height="{h:.1}" {style}/>"#
        ),
        "flowChartDecision" => poly(vec![(cx, y0), (x1, cy), (cx, y1), (x0, cy)]),
        "flowChartInputOutput" => {
            let a = w * 0.25;
            poly(vec![(x0 + a, y0), (x1, y0), (x1 - a, y1), (x0, y1)])
        }
        "flowChartConnector" => format!(
            r#"<ellipse cx="{cx:.1}" cy="{cy:.1}" rx="{:.1}" ry="{:.1}" {style}/>"#,
            w / 2.0,
            h / 2.0
        ),
        "flowChartDocument" => {
            // 下辺が波打つ紙
            let a = h * 0.16;
            format!(
                r#"<path d="M{x0:.1},{y0:.1} L{x1:.1},{y0:.1} L{x1:.1},{:.1} Q{:.1},{:.1} {cx:.1},{:.1} Q{:.1},{:.1} {x0:.1},{:.1} Z" {style}/>"#,
                y1 - a,
                x1 - w * 0.25, y1 - a * 2.0,
                y1 - a,
                x0 + w * 0.25, y1,
                y1 - a
            )
        }
        // ---- 吹き出し ----
        "wedgeRectCallout" => {
            let b = y1 - h * 0.22; // 本体の下辺
            format!(
                r#"<path d="M{x0:.1},{y0:.1} L{x1:.1},{y0:.1} L{x1:.1},{b:.1} L{:.1},{b:.1} L{:.1},{y1:.1} L{:.1},{b:.1} L{x0:.1},{b:.1} Z" {style}/>"#,
                x0 + w * 0.42,
                x0 + w * 0.18,
                x0 + w * 0.30
            )
        }
        "wedgeEllipseCallout" => {
            // **楕円と尻尾を1本の輪郭で描く。** 別々に描くと継ぎ目の線が
            // 吹き出しの中を横切る(実機の絵で見えた)
            let (rx, ry) = (w / 2.0, h * 0.39);
            let ec = y0 + ry;
            let at = |deg: f32| {
                let a = deg.to_radians();
                (cx + a.cos() * rx, ec + a.sin() * ry)
            };
            // 下側の左寄りに口を開ける(y は下向きなので 90 度が真下)
            let (p1x, p1y) = at(100.0);
            let (p2x, p2y) = at(140.0);
            format!(
                r#"<path d="M{p1x:.1},{p1y:.1} A{rx:.1},{ry:.1} 0 1 0 {p2x:.1},{p2y:.1} L{:.1},{y1:.1} Z" {style}/>"#,
                x0 + w * 0.18
            )
        }
        _ => return None,
    };
    Some(s)
}

/// シートに浮かぶ画像。左上をセルに留める(xlsx の oneCellAnchor)。
#[derive(Debug, Clone)]
pub struct SheetImage {
    /// 左上を留めるセル
    pub at: Pos,
    /// アンカーのセルからのずらし(px)。移動でセルに収まらない分を持つ
    pub dx_px: f32,
    pub dy_px: f32,
    /// 画面での大きさ(px)。xlsx の EMU とは 9525 EMU = 1px で換算
    pub width_px: f32,
    pub height_px: f32,
    /// 絵の実体(PNG / JPEG)
    pub data: Vec<u8>,
}

/// データの入力規則(list だけ)。「この範囲は、この候補から選ぶ」。
///
/// `formula` は xlsx の formula1 の**原文**で持つ — `"甲,乙,丙"`(引用符つきの
/// 直書き)か `$D$2:$D$5`(同じシートの範囲参照)。候補は使うときに解決する
/// (範囲参照の中身が変われば候補も変わる — 原文を持てば追従できる)。
#[derive(Debug, Clone, PartialEq)]
pub struct Validation {
    pub range: (Pos, Pos),
    /// list の候補(直書き `"a,b"` か範囲参照)。他の種類では formula1
    pub formula: String,
    /// 種類。xlsx の type をそのまま持つ: "list" / "whole"(整数)/
    /// "decimal" / "textLength" / "date" / "time" / "custom" / ""(文言だけ)。
    /// 知らない種類も**落とさず持ち越す**(判定は分かるものだけ)
    pub kind: String,
    /// 比較(xlsx の operator)。between / notBetween / equal / notEqual /
    /// greaterThan / lessThan / greaterThanOrEqual / lessThanOrEqual
    pub op: String,
    /// formula2(between / notBetween の右端)
    pub formula2: String,
    /// 入力メッセージ(題, 本文)。セルに乗ると出す
    pub input_msg: Option<(String, String)>,
    /// エラーの文言(様式 "stop"/"warning"/"information", 題, 本文)。
    /// stop は堰き止める、warning/information は通すが言う
    pub error_msg: Option<(String, String, String)>,
    /// 空白を無視(xlsx の allowBlank)。false なら空にするのも堰き止める
    pub allow_blank: bool,
    /// セルの ▾ を出さない(xlsx の showDropDown="1" — 名前と逆で「隠す」)
    pub hide_arrow: bool,
}

impl Validation {
    /// リストの規則(従来の形)
    pub fn list(range: (Pos, Pos), formula: String) -> Validation {
        Validation {
            range,
            formula,
            kind: "list".into(),
            op: String::new(),
            formula2: String::new(),
            input_msg: None,
            error_msg: None,
            allow_blank: true,
            hide_arrow: false,
        }
    }

    pub fn contains(&self, p: Pos) -> bool {
        let (a, b) = self.range;
        (a.row..=b.row).contains(&p.row) && (a.col..=b.col).contains(&p.col)
    }

    /// 打った文字が規則に合うか。**判定できない規則は堰き止めない**
    /// (date/time/custom、数でない式 — 読めない規則で入力を止めない方針)
    pub fn passes(&self, sheet: &Sheet, text: &str) -> bool {
        match self.kind.as_str() {
            "list" => {
                let opts = self.options(sheet);
                opts.is_empty() || opts.iter().any(|o| o == text)
            }
            "whole" | "decimal" => {
                // 規則の式が数として読めない(セル参照など)なら判定できない —
                // 文字を打っても堰き止めない(読めない規則で入力を止めない方針)
                if !self.judgeable() {
                    return true;
                }
                let Ok(x) = text.replace(',', "").parse::<f64>() else {
                    return false; // 数の規則に数でないものは合わない
                };
                if self.kind == "whole" && x.fract() != 0.0 {
                    return false;
                }
                self.op_passes(x).unwrap_or(true)
            }
            "textLength" => {
                let n = text.chars().count() as f64;
                self.op_passes(n).unwrap_or(true)
            }
            _ => true,
        }
    }

    /// この規則は判定できるか(比較が分かり、式が数として読めるか)
    fn judgeable(&self) -> bool {
        let f1 = self.formula.trim().parse::<f64>().is_ok();
        match self.op.as_str() {
            "between" | "" | "notBetween" => {
                f1 && self.formula2.trim().parse::<f64>().is_ok()
            }
            "equal" | "notEqual" | "greaterThan" | "lessThan"
            | "greaterThanOrEqual" | "lessThanOrEqual" => f1,
            _ => false,
        }
    }

    /// 比較そのもの。式が数として読めなければ None(判定できない)
    fn op_passes(&self, x: f64) -> Option<bool> {
        let f1: f64 = self.formula.trim().parse().ok()?;
        Some(match self.op.as_str() {
            "between" | "" => {
                let f2: f64 = self.formula2.trim().parse().ok()?;
                (f1..=f2).contains(&x)
            }
            "notBetween" => {
                let f2: f64 = self.formula2.trim().parse().ok()?;
                !(f1..=f2).contains(&x)
            }
            "equal" => x == f1,
            "notEqual" => x != f1,
            "greaterThan" => x > f1,
            "lessThan" => x < f1,
            "greaterThanOrEqual" => x >= f1,
            "lessThanOrEqual" => x <= f1,
            _ => return None,
        })
    }

    /// 規則の言い直し(エラーの既定の文言に使う)。例: 「1 から 100 の整数」
    pub fn describe(&self) -> String {
        let noun = match self.kind.as_str() {
            "whole" => "整数",
            "decimal" => "数",
            "textLength" => "文字数",
            _ => return String::new(),
        };
        let (f1, f2) = (self.formula.trim(), self.formula2.trim());
        match self.op.as_str() {
            "between" | "" => format!("{f1} から {f2} の{noun}"),
            "notBetween" => format!("{f1} から {f2} の外の{noun}"),
            "equal" => format!("{f1} に等しい{noun}"),
            "notEqual" => format!("{f1} 以外の{noun}"),
            "greaterThan" => format!("{f1} より大きい{noun}"),
            "lessThan" => format!("{f1} より小さい{noun}"),
            "greaterThanOrEqual" => format!("{f1} 以上の{noun}"),
            "lessThanOrEqual" => format!("{f1} 以下の{noun}"),
            _ => noun.to_string(),
        }
    }

    /// 候補の一覧。直書きは `,` で割り、範囲参照はそのシートの値を集める。
    /// 解決できない参照(別のシート等)は空 — 空の候補は「制限なし」と扱うこと
    /// (読めない規則で入力を堰き止めない)。
    pub fn options(&self, sheet: &Sheet) -> Vec<String> {
        let f = self.formula.trim();
        if let Some(inner) = f.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            return inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        // 範囲参照。$ は絶対参照の印なので剥がして読む
        let clean: String = f.chars().filter(|c| *c != '$').collect();
        let (a, b) = match clean.split_once(':') {
            Some((x, y)) => match (Pos::parse(x), Pos::parse(y)) {
                (Some(a), Some(b)) => (a, b),
                _ => return Vec::new(),
            },
            None => match Pos::parse(&clean) {
                Some(p) => (p, p),
                None => return Vec::new(),
            },
        };
        let mut out = Vec::new();
        for r in a.row..=b.row {
            for c in a.col..=b.col {
                if let Some(cell) = sheet.cells.get(&Pos::new(r, c)) {
                    let v = cell.value.display();
                    if !v.is_empty() && !out.contains(&v) {
                        out.push(v);
                    }
                }
            }
        }
        out
    }
}

/// 条件付き書式が当たったときの見た目(xlsx の `dxf`)。
///
/// **色と塗りだけではない。** dxf は太字・斜体・下線・取り消し線も持てる。
/// 前は色と塗りしか読んでおらず、Excel で「赤字・太字」にした規則が
/// **規則は残ったまま太字と文字色だけ落ちて**開いていた
/// (2026-08-10 pyoffice セッションの報告。`~/xlsx-corpus/make_cond.xlsx`)。
/// 規則が消えるより気付きにくい壊れ方で、画面では「条件付き書式が効いて
/// いるのに赤くならない」と出る。
///
/// 飾りが `Option<bool>` なのは **dxf が三択だから** — 書いていない
/// (触らない)・`<b/>`(太字にする)・`<b val="0"/>`(太字を外す)。
/// `bool` に潰すと「触らない」と「外す」の区別が消えて、
/// 元から太字のセルが規則に当たった途端に細くなる
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CondLook {
    /// 文字色 RRGGBB
    pub color: Option<String>,
    /// 塗り RRGGBB
    pub fill: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
}

impl CondLook {
    /// 色も塗りも飾りも無い(= dxf を書く必要が無い)
    pub fn is_empty(&self) -> bool {
        *self == CondLook::default()
    }
}

/// 条件付き書式の1本。「範囲の値が◯◯なら、この見た目」。
#[derive(Debug, Clone, PartialEq)]
pub struct CondRule {
    pub range: (Pos, Pos),
    pub kind: CondKind,
    /// 当たったときの見た目
    pub look: CondLook,
}

/// 規則の種類(第1版 2026-08-07 で拡張 — 前は数の比較だけ)。
/// データバー・カラースケール・アイコン・日付・数式は控え(読みで報告)
#[derive(Debug, Clone, PartialEq)]
pub enum CondKind {
    /// 数の比較(cellIs)
    Cmp(CondOp, f64),
    /// 間(小さい方, 大きい方, true=外側)
    Between(f64, f64, bool),
    /// 文字を含む(containsText)
    Text(String),
    /// 重複する値(true = 一意のほう)
    Dup(bool),
    /// 上位N(true = 下位)
    Top(u32, bool),
    /// 平均より上(true = 下)
    Avg(bool),
    /// データバー(棒の色 RRGGBB)。最小〜最大を棒の長さに
    Bar(String),
    /// カラースケール(最小色, 中間色, 最大色)。中間なしは2色
    Scale(String, Option<String>, String),
    /// アイコンセット(xlsx の iconSet 名。例 "3Arrows")
    Icons(String),
    /// 数式で指定(xlsx の `expression`)。真になったセルに書式が付く。
    /// 実物の帳票でいちばん多い条件付き書式 — `MOD(ROW(),2)=0` の縞。
    ///
    /// **持つのは範囲の左上を錨にした原文**(`=` は付けない。xlsx の
    /// `<formula>` と同じ形)。例えば範囲 C2:C11 の `LEN(C2)>3` は、
    /// C2 のことを書いた式として貯める。
    ///
    /// **評価する。** セルごとに左上からのずれだけ相対参照をずらし
    /// (`offset_refs`)、その位置で解く(`aux()` が範囲を1回だけ歩いて
    /// 当たりを控え、`hits()` はその控えを見る)。上の例なら C5 では
    /// `LEN(C5)>3` を解く。**`$` で固定した側は動かない** — `$D2="済"` の
    /// ような行まるごとの色分けが、これで意図どおりに効く。
    ///
    /// ずらすのは**解くときだけ**で、持っている原文は書き換えない。
    /// 保存はいつも原文をそのまま返すので、評価が外れても**ファイルは減らない**。
    Formula(String),
}

/// `expression` を解くセル数の上限。式は1セルずつ字句解析から解くので、
/// 列まるごと(100万セル)を毎回の描画で回すわけにはいかない。
/// 超えた範囲は**当たり無し**にする(効かないのは見れば分かるが、
/// 見当違いの縞を自信たっぷりに描かれると気付けない)
pub(super) const COND_FORMULA_MAX: u64 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondOp {
    Gt,
    Lt,
    Eq,
    Ge,
    Le,
    Ne,
}

impl CondOp {
    pub fn as_xlsx(self) -> &'static str {
        match self {
            CondOp::Gt => "greaterThan",
            CondOp::Lt => "lessThan",
            CondOp::Eq => "equal",
            CondOp::Ge => "greaterThanOrEqual",
            CondOp::Le => "lessThanOrEqual",
            CondOp::Ne => "notEqual",
        }
    }
    pub fn from_xlsx(s: &str) -> Option<CondOp> {
        match s {
            "greaterThan" => Some(CondOp::Gt),
            "lessThan" => Some(CondOp::Lt),
            "equal" => Some(CondOp::Eq),
            "greaterThanOrEqual" => Some(CondOp::Ge),
            "lessThanOrEqual" => Some(CondOp::Le),
            "notEqual" => Some(CondOp::Ne),
            _ => None,
        }
    }
}

/// 範囲ぐるみの規則(重複・上位N・平均)の下ごしらえ。
/// 描画の前に1回だけ作り、セルごとの判定はこれを見る(毎セル走査しない)
#[derive(Debug, Clone, Default)]
pub struct CondAux {
    pub avg: f64,
    pub cutoff: f64,
    pub dups: std::collections::HashSet<String>,
    /// 範囲の最小・最大(バー/スケール/アイコンの物差し)
    pub min: f64,
    pub max: f64,
    /// 数式で指定(expression)が当たった位置。`hits()` には表が無いので、
    /// 式を解くのはここ(範囲を1回歩く)で済ませ、判定は引くだけにする
    pub hit: std::collections::HashSet<Pos>,
}

impl CondRule {
    /// 下ごしらえ(必要な種類のときだけ範囲を歩く)
    pub fn aux(&self, s: &Sheet) -> CondAux {
        let mut aux = CondAux::default();
        let (a, b) = self.range;
        match &self.kind {
            CondKind::Bar(_) | CondKind::Scale(..) | CondKind::Icons(_) => {
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for r in a.row..=b.row {
                    for c in a.col..=b.col {
                        if let Value::Number(x) = s.value(Pos::new(r, c)) {
                            lo = lo.min(x);
                            hi = hi.max(x);
                        }
                    }
                }
                if lo.is_finite() {
                    aux.min = lo;
                    aux.max = hi;
                }
            }
            CondKind::Avg(_) => {
                let (mut sum, mut n) = (0.0, 0u32);
                for r in a.row..=b.row {
                    for c in a.col..=b.col {
                        if let Value::Number(x) = s.value(Pos::new(r, c)) {
                            sum += x;
                            n += 1;
                        }
                    }
                }
                aux.avg = if n > 0 { sum / n as f64 } else { 0.0 };
            }
            CondKind::Top(n, bottom) => {
                let mut xs: Vec<f64> = Vec::new();
                for r in a.row..=b.row {
                    for c in a.col..=b.col {
                        if let Value::Number(x) = s.value(Pos::new(r, c)) {
                            xs.push(x);
                        }
                    }
                }
                xs.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                if *bottom {
                    xs.reverse();
                }
                // 上位N の足切り(N個に満たなければ全部)
                let i = xs.len().saturating_sub(*n as usize);
                aux.cutoff = xs.get(i).copied().unwrap_or(f64::NEG_INFINITY);
                if *bottom && !xs.is_empty() {
                    // 下位: 逆順で「上位N」を取ったのと同じ — 足切りは N 番目に小さい値
                    aux.cutoff = xs.get(i).copied().unwrap_or(f64::INFINITY);
                }
            }
            CondKind::Dup(_) => {
                let mut seen: std::collections::HashMap<String, u32> = Default::default();
                for r in a.row..=b.row {
                    for c in a.col..=b.col {
                        let v = s.value(Pos::new(r, c)).display();
                        if !v.is_empty() {
                            *seen.entry(v).or_insert(0) += 1;
                        }
                    }
                }
                aux.dups = seen.into_iter().filter(|(_, n)| *n >= 2).map(|(k, _)| k).collect();
            }
            CondKind::Formula(f) => {
                // 式は範囲の左上に錨がある。セルごとに左上からのずれだけ
                // 相対参照をずらして解く($ で固定した側は動かない)
                // 逆さまの範囲("B10:A1")でも引き算が回らないよう abs_diff で数える
                let cells =
                    (b.row.abs_diff(a.row) as u64 + 1) * (b.col.abs_diff(a.col) as u64 + 1);
                if f.trim().is_empty() || cells > COND_FORMULA_MAX {
                    return aux;
                }
                for r in a.row..=b.row {
                    for c in a.col..=b.col {
                        let p = Pos::new(r, c);
                        let moved =
                            offset_refs(f, (r - a.row) as i64, (c - a.col) as i64);
                        // 真(TRUE か 0 でない数)だけを当たりにする。
                        // 誤り・文字・空は当たらない側へ倒す — 解けなかった式で
                        // 書式を**付ける**より、付けないほうが害が小さい
                        let hit = match crate::calc::eval_once(s, p, &moved) {
                            Value::Bool(t) => t,
                            Value::Number(n) => n != 0.0,
                            _ => false,
                        };
                        if hit {
                            aux.hit.insert(p);
                        }
                    }
                }
            }
            _ => {}
        }
        aux
    }

    /// この位置のこの値に効くか(aux は同じ規則の下ごしらえ)
    pub fn hits(&self, p: Pos, v: &Value, aux: &CondAux) -> bool {
        let (a, b) = self.range;
        if !((a.row..=b.row).contains(&p.row) && (a.col..=b.col).contains(&p.col)) {
            return false;
        }
        match &self.kind {
            CondKind::Cmp(op, value) => {
                let Value::Number(n) = v else { return false };
                match op {
                    CondOp::Gt => *n > *value,
                    CondOp::Lt => *n < *value,
                    CondOp::Eq => (*n - *value).abs() < f64::EPSILON,
                    CondOp::Ge => *n >= *value,
                    CondOp::Le => *n <= *value,
                    CondOp::Ne => (*n - *value).abs() >= f64::EPSILON,
                }
            }
            CondKind::Between(lo, hi, out) => {
                let Value::Number(n) = v else { return false };
                let inside = *n >= *lo && *n <= *hi;
                inside != *out
            }
            CondKind::Text(t) => !t.is_empty() && v.display().contains(t.as_str()),
            CondKind::Dup(unique) => {
                let d = v.display();
                if d.is_empty() {
                    return false;
                }
                aux.dups.contains(&d) != *unique
            }
            CondKind::Top(_, bottom) => {
                let Value::Number(n) = v else { return false };
                if *bottom { *n <= aux.cutoff } else { *n >= aux.cutoff }
            }
            CondKind::Avg(below) => {
                let Value::Number(n) = v else { return false };
                if *below { *n < aux.avg } else { *n > aux.avg }
            }
            // 数式で指定は aux() で解き終えている(ここに表が無いため)
            CondKind::Formula(_) => aux.hit.contains(&p),
            // バー/スケール/アイコンは「当たり外れ」ではなく物差し —
            // scalar() で 0〜1 を取り、描く側が形にする
            CondKind::Bar(_) | CondKind::Scale(..) | CondKind::Icons(_) => false,
        }
    }

    /// 範囲の中での位置(0=最小 〜 1=最大)。バー/スケール/アイコン用。
    /// 全部同じ値なら 1.0(Excel も満杯の棒を描く)
    pub fn scalar(&self, p: Pos, v: &Value, aux: &CondAux) -> Option<f64> {
        if !matches!(self.kind, CondKind::Bar(_) | CondKind::Scale(..) | CondKind::Icons(_)) {
            return None;
        }
        let (a, b) = self.range;
        if !((a.row..=b.row).contains(&p.row) && (a.col..=b.col).contains(&p.col)) {
            return None;
        }
        let Value::Number(n) = v else { return None };
        if aux.max <= aux.min {
            return Some(1.0);
        }
        Some(((n - aux.min) / (aux.max - aux.min)).clamp(0.0, 1.0))
    }

    /// カラースケールの色(0〜1 → RRGGBB)。2色は直線、3色は 0.5 で折り返し
    pub fn scale_color(&self, t: f64) -> Option<String> {
        let CondKind::Scale(lo, mid, hi) = &self.kind else { return None };
        fn ch(s: &str, i: usize) -> f64 {
            u8::from_str_radix(s.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f64
        }
        let lerp = |x: &str, y: &str, t: f64| -> String {
            format!(
                "{:02X}{:02X}{:02X}",
                (ch(x, 0) + (ch(y, 0) - ch(x, 0)) * t).round() as u8,
                (ch(x, 2) + (ch(y, 2) - ch(x, 2)) * t).round() as u8,
                (ch(x, 4) + (ch(y, 4) - ch(x, 4)) * t).round() as u8,
            )
        };
        Some(match mid {
            None => lerp(lo, hi, t),
            Some(m) if t < 0.5 => lerp(lo, m, t * 2.0),
            Some(m) => lerp(m, hi, (t - 0.5) * 2.0),
        })
    }
}

impl Sheet {
    pub fn new(name: &str) -> Sheet {
        Sheet { name: name.to_string(), ..Default::default() }
    }
    pub fn get(&self, p: Pos) -> Option<&Cell> {
        self.cells.get(&p)
    }
    pub fn value(&self, p: Pos) -> Value {
        self.cells.get(&p).map(|c| c.value.clone()).unwrap_or(Value::Empty)
    }
    /// セルを置く。
    ///
    /// **中身も書式も無いセルは持たない**(表が無駄に太る)。
    /// ただし**罫線だけのセルは残す** — 値が無くても、
    /// 枠が引いてあれば帳票では意味を持つ。
    pub fn set(&mut self, p: Pos, c: Cell) {
        if c.formula.is_none() && c.value.is_empty() && c.fmt.is_plain() {
            self.cells.remove(&p);
        } else {
            self.cells.insert(p, c);
        }
    }
    /// 使われている範囲(行数, 列数)。空なら (0,0)。
    pub fn extent(&self) -> (u32, u32) {
        self.cells.keys().fold((0, 0), |(r, c), p| (r.max(p.row + 1), c.max(p.col + 1)))
    }

    /// **見せる大きさ。** 次の4つの、いちばん大きい所:
    ///
    /// - `extent` — 中身のあるセル
    /// - `dim` — 原本の `<dimension>` の申告
    /// - `seen` — **`<c>` は置かれていたが、値も書式も無かった所**
    /// - セルは無いが高さ・幅・隠し・畳みを持つ行や列
    ///
    /// `extent` と分けてあるのは、`extent` が「中身のある範囲」として
    /// 38 箇所から使われているため。**あちらの意味は変えない。**
    ///
    /// なぜ大きいほうを採るか(2026-08-10 発注者確定): 申告だけを信じると、
    /// `A1` としか書かない手抜きな書き手のときに**中身のあるセルが範囲の外に
    /// 落ちる** — 値が消えるのは高さが消えるより重い。実際だけを見ると、
    /// 末尾の空行の高さや罫線だけの枠が届かず、帳票の見た目が変わる。
    /// 日銀の資金循環では 59 枚のシートで申告と実際が食い違っていた
    /// (最大で 30 行ぶん)。
    pub fn size(&self) -> (u32, u32) {
        let (mut r, mut c) = self.extent();
        for (dr, dc) in [self.dim, self.seen].into_iter().flatten() {
            r = r.max(dr);
            c = c.max(dc);
        }
        let rows = self
            .row_height
            .keys()
            .chain(self.row_hidden.iter())
            .chain(self.row_outline.keys());
        for k in rows {
            r = r.max(k + 1);
        }
        let cols = self
            .col_width
            .keys()
            .chain(self.col_hidden.iter())
            .chain(self.col_outline.keys());
        for k in cols {
            c = c.max(k + 1);
        }
        (r, c)
    }
}

/// ピボットの指図。src の表を集計して dest に**その時の値**で置いたもの。
/// セルには値しか残らない — この指図があるから「更新」で置き直せる。
/// 更新は明示の操作のときだけ polars を回す(開く=再計算はしない)。
#[derive(Debug, Clone, PartialEq)]
pub struct PivotDef {
    /// 元の表があるシートの名前(番号ではなく名前 — 並べ替えに耐える)
    pub sheet: String,
    pub src: (Pos, Pos),
    pub rows_sel: Vec<String>,
    pub cols_sel: Vec<String>,
    pub value: String,
    pub agg: String,
    /// 総計(行。列に広げていれば総計の列も)
    pub totals: bool,
    /// 小計(行の見出しが2つ以上のとき、1つ目の区切りごと)
    pub subtotals: bool,
    /// 空行(1つ目の区切りごとに1行空ける)
    pub blank_rows: bool,
    /// コンパクト形式(繰り返しの見出しを空欄に)。false = 表形式
    pub compact: bool,
    pub dest: Pos,
    /// 置いた広さ(行数, 列数)— 更新のとき前の面を消すため
    pub size: (u32, u32),
    /// 絞り込み: (見出し, 隠す値の列)。空 = 素通し。
    /// 見出しの ▼ から入切し、更新のたびに polars へ渡す
    pub hide: Vec<(String, Vec<String>)>,
    /// 見た目の組(""=青(既定) / "緑" / "橙" / "灰")。置くときの帯の色
    pub style: String,
    /// 名前(ピボットテーブル1, 2, …)。パネルの題と状態行で名指しする
    pub name: String,
    /// 値のフィルター: (比較 ">" ">=" "<" "<=" "=", しきい値)。
    /// 集計した後の行に掛ける(列に広げていれば行の総計で判定)
    pub vfilter: Option<(String, f64)>,
    /// グループ化: (見出し, 単位)。単位は 月/四半期/年/幅:N
    pub group_by: Vec<(String, String)>,
    /// 計算の種類(""=そのまま / "比率"=総計に対する % / "累計" / "差"=前の行との差)。
    /// **累計と差は小計・総計を出さない** — 積み上げの途中に総計が挟まると
    /// 読み違えるため(効かせるときに totals/subtotals を落とす)
    pub show_as: String,
    /// 並べ替え(""=そのまま / "見出しの昇順" / "見出しの降順" /
    /// "値の大きい順" / "値の小さい順")。**小計・空行を出しているときは
    /// 掛けない** — 区切りの塊を崩してしまうので(2026-08-13)
    pub sort: String,
}

/// ブックの情報(docProps/core.xml の主な欄)。読んで見せる。
/// 保存は原文持ち越しなので、開いたファイルの情報は保存で消えない。
#[derive(Debug, Clone, Default)]
pub struct BookProps {
    /// 著者。**`dc:creator` は1つの要素に `;` 区切りで何人でも入る**
    /// (Excel の慣習)。並びが意味を持つ(先頭が主著者)ので集合ではなく列。
    /// 読みで割り、書きで繋ぐ — 綴りの都合はここより外に出さない
    pub creators: Vec<String>,
    pub title: String,
    pub subject: String,
    pub keywords: String,
    pub description: String,
    /// カスタムプロパティ(docProps/custom.xml)。**core.xml とは別の部品** —
    /// 宣言(Content_Types)と関係(_rels/.rels)も要る。空なら部品ごと作らない
    pub custom: Vec<CustomProp>,
}

/// カスタムプロパティ1件(`docProps/custom.xml` の `<property>`)。
/// 名前は**ブックの中で一意**(Excel も重複を許さない)。
#[derive(Debug, Clone, PartialEq)]
pub struct CustomProp {
    pub name: String,
    pub value: CustomVal,
    /// 「内容にリンク」(`linkTarget` — 値の出どころが名前付き範囲)。
    /// こちらは**繋ぎ直さないが、外しもしない**。落とすと Excel で
    /// 繋がっていた札が黙って静かな値に変わる。画面では鎖の印を出す
    pub link: Option<String>,
}

/// カスタムプロパティの値。型は `vt:` の要素名で決まる。
#[derive(Debug, Clone, PartialEq)]
pub enum CustomVal {
    /// `vt:lpwstr` — 文字
    Text(String),
    /// `vt:r8` — 数
    Number(f64),
    /// `vt:filetime` — 日付。中身は `2026-08-13T00:00:00Z` の綴りのまま持つ
    /// (暦の計算はしない。往復で崩さないことが先)
    Date(String),
    /// `vt:bool` — はい・いいえ
    Bool(bool),
    /// **こちらが知らない型**(`vt:i4`・`vt:cy` など)。タグ名と中身をそのまま
    /// 持って保存で同じ要素を書き戻す。**黙って落とさない** —
    /// 画面では値だけ見せ、打ち直しはさせない
    Other(String, String),
}

/// 表オブジェクト(xlsx の table。範囲に名前と性質を付けたもの)。
/// **方針変更 2026-08-04(発注者)**: 前は「持たない」としていたが、
/// 範囲に変換・サイズ変更は表そのものが無いと成り立たないので持つことにした。
/// 見た目(帯・縞々)は書式として掛かる — 表を外しても書式は残る(Excel と同じ)
#[derive(Debug, Clone, PartialEq)]
pub struct TableDef {
    /// 表の名前(式から使える。空白は入れられない)
    pub name: String,
    /// **表の様式の名前**(`<tableStyleInfo name="TableStyleMedium2"/>`)。
    ///
    /// **`name` とは別物。** `name` は `Table1` のような式から使う識別子で、
    /// こちらは見た目の名前。混同すると、保存で他人の帳票の配色が変わる。
    ///
    /// `None` は原本に指定が無いという意味。書くときは既定の
    /// `TableStyleMedium2` に落とす(Excel が新しい表に付けるもの)
    pub style: Option<String>,
    /// 範囲(左上, 右下)。見出し行を含む
    pub a: Pos,
    pub b: Pos,
    /// 見出し行がある(1行目が見出し)
    pub header: bool,
    /// 合計行がある(最後の行が合計)
    pub totals: bool,
    pub banded_rows: bool,
    pub banded_cols: bool,
    pub first_col: bool,
    pub last_col: bool,
    /// 見出しに絞り込みのボタンを出す
    pub filter: bool,
}

impl Default for TableDef {
    fn default() -> Self {
        TableDef {
            name: "テーブル1".into(),
            style: None,
            a: Pos::new(0, 0),
            b: Pos::new(0, 0),
            header: true,
            totals: false,
            banded_rows: true,
            banded_cols: false,
            first_col: false,
            last_col: false,
            filter: true,
        }
    }
}

impl TableDef {
    /// この位置は表の中か
    pub fn contains(&self, p: Pos) -> bool {
        (self.a.row..=self.b.row).contains(&p.row)
            && (self.a.col..=self.b.col).contains(&p.col)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Book {
    pub sheets: Vec<Sheet>,
    /// ブックの情報(ファイルの全面ページで見せる)
    pub props: BookProps,
    /// テーマの色の組(12色)。空なら Office の既定
    pub theme: Vec<String>,
    /// こちらが理解できなかった definedName の原文(Print_Area など)。
    /// **理解はしないが、捨てない。** 保存でそのまま返す
    pub names_raw: Vec<String>,
    /// 名前付きセルスタイル(名前, builtinId, 書式)。xlsx の
    /// `cellStyles` + `cellStyleXfs` から読む。**型紙が持つ書式の器** —
    /// マークダウンの見出し(`# `)の大きさはここから引く(2026-08-09 発注者
    /// 「テンプレートに設定できませんか?」)。無ければアプリの既定に落ちる。
    /// 保存は原本の styles.xml 据え置きが担うので、こちらは読むだけ
    pub named_styles: Vec<(String, Option<u32>, CellFormat)>,
    /// **このアプリで足した**名前付きセル様式(名前, 書式)。保存で
    /// styles.xml の cellStyleXfs / cellStyles に追記する(画像や docx の
    /// styles_new と同じ「原本へ差す」作法。2026-08-13)
    pub named_styles_new: Vec<(String, CellFormat)>,
    /// 古いブックに載っていた Python(名前, コード)。**読むだけ** —
    /// 実行せず、保存では書き戻さない(2026-08-09 発注者確定: データと
    /// プログラムを1つのファイルにしない)。@export で .py に取り出し、
    /// 中を確かめて plugins へ置くのが取り込みの門。SEKKEI「Python in Calc」参照
    pub scripts: Vec<(String, String)>,
    /// ピボットの指図(xl/joPivot.xml で往復する独自部品)。
    /// Excel で保存し直すと消える — そのときピボットはただの値になる(正直な劣化)
    pub pivots: Vec<PivotDef>,
    /// 計算方法が手動(xlsx の calcPr calcMode="manual")。
    /// F9 で手回し。ファイルに残す — 開き直して勝手に自動へ戻さない
    pub calc_manual: bool,
    /// 反復計算(循環参照の反復解決)。Some((最大反復回数, 最大変化量))。
    /// xlsx の calcPr iterate/iterateCount/iterateDelta と往復
    pub calc_iter: Option<(u32, f64)>,
    /// R1C1 参照スタイル(calcPr refMode="R1C1")。式は内部では A1 のまま —
    /// 見せるとき・打つときに formula_to_r1c1 / formula_from_r1c1 で変換する
    pub r1c1: bool,
    /// **読み取り専用を勧める**(xlsx の workbookProtection@readOnlyRecommended)。
    /// 鍵ではなく**お願い** — 開いた人に「見るだけにしませんか」と聞く。
    /// 鍵を掛けた振りをしないのがこちらの作法なので、そのまま「お願い」で持つ
    pub read_only_rec: bool,
    /// 1904 起点のブック(workbookPr の date1904。古い Mac 由来)。
    /// 通し番号 0 が 1904-01-01 — 日付の関数と表示はこの旗で起点を替える
    /// (2026-08-13、起点の解釈をエンジンに)。保存は原本の workbook.xml が
    /// 属性ごと持ち越すので、旗はここでは書かない
    pub date1904: bool,
    /// 変更履歴(校閲の記録)。**記録中の差分を刻んだもの**で、
    /// xl/joChanges.xml で往復する独自部品 — Excel は読まない(正直な劣化)
    pub changes: Vec<ChangeRec>,
    /// このブックの出どころ(絶対の径路)。**ファイルには入れない** —
    /// 開いた側が入れる。空 = まだ保存していない。
    ///
    /// `CELL("filename")` だけが使う。Excel は `径路[ファイル名]シート名`
    /// を返し、実物では **`]` の後ろを取ってシート名にする常套句**として
    /// 使われている(`=MID(CELL("filename",A1), FIND("]",…)+1, 31)`)
    pub path: String,
}

/// 変更履歴の1件。「誰が・いつ・どのシートのどのセルを・何から何へ」。
/// 値でなく**打った姿**(editable)で持つ — 式が式のまま残る
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChangeRec {
    /// 名乗り(USER@ホスト。ロック・チャットと同じ)
    pub who: String,
    /// いつ(YYYY-MM-DD HH:MM。文字のまま持つ — 暦の計算はしない)
    pub when: String,
    pub sheet: String,
    pub at: Pos,
    /// 前の姿(空 = 無かった)
    pub before: String,
    /// 後の姿(空 = 消した)
    pub after: String,
}

impl Book {
    pub fn new() -> Book {
        Book { sheets: vec![Sheet::new("Sheet1")], ..Default::default() }
    }

}
