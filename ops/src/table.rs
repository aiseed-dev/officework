//! **文書の中の表で、セル関数を使う**(SEKKEI「エンジンの統一」3段目)。
//!
//! writer の表のセルに `=SUM(B2:B4)` と書くと、その答えが出ます。
//! 計算するのは calc と同じエンジン(`sheet::calc`)です。**式の言葉は
//! 表計算と文章で同じ**で、覚え直す物はありません。
//!
//! *式は消しません。* セルに残るのはあくまで `=SUM(…)` の字で、答えは
//! 見せるときに作ります。だから `.adoc` に保存しても式のまま残り、
//! 開き直せばまた計算されます。**元の字が正本**です。
//!
//! *計算は calc と同じ道を通します。* 文書の表をいったんシートに写して
//! `sheet::recalc` に渡し、答えを読み戻します。**式の順番の解決も、
//! 循環参照の検出も、書き直しません** — 別に書けば、同じ式が calc と
//! writer で違う答えを出す形になります。
//!
//! ここに置いたのは、`kumihan`(文書)と `sheet`(計算)の両方を知って
//! いるのが `ops` だけだからです。2つのクレートは互いを知りません。

use kumihan::Table;
use sheet::model::TableDef;
use sheet::{recalc, Cell, Pos, Sheet, Value};

/// 表の中の式を計算して、**値の並び**を返す(行優先)。
///
/// 式でないセルは、字がそのまま値になります(数に読めれば数)。
/// 並びの形は [`Table::text_rows`] と同じなので、行と列で引けます。
pub fn values(t: &Table) -> Vec<Vec<Value>> {
    let text = t.text_rows();
    let sheet = to_sheet(t, &text);
    text.iter()
        .enumerate()
        .map(|(r, row)| (0..row.len()).map(|c| sheet.value(Pos::new(r as u32, c as u32))).collect())
        .collect()
}

/// 表の中の式を計算して、**見せる字**を返す(行優先)。
/// 画面・HTML・紙は、これを表の中身として描きます。
pub fn display(t: &Table) -> Vec<Vec<String>> {
    values(t).iter().map(|row| row.iter().map(|v| v.display()).collect()).collect()
}

/// 文書の表を、計算のためのシートに写す。
///
/// 題が付いていれば**表の名前**にもするので、`=SUM(売上台帳[金額])` の
/// ような構造化参照が使えます。範囲は表全体で、見出しの行があるかは
/// `header_row` がそのまま伝わります。
fn to_sheet(t: &Table, text: &[Vec<String>]) -> Sheet {
    let name = t.title.clone().unwrap_or_default();
    let mut s = Sheet::new(&name);
    let mut cols = 0usize;
    for (r, row) in text.iter().enumerate() {
        cols = cols.max(row.len());
        for (c, cell) in row.iter().enumerate() {
            // 空のセルは置かない(シートは中身の無いセルを持たない主義)
            if !cell.trim().is_empty() {
                s.set(Pos::new(r as u32, c as u32), Cell::input(cell));
            }
        }
    }
    if !name.is_empty() && cols > 0 && !text.is_empty() {
        s.tables.push(TableDef {
            name,
            a: Pos::new(0, 0),
            b: Pos::new(text.len() as u32 - 1, cols as u32 - 1),
            header: t.header_row,
            ..Default::default()
        });
    }
    recalc(&mut s);
    s
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use kumihan::{Cellbox, Document, Table};

    /// 字の並びから表を作る(試験の下ごしらえ)
    fn 表(title: &str, header: bool, rows: &[&[&str]]) -> Table {
        Table {
            rows: rows
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|s| Cellbox {
                            paragraphs: Document::plain(s).paragraphs().cloned().collect(),
                            ..Default::default()
                        })
                        .collect()
                })
                .collect(),
            title: (!title.is_empty()).then(|| title.to_string()),
            header_row: header,
            ..Default::default()
        }
    }

    /// 番地の参照。見出しが1行目なので金額は B2:B3
    #[test]
    fn 番地の参照が文書の表で動く() {
        let t = 表("表", true, &[&["品名", "金額"], &["机", "1200"], &["椅子", "800"], &["計", "=SUM(B2:B3)"]]);
        assert_eq!(display(&t)[3][1], "2000");
    }

    /// **構造化参照。** 表の題が名前になり、見出しの字で列を引く。
    /// ここでは合計を表の外(別の列)に置く — Excel と同じで、
    /// 列の中で自分の列を合計すると循環参照になる
    #[test]
    fn 構造化参照が文書の表で動く() {
        let t = 表(
            "売上台帳",
            true,
            &[&["品名", "金額", "全体"], &["机", "1200", "=SUM(売上台帳[金額])"], &["椅子", "800", ""]],
        );
        assert_eq!(display(&t)[1][2], "2000");
    }

    /// **この行だけを指す構造化参照**(`[@列]`)。単価×数量の型
    #[test]
    fn この行の構造化参照が動く() {
        let t = 表(
            "明細",
            true,
            &[&["単価", "数量", "金額"], &["100", "3", "=明細[@単価]*明細[@数量]"]],
        );
        assert_eq!(display(&t)[1][2], "300");
    }

    /// **式が式を指す。** 順番によらず解ける(依存の解決はエンジン任せ)
    #[test]
    fn 式が式を指しても解ける() {
        let t = 表("表", false, &[&["1"], &["=A3*2"], &["=A1+9"]]);
        let d = display(&t);
        assert_eq!(d[2][0], "10"); // A3 = A1+9
        assert_eq!(d[1][0], "20"); // A2 = A3*2
    }

    /// 輪になっていたら **#CIRC!**。黙って 0 を返さない
    #[test]
    fn 循環参照は印になる() {
        let t = 表("表", false, &[&["=A2"], &["=A1"]]);
        let d = display(&t);
        assert_eq!(d[0][0], "#CIRC!");
        assert_eq!(d[1][0], "#CIRC!");
    }

    /// 式の無い表は、字がそのまま出る
    #[test]
    fn 式が無ければ字のまま() {
        let t = 表("", false, &[&["あ", "1200"], &["い", "1,200"]]);
        let d = display(&t);
        assert_eq!(d[0], vec!["あ", "1200"]);
        // 桁区切りは字のまま(数と読み違えない)
        assert_eq!(d[1], vec!["い", "1,200"]);
    }

    /// **結合したセルで列がずれない。** 左上に字を置き、残りは空
    #[test]
    fn 結合しても列がずれない() {
        let mut t = 表("表", false, &[&["見出し", ""], &["10", "20"], &["", "=SUM(A2:B2)"]]);
        t.rows[0][0].col_span = 2;
        t.rows[0].remove(1); // 結合したので格子の欄は1つ
        let d = display(&t);
        assert_eq!(d[0], vec!["見出し", ""], "結合の右は空で埋まる");
        assert_eq!(d[2][1], "30");
    }
}
