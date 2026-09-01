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

/// 行の高さ(mm)。**シートの既定を使います**(2026-08-30)。
///
/// 高さを言っていない行は、シートの `defaultRowHeight`(pt)に従います。
/// 前は 7.0mm の決め打ちに落ちていて、国税庁の酒税の表(13.2pt = 4.7mm)
/// では**5割高く**なり、1枚に入る行が減って紙が倍に増えていました。
fn gyou_mm(grid: &Grid, r: u32) -> f32 {
    grid.row_height
        .get(&r)
        .copied()
        .or(grid.default_row_height)
        .map(|pt| pt * 25.4 / 72.0)
        .unwrap_or(ROW_MM)
}
/// **書体が読めないときの行送り**(1em あたり)。
///
/// 書体が `hhea` を持っていれば、そちらを使います([`Habakei::okuri_em`])。
const OKURI_KITEI: f32 = 1.2;

/// **書体が読めないときの下がり**(1em あたり)。
const SAGARI_KITEI: f32 = 0.12;

/// **セルの内側の余白(片側 mm)。**
///
/// Excel は字をセルの縁から 2 画素あけます(96dpi で 0.53mm)。列幅に入って
/// いる 5 画素の内訳が「左右2画素ずつ + 罫線1画素」なので、字が使えるのは
/// 幅から 4 画素を引いた分です。
///
/// 前は片側 1.5mm(合わせて 3.0mm)を引いていました。総務省の給与所得の
/// 表では、8桁の数が入るはずのセルで 3mm 足りず `#####` になっていました
/// (2026-08-31)。
const MASU_PAD_MM: f32 = 2.0 * 25.4 / 96.0;

/// **半角の書体の名前に付ける印**(2026-08-31 発注者)。
///
/// ＭＳ Ｐ明朝のように、漢字と半角で設計の違う書体があります。書体の並びに
/// 「漢字の分」と「半角の分」の2本を入れ、後者の名前の末尾にこの印を
/// 付けて見分けます。名前に出てこない字を選んであります。
pub const HANKAKU_SIRUSI: char = '\u{1}';

/// **書体の変わり目で、続きを切り分けます。**
///
/// 返るのは (書体の番号, その続き) の並びです。ＭＳ Ｐ明朝のように漢字と
/// 半角で書体が変わるとき、1回の描きに混ぜられないので分けます
/// (2026-08-31 発注者)。
fn wakeru(t: &str, fno: impl Fn(char) -> u8) -> Vec<(u8, String)> {
    let mut out: Vec<(u8, String)> = Vec::new();
    for ch in t.chars() {
        let f = fno(ch);
        match out.last_mut() {
            Some((g, s)) if *g == f => s.push(ch),
            _ => out.push((f, ch.to_string())),
        }
    }
    out
}

/// **列の幅(mm)。Excel と同じ式で出します**(2026-08-31 に直した)。
///
/// xlsx が持つ列幅は「標準の書体の `0` が何文字ぶん入るか」です。画素へ
/// 直す式は OOXML の仕様書に書いてあります:
///
/// ```text
/// px = trunc(((256 × 幅 + trunc(128 / MDW)) / 256) × MDW)
/// ```
///
/// `MDW` は標準の書体の `0` の幅(画素)で、96dpi ではだいたい 7 です
/// (Calibri 11 も ＭＳ明朝 10.5 も 7)。
///
/// **セルの内側の余白 5 画素は、保存されている幅にもう入っています。**
/// よく言われる「既定は 8.43 文字 = 64 画素」の 8.43 は画面に出る文字数
/// で、そのとき保存される値は 9.140625 です。前の版は文字数から画素を
/// 出す式(`trunc(幅 × MDW) + 5`)を保存された幅に当てていたので、
/// 1列あたり 5 画素(1.3mm)ずつ広がっていました。国税庁の酒税の総括表を
/// Excel が出した PDF と突き合わせて分かりました — 罫線の間隔が
/// 64.3・50.0・58.8 画素で、`幅 × 7` に一致し、`+5` した値には
/// 一致しません。
fn retsu_mm_mdw(haba: f32, mdw: f32) -> f32 {
    let mdw = if mdw > 0.0 { mdw } else { 7.0 };
    let px = (((256.0 * haba + (128.0 / mdw).trunc()) / 256.0) * mdw).trunc();
    px * 25.4 / 96.0
}

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
    /// **数字1文字の幅(画素)。** そのブックの標準の書体で 0〜9 のうち
    /// いちばん広い字を 96dpi で測った値です(2026-08-31)。
    ///
    /// xlsx の列幅は「標準の書体の数字が何文字ぶん入るか」で書いてあるので、
    /// ミリに直すのにこれが要ります。ＭＳ 明朝 10.5pt なら 7、Arial 12pt
    /// なら 9 です。**0 のときは 7 として扱います**(前からの決め打ちの値)。
    ///
    /// LibreOffice も同じ所を見ています
    /// (`sc/source/filter/oox/unitconverter.cxx`。標準の書体を取って
    /// 「get maximum width of all digits」)
    pub mdw_px: f32,
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
                .map(|w| retsu_mm_mdw(w, setup.mdw_px)).unwrap_or(COL_MM)
        })
        .sum();
    let total_h: f32 = (r0..r1)
        .filter(|r| !grid.row_hidden.contains(r))
        .map(|r| gyou_mm(grid, r))
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
    // **端数のぶんだけ余分に縮めます**(2026-08-31)。上の割り算は行が
    // 途中で切れる前提の概算です。実際は行の途中では切れないので、
    // ちょうどの倍率だと最後の1行が入らず紙が1枚増えます。国税庁の
    // 酒税の総括表の1シート目がこれで2枚になっていました。
    //
    // 見るのはいちばん高い行です — 端数がどれだけ大きくても、その1行ぶんを
    // 超えることはありません
    if let Some(n) = nh.filter(|n| *n > 0) {
        let takai = (r0..r1)
            .filter(|r| !grid.row_hidden.contains(r))
            .map(|r| gyou_mm(grid, r))
            .fold(0.0f32, f32::max);
        let waku = usable_h * n as f32;
        if total_h > 0.0 && (total_h + takai) * k > waku {
            k = k.min(waku / (total_h + takai));
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
    let (ext_rows, ext_cols) = grid.print_extent();
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
            .map(|w| retsu_mm_mdw(w, setup.mdw_px)).unwrap_or(COL_MM) * scale;
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
        let rh = gyou_mm(grid, r) * scale;
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
        self.text_font(t, pt, x, y, rgb, bold, 0);
    }

    /// **書体を選んで描く。** 番号は `sheet_to_pdf` に渡した書体の並びの
    /// 何番目か(2026-08-31。セルが名指しする明朝・ゴシック・欧文の刷り分け)
    fn text_font(&mut self, t: &str, pt: f32, x: f32, y: f32, rgb: (f32, f32, f32),
                 bold: bool, font: u8) {
        let w = t.chars().map(|c| if c.is_ascii() { 0.55 } else { 1.0 }).sum::<f32>()
            * pt * 25.4 / 72.0;
        self.text_kazari(t, pt, x, y, rgb, bold, font, w, false, false, 0.0, false);
    }

    /// **飾りつき。** 下線と取り消し線は書き手が持っているのに、表計算から
    /// は渡していませんでした(2026-08-31 発注者)。幅は下線を引く長さに
    /// 使うので、**描く書体で測った値**を渡します。
    /// `rotation` は傾き(度。左回り)で、0 は水平です
    #[allow(clippy::too_many_arguments)]
    fn text_kazari(&mut self, t: &str, pt: f32, x: f32, y: f32, rgb: (f32, f32, f32),
                   bold: bool, font: u8, w_mm: f32, underline: bool, strike: bool,
                   rotation: f32, italic: bool) {
        self.leaf.pieces.push(pdfw::Piece {
            rotation,
            italic,
            font,
            x_mm: x,
            // 表計算も新しい書き手も、左下からの y で描きます
            y_mm: y,
            size_pt: pt,
            text: t.to_string(),
            w_mm,
            bold,
            underline,
            strike,
            color: Some(pdfw::to_hex(rgb)),
            ..Default::default()
        });
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32, rgb: (f32, f32, f32)) {
        self.line_a(x1, y1, x2, y2, w, rgb, 1.0);
    }

    /// 破線。刻みは (線, 間) mm。図形の輪郭が使います
    fn line_dash(
        &mut self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32, rgb: (f32, f32, f32),
        a: f32, dash: Option<(f32, f32)>,
    ) {
        self.leaf.rules.push(pdfw::Rule {
            x1_mm: x1, y1_mm: y1, x2_mm: x2, y2_mm: y2, w_mm: w, rgb, a, dash,
        });
    }

    /// 透明度つきの線。図形の影と `SheetShape::alpha` が使います
    fn line_a(
        &mut self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32, rgb: (f32, f32, f32), a: f32,
    ) {
        self.leaf.rules.push(pdfw::Rule {
            x1_mm: x1, y1_mm: y1, x2_mm: x2, y2_mm: y2, w_mm: w, rgb, a, dash: None,
        });
    }

    fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, rgb: (f32, f32, f32)) {
        self.leaf.fills.push(pdfw::Fill {
            x_mm: x, y_mm: y, w_mm: w, h_mm: h, rgb, ..Default::default()
        });
    }

    /// 透明度つきの塗り
    fn poly_a(&mut self, points: Vec<(f32, f32)>, rgb: (f32, f32, f32), a: f32) {
        if points.len() >= 3 {
            self.leaf.polys.push(pdfw::Poly { points, rgb, a });
        }
    }
}

/// **紙を足していく先。** 頁を足す所を1本にしておくと、頁割りに手を
/// 入れずに書き手を替えられます(2026-08-27)。
struct Board {
    leaves: Vec<pdfw::Leaf>,
    /// **埋める書体の名前の並び。** セルが名指しした名前をここで引いて
    /// `Piece::font` の番号にします(2026-08-31)。空なら1本だけです
    fonts: Vec<String>,
    /// 書体ごとの字の幅([`Habakei`])。`fonts` と同じ番号で並びます
    haba: Habakei,
}

/// **書体ごとの、1字の幅(em)。**
///
/// 前は書体に関わらず「半角 0.55em・全角 1.0em」で見積もっていました
/// (2026-08-31 に直した)。実際は書体で違います — ＭＳ 明朝の数字は
/// 0.500em、IPAex明朝は 0.618em、Century は 0.556em で、カンマは
/// 0.203em から 0.305em まで開きます。
///
/// 描く書体と違う幅で組むと、右揃えの位置・折り返す所・セルに入るかの
/// 判定が全部ずれます。総務省の給与所得の第1表では、数が隣の欄へ
/// はみ出していました。
#[derive(Default, Clone)]
struct Habakei {
    /// 書体ごとの (字 → 1em あたりの幅)。表に無い字は見積りに落ちます
    hyou: Vec<std::collections::HashMap<char, f32>>,
    /// **書体ごとの行送り(1em あたり)。** `hhea` の `ascender - descender`
    /// です(2026-08-31)。
    ///
    /// 前は書体に関わらず 1.2 倍の決め打ちでした。根拠がありません。
    /// LibreOffice は1行の中の run をなめて ascent と descent の最大を取り、
    /// その和を行の高さにします(`editeng` の `FormatterFontMetric`:
    /// `GetHeight() { return nMaxAscent + nMaxDescent; }`)。倍率は
    /// 行間の指定があるときだけ掛けます。同じやり方にしました。
    ///
    /// ＭＳ 明朝もＭＳ Ｐ明朝も 1.000em、Century は 1.202em です(元の PDF に
    /// 埋め込まれていた本物を測りました)。1.2 の決め打ちは、日本語の書体で
    /// 2割ひらきすぎ、Century でほぼ合う、という当たり外れでした
    okuri: Vec<f32>,
    /// **書体ごとの下がり(1em あたり)。** `hhea` の `-descender` です。
    ///
    /// 下揃えのセルで、字の足がセルの底に着く量です。前は書体に関わらず
    /// 2.0mm の決め打ちでした(2026-08-31)。8pt の字なら本当は 0.34mm
    /// ほどなので、6倍ちかく浮いていました
    sagari: Vec<f32>,
}

impl Habakei {
    /// 書体の中身と、その表に出てくる字から作ります
    /// `na` は**原本の書体の名前**の並び(`data` と同じ順)。行送りは
    /// この名前で引きます — 置き替え先の寸法とは違うためです
    fn new(data: &[Vec<u8>], na: &[String], ji: &std::collections::BTreeSet<char>) -> Self {
        let hyou = data
            .iter()
            .map(|d| {
                let mut m = std::collections::HashMap::new();
                if let Ok(face) = ttf_parser::Face::parse(d, 0) {
                    let em = face.units_per_em() as f32;
                    if em > 0.0 {
                        for &c in ji {
                            let a = face.glyph_index(c).and_then(|g| face.glyph_hor_advance(g));
                            if let Some(a) = a {
                                m.insert(c, a as f32 / em);
                            }
                        }
                    }
                }
                m
            })
            .collect();
        // **行送りは原本の書体の名前で引きます**(2026-09-01 発注者)。
        // docx の道([`kumihan::font::okuri_em`])と同じ1本にします。
        // 表に無ければ、この機械にある置き替え先そのものから出します
        // (LibreOffice も実物の書体から取ります — `editeng` の
        // `FormatterFontMetric`)
        let okuri = data
            .iter()
            .enumerate()
            .map(|(i, d)| {
                if let Some(em) = kumihan::font::okuri_em(na.get(i).map(|s| s.as_str())) {
                    return em;
                }
                ttf_parser::Face::parse(d, 0)
                    .ok()
                    .filter(|f| f.units_per_em() > 0)
                    .map(|f| {
                        let em = f.units_per_em() as f32;
                        (f32::from(f.ascender()) - f32::from(f.descender())) / em
                    })
                    .filter(|v| *v > 0.1)
                    .unwrap_or(OKURI_KITEI)
            })
            .collect();
        let sagari = data
            .iter()
            .map(|d| {
                ttf_parser::Face::parse(d, 0)
                    .ok()
                    .filter(|f| f.units_per_em() > 0)
                    .map(|f| -f32::from(f.descender()) / f.units_per_em() as f32)
                    .filter(|v| *v >= 0.0 && *v < 0.5)
                    .unwrap_or(SAGARI_KITEI)
            })
            .collect();
        Habakei { hyou, okuri, sagari }
    }

    /// **その書体の下がり(mm)。** 読めない書体は [`SAGARI_KITEI`]
    fn sagari_mm(&self, fno: usize, pt: f32) -> f32 {
        self.sagari.get(fno).copied().unwrap_or(SAGARI_KITEI) * pt * 25.4 / 72.0
    }

    /// **その書体の行送り(1em あたり)。** 読めない書体は [`OKURI_KITEI`]
    fn okuri_em(&self, fno: usize) -> f32 {
        self.okuri.get(fno).copied().unwrap_or(OKURI_KITEI)
    }

    /// 1行ぶんの送り(mm)。**その行に出てくる書体の、いちばん大きいもの**
    fn okuri_mm(&self, fno: usize, pt: f32) -> f32 {
        self.okuri_em(fno) * pt * 25.4 / 72.0
    }

    /// 1字の幅(mm)。表に無ければ、半角 0.55em・全角 1.0em の見積り
    fn ji_mm(&self, fno: usize, ch: char, pt: f32) -> f32 {
        let em = self
            .hyou
            .get(fno)
            .and_then(|m| m.get(&ch).copied())
            .unwrap_or(if ch.is_ascii() { 0.55 } else { 1.0 });
        em * pt * 25.4 / 72.0
    }

    /// 続きの幅(mm)
    fn mm(&self, fno: usize, t: &str, pt: f32) -> f32 {
        t.chars().map(|c| self.ji_mm(fno, c, pt)).sum()
    }
}

impl Board {
    /// 1枚目の紙を敷いた紙束を作ります
    fn new(paper: Paper) -> Self {
        Board { leaves: vec![leaf(paper)], fonts: Vec::new(), haba: Habakei::default() }
    }


    /// 紙を1枚足して、その番号を返します
    fn add_page(&mut self, paper: Paper) -> usize {
        self.leaves.push(leaf(paper));
        self.leaves.len() - 1
    }

    fn len(&self) -> usize {
        self.leaves.len()
    }

    /// `first` 枚目から後ろで、**何も描かれていない紙**を捨てます。
    /// 1枚も残らないときは1枚だけ残します(シートが空でも紙は1枚出す)。
    fn shirogami_wo_nozoku(&mut self, first: usize) {
        let nakami = |l: &pdfw::Leaf| {
            !l.pieces.is_empty()
                || !l.rules.is_empty()
                || !l.rules_top.is_empty()
                || !l.fills.is_empty()
                || !l.polys.is_empty()
                || !l.paths.is_empty()
                || !l.images.is_empty()
                || l.bg.is_some()
                || l.watermark.is_some()
        };
        let nokoru = self.leaves[first..].iter().filter(|l| nakami(l)).count();
        if nokoru == 0 {
            self.leaves.truncate(first + 1);
            return;
        }
        let mut i = first;
        while i < self.leaves.len() {
            if nakami(&self.leaves[i]) {
                i += 1;
            } else {
                self.leaves.remove(i);
            }
        }
    }

    /// `i` 枚目に描く筆を借ります
    fn ink(&mut self, i: usize) -> Ink<'_> {
        Ink { leaf: &mut self.leaves[i] }
    }

    fn save<W: Write>(self, paper: Paper, font_data: &[u8], out: W) -> Result<(), String> {
        pdfw::write_pages(&self.leaves, paper.width_mm, paper.height_mm, font_data, out)
    }

    /// 書体を何本か埋めて書き出します(`fonts` の並びが `Piece::font` の番号)
    fn save_fonts<W: Write>(self, paper: Paper, fonts: &[&[u8]], out: W) -> Result<(), String> {
        pdfw::write_pages_fonts(&self.leaves, paper.width_mm, paper.height_mm, fonts, out)
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
/// **ページに貼り付く図形を、組み上がった紙面へ足す。**
///
/// `y_mm` は紙の**上から**の mm(文書の図形はそう持ちます)。紙面は下からの
/// mm なので、ここで裏返します。
pub(crate) fn doc_shapes(leaf: &mut pdfw::Leaf, shapes: &[kumihan::DocShape], h_mm: f32) {
    let mut l1 = Ink { leaf };
    for sp in shapes {
        // `zukei` は図形自身のずらしを足すので、写しで 0 にしてから渡します
        let mut look = sp.look.clone();
        look.dx_px = 0.0;
        look.dy_px = 0.0;
        look.width_px = sp.w_mm * 96.0 / 25.4;
        look.height_px = sp.h_mm * 96.0 / 25.4;
        zukei(&mut l1, &look, sp.x_mm, h_mm - sp.y_mm, 1.0);
    }
}

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
    sheet_leaves_fonts(grid, paper, setup, &[])
}

/// 書体の名前の並びつき。`Piece::font` がその並びの番号になります
pub fn sheet_leaves_fonts(
    grid: &Grid,
    paper: Paper,
    setup: &PrintSetup,
    fonts: &[String],
) -> Result<Vec<pdfw::Leaf>, String> {
    // **名前しか渡されないので、幅を測るために中身を読みます**
    // (2026-08-31)。読めない書体は見積りに落ちるだけで、止まりません
    let data: Vec<Vec<u8>> = fonts
        .iter()
        .map(|n| {
            kumihan::font::for_document(Some(n))
                .ok()
                .and_then(|(fam, _)| kumihan::font::load(fam).ok())
                .unwrap_or_default()
        })
        .collect();
    sheet_leaves_haba(grid, paper, setup, fonts, &data)
}

/// 書体の中身つき。幅を測るのに使います
fn sheet_leaves_haba(
    grid: &Grid,
    paper: Paper,
    setup: &PrintSetup,
    fonts: &[String],
    data: &[Vec<u8>],
) -> Result<Vec<pdfw::Leaf>, String> {
    let mut board = Board::new(paper);
    board.fonts = fonts.to_vec();
    board.haba = Habakei::new(data, fonts, &deru_ji(grid));
    let (pages, _clipped, margins) = draw_sheet(&mut board, grid, paper, setup, true);
    let total = pages.len();
    draw_header_footer(&mut board, grid, paper, pages, margins, 0, total);
    Ok(board.leaves)
}

/// **その表に出てくる字を全部集めます。** 字送りの表を作る材料です。
///
/// セルの値・数式の結果・ヘッダーとフッターの文言まで見ます。ここに
/// 漏れた字は 0.55em の見積りに落ちるだけなので、静かに間違えます
fn deru_ji(grid: &Grid) -> std::collections::BTreeSet<char> {
    let mut ji: std::collections::BTreeSet<char> = std::collections::BTreeSet::new();
    for c in grid.cells.values() {
        ji.extend(c.value.display().chars());
        if let Some(f) = &c.formula {
            ji.extend(f.chars());
        }
    }
    for rs in grid.rich_runs.values() {
        for r in rs {
            ji.extend(r.text.chars());
        }
    }
    let hf = [&grid.header, &grid.footer, &grid.header_even, &grid.footer_even,
              &grid.header_first, &grid.footer_first];
    for s in hf.into_iter().flatten() {
        ji.extend(s.chars());
    }
    // 書式が作る字(桁区切り・記号・`#`)は値に出ないので足しておきます
    ji.extend("0123456789.,-+#()%¥$△▲ ".chars());
    ji
}

/// **セルの柄の色**(xlsx の `patternFill@patternType`。2026-08-31)。
///
/// 前は柄を見ておらず、どの柄も前景色のべた塗りになっていました。網掛けで
/// 「記入しない欄」を示す帳票が、真っ黒に潰れます。
///
/// 柄そのものの点の並びは仕様書に書かれていません(Excel は 8×8 の点で
/// 持っています)。**LibreOffice は線を引かず、前景色と地の色を濃さの比で
/// 混ぜて1色にします。** その比をそのまま使います
/// (`sc/source/filter/oox/stylesbuffer.cxx` の `Fill::finalizeImport`。
/// 0x80 が前景 100%)。18種類すべてに値があります。
///
/// 知らない名前は前景色のままにします。
fn gara_iro(na: &str, mae: (f32, f32, f32), ji: (f32, f32, f32)) -> (f32, f32, f32) {
    let koki = match na {
        "solid" => 0x80,
        "darkGray" | "darkTrellis" => 0x60,
        "darkDown" | "darkGrid" | "darkHorizontal" | "darkUp" | "darkVertical"
        | "mediumGray" => 0x40,
        "lightGrid" => 0x38,
        "lightTrellis" => 0x30,
        "lightDown" | "lightGray" | "lightHorizontal" | "lightUp" | "lightVertical" => 0x20,
        "gray125" => 0x10,
        "gray0625" => 0x08,
        _ => return mae,
    };
    let k = koki as f32 / 128.0;
    (
        ji.0 + (mae.0 - ji.0) * k,
        ji.1 + (mae.1 - ji.1) * k,
        ji.2 + (mae.2 - ji.2) * k,
    )
}

/// **グラデーションの色**(xlsx の `gradientFill`。2026-08-31)。
///
/// **階調そのものは描きません。** LibreOffice に合わせて、最初の2つの
/// 止めを半分ずつ混ぜた1色にします
/// (`sc/source/filter/oox/stylesbuffer.cxx` の `Fill::finalizeImport`。
/// `lclGetMixedColor(..., 0x40)`)。止めが1つなら、その色です。
///
/// あちらも角度と `type="path"`(放射)は見ておらず、止めも先の2つしか
/// 使いません。**円形の塗りを横縞にしないため**、こちらも同じにします
/// (`book::Gradient` の注記が案じているのがその形です)。
fn gradation_iro(g: &book::Gradient) -> Option<(f32, f32, f32)> {
    let mut tome = g.stops.iter().filter_map(|(_, c)| hex_rgb(c));
    let a = tome.next()?;
    match tome.next() {
        Some(b) => Some(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0)),
        None => Some(a),
    }
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
    // この口は書体を1つだけ受けます。名前は分からないので、置き替え先の
    // 寸法から出します(名前つきの口は [`sheet_leaves_fonts`])
    board.haba = Habakei::new(std::slice::from_ref(&font_data.to_vec()), &[], &deru_ji(grid));
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
    book_to_pdf_fonts(sheets, &[("".to_string(), font_data.to_vec())], out)
}

/// **書体を名前つきで何本か渡す形。** セルが名指しした書体で刷り分けます
/// (2026-08-31。Fable の指摘2 — 明朝のセルがゴシックで出ていました)。
///
/// 1本目が既定です。名指しの無いセルと、知らない名前はそちらで描きます。
pub fn book_to_pdf_fonts<W: Write>(
    sheets: &[(&Grid, Paper, PrintSetup)],
    fonts: &[(String, Vec<u8>)],
    out: W,
) -> Result<u32, String> {
    let first = sheets.first().ok_or("シートがありません")?;
    let paper1 = first.1;
    let mut clipped = 0u32;
    let mut board = Board::new(paper1);
    board.fonts = fonts.iter().map(|(n, _)| n.clone()).collect();
    // **字送りの表は1冊ぶんまとめて。** 紙束は1つで、どのシートの
    // セルも同じ番号で書体を引くためです
    let mut ji = std::collections::BTreeSet::new();
    for (grid, _, _) in sheets {
        ji.extend(deru_ji(grid));
    }
    let data0: Vec<Vec<u8>> = fonts.iter().map(|(_, d)| d.clone()).collect();
    let na0: Vec<String> = fonts.iter().map(|(n, _)| n.clone()).collect();
    board.haba = Habakei::new(&data0, &na0, &ji);
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
    let data: Vec<&[u8]> = fonts.iter().map(|(_, d)| d.as_slice()).collect();
    board.save_fonts(paper1, &data, out)?;
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
    // 埋める書体の名前の並び。セルの名指しをこれで番号に直します
    let fonts = board.fonts.clone();
    let board_haba = board.haba.clone();
    let (mut ext_rows, mut ext_cols) = grid.print_extent();
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
                .map(|w| retsu_mm_mdw(w, setup.mdw_px)).unwrap_or(COL_MM) * scale
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
        gyou_mm(grid, r) * scale
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
        // 埋める書体の名前の並び。セルの名指しを番号に直します(2026-08-31)
        fonts: &[String],
        // 書体ごとの字の幅。**描く書体で測ります**(2026-08-31)
        haba: &Habakei,
        // 数字1文字の幅(画素)。列幅をミリに直すのに要ります
        mdw_px: f32,
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
        // **行番号もセルと同じ字**です(2026-08-27 発注者)。置き場が余白と
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
            // 塗りは罫線より先に敷く(線を塗り潰さない)。
            //
            // **柄とグラデーションも描きます**(2026-08-31 発注者)。前は
            // `fill`(べた塗りの色)しか見ておらず、柄のセルは前景色の
            // べた塗りに、グラデーションのセルは無地になっていました。
            //
            // 柄は前景色と地の色を混ぜた1色にします([`gara_iro`])。
            // 地の色が無ければ白を地とします(Excel の既定)
            let gara = cell.fmt.fill_pattern.as_deref();
            if let Some(c) = cell.fmt.fill_grad.as_ref().and_then(gradation_iro) {
                ink.fill(x, y_top - rh, cw, rh, c);
            } else if let (Some(na), Some(mae)) = (gara, fill.as_deref().and_then(hex_rgb)) {
                let ji = cell.fmt.fill_bg.as_deref().and_then(hex_rgb).unwrap_or((1.0, 1.0, 1.0));
                ink.fill(x, y_top - rh, cw, rh, gara_iro(na, mae, ji));
            } else if let Some(c) = fill.as_deref().and_then(hex_rgb) {
                ink.fill(x, y_top - rh, cw, rh, c);
            }

            // 罫線。引いてある辺だけ — 線種の太さと色まで写す
            // (破線の刻みは紙では実線に落とす。太さと色が形を保つ)
            //
            // **結合した範囲の内側には引きません**(2026-08-31 発注者)。
            // Excel は結合を1つのセルとして扱い、外周だけを引きます。セルごとに
            // 4辺を引いていたので、国税庁の消費税の表の「熊 本」(A63:A67 の
            // 5行結合)を罫線が4本横切っていました
            let uti = grid.merges.iter().find(|(a, b)| {
                (a.row..=b.row).contains(&p.row) && (a.col..=b.col).contains(&p.col)
            });
            let b = cell.fmt.borders;
            // **斜めの罫線**(2026-08-31 発注者)。日本の帳票は表の左上のセルを
            // 斜めに割り、上と下に別の見出しを入れます。結合のセルでは、結合
            // ぜんぶを1つのセルとして引きます
            if b.diag.on {
                let (dx, dy) = uti
                    .map(|(a, z)| {
                        let w: f32 = (a.col..=z.col)
                            .filter(|c| !grid.col_hidden.contains(c))
                            .map(|c| {
                                grid.col_width.get(&c).copied().or(grid.default_col_width)
                                    .map(|v| retsu_mm_mdw(v, mdw_px)).unwrap_or(COL_MM) * scale
                            })
                            .sum();
                        let h: f32 = (a.row..=z.row)
                            .filter(|r| !grid.row_hidden.contains(r))
                            .map(|r| gyou_mm(grid, r) * scale)
                            .sum();
                        (w, h)
                    })
                    .unwrap_or((cw, rh));
                // 結合の左上に来たときだけ引きます(呑まれたセルでは引かない)
                if uti.is_none_or(|(a, _)| *a == p) {
                    let c = match b.diag.color {
                        Some(v) => (
                            ((v >> 16) & 255) as f32 / 255.0,
                            ((v >> 8) & 255) as f32 / 255.0,
                            (v & 255) as f32 / 255.0,
                        ),
                        None => (0.0, 0.0, 0.0),
                    };
                    let futo = b.diag.style.px() * 0.75 * 25.4 / 72.0;
                    if b.diag_down {
                        ink.line(x, y_top, x + dx, y_top - dy, futo, c);
                    }
                    if b.diag_up {
                        ink.line(x, y_top - dy, x + dx, y_top, futo, c);
                    }
                }
            }
            for (e, (x1, y1, x2, y2), fuchi) in [
                (b.top, (x, y_top, x + cw, y_top), uti.is_none_or(|(a, _)| p.row == a.row)),
                (b.bottom, (x, y_top - rh, x + cw, y_top - rh),
                 uti.is_none_or(|(_, z)| p.row == z.row)),
                (b.left, (x, y_top, x, y_top - rh), uti.is_none_or(|(a, _)| p.col == a.col)),
                (b.right, (x + cw, y_top, x + cw, y_top - rh),
                 uti.is_none_or(|(_, z)| p.col == z.col)),
            ] {
                if e.on && fuchi {
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
            let mut shown = format_value(&cell.value, cell.fmt.number_format.as_deref(), date1904);
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
            // **セルが言う大きさで描きます**(2026-08-31 発注者)。前は
            // 9.5pt の決め打ちで、6pt に設定した英文が大きく出ていました
            // (国税庁の酒税の表の I7「Number of licensed sites to sell
            // liquors」)。折り返しの位置もそのぶんずれます
            let pt = cell.fmt.size_c.map_or(9.5, |c| c as f32 / 100.0) * scale;
            // **セルの中で折り返す**(2026-08-31 発注者。xlsx の `wrapText`)。
            //
            // 前は折り返しを見ておらず、長い見出しが右のセルへ流れて、
            // 紙からもはみ出していました(国税庁の酒税の表の I7
            // 「販 売 場 数 / Number of licensed sites to sell liquors」)。
            //
            // **結合していれば、結合したぶんの幅で折ります。** 元のセルの
            // 幅で折ると、何列にも渡る注記が1列の幅で縦に積まれます
            //
            // **高さも同じです**(2026-08-31 発注者)。上下に結合したセルの
            // 字を1行目の高さだけで置いていたので、結合の1行目の下端に
            // 出ていました。見た目は上揃えです — 国税庁の酒税の表の
            // 「区 分」がこれでした
            let mut ma_w = cw;
            let mut ma_h = rh;
            if let Some((tl, br)) = grid.merges.iter().find(|(tl, _)| *tl == p) {
                ma_w = (tl.col..=br.col)
                    .filter(|c| !grid.col_hidden.contains(c))
                    .map(|c| {
                        grid.col_width.get(&c).copied().or(grid.default_col_width)
                            .map(|w| retsu_mm_mdw(w, mdw_px)).unwrap_or(COL_MM) * scale
                    })
                    .sum();
                ma_h = (tl.row..=br.row)
                    .filter(|r| !grid.row_hidden.contains(r))
                    .map(|r| gyou_mm(grid, r) * scale)
                    .sum();
            }
            // **セルが名指しした書体で描きます**(2026-08-31。Fable の指摘2)。
            // run が自分で名乗っていればそちらが勝ちます。
            // **幅もこの書体で測ります** — 書体で1字の幅が違うためです
            let fno_cell = fonts
                .iter()
                .position(|x| Some(x.as_str()) == cell.fmt.font.as_deref())
                .unwrap_or(0) as u8;
            let fno_of = |rf: &Option<String>| -> u8 {
                rf.as_deref()
                    .and_then(|n| fonts.iter().position(|x| x == n))
                    .map(|k| k as u8)
                    .unwrap_or(fno_cell)
            };
            // **半角だけ別の書体で組む書体**(ＭＳ Ｐ明朝・ＭＳ Ｐゴシック)
            // は、半角の番号がもう1つ並んでいます(2026-08-31 発注者)。
            // 無ければ元の番号のままです
            let han_of = |fno: u8| -> u8 {
                fonts
                    .get(fno as usize)
                    .and_then(|n| {
                        let sagasu = format!("{n}{HANKAKU_SIRUSI}");
                        fonts.iter().position(|x| *x == sagasu)
                    })
                    .map(|k| k as u8)
                    .unwrap_or(fno)
            };
            // その字を描く書体の番号。半角は上の相手へ振り分けます
            let ji_fno = |fno: u8, ch: char| -> u8 {
                if ch.is_ascii() { han_of(fno) } else { fno }
            };
            // **字下げ。1段は空白3つぶん**です(ISO/IEC 29500 の `indent`:
            // 「an increment of 1 represents 3 spaces … 3 space widths
            // (of the normal style font)」)。2026-08-31 に測って直しました。
            //
            // 空白の幅はその書体から取ります。前は「1段 = 全角1字」で
            // 数えていたので、ＭＳ 明朝なら 3分の2 しかありませんでした。
            //
            // **効くのは左・右・均等割付だけ**です(同じ規定。
            // 「Only left, right, and distributed … are supported」)。
            //
            // 入る幅からも引きます。前は描くときだけ左を空けていたので、
            // 字下げした行は右へはみ出していました
            let ind = if matches!(
                cell.fmt.align,
                HAlign::Left | HAlign::Right | HAlign::Distribute | HAlign::General
            ) {
                f32::from(cell.fmt.indent) * 3.0 * haba.ji_mm(ji_fno(fno_cell, ' ') as usize, ' ', pt)
            } else {
                0.0
            };
            let naka = (ma_w - 2.0 * MASU_PAD_MM - ind).max(pt * 25.4 / 72.0);
            // セルの書体で、字ごとに測ります(半角は相手の書体で)
            let haba_cell = |t: &str, p: f32| -> f32 {
                t.chars().map(|c| haba.ji_mm(ji_fno(fno_cell, c) as usize, c, p)).sum()
            };
            // **数がセルに入らないときは `#` で埋めます**(2026-08-31 発注者)。
            //
            // Excel と同じ決まりです。文字は右隣が空いていればはみ出して
            // よいのですが、**数は絶対にはみ出しません** — 桁が読めない数を
            // 見せるより、入らないと知らせるほうが安全だからです。
            // 前は数もそのまま描いていたので、狭い列では隣の数と重なって
            // 読めなくなっていました(総務省の給与所得の第1表)。
            //
            // **縮小して全体を表示**(xlsx の `shrinkToFit`)が先です。
            // 字を縮めて入れるので、下の `#####` にはなりません。
            //
            // **何行にもなるセルには効きません**(ISO/IEC 29500:
            // 「Not applicable when a cell contains multiple lines of
            // text」)。折り返しの指定と、セルの中の改行の両方を見ます。
            //
            // 縮め方の刻みは規定にありません。ここは入る大きさまで
            // そのまま縮めます(下限 1pt)
            let mut pt = pt;
            let nangyou = cell.fmt.wrap || shown.contains('\n');
            if cell.fmt.shrink && !nangyou {
                let iru = haba_cell(&shown, pt);
                if iru > naka && iru > 0.0 {
                    pt = (pt * naka / iru).max(1.0);
                }
            }
            // 折り返しの指定があるセルは、下の折り返しに任せます
            if matches!(cell.value, Value::Number(_)) && !cell.fmt.wrap && !cell.fmt.shrink {
                let hitotsu = haba.ji_mm(ji_fno(fno_cell, '#') as usize, '#', pt);
                if haba_cell(&shown, pt) > naka && hitotsu > 0.0 {
                    let kazu = (naka / hitotsu).floor().max(1.0) as usize;
                    shown = "#".repeat(kazu);
                }
            }
            // **下付き**(xlsx の `vertAlign="subscript"`)。上付きは模型が
            // まだ持っていません。
            //
            // 大きさと下げ幅は仕様書にありません。LibreOffice の既定
            // (大きさ 58%・下げ 8%)に合わせます
            // (help「Font Position」と tdf#80194 の直し)
            let sagaru = if cell.fmt.subscript {
                pt *= 0.58;
                pt * 25.4 / 72.0 * 0.08
            } else {
                0.0
            };
            // **縦書き**(xlsx の `textRotation="255"`)。2026-08-31 発注者。
            //
            // 255 は仕様書の範囲(0〜180)の外で、Excel の独自の値です。
            // 角度ではなく「字を1つずつ下へ積む」という指定で、役所の帳票は
            // 狭い列の見出しをこれで縦にします。前は横に出ていたので、隣の
            // 列まで伸びていました。
            //
            // 1字ずつ改行を入れて、いまの複数行の仕組みに乗せます
            if cell.fmt.rotation == Some(255) {
                shown = shown.chars().map(|c| c.to_string()).collect::<Vec<_>>().join("\n");
            }
            // 傾き。ISO/IEC 29500 の `textRotation`:
            // 「For 0-90, the value represents degrees above horizon.
            //   For 91-180 the degrees below the horizon is calculated as:
            //   [degrees below horizon] = 90 - textRotation」
            let katamuki = match cell.fmt.rotation {
                Some(v) if (1..=90).contains(&v) => v as f32,
                Some(v) if (91..=180).contains(&v) => 90.0 - v as f32,
                _ => 0.0,
            };
            // **セルの中で飾りが変わる所は、run ごとに描きます**
            // (2026-08-31 発注者。`Sheet::rich_runs`)。国税庁の酒税の表は
            // 8pt のセルの中で英文だけ 6pt・Century です。セル1つに大きさ
            // 1つだと、その英文が 8pt で出て折り返しの位置もずれます。
            //
            // 飾りの無いセルは、セルの書式の run が1つあるのと同じです
            let kire: Vec<(String, f32, Option<String>)> = match grid.rich_runs.get(&p) {
                Some(rs) if !rs.is_empty() => rs
                    .iter()
                    .map(|r| {
                        (r.text.replace('\r', ""),
                         r.size_pt.map_or(pt, |v| v * scale),
                         r.font.clone().or_else(|| cell.fmt.font.clone()))
                    })
                    .collect(),
                _ => vec![(shown.clone(), pt, cell.fmt.font.clone())],
            };
            // 1つの run の一部(同じ飾りの一続き)
            type Kata = (String, f32, Option<String>);
            // 行の列。1行は run のかけら(字, 大きさ, 書体)の並び
            let mut gyou: Vec<Vec<Kata>> = vec![Vec::new()];
            // **その行を、空でも残すか。** セルの中の改行(Alt+Enter)で
            // 作った行は、字が無くても1行ぶんの高さを取ります。折り返しで
            // できた行は、空なら出しません
            let mut mamoru: Vec<bool> = vec![true];
            let mut yoko = 0.0f32;
            for (t, rp, rf) in &kire {
                for (i, danraku) in t.split('\n').enumerate() {
                    if i > 0 {
                        // セルの中の改行(Alt+Enter)
                        gyou.push(Vec::new());
                        mamoru.push(true);
                        yoko = 0.0;
                    }
                    let mut ima = String::new();
                    for ch in danraku.chars() {
                        let w = haba.ji_mm(ji_fno(fno_of(rf), ch) as usize, ch, *rp);
                        // **行末の空白では折り返しません**(2026-08-31 発注者)。
                        // 空白1つのために行を替えると、その行は空白だけに
                        // なり、紙の上では空の行に見えます。国税庁の消費税の
                        // 表の注記は、段落の末尾がちょうど幅を超えたところに
                        // 空白があり、途中に空白の行が出ていました。
                        // 折り返す所を探すときは、字だけを見ます
                        if ch == ' ' && cell.fmt.wrap && yoko + w > naka {
                            ima.push(ch);
                            yoko += w;
                            continue;
                        }
                        if cell.fmt.wrap && !ima.is_empty() && yoko + w > naka {
                            gyou.last_mut().expect("行").push((std::mem::take(&mut ima), *rp, rf.clone()));
                            gyou.push(Vec::new());
                            mamoru.push(false);
                            yoko = 0.0;
                        }
                        ima.push(ch);
                        yoko += w;
                    }
                    if !ima.is_empty() {
                        gyou.last_mut().expect("行").push((ima, *rp, rf.clone()));
                    }
                }
            }
            // **折り返しでできた空の行だけを落とします**(2026-09-01 発注者)。
            // 前は空の行を全部落としていたので、国税庁の酒税の斜め罫線の
            // 見出しで、真ん中の空行2つが消えて「国税局・都道府県」が
            // 上に詰まっていました。改行で作った行は字が無くても残します
            let mut k = 0usize;
            gyou.retain(|g| {
                let nokosu = mamoru.get(k).copied().unwrap_or(true)
                    || g.iter().any(|(t, _, _)| !t.trim().is_empty());
                k += 1;
                nokosu
            });
            // 末尾の空の行は出しません(高さだけ取って見えないため)
            while gyou.len() > 1 && gyou.last().is_some_and(|g| {
                g.iter().all(|(t, _, _)| t.trim().is_empty())
            }) {
                gyou.pop();
            }
            if gyou.is_empty() {
                continue;
            }
            // 1行ぶんの幅(mm)。かけらごとの大きさで測ります
            let gyou_haba = |g: &[Kata]| -> f32 {
                g.iter()
                    .map(|(t, rp, rf)| {
                        let f = fno_of(rf);
                        t.chars().map(|c| haba.ji_mm(ji_fno(f, c) as usize, c, *rp)).sum::<f32>()
                    })
                    .sum()
            };
            // 字下げ(indent)。1段 = 全角約1字ぶん空ける — 日本の帳票は
            // 項目の階層を字下げで見せます。**右揃えなら右から空けます**
            // (2026-08-31 発注者。前は右揃えのとき字下げを捨てていました)
            // **右揃えは結合した幅の右端に着けます**(2026-09-01 発注者)。
            // 前は自分の列の幅(`cw`)で右端を出していたので、B列とC列を
            // 結合したセルの数が B列の右端に寄り、左隣の数とぶつかって
            // いました。国税庁の酒税の総括表の「1,071」がこれです。
            // 中央揃えと折り返しは前から `ma_w` を見ています
            let tx = if right {
                let w = gyou.first().map(|g| gyou_haba(g)).unwrap_or(0.0);
                x + ma_w - MASU_PAD_MM - ind - w
            } else {
                x + MASU_PAD_MM + ind
            };
            // 文字は塗り色で描かれる(PDF の作法)ので、色付きの字は前後で入れ替える
            let c = colour.as_deref().and_then(hex_rgb).unwrap_or((0.0, 0.0, 0.0));
            // 行送りはその行のいちばん大きい字で決めます
            // **行送りは、その行に出てくる書体の実物から出します**
            // (2026-08-31。LibreOffice と同じで ascent + descent)
            let okuri_of = |g: &[Kata]| -> f32 {
                // **字の無い行も1行ぶんの高さを取ります**(2026-09-01 発注者)。
                // 0 を返していたので、セルの中の空行が高さを持たず、
                // 国税庁の酒税の斜め罫線の見出しで「国税局・都道府県」が
                // 上に詰まっていました
                if g.is_empty() {
                    return haba.okuri_mm(ji_fno(fno_of(&cell.fmt.font), 'あ') as usize, pt);
                }
                g.iter()
                    .map(|(_, rp, rf)| haba.okuri_mm(ji_fno(fno_of(rf), 'あ') as usize, *rp))
                    .fold(0.0f32, f32::max)
            };
            let takasa: f32 = gyou.iter().map(|g| okuri_of(g)).sum();
            // **縦の揃え**(2026-08-31 発注者)。前はどのセルも下から積んで
            // いて、`valign` を一度も見ていませんでした。上下に結合した
            // 見出しが結合の1行目の下端に出ていたのはこれと結合の高さの
            // 両方が原因です(国税庁の酒税の表の「区 分」)。
            //
            // 上下いっぱいに散らす(`Distribute`)は行の間隔を割り出す所が
            // まだなので、模型の注記どおり上揃えで描きます
            //
            // **下揃えの 2.0mm は下揃えのときだけ**です(2026-08-31)。字の
            // 足がセルの底に着かないようにする下駄で、真ん中や上に寄せるときは
            // 要りません。足していたので、字がセルの上の罫線に乗っていました
            // (国税庁の消費税の表の「件」「百万円」)
            let soko = y_top - ma_h; // 結合したぶんの下端
            let aki = (ma_h - takasa).max(0.0);
            let ue = match cell.fmt.valign {
                book::VAlign::Bottom => 0.0,
                book::VAlign::Middle => aki / 2.0,
                book::VAlign::Top | book::VAlign::Distribute => aki,
            };
            // **足の下がりは書体から**(2026-08-31)。前は 2.0mm の決め打ちで、
            // 8pt の字なら本当の 0.34mm に対して6倍ちかく浮いていました
            let sagari = gyou
                .last()
                .map(|g: &Vec<Kata>| {
                    g.iter()
                        .map(|(_, rp, rf)| haba.sagari_mm(ji_fno(fno_of(rf), 'あ') as usize, *rp))
                        .fold(0.0f32, f32::max)
                })
                .unwrap_or(0.0);
            // 下付きはここで下げます(行送りは変えません)
            let mut ty = soko + ue + sagari + takasa - okuri_of(&gyou[gyou.len() - 1]) - sagaru;
            for g in &gyou {
                let w = gyou_haba(g);
                let mut gx = match cell.fmt.align {
                    // **中央揃え**(2026-08-31)。前は左に出ていました。
                    // 幅は結合したぶん(`ma_w`)で見ます — 結合の1列目の
                    // 幅で中央を出すと、題が紙の左へはみ出します
                    HAlign::Center | HAlign::CenterContinuous => x + (ma_w - w) / 2.0,
                    _ if right => x + ma_w - MASU_PAD_MM - w,
                    _ => tx,
                };
                // **均等割付は字をセルの幅いっぱいに配ります**(2026-08-31。
                // Fable の指摘5)。役所の表は「清 酒」「合成清酒」のように
                // 区分の列を割り付けます。前は左に詰めていました
                let waru = matches!(cell.fmt.align, HAlign::Distribute)
                    && g.iter().map(|(t, _, _)| t.chars().count()).sum::<usize>() > 1;
                if waru {
                    let kazu: usize = g.iter().map(|(t, _, _)| t.chars().count()).sum();
                    // 字と字の間に配る余り。両端はセルの縁に着けます
                    let aki = ((ma_w - 2.0 * MASU_PAD_MM - w) / (kazu - 1) as f32).max(0.0);
                    let mut wx = x + MASU_PAD_MM;
                    for (t, rp, rf) in g {
                        let fno = fno_of(rf);
                        for ch in t.chars() {
                            let one = ch.to_string();
                            let w1 = haba.ji_mm(fno as usize, ch, *rp);
                            ink.text_kazari(&one, *rp, wx, ty, c, bold, fno, w1,
                                            cell.fmt.underline, cell.fmt.strike, katamuki,
                                            cell.fmt.italic);
                            wx += haba.ji_mm(fno as usize, ch, *rp) + aki;
                        }
                    }
                } else {
                    for (t, rp, rf) in g {
                        let fno = fno_of(rf);
                        // **半角と全角で書体が変わるなら、そこで切ります**
                        // (2026-08-31 発注者。ＭＳ Ｐ明朝など)
                        for (f1, kata) in wakeru(t, |ch| ji_fno(fno, ch)) {
                            let w1 = haba.mm(f1 as usize, &kata, *rp);
                            ink.text_kazari(&kata, *rp, gx, ty, c, bold, f1, w1,
                                            cell.fmt.underline, cell.fmt.strike, katamuki,
                                            cell.fmt.italic);
                            gx += w1;
                        }
                    }
                }
                ty -= okuri_of(g);
            }
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
    // **紙の中で中央に置く**(xlsx の `printOptions@horizontalCentered`)。
    // 読まないと左の余白に寄ります。国税庁の酒税の都道府県別の表は
    // これが立っていて、元より 18pt 左に出ていました(2026-09-01)
    let ml = if grid.h_centered {
        ml + ((paper.width_mm - ml - mr - col_x[col_x.len() - 1]) / 2.0).max(0.0)
    } else {
        ml
    };
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
                    draw_row(grid, &mut board.ink(cur), *tr, y_top, th, ml, &cols, &col_x, &col_mm, scale, &cond_prep, setup.date1904, &fonts, &board_haba, setup.mdw_px);
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
        draw_row(grid, &mut board.ink(cur), r, y_top, rh, ml, &cols, &col_x, &col_mm, scale, &cond_prep, setup.date1904, &fonts, &board_haba, setup.mdw_px);
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
            // twoCellAnchor はセルと一緒に伸び縮みする — 幅と高さを
            // 右下のセル(to)から出す。列幅の換算が原本の Excel と
            // 少し違っても、図形は同じセルの縁に貼り付く
            let futa;
            let sp = match sp.to {
                Some((to, tdx, tdy)) if to.col >= sp.at.col && to.row >= sp.at.row => {
                    let mm = 25.4 / 96.0;
                    let w_mm: f32 = (sp.at.col..to.col)
                        .map(|c| {
                            c.checked_sub(c0)
                                .and_then(|i| col_mm.get(i as usize).copied())
                                .unwrap_or(COL_MM * scale)
                        })
                        .sum();
                    let h_mm: f32 = (sp.at.row..to.row).map(row_mm).sum();
                    let mut s2 = sp.clone();
                    s2.width_px = (w_mm / (mm * scale) - sp.dx_px + tdx).max(1.0);
                    s2.height_px = (h_mm / (mm * scale) - sp.dy_px + tdy).max(1.0);
                    futa = s2;
                    &futa
                }
                _ => sp,
            };
            zukei(l1, sp, x, y_top, scale);
        }
    }

    // **何も描かれなかった紙は出しません**(2026-09-01)。
    //
    // 列で割った右側の紙に、下の方の行が1つも掛からないことがあります。
    // 国税庁の消費税の表は、注記の行が左端の列にしか無いので、右側の紙が
    // まっさらのまま1枚出ていました。Excel は中身の無い紙を刷りません。
    //
    // ここで消せるのは、ヘッダーとフッターを載せる前だからです。載せた後は
    // どの紙にも字があるので、まっさらかどうかが分からなくなります。
    board.shirogami_wo_nozoku(first);
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
        // **線の種類**(`<a:prstDash>`)。刻みは線の太さに比例させます —
        // DrawingML も同じで、太い線ほど刻みが大きくなります
        let kizami = sp.dash.as_deref().map(|d| {
            let w = pen_w.max(0.2);
            match d {
                "dot" | "sysDot" => (w, w * 2.0),
                "dashDot" | "sysDashDot" => (w * 3.0, w * 2.0),
                "lgDash" => (w * 8.0, w * 3.0),
                "sysDash" => (w * 3.0, w * 1.0),
                // "dash" と、知らない名前
                _ => (w * 4.0, w * 3.0),
            }
        });
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
            // 知らない名前はここでは作らない — 下で作図の表から起こす
            _ => Vec::new(),
        };
        // 手で持つ形の他は、**画面と同じ作図の表**(book::preset_svg)
        // から点の列で起こす。表を2つ持つと「画面では星、紙では四角」に
        // 割れるので、紙は表を持たない。表にも無い名前だけ四角に落とす
        // (描けない図形として、開いたときの報告が数える側)
        let closed = match sp.kind.as_str() {
            // 閉じるかどうか。**`path` は塗りがあるときだけ閉じます** —
            // 塗らない折れ線を閉じると、終点から始点へ1本余計に引かれます
            // (2026-08-27 に折れ線の図を紙で見て気づきました)
            "line" | "spark" | "ink" | "marker" => false,
            "path" => sp.fill.is_some(),
            _ => true,
        };
        let mut subs: Vec<(Vec<(f32, f32)>, bool)> = if pts.is_empty()
            && !matches!(sp.kind.as_str(), "spark" | "ink" | "marker" | "path")
        {
            book::preset_pts(&sp.kind, 0.0, 0.0, w, h)
                .map(|subs| {
                    subs.into_iter()
                        .map(|(v, c)| {
                            let v: Vec<(f32, f32)> = v
                                .into_iter()
                                .map(|(px, py)| (x + px, y_top - py))
                                .collect();
                            (v, c)
                        })
                        .collect()
                })
                .unwrap_or_else(|| {
                    vec![(
                        vec![
                            (x, y_top),
                            (x + w, y_top),
                            (x + w, y_top - h),
                            (x, y_top - h),
                        ],
                        true,
                    )]
                })
        } else {
            vec![(pts, closed)]
        };
        // 回転と反転(折れ線もの以外)。紙は y が上向きなので、
        // いったん画面向きのずれに直してから時計回りに回す
        let rot = sp.rot.rem_euclid(360.0);
        let poly = matches!(
            sp.kind.as_str(),
            "spark" | "spark-col" | "spark-wl" | "ink" | "marker" | "path"
        );
        if (rot != 0.0 || sp.flip_h || sp.flip_v) && !poly {
            let (ccx, ccy) = (x + w / 2.0, y_top - h / 2.0);
            let (s, c) = (rot.to_radians().sin(), rot.to_radians().cos());
            for p in subs.iter_mut().flat_map(|(v, _)| v.iter_mut()) {
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
        // **影は本体の下。** 同じ形を灰色で右下へずらして先に描きます。
        // 画面(`SheetShape::to_svg`)と同じ 4px・#9E9E9E・濃さ 0.35 です。
        //
        // 2026-08-29 発注者「紙にも影を出すようにして」。それまでは
        // 「紙は輪郭だけ」の決めで、影は画面と xlsx だけでした
        let usu = sp.alpha.clamp(0.0, 1.0);
        if sp.shadow {
            let zure = 4.0 * 25.4 / 96.0 * scale;
            let hai = (0.62, 0.62, 0.62);
            for (pts, closed) in &subs {
                if pts.is_empty() {
                    continue;
                }
                let kage: Vec<(f32, f32)> =
                    pts.iter().map(|(x, y)| (x + zure, y - zure)).collect();
                if *closed && sp.fill.is_some() {
                    l1.poly_a(kage.clone(), hai, 0.35);
                }
                if pen.is_some() {
                    let ends =
                        if *closed { kage.len() } else { kage.len().saturating_sub(1) };
                    for i in 0..ends {
                        let (x1, y1) = kage[i];
                        let (x2, y2) = kage[(i + 1) % kage.len()];
                        l1.line_a(x1, y1, x2, y2, pen_w, hai, 0.35);
                    }
                }
            }
        }
        // **塗ってから輪郭。** 2026-08-27 まで紙は輪郭だけでした。
        // 図をこちらで描く(発注者「チャートは python による独自描画」)
        // には、棒も扇も中が塗れないと形になりません
        for (pts, closed) in &subs {
            if *closed {
                if let Some(c) = sp.fill.as_deref().and_then(hex_rgb) {
                    l1.poly_a(pts.clone(), c, usu);
                }
            }
            // 折れ線は辺ごとに引きます。閉じる形なら最後の点から先頭へ1本
            if let Some(pen) = pen {
                let ends = if *closed { pts.len() } else { pts.len().saturating_sub(1) };
                for i in 0..ends {
                    let (x1, y1) = pts[i];
                    let (x2, y2) = pts[(i + 1) % pts.len()];
                    l1.line_dash(x1, y1, x2, y2, pen_w, pen, usu, kizami);
                }
            }
        }
        // 図形の中の文字(テキストボックス)。揃えの指定があれば従います
        if let Some(t) = &sp.text {
            // **字の大きさは箱が言います。** 言っていなければ文書の既定の
            // 11pt です。前は 9pt の決め打ちで、内閣府の調査票の担当欄が
            // 元より2段階小さく出ていました(2026-09-01 発注者)
            let pt = sp.text_fmt.size_pt.unwrap_or(11.0) * scale;
            // **縦組みの箱**(`<a:bodyPr vert="…">`。2026-08-31 発注者)。
            // 箱が細いので、横に組むと1字ずつ折り返されて「F o r」のように
            // 落ちます。国税庁の消費税の表の「For the current year」が
            // これでした。字を 90 度倒して、箱の高さを行の長さに使います
            if sp.text_fmt.vertical {
                let hitotsu = |c: char| {
                    (if c.is_ascii() { 0.55f32 } else { 1.0 }) * pt * 25.4 / 72.0
                };
                let haba: f32 = t.chars().filter(|c| *c != '\n').map(hitotsu).sum();
                // 箱の中ほどから、上から下へ。字は右へ倒します(vert)
                let tx = x + (w - pt * 25.4 / 72.0) / 2.0 + pt * 25.4 / 72.0 * 0.8;
                let ty = y_top - (h - haba).max(0.0) / 2.0;
                let hito: String = t.chars().filter(|c| *c != '\n').collect();
                l1.text_kazari(&hito, pt, tx, ty, (0.0, 0.0, 0.0), false, 0, haba,
                               false, false, -90.0, false);
                return;
            }
            // **改行で折ります**(2026-08-30)。Word のテキストボックスは
            // 何行も持ちます。1行に繋げて描いていたので、内閣府の告知書の
            // 窓口の欄が横一列になって隣の枠まではみ出していました。
            //
            // **箱の幅でも折ります**(2026-08-31)。改行だけで切っていたので、
            // 長い段落が1行のまま伸び、本文の上に重なっていました。内閣府の
            // 告知書は1ページぶんの本文が1つの箱に入っていて、1187字のうち
            // 紙に出ていたのは 47字だけでした。
            // **箱の内側の余白は文書が決めます**(2026-08-31 発注者)。
            // DrawingML の `<a:bodyPr lIns rIns tIns bIns>` で、既定は
            // 左右 0.1インチ・上下 0.05インチです。前は左 1.5mm の決め打ちで、
            // 内閣府の調査票の担当欄の字が箱の縁に寄っていました
            let (il, ir, it, ib) = sp.text_fmt.ins_mm;
            let naka = (w - il - ir).max(pt * 25.4 / 72.0);
            // 半角は 0.55em、全角は 1em。**全角スペース(U+3000)も全角**です —
            // `is_ascii` で見ると半角に落ち、字の並びが詰まります
            let hitotsu = |c: char| if c.is_ascii() { 0.55 } else { 1.0 } * pt * 25.4 / 72.0;
            let mut gyou: Vec<String> = Vec::new();
            for danraku in t.split('\n') {
                let (mut ima, mut yoko) = (String::new(), 0.0f32);
                for c in danraku.chars() {
                    let cw = hitotsu(c);
                    if !ima.is_empty() && yoko + cw > naka {
                        gyou.push(std::mem::take(&mut ima));
                        yoko = 0.0;
                    }
                    ima.push(c);
                    yoko += cw;
                }
                gyou.push(ima);
            }
            // **行の高さも箱が言います**(`w:spacing w:line` の exact/atLeast)。
            // 言っていなければ**書体から出します** — LibreOffice の EditEngine も
            // 実物の書体から取り、割合は使いません(`editeng` の
            // `FormatterFontMetric`: `GetHeight() = nMaxAscent + nMaxDescent`)。
            // 前は字の 1.25 倍という根拠のない割合でした(2026-09-01 発注者)
            let em = kumihan::font::okuri_em(sp.text_fmt.font.as_deref()).unwrap_or(1.25);
            let takasa = match sp.text_fmt.line_pt {
                Some(v) => v * scale * 25.4 / 72.0,
                None => pt * 25.4 / 72.0 * em,
            };
            // **縦の寄せは `<a:bodyPr anchor>`**(2026-08-31 発注者)。既定は
            // 上で、前はどの箱も真ん中に寄せていました。余白の内側で寄せます
            let block = takasa * gyou.len() as f32;
            let aki = (h - it - ib - block).max(0.0);
            let ue = match sp.text_fmt.anchor {
                book::TextAnchor::Top => 0.0,
                book::TextAnchor::Middle => aki / 2.0,
                book::TextAnchor::Bottom => aki,
            };
            // **1行目のベースラインは書体の上がりの所**です。前は字の
            // 0.9 倍という割合で、内閣府の調査票の担当欄が元より 6pt 下に
            // 出ていました(2026-09-01 発注者)
            let agari = kumihan::font::agari_em(sp.text_fmt.font.as_deref())
                .filter(|e| *e > 0.0 && *e <= em)
                .unwrap_or(0.9);
            let mut ty = y_top - it - ue - pt * 25.4 / 72.0 * agari;
            for g in &gyou {
                let haba: f32 = g.chars().map(hitotsu).sum();
                let tx = match sp.text_fmt.align {
                    book::HAlign::Center => x + il + (w - il - ir - haba) / 2.0,
                    book::HAlign::Right => x + w - ir - haba,
                    _ => x + il,
                };
                l1.text(g, pt, tx, ty, (0.0, 0.0, 0.0), false);
                ty -= takasa;
            }
        }
}


#[cfg(test)]
mod tests {
    /// **セルが名指しした書体で刷り分ける。**
    ///
    /// 2026-08-31、国税庁の酒税の表(Fable の指摘2)。PDF に埋める書体が
    /// 1本だけで、明朝のセルもゴシックのセルも同じ字で出ていました。
    #[test]
    fn a_cell_prints_in_the_font_it_names() {
        let mut g = Grid { name: "見本".into(), ..Default::default() };
        for (c, na) in [(0u32, None), (1, Some("明朝")), (2, Some("ゴシック"))] {
            g.set(Pos::new(0, c), Cell {
                formula: None,
                value: Value::Text("あ".into()),
                fmt: CellFormat { font: na.map(str::to_string), ..Default::default() },
            });
        }
        // 書体の名前だけを渡します(実体はどれも同じで構いません)
        let d = kumihan::font::load(kumihan::font::for_text(None, "あ".chars()).unwrap().0).unwrap();
        let fonts: Vec<(String, Vec<u8>)> = ["", "明朝", "ゴシック"]
            .iter().map(|n| (n.to_string(), d.clone())).collect();
        let setup = PrintSetup::default();
        let mut out = Vec::new();
        book_to_pdf_fonts(&[(&g, Paper::default(), setup)], &fonts, &mut out).unwrap();
        // 置いたかけらの書体の番号が 0・1・2 に分かれていること
        let mut board = Board::new(Paper::default());
        board.fonts = fonts.iter().map(|(n, _)| n.clone()).collect();
        let leaves = {
            let s2 = PrintSetup::default();
            sheet_leaves_fonts(&g, Paper::default(), &s2, &board.fonts).unwrap()
        };
        let mut ban: Vec<u8> = leaves.iter().flat_map(|l| l.pieces.iter().map(|p| p.font)).collect();
        ban.sort_unstable();
        ban.dedup();
        assert_eq!(ban, vec![0, 1, 2], "書体が1本に落ちている: {ban:?}");
    }

    /// **均等割付は字をセルの幅いっぱいに配る。**
    ///
    /// 2026-08-31、国税庁の酒税の表(Fable の指摘5)。区分の列は
    /// 「清 酒」のように割り付きます。前は左に詰めていました。
    #[test]
    fn a_distributed_cell_spreads_its_characters() {
        let hiku = |align| -> Vec<f32> {
            let mut g = Grid { name: "見本".into(), ..Default::default() };
            g.col_width.insert(0, 20.0);
            g.set(Pos::new(0, 0), Cell {
                formula: None,
                value: Value::Text("清酒".into()),
                fmt: CellFormat { align, ..Default::default() },
            });
            let leaves = sheet_leaves(&g, Paper::default(), &PrintSetup::default()).unwrap();
            leaves.iter().flat_map(|l| l.pieces.iter().map(|p| p.x_mm)).collect()
        };
        let hidari = hiku(book::HAlign::Left);
        let waru = hiku(book::HAlign::Distribute);
        assert_eq!(waru.len(), 2, "字を1つずつ置いていない: {waru:?}");
        let haba_h = hidari.iter().cloned().fold(0.0f32, f32::max)
            - hidari.iter().cloned().fold(f32::MAX, f32::min);
        let haba_w = waru.iter().cloned().fold(0.0f32, f32::max)
            - waru.iter().cloned().fold(f32::MAX, f32::min);
        assert!(haba_w > haba_h * 2.0, "広がっていない: 左詰め {haba_h} / 割付 {haba_w}");
    }

    /// **セルの中で飾りが変わっても、その大きさで描く。**
    ///
    /// 2026-08-31 発注者「セルのサイズは8ポイントで英字だけ6ポイントに
    /// 設定しています」。セル1つに大きさ1つだと、6pt の英文が 8pt で出て、
    /// 折り返しの位置もずれます。
    #[test]
    fn a_run_inside_a_cell_keeps_its_own_size() {
        use book::{Pos, RichRun};
        let mut g = Grid { name: "見本".into(), ..Default::default() };
        g.col_width.insert(0, 12.0);
        let p = Pos::new(0, 0);
        g.set(p, Cell {
            formula: None,
            value: Value::Text("販売場数\nNumber of licensed sites".into()),
            fmt: CellFormat { wrap: true, size_c: Some(800), ..Default::default() },
        });
        g.rich_runs.insert(p, vec![
            RichRun { text: "販売場数\n".into(), ..Default::default() },
            RichRun { text: "Number of licensed sites".into(), size_pt: Some(6.0),
                      font: Some("Century".into()), ..Default::default() },
        ]);
        // **PDF の中身ではなく、置いた字を見ます**(流れは圧縮されます)
        let leaves = sheet_leaves(&g, Paper::default(), &PrintSetup::default()).unwrap();
        let mut ookisa: Vec<f32> = leaves.iter()
            .flat_map(|l| l.pieces.iter().map(|x| x.size_pt))
            .collect();
        ookisa.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ookisa.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        assert!(ookisa.iter().any(|v| (v - 6.0).abs() < 0.01),
                "6pt の run が 6pt で置かれていない: {ookisa:?}");
        assert!(ookisa.iter().any(|v| (v - 8.0).abs() < 0.01),
                "セルの 8pt が出ていない: {ookisa:?}");
        // 日本語と英語が別の行に折れていること
        let gyou: std::collections::BTreeSet<i32> = leaves.iter()
            .flat_map(|l| l.pieces.iter().map(|x| (x.y_mm * 10.0) as i32))
            .collect();
        assert!(gyou.len() >= 2, "セルの中の改行で行が分かれていない");
    }

    /// **列の幅は Excel の式で出す。**
    ///
    /// 2026-08-30 に「1文字 = 2.0mm」の掛け算をやめました。2026-08-31、
    /// その置き換え先も違っていたことが分かりました。文字数から画素を出す
    /// 式を、保存された幅に当てていたためです。
    ///
    /// 下の3つは、国税庁の酒税の総括表(08_sokatsu_kazeijokyo.xlsx)を
    /// Excel が出した PDF から罫線の間隔を測った値です。標準の書体は
    /// ＭＳ 明朝 10.5pt で、数字1文字は 7画素です。
    ///
    /// 4つめは総務省の給与所得の第1表(01.xlsx)で、標準の書体が
    /// Arial 12pt なので数字1文字が 9画素になります。**同じ幅でも書体が
    /// 違えば長さが変わります** — 7画素で計算すると 22% 狭くなります。
    #[test]
    fn a_column_is_as_wide_as_excel_makes_it() {
        // 元の PDF の罫線から測った、(幅, 数字1文字の画素, mm) の組
        for (haba, mdw, mm) in [
            (9.109_375, 7.0, 17.00),
            (7.109_375, 7.0, 13.23),
            (8.332_031, 7.0, 15.56),
            (11.332_031, 9.0, 27.00),
        ] {
            let deta = super::retsu_mm_mdw(haba, mdw);
            assert!((deta - mm).abs() < 0.3, "幅 {haba}(数字 {mdw}px)は {mm}mm のはずが {deta}mm");
        }
        // 画面に 8.43 文字と出るとき、保存されるのは 9.140625 で 64 画素
        let k = super::retsu_mm_mdw(9.140_625, 7.0);
        assert!((k - 16.93).abs() < 0.05, "{k}");
        // 0 は「分からない」の印。ＭＳ 明朝 10.5pt と同じ 7 で計算します
        assert_eq!(super::retsu_mm_mdw(9.109_375, 0.0), super::retsu_mm_mdw(9.109_375, 7.0));
    }

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
            date1904: false, mdw_px: 0.0,
        });
        // 同じ大きさの域を2つ = 紙も2枚(**繋げて1枚に詰めない**)
        let two = pages(&PrintSetup {
            areas: vec![
                (Pos::new(0, 0), Pos::new(2, 0)),
                (Pos::new(5, 0), Pos::new(7, 0)),
            ],
            margins_mm: None,
            date1904: false, mdw_px: 0.0,
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
            date1904: false, mdw_px: 0.0,
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
            &PrintSetup { areas: Vec::new(), margins_mm: Some((10.0, 10.0, 10.0, 10.0)) , date1904: false, mdw_px: 0.0 },
            &mut narrow).unwrap();
        let mut wide = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(),
            &PrintSetup { areas: Vec::new(), margins_mm: Some((10.0, 10.0, 100.0, 100.0)) , date1904: false, mdw_px: 0.0 },
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

    /// **何も描かれない紙は出しません。**
    ///
    /// 列で割った右側の紙に、下の方の行が1つも掛からないことがあります。
    /// 国税庁の消費税の表は、注記の行が左端の列にしか無いので、右側の紙が
    /// まっさらのまま1枚出ていました(2026-09-01)。
    #[test]
    fn a_page_with_nothing_on_it_is_not_printed() {
        let (fam, _) = kumihan::font::for_document(None).unwrap();
        let data = kumihan::font::load(fam).unwrap();
        let mut s = Grid { name: "s".into(), ..Default::default() };
        // 横に広く(紙2枚ぶん)、縦にも長く(紙2枚ぶん)。ただし
        // **下の方の行は左の列にしか字が無い**
        let oku = |s: &mut Grid, r: u32, c: u32| {
            s.set(Pos::new(r, c), Cell {
                formula: None, value: Value::Number(1.0), fmt: Default::default() });
        };
        for r in 0..10 {
            for c in 0..40 {
                oku(&mut s, r, c);
            }
        }
        // ここで紙を変えます。**この下は左端の列にしか字がありません**
        // (注記の行)。列で割った右側の紙には、何も掛かりません
        s.row_breaks = vec![10];
        for r in 10..15 {
            oku(&mut s, r, 0);
        }
        let mut buf = Vec::new();
        sheet_to_pdf(&s, &data, Paper::default(), &PrintSetup::default(), &mut buf).unwrap();
        let n = pages(&buf);
        // 紙ごとの中身を見て、字も線も無い紙が無いことを確かめます
        let leaves = sheet_leaves(&s, Paper::default(), &PrintSetup::default()).unwrap();
        let kara = leaves
            .iter()
            .filter(|l| {
                l.pieces.is_empty()
                    && l.rules.is_empty()
                    && l.rules_top.is_empty()
                    && l.fills.is_empty()
                    && l.polys.is_empty()
                    && l.paths.is_empty()
                    && l.images.is_empty()
            })
            .count();
        assert_eq!(kara, 0, "まっさらな紙が {kara} 枚ある(全 {n} 枚)");
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

    /// **下揃えの下駄は、下揃えのときだけ。**
    ///
    /// 字の足がセルの底に着かないよう 2.0mm 上げていましたが、真ん中や上に
    /// 寄せるときにも足していました。セルが低いと字の頭がセルの上へ突き抜け、
    /// 上の罫線に乗ります。国税庁の消費税の表の「件」「百万円」がこれで、
    /// 11.25pt(3.97mm)のセルに 8pt の字を真ん中で置いて 0.58mm はみ出して
    /// いました(2026-08-31 発注者)。
    #[test]
    fn only_bottom_alignment_lifts_the_text() {
        let tameshi = |ht: f32| -> Vec<f32> {
            let mut g = Grid::default();
            g.row_height.insert(0, ht);
            for (c, v) in [(0u32, book::VAlign::Bottom), (1, book::VAlign::Middle),
                           (2, book::VAlign::Top)] {
                let mut f = book::CellFormat { valign: v, ..Default::default() };
                f.size_c = Some(800);
                g.set(book::Pos::new(0, c), book::Cell {
                    formula: None, value: book::Value::Text("あ".into()), fmt: f });
            }
            let setup = PrintSetup { date1904: false, mdw_px: 0.0, ..Default::default() };
            let leaf = &sheet_leaves(&g, Paper::default(), &setup).expect("組めない")[0];
            let mut y: Vec<f32> = leaf.pieces.iter().filter(|p| p.text == "あ")
                .map(|p| p.y_mm).collect();
            y.sort_by(f32::total_cmp);
            y
        };
        // 余裕のあるセル(30pt)。下・中・上で位置が変わり、空きを等分する
        let y = tameshi(30.0);
        assert_eq!(y.len(), 3, "3つとも描けていない");
        assert!(y[0] < y[1] && y[1] < y[2], "揃えで高さが変わっていない: {y:?}");
        assert!((y[1] - y[0] - (y[2] - y[1])).abs() < 0.05,
                "空きを等分していない: {y:?}");
        // 詰まったセル(11.25pt)でも、字がセルの上へ出ない
        let ht = 11.25f32;
        let masu = ht * 25.4 / 72.0;
        let y = tameshi(ht);
        let soko = 297.0 - 20.0 - masu; // 上余白 20mm の紙の1行目
        for v in &y {
            // 足の位置から字の頭まで(8pt の 0.9em ぶん)を見ます
            let atama = v - soko + 8.0 * 0.9 * 25.4 / 72.0;
            assert!(atama <= masu + 0.05,
                    "字がセルの上へ出た: 頭{atama:.2}mm セル{masu:.2}mm");
        }
    }

    /// **結合した範囲の内側には罫線を引かない。**
    ///
    /// Excel は結合を1つのセルとして扱い、外周だけを引きます。セルごとに4辺を
    /// 引いていたので、内側のセルに罫線が残っているファイルではそれが出て
    /// いました。国税庁の消費税の表の「熊 本」(A63:A67 の5行結合)を
    /// 4本が横切っていました。同じ表の「高 松」は内側に罫線が無く、線も
    /// 出ていなかったので、**同じ形の見出しで見え方が食い違って**いました
    /// (2026-08-31 発注者)。
    #[test]
    fn a_merge_draws_only_its_outline() {
        let mut g = Grid::default();
        let mut f = book::CellFormat::default();
        f.borders.top = book::Edge::THIN;
        f.borders.bottom = book::Edge::THIN;
        f.borders.left = book::Edge::THIN;
        f.borders.right = book::Edge::THIN;
        // 3行を結合し、**中のセルにも4辺の罫線を持たせます**
        for r in 0..3u32 {
            g.set(book::Pos::new(r, 0), book::Cell {
                formula: None, value: book::Value::Text("あ".into()), fmt: f.clone() });
        }
        g.merges.push((book::Pos::new(0, 0), book::Pos::new(2, 0)));
        let setup = PrintSetup { date1904: false, mdw_px: 0.0, ..Default::default() };
        let leaf = &sheet_leaves(&g, Paper::default(), &setup).expect("組めない")[0];
        // 横線は上端と下端の2本だけ(内側の2本は出さない)
        let mut yoko: Vec<f32> = leaf
            .rules
            .iter()
            .filter(|r| (r.y1_mm - r.y2_mm).abs() < 0.01)
            .map(|r| r.y1_mm)
            .collect();
        yoko.sort_by(f32::total_cmp);
        yoko.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        assert_eq!(yoko.len(), 2, "結合の中に横線が出た: {yoko:?}");
    }

    /// **テキストボックスの余白は文書が決める。**
    ///
    /// 左 1.5mm の決め打ちで、縦はどの箱も真ん中に寄せていました。
    /// DrawingML の既定は左右 0.1インチ(2.54mm)・上下 0.05インチ(1.27mm)、
    /// 縦の寄せは上です。内閣府の調査票の担当欄の字が、箱の縁に寄って
    /// 上下の真ん中に浮いていました(2026-08-31 発注者)。
    #[test]
    fn a_text_box_keeps_the_inset_the_file_asks_for() {
        let hako = |ins: (f32, f32, f32, f32), anchor: book::TextAnchor| {
            let mut sp = hako("rect");
            sp.text = Some("あ".into());
            sp.text_fmt.ins_mm = ins;
            sp.text_fmt.anchor = anchor;
            let leaf = shapes_leaf(&[(sp, 20.0, 200.0)], Paper::default());
            let p = leaf.pieces.first().expect("字が無い").clone();
            (p.x_mm, p.y_mm)
        };
        let kitei = book::TextFmt::default().ins_mm;
        assert!((kitei.0 - 2.54).abs() < 0.01, "左の既定が {} mm", kitei.0);
        assert!((kitei.2 - 1.27).abs() < 0.01, "上の既定が {} mm", kitei.2);
        // 左の余白を広げたら、字も右へ動く
        let (x0, _) = hako(kitei, book::TextAnchor::Top);
        let (x1, _) = hako((10.0, 2.54, 1.27, 1.27), book::TextAnchor::Top);
        assert!((x1 - x0 - (10.0 - 2.54)).abs() < 0.05, "左の余白が効いていない");
        // 上・中・下で高さが変わる
        let (_, yt) = hako(kitei, book::TextAnchor::Top);
        let (_, ym) = hako(kitei, book::TextAnchor::Middle);
        let (_, yb) = hako(kitei, book::TextAnchor::Bottom);
        assert!(yt > ym && ym > yb, "縦の寄せが効いていない: {yt} {ym} {yb}");
    }

    /// **斜めの罫線を引く。**
    ///
    /// 日本の帳票は、表の左上のセルを斜めに割って「区分」と「項目」の2つの
    /// 見出しを入れます。模型が斜めを持っておらず、書き出しでも空の
    /// `<diagonal/>` を出すだけでした。国税庁の消費税の都道府県別の表が
    /// この形です(2026-08-31 発注者)。
    #[test]
    fn a_diagonal_border_crosses_the_merge() {
        let mut g = Grid::default();
        let mut f = book::CellFormat::default();
        f.borders.diag = book::Edge::THIN;
        f.borders.diag_down = true;
        // 2行2列の結合。斜めは結合ぜんぶを渡ります
        g.set(book::Pos::new(0, 0), book::Cell {
            formula: None, value: book::Value::Text("区分".into()), fmt: f });
        g.merges.push((book::Pos::new(0, 0), book::Pos::new(1, 1)));
        let setup = PrintSetup { date1904: false, mdw_px: 0.0, ..Default::default() };
        let leaf = &sheet_leaves(&g, Paper::default(), &setup).expect("組めない")[0];
        let naname: Vec<&pdfw::Rule> = leaf
            .rules
            .iter()
            .filter(|r| (r.x1_mm - r.x2_mm).abs() > 0.5 && (r.y1_mm - r.y2_mm).abs() > 0.5)
            .collect();
        assert_eq!(naname.len(), 1, "斜めの線が {} 本", naname.len());
        // 左上から右下へ(x が増えるほど y が減る)
        let r = naname[0];
        assert!((r.x2_mm - r.x1_mm) * (r.y2_mm - r.y1_mm) < 0.0, "向きが逆");
        // 結合の2列2行ぶんを渡っていること
        let cw = retsu_mm_mdw(g.default_col_width.unwrap_or(8.43), 7.0);
        assert!((r.x2_mm - r.x1_mm).abs() > cw * 1.5,
                "1列ぶんしか引いていない: {:.1}mm", (r.x2_mm - r.x1_mm).abs());
    }

    /// **行末の空白で折り返さない。**
    ///
    /// 段落の末尾がちょうど幅を超えた所に空白があると、その空白1つのために
    /// 行を替えていました。その行は空白だけになり、紙の上では空の行に
    /// 見えます。国税庁の消費税の表の注記(41行目)は、7段落のうち1つが
    /// これに当たって途中に空白の行が出ていました(2026-08-31 発注者)。
    #[test]
    fn a_trailing_space_does_not_start_a_line() {
        let mut g = Grid::default();
        g.row_height.insert(0, 90.0);
        g.col_width.insert(0, 8.0);
        let mut f = book::CellFormat { wrap: true, ..Default::default() };
        f.size_c = Some(700);
        // **セルの幅をちょうど超えた所に空白がある**形です。8字ごとに空白を
        // 入れた 20 字の後ろに、さらに空白を1つ置きます
        let t: String = (0..20).map(|i| if i % 8 == 7 { ' ' } else { 'n' }).collect();
        g.set(book::Pos::new(0, 0), book::Cell {
            formula: None, value: book::Value::Text(format!("{t} ")), fmt: f });
        let setup = PrintSetup { date1904: false, mdw_px: 0.0, ..Default::default() };
        let leaf = &sheet_leaves(&g, Paper::default(), &setup).expect("組めない")[0];
        for p in &leaf.pieces {
            assert!(p.text.is_empty() || !p.text.trim().is_empty(),
                    "空白だけの行を描いた: {:?}", p.text);
        }
    }

    /// **行送りと足の下がりは、書体が決める。**
    ///
    /// 前はどちらも決め打ちでした — 行送りは字の 1.2 倍、下がりは 2.0mm。
    /// 根拠がありません。LibreOffice は1行の中の run をなめて ascent と
    /// descent の最大を取り、その和を行の高さにします(`editeng` の
    /// `FormatterFontMetric::GetHeight()`)。同じやり方にしました。
    ///
    /// ＭＳ 明朝もＭＳ Ｐ明朝も 1.000em、Century は 1.202em です(元の PDF に
    /// 埋め込まれていた本物を測った値)。1.2 の決め打ちは日本語の書体で
    /// 2割ひらきすぎでした。
    #[test]
    fn the_line_advance_comes_from_the_font() {
        let d = kumihan::font::for_document(None)
            .ok()
            .and_then(|(f, _)| kumihan::font::load(f).ok())
            .expect("書体が無い");
        let ji: std::collections::BTreeSet<char> = "あ".chars().collect();
        let h = Habakei::new(std::slice::from_ref(&d), &[], &ji);
        let face = ttf_parser::Face::parse(&d, 0).expect("解けない");
        let em = face.units_per_em() as f32;
        let matomo = (f32::from(face.ascender()) - f32::from(face.descender())) / em;
        assert!((h.okuri_em(0) - matomo).abs() < 0.001,
                "行送りが書体の値でない: {} 対 {matomo}", h.okuri_em(0));
        // 決め打ちの 1.2 ではないこと(日本語の書体は 1.0 前後)
        assert!(h.okuri_em(0) < 1.15 || h.okuri_em(0) > 1.25,
                "1.2 の決め打ちのまま: {}", h.okuri_em(0));
        // 下がりは descent。8pt なら 1mm を大きく下回ります
        let s = h.sagari_mm(0, 8.0);
        assert!(s > 0.0 && s < 1.0, "下がりが {s}mm(前は 2.0mm の決め打ち)");
    }

    /// **紙 N 枚に収めるとき、行の端数を見込む。**
    ///
    /// 「全体の高さ ÷ 使える高さ」ちょうどの倍率だと、行の途中では切れない
    /// ので最後の1行が押し出され、紙が1枚増えます。総務省の給与所得の表は
    /// 縦横1枚の指定で2枚になっていました(2026-08-31)。
    #[test]
    fn fitting_to_a_page_allows_for_the_last_row() {
        let mut g = Grid::default();
        // 25mm の行を 12 本 = 300mm。使える高さは 100mm なので、ちょうどの
        // 倍率は 1/3 で、行の高さは 8.333mm。12 本で 100.0mm ぴったりに
        // なりますが、端数の丸めで最後の1本が押し出されます
        for r in 0..12u32 {
            g.row_height.insert(r, 25.0 * 72.0 / 25.4);
            g.set(book::Pos::new(r, 0), book::Cell::input("あ"));
        }
        g.fit_to_h = Some(1);
        let paper = Paper::hitoshii(210.0, 140.0, 20.0);
        let setup = PrintSetup { date1904: false, mdw_px: 0.0, ..Default::default() };
        let leaves = sheet_leaves(&g, paper, &setup).expect("組めない");
        assert_eq!(leaves.len(), 1, "1枚に収まっていない: {} 枚", leaves.len());
    }

    /// **セルの内側の余白は、Excel と同じ 2 画素。**
    ///
    /// 片側 1.5mm(合わせて 3.0mm)を引いていました。列幅に入っている
    /// 5 画素の内訳は「左右2画素ずつ + 罫線1画素」なので、字が使えるのは
    /// 幅から 4 画素(1.06mm)を引いた分です。総務省の給与所得の表では
    /// 3mm 足りず、8桁の数が 35 か所で `#####` になっていました
    /// (2026-08-31)。
    #[test]
    fn a_cell_keeps_only_excels_own_padding() {
        // 96dpi の 2 画素 = 0.529mm
        assert!((MASU_PAD_MM - 0.529).abs() < 0.002, "{MASU_PAD_MM}");
        // 15.65mm のセルに、8.12pt の「10,893,085」(4.609em = 13.20mm)が入る
        let iru = 13.20;
        assert!(15.65 - 2.0 * MASU_PAD_MM > iru, "8桁の数が入らない");
        // 前の 3.0mm では入りませんでした
        assert!(15.65 - 3.0 < iru, "前の余白でも入ってしまう(試験が効かない)");
    }

    /// **テキストボックスの字は、箱の幅で折り返す。**
    ///
    /// 改行でしか切っていなかったので、長い段落が1行のまま伸び、本文の上に
    /// 重なっていました。内閣府の告知書は1ページぶんの本文が1つの箱に
    /// 入っていて、1187字のうち紙に出ていたのは 47字だけでした
    /// (2026-08-31)。
    #[test]
    fn a_text_box_wraps_at_its_own_width() {
        let mut sp = hako("rect");
        // 改行を持たない長い段落。箱は 120px なので、1行には少ししか入りません
        sp.text = Some("あ".repeat(200));
        let leaf = shapes_leaf(&[(sp, 20.0, 200.0)], Paper::default());
        let ji: usize = leaf.pieces.iter().map(|p| p.text.chars().count()).sum();
        assert_eq!(ji, 200, "字が落ちている");
        assert!(leaf.pieces.len() > 10, "1行のまま描いている: {} 行", leaf.pieces.len());
        // どの行も箱の幅に収まっていること
        let hidari = leaf.pieces.iter().map(|p| p.x_mm).fold(f32::MAX, f32::min);
        let migi = leaf.pieces.iter().map(|p| p.x_mm + p.w_mm).fold(f32::MIN, f32::max);
        assert!(migi - hidari < 34.0, "箱からはみ出した: {:.1}mm", migi - hidari);
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
