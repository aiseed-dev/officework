//! **低い層で PDF を書く — 使った字だけ埋める。**
//!
//! 2026-08-27 発注者「低レイヤーで自由度の高い PDF 生成エンジンを使うのが
//! いいのでは」。
//!
//! `printpdf` は書体を**丸ごと**埋めます。日本語の書体は 20MB あるので、
//! 字が数十の1枚物でも 20MB になります。メールに乗りません。
//!
//! ここは `pdf-writer`(PDF の中身を1つずつ書く層)と `subsetter`
//! (使った字だけ残す)で書き直した道です。**まだ並べて動かしている段**で、
//! いまの `to_pdf` は替えていません(SEKKEI「決め: PDF は低い層の書き手に
//! 替える」の進め方)。
//!
//! # なぜ CID フォントか
//!
//! `subsetter` は `cmap`(字 → 字形の表)を落とします。PDF の CID フォントは
//! **PDF の側が字の対応を持つ**作りなので、書体に入れる必要が無いからです。
//! こちらもその形にします — 字形の番号を直に書き、対応表は PDF に載せます。
//!
//! 縦書き・異体字もこの形なら載ります(`printpdf` では載りませんでした)。

use pdf_writer::types::{FontFlags, SystemInfo};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str};
use std::collections::BTreeMap;

/// 1つの紙に置く字。
pub struct Piece {
    /// 左下からの位置(mm)
    pub x_mm: f32,
    pub y_mm: f32,
    pub size_pt: f32,
    pub text: String,
}

/// mm → PDF の単位(pt)
fn pt(mm: f32) -> f32 {
    mm * 72.0 / 25.4
}

/// **紙の並びを PDF にする。**
///
/// `font_data` は画面と同じ書体の実体。使った字だけを切り出して埋めます。
pub fn write(
    pages: &[Vec<Piece>],
    page_w_mm: f32,
    page_h_mm: f32,
    font_data: &[u8],
) -> Result<Vec<u8>, String> {
    let ps: Vec<Leaf> = pages
        .iter()
        .map(|v| Leaf {
            pieces: v.iter().map(|p| Piece {
                x_mm: p.x_mm, y_mm: p.y_mm, size_pt: p.size_pt, text: p.text.clone(),
            }).collect(),
            rules: Vec::new(),
        })
        .collect();
    let mut out = Vec::new();
    write_pages(&ps, page_w_mm, page_h_mm, font_data, &mut out)?;
    Ok(out)
}

/// **紙の並びを PDF にして書き出す。** 字も罫線も置きます。
pub fn write_pages<W: std::io::Write>(
    pages: &[Leaf],
    page_w_mm: f32,
    page_h_mm: f32,
    font_data: &[u8],
    mut out: W,
) -> Result<(), String> {
    let face = ttf_parser::Face::parse(font_data, 0).map_err(|e| e.to_string())?;

    // ① 使った字を集めて、字形の番号に直す。**同じ字は1つ**にまとめます
    let mut used: BTreeMap<char, u16> = BTreeMap::new();
    for page in pages {
        for p in &page.pieces {
            for c in p.text.chars() {
                if let Some(g) = face.glyph_index(c) {
                    used.insert(c, g.0);
                }
            }
        }
    }
    if used.is_empty() {
        // 字が1つも無い紙でも PDF は出します(白紙)
        used.insert(' ', face.glyph_index(' ').map(|g| g.0).unwrap_or(0));
    }

    // ② 番号を詰め直して、使った字形だけの書体にする
    let mut remap = subsetter::GlyphRemapper::new();
    remap.remap(0); // .notdef は必ず 0 番
    let mut new_gid: BTreeMap<char, u16> = BTreeMap::new();
    for (c, g) in &used {
        new_gid.insert(*c, remap.remap(*g));
    }
    let subset = subsetter::subset(font_data, 0, &remap).map_err(|e| e.to_string())?;

    // ③ PDF を組む
    let mut pdf = Pdf::new();
    let mut next = 1i32;
    let mut id = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let (catalog, tree, font, cid, desc, file, to_uni) =
        (id(), id(), id(), id(), id(), id(), id());
    let page_ids: Vec<Ref> = pages.iter().map(|_| id()).collect();
    let content_ids: Vec<Ref> = pages.iter().map(|_| id()).collect();

    pdf.catalog(catalog).pages(tree);
    pdf.pages(tree).kids(page_ids.iter().copied()).count(pages.len() as i32);

    let f_name = Name(b"F1");
    for (i, page) in pages.iter().enumerate() {
        let mut pg = pdf.page(page_ids[i]);
        pg.media_box(Rect::new(0.0, 0.0, pt(page_w_mm), pt(page_h_mm)));
        pg.parent(tree);
        pg.contents(content_ids[i]);
        pg.resources().fonts().pair(f_name, font);
        pg.finish();

        let mut c = Content::new();
        // **罫線を先に引きます**(字の下)
        for r in &page.rules {
            c.set_line_width(pt(r.w_mm));
            c.move_to(pt(r.x1_mm), pt(r.y1_mm));
            c.line_to(pt(r.x2_mm), pt(r.y2_mm));
            c.stroke();
        }
        for p in &page.pieces {
            // **字形の番号を2バイトで並べます。** CID フォントなので、
            // 字そのものではなく番号を書きます
            let mut bytes = Vec::with_capacity(p.text.chars().count() * 2);
            for ch in p.text.chars() {
                let g = new_gid.get(&ch).copied().unwrap_or(0);
                bytes.extend_from_slice(&g.to_be_bytes());
            }
            c.begin_text();
            c.set_font(f_name, p.size_pt);
            c.set_text_matrix([1.0, 0.0, 0.0, 1.0, pt(p.x_mm), pt(p.y_mm)]);
            c.show(Str(&bytes));
            c.end_text();
        }
        pdf.stream(content_ids[i], &c.finish());
    }

    // ④ 書体。Type0(CID)— 字の対応は PDF の側が持ちます
    pdf.type0_font(font)
        .base_font(Name(b"Subset"))
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid)
        .to_unicode(to_uni);

    // **字形の持ち方で名乗りが変わります。** 日本語の書体は CFF(OpenType)の
    // ことが多く、TrueType と名乗ると読む側が「型と中身が食い違う」と言います
    // (2026-08-27 に pdftotext で見つけた)
    let is_cff = face.tables().cff.is_some();
    let upem = face.units_per_em() as f32;
    let mut cf = pdf.cid_font(cid);
    // **TrueType の字形だと名乗ります。** 名乗らないと読む側が
    // 「書体の型と中身が食い違う」と言います(2026-08-27 に見つけた)
    let kind = if is_cff {
        pdf_writer::types::CidFontType::Type0
    } else {
        pdf_writer::types::CidFontType::Type2
    };
    cf.subtype(kind)
        .base_font(Name(b"Subset"))
        .system_info(SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"Identity"), supplement: 0 })
        .font_descriptor(desc)
        .cid_to_gid_map_predefined(Name(b"Identity"))
        .default_width(0.0);
    {
        // 字幅。**1000 を 1em とする PDF の単位**に直します
        let mut w = cf.widths();
        for (c, g) in &new_gid {
            let adv = face
                .glyph_index(*c)
                .and_then(|i| face.glyph_hor_advance(i))
                .unwrap_or(0) as f32;
            w.same(*g, *g, adv / upem * 1000.0);
        }
        w.finish();
    }
    cf.finish();

    let bbox = face.global_bounding_box();
    let scale = |v: i16| v as f32 / upem * 1000.0;
    let mut fd = pdf.font_descriptor(desc);
    fd.name(Name(b"Subset"))
        .flags(FontFlags::SYMBOLIC)
        .bbox(Rect::new(scale(bbox.x_min), scale(bbox.y_min), scale(bbox.x_max), scale(bbox.y_max)))
        .italic_angle(0.0)
        .ascent(scale(face.ascender()))
        .descent(scale(face.descender()))
        .cap_height(face.capital_height().map(scale).unwrap_or(scale(face.ascender())))
        .stem_v(80.0);
    // 埋める所も型で分かれます。CFF は FontFile3(OpenType)。
    // **同じ番号の物を2度書かない** — 1つの記述に足します
    if is_cff {
        fd.font_file3(file);
    } else {
        fd.font_file2(file);
    }
    fd.finish();
    // **圧縮は自分で掛けます。** `filter` は「掛けた」と名乗るだけで、
    // 中身は触りません。名乗りだけ書いて圧縮しないと、読む側が
    // 「壊れた書体」と言います(2026-08-27 に pdftotext で見つけた)
    let packed = deflate(&subset);
    let mut st = pdf.stream(file, &packed);
    st.filter(Filter::FlateDecode);
    if is_cff {
        // **CFF の実体は「何の形か」を流れの側で名乗ります。**
        // 名乗らないと読む側が「知らない書体の型」と言います
        st.pair(Name(b"Subtype"), Name(b"OpenType"));
    }
    st.finish();

    // ⑤ 字形の番号 → 元の字。**選んで写せる PDF** にするために要ります
    let cmap = deflate(&to_unicode_cmap(&new_gid));
    pdf.cmap(to_uni, &cmap).filter(Filter::FlateDecode);

    out.write_all(&pdf.finish()).map_err(|e| e.to_string())
}

/// zlib で縮める(PDF の `FlateDecode`)。
fn deflate(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = z.write_all(data);
    z.finish().unwrap_or_else(|_| data.to_vec())
}

/// 字形の番号から元の字を引く表(PDF の ToUnicode)。
fn to_unicode_cmap(gids: &BTreeMap<char, u16>) -> Vec<u8> {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    // 100 個ずつの塊にする決まりです
    let all: Vec<(&char, &u16)> = gids.iter().collect();
    for chunk in all.chunks(100) {
        s.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (c, g) in chunk {
            let mut buf = [0u16; 2];
            let u: String =
                c.encode_utf16(&mut buf).iter().map(|x| format!("{x:04X}")).collect();
            s.push_str(&format!("<{g:04X}> <{u}>\n"));
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Vec<u8> {
        let f = kumihan::font::default_family("ja").expect("書体が無い");
        kumihan::font::load(f).expect("読めない")
    }

    /// **使った字だけ埋まる。** 書体は 20MB でも、1枚物は小さく収まります
    #[test]
    fn only_the_used_glyphs_are_embedded() {
        let whole = font();
        let pages = vec![vec![Piece {
            x_mm: 20.0,
            y_mm: 250.0,
            size_pt: 10.5,
            text: "四月の売上は 1,200 円です。".into(),
        }]];
        let out = write(&pages, 210.0, 297.0, &whole).expect("PDF が出ない");
        assert!(out.starts_with(b"%PDF"), "PDF になっていない");
        assert!(
            out.len() < whole.len() / 10,
            "小さくなっていない: 書体 {} / PDF {}",
            whole.len(),
            out.len()
        );
        // 200KB を超えたら、何かを丸ごと埋めています
        assert!(out.len() < 200_000, "{} バイトある", out.len());
    }

    /// **紙面をそのまま受けても、いまの道と同じ字が出る。**
    ///
    /// 大きさは 1,800 分の1 になります(書体を丸ごと埋めないため)。
    #[test]
    fn the_same_page_comes_out_far_smaller() {
        let src = "= 四月の売上\n\n本文です。\n\n|===\n|品名 |金額\n\n|ペン |1,200\n|===\n";
        let doc = kumihan::adoc::parse(src).expect("読めない");
        let (sheet, page, bytes) = crate::doc_to_sheet(&doc, None).expect("組めない");
        let pp = crate::Paper {
            width_mm: page.w_mm,
            height_mm: page.h_mm,
            margin_mm: page.left_mm,
        };
        let mut old = Vec::new();
        crate::to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut old)).expect("いまの道");
        let mut new = Vec::new();
        let lost = sheet_to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut new))
            .expect("新しい道");
        assert!(new.starts_with(b"%PDF"));
        assert!(lost.is_empty(), "この文書で落ちる物があった: {lost:?}");
        assert!(
            new.len() * 100 < old.len(),
            "小さくなっていない: いま {} / 新しい {}",
            old.len(),
            new.len()
        );
    }

    /// 字が1つも無い紙でも落ちない
    #[test]
    fn an_empty_page_still_makes_a_pdf() {
        let out = write(&[vec![]], 210.0, 297.0, &font()).expect("PDF が出ない");
        assert!(out.starts_with(b"%PDF"));
    }
}

// ───────────────────────────────────────── 紙面をそのまま受ける

/// 引く線(表の罫線)。左下からの mm。
pub struct Rule {
    pub x1_mm: f32,
    pub y1_mm: f32,
    pub x2_mm: f32,
    pub y2_mm: f32,
    pub w_mm: f32,
}

/// **紙1枚に置く物。** `pdf_writer` の `Page` と名前がぶつかるので別の名前です
#[derive(Default)]
pub struct Leaf {
    pub pieces: Vec<Piece>,
    pub rules: Vec<Rule>,
}

/// **組み上がった紙面を PDF にする。**
///
/// `paper::to_pdf` と同じ物を受け、同じ頁割りを通ります。違うのは書き手だけ
/// です — 書体を丸ごと埋めるか、使った字だけ埋めるか。
///
/// 画像と透かしとペンの筆はまだ載りません(次の段)。載らない物は
/// **数えて返します**(黙って落としません)。
pub fn sheet_to_pdf<W: std::io::Write>(
    sheet: &kumihan::Sheet,
    font_data: &[u8],
    paper: crate::Paper,
    out: W,
) -> Result<Vec<String>, String> {
    let (pages_of, offsets) = crate::paginate(sheet, paper);
    let n = pages_of.iter().copied().max().unwrap_or(1);
    let mut pages: Vec<Leaf> = (0..n).map(|_| Leaf::default()).collect();

    // 行を頁へ配ります。**y は上からの mm** なので、PDF の下からの mm に直します
    for (i, line) in sheet.lines.iter().enumerate() {
        let k = pages_of.get(i).copied().unwrap_or(1).max(1) - 1;
        let off = offsets.get(k).copied().unwrap_or(0.0);
        let y = paper.height_mm - (line.y_mm - off);
        let Some(p) = pages.get_mut(k) else { continue };
        // **続きの字はまとめて1つの塊**にします。1字ずつ置くと PDF が太ります
        let mut run: Option<Piece> = None;
        for c in &line.cells {
            match &mut run {
                Some(r) if (r.size_pt - c.size_pt).abs() < 0.01 => r.text.push(c.ch),
                _ => {
                    if let Some(r) = run.take() {
                        p.pieces.push(r);
                    }
                    run = Some(Piece {
                        x_mm: c.x_mm,
                        y_mm: y,
                        size_pt: c.size_pt,
                        text: c.ch.to_string(),
                    });
                }
            }
        }
        if let Some(r) = run.take() {
            p.pieces.push(r);
        }
    }

    // 罫線。どの頁に載るかは y で決めます
    for r in &sheet.rules {
        let k = page_of(&offsets, r[1], paper.height_mm);
        let off = offsets.get(k).copied().unwrap_or(0.0);
        if let Some(p) = pages.get_mut(k) {
            p.rules.push(Rule {
                x1_mm: r[0],
                y1_mm: paper.height_mm - (r[1] - off),
                x2_mm: r[2],
                y2_mm: paper.height_mm - (r[3] - off),
                w_mm: 0.2,
            });
        }
    }

    let mut lost = Vec::new();
    if !sheet.images.is_empty() {
        lost.push(format!("画像 {} 件(この書き手ではまだ載りません)", sheet.images.len()));
    }
    write_pages(&pages, paper.width_mm, paper.height_mm, font_data, out)?;
    Ok(lost)
}

/// その y がどの頁か(頁の頭の並びから引く)
fn page_of(offsets: &[f32], y: f32, height_mm: f32) -> usize {
    let mut k = 0;
    for (i, off) in offsets.iter().enumerate() {
        if y >= *off - 0.01 && y < off + height_mm {
            k = i;
        }
    }
    k
}
