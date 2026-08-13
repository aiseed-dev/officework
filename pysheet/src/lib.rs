//! office_sheet — `sheet` crate の Python 束縛。
//!
//! 分業の設計(SEKKEI.md): **データを作る・分析する仕事は Python、
//! 見ながら整える仕事は calc。** その橋がこれ。
//! polars で集計した結果を、**帳票の枠(罫線・結合・列幅・図形)を
//! 保ったまま**実物の様式 xlsx に差し込んで保存できる。
//! openpyxl との違いはそこ — 開いて保存しただけで様式が崩れない。
//!
//! マクロの置き換えでもある: 表の中に実行コードを埋める(VBA)代わりに、
//! 表の外のスクリプトが表を扱う。「開く=実行」という攻撃経路を持たない。
//!
//!     import office_sheet
//!     b = office_sheet.Book.open("様式7.xlsx")
//!     s = b["提案見積書"]
//!     s["A30"] = "日本フネン株式会社"
//!     s["C30"] = "=B30*100"        # 文字列は「打ったのと同じ」解釈
//!     b.save("out.xlsx")           # 罫線・結合・列幅・図形は元のまま

mod doc;

use std::sync::{Arc, Mutex, MutexGuard};

use pyo3::exceptions::{PyIOError, PyIndexError, PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDateTime, PyDict, PyTime};

use sheet::calc::{date_serial_at, excel_epoch};
use sheet::model::{
    format_value, rename_sheet_refs, BStyle, Edge, FreezePane, HAlign, SheetImage, VAlign,
};
use sheet::{recalc_all, recalc_book, xlsx, Cell, Pos, Value};

/// ブックの中身。Book と Sheet が同じものを見るために1枚挟む。
struct Inner {
    book: sheet::Book,
    /// 開いた元のファイル。保存時に、こちらが作り直さない部品
    /// (図形・テーマ・印刷設定)を持ち越すために取っておく
    original: Option<Vec<u8>>,
    /// 読めなかったものの帳簿。黙って落とさない(ooxml と同じ作法)
    unsupported: Vec<(String, usize)>,
}

fn lock(inner: &Arc<Mutex<Inner>>) -> PyResult<MutexGuard<'_, Inner>> {
    inner
        .lock()
        .map_err(|_| PyValueError::new_err("別の操作が失敗した後で、ブックの状態が信用できない"))
}

fn parse_ref(s: &str) -> PyResult<Pos> {
    Pos::parse(s).ok_or_else(|| PyValueError::new_err(format!("セル参照として読めない: {s:?}")))
}

/// "A1:B2"(か "A1")→(左上, 右下)。向きが逆でも直す。
fn parse_range(s: &str) -> PyResult<(Pos, Pos)> {
    let (a, b) = match s.split_once(':') {
        Some((l, r)) => (parse_ref(l.trim())?, parse_ref(r.trim())?),
        None => {
            let p = parse_ref(s.trim())?;
            (p, p)
        }
    };
    Ok((
        Pos::new(a.row.min(b.row), a.col.min(b.col)),
        Pos::new(a.row.max(b.row), a.col.max(b.col)),
    ))
}

/// xlsx のシート名の決まり(アプリの改名と同じ検査):
/// 空は不可・31字まで・`: \ / ? * [ ]` は不可・同じ名前は不可。
fn check_sheet_name(book: &sheet::Book, name: &str) -> PyResult<()> {
    if name.is_empty() {
        return Err(PyValueError::new_err("シート名が空です"));
    }
    if name.chars().count() > 31 || name.contains([':', '\\', '/', '?', '*', '[', ']']) {
        return Err(PyValueError::new_err(format!(
            "「{name}」はシート名にできません(31字まで。: \\ / ? * [ ] は不可)"
        )));
    }
    if book.sheets.iter().any(|s| s.name == name) {
        return Err(PyValueError::new_err(format!("同じ名前のシートがある: {name:?}")));
    }
    Ok(())
}

/// Python へ返す値。Empty は None(Option 側)で返す。
/// エラー値(#DIV/0! など)は文字列で返す — 表計算の作法どおり。
#[derive(IntoPyObject)]
enum Out {
    Num(f64),
    Text(String),
    Bool(bool),
}

fn to_out(v: &Value) -> Option<Out> {
    match v {
        Value::Empty => None,
        Value::Number(n) => Some(Out::Num(*n)),
        Value::Text(s) => Some(Out::Text(s.clone())),
        Value::Bool(b) => Some(Out::Bool(*b)),
        Value::Error(e) => Some(Out::Text(e.clone())),
    }
}

/// 数の入ったセル(書式は呼び側で付け直す)。
fn num_cell(n: f64) -> Cell {
    Cell { formula: None, value: Value::Number(n), fmt: Default::default() }
}

/// date/datetime の年月日 → Excel の通し番号(日の部分)。
/// abi3 では C の accessor が使えないので、属性(year/month/day)で読む
fn date_days(v: &Bound<'_, PyAny>, date1904: bool) -> PyResult<i64> {
    let g = |n: &str| -> PyResult<i64> { v.getattr(n)?.extract() };
    Ok(date_serial_at(g("year")?, g("month")?, g("day")?, excel_epoch(date1904)))
}

/// time/datetime の時刻 → 日の割合(0.0〜)。
fn time_frac(v: &Bound<'_, PyAny>) -> PyResult<f64> {
    let g = |n: &str| -> PyResult<i64> { v.getattr(n)?.extract() };
    Ok((g("hour")? as f64 * 3600.0
        + g("minute")? as f64 * 60.0
        + g("second")? as f64
        + g("microsecond")? as f64 / 1e6)
        / 86400.0)
}

/// xlsx のブック。
#[pyclass(name = "Book")]
struct PyBook {
    inner: Arc<Mutex<Inner>>,
}

#[pymethods]
impl PyBook {
    /// 空のブック(Sheet1 が1枚)。
    #[new]
    fn new() -> PyBook {
        PyBook {
            inner: Arc::new(Mutex::new(Inner {
                book: sheet::Book::new(),
                original: None,
                unsupported: Vec::new(),
            })),
        }
    }

    /// xlsx を開く。式は開いた時点で再計算される。
    #[staticmethod]
    fn open(path: &str) -> PyResult<PyBook> {
        let bytes = std::fs::read(path)
            .map_err(|e| PyIOError::new_err(format!("{path}: 読めない: {e}")))?;
        let (mut book, rep) = xlsx::read(std::io::Cursor::new(&bytes))
            .map_err(|e| PyIOError::new_err(format!("{path}: xlsx として読めない: {e}")))?;
        recalc_all(&mut book);
        Ok(PyBook {
            inner: Arc::new(Mutex::new(Inner {
                book,
                original: Some(bytes),
                unsupported: rep.unsupported,
            })),
        })
    }

    /// 保存する。開いた元のファイルがあれば、こちらが作り直さない部品
    /// (図形・テーマ・印刷設定・文書情報)を原本から持ち越す。
    fn save(&self, path: &str) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        recalc_all(&mut g.book);
        let mut buf = std::io::Cursor::new(Vec::new());
        let r = match &g.original {
            Some(bytes) => xlsx::write_with(&g.book, Some(std::io::Cursor::new(bytes)), &mut buf),
            None => xlsx::write(&g.book, &mut buf),
        };
        r.map_err(|e| PyIOError::new_err(format!("{path}: 書けない: {e}")))?;
        std::fs::write(path, buf.into_inner())
            .map_err(|e| PyIOError::new_err(format!("{path}: 書けない: {e}")))
    }

    /// シートの名前の一覧。
    #[getter]
    fn sheet_names(&self) -> PyResult<Vec<String>> {
        Ok(lock(&self.inner)?.book.sheets.iter().map(|s| s.name.clone()).collect())
    }

    /// 読めなかったものの帳簿 [(名前, 件数)]。空なら取りこぼしなし。
    /// **黙って落とさない** — 開いた様式に読めないものがあれば、ここに出る。
    #[getter]
    fn unsupported(&self) -> PyResult<Vec<(String, usize)>> {
        Ok(lock(&self.inner)?.unsupported.clone())
    }

    /// 名前か番号(0起点)でシートを取る。`book["提案見積書"]` / `book[0]`。
    fn __getitem__(&self, key: &Bound<'_, PyAny>) -> PyResult<PySheet> {
        let g = lock(&self.inner)?;
        let idx = if let Ok(i) = key.extract::<usize>() {
            if i >= g.book.sheets.len() {
                return Err(PyIndexError::new_err(format!(
                    "シートは {} 枚しかない: {i}",
                    g.book.sheets.len()
                )));
            }
            i
        } else {
            let name = key.extract::<String>()?;
            g.book
                .sheets
                .iter()
                .position(|s| s.name == name)
                .ok_or_else(|| PyKeyError::new_err(format!("シートが無い: {name:?}")))?
        };
        Ok(PySheet { inner: Arc::clone(&self.inner), idx })
    }

    fn __len__(&self) -> PyResult<usize> {
        Ok(lock(&self.inner)?.book.sheets.len())
    }

    /// シートを1枚足す。同じ名前・xlsx で使えない名前はエラー。
    fn add_sheet(&self, name: &str) -> PyResult<PySheet> {
        let mut g = lock(&self.inner)?;
        check_sheet_name(&g.book, name)?;
        g.book.sheets.push(sheet::Sheet::new(name));
        let idx = g.book.sheets.len() - 1;
        Ok(PySheet { inner: Arc::clone(&self.inner), idx })
    }

    /// シートを丸ごと写して末尾に足す(中身・書式・結合・列幅・入力規則まで)。
    fn copy_sheet(&self, name: &str, new_name: &str) -> PyResult<PySheet> {
        let mut g = lock(&self.inner)?;
        check_sheet_name(&g.book, new_name)?;
        let src = g
            .book
            .sheets
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| PyKeyError::new_err(format!("シートが無い: {name:?}")))?;
        let mut copy = src.clone();
        copy.name = new_name.to_string();
        g.book.sheets.push(copy);
        let idx = g.book.sheets.len() - 1;
        Ok(PySheet { inner: Arc::clone(&self.inner), idx })
    }

    /// シートを抜く。**最後の1枚は抜けない**(シートの無い xlsx は無い)。
    /// 抜いたシートを指していた式は再計算でエラー値になる(黙って別の
    /// シートを指すより良い)。手元の Sheet の札は**位置**で指しているので、
    /// 抜いた後は book[...] で引き直すこと。
    fn remove_sheet(&self, name: &str) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        if g.book.sheets.len() == 1 {
            return Err(PyValueError::new_err("最後の1枚は抜けません"));
        }
        let idx = g
            .book
            .sheets
            .iter()
            .position(|s| s.name == name)
            .ok_or_else(|| PyKeyError::new_err(format!("シートが無い: {name:?}")))?;
        g.book.sheets.remove(idx);
        recalc_all(&mut g.book);
        Ok(())
    }

    /// シートを並べ替える(`to` は 0 起点の新しい位置)。
    /// 手元の Sheet の札は**位置**で指しているので、並べ替えの後は引き直すこと。
    fn move_sheet(&self, name: &str, to: usize) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        let idx = g
            .book
            .sheets
            .iter()
            .position(|s| s.name == name)
            .ok_or_else(|| PyKeyError::new_err(format!("シートが無い: {name:?}")))?;
        if to >= g.book.sheets.len() {
            return Err(PyIndexError::new_err(format!(
                "シートは {} 枚しかない: {to}",
                g.book.sheets.len()
            )));
        }
        let s = g.book.sheets.remove(idx);
        g.book.sheets.insert(to, s);
        Ok(())
    }

    /// 1904 起点のブックか(workbookPr の date1904)。日付の計算・表示・
    /// datetime の受け渡しは全部この旗の起点で解釈する(2026-08-13)。
    #[getter]
    fn date1904(&self) -> PyResult<bool> {
        Ok(lock(&self.inner)?.book.date1904)
    }

    /// 起点を替える。**通し番号はそのまま**なので、既にある日付の意味が
    /// 4年動く(Excel でこの設定を切り替えたときと同じ)。再計算する。
    #[setter]
    fn set_date1904(&self, value: bool) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        g.book.date1904 = value;
        recalc_all(&mut g.book);
        Ok(())
    }


    /// 名前付きセル様式を**作る**(openpyxl の add_named_style)。
    /// 書式は `set_fmt` と同じ鍵の dict で渡す。保存で styles.xml の
    /// cellStyleXfs / cellStyles に**追記**する(原本の索引は動かさない)。
    #[pyo3(signature = (name, **kw))]
    fn add_named_style(&self, name: &str, kw: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        if name.is_empty() {
            return Err(PyValueError::new_err("様式の名前が空です"));
        }
        let mut g = lock(&self.inner)?;
        if g.book.named_styles.iter().any(|(n, _, _)| n == name)
            || g.book.named_styles_new.iter().any(|(n, _)| n == name)
        {
            return Err(PyValueError::new_err(format!("様式「{name}」は既にあります")));
        }
        let mut f = sheet::model::CellFormat::default();
        if let Some(kw) = kw {
            apply_fmt(&mut f, kw)?;
        }
        g.book.named_styles_new.push((name.to_string(), f));
        Ok(())
    }

    /// 名前付きセル様式の一覧 [(名前, 組み込みの番号)]。
    /// **定義は原本の styles.xml が持ち、保存でそのまま持ち越される** —
    /// ここは名乗りの写し(型紙の見出しの大きさなどを引くのに使う)。
    #[getter]
    fn named_styles(&self) -> PyResult<Vec<(String, Option<u32>)>> {
        let g = lock(&self.inner)?;
        // 原本の様式 + **このアプリで足した分**(保存で styles.xml に入る)
        Ok(g.book
            .named_styles
            .iter()
            .map(|(n, b, _)| (n.clone(), *b))
            .chain(g.book.named_styles_new.iter().map(|(n, _)| (n.clone(), None)))
            .collect())
    }

    /// 名前付き様式の**書式**を dict で引く(Sheet.fmt と同じ鍵)。
    /// 無い名前は KeyError。
    fn named_style_fmt<'py>(
        &self,
        py: Python<'py>,
        name: &str,
    ) -> PyResult<Bound<'py, PyDict>> {
        let g = lock(&self.inner)?;
        let f = g
            .book
            .named_styles
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, _, f)| f.clone())
            .or_else(|| {
                g.book.named_styles_new.iter().find(|(n, _)| n == name).map(|(_, f)| f.clone())
            })
            .ok_or_else(|| PyKeyError::new_err(format!("名前付き様式が無い: {name:?}")))?;
        fmt_dict(py, &f)
    }

    /// 全シートを再計算する(セルを置いた時点でそのシートは再計算済み。
    /// 明示的にやり直したいとき用)。
    fn recalc(&self) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        recalc_all(&mut g.book);
        Ok(())
    }
}

/// 1枚のシート。セルは A1 形式で読み書きする。
#[pyclass(name = "Sheet")]
struct PySheet {
    inner: Arc<Mutex<Inner>>,
    idx: usize,
}

impl PySheet {
    fn with<T>(&self, f: impl FnOnce(&mut sheet::Sheet) -> PyResult<T>) -> PyResult<T> {
        let mut g = lock(&self.inner)?;
        let s = g
            .idx_sheet(self.idx)
            .ok_or_else(|| PyKeyError::new_err("このシートはもうブックに無い"))?;
        f(s)
    }

    /// 書き換えてから、**ブック全体の文脈で**このシートを再計算する
    /// (INDIRECT("別の表!A1") も解ける)
    fn with_calc<T>(&self, f: impl FnOnce(&mut sheet::Sheet) -> PyResult<T>) -> PyResult<T> {
        let mut g = lock(&self.inner)?;
        let s = g
            .idx_sheet(self.idx)
            .ok_or_else(|| PyKeyError::new_err("このシートはもうブックに無い"))?;
        let r = f(s)?;
        recalc_book(&mut g.book, self.idx);
        Ok(r)
    }
}

impl Inner {
    fn idx_sheet(&mut self, idx: usize) -> Option<&mut sheet::Sheet> {
        self.book.sheets.get_mut(idx)
    }
}

#[pymethods]
impl PySheet {
    #[getter]
    fn name(&self) -> PyResult<String> {
        self.with(|s| Ok(s.name.clone()))
    }

    /// 改名。**式の参照(`古い名前!A1`)と名前の定義も追随する** — アプリの
    /// 改名と同じ作法(sheet::model::rename_sheet_refs)。文字列の中
    /// (INDIRECT("古!A1") 等)は書き換えない — あれは data であって参照ではない。
    #[setter]
    fn set_name(&self, value: &str) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        let old = g
            .book
            .sheets
            .get(self.idx)
            .map(|s| s.name.clone())
            .ok_or_else(|| PyKeyError::new_err("このシートはもうブックに無い"))?;
        if value == old {
            return Ok(());
        }
        check_sheet_name(&g.book, value)?;
        rename_sheet_refs(&mut g.book, &old, value);
        g.book.sheets[self.idx].name = value.to_string();
        recalc_book(&mut g.book, self.idx);
        Ok(())
    }

    /// 計算後の値。空なら None、エラー(#DIV/0! 等)は文字列。
    fn __getitem__(&self, key: &str) -> PyResult<Option<Out>> {
        let p = parse_ref(key)?;
        self.with(|s| Ok(to_out(&s.value(p))))
    }

    /// セルに置く。**書式(罫線・結合・表示形式)は据え置き** — それが存在理由。
    ///
    /// - 数・bool はそのまま値になる
    /// - `datetime.date` / `datetime.datetime` / `datetime.time` は Excel の
    ///   通し番号(1899-12-30 起点。DATE 関数と同じ一本道)になる。
    ///   セルに表示形式が無いときだけ日付の形式を付ける(数字の羅列で見せない)。
    ///   帳票の日付セルには元の表示形式が付いているので、それに従う
    /// - 文字列は「calc で打ったのと同じ」解釈: `"=SUM(A1:A3)"` は式、
    ///   `"123"` は数、それ以外は文字
    /// - None は中身を消す(罫線だけのセルは枠として残る)
    ///
    /// 置いたらこのシートは再計算される。
    fn __setitem__(&self, key: &str, value: Option<Bound<'_, PyAny>>) -> PyResult<()> {
        let p = parse_ref(key)?;
        let d1904 = lock(&self.inner)?.book.date1904;
        // (セルの中身, セルに表示形式が無いときに付ける形式)
        let (cell, date_fmt): (Cell, Option<&str>) = match &value {
            None => (Cell::default(), None),
            // datetime は date の子なので、必ず datetime を先に見る
            Some(v) => {
                if v.cast::<PyDateTime>().is_ok() {
                    (num_cell(date_days(v, d1904)? as f64 + time_frac(v)?), Some("yyyy/m/d h:mm"))
                } else if v.cast::<PyDate>().is_ok() {
                    (num_cell(date_days(v, d1904)? as f64), Some("yyyy/m/d"))
                } else if v.cast::<PyTime>().is_ok() {
                    (num_cell(time_frac(v)?), Some("h:mm"))
                } else if let Ok(b) = v.extract::<bool>() {
                    // bool を数より先に見る(Python の bool は int の子)
                    (Cell { formula: None, value: Value::Bool(b), fmt: Default::default() }, None)
                } else if let Ok(n) = v.extract::<f64>() {
                    (num_cell(n), None)
                } else if let Ok(t) = v.extract::<String>() {
                    // **日付を返す式には日付の形式を薦める**(元の形式が無いときだけ)。
                    // Python の date を置いたときと同じ作法 — 打った字が
                    // `"=TODAY()"` でも `date.today()` でも画面は同じであるべき
                    let c = Cell::input(&t);
                    let df = c.formula.as_deref().and_then(Cell::date_format_of);
                    (c, df)
                } else {
                    return Err(PyTypeError::new_err(format!(
                        "セルに置けるのは 数・bool・文字列・datetime/date/time・None。渡されたのは {}",
                        v.get_type().name().map(|n| n.to_string()).unwrap_or_default()
                    )));
                }
            }
        };
        self.with_calc(|s| {
            let mut fmt = s.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
            if fmt.number_format.is_none() {
                if let Some(df) = date_fmt {
                    fmt.number_format = Some(df.into());
                }
            }
            let mut cell = cell;
            cell.fmt = fmt;
            s.set(p, cell);
            Ok(())
        })
    }

    /// セルの式("=SUM(A1:A3)" の形)。式が無ければ None。
    fn formula(&self, key: &str) -> PyResult<Option<String>> {
        let p = parse_ref(key)?;
        self.with(|s| Ok(s.get(p).and_then(|c| c.formula.as_ref().map(|f| format!("={f}")))))
    }

    /// 表示形式(#,##0 など)を当てた、画面に出るのと同じ文字列。
    fn display(&self, key: &str) -> PyResult<String> {
        let p = parse_ref(key)?;
        let mut g = lock(&self.inner)?;
        let d1904 = g.book.date1904;
        let s = g
            .idx_sheet(self.idx)
            .ok_or_else(|| PyIndexError::new_err("このシートはもうブックに無い"))?;
        Ok(match s.get(p) {
            Some(c) => format_value(&c.value, c.fmt.number_format.as_deref(), d1904),
            None => String::new(),
        })
    }

    /// 使われている範囲 (行数, 列数)。DataFrame の shape と同じ向き。
    #[getter]
    fn shape(&self) -> PyResult<(u32, u32)> {
        self.with(|s| Ok(s.extent()))
    }

    /// 使われている範囲の値を list[list](行ごと)で。
    /// `polars.DataFrame(s.values(), orient="row")` でそのまま表になる。
    fn values(&self) -> PyResult<Vec<Vec<Option<Out>>>> {
        self.with(|s| {
            let (rows, cols) = s.extent();
            Ok((0..rows)
                .map(|r| {
                    (0..cols).map(|c| to_out(&s.value(Pos { row: r, col: c }))).collect()
                })
                .collect())
        })
    }

    /// セル結合の一覧 [("A1", "B2"), …](左上, 右下)。帳票の枠組みが見える。
    #[getter]
    fn merges(&self) -> PyResult<Vec<(String, String)>> {
        self.with(|s| Ok(s.merges.iter().map(|(a, b)| (a.a1(), b.a1())).collect()))
    }

    /// セルを結合する("A1:B2")。アプリの「結合だけ」と同じ家の作法
    /// (sheet::model::ops の Sheet::merge): 重なる結合は先に外れ、左上が
    /// 空なら読み順で最初の中身が**書式ごと**左上へ移り、左上以外の中身は
    /// 消える(書式は残る)— 残すと見えない値が SUM に効いて帳票が嘘をつく。
    /// 揃えは触らない。
    fn merge_cells(&self, range: &str) -> PyResult<()> {
        let (a, b) = parse_range(range)?;
        if a == b {
            return Err(PyValueError::new_err(format!("1マスは結合できない: {range:?}")));
        }
        self.with_calc(|s| {
            s.merge(a, b);
            Ok(())
        })
    }

    /// 範囲に**掛かる**結合を解く(アプリの「解除」と同じ)。返りは解いた数。
    /// 中身は戻らない(結合のときに消している)。
    fn unmerge_cells(&self, range: &str) -> PyResult<usize> {
        let (a, b) = parse_range(range)?;
        self.with(|s| Ok(s.unmerge(a, b)))
    }

    /// 画像(PNG / JPEG)をシートに浮かべる。左上を `at` のセルに留める
    /// (xlsx の oneCellAnchor)。アプリの「挿入 > グラフ」と同じ道 —
    /// matplotlib で描いた PNG の径路か bytes をそのまま渡せる。
    /// 大きさは絵から測る(width_px / height_px で上書きできる)。
    #[pyo3(signature = (image, at="A1", width_px=None, height_px=None))]
    fn add_image(
        &self,
        image: &Bound<'_, PyAny>,
        at: &str,
        width_px: Option<f32>,
        height_px: Option<f32>,
    ) -> PyResult<()> {
        let data: Vec<u8> = if let Ok(b) = image.extract::<Vec<u8>>() {
            b
        } else if let Ok(p) = image.extract::<String>() {
            std::fs::read(&p).map_err(|e| PyIOError::new_err(format!("{p}: 読めない: {e}")))?
        } else {
            return Err(PyTypeError::new_err(
                "画像は 径路の文字列 か bytes(PNG / JPEG)で渡してください",
            ));
        };
        let p = parse_ref(at)?;
        let (w, h) = ops::image_px(&data)
            .ok_or_else(|| PyValueError::new_err("PNG / JPEG として読めない(大きさが測れない)"))?;
        self.with(|s| {
            s.images_new.push(SheetImage {
                at: p,
                dx_px: 0.0,
                dy_px: 0.0,
                width_px: width_px.unwrap_or(w as f32),
                height_px: height_px.unwrap_or(h as f32),
                data,
            });
            Ok(())
        })
    }

    /// シートの画像 [(留めたセル, 幅px, 高さpx)]。開いた帳票にあった物と、
    /// add_image で足した物の両方が見える。
    #[getter]
    fn images(&self) -> PyResult<Vec<(String, f32, f32)>> {
        self.with(|s| {
            Ok(s.images
                .iter()
                .chain(s.images_new.iter())
                .map(|im| (im.at.a1(), im.width_px, im.height_px))
                .collect())
        })
    }

    /// 固定枠。openpyxl と同じ A1 形式 — "B2" は上1行・左1列を固定、
    /// "A2" は上1行だけ、None(か "A1")は固定なし。
    #[getter]
    fn freeze_panes(&self) -> PyResult<Option<String>> {
        self.with(|s| {
            Ok(s.freeze.map(|f| Pos { row: f.frozen_rows, col: f.frozen_columns }.a1()))
        })
    }

    #[setter]
    fn set_freeze_panes(&self, value: Option<&str>) -> PyResult<()> {
        let f = match value {
            None => None,
            Some(v) => {
                let p = parse_ref(v)?;
                (p.row > 0 || p.col > 0)
                    .then_some(FreezePane { frozen_rows: p.row, frozen_columns: p.col })
            }
        };
        self.with(|s| {
            s.freeze = f;
            Ok(())
        })
    }

    /// セルの書式を dict で読む。**持っている項目だけ**が入る(素のセルは空の
    /// dict)。鍵: bold / italic / underline / strike / font / size(pt)/
    /// color / fill(RRGGBB)/ number_format / horizontal / vertical
    /// (xlsx の言葉)/ wrap / shrink / rotation / border_top・bottom・
    /// left・right((線種, 色) — 色 None は自動の黒)。
    fn fmt<'py>(&self, py: Python<'py>, key: &str) -> PyResult<Bound<'py, PyDict>> {
        let p = parse_ref(key)?;
        self.with(|s| match s.get(p) {
            Some(c) => fmt_dict(py, &c.fmt),
            None => Ok(PyDict::new(py)),
        })
    }

    /// セルの書式を書く。**渡した項目だけ**が変わる(他は据え置き)。
    /// 消すには None を渡す(color=None で文字色が自動に戻る、等)。
    /// 鍵と値の形は `fmt` の返りと同じ。罫線は None(消す)/ 線種の文字 /
    /// (線種, 色) のどれでも。知らない鍵は黙って捨てず、エラーで言う。
    #[pyo3(signature = (key, **kw))]
    fn set_fmt(&self, key: &str, kw: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let p = parse_ref(key)?;
        let Some(kw) = kw else { return Ok(()) };
        self.with(|s| {
            let mut cell = s.get(p).cloned().unwrap_or_else(|| Cell::input(""));
            apply_fmt(&mut cell.fmt, kw)?;
            s.set(p, cell);
            Ok(())
        })
    }

    /// 印刷範囲。openpyxl と同じ「'シート名'!$A$1:$C$10」の形
    /// (複数の域は , 区切り)。無ければ None。
    #[getter]
    fn print_area(&self) -> PyResult<Option<String>> {
        self.with(|s| {
            if s.print_areas.is_empty() {
                return Ok(None);
            }
            let name = s.name.clone();
            Ok(Some(
                s.print_areas
                    .iter()
                    .map(|(a, b)| format!("'{}'!{}:{}", name, abs_a1(*a), abs_a1(*b)))
                    .collect::<Vec<_>>()
                    .join(","),
            ))
        })
    }

    /// 印刷範囲を置く。"A1:C10"($ や シート名! 付きでも)を , 区切りで。
    /// None か空で消す。PDF と印刷がこれに従う。
    #[setter]
    fn set_print_area(&self, value: Option<&str>) -> PyResult<()> {
        let mut areas: Vec<(Pos, Pos)> = Vec::new();
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            for part in v.split(',') {
                let p = part.trim().replace('$', "");
                let p = p.rsplit('!').next().unwrap_or(&p);
                areas.push(parse_range(p)?);
            }
        }
        self.with(|s| {
            s.print_areas = areas;
            Ok(())
        })
    }

    /// 範囲を動かす(切り取って貼るのと同じ)。openpyxl と同じ呼び方:
    /// `move_range("B1:C3", rows=5, cols=0)` で下へ5行。
    ///
    /// **参照の作法は Excel の切り貼りに合わせる**(ここが openpyxl との違い):
    /// 外から動かした範囲を指していた式は**付いて動く**(`=B1+1` は `=B6+1`)。
    /// openpyxl はここを古びたままにする — 空になったセルを黙って指す方が
    /// 危ないので、こちらは追随させる。範囲の中の式はそのまま(指していた先を
    /// 指し続ける)で、`translate=True` なら中の相対参照も同じだけずらす
    /// (openpyxl の translate と同じ定義)。移った先の中身は上書きされる。
    #[pyo3(signature = (cell_range, rows=0, cols=0, translate=false))]
    fn move_range(
        &self,
        cell_range: &str,
        rows: i64,
        cols: i64,
        translate: bool,
    ) -> PyResult<usize> {
        let (a, b) = parse_range(cell_range)?;
        if a.row as i64 + rows < 0 || a.col as i64 + cols < 0 {
            return Err(PyValueError::new_err(
                "紙の外(0行・0列より上)へは動かせません",
            ));
        }
        self.with_calc(|s| Ok(s.move_range(a, b, rows, cols, translate)))
    }

    /// 行のグループ化 [(行, 深さ, 畳んで隠れているか)](1起点)。
    #[getter]
    fn row_groups(&self) -> PyResult<Vec<(u32, u8, bool)>> {
        self.with(|s| {
            Ok(s.row_outline
                .iter()
                .map(|(r, lv)| (r + 1, *lv, s.row_hidden.contains(r)))
                .collect())
        })
    }

    /// 列のグループ化 [(列の字, 深さ, 畳んで隠れているか)]。
    #[getter]
    fn col_groups(&self) -> PyResult<Vec<(String, u8, bool)>> {
        self.with(|s| {
            Ok(s.col_outline
                .iter()
                .map(|(c, lv)| {
                    (Pos::new(0, *c).a1().trim_end_matches('1').to_string(), *lv,
                     s.col_hidden.contains(c))
                })
                .collect())
        })
    }

    /// 列の幅(xlsx の単位 = 標準の書体の「0」何個ぶん)。
    /// 指定の無い列は None = 既定幅(openpyxl の ColumnDimension.width と同じ)。
    /// **列の字("A")で引く** — 行と間違えないため
    fn col_width(&self, col: &str) -> PyResult<Option<f32>> {
        let c = col0(col)?;
        self.with(|s| Ok(s.col_width.get(&c).copied()))
    }

    /// 列の幅を置く。None で「指定なし」に戻す(既定幅で描く)
    #[pyo3(signature = (col, width))]
    fn set_col_width(&self, col: &str, width: Option<f32>) -> PyResult<()> {
        let c = col0(col)?;
        match width {
            Some(w) if w < 0.0 => return Err(PyValueError::new_err("列幅に負の数は置けない")),
            _ => {}
        }
        self.with(|s| {
            match width {
                Some(w) => s.col_width.insert(c, w),
                None => s.col_width.remove(&c),
            };
            Ok(())
        })
    }

    /// 行の高さ(ポイント)。指定の無い行は None = 既定の高さ。行番号は1起点
    fn row_height(&self, row: u32) -> PyResult<Option<f32>> {
        let r = row0(row)?;
        self.with(|s| Ok(s.row_height.get(&r).copied()))
    }

    /// 行の高さを置く。None で「指定なし」に戻す
    #[pyo3(signature = (row, height))]
    fn set_row_height(&self, row: u32, height: Option<f32>) -> PyResult<()> {
        let r = row0(row)?;
        match height {
            Some(h) if h < 0.0 => return Err(PyValueError::new_err("行の高さに負の数は置けない")),
            _ => {}
        }
        self.with(|s| {
            match height {
                Some(h) => s.row_height.insert(r, h),
                None => s.row_height.remove(&r),
            };
            Ok(())
        })
    }

    /// 列を隠す/出す(xlsx の hidden。**絞り込みと違って保存に残る**)
    fn col_hidden(&self, col: &str) -> PyResult<bool> {
        let c = col0(col)?;
        self.with(|s| Ok(s.col_hidden.contains(&c)))
    }

    fn set_col_hidden(&self, col: &str, hidden: bool) -> PyResult<()> {
        let c = col0(col)?;
        self.with(|s| {
            if hidden {
                s.col_hidden.insert(c);
            } else {
                s.col_hidden.remove(&c);
            }
            Ok(())
        })
    }

    /// 行を隠す/出す
    fn row_hidden(&self, row: u32) -> PyResult<bool> {
        let r = row0(row)?;
        self.with(|s| Ok(s.row_hidden.contains(&r)))
    }

    fn set_row_hidden(&self, row: u32, hidden: bool) -> PyResult<()> {
        let r = row0(row)?;
        self.with(|s| {
            if hidden {
                s.row_hidden.insert(r);
            } else {
                s.row_hidden.remove(&r);
            }
            Ok(())
        })
    }

    /// 行をグループにする(openpyxl の row_dimensions.group と同じ定義)。
    /// start / end は1起点、深さは 1〜7。hidden なら畳んだ状態で持つ
    /// (**畳んだ台帳は畳んだまま次の人に渡る** — 絞り込みと違い保存に残る)。
    #[pyo3(signature = (start, end=None, outline_level=1, hidden=false))]
    fn group_rows(
        &self,
        start: u32,
        end: Option<u32>,
        outline_level: u8,
        hidden: bool,
    ) -> PyResult<()> {
        let end = end.unwrap_or(start);
        if start == 0 || end == 0 {
            return Err(PyValueError::new_err("行番号は1から(0行は無い)"));
        }
        if !(1..=7).contains(&outline_level) {
            return Err(PyValueError::new_err("グループの深さは 1〜7"));
        }
        self.with(|s| {
            for r in start.min(end)..=start.max(end) {
                s.row_outline.insert(r - 1, outline_level);
                if hidden {
                    s.row_hidden.insert(r - 1);
                } else {
                    s.row_hidden.remove(&(r - 1));
                }
            }
            Ok(())
        })
    }

    /// 列をグループにする(start / end は "B" の形)。
    #[pyo3(signature = (start, end=None, outline_level=1, hidden=false))]
    fn group_cols(
        &self,
        start: &str,
        end: Option<&str>,
        outline_level: u8,
        hidden: bool,
    ) -> PyResult<()> {
        let a = col0(start)?;
        let b = match end {
            Some(e) => col0(e)?,
            None => a,
        };
        if !(1..=7).contains(&outline_level) {
            return Err(PyValueError::new_err("グループの深さは 1〜7"));
        }
        self.with(|s| {
            for c in a.min(b)..=a.max(b) {
                s.col_outline.insert(c, outline_level);
                if hidden {
                    s.col_hidden.insert(c);
                } else {
                    s.col_hidden.remove(&c);
                }
            }
            Ok(())
        })
    }

    /// 配列式(スピル)の一覧 [(左上のセル, 式, 行数, 列数)]。
    /// openpyxl の array_formulae と同じ役。
    #[getter]
    fn array_formulae(&self) -> PyResult<Vec<(String, String, u32, u32)>> {
        self.with(|s| {
            Ok(s.cse
                .iter()
                .map(|(p, (rows, cols))| {
                    let f = s
                        .get(*p)
                        .and_then(|c| c.formula.clone())
                        .map(|f| format!("={f}"))
                        .unwrap_or_default();
                    (p.a1(), f, *rows, *cols)
                })
                .collect())
        })
    }

    /// 表(テーブル)の一覧 [(名前, 範囲, 様式の名前, 見出し行, 合計行)]。
    /// 名前は式から使える(構造化参照 `=SUM(明細[金額])`)。
    #[getter]
    fn tables(&self) -> PyResult<Vec<(String, String, Option<String>, bool, bool)>> {
        self.with(|s| {
            Ok(s.tables
                .iter()
                .map(|t| {
                    (
                        t.name.clone(),
                        format!("{}:{}", t.a.a1(), t.b.a1()),
                        t.style.clone(),
                        t.header,
                        t.totals,
                    )
                })
                .collect())
        })
    }

    /// 表を作る。範囲は見出し行を含む "A1:C10"。名前は式から使う識別子で、
    /// **空白は入れられない**(構造化参照が解けなくなる)。
    /// style は様式の名前(`TableStyleMedium2` 等。省略は Excel の既定)。
    #[pyo3(signature = (range, name, style=None, header=true, totals=false,
                        banded_rows=true, banded_cols=false, filter=true))]
    #[allow(clippy::too_many_arguments)]
    fn add_table(
        &self,
        range: &str,
        name: &str,
        style: Option<String>,
        header: bool,
        totals: bool,
        banded_rows: bool,
        banded_cols: bool,
        filter: bool,
    ) -> PyResult<()> {
        if name.is_empty() || name.contains(char::is_whitespace) {
            return Err(PyValueError::new_err(format!(
                "表の名前に空白は入れられない(式から引けなくなる): {name:?}"
            )));
        }
        let (a, b) = parse_range(range)?;
        self.with(|s| {
            if s.tables.iter().any(|t| t.name == name) {
                return Err(PyValueError::new_err(format!("表「{name}」は既にあります")));
            }
            s.tables.push(sheet::model::TableDef {
                name: name.to_string(),
                style,
                a,
                b,
                header,
                totals,
                banded_rows,
                banded_cols,
                first_col: false,
                last_col: false,
                filter,
            });
            Ok(())
        })
    }

    /// 表を外す(中身と書式は残る — Excel と同じ)。返りは外せたか。
    fn remove_table(&self, name: &str) -> PyResult<bool> {
        self.with(|s| {
            let before = s.tables.len();
            s.tables.retain(|t| t.name != name);
            Ok(s.tables.len() != before)
        })
    }

    /// 画面の枠線を出すか(xlsx の sheetView showGridLines)。
    /// 原本に指定が無ければ None(= 出す、が既定)。
    #[getter]
    fn show_gridlines(&self) -> PyResult<Option<bool>> {
        self.with(|s| Ok(s.show_gridlines))
    }

    #[setter]
    fn set_show_gridlines(&self, value: Option<bool>) -> PyResult<()> {
        self.with(|s| {
            s.show_gridlines = value;
            Ok(())
        })
    }

    /// **印刷**の枠線を出すか(xlsx の printOptions gridLines)。
    /// 画面の枠線(show_gridlines)とは別の設定。
    #[getter]
    fn print_gridlines(&self) -> PyResult<bool> {
        self.with(|s| Ok(s.print_gridlines))
    }

    #[setter]
    fn set_print_gridlines(&self, value: bool) -> PyResult<()> {
        self.with(|s| {
            s.print_gridlines = value;
            Ok(())
        })
    }


    /// 偶数頁・先頭頁だけのヘッダー/フッター(xlsx の evenHeader ほか)と、
    /// それを使うかの旗。**左右で綴じる帳票**は偶数頁を別に組む。
    #[getter]
    fn print_header_even(&self) -> PyResult<Option<String>> {
        self.with(|s| Ok(s.header_even.clone()))
    }

    #[setter]
    fn set_print_header_even(&self, value: Option<&str>) -> PyResult<()> {
        self.with(|s| {
            s.header_even = value.filter(|v| !v.is_empty()).map(str::to_string);
            if s.header_even.is_some() {
                s.hf_diff_odd_even = true; // 置いたなら使う(旗の付け忘れを防ぐ)
            }
            Ok(())
        })
    }

    #[getter]
    fn print_footer_even(&self) -> PyResult<Option<String>> {
        self.with(|s| Ok(s.footer_even.clone()))
    }

    #[setter]
    fn set_print_footer_even(&self, value: Option<&str>) -> PyResult<()> {
        self.with(|s| {
            s.footer_even = value.filter(|v| !v.is_empty()).map(str::to_string);
            if s.footer_even.is_some() {
                s.hf_diff_odd_even = true;
            }
            Ok(())
        })
    }

    #[getter]
    fn print_header_first(&self) -> PyResult<Option<String>> {
        self.with(|s| Ok(s.header_first.clone()))
    }

    #[setter]
    fn set_print_header_first(&self, value: Option<&str>) -> PyResult<()> {
        self.with(|s| {
            s.header_first = value.filter(|v| !v.is_empty()).map(str::to_string);
            if s.header_first.is_some() {
                s.hf_diff_first = true;
            }
            Ok(())
        })
    }

    #[getter]
    fn print_footer_first(&self) -> PyResult<Option<String>> {
        self.with(|s| Ok(s.footer_first.clone()))
    }

    #[setter]
    fn set_print_footer_first(&self, value: Option<&str>) -> PyResult<()> {
        self.with(|s| {
            s.footer_first = value.filter(|v| !v.is_empty()).map(str::to_string);
            if s.footer_first.is_some() {
                s.hf_diff_first = true;
            }
            Ok(())
        })
    }

    /// 印刷のタイトル列(頁ごとに左で繰り返す列。openpyxl と同じ "A:B" の形)。
    /// **横に長い台帳で品名の列を毎ページ出す**ための物。
    #[getter]
    fn print_title_cols(&self) -> PyResult<Option<String>> {
        self.with(|s| {
            Ok(s.print_title_cols.map(|(a, b)| {
                let letter = |c: u32| {
                    let a1 = Pos::new(0, c).a1();
                    a1.trim_end_matches(|ch: char| ch.is_ascii_digit()).to_string()
                };
                format!("{}:{}", letter(a), letter(b))
            }))
        })
    }

    /// タイトル列を置く。"A:B"($ 付きでも)。None か空で消す。
    #[setter]
    fn set_print_title_cols(&self, value: Option<&str>) -> PyResult<()> {
        let cols = match value.map(str::trim).filter(|v| !v.is_empty()) {
            None => None,
            Some(v) => {
                let v = v.replace('$', "");
                let (a, b) = v.split_once(':').unwrap_or((v.as_str(), v.as_str()));
                let (a, b) = (col0(a.trim())?, col0(b.trim())?);
                Some((a.min(b), a.max(b)))
            }
        };
        self.with(|s| {
            s.print_title_cols = cols;
            Ok(())
        })
    }

    /// 印刷のヘッダー(xlsx の oddHeader の原文。&L 左 &C 中 &R 右)。
    /// 無ければ None。三分割で扱うのは互換層の役。
    #[getter]
    fn print_header(&self) -> PyResult<Option<String>> {
        self.with(|s| Ok(s.header.clone()))
    }

    #[setter]
    fn set_print_header(&self, value: Option<&str>) -> PyResult<()> {
        self.with(|s| {
            s.header = value.filter(|v| !v.is_empty()).map(str::to_string);
            Ok(())
        })
    }

    /// 印刷のフッター(xlsx の oddFooter の原文)。
    #[getter]
    fn print_footer(&self) -> PyResult<Option<String>> {
        self.with(|s| Ok(s.footer.clone()))
    }

    #[setter]
    fn set_print_footer(&self, value: Option<&str>) -> PyResult<()> {
        self.with(|s| {
            s.footer = value.filter(|v| !v.is_empty()).map(str::to_string);
            Ok(())
        })
    }

    /// 印刷のタイトル行(頁ごとに繰り返す見出し)。openpyxl と同じ "1:2" の形。
    /// 無ければ None。**PDF と印刷が実際に繰り返す**(paper::grid)。
    #[getter]
    fn print_title_rows(&self) -> PyResult<Option<String>> {
        self.with(|s| Ok(s.print_title_rows.map(|(a, b)| format!("{}:{}", a + 1, b + 1))))
    }

    /// タイトル行を置く。"1:2"($ 付きでも)。None か空で消す。
    #[setter]
    fn set_print_title_rows(&self, value: Option<&str>) -> PyResult<()> {
        let rows = match value.map(str::trim).filter(|v| !v.is_empty()) {
            None => None,
            Some(v) => {
                let v = v.replace('$', "");
                let (a, b) = v.split_once(':').unwrap_or((v.as_str(), v.as_str()));
                let (a, b) = (
                    a.trim().parse::<u32>().map_err(|_| {
                        PyValueError::new_err(format!("タイトル行は \"1:2\" の形で: {v:?}"))
                    })?,
                    b.trim().parse::<u32>().map_err(|_| {
                        PyValueError::new_err(format!("タイトル行は \"1:2\" の形で: {v:?}"))
                    })?,
                );
                if a == 0 || b == 0 {
                    return Err(PyValueError::new_err("行番号は1から(0行は無い)"));
                }
                Some((a.min(b) - 1, a.max(b) - 1))
            }
        };
        self.with(|s| {
            s.print_title_rows = rows;
            Ok(())
        })
    }

    /// 入力規則の一覧 [(範囲, type, formula1, formula2, operator)]。
    #[getter]
    fn validations(&self) -> PyResult<Vec<(String, String, String, String, String)>> {
        self.with(|s| {
            Ok(s.validations
                .iter()
                .map(|v| {
                    let r = if v.range.0 == v.range.1 {
                        v.range.0.a1()
                    } else {
                        format!("{}:{}", v.range.0.a1(), v.range.1.a1())
                    };
                    (r, v.kind.clone(), v.formula.clone(), v.formula2.clone(), v.op.clone())
                })
                .collect())
        })
    }

    /// 入力規則を足す。list なら formula1 は `"甲,乙"`(直書き)か範囲参照。
    /// エンジンは list を効かせ(規則に合わない入力を堰き止め)、他の種類も
    /// **落とさず持ち越す**(判定は分かる物だけ — 模型の注のとおり)。
    #[pyo3(signature = (range, formula1, kind="list", operator="", formula2="", allow_blank=true))]
    fn add_validation(
        &self,
        range: &str,
        formula1: &str,
        kind: &str,
        operator: &str,
        formula2: &str,
        allow_blank: bool,
    ) -> PyResult<()> {
        let (a, b) = parse_range(range)?;
        self.with(|s| {
            s.validations.push(sheet::model::Validation {
                range: (a, b),
                formula: formula1.to_string(),
                kind: kind.to_string(),
                op: operator.to_string(),
                formula2: formula2.to_string(),
                input_msg: None,
                error_msg: None,
                allow_blank,
                hide_arrow: false,
            });
            Ok(())
        })
    }

    /// 名前の定義 [(名前, 参照 "A1" か "A1:B2")]。式の中で名前が使える。
    #[getter]
    fn names(&self) -> PyResult<Vec<(String, String)>> {
        self.with(|s| Ok(s.names.clone()))
    }

    /// 名前を定義する(同じ名前は置き換え)。参照はこのシートの "A1" か "A1:B2"。
    /// 定義した名前は式(=名前*2)で使え、再計算が追随する。
    fn define_name(&self, name: &str, reference: &str) -> PyResult<()> {
        if name.is_empty() || name.contains([' ', '!', ':']) {
            return Err(PyValueError::new_err(format!(
                "名前に空白・! ・: は使えない: {name:?}"
            )));
        }
        parse_range(reference)?; // 形の検査だけ(向きの正規化はしない — 原文を保つ)
        self.with_calc(|s| {
            s.names.retain(|(n, _)| n != name);
            s.names.push((name.to_string(), reference.to_string()));
            Ok(())
        })
    }

    /// 名前を消す。返りは消せたか。
    fn delete_name(&self, name: &str) -> PyResult<bool> {
        self.with_calc(|s| {
            let before = s.names.len();
            s.names.retain(|(n, _)| n != name);
            Ok(s.names.len() != before)
        })
    }

    /// セルのコメント(無ければ None)。
    fn comment(&self, key: &str) -> PyResult<Option<String>> {
        let p = parse_ref(key)?;
        self.with(|s| Ok(s.comments.get(&p).cloned()))
    }

    /// セルのコメントを置く(None で消す)。保存で commentsN.xml に入る。
    fn set_comment(&self, key: &str, value: Option<&str>) -> PyResult<()> {
        let p = parse_ref(key)?;
        self.with(|s| {
            match value.filter(|v| !v.is_empty()) {
                Some(v) => {
                    s.comments.insert(p, v.to_string());
                }
                None => {
                    s.comments.remove(&p);
                }
            }
            Ok(())
        })
    }

    /// セルのハイパーリンク(外部URL。無ければ None)。
    fn hyperlink(&self, key: &str) -> PyResult<Option<String>> {
        let p = parse_ref(key)?;
        self.with(|s| Ok(s.links.get(&p).cloned()))
    }

    /// セルのハイパーリンクを置く(None で消す)。
    fn set_hyperlink(&self, key: &str, value: Option<&str>) -> PyResult<()> {
        let p = parse_ref(key)?;
        self.with(|s| {
            match value.filter(|v| !v.is_empty()) {
                Some(v) => {
                    s.links.insert(p, v.to_string());
                }
                None => {
                    s.links.remove(&p);
                }
            }
            Ok(())
        })
    }

    /// 行を挿す。`at` は画面で見える行番号(1起点)。その行の位置に空行が入り、
    /// 下の行と**残った式の参照**が下がる(明細の行を増やす操作)。
    fn insert_row(&self, at: u32) -> PyResult<()> {
        self.with_calc(|s| {
            s.insert_row(row0(at)?);
            Ok(())
        })
    }

    /// 行を抜く(1起点)。抜いた行を指していた式は #REF! になる — 黙って
    /// 別のセルを指すより良い。
    fn remove_row(&self, at: u32) -> PyResult<()> {
        self.with_calc(|s| {
            s.remove_row(row0(at)?);
            Ok(())
        })
    }

    /// 列を挿す。`at` は列の文字("C" なら C 列の位置に空列が入る)。
    fn insert_col(&self, at: &str) -> PyResult<()> {
        let c = col0(at)?;
        self.with_calc(|s| {
            s.insert_col(c);
            Ok(())
        })
    }

    /// 列を抜く(列の文字で指す)。
    fn remove_col(&self, at: &str) -> PyResult<()> {
        let c = col0(at)?;
        self.with_calc(|s| {
            s.remove_col(c);
            Ok(())
        })
    }
}

/// CellFormat を Python の dict に写す(Sheet.fmt と Book.named_style_fmt の
/// **一本道** — 別々に書くと鍵が食い違う)。持っている項目だけを入れる。
fn fmt_dict<'py>(
    py: Python<'py>,
    f: &sheet::model::CellFormat,
) -> PyResult<Bound<'py, PyDict>> {
    {
        {
            let d = PyDict::new(py);
            for (k, on) in [
                ("bold", f.bold),
                ("italic", f.italic),
                ("underline", f.underline),
                ("strike", f.strike),
                ("wrap", f.wrap),
                ("shrink", f.shrink),
            ] {
                if on {
                    d.set_item(k, true)?;
                }
            }
            if let Some(v) = &f.font {
                d.set_item("font", v)?;
            }
            if let Some(sc) = f.size_c {
                d.set_item("size", sc as f64 / 100.0)?;
            }
            if let Some(v) = &f.color {
                d.set_item("color", v)?;
            }
            if let Some(v) = &f.fill {
                d.set_item("fill", v)?;
            }
            if let Some(v) = &f.number_format {
                d.set_item("number_format", v)?;
            }
            if let Some(v) = f.align.as_xlsx() {
                d.set_item("horizontal", v)?;
            }
            if let Some(v) = f.valign.as_xlsx() {
                d.set_item("vertical", v)?;
            }
            if let Some(v) = f.rotation {
                d.set_item("rotation", v)?;
            }
            for (k, e) in [
                ("border_top", f.borders.top),
                ("border_bottom", f.borders.bottom),
                ("border_left", f.borders.left),
                ("border_right", f.borders.right),
            ] {
                if e.on {
                    d.set_item(k, (e.style.xlsx(), e.color.map(|c| format!("{c:06X}"))))?;
                }
            }
            if f.indent > 0 {
                d.set_item("indent", f.indent)?;
            }
            if f.unlocked {
                // 保護中でも書けるセル(xlsx の locked="0")。既定(ロック)は出さない
                d.set_item("locked", false)?;
            }
            Ok(d)
        }
    }
}


/// dict の鍵を CellFormat に写す(Sheet.set_fmt と Book.add_named_style の
/// **一本道** — 別々に書くと受ける鍵がずれる)。渡した項目だけ変える。
fn apply_fmt(f: &mut sheet::model::CellFormat, kw: &Bound<'_, PyDict>) -> PyResult<()> {
        for (k, v) in kw.iter() {
            let k: String = k.extract()?;
            match k.as_str() {
                "bold" => f.bold = v.extract::<Option<bool>>()?.unwrap_or(false),
                "italic" => f.italic = v.extract::<Option<bool>>()?.unwrap_or(false),
                "underline" => f.underline = v.extract::<Option<bool>>()?.unwrap_or(false),
                "strike" => f.strike = v.extract::<Option<bool>>()?.unwrap_or(false),
                "wrap" => f.wrap = v.extract::<Option<bool>>()?.unwrap_or(false),
                "shrink" => f.shrink = v.extract::<Option<bool>>()?.unwrap_or(false),
                "font" => f.font = v.extract()?,
                "size" => {
                    f.size_c = v.extract::<Option<f64>>()?.map(|x| (x * 100.0).round() as u32)
                }
                "color" => {
                    f.color = v.extract::<Option<String>>()?;
                    f.color_theme = None; // 直に塗った — テーマ由来ではなくなる
                }
                "fill" => {
                    f.fill = v.extract::<Option<String>>()?;
                    f.fill_theme = None;
                }
                "number_format" => f.number_format = v.extract()?,
                "horizontal" => {
                    f.align = v
                        .extract::<Option<String>>()?
                        .map(|x| HAlign::from_xlsx(&x))
                        .unwrap_or(HAlign::General)
                }
                "vertical" => {
                    f.valign = v
                        .extract::<Option<String>>()?
                        .map(|x| VAlign::from_xlsx(&x))
                        .unwrap_or(VAlign::Bottom)
                }
                "rotation" => f.rotation = v.extract()?,
                "indent" => {
                    f.indent = v.extract::<Option<u8>>()?.unwrap_or(0).min(250)
                }
                "locked" => {
                    f.unlocked = !v.extract::<Option<bool>>()?.unwrap_or(true)
                }
                "border_top" | "border_bottom" | "border_left" | "border_right" => {
                    let e = if v.is_none() {
                        Edge::OFF
                    } else if let Ok(style) = v.extract::<String>() {
                        Edge::line(BStyle::from_xlsx(&style), None)
                    } else if let Ok((style, color)) =
                        v.extract::<(String, Option<String>)>()
                    {
                        let c = color
                            .map(|x| {
                                u32::from_str_radix(&x, 16).map_err(|_| {
                                    PyValueError::new_err(format!(
                                        "罫線の色は RRGGBB で: {x:?}"
                                    ))
                                })
                            })
                            .transpose()?;
                        Edge::line(BStyle::from_xlsx(&style), c)
                    } else {
                        return Err(PyTypeError::new_err(
                            "罫線は None / 線種の文字 / (線種, 色) で渡してください",
                        ));
                    };
                    match k.as_str() {
                        "border_top" => f.borders.top = e,
                        "border_bottom" => f.borders.bottom = e,
                        "border_left" => f.borders.left = e,
                        _ => f.borders.right = e,
                    }
                }
                other => {
                    return Err(PyValueError::new_err(format!(
                        "知らない書式の鍵: {other:?}(fmt() の返りと同じ鍵で)"
                    )))
                }
            }
        }    Ok(())
}

/// 画面の行番号(1起点)→ 内部の行(0起点)。0行目は無い。
fn row0(at: u32) -> PyResult<u32> {
    at.checked_sub(1).ok_or_else(|| PyValueError::new_err("行番号は1から(0行は無い)"))
}

/// "C10" → "$C$10"(openpyxl の印刷範囲の形)。
fn abs_a1(p: Pos) -> String {
    let a1 = p.a1();
    let i = a1.chars().take_while(|c| c.is_ascii_alphabetic()).count();
    format!("${}${}", &a1[..i], &a1[i..])
}

/// 列の文字("A"〜)→ 内部の列(0起点)。
fn col0(s: &str) -> PyResult<u32> {
    let p = Pos::parse(&format!("{}1", s.trim()))
        .ok_or_else(|| PyValueError::new_err(format!("列の文字として読めない: {s:?}")))?;
    Ok(p.col)
}

#[pymodule]
fn _sheet(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBook>()?;
    m.add_class::<PySheet>()?;
    // docx の束縛。**同じ .so に同居させる** — maturin が組む拡張は1つなので、
    // 副モジュールとして建て、officework/_doc.py が `officework.doc` の名前で受ける
    doc::register(m)?;
    Ok(())
}
