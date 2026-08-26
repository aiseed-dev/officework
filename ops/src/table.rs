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
use sheet::{recalc_book, Book, Cell, Pos, Sheet, Value};

/// 表の中の式を計算して、**値の並び**を返す(行優先)。
///
/// 式でないセルは、字がそのまま値になります(数に読めれば数)。
/// 並びの形は [`Table::text_rows`] と同じなので、行と列で引けます。
pub fn values(t: &Table) -> Vec<Vec<Value>> {
    values_with(t, None)
}

/// 表の中の式を計算する。**反復計算の設定つき**(2026-08-20 発注者
/// 「双方でできるようにしたいです」)。
///
/// `iter` は (最大の回数, 変化量) — `None` なら反復しません。表の計算は
/// `recalc_book` を通ります。**反復を読むのは Book の側**なので、
/// 裸のシートに `recalc` を掛けていたときは設定が効きませんでした。
///
/// *文書の表の反復は「アプリの設定」です。* ブックは xlsx の `calcPr` に
/// 持って往復しますが、`.adoc` の文書にはその置き場がありません。
/// 循環参照は**表の中で閉じている**ので、文書ごとに変える意味も薄いと見ました。
pub fn values_with(t: &Table, iter: Option<(u32, f64)>) -> Vec<Vec<Value>> {
    let text = t.text_rows();
    let sheet = to_sheet_with(t, &text, iter);
    text.iter()
        .enumerate()
        .map(|(r, row)| (0..row.len()).map(|c| sheet.value(Pos::new(r as u32, c as u32))).collect())
        .collect()
}

/// 表の中の式を計算して、**見せる字**を返す(行優先)。
/// 画面・HTML・紙は、これを表の中身として描きます。
pub fn display(t: &Table) -> Vec<Vec<String>> {
    display_with(t, None)
}

/// 見せる字(反復計算の設定つき)。
pub fn display_with(t: &Table, iter: Option<(u32, f64)>) -> Vec<Vec<String>> {
    values_with(t, iter).iter().map(|row| row.iter().map(|v| v.display()).collect()).collect()
}

/// 文書の中に**式の入った表**があるか。
///
/// 写しを作る前の見極めに使います(式が無ければ写しも計算も要りません)。
pub fn has_formula(doc: &kumihan::Document) -> bool {
    doc.blocks.iter().any(|b| match b {
        kumihan::Block::Table(t) => t
            .rows
            .iter()
            .flatten()
            .any(|c| kumihan::adoc::is_formula_cell(&kumihan::paras_text(&c.paragraphs))),
        _ => false,
    })
}

/// 文書の中の表の式を計算して、**見せる字に置き換える**。返すのは直した升の数。
///
/// **写しの上で呼んでください。** 元の文書は式のまま残します — 式が正本で、
/// 答えは見せるときに作る、が決めです(SEKKEI「エンジンの統一」3段目)。
///
/// 式でない升は触りません。太字などの書式を持った升を、答えの字で
/// 塗り潰さないためです。
pub fn fill(doc: &mut kumihan::Document) -> usize {
    fill_with(doc, None)
}

/// 写しに答えを入れる(反復計算の設定つき)。
pub fn fill_with(doc: &mut kumihan::Document, iter: Option<(u32, f64)>) -> usize {
    let mut fixed = 0;
    for b in doc.blocks.iter_mut() {
        let kumihan::Block::Table(t) = b else { continue };
        let value = values_with(t, iter);
        for (r, row) in t.rows.iter_mut().enumerate() {
            // 格子の桁。結合した升はそのぶん進みます
            let mut c = 0usize;
            for cell in row.iter_mut() {
                let widths = cell.span();
                if kumihan::adoc::is_formula_cell(&kumihan::paras_text(&cell.paragraphs)) {
                    if let Some(v) = value.get(r).and_then(|x| x.get(c)) {
                        kumihan::set_paras_text(&mut cell.paragraphs, &v.display());
                        fixed += 1;
                    }
                }
                c += widths;
            }
        }
    }
    fixed
}

/// 文書の表を、計算のためのシートに写す。
///
/// 題が付いていれば**表の名前**にもするので、`=SUM(売上台帳[金額])` の
/// ような構造化参照が使えます。範囲は表全体で、見出しの行があるかは
/// `header_row` がそのまま伝わります。
fn to_sheet_with(t: &Table, text: &[Vec<String>], iter: Option<(u32, f64)>) -> Sheet {
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
    // **Book の道を通す。** 反復計算の設定を読むのは Book の側で、
    // 裸のシートに `recalc` を掛けると設定が落ちます(2026-08-20)
    let mut b = Book::new();
    b.calc_iter = iter;
    b.sheets[0] = s;
    recalc_book(&mut b, 0);
    b.sheets.remove(0)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use kumihan::{Cellbox, Document, Table};

    /// **反復計算が文書の表でも効く**(2026-08-20 発注者「双方でできるように
    /// したいです」)。
    ///
    /// 前は裸のシートに `recalc` を掛けていて、**反復の設定を読むのは Book の
    /// 側**なので効きませんでした。表計算では設定できるのに文書の表ではできない、
    /// という食い違いでした。
    #[test]
    fn 反復計算は設定したときだけ効く() {
        // A1 = B1 + 1、B1 = A1 — そのままでは循環参照
        let t = table("", false, &[&["=B1+1", "=A1"]]);
        // 設定なし: 循環参照の印が出る(いままでどおり)
        let plain = display(&t);
        assert!(plain[0][0].contains('#'), "循環参照の印が出ていない: {plain:?}");
        // 反復あり: 落ち着いた値になる
        let repeat = display_with(&t, Some((100, 1e-9)));
        assert!(!repeat[0][0].contains('#'), "反復しても印のまま: {repeat:?}");
    }

    /// **写しに答えを入れる道でも反復が効く。**
    ///
    /// `display_with` だけ試験して安心していたら、`fill_with` が受け取った
    /// 設定を捨てて `values` を呼んでいました(2026-08-20 に見つけた)。
    /// 保存する docx はこちらの道を通るので、**画面では落ち着いた値なのに
    /// 配ったファイルは循環参照の印**という食い違いが出ていました
    #[test]
    fn 写しに入れる道でも反復が効く() {
        let mut d = kumihan::Document::default();
        d.blocks.push(kumihan::Block::Table(table("", false, &[&["=B1+1", "=A1"]])));
        let mut repeat = d.clone();
        fill_with(&mut repeat, Some((100, 1e-9)));
        let kumihan::Block::Table(t) = &repeat.blocks[0] else { panic!() };
        let out = kumihan::paras_text(&t.rows[0][0].paragraphs);
        assert!(!out.contains('#'), "写しの道に反復が効いていない: {out:?}");
        // 設定なしはいままでどおり印
        let mut plain = d.clone();
        fill_with(&mut plain, None);
        let kumihan::Block::Table(t) = &plain.blocks[0] else { panic!() };
        assert!(kumihan::paras_text(&t.rows[0][0].paragraphs).contains('#'), "既定が変わった");
    }

    /// **設定を渡さない道は今までどおり。** 既定で反復しないのは
    /// Excel と同じで、循環参照は黙って値にせず印で言う
    #[test]
    fn 既定は反復しない() {
        let t = table("", false, &[&["=B1+1", "=A1"]]);
        assert_eq!(display(&t), display_with(&t, None), "既定が変わった");
    }

    /// 字の並びから表を作る(試験の下ごしらえ)
    fn table(title: &str, header: bool, rows: &[&[&str]]) -> Table {
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
        let t = table("表", true, &[&["品名", "金額"], &["机", "1200"], &["椅子", "800"], &["計", "=SUM(B2:B3)"]]);
        assert_eq!(display(&t)[3][1], "2000");
    }

    /// **構造化参照。** 表の題が名前になり、見出しの字で列を引く。
    /// ここでは合計を表の外(別の列)に置く — Excel と同じで、
    /// 列の中で自分の列を合計すると循環参照になる
    #[test]
    fn 構造化参照が文書の表で動く() {
        let t = table(
            "売上台帳",
            true,
            &[&["品名", "金額", "全体"], &["机", "1200", "=SUM(売上台帳[金額])"], &["椅子", "800", ""]],
        );
        assert_eq!(display(&t)[1][2], "2000");
    }

    /// **この行だけを指す構造化参照**(`[@列]`)。単価×数量の型
    #[test]
    fn この行の構造化参照が動く() {
        let t = table(
            "明細",
            true,
            &[&["単価", "数量", "金額"], &["100", "3", "=明細[@単価]*明細[@数量]"]],
        );
        assert_eq!(display(&t)[1][2], "300");
    }

    /// **式が式を指す。** 順番によらず解ける(依存の解決はエンジン任せ)
    #[test]
    fn 式が式を指しても解ける() {
        let t = table("表", false, &[&["1"], &["=A3*2"], &["=A1+9"]]);
        let d = display(&t);
        assert_eq!(d[2][0], "10"); // A3 = A1+9
        assert_eq!(d[1][0], "20"); // A2 = A3*2
    }

    /// 輪になっていたら **#CIRC!**。黙って 0 を返さない
    #[test]
    fn 循環参照は印になる() {
        let t = table("表", false, &[&["=A2"], &["=A1"]]);
        let d = display(&t);
        assert_eq!(d[0][0], "#CIRC!");
        assert_eq!(d[1][0], "#CIRC!");
    }

    /// 式の無い表は、字がそのまま出る
    #[test]
    fn 式が無ければ字のまま() {
        let t = table("", false, &[&["あ", "1200"], &["い", "1,200"]]);
        let d = display(&t);
        assert_eq!(d[0], vec!["あ", "1200"]);
        // 桁区切りは字のまま(数と読み違えない)
        assert_eq!(d[1], vec!["い", "1,200"]);
    }

    /// **結合したセルで列がずれない。** 左上に字を置き、残りは空
    #[test]
    fn 結合しても列がずれない() {
        let mut t = table("表", false, &[&["見出し", ""], &["10", "20"], &["", "=SUM(A2:B2)"]]);
        t.rows[0][0].col_span = 2;
        t.rows[0].remove(1); // 結合したので格子の欄は1つ
        let d = display(&t);
        assert_eq!(d[0], vec!["見出し", ""], "結合の右は空で埋まる");
        assert_eq!(d[2][1], "30");
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod answer_into_copy {
    use super::*;
    use kumihan::{Block, Cellbox, Document, Table};

    fn doc() -> Document {
        let cell = |s: &str| Cellbox {
            paragraphs: Document::plain(s).paragraphs().cloned().collect(),
            ..Default::default()
        };
        let mut d = Document::plain("本文");
        d.blocks.push(Block::Table(Table {
            rows: vec![
                vec![cell("品名"), cell("金額")],
                vec![cell("机"), cell("1200")],
                vec![cell("椅子"), cell("800")],
                vec![cell("計"), cell("=SUM(B2:B3)")],
            ],
            header_row: true,
            ..Default::default()
        }));
        d
    }

    #[test]
    fn 式のある表を見つける() {
        assert!(has_formula(&doc()));
        assert!(!has_formula(&Document::plain("式の無い文書")));
    }

    /// **式の升だけ答えの字になる。** ほかの升は触らない
    #[test]
    fn 式の升だけ差し替わる() {
        let mut d = doc();
        assert_eq!(fill(&mut d), 1, "直した升の数が合わない");
        let t = d.blocks.iter().find_map(|b| if let Block::Table(t) = b { Some(t) } else { None }).unwrap();
        assert_eq!(kumihan::paras_text(&t.rows[3][1].paragraphs), "2000");
        // 式でない升はそのまま
        assert_eq!(kumihan::paras_text(&t.rows[1][1].paragraphs), "1200");
        assert_eq!(kumihan::paras_text(&t.rows[0][0].paragraphs), "品名");
    }

    /// **元の文書は式のまま。** 差し替えるのは写しだけ
    #[test]
    fn 元は式のまま() {
        let from = doc();
        let mut copy = from.clone();
        fill(&mut copy);
        let t = from.blocks.iter().find_map(|b| if let Block::Table(t) = b { Some(t) } else { None }).unwrap();
        assert_eq!(kumihan::paras_text(&t.rows[3][1].paragraphs), "=SUM(B2:B3)", "元まで書き替えた");
    }
}
