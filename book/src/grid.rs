//! 値を引ける表。**式の計算が表に求めるのは、この面だけです。**
//!
//! いままで `kumihan::calc` は `Sheet` を直に触っていました。けれども実際に
//! 見ていたのは 5 つの物だけで、`Sheet` の残り(書式・列幅・図形・
//! ピボット…)は式の計算に一度も出てきません。数えたところ、関数の
//! 2,338 行は `Sheet` を1度も見ていません。
//!
//! そこで「値を引ける表」を [`Grid`] という名前で決めて、`Sheet` は
//! その1つ、という形にしました。式の計算は `Sheet` に縛られません。
//!
//! *いま実装しているのは `Sheet` だけです。* 文書の中の表
//! (`kumihan::Table`)の計算は、`ops::table` が表をシートに写して行います。
//! 式の順番の解決も循環参照の検出もエンジンに1本で持たせるためで、
//! **同じ式が calc と writer で違う答えを出す形を作らない**という判断です。
//!
//! 設計は SEKKEI.adoc「エンジンの統一 — 表を1つにする」の 2 段目です。

use crate::{Pos, Sheet, TableDef, Value};

/// 値を引ける表。
///
/// 名前と値の 2 つだけが必須です。残りは持っていなければ既定のまま
/// (隠した行は無い・構造化参照の表は無い・ふりがなは無い)で構いません。
pub trait Grid {
    /// 表の名前。`別表!A1` の照合と `CELL("filename")` が使います。
    fn name(&self) -> &str;

    /// セルの値。中身の無い所は [`Value::Empty`] を返します。
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
