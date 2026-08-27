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

/// 1つの紙に置く字。**要る所だけ書けます**(`..Default::default()`)
#[derive(Default, Clone)]
pub struct Piece {
    /// 左下からの位置(mm)
    pub x_mm: f32,
    pub y_mm: f32,
    pub size_pt: f32,
    pub text: String,
    /// 字の色(RRGGBB)。無ければ黒
    pub color: Option<String>,
    /// この字の幅(mm)。下線と取り消し線と蛍光ペンを引くのに要ります
    pub w_mm: f32,
    pub underline: bool,
    pub strike: bool,
    /// 蛍光ペンの色(RRGGBB)
    pub highlight: Option<String>,
}

/// 絵を PDF に載せる形にする。返りは(中身, 幅, 高さ, JPEG か)。
///
/// **JPEG はそのまま**入れます(PDF が読めるので解く必要がありません)。
/// PNG は解いて RGB に並べ直し、zlib で縮めます。
fn decode(data: &[u8]) -> Option<(Vec<u8>, u32, u32, bool)> {
    let img = image::load_from_memory(data).ok()?;
    let (w, h) = (image::GenericImageView::width(&img), image::GenericImageView::height(&img));
    if data.starts_with(&[0xFF, 0xD8]) {
        // JPEG。**そのまま埋めます** — 解いて縮め直すと大きくなります
        return Some((data.to_vec(), w, h, true));
    }
    Some((deflate(img.to_rgb8().as_raw()), w, h, false))
}

/// `RRGGBB` を 0〜1 の三つ組に。読めなければ黒
fn rgb(s: &str) -> (f32, f32, f32) {
    let h = s.trim_start_matches('#');
    let v = |i: usize| {
        u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f32 / 255.0
    };
    if h.len() < 6 {
        return (0.0, 0.0, 0.0);
    }
    (v(0), v(2), v(4))
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
            pieces: v.clone(),
            ..Default::default()
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
        // 透かしの字も埋めないと、**透かしだけ豆腐**になります
        let texts = page
            .pieces
            .iter()
            .map(|p| p.text.as_str())
            .chain(page.watermark.as_deref());
        for t in texts {
            for c in t.chars() {
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

    // **画像は先に部品にします。** 同じ絵が2枚に出ても1つで済みます
    let mut img_ids: Vec<Vec<(Ref, &Image)>> = Vec::new();
    let mut img_parts: Vec<(Ref, Vec<u8>, u32, u32, bool)> = Vec::new();
    for page in pages {
        let mut on_this = Vec::new();
        for im in &page.images {
            match decode(&im.data) {
                Some((rgb, w, h, jpeg)) => {
                    let r = id();
                    img_parts.push((r, rgb, w, h, jpeg));
                    on_this.push((r, im));
                }
                // **読めない絵は数えて返します**(呼ぶ側が言う)
                None => {}
            }
        }
        img_ids.push(on_this);
    }

    let f_name = Name(b"F1");
    for (i, page) in pages.iter().enumerate() {
        let mut pg = pdf.page(page_ids[i]);
        pg.media_box(Rect::new(0.0, 0.0, pt(page_w_mm), pt(page_h_mm)));
        pg.parent(tree);
        pg.contents(content_ids[i]);
        {
            let mut res = pg.resources();
            res.fonts().pair(f_name, font);
            if !img_ids[i].is_empty() {
                let mut xo = res.x_objects();
                for (k, (r, _)) in img_ids[i].iter().enumerate() {
                    xo.pair(Name(format!("I{k}").as_bytes()), *r);
                }
                xo.finish();
            }
            res.finish();
        }
        pg.finish();

        let mut c = Content::new();
        // **紙の色はいちばん下**。全部の上に敷き直すと字が消えます
        if let Some((r, g, b)) = page.bg {
            c.set_fill_rgb(r, g, b);
            c.rect(0.0, 0.0, pt(page_w_mm), pt(page_h_mm));
            c.fill_nonzero();
        }
        // **絵はいちばん下**。字と罫線が上に載ります
        for (k, (_, im)) in img_ids[i].iter().enumerate() {
            c.save_state();
            // 置き方の行列。大きさをそのまま使います
            c.transform([pt(im.w_mm), 0.0, 0.0, pt(im.h_mm), pt(im.x_mm), pt(im.y_mm)]);
            c.x_object(Name(format!("I{k}").as_bytes()));
            c.restore_state();
        }
        // **罫線を先に引きます**(字の下)
        for r in &page.rules {
            c.set_stroke_rgb(0.0, 0.0, 0.0);
            c.set_line_width(pt(r.w_mm));
            c.move_to(pt(r.x1_mm), pt(r.y1_mm));
            c.line_to(pt(r.x2_mm), pt(r.y2_mm));
            c.stroke();
        }
        // **蛍光ペンは字の下に敷きます**(字が隠れないように)
        for p in &page.pieces {
            if let Some(h) = &p.highlight {
                let (r, g, b) = rgb(h);
                c.set_fill_rgb(r, g, b);
                // 字の高さのぶん。下に少し出して本家の見え方に寄せます
                let h_mm = p.size_pt * 25.4 / 72.0;
                c.rect(pt(p.x_mm), pt(p.y_mm - h_mm * 0.22), pt(p.w_mm), pt(h_mm));
                c.fill_nonzero();
            }
        }
        for p in &page.pieces {
            // **字形の番号を2バイトで並べます。** CID フォントなので、
            // 字そのものではなく番号を書きます
            let mut bytes = Vec::with_capacity(p.text.chars().count() * 2);
            for ch in p.text.chars() {
                let g = new_gid.get(&ch).copied().unwrap_or(0);
                bytes.extend_from_slice(&g.to_be_bytes());
            }
            let (r, g, b) = p.color.as_deref().map(rgb).unwrap_or((0.0, 0.0, 0.0));
            c.begin_text();
            c.set_fill_rgb(r, g, b);
            c.set_font(f_name, p.size_pt);
            c.set_text_matrix([1.0, 0.0, 0.0, 1.0, pt(p.x_mm), pt(p.y_mm)]);
            c.show(Str(&bytes));
            c.end_text();
            // 下線と取り消し線。**字の下に引く線**なので、字を書いた後に
            for (on, at) in [(p.underline, -0.18f32), (p.strike, 0.28)] {
                if !on || p.w_mm <= 0.0 {
                    continue;
                }
                let h_mm = p.size_pt * 25.4 / 72.0;
                let y = p.y_mm + h_mm * at;
                c.set_stroke_rgb(r, g, b);
                c.set_line_width(pt(h_mm * 0.05).max(0.3));
                c.move_to(pt(p.x_mm), pt(y));
                c.line_to(pt(p.x_mm + p.w_mm), pt(y));
                c.stroke();
            }
        }
        // **透かしは字の上**。薄い灰で斜めに置きます(本家と同じ見え方)
        if let Some(w) = &page.watermark {
            let mut bytes = Vec::with_capacity(w.chars().count() * 2);
            for ch in w.chars() {
                bytes.extend_from_slice(&new_gid.get(&ch).copied().unwrap_or(0).to_be_bytes());
            }
            let size = 60.0f32;
            // 45 度に倒して紙の真ん中あたりへ
            let (sin, cos) = (0.7071f32, 0.7071f32);
            c.begin_text();
            c.set_fill_rgb(0.85, 0.85, 0.85);
            c.set_font(f_name, size);
            c.set_text_matrix([
                cos, sin, -sin, cos,
                pt(page_w_mm) * 0.2,
                pt(page_h_mm) * 0.3,
            ]);
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

    // 画像の実体
    for (r, data, w, h, jpeg) in img_parts {
        let mut x = pdf.image_xobject(r, &data);
        x.width(w as i32)
            .height(h as i32)
            .color_space()
            .device_rgb();
        x.bits_per_component(8);
        x.filter(if jpeg { Filter::DctDecode } else { Filter::FlateDecode });
        x.finish();
    }

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
                ..Default::default()
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

    /// **色と飾りが PDF に出る。** 事務の書類は見出しの色と下線が要ります
    #[test]
    fn colour_and_decorations_reach_the_paper() {
        let pages = vec![vec![
            Piece {
                x_mm: 20.0, y_mm: 250.0, size_pt: 12.0, w_mm: 30.0,
                text: "赤い見出し".into(),
                color: Some("CC0000".into()),
                ..Default::default()
            },
            Piece {
                x_mm: 20.0, y_mm: 240.0, size_pt: 10.5, w_mm: 24.0,
                text: "下線と蛍光".into(),
                underline: true,
                highlight: Some("FFFF00".into()),
                ..Default::default()
            },
        ]];
        let f = font();
        let out = write(&pages, 210.0, 297.0, &f).expect("PDF が出ない");
        let body = unpack(&out);
        // 字の色(rg)・線(RG)・塗り(蛍光ペン)が出ていること
        assert!(body.contains("rg"), "色の命令が無い");
        assert!(body.contains(" RG"), "線の色が無い");
        assert!(body.contains(" re"), "蛍光ペンの四角が無い");
    }

    /// 流れを解いて中の命令を字にする(実物を見るため)
    fn unpack(pdf: &[u8]) -> String {
        let mut out = String::new();
        let mut from = 0;
        while let Some(i) = pdf[from..].windows(6).position(|w| w == b"stream") {
            let a = from + i + 6;
            let a = a + pdf[a..].iter().take_while(|c| **c == b'\r' || **c == b'\n').count();
            let Some(j) = pdf[a..].windows(9).position(|w| w == b"endstream") else { break };
            if let Ok(s) = std::str::from_utf8(&pdf[a..a + j]) {
                out.push_str(s);
            }
            from = a + j;
        }
        out
    }

    /// **絵が紙に載る。** PNG は解いて並べ直し、JPEG はそのまま埋めます
    #[test]
    fn a_picture_reaches_the_paper() {
        let mut img = image::RgbImage::new(8, 6);
        img.put_pixel(0, 0, image::Rgb([220, 40, 40]));
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut png, image::ImageOutputFormat::Png)
            .expect("PNG");
        let leaf = Leaf {
            pieces: vec![],
            rules: vec![],
            images: vec![Image {
                x_mm: 20.0, y_mm: 200.0, w_mm: 40.0, h_mm: 30.0,
                data: std::sync::Arc::new(png.into_inner()),
            }],
            ..Default::default()
        };
        let mut out = Vec::new();
        write_pages(&[leaf], 210.0, 297.0, &font(), &mut out).expect("PDF が出ない");
        assert!(out.starts_with(b"%PDF"));
        let body = String::from_utf8_lossy(&out);
        assert!(body.contains("/Image"), "絵の部品が無い");
        assert!(body.contains("/DeviceRGB"), "色の指定が無い");
        assert!(unpack(&out).contains("/I0 Do"), "絵を置く命令が無い");
    }

    /// **読めない絵は数えて返す。** 黙って落としません
    #[test]
    fn a_broken_picture_is_counted_not_dropped() {
        let mut sheet = kumihan::Sheet::default();
        sheet.images.push((std::sync::Arc::new(vec![0u8, 1, 2, 3]), [10.0, 10.0, 20.0, 20.0]));
        let pp = crate::Paper { width_mm: 210.0, height_mm: 297.0, margin_mm: 20.0 };
        let mut out = Vec::new();
        let lost = sheet_to_pdf(&sheet, &font(), pp, std::io::Cursor::new(&mut out))
            .expect("PDF が出ない");
        assert!(lost.iter().any(|s| s.contains("読めない画像")), "数えていない: {lost:?}");
    }

    /// **ヘッダーと透かしと紙の色が出る。** 差し替えに要る飾りです
    #[test]
    fn the_page_dress_and_the_header_reach_the_paper() {
        let doc = kumihan::adoc::parse("= 題\n\n本文です。\n").expect("読めない");
        let (sheet, page, bytes) = crate::doc_to_sheet(&doc, None).expect("組めない");
        let pp = crate::Paper {
            width_mm: page.w_mm, height_mm: page.h_mm, margin_mm: page.left_mm,
        };
        let dress = crate::PageDress {
            bg: Some((0.98, 0.98, 0.94)),
            watermark: Some("見本".into()),
            ink: Vec::new(),
        };
        // ヘッダーの行を1つ作って渡します(組む側が字にして寄越す約束)
        let hf = |_k: usize| {
            vec![kumihan::Line {
                y_mm: 12.0,
                cells: "第1頁"
                    .chars()
                    .enumerate()
                    .map(|(i, ch)| kumihan::Cell {
                        ch,
                        x_mm: 20.0 + i as f32 * 3.5,
                        w_mm: 3.5,
                        size_pt: 9.0,
                        off: 0,
                        fmt: kumihan::CharFormat::default(),
                        font: None,
                    })
                    .collect(),
                from_body: false,
                byte0: 0,
                cell: None,
            }]
        };
        let mut out = Vec::new();
        let lost = sheet_to_pdf_with(&sheet, &bytes, pp, &dress, hf, std::io::Cursor::new(&mut out))
            .expect("PDF が出ない");
        assert!(lost.is_empty(), "落ちた物がある: {lost:?}");
        let body = unpack(&out);
        assert!(body.contains(" re"), "紙の色の四角が無い");
        // 透かしは倒して置くので、行列に 0.7071 が出ます
        assert!(body.contains("0.7071"), "透かしが斜めに置かれていない");
    }

    /// **ペンの筆はまだ載りません。** 数えて返します(黙って落としません)
    #[test]
    fn pen_strokes_are_counted_not_dropped() {
        let sheet = kumihan::Sheet::default();
        let pp = crate::Paper { width_mm: 210.0, height_mm: 297.0, margin_mm: 20.0 };
        let dress = crate::PageDress {
            bg: None,
            watermark: None,
            ink: vec![kumihan::Stroke::default()],
        };
        let mut out = Vec::new();
        let lost =
            sheet_to_pdf_with(&sheet, &font(), pp, &dress, |_| Vec::new(), std::io::Cursor::new(&mut out))
                .expect("PDF が出ない");
        assert!(lost.iter().any(|s| s.contains("ペンの筆")), "数えていない: {lost:?}");
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
    pub images: Vec<Image>,
    /// 紙の色(0〜1 の RGB)
    pub bg: Option<(f32, f32, f32)>,
    /// 透かし(斜めの薄い字)
    pub watermark: Option<String>,
}

/// 紙に置く画像。左下からの mm。
pub struct Image {
    pub x_mm: f32,
    pub y_mm: f32,
    pub w_mm: f32,
    pub h_mm: f32,
    /// PNG か JPEG の実体
    pub data: std::sync::Arc<Vec<u8>>,
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
    sheet_to_pdf_with(sheet, font_data, paper, &crate::PageDress::default(), |_| Vec::new(), out)
}

/// **紙面を PDF にする(ページごとの飾りつき)。**
///
/// `page_decor(k)` は k ページ目(1始まり)に置く行 — ヘッダーとフッターです。
/// `kumihan::layout_hf` がこの形で返します。ページ番号は組む側が字にして
/// 寄越すので、ここでは**置くだけ**です([`crate::to_pdf_with`] と同じ約束)。
pub fn sheet_to_pdf_with<W: std::io::Write, F: Fn(usize) -> Vec<kumihan::Line>>(
    sheet: &kumihan::Sheet,
    font_data: &[u8],
    paper: crate::Paper,
    dress: &crate::PageDress,
    page_decor: F,
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
                ..Default::default()
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

    // 画像。どの頁に載るかは上端の y で決めます
    let mut lost = Vec::new();
    let mut bad = 0;
    for (data, at) in &sheet.images {
        let k = page_of(&offsets, at[1], paper.height_mm);
        let off = offsets.get(k).copied().unwrap_or(0.0);
        if image::load_from_memory(data).is_err() {
            bad += 1;
            continue;
        }
        if let Some(p) = pages.get_mut(k) {
            p.images.push(Image {
                x_mm: at[0],
                // 紙面は上端の y。PDF は左下からなので、高さのぶん下げます
                y_mm: paper.height_mm - (at[1] - off) - at[3],
                w_mm: at[2],
                h_mm: at[3],
                data: data.clone(),
            });
        }
    }
    if bad > 0 {
        lost.push(format!("読めない画像 {bad} 件"));
    }

    // 紙の飾りと、ページごとのヘッダー・フッター
    for (k, p) in pages.iter_mut().enumerate() {
        p.bg = dress.bg;
        p.watermark = dress.watermark.clone();
        // **飾りの行の y はページの上端からの mm**(巻物の座標ではありません)
        for line in page_decor(k + 1) {
            for c in &line.cells {
                p.pieces.push(Piece {
                    x_mm: c.x_mm,
                    y_mm: paper.height_mm - line.y_mm,
                    size_pt: c.size_pt,
                    text: c.ch.to_string(),
                    color: c.fmt.color.clone(),
                    w_mm: c.w_mm,
                    underline: c.fmt.underline,
                    strike: c.fmt.strike,
                    highlight: c.fmt.highlight.clone(),
                });
            }
        }
    }
    if !dress.ink.is_empty() {
        lost.push(format!("ペンの筆 {} 本(この書き手ではまだ載りません)", dress.ink.len()));
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
