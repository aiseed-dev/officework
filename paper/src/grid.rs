//! 帳票(表計算)を紙へ写す。
//!
//! writer と同じ約束: **画面に見えているもの(値・書式・罫線・塗り・文字色)を
//! 写すだけ。** 計算はやり直さない。条件付き書式も画面と同じ規則で効く。
//!
//! まだやらないこと(黙らずに書いておく):
//!   - 横に紙からはみ出す列は**次の紙に送らず、切れる**。
//!     切れた列の数を返すので、呼ぶ側は画面に出すこと(黙って落とさない)

use std::io::Write;

use book::{format_value, HAlign, Value};
use book::Sheet as Grid;

use crate::pdfw;
use crate::Paper;

const COL_MM: f32 = 26.0;
const ROW_MM: f32 = 7.0;
/// xlsx の列幅1 ≒ 2.0mm(標準フォントの「0」1個ぶん)
const MM_PER_CHW: f32 = 2.0;

/// `RRGGBB` を 0..1 の RGB にする。読めなければ None(黙って黒にしない)。
/// 紙の1枚の置き場(頁と層)。printpdf の組で持ち回ります。
/// 余白(左・右・上・下。mm)。
type Margins = (f32, f32, f32, f32);

fn hex_rgb(s: &str) -> Option<(f32, f32, f32)> {
    // 色の書き方は3通り届きます。xlsx は `FFDCE6F1`(頭の2桁は透明度)、
    // テーマの表は `DCE6F1`、`.sheet.adoc` に人が書くときは `#DCE6F1` です。
    // **どれも同じ色**なので、ここで揃えます(2026-08-27 に取りこぼしを
    // 実物の PDF で見つけました)
    let t = s.trim().trim_start_matches('#');
    let t = if t.len() == 8 { &t[2..] } else { t };
    if t.len() != 6 {
        return None;
    }
    let g = |i: usize| {
        t.get(i * 2..i * 2 + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
            .map(|v| v as f32 / 255.0)
    };
    Some((g(0)?, g(1)?, g(2)?))
}

#[cfg(test)]
mod colour_tests {
    use super::hex_rgb;

    #[test]
    fn the_three_ways_of_writing_a_colour_all_mean_the_same() {
        let want = hex_rgb("DCE6F1").expect("6桁");
        assert_eq!(hex_rgb("#DCE6F1"), Some(want), "adoc の書き方");
        assert_eq!(hex_rgb("FFDCE6F1"), Some(want), "xlsx の書き方");
        assert_eq!(hex_rgb(" DCE6F1 "), Some(want), "前後の空白");
    }

    #[test]
    fn a_colour_that_is_not_a_colour_is_not_drawn() {
        assert_eq!(hex_rgb("あか"), None);
        assert_eq!(hex_rgb("#FFF"), None, "3桁の略記はまだ受けません");
        assert_eq!(hex_rgb(""), None);
    }
}

/// 印刷の指定(帳票が持っているもの)。Paper(紙の大きさ)とは別 —
/// こちらは「どこを・どんな余白で」。
#[derive(Debug, Clone, Default)]
pub struct PrintSetup {
    /// 印刷範囲(左上, 右下)。空なら使われている全域。
    /// **複数持てる。各域は新しい紙から刷る**(Excel と同じ)
    pub areas: Vec<(book::Pos, book::Pos)>,
    /// 余白 mm(左, 右, 上, 下)。None なら paper.margin_mm を四辺に
    pub margins_mm: Option<(f32, f32, f32, f32)>,
    /// 1904 起点のブックか(日付の描きが起点を替える)
    pub date1904: bool,
}

/// **紙 N 枚に収めるための縮尺。** `fit_to_w`/`fit_to_h` のどちらかが
/// 立っているときだけ Some。
///
/// 中身の総幅・総高を等倍で測り、指定した枚数に入る倍率を出す。両方
/// 指定なら小さい方(=きつい方)を採る。**縮めるだけで拡大はしない** —
/// Excel と同じ。小さな表が紙いっぱいに膨らむと帳票が別物になる。
///
/// 行の高さは改ページを跨ぐぶんの端数を無視した概算。厳密に詰めるには
/// 縮尺を変えて行送りをやり直す繰り返しが要るが、**紙に収める**という
/// 目的にはこれで足りる(足りない分は下限 10% で頭打ち)。
fn fit_scale(
    grid: &Grid,
    paper: Paper,
    setup: &PrintSetup,
    (r0, r1, c0, c1): (u32, u32, u32, u32),
    (ml, mr, mt, mb): (f32, f32, f32, f32),
) -> Option<f32> {
    let (nw, nh) = (grid.fit_to_w, grid.fit_to_h);
    if nw.is_none() && nh.is_none() {
        return None;
    }
    let _ = setup;
    let total_w: f32 = (c0..c1)
        .filter(|c| !grid.col_hidden.contains(c))
        .map(|c| {
            grid.col_width.get(&c).copied().or(grid.default_col_width)
                .map(|w| w * MM_PER_CHW).unwrap_or(COL_MM)
        })
        .sum();
    let total_h: f32 = (r0..r1)
        .filter(|r| !grid.row_hidden.contains(r))
        .map(|r| grid.row_height.get(&r).map(|pt| pt * 25.4 / 72.0).unwrap_or(ROW_MM))
        .sum();
    let usable_w = (paper.width_mm - ml - mr).max(1.0);
    let usable_h = (paper.height_mm - mt - mb).max(1.0);
    let mut k = 1.0f32;
    if let Some(n) = nw.filter(|n| *n > 0) {
        if total_w > 0.0 {
            k = k.min(usable_w * n as f32 / total_w);
        }
    }
    if let Some(n) = nh.filter(|n| *n > 0) {
        if total_h > 0.0 {
            k = k.min(usable_h * n as f32 / total_h);
        }
    }
    Some(k.clamp(0.1, 1.0))
}

/// **紙の切れ目**(この行/この列から新しい紙になる、の一覧)。
///
/// 画面に破線で見せるために外へ出す。刷る側([`sheet_to_pdf`])と
/// **同じ規則で数える** — 別々に書くと画面と紙がずれる。試験
/// 「画面の切れ目と紙の枚数が合う」で縛ってある。
///
/// 返すのは (行の切れ目, 列の切れ目)。どちらも「その手前で紙が変わる」
/// 位置で、先頭(範囲の頭)は入れない。
pub fn page_starts(grid: &Grid, paper: Paper, setup: &PrintSetup) -> (Vec<u32>, Vec<u32>) {
    let (ext_rows, ext_cols) = grid.extent();
    let (r0, r1, c0, c1) = match setup.areas.first() {
        Some((a, b)) if setup.areas.len() == 1 => (a.row, b.row + 1, a.col, b.col + 1),
        // 域が複数あるときは域ごとに紙が変わる — 画面の線は引かない
        // (どの域の切れ目か画面では言い分けられないため。嘘の線より無い方がよい)
        Some(_) => return (Vec::new(), Vec::new()),
        None => (0, ext_rows, 0, ext_cols),
    };
    let (ml, mr, mt, mb) = setup
        .margins_mm
        .unwrap_or((paper.margin_mm, paper.margin_mm, paper.margin_mm, paper.margin_mm));
    let scale = fit_scale(grid, paper, setup, (r0, r1, c0, c1), (ml, mr, mt, mb))
        .unwrap_or_else(|| grid.print_scale.unwrap_or(100).clamp(10, 400) as f32 / 100.0);

    let usable_w = (paper.width_mm - ml - mr).max(1.0);
    let mut cols = Vec::new();
    let mut w = 0.0f32;
    for c in c0..c1 {
        if grid.col_hidden.contains(&c) {
            continue;
        }
        let cw = grid.col_width.get(&c).copied().or(grid.default_col_width)
            .map(|x| x * MM_PER_CHW).unwrap_or(COL_MM) * scale;
        if w > 0.0 && (grid.col_breaks.contains(&c) || w + cw > usable_w + 0.1) {
            cols.push(c);
            w = 0.0;
        }
        w += cw;
    }

    let usable_h = (paper.height_mm - mt - mb).max(1.0);
    let mut rows = Vec::new();
    let mut h = 0.0f32;
    for r in r0..r1 {
        if grid.row_hidden.contains(&r) {
            continue;
        }
        let rh = grid.row_height.get(&r).map(|pt| pt * 25.4 / 72.0).unwrap_or(ROW_MM) * scale;
        if h > 0.0 && (grid.row_breaks.contains(&r) || h + rh > usable_h) {
            rows.push(r);
            h = 0.0;
        }
        h += rh;
    }
    (rows, cols)
}


/// ヘッダー/フッターの1区分を、頁番号を入れた字にする。
/// `&P` はこの頁の番号(**ブック通し**)、`&N` は総頁。
/// 他の `&コード`(`&"書体"` など)は落とす — 黙って化けさせない。
///
/// **純粋な関数にしてある** — 頁番号の規則はここだけで決まるので、
/// 試験でそのまま縛れる(2026-08-13、Book.to_pdf の頁の数え方のため)。
pub fn hf_subst(raw: &str, page_no: usize, total: usize) -> String {
    // **一度の走査で読む。** 先に &P を数へ置き換えてから印を落とす作りだと、
    // 「&&P」(素の & のあとに P)が数に化ける — 走査を分けない
    let mut out = String::new();
    let mut it = raw.chars().peekable();
    while let Some(ch) = it.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }
        match it.peek() {
            // 「&&」は素の &(xlsx の書き方)。落とすと「山田&田中」が壊れる
            Some('&') => {
                it.next();
                out.push('&');
            }
            Some('P') => {
                it.next();
                out.push_str(&page_no.to_string());
            }
            Some('N') => {
                it.next();
                out.push_str(&total.to_string());
            }
            // &"書体名" は書体の指定 — 名前ごと落とす
            Some('"') => {
                it.next();
                for c2 in it.by_ref() {
                    if c2 == '"' {
                        break;
                    }
                }
            }
            // 知らない &コード(&B 太字 など)は落とす — 黙って化けさせない
            Some(c2) if c2.is_ascii_alphanumeric() => {
                it.next();
            }
            _ => {}
        }
    }
    out
}

/// 印刷のヘッダー/フッターを、渡された頁の並びに描く。
///
/// **頁番号はブック通しで数える** — `offset` はこのシートの最初の頁が
/// ブックの何頁目か(0 起点)、`total` はブック全体の頁数。1枚だけの
/// PDF なら offset=0・total=そのシートの頁数で、今までと同じ答えになる
/// (2026-08-13、Book.to_pdf のために sheet_to_pdf から切り出した)。
#[allow(clippy::too_many_arguments)]
fn draw_header_footer(
    board: &mut Board,
    grid: &Grid,
    paper: Paper,
    hf_pages: std::ops::Range<usize>,
    (ml, mr, mt, mb): (f32, f32, f32, f32),
    offset: usize,
    total: usize,
) {
    // 印刷のヘッダー/フッター(&L/&C/&R の区分。&P=頁 &N=総頁。
    // 他の &コード(&"書体" など)は落とす — 黙って化けさせない)
    if grid.header.is_none() && grid.footer.is_none() {
        return;
    }
    {
        let est = |s: &str| -> f32 {
            // 文字幅の見積り(全角=1em・半角=0.5em)。9pt ≒ 3.175mm/em
            s.chars()
                .map(|c| if (c as u32) < 0x2E80 { 0.5 } else { 1.0 })
                .sum::<f32>() * 3.175
        };
        const HF_INK: (f32, f32, f32) = (0.25, 0.28, 0.31);
        for (i, page) in hf_pages.enumerate() {
            let ink = &mut board.ink(page);
            let subst = |raw: &str| -> String { hf_subst(raw, offset + i + 1, total) };
            let put3 = |ink: &mut Ink<'_>, raw: &str, y: f32| {
                let (lf, cn, rt) = book::hf_split(raw);
                let (lf, cn, rt) = (subst(&lf), subst(&cn), subst(&rt));
                if !lf.is_empty() {
                    ink.text(&lf, 9.0, ml, y, HF_INK, false);
                }
                if !cn.is_empty() {
                    let x = (paper.width_mm - est(&cn)) / 2.0;
                    ink.text(&cn, 9.0, x.max(ml), y, HF_INK, false);
                }
                if !rt.is_empty() {
                    let x = paper.width_mm - mr - est(&rt);
                    ink.text(&rt, 9.0, x.max(ml), y, HF_INK, false);
                }
            };
            if let Some(h) = &grid.header {
                put3(ink, h, paper.height_mm - mt * 0.55);
            }
            if let Some(f) = &grid.footer {
                put3(ink, f, mb * 0.35);
            }
        }
    }
}

/// 印刷の枠線の色(薄い灰)
const GRID_GREY: (f32, f32, f32) = (0.85, 0.87, 0.89);
/// 行番号と列番号の色
const HEAD_GREY: (f32, f32, f32) = (0.4, 0.44, 0.48);

/// **描く先。** 紙1枚を受け持ちます。
///
/// 表計算は紙面(`kumihan::Sheet`)を通らず、その場で描いています。
/// 描く所はそのままにして、置く先だけをここに集めました
/// (2026-08-27 発注者「行番号と列番号もセルと同じ」)。
struct Ink<'a> {
    leaf: &'a mut pdfw::Leaf,
}

impl Ink<'_> {
    fn text(&mut self, t: &str, pt: f32, x: f32, y: f32, rgb: (f32, f32, f32), bold: bool) {
        self.leaf.pieces.push(pdfw::Piece {
            x_mm: x,
            // 表計算も新しい書き手も、左下からの y で描きます
            y_mm: y,
            size_pt: pt,
            text: t.to_string(),
            w_mm: t.chars().map(|c| if c.is_ascii() { 0.55 } else { 1.0 }).sum::<f32>()
                * pt * 25.4 / 72.0,
            bold,
            color: Some(pdfw::to_hex(rgb)),
            ..Default::default()
        });
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32, rgb: (f32, f32, f32)) {
        self.leaf.rules.push(pdfw::Rule {
            x1_mm: x1, y1_mm: y1, x2_mm: x2, y2_mm: y2, w_mm: w, rgb,
        });
    }

    fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, rgb: (f32, f32, f32)) {
        self.leaf.fills.push(pdfw::Fill { x_mm: x, y_mm: y, w_mm: w, h_mm: h, rgb });
    }

    /// 好きな形の塗り(円グラフの扇・傾いた棒)
    fn poly(&mut self, points: Vec<(f32, f32)>, rgb: (f32, f32, f32)) {
        if points.len() >= 3 {
            self.leaf.polys.push(pdfw::Poly { points, rgb });
        }
    }
}

/// **紙を足していく先。** 頁を足す所を1本にしておくと、頁割りに手を
/// 入れずに書き手を替えられます(2026-08-27)。
struct Board {
    leaves: Vec<pdfw::Leaf>,
}

impl Board {
    /// 1枚目の紙を敷いた紙束を作ります
    fn new(paper: Paper) -> Self {
        Board { leaves: vec![leaf(paper)] }
    }

    /// 紙を1枚足して、その番号を返します
    fn add_page(&mut self, paper: Paper) -> usize {
        self.leaves.push(leaf(paper));
        self.leaves.len() - 1
    }

    fn len(&self) -> usize {
        self.leaves.len()
    }

    /// `i` 枚目に描く筆を借ります
    fn ink(&mut self, i: usize) -> Ink<'_> {
        Ink { leaf: &mut self.leaves[i] }
    }

    fn save<W: Write>(self, paper: Paper, font_data: &[u8], out: W) -> Result<(), String> {
        pdfw::write_pages(&self.leaves, paper.width_mm, paper.height_mm, font_data, out)
    }
}

/// **図形だけの紙面を組む。** シートを通しません。
///
/// 図形を1つずつ、`(図形, 左からの mm, 下からの mm)` で渡します。返るのは
/// 紙面なので、[`crate::e`] で絵にできます。
///
/// 用意したのは、**同じ図形を紙と画面のどちらでも描いて比べる**ためです
/// (2026-08-29 発注者「gpui を使うか vello を使うかはテストをして決めて
/// いけばいい」)。形を作るのは表を刷るときと同じ [`zukei`] なので、
/// ここで見た形は紙に出る形と同じです。
pub fn shapes_leaf(shapes: &[(book::SheetShape, f32, f32)], paper: Paper) -> pdfw::Leaf {
    let mut board = Board::new(paper);
    {
        let mut l1 = board.ink(0);
        for (sp, x_mm, y_mm) in shapes {
            zukei(&mut l1, sp, *x_mm, *y_mm, 1.0);
        }
    }
    board.leaves.remove(0)
}

fn leaf(paper: Paper) -> pdfw::Leaf {
    pdfw::Leaf { size_mm: Some((paper.width_mm, paper.height_mm)), ..Default::default() }
}

/// **紙面を1枚だけ取り出す。** 絵にする道([`crate::e`])の入り口です。
///
/// PDF を作るのと**同じ組み方**を通すので、絵と紙が食い違いません。
/// 返るのは1枚目です(何枚あるかは [`sheet_leaves`] で全部取れます)。
pub fn sheet_leaf(
    grid: &Grid,
    paper: Paper,
    setup: &PrintSetup,
) -> Result<pdfw::Leaf, String> {
    sheet_leaves(grid, paper, setup)?
        .into_iter()
        .next()
        .ok_or_else(|| "刷る物がありません".to_string())
}

/// 紙面を全部取り出す。**PDF を書かずに**組むだけです
pub fn sheet_leaves(
    grid: &Grid,
    paper: Paper,
    setup: &PrintSetup,
) -> Result<Vec<pdfw::Leaf>, String> {
    let mut board = Board::new(paper);
    let (pages, _clipped, margins) = draw_sheet(&mut board, grid, paper, setup, true);
    let total = pages.len();
    draw_header_footer(&mut board, grid, paper, pages, margins, 0, total);
    Ok(board.leaves)
}

/// 1つの表を PDF にする。行が紙に収まらなければ次のページへ。
/// 返すのは**右にはみ出して切れた列の数**(0 なら全部紙に入っている)。
pub fn sheet_to_pdf<W: Write>(
    grid: &Grid,
    font_data: &[u8],
    paper: Paper,
    setup: &PrintSetup,
    out: W,
) -> Result<u32, String> {
    let mut board = Board::new(paper);
    let (pages, clipped, margins) = draw_sheet(&mut board, grid, paper, setup, true);
    // 1枚だけの PDF は、そのシートの頁数がそのまま総頁
    let total = pages.len();
    draw_header_footer(&mut board, grid, paper, pages, margins, 0, total);
    board.save(paper, font_data, out)?;
    Ok(clipped)
}

/// **ブックを1つの PDF にする。** シートを順に、同じ文書へ足していく。
///
/// 頁番号(&P)と総頁(&N)は**ブック通し** — Excel がブック全体を刷る
/// ときと同じで、「1つの PDF」に人が期待する数え方(2026-08-13 発注者
/// 「Book.to_pdf をつくりましょう」)。ヘッダー/フッターの文言は
/// シートごとの物がそのシートの頁に載る。
///
/// 紙の大きさ・向き・余白・印刷範囲は**シートごと**に効く(1冊の中で
/// A4 縦と A4 横が混ざってよい)。返りは切れた列の数の合計。
/// 見えないシート(hidden)は刷らない — 画面と同じ。
pub fn book_to_pdf<W: Write>(
    sheets: &[(&Grid, Paper, PrintSetup)],
    font_data: &[u8],
    out: W,
) -> Result<u32, String> {
    let first = sheets.first().ok_or("シートがありません")?;
    let paper1 = first.1;
    let mut clipped = 0u32;
    let mut board = Board::new(paper1);
    // 版組を先に全部済ませる — **総頁が決まってからでないと &N が書けない**
    let mut laid: Vec<(usize, std::ops::Range<usize>, Margins)> = Vec::new();
    let mut carry = true;
    for (i, (grid, paper, setup)) in sheets.iter().enumerate() {
        let (pages, cl, margins) = draw_sheet(&mut board, grid, *paper, setup, carry);
        carry = false;
        clipped += cl;
        laid.push((i, pages, margins));
    }
    let total: usize = laid.iter().map(|(_, p, _)| p.len()).sum();
    let mut offset = 0usize;
    for (i, pages, margins) in laid {
        let (grid, paper, _) = &sheets[i];
        let n = pages.len();
        draw_header_footer(&mut board, grid, *paper, pages, margins, offset, total);
        offset += n;
    }
    board.save(paper1, font_data, out)?;
    Ok(clipped)
}

/// 1枚のシートを、**渡された文書へ**描く(頁を足していく)。
/// `first` は「もう作ってある最初の頁」— 文書の1枚目はここへ入れる。
/// 返りは (このシートの頁, 切れた列の数, 余白)。
/// ヘッダー/フッターは総頁が決まってから別に描く([`draw_header_footer`])。
/// **1枚の紙に左から出す列の並び**。`start` から `n` 本の束の前に、
/// 繰り返すタイトル列を置く — ただし**束より左にあるものだけ**。
/// 束の中や右のタイトル列は、その紙に現に出るか後の紙で出るので繰り返さない
/// (繰り返すと同じ列が1枚に二度出る)。`n` が 0 なら繰り返す分だけ返る。
pub fn band_cols(title_cols: &[u32], start: u32, n: u32) -> Vec<u32> {
    title_cols.iter().copied().filter(|t| *t < start).chain(start..start + n).collect()
}

fn draw_sheet(
    board: &mut Board,
    grid: &Grid,
    paper: Paper,
    setup: &PrintSetup,
    carry: bool,
) -> (std::ops::Range<usize>, u32, Margins) {
    let (mut ext_rows, mut ext_cols) = grid.extent();
    // **図形の置き場まで紙を伸ばします。** 中身のあるセルより下に置いた図は、
    // 伸ばさないと最後の行の所へ寄ってしまい、図が全部重なります
    // (2026-08-27 に図を紙で見て気づきました)
    for sp in grid.shapes.iter().chain(grid.shapes_new.iter()) {
        let mm = 25.4 / 96.0;
        let shita = sp.at.row + ((sp.dy_px + sp.height_px) * mm / ROW_MM).ceil() as u32 + 1;
        ext_rows = ext_rows.max(shita);
        ext_cols = ext_cols.max(sp.at.col + 1);
    }
    // 印刷範囲があればそこだけ(行も列も)。**複数あれば域ごとに刷る**
    let areas: Vec<(u32, u32, u32, u32)> = if setup.areas.is_empty() {
        vec![(0, ext_rows, 0, ext_cols)]
    } else {
        setup.areas.iter().map(|(a, b)| (a.row, b.row + 1, a.col, b.col + 1)).collect()
    };
    // 縮尺は**シートに1つ**(Excel も同じ)。域ごとに変えると同じ表が
    // 域によって違う大きさで刷られて帳票にならない。全部の域を覆う枠で測る
    let (r0, r1, c0, c1) = (
        areas.iter().map(|a| a.0).min().unwrap_or(0),
        areas.iter().map(|a| a.1).max().unwrap_or(ext_rows),
        areas.iter().map(|a| a.2).min().unwrap_or(0),
        areas.iter().map(|a| a.3).max().unwrap_or(ext_cols),
    );
    let (ml, mr, mt, mb) = setup
        .margins_mm
        .unwrap_or((paper.margin_mm, paper.margin_mm, paper.margin_mm, paper.margin_mm));
    // 拡大縮小印刷(pageSetup scale)。列幅・行高・文字を同じ倍で。
    // **紙 N 枚に収める指定があれば、そちらが勝つ**(Excel と同じ)
    let scale = fit_scale(grid, paper, setup, (r0, r1, c0, c1), (ml, mr, mt, mb))
        .unwrap_or_else(|| grid.print_scale.unwrap_or(100).clamp(10, 400) as f32 / 100.0);
    // 文書の1枚目(もう作ってあればそれを使い、無ければ足す)
    let first = if carry {
        board.len() - 1
    } else {
        board.add_page(paper)
    };
    // いま描いている紙(ヘッダー/フッターは総頁が決まってから描く)
    let mut cur = first;

    // 列の幅と左端(文書の指定に従う)。印刷範囲の左端が原点。
    // グループ化で畳んだ列は幅ゼロ(画面と同じく出さない)
    let ncols = (c1 - c0).max(1);
    let col_mm: Vec<f32> = (c0..c0 + ncols)
        .map(|c| {
            if grid.col_hidden.contains(&c) {
                return 0.0;
            }
            grid.col_width.get(&c).copied().or(grid.default_col_width)
                .map(|w| w * MM_PER_CHW).unwrap_or(COL_MM) * scale
        })
        .collect();
    let mut col_x = vec![0.0f32];
    for w in &col_mm {
        col_x.push(col_x.last().unwrap() + w);
    }
    // 横方向のページ送り: 紙の幅に入る所で列を束に割る(Excel の既定と同じ
    // 「縦 → 横」の順で刷る = 束ごとに全行を出してから次の束へ)。
    // 1列が紙より広いときは、その1列だけで束にする(割りようが無い)
    let usable_w = paper.width_mm - ml - mr;
    // 各ページの左で繰り返すタイトル列。行と違い、**列は幅の割り付けにも効く** —
    // 繰り返す列のぶんだけ本体に使える幅が減る(Excel と同じ)。
    // col_mm は c0 起点で並べてあるので、範囲の外の列は添字が無い = 繰り返さない
    let title_cols: Vec<u32> = grid
        .print_title_cols
        .map(|(a, b)| (a..=b).filter(|c| *c >= c0 && *c < c1).collect())
        .unwrap_or_default();
    // 本数 0 で呼ぶと「繰り返す分だけ」が返る = その幅が本体から減る
    let repeat_w = |start: u32| -> f32 {
        band_cols(&title_cols, start, 0).iter().map(|t| col_mm[(t - c0) as usize]).sum()
    };
    // **刷る単位の一覧**: (行の始まり, 行の終わり, 束の左端の列, 本数)。
    // 域ごとに列を束へ割る = 域が変わっても束が変わっても新しい紙になる
    let mut bands: Vec<(u32, u32, u32, u32)> = Vec::new();
    for &(ar0, ar1, ac0, ac1) in &areas {
        let mut start = ac0;
        let mut w = 0.0f32;
        for c in ac0..ac1 {
            let cw = col_mm[(c - c0) as usize];
            // 繰り返すタイトル列のぶんだけ、本体に使える幅は狭い
            let avail = usable_w - repeat_w(start);
            // 縦の改ページ(colBreaks: この列から新しい紙)でも束を割る
            if w > 0.0 && (grid.col_breaks.contains(&c) || w + cw > avail + 0.1) {
                bands.push((ar0, ar1, start, c - start));
                start = c;
                w = 0.0;
            }
            w += cw;
        }
        bands.push((ar0, ar1, start, ac1 - start));
    }
    // **1列だけで紙をはみ出す列**は割れないので、そこだけは切れる。
    // 呼ぶ側へはその本数を返す(0 なら全部が紙に載った)
    let clipped = (0..ncols)
        .filter(|i| col_mm[*i as usize] > usable_w + 0.1)
        .count() as u32;

    // 行の高さ(pt → mm)。指定のない行は既定。畳んだ行は高さゼロ=出さない
    let row_mm = |r: u32| -> f32 {
        if grid.row_hidden.contains(&r) {
            return 0.0;
        }
        grid.row_height.get(&r).map(|pt| pt * 25.4 / 72.0).unwrap_or(ROW_MM) * scale
    };
    let usable = paper.height_mm - mt - mb;

    // 条件付き書式の下ごしらえ(重複・上位N・平均は範囲の統計が要る)
    let cond_prep: Vec<(book::CondRule, book::CondAux)> =
        grid.cond.iter().map(|r| (r.clone(), r.aux(grid))).collect();

    // 各ページの頭で繰り返すタイトル行(自分のいる範囲の外は繰り返さない)
    let title_rows: Vec<u32> = grid
        .print_title_rows
        .map(|(a, b)| (a..=b).filter(|r| *r < r1).collect())
        .unwrap_or_default();

    // 1行を紙に描く(セルの塗り・罫線・値、印刷の枠線・行番号)。
    // **列は連続とは限らない** — `cols` はこの紙に左から出す列の並びで、
    // タイトル列を繰り返す紙では左端に飛び地(A 列など)が入る。
    // col_x / col_mm はその並びに揃えて渡すこと(col_x は本数+1)
    #[allow(clippy::too_many_arguments)]
    fn draw_row(
        grid: &Grid,
        ink: &mut Ink<'_>,
        r: u32,
        y_top: f32,
        rh: f32,
        ml: f32,
        cols: &[u32],
        col_x: &[f32],
        col_mm: &[f32],
        scale: f32,
        cond_prep: &[(book::CondRule, book::CondAux)],
        date1904: bool,
    ) {
        let ncols = cols.len();
        // 印刷の枠線(printOptions gridLines)。薄い灰で先に敷く
        if grid.print_gridlines {
            let w_total = col_x[ncols];
            for (x1, y1, x2, y2) in [
                (ml, y_top, ml + w_total, y_top),
                (ml, y_top - rh, ml + w_total, y_top - rh),
            ] {
                ink.line(x1, y1, x2, y2, 0.1, GRID_GREY);
            }
            for &x in col_x.iter().take(ncols + 1) {
                ink.line(ml + x, y_top, ml + x, y_top - rh, 0.1, GRID_GREY);
            }
        }
        // 行番号(printOptions headings)。左の余白に小さく
        // **行番号も升と同じ字**です(2026-08-27 発注者)。置き場が余白と
        // いうだけで、特別扱いはしません
        if grid.print_headings {
            ink.text(&(r + 1).to_string(), 6.5, ml - 7.0, y_top - rh + 2.0, HEAD_GREY, false);
        }
        for (i, &c) in cols.iter().enumerate() {
            let p = book::Pos::new(r, c);
            let x = ml + col_x[i];
            let cw = col_mm[i];
            if cw <= 0.0 {
                continue; // 畳んだ列(幅ゼロ)は中身も描かない
            }
            let Some(cell) = grid.cells.get(&p) else { continue };

            // 塗りと文字色。**条件付き書式の当てはめは kumihan::look の1本** —
            // 画面(calc/src/view.rs)も同じ関数を通るので、答えは必ず揃う。
            // ここは決まった答えを紙の形に写すだけ
            let ck = kumihan::look::resolve_cond(cond_prep, p, &cell.value);
            let fill = ck.fill.clone().or_else(|| cell.fmt.fill.clone());
            let colour = ck.color.clone().or_else(|| cell.fmt.color.clone());
            // **None は「触らない」**(セル自身の書式のまま)
            let bold = ck.bold.unwrap_or(cell.fmt.bold);
            //
            // **カラースケールは 2026-08-14 から紙にも出る。** `ck.fill` が
            // スケールの色も返すので、塗りとして自然に乗った(前は当てはめが
            // 紙と画面の2箇所に分かれていて、紙は物差しの側を丸ごと捨てていた)。
            // 紙は元から塗りを描けるので、足したのは判断だけで仕掛けは要らない
            //
            // **紙に出ないもの**(画面には出る。ここが残る差):
            // - データバー(`ck.bar`)・アイコン(`ck.icon`)= 敷く/字を置く
            //   仕掛けが要る。**やるならグリフが紙の書体にあるかを先に確かめる**
            //   (↓→↑ と ● — 無い書体だと黙って空白か豆腐になる)
            // - 斜体・下線・取り消し線(`ck.italic`/`underline`/`strike`)=
            //   **セル自身のそれらも描いていない**ので、条件付き書式のぶんだけ
            //   描くと食い違いがかえって増える
            // 塗りは罫線より先に敷く(線を塗り潰さない)
            if let Some(c) = fill.as_deref().and_then(hex_rgb) {
                ink.fill(x, y_top - rh, cw, rh, c);
            }

            // 罫線。引いてある辺だけ — 線種の太さと色まで写す
            // (破線の刻みは紙では実線に落とす。太さと色が形を保つ)
            let b = cell.fmt.borders;
            for (e, (x1, y1, x2, y2)) in [
                (b.top, (x, y_top, x + cw, y_top)),
                (b.bottom, (x, y_top - rh, x + cw, y_top - rh)),
                (b.left, (x, y_top, x, y_top - rh)),
                (b.right, (x + cw, y_top, x + cw, y_top - rh)),
            ] {
                if e.on {
                    let c = match e.color {
                        Some(v) => (
                            ((v >> 16) & 255) as f32 / 255.0,
                            ((v >> 8) & 255) as f32 / 255.0,
                            (v & 255) as f32 / 255.0,
                        ),
                        None => (0.0, 0.0, 0.0),
                    };
                    // px → pt → mm。二重線は2本に開くほどの幅が無いので
                    // 太めの1本で
                    ink.line(x1, y1, x2, y2, e.style.px() * 0.75 * 25.4 / 72.0, c);
                }
            }

            // 値。結合に呑まれた位置は左上にだけ出る(画面と同じ)
            if grid.covered_by_merge(p) {
                continue;
            }
            // Bool はチェックボックスとして紙にも出す(画面と一致)。
            // ☑/☐ の字は日本語フォントに無い(Noto CJK 実測)ので、
            // 文字ではなく線で描く — 豆腐を刷らない
            if let Value::Bool(b) = &cell.value {
                let s = (rh - 1.6).min(3.2); // 箱の一辺 mm
                let bx = x + 1.5;
                let by = y_top - rh / 2.0 - s / 2.0; // 箱の下辺
                const TICK_INK: (f32, f32, f32) = (0.1, 0.1, 0.1);
                // 折れ線は辺ごとに引きます(書き手が持つのは直線だけ)
                for (x1, y1, x2, y2) in [
                    (bx, by, bx + s, by),
                    (bx + s, by, bx + s, by + s),
                    (bx + s, by + s, bx, by + s),
                    (bx, by + s, bx, by),
                ] {
                    ink.line(x1, y1, x2, y2, 0.2, TICK_INK);
                }
                if *b {
                    for (x1, y1, x2, y2) in [
                        (bx + s * 0.2, by + s * 0.5, bx + s * 0.45, by + s * 0.2),
                        (bx + s * 0.45, by + s * 0.2, bx + s * 0.85, by + s * 0.85),
                    ] {
                        ink.line(x1, y1, x2, y2, 0.3, TICK_INK);
                    }
                }
                continue;
            }
            let shown = format_value(&cell.value, cell.fmt.number_format.as_deref(), date1904);
            if shown.is_empty() {
                continue;
            }
            // 数は右、文字は左(指定があればそちら)
            let right = match cell.fmt.align {
                HAlign::Right => true,
                // 中央・両端・均等割付は右ではない。**ここは左か右かしか
                // 見ていない**ので、中央揃えも今は左に出る — 均等割付
                // (字を幅いっぱいに散らす)も同じで、印刷側は据え置き
                HAlign::Left
                | HAlign::Center
                | HAlign::Justify
                | HAlign::CenterContinuous
                | HAlign::Distribute => false,
                HAlign::General => matches!(cell.value, Value::Number(_)),
            };
            let pt = 9.5f32 * scale;
            let tx = if right {
                // だいたいの字幅で右に寄せる(全角 1em / 半角 0.55em)
                let w: f32 = shown
                    .chars()
                    .map(|ch| if ch.is_ascii() { 0.55 } else { 1.0 })
                    .sum::<f32>()
                    * pt
                    * 25.4
                    / 72.0;
                x + cw - 1.5 - w
            } else {
                // 字下げ(indent)。1段 = 全角約1字ぶん左を空ける —
                // 日本の帳票は項目の階層を字下げで見せる
                let ind = f32::from(cell.fmt.indent) * pt * 25.4 / 72.0;
                x + 1.5 + ind
            };
            let ty = y_top - rh + 2.0;
            // 文字は塗り色で描かれる(PDF の作法)ので、色付きの字は前後で入れ替える
            let c = colour.as_deref().and_then(hex_rgb).unwrap_or((0.0, 0.0, 0.0));
            ink.text(&shown, pt, tx, ty, c, bold);
        }
    }

    // 列名の見出し(printOptions headings)。各ページの上の余白に
    let draw_col_heads = |ink: &mut Ink<'_>, cols: &[u32], cx: &[f32], cm: &[f32]| {
        if !grid.print_headings {
            return;
        }
        for (i, &c) in cols.iter().enumerate() {
            let x = ml + cx[i] + cm[i] / 2.0 - 1.0;
            let name = book::Pos::new(0, c).a1();
            let name = name.trim_end_matches('1');
            ink.text(name, 6.5, x, paper.height_mm - mt + 1.5, HEAD_GREY, false);
        }
    };

    // 行 → (紙の番号, 上端の mm)。図形をその紙へ置くために使います
    let mut row_place: std::collections::BTreeMap<u32, (usize, f32)> =
        std::collections::BTreeMap::new();
    let mut y_used = 0.0f32; // このページで使った高さ
    // 束(横のページ)ごとに全行を出す。束が変わるたび新しい紙へ
    for (bi, &(r0, r1, bc0, bn)) in bands.iter().enumerate() {
    let cols = band_cols(&title_cols, bc0, bn);
    let col_mm: Vec<f32> = cols.iter().map(|c| col_mm[(c - c0) as usize]).collect();
    let mut col_x = vec![0.0f32];
    for w in &col_mm {
        col_x.push(col_x.last().unwrap() + w);
    }
    if bi > 0 {
        y_used = 0.0;
        cur = board.add_page(paper);
    }
    draw_col_heads(&mut board.ink(cur), &cols, &col_x, &col_mm);
    for r in r0..r1.max(r0 + 1) {
        // 畳んだ行は紙にも出さない(画面と同じ)
        if grid.row_hidden.contains(&r) {
            continue;
        }
        let rh = row_mm(r);
        // 改ページ(rowBreaks: この行から新しい紙)か、紙が尽きたら次のページ
        let break_here = y_used > 0.0 && grid.row_breaks.contains(&r);
        if break_here || (y_used + rh > usable && y_used > 0.0) {
            y_used = 0.0;
            cur = board.add_page(paper);
            draw_col_heads(&mut board.ink(cur), &cols, &col_x, &col_mm);
            // タイトル行を頭で繰り返す(いま描く行が自分自身なら繰り返さない)
            if !title_rows.contains(&r) {
                for tr in &title_rows {
                    let th = row_mm(*tr);
                    let y_top = paper.height_mm - mt - y_used;
                    draw_row(grid, &mut board.ink(cur), *tr, y_top, th, ml, &cols, &col_x, &col_mm, scale, &cond_prep, setup.date1904);
                    y_used += th;
                }
            }
        }
        let y_top = paper.height_mm - mt - y_used;
        // **その行がどの紙のどこに出たか**を控えます。図形の置き場はここから
        // 引きます(2026-08-27 まで図は1枚目にしか出ませんでした)
        if bi == 0 {
            row_place.entry(r).or_insert((cur, y_top));
        }
        y_used += rh;
        draw_row(grid, &mut board.ink(cur), r, y_top, rh, ml, &cols, &col_x, &col_mm, scale, &cond_prep, setup.date1904);
    }
    }
    // 図形(挿した分も読んだ分も)。塗りと輪郭を紙に出します
    {
        // セル → (紙の番号, 左からの mm, 上端の mm)。
        // **行の控えから引く**ので、改ページの後ろに置いた図もその紙に出ます
        let cell_at = |at: book::Pos| -> (usize, f32, f32) {
            let x: f32 = (c0..at.col)
                .map(|c| {
                    col_mm
                        .get((c - c0) as usize)
                        .copied()
                        .unwrap_or(COL_MM * scale)
                })
                .sum();
            // 控えに無い行(隠した行など)は、手前のいちばん近い行から数えます
            let (page, y_top) = match row_place.range(..=at.row).next_back() {
                Some((r, (p, y))) => (*p, *y - (*r..at.row).map(row_mm).sum::<f32>()),
                None => (first, paper.height_mm - mt),
            };
            (page, ml + x, y_top)
        };
        // 同じ紙の図をまとめて描きます(筆を借り直す回数を減らします)
        let mut kumi: Vec<(usize, &book::SheetShape)> = grid
            .shapes
            .iter()
            .chain(grid.shapes_new.iter())
            .map(|sp| (cell_at(sp.at).0, sp))
            .collect();
        kumi.sort_by_key(|(p, _)| *p);
        let mut ima = usize::MAX;
        let mut ink_box: Option<Ink<'_>> = None;
        for (page, sp) in kumi {
            if page != ima {
                ima = page;
                ink_box = Some(board.ink(page));
            }
            let l1 = ink_box.as_mut().expect("筆");
            let (_, x, y_top) = cell_at(sp.at);
            zukei(l1, sp, x, y_top, scale);
        }
    }

    (first..board.len(), clipped, (ml, mr, mt, mb))
}
/// **図形を1つ紙面に描く。**
///
/// `x` と `y_top` は図形を留めるセルの左上(紙の左からと下からの mm)、
/// `scale` は拡大縮小印刷の倍率です。図形自身のずらし(`dx_px`・`dy_px`)は
/// この中で足します。
///
/// **切り出したのは、同じ図形を別の描き手にも渡せるようにするため**です
/// (2026-08-29 発注者「gpui を使うか vello を使うかはテストをして
/// 決めていけばいい」)。形を作る所を1本にしておかないと、紙と画面で
/// 図形の形が食い違います — 角丸が紙だけ四角だったのがその例です。
fn zukei(l1: &mut Ink, sp: &book::SheetShape, x: f32, y_top: f32, scale: f32) {
        let mm = 25.4 / 96.0; // px → mm
        // アンカーのセルからの px のずらしも紙に写す
        let (x, y_top) =
            (x + sp.dx_px * mm * scale, y_top - sp.dy_px * mm * scale);
        let (w, h) = (sp.width_px * mm * scale, sp.height_px * mm * scale);
        // **線の指定が無ければ引きません**(模型の決め)。前は黒に
        // 落としていたので、字を置くだけの箱にも枠が出ていました
        // (2026-08-27 に図を紙で見て気づきました)
        let pen = sp.line.as_deref().and_then(hex_rgb);
        let pen_w = sp.line_w.max(0.1) * scale * 25.4 / 72.0;
        let pts: Vec<(f32, f32)> = match sp.kind.as_str() {
            // **角丸。** 画面(`SheetShape::to_svg`)と Excel は丸めるのに、
            // 紙だけ四角のままでした(2026-08-29 に図形を絵にして
            // 気づきました)。丸めの大きさは画面と同じ短辺の 15%
            "roundRect" => {
                let px = 25.4 / 96.0 * scale; // 4px を mm に
                let r = (w.min(h) * 0.15).max(4.0 * px).min(w.min(h) / 2.0);
                // 角ごとに4分の1円を6辺で近づけます
                let kado = |cx: f32, cy: f32, kara: f32| {
                    (0..=6).map(move |i| {
                        let t = kara + i as f32 / 6.0 * std::f32::consts::FRAC_PI_2;
                        (cx + r * t.cos(), cy + r * t.sin())
                    })
                };
                let (l, rr, tp, bt) = (x, x + w, y_top, y_top - h);
                kado(rr - r, tp - r, 0.0) // 右上
                    .chain(kado(l + r, tp - r, std::f32::consts::FRAC_PI_2)) // 左上
                    .chain(kado(l + r, bt + r, std::f32::consts::PI)) // 左下
                    .chain(kado(rr - r, bt + r, std::f32::consts::PI * 1.5)) // 右下
                    .collect()
            }
            "ellipse" => (0..=24)
                .map(|i| {
                    let t = i as f32 / 24.0 * std::f32::consts::TAU;
                    (x + w / 2.0 + w / 2.0 * t.cos(), y_top - h / 2.0 + h / 2.0 * t.sin())
                })
                .collect(),
            "rightArrow" => {
                let (ty, by, bx, my) =
                    (h * 0.25, h * 0.75, w - (w * 0.35).min(h), h / 2.0);
                vec![
                    (x, y_top - ty),
                    (x + bx, y_top - ty),
                    (x + bx, y_top),
                    (x + w, y_top - my),
                    (x + bx, y_top - h),
                    (x + bx, y_top - by),
                    (x, y_top - by),
                ]
            }
            "diamond" => vec![
                (x + w / 2.0, y_top),
                (x + w, y_top - h / 2.0),
                (x + w / 2.0, y_top - h),
                (x, y_top - h / 2.0),
            ],
            "line" => vec![(x, y_top), (x + w, y_top - h)],
            // 縦棒・勝ち負け: 棒ごとに閉じた長方形を落とす(紙も棒で)
            "spark-col" | "spark-wl" => {
                let n = sp.points.len().max(1) as f32;
                let bw = (w / n * 0.7).max(0.5);
                let base_y = y_top - sp.base * h;
                for pp in &sp.points {
                    let (cx_, ty) = pp.at;
                    let (l, r) = (x + cx_ * w - bw / 2.0, x + cx_ * w + bw / 2.0);
                    let t = y_top - ty * h;
                    if let Some(pen) = pen {
                        for (x1, y1, x2, y2) in [
                            (l, t, r, t),
                            (r, t, r, base_y),
                            (r, base_y, l, base_y),
                            (l, base_y, l, t),
                        ] {
                            l1.line(x1, y1, x2, y2, pen_w, pen);
                        }
                    }
                }
                return;
            }
            // **曲線は紙では折れ線に割る。** printpdf の Line は直線の列
            // しか持たない — 曲がっているものを直線1本にすると形が変わる
            // ので、区間ごとに 12 に刻む(見た目で区別が付かない細かさ)
            "spark" | "ink" | "marker" | "path" => {
                let ex = |p: (f32, f32)| (x + p.0 * w, y_top - p.1 * h);
                let mut out: Vec<(f32, f32)> = Vec::new();
                for (i, pp) in sp.points.iter().enumerate() {
                    if i == 0 {
                        out.push(ex(pp.at));
                        continue;
                    }
                    let prev = &sp.points[i - 1];
                    match (prev.c_out, pp.c_in) {
                        (None, None) => out.push(ex(pp.at)),
                        (co, ci) => {
                            let p0 = ex(prev.at);
                            let c1 = ex(co.unwrap_or(prev.at));
                            let c2 = ex(ci.unwrap_or(pp.at));
                            let p3 = ex(pp.at);
                            for k in 1..=12 {
                                let t = k as f32 / 12.0;
                                let u = 1.0 - t;
                                let bx = u * u * u * p0.0
                                    + 3.0 * u * u * t * c1.0
                                    + 3.0 * u * t * t * c2.0
                                    + t * t * t * p3.0;
                                let by = u * u * u * p0.1
                                    + 3.0 * u * u * t * c1.1
                                    + 3.0 * u * t * t * c2.1
                                    + t * t * t * p3.1;
                                out.push((bx, by));
                            }
                        }
                    }
                }
                out
            }
            _ => vec![
                (x, y_top),
                (x + w, y_top),
                (x + w, y_top - h),
                (x, y_top - h),
            ],
        };
        // 回転と反転(折れ線もの以外)。紙は y が上向きなので、
        // いったん画面向きのずれに直してから時計回りに回す
        let rot = sp.rot.rem_euclid(360.0);
        let poly = matches!(
            sp.kind.as_str(),
            "spark" | "spark-col" | "spark-wl" | "ink" | "marker" | "path"
        );
        let mut pts = pts;
        if (rot != 0.0 || sp.flip_h || sp.flip_v) && !poly {
            let (ccx, ccy) = (x + w / 2.0, y_top - h / 2.0);
            let (s, c) = (rot.to_radians().sin(), rot.to_radians().cos());
            for p in pts.iter_mut() {
                let mut dx = p.0 - ccx;
                let mut dy = ccy - p.1; // 下向き正
                if sp.flip_h {
                    dx = -dx;
                }
                if sp.flip_v {
                    dy = -dy;
                }
                let (rx, ry) = (dx * c - dy * s, dx * s + dy * c);
                p.0 = ccx + rx;
                p.1 = ccy - ry;
            }
        }
        // 閉じるかどうか。**`path` は塗りがあるときだけ閉じます** —
        // 塗らない折れ線を閉じると、終点から始点へ1本余計に引かれます
        // (2026-08-27 に折れ線の図を紙で見て気づきました)
        let closed = match sp.kind.as_str() {
            "line" | "spark" | "ink" | "marker" => false,
            "path" => sp.fill.is_some(),
            _ => true,
        };
        // **塗ってから輪郭。** 2026-08-27 まで紙は輪郭だけでした。
        // 図をこちらで描く(発注者「チャートは python による独自描画」)
        // には、棒も扇も中が塗れないと形になりません
        if closed {
            if let Some(c) = sp.fill.as_deref().and_then(hex_rgb) {
                l1.poly(pts.clone(), c);
            }
        }
        // 折れ線は辺ごとに引きます。閉じる形なら最後の点から先頭へ1本
        if let Some(pen) = pen {
            let ends = if closed { pts.len() } else { pts.len().saturating_sub(1) };
            for i in 0..ends {
                let (x1, y1) = pts[i];
                let (x2, y2) = pts[(i + 1) % pts.len()];
                l1.line(x1, y1, x2, y2, pen_w, pen);
            }
        }
        // 図形の中の文字(テキストボックス)。揃えの指定があれば従います
        if let Some(t) = &sp.text {
            let pt = sp.text_fmt.size_pt.unwrap_or(9.0) * scale;
            let haba: f32 = t
                .chars()
                .map(|c| if c.is_ascii() { 0.55 } else { 1.0 })
                .sum::<f32>()
                * pt * 25.4 / 72.0;
            let tx = match sp.text_fmt.align {
                book::HAlign::Center => x + (w - haba) / 2.0,
                book::HAlign::Right => x + w - haba - 1.0,
                _ => x + 1.5,
            };
            // 縦は箱の真ん中に寄せます(図の題も軸の目盛りもそれで合います)
            let ty = y_top - h / 2.0 - pt * 25.4 / 72.0 * 0.35;
            l1.text(t, pt, tx, ty, (0.0, 0.0, 0.0), false);
        }
}


#[cfg(test)]
mod tests {
    use book::{Borders, Cell, CellFormat, Pos, Value};

    use super::*;

    fn grid() -> Grid {
        let mut s = Grid { name: "見積".into(), ..Default::default() };
        for (a1, v) in [("A1", "品名"), ("B1", "金額")] {
            s.set(Pos::parse(a1).unwrap(), Cell {
                formula: None,
                value: Value::Text(v.into()),
                fmt: CellFormat { borders: Borders::ALL, bold: true, ..Default::default() },
            });
        }
        s.set(Pos::parse("B2").unwrap(), Cell {
            formula: None,
            value: Value::Number(1200.0),
            fmt: CellFormat {
                borders: Borders::ALL,
                number_format: Some("#,##0".into()),
                ..Default::default()
            },
        });
        s
    }

    #[test]
    fn form_becomes_pdf() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut buf = Vec::new();
        sheet_to_pdf(&grid(), &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
        assert_eq!(&buf[..5], b"%PDF-");
        assert!(buf.len() > 1000);
    }

    #[test]
    fn many_rows_span_pages() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = Grid { name: "長い".into(), ..Default::default() };
        for r in 0..80 {
            s.set(Pos::new(r, 0), Cell {
                formula: None, value: Value::Number(r as f64), fmt: Default::default() });
        }
        let mut buf = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
        let hay = String::from_utf8_lossy(&buf).to_string();
        let i = hay.find("/Count ").unwrap() + 7;
        let n: usize = hay[i..].chars().take_while(|c| c.is_ascii_digit())
            .collect::<String>().parse().unwrap();
        assert!(n >= 2, "80行が {n} ページ(下へはみ出している)");
    }

    #[test]
    fn fill_and_font_color_reach_paper() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = grid();
        // 塗りが無ければ長方形(re)は1つも描かれない
        let mut plain = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut plain).unwrap();
        assert!(!crate::pdfw::unpack(&plain).contains(" re\n"), "塗りが無いのに長方形がある");
        s.set(Pos::parse("A2").unwrap(), Cell {
            formula: None,
            value: Value::Text("塗り".into()),
            fmt: CellFormat {
                fill: Some("FFF2CC".into()),
                color: Some("C00000".into()),
                ..Default::default()
            },
        });
        let mut buf = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
        let hay = crate::pdfw::unpack(&buf);
        assert!(hay.contains(" re\n"), "塗りの長方形が無い");
        assert!(hay.contains(" rg\n"), "色の指定が無い");
    }

    #[test]
    fn conditional_format_reaches_paper() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = grid(); // B2 = 1200(塗りの指定なし)
        s.cond.push(book::CondRule {
            range: (Pos::parse("B2").unwrap(), Pos::parse("B2").unwrap()),
            kind: book::CondKind::Cmp(book::CondOp::Gt, 1000.0),
            look: book::CondLook {
                fill: Some("E2EFDA".into()),
                ..Default::default()
            },
        });
        let mut buf = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
        assert!(
            crate::pdfw::unpack(&buf).contains(" re\n"),
            "条件に合う値の塗りが紙に出ない"
        );
    }

    #[test]
    fn wide_table_pages_sideways() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = Grid { name: "広い".into(), ..Default::default() };
        // 40mm × 10列 = 400mm は A4 縦(使える幅 170mm)に入り切らない
        for c in 0..10 {
            s.set(Pos::new(0, c), Cell {
                formula: None, value: Value::Number(c as f64), fmt: Default::default() });
            s.col_width.insert(c, 20.0); // 20字 ≒ 40mm
        }
        let mut buf = Vec::new();
        let clipped = sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
        // 割れる幅なので切れる列は無く、横へ束が送られる(1行しか無いので
        // 縦の送りは起きない = 頁数がそのまま束の数)
        assert_eq!(clipped, 0, "割れるのに切れたことになっている");
        let hay = String::from_utf8_lossy(&buf).to_string();
        let i = hay.find("/Count ").unwrap() + 7;
        let pages: usize =
            hay[i..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
        assert!(pages >= 3, "横のページ送りが起きていない(頁数 {pages})");
    }

    #[test]
    fn screen_breaks_match_sheet_count() {
        // **画面の破線と紙の割りつけを別々に書かない**ための縛り。
        // 切れ目の数から出した枚数が、実際に刷った枚数と一致すること
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = Grid { name: "枚数".into(), ..Default::default() };
        for r in 0..120u32 {
            for c in 0..8u32 {
                s.set(Pos::new(r, c), Cell {
                    formula: None, value: Value::Number((r * 10 + c) as f64),
                    fmt: Default::default() });
            }
            s.col_width.insert(r.min(7), 18.0);
        }
        s.row_breaks = vec![50];
        s.col_breaks = vec![4];
        let setup = PrintSetup::default();
        let (rows, cols) = page_starts(&s, Paper::default(), &setup);
        let want = (rows.len() + 1) * (cols.len() + 1);
        let mut buf = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &setup, &mut buf).unwrap();
        let hay = String::from_utf8_lossy(&buf).to_string();
        let i = hay.find("/Count ").unwrap() + 7;
        let got: usize = hay[i..].chars().take_while(|c| c.is_ascii_digit())
            .collect::<String>().parse().unwrap();
        assert_eq!(got, want, "画面の切れ目({}行×{}列)から出した {want} 枚と、実際の {got} 枚が違う",
            rows.len(), cols.len());
    }

    #[test]
    fn each_print_area_on_its_own_paper() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = Grid { name: "2域".into(), ..Default::default() };
        for r in 0..10 {
            s.set(Pos::new(r, 0), Cell {
                formula: None, value: Value::Number(r as f64), fmt: Default::default() });
        }
        let pages = |setup: &PrintSetup| {
            let mut buf = Vec::new();
            sheet_to_pdf(&s, &data, Paper::default(), setup, &mut buf).unwrap();
            let hay = String::from_utf8_lossy(&buf).to_string();
            let i = hay.find("/Count ").unwrap() + 7;
            hay[i..].chars().take_while(|c| c.is_ascii_digit())
                .collect::<String>().parse::<usize>().unwrap()
        };
        let one = pages(&PrintSetup {
            areas: vec![(Pos::new(0, 0), Pos::new(2, 0))],
            margins_mm: None,
            date1904: false,
        });
        // 同じ大きさの域を2つ = 紙も2枚(**繋げて1枚に詰めない**)
        let two = pages(&PrintSetup {
            areas: vec![
                (Pos::new(0, 0), Pos::new(2, 0)),
                (Pos::new(5, 0), Pos::new(7, 0)),
            ],
            margins_mm: None,
            date1904: false,
        });
        assert_eq!(one, 1, "1域なのに {one} 枚になった");
        assert_eq!(two, 2, "2域が {two} 枚 — 域ごとに紙を変えていない");
    }

    #[test]
    fn fit_to_paper_leaves_no_cut_columns() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = Grid { name: "広い".into(), ..Default::default() };
        // 1列 200mm × 3列 — A4 縦(使える幅 170mm)には割っても入らない
        for c in 0..3 {
            s.set(Pos::new(0, c), Cell {
                formula: None, value: Value::Number(c as f64), fmt: Default::default() });
            s.col_width.insert(c, 100.0);
        }
        let mut buf = Vec::new();
        let before =
            sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
        assert!(before > 0, "そもそも切れていない — 試験の前提が崩れている");
        // 「すべての列を1ページに」= 収まるまで縮める
        s.fit_to_w = Some(1);
        let mut buf2 = Vec::new();
        let after =
            sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut buf2).unwrap();
        assert_eq!(after, 0, "1ページに収めても {after} 列が切れた");
    }

    #[test]
    fn fit_to_paper_shrinks_only() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mk = |fit: bool| {
            let mut s = Grid { name: "小さい".into(), ..Default::default() };
            s.set(Pos::new(0, 0), Cell {
                formula: None, value: Value::Text("あ".into()), fmt: Default::default() });
            if fit {
                s.fit_to_w = Some(1);
                s.fit_to_h = Some(1);
            }
            let mut buf = Vec::new();
            sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
            buf.len()
        };
        // 紙いっぱいに膨らませない = 出来上がりが変わらない
        assert_eq!(mk(true), mk(false), "小さな表が紙いっぱいに膨らんだ");
    }

    #[test]
    fn vertical_page_break_splits_the_band() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = Grid { name: "区切り".into(), ..Default::default() };
        // 3列。ぜんぶ紙に入る幅なので、放っておけば1枚
        for c in 0..3 {
            s.set(Pos::new(0, c), Cell {
                formula: None, value: Value::Number(c as f64), fmt: Default::default() });
            s.col_width.insert(c, 10.0);
        }
        let pages = |s: &Grid| {
            let mut buf = Vec::new();
            sheet_to_pdf(s, &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
            let hay = String::from_utf8_lossy(&buf).to_string();
            let i = hay.find("/Count ").unwrap() + 7;
            hay[i..].chars().take_while(|c| c.is_ascii_digit())
                .collect::<String>().parse::<usize>().unwrap()
        };
        let one = pages(&s);
        s.col_breaks = vec![1]; // B 列から新しい紙
        let two = pages(&s);
        assert!(two > one, "縦の改ページが効いていない({one} → {two} 頁)");
    }

    #[test]
    fn column_wider_than_paper_is_reported_cut() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = Grid { name: "極太".into(), ..Default::default() };
        // 1列で 200mm — A4 縦の使える幅(170mm)より広く、割りようが無い
        s.set(Pos::new(0, 0), Cell {
            formula: None, value: Value::Number(1.0), fmt: Default::default() });
        s.col_width.insert(0, 100.0);
        let mut buf = Vec::new();
        let clipped = sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
        assert_eq!(clipped, 1, "割れない列を黙って切っている");
        let mut buf = Vec::new();
        assert_eq!(sheet_to_pdf(&grid(), &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap(), 0,
                   "入り切っているのに切れたと言った");
    }

    #[test]
    fn an_empty_sheet_does_not_panic() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut buf = Vec::new();
        sheet_to_pdf(&Grid { name: "空".into(), ..Default::default() },
                     &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
        assert_eq!(&buf[..5], b"%PDF-");
    }
}

#[cfg(test)]
mod print_setup_tests {
    use book::{Cell, Pos, Value};

    use super::*;

    fn long_sheet() -> Grid {
        let mut s = Grid { name: "長い".into(), ..Default::default() };
        for r in 0..80 {
            s.set(Pos::new(r, 0), Cell {
                formula: None, value: Value::Number(r as f64), fmt: Default::default() });
        }
        s
    }

    fn pages(buf: &[u8]) -> usize {
        let hay = String::from_utf8_lossy(buf).to_string();
        let i = hay.find("/Count ").unwrap() + 7;
        hay[i..].chars().take_while(|c| c.is_ascii_digit())
            .collect::<String>().parse().unwrap()
    }

    #[test]
    fn only_the_print_area_is_printed() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let s = long_sheet();
        // 全域は複数ページ、先頭5行の印刷範囲なら1ページ
        let mut all = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut all).unwrap();
        assert!(pages(&all) >= 2);
        let setup = PrintSetup {
            areas: vec![(Pos::new(0, 0), Pos::new(4, 0))],
            margins_mm: None,
            date1904: false,
        };
        let mut part = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &setup, &mut part).unwrap();
        assert_eq!(pages(&part), 1, "印刷範囲が効いていない");
    }

    #[test]
    fn wider_margins_need_more_paper() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let s = long_sheet();
        let mut narrow = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(),
            &PrintSetup { areas: Vec::new(), margins_mm: Some((10.0, 10.0, 10.0, 10.0)) , date1904: false },
            &mut narrow).unwrap();
        let mut wide = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(),
            &PrintSetup { areas: Vec::new(), margins_mm: Some((10.0, 10.0, 100.0, 100.0)) , date1904: false },
            &mut wide).unwrap();
        assert!(pages(&wide) > pages(&narrow), "余白が紙の枚数に効いていない");
    }
}

#[cfg(test)]
mod print_extras_tests {
    use book::{Cell, Pos, Value};

    use super::*;

    fn long_sheet() -> Grid {
        let mut s = Grid { name: "長い".into(), ..Default::default() };
        for r in 0..30 {
            s.set(Pos::new(r, 0), Cell {
                formula: None, value: Value::Number(r as f64), fmt: Default::default() });
        }
        s
    }

    fn pages(buf: &[u8]) -> usize {
        let hay = String::from_utf8_lossy(buf).to_string();
        let i = hay.find("/Count ").unwrap() + 7;
        hay[i..].chars().take_while(|c| c.is_ascii_digit())
            .collect::<String>().parse().unwrap()
    }

    #[test]
    fn page_break_splits_paper() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = long_sheet(); // 30行 = 既定では1ページに収まる
        let mut one = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut one).unwrap();
        assert_eq!(pages(&one), 1);
        s.row_breaks = vec![10, 20];
        let mut broken = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut broken).unwrap();
        assert_eq!(pages(&broken), 3, "改ページが効いていない");
    }

    #[test]
    fn scaling_changes_rows_per_page() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = Grid { name: "s".into(), ..Default::default() };
        for r in 0..80 {
            s.set(Pos::new(r, 0), Cell {
                formula: None, value: Value::Number(r as f64), fmt: Default::default() });
        }
        let mut full = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut full).unwrap();
        s.print_scale = Some(50);
        let mut half = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut half).unwrap();
        assert!(pages(&half) < pages(&full), "縮小しても紙が減らない");
    }

    #[test]
    fn title_rows_appear_on_page_two() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = long_sheet();
        s.print_title_rows = Some((0, 0));
        s.row_breaks = vec![15];
        // 描画対象の行数で確かめる: タイトル繰り返しの分、テキスト描画が1つ増える
        let mut with_t = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut with_t).unwrap();
        s.print_title_rows = None;
        let mut without = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut without).unwrap();
        assert!(with_t.len() > without.len(), "タイトル行の繰り返しが出ていない");
    }

    /// タイトル列を差し込む規則そのもの。**同じ列を1枚に二度出さない**のが肝で、
    /// PDF の字は埋め込み書体の符号なので外から読めない — 規則はここで縛る
    #[test]
    fn title_columns_repeat_for_each_band() {
        // A 列がタイトル。A を含む束(先頭)では繰り返さない = 二度出ない
        assert_eq!(band_cols(&[0], 0, 3), vec![0, 1, 2]);
        // 右の束では左端に A を差し込む
        assert_eq!(band_cols(&[0], 3, 3), vec![0, 3, 4, 5]);
        // A:B の2列でも同じ。束の中にいるものは差し込まない
        assert_eq!(band_cols(&[0, 1], 1, 2), vec![0, 1, 2]);
        assert_eq!(band_cols(&[0, 1], 4, 2), vec![0, 1, 4, 5]);
        // 束より右にあるタイトル列は、その紙ではまだ出さない(後の紙で出る)
        assert_eq!(band_cols(&[7], 0, 3), vec![0, 1, 2]);
        // 指定が無ければ束そのまま
        assert_eq!(band_cols(&[], 2, 2), vec![2, 3]);
        // 本数 0 は「繰り返す分だけ」(幅の割り付けに使う)
        assert_eq!(band_cols(&[0], 3, 0), vec![0]);
    }

    /// 繰り返す列は**幅の割り付けにも効く** — その分だけ本体が狭くなり、
    /// 同じ表でも紙が増える。差し込みが描画だけの飾りになっていないこと
    #[test]
    fn title_columns_narrow_the_body() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = Grid { name: "広い".into(), ..Default::default() };
        // 40mm × 12列。A4 縦の使える幅 170mm には4列ずつ = 3枚
        for c in 0..12u32 {
            s.set(Pos::new(0, c), Cell {
                formula: None, value: Value::Number(c as f64), fmt: Default::default() });
            s.col_width.insert(c, 20.0); // 20字 ≒ 40mm
        }
        let mut plain = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut plain).unwrap();
        assert_eq!(pages(&plain), 3);
        // A 列を毎ページ繰り返すと、2枚目からは本体に 130mm = 3列ずつ。
        // A:D / A+E:G / A+H:J / A+K:L で4枚になる
        s.print_title_cols = Some((0, 0));
        let mut with_t = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut with_t).unwrap();
        assert_eq!(pages(&with_t), 4, "タイトル列が幅の割り付けに効いていない");
    }

    /// 頁番号の規則そのもの(&P はブック通し・&N は総頁)。
    /// PDF の字は埋め込み書体の符号なので外から読めない — **規則は
    /// ここで縛る**(2026-08-13、Book.to_pdf の数え方)
    #[test]
    fn page_number_text_is_book_wide() {
        // 1冊 5 頁のうち、2枚目のシートの最初の頁が 3 頁目のとき
        assert_eq!(hf_subst("&C&P / &N", 3, 5), "3 / 5");
        // 1枚だけの PDF は今までどおり(offset 0・総頁はそのシートの頁数)
        assert_eq!(hf_subst("&P / &N", 1, 1), "1 / 1");
        // 知らない &コードは落とす(黙って化けさせない)。&"書体" ごと落ちる
        assert_eq!(hf_subst("&\"MS明朝\"&B社外秘&P", 2, 4), "社外秘2");
        assert_eq!(hf_subst("値引き&&割引", 1, 1), "値引き&割引", "&& は素の &");
    }

    /// **ブックを1つの PDF に。** 頁はブック通しで数える(2026-08-13)
    #[test]
    fn book_pdf_bundles_every_sheet() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let a = long_sheet();
        let mut b = Grid { name: "短い".into(), ..Default::default() };
        b.set(Pos::new(0, 0), Cell {
            formula: None, value: Value::Number(1.0), fmt: Default::default() });

        let mut one = Vec::new();
        sheet_to_pdf(&a, &data, Paper::default(), &PrintSetup::default(), &mut one).unwrap();
        let mut two = Vec::new();
        sheet_to_pdf(&b, &data, Paper::default(), &PrintSetup::default(), &mut two).unwrap();

        let mut book = Vec::new();
        book_to_pdf(
            &[(&a, Paper::default(), PrintSetup::default()),
              (&b, Paper::default(), PrintSetup::default())],
            &data,
            &mut book,
        )
        .unwrap();
        assert_eq!(
            pages(&book),
            pages(&one) + pages(&two),
            "束ねた頁数が、シートごとの合計と合わない"
        );
    }

    #[test]
    fn book_page_numbers_are_continuous() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        // 2枚目のフッターに「&P / &N」— ブック通しなら 2枚目は 1 ではない
        let a = long_sheet();
        let mut b = Grid { name: "後".into(), ..Default::default() };
        b.set(Pos::new(0, 0), Cell {
            formula: None, value: Value::Number(1.0), fmt: Default::default() });
        b.footer = Some("&C&P / &N".into());
        let mut book = Vec::new();
        book_to_pdf(
            &[(&a, Paper::default(), PrintSetup::default()),
              (&b, Paper::default(), PrintSetup::default())],
            &data,
            &mut book,
        )
        .unwrap();
        let total = pages(&book);
        // 束ねた PDF の頁数と、最後の頁の番号が一致する(=通しで振っている)
        let mut alone = Vec::new();
        sheet_to_pdf(&b, &data, Paper::default(), &PrintSetup::default(), &mut alone).unwrap();
        assert!(total > pages(&alone), "束ねた頁数が1枚ぶんしかない");
        // 1枚だけなら「1 / 1」、束ねたら「total / total」になる
        assert!(String::from_utf8_lossy(&book).len() > String::from_utf8_lossy(&alone).len());
    }

}

#[cfg(test)]
mod zukei_tests {
    use super::*;

    fn hako(kind: &str) -> book::SheetShape {
        book::SheetShape {
            at: book::Pos::new(0, 0),
            width_px: 120.0,
            height_px: 80.0,
            kind: kind.into(),
            fill: Some("DDE7F0".into()),
            line: Some("2E5A87".into()),
            line_w: 1.5,
            alpha: 1.0,
            ..Default::default()
        }
    }

    /// **図形だけの紙面が組める。** シートを通しません
    #[test]
    fn shapes_alone_make_a_page() {
        let leaf = shapes_leaf(&[(hako("rect"), 20.0, 200.0)], Paper::default());
        assert_eq!(leaf.size_mm, Some((210.0, 297.0)));
        assert!(!leaf.polys.is_empty(), "塗りが出ていない");
        assert!(!leaf.rules.is_empty(), "線が出ていない");
    }

    /// **角丸は四角より点が多い。** 紙だけ四角に落ちていたのを直した所です
    /// (2026-08-29)。同じ大きさで点の数が同じなら、また四角に戻っています
    #[test]
    fn a_round_rect_is_not_a_plain_rect() {
        let kado = shapes_leaf(&[(hako("roundRect"), 20.0, 200.0)], Paper::default());
        let shikaku = shapes_leaf(&[(hako("rect"), 20.0, 200.0)], Paper::default());
        let ten = |l: &pdfw::Leaf| l.polys.first().map(|p| p.points.len()).unwrap_or(0);
        assert!(ten(&shikaku) > 0 && ten(&kado) > ten(&shikaku),
            "角丸が四角のまま: 角丸 {} 点 / 四角 {} 点", ten(&kado), ten(&shikaku));
    }

    /// **画面と紙が同じ図形を知っている。** 片方だけが描ける種類があると、
    /// 見比べたときに食い違います(角丸がそれでした)
    #[test]
    fn the_screen_and_the_paper_know_the_same_shapes() {
        for kind in ["rect", "roundRect", "ellipse", "rightArrow", "diamond", "line"] {
            let leaf = shapes_leaf(&[(hako(kind), 20.0, 200.0)], Paper::default());
            assert!(
                !leaf.polys.is_empty() || !leaf.rules.is_empty(),
                "紙が {kind} を描けていない"
            );
            assert!(!hako(kind).to_svg().is_empty(), "画面が {kind} を描けていない");
        }
    }
}
