//! 紙面を紙へ写す — 印刷と PDF 出力。
//!
//! **組版はやり直さない。** 画面に出しているのと同じ [`kumihan::Sheet`] を、
//! 座標そのままで PDF の面に置く。だから**画面と紙が必ず一致する**
//! (別々に組み直すと、そこで食い違いが生まれる)。
//!
//! engine 側に置かないのは、engine を PDF から独立させておくため。

pub mod grid;
/// 低い層で PDF を書く(使った字だけ埋める)。**まだ並べて動かす段**
pub mod pdfw;
/// WMF(Windows メタファイル)の図を、紙の道に直す係
pub mod wmf;
/// 紙面を絵にする(画面の下絵・回帰検査・PNG 書き出し)。
/// **`--features e` のときだけ**入ります — 絵にする裏(vello)は
/// 荷物が大きく、ファイルを触るだけの人には要らないためです
#[cfg(feature = "e")]
pub mod e;

use std::io::{BufWriter, Write};

use kumihan::{CharFormat, Sheet};
use printpdf::*;

/// ページ全体の飾り(色・透かし)。文書の設定から来る。
#[derive(Debug, Clone, Default)]
pub struct PageDress {
    /// ページの色(0.0〜1.0 の RGB)
    pub bg: Option<(f32, f32, f32)>,
    /// 透かし(斜めの薄い字)
    pub watermark: Option<String>,
    /// 手描きの線(ページ固定)。蛍光ペンは文字の下、ペンは上に描く
    pub ink: Vec<kumihan::Stroke>,
    /// **ページに貼り付く図形。** 形を作るのは表を刷るときと同じ
    /// [`grid`] の1本なので、文書と表で図形の形が食い違いません
    pub shapes: Vec<kumihan::DocShape>,
}

/// 紙の大きさ(mm)。既定は A4 縦。
#[derive(Debug, Clone, Copy)]
pub struct Paper {
    pub width_mm: f32,
    pub height_mm: f32,
    /// 左の余白。紙面の x はここからの相対
    pub margin_mm: f32,
    /// 上の余白。**2頁目からの本文の頭がここに来ます**
    pub top_mm: f32,
    /// 下の余白。ここまで来たら頁を折ります
    pub bottom_mm: f32,
}

impl Default for Paper {
    fn default() -> Self {
        Paper { width_mm: 210.0, height_mm: 297.0, margin_mm: 20.0, top_mm: 20.0, bottom_mm: 20.0 }
    }
}

impl Paper {
    /// 上下の余白が左と同じ紙。**試験と、余白を1つしか持たない呼び出し**に
    /// 使います。docx から来た文書は4つとも別なので、そちらは
    /// [`from_page`](Paper::from_page) を通します
    pub fn hitoshii(width_mm: f32, height_mm: f32, margin_mm: f32) -> Paper {
        Paper { width_mm, height_mm, margin_mm, top_mm: margin_mm, bottom_mm: margin_mm }
    }

    /// 紙の設定から。**上下と左右を別に持ちます**(2026-08-30)。
    ///
    /// 前は余白を1つしか持たず、頁割りが左の余白を上下にも使っていました。
    /// 内閣府の告知書(左右 25mm・上下 30mm)で、**2頁目から本文が
    /// 25mm の高さで始まり**、1頁に2行余分に入っていました。13頁の文書が
    /// 11頁になります。
    pub fn from_page(pg: &kumihan::PageSetup) -> Paper {
        Paper {
            width_mm: pg.w_mm,
            height_mm: pg.h_mm,
            margin_mm: pg.left_mm,
            top_mm: pg.top_mm,
            bottom_mm: pg.bottom_mm,
        }
    }
}

/// 紙面を PDF にする。
///
/// `font_data` は画面に使っているのと**同じフォントの実体**を渡すこと。
/// 別のものを渡すと字幅が変わり、画面と紙がずれる。
pub fn to_pdf<W: Write>(
    sheet: &Sheet,
    font_data: &[u8],
    paper: Paper,
    out: W,
) -> Result<(), String> {
    to_pdf_with(sheet, font_data, paper, &PageDress::default(), |_| Vec::new(), out)
}

/// 紙面を PDF にする(ページごとの飾りつき)。
///
/// `page_decor(k)` は k ページ目(1始まり)に置く行 — ヘッダー・フッター。
/// 行の y はページ上端からの mm、x は左余白からの mm
/// ([`kumihan::layout_hf`] がこの形で返す)。
/// ページ番号は組む側が字にして寄越すので、ここでは**置くだけ**。
/// `dress` はページの色と透かし。画面と同じものを紙にも出す。
pub fn to_pdf_with<W: Write, F: Fn(usize) -> Vec<kumihan::Line>>(
    sheet: &Sheet,
    font_data: &[u8],
    paper: Paper,
    dress: &PageDress,
    page_decor: F,
    out: W,
) -> Result<(), String> {
    // **1ページ目の紙は最初の節のもの。** 呼ぶ側が渡してくる `paper` は
    // 文書の用紙(= docx では**最後の節**)なので、途中で向きが変わる文書では
    // これをそのまま使うと1ページ目だけ紙が違う、という形で狂う
    let paper1 = sheet
        .setup_at(0.0)
        .map(|pg| Paper::from_page(&pg))
        .unwrap_or(paper);
    let (doc, page, layer) = PdfDocument::new(
        "office",
        Mm(paper1.width_mm),
        Mm(paper1.height_mm),
        "本文",
    );
    let font = doc
        .add_external_font(std::io::Cursor::new(font_data))
        .map_err(|e| e.to_string())?;
    let l = doc.get_page(page).get_layer(layer);
    // ページの色と透かし。文字より先に敷き、文字の色(fill)は黒へ戻す
    let ink_polyline = |l: &PdfLayerReference, st: &kumihan::Stroke| {
        if st.points.len() < 2 {
            return;
        }
        let pts: Vec<(Point, bool)> = st
            .points
            .iter()
            .map(|(x, y)| (Point::new(Mm(*x), Mm(paper.height_mm - *y)), false))
            .collect();
        let (w_pt, (r, g, b)) = if st.highlighter {
            (3.0 * 72.0 / 25.4, (1.0, 0.92, 0.45))
        } else {
            (0.45 * 72.0 / 25.4, (0.11, 0.23, 0.32))
        };
        l.set_outline_color(Color::Rgb(Rgb::new(r, g, b, None)));
        l.set_outline_thickness(w_pt);
        l.add_line(Line { points: pts, is_closed: false });
        l.set_outline_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
        l.set_outline_thickness(1.0);
    };
    let paint_bg = |l: &PdfLayerReference, page: usize| {
        if let Some((r, g, b)) = dress.bg {
            l.set_fill_color(Color::Rgb(Rgb::new(r, g, b, None)));
            l.add_rect(Rect::new(Mm(0.0), Mm(0.0), Mm(paper.width_mm), Mm(paper.height_mm)));
            l.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
        }
        if let Some(text) = dress.watermark.as_deref().filter(|t| !t.is_empty()) {
            // 紙の対角線に沿った薄い字。大きさは文字数から(紙に収まる程度)
            let n = text.chars().count().max(1) as f32;
            let pt = (520.0 / n).clamp(36.0, 120.0);
            let em_mm = pt * 25.4 / 72.0;
            let w_mm = em_mm * n; // 全角=1em の見積り
            let k = std::f32::consts::FRAC_1_SQRT_2; // cos45°
            let (cx, cy) = (paper.width_mm / 2.0, paper.height_mm / 2.0);
            let (x0, y0) = (cx - w_mm / 2.0 * k, cy - w_mm / 2.0 * k - em_mm * 0.35);
            l.set_fill_color(Color::Rgb(Rgb::new(0.85, 0.85, 0.85, None)));
            l.begin_text_section();
            l.set_font(&font, pt);
            l.set_text_matrix(TextMatrix::TranslateRotate(
                Mm(x0).into_pt(),
                Mm(y0).into_pt(),
                45.0,
            ));
            l.write_text(text, &font);
            l.end_text_section();
            l.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
        }
        // 蛍光ペンは文字より先(下)に敷く
        for st in dress.ink.iter().filter(|s| s.highlighter && s.page == page) {
            ink_polyline(l, st);
        }
    };
    paint_bg(&l, 0);
    // ページごとの描き先を控えておく(罫線を後から同じ頁割りで引くため)
    let mut layers: Vec<PdfLayerReference> = vec![l.clone()];

    // **改ページの計算は paginate に一本化。** 目次のページ番号も
    // 同じ関数から出るので、紙と番号が食い違わない
    let pg = paginate_full(sheet, paper);
    let Pagination { pages, offsets, papers, .. } = &pg;
    for (i, line) in sheet.lines.iter().enumerate() {
        if line.cells.is_empty() {
            continue;
        }
        let k = pages[i];
        while layers.len() < k {
            // **ページごとに紙の大きさが違いうる**(節が途中で変わる文書)。
            // 節が1つなら papers はどれも同じなので、今までと変わらない
            let pp = papers.get(layers.len()).copied().unwrap_or(paper);
            let (np, nl) = doc.add_page(
                Mm(pp.width_mm),
                Mm(pp.height_mm),
                format!("本文 {}", layers.len() + 1),
            );
            let nl = doc.get_page(np).get_layer(nl);
            paint_bg(&nl, layers.len());
            layers.push(nl);
        }
        let l = &layers[k - 1];
        // **裏返しはそのページの紙の高さで。** 節で紙が変わる文書では、
        // 文書の紙(= 最後の節)で裏返すと、向きの違うページで字が
        // 紙の外へ出る(縦 297 と横 210 なら 87mm ずれる)
        let pp = papers.get(k - 1).copied().unwrap_or(paper);
        let y_roll = line.y_mm - offsets[k - 1];
        // PDF の原点は左下。紙面の y は上からなので裏返す
        let y = pp.height_mm - y_roll;
        if sheet.vertical {
            // 縦書き: 1字ずつ、列の x(絶対 mm)に正立で置く。
            // 字の腰は「上からの距離 + だいたいの上がり」で合わせる
            let colx = sheet.vert_x.get(i).copied().unwrap_or(0.0);
            for c in &line.cells {
                let em = c.size_pt * 0.3528;
                let cy = pp.height_mm - (y_roll + c.x_mm + em * 0.85);
                let txt = c.ch.to_string();
                l.use_text(&txt, c.size_pt, Mm(colx), Mm(cy), &font);
                if c.fmt.bold {
                    l.use_text(&txt, c.size_pt, Mm(colx + 0.12), Mm(cy), &font);
                }
            }
            continue;
        }
        // **同じ書式の連なり**ごとに打つ(部分書式)。書体の実体は1つなので、
        // 大きさと飾りだけが変わる(太字は少しずらして二度打つ合成 —
        // 太字の実体を持っていないものを持っている顔をしない)
        let mut i = 0usize;
        while i < line.cells.len() {
            let c0 = &line.cells[i];
            let mut j = i + 1;
            while j < line.cells.len()
                && line.cells[j].fmt == c0.fmt
                && line.cells[j].size_pt == c0.size_pt
                // 均等割付などで字間が広がった行は、x が飛んだら切る
                && (line.cells[j].x_mm
                    - line.cells[j - 1].x_mm
                    - line.cells[j - 1].w_mm)
                    .abs()
                    < 0.05
            {
                j += 1;
            }
            let seg = &line.cells[i..j];
            let text: String = seg.iter().map(|c| c.ch).collect();
            let w: f32 = seg.iter().map(|c| c.w_mm).sum();
            let x = pp.margin_mm + c0.x_mm;
            l.use_text(&text, c0.size_pt, Mm(x), Mm(y), &font);
            if c0.fmt.bold {
                l.use_text(&text, c0.size_pt, Mm(x + 0.12), Mm(y), &font);
            }
            rule(l, &c0.fmt, x, y, w, c0.size_pt);
            i = j;
        }
    }

    // **頁割りは本文と同じ物を使う**([`page_at`])。以前ここは
    // 「どの頁も同じ高さ」と決め打ちした近似を**別に持っていた**ので、
    // 本文とずれることがあった(2026-08-10 に一本化)
    let page_of = |y: f32| -> usize { pg.page_at(y) };
    // 画像。行と同じ頁割りで置く
    {
        for (bytes, [x, top, w_mm, h_mm]) in &sheet.images {
            let k = page_of(*top);
            if k >= layers.len() {
                continue;
            }
            let off = offsets[k];
            let pp = papers.get(k).copied().unwrap_or(paper);
            // 復号できない画像は飛ばして続ける(1枚のために紙全体を失敗させない)
            let Ok(im) = ::image::load_from_memory(bytes) else { continue };
            // 透過つき(RGBA)は printpdf 0.7 が正しく埋め込めないので RGB に落とす
            let im = ::image::DynamicImage::ImageRgb8(im.to_rgb8());
            let xobj = printpdf::ImageXObject::from_dynamic_image(&im);
            let pdf_im = printpdf::Image::from(xobj);
            // 目標の mm に合わせて拡縮(printpdf は px と dpi から実寸を出す)
            let (pw, ph) = (im.width() as f32, im.height() as f32);
            let dpi = 300.0;
            let natural_w = pw / dpi * 25.4;
            let natural_h = ph / dpi * 25.4;
            pdf_im.add_to_layer(layers[k].clone(), printpdf::ImageTransform {
                translate_x: Some(Mm(pp.margin_mm + x)),
                translate_y: Some(Mm(pp.height_mm - (top - off) - h_mm)),
                scale_x: Some(w_mm / natural_w),
                scale_y: Some(h_mm / natural_h),
                dpi: Some(dpi),
                ..Default::default()
            });
        }
    }

    // 表の罫線。行と同じ頁割りで引く(頁をまたぐ縦線は窓で切る)
    {
        for r in &sheet.rules {
            let [x1, y1, x2, y2] = *r;
            let k = page_of(y1.min(y2));
            if k >= layers.len() {
                continue;
            }
            let off = offsets[k];
            let pp = papers.get(k).copied().unwrap_or(paper);
            let bottom = pp.height_mm - pp.margin_mm;
            let l = &layers[k];
            let (ry1, ry2) = (
                pp.height_mm - (y1 - off).clamp(pp.margin_mm, bottom),
                pp.height_mm - (y2 - off).clamp(pp.margin_mm, bottom),
            );
            l.add_line(Line {
                points: vec![
                    (Point::new(Mm(pp.margin_mm + x1), Mm(ry1)), false),
                    (Point::new(Mm(pp.margin_mm + x2), Mm(ry2)), false),
                ],
                is_closed: false,
            });
        }
    }
    // 脚注。**紙の下**に、仕切り線を挟んで置く。
    // どの頁に載るかは頁割りが決めている(脚注の高さぶん本文の底が上がる)
    for (k, l) in layers.iter().enumerate() {
        let idx = match pg.notes.get(k) {
            Some(v) if !v.is_empty() => v,
            _ => continue,
        };
        let pp = papers.get(k).copied().unwrap_or(paper);
        let total: f32 = idx.iter().map(|i| sheet.notes[*i].h_mm).sum();
        // 下余白のすぐ上に積む。PDF の原点は左下なので、ここは上へ数える
        let top = pp.margin_mm + total;
        // 仕切り線。紙幅いっぱいには引かない(Word の作法に近い三分の一)
        let sep_y = top + NOTE_GAP_MM * 0.5;
        l.add_line(Line {
            points: vec![
                (Point::new(Mm(pp.margin_mm), Mm(sep_y)), false),
                (Point::new(Mm(pp.margin_mm + (pp.width_mm - pp.margin_mm * 2.0) / 3.0),
                            Mm(sep_y)), false),
            ],
            is_closed: false,
        });
        let mut up = 0.0f32;
        for i in idx {
            let nb = &sheet.notes[*i];
            for nl in &nb.lines {
                // nl.y_mm は脚注の中の相対(上から下へ)。紙の上では下から数える
                let y = top - up - nl.y_mm;
                let mut a = 0usize;
                while a < nl.cells.len() {
                    let c0 = &nl.cells[a];
                    let mut b = a + 1;
                    while b < nl.cells.len()
                        && nl.cells[b].fmt == c0.fmt
                        && nl.cells[b].size_pt == c0.size_pt
                    {
                        b += 1;
                    }
                    let seg = &nl.cells[a..b];
                    let text: String = seg.iter().map(|c| c.ch).collect();
                    let x = pp.margin_mm + c0.x_mm;
                    l.use_text(&text, c0.size_pt, Mm(x), Mm(y), &font);
                    if c0.fmt.bold {
                        l.use_text(&text, c0.size_pt, Mm(x + 0.12), Mm(y), &font);
                    }
                    a = b;
                }
            }
            up += nb.h_mm;
        }
    }

    // ペン(手描きの線)は文字の上に描く
    for (k, l) in layers.iter().enumerate() {
        for st in dress.ink.iter().filter(|s| !s.highlighter && s.page == k) {
            ink_polyline(l, st);
        }
    }
    // ページごとの飾り(ヘッダー・フッター)。y はページ上端からの絶対位置
    for (k, l) in layers.iter().enumerate() {
        for line in page_decor(k + 1) {
            if line.cells.is_empty() {
                continue;
            }
            let text = line.text();
            let pt = line.cells[0].size_pt;
            let x = paper.margin_mm + line.cells[0].x_mm;
            let y = paper.height_mm - line.y_mm;
            l.use_text(&text, pt, Mm(x), Mm(y), &font);
            if line.cells[0].fmt.bold {
                l.use_text(&text, pt, Mm(x + 0.12), Mm(y), &font);
            }
        }
    }
    doc.save(&mut BufWriter::new(out)).map_err(|e| e.to_string())
}

/// 巻物(紙面)をページに折る。
/// 返り値: 各行が載るページ(1始まり。行の並びは `sheet.lines` の順)と、
/// ページごとの繰り上げ量(そのページの先頭が巻物のどの高さか)。
/// `to_pdf` もこれを使うので、**目次のページ番号と紙が必ず一致する**。
pub fn paginate(sheet: &Sheet, paper: Paper) -> (Vec<usize>, Vec<f32>) {
    let p = paginate_full(sheet, paper);
    (p.pages, p.offsets)
}

/// 頁割りの答え一式。[`paginate_full`] が返す。
#[derive(Debug, Clone, Default)]
pub struct Pagination {
    /// 行ごとの頁(**1始まり**。並びは `sheet.lines` の順)
    pub pages: Vec<usize>,
    /// 頁ごとの繰り上げ量(その頁の先頭が巻物のどの高さか)
    pub offsets: Vec<f32>,
    /// 頁ごとの紙。節で紙が変わる文書では頁ごとに違う
    pub papers: Vec<Paper>,
    /// 頁ごとに載る脚注(`sheet.notes` の添字)。**脚注は紙の下を占める**ので、
    /// その高さぶん本文の底が上がる — 頁割りと切り離せない
    pub notes: Vec<Vec<usize>>,
    /// **その頁の頭で繰り返す表の見出しの高さ(mm)。** 0 なら繰り返さない。
    ///
    /// 表が頁をまたぐとき、次の頁の頭に見出しの行を写します。その高さぶん
    /// 本文の頭が下がるので、**頁割りが数えないと重なります**
    /// (2026-08-27 に実物で重ねてしまった)。脚注が本文の底を上げるのと
    /// 同じ形です。
    pub header_h: Vec<f32>,
    /// 頁ごとの**切れ目**(その頁に載る最初の行の巻物 y)。
    /// `offsets` は余白を引いた後の値で**前の頁の裾と重なる**ので、
    /// 「この y はどの頁か」を引くのにそのまま使うと1頁ずれる。
    /// 引くときは必ずこちら([`Pagination::page_at`])
    pub starts: Vec<f32>,
}

impl Pagination {
    /// 巻物の高さ `y` が載る頁(**0始まり** — `layers` の添字と同じ)。
    /// **画像も罫線もこれを使う** — 本文と同じ表を引くので必ず一致する。
    pub fn page_at(&self, y: f32) -> usize {
        let mut k = 0usize;
        for (j, st) in self.starts.iter().enumerate() {
            if y >= *st - 0.01 {
                k = j;
            } else {
                break;
            }
        }
        k
    }
}

/// [`paginate`] と同じ折り方をして、**頁ごとの紙と切れ目**も返す。
///
/// 節が途中で変わる文書は、**ページごとに紙の大きさが違う**(縦の節のあとに
/// 横の節、など)。`sheet.sect_pages` が空(節が1つ)なら、どのページも
/// 渡された `paper` のままなので、今までと1ミリも変わらない。
pub fn paginate_full(sheet: &Sheet, paper: Paper) -> Pagination {
    // その高さに効いている紙。節が無ければ呼ぶ側の紙をそのまま使う
    let paper_at = |y: f32| -> Paper {
        match sheet.setup_at(y) {
            Some(pg) => Paper::from_page(&pg),
            None => paper,
        }
    };
    // **繰り返す見出しの高さ。** その表の見出しの行が占める高さです
    let head_h = |t: usize| -> f32 {
        let ys: Vec<f32> = sheet
            .lines
            .iter()
            .filter(|l| matches!(l.cell, Some((tn, 0, _)) if tn == t))
            .map(|l| l.y_mm)
            .collect();
        match (ys.iter().cloned().fold(f32::MAX, f32::min), ys.iter().cloned().fold(0.0, f32::max)) {
            (lo, hi) if lo <= hi => hi - lo + LINE_GUESS_MM,
            _ => 0.0,
        }
    };
    // **表の1行は、セルごとに別の紙へ割りません**(2026-09-01 発注者
    // 「罫線が前ページ、内容がこちらのページと別れてしまっている」)。
    //
    // 組む所はセルごとに行を並べるので、`sheet.lines` の並びはセル1の全行 →
    // セル2の全行です。1行ずつ紙に振ると、同じ表の行でもセル1が前の紙・セル2が
    // 次の紙になります。罫線は表の行の位置で引くので、字だけが次の紙へ
    // 動き、上の余白にも出ていました。
    //
    // その表の行に属する行の**いちばん下**で判断すれば、行の頭で紙が
    // 変わり、セルが揃って動きます。紙1枚に収まらない行だけは今までどおり
    // (途中で割らないと永遠に入らないため)
    let waku: std::collections::HashMap<(usize, usize), (f32, f32)> = {
        let mut m: std::collections::HashMap<(usize, usize), (f32, f32)> = Default::default();
        for l in &sheet.lines {
            if let Some((t, ri, _)) = l.cell {
                let e = m.entry((t, ri)).or_insert((l.y_mm, l.y_mm));
                e.0 = e.0.min(l.y_mm);
                e.1 = e.1.max(l.y_mm);
            }
        }
        m
    };
    let mut pages = Vec::with_capacity(sheet.lines.len());
    let mut offsets = vec![0.0f32];
    let mut header_h = vec![0.0f32];
    let mut papers = vec![paper_at(0.0)];
    // 頁ごとの脚注と、その高さ。**脚注が増えるとその頁の本文の底が上がる**
    let mut notes: Vec<Vec<usize>> = vec![Vec::new()];
    let mut note_h = 0.0f32;
    // 頁の切れ目 = その頁に載る最初の行の y。1頁目は巻物の頭から
    let mut starts = vec![f32::NEG_INFINITY];
    // 明示の改ページ(文書側の指定)。高さ超過とは別に、ここでも頁を割る
    let mut breaks = sheet.breaks.iter().copied().peekable();
    for line in &sheet.lines {
        if line.cells.is_empty() {
            // 空行は頁を進めない(描かれないので)。いまの頁に属するとみなす
            pages.push(offsets.len());
            continue;
        }
        // **改ページは1行につき1つだけ引き取ります**(2026-09-01 発注者
        // 「告知書がおかしいのは7ページ。重複している」)。
        //
        // 前はその行までに溜まった改ページを全部まとめて1回の紙送りに
        // していました。改ページが2つ続く文書(空の段落で1つ、次の段落で
        // もう1つ)では2枚ぶんが1枚に潰れ、内閣府の告知書は法令の抄が
        // 前の紙の字に重なって出ていました。Word は続けて2枚送ります
        // その行までに溜まった改ページの数。**数のぶんだけ紙を送ります**
        let mut kaisu = 0usize;
        while let Some(&b) = breaks.peek() {
            if line.y_mm >= b - 0.01 {
                breaks.next();
                kaisu += 1;
            } else {
                break;
            }
        }
        let forced = kaisu > 0;
        // この行に付いている脚注(まだこの頁に数えていないもの)
        let mine: Vec<usize> = sheet.notes.iter().enumerate()
            .filter(|(i, n)| (n.at_y - line.y_mm).abs() < 0.01
                && !notes.last().unwrap().contains(i))
            .map(|(i, _)| i)
            .collect();
        let add: f32 = mine.iter().map(|i| sheet.notes[*i].h_mm).sum();

        // **いま居るページの紙**で高さを測る。縦の節と横の節では
        // 1ページに入る行数がそもそも違う
        let cur = *papers.last().unwrap();
        let y_roll = line.y_mm - offsets.last().unwrap();
        // 脚注のぶん、本文に使える底が上がる(仕切りの隙間も見る)
        let reserve = if note_h + add > 0.0 { note_h + add + NOTE_GAP_MM } else { 0.0 };
        // **いまの頁で繰り返している見出し**のぶんも底が上がります
        let hh = *header_h.last().unwrap();
        // **行の箱ごと入る分しか置きません。** ベースラインだけで見ると、
        // 字の足(箱の下 2.4mm)が余白へはみ出します(2026-08-29 に測りました)
        let asi = kumihan::LINE_MM - kumihan::BASE_UP_MM;
        // **下の余白は下の余白で見ます**(2026-08-30)。前は左の余白を
        // 上下にも使っていました
        let soko = cur.height_mm - cur.bottom_mm - reserve - hh - asi;
        // 表の行は、その行のいちばん下で判断します。紙1枚に収まらない行は
        // 自分の位置で判断します(そうしないと入る所が無くなります)
        // 表の行の上端と下端。紙1枚に収まらない行は、今までどおり自分の
        // 位置で見ます(そうしないと入る所が無くなります)
        let hako = line
            .cell
            .and_then(|(t, ri, _)| waku.get(&(t, ri)))
            .copied()
            .filter(|(a, b)| b - a < soko - cur.top_mm);
        let mite = match hako {
            Some((_, b)) => b - offsets.last().unwrap(),
            None => y_roll,
        };
        if forced || mite > soko {
            // 次のページへ。行の紙面上の高さは(余白ぶんを除いて)そのまま続ける
            let next = paper_at(line.y_mm);
            // **見出しを繰り返す表の途中なら、その高さぶん頭を下げます。**
            // 数えないと、繰り返した見出しが次の行と重なります
            let repeat = match line.cell {
                Some((t, ri, _)) if ri > 0 && sheet.header_tables.contains(&t) => head_h(t),
                _ => 0.0,
            };
            // **次の頁の頭は上の余白**です(同上)。左の余白を使っていたので、
            // 上下と左右が違う文書では2頁目から本文の頭がずれていました
            // 1頁目の頭は `doc_to_sheet` が `top_mm + BASE_UP_MM` に置きます。
            // **続きの頁も同じ高さに揃えます** — 足さないと、2頁目からだけ
            // 1行の腰のぶん(4mm)高く始まります
            // **表の行なら、その行の上端を紙の頭に合わせます**(2026-09-01)。
            //
            // `sheet.lines` はセルごとに並ぶので、y の順ではありません。頁を
            // 変えた行を基準にすると、同じ行でもそれより上にあるセルの行が
            // 上の余白へ出ます。内閣府の調査票の3枚目は、見出しのセルが
            // 余白の外に 28pt 出ていました
            let atama = hako.map(|(a, _)| a).unwrap_or(line.y_mm);
            // **改ページが続いた分は、白い紙を挟みます。** まとめて1回に
            // すると2枚ぶんが1枚に潰れます
            let tsukaeru = (next.height_mm - next.top_mm - next.bottom_mm).max(1.0);
            for k in 1..kaisu.max(1) {
                let zure = tsukaeru * (kaisu - k) as f32;
                offsets.push(atama - next.top_mm - kumihan::BASE_UP_MM - zure);
                header_h.push(0.0);
                papers.push(next);
                starts.push(atama - zure);
                notes.push(Vec::new());
            }
            offsets.push(atama - next.top_mm - kumihan::BASE_UP_MM - repeat);
            header_h.push(repeat);
            papers.push(next);
            starts.push(line.y_mm);
            // 行が次の頁へ動けば、**その行に付いた脚注も一緒に動く**
            notes.push(mine.clone());
            note_h = add;
        } else {
            notes.last_mut().unwrap().extend(mine.iter().copied());
            note_h += add;
        }
        pages.push(offsets.len());
    }
    Pagination { pages, offsets, papers, starts, notes, header_h }
}

/// 見出しの行1行ぶんの見当(mm)。**行の高さは紙面が持っていない**ので、
/// 繰り返す高さを測るときの下駄にします
const LINE_GUESS_MM: f32 = 7.0;

/// 本文と脚注の間の隙間(mm)。仕切り線もこの中に引く
pub const NOTE_GAP_MM: f32 = 3.0;

/// 下線と取り消し線。フォントが持っていないので線として引く。
fn rule(l: &PdfLayerReference, f: &CharFormat, x: f32, y: f32, w: f32, pt: f32) {
    let em = pt * 25.4 / 72.0;
    for (on, dy) in [(f.underline, -em * 0.18), (f.strike, em * 0.28)] {
        if !on {
            continue;
        }
        l.add_line(Line {
            points: vec![
                (Point::new(Mm(x), Mm(y + dy)), false),
                (Point::new(Mm(x + w), Mm(y + dy)), false),
            ],
            is_closed: false,
        });
    }
}

#[cfg(test)]
mod tests {

    /// **紙に入らない図形は次の紙へ送る。**
    ///
    /// 錨の段落が紙の下の方にあると、そこからのずれを足した位置が紙の外に
    /// 出ます。前はそのまま置いていたので、内閣府の調査票の窓口の欄2つが
    /// 305mm(A4 は 297mm)に来て、紙に出ていませんでした(2026-08-31)。
    #[test]
    fn a_shape_that_does_not_fit_moves_to_the_next_page() {
        // 錨の段落を紙の下の方に置くため、本文を長くします
        let mut d = kumihan::Document::default();
        for _ in 0..40 {
            d.push_para(kumihan::Paragraph {
                runs: vec![kumihan::Run {
                    text: "本文です。".into(),
                    size_pt: None,
                    font: None,
                    fmt: Default::default(),
                }],
                ..Default::default()
            });
        }
        // 最後の段落に、段落から 60mm 下という錨を付けます
        let a = r#"<w:drawing><wp:anchor><wp:positionH relativeFrom="column"><wp:posOffset>0</wp:posOffset></wp:positionH>
<wp:positionV relativeFrom="paragraph"><wp:posOffset>2160000</wp:posOffset></wp:positionV>
<wp:extent cx="1800000" cy="720000"/><a:graphic><a:graphicData><wps:wsp><wps:spPr>
<a:prstGeom prst="rect"/></wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:t>窓口</w:t></w:r></w:p></w:txbxContent></wps:txbx>
</wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing>"#;
        if let Some(kumihan::Block::Para(p)) = d.blocks.last_mut() {
            p.anchors.push(a.into());
        }
        let (sheet, page, _) = doc_to_sheet(&d, None).expect("組めない");
        let v = foreign_shapes(&d, &sheet, page);
        assert_eq!(v.len(), 1, "図形が置かれていない");
        assert!(
            v[0].y_mm + v[0].h_mm <= page.h_mm,
            "紙({}mm)から落ちている: y {}mm + 高さ {}mm",
            page.h_mm, v[0].y_mm, v[0].h_mm
        );
    }
    use kumihan::{font, layout, Align, Document, Frame, Metrics};

    use super::*;

    fn sheet(text: &str, align: Align) -> (Sheet, Vec<u8>) {
        // 組む字が組める書体を選びます(上の `build` と同じ理由)
        let (fam, _) = font::for_text(None, text.chars()).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain(text);
        d.apply_align(0..text.len(), align);
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 24.0 });
        (s, data)
    }

    fn pdf_of(text: &str, align: Align) -> Vec<u8> {
        let (s, data) = sheet(text, align);
        let mut buf = Vec::new();
        to_pdf(&s, &data, Paper::default(), &mut buf).unwrap();
        buf
    }


    /// **途中で用紙の向きが変わる文書。** engine が節ごとに行を組み、
    /// paper が節ごとの紙で折る — その2つが噛み合っているかを端から端まで見る。
    /// (紙の大きさが違えば1ページに入る行数も違うので、折り目もずれる)
    #[test]
    fn paper_size_changes_per_section() {
        use kumihan::{Block, PageSetup, Paragraph, Run};
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let paper = |w: f32, h: f32| PageSetup {
            w_mm: w, h_mm: h, left_mm: 20.0, right_mm: 20.0,
            top_mm: 20.0, bottom_mm: 20.0, columns: 1,
        };
        let tab = |t: &str, sect: Option<PageSetup>| Block::Para(Paragraph {
            runs: vec![Run { text: t.into(), size_pt: None, font: None, fmt: Default::default() }],
            line_spacing: 1.0,
            sect: sect.map(|page| kumihan::SectionBreak {
                raw: String::new(), page, continuous: false }),
            ..Default::default()
        });
        let d = Document {
            page: Some(paper(210.0, 297.0)),          // 最後の節 = 縦
            blocks: vec![
                tab("縦の節の本文", None),
                tab("縦の節の終わり", Some(paper(210.0, 297.0))),
                tab("横の節の本文", Some(paper(297.0, 210.0))),
                tab("また縦の節", None),
            ],
            ..Default::default()
        };
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 24.0 });
        assert_eq!(s.sect_pages.len(), 3, "節ごとの紙が揃っていない: {:?}", s.sect_pages);

        let papers = paginate_full(&s, Paper::default()).papers;
        let widths: Vec<f32> = papers.iter().map(|p| p.width_mm).collect();
        // 縦 → 横 → 縦。**節の切れ目で必ず頁が割れる**ので3ページ以上になる
        assert!(papers.len() >= 3, "節の切れ目で頁が割れていない: {widths:?}");
        assert_eq!(widths[0], 210.0, "1ページ目が最初の節の紙になっていない: {widths:?}");
        assert!(widths.contains(&297.0), "横の節の紙が出ていない: {widths:?}");
        assert_eq!(*widths.last().unwrap(), 210.0, "最後の節が縦に戻っていない: {widths:?}");

        // PDF にしても落ちない(ページごとに大きさが違う紙を作る)
        let mut buf = Vec::new();
        to_pdf(&s, &data, Paper::default(), &mut buf).unwrap();
        assert_eq!(&buf[..5], b"%PDF-");
    }

    #[test]
    fn single_section_keeps_the_old_layout() {
        let (s, _) = sheet("いろはにほへと。".repeat(200).as_str(), Align::Left);
        assert!(s.sect_pages.is_empty(), "節が1つなのに節ごとの紙を持った");
        let (pages, offsets) = paginate(&s, Paper::default());
        let pf = paginate_full(&s, Paper::default());
        let (p2, o2, papers) = (pf.pages, pf.offsets, pf.papers);
        assert_eq!(pages, p2);
        assert_eq!(offsets, o2);
        assert!(papers.iter().all(|p| p.width_mm == 210.0), "紙が勝手に変わった");
    }


    /// **紙の外へ字が出ないか。** 節で紙が変わるとき、文書の紙(最後の節)で
    /// 裏返すと、向きの違うページで字が紙からはみ出す(縦297と横210で87mmずれる)。
    /// ページの大きさだけを見る試験では通ってしまうので、**中身の座標**を見る
    #[test]
    fn text_stays_inside_paper_across_sections() {
        use kumihan::{Block, PageSetup, Paragraph, Run};
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let paper = |w: f32, h: f32| PageSetup {
            w_mm: w, h_mm: h, left_mm: 20.0, right_mm: 20.0,
            top_mm: 20.0, bottom_mm: 20.0, columns: 1,
        };
        let tab = |t: &str, sect: Option<PageSetup>| Block::Para(Paragraph {
            runs: vec![Run { text: t.into(), size_pt: None, font: None, fmt: Default::default() }],
            line_spacing: 1.0,
            sect: sect.map(|page| kumihan::SectionBreak {
                raw: String::new(), page, continuous: false }),
            ..Default::default()
        });
        let d = Document {
            page: Some(paper(210.0, 297.0)),
            blocks: vec![
                tab("縦の節。", Some(paper(210.0, 297.0))),
                tab("横の節。ここは紙が低い(210mm)ので、裏返しを間違えると外へ出る。", None),
            ],
            ..Default::default()
        };
        // 2つ目の節は Document::page(縦)なので、横は1つ目…ではない。
        // 節末に紙を置いた1段目が縦、残りが最後の節。ここでは横紙を最後に置く
        let d = Document { page: Some(paper(297.0, 210.0)), ..d };
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 24.0 });
        let pf = paginate_full(&s, Paper::default());
        let (pages, offsets, papers) = (pf.pages, pf.offsets, pf.papers);
        for (i, line) in s.lines.iter().enumerate() {
            if line.cells.is_empty() { continue }
            let k = pages[i];
            let pp = papers[k - 1];
            let y = pp.height_mm - (line.y_mm - offsets[k - 1]);
            assert!(y >= 0.0 && y <= pp.height_mm,
                "{k}ページ目({}x{})で字が紙の外: y={y}", pp.width_mm, pp.height_mm);
            let right = pp.margin_mm + line.cells.last().unwrap().x_mm
                + line.cells.last().unwrap().w_mm;
            assert!(right <= pp.width_mm + 0.5,
                "{k}ページ目で字が右へはみ出した: {right}mm > {}mm", pp.width_mm);
        }
    }


    /// **1ページ目だけ紙が違う**という壊れ方。節ごとの紙を「上の余白から」
    /// 置くと、その手前を引いたときに節が無いと読めて、1ページ目が
    /// 最後の節の紙で刷られる。実物の2節 docx を PDF まで通して見つけた
    #[test]
    fn first_page_uses_the_first_sections_paper() {
        use kumihan::{Block, PageSetup, Paragraph, Run};
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let paper = |w: f32, h: f32| PageSetup {
            w_mm: w, h_mm: h, left_mm: 20.0, right_mm: 20.0,
            top_mm: 20.0, bottom_mm: 20.0, columns: 1,
        };
        let tab = |t: &str, sect: Option<PageSetup>| Block::Para(Paragraph {
            runs: vec![Run { text: t.into(), size_pt: None, font: None, fmt: Default::default() }],
            line_spacing: 1.0,
            sect: sect.map(|page| kumihan::SectionBreak {
                raw: String::new(), page, continuous: false }),
            ..Default::default()
        });
        let d = Document {
            page: Some(paper(297.0, 210.0)),                  // 最後の節 = 横
            blocks: vec![
                tab("縦の節", Some(paper(210.0, 297.0))),       // 最初の節 = 縦
                tab("横の節", None),
            ],
            ..Default::default()
        };
        // 上の余白より上を引いても、最初の節が返らねばならない
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 20.0 });
        assert_eq!(s.setup_at(0.0).map(|g| (g.w_mm, g.h_mm)), Some((210.0, 297.0)),
            "巻物の頭で最初の節が引けない: {:?}", s.sect_pages);

        let page_of = Paper::hitoshii(297.0, 210.0, 20.0 );
        let papers = paginate_full(&s, page_of).papers;
        assert_eq!((papers[0].width_mm, papers[0].height_mm), (210.0, 297.0),
            "1ページ目が最後の節の紙で刷られた");
    }


    /// **画像・罫線の頁割りが本文とずれない。**
    ///
    /// ここは長らく本文とは別に「どの頁も同じ高さ」と決め打ちした近似を
    /// 持っていた。同じ y に居る本文の行と同じ頁に来ることを直接見る —
    /// 「PDF が出来た」だけを見る試験はこのずれを通してしまう
    /// (SEKKEI.md「緑は『正しい』ではなく『この物差しでは差が出ない』」)
    /// **上下の余白が左右と違う文書でも、どの頁も同じ高さで始まる。**
    ///
    /// 2026-08-30、内閣府の告知書(左右 25mm・上下 30mm)で見つけました。
    /// `Paper` が余白を1つしか持たず、頁割りが**左の余白を上下にも**
    /// 使っていたので、2頁目からの本文が 25mm の高さで始まり、1頁に
    /// 2行余分に入っていました。13頁の文書が 11頁になります。
    #[test]
    fn every_page_starts_at_the_top_margin() {
        let pg = kumihan::PageSetup {
            w_mm: 210.0, h_mm: 297.0,
            left_mm: 25.0, right_mm: 25.0, top_mm: 30.0, bottom_mm: 30.0,
            columns: 1,
        };
        // 助手の `sheet` は固定の枠で組むので、ここは紙の設定に合わせて
        // 自分で組みます(1頁目の頭も `top_mm + BASE_UP_MM` になります)
        let (fam, _) = font::for_text(None, "いろは".chars()).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let d = Document::plain(&"いろはにほへとちりぬるを。".repeat(400));
        let s = layout(&d, &m, &Frame {
            measure_mm: pg.column_measure_mm(),
            line_height_mm: kumihan::LINE_MM,
            y0_mm: pg.top_mm + kumihan::BASE_UP_MM,
        });
        let pg2 = paginate_full(&s, Paper::from_page(&pg));
        assert!(pg2.offsets.len() >= 3, "頁が足りず試験にならない");
        // 各頁の1行目が、紙の上から見て同じ高さに来ること
        let mut atama: Vec<f32> = Vec::new();
        for (k, _) in pg2.offsets.iter().enumerate() {
            let i = pg2.pages.iter().position(|p| *p == k + 1).expect("頁が空");
            atama.push(s.lines[i].y_mm - pg2.offsets[k]);
        }
        for (k, y) in atama.iter().enumerate().skip(1) {
            assert!((y - atama[0]).abs() < 0.05,
                    "{} 頁目の頭が1頁目とずれた: {y} / {}", k + 1, atama[0]);
        }
        // その高さは**上の余白**(左の 25mm ではない)
        assert!((atama[0] - (pg.top_mm + kumihan::BASE_UP_MM)).abs() < 0.05,
                "頭が上の余白から始まっていない: {}", atama[0]);
    }

    #[test]
    fn image_and_border_pagination_matches_the_text() {
        let (s, _) = sheet(&"いろはにほへとちりぬるを。".repeat(400), Align::Left);
        let pg = paginate_full(&s, Paper::default());
        assert!(pg.offsets.len() >= 3, "頁が足りず試験にならない: {}", pg.offsets.len());
        for (i, line) in s.lines.iter().enumerate() {
            if line.cells.is_empty() { continue }
            assert_eq!(pg.page_at(line.y_mm), pg.pages[i] - 1,
                "行 {i}(y={})の頁が本文とずれた", line.y_mm);
        }
    }

    /// 節で紙が変わる文書でも同じ。**紙の高さが頁ごとに違う**ので、
    /// 「どの頁も同じ高さ」の近似はここで必ず外れる
    #[test]
    fn image_and_border_pagination_matches_the_text_across_sections() {
        use kumihan::{Block, PageSetup, Paragraph, Run};
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let paper = |w: f32, h: f32| PageSetup {
            w_mm: w, h_mm: h, left_mm: 20.0, right_mm: 20.0,
            top_mm: 20.0, bottom_mm: 20.0, columns: 1,
        };
        let tab = |t: &str, sect: Option<PageSetup>| Block::Para(Paragraph {
            runs: vec![Run { text: t.into(), size_pt: None, font: None, fmt: Default::default() }],
            line_spacing: 1.0,
            sect: sect.map(|page| kumihan::SectionBreak {
                raw: String::new(), page, continuous: false }),
            ..Default::default()
        });
        let long_text = "いろはにほへとちりぬるを。".repeat(120);
        let d = Document {
            page: Some(paper(297.0, 210.0)),                       // 最後の節 = 横
            blocks: vec![
                tab(&long_text, Some(paper(210.0, 297.0))),              // 縦の節
                tab(&long_text, None),                                  // 横の節
            ],
            ..Default::default()
        };
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 20.0 });
        let pg = paginate_full(&s, Paper::default());
        assert!(pg.papers.iter().any(|q| q.width_mm == 297.0), "横の紙が出ていない");
        for (i, line) in s.lines.iter().enumerate() {
            if line.cells.is_empty() { continue }
            assert_eq!(pg.page_at(line.y_mm), pg.pages[i] - 1,
                "行 {i}(y={})の頁が本文とずれた", line.y_mm);
        }
    }

    #[test]
    fn becomes_pdf() {
        let b = pdf_of("日本語の書類を紙にする。", Align::Left);
        assert_eq!(&b[..5], b"%PDF-", "PDF になっていない");
        assert!(b.len() > 1000, "中身が薄すぎる: {} バイト", b.len());
    }

    #[test]
    fn built_from_the_same_layout_as_the_screen() {
        // 組み直さないので、行数は紙面のまま
        let (s, data) = sheet("一行目\n二行目\n三行目", Align::Left);
        assert_eq!(s.lines.len(), 3);
        let mut buf = Vec::new();
        to_pdf(&s, &data, Paper::default(), &mut buf).unwrap();
        assert_eq!(&buf[..5], b"%PDF-");
    }

    #[test]
    fn centering_reaches_paper() {
        // 揃えは紙面の x に入っているので、PDF 側で作り直さない
        let (left, _) = sheet("表題", Align::Left);
        let (center, _) = sheet("表題", Align::Center);
        assert!(
            center.lines[0].cells[0].x_mm > left.lines[0].cells[0].x_mm,
            "中央揃えが紙面に出ていない"
        );
    }

    #[test]
    fn empty_layout_does_not_panic() {
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let mut buf = Vec::new();
        to_pdf(&Sheet::default(), &data, Paper::default(), &mut buf).unwrap();
        assert_eq!(&buf[..5], b"%PDF-");
    }
}

#[cfg(test)]
mod page_tests {
    use kumihan::{font, layout, Document, Frame, Metrics};

    use super::*;

    fn pages(n_lines: usize) -> usize {
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let text = vec!["行"; n_lines].join("\n");
        let d = Document::plain(&text);
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 24.0 });
        let mut buf = Vec::new();
        to_pdf(&s, &data, Paper::default(), &mut buf).unwrap();
        // ページ数は PDF の /Count に書かれている
        let hay = String::from_utf8_lossy(&buf).to_string();
        let i = hay.find("/Count ").expect("/Count が無い") + 7;
        hay[i..].chars().take_while(|c| c.is_ascii_digit())
            .collect::<String>().parse().unwrap()
    }

    #[test]
    fn long_document_spans_pages() {
        // A4 で本文が入るのは 40行くらい。100行が1ページに収まっていたら
        // それは「下へ黙ってはみ出している」ということ
        assert_eq!(pages(10), 1, "10行で複数ページになった");
        let p100 = pages(100);
        assert!(p100 >= 3, "100行が {p100} ページにしかならない(はみ出している)");
    }

    #[test]
    fn page_count_matches_line_count() {
        // A4(y0=24, 下余白20)に入るのは約40行。100行なら3ページ
        let n = pages(100);
        assert!((3..=4).contains(&n), "100行が {n} ページ(40行/頁の見当と合わない)");
        assert!(pages(300) >= 8, "300行が {} ページ", pages(300));
    }
}

#[cfg(test)]
mod hf_tests {
    use kumihan::{font, layout, layout_hf, Document, Frame, HeadFoot, Metrics, PageSetup,
                  PAGE_MARK};

    use super::*;

    #[test]
    fn page_numbers_on_every_page_without_changing_the_count() {
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let text = vec!["行"; 100].join("\n");
        let d = Document::plain(&text);
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 24.0 });
        let pg = PageSetup::default();
        let hf = HeadFoot {
            paragraphs: Document::plain(&PAGE_MARK.to_string())
                .paragraphs().cloned().collect(),
            part: None,
            anchors: Vec::new(),
        };
        let mut buf = Vec::new();
        to_pdf_with(&s, &data, Paper::default(), &PageDress::default(),
            |k| layout_hf(&hf, &m, &pg, 6.4, k, 9, true, kumihan::DEFAULT_PT), &mut buf).unwrap();
        assert_eq!(&buf[..5], b"%PDF-");
        // ページ数は本文で決まる(飾りで増えない)
        let hay = String::from_utf8_lossy(&buf).to_string();
        let i = hay.find("/Count ").unwrap() + 7;
        let n: usize = hay[i..].chars().take_while(|c| c.is_ascii_digit())
            .collect::<String>().parse().unwrap();
        assert!((3..=4).contains(&n), "{n} ページ");
    }
}

#[cfg(test)]
mod break_tests {
    use kumihan::{font, layout, Block, Document, Frame, Metrics};

    use super::*;

    #[test]
    fn page_break_splits_pages() {
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("一頁目\n二頁目");
        if let Block::Para(p) = &mut d.blocks[1] {
            p.page_break_before = true;
        }
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 24.0 });
        assert_eq!(s.breaks.len(), 1, "改ページが紙面に伝わっていない");
        let mut buf = Vec::new();
        to_pdf(&s, &data, Paper::default(), &mut buf).unwrap();
        let hay = String::from_utf8_lossy(&buf).to_string();
        let i = hay.find("/Count ").unwrap() + 7;
        let n: usize = hay[i..].chars().take_while(|c| c.is_ascii_digit())
            .collect::<String>().parse().unwrap();
        assert_eq!(n, 2, "2行の文書が改ページで2頁にならず {n} 頁");
    }

    #[test]
    fn leading_page_break_adds_no_page() {
        // 1段落目に改ページが付いていても、空の1頁目を作らない
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("本文");
        if let Block::Para(p) = &mut d.blocks[0] {
            p.page_break_before = true;
        }
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 24.0 });
        assert!(s.breaks.is_empty(), "先頭で頁を割った");
    }
}

#[cfg(test)]
mod image_tests {
    use kumihan::{font, layout, Document, Frame, InlineImage, Metrics};

    use super::*;

    /// 本物の PNG をその場で作る(手打ちのバイト列は CRC を壊しやすい)
    fn png() -> Vec<u8> {
        let img = ::image::RgbImage::from_pixel(4, 4, ::image::Rgb([200, 30, 30]));
        let mut buf = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ::image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn image_is_embedded_in_paper() {
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("写真の前\n写真の後");
        if let kumihan::Block::Para(p) = &mut d.blocks[0] {
            p.images.push(InlineImage {
                bytes: std::sync::Arc::new(png()),
                w_mm: 40.0,
                h_mm: 30.0,
                tex: None,
                src: None,
                off: 0,
            });
        }
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 24.0 });
        assert_eq!(s.images.len(), 1, "紙面に画像が無い");
        let mut buf = Vec::new();
        to_pdf(&s, &data, Paper::default(), &mut buf).unwrap();
        let hay = String::from_utf8_lossy(&buf);
        // 画像は XObject として埋まる
        assert!(hay.contains("/XObject"), "PDF に画像が埋まっていない");
    }

    #[test]
    fn broken_image_skipped_and_paper_still_made() {
        // 1枚のために紙全体を失敗させない
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("本文");
        if let kumihan::Block::Para(p) = &mut d.blocks[0] {
            p.images.push(InlineImage {
                bytes: std::sync::Arc::new(b"not an image".to_vec()),
                w_mm: 40.0,
                h_mm: 30.0,
                tex: None,
                src: None,
                off: 0,
            });
        }
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 24.0 });
        let mut buf = Vec::new();
        to_pdf(&s, &data, Paper::default(), &mut buf).unwrap();
        assert_eq!(&buf[..5], b"%PDF-");
    }
}



#[cfg(test)]
mod footnote_area_tests {
    use super::*;
    use kumihan::{font, layout, Block, CharFormat, Document, FootnoteRef, Footnote,
                  Frame, Metrics, Paragraph, Run};

    fn build(d: &Document) -> Sheet {
        // **文中の字が組める書体を選びます。** 既定の言語は機械の設定に
        // よるので(2026-08-30 から en が落ち先)、`for_document(None)` だと
        // 言語を設定していない機械で仮名の無い書体が返り、行が0本になります
        let (fam, _) = font::for_text(None, d.chars()).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        layout(d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 20.0 })
    }
    fn mark(id: &str) -> Run {
        Run { text: String::new(), size_pt: None, font: None,
              fmt: CharFormat { footnote: Some(FootnoteRef { id: id.into(), endnote: false }),
                                ..Default::default() } }
    }
    fn text(t: &str) -> Run {
        Run { text: t.into(), size_pt: None, font: None, fmt: CharFormat::default() }
    }
    fn tab(runs: Vec<Run>) -> Block {
        Block::Para(Paragraph { runs, line_spacing: 1.0, ..Default::default() })
    }
    fn note(id: &str, t: &str) -> Footnote {
        Footnote { added: false, id: id.into(), endnote: false,
                   paragraphs: vec![Paragraph { runs: vec![text(t)], line_spacing: 1.0,
                                                ..Default::default() }] }
    }

    /// **脚注は本文に使える高さを削る。** 削っていないと、脚注の上に
    /// 本文が重なって刷られる
    #[test]
    fn footnotes_raise_the_text_bottom() {
        let long_text = "いろはにほへとちりぬるを。".repeat(200);
        let none_of = Document {
            blocks: vec![tab(vec![text(&long_text)])],
            ..Default::default()
        };
        let some_of = Document {
            blocks: vec![tab(vec![mark("9"), text(&long_text)])],
            footnotes: vec![note("9", &"脚注の文章。".repeat(20))],
            ..Default::default()
        };
        let (s1, s2) = (build(&none_of), build(&some_of));
        let p1 = paginate_full(&s1, Paper::default());
        let p2 = paginate_full(&s2, Paper::default());
        assert!(!s2.notes.is_empty(), "脚注が組まれていない");
        let first_page = |p: &Pagination| p.pages.iter().filter(|k| **k == 1).count();
        assert!(first_page(&p2) < first_page(&p1),
            "脚注があるのに1頁目の本文の行数が減っていない: {} / {}",
            first_page(&p2), first_page(&p1));
    }

    /// **字が紙の中に収まる。** 「脚注が出た」だけを見る試験は、
    /// 紙の外へ出ていても緑になる(SEKKEI.md の教訓)
    #[test]
    fn footnote_text_stays_inside_paper() {
        let long_text = "いろはにほへとちりぬるを。".repeat(200);
        let d = Document {
            blocks: vec![tab(vec![mark("9"), text(&long_text)])],
            footnotes: vec![note("9", "脚注の文章。")],
            ..Default::default()
        };
        let s = build(&d);
        let pg = paginate_full(&s, Paper::default());
        let mut seen = 0usize;
        for (k, idx) in pg.notes.iter().enumerate() {
            if idx.is_empty() { continue }
            let pp = pg.papers.get(k).copied().unwrap_or(Paper::default());
            let total: f32 = idx.iter().map(|i| s.notes[*i].h_mm).sum();
            let top = pp.margin_mm + total;
            let mut up = 0.0f32;
            for i in idx {
                let nb = &s.notes[*i];
                for nl in &nb.lines {
                    let y = top - up - nl.y_mm;
                    assert!(y > 0.0 && y < pp.height_mm,
                        "脚注の字が紙の外: y={y} 紙の高さ={}", pp.height_mm);
                    // 下余白より上、かつ本文の底より下に居る
                    assert!(y <= top + 0.01, "脚注が本文の側へ食い込んだ: y={y} top={top}");
                    seen += 1;
                }
                up += nb.h_mm;
            }
        }
        assert!(seen > 0, "脚注の行を1つも見ていない(試験になっていない)");
    }

    /// 脚注は**印のある頁**に出る。印が2頁目なら脚注も2頁目
    #[test]
    fn footnote_appears_on_the_page_with_its_mark() {
        let long_text = "いろはにほへとちりぬるを。".repeat(200);
        let d = Document {
            blocks: vec![
                tab(vec![text(&long_text)]),
                tab(vec![text("後ろの段落"), mark("9")]),
            ],
            footnotes: vec![note("9", "後ろの脚注。")],
            ..Default::default()
        };
        let s = build(&d);
        let pg = paginate_full(&s, Paper::default());
        assert!(pg.offsets.len() >= 2, "頁が足りず試験にならない");
        let placed: Vec<usize> = pg.notes.iter().enumerate()
            .filter(|(_, v)| !v.is_empty()).map(|(k, _)| k).collect();
        assert_eq!(placed.len(), 1, "脚注が複数の頁に出た: {placed:?}");
        // 印のある行の頁と一致するか
        let at_y = s.notes[0].at_y;
        let mark_page = pg.page_at(at_y);
        assert_eq!(placed[0], mark_page, "脚注が印と違う頁に出た");
        assert!(mark_page > 0, "この試験は2頁目に印が来る形を見ている");
    }

    /// 脚注が無ければ今までどおり(頁割りは1ミリも変わらない)
    #[test]
    fn no_footnotes_no_pagination_change() {
        let long_text = "いろはにほへとちりぬるを。".repeat(200);
        let d = Document { blocks: vec![tab(vec![text(&long_text)])], ..Default::default() };
        let s = build(&d);
        assert!(s.notes.is_empty(), "脚注が無いのに組んだ");
        let pg = paginate_full(&s, Paper::default());
        assert!(pg.notes.iter().all(|v| v.is_empty()), "脚注が無いのに頁に付いた");
    }
}


/// 試験のための読み(adoc → 文書)。**本体では使いません**
#[cfg(test)]
pub(crate) fn super_parse(src: &str) -> kumihan::Document {
    kumihan::adoc::parse(src).expect("読めない")
}

/// **文書から PDF を1手で作る**(2026-08-27 発注者「エンジンで pdf を
/// つくるところまで」)。
///
/// これまで PDF を作れたのは画面(writer)だけでした。組む所と紙にする所は
/// エンジンに在ったのに、**その2つを繋ぐ入り口が無かった**ので、Python から
/// PDF を作るには動いているアプリを呼ぶしかありませんでした。
///
/// テンプレートを渡すと、見た目を合成してから組みます(渡さなければ同梱の
/// 既定)。**画面と同じ字幅で組みます** — 書体の実体を読んで渡すので、
/// 画面と紙がずれません。
///
/// # 例
///
/// ```no_run
/// let doc = kumihan::adoc::parse("= 題\n\n本文です。\n").unwrap();
/// let f = std::fs::File::create("out.pdf").unwrap();
/// paper::doc_to_pdf(&doc, None, f).unwrap();
/// ```
pub fn doc_to_pdf<W: Write>(
    doc: &kumihan::Document,
    theme: Option<&kumihan::theme::Theme>,
    out: W,
) -> Result<(), String> {
    let (sheet, page, bytes) = doc_to_sheet(doc, theme)?;
    // **低い層の書き手を通します**(2026-08-27)。使った字だけ埋めるので、
    // 1枚物が 20MB から 10KB になります。ここが最初の差し替えです —
    // 画面(writer)の書き出しはまだ printpdf のままです
    // **ページに貼り付く図形と紙の飾りを渡します**(2026-08-29)。
    // 渡さないと、模型に在っても紙に出ません
    // **他所のテキストボックスも紙に出します**(2026-08-30)。模型には
    // 入っていない(原文の控えのまま持ち越している)ので、ここで置き直します
    let mut shapes = doc.shapes.clone();
    shapes.extend(foreign_shapes(doc, &sheet, page));
    let dress = PageDress {
        watermark: doc.watermark.clone(),
        shapes,
        ..Default::default()
    };
    let lost = pdfw::sheet_to_pdf_with(
        &sheet,
        &bytes,
        Paper::from_page(&page),
        &dress,
        |_| Vec::new(),
        out,
    )?;
    // 載らなかった物は呼ぶ側へ言えないので、ここでは黙るしかありません。
    // **数える口が要るなら [`pdfw::sheet_to_pdf`] を直に呼びます**
    let _ = lost;
    Ok(())
}

/// **文書の紙面を全部取り出す。** PDF は書きません。
///
/// 絵にする道([`e`])と回帰検査の入り口です。**PDF と同じ組み方**を
/// 通るので、絵と紙が食い違いません。
///
/// `page` は [`doc_to_sheet`] が返した紙の設定をそのまま渡します。
pub fn doc_leaves(sheet: &kumihan::Sheet, page: kumihan::PageSetup) -> Vec<pdfw::Leaf> {
    doc_leaves_with(sheet, page, &PageDress::default())
}

/// 紙の飾り(透かし・ページに貼り付く図形)も渡して紙面を組む
pub fn doc_leaves_with(
    sheet: &kumihan::Sheet,
    page: kumihan::PageSetup,
    dress: &PageDress,
) -> Vec<pdfw::Leaf> {
    let paper = Paper::from_page(&page);
    let (pages, _lost) = pdfw::sheet_leaves_with(sheet, paper, dress, |_| Vec::new());
    pages
}

/// 文書の紙面を1枚だけ取り出す。`k` は0から数えた頁です
pub fn doc_leaf(sheet: &kumihan::Sheet, page: kumihan::PageSetup, k: usize) -> Option<pdfw::Leaf> {
    doc_leaves(sheet, page).into_iter().nth(k)
}

/// 文書を紙面に組む。**PDF と画面が同じ道を通る**ための1本です。
///
/// 返りは(組んだ紙面, 紙の設定, 書体の実体)。
pub fn doc_to_sheet(
    doc: &kumihan::Document,
    theme: Option<&kumihan::theme::Theme>,
) -> Result<(kumihan::Sheet, kumihan::PageSetup, Vec<u8>), String> {
    // **見た目はテンプレートが決めます。** 渡されなければ同梱の既定です
    let fallback;
    let t = match theme {
        Some(t) => t,
        None => {
            fallback = kumihan::theme::default_theme();
            &fallback
        }
    };
    // **`compose` です。`compose_page` ではありません。**
    // `compose_page` は紙の設定と飾りだけで、段落にスタイルを着せません。
    // 間違えると註記の帯も見出しの背景も出ません(2026-08-27 に実物で
    // 気づいた — 試験は緑でした)
    let mut d = kumihan::theme::compose(doc, t);

    // **数式を絵にします。** docx の数式(OMML)は読むときに LaTeX へ直して
    // 置いてあるだけなので、ここで組まないと紙に何も出ません。
    // 中の日本語のために、文書の書体のファイルを渡します
    {
        let na = d.font.clone().or_else(|| t.font.clone());
        let moji = kumihan::font::for_document(na.as_deref())
            .ok()
            .and_then(|(f, _)| kumihan::font::load(f).ok());
        kumihan::suushiki::kumu_bunsho(&mut d, moji.as_deref());
    }

    // 書体は**文書が名乗った物**が先。次にテンプレートの物。
    // 文中の字も渡します — 選んだ書体がその字を持っていないと、
    // PDF ではその字だけ消えます(2026-08-30)
    //
    // **どちらも名乗らなければ本文の書体(明朝・セリフ)です**(2026-08-31)。
    // 同梱の既定テンプレートは書体の名前を書きません(機械にある物から
    // 選ぶため)ので、ここが空のまま `default_family` に落ちていました。
    // それはゴシック・サンセリフなので、**同じ文書が docx では明朝、
    // PDF ではゴシック**で出ていました。手引きはこれを「この版の限界」と
    // 断り書きしていたところです。
    let want = d.font.clone().or_else(|| t.font.clone()).or_else(|| {
        kumihan::font::default_generic(
            &kumihan::font::default_language(),
            kumihan::font::Generic::Serif,
        )
        .map(|f| f.name.clone())
    });
    let (family, _) = kumihan::font::for_text(want.as_deref(), d.chars())?;
    let bytes = kumihan::font::load(family)?;
    let m = kumihan::Metrics::new(&bytes)?;

    let page = d.page.unwrap_or_default();
    // **行送りはエンジンの1つを見ます**(画面と紙と PDF で同じ)。
    // ここで計算し直すと、同じ文書が別の頁数に折れます
    let line_mm = kumihan::LINE_MM;
    // 段組みも紙の設定から。writer の画面と同じ関数を通します
    let measure = page.column_measure_mm();
    let y0 = page.top_mm + kumihan::BASE_UP_MM;
    let sheet = kumihan::layout(
        &d,
        &m,
        &kumihan::Frame { measure_mm: measure, line_height_mm: line_mm, y0_mm: y0 },
    );
    Ok((sheet, page, bytes))
}

#[cfg(test)]
mod doc_pdf_tests {
    /// **文書から PDF が1手で出る。**
    #[test]
    fn a_document_becomes_a_pdf_in_one_step() {
        let doc = kumihan::adoc::parse("= 四月の売上\n\n本文です。\n\n== まとめ\n\n終わり。\n")
            .expect("読めない");
        let mut buf = Vec::new();
        super::doc_to_pdf(&doc, None, std::io::Cursor::new(&mut buf)).expect("PDF が出ない");
        assert!(buf.starts_with(b"%PDF"), "PDF になっていない");
        assert!(buf.len() > 1000, "中身が薄すぎる: {} バイト", buf.len());
    }

    /// **本文は明朝・セリフで組む。**
    ///
    /// 書体を名乗らない文書は、同梱の既定テンプレートも書体を書かないので、
    /// 機械の既定(ゴシック・サンセリフ)に落ちていました。docx で保存した
    /// 同じ文書は明朝で出るので、**同じ文書が形式で違う顔**になっていました。
    #[test]
    fn the_body_is_set_in_a_serif_face() {
        let _lock = kumihan::font::lang_lock();
        kumihan::font::set_default_language("ja");
        let doc = kumihan::adoc::parse("= 見本\n\n本文です。\n").expect("読めない");
        let (_sheet, _page, bytes) =
            crate::doc_to_sheet(&doc, None).expect("組めない");
        let serif = kumihan::font::default_generic("ja", kumihan::font::Generic::Serif)
            .expect("この機械に明朝がありません");
        let hoshii = kumihan::font::load(serif).expect("読めない");
        assert_eq!(
            bytes.len(),
            hoshii.len(),
            "本文が明朝で組まれていません(選ばれたのは {} ではない何か)",
            serif.name
        );
    }

    /// 空の文書でも落ちない(紙が1枚出る)
    #[test]
    fn an_empty_document_still_makes_a_page() {
        let doc = kumihan::Document::default();
        let mut buf = Vec::new();
        super::doc_to_pdf(&doc, None, std::io::Cursor::new(&mut buf)).expect("PDF が出ない");
        assert!(buf.starts_with(b"%PDF"));
    }
}

/// **ページごとの「先頭の段落」を出す。**
///
/// docx の図形は「どの段落に留まるか」でページが決まります(紙からの mm は
/// 持てても、何ページ目かは持てません)。だから、そのページに載っている
/// 段落へ結び付けないと違うページに出ます。
///
/// 返りは(ページ番号(0始まり)→ 段落の番号, 段落の番号 → 塊の番号)。
///
/// 2026-08-29 に writer の中から出しました。画面と同じ答えを Python の
/// 保存からも使うためです — **別に書くとページが食い違います**。
/// **他所のソフトが作った図形を、紙の上の場所へ置き直す。**
///
/// Word のテキストボックスは「この段落から下へ○mm」のような相対の位置で
/// 書いてあるので、組んでみないと場所が決まりません。うちが書く図形は名前に
/// ページ番号を持っているので、この道は通りません。
///
/// 2026-08-30 に足しました。内閣府の告知書の窓口の欄が3つとも、紙にも画面にも
/// 出ていませんでした(保存では原文のまま残っていたので、往復では気づけません)。

/// **錨の位置を解く**(docx の `wp:positionH` / `wp:positionV`)。
///
/// 位置は「基準(`relativeFrom`)」と「距離(`wp:posOffset`)または
/// 寄せ方(`wp:align`)」の組で書いてあります。基準ごとに原点と、
/// 寄せるときの幅が違います。ECMA-376 の `ST_RelFromH` / `ST_RelFromV` と、
/// LibreOffice の `GraphicHelpers.cxx`(`PositionHandler::lcl_attribute`)の
/// 対応がこの表です(2026-09-03 発注者「急ぎだけでやらずに全部やれ」)。
///
/// | 横の基準 | 原点 | 寄せる幅 |
/// |---|---|---|
/// | `page` | 紙の左端 | 紙の幅 |
/// | `margin` / `column` | 本文の左端 | 本文の幅 |
/// | `leftMargin` | 紙の左端 | 左の余白 |
/// | `rightMargin` | 本文の右端 | 右の余白 |
/// | `insideMargin` | 紙の左端 | 左の余白(見開きで左右が入れ替わる) |
/// | `outsideMargin` | 本文の右端 | 右の余白(同上) |
/// | `character` | 本文の左端 | 本文の幅 |
///
/// | 縦の基準 | 原点 | 寄せる幅 |
/// |---|---|---|
/// | `page` | 紙の上端 | 紙の高さ |
/// | `margin` | 本文の上端 | 本文の高さ |
/// | `topMargin` | 紙の上端 | 上の余白 |
/// | `bottomMargin` | 本文の下端 | 下の余白 |
/// | `paragraph` / `line` | その段落の上端 | 本文の高さ |
///
/// `kono` は段落の上端(mm)で、`paragraph` と `line` だけが使います。
/// `migi_page` は見開きの右の紙か(`inside` / `outside` が使います)。
fn anchor_place(
    from: &str,
    zure: f32,
    yose: Option<&str>,
    ookisa: f32,
    page: &kumihan::PageSetup,
    tate: bool,
    kono: f32,
    migi_page: bool,
) -> f32 {
    let (moto, haba) = if tate {
        let honbun = (page.h_mm - page.top_mm - page.bottom_mm).max(0.0);
        match from {
            "page" => (0.0, page.h_mm),
            "topMargin" => (0.0, page.top_mm),
            "bottomMargin" => (page.h_mm - page.bottom_mm, page.bottom_mm),
            "paragraph" | "line" => (kono, honbun),
            // "margin" と、知らない名前
            _ => (page.top_mm, honbun),
        }
    } else {
        let honbun = (page.w_mm - page.left_mm - page.right_mm).max(0.0);
        // 見開きの内・外。右の紙では左右が入れ替わります
        let (uchi_moto, uchi_haba) = if migi_page {
            (0.0, page.left_mm)
        } else {
            (page.w_mm - page.right_mm, page.right_mm)
        };
        match from {
            "page" => (0.0, page.w_mm),
            "leftMargin" => (0.0, page.left_mm),
            "rightMargin" => (page.w_mm - page.right_mm, page.right_mm),
            "insideMargin" => (uchi_moto, uchi_haba),
            "outsideMargin" => {
                if migi_page {
                    (page.w_mm - page.right_mm, page.right_mm)
                } else {
                    (0.0, page.left_mm)
                }
            }
            // "margin" / "column" / "character" と、知らない名前
            _ => (page.left_mm, honbun),
        }
    };
    let Some(y) = yose else {
        // 寄せ方が無ければ、基準からの距離です
        return moto + zure;
    };
    // 見開きの内・外は、右の紙で向きが逆になります
    let uchi = if migi_page { "right" } else { "left" };
    let soto = if migi_page { "left" } else { "right" };
    let y = match y {
        "inside" if !tate => uchi,
        "outside" if !tate => soto,
        "inside" if tate => "top",
        "outside" if tate => "bottom",
        other => other,
    };
    match y {
        "left" | "top" => moto,
        "right" | "bottom" => moto + haba - ookisa,
        "center" => moto + (haba - ookisa) / 2.0,
        _ => moto + zure,
    }
}


/// **紙や余白に対する百分率で決まる大きさ**(Word 2010 の `wp14:sizeRelH` /
/// `wp14:sizeRelV`)。`wp:extent` はそのときの控えで、こちらが本当の大きさです。
///
/// 基準は位置の側と同じ名前です([`anchor_place`] の表)。
fn anchor_size(
    pct: Option<&(String, f32)>,
    kitei: f32,
    page: &kumihan::PageSetup,
    tate: bool,
) -> f32 {
    let Some((from, wari)) = pct else { return kitei };
    let moto = if tate {
        match from.as_str() {
            "page" => page.h_mm,
            "topMargin" => page.top_mm,
            "bottomMargin" => page.bottom_mm,
            _ => page.h_mm - page.top_mm - page.bottom_mm,
        }
    } else {
        match from.as_str() {
            "page" => page.w_mm,
            "leftMargin" => page.left_mm,
            "rightMargin" => page.right_mm,
            _ => page.w_mm - page.left_mm - page.right_mm,
        }
    };
    (moto * wari).max(0.1)
}

pub fn foreign_shapes(
    doc: &kumihan::Document,
    sheet: &kumihan::Sheet,
    page: kumihan::PageSetup,
) -> Vec<kumihan::DocShape> {
    let pg = paginate_full(
        sheet,
        Paper::from_page(&page),
    );
    // 段落の頭が本文の何バイト目か([`page_head_paras`] と同じ数え方)
    let mut starts: Vec<usize> = Vec::new();
    let mut at = 0usize;
    for p in doc.paragraphs() {
        starts.push(at);
        at += p.runs.iter().map(|r| r.text.len()).sum::<usize>() + 1;
    }
    let mut out: Vec<kumihan::DocShape> = Vec::new();
    // **ヘッダーとフッターの図形は、どの紙にも出します**(2026-09-01)。
    // 紙の飾り枠がこれです。位置は紙が基準(`relativeFrom="page"`)なので、
    // 紙ごとに同じ所へ置けば足ります
    let kami_kazu = pg.offsets.len().max(1);
    for a in doc.header.anchors.iter().chain(doc.footer.anchors.iter()) {
        let Some(f) = ooxml::foreign_shape(a) else { continue };
        // 大きさが百分率で書いてあれば、そちらが本当の大きさです
        let w_mm = anchor_size(f.w_pct.as_ref(), f.w_mm, &page, false);
        let h_mm = anchor_size(f.h_pct.as_ref(), f.h_mm, &page, true);
        for k in 0..kami_kazu {
            // 見開きの内・外は紙ごとに向きが変わるので、紙の中で解きます
            let migi = k % 2 == 1;
            let x = anchor_place(&f.h_from, f.x_mm, f.h_align.as_deref(),
                                 w_mm, &page, false, page.top_mm, migi);
            let y = anchor_place(&f.v_from, f.y_mm, f.v_align.as_deref(),
                                 h_mm, &page, true, page.top_mm, migi);
            out.push(kumihan::DocShape {
                page: k,
                x_mm: x,
                y_mm: y,
                w_mm,
                h_mm,
                look: f.look.clone(),
            });
        }
    }
    for (pi, para) in doc.paragraphs().enumerate() {
        if para.anchors.is_empty() {
            continue;
        }
        // その段落の1行目が、どの頁の、頁の中のどの高さに来たか
        let hajime = starts[pi];
        let Some(li) = sheet.lines.iter().position(|l| l.from_body && l.byte0 >= hajime) else {
            continue;
        };
        let kami = pg.pages.get(li).copied().unwrap_or(1) - 1;
        let soko = pg.offsets.get(kami).copied().unwrap_or(0.0);
        // 行を紙に置くときと同じ数え方です(`pdfw` の `y_roll`)。
        // 上の余白はもう `y_mm` に入っているので、足すと二重になります
        let y_para = sheet.lines[li].y_mm - soko;
        for a in &para.anchors {
            let Some(mut f) = ooxml::foreign_shape(a) else { continue };
            // **箱が書体を言っていなければ文書の既定**です。行送りと
            // ベースラインの位置がこれで決まります(2026-09-01)
            if f.look.text_fmt.font.is_none() {
                f.look.text_fmt.font = doc.font.clone();
            }
            // 基準と寄せ方は [`anchor_place`] の表のとおりに解きます
            let migi = kami % 2 == 1;
            let w_mm = anchor_size(f.w_pct.as_ref(), f.w_mm, &page, false);
            let h_mm = anchor_size(f.h_pct.as_ref(), f.h_mm, &page, true);
            let x = anchor_place(&f.h_from, f.x_mm, f.h_align.as_deref(),
                                 w_mm, &page, false, y_para, migi);
            let mut y = anchor_place(&f.v_from, f.y_mm, f.v_align.as_deref(),
                                     h_mm, &page, true, y_para, migi);
            // **紙に入らない図形は次の紙へ送ります**(2026-08-31)。Word は
            // 錨の段落ごと送ります。内閣府の調査票は窓口の欄2つが 305mm の
            // 所に来ていて、A4(297mm)の下に落ちて紙に出ていませんでした
            let mut kami = kami;
            let tsukaeru = (page.h_mm - page.top_mm - page.bottom_mm).max(1.0);
            let mut nogare = 0;
            while y + h_mm > page.h_mm && nogare < 8 {
                y -= tsukaeru;
                kami += 1;
                nogare += 1;
            }
            out.push(kumihan::DocShape {
                page: kami,
                x_mm: x,
                y_mm: y,
                w_mm,
                h_mm,
                look: f.look,
            });
        }
    }
    out
}

pub fn page_head_paras(
    doc: &kumihan::Document,
    sheet: &kumihan::Sheet,
    page: kumihan::PageSetup,
) -> (std::collections::BTreeMap<usize, usize>, Vec<usize>) {
    let (pages, _) = paginate(
        sheet,
        Paper::from_page(&page),
    );
    // 段落の頭が本文の何バイト目か
    let mut starts: Vec<usize> = Vec::new();
    let mut at = 0usize;
    for p in doc.paragraphs() {
        starts.push(at);
        at += p.runs.iter().map(|r| r.text.len()).sum::<usize>() + 1;
    }
    let mut page_para: std::collections::BTreeMap<usize, usize> = Default::default();
    for (l, pg) in sheet.lines.iter().zip(&pages) {
        if !l.from_body {
            continue;
        }
        let pi = starts.iter().rposition(|s| *s <= l.byte0).unwrap_or(0);
        page_para.entry(pg - 1).or_insert(pi);
    }
    let para_block_idx: Vec<usize> = doc
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| matches!(b, kumihan::Block::Para(_)))
        .map(|(i, _)| i)
        .collect();
    (page_para, para_block_idx)
}

/// **ページに貼り付く図形を、そのページの段落へ結び付ける。**
///
/// 保存の前に1度呼びます。返った写しを `ooxml::write*` へ渡すと、
/// 2ページ目以降の図形も正しいページに出ます。
///
/// 組み上がりが要るので、ここ(paper)に置いています。ooxml は組む所を
/// 知りません。
pub fn doc_with_shapes(doc: &kumihan::Document) -> kumihan::Document {
    if doc.shapes.is_empty() {
        return doc.clone();
    }
    let Ok((sheet, page, _)) = doc_to_sheet(doc, None) else { return doc.clone() };
    let (page_para, para_block) = page_head_paras(doc, &sheet, page);
    let mut out = doc.clone();
    let mut nokori: Vec<kumihan::DocShape> = Vec::new();
    for (i, sp) in doc.shapes.iter().enumerate() {
        let saki = page_para
            .get(&sp.page)
            .and_then(|pi| para_block.get(*pi))
            .copied();
        let Some(bi) = saki else {
            // そのページが無い(図形が紙より後ろ)— 模型には残します
            nokori.push(sp.clone());
            continue;
        };
        if let Some(kumihan::Block::Para(p)) = out.blocks.get_mut(bi) {
            p.anchors.push(ooxml::shape_anchor_run(sp, 9000 + i));
        } else {
            nokori.push(sp.clone());
        }
    }
    // 控えへ移した分は模型から外します(二重に書かないため)
    out.shapes = nokori;
    out
}
