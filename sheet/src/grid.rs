//! 値を引ける表。**式の計算が表に求めるのは、この面だけです。**
//!
//! いままで `sheet::calc` は `Sheet` を直に触っていました。けれども実際に
//! 見ていたのは 5 つの物だけで、`Sheet` の残り(書式・列幅・図形・
//! ピボット…)は式の計算に一度も出てきません。数えたところ、関数の
//! 2,338 行は `Sheet` を1度も見ていません。
//!
//! そこで「値を引ける表」を [`Grid`] という名前で決めて、`Sheet` は
//! その1つ、という形にしました。文書の中の表([`kumihan::Table`])も
//! 同じ形にできるので、**文章の中の表でセル関数が使えます**。
//!
//! 設計は SEKKEI.adoc「エンジンの統一 — 表を1つにする」の 2 段目です。

use crate::model::{Pos, Sheet, TableDef, Value};

/// **字で書かれた表**を、値を引ける表にした物。
///
/// 文書の中の表(`kumihan::Table`)や CSV のように、中身が字だけの表を
/// 計算に載せるための容れ物です。`kumihan` に依存しないよう、
/// 受け取るのは字の並びだけにしてあります — 表から字を取り出すのは
/// 表を持っている側の仕事です(`kumihan::Table::text_rows`)。
///
/// 見出しの行を持つ表は、題を名前にした構造化参照が使えます。
/// `.売上台帳` と見出し `金額` があれば `=SUM(売上台帳[金額])` が通ります。
pub struct CellsGrid {
    name: String,
    /// 行ごとの値(行優先)。行の長さは揃っていなくて構いません
    rows: Vec<Vec<Value>>,
    /// 構造化参照のための表の定義。`tables` が返すので持っておきます
    defs: Vec<TableDef>,
}

impl CellsGrid {
    /// 字の並びから作る。
    ///
    /// 数と真偽は**ここで見分けます** — 表のセルは全部字なので、
    /// 形の上では区別が付きません(SEKKEI「失う物」)。
    /// `1,234` のような桁区切りは字のままにします。読み違えて
    /// 勝手に数にするより、字のほうが安全だからです。
    pub fn from_text_rows(name: &str, rows: Vec<Vec<String>>, header_row: bool) -> CellsGrid {
        let vals: Vec<Vec<Value>> = rows.iter().map(|r| r.iter().map(|s| text_to_value(s)).collect()).collect();
        let mut defs = Vec::new();
        // 題が付いていて中身のある表だけが、構造化参照で引ける
        let cols = vals.iter().map(|r| r.len()).max().unwrap_or(0);
        if !name.is_empty() && cols > 0 && !vals.is_empty() {
            defs.push(TableDef {
                name: name.to_string(),
                a: Pos::new(0, 0),
                b: Pos::new(vals.len() as u32 - 1, cols as u32 - 1),
                header: header_row,
                ..Default::default()
            });
        }
        CellsGrid { name: name.to_string(), rows: vals, defs }
    }
}

/// 字を値にする。数に読めれば数、`TRUE`/`FALSE` なら論理値、
/// 空なら空。それ以外は字のまま
fn text_to_value(s: &str) -> Value {
    let t = s.trim();
    if t.is_empty() {
        return Value::Empty;
    }
    match t.to_ascii_uppercase().as_str() {
        "TRUE" => return Value::Bool(true),
        "FALSE" => return Value::Bool(false),
        _ => {}
    }
    match t.parse::<f64>() {
        Ok(n) if n.is_finite() => Value::Number(n),
        _ => Value::Text(s.to_string()),
    }
}

impl Grid for CellsGrid {
    fn name(&self) -> &str {
        &self.name
    }
    fn value(&self, p: Pos) -> Value {
        self.rows
            .get(p.row as usize)
            .and_then(|r| r.get(p.col as usize))
            .cloned()
            .unwrap_or(Value::Empty)
    }
    fn tables(&self) -> &[TableDef] {
        &self.defs
    }
}

/// 値を引ける表。
///
/// 名前と値の 2 つだけが必須です。残りは持っていなければ既定のまま
/// (隠した行は無い・構造化参照の表は無い・ふりがなは無い)で構いません。
pub trait Grid {
    /// 表の名前。`別表!A1` の照合と `CELL("filename")` が使います。
    fn name(&self) -> &str;

    /// 升目の値。中身の無い所は [`Value::Empty`] を返します。
    fn value(&self, p: Pos) -> Value;

    /// 手で隠した行か。`SUBTOTAL`/`AGGREGATE` の 101〜111 が飛ばします。
    fn row_hidden(&self, _row: u32) -> bool {
        false
    }

    /// 隠した行が1つでもあるか。`SUBTOTAL` は、隠した行が無ければ
    /// 読み直しをしません(その確認だけに使います)。
    fn any_row_hidden(&self) -> bool {
        false
    }

    /// 構造化参照(`売上台帳[金額]`)が引く表の定義。
    fn tables(&self) -> &[TableDef] {
        &[]
    }

    /// ふりがな。`PHONETIC` が引きます。
    fn phonetic(&self, _p: Pos) -> Option<&str> {
        None
    }
}

impl Grid for Sheet {
    fn name(&self) -> &str {
        &self.name
    }
    fn value(&self, p: Pos) -> Value {
        Sheet::value(self, p)
    }
    fn row_hidden(&self, row: u32) -> bool {
        self.row_hidden.contains(&row)
    }
    fn any_row_hidden(&self) -> bool {
        !self.row_hidden.is_empty()
    }
    fn tables(&self) -> &[TableDef] {
        &self.tables
    }
    fn phonetic(&self, p: Pos) -> Option<&str> {
        self.phonetics.get(&p).map(|s| s.as_str())
    }
}
