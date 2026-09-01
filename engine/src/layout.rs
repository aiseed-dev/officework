//! **組版。** 文書を、実フォントの字幅で行に組み、置かれた字の座標を返す。
//!
//! 行頭・行末の禁則、欧文の語中で改行しない、段組み、頁割り。

use ttf_parser::Face;

use super::doc::*;

pub struct Metrics<'a> {
    face: Face<'a>,
    upem: f32,
}

pub(super) const PT_TO_MM: f32 = 25.4 / 72.0;

impl<'a> Metrics<'a> {
    pub fn new(font_data: &'a [u8]) -> Result<Metrics<'a>, String> {
        let face = Face::parse(font_data, 0).map_err(|e| e.to_string())?;
        let upem = face.units_per_em() as f32;
        Ok(Metrics { face, upem })
    }

    /// 1文字の送り幅(mm)。フォントに無い文字は全角の半分で仮置きする。
    pub fn advance_mm(&self, ch: char, size_pt: f32) -> f32 {
        let adv = self
            .face
            .glyph_index(ch)
            .and_then(|g| self.face.glyph_hor_advance(g))
            .map(|a| a as f32 / self.upem)
            .unwrap_or(0.5);
        adv * size_pt * PT_TO_MM
    }
}

// ---------- 禁則(JIS X 4051 の主要部) ----------

/// 行頭に置けない(句読点・閉じ括弧・小書き仮名・長音など)
pub const GYOTO_KINSOKU: &str =
    "、。，．・：；？！ヽヾゝゞ々ー〜…‥ぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮ\
     ）」』】〕〉》〙〗ゕゖㇷ゚%‰′″℃)]}>,.:;?!";

/// 行末に置けない(開き括弧など)
pub const GYOMATSU_KINSOKU: &str = "（「『【〔〈《〘〖([{<";

pub(super) fn is_gyoto_kinsoku(c: char) -> bool {
    GYOTO_KINSOKU.contains(c)
}
pub(super) fn is_gyomatsu_kinsoku(c: char) -> bool {
    GYOMATSU_KINSOKU.contains(c)
}

// ---------- 行組み ----------

/// 改行の単位。CJKは1字ずつ、欧文は語ごと(語中では折らない)。
#[derive(Debug)]
pub(super) enum Tok {
    // (字, 幅mm, サイズpt, 書式, 書体, バイト位置)
    One(char, f32, f32, CharFormat, Option<String>, usize),
    // (字と幅と位置の列, サイズpt, 書式, 書体)
    Word(Vec<(char, f32, usize)>, f32, CharFormat, Option<String>),
    // (字, 幅mm, サイズpt, 書式, 書体, バイト位置)。
    // **字も持ちます**(2026-08-30)。前は半角スペースに置き替えていましたが、
    // 紙に出すとき字を繋げて1つの塊で書くので、**送りは書体の半角の幅**に
    // なります。全角スペース(U+3000)で字下げした文書が、1字あたり
    // 12pt のはずが 3.48pt になっていました(内閣府の告知書、9字で 76.7pt)。
    // バイト位置も全角は3バイトなので、半角に替えると数が合いません
    Space(char, f32, f32, CharFormat, Option<String>, usize),
}

pub(super) fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

/// 注の通し番号。**脚注と文末脚注は別々に数える** — docx が
/// `footnotes.xml` と `endnotes.xml` を別に番号付けするのと同じで、
/// 1本の連番にすると脚注が「1・3」文末脚注が「2・4」と飛んで見える
#[derive(Debug, Clone, Copy, Default)]
pub struct NoteCount {
    pub foot: usize,
    pub end: usize,
    pub foot_fmt: NoteNumFmt,
    pub end_fmt: NoteNumFmt,
}

impl NoteCount {
    fn of(doc: &Document) -> NoteCount {
        NoteCount { foot: 0, end: 0,
                    foot_fmt: doc.footnote_fmt, end_fmt: doc.endnote_fmt }
    }
    /// 次の番号を1つ進めて、その書式の字にする
    fn next(&mut self, endnote: bool) -> String {
        if endnote {
            self.end += 1;
            self.end_fmt.label(self.end)
        } else {
            self.foot += 1;
            self.foot_fmt.label(self.foot)
        }
    }
}

pub(super) fn tokenize(p: &Paragraph, m: &Metrics, notes: &mut NoteCount, base: f32) -> Vec<Tok> {
    let mut out = Vec::new();
    // 段落の頭からのバイト位置。run をまたいで通しで数える
    let mut off = 0usize;
    for run in &p.runs {
        // 無指定(None)はここで文書の既定に解く — Tok/Cell は紙面の産物なので
        // 解決済みの数を持つ(模型の Option を紙面まで運ばない)
        let rpt = run.pt(base);
        // 脚注の印。**番号は出てくる順**(id の数ではない)に振る。
        // 箇条書きの印と同じで**本文の字ではない**ので、
        // off は動かさない — 動かすとカーソルが本文とずれる
        if let Some(fr) = &run.fmt.footnote {
            let label = notes.next(fr.endnote);
            let size = rpt * 0.7;
            let mut fmt = run.fmt.clone();
            fmt.superscript = true;
            for ch in label.chars() {
                out.push(Tok::One(ch, m.advance_mm(ch, size), size,
                                  fmt.clone(), run.font.clone(), off));
            }
            continue;
        }
        // **字間**(`w:rPr` の `w:spacing`)。1文字ごとに足します
        let aki = run.fmt.spacing_pt * PT_TO_MM;
        let okuri = |ch: char| (m.advance_mm(ch, rpt) + aki).max(0.0);
        let mut word: Vec<(char, f32, usize)> = Vec::new();
        for ch in run.text.chars() {
            if is_word_char(ch) {
                word.push((ch, okuri(ch), off));
                off += ch.len_utf8();
                continue;
            }
            if !word.is_empty() {
                out.push(Tok::Word(std::mem::take(&mut word), rpt, run.fmt.clone(),
                                   run.font.clone()));
            }
            if ch == ' ' || ch == '\u{3000}' {
                out.push(Tok::Space(ch, okuri(ch), rpt,
                                    run.fmt.clone(), run.font.clone(), off));
            } else {
                out.push(Tok::One(ch, okuri(ch), rpt,
                                  run.fmt.clone(), run.font.clone(), off));
            }
            off += ch.len_utf8();
        }
        if !word.is_empty() {
            out.push(Tok::Word(word, rpt, run.fmt.clone(), run.font.clone()));
        }
    }
    out
}

/// **1行の高さ(mm)。** 本文 10.5pt に対する行送りです。
///
/// 画面も紙も PDF も**この1つを見ます**。アプリの側に置いていたので、
/// エンジンから PDF を作る道を足したとき 6.30mm と 6.40mm の2つになり、
/// 同じ文書が別の頁数に折れる形になっていました(2026-08-27)。
pub const LINE_MM: f32 = 6.4;

/// **行の箱の中で、ベースラインが上端から何 mm 下か。**
///
/// 残りの `LINE_MM - BASE_UP_MM`(2.4mm)が字の足の分です。
///
/// 頁割りはここを見ます。見ないとベースラインだけで判断してしまい、
/// **最後の行の足が下の余白へ 1.4mm はみ出します**(2026-08-29 に測って
/// 分かりました。Word は行の箱ごと入る分しか置かないので、同じ文書が
/// 1ページあたり1行ずれていました)。
pub const BASE_UP_MM: f32 = 4.0;

pub struct Frame {
    pub measure_mm: f32,   // 行長
    pub line_height_mm: f32,
    pub y0_mm: f32,        // 最初のベースライン
}

/// 段落の列を行に組む。
///
/// 禁則はその場で解決する(後処理にしない — 後から字を送ると送った先が
/// 行長を超えるため)。行を折る瞬間に:
///   1. 折る原因の字が行頭禁則なら、新しい行の頭が禁則でなくなるまで
///      前の行の末尾から字を引き取る(追い出し)
///   2. 前の行の末尾に行末禁則(開き括弧)が残っていれば、それも引き取る
///
/// 引き取った分だけ前の行は短くなる — 行長を超える方向には決して動かない。
///
/// 段落を行長で折る。x はまだ置かない(呼ぶ側が揃え・字下げを決める)。
/// 1行目の字下げ(mm)。段落が持つ twips から。負(ぶら下げ)は 0 とみなす。
///
/// **組み手と呼ぶ側の両方が同じ値を使う**ので、ここに1つだけ置きます。
pub(super) fn first_line_mm(para: &Paragraph, base: f32) -> f32 {
    // **文字数での指定は、その段落の字の大きさで解きます**(2026-09-01)。
    // Word が書き置いた twip はその段落の大きさで解いた値なので、こちらの
    // 既定(10.5pt)で解くとずれます。調査票は 12pt の段落で 240twip です
    let tw = match para.first_line_chars {
        Some(c) => c / 100.0 * base * 20.0,
        None => para.first_line_twips as f32,
    };
    (tw.max(0.0) / 20.0) * 25.4 / 72.0 + atama_no_gazou_mm(para)
}

/// **段落の頭に置かれた画像の幅(mm)。**
///
/// docx の `<wp:inline>` は run の中に入るので、字と同じ行に並びます。
/// 前は段落の下にしか置けず、内閣府の document_4 では見出しの絵が
/// 見出しの1行下に落ちていました(2026-09-01)。1行目の字下げに足すと、
/// 折り返しも揃えもそのまま正しくなります — 中央揃えなら絵と字を
/// ひとまとまりにして中央に置きます。
///
/// **見るのは頭(位置0)に在る画像だけです。** 途中に入る絵は、行の中の
/// どこで折るかまで見ないと置けないので、今までどおり段落の下です。
pub(super) fn atama_no_gazou_takasa(para: &Paragraph) -> f32 {
    para.images
        .iter()
        .chain(para.images_new.iter())
        .filter(|im| im.off == 0)
        .map(|im| im.h_mm)
        .fold(0.0, f32::max)
}

pub(super) fn atama_no_gazou_mm(para: &Paragraph) -> f32 {
    para.images
        .iter()
        .chain(para.images_new.iter())
        .filter(|im| im.off == 0)
        .map(|im| im.w_mm)
        .sum()
}

/// 左のインデント(mm)。**twip の指定があればそちらが勝ちます。**
///
/// 段数(`indent`)は全角2文字きざみなので、1文字や3文字を表せません。
/// docx から読んだ細かい値は `left_twips` が持っているので、それを先に
/// 見ます(2026-08-30)。`em` は全角1文字の幅(mm)です。
pub(super) fn left_mm(para: &Paragraph, em: f32) -> f32 {
    if para.left_twips > 0 {
        return (para.left_twips as f32 / 20.0) * 25.4 / 72.0;
    }
    para.indent as f32 * em * 2.0
}

pub(super) fn break_para(para: &Paragraph, m: &Metrics, measure: f32, marker: Option<&str>,
              hyphenate: bool, notes: &mut NoteCount, base: f32) -> Vec<Vec<Cell>> {
    // **見出しは大きく太く組む**([`head_scale`])。大きさは「基準」を
    // 持ち上げる形にするので、run が自分で大きさを言っていればそちらが勝つ
    // (docx の作法どおり — run の指定はスタイルより強い)
    let scale = head_scale(para.style);
    let base = base * scale;
    let mut done: Vec<Vec<Cell>> = Vec::new();
    let mut cur: Vec<Cell> = Vec::new();
    let mut w_cur = 0.0f32;

    // 行を閉じ、禁則ぶんを引き取って次の行の頭(carry)を返す
    fn close(done: &mut Vec<Vec<Cell>>, cur: &mut Vec<Cell>, w_cur: &mut f32,
             incoming_head: Option<char>) -> Vec<Cell> {
        let mut carry: Vec<Cell> = Vec::new();
        // 1) 折る原因の字が行頭禁則 → 頭が禁則でなくなるまで引き取る
        if incoming_head.is_some_and(is_gyoto_kinsoku) {
            while cur.len() > 1 {
                let c = cur.pop().unwrap();
                let head_ok = !is_gyoto_kinsoku(c.ch);
                carry.insert(0, c);
                if head_ok {
                    break;
                }
            }
        }
        // 2) 行末に開き括弧を残さない
        while cur.len() > 1 && cur.last().is_some_and(|c| is_gyomatsu_kinsoku(c.ch)) {
            let c = cur.pop().unwrap();
            carry.insert(0, c);
        }
        done.push(std::mem::take(cur));
        *w_cur = carry.iter().map(|c| c.w_mm).sum();
        carry
    }

    // **1行目の字下げ**(日本語の本文の作法)。1行目だけ行長を縮めます。
    // x をずらすのは呼ぶ側です([`first_line_mm`] を足す)。
    //
    // 空白の桝を1つ置く形も試しましたが、**画面の幅と合いません** — 桝の幅は
    // こちらが決めても、画面は空白の字をフォントの幅で描くからです
    // (2026-08-18 に実機で見て気づきました)。
    // ぶら下げ(負の値)はまだ組めないので、0 として扱います
    let first_mm = first_line_mm(para, base);

    // 箇条書きの印は本文の前に置く。**本文の一部にはしない**ので、
    // 編集中の文字位置とずれない(印は組版のときだけ現れる)
    if let Some(mk) = marker {
        let size = para.runs.first().and_then(|r| r.size_pt).unwrap_or(base);
        let fmt = para.runs.first().map(|r| r.fmt.clone()).unwrap_or_default();
        let font = para.runs.first().and_then(|r| r.font.clone());
        for ch in mk.chars() {
            let w = m.advance_mm(ch, size);
            // 印は本文の一部ではないので off は段落頭(0)のまま
            cur.push(Cell { ch, x_mm: 0.0, w_mm: w, size_pt: size, fmt: fmt.clone(),
                            font: font.clone(), off: 0 });
            w_cur += w;
        }
    }
    // **タブは決まった位置まで送ります**(2026-09-01)。
    //
    // 前は1文字ぶんの幅しか送らず、字形も持っていないので豆腐が出ていました。
    // 内閣府の調査票の氏名欄は、下線が 78.6pt ぶん縮んでいました。
    // 止まる位置は段落の `w:tabs`、どれも越えていれば既定の刻みです。
    let tab_saki = |ima_mm: f32| -> f32 {
        let ima_tw = ima_mm * 72.0 * 20.0 / 25.4;
        let tugi = para
            .tab_stops
            .iter()
            .copied()
            .filter(|t| *t as f32 > ima_tw + 0.5)
            .min()
            .map(|t| t as f32)
            .unwrap_or_else(|| {
                let k = crate::TAB_TWIPS as f32;
                ((ima_tw / k).floor() + 1.0) * k
            });
        (tugi / 20.0) * 25.4 / 72.0
    };
    for tok in tokenize(para, m, notes, base) {
        // タブの幅は、いまの位置から次の止まる所までです
        let tok = match &tok {
            Tok::One('\t', _, s, f, ft, o) => {
                let saki = tab_saki(w_cur + if done.is_empty() { first_mm } else { 0.0 });
                let haba = (saki - w_cur - if done.is_empty() { first_mm } else { 0.0 }).max(0.0);
                Tok::Space('\t', haba, *s, f.clone(), ft.clone(), *o)
            }
            _ => tok,
        };
        let (cells, w): (Vec<Cell>, f32) = match &tok {
            Tok::One(ch, w, s, f, ft, o) =>
                (vec![Cell { ch: *ch, x_mm: 0.0, w_mm: *w, size_pt: *s, fmt: f.clone(),
                             font: ft.clone(), off: *o }], *w),
            Tok::Word(cs, s, f, ft) => (
                cs.iter().map(|(c, w, o)| Cell { ch: *c, x_mm: 0.0, w_mm: *w, size_pt: *s,
                                                 fmt: f.clone(), font: ft.clone(), off: *o })
                    .collect(),
                cs.iter().map(|(_, w, _)| *w).sum()),
            Tok::Space(ch, w, s, f, ft, o) =>
                (vec![Cell { ch: *ch, x_mm: 0.0, w_mm: *w, size_pt: *s, fmt: f.clone(),
                             font: ft.clone(), off: *o }], *w),
        };

        // 1行目だけ行長が短い(字下げのぶん)
        let measure = if done.is_empty() { (measure - first_mm).max(1.0) } else { measure };
        // **行頭に置けない字は、はみ出させて行末に留めます**(追い込み)。
        //
        // 句読点や閉じ括弧は行の頭に来られません。前は手前の字を次の行へ
        // 送り出していた(追い出し)ので、1行に入る字が元より1〜2字少なく
        // なっていました。内閣府の調査票は、行末の「」」が右の余白へ
        // 1字ぶん出ています(2026-09-01 発注者「漢字の横幅は同じはず。
        // どうして文字数が違ってくる」)。
        //
        // 出るのは**1字だけ**です。続けて出ると行が伸び続けます
        let oikomi = matches!(&tok, Tok::One(ch, ..) if is_gyoto_kinsoku(*ch))
            && !cur.is_empty()
            && !cur.last().is_some_and(|c: &Cell| is_gyoto_kinsoku(c.ch));
        if w_cur + w > measure && !cur.is_empty() && !oikomi {
            if let Tok::Space(..) = tok {
                // 行末に空白は要らない。行を折るだけ
                cur = close(&mut done, &mut cur, &mut w_cur, None);
                continue;
            }
            // 欧文の語は、設定が入っていれば音節で折って - を付ける
            if hyphenate {
                if let Tok::Word(cs, sz, f, ft) = &tok {
                    if let Some(k) = hyphen_split(cs, *sz, m, measure - w_cur) {
                        for (c, wch, o) in &cs[..k] {
                            cur.push(Cell { ch: *c, x_mm: 0.0, w_mm: *wch,
                                size_pt: *sz, fmt: f.clone(), font: ft.clone(), off: *o });
                            w_cur += *wch;
                        }
                        // ハイフンは本文の字ではない。バイト位置は直前の字に重ねる
                        // (欧文の字は1バイトなので、行末の勘定が壊れない)
                        let hw = m.advance_mm('-', *sz);
                        let off_h = cs[k - 1].2;
                        cur.push(Cell { ch: '-', x_mm: 0.0, w_mm: hw,
                            size_pt: *sz, fmt: f.clone(), font: ft.clone(), off: off_h });
                        w_cur += hw;
                        cur = close(&mut done, &mut cur, &mut w_cur, None);
                        for (c, wch, o) in &cs[k..] {
                            cur.push(Cell { ch: *c, x_mm: 0.0, w_mm: *wch,
                                size_pt: *sz, fmt: f.clone(), font: ft.clone(), off: *o });
                            w_cur += *wch;
                        }
                        continue;
                    }
                }
            }
            let head = cells.first().map(|c| c.ch);
            cur = close(&mut done, &mut cur, &mut w_cur, head);
        }
        // **折り返した行の頭の空白は組みません。** 語と語の区切りが
        // 行頭に来ただけなので、字下げではありません。
        //
        // ただし**段落の1行目は別**です(2026-08-30)。日本の書類は
        // `w:ind` を使わず全角スペースで字下げすることが多く、内閣府の
        // 告知書では「　あなたは、」の1字と「　　…　様」の9字がそれです。
        // 落とすと字下げが消えて、宛名が余白に貼り付きます
        if cur.is_empty() && !done.is_empty() {
            if let Tok::Space(..) = tok {
                continue;
            }
        }
        w_cur += w;
        cur.extend(cells);
    }
    if !cur.is_empty() || done.is_empty() {
        done.push(cur);
    }
    // 太字は**紙面のセルにだけ**掛ける。模型の run は触らない —
    // 触ると開いて保存しただけで文書に `<w:b/>` が焼き付く。
    // `CharFormat.bold` は bool なので「未指定」と「明示的に太字でない」を
    // 区別できず、見出しの中で太字を外している run も太字になる。
    // **平らな見出しより害は小さい**と見て倒した(styles.xml を読むように
    // したら、そこで正しく解ける)
    if scale > 1.0 {
        for line in &mut done {
            for c in line {
                c.fmt.bold = true;
            }
        }
    }
    done
}

/// 欧文の語の分割点(Knuth-Liang のパターン。TeX と同じ方式)。
/// 前半の字数を返す。avail(残りの幅)に「前半 + '-'」が収まる最長の点を選ぶ。
pub(super) fn hyphen_split(cs: &[(char, f32, usize)], size_pt: f32, m: &Metrics, avail: f32) -> Option<usize> {
    use hyphenation::{Hyphenator, Load};
    static DICT: std::sync::OnceLock<Option<hyphenation::Standard>> = std::sync::OnceLock::new();
    let dict = DICT
        .get_or_init(|| {
            hyphenation::Standard::from_embedded(hyphenation::Language::EnglishUS).ok()
        })
        .as_ref()?;
    let word: String = cs.iter().map(|(c, _, _)| *c).collect();
    // 数字入りの語(型番など)は折らない
    if word.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let hyphen_w = m.advance_mm('-', size_pt);
    let mut best = None;
    for b in dict.hyphenate(&word).breaks {
        // 語は ASCII なのでバイト位置 = 字数
        let w: f32 = cs[..b].iter().map(|(_, w, _)| *w).sum();
        if w + hyphen_w <= avail {
            best = Some(b);
        }
    }
    best
}

/// セルの中の余白(mm)
pub(super) const CELL_PAD: f32 = 1.4;

/// **見出しの見え方**(2026-08-15)。基準の字に対する倍率で持つ。
///
/// 前は `ParaStyle::Heading` を**組版が一度も見ていなかった** — docx の
/// `pStyle` も styles.xml も読めていて、模型にも `Heading(n)` として
/// 入っているのに、本文と同じ大きさで組まれていた。生成した納品書も
/// 既存の見本(報告書.docx)も見出しが平らで、実機で見て気づいた。
///
/// **模型には触らない。** 見出しであること(何であるか)は模型の持ち物で、
/// 大きく太く組むこと(どう見えるか)は組版の持ち物。読み書きは前のまま
/// なので、開いて保存しても文書は変わらない。
///
/// 倍率は自前で書き出す styles.xml と揃う値にした(基準 10.5pt のとき
/// H1≒15.8pt・H2≒14.2pt・H3≒12.6pt。styles.xml は 16/14/12pt)。
/// **絶対の pt にしないのは**、基準の字が大きい文書で見出しが本文より
/// 小さくなるのを避けるため。
///
/// **まだ読んでいない物**: 文書自身の styles.xml。そこに書かれた本当の
/// 大きさ・色・前後の空きは読めていないので、ここは既定の見え方でしかない。
/// 見出しの前後の空きも入れていない(段の送りに関わるので別便)
/// 見出しの行の高さの倍率。**docx へ書く行の高さもこれを見ます**
/// (書かないと開いた側が自分の既定を当て、頁数が食い違います)
pub fn head_scale_of(style: ParaStyle) -> f32 {
    head_scale(style)
}

pub(super) fn head_scale(style: ParaStyle) -> f32 {
    match style {
        // 文書の表題。見出し1 より大きい(テンプレートが言えばそちらが勝つ)
        ParaStyle::Title => 1.8,
        ParaStyle::Heading(1) => 1.5,
        ParaStyle::Heading(2) => 1.35,
        ParaStyle::Heading(3) => 1.2,
        // 4 以降も見出しではあるので、本文よりは大きくする
        ParaStyle::Heading(_) => 1.1,
        _ => 1.0,
    }
}

/// **段落の前の空き**(mm)。文書が言っていればそれ、言っていなくて見出しなら
/// 既定の空き。docx の `w:spacing w:before` は pt で持っている。
///
/// 見出しの既定を置くのは、**見出しが本文に貼り付いて見えるのを防ぐ**ため
/// (Word の Heading も styles.xml に前の空きを持っている — うちはまだ
/// styles.xml を読まないので、ここで既定として与える)。
/// 文書が明示していればそちらが勝つ。
pub(super) fn space_before_mm(para: &Paragraph, base: f32) -> f32 {
    if para.space_before_pt > 0.0 {
        return para.space_before_pt * 25.4 / 72.0;
    }
    match para.style {
        ParaStyle::Heading(1) => base * 0.9 * 25.4 / 72.0,
        ParaStyle::Heading(_) => base * 0.7 * 25.4 / 72.0,
        _ => 0.0,
    }
}

/// 段落の後の空き(mm)。[`space_before_mm`] と同じ決め方で、見出しの既定は
/// 前より小さい — **見出しは次の本文と組で読む**ものなので、上を広く、
/// 下を狭くすると塊が見える(組版の定石)
pub(super) fn space_after_mm(para: &Paragraph, base: f32) -> f32 {
    if para.space_after_pt > 0.0 {
        return para.space_after_pt * 25.4 / 72.0;
    }
    match para.style {
        ParaStyle::Heading(_) => base * 0.25 * 25.4 / 72.0,
        _ => 0.0,
    }
}

/// **1行の高さ(mm)。**
///
/// 決め方は Word と LibreOffice に合わせて2段です。まず書体と字の大きさから
/// 高さを出し、その上に段落の指定を掛けます
/// (LibreOffice は `sw/source/core/text/itrform2.cxx` の `CalcRealHeight`)。
///
/// - `w:lineRule="exact"` — その高さで固定します
/// - `w:lineRule="atLeast"` — その高さを下限にします
/// - `w:lineRule="auto"` — 書体から出した高さに倍率を掛けます
///
/// 書体の名前が分からない段落(AsciiDoc から起こした文書など)は、
/// [`Frame::line_height_mm`] をそのまま使います。
pub(super) fn lh_of(para: &Paragraph, frame: &Frame, base: f32, font: Option<&str>) -> f32 {
    let kihon = match syotai_lh_mm(para, base, font) {
        Some(mm) => mm,
        None => frame.line_height_mm * head_scale(para.style),
    };
    match para.line_pt {
        // exact は書体を見ません。atLeast は下限です
        Some((pt, true)) => pt * PT_TO_MM,
        Some((pt, false)) => kihon.max(pt * PT_TO_MM),
        None => kihon * para.spacing(),
    }
}

/// **書体と字の大きさから出した1行の高さ(mm)。**
///
/// その行に乗る一番大きい字が決めます。書体を1つも名乗っていない段落は
/// `None` を返します。行送りの倍率は [`crate::font::okuri_em`] が引きます。
fn syotai_lh_mm(para: &Paragraph, base: f32, font: Option<&str>) -> Option<f32> {
    let base = base * head_scale(para.style);
    // **字が1つも無い段落も高さを持ちます。** run が無いとここが None に
    // なり、書体を見ない既定に落ちていました。
    //
    // **run があるときは使いません**(2026-09-01)。基準の大きさは
    // 「run が大きさを言わないとき」の受けでしかないので、これを高さの
    // 下限にすると、11pt の受けを持つ文書の 10pt の段落が 11pt の行送りに
    // なります。内閣府の調査票の参考条文が 12.9pt のところ 14.2pt でした
    let mut takai = if para.runs.is_empty() {
        crate::font::okuri_em(font).map(|em| base * em * PT_TO_MM)
    } else {
        None
    };
    for r in &para.runs {
        let em = crate::font::okuri_em(r.font.as_deref().or(font))?;
        let mm = r.pt(base) * em * PT_TO_MM;
        takai = Some(takai.map_or(mm, |t: f32| t.max(mm)));
    }
    takai
}

/// 節ごとの用紙を、**ブロックの番号で引ける形**に開く。
///
/// docx は `w:sectPr` を**その節の終わりに**置く。つまりある段落に効いている
/// 用紙は「**その段落以降で最初に現れる節末**のもの」で、どれにも当たらない
/// 後ろの部分だけが `Document::page`(最後の節)になる。
/// **後ろから前へ**なぞると、これがそのまま書ける — 前から数えると
/// 必ず1つずれる(節末の段落自身がどちらの節かを取り違える)。
///
/// 節が1つも無ければ空を返す。呼ぶ側はそのとき今までどおりに振る舞う。
pub(super) fn section_geometry(doc: &Document) -> Vec<PageSetup> {
    if !doc.blocks.iter().any(|b| matches!(b, Block::Para(p) if p.sect.is_some())) {
        return Vec::new();
    }
    let mut cur = doc.page.unwrap_or_default();
    let mut geo = vec![cur; doc.blocks.len()];
    for (i, b) in doc.blocks.iter().enumerate().rev() {
        // 節末の段落は**その節に属する**ので、先に切り替えてから置く
        if let Block::Para(p) = b {
            if let Some(sb) = &p.sect {
                cur = sb.page;
            }
        }
        geo[i] = cur;
    }
    geo
}

pub fn layout(doc: &Document, m: &Metrics, frame: &Frame) -> Sheet {
    let mut sheet = Sheet::default();
    let mut y = frame.y0_mm;
    // 無指定の run をどの大きさで組むか。文書ごとに1回だけ解く
    let base = doc.base_pt();
    // 節ごとの用紙。空なら節は1つで、行長は frame のものをそのまま使う
    // (今までの道を1ミリも変えないため — 節の無い文書が大多数)
    let sect_geo = section_geometry(doc);
    // 脚注の通し番号。**文書に出てくる順**に振る(表の中の印も同じ流れで数える)
    let mut note_no = NoteCount::of(doc);
    if let Some(first) = sect_geo.first() {
        // **0 から**置く。上の余白(y0_mm)から置くと、その手前を引いたときに
        // 「節が無い」と読めてしまい、**1ページ目だけ最後の節の紙**で刷られる
        // (2026-08-10、実物の2節 docx を PDF まで通して見つけた)
        sheet.sect_pages.push((0.0, *first));
    }

    // 段落番号は「何番目の箇条書きか」で決まる。段落の位置ではない。
    // レベル(インデント)ごとに数え、浅い番号が進んだら深い数えは振り出しへ
    let mut counters: Vec<usize> = Vec::new();
    // **段ごとの種類**(箇条書きか番号付きか)。種類が変われば別のリストなので
    // 番号は1から振り直す。前は種類を見ていなかったので、箇条書き2つの後の
    // 番号付きが「3.」から始まっていた(2026-08-18 に実機で見つけた)
    let mut kinds: Vec<ListKind> = Vec::new();
    // 本文(段落を \n で繋いだもの)における、いまの段落の頭のバイト位置
    let mut para_byte0 = 0usize;
    let mut table_no = 0usize;
    for (bi, block) in doc.blocks.iter().enumerate() {
        // この段落に効いている行長。節が変われば紙の幅も余白も変わるので、
        // **折り返しそのものがやり直しになる**(折る所だけの話ではない)
        let block_measure = match sect_geo.get(bi) {
            Some(pg) => pg.column_measure_mm(),
            None => frame.measure_mm,
        };
        match block {
            Block::Para(para) => {
                // **段落スタイルが大きさを言っていればそちらが勝ちます。**
                // run の指定はさらに強く、下の break_para が見ます
                let base = doc.style_pt(para.style_id.as_deref()).unwrap_or(base);
                // **書体もスタイルが言います。** run が名乗らない段落の
                // 行送りは、これが引けないと出せません
                let pfont = doc.style_font(para.style_id.as_deref()).or_else(|| doc.font.clone());
                // 改ページ。紙に写すときにここで頁が割れる
                if para.page_break_before && !sheet.lines.is_empty() {
                    sheet.breaks.push(y);
                }
                // **段落の前の空き。** 文書が言っていればそれ、言っていなくて
                // 見出しなら既定の空き(見出しが本文に貼り付いて見えるのを防ぐ)。
                // **紙の頭では空けない** — 上の余白が二重になる
                if !sheet.lines.is_empty() {
                    y += space_before_mm(para, base);
                }
                // **段落の背景色の始まり**を覚えます。終わりは行を積んだ
                // 後に分かるので、そこで四角にします
                let shade_top = y;
                // インデント1段 = 全角2文字ぶん(日本の書類の慣習)
                // **作業のリスト**(`* [ ] やること`)。
                // 印をそのまま組むと `* [ ]` が紙に出ます。この版は
                // 記入欄と同じ ☐ / ☑ で出します(画面の作法を揃える)
                let job = task_list(para);
                let para_eff_check;
                let para = if let Some((mark, body, tab)) = job {
                    let mut q = para.clone();
                    q.indent = tab;
                    let mut rest = body.as_str();
                    for r in &mut q.runs {
                        let n = r.text.len().min(rest.len());
                        r.text = rest[..n].to_string();
                        rest = &rest[n..];
                    }
                    if let Some(r) = q.runs.first_mut() {
                        r.text = format!("{mark}{}", r.text);
                    }
                    para_eff_check = q;
                    &para_eff_check
                } else {
                    para
                };
                // **コードの塊は等幅で組みます**(2026-08-25)。
                // 本文と同じ字だと、コードなのか文章なのか分かりません。
                // 等幅の書体がこの機械に無ければ、そのまま組みます
                let em = para.runs.first().and_then(|r| r.size_pt).unwrap_or(base) * 25.4 / 72.0;
                let indent_mm = left_mm(para, em);
                let measure = (block_measure - indent_mm).max(em);
                // **塊の印の行は、紙に出しません**(2026-08-25)。
                // `[source,python]` と `----` がそのまま印刷されていました。
                // 印はここからここまでが塊だという合図で、文章ではありません
                if matches!(para.style_id.as_deref(), Some("塊の区切り") | Some("指定の行")) {
                    para_byte0 += para.runs.iter().map(|r| r.text.len()).sum::<usize>() + 1;
                    continue;
                }
                let mono = (para.style_id.as_deref() == Some("塊の中"))
                    .then(crate::font::monospace)
                    .flatten()
                    .map(|f| f.name.clone());
                let para_eff_mono;
                let para = if let Some(name) = mono {
                    let mut q = para.clone();
                    for r in &mut q.runs {
                        if r.font.is_none() {
                            r.font = Some(name.clone());
                        }
                    }
                    para_eff_mono = q;
                    &para_eff_mono
                } else {
                    para
                };
                let marker = match para.list {
                    ListKind::None => {
                        counters.clear();
                        kinds.clear();
                        // **註記は印を紙にも出します。** 読むときに
                        // `NOTE: ` を字から外しているので、ここで戻さないと
                        // 紙の上では普通の段落と見分けが付きません
                        admon_heading(para.style_id.as_deref()).map(str::to_string)
                    }
                    _ => {
                        let l = para.indent as usize;
                        counters.truncate(l + 1);
                        kinds.truncate(l + 1);
                        while counters.len() <= l {
                            counters.push(0);
                            kinds.push(ListKind::None);
                        }
                        if kinds[l] != para.list {
                            counters[l] = 0;
                            kinds[l] = para.list;
                        }
                        counters[l] += 1;
                        para.marker(counters[l] - 1)
                    }
                };
                // ドロップキャップ: 頭の1字を大きな1行として先に置き、
                // 残りは行長をその幅ぶん狭めて組む(Word は数行だけ回り込むが、
                // この版は段落まるごと狭める近似)
                let mut cap_shift = 0.0f32;
                let mut cap_len = 0usize;
                let mut owned_rest: Option<Paragraph> = None;
                if para.dropcap {
                    let first = para.runs.first();
                    if let Some(ch) = first.and_then(|r| r.text.chars().next()) {
                        let size0 = first.and_then(|r| r.size_pt).unwrap_or(base);
                        let cap_pt = size0 * 2.8;
                        let cap_w = m.advance_mm(ch, cap_pt) + 1.0;
                        cap_len = ch.len_utf8();
                        cap_shift = cap_w;
                        // 1行下のベースラインに置く = 2行に掛かる見た目
                        sheet.lines.push(Line {
                            cells: vec![Cell {
                                ch,
                                x_mm: indent_mm,
                                w_mm: cap_w,
                                size_pt: cap_pt,
                                fmt: first.map(|r| r.fmt.clone()).unwrap_or_default(),
                                font: first.and_then(|r| r.font.clone()),
                                off: 0,
                            }],
                            y_mm: y + lh_of(para, frame, base, pfont.as_deref()),
                            from_body: true,
                            byte0: para_byte0,
                            cell: None,
                        });
                        let mut rest = para.clone();
                        if let Some(r0) = rest.runs.first_mut() {
                            r0.text = r0.text[cap_len..].to_string();
                        }
                        owned_rest = Some(rest);
                    }
                }
                let para_eff: &Paragraph = owned_rest.as_ref().unwrap_or(para);
                let measure = (measure - cap_shift).max(em);
                let first_mm = first_line_mm(para_eff, base);
                let gyou = break_para(para_eff, m, measure, marker.as_deref(),
                                      doc.hyphenate, &mut note_no, base);
                let gyou_kazu = gyou.len();
                for (line_no, mut cells) in gyou.into_iter().enumerate() {
                    // 1行目だけ字下げのぶん右へ(行長は組み手が縮めている)
                    let indent_of = if line_no == 0 { first_mm } else { 0.0 };
                    // 頭の1字を除いたぶん、バイト位置を戻す
                    if cap_len > 0 {
                        for c in &mut cells {
                            c.off += cap_len;
                        }
                    }
                    // **絵が行より高ければ、行を絵の高さまで広げます。**
                    // Word と同じで、絵の下端が字のベースラインに乗り、
                    // はみ出す分は上へ伸びます。広げないと前の行に重なります
                    let e_h = if line_no == 0 { atama_no_gazou_takasa(para_eff) } else { 0.0 };
                    let hikui = lh_of(para, frame, base, pfont.as_deref());
                    if e_h > hikui {
                        y += e_h - hikui;
                    }
                    if cells.is_empty() {
                        // 空の段落も**行として持つ**。持たないと、後ろの行の
                        // バイト勘定が1つずつずれて、カーソルが本文とずれる
                        sheet.lines.push(Line {
                            cells: Vec::new(), y_mm: y, from_body: true,
                            byte0: para_byte0 + cap_len, cell: None });
                        // 字が無くても絵は置きます(絵だけの段落)
                        if line_no == 0 && e_h > 0.0 {
                            let hiroi: f32 = atama_no_gazou_mm(para_eff);
                            let aki = (measure - hiroi).max(0.0);
                            let mut ix = indent_mm + cap_shift + match para.align {
                                Align::Center => aki / 2.0,
                                Align::Right => aki,
                                _ => 0.0,
                            };
                            for im in para_eff.images.iter().chain(para_eff.images_new.iter()) {
                                if im.off != 0 {
                                    continue;
                                }
                                sheet.images.push((
                                    im.bytes.clone(),
                                    [ix, y - im.h_mm, im.w_mm, im.h_mm],
                                ));
                                ix += im.w_mm;
                            }
                        }
                        y += hikui;
                        continue;
                    }
                    // 揃え。**行の幅と行長の差を、どこに置くか**の話でしかない
                    let w: f32 = cells.iter().map(|c| c.w_mm).sum();
                    let slack = (measure - indent_of - w).max(0.0);
                    let mut x = indent_mm + cap_shift + indent_of + match para.align {
                        Align::Left | Align::Justify | Align::Distribute => 0.0,
                        Align::Center => slack / 2.0,
                        Align::Right => slack,
                    };
                    // 均等割付: 差を字間に等しく配る(最後の行も配る)。
                    //
                    // **両端揃え(docx の `w:jc="both"`)は最後の行を配りません**
                    // (2026-09-01)。前は左揃えと同じ扱いだったので、右端が
                    // 揃わず、内閣府の調査票は行末が元より最大 26pt 手前で
                    // 終わっていました。
                    let owari = line_no + 1 == gyou_kazu;
                    let kubaru = para.align == Align::Distribute
                        || (para.align == Align::Justify && !owari);
                    let gap = if kubaru && cells.len() >= 2 {
                        slack / (cells.len() - 1) as f32
                    } else {
                        0.0
                    };
                    // **頭の画像は、この行の字の左に置きます**(2026-09-01)。
                    // 幅は `first_line_mm` が字下げとして空けてあるので、
                    // 揃えの計算はもう済んでいます
                    if line_no == 0 {
                        let mut ix = x - atama_no_gazou_mm(para_eff);
                        for im in para_eff.images.iter().chain(para_eff.images_new.iter()) {
                            if im.off != 0 {
                                continue;
                            }
                            // 絵の下端を行のベースラインに合わせます(Word と同じ)
                            sheet.images.push((
                                im.bytes.clone(),
                                [ix, y - im.h_mm, im.w_mm, im.h_mm],
                            ));
                            ix += im.w_mm;
                        }
                    }
                    let cells: Vec<Cell> = cells
                        .into_iter()
                        .map(|mut c| { c.x_mm = x; x += c.w_mm + gap; c })
                        .collect();
                    // ルビ。同じ読みの連なりの上に、半分の大きさの行を置く。
                    // 基底より狭ければ中付き(字間を等配)、広ければ中央から
                    // はみ出す(v1 — 基底を広げる詰めはまだしない)
                    {
                        let mut i = 0usize;
                        while i < cells.len() {
                            let Some(rt) = cells[i].fmt.ruby.clone() else {
                                i += 1;
                                continue;
                            };
                            let mut j = i + 1;
                            while j < cells.len()
                                && cells[j].fmt.ruby.as_deref() == Some(rt.as_str())
                            {
                                j += 1;
                            }
                            let x0 = cells[i].x_mm;
                            let x1 = cells[j - 1].x_mm + cells[j - 1].w_mm;
                            let pt = cells[i].size_pt / 2.0;
                            let rw: f32 =
                                rt.chars().map(|c| m.advance_mm(c, pt)).sum();
                            let n = rt.chars().count();
                            let gap = if n >= 1 && rw < (x1 - x0) {
                                ((x1 - x0) - rw) / (n as f32 + 1.0)
                            } else {
                                0.0
                            };
                            let mut rx = if gap > 0.0 {
                                x0 + gap
                            } else {
                                (x0 + x1 - rw) / 2.0
                            };
                            let mut rcells = Vec::new();
                            for ch in rt.chars() {
                                let w = m.advance_mm(ch, pt);
                                rcells.push(Cell {
                                    ch,
                                    x_mm: rx,
                                    w_mm: w,
                                    size_pt: pt,
                                    off: cells[i].off,
                                    fmt: CharFormat::default(),
                                    font: cells[i].font.clone(),
                                });
                                rx += w + gap;
                            }
                            sheet.lines.push(Line {
                                cells: rcells,
                                // 行送りの空き(黄金比の余白)の中、基底の頭の上
                                y_mm: y - frame.line_height_mm * 0.45,
                                from_body: false,
                                byte0: para_byte0 + cells[i].off,
                                cell: None,
                            });
                            i = j;
                        }
                    }
                    // 行頭の字の段落内位置から、本文の絶対位置を出す。
                    // 箇条書きの印は off=0 で入っているので、最小値を取れば
                    // 1行目(印+本文頭)も続きの行も正しく出る
                    let byte0 = para_byte0
                        + cells.iter().map(|c| c.off).min().unwrap_or(0);
                    sheet.lines.push(Line { cells, y_mm: y, from_body: true, byte0, cell: None });
                    y += lh_of(para, frame, base, pfont.as_deref());
                }
                // **段落の罫線**(docx の `w:pBdr`)。記入欄の下線はこれです。
                // 前は「囲みが付いている」という札だけで、辺の区別も無く、
                // 紙にも出していませんでした(2026-09-01 発注者
                // 「このラインはなんでできていますか」)。
                //
                // `w:between` は同じ指定の段落が続くときの間の線ですが、
                // どちらも段落の下に1本引けば同じ見え方になります
                if para.border.aru() {
                    let (x0, x1) = (indent_mm, indent_mm + measure);
                    let lh = lh_of(para, frame, base, pfont.as_deref());
                    // **線は段落の下端に引きます。**`y` はもう次の行の
                    // ベースラインなので、そこまで下げると次の段落の字に
                    // 掛かります。字の足のぶんだけ戻した所が下端です
                    // (行の箱の中でベースラインが上から [`BASE_UP_MM`])
                    let asi = lh * (LINE_MM - BASE_UP_MM) / LINE_MM;
                    let sita = y - lh + asi;
                    let ue = shade_top - lh * 0.8;
                    if para.border.top {
                        sheet.rules.push([x0, ue, x1, ue]);
                    }
                    if para.border.bottom || para.border.between {
                        sheet.rules.push([x0, sita, x1, sita]);
                    }
                    if para.border.left {
                        sheet.rules.push([x0, ue, x0, sita]);
                    }
                    if para.border.right {
                        sheet.rules.push([x1, ue, x1, sita]);
                    }
                }
                // **段落の背景色**(2026-08-27)。模型に在り、画面は塗って
                // いたのに、組む所で落としていたので紙と PDF に出ていません
                // でした。註記の帯も見出しの背景も印刷で消えます
                if let Some(c) = para.shade.as_deref() {
                    let h = (y - shade_top).max(lh_of(para, frame, base, pfont.as_deref()));
                    sheet.fills.push((
                        [indent_mm, shade_top - lh_of(para, frame, base, pfont.as_deref()) * 0.8, measure, h],
                        c.to_string(),
                    ));
                }
                // **段落の後の空き**(前の空きと同じ決め方)
                y += space_after_mm(para, base);
                // 画像は段落の下に置く。幅が行長を超えるなら比例で縮める
                for im in para.images.iter().chain(para.images_new.iter()) {
                    // 頭の画像はもう1行目の中に置いてあります
                    if im.off == 0 {
                        continue;
                    }
                    let scale = if im.w_mm > measure { measure / im.w_mm } else { 1.0 };
                    let (w, h) = (im.w_mm * scale, im.h_mm * scale);
                    sheet.images.push((im.bytes.clone(), [indent_mm, y - lh_of(para, frame, base, pfont.as_deref()) * 0.6, w, h]));
                    y += h + frame.line_height_mm * 0.4;
                }
                // 次の段落の頭 = この段落のバイト数 + 改行1つ
                let plen: usize = para.runs.iter().map(|r| r.text.len()).sum();
                para_byte0 += plen + 1;
                // 節の切れ目。**この段落で節が終わる**ので、次の段落は新しい紙から。
                // 折る側は breaks を見て頁を割り、sect_pages で用紙を引き直す
                if let Some(sb) = &para.sect {
                    if let Some(next) = sect_geo.get(bi + 1).copied() {
                        let here = sect_geo[bi];
                        // **紙の大きさが同じなら、continuous は頁を割らない。**
                        // 段組みを変えるためだけの節がそれで、割ると見た目が変わる。
                        // 大きさが違えば割るしかない — 1枚の紙は1つの大きさしか
                        // 取れないので、continuous でも従えない
                        let same = (here.w_mm - next.w_mm).abs() < 0.01
                            && (here.h_mm - next.h_mm).abs() < 0.01;
                        if !(sb.continuous && same) {
                            sheet.breaks.push(y);
                            sheet.sect_pages.push((y, next));
                        }
                    }
                }
            }
            Block::Table(table) => {
                y = layout_table(table, m, frame, y, &mut sheet, table_no, doc.hyphenate,
                                 &mut note_no, base, doc);
                table_no += 1;
            }
        }
    }
    layout_notes(doc, m, frame, &mut sheet);
    sheet
}

/// 紙面の下に出す脚注を組む。
///
/// 本文を組んだ**あと**に、置かれた印([`CharFormat::footnote`] を持つ字)を
/// なぞって拾う — 印は本文と同じ行に居るので、その行の y がそのまま
/// 「どのページの下に出すか」の手がかりになる。
///
/// 番号は本文の印と同じ数(出てくる順)。脚注の中の文字は本文より小さく組む。
pub(super) fn layout_notes(doc: &Document, m: &Metrics, frame: &Frame, sheet: &mut Sheet) {
    if doc.footnotes.is_empty() {
        return;
    }
    let base = doc.base_pt();
    // 印のある行を、出てくる順に拾う。番号の字は1桁ずつ別の Cell になるので、
    // **同じ印が続くぶんは1つに畳む**(2桁の脚注を2つと数えない)。
    // 畳むときも id だけで見ない — 下と同じ理由で、脚注と文末脚注は
    // 同じ id を持ちうる
    let mut anchors: Vec<(FootnoteRef, f32)> = Vec::new();
    for line in &sheet.lines {
        let mut last: Option<FootnoteRef> = None;
        for c in &line.cells {
            match &c.fmt.footnote {
                Some(fr) => {
                    if last.as_ref() != Some(fr) {
                        anchors.push((fr.clone(), line.y_mm));
                        last = Some(fr.clone());
                    }
                }
                None => last = None,
            }
        }
    }
    // 脚注の行の高さ。本文より小さく組む(Word の作法に近い比)
    let note_lh = frame.line_height_mm * 0.82;
    // 番号は本文の印と**同じ数え方**でなければ意味がない。だから
    // ここでも脚注と文末脚注を別々に、同じ順で数え直す
    let mut count = NoteCount::of(doc);
    // 文末脚注は紙の下ではなく**文書の末尾**へ。本文の行として後ろに足すので、
    // 普通に頁をまたいで流れる(Word もそこに集める)
    let mut tail: Vec<NoteBlock> = Vec::new();

    for (fr, at_y) in anchors.iter() {
        // **id だけで引いてはいけない。** docx は footnotes.xml と
        // endnotes.xml を別々に番号付けするので、どちらも 1・2・3… から
        // 始まり **id は必ず衝突する**。脚注か文末脚注かまで見て一意になる
        let Some(note) = doc.footnotes.iter()
            .find(|n| n.id == fr.id && n.endnote == fr.endnote)
        else {
            // 印はあるのに中身が引けない。**作り話をせず、出さない**
            continue;
        };
        let label = count.next(fr.endnote);
        let mut lines: Vec<Line> = Vec::new();
        let mut y = 0.0f32;
        for (pi, para) in note.paragraphs.iter().enumerate() {
            // 番号は注の頭に置く。箇条書きの印と同じ扱いで**本文の字ではない**
            let marker = (pi == 0).then(|| format!("{label} "));
            let mut throwaway = NoteCount::default();
            for cells in break_para(para, m, frame.measure_mm, marker.as_deref(),
                                    doc.hyphenate, &mut throwaway, base) {
                let mut x = 0.0f32;
                let cells: Vec<Cell> = cells.into_iter()
                    .map(|mut c| { c.x_mm = x; x += c.w_mm; c })
                    .collect();
                y += note_lh;
                lines.push(Line { cells, y_mm: y, from_body: false, byte0: 0, cell: None });
            }
        }
        if lines.is_empty() {
            continue;
        }
        let block = NoteBlock { no: 0, at_y: *at_y, lines, h_mm: y };
        if fr.endnote {
            tail.push(block);
        } else {
            sheet.notes.push(block);
        }
    }

    // 文末脚注を本文の後ろへ流す。**紙の下(sheet.notes)には入れない** —
    // 入れると印のあるページの下に出てしまい、置き場が違う
    if !tail.is_empty() {
        let mut y = sheet.lines.iter().map(|l| l.y_mm).fold(frame.y0_mm, f32::max);
        // 本文との間を1行あける(仕切りは引かない — 文末は改まった場所なので)
        y += frame.line_height_mm;
        for b in tail {
            for l in b.lines {
                y += 0.0;
                sheet.lines.push(Line { y_mm: y + l.y_mm, ..l });
            }
            y += b.h_mm;
        }
    }
}

/// ヘッダー(footer=false)・フッター(footer=true)を**1ページぶん**組む。
///
/// [`PAGE_MARK`] はそのページの番号、[`PAGES_MARK`] は総頁の字に置き換わる。
/// **表示専用** — 置き換えでバイト位置が変わるので、行の byte0 は編集と結ばない
/// (ヘッダーの編集は紙面上ではなくパネルで行う)。
/// y はページ上端からの mm、x は左余白からの mm(本文の行と同じ物差し)。
// **引数を束ねません。** どれも別々の物で、まとめた構造体を作ると
// 「何を渡したか」が呼ぶ側から見えなくなります(組版の位置は間違えても
// 静かにずれるだけなので、渡す物が目に見えている方が安全です)
#[allow(clippy::too_many_arguments)]
pub fn layout_hf(
    hf: &HeadFoot,
    m: &Metrics,
    pg: &PageSetup,
    line_height_mm: f32,
    page_no: usize,
    total: usize,
    footer: bool,
    base_pt: f32,
) -> Vec<Line> {
    if hf.paragraphs.is_empty() {
        return Vec::new();
    }
    let num = page_no.to_string();
    let tot = total.to_string();
    let measure = pg.measure_mm();
    // ヘッダーは上余白の中を上から、フッターは下余白の頭から下へ
    let mut y = if footer {
        pg.h_mm - pg.bottom_mm + line_height_mm * 0.8
    } else {
        (pg.top_mm * 0.45).max(line_height_mm * 0.8)
    };
    let mut out = Vec::new();
    for para in &hf.paragraphs {
        let mut para = para.clone();
        for r in &mut para.runs {
            if r.text.contains(PAGE_MARK) {
                r.text = r.text.replace(PAGE_MARK, &num);
            }
            if r.text.contains(PAGES_MARK) {
                r.text = r.text.replace(PAGES_MARK, &tot);
            }
        }
        for cells in break_para(&para, m, measure, None, false, &mut NoteCount::default(),
                                base_pt) {
            let w: f32 = cells.iter().map(|c| c.w_mm).sum();
            let slack = (measure - w).max(0.0);
            let mut x = match para.align {
                Align::Left | Align::Justify | Align::Distribute => 0.0,
                Align::Center => slack / 2.0,
                Align::Right => slack,
            };
            let gap = if para.align == Align::Distribute && cells.len() >= 2 {
                slack / (cells.len() - 1) as f32
            } else {
                0.0
            };
            let cells: Vec<Cell> = cells
                .into_iter()
                .map(|mut c| {
                    c.x_mm = x;
                    x += c.w_mm + gap;
                    c
                })
                .collect();
            out.push(Line { cells, y_mm: y, from_body: false, byte0: 0, cell: None });
            y += line_height_mm;
        }
    }
    out
}

/// 段組み。**細い行長(column_measure_mm)で組んだ巻物**を、
/// ページごとに n 段へ折る。
///
/// 出てくる座標は「ページを縦に積み上げた物理座標」— y はページの並び、
/// x は段の位置へずらし済み。だから画面もクリックもキャレットも PDF も、
/// 座標を使う側は**何も変えずに**そのまま写せる。
/// ページの頭には breaks を置くので、紙に写す側はそこで頁を割る。
/// 縦書き(K4)。横に組んだ巻物を、右から左への列に写す。
/// 行 k → 右から k 本目の列。vert_x[i] が列の左肩の x(絶対 mm)、
/// Line.y_mm はその物理ページの本文の上端、Cell.x_mm は上からの距離。
/// ルビの行(基底より 0.45 行ぶん上)は基底の列の右肩に寄る。
/// 約物は縦用の字形へ置き換える(フォントの vert の近似)。
/// 初版の約束: 表・段組みとの併用はしない(呼ぶ側が避ける)。
/// 明示の改ページは列送りに畳まれる(頁の頭には来ない)
pub fn fold_vertical(sheet: &mut Sheet, pg: &PageSetup, y0_mm: f32, line_mm: f32) {
    sheet.vertical = true;
    let usable_w = (pg.w_mm - pg.left_mm - pg.right_mm).max(line_mm);
    let cpp = (usable_w / line_mm).floor().max(1.0) as usize; // 1頁の列数
    let right = pg.w_mm - pg.right_mm;
    sheet.breaks.clear();
    sheet.vert_x = Vec::with_capacity(sheet.lines.len());
    let mut max_page = 0usize;
    for line in &mut sheet.lines {
        let col_f = (line.y_mm - y0_mm) / line_mm;
        let col = col_f.round().max(0.0) as usize;
        let frac = col as f32 - col_f; // ルビなら正(基底の右へ)
        let (page, cip) = (col / cpp, col % cpp);
        max_page = max_page.max(page);
        sheet.vert_x.push(right - (cip as f32 + 1.0) * line_mm + frac * line_mm);
        line.y_mm = page as f32 * pg.h_mm + pg.top_mm;
        for c in &mut line.cells {
            c.ch = match c.ch {
                '、' => '︑',
                '。' => '︒',
                'ー' => '丨',
                '「' => '﹁',
                '」' => '﹂',
                '『' => '﹃',
                '』' => '﹄',
                '(' => '︵',
                ')' => '︶',
                '…' => '︙',
                other => other,
            };
        }
    }
    // ページの頭(物理座標)。紙に写す側がこの目印で頁を割る
    sheet.breaks = (1..=max_page).map(|k| k as f32 * pg.h_mm).collect();
}

/// 複数ページ(見開き)。巻物のページを横 n 枚ずつ並べる(画面だけの
/// 見え方 — 紙は1ページずつのまま)。offsets は paginate が出したページの
/// 頭(巻物の y)。gap はページの間の空き mm。
/// **座標を変えるだけ**なので、描く側も当たり判定も無変更で効く
/// 巻物を**1ページずつ縦に積む**(印刷モードの折り方)。
///
/// 通常の編集の画面は**切れ目の無い巻物**で、頁の間隔は紙の高さより
/// 詰まっている(余白ぶん)。実測で紙 297mm に対し間隔 260mm — だから
/// 紙の絵をそのまま後ろに敷くと 37mm ずつ重なる。**中身を折り直す**しかない。
///
/// [`fold_pages`](fold_pages) の見開きと違い、**頁ごとに紙が違ってよい**
/// (節で縦から横に変わる文書)。`papers` は頁ごとの用紙で、`offsets` と
/// 同じ数だけ要る。足りない分は最後の紙を使う。
///
/// 返すのは**頁ごとの上端**(折った後の y)。紙の絵はここへ置く。
///
/// `starts` は**各頁に載る最初の行の y**(1枚目は `-∞`)で、どの頁に属するかは
/// これで決める。`offsets`(紙の上端)は最初の行より余白ぶん上にあり、巻物は
/// 空きを詰めて流れるので、境として使うと前の頁の末尾が次の頁へ化ける
/// (2026-08-17、発表の組み方で踏んだ)。頁の中の位置は `offsets` から測る。
pub fn fold_print(
    sheet: &mut Sheet,
    papers: &[PageSetup],
    offsets: &[f32],
    starts: &[f32],
    gap: f32,
) -> Vec<f32> {
    let paper_of = |k: usize| -> PageSetup {
        papers.get(k).copied().or_else(|| papers.last().copied()).unwrap_or_default()
    };
    // 頁ごとの上端を先に積む(紙の高さは頁ごとに違う)
    let mut tops = Vec::with_capacity(offsets.len());
    let mut y = 0.0f32;
    for k in 0..offsets.len().max(1) {
        tops.push(y);
        y += paper_of(k).h_mm + gap;
    }
    if offsets.len() <= 1 {
        return tops;
    }
    let page_of = |y: f32| -> (usize, f32) {
        let mut k = 0usize;
        for (i, s) in starts.iter().enumerate() {
            if y >= *s - 0.01 {
                k = i;
            }
        }
        (k, y - offsets[k])
    };
    // 巻物の y → 折った後の y。中身は自分の頁の中の位置を保つ
    let shift = |y: f32| -> f32 {
        let (k, inner) = page_of(y);
        tops[k] + inner
    };
    for line in &mut sheet.lines {
        line.y_mm = shift(line.y_mm);
    }
    for r in &mut sheet.rules {
        let h = r[3] - r[1];
        r[1] = shift(r[1]);
        r[3] = r[1] + h;
    }
    for b in &mut sheet.cell_boxes {
        let h = b.h_mm;
        b.top_mm = shift(b.top_mm);
        b.h_mm = h;
    }
    for (_, im) in &mut sheet.images {
        let h = im[3];
        im[1] = shift(im[1]);
        im[3] = h;
    }
    // 脚注は「印のある行」を手掛かりに置くので、その y も折る
    for nb in &mut sheet.notes {
        nb.at_y = shift(nb.at_y);
    }
    sheet.breaks = tops.iter().skip(1).copied().collect();
    tops
}

pub fn fold_pages(
    sheet: &mut Sheet,
    pg: &PageSetup,
    offsets: &[f32],
    starts: &[f32],
    n: usize,
    gap: f32,
) {
    if n <= 1 || offsets.len() <= 1 {
        return;
    }
    let step = pg.w_mm + gap;
    // 巻物の y → (ページ番号, ページ内の y)
    let page_of = |y: f32| -> (usize, f32) {
        let mut k = 0usize;
        for (i, s) in starts.iter().enumerate() {
            if y >= *s - 0.01 {
                k = i;
            }
        }
        (k, y - offsets[k])
    };
    let shift = |y: f32| -> (f32, f32) {
        let (k, inner) = page_of(y);
        // 横は列、縦は段(行送りの巻物ではなく物理ページの高さで積む)
        ((k % n) as f32 * step, (k / n) as f32 * pg.h_mm + inner)
    };
    for line in &mut sheet.lines {
        let (dx, ny) = shift(line.y_mm);
        line.y_mm = ny;
        for c in &mut line.cells {
            c.x_mm += dx;
        }
    }
    for r in &mut sheet.rules {
        let (dx, ny) = shift(r[1]);
        let h = r[3] - r[1];
        r[0] += dx;
        r[2] += dx;
        r[1] = ny;
        r[3] = ny + h;
    }
    for (_, b) in &mut sheet.images {
        let (dx, ny) = shift(b[1]);
        b[0] += dx;
        b[1] = ny;
    }
    for cb in &mut sheet.cell_boxes {
        let (dx, ny) = shift(cb.top_mm);
        cb.x_mm += dx;
        cb.top_mm = ny;
    }
    sheet.breaks.clear();
}

pub fn fold_columns(sheet: &mut Sheet, pg: &PageSetup, y0_mm: f32) {
    let n = pg.cols();
    if n <= 1 {
        return;
    }
    let col_w = pg.column_measure_mm();
    let span = (pg.h_mm - pg.bottom_mm - y0_mm).max(10.0); // 1段に入る高さ
    // 第1走: 行を順に歩き、どの段(strip)に入るかを決める。
    // 巻物の y の区間 → 段、の対応も控える(罫線・画像を同じ折り方にするため)
    let mut strip = 0usize;
    let mut strip_y0 = y0_mm; // いまの段の頭(巻物の座標)
    let mut ranges: Vec<(f32, usize)> = vec![(strip_y0, 0)]; // (巻物のy起点, 段)
    let mut line_strip: Vec<usize> = Vec::with_capacity(sheet.lines.len());
    let mut breaks = std::mem::take(&mut sheet.breaks).into_iter().peekable();
    for line in &sheet.lines {
        // 明示の改ページ: 次のページの頭(= n の倍数の段)へ
        let mut forced = false;
        while let Some(&b) = breaks.peek() {
            if line.y_mm >= b - 0.01 {
                breaks.next();
                forced = true;
            } else {
                break;
            }
        }
        if forced {
            strip = (strip / n + 1) * n;
            strip_y0 = line.y_mm;
            ranges.push((strip_y0, strip));
        } else if line.y_mm - strip_y0 > span {
            strip += 1;
            strip_y0 = line.y_mm;
            ranges.push((strip_y0, strip));
        }
        line_strip.push(strip);
    }
    // 巻物の y → 段(罫線・画像・当たり判定に使う)
    let strip_of = |y: f32| -> usize {
        let mut s = 0usize;
        for (y0, k) in &ranges {
            if y >= *y0 - 0.01 {
                s = *k;
            }
        }
        s
    };
    // その段の起点(巻物の座標)。中身の無い段は最後の起点を使う
    let strip_start = |k: usize| -> f32 {
        ranges.iter().filter(|(_, s)| *s <= k).map(|(y0, _)| *y0).next_back().unwrap_or(y0_mm)
    };
    // 折る: y はページの積み上げへ、x は段の位置へ
    let place = |y: f32, k: usize| -> f32 {
        let page = k / n;
        page as f32 * pg.h_mm + y0_mm + (y - strip_start(k))
    };
    let dx = |k: usize| -> f32 { (k % n) as f32 * (col_w + COLUMN_GAP_MM) };
    for (line, k) in sheet.lines.iter_mut().zip(&line_strip) {
        line.y_mm = place(line.y_mm, *k);
        for c in &mut line.cells {
            c.x_mm += dx(*k);
        }
    }
    for r in &mut sheet.rules {
        let k = strip_of(r[1].min(r[3]));
        let (y1, y2) = (place(r[1], k), place(r[3], k));
        r[0] += dx(k);
        r[2] += dx(k);
        r[1] = y1;
        r[3] = y2;
    }
    for (_, rect) in &mut sheet.images {
        let k = strip_of(rect[1]);
        rect[1] = place(rect[1], k);
        rect[0] += dx(k);
    }
    for b in &mut sheet.cell_boxes {
        let k = strip_of(b.top_mm);
        b.top_mm = place(b.top_mm, k);
        b.x_mm += dx(k);
    }
    // 紙に写す側のために、2ページ目からの頭に改ページを置く
    let pages = line_strip.iter().map(|k| k / n + 1).max().unwrap_or(1);
    sheet.breaks = (1..pages).map(|p| p as f32 * pg.h_mm + y0_mm).collect();
}

/// 表を組む。戻り値は表の下の、次のベースライン。
///
/// セル結合を含めて組む:
/// - 横(gridSpan): セルが複数の格子を占め、幅はその合計
/// - 縦(vMerge): Continue のセルは描かず、Start のセルが行をまたいで延びる
///
/// 罫線は「格子」ではなく**結合後のセルの縁**に引く — 結合の中を
/// 線が横切ると、様式の枠が壊れて見える。
// 上と同じ理由で束ねません
#[allow(clippy::too_many_arguments)]
pub(super) fn layout_table(table: &Table, m: &Metrics, frame: &Frame, y_in: f32, sheet: &mut Sheet,
                table_no: usize, hyphenate: bool, notes: &mut NoteCount, base: f32,
                doc: &Document) -> f32 {
    // 列数は「セルの数」ではなく「セルが占める格子の数」
    let ncols = table
        .rows
        .iter()
        .map(|r| r.iter().map(|c| c.span()).sum::<usize>())
        .max()
        .unwrap_or(1)
        .max(1);
    // 列幅。指定があればそれを使い、行長に収まらなければ**比例で縮める**
    // (右へ黙ってはみ出すより、比率を守って縮む方が様式の見た目が保たれる)
    let widths: Vec<f32> = if table.col_mm.len() == ncols
        && table.col_mm.iter().all(|w| *w > 0.5)
    {
        let total: f32 = table.col_mm.iter().sum();
        if total > frame.measure_mm {
            let k = frame.measure_mm / total;
            table.col_mm.iter().map(|w| w * k).collect()
        } else {
            table.col_mm.clone()
        }
    } else {
        vec![frame.measure_mm / ncols as f32; ncols]
    };
    // 列の左端(累積)
    let mut xs = vec![0.0f32];
    for w in &widths {
        xs.push(xs.last().unwrap() + w);
    }
    let lh = frame.line_height_mm;

    // 表の上端。直前のベースラインから少し空ける
    let table_top = y_in - lh * 0.55;

    // 第1走: 各セルを折り、格子の位置と行の高さを決める。
    // 行はセルの文章(段落を \n で繋いだもの)の中のバイト位置を持つ
    struct Laid {
        ci: usize,     // 行の中の何番目のセルか(編集はこの番号で結ぶ)
        gc: usize,     // 占める格子の左端
        span: usize,
        v: VMerge,
        /// 行の字と、升の中でのバイト位置と、その行の高さ(mm)
        lines: Vec<(Vec<Cell>, usize, f32)>,
        x: f32,
        w: f32,
        /// 升の背景色。**升の中の最初の段落の物**を使います
        shade: Option<String>,
    }
    let mut rows_laid: Vec<Vec<Laid>> = Vec::new();
    let mut row_hs: Vec<f32> = Vec::new();
    for row in &table.rows {
        let mut gc = 0usize;
        // **升ごとに高さを足します。** 升の中の段落が別の大きさなら、
        // 行の高さも別です。行の高さはいちばん高い升で決まります
        let mut takasa = lh;
        let mut laid: Vec<Laid> = Vec::new();
        for (ci, cell) in row.iter().enumerate() {
            let span = cell.span().min(ncols.saturating_sub(gc)).max(1);
            let x = xs[gc.min(ncols)];
            let w = xs[(gc + span).min(ncols)] - x;
            let mut ls: Vec<(Vec<Cell>, usize, f32)> = Vec::new();
            // 縦結合の続きは上のセルに呑まれている。中身は組まない
            if cell.v_merge != VMerge::Continue {
                let inner = (w - 2.0 * CELL_PAD).max(2.0);
                let mut para0 = 0usize;
                // **升の中でも箇条書きの印を出します**(2026-08-31)。前は
                // `None` を渡していたので、内閣府の調査票の `○` が8か所
                // 消えていました。番号は升ごとに数え直します
                let mut kazu = 0usize;
                for para in &cell.paragraphs {
                    let pbase = doc.style_pt(para.style_id.as_deref()).unwrap_or(base);
                    let pfont =
                        doc.style_font(para.style_id.as_deref()).or_else(|| doc.font.clone());
                    let plh = lh_of(para, frame, pbase, pfont.as_deref());
                    let mk = match para.list {
                        ListKind::None => None,
                        _ => {
                            kazu += 1;
                            para.marker(kazu - 1)
                        }
                    };
                    for cs in break_para(para, m, inner, mk.as_deref(), hyphenate, notes, pbase) {
                        let b0 = para0 + cs.iter().map(|c| c.off).min().unwrap_or(0);
                        ls.push((cs, b0, plh));
                    }
                    let plen: usize = para.runs.iter().map(|r| r.text.len()).sum();
                    para0 += plen + 1;
                }
                takasa = takasa.max(ls.iter().map(|(_, _, h)| *h).sum::<f32>());
            }
            let shade = cell.paragraphs.first().and_then(|p| p.shade.clone());
            laid.push(Laid { ci, gc, span, v: cell.v_merge, lines: ls, x, w, shade });
            gc += span;
        }
        rows_laid.push(laid);
        row_hs.push(takasa + 2.0 * CELL_PAD);
    }

    // 行の上端(累積)
    let mut tops = vec![table_top];
    for h in &row_hs {
        tops.push(tops.last().unwrap() + h);
    }
    let table_bottom = *tops.last().unwrap();

    // 格子の地図(第2走が中身を消費した後も結合の形を見られるように)
    let grid: Vec<Vec<(usize, usize, VMerge)>> = rows_laid
        .iter()
        .map(|r| r.iter().map(|l| (l.gc, l.span, l.v)).collect())
        .collect();
    // row 行で格子 g を覆うセル
    let cover = |row: usize, g: usize| -> Option<(usize, usize, VMerge)> {
        grid.get(row)?.iter().find(|(gc, span, _)| *gc <= g && g < gc + span).copied()
    };
    // 格子の位置から**セルそのもの**を引く。`grid[row]` の並びは
    // `table.rows[row]` の並びと同じなので、位置がそのまま添字です
    let cell_at = |row: usize, g: usize| -> Option<&Cellbox> {
        let i = grid.get(row)?.iter().position(|(gc, span, _)| *gc <= g && g < gc + span)?;
        table.rows.get(row)?.get(i)
    };
    // (ri, gc) から始まる縦結合の高さ: 同じ格子位置で Continue が続く間
    let merged_h = |ri: usize, gc: usize| -> f32 {
        let mut h = row_hs[ri];
        // `row_hs` は行と同じ数(上の走査で1行ごとに1つ積む)
        for (r, rh) in row_hs.iter().enumerate().skip(ri + 1) {
            match cover(r, gc) {
                Some((g0, _, VMerge::Continue)) if g0 == gc => h += rh,
                _ => break,
            }
        }
        h
    };

    // **見出しの行を持つ表**を覚えます。紙が頁をまたぐとき繰り返します
    if table.header_row && !sheet.header_tables.contains(&table_no) {
        sheet.header_tables.push(table_no);
    }
    // 第2走: 中身と当たり判定(from_body=false。本文の位置合わせに入れない)
    for (ri, laid) in rows_laid.into_iter().enumerate() {
        let row_top = tops[ri];
        for l in laid {
            if l.v == VMerge::Continue {
                continue;
            }
            let h = if l.v == VMerge::Start { merged_h(ri, l.gc) } else { row_hs[ri] };
            let x0 = l.x + CELL_PAD;
            let mut yy = row_top + CELL_PAD;
            let id = Some((table_no, ri, l.ci));
            for (cells, b0, plh) in l.lines {
                yy += plh * 0.8;
                let mut x = x0;
                let cells: Vec<Cell> = cells
                    .into_iter()
                    .map(|mut c| { c.x_mm = x; x += c.w_mm; c })
                    .collect();
                sheet.lines.push(Line { cells, y_mm: yy, from_body: false, byte0: b0, cell: id });
                yy += plh * 0.2;
            }
            // **升の塗り**(2026-08-27)。段落の背景色は模型に在り、画面は
            // 塗っていたのに、**組む所で落としていた**ので紙と PDF に出て
            // いませんでした。註記の帯も見出しの背景も印刷で消えます。
            // 罫線より先に敷くよう、`fills` は `rules` と別に持ちます
            if let Some(c) = l.shade.as_deref() {
                sheet.fills.push(([l.x, row_top, l.w, h], c.to_string()));
            }
            // クリックの当たり判定(結合したセルは結合後の大きさで当てる)
            sheet.cell_boxes.push(CellBox {
                table: table_no,
                row: ri,
                col: l.ci,
                x_mm: l.x,
                top_mm: row_top,
                w_mm: l.w,
                h_mm: h,
            });
        }
    }

    // 罫線・横: 行の境ごとに、格子を歩いて「引ける区間」を繋いで引く。
    // 縦結合の中を横切る線は引かない
    // `tops` は行より1つ多い(上端に加えて最後の下端を持つ)ので、
    // そのまま歩けば行の境を全部通る
    for (b, &y) in tops.iter().enumerate() {
        // **その辺を引く決まりか**(2026-08-30)。docx の `w:tblBorders` に
        // 挙がっていない辺は引きません。前は必ず四方に引いていたので、
        // 下線だけの様式が枠だらけになっていました
        let hiku_yoko = if b == 0 {
            table.borders.top
        } else if b >= grid.len() {
            table.borders.bottom
        } else {
            table.borders.inside_h
        };
        // **桁ごとに決めてから繋ぎます。** セルごとに指定が違う様式
        // (記入欄だけ下線)があるので、先に区間をまとめてしまうと
        // 先頭のセルの指定で塗り潰されます
        let hiku_at = |g: usize| -> bool {
            // 境の下の行が Continue なら、この格子の上に線は引かない
            if b > 0 && b < grid.len()
                && matches!(cover(b, g), Some((_, _, VMerge::Continue)))
            {
                return false;
            }
            // セルの指定が表の指定より強い。上の行の「下」と、
            // 下の行の「上」のどちらかが言っていればそちらに従います
            let ue = b.checked_sub(1).and_then(|r| cell_at(r, g)).and_then(|c| c.borders.bottom);
            let shita = cell_at(b, g).and_then(|c| c.borders.top);
            ue.or(shita).unwrap_or(hiku_yoko)
        };
        let mut g = 0usize;
        while g < ncols {
            if !hiku_at(g) {
                g += 1;
                continue;
            }
            let start = g;
            while g < ncols && hiku_at(g) {
                g += 1;
            }
            sheet.rules.push([xs[start], y, xs[g], y]);
        }
    }
    // 罫線・縦: 行ごとに、結合後のセルの縁に引く(結合の中には引かない)
    for (ri, row) in grid.iter().enumerate() {
        let (top, bottom) = (tops[ri], tops[ri + 1]);
        let mut edges: Vec<f32> = Vec::new();
        for (gc, span, _) in row {
            edges.push(xs[*gc]);
            edges.push(xs[(gc + span).min(ncols)]);
        }
        if row.is_empty() {
            edges.push(xs[0]);
            edges.push(xs[ncols]);
        }
        edges.sort_by(|a, b| a.partial_cmp(b).unwrap());
        edges.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        for x in edges {
            // 左端・右端・その間で、引く決まりが違います
            let hiku = if (x - xs[0]).abs() < 0.01 {
                table.borders.left
            } else if (x - xs[ncols]).abs() < 0.01 {
                table.borders.right
            } else {
                table.borders.inside_v
            };
            if hiku {
                sheet.rules.push([x, top, x, bottom]);
            }
        }
    }
    // 次のベースライン
    table_bottom + lh
}

/// 註記のスタイル名 → 紙に出す見出し。
///
/// 読み手は `NOTE: ` を字から外して、どれなのかをスタイルの名前に移します。
/// **紙の上では字しか見えない**ので、組むときに見出しを戻します。
/// 戻さないと、註記が普通の段落に化けます(2026-08-25)。
pub(super) fn admon_heading(name: Option<&str>) -> Option<&'static str> {
    Some(match name? {
        "註記" => "メモ ",
        "ヒント" => "こつ ",
        "重要" => "大事 ",
        "警告" => "警告 ",
        "注意" => "注意 ",
        _ => return None,
    })
}

/// 作業のリストの行を、印・本文・段に割る。
///
/// `* [ ] やること` は `(☐ , "やること", 0)`、`** [x] 済み` は
/// `(☑ , "済み", 1)` です。作業のリストでなければ `None`。
///
/// **紙では ☐ / ☑ で出します。** 記入欄のチェックボックスと同じ字なので、
/// 画面の中で見た目が揃います(2026-08-25。前は `* [ ]` がそのまま
/// 印刷されていました)。
fn task_list(p: &Paragraph) -> Option<(&'static str, String, u8)> {
    if p.style_id.as_deref() != Some("チェック") {
        return None;
    }
    let text: String = p.runs.iter().map(|r| r.text.as_str()).collect();
    // 印は `*` でも `-`(Markdown の書き方)でもよい
    let head = text.chars().next().unwrap_or('*');
    let stars = text.chars().take_while(|c| *c == head).count();
    let rest = text.trim_start_matches(head).trim_start();
    let (mark, body) = if let Some(r) = rest.strip_prefix("[x] ").or_else(|| rest.strip_prefix("[X] ")) {
        ("☑ ", r)
    } else {
        ("☐ ", rest.strip_prefix("[ ] ")?)
    };
    Some((mark, body.to_string(), stars.saturating_sub(1) as u8))
}
