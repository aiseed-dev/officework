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
    Space(f32, f32, CharFormat, Option<String>, usize),
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

pub(super) fn tokenize(p: &Paragraph, m: &Metrics, notes: &mut NoteCount) -> Vec<Tok> {
    let mut out = Vec::new();
    // 段落の頭からのバイト位置。run をまたいで通しで数える
    let mut off = 0usize;
    for run in &p.runs {
        // 脚注の印。**番号は出てくる順**(id の数ではない)に振る。
        // 箇条書きの印と同じで**本文の字ではない**ので、
        // off は動かさない — 動かすとカーソルが本文とずれる
        if let Some(fr) = &run.fmt.footnote {
            let label = notes.next(fr.endnote);
            let size = run.size_pt * 0.7;
            let mut fmt = run.fmt.clone();
            fmt.superscript = true;
            for ch in label.chars() {
                out.push(Tok::One(ch, m.advance_mm(ch, size), size,
                                  fmt.clone(), run.font.clone(), off));
            }
            continue;
        }
        let mut word: Vec<(char, f32, usize)> = Vec::new();
        for ch in run.text.chars() {
            if is_word_char(ch) {
                word.push((ch, m.advance_mm(ch, run.size_pt), off));
                off += ch.len_utf8();
                continue;
            }
            if !word.is_empty() {
                out.push(Tok::Word(std::mem::take(&mut word), run.size_pt, run.fmt.clone(),
                                   run.font.clone()));
            }
            if ch == ' ' || ch == '\u{3000}' {
                out.push(Tok::Space(m.advance_mm(ch, run.size_pt), run.size_pt,
                                    run.fmt.clone(), run.font.clone(), off));
            } else {
                out.push(Tok::One(ch, m.advance_mm(ch, run.size_pt), run.size_pt,
                                  run.fmt.clone(), run.font.clone(), off));
            }
            off += ch.len_utf8();
        }
        if !word.is_empty() {
            out.push(Tok::Word(word, run.size_pt, run.fmt.clone(), run.font.clone()));
        }
    }
    out
}

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
pub(super) fn break_para(para: &Paragraph, m: &Metrics, measure: f32, marker: Option<&str>,
              hyphenate: bool, notes: &mut NoteCount) -> Vec<Vec<Cell>> {
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

    // 箇条書きの印は本文の前に置く。**本文の一部にはしない**ので、
    // 編集中の文字位置とずれない(印は組版のときだけ現れる)
    if let Some(mk) = marker {
        let size = para.runs.first().map(|r| r.size_pt).unwrap_or(10.5);
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
    for tok in tokenize(para, m, notes) {
        let (cells, w): (Vec<Cell>, f32) = match &tok {
            Tok::One(ch, w, s, f, ft, o) =>
                (vec![Cell { ch: *ch, x_mm: 0.0, w_mm: *w, size_pt: *s, fmt: f.clone(),
                             font: ft.clone(), off: *o }], *w),
            Tok::Word(cs, s, f, ft) => (
                cs.iter().map(|(c, w, o)| Cell { ch: *c, x_mm: 0.0, w_mm: *w, size_pt: *s,
                                                 fmt: f.clone(), font: ft.clone(), off: *o })
                    .collect(),
                cs.iter().map(|(_, w, _)| *w).sum()),
            Tok::Space(w, s, f, ft, o) =>
                (vec![Cell { ch: ' ', x_mm: 0.0, w_mm: *w, size_pt: *s, fmt: f.clone(),
                             font: ft.clone(), off: *o }], *w),
        };

        if w_cur + w > measure && !cur.is_empty() {
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
        if cur.is_empty() {
            if let Tok::Space(..) = tok {
                continue; // 行頭の空白は組まない
            }
        }
        w_cur += w;
        cur.extend(cells);
    }
    if !cur.is_empty() || done.is_empty() {
        done.push(cur);
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

pub(super) fn lh_of(para: &Paragraph, frame: &Frame) -> f32 {
    frame.line_height_mm * para.spacing()
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
                // 改ページ。紙に写すときにここで頁が割れる
                if para.page_break_before && !sheet.lines.is_empty() {
                    sheet.breaks.push(y);
                }
                // インデント1段 = 全角2文字ぶん(日本の書類の慣習)
                let em = para.runs.first().map(|r| r.size_pt).unwrap_or(10.5) * 25.4 / 72.0;
                let indent_mm = para.indent as f32 * em * 2.0;
                let measure = (block_measure - indent_mm).max(em);
                let marker = match para.list {
                    ListKind::None => {
                        counters.clear();
                        None
                    }
                    _ => {
                        let l = para.indent as usize;
                        counters.truncate(l + 1);
                        while counters.len() <= l {
                            counters.push(0);
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
                        let size0 = first.map(|r| r.size_pt).unwrap_or(10.5);
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
                            y_mm: y + frame.line_height_mm * para.spacing(),
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
                for mut cells in break_para(para_eff, m, measure, marker.as_deref(),
                                            doc.hyphenate, &mut note_no) {
                    // 頭の1字を除いたぶん、バイト位置を戻す
                    if cap_len > 0 {
                        for c in &mut cells {
                            c.off += cap_len;
                        }
                    }
                    if cells.is_empty() {
                        // 空の段落も**行として持つ**。持たないと、後ろの行の
                        // バイト勘定が1つずつずれて、カーソルが本文とずれる
                        sheet.lines.push(Line {
                            cells: Vec::new(), y_mm: y, from_body: true,
                            byte0: para_byte0 + cap_len, cell: None });
                        y += frame.line_height_mm * para.spacing();
                        continue;
                    }
                    // 揃え。**行の幅と行長の差を、どこに置くか**の話でしかない
                    let w: f32 = cells.iter().map(|c| c.w_mm).sum();
                    let slack = (measure - w).max(0.0);
                    let mut x = indent_mm + cap_shift + match para.align {
                        Align::Left | Align::Justify | Align::Distribute => 0.0,
                        Align::Center => slack / 2.0,
                        Align::Right => slack,
                    };
                    // 均等割付: 差を字間に等しく配る(最後の行も配る)
                    let gap = if para.align == Align::Distribute && cells.len() >= 2 {
                        slack / (cells.len() - 1) as f32
                    } else {
                        0.0
                    };
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
                    y += frame.line_height_mm * para.spacing();
                }
                // 画像は段落の下に置く。幅が行長を超えるなら比例で縮める
                for im in para.images.iter().chain(para.images_new.iter()) {
                    let scale = if im.w_mm > measure { measure / im.w_mm } else { 1.0 };
                    let (w, h) = (im.w_mm * scale, im.h_mm * scale);
                    sheet.images.push((im.bytes.clone(), [indent_mm, y - lh_of(para, frame) * 0.6, w, h]));
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
                                 &mut note_no);
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
                                    doc.hyphenate, &mut throwaway) {
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
pub fn layout_hf(
    hf: &HeadFoot,
    m: &Metrics,
    pg: &PageSetup,
    line_height_mm: f32,
    page_no: usize,
    total: usize,
    footer: bool,
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
        for cells in break_para(&para, m, measure, None, false, &mut NoteCount::default()) {
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
pub fn fold_pages(sheet: &mut Sheet, pg: &PageSetup, offsets: &[f32], n: usize, gap: f32) {
    if n <= 1 || offsets.len() <= 1 {
        return;
    }
    let step = pg.w_mm + gap;
    // 巻物の y → (ページ番号, ページ内の y)
    let page_of = |y: f32| -> (usize, f32) {
        let mut k = 0usize;
        for (i, o) in offsets.iter().enumerate() {
            if y >= *o - 0.01 {
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
pub(super) fn layout_table(table: &Table, m: &Metrics, frame: &Frame, y_in: f32, sheet: &mut Sheet,
                table_no: usize, hyphenate: bool, notes: &mut NoteCount) -> f32 {
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
        lines: Vec<(Vec<Cell>, usize)>,
        x: f32,
        w: f32,
    }
    let mut rows_laid: Vec<Vec<Laid>> = Vec::new();
    let mut row_hs: Vec<f32> = Vec::new();
    for row in &table.rows {
        let mut gc = 0usize;
        let mut nlines = 1usize;
        let mut laid: Vec<Laid> = Vec::new();
        for (ci, cell) in row.iter().enumerate() {
            let span = cell.span().min(ncols.saturating_sub(gc)).max(1);
            let x = xs[gc.min(ncols)];
            let w = xs[(gc + span).min(ncols)] - x;
            let mut ls: Vec<(Vec<Cell>, usize)> = Vec::new();
            // 縦結合の続きは上のセルに呑まれている。中身は組まない
            if cell.v_merge != VMerge::Continue {
                let inner = (w - 2.0 * CELL_PAD).max(2.0);
                let mut para0 = 0usize;
                for para in &cell.paragraphs {
                    for cs in break_para(para, m, inner, None, hyphenate, notes) {
                        let b0 = para0 + cs.iter().map(|c| c.off).min().unwrap_or(0);
                        ls.push((cs, b0));
                    }
                    let plen: usize = para.runs.iter().map(|r| r.text.len()).sum();
                    para0 += plen + 1;
                }
                nlines = nlines.max(ls.len());
            }
            laid.push(Laid { ci, gc, span, v: cell.v_merge, lines: ls, x, w });
            gc += span;
        }
        rows_laid.push(laid);
        row_hs.push(nlines as f32 * lh + 2.0 * CELL_PAD);
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
    // (ri, gc) から始まる縦結合の高さ: 同じ格子位置で Continue が続く間
    let merged_h = |ri: usize, gc: usize| -> f32 {
        let mut h = row_hs[ri];
        for r in ri + 1..grid.len() {
            match cover(r, gc) {
                Some((g0, _, VMerge::Continue)) if g0 == gc => h += row_hs[r],
                _ => break,
            }
        }
        h
    };

    // 第2走: 中身と当たり判定(from_body=false。本文の位置合わせに入れない)
    for (ri, laid) in rows_laid.into_iter().enumerate() {
        let row_top = tops[ri];
        for l in laid {
            if l.v == VMerge::Continue {
                continue;
            }
            let h = if l.v == VMerge::Start { merged_h(ri, l.gc) } else { row_hs[ri] };
            let x0 = l.x + CELL_PAD;
            let mut yy = row_top + CELL_PAD + lh * 0.8;
            let id = Some((table_no, ri, l.ci));
            for (cells, b0) in l.lines {
                let mut x = x0;
                let cells: Vec<Cell> = cells
                    .into_iter()
                    .map(|mut c| { c.x_mm = x; x += c.w_mm; c })
                    .collect();
                sheet.lines.push(Line { cells, y_mm: yy, from_body: false, byte0: b0, cell: id });
                yy += lh;
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
    for b in 0..=grid.len() {
        let y = tops[b];
        let mut g = 0usize;
        while g < ncols {
            // 境の下の行が Continue なら、この格子の上に線は引かない
            let blocked = b > 0
                && b < grid.len()
                && matches!(cover(b, g), Some((_, _, VMerge::Continue)));
            if blocked {
                g += 1;
                continue;
            }
            let start = g;
            while g < ncols {
                let blk = b > 0
                    && b < grid.len()
                    && matches!(cover(b, g), Some((_, _, VMerge::Continue)));
                if blk {
                    break;
                }
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
            sheet.rules.push([x, top, x, bottom]);
        }
    }
    // 次のベースライン
    table_bottom + lh
}
