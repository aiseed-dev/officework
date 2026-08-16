//! 紙面を紙へ写す — 印刷と PDF 出力。
//!
//! **組版はやり直さない。** 画面に出しているのと同じ [`kumihan::Sheet`] を、
//! 座標そのままで PDF の面に置く。だから**画面と紙が必ず一致する**
//! (別々に組み直すと、そこで食い違いが生まれる)。
//!
//! engine 側に置かないのは、engine を PDF から独立させておくため。

pub mod grid;

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
}

/// 紙の大きさ(mm)。既定は A4 縦。
#[derive(Debug, Clone, Copy)]
pub struct Paper {
    pub width_mm: f32,
    pub height_mm: f32,
    /// 左の余白。紙面の x はここからの相対
    pub margin_mm: f32,
}

impl Default for Paper {
    fn default() -> Self {
        Paper { width_mm: 210.0, height_mm: 297.0, margin_mm: 20.0 }
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
        .map(|pg| Paper { width_mm: pg.w_mm, height_mm: pg.h_mm, margin_mm: pg.left_mm })
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
            Some(pg) => Paper {
                width_mm: pg.w_mm,
                height_mm: pg.h_mm,
                margin_mm: pg.left_mm,
            },
            None => paper,
        }
    };
    let mut pages = Vec::with_capacity(sheet.lines.len());
    let mut offsets = vec![0.0f32];
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
        let mut forced = false;
        while let Some(&b) = breaks.peek() {
            if line.y_mm >= b - 0.01 {
                breaks.next();
                forced = true;
            } else {
                break;
            }
        }
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
        if forced || y_roll > cur.height_mm - cur.margin_mm - reserve {
            // 次のページへ。行の紙面上の高さは(余白ぶんを除いて)そのまま続ける
            let next = paper_at(line.y_mm);
            offsets.push(line.y_mm - next.margin_mm);
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
    Pagination { pages, offsets, papers, starts, notes }
}

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
    use kumihan::{font, layout, Align, Document, Frame, Metrics};

    use super::*;

    fn sheet(text: &str, align: Align) -> (Sheet, Vec<u8>) {
        let (fam, _) = font::for_document(None).unwrap();
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
    fn 節ごとに紙の大きさが変わる() {
        use kumihan::{Block, PageSetup, Paragraph, Run};
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let 紙 = |w: f32, h: f32| PageSetup {
            w_mm: w, h_mm: h, left_mm: 20.0, right_mm: 20.0,
            top_mm: 20.0, bottom_mm: 20.0, columns: 1,
        };
        let 段 = |t: &str, sect: Option<PageSetup>| Block::Para(Paragraph {
            runs: vec![Run { text: t.into(), size_pt: None, font: None, fmt: Default::default() }],
            line_spacing: 1.0,
            sect: sect.map(|page| kumihan::SectionBreak {
                raw: String::new(), page, continuous: false }),
            ..Default::default()
        });
        let d = Document {
            page: Some(紙(210.0, 297.0)),          // 最後の節 = 縦
            blocks: vec![
                段("縦の節の本文", None),
                段("縦の節の終わり", Some(紙(210.0, 297.0))),
                段("横の節の本文", Some(紙(297.0, 210.0))),
                段("また縦の節", None),
            ],
            ..Default::default()
        };
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 24.0 });
        assert_eq!(s.sect_pages.len(), 3, "節ごとの紙が揃っていない: {:?}", s.sect_pages);

        let papers = paginate_full(&s, Paper::default()).papers;
        let 幅: Vec<f32> = papers.iter().map(|p| p.width_mm).collect();
        // 縦 → 横 → 縦。**節の切れ目で必ず頁が割れる**ので3ページ以上になる
        assert!(papers.len() >= 3, "節の切れ目で頁が割れていない: {幅:?}");
        assert_eq!(幅[0], 210.0, "1ページ目が最初の節の紙になっていない: {幅:?}");
        assert!(幅.contains(&297.0), "横の節の紙が出ていない: {幅:?}");
        assert_eq!(*幅.last().unwrap(), 210.0, "最後の節が縦に戻っていない: {幅:?}");

        // PDF にしても落ちない(ページごとに大きさが違う紙を作る)
        let mut buf = Vec::new();
        to_pdf(&s, &data, Paper::default(), &mut buf).unwrap();
        assert_eq!(&buf[..5], b"%PDF-");
    }

    #[test]
    fn 節が一つなら折り方は今までどおり() {
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
    fn 節が変わっても字が紙の中に収まる() {
        use kumihan::{Block, PageSetup, Paragraph, Run};
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let 紙 = |w: f32, h: f32| PageSetup {
            w_mm: w, h_mm: h, left_mm: 20.0, right_mm: 20.0,
            top_mm: 20.0, bottom_mm: 20.0, columns: 1,
        };
        let 段 = |t: &str, sect: Option<PageSetup>| Block::Para(Paragraph {
            runs: vec![Run { text: t.into(), size_pt: None, font: None, fmt: Default::default() }],
            line_spacing: 1.0,
            sect: sect.map(|page| kumihan::SectionBreak {
                raw: String::new(), page, continuous: false }),
            ..Default::default()
        });
        let d = Document {
            page: Some(紙(210.0, 297.0)),
            blocks: vec![
                段("縦の節。", Some(紙(210.0, 297.0))),
                段("横の節。ここは紙が低い(210mm)ので、裏返しを間違えると外へ出る。", None),
            ],
            ..Default::default()
        };
        // 2つ目の節は Document::page(縦)なので、横は1つ目…ではない。
        // 節末に紙を置いた1段目が縦、残りが最後の節。ここでは横紙を最後に置く
        let d = Document { page: Some(紙(297.0, 210.0)), ..d };
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
    fn 一ページ目は最初の節の紙になる() {
        use kumihan::{Block, PageSetup, Paragraph, Run};
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let 紙 = |w: f32, h: f32| PageSetup {
            w_mm: w, h_mm: h, left_mm: 20.0, right_mm: 20.0,
            top_mm: 20.0, bottom_mm: 20.0, columns: 1,
        };
        let 段 = |t: &str, sect: Option<PageSetup>| Block::Para(Paragraph {
            runs: vec![Run { text: t.into(), size_pt: None, font: None, fmt: Default::default() }],
            line_spacing: 1.0,
            sect: sect.map(|page| kumihan::SectionBreak {
                raw: String::new(), page, continuous: false }),
            ..Default::default()
        });
        let d = Document {
            page: Some(紙(297.0, 210.0)),                  // 最後の節 = 横
            blocks: vec![
                段("縦の節", Some(紙(210.0, 297.0))),       // 最初の節 = 縦
                段("横の節", None),
            ],
            ..Default::default()
        };
        // 上の余白より上を引いても、最初の節が返らねばならない
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 20.0 });
        assert_eq!(s.setup_at(0.0).map(|g| (g.w_mm, g.h_mm)), Some((210.0, 297.0)),
            "巻物の頭で最初の節が引けない: {:?}", s.sect_pages);

        let 紙面 = Paper { width_mm: 297.0, height_mm: 210.0, margin_mm: 20.0 };
        let papers = paginate_full(&s, 紙面).papers;
        assert_eq!((papers[0].width_mm, papers[0].height_mm), (210.0, 297.0),
            "1ページ目が最後の節の紙で刷られた");
    }


    /// **画像・罫線の頁割りが本文とずれない。**
    ///
    /// ここは長らく本文とは別に「どの頁も同じ高さ」と決め打ちした近似を
    /// 持っていた。同じ y に居る本文の行と同じ頁に来ることを直接見る —
    /// 「PDF が出来た」だけを見る試験はこのずれを通してしまう
    /// (SEKKEI.md「緑は『正しい』ではなく『この物差しでは差が出ない』」)
    #[test]
    fn 画像と罫線の頁割りが本文と一致する() {
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
    fn 節が変わっても画像と罫線の頁割りが本文と一致する() {
        use kumihan::{Block, PageSetup, Paragraph, Run};
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let 紙 = |w: f32, h: f32| PageSetup {
            w_mm: w, h_mm: h, left_mm: 20.0, right_mm: 20.0,
            top_mm: 20.0, bottom_mm: 20.0, columns: 1,
        };
        let 段 = |t: &str, sect: Option<PageSetup>| Block::Para(Paragraph {
            runs: vec![Run { text: t.into(), size_pt: None, font: None, fmt: Default::default() }],
            line_spacing: 1.0,
            sect: sect.map(|page| kumihan::SectionBreak {
                raw: String::new(), page, continuous: false }),
            ..Default::default()
        });
        let 長文 = "いろはにほへとちりぬるを。".repeat(120);
        let d = Document {
            page: Some(紙(297.0, 210.0)),                       // 最後の節 = 横
            blocks: vec![
                段(&長文, Some(紙(210.0, 297.0))),              // 縦の節
                段(&長文, None),                                  // 横の節
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
    fn pdfになる() {
        let b = pdf_of("日本語の書類を紙にする。", Align::Left);
        assert_eq!(&b[..5], b"%PDF-", "PDF になっていない");
        assert!(b.len() > 1000, "中身が薄すぎる: {} バイト", b.len());
    }

    #[test]
    fn 画面と同じ紙面から作る() {
        // 組み直さないので、行数は紙面のまま
        let (s, data) = sheet("一行目\n二行目\n三行目", Align::Left);
        assert_eq!(s.lines.len(), 3);
        let mut buf = Vec::new();
        to_pdf(&s, &data, Paper::default(), &mut buf).unwrap();
        assert_eq!(&buf[..5], b"%PDF-");
    }

    #[test]
    fn 中央揃えが紙にも効く() {
        // 揃えは紙面の x に入っているので、PDF 側で作り直さない
        let (left, _) = sheet("表題", Align::Left);
        let (center, _) = sheet("表題", Align::Center);
        assert!(
            center.lines[0].cells[0].x_mm > left.lines[0].cells[0].x_mm,
            "中央揃えが紙面に出ていない"
        );
    }

    #[test]
    fn 空の紙面でも落ちない() {
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
    fn 長い文書は複数ページになる() {
        // A4 で本文が入るのは 40行くらい。100行が1ページに収まっていたら
        // それは「下へ黙ってはみ出している」ということ
        assert_eq!(pages(10), 1, "10行で複数ページになった");
        let p100 = pages(100);
        assert!(p100 >= 3, "100行が {p100} ページにしかならない(はみ出している)");
    }

    #[test]
    fn ページ数が行数に見合う() {
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
    fn ページ番号が各ページに載りページ数は変わらない() {
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
    fn 改ページで頁が割れる() {
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
    fn 先頭の改ページは頁を増やさない() {
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
    fn 画像が紙に埋まる() {
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
    fn 壊れた画像は飛ばして紙は出来る() {
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

    fn 組む(d: &Document) -> Sheet {
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        layout(d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 20.0 })
    }
    fn 印(id: &str) -> Run {
        Run { text: String::new(), size_pt: None, font: None,
              fmt: CharFormat { footnote: Some(FootnoteRef { id: id.into(), endnote: false }),
                                ..Default::default() } }
    }
    fn 字(t: &str) -> Run {
        Run { text: t.into(), size_pt: None, font: None, fmt: CharFormat::default() }
    }
    fn 段(runs: Vec<Run>) -> Block {
        Block::Para(Paragraph { runs, line_spacing: 1.0, ..Default::default() })
    }
    fn 注(id: &str, t: &str) -> Footnote {
        Footnote { added: false, id: id.into(), endnote: false,
                   paragraphs: vec![Paragraph { runs: vec![字(t)], line_spacing: 1.0,
                                                ..Default::default() }] }
    }

    /// **脚注は本文に使える高さを削る。** 削っていないと、脚注の上に
    /// 本文が重なって刷られる
    #[test]
    fn 脚注のある頁では本文の底が上がる() {
        let 長文 = "いろはにほへとちりぬるを。".repeat(200);
        let なし = Document {
            blocks: vec![段(vec![字(&長文)])],
            ..Default::default()
        };
        let あり = Document {
            blocks: vec![段(vec![印("9"), 字(&長文)])],
            footnotes: vec![注("9", &"脚注の文章。".repeat(20))],
            ..Default::default()
        };
        let (s1, s2) = (組む(&なし), 組む(&あり));
        let p1 = paginate_full(&s1, Paper::default());
        let p2 = paginate_full(&s2, Paper::default());
        assert!(!s2.notes.is_empty(), "脚注が組まれていない");
        let 一頁目 = |p: &Pagination| p.pages.iter().filter(|k| **k == 1).count();
        assert!(一頁目(&p2) < 一頁目(&p1),
            "脚注があるのに1頁目の本文の行数が減っていない: {} / {}",
            一頁目(&p2), 一頁目(&p1));
    }

    /// **字が紙の中に収まる。** 「脚注が出た」だけを見る試験は、
    /// 紙の外へ出ていても緑になる(SEKKEI.md の教訓)
    #[test]
    fn 脚注の字が紙の中に収まる() {
        let 長文 = "いろはにほへとちりぬるを。".repeat(200);
        let d = Document {
            blocks: vec![段(vec![印("9"), 字(&長文)])],
            footnotes: vec![注("9", "脚注の文章。")],
            ..Default::default()
        };
        let s = 組む(&d);
        let pg = paginate_full(&s, Paper::default());
        let mut 見た = 0usize;
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
                    見た += 1;
                }
                up += nb.h_mm;
            }
        }
        assert!(見た > 0, "脚注の行を1つも見ていない(試験になっていない)");
    }

    /// 脚注は**印のある頁**に出る。印が2頁目なら脚注も2頁目
    #[test]
    fn 脚注は印のある頁に出る() {
        let 長文 = "いろはにほへとちりぬるを。".repeat(200);
        let d = Document {
            blocks: vec![
                段(vec![字(&長文)]),
                段(vec![字("後ろの段落"), 印("9")]),
            ],
            footnotes: vec![注("9", "後ろの脚注。")],
            ..Default::default()
        };
        let s = 組む(&d);
        let pg = paginate_full(&s, Paper::default());
        assert!(pg.offsets.len() >= 2, "頁が足りず試験にならない");
        let 載った: Vec<usize> = pg.notes.iter().enumerate()
            .filter(|(_, v)| !v.is_empty()).map(|(k, _)| k).collect();
        assert_eq!(載った.len(), 1, "脚注が複数の頁に出た: {載った:?}");
        // 印のある行の頁と一致するか
        let at_y = s.notes[0].at_y;
        let 印の頁 = pg.page_at(at_y);
        assert_eq!(載った[0], 印の頁, "脚注が印と違う頁に出た");
        assert!(印の頁 > 0, "この試験は2頁目に印が来る形を見ている");
    }

    /// 脚注が無ければ今までどおり(頁割りは1ミリも変わらない)
    #[test]
    fn 脚注が無ければ頁割りは変わらない() {
        let 長文 = "いろはにほへとちりぬるを。".repeat(200);
        let d = Document { blocks: vec![段(vec![字(&長文)])], ..Default::default() };
        let s = 組む(&d);
        assert!(s.notes.is_empty(), "脚注が無いのに組んだ");
        let pg = paginate_full(&s, Paper::default());
        assert!(pg.notes.iter().all(|v| v.is_empty()), "脚注が無いのに頁に付いた");
    }
}

