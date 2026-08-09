//! officework.doc — `ooxml`(docx)の Python 束縛。
//!
//! `sheet` と同じ論法: **原本を正として、変えた所だけ書き戻す。**
//! `Doc.open` が読んだバイトを抱えたまま、`save` が `ooxml::write_with` に渡すので、
//! 様式・ヘッダー・図形・変更履歴・こちらが読めなかった部品まで原本のまま残る。
//! python-docx が苦手なのはまさにそこ(理解できない部品を書き直してしまう)。
//!
//!     from officework import doc
//!     d = doc.Doc.open("報告書.docx")
//!     print(d.unsupported)     # 読めなかった物はここに出る(黙って落とさない)
//!     d[3].text = "差し替え"    # 段落の書式は据え置き
//!     d.replace("旧社名", "新社名")
//!     d.save("out.docx")
//!
//! **`kumihan::Document` をそのまま見せない。** あれは組版の模型で、
//! `Run.size_pt` や `CharFormat` まで Python に出すと細かすぎる。
//! ここで見せるのは 文書 / 段落 / 表・行・セル の薄い層だけ。

use std::sync::{Arc, Mutex, MutexGuard};

use pyo3::exceptions::{PyIOError, PyIndexError, PyValueError};
use pyo3::prelude::*;

use kumihan::{Block, CharFormat, Document, Paragraph, Run};

/// 書式を持たない段落に字を入れるときの大きさ。
/// kumihan の各所が `unwrap_or(10.5)` で使っている既定値に合わせる。
const DEFAULT_PT: f32 = 10.5;

/// 文書の中身。Doc / Paragraph / Table が同じ物を見るために1枚挟む
/// (pysheet の `Inner` と同じ作り)。
struct Inner {
    doc: Document,
    /// 開いた元のファイル。保存時に、こちらが作り直さない部品
    /// (図形・様式・変更履歴・読めなかった部品)を持ち越すために取っておく。
    /// **これが売り文句の土台** — 無いと python-docx と同じ問題を抱える
    original: Option<Vec<u8>>,
    /// 読めなかった物の帳簿。黙って落とさない(ooxml の Report と同じ)
    unsupported: Vec<(String, usize)>,
}

fn lock(inner: &Arc<Mutex<Inner>>) -> PyResult<MutexGuard<'_, Inner>> {
    inner
        .lock()
        .map_err(|_| PyValueError::new_err("別の操作が失敗した後で、文書の状態が信用できない"))
}

/// 段落の在り処。Python 側の handle は位置だけ持ち、中身は触るたびに引き直す
/// (pysheet の `PySheet { idx }` と同じ考え方)。
#[derive(Clone)]
enum Loc {
    /// 本文の blocks[i](必ず Block::Para)
    Body(usize),
    /// 表のセルの中の段落
    Cell { block: usize, row: usize, col: usize, para: usize },
}

impl Inner {
    /// 本文の段落が blocks の何番目にいるか(順番どおり)。
    fn body_blocks(&self) -> Vec<usize> {
        self.doc
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| matches!(b, Block::Para(_)))
            .map(|(i, _)| i)
            .collect()
    }

    /// 表が blocks の何番目にいるか(順番どおり)。
    fn table_blocks(&self) -> Vec<usize> {
        self.doc
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| matches!(b, Block::Table(_)))
            .map(|(i, _)| i)
            .collect()
    }

    fn para(&self, loc: &Loc) -> Option<&Paragraph> {
        match loc {
            Loc::Body(b) => match self.doc.blocks.get(*b)? {
                Block::Para(p) => Some(p),
                _ => None,
            },
            Loc::Cell { block, row, col, para } => match self.doc.blocks.get(*block)? {
                Block::Table(t) => t.rows.get(*row)?.get(*col)?.paragraphs.get(*para),
                _ => None,
            },
        }
    }

    fn para_mut(&mut self, loc: &Loc) -> Option<&mut Paragraph> {
        match loc {
            Loc::Body(b) => match self.doc.blocks.get_mut(*b)? {
                Block::Para(p) => Some(p),
                _ => None,
            },
            Loc::Cell { block, row, col, para } => match self.doc.blocks.get_mut(*block)? {
                Block::Table(t) => t.rows.get_mut(*row)?.get_mut(*col)?.paragraphs.get_mut(*para),
                _ => None,
            },
        }
    }

    fn table(&self, block: usize) -> Option<&kumihan::Table> {
        match self.doc.blocks.get(block)? {
            Block::Table(t) => Some(t),
            _ => None,
        }
    }

    /// 本文と表のセル、すべての段落の在り処を上から順に。
    /// `find` と `replace` が歩く順番でもある。
    fn all_locs(&self) -> Vec<Loc> {
        let mut out = Vec::new();
        for (bi, b) in self.doc.blocks.iter().enumerate() {
            match b {
                Block::Para(_) => out.push(Loc::Body(bi)),
                Block::Table(t) => {
                    for (ri, row) in t.rows.iter().enumerate() {
                        for (ci, cell) in row.iter().enumerate() {
                            for pi in 0..cell.paragraphs.len() {
                                out.push(Loc::Cell { block: bi, row: ri, col: ci, para: pi });
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

/// 段落の字(run をつないだもの)。
fn para_text(p: &Paragraph) -> String {
    p.runs.iter().map(|r| r.text.as_str()).collect()
}

/// ページ番号の印を読める字に直す。
///
/// kumihan はページ番号を私用領域の1字(`PAGE_MARK` = U+E000、総ページ数は
/// U+E001)で持ち、組むときに実際の番号にする。**Python へその字をそのまま
/// 出さない** — 見ても意味が分からず、比べる相手も持っていない。
/// `#`(ページ番号)と `##`(総ページ数)に置く。
///
/// **使うのはヘッダーとフッターだけ。** 段落やセルの字には掛けない —
/// あちらは `text` の代入や `replace` で書き戻す口があり、読んだ字と
/// 中の字が食い違うと「読んだままを戻したら壊れた」が起きるため。
fn marks_to_text(s: &str) -> String {
    s.replace(kumihan::PAGES_MARK, "##").replace(kumihan::PAGE_MARK, "#")
}

/// 段落の字を丸ごと入れ替える。**段落の性質(見出し・寄せ・箇条書き・字下げ・
/// しおり・図・アンカー)はそのまま**で、run だけを1本に置き換える。
///
/// 書式は**先頭 run のものを継ぐ**。これは kumihan の `set_paras_text`
/// (writer がセルとヘッダーの編集で使っている規則)と同じ — Python から
/// 触ったときと writer で打ったときで結果が変わらないようにする。
/// 継ぐ物には `CharFormat` を丸ごと含む(記入欄 `sdt` も継ぐ)。
/// **帳票の差し込みでは、記入欄が記入欄のまま残るのが正しい**ため。
///
/// 段落が空(run が無い)なら、既定の大きさの素の run を作る。
fn set_para_text(p: &mut Paragraph, text: &str) {
    let (pt, font, fmt) = p
        .runs
        .first()
        .map(|r| (r.size_pt, r.font.clone(), r.fmt.clone()))
        .unwrap_or((DEFAULT_PT, None, CharFormat::default()));
    p.runs = vec![Run { text: text.to_string(), size_pt: pt, font, fmt }];
}

/// run の並びの中で `old` を `new` に置き換える。返りは置き換えた回数。
///
/// `text` の代入と違い、**run の切れ目をそのまま残す** — 見つかった所だけを
/// 差し替えるので、同じ段落の他の書式(太字の見出し語など)は動かない。
/// 差し替えた字は、見つかり始めた run の書式になる。
/// 帳票の差し込み(「旧社名」→「新社名」)はこちらが本筋。
fn replace_in_runs(runs: &mut [Run], old: &str, new: &str) -> usize {
    if old.is_empty() || runs.is_empty() {
        return 0;
    }
    let whole: String = runs.iter().map(|r| r.text.as_str()).collect();
    // run ごとの [始まり, 終わり)(バイト)
    let mut spans = Vec::with_capacity(runs.len());
    let mut at = 0usize;
    for r in runs.iter() {
        spans.push((at, at + r.text.len()));
        at += r.text.len();
    }
    // 見つかった所を左から、重ならないように拾う
    let mut hits: Vec<(usize, usize)> = Vec::new();
    let mut from = 0usize;
    while let Some(i) = whole[from..].find(old) {
        let s = from + i;
        hits.push((s, s + old.len()));
        from = s + old.len();
    }
    if hits.is_empty() {
        return 0;
    }
    // run ごとに、自分に掛かる部分だけを切り貼りする。
    // 掛かりが複数の run に跨るときは、**始まりを含む run が新しい字を持ち**、
    // 続きの run からは掛かった分を落とす
    for (ri, r) in runs.iter_mut().enumerate() {
        let (rs, re) = spans[ri];
        if rs == re {
            continue;
        }
        let mut out = String::new();
        let mut cur = rs; // この run で、まだ書き出していない位置
        for &(hs, he) in &hits {
            if he <= rs || hs >= re {
                continue; // 掛からない
            }
            let cut = hs.max(rs);
            out.push_str(&whole[cur..cut]);
            if hs >= rs {
                out.push_str(new); // 始まりを含む run だけが新しい字を持つ
            }
            cur = he.min(re);
        }
        if cur < re {
            out.push_str(&whole[cur..re]);
        }
        r.text = out;
    }
    hits.len()
}

/// Python の添字(負も可)→ 0起点の位置。
fn resolve(i: isize, len: usize, what: &str) -> PyResult<usize> {
    let n = len as isize;
    let k = if i < 0 { i + n } else { i };
    if k < 0 || k >= n {
        return Err(PyIndexError::new_err(format!("{what}は {len} しかない: {i}")));
    }
    Ok(k as usize)
}

// ───────────────────────────────────────── 文書

/// docx の文書。
#[pyclass(name = "Doc", module = "officework.doc")]
struct PyDoc {
    inner: Arc<Mutex<Inner>>,
}

#[pymethods]
impl PyDoc {
    /// 空の文書。
    #[new]
    fn new() -> PyDoc {
        PyDoc {
            inner: Arc::new(Mutex::new(Inner {
                doc: Document::default(),
                original: None,
                unsupported: Vec::new(),
            })),
        }
    }

    /// docx を開く。**元のバイトを抱えたまま**持つ(`save` で使う)。
    #[staticmethod]
    fn open(path: &str) -> PyResult<PyDoc> {
        let bytes =
            std::fs::read(path).map_err(|e| PyIOError::new_err(format!("{path}: 読めない: {e}")))?;
        let (doc, rep) = ooxml::read(std::io::Cursor::new(&bytes))
            .map_err(|e| PyIOError::new_err(format!("{path}: docx として読めない: {e}")))?;
        Ok(PyDoc {
            inner: Arc::new(Mutex::new(Inner {
                doc,
                original: Some(bytes),
                unsupported: rep.unsupported,
            })),
        })
    }

    /// 保存する。開いた元のファイルがあれば `ooxml::write_with` に渡し、
    /// **こちらが作り直さない部品は原本のまま持ち越す**(様式・図形・変更履歴・
    /// 読めなかった部品)。openpyxl / python-docx との違いはここ。
    fn save(&self, path: &str) -> PyResult<()> {
        let g = lock(&self.inner)?;
        let mut buf = std::io::Cursor::new(Vec::new());
        let r = match &g.original {
            Some(bytes) => {
                ooxml::write_with(&g.doc, Some(std::io::Cursor::new(bytes)), &mut buf)
            }
            None => ooxml::write(&g.doc, &mut buf),
        };
        r.map_err(|e| PyIOError::new_err(format!("{path}: 書けない: {e}")))?;
        std::fs::write(path, buf.into_inner())
            .map_err(|e| PyIOError::new_err(format!("{path}: 書けない: {e}")))
    }

    /// 読めなかった物の帳簿 [(名前, 回数)]。空なら取りこぼしなし。
    /// **黙って落とさない** — ここに出た物も、原本から持ち越されて保存はされる。
    #[getter]
    fn unsupported(&self) -> PyResult<Vec<(String, usize)>> {
        Ok(lock(&self.inner)?.unsupported.clone())
    }

    /// 本文の段落の一覧(表の中の段落は入らない)。
    #[getter]
    fn paragraphs(&self) -> PyResult<Vec<PyParagraph>> {
        let g = lock(&self.inner)?;
        Ok(g.body_blocks()
            .into_iter()
            .map(|b| PyParagraph { inner: Arc::clone(&self.inner), loc: Loc::Body(b) })
            .collect())
    }

    /// 表の一覧。
    #[getter]
    fn tables(&self) -> PyResult<Vec<PyTable>> {
        let g = lock(&self.inner)?;
        Ok(g.table_blocks()
            .into_iter()
            .map(|b| PyTable { inner: Arc::clone(&self.inner), block: b })
            .collect())
    }

    /// 本文の段落を番号で。`d[3]` / `d[-1]`。
    fn __getitem__(&self, i: isize) -> PyResult<PyParagraph> {
        let g = lock(&self.inner)?;
        let blocks = g.body_blocks();
        let k = resolve(i, blocks.len(), "本文の段落は")?;
        Ok(PyParagraph { inner: Arc::clone(&self.inner), loc: Loc::Body(blocks[k]) })
    }

    /// 本文の段落の数。
    fn __len__(&self) -> PyResult<usize> {
        Ok(lock(&self.inner)?.body_blocks().len())
    }

    /// 本文の字(段落を改行で繋いだもの)。表の中は入らない。
    #[getter]
    fn text(&self) -> PyResult<String> {
        let g = lock(&self.inner)?;
        Ok(g.body_blocks()
            .into_iter()
            .filter_map(|b| g.para(&Loc::Body(b)).map(para_text))
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// ヘッダーの字(読むだけ)。保存では原本の物がそのまま残る。
    /// ページ番号は `#`、総ページ数は `##` に置いて返す([`marks_to_text`])。
    #[getter]
    fn header(&self) -> PyResult<String> {
        Ok(marks_to_text(&kumihan::paras_text(&lock(&self.inner)?.doc.header.paragraphs)))
    }

    /// フッターの字(読むだけ)。ヘッダーと同じくページ番号は `#`。
    #[getter]
    fn footer(&self) -> PyResult<String> {
        Ok(marks_to_text(&kumihan::paras_text(&lock(&self.inner)?.doc.footer.paragraphs)))
    }

    /// `needle` を含む段落を、本文・表のセルの区別なく上から順に返す。
    /// 差し込みの下ごしらえ(どこに入るのか先に見る)。
    fn find(&self, needle: &str) -> PyResult<Vec<PyParagraph>> {
        let g = lock(&self.inner)?;
        Ok(g.all_locs()
            .into_iter()
            .filter(|l| g.para(l).is_some_and(|p| para_text(p).contains(needle)))
            .map(|loc| PyParagraph { inner: Arc::clone(&self.inner), loc })
            .collect())
    }

    /// 本文と表のセルを通して `old` を `new` に置き換える。返りは置き換えた回数。
    /// **run の切れ目を残す**ので、同じ段落の他の書式は動かない。帳票の差し込みはこれ。
    fn replace(&self, old: &str, new: &str) -> PyResult<usize> {
        let mut g = lock(&self.inner)?;
        let locs = g.all_locs();
        let mut n = 0;
        for loc in locs {
            if let Some(p) = g.para_mut(&loc) {
                n += replace_in_runs(&mut p.runs, old, new);
            }
        }
        Ok(n)
    }

    /// 本文の末尾に段落を足す。
    fn add_paragraph(&self, text: &str) -> PyResult<PyParagraph> {
        let mut g = lock(&self.inner)?;
        let mut p = Paragraph { line_spacing: 1.0, ..Default::default() };
        set_para_text(&mut p, text);
        g.doc.blocks.push(Block::Para(p));
        let b = g.doc.blocks.len() - 1;
        Ok(PyParagraph { inner: Arc::clone(&self.inner), loc: Loc::Body(b) })
    }

    fn __repr__(&self) -> PyResult<String> {
        let g = lock(&self.inner)?;
        Ok(format!(
            "<officework.doc.Doc 段落 {} ・表 {}>",
            g.body_blocks().len(),
            g.table_blocks().len()
        ))
    }
}

// ───────────────────────────────────────── 段落

/// 1つの段落。本文にも表のセルの中にもある。
#[pyclass(name = "Paragraph", module = "officework.doc")]
struct PyParagraph {
    inner: Arc<Mutex<Inner>>,
    loc: Loc,
}

impl PyParagraph {
    fn with<T>(&self, f: impl FnOnce(&Paragraph) -> T) -> PyResult<T> {
        let g = lock(&self.inner)?;
        let p = g.para(&self.loc).ok_or_else(|| {
            PyIndexError::new_err("この段落はもう文書に無い(文書の形が変わった)")
        })?;
        Ok(f(p))
    }

    fn with_mut<T>(&self, f: impl FnOnce(&mut Paragraph) -> T) -> PyResult<T> {
        let mut g = lock(&self.inner)?;
        let p = g.para_mut(&self.loc).ok_or_else(|| {
            PyIndexError::new_err("この段落はもう文書に無い(文書の形が変わった)")
        })?;
        Ok(f(p))
    }
}

#[pymethods]
impl PyParagraph {
    /// 段落の字(run をつないだもの)。
    #[getter]
    fn text(&self) -> PyResult<String> {
        self.with(para_text)
    }

    /// 字を入れ替える。**段落の性質(見出し・寄せ・箇条書き・字下げ・しおり・図)は
    /// 据え置き**で、字の書式は先頭 run のものを継ぐ。
    ///
    /// 段落の中で書式が分かれている(「請求先: 」が明朝で「□□□」が太字、など)
    /// ときは、**全部が先頭 run の書式になる**。分かれ目を残したいときは
    /// `replace()` を使う — そちらは見つかった所だけを差し替える。
    #[setter]
    fn set_text(&self, text: &str) -> PyResult<()> {
        if text.contains('\n') {
            return Err(PyValueError::new_err(
                "段落は1つ。改行を入れたいなら段落を分けるか、セルなら Cell.text に入れる",
            ));
        }
        self.with_mut(|p| set_para_text(p, text))
    }

    /// この段落の中だけで置き換える。返りは置き換えた回数。
    /// **run の切れ目を残す**(書式の分かれ目が動かない)。
    fn replace(&self, old: &str, new: &str) -> PyResult<usize> {
        self.with_mut(|p| replace_in_runs(&mut p.runs, old, new))
    }

    /// 書式のまとまり(run)の一覧。読むだけ — 差し込みの結果、書式が
    /// 残っているかを確かめるのに使う。
    #[getter]
    fn runs(&self) -> PyResult<Vec<PyRun>> {
        self.with(|p| {
            p.runs
                .iter()
                .map(|r| PyRun {
                    text: r.text.clone(),
                    size_pt: r.size_pt,
                    font: r.font.clone(),
                    bold: r.fmt.bold,
                    italic: r.fmt.italic,
                    underline: r.fmt.underline,
                    color: r.fmt.color.clone(),
                })
                .collect()
        })
    }

    /// 段落の役目。"body" か "heading1"〜"heading9"、目次なら "toc1" 等。
    #[getter]
    fn style(&self) -> PyResult<String> {
        self.with(|p| match p.style {
            kumihan::ParaStyle::Body => "body".to_string(),
            kumihan::ParaStyle::Heading(n) => format!("heading{n}"),
            kumihan::ParaStyle::Toc(n) => format!("toc{n}"),
            kumihan::ParaStyle::Tof => "tof".to_string(),
        })
    }

    /// 行の寄せ。"left" / "center" / "right" / "justify" / "distribute"。
    #[getter]
    fn align(&self) -> PyResult<String> {
        self.with(|p| {
            match p.align {
                kumihan::Align::Left => "left",
                kumihan::Align::Center => "center",
                kumihan::Align::Right => "right",
                kumihan::Align::Justify => "justify",
                kumihan::Align::Distribute => "distribute",
            }
            .to_string()
        })
    }

    /// 表のセルの中の段落なら True。
    #[getter]
    fn in_table(&self) -> bool {
        matches!(self.loc, Loc::Cell { .. })
    }

    fn __repr__(&self) -> PyResult<String> {
        let t = self.text()?;
        let short: String = t.chars().take(20).collect();
        Ok(format!(
            "<officework.doc.Paragraph {:?}{}>",
            short,
            if t.chars().count() > 20 { "…" } else { "" }
        ))
    }
}

/// 書式のまとまり。**写しであって handle ではない**(読むだけ)。
/// 字を替えるのは段落の `text` か `replace` から。
#[pyclass(name = "Run", module = "officework.doc", frozen, get_all)]
struct PyRun {
    text: String,
    size_pt: f32,
    font: Option<String>,
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<String>,
}

#[pymethods]
impl PyRun {
    fn __repr__(&self) -> String {
        let mut m = Vec::new();
        if self.bold {
            m.push("太");
        }
        if self.italic {
            m.push("斜");
        }
        if self.underline {
            m.push("下線");
        }
        format!(
            "<officework.doc.Run {:?} {}pt{}>",
            self.text,
            self.size_pt,
            if m.is_empty() { String::new() } else { format!(" {}", m.join("・")) }
        )
    }
}

// ───────────────────────────────────────── 表

/// 表。`t[行][列]` でセルに届く。
#[pyclass(name = "Table", module = "officework.doc")]
struct PyTable {
    inner: Arc<Mutex<Inner>>,
    block: usize,
}

#[pymethods]
impl PyTable {
    /// 行の数。
    fn __len__(&self) -> PyResult<usize> {
        let g = lock(&self.inner)?;
        Ok(g.table(self.block).map(|t| t.rows.len()).unwrap_or(0))
    }

    /// 行を番号で。`t[1]` / `t[-1]`。
    fn __getitem__(&self, i: isize) -> PyResult<PyRow> {
        let g = lock(&self.inner)?;
        let t = g
            .table(self.block)
            .ok_or_else(|| PyIndexError::new_err("この表はもう文書に無い"))?;
        let k = resolve(i, t.rows.len(), "行は")?;
        Ok(PyRow { inner: Arc::clone(&self.inner), block: self.block, row: k })
    }

    /// 行の一覧。
    #[getter]
    fn rows(&self) -> PyResult<Vec<PyRow>> {
        let g = lock(&self.inner)?;
        let n = g.table(self.block).map(|t| t.rows.len()).unwrap_or(0);
        Ok((0..n)
            .map(|row| PyRow { inner: Arc::clone(&self.inner), block: self.block, row })
            .collect())
    }

    /// (行数, いちばん長い行の列数)。DataFrame の shape と同じ向き。
    /// 列数を行ごとに見ないのは、結合のある帳票では行によって数が違うため。
    #[getter]
    fn shape(&self) -> PyResult<(usize, usize)> {
        let g = lock(&self.inner)?;
        Ok(match g.table(self.block) {
            Some(t) => (t.rows.len(), t.rows.iter().map(|r| r.len()).max().unwrap_or(0)),
            None => (0, 0),
        })
    }

    /// 中身を list[list[str]] で(行ごと)。そのまま polars に渡せる。
    fn values(&self) -> PyResult<Vec<Vec<String>>> {
        let g = lock(&self.inner)?;
        Ok(match g.table(self.block) {
            Some(t) => t
                .rows
                .iter()
                .map(|r| r.iter().map(|c| kumihan::paras_text(&c.paragraphs)).collect())
                .collect(),
            None => Vec::new(),
        })
    }

    fn __repr__(&self) -> PyResult<String> {
        let (r, c) = self.shape()?;
        Ok(format!("<officework.doc.Table {r}行 × {c}列>"))
    }
}

/// 表の1行。
#[pyclass(name = "Row", module = "officework.doc")]
struct PyRow {
    inner: Arc<Mutex<Inner>>,
    block: usize,
    row: usize,
}

impl PyRow {
    fn len_of(&self, g: &Inner) -> usize {
        g.table(self.block).and_then(|t| t.rows.get(self.row)).map(|r| r.len()).unwrap_or(0)
    }
}

#[pymethods]
impl PyRow {
    /// 列の数。
    fn __len__(&self) -> PyResult<usize> {
        let g = lock(&self.inner)?;
        Ok(self.len_of(&g))
    }

    /// セルを番号で。`t[1][2]` / `row[-1]`。
    fn __getitem__(&self, i: isize) -> PyResult<PyCell> {
        let g = lock(&self.inner)?;
        let k = resolve(i, self.len_of(&g), "列は")?;
        Ok(PyCell {
            inner: Arc::clone(&self.inner),
            block: self.block,
            row: self.row,
            col: k,
        })
    }

    /// セルの一覧。
    #[getter]
    fn cells(&self) -> PyResult<Vec<PyCell>> {
        let g = lock(&self.inner)?;
        Ok((0..self.len_of(&g))
            .map(|col| PyCell {
                inner: Arc::clone(&self.inner),
                block: self.block,
                row: self.row,
                col,
            })
            .collect())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("<officework.doc.Row {}列>", self.__len__()?))
    }
}

/// 表のセル。中には段落が並んでいる。
#[pyclass(name = "Cell", module = "officework.doc")]
struct PyCell {
    inner: Arc<Mutex<Inner>>,
    block: usize,
    row: usize,
    col: usize,
}

impl PyCell {
    fn paras<T>(&self, f: impl FnOnce(&Vec<Paragraph>) -> T) -> PyResult<T> {
        let g = lock(&self.inner)?;
        let c = g
            .table(self.block)
            .and_then(|t| t.rows.get(self.row))
            .and_then(|r| r.get(self.col))
            .ok_or_else(|| PyIndexError::new_err("このセルはもう文書に無い"))?;
        Ok(f(&c.paragraphs))
    }
}

#[pymethods]
impl PyCell {
    /// セルの字(段落を改行で繋いだもの)。
    #[getter]
    fn text(&self) -> PyResult<String> {
        self.paras(|ps| kumihan::paras_text(ps))
    }

    /// セルの字を入れ替える。改行があれば段落が分かれる。
    /// 段落と同じ規則で、**同じ位置の段落の書式を継ぐ**
    /// (kumihan の `set_paras_text` — writer のセル編集と同じ)。
    #[setter]
    fn set_text(&self, text: &str) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        let cell = match g.doc.blocks.get_mut(self.block) {
            Some(Block::Table(t)) => {
                t.rows.get_mut(self.row).and_then(|r| r.get_mut(self.col))
            }
            _ => None,
        }
        .ok_or_else(|| PyIndexError::new_err("このセルはもう文書に無い"))?;
        kumihan::set_paras_text(&mut cell.paragraphs, text, DEFAULT_PT);
        Ok(())
    }

    /// セルの中の段落の一覧。
    #[getter]
    fn paragraphs(&self) -> PyResult<Vec<PyParagraph>> {
        let n = self.paras(|ps| ps.len())?;
        Ok((0..n)
            .map(|para| PyParagraph {
                inner: Arc::clone(&self.inner),
                loc: Loc::Cell {
                    block: self.block,
                    row: self.row,
                    col: self.col,
                    para,
                },
            })
            .collect())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("<officework.doc.Cell {:?}>", self.text()?))
    }
}

/// `officework._sheet` の副モジュールとして `doc` を建てる。
/// **1つの wheel に同居させる**ため(利用者に2つ入れさせない)。
/// Python から見える名前は `officework.doc`(officework/_doc.py が受ける)。
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "doc")?;
    m.add_class::<PyDoc>()?;
    m.add_class::<PyParagraph>()?;
    m.add_class::<PyRun>()?;
    m.add_class::<PyTable>()?;
    m.add_class::<PyRow>()?;
    m.add_class::<PyCell>()?;
    parent.add_submodule(&m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, bold: bool) -> Run {
        Run {
            text: text.to_string(),
            size_pt: 10.5,
            font: None,
            fmt: CharFormat { bold, ..Default::default() },
        }
    }

    #[test]
    fn 段落の字は_run_をつないだもの() {
        let p = Paragraph { runs: vec![run("請求先: ", false), run("株式会社甲", true)], ..Default::default() };
        assert_eq!(para_text(&p), "請求先: 株式会社甲");
    }

    #[test]
    fn text_の代入は先頭_run_の書式を継ぐ() {
        let mut p = Paragraph {
            runs: vec![run("見出し", true), run("つづき", false)],
            align: kumihan::Align::Center,
            ..Default::default()
        };
        set_para_text(&mut p, "差し替え");
        assert_eq!(p.runs.len(), 1, "run は1本にまとまる");
        assert_eq!(p.runs[0].text, "差し替え");
        assert!(p.runs[0].fmt.bold, "先頭 run の太字を継ぐ");
        assert_eq!(p.align, kumihan::Align::Center, "段落の性質は据え置き");
    }

    #[test]
    fn 空の段落へ代入すると既定の大きさになる() {
        let mut p = Paragraph::default();
        set_para_text(&mut p, "あ");
        assert_eq!(p.runs[0].size_pt, DEFAULT_PT);
    }

    #[test]
    fn replace_は_run_の切れ目を残す() {
        let mut runs = vec![run("請求先: ", false), run("旧社名", true), run(" 御中", false)];
        assert_eq!(replace_in_runs(&mut runs, "旧社名", "新社名"), 1);
        assert_eq!(runs.len(), 3, "run の数は変わらない");
        assert_eq!(runs[1].text, "新社名");
        assert!(runs[1].fmt.bold, "見つかった run の書式のまま");
        assert!(!runs[0].fmt.bold, "隣の run は動かない");
    }

    #[test]
    fn replace_は_run_を跨いだ字も拾う() {
        // 「旧社名」が 旧/社名 に割れている(Word が普通に作る形)
        let mut runs = vec![run("あ旧", false), run("社名い", true)];
        assert_eq!(replace_in_runs(&mut runs, "旧社名", "新社名"), 1);
        assert_eq!(runs[0].text, "あ新社名", "始まりを含む run が新しい字を持つ");
        assert_eq!(runs[1].text, "い", "続きの run からは掛かった分だけ落ちる");
    }

    #[test]
    fn replace_は_同じ字が何度出ても数える() {
        let mut runs = vec![run("甲と甲と甲", false)];
        assert_eq!(replace_in_runs(&mut runs, "甲", "乙"), 3);
        assert_eq!(runs[0].text, "乙と乙と乙");
    }

    #[test]
    fn replace_は_見つからなければ何もしない() {
        let mut runs = vec![run("あいう", false)];
        assert_eq!(replace_in_runs(&mut runs, "えお", "x"), 0);
        assert_eq!(runs[0].text, "あいう");
        // 空の needle で無限に回らないこと
        assert_eq!(replace_in_runs(&mut runs, "", "x"), 0);
    }

    #[test]
    fn replace_は_空の_run_があっても崩れない() {
        let mut runs = vec![run("", false), run("旧社名", false), run("", false)];
        assert_eq!(replace_in_runs(&mut runs, "旧社名", "新"), 1);
        assert_eq!(runs.iter().map(|r| r.text.as_str()).collect::<String>(), "新");
    }

    #[test]
    fn ページ番号の印は読める字になる() {
        // 私用領域の字をそのまま Python へ出さない
        let s = format!("- {} / {} -", kumihan::PAGE_MARK, kumihan::PAGES_MARK);
        assert_eq!(marks_to_text(&s), "- # / ## -");
        assert_eq!(marks_to_text("印の無い字"), "印の無い字");
    }

    #[test]
    fn 添字は負も引ける() {
        assert_eq!(resolve(-1, 3, "行は").unwrap(), 2);
        assert_eq!(resolve(0, 3, "行は").unwrap(), 0);
        assert!(resolve(3, 3, "行は").is_err());
        assert!(resolve(-4, 3, "行は").is_err());
    }
}
