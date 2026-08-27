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

use pyo3::exceptions::{PyIOError, PyIndexError, PyTypeError, PyValueError};
use pyo3::prelude::*;

use kumihan::{Block, CharFormat, Document, Paragraph, Run};

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

    /// 途中の節(sectPr を持つ段落)が blocks の何番目にいるか(順番どおり)。
    /// 文書末の節はこれに含まれない(Document::sect_raw / page が持つ)
    fn section_blocks(&self) -> Vec<usize> {
        self.doc
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| matches!(b, Block::Para(p) if p.sect.is_some()))
            .map(|(i, _)| i)
            .collect()
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

/// [`marks_to_text`] の逆。**`##` を先に見ます** — `#` から先に直すと
/// `##` が `#` 2つになり、総ページ数がページ番号2つに化けます。
fn text_to_marks(s: &str) -> String {
    s.replace("##", &kumihan::PAGES_MARK.to_string())
        .replace('#', &kumihan::PAGE_MARK.to_string())
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
        .unwrap_or((None, None, CharFormat::default()));
    p.runs = vec![Run { text: text.to_string(), size_pt: pt, font, fmt }];
}

/// run の並びから、名前つき記入欄のまとまりを拾う → (名前, 始まり, 終わり)。
/// 記入欄(w:sdt)は fmt.sdt を持つ**連続した run** で、同じ欄の run は
/// 同じ中身の Sdt を指す。名前(w:tag)が空の欄は「名前で引く」対象にならない
fn sdt_groups(runs: &[kumihan::Run]) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < runs.len() {
        let Some(sdt) = runs[i].fmt.sdt.clone() else {
            i += 1;
            continue;
        };
        let start = i;
        while i < runs.len() && runs[i].fmt.sdt.as_deref() == Some(sdt.as_ref()) {
            i += 1;
        }
        if !sdt.tag.is_empty() {
            out.push((sdt.tag.clone(), start, i));
        }
    }
    out
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

/// 空のセル(空の段落を1つ持つ)。docx のセルは段落無しでは立たないので、
/// 表を組むとき・行や列を足すときはこれを敷く。
fn empty_cell() -> kumihan::Cellbox {
    kumihan::Cellbox {
        paragraphs: vec![Paragraph { line_spacing: 1.0, ..Default::default() }],
        ..Default::default()
    }
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
        let mut doc = Document::default();
        // まっさらの文書が保存で持つスタイル定義と**一覧を一致させる** —
        // 書かれる物が見えない一覧は嘘になる。
        //
        // 出どころは同梱の既定のテンプレートです(2026-08-27)。前は
        // 4つ決め打ちで、実際に書かれる 23 個と食い違っていました
        doc.styles.push(kumihan::StyleInfo {
            id: "Normal".into(),
            name: "Normal".into(),
            kind: "paragraph".into(),
            look: Default::default(),
        });
        for d in &kumihan::theme::default_theme().styles {
            if d.name == "本文" {
                continue;
            }
            let (id, name) = ooxml::style_names(&d.name);
            doc.styles.push(kumihan::StyleInfo {
                id,
                name,
                kind: "paragraph".into(),
                look: Default::default(),
            });
        }
        PyDoc {
            inner: Arc::new(Mutex::new(Inner {
                doc,
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

    /// **雛形にデータを流し込む**(帳票。2026-08-17)。
    ///
    /// 記入欄に1つ書く [`fill`](Self::fill) とは別物です。あちらは欄1つ、
    /// こちらは雛形まるごとです。
    ///
    /// `{{member}}` を置き換え、`{{群.項目}}` を含む表の行はデータの数だけ
    /// 増やします。**この文書を書き換えます**(雛形を残したいときは、
    /// 先に別名で保存してください)。
    ///
    /// データに無い名前は `{{member}}` のまま残し、返り値で知らせます。
    /// 空にすると、金額の欄が空いた請求書が黙って出来上がるためです。
    ///
    /// ```python
    /// d = doc.Doc.open("請求書.docx")
    /// d.render({"宛名": "みほん商事", "合計": "3,000"},
    ///          {"明細": [{"品名": "鉛筆", "数量": "10"},
    ///                    {"品名": "消しゴム", "数量": "5"}]})
    /// d.save("out.docx")
    /// ```
    #[pyo3(signature = (values, rows = None))]
    fn render(
        &self,
        values: std::collections::BTreeMap<String, String>,
        rows: Option<std::collections::BTreeMap<String, Vec<std::collections::BTreeMap<String, String>>>>,
    ) -> PyResult<String> {
        let mut d = kumihan::fill::Data { values, rows: rows.unwrap_or_default() };
        // 数でも文字でも受けたいので、値はここまでで文字にしてもらう
        let _ = &mut d;
        let mut g = lock(&self.inner)?;
        let (out, rep) = kumihan::fill::fill(&g.doc, &d);
        g.doc = out;
        Ok(rep.summary())
    }

    /// 保存する。開いた元のファイルがあれば `ooxml::write_with` に渡し、
    /// **こちらが作り直さない部品は原本のまま持ち越す**(様式・図形・変更履歴・
    /// 読めなかった部品)。openpyxl / python-docx との違いはここ。
    fn save(&self, path: &str) -> PyResult<()> {
        let g = lock(&self.inner)?;
        // **`.pdf` なら紙にします**(2026-08-27 発注者「エンジンで pdf を
        // つくるところまで」)。新しい口は作りません — 拡張子で行き先が
        // 決まるのは、この階が前からやっていることです。
        //
        // openpyxl にも python-docx にも無い所です。本家は組版を持たないので
        // PDF を作れず、有料の別実装を買うか LibreOffice を裏で起こすしか
        // ありません
        if std::path::Path::new(path)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        {
            return ops::pdf::doc(&g.doc, None, std::path::Path::new(path))
                .map_err(|e| PyIOError::new_err(format!("{path}: PDF にできない: {e}")));
        }
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

    /// ヘッダーの字。ページ番号は `#`、総ページ数は `##` で書きます
    /// ([`marks_to_text`])。改行があれば段落が分かれます。
    #[getter]
    fn header(&self) -> PyResult<String> {
        Ok(marks_to_text(&kumihan::paras_text(&lock(&self.inner)?.doc.header.paragraphs)))
    }

    #[setter]
    fn set_header(&self, text: &str) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        let t = text_to_marks(text);
        kumihan::set_paras_text(&mut g.doc.header.paragraphs, &t);
        Ok(())
    }

    /// フッターの字。ヘッダーと同じくページ番号は `#`。
    #[getter]
    fn footer(&self) -> PyResult<String> {
        Ok(marks_to_text(&kumihan::paras_text(&lock(&self.inner)?.doc.footer.paragraphs)))
    }

    #[setter]
    fn set_footer(&self, text: &str) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        let t = text_to_marks(text);
        kumihan::set_paras_text(&mut g.doc.footer.paragraphs, &t);
        Ok(())
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

    /// 名前つき記入欄(コンテンツコントロール)の一覧 [(名前, いまの値)]。
    /// 名前は docx の w:tag — writer の「記入欄に名前を付ける」が付ける物で、
    /// writer マクロの fields() と同じ言葉。同じ名前が複数あればその数だけ並ぶ。
    /// 本文も表のセルの中も見る(帳票の記入欄はたいてい表の中にある)
    fn fields(&self) -> PyResult<Vec<(String, String)>> {
        let g = lock(&self.inner)?;
        let mut out = Vec::new();
        for loc in g.all_locs() {
            let Some(p) = g.para(&loc) else { continue };
            for (tag, s, e) in sdt_groups(&p.runs) {
                let text: String = p.runs[s..e].iter().map(|r| r.text.as_str()).collect();
                out.push((tag, text));
            }
        }
        Ok(out)
    }

    /// 名前の記入欄**すべて**に value を書く(writer マクロの fill と同じ言葉)。
    /// 返りは書いた欄の数(0 なら、その名前の欄が無い — 黙って成功にしない)。
    /// 書式は欄の先頭 run のまま
    fn fill(&self, name: &str, value: &str) -> PyResult<usize> {
        let mut g = lock(&self.inner)?;
        let locs = g.all_locs();
        let mut n = 0;
        for loc in locs {
            let Some(p) = g.para_mut(&loc) else { continue };
            let groups = sdt_groups(&p.runs);
            for (tag, s, e) in groups {
                if tag != name {
                    continue;
                }
                p.runs[s].text = value.to_string();
                for r in &mut p.runs[s + 1..e] {
                    r.text.clear();
                }
                n += 1;
            }
        }
        Ok(n)
    }

    /// 名前の記入欄の最初の一つの値。無ければ None(空文字と区別が付く)
    fn extract(&self, name: &str) -> PyResult<Option<String>> {
        let g = lock(&self.inner)?;
        for loc in g.all_locs() {
            let Some(p) = g.para(&loc) else { continue };
            for (tag, s, e) in sdt_groups(&p.runs) {
                if tag == name {
                    return Ok(Some(p.runs[s..e].iter().map(|r| r.text.as_str()).collect()));
                }
            }
        }
        Ok(None)
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

    /// スタイル定義の一覧 [(styleId, 名前, 種類)]。
    /// 種類は docx のまま: "paragraph" / "character" / "table" / "numbering"。
    /// 定義の本体は styles.xml が持ち、保存で原本のまま持ち越される —
    /// ここは名乗りの一覧(2026-08-12 発注者確定「持たない主義では無理」)。
    #[getter]
    fn styles(&self) -> PyResult<Vec<(String, String, String)>> {
        let g = lock(&self.inner)?;
        Ok(g.doc
            .styles
            .iter()
            .chain(g.doc.styles_new.iter())
            .map(|s| (s.id.clone(), s.name.clone(), s.kind.clone()))
            .collect())
    }

    /// スタイルを足す。styleId は名前から空白を抜いた形(Word の流儀)。
    /// 同じ物が居れば断る。
    ///
    /// **見た目も一緒に渡せます**(2026-08-27)。渡さなかった物は
    /// 「言わない」— 元になるスタイルから受け継ぎます。`False` を渡すと
    /// **わざわざ切る**ことになり、意味が違います。
    ///
    ///     d.add_style("注記", bold=True, color="9C2B2B", size=9)
    ///
    /// 後から変えるときは [`set_style_look`] を使います。
    #[pyo3(signature = (name, kind="paragraph", *, bold=None, italic=None,
                        underline=None, strike=None, size=None, color=None,
                        font=None, fill=None))]
    #[allow(clippy::too_many_arguments)]
    fn add_style(
        &self,
        name: &str,
        kind: &str,
        bold: Option<bool>,
        italic: Option<bool>,
        underline: Option<bool>,
        strike: Option<bool>,
        size: Option<f32>,
        color: Option<String>,
        font: Option<String>,
        fill: Option<String>,
    ) -> PyResult<()> {
        if name.is_empty() {
            return Err(PyValueError::new_err("スタイルの名前が空です"));
        }
        if !matches!(kind, "paragraph" | "character" | "table") {
            return Err(PyValueError::new_err(format!(
                "種類は paragraph / character / table: {kind:?}"
            )));
        }
        let id: String = name.chars().filter(|c| !c.is_whitespace()).collect();
        let mut g = lock(&self.inner)?;
        if g.doc
            .styles
            .iter()
            .chain(g.doc.styles_new.iter())
            .any(|s| s.id == id || s.name == name)
        {
            return Err(PyValueError::new_err(format!("スタイル「{name}」は既にあります")));
        }
        g.doc.styles_new.push(kumihan::StyleInfo {
            id,
            name: name.to_string(),
            kind: kind.to_string(),
            look: kumihan::StyleLook {
                bold, italic, underline, strike,
                size_pt: size, color, font, fill,
            },
        });
        Ok(())
    }

    /// **自作スタイルの見た目を変える。**
    ///
    /// 渡さなかった物はそのまま(消しません)。消したいときは
    /// `clear=True` を付けて、消したい物だけ渡し直します。
    ///
    ///     d.set_style_look("注記", size=10.5)
    ///
    /// **原本から読んだスタイルは変えられません。** 定義は据え置きで
    /// 持ち越す決めなので、触ると原本の様式が崩れます。
    #[pyo3(signature = (name, *, bold=None, italic=None, underline=None,
                        strike=None, size=None, color=None, font=None,
                        fill=None, clear=false))]
    #[allow(clippy::too_many_arguments)]
    fn set_style_look(
        &self,
        name: &str,
        bold: Option<bool>,
        italic: Option<bool>,
        underline: Option<bool>,
        strike: Option<bool>,
        size: Option<f32>,
        color: Option<String>,
        font: Option<String>,
        fill: Option<String>,
        clear: bool,
    ) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        if g.doc.styles.iter().any(|s| s.name == name || s.id == name) {
            return Err(PyValueError::new_err(format!(
                "「{name}」は原本のスタイルです。定義は原本のまま持ち越すので変えられません"
            )));
        }
        let Some(s) = g.doc.styles_new.iter_mut().find(|s| s.name == name || s.id == name) else {
            return Err(PyValueError::new_err(format!("スタイル「{name}」がありません")));
        };
        if clear {
            s.look = Default::default();
        }
        let l = &mut s.look;
        for (v, slot) in [
            (bold, &mut l.bold), (italic, &mut l.italic),
            (underline, &mut l.underline), (strike, &mut l.strike),
        ] {
            if v.is_some() {
                *slot = v;
            }
        }
        if size.is_some() {
            l.size_pt = size;
        }
        for (v, slot) in [(color, &mut l.color), (font, &mut l.font), (fill, &mut l.fill)] {
            if v.is_some() {
                *slot = v;
            }
        }
        Ok(())
    }

    /// スタイルの見た目を読む。無ければ `None`。
    /// 返るのは設定した物だけの辞書です。
    fn style_look(&self, py: Python<'_>, name: &str) -> PyResult<Option<Py<PyAny>>> {
        let g = lock(&self.inner)?;
        let Some(s) = g
            .doc
            .styles
            .iter()
            .chain(g.doc.styles_new.iter())
            .find(|s| s.name == name || s.id == name)
        else {
            return Ok(None);
        };
        let d = pyo3::types::PyDict::new(py);
        let l = &s.look;
        for (k, v) in [("bold", l.bold), ("italic", l.italic),
                       ("underline", l.underline), ("strike", l.strike)] {
            if let Some(x) = v {
                d.set_item(k, x)?;
            }
        }
        if let Some(x) = l.size_pt {
            d.set_item("size", x)?;
        }
        for (k, v) in [("color", &l.color), ("font", &l.font), ("fill", &l.fill)] {
            if let Some(x) = v {
                d.set_item(k, x)?;
            }
        }
        Ok(Some(d.into_any().unbind()))
    }

    /// 段落と表を**文書の順で**返す(python-docx の iter_inner_content)。
    /// paragraphs / tables は種類別 — 差し込み文書の「上から順に」はこちら。
    fn iter_inner_content(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let g = lock(&self.inner)?;
        let mut out: Vec<Py<PyAny>> = Vec::new();
        for (bi, b) in g.doc.blocks.iter().enumerate() {
            match b {
                Block::Para(_) => out.push(
                    Py::new(py, PyParagraph { inner: Arc::clone(&self.inner), loc: Loc::Body(bi) })?
                        .into_any(),
                ),
                Block::Table(_) => out.push(
                    Py::new(py, PyTable { inner: Arc::clone(&self.inner), block: bi })?.into_any(),
                ),
            }
        }
        Ok(out)
    }

    /// 文書の情報(docProps/core.xml)。python-docx の core_properties の役 —
    /// author(作成者)・title・keywords・subject・comments の読み書き。
    #[getter]
    fn core_properties(&self) -> PyCoreProps {
        PyCoreProps { inner: Arc::clone(&self.inner) }
    }

    /// 節を足す(python-docx の add_section と同じ切り方)。
    ///
    /// **切るのは末尾** — いままで書いた分が前の節になり、これから足す物が
    /// 新しい節に入る。中では、いまの文書末の節を「sectPr だけを持つ空の
    /// 段落」として本文の末尾に置く(これが docx の途中の節の書き方)。
    /// 文書末の節はそのまま残るので、新しい節は同じ紙と余白を継ぐ(本家も
    /// 設定を写す)— 変えたければ返ってきた節の紙・余白を書き替える。
    ///
    /// start_type は "new_page"(既定)と "continuous" だけ。新しい段・
    /// 偶数頁・奇数頁は模型に無いので**正直に断る** — 黙って改ページに
    /// 落とすと、刷ったときに別物になる。返りは新しい**文書末の節**。
    #[pyo3(signature = (start_type="new_page"))]
    fn add_section(&self, start_type: &str) -> PyResult<PySection> {
        let continuous =
            match start_type.to_ascii_lowercase().replace(['_', ' '], "").as_str() {
                "newpage" | "2" => false,
                "continuous" | "0" => true,
                other => {
                    return Err(PyValueError::new_err(format!(
                        "節の始め方は new_page か continuous だけ\
                         (新しい段・偶数頁・奇数頁は模型に無い): {other:?}"
                    )))
                }
            };
        let mut g = lock(&self.inner)?;
        let page = g.doc.page.unwrap_or_default();
        let raw = g
            .doc
            .sect_raw
            .clone()
            .unwrap_or_else(|| "<w:sectPr></w:sectPr>".to_string());
        let brk = kumihan::SectionBreak {
            raw: set_sect_continuous(&raw, continuous),
            page,
            continuous,
        };
        g.doc.blocks.push(Block::Para(Paragraph {
            line_spacing: 1.0,
            sect: Some(brk),
            ..Default::default()
        }));
        // 文書末の節は据え置き = 新しい節が同じ紙と余白を継ぐ(本家と同じ)
        g.doc.page = Some(page);
        g.doc.sect_raw = Some(raw);
        let idx = g.section_blocks().len(); // 末尾の節は途中の節の次
        Ok(PySection { inner: Arc::clone(&self.inner), idx })
    }

    /// 節(セクション)の一覧。途中の節(順)+ 文書末の節。
    /// 紙の大きさ・余白の読み書きは各節の手から — 書きは**原文の sectPr へ
    /// 属性差し替え**なので、理解しない設定(ヘッダー参照・段組み)は崩れない。
    #[getter]
    fn sections(&self) -> PyResult<Vec<PySection>> {
        let g = lock(&self.inner)?;
        let n = g.section_blocks().len() + 1; // +1 = 文書末の節
        Ok((0..n).map(|idx| PySection { inner: Arc::clone(&self.inner), idx }).collect())
    }

    /// 画像を足す(python-docx の add_picture の役)。径路でも bytes でも。
    /// 大きさは mm(省略は絵の実寸を 96dpi で mm に直した値。片方だけ
    /// 渡せば縦横比を保つ)。返りは画像を持つ段落。
    #[pyo3(signature = (image, width_mm=None, height_mm=None))]
    fn add_picture(
        &self,
        image: &Bound<'_, PyAny>,
        width_mm: Option<f32>,
        height_mm: Option<f32>,
    ) -> PyResult<PyParagraph> {
        let data: Vec<u8> = if let Ok(b) = image.extract::<Vec<u8>>() {
            b
        } else if let Ok(p) = image.extract::<String>() {
            std::fs::read(&p).map_err(|e| PyIOError::new_err(format!("{p}: 読めない: {e}")))?
        } else {
            return Err(PyTypeError::new_err(
                "画像は 径路の文字列 か bytes(PNG / JPEG)で渡してください",
            ));
        };
        let (wpx, hpx) = ops::image_px(&data).ok_or_else(|| {
            PyValueError::new_err("PNG / JPEG として読めない(大きさが測れない)")
        })?;
        // 実寸(96dpi)を既定に、渡された辺へ縦横比を保って合わせる
        let (w0, h0) = (wpx as f32 * 25.4 / 96.0, hpx as f32 * 25.4 / 96.0);
        let (w_mm, h_mm) = match (width_mm, height_mm) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, w * h0 / w0),
            (None, Some(h)) => (h * w0 / h0, h),
            (None, None) => (w0, h0),
        };
        let mut g = lock(&self.inner)?;
        let mut p = Paragraph { line_spacing: 1.0, ..Default::default() };
        p.images_new.push(kumihan::InlineImage {
            bytes: std::sync::Arc::new(data),
            w_mm,
            h_mm,
            tex: None, // python-docx の add_picture。数式は別の口
            src: None, // ネイティブ文書の相対の径路。ここは中身を直に持つ
        });
        g.doc.blocks.push(Block::Para(p));
        let b = g.doc.blocks.len() - 1;
        Ok(PyParagraph { inner: Arc::clone(&self.inner), loc: Loc::Body(b) })
    }

    /// 改ページを足す(python-docx の add_page_break の役)。
    /// 本家は「改ページの run を持つ段落」を足すが、うちの模型は改ページを
    /// **段落の性質(page_break_before)**で持つ — 空の段落に印を付けて返す。
    /// 紙の上の意味(ここで頁が変わる)は同じで、本家の
    /// paragraph_format.page_break_before でもそのまま読める。
    fn add_page_break(&self) -> PyResult<PyParagraph> {
        let mut g = lock(&self.inner)?;
        let p = Paragraph { line_spacing: 1.0, page_break_before: true, ..Default::default() };
        g.doc.blocks.push(Block::Para(p));
        let b = g.doc.blocks.len() - 1;
        Ok(PyParagraph { inner: Arc::clone(&self.inner), loc: Loc::Body(b) })
    }

    /// 本文の末尾に見出しを足す(python-docx の add_heading の役)。
    /// level は 1〜3 — この模型の見出しは3段まで(スタイル定義は持たない主義)。
    /// level=0(Title)は無い物なので正直に断る。
    #[pyo3(signature = (text="", level=1))]
    fn add_heading(&self, text: &str, level: u8) -> PyResult<PyParagraph> {
        if !(1..=3).contains(&level) {
            return Err(PyValueError::new_err(format!(
                "見出しは 1〜3(この模型の見出しは3段まで。0=Title は持たない): {level}"
            )));
        }
        let mut g = lock(&self.inner)?;
        let mut p = Paragraph {
            line_spacing: 1.0,
            style: kumihan::ParaStyle::Heading(level),
            ..Default::default()
        };
        set_para_text(&mut p, text);
        g.doc.blocks.push(Block::Para(p));
        let b = g.doc.blocks.len() - 1;
        Ok(PyParagraph { inner: Arc::clone(&self.inner), loc: Loc::Body(b) })
    }

    /// 本文の末尾に表を組む(rows × cols)。各セルは空の段落を1つ持つ
    /// (docx のセルは段落無しでは立たない)。列の幅は等分。
    fn add_table(&self, rows: usize, cols: usize) -> PyResult<PyTable> {
        if rows == 0 || cols == 0 {
            return Err(PyValueError::new_err("表は1行1列から"));
        }
        let mut g = lock(&self.inner)?;
        let t = kumihan::Table {
            rows: (0..rows).map(|_| (0..cols).map(|_| empty_cell()).collect()).collect(),
            col_mm: Vec::new(),
            ..Default::default()
        };
        g.doc.blocks.push(Block::Table(t));
        let block = g.doc.blocks.len() - 1;
        Ok(PyTable { inner: Arc::clone(&self.inner), block })
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

// ───────────────────────────────────────── 節(セクション)

/// mm → docx の twips(1/20 pt)。sectPr の原文の属性がこの単位
fn mm_twips(mm: f32) -> i64 {
    (mm as f64 / 25.4 * 1440.0).round() as i64
}

/// sectPr の原文の属性を差し替える(要素・属性が無ければ足す)。
/// **原文を正として、変えた属性だけ触る** — 理解しない属性はそのまま残る
/// **字の属性**を差し替える(`w:orient="landscape"` など)。
/// 数の版と分けてあるのは、数に直せない値があるためです。
fn patch_sect_text(raw: &str, tag: &str, attr: &str, val: &str) -> String {
    let open = format!("<{tag}");
    let Some(i) = raw.find(&open) else { return raw.into() };
    let Some(end) = raw[i..].find("/>").map(|g| i + g) else { return raw.into() };
    let pat = format!("{attr}=\"");
    let mut out = raw.to_string();
    match raw[i..end].find(&pat) {
        Some(k) => {
            let a = i + k + pat.len();
            let Some(b) = raw[a..end].find('"').map(|g| a + g) else { return raw.into() };
            out.replace_range(a..b, val);
        }
        None => out.insert_str(end, &format!(" {attr}=\"{val}\"")),
    }
    out
}

fn patch_sect(raw: &str, tag: &str, attr: &str, val: i64) -> String {
    let open = format!("<{tag}");
    if let Some(i) = raw.find(&open) {
        let Some(end) = raw[i..].find("/>").map(|g| i + g) else { return raw.into() };
        let pat = format!("{attr}=\"");
        if let Some(a) = raw[i..end].find(&pat) {
            let vs = i + a + pat.len();
            let Some(ve) = raw[vs..].find('"') else { return raw.into() };
            format!("{}{}{}", &raw[..vs], val, &raw[vs + ve..])
        } else {
            format!("{} {attr}=\"{}\"{}", &raw[..end], val, &raw[end..])
        }
    } else if let Some(c) = raw.rfind("</w:sectPr>") {
        format!("{}<{tag} {attr}=\"{}\"/>{}", &raw[..c], val, &raw[c..])
    } else {
        raw.into()
    }
}

/// sectPr の原文に「続き」の印(`<w:type w:val="continuous"/>`)を置く・外す。
/// **改ページする節が docx の既定**なので、印は続きのときだけ書く
/// (スキーマでは sectPr の頭のほう — pgSz より前に来る)。
fn set_sect_continuous(raw: &str, continuous: bool) -> String {
    // 既にある w:type は落としてから置き直す
    let mut s = raw.to_string();
    while let Some(i) = s.find("<w:type") {
        match s[i..].find("/>") {
            Some(j) => s.replace_range(i..i + j + 2, ""),
            None => break,
        }
    }
    if !continuous {
        return s;
    }
    let ins = r#"<w:type w:val="continuous"/>"#;
    match s.find('>') {
        Some(i) if s.starts_with("<w:sectPr") => {
            s.insert_str(i + 1, ins);
            s
        }
        _ => format!("<w:sectPr>{ins}</w:sectPr>"),
    }
}

/// 1つの節。idx が途中の節(sectPr を持つ段落の順)の数までなら途中、
/// その次が文書末の節。位置で引き直す手(段落・run と同じ作法)。
#[pyclass(name = "Section", module = "officework.doc")]
struct PySection {
    inner: Arc<Mutex<Inner>>,
    idx: usize,
}

impl PySection {
    /// (節の PageSetup を読む)。途中の節は段落の sect、最後は文書の page
    fn page(&self) -> PyResult<kumihan::PageSetup> {
        let g = lock(&self.inner)?;
        let mids = g.section_blocks();
        if self.idx < mids.len() {
            match g.doc.blocks.get(mids[self.idx]) {
                Some(Block::Para(p)) => {
                    Ok(p.sect.as_ref().map(|s| s.page).unwrap_or_default())
                }
                _ => Err(PyIndexError::new_err("この節はもう文書に無い")),
            }
        } else {
            Ok(g.doc.page.unwrap_or_default())
        }
    }

    /// 節の PageSetup と sectPr の原文を一緒に書き替える
    fn mutate(&self, f: impl FnOnce(&mut kumihan::PageSetup, &mut String)) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        let mids = g.section_blocks();
        if self.idx < mids.len() {
            let bi = mids[self.idx];
            match g.doc.blocks.get_mut(bi) {
                Some(Block::Para(p)) => match p.sect.as_mut() {
                    Some(s) => {
                        f(&mut s.page, &mut s.raw);
                        Ok(())
                    }
                    None => Err(PyIndexError::new_err("この節はもう文書に無い")),
                },
                _ => Err(PyIndexError::new_err("この節はもう文書に無い")),
            }
        } else {
            let mut page = g.doc.page.unwrap_or_default();
            let mut raw = g
                .doc
                .sect_raw
                .clone()
                .unwrap_or_else(|| "<w:sectPr></w:sectPr>".to_string());
            f(&mut page, &mut raw);
            g.doc.page = Some(page);
            g.doc.sect_raw = Some(raw);
            Ok(())
        }
    }
}

#[pymethods]
impl PySection {
    #[getter]
    fn page_width_mm(&self) -> PyResult<f32> {
        Ok(self.page()?.w_mm)
    }

    #[setter]
    fn set_page_width_mm(&self, v: f32) -> PyResult<()> {
        self.mutate(|p, raw| {
            p.w_mm = v;
            *raw = patch_sect(raw, "w:pgSz", "w:w", mm_twips(v));
        })
    }

    #[getter]
    fn page_height_mm(&self) -> PyResult<f32> {
        Ok(self.page()?.h_mm)
    }

    #[setter]
    fn set_page_height_mm(&self, v: f32) -> PyResult<()> {
        self.mutate(|p, raw| {
            p.h_mm = v;
            *raw = patch_sect(raw, "w:pgSz", "w:h", mm_twips(v));
        })
    }

    /// **紙の向きを変える**(2026-08-27。台帳の追補)。
    /// `"portrait"` / `"landscape"`。
    ///
    /// 読むのは動いていましたが、**変える口がありません**でした。
    /// 幅と高さを1つずつ入れ替えると、途中で正方形になって
    /// 「どちらの向きか」が決まらない瞬間ができます。1手で入れ替えます。
    #[setter]
    fn set_orientation(&self, value: &str) -> PyResult<()> {
        let want_landscape = match value {
            "landscape" | "横" => true,
            "portrait" | "縦" => false,
            other => {
                return Err(PyValueError::new_err(format!(
                    "向きは portrait / landscape: {other:?}"
                )))
            }
        };
        let p0 = self.page()?;
        if (p0.w_mm > p0.h_mm) == want_landscape {
            return Ok(());
        }
        let (w, h) = (p0.h_mm, p0.w_mm);
        self.mutate(|p, raw| {
            p.w_mm = w;
            p.h_mm = h;
            *raw = patch_sect(raw, "w:pgSz", "w:w", mm_twips(w));
            *raw = patch_sect(raw, "w:pgSz", "w:h", mm_twips(h));
            // **本家は向きの札も持ちます。** 大きさだけ入れ替えて札を
            // 置いたままにすると、Word の「ページ設定」が食い違います
            *raw = patch_sect_text(
                raw,
                "w:pgSz",
                "w:orient",
                if want_landscape { "landscape" } else { "portrait" },
            );
        })
    }

    #[getter]
    fn left_margin_mm(&self) -> PyResult<f32> {
        Ok(self.page()?.left_mm)
    }

    #[setter]
    fn set_left_margin_mm(&self, v: f32) -> PyResult<()> {
        self.mutate(|p, raw| {
            p.left_mm = v;
            *raw = patch_sect(raw, "w:pgMar", "w:left", mm_twips(v));
        })
    }

    #[getter]
    fn right_margin_mm(&self) -> PyResult<f32> {
        Ok(self.page()?.right_mm)
    }

    #[setter]
    fn set_right_margin_mm(&self, v: f32) -> PyResult<()> {
        self.mutate(|p, raw| {
            p.right_mm = v;
            *raw = patch_sect(raw, "w:pgMar", "w:right", mm_twips(v));
        })
    }

    #[getter]
    fn top_margin_mm(&self) -> PyResult<f32> {
        Ok(self.page()?.top_mm)
    }

    #[setter]
    fn set_top_margin_mm(&self, v: f32) -> PyResult<()> {
        self.mutate(|p, raw| {
            p.top_mm = v;
            *raw = patch_sect(raw, "w:pgMar", "w:top", mm_twips(v));
        })
    }

    #[getter]
    fn bottom_margin_mm(&self) -> PyResult<f32> {
        Ok(self.page()?.bottom_mm)
    }

    #[setter]
    fn set_bottom_margin_mm(&self, v: f32) -> PyResult<()> {
        self.mutate(|p, raw| {
            p.bottom_mm = v;
            *raw = patch_sect(raw, "w:pgMar", "w:bottom", mm_twips(v));
        })
    }

    /// 向き。"portrait" か "landscape"(紙の幅と高さから見る)。
    #[getter]
    fn orientation(&self) -> PyResult<&'static str> {
        let p = self.page()?;
        Ok(if p.w_mm > p.h_mm { "landscape" } else { "portrait" })
    }

    fn __repr__(&self) -> PyResult<String> {
        let p = self.page()?;
        Ok(format!("<officework.doc.Section {:.0}×{:.0}mm>", p.w_mm, p.h_mm))
    }
}

// ───────────────────────────────────────── 文書の情報

/// 文書の情報(docProps/core.xml)。python-docx の core_properties の形。
/// 呼び名は本家に合わせる: author = docx の dc:creator、comments = 説明欄。
#[pyclass(name = "CoreProperties", module = "officework.doc")]
struct PyCoreProps {
    inner: Arc<Mutex<Inner>>,
}

#[pymethods]
impl PyCoreProps {
    #[getter]
    fn author(&self) -> PyResult<String> {
        Ok(lock(&self.inner)?.doc.props.creator.clone())
    }
    #[setter]
    fn set_author(&self, value: &str) -> PyResult<()> {
        lock(&self.inner)?.doc.props.creator = value.to_string();
        Ok(())
    }
    #[getter]
    fn title(&self) -> PyResult<String> {
        Ok(lock(&self.inner)?.doc.props.title.clone())
    }
    #[setter]
    fn set_title(&self, value: &str) -> PyResult<()> {
        lock(&self.inner)?.doc.props.title = value.to_string();
        Ok(())
    }
    #[getter]
    fn keywords(&self) -> PyResult<String> {
        Ok(lock(&self.inner)?.doc.props.keywords.clone())
    }
    #[setter]
    fn set_keywords(&self, value: &str) -> PyResult<()> {
        lock(&self.inner)?.doc.props.keywords = value.to_string();
        Ok(())
    }
    #[getter]
    fn subject(&self) -> PyResult<String> {
        Ok(lock(&self.inner)?.doc.props.subject.clone())
    }
    #[setter]
    fn set_subject(&self, value: &str) -> PyResult<()> {
        lock(&self.inner)?.doc.props.subject = value.to_string();
        Ok(())
    }
    #[getter]
    fn comments(&self) -> PyResult<String> {
        Ok(lock(&self.inner)?.doc.props.description.clone())
    }
    #[setter]
    fn set_comments(&self, value: &str) -> PyResult<()> {
        lock(&self.inner)?.doc.props.description = value.to_string();
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let g = lock(&self.inner)?;
        Ok(format!(
            "<officework.doc.CoreProperties {:?} by {:?}>",
            g.doc.props.title, g.doc.props.creator
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

    /// この段落の**前**に段落を差す(python-docx と同じ口。add_paragraph は
    /// 末尾だけ)。手元の段落・run の札は**位置**で指しているので、
    /// 差した後は引き直すこと(シートの札と同じ作法)。
    #[pyo3(signature = (text=""))]
    fn insert_paragraph_before(&self, text: &str) -> PyResult<PyParagraph> {
        let mut g = lock(&self.inner)?;
        let mut p = Paragraph { line_spacing: 1.0, ..Default::default() };
        set_para_text(&mut p, text);
        match self.loc {
            Loc::Body(b) => {
                if b > g.doc.blocks.len() {
                    return Err(PyIndexError::new_err("この段落はもう文書に無い"));
                }
                g.doc.blocks.insert(b, Block::Para(p));
                Ok(PyParagraph { inner: Arc::clone(&self.inner), loc: Loc::Body(b) })
            }
            Loc::Cell { block, row, col, para } => {
                let cell = match g.doc.blocks.get_mut(block) {
                    Some(Block::Table(t)) => {
                        t.rows.get_mut(row).and_then(|r| r.get_mut(col))
                    }
                    _ => None,
                }
                .ok_or_else(|| PyIndexError::new_err("このセルはもう文書に無い"))?;
                if para > cell.paragraphs.len() {
                    return Err(PyIndexError::new_err("この段落はもうセルに無い"));
                }
                cell.paragraphs.insert(para, p);
                Ok(PyParagraph {
                    inner: Arc::clone(&self.inner),
                    loc: Loc::Cell { block, row, col, para },
                })
            }
        }
    }

    /// 書式のまとまり(run)の一覧。位置で引き直す**手**(handle)—
    /// 読みも書きもここから(python-docx の run と同じ使い方)。
    #[getter]
    fn runs(&self) -> PyResult<Vec<PyRun>> {
        let n = self.with(|p| p.runs.len())?;
        Ok((0..n)
            .map(|idx| PyRun { inner: Arc::clone(&self.inner), loc: self.loc.clone(), idx })
            .collect())
    }

    /// この段落のリンク [(字, URL)]。python-docx の hyperlinks と同じ役。
    #[getter]
    fn hyperlinks(&self) -> PyResult<Vec<(String, String)>> {
        self.with(|p| {
            p.runs
                .iter()
                .filter_map(|r| r.fmt.link.clone().map(|u| (r.text.clone(), u)))
                .collect()
        })
    }

    /// 段落の末尾にリンクを足す(python-docx の add_hyperlink と同じ役)。
    /// 書式は末尾の run を継ぐ。返りは足した run。
    #[pyo3(signature = (text, address))]
    fn add_hyperlink(&self, text: &str, address: &str) -> PyResult<PyRun> {
        let idx = self.with_mut(|p| {
            let (pt, font, mut fmt) = p
                .runs
                .last()
                .map(|r| (r.size_pt, r.font.clone(), r.fmt.clone()))
                .unwrap_or((None, None, CharFormat::default()));
            fmt.link = Some(address.to_string());
            p.runs.push(Run { text: text.to_string(), size_pt: pt, font, fmt });
            p.runs.len() - 1
        })?;
        Ok(PyRun { inner: Arc::clone(&self.inner), loc: self.loc.clone(), idx })
    }

    /// 段落の末尾に run を継ぎ足す(python-docx の add_run)。
    /// 書式は**末尾の run のものを継ぐ**(text の代入が先頭を継ぐのと対 —
    /// 続きを書くなら続きの書式)。段落が空なら既定の大きさの素の run。
    #[pyo3(signature = (text=""))]
    fn add_run(&self, text: &str) -> PyResult<PyRun> {
        let idx = self.with_mut(|p| {
            let (pt, font, mut fmt) = p
                .runs
                .last()
                .map(|r| (r.size_pt, r.font.clone(), r.fmt.clone()))
                .unwrap_or((None, None, CharFormat::default()));
            // **リンクは継がない** — 掛かりを決めるのは囲み(w:hyperlink)で、
            // 字の書式ではない。継ぐと、リンクの隣に足した字まで青くなる
            fmt.link = None;
            p.runs.push(Run { text: text.to_string(), size_pt: pt, font, fmt });
            p.runs.len() - 1
        })?;
        Ok(PyRun { inner: Arc::clone(&self.inner), loc: self.loc.clone(), idx })
    }

    /// 段落の役目かスタイル。役目を知る物は "body" / "heading1"〜 / "toc1" 等、
    /// それ以外のスタイルは**名前**(styles.xml の名乗り。無ければ styleId)。
    #[getter]
    fn style(&self) -> PyResult<String> {
        let g = lock(&self.inner)?;
        let p = g.para(&self.loc).ok_or_else(|| {
            PyIndexError::new_err("この段落はもう文書に無い(文書の形が変わった)")
        })?;
        Ok(match p.style {
            kumihan::ParaStyle::Body => match &p.style_id {
                Some(id) => g
                    .doc
                    .styles
                    .iter()
                    .chain(g.doc.styles_new.iter())
                    .find(|s| s.id == *id)
                    .map(|s| if s.name.is_empty() { s.id.clone() } else { s.name.clone() })
                    .unwrap_or_else(|| id.clone()),
                None => "body".to_string(),
            },
            kumihan::ParaStyle::Title => "title".to_string(),
            kumihan::ParaStyle::Heading(n) => format!("heading{n}"),
            kumihan::ParaStyle::Toc(n) => format!("toc{n}"),
            kumihan::ParaStyle::Tof => "tof".to_string(),
            // 引用(AsciiDoc の `____`)。**下の set_style が受ける綴りと
            // 同じ物を返す** — 読んで書き戻したら元に戻ること
            kumihan::ParaStyle::Quote => "quote".to_string(),
        })
    }

    /// 段落の役目かスタイルを替える。"body"("Normal")と "heading1"〜3
    /// ("Heading 1"・"見出し 1" でも)は役目として持ち、それ以外は
    /// **styles にある段落スタイルの名前**を受ける(2026-08-12 発注者確定 —
    /// 無い名前は add_style で作ってから。黙って作らない)。
    #[setter]
    fn set_style(&self, value: &str) -> PyResult<()> {
        let v = value.to_ascii_lowercase().replace(' ', "");
        let role = match v.as_str() {
            "body" | "normal" | "標準" => Some(kumihan::ParaStyle::Body),
            "heading1" | "見出し1" => Some(kumihan::ParaStyle::Heading(1)),
            "heading2" | "見出し2" => Some(kumihan::ParaStyle::Heading(2)),
            "heading3" | "見出し3" => Some(kumihan::ParaStyle::Heading(3)),
            "quote" | "引用" => Some(kumihan::ParaStyle::Quote),
            _ => None,
        };
        let mut g = lock(&self.inner)?;
        let (style, style_id) = match role {
            Some(r) => (r, None),
            None => {
                let found = g
                    .doc
                    .styles
                    .iter()
                    .chain(g.doc.styles_new.iter())
                    .find(|s| {
                        (s.name == value || s.id == value)
                            && (s.kind == "paragraph" || s.kind.is_empty())
                    })
                    .map(|s| s.id.clone());
                // **Word が使ったときに作る組み込みスタイル**なら、ここで
                // 作ります(List Bullet など)。Word と同じ振る舞いです
                let found = found.or_else(|| {
                    kumihan::latent_style(value).map(|(id, name)| {
                        g.doc.styles_new.push(kumihan::StyleInfo {
                            id: id.to_string(),
                            name: name.to_string(),
                            kind: "paragraph".into(),
                            ..Default::default()
                        });
                        id.to_string()
                    })
                });
                match found {
                    Some(id) => (kumihan::ParaStyle::Body, Some(id)),
                    None => {
                        return Err(PyValueError::new_err(format!(
                            "スタイル「{value}」が styles に無い(add_style で作ってから)"
                        )))
                    }
                }
            }
        };
        let p = g.para_mut(&self.loc).ok_or_else(|| {
            PyIndexError::new_err("この段落はもう文書に無い(文書の形が変わった)")
        })?;
        p.style = style;
        p.style_id = style_id;
        Ok(())
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

    #[setter]
    fn set_align(&self, value: &str) -> PyResult<()> {
        let a = match value {
            "left" => kumihan::Align::Left,
            "center" => kumihan::Align::Center,
            "right" => kumihan::Align::Right,
            "justify" => kumihan::Align::Justify,
            "distribute" => kumihan::Align::Distribute,
            _ => {
                return Err(PyValueError::new_err(format!(
                    "寄せは left / center / right / justify / distribute: {value:?}"
                )))
            }
        };
        self.with_mut(|p| p.align = a)
    }

    /// 行間の倍率(1.0 が既定)。docx の w:spacing lineRule="auto" と対。
    #[getter]
    fn line_spacing(&self) -> PyResult<f32> {
        self.with(|p| p.spacing())
    }

    #[setter]
    fn set_line_spacing(&self, value: f32) -> PyResult<()> {
        if !(0.5..=5.0).contains(&value) {
            return Err(PyValueError::new_err(format!(
                "行間の倍率は 0.5〜5.0(それ以外は読みにくさの事故): {value}"
            )));
        }
        self.with_mut(|p| p.line_spacing = value)
    }

    /// **1行目の字下げ(pt)。** 正で字下げ、負でぶら下げです。
    ///
    /// 日本の書類は本文の1行目を全角1字ぶん下げます(10.5pt の字なら
    /// 10.5pt)。模型は docx と同じ twip で持っているので、ここで直します。
    #[getter]
    fn first_line_indent(&self) -> PyResult<f32> {
        self.with(|p| p.first_line_twips as f32 / 20.0)
    }

    #[setter]
    fn set_first_line_indent(&self, value: f32) -> PyResult<()> {
        if !(-200.0..=200.0).contains(&value) {
            return Err(PyValueError::new_err(format!(
                "1行目の字下げは -200〜200pt: {value}"
            )));
        }
        self.with_mut(|p| p.first_line_twips = (value * 20.0).round() as i32)
    }

    /// **左のインデント(段数)。** 1段 = 全角2字ぶんです。
    ///
    /// docx は twip で持ちますが、模型は段数です(日本の書類の慣習)。
    /// pt との対応は本文の字の大きさで変わるので、ここは段数のまま渡します。
    #[getter]
    fn indent_level(&self) -> PyResult<u32> {
        self.with(|p| p.indent as u32)
    }

    #[setter]
    fn set_indent_level(&self, value: u32) -> PyResult<()> {
        if value > 9 {
            return Err(PyValueError::new_err(format!("インデントは 0〜9 段: {value}")));
        }
        self.with_mut(|p| p.indent = value as u8)
    }

    /// **段落の前の空き(pt)**(台帳 #5。2026-08-27)。
    ///
    /// 開催通知のような1枚物で、見出しと本文の間を空けるのに要ります。
    #[getter]
    fn space_before(&self) -> PyResult<f32> {
        self.with(|p| p.space_before_pt)
    }

    #[setter]
    fn set_space_before(&self, value: f32) -> PyResult<()> {
        if !(0.0..=200.0).contains(&value) {
            return Err(PyValueError::new_err(format!("段落の前の空きは 0〜200pt: {value}")));
        }
        self.with_mut(|p| p.space_before_pt = value)
    }

    /// **段落の後ろの空き(pt)**(台帳 #5)。
    #[getter]
    fn space_after(&self) -> PyResult<f32> {
        self.with(|p| p.space_after_pt)
    }

    #[setter]
    fn set_space_after(&self, value: f32) -> PyResult<()> {
        if !(0.0..=200.0).contains(&value) {
            return Err(PyValueError::new_err(format!("段落の後ろの空きは 0〜200pt: {value}")));
        }
        self.with_mut(|p| p.space_after_pt = value)
    }

    /// この段落の前で改ページする(docx の w:pageBreakBefore)。
    #[getter]
    fn page_break_before(&self) -> PyResult<bool> {
        self.with(|p| p.page_break_before)
    }

    #[setter]
    fn set_page_break_before(&self, value: bool) -> PyResult<()> {
        self.with_mut(|p| p.page_break_before = value)
    }

    /// 表のセルの中の段落なら True。
    #[getter]
    fn in_table(&self) -> bool {
        matches!(self.loc, Loc::Cell { .. })
    }

    /// この段落の画像 [(幅mm, 高さmm)]。開いた文書にあった物と
    /// add_picture で足した物の両方が見える(sheet の images と同じ形)。
    #[getter]
    fn images(&self) -> PyResult<Vec<(f32, f32)>> {
        self.with(|p| {
            p.images
                .iter()
                .chain(p.images_new.iter())
                .map(|im| (im.w_mm, im.h_mm))
                .collect()
        })
    }

    /// この段落に付いたコメント [(書いた人, 中身)]。
    /// **段落単位**で持つ(文中の範囲は持たない — 模型の粒度)。
    #[getter]
    fn comments(&self) -> PyResult<Vec<(String, String)>> {
        self.with(|p| p.comments.iter().map(|c| (c.author.clone(), c.text.clone())).collect())
    }

    /// この段落にコメントを付ける。保存で comments.xml に入る。
    #[pyo3(signature = (text, author=""))]
    fn add_comment(&self, text: &str, author: &str) -> PyResult<()> {
        self.with_mut(|p| {
            p.comments.push(kumihan::Comment {
                author: author.to_string(),
                text: text.to_string(),
            })
        })
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

/// 書式のまとまり。**位置(段落+何番目)で引き直す手**(handle)。
/// 当初は凍った写しだったが、run 単位の書き(add_text / clear / bold の
/// 代入)のために手に変えた(2026-08-12 夜)。段落の text の代入や
/// replace で run の並びが変わった後は、runs から引き直すこと。
#[pyclass(name = "Run", module = "officework.doc")]
struct PyRun {
    inner: Arc<Mutex<Inner>>,
    loc: Loc,
    idx: usize,
}

impl PyRun {
    fn with<T>(&self, f: impl FnOnce(&Run) -> T) -> PyResult<T> {
        let g = lock(&self.inner)?;
        let r = g
            .para(&self.loc)
            .and_then(|p| p.runs.get(self.idx))
            .ok_or_else(|| {
                PyIndexError::new_err("この run はもう文書に無い(段落の形が変わった)")
            })?;
        Ok(f(r))
    }

    fn with_mut<T>(&self, f: impl FnOnce(&mut Run) -> T) -> PyResult<T> {
        let mut g = lock(&self.inner)?;
        let p = g.para_mut(&self.loc).ok_or_else(|| {
            PyIndexError::new_err("この段落はもう文書に無い(文書の形が変わった)")
        })?;
        let r = p.runs.get_mut(self.idx).ok_or_else(|| {
            PyIndexError::new_err("この run はもう文書に無い(段落の形が変わった)")
        })?;
        Ok(f(r))
    }
}

#[pymethods]
impl PyRun {
    #[getter]
    fn text(&self) -> PyResult<String> {
        self.with(|r| r.text.clone())
    }

    #[setter]
    fn set_text(&self, value: &str) -> PyResult<()> {
        self.with_mut(|r| r.text = value.to_string())
    }

    /// 字の大きさ(pt)。**None は「指定なし」**(文書の既定に従う)—
    /// 本家 python-docx の font.size が None を返すのと同じ約束。
    /// 以前はここで 10.5 を作って返していた(往復で焼き付く穴の一部)
    #[getter]
    fn size_pt(&self) -> PyResult<Option<f32>> {
        self.with(|r| r.size_pt)
    }

    /// None を入れると指定を外す(文書の既定に従う字に戻る)
    #[setter]
    fn set_size_pt(&self, value: Option<f32>) -> PyResult<()> {
        if let Some(v) = value {
            if !(1.0..=400.0).contains(&v) {
                return Err(PyValueError::new_err(format!("文字の大きさ(pt)が変: {v}")));
            }
        }
        self.with_mut(|r| r.size_pt = value)
    }

    #[getter]
    fn font(&self) -> PyResult<Option<String>> {
        self.with(|r| r.font.clone())
    }

    #[setter]
    fn set_font(&self, value: Option<String>) -> PyResult<()> {
        self.with_mut(|r| r.font = value.filter(|v| !v.is_empty()))
    }

    #[getter]
    fn bold(&self) -> PyResult<bool> {
        self.with(|r| r.fmt.bold)
    }

    #[setter]
    fn set_bold(&self, value: bool) -> PyResult<()> {
        self.with_mut(|r| r.fmt.bold = value)
    }

    #[getter]
    fn italic(&self) -> PyResult<bool> {
        self.with(|r| r.fmt.italic)
    }

    #[setter]
    fn set_italic(&self, value: bool) -> PyResult<()> {
        self.with_mut(|r| r.fmt.italic = value)
    }

    /// **蛍光ペン**(台帳 #9)。色の名前(`yellow` `green` …)か、無ければ `None`。
    ///
    /// docx の `w:highlight` は**決まった色の名前**しか受けません。
    /// 好きな色を塗りたいときは背景の塗り(`fill`)を使います。
    #[getter]
    fn highlight(&self) -> PyResult<Option<String>> {
        self.with(|r| r.fmt.highlight.clone())
    }

    #[setter]
    fn set_highlight(&self, value: Option<String>) -> PyResult<()> {
        self.with_mut(|r| r.fmt.highlight = value)
    }

    #[getter]
    fn underline(&self) -> PyResult<bool> {
        self.with(|r| r.fmt.underline)
    }

    #[setter]
    fn set_underline(&self, value: bool) -> PyResult<()> {
        self.with_mut(|r| r.fmt.underline = value)
    }

    #[getter]
    fn strike(&self) -> PyResult<bool> {
        self.with(|r| r.fmt.strike)
    }

    #[setter]
    fn set_strike(&self, value: bool) -> PyResult<()> {
        self.with_mut(|r| r.fmt.strike = value)
    }

    /// 上付き(x²)。docx の `w:vertAlign w:val="superscript"`
    #[getter]
    fn superscript(&self) -> PyResult<bool> {
        self.with(|r| r.fmt.superscript)
    }

    #[setter]
    fn set_superscript(&self, value: bool) -> PyResult<()> {
        // 上と下は同時に付きません。片方を立てたらもう片方は寝かせます
        self.with_mut(|r| {
            r.fmt.superscript = value;
            if value {
                r.fmt.subscript = false;
            }
        })
    }

    /// 下付き(H₂O)
    #[getter]
    fn subscript(&self) -> PyResult<bool> {
        self.with(|r| r.fmt.subscript)
    }

    #[setter]
    fn set_subscript(&self, value: bool) -> PyResult<()> {
        self.with_mut(|r| {
            r.fmt.subscript = value;
            if value {
                r.fmt.superscript = false;
            }
        })
    }

    #[getter]
    fn color(&self) -> PyResult<Option<String>> {
        self.with(|r| r.fmt.color.clone())
    }

    #[setter]
    fn set_color(&self, value: Option<String>) -> PyResult<()> {
        self.with_mut(|r| r.fmt.color = value.filter(|v| !v.is_empty()))
    }

    /// 文字スタイル。読みは styles の名前(無ければ styleId、指定なしは None)。
    /// 書きは **styles にある文字スタイルの名前**(None で外す)。
    #[getter]
    fn style(&self) -> PyResult<Option<String>> {
        let g = lock(&self.inner)?;
        let r = g
            .para(&self.loc)
            .and_then(|p| p.runs.get(self.idx))
            .ok_or_else(|| PyIndexError::new_err("この run はもう文書に無い"))?;
        Ok(r.fmt.style_id.as_ref().map(|id| {
            g.doc
                .styles
                .iter()
                .chain(g.doc.styles_new.iter())
                .find(|s| s.id == *id)
                .map(|s| if s.name.is_empty() { s.id.clone() } else { s.name.clone() })
                .unwrap_or_else(|| id.clone())
        }))
    }

    #[setter]
    fn set_style(&self, value: Option<&str>) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        let id = match value {
            None => None,
            Some(v) => {
                let found = g
                    .doc
                    .styles
                    .iter()
                    .chain(g.doc.styles_new.iter())
                    .find(|s| (s.name == v || s.id == v) && s.kind == "character")
                    .map(|s| s.id.clone());
                match found {
                    Some(id) => Some(id),
                    None => {
                        return Err(PyValueError::new_err(format!(
                            "文字スタイル「{v}」が styles に無い(add_style(名前, \"character\") で作ってから)"
                        )))
                    }
                }
            }
        };
        let p = g
            .para_mut(&self.loc)
            .ok_or_else(|| PyIndexError::new_err("この段落はもう文書に無い"))?;
        let r = p
            .runs
            .get_mut(self.idx)
            .ok_or_else(|| PyIndexError::new_err("この run はもう文書に無い"))?;
        r.fmt.style_id = id;
        Ok(())
    }

    /// リンク先(URL。無ければ None)。**囲み(w:hyperlink)が掛かりを決める**
    /// ので、書式と同じ持ち場に置いてある — run を切り貼りしても付いて回る。
    #[getter]
    fn hyperlink(&self) -> PyResult<Option<String>> {
        self.with(|r| r.fmt.link.clone())
    }

    #[setter]
    fn set_hyperlink(&self, value: Option<String>) -> PyResult<()> {
        self.with_mut(|r| r.fmt.link = value.filter(|v| !v.is_empty()))
    }

    /// 改行を足す(python-docx の add_break)。docx の w:br になる。
    fn add_break(&self) -> PyResult<()> {
        self.with_mut(|r| r.text.push('\n'))
    }

    /// タブを足す(python-docx の add_tab)。docx の w:tab になる。
    fn add_tab(&self) -> PyResult<()> {
        self.with_mut(|r| r.text.push('\t'))
    }

    /// 字を後ろに継ぎ足す(python-docx の add_text)。書式はこの run のまま。
    fn add_text(&self, text: &str) -> PyResult<()> {
        self.with_mut(|r| r.text.push_str(text))
    }

    /// 字を消す(書式は残る)。返りは自分(python-docx と同じ)。
    fn clear(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        slf.with_mut(|r| r.text.clear())?;
        Ok(slf)
    }

    fn __repr__(&self) -> PyResult<String> {
        self.with(|r| {
            let mut m = Vec::new();
            if r.fmt.bold {
                m.push("太");
            }
            if r.fmt.italic {
                m.push("斜");
            }
            if r.fmt.underline {
                m.push("下線");
            }
            format!(
                "<officework.doc.Run {:?}{}{}>",
                r.text,
                match r.size_pt {
                    Some(pt) => format!(" {pt}pt"),
                    None => String::new(),
                },
                if m.is_empty() { String::new() } else { format!(" {}", m.join("・")) }
            )
        })
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

    /// 行を1つ足す(末尾)。列の数はいちばん広い行に合わせ、
    /// 各セルは空の段落を1つ持つ。明細行の継ぎ足しはこれ。
    fn add_row(&self) -> PyResult<PyRow> {
        let mut g = lock(&self.inner)?;
        let t = match g.doc.blocks.get_mut(self.block) {
            Some(Block::Table(t)) => t,
            _ => return Err(PyIndexError::new_err("この表はもう文書に無い")),
        };
        let cols = t.rows.iter().map(|r| r.len()).max().unwrap_or(1);
        t.rows.push((0..cols).map(|_| empty_cell()).collect());
        let row = t.rows.len() - 1;
        Ok(PyRow { inner: Arc::clone(&self.inner), block: self.block, row })
    }

    /// 列を1つ足す(右端。全部の行に空のセルが付く)。
    /// `width_mm` は新しい列の幅 — 元の列に幅の指定が無い(等分)ときは
    /// 受けられない(1列だけ幅を持つと形が決まらない)ので正直に断る。
    #[pyo3(signature = (width_mm=None))]
    fn add_column(&self, width_mm: Option<f32>) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        let t = match g.doc.blocks.get_mut(self.block) {
            Some(Block::Table(t)) => t,
            _ => return Err(PyIndexError::new_err("この表はもう文書に無い")),
        };
        match (width_mm, t.col_mm.is_empty()) {
            (Some(w), false) => t.col_mm.push(w),
            (Some(_), true) => {
                return Err(PyValueError::new_err(
                    "この表は列の幅が等分(未指定)なので、新しい列だけに幅を持てません。幅なしで足してください",
                ))
            }
            (None, true) => {} // 等分のまま
            (None, false) => {
                // 幅を持つ表に幅なしの列は形が決まらない — 平均で足す
                let avg = t.col_mm.iter().sum::<f32>() / t.col_mm.len() as f32;
                t.col_mm.push(avg);
            }
        }
        for row in t.rows.iter_mut() {
            row.push(empty_cell());
        }
        Ok(())
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

    /// **列の幅(mm)。**(2026-08-27。台帳の追補)
    ///
    /// 空なら等分です。列の数より短ければ、足りない分は等分になります。
    ///
    ///     t.col_widths_mm = [30, 60, 20]
    #[getter]
    fn col_widths_mm(&self) -> PyResult<Vec<f32>> {
        let g = lock(&self.inner)?;
        Ok(g.table(self.block).map(|t| t.col_mm.clone()).unwrap_or_default())
    }

    #[setter]
    fn set_col_widths_mm(&self, value: Vec<f32>) -> PyResult<()> {
        if value.iter().any(|v| *v <= 0.0) {
            return Err(PyValueError::new_err("列の幅は 0 より大きい数です"));
        }
        let mut g = lock(&self.inner)?;
        match g.doc.blocks.get_mut(self.block) {
            Some(Block::Table(t)) => {
                t.col_mm = value;
                // **mm を入れたら比は捨てます。** 両方あると、どちらが正か
                // 分からなくなります(adoc は比、docx は mm で持ちます)
                t.col_ratio.clear();
                Ok(())
            }
            _ => Err(PyIndexError::new_err("この表はもう文書に無い")),
        }
    }

    /// 表のスタイルの**名前だけ**(docx の w:tblStyle の styleId)。
    /// 定義(styles.xml)は持たない主義 — 読んだ名前を運んで返すだけ。
    /// 定義が要る名前は、原本(雛形)の styles.xml が持っているのが前提。
    #[getter]
    fn style(&self) -> PyResult<Option<String>> {
        let g = lock(&self.inner)?;
        Ok(g.table(self.block).and_then(|t| t.style.clone()))
    }

    #[setter]
    fn set_style(&self, value: Option<String>) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        match g.doc.blocks.get_mut(self.block) {
            Some(Block::Table(t)) => {
                t.style = value.filter(|v| !v.is_empty());
                Ok(())
            }
            _ => Err(PyIndexError::new_err("この表はもう文書に無い")),
        }
    }

    /// 表の置き方。"left" / "center" / "right"、指定なしは None。
    #[getter]
    fn alignment(&self) -> PyResult<Option<String>> {
        let g = lock(&self.inner)?;
        Ok(g.table(self.block).and_then(|t| t.align).map(|a| {
            match a {
                kumihan::Align::Center => "center",
                kumihan::Align::Right => "right",
                _ => "left",
            }
            .to_string()
        }))
    }

    #[setter]
    fn set_alignment(&self, value: Option<&str>) -> PyResult<()> {
        let a = match value {
            None => None,
            Some("left") => Some(kumihan::Align::Left),
            Some("center") => Some(kumihan::Align::Center),
            Some("right") => Some(kumihan::Align::Right),
            Some(v) => {
                return Err(PyValueError::new_err(format!(
                    "表の置き方は left / center / right(か None): {v:?}"
                )))
            }
        };
        let mut g = lock(&self.inner)?;
        match g.doc.blocks.get_mut(self.block) {
            Some(Block::Table(t)) => {
                t.align = a;
                Ok(())
            }
            _ => Err(PyIndexError::new_err("この表はもう文書に無い")),
        }
    }

    /// 列幅を中身に合わせるか(docx の tblLayout。python-docx と同じ真偽)。
    /// False = 固定(w:tblLayout type="fixed")。
    #[getter]
    fn autofit(&self) -> PyResult<bool> {
        let g = lock(&self.inner)?;
        Ok(g.table(self.block).map(|t| !t.fixed_layout).unwrap_or(true))
    }

    #[setter]
    fn set_autofit(&self, value: bool) -> PyResult<()> {
        let mut g = lock(&self.inner)?;
        match g.doc.blocks.get_mut(self.block) {
            Some(Block::Table(t)) => {
                t.fixed_layout = !value;
                Ok(())
            }
            _ => Err(PyIndexError::new_err("この表はもう文書に無い")),
        }
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

    /// **行の高さ(mm)。** 0 は「中身なり」(指定なし)です。
    ///
    /// docx は「少なくともこの高さ」で書きます。字が入り切らない行は
    /// 指定より高くなります(Word と同じで、字を切りません)。
    #[getter]
    fn height(&self) -> PyResult<f32> {
        let g = lock(&self.inner)?;
        Ok(g.table(self.block)
            .and_then(|t| t.row_mm.get(self.row).copied())
            .unwrap_or(0.0))
    }

    #[setter]
    fn set_height(&self, value: f32) -> PyResult<()> {
        if !(0.0..=1000.0).contains(&value) {
            return Err(PyValueError::new_err(format!("行の高さは 0〜1000mm: {value}")));
        }
        let mut g = lock(&self.inner)?;
        let Some(Block::Table(t)) = g.doc.blocks.get_mut(self.block) else {
            return Err(PyIndexError::new_err("この表はもう文書に無い"));
        };
        // **行と同じ長さに伸ばしてから**入れます。足りないまま添字で
        // 触ると、後ろの行だけ高さを付けたときに前の行がずれます
        if t.row_mm.len() < t.rows.len() {
            t.row_mm.resize(t.rows.len(), 0.0);
        }
        match t.row_mm.get_mut(self.row) {
            Some(h) => *h = value,
            None => return Err(PyIndexError::new_err("その行は表の外です")),
        }
        Ok(())
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
    /// **このセルから相手のセルまでを1つに結合する。**
    ///
    /// python-docx と同じく、結合した左上のセルを返します。横の結合は
    /// docx の `w:gridSpan`、縦は `w:vMerge` です。呑まれたセルの字は
    /// 消えます(Excel の結合と同じ — 左上だけが残ります)。
    ///
    /// **同じ行か同じ列だけ**です。四角い塊の結合(2行2列など)は、
    /// 行ごとに横を結んでから縦に結びます。
    fn merge(&self, other: &PyCell) -> PyResult<PyCell> {
        if self.block != other.block {
            return Err(PyValueError::new_err("別の表のセルとは結合できません"));
        }
        let (r0, r1) = (self.row.min(other.row), self.row.max(other.row));
        let (c0, c1) = (self.col.min(other.col), self.col.max(other.col));
        if r0 != r1 && c0 != c1 {
            return Err(PyValueError::new_err(
                "結合できるのは同じ行か同じ列です。四角い塊は、行ごとに横を\
                 結んでから縦に結んでください",
            ));
        }
        let mut g = lock(&self.inner)?;
        let Some(Block::Table(t)) = g.doc.blocks.get_mut(self.block) else {
            return Err(PyIndexError::new_err("この表はもう文書に無い"));
        };
        if r1 >= t.rows.len() || t.rows[r0..=r1].iter().any(|r| c1 >= r.len()) {
            return Err(PyIndexError::new_err("そのセルは表の外です"));
        }
        if r0 == r1 {
            // 横に結ぶ。**呑まれたセルは格子から取り除きます** — 残すと
            // 読み手には「3つ分の1つ + 空2つ = 5列」に見えます
            // (2026-08-27、python-docx で開いて気づきました)
            let haba = (c1 - c0 + 1) as u8;
            t.rows[r0].drain((c0 + 1)..=c1);
            t.rows[r0][c0].col_span = haba;
        } else {
            // 縦に結ぶ。先頭が Start、続きが Continue
            t.rows[r0][c0].v_merge = kumihan::VMerge::Start;
            for r in (r0 + 1)..=r1 {
                t.rows[r][c0] = kumihan::Cellbox {
                    v_merge: kumihan::VMerge::Continue,
                    ..Default::default()
                };
            }
        }
        Ok(PyCell {
            inner: Arc::clone(&self.inner),
            block: self.block,
            row: r0,
            col: c0,
        })
    }

    /// **セルの中の縦位置。** `"top"` / `"center"` / `"bottom"`。
    /// docx の既定は `"top"` です(表計算の既定の下揃えとは違います)。
    #[getter]
    fn vertical_alignment(&self) -> PyResult<&'static str> {
        let g = lock(&self.inner)?;
        let c = g
            .table(self.block)
            .and_then(|t| t.rows.get(self.row))
            .and_then(|r| r.get(self.col))
            .ok_or_else(|| PyIndexError::new_err("このセルはもう文書に無い"))?;
        Ok(match c.valign {
            book::VAlign::Middle => "center",
            book::VAlign::Bottom => "bottom",
            _ => "top",
        })
    }

    #[setter]
    fn set_vertical_alignment(&self, value: &str) -> PyResult<()> {
        let v = match value {
            "top" => book::VAlign::Top,
            "center" | "middle" => book::VAlign::Middle,
            "bottom" => book::VAlign::Bottom,
            _ => {
                return Err(PyValueError::new_err(format!(
                    "縦位置は top / center / bottom です: {value:?}"
                )))
            }
        };
        let mut g = lock(&self.inner)?;
        let Some(Block::Table(t)) = g.doc.blocks.get_mut(self.block) else {
            return Err(PyIndexError::new_err("この表はもう文書に無い"));
        };
        match t.rows.get_mut(self.row).and_then(|r| r.get_mut(self.col)) {
            Some(c) => c.valign = v,
            None => return Err(PyIndexError::new_err("そのセルは表の外です")),
        }
        Ok(())
    }

    /// **セルの幅(mm)。** docx は幅を**列**で持つので、これはこのセルが
    /// 居る列の幅です。同じ列の別の行に入れても同じ所を触ります
    /// (python-docx も同じ振る舞いです)。
    #[getter]
    fn width(&self) -> PyResult<f32> {
        let g = lock(&self.inner)?;
        Ok(g.table(self.block)
            .and_then(|t| t.col_mm.get(self.col).copied())
            .unwrap_or(0.0))
    }

    #[setter]
    fn set_width(&self, value: f32) -> PyResult<()> {
        if !(0.0..=1000.0).contains(&value) {
            return Err(PyValueError::new_err(format!("列の幅は 0〜1000mm: {value}")));
        }
        let mut g = lock(&self.inner)?;
        let Some(Block::Table(t)) = g.doc.blocks.get_mut(self.block) else {
            return Err(PyIndexError::new_err("この表はもう文書に無い"));
        };
        let haba = t.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if t.col_mm.len() < haba {
            t.col_mm.resize(haba, 0.0);
        }
        match t.col_mm.get_mut(self.col) {
            Some(w) => *w = value,
            None => return Err(PyIndexError::new_err("その列は表の外です")),
        }
        Ok(())
    }

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
        kumihan::set_paras_text(&mut cell.paragraphs, text);
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
    m.add_class::<PyCoreProps>()?;
    m.add_class::<PySection>()?;
    parent.add_submodule(&m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, bold: bool) -> Run {
        Run {
            text: text.to_string(),
            size_pt: Some(10.5),
            font: None,
            fmt: CharFormat { bold, ..Default::default() },
        }
    }

    #[test]
    fn paragraph_text_is_the_runs_joined() {
        let p = Paragraph { runs: vec![run("請求先: ", false), run("株式会社甲", true)], ..Default::default() };
        assert_eq!(para_text(&p), "請求先: 株式会社甲");
    }

    #[test]
    fn assigning_text_inherits_the_first_runs_format() {
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
    fn assigning_to_an_empty_paragraph_leaves_the_size_unset() {
        // 以前はここで 10.5 が入っていた — それが往復の焼き付きの入り口。
        // 無指定(None)は文書の既定に従う、が正しい形
        let mut p = Paragraph::default();
        set_para_text(&mut p, "あ");
        assert_eq!(p.runs[0].size_pt, None);
    }

    #[test]
    fn replace_keeps_run_boundaries() {
        let mut runs = vec![run("請求先: ", false), run("旧社名", true), run(" 御中", false)];
        assert_eq!(replace_in_runs(&mut runs, "旧社名", "新社名"), 1);
        assert_eq!(runs.len(), 3, "run の数は変わらない");
        assert_eq!(runs[1].text, "新社名");
        assert!(runs[1].fmt.bold, "見つかった run の書式のまま");
        assert!(!runs[0].fmt.bold, "隣の run は動かない");
    }

    #[test]
    fn replace_matches_across_runs() {
        // 「旧社名」が 旧/社名 に割れている(Word が普通に作る形)
        let mut runs = vec![run("あ旧", false), run("社名い", true)];
        assert_eq!(replace_in_runs(&mut runs, "旧社名", "新社名"), 1);
        assert_eq!(runs[0].text, "あ新社名", "始まりを含む run が新しい字を持つ");
        assert_eq!(runs[1].text, "い", "続きの run からは掛かった分だけ落ちる");
    }

    #[test]
    fn replace_counts_every_occurrence() {
        let mut runs = vec![run("甲と甲と甲", false)];
        assert_eq!(replace_in_runs(&mut runs, "甲", "乙"), 3);
        assert_eq!(runs[0].text, "乙と乙と乙");
    }

    #[test]
    fn replace_does_nothing_when_not_found() {
        let mut runs = vec![run("あいう", false)];
        assert_eq!(replace_in_runs(&mut runs, "えお", "x"), 0);
        assert_eq!(runs[0].text, "あいう");
        // 空の needle で無限に回らないこと
        assert_eq!(replace_in_runs(&mut runs, "", "x"), 0);
    }

    #[test]
    fn replace_survives_empty_runs() {
        let mut runs = vec![run("", false), run("旧社名", false), run("", false)];
        assert_eq!(replace_in_runs(&mut runs, "旧社名", "新"), 1);
        assert_eq!(runs.iter().map(|r| r.text.as_str()).collect::<String>(), "新");
    }

    #[test]
    fn the_page_number_field_reads_as_text() {
        // 私用領域の字をそのまま Python へ出さない
        let s = format!("- {} / {} -", kumihan::PAGE_MARK, kumihan::PAGES_MARK);
        assert_eq!(marks_to_text(&s), "- # / ## -");
        assert_eq!(marks_to_text("印の無い字"), "印の無い字");
    }

    #[test]
    fn negative_indexes_work() {
        assert_eq!(resolve(-1, 3, "行は").unwrap(), 2);
        assert_eq!(resolve(0, 3, "行は").unwrap(), 0);
        assert!(resolve(3, 3, "行は").is_err());
        assert!(resolve(-4, 3, "行は").is_err());
    }

    #[test]
    fn form_fields_are_found_by_name() {
        let sdt = |tag: &str| {
            Some(Box::new(kumihan::Sdt { tag: tag.into(), ..Default::default() }))
        };
        let run = |text: &str, s: Option<Box<kumihan::Sdt>>| kumihan::Run {
            text: text.into(),
            size_pt: Some(10.5),
            font: None,
            fmt: kumihan::CharFormat { sdt: s, ..Default::default() },
        };
        let runs = vec![
            run("前置き ", None),
            run("ここに", sdt("宛先")), // 同じ欄が2つの run に割れている形
            run("社名", sdt("宛先")),
            run(" と ", None),
            run("0", sdt("金額")),
            run("印", sdt("")), // 名前なしの欄は「名前で引く」対象にならない
        ];
        let g = sdt_groups(&runs);
        assert_eq!(
            g,
            vec![("宛先".to_string(), 1, 3), ("金額".to_string(), 4, 5)],
            "欄のまとまりが違う: {g:?}"
        );
    }
}
