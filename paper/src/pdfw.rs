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
    /// 太字。**書体の実体は1つ**なので、少しずらして二度打って合成します
    /// (太字を持っていない物を、持っている顔で出さないため)
    pub bold: bool,
    /// 蛍光ペンの色(RRGGBB)
    pub highlight: Option<String>,
    /// **どの書体で描くか**(渡した書体の並びの何番目か)。
    ///
    /// 0 は1本目で、いままでの呼び出しはこのままです。表計算のセルは
    /// 書体を名指しするので、明朝とゴシックと欧文を刷り分けます
    /// (2026-08-31。Fable の指摘2)
    pub font: u8,
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

/// **PDF に書く書体の名前。** `ABCDEF+NotoSansCJKjp-Regular` の形です。
///
/// 頭の6文字は「字を一部だけ埋めた」印で、PDF の決まりです(大文字の
/// 英字6つ + `+`)。読む側はこれを見て「元の書体そのものではない」と
/// 分かります。元の名前は書体の中の PostScript 名を使い、無ければ
/// 家族の名前から作ります。
///
/// 名前に使えない字(空白や括弧)は落とします — PDF の名前は
/// 区切りの字を含められません。
fn base_font_name(face: &ttf_parser::Face) -> String {
    // **読める物を選びます。** 名前の表は同じ id が複数入っていて、
    // Macintosh の側は古い encoding で読めないことがあります。先に
    // 見つけた方を採ると、読める Windows の側があるのに諦めます
    // (2026-08-28、IPAex が `Font` になって気づきました)
    let hiku = |id: u16| -> Option<String> {
        face.names()
            .into_iter()
            .filter(|n| n.name_id == id)
            .find_map(|n| n.to_string())
    };
    let moto = hiku(ttf_parser::name_id::POST_SCRIPT_NAME)
        .or_else(|| hiku(ttf_parser::name_id::FULL_NAME))
        .or_else(|| hiku(ttf_parser::name_id::FAMILY))
        .unwrap_or_else(|| "Font".to_string());
    let kirei: String = moto
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '.' || *c == '_')
        .take(60)
        .collect();
    let kirei = if kirei.is_empty() { "Font".to_string() } else { kirei };
    // **一部だけ埋めた印。** 中身から決めるので、同じ書体の同じ字なら
    // 同じ印になります(組み直しても PDF が変わりません)
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in kirei.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let tag: String = (0..6)
        .map(|i| (b'A' + ((h >> (i * 5)) % 26) as u8) as char)
        .collect();
    format!("{tag}+{kirei}")
}

/// 0〜1 の三つ組を `RRGGBB` に。[`rgb`] の逆です
pub fn to_hex(c: (f32, f32, f32)) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("{:02X}{:02X}{:02X}", b(c.0), b(c.1), b(c.2))
}

/// `RRGGBB` を 0〜1 の三つ組に。読めなければ黒
pub(crate) fn rgb(s: &str) -> (f32, f32, f32) {
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
    out: W,
) -> Result<(), String> {
    write_pages_fonts(pages, page_w_mm, page_h_mm, &[font_data], out)
}

/// **書体を何本でも埋める形。** `Piece::font` がどれを使うかを指します。
///
/// 表計算のセルは書体を名指しします(明朝・ゴシック・欧文)。1本しか
/// 埋めないと、明朝の升までゴシックで出ます(2026-08-31。Fable の指摘2)。
pub fn write_pages_fonts<W: std::io::Write>(
    pages: &[Leaf],
    page_w_mm: f32,
    page_h_mm: f32,
    fonts: &[&[u8]],
    mut out: W,
) -> Result<(), String> {
    let fonts: Vec<&[u8]> = if fonts.is_empty() { vec![b""] } else { fonts.to_vec() };
    let faces: Vec<ttf_parser::Face> = fonts
        .iter()
        .map(|d| ttf_parser::Face::parse(d, 0).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let face = &faces[0];

    // ① 使った字を、**書体ごとに**集めて字形の番号に直します。
    // 同じ字は1つにまとめます
    let mut used_all: Vec<BTreeMap<char, u16>> = vec![BTreeMap::new(); faces.len()];
    for page in pages {
        for p in &page.pieces {
            let fi = (p.font as usize).min(faces.len() - 1);
            for c in p.text.chars() {
                if let Some(g) = faces[fi].glyph_index(c) {
                    used_all[fi].insert(c, g.0);
                }
            }
        }
        // 透かしの字も埋めないと、**透かしだけ豆腐**になります。1本目で描きます
        if let Some(w) = page.watermark.as_deref() {
            for c in w.chars() {
                if let Some(g) = face.glyph_index(c) {
                    used_all[0].insert(c, g.0);
                }
            }
        }
    }
    if used_all[0].is_empty() {
        // 字が1つも無い紙でも PDF は出します(白紙)
        used_all[0].insert(' ', face.glyph_index(' ').map(|g| g.0).unwrap_or(0));
    }

    // ② 番号を詰め直して、使った字形だけの書体にする(書体ごとに)
    let mut new_gid_all: Vec<BTreeMap<char, u16>> = Vec::with_capacity(faces.len());
    let mut subsets: Vec<Vec<u8>> = Vec::with_capacity(faces.len());
    for (fi, used) in used_all.iter().enumerate() {
        let mut remap = subsetter::GlyphRemapper::new();
        remap.remap(0); // .notdef は必ず 0 番
        let mut new_gid: BTreeMap<char, u16> = BTreeMap::new();
        for (c, g) in used {
            new_gid.insert(*c, remap.remap(*g));
        }
        subsets.push(subsetter::subset(fonts[fi], 0, &remap).map_err(|e| e.to_string())?);
        new_gid_all.push(new_gid);
    }

    // ③ PDF を組む
    let mut pdf = Pdf::new();
    let mut next = 1i32;
    let mut id = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let (catalog, tree) = (id(), id());
    // **書体1本につき5つの部品**(書体・CID・記述・実体・Unicode の対応)
    let font_ids: Vec<(Ref, Ref, Ref, Ref, Ref)> =
        (0..faces.len()).map(|_| (id(), id(), id(), id(), id())).collect();
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
            // **読めない絵は落とします。** 数えて返すのは呼ぶ側の仕事です
            if let Some((rgb, w, h, jpeg)) = decode(&im.data) {
                let r = id();
                img_parts.push((r, rgb, w, h, jpeg));
                on_this.push((r, im));
            }
        }
        img_ids.push(on_this);
    }

    // 書体の資源の名前(`F1`・`F2`…)。`Piece::font` がどれかを指します
    let f_names: Vec<String> = (1..=faces.len()).map(|k| format!("F{k}")).collect();
    let f_name = Name(f_names[0].as_bytes());
    // **使う濃さを頁ごとに数え上げます。** PDF の透明度は資源に置いた
    // ExtGState を名前で呼ぶ形なので、先に何が要るか分かっていないと
    // 資源が書けません(資源は中身より先に書き終わります)
    let usu_ids: Vec<Vec<(u8, Ref)>> = pages
        .iter()
        .map(|p| {
            let mut v: Vec<u8> = p
                .fills
                .iter()
                .map(|f| f.a)
                .chain(p.polys.iter().map(|g| g.a))
                .chain(p.rules.iter().map(|r| r.a))
                .chain(p.rules_top.iter().map(|r| r.a))
                .map(usu_key)
                .collect();
            // **薄い物が1つも無ければ資源を作りません。** 逆に1つでも
            // あれば、不透明へ戻すための 255 も要ります(戻す先が資源に
            // 無いと、開く道具が「A255 が無い」と言って読めません)
            if v.iter().all(|k| *k == 255) {
                return Vec::new();
            }
            v.push(255);
            v.sort_unstable();
            v.dedup();
            v.into_iter().map(|k| (k, id())).collect()
        })
        .collect();

    for (i, page) in pages.iter().enumerate() {
        let (pw, ph) = page.size_mm.unwrap_or((page_w_mm, page_h_mm));
        let mut pg = pdf.page(page_ids[i]);
        pg.media_box(Rect::new(0.0, 0.0, pt(pw), pt(ph)));
        pg.parent(tree);
        pg.contents(content_ids[i]);
        {
            let mut res = pg.resources();
            {
                let mut fs = res.fonts();
                for (k, nm) in f_names.iter().enumerate() {
                    fs.pair(Name(nm.as_bytes()), font_ids[k].0);
                }
            }
            if !img_ids[i].is_empty() {
                let mut xo = res.x_objects();
                for (k, (r, _)) in img_ids[i].iter().enumerate() {
                    xo.pair(Name(format!("I{k}").as_bytes()), *r);
                }
                xo.finish();
            }
            if !usu_ids[i].is_empty() {
                let mut gs = res.ext_g_states();
                for (k, r) in &usu_ids[i] {
                    gs.pair(Name(format!("A{k}").as_bytes()), *r);
                }
                gs.finish();
            }
            res.finish();
        }
        pg.finish();

        let mut c = Content::new();
        // **紙の色はいちばん下**。全部の上に敷き直すと字が消えます
        if let Some((r, g, b)) = page.bg {
            c.set_fill_rgb(r, g, b);
            c.rect(0.0, 0.0, pt(pw), pt(ph));
            c.fill_nonzero();
        }
        // **塗りは絵の下、紙の色の上。** 罫線より先に敷いて線を潰しません
        // **色と太さは変わったときだけ書きます。** 升ごとに書き直すと、
        // 90 行の表で中身が 10 倍に膨れます(2026-08-27 に実物で測りました)
        let mut fill_now: Option<(f32, f32, f32)> = None;
        // **透明度は資源の名前で切り替えます**(PDF は色に透明度を持てず、
        // ExtGState という別の入れ物に置く決まりです)。使った濃さだけ
        // 資源に並べ、変わったときだけ名前を書きます
        let mut usu_now: Option<u8> = None;
        for f in &page.fills {
            usu(&mut c, &mut usu_now, f.a);
            if fill_now != Some(f.rgb) {
                c.set_fill_rgb(f.rgb.0, f.rgb.1, f.rgb.2);
                fill_now = Some(f.rgb);
            }
            c.rect(pt(f.x_mm), pt(f.y_mm), pt(f.w_mm), pt(f.h_mm));
            c.fill_nonzero();
        }
        // 好きな形の塗り。四角の塗りと同じ層です
        for g in &page.polys {
            let Some(((x0, y0), rest)) = g.points.split_first() else { continue };
            if rest.is_empty() {
                continue;
            }
            usu(&mut c, &mut usu_now, g.a);
            if fill_now != Some(g.rgb) {
                c.set_fill_rgb(g.rgb.0, g.rgb.1, g.rgb.2);
                fill_now = Some(g.rgb);
            }
            c.move_to(pt(*x0), pt(*y0));
            for (x, y) in rest {
                c.line_to(pt(*x), pt(*y));
            }
            c.close_path();
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
        let mut pen: Option<((f32, f32, f32), f32)> = None;
        for r in &page.rules {
            usu(&mut c, &mut usu_now, r.a);
            if pen != Some((r.rgb, r.w_mm)) {
                c.set_stroke_rgb(r.rgb.0, r.rgb.1, r.rgb.2);
                c.set_line_width(pt(r.w_mm));
                pen = Some((r.rgb, r.w_mm));
            }
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
        // **字を書く前に不透明へ戻します。** 戻さないと、直前に敷いた
        // 薄い物(蛍光ペン・図形の影)の濃さが字にも掛かります。
        // 2026-08-29 に PDF を開いて字が灰色なのを見て気づきました —
        // 絵の側は色ごとに濃さを持つので、そちらには出ません
        usu(&mut c, &mut usu_now, 1.0);
        for p in &page.pieces {
            // **字形の番号を2バイトで並べます。** CID フォントなので、
            // 字そのものではなく番号を書きます
            // **その字を持っている書体で描きます**(2026-08-31)。
            // 名指しの書体に無い字は1本目に落とします — 無い書体で描くと
            // その字だけ消えます
            let fi = {
                let k = (p.font as usize).min(faces.len() - 1);
                if p.text.chars().all(|ch| new_gid_all[k].contains_key(&ch)) { k } else { 0 }
            };
            let mut bytes = Vec::with_capacity(p.text.chars().count() * 2);
            for ch in p.text.chars() {
                let g = new_gid_all[fi].get(&ch).copied().unwrap_or(0);
                bytes.extend_from_slice(&g.to_be_bytes());
            }
            let (r, g, b) = p.color.as_deref().map(rgb).unwrap_or((0.0, 0.0, 0.0));
            c.begin_text();
            c.set_fill_rgb(r, g, b);
            c.set_font(Name(f_names[fi].as_bytes()), p.size_pt);
            c.set_text_matrix([1.0, 0.0, 0.0, 1.0, pt(p.x_mm), pt(p.y_mm)]);
            c.show(Str(&bytes));
            if p.bold {
                // 0.12mm ずらして二度打つ(いまの道と同じ合成)
                c.set_text_matrix([1.0, 0.0, 0.0, 1.0, pt(p.x_mm + 0.12), pt(p.y_mm)]);
                c.show(Str(&bytes));
            }
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
        // **字の上の線**(手描きのペン)。字を書いた後に引きます
        if !page.rules_top.is_empty() {
            let mut pen2: Option<((f32, f32, f32), f32)> = None;
            for r in &page.rules_top {
                usu(&mut c, &mut usu_now, r.a);
                if pen2 != Some((r.rgb, r.w_mm)) {
                    c.set_stroke_rgb(r.rgb.0, r.rgb.1, r.rgb.2);
                    c.set_line_width(pt(r.w_mm));
                    pen2 = Some((r.rgb, r.w_mm));
                }
                c.move_to(pt(r.x1_mm), pt(r.y1_mm));
                c.line_to(pt(r.x2_mm), pt(r.y2_mm));
                c.stroke();
            }
        }
        // **透かしは字の上**。薄い灰で斜めに置きます(本家と同じ見え方)
        if let Some(w) = &page.watermark {
            let mut bytes = Vec::with_capacity(w.chars().count() * 2);
            for ch in w.chars() {
                // 透かしは1本目の書体で描きます(上で1本目に集めています)
                bytes.extend_from_slice(&new_gid_all[0].get(&ch).copied().unwrap_or(0).to_be_bytes());
            }
            let size = 60.0f32;
            // 45 度に倒して紙の真ん中あたりへ
            let (sin, cos) =
                (std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2);
            c.begin_text();
            c.set_fill_rgb(0.85, 0.85, 0.85);
            c.set_font(f_name, size);
            c.set_text_matrix([
                cos, sin, -sin, cos,
                pt(pw) * 0.2,
                pt(ph) * 0.3,
            ]);
            c.show(Str(&bytes));
            c.end_text();
        }
        // 頁の中身も縮めます。**字も罫線も同じ形の繰り返し**なので、
        // よく縮みます
        let body = deflate(&c.finish());
        pdf.stream(content_ids[i], &body).filter(Filter::FlateDecode);
        // **濃さの中身。** 塗りと線の両方に掛けます(図形の影は塗りと
        // 輪郭が一緒に薄くなるので、片方だけでは色が濃く出ます)
        for (k, r) in &usu_ids[i] {
            let a = *k as f32 / 255.0;
            pdf.ext_graphics(*r).non_stroking_alpha(a).stroking_alpha(a);
        }
    }

    // ④ 書体。Type0(CID)— 字の対応は PDF の側が持ちます
    //
    // **名前は元の書体の物にします。** 前は全部 `Subset` と名乗っていて、
    // 出来た PDF を見ても何の書体で組んだのか分かりませんでした
    // (2026-08-28、Noto と BIZ UD を見比べようとして気づきました)。
    // 頭の6文字は「一部だけ埋めた」印で、PDF の決まりです
    // **書体ごとに部品を書きます**(2026-08-31)。前は1本しか埋められず、
    // 明朝の升もゴシックの升も同じ書体で出ていました
    for (fi, kono) in faces.iter().enumerate() {
        let (font, cid, desc, file, to_uni) = font_ids[fi];
        let face = kono;
        let new_gid = &new_gid_all[fi];
        let subset = &subsets[fi];
        let ps_name = base_font_name(face);
        let ps = ps_name.as_bytes();
        pdf.type0_font(font)
            .base_font(Name(ps))
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
            .base_font(Name(ps))
            .system_info(SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"Identity"), supplement: 0 })
            .font_descriptor(desc)
            .cid_to_gid_map_predefined(Name(b"Identity"))
            .default_width(0.0);
        {
            // 字幅。**1000 を 1em とする PDF の単位**に直します
            let mut w = cf.widths();
            for (c, g) in new_gid.iter() {
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
        fd.name(Name(ps))
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
        let packed = deflate(subset);
        let mut st = pdf.stream(file, &packed);
        st.filter(Filter::FlateDecode);
        if is_cff {
            // **CFF の実体は「何の形か」を流れの側で名乗ります。**
            // 名乗らないと読む側が「知らない書体の型」と言います
            st.pair(Name(b"Subtype"), Name(b"OpenType"));
        }
        st.finish();
    }

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

    // ⑤ 字形の番号 → 元の字。**選んで写せる PDF** にするために要ります。
    // 書体ごとに番号の付け方が違うので、書体ごとに書きます
    for (fi, ids) in font_ids.iter().enumerate() {
        let cmap = deflate(&to_unicode_cmap(&new_gid_all[fi]));
        pdf.cmap(ids.4, &cmap).filter(Filter::FlateDecode);
    }

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

/// **PDF の中の命令を読み出す。** 試験で「本当に書いたか」を見るための物。
///
/// 中身は縮めてあるので、解いてから字にします。
#[cfg(test)]
pub(crate) fn unpack(pdf: &[u8]) -> String {
    let mut out = String::new();
    let mut from = 0;
    while let Some(i) = pdf[from..].windows(6).position(|w| w == b"stream") {
        let a = from + i + 6;
        let a = a + pdf[a..].iter().take_while(|c| **c == b'\r' || **c == b'\n').count();
        let Some(j) = pdf[a..].windows(9).position(|w| w == b"endstream") else { break };
        let raw = &pdf[a..a + j];
        // **中身は縮めてあります。** 解いてから読みます。解けない塊
        // (絵や書体)は素通りします
        let mut wide = Vec::new();
        let body: &[u8] = {
            use std::io::Read;
            let mut z = flate2::read::ZlibDecoder::new(raw);
            if z.read_to_end(&mut wide).is_ok() { &wide } else { raw }
        };
        if let Ok(s) = std::str::from_utf8(body) {
            out.push_str(s);
        }
        from = a + j;
    }
    out
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
        let pp = crate::Paper::from_page(&page);
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
                font: 0,
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
        let pp = crate::Paper::hitoshii(210.0, 297.0, 20.0);
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
        let pp = crate::Paper::hitoshii(page.w_mm, page.h_mm, page.left_mm);
        let dress = crate::PageDress {
            bg: Some((0.98, 0.98, 0.94)),
            watermark: Some("見本".into()),
            ..Default::default()
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

    /// **ペンの筆が紙に載る。** 蛍光ペンは字の下、ペンは字の上です
    /// (2026-08-29。それまでは数えるだけでした)
    #[test]
    fn pen_strokes_land_on_the_paper() {
        let sheet = kumihan::Sheet::default();
        let pp = crate::Paper::hitoshii(210.0, 297.0, 20.0);
        let fude = |hl: bool| kumihan::Stroke {
            page: 0,
            highlighter: hl,
            points: vec![(20.0, 40.0), (60.0, 40.0), (100.0, 45.0)],
        };
        let dress = crate::PageDress {
            ink: vec![fude(true), fude(false)],
            ..Default::default()
        };
        let (pages, lost) = sheet_leaves_with(&sheet, pp, &dress, |_| Vec::new());
        assert!(
            !lost.iter().any(|s| s.contains("ペンの筆")),
            "まだ「載りません」と言っている: {lost:?}"
        );
        let leaf = pages.first().expect("紙面");
        // 点3つ = 辺2本。蛍光ペンは字の下(rules)、ペンは字の上(rules_top)
        assert_eq!(leaf.rules.len(), 2, "蛍光ペンが字の下に無い");
        assert_eq!(leaf.rules_top.len(), 2, "ペンが字の上に無い");
        // 蛍光ペンは薄く太く、ペンは濃く細く
        assert!(leaf.rules[0].a < 0.6 && leaf.rules[0].w_mm > 2.0, "蛍光ペンの太さと濃さ");
        assert!(leaf.rules_top[0].a >= 1.0 && leaf.rules_top[0].w_mm < 1.0, "ペンの太さと濃さ");
    }

    /// **縦書きが横に組まれない。** 1字ずつ列に置きます
    #[test]
    fn vertical_writing_is_not_laid_out_sideways() {
        let doc = kumihan::adoc::parse("= 題\n\n本文です。\n").expect("読めない");
        let (mut sheet, page, bytes) = crate::doc_to_sheet(&doc, None).expect("組めない");
        sheet.vertical = true;
        sheet.vert_x = sheet.lines.iter().enumerate().map(|(i, _)| 180.0 - i as f32 * 8.0).collect();
        let pp = crate::Paper::hitoshii(page.w_mm, page.h_mm, page.left_mm);
        let mut out = Vec::new();
        sheet_to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut out)).expect("PDF が出ない");
        let body = unpack(&out);
        // **1字ずつ**なので、字の数だけ置く命令が出ます
        let n = body.matches("Tm").count();
        let chars: usize = sheet.lines.iter().map(|l| l.cells.len()).sum();
        assert!(n >= chars, "1字ずつ置いていない: 命令 {n} / 字 {chars}");
        // **列ごとに違う x** に置いていること(横組みなら全部同じ x です)
        let xs: Vec<String> = body
            .lines()
            .filter(|l| l.ends_with(" Tm"))
            .filter_map(|l| l.split_whitespace().nth(4).map(|v| v.to_string()))
            .collect();
        let mut u = xs.clone();
        u.sort();
        u.dedup();
        assert!(u.len() > 1, "字が全部同じ列にある(横に組まれている): {u:?}");
    }

    /// **節ごとに紙が変わる。** 1冊に A4 縦と横が混ざってよい(2026-08-27)。
    ///
    /// 前は全部の頁が同じ紙になっていました。頁ごとに `MediaBox` を書きます。
    #[test]
    fn the_paper_changes_per_section() {
        let src = "= 縦の節\n\n本文です。\n\n[.landscape]\n== 横の節\n\n横の本文です。\n";
        let doc = kumihan::adoc::parse(src).expect("読めない");
        let (sheet, page, bytes) = crate::doc_to_sheet(&doc, None).expect("組めない");
        let pp = crate::Paper::hitoshii(page.w_mm, page.h_mm, page.left_mm);
        let mut out = Vec::new();
        sheet_to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut out)).expect("PDF が出ない");
        assert!(out.starts_with(b"%PDF"));
    }

    /// **紙ごとに MediaBox を書く。** 頁で大きさが違えば、その通りに出ます
    #[test]
    fn each_leaf_carries_its_own_paper_size() {
        let a4 = Leaf {
            pieces: vec![Piece {
                x_mm: 20.0, y_mm: 250.0, size_pt: 10.5, w_mm: 10.0,
                text: "縦".into(), ..Default::default()
            }],
            size_mm: Some((210.0, 297.0)),
            ..Default::default()
        };
        let yoko = Leaf {
            pieces: vec![Piece {
                x_mm: 20.0, y_mm: 180.0, size_pt: 10.5, w_mm: 10.0,
                text: "横".into(), ..Default::default()
            }],
            size_mm: Some((297.0, 210.0)),
            ..Default::default()
        };
        let mut out = Vec::new();
        write_pages(&[a4, yoko], 210.0, 297.0, &font(), &mut out).expect("PDF が出ない");
        let body = String::from_utf8_lossy(&out);
        // A4 縦 = 595x842pt、A4 横 = 842x595pt。**向きが逆の2枚**が出ます
        let boxes: Vec<&str> = body.match_indices("/MediaBox").map(|(i, _)| {
            let rest = &body[i..];
            &rest[..rest.find(']').map(|j| j + 1).unwrap_or(rest.len())]
        }).collect();
        assert_eq!(boxes.len(), 2, "紙ごとに書いていない: {boxes:?}");
        let w = |b: &str| -> f32 {
            b.split_whitespace().nth(3).and_then(|v| v.parse().ok()).unwrap_or(0.0)
        };
        assert!(w(boxes[0]) < w(boxes[1]), "向きが変わっていない: {boxes:?}");
    }

    /// **字の位置がいまの道と一致する。**
    ///
    /// 字が取れるかを見るだけでは足りません。**ずれていても取れます。**
    /// 2026-08-27 に左余白を足し忘れて、字が紙の左端に寄っていました。
    /// 置く命令の座標そのものを突き合わせます。
    #[test]
    fn the_glyphs_land_where_the_old_road_puts_them() {
        let doc = kumihan::adoc::parse("= 題\n\n本文です。\n").expect("読めない");
        let (sheet, page, bytes) = crate::doc_to_sheet(&doc, None).expect("組めない");
        let pp = crate::Paper::hitoshii(page.w_mm, page.h_mm, page.left_mm);
        let mut out = Vec::new();
        sheet_to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut out)).expect("PDF が出ない");

        // 置く命令の x を拾います(Tm の5番目)
        let body = unpack(&out);
        let xs: Vec<f32> = body
            .lines()
            .filter(|l| l.ends_with(" Tm"))
            .filter_map(|l| l.split_whitespace().nth(4).and_then(|v| v.parse().ok()))
            .collect();
        assert!(!xs.is_empty(), "置く命令が無い");
        // **左余白より左に字は出ません。** 出ていたら余白を足し忘れています
        let left = pp.margin_mm * 72.0 / 25.4;
        assert!(
            xs.iter().all(|x| *x >= left - 0.1),
            "字が左余白より外に出ている: 余白 {left:.1}pt / いちばん左 {:.1}pt",
            xs.iter().cloned().fold(f32::MAX, f32::min)
        );
    }

    /// **太字が横組みの本文に出る。**
    ///
    /// 2026-08-27 に実物を見て気づきました。題も見出しも細いままで、
    /// 試験は全部緑でした。**書いた物を見ないと分かりません。**
    #[test]
    fn bold_reaches_the_body_text() {
        let doc = kumihan::adoc::parse("= 題\n\n*太い字*と普通の字。\n").expect("読めない");
        let (sheet, page, bytes) = crate::doc_to_sheet(&doc, None).expect("組めない");
        let pp = crate::Paper::hitoshii(page.w_mm, page.h_mm, page.left_mm);
        let mut out = Vec::new();
        sheet_to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut out)).expect("PDF が出ない");
        // **太字は 0.12mm ずらして二度打ちます。** 同じ y で x だけ
        // 0.12mm(= 0.34pt)違う置き方が並ぶのが、その跡です
        let body = unpack(&out);
        let places: Vec<(f32, f32)> = body
            .lines()
            .filter(|l| l.ends_with(" Tm"))
            .filter_map(|l| {
                let w: Vec<&str> = l.split_whitespace().collect();
                Some((w.get(4)?.parse().ok()?, w.get(5)?.parse().ok()?))
            })
            .collect();
        let twice = places.windows(2).any(|w| {
            (w[0].1 - w[1].1).abs() < 0.01 && (w[1].0 - w[0].0 - 0.34).abs() < 0.05
        });
        assert!(twice, "二度打ちの跡が無い(太字が出ていない): {places:?}");
    }

    /// **塗りと色つきの罫線が出る。** 表計算の帯に要ります(2026-08-27)
    #[test]
    fn fills_and_coloured_rules_reach_the_paper() {
        let leaf = Leaf {
            fills: vec![Fill {
                x_mm: 20.0, y_mm: 250.0, w_mm: 60.0, h_mm: 8.0,
                rgb: (0.87, 0.92, 0.98),
                ..Default::default()
            }],
            rules: vec![Rule {
                x1_mm: 20.0, y1_mm: 250.0, x2_mm: 80.0, y2_mm: 250.0,
                w_mm: 0.3, rgb: (0.2, 0.4, 0.7),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut out = Vec::new();
        write_pages(&[leaf], 210.0, 297.0, &font(), &mut out).expect("PDF が出ない");
        let body = unpack(&out);
        assert!(body.contains(" re"), "塗りの四角が無い");
        assert!(body.contains(" rg"), "塗りの色が無い");
        assert!(body.contains(" RG"), "線の色が無い");
        // **塗りは線より先**。潰すと罫線が消えます
        let fill_at = body.find(" re").expect("四角");
        let line_at = body.find(" l\n").unwrap_or(usize::MAX);
        assert!(fill_at < line_at, "塗りが線より後ろにある(線が消えます)");
    }

    /// **表が頁をまたぐと、見出しの行が繰り返される。**
    ///
    /// 事務の帳票は、2頁目が数字の並びだけでは読めません。頁割りが
    /// 見出しのぶんだけ頭を下げ、そこへ1頁目の見出しを写します。
    /// 数えないと重なります(2026-08-27 に実物で重ねた)。
    #[test]
    fn a_header_row_repeats_on_later_pages() {
        let mut src = String::from("|===\n|品名 |数量\n\n");
        for i in 1..=60 {
            src.push_str(&format!("|品目{i} |{}\n", i * 3));
        }
        src.push_str("|===\n");
        let doc = crate::super_parse(&src);
        let (sheet, page, bytes) = crate::doc_to_sheet(&doc, None).expect("組めない");
        let pp = crate::Paper::hitoshii(page.w_mm, page.h_mm, page.left_mm);
        let mut out = Vec::new();
        let lost = sheet_to_pdf(&sheet, &bytes, pp, std::io::Cursor::new(&mut out))
            .expect("PDF が出ない");
        assert!(!sheet.header_tables.is_empty(), "見出しの表を覚えていない");
        assert!(lost.is_empty(), "落ちた物がある: {lost:?}");

        // **2頁目以降に場所が空いていること。** 空けないと重なります
        let full = crate::paginate_full(&sheet, pp);
        let 頁 = full.pages.iter().copied().max().unwrap_or(1);
        assert!(頁 > 1, "1頁に収まってしまい、繰り返しを見られない");
        assert!(
            full.header_h.iter().skip(1).any(|h| *h > 0.0),
            "見出しのぶんの場所が空いていない: {:?}",
            full.header_h
        );

        // 見出しの字が頁の数だけ出ていること
        let body = unpack(&out);
        let n = body.matches("Tm").count();
        assert!(n > 0, "置く命令が無い");
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
    /// 線の色(0〜1 の RGB)。既定は黒
    pub rgb: (f32, f32, f32),
    /// 不透明度(0〜1、1 = 不透明)。図形の影と `SheetShape::alpha` が使います
    pub a: f32,
}

impl Default for Rule {
    fn default() -> Self {
        // **透明度の既定は1**です。0 にすると足した所が全部消えます
        Rule { x1_mm: 0.0, y1_mm: 0.0, x2_mm: 0.0, y2_mm: 0.0, w_mm: 0.0, rgb: (0.0, 0.0, 0.0), a: 1.0 }
    }
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
    /// **塗り(表の帯・セルの背景)。** 罫線より先に敷きます — 線を
    /// 塗り潰さないためです
    pub fills: Vec<Fill>,
    /// **好きな形の塗り。** 円グラフの扇や、傾いた棒に使います。
    /// 四角で足りるものは [`Fill`] の方が小さく済みます
    pub polys: Vec<Poly>,
    /// **字の上に引く線。** [`Leaf::rules`] は字の下です。
    ///
    /// 手描きのペンはここへ入ります(紙に書き込んだ線なので、字の上に
    /// 乗るのが本当です)。蛍光ペンは逆に字の下なので `rules` の方です。
    pub rules_top: Vec<Rule>,
    /// **この紙の大きさ(mm)。** 節で紙が変わる文書は頁ごとに違います。
    /// `None` なら呼ぶ側に渡した既定の大きさ
    pub size_mm: Option<(f32, f32)>,
}

/// 塗る四角。左下からの mm。
pub struct Fill {
    pub x_mm: f32,
    pub y_mm: f32,
    pub w_mm: f32,
    pub h_mm: f32,
    /// 0〜1 の RGB
    pub rgb: (f32, f32, f32),
    /// 不透明度(0〜1、1 = 不透明)
    pub a: f32,
}

impl Default for Fill {
    fn default() -> Self {
        Fill { x_mm: 0.0, y_mm: 0.0, w_mm: 0.0, h_mm: 0.0, rgb: (0.0, 0.0, 0.0), a: 1.0 }
    }
}

/// **好きな形の塗り。** 点を順に結んで閉じ、中を塗ります。
///
/// 円は多角形に刻んで渡します(細かく刻めば目では区別が付きません)。
/// PDF は曲線も持てますが、点の列だけで済ませると呼ぶ側が1つの形で
/// 何でも描けます。
#[derive(Debug, Clone)]
pub struct Poly {
    /// 左下からの mm
    pub points: Vec<(f32, f32)>,
    /// 0〜1 の RGB
    pub rgb: (f32, f32, f32),
    /// 不透明度(0〜1、1 = 不透明)
    pub a: f32,
}

impl Default for Poly {
    fn default() -> Self {
        Poly { points: Vec::new(), rgb: (0.0, 0.0, 0.0), a: 1.0 }
    }
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
    let (pages, lost) = sheet_leaves_with(sheet, paper, dress, page_decor);
    write_pages(&pages, paper.width_mm, paper.height_mm, font_data, out)?;
    Ok(lost)
}

/// **紙面だけを組む。** PDF は書きません。
///
/// 絵にする道([`crate::e`])と回帰検査の入り口です。書く所と組む所を
/// 分けてあるので、**紙と絵が同じ紙面から出ます**。
/// 返りは (紙面の並び, 載らなかった物の報せ)。
pub fn sheet_leaves_with<F: Fn(usize) -> Vec<kumihan::Line>>(
    sheet: &kumihan::Sheet,
    paper: crate::Paper,
    dress: &crate::PageDress,
    page_decor: F,
) -> (Vec<Leaf>, Vec<String>) {
    // **紙面の x は左余白からの距離**です。紙の左端からではありません。
    // 足さないと字が左端に寄ります(2026-08-27 に pdftotext -bbox で
    // 突き合わせて見つけた — 字が取れるかを見るだけでは分かりません)
    let full = crate::paginate_full(sheet, paper);
    let (pages_of, offsets) = (&full.pages, &full.offsets);
    let n = pages_of.iter().copied().max().unwrap_or(1);
    // **紙は頁ごと**です(節で A4 縦と横が混ざる)。余白も紙ごとに変わるので、
    // 字の位置を決めるときの左余白もその頁の物を使います
    let paper_of = |k: usize| full.papers.get(k).copied().unwrap_or(paper);
    let mut pages: Vec<Leaf> = (0..n)
        .map(|k| {
            let pp = paper_of(k);
            Leaf { size_mm: Some((pp.width_mm, pp.height_mm)), ..Default::default() }
        })
        .collect();

    // 行を頁へ配ります。**y は上からの mm** なので、PDF の下からの mm に直します
    for (i, line) in sheet.lines.iter().enumerate() {
        let k = pages_of.get(i).copied().unwrap_or(1).max(1) - 1;
        let off = offsets.get(k).copied().unwrap_or(0.0);
        let pp = paper_of(k);
        let (mx, ph) = (pp.margin_mm, pp.height_mm);
        let y_roll = line.y_mm - off;
        let y = ph - y_roll;
        let Some(p) = pages.get_mut(k) else { continue };
        if sheet.vertical {
            // **縦書きは1字ずつ**。列の x(絶対 mm)に正立で置き、
            // 字の腰は「上からの距離 + だいたいの上がり」で合わせます
            // (いまの道と同じ決め)
            let colx = sheet.vert_x.get(i).copied().unwrap_or(0.0);
            for c in &line.cells {
                let em = c.size_pt * 0.3528;
                p.pieces.push(Piece {
                    x_mm: mx + colx,
                    y_mm: ph - (y_roll + c.x_mm + em * 0.85),
                    size_pt: c.size_pt,
                    text: c.ch.to_string(),
                    color: c.fmt.color.clone(),
                    w_mm: em,
                    underline: c.fmt.underline,
                    strike: c.fmt.strike,
                    bold: c.fmt.bold,
                    highlight: c.fmt.highlight.clone(),
                    font: 0,
                });
            }
            continue;
        }
        // **見た目が同じ続きの字だけ**まとめます。1字ずつ置くと PDF が
        // 太りますが、まとめすぎると下線が隣の字まで伸び、太字が普通の字に
        // 掛かります(2026-08-27 に実物を見て気づいた — 題も見出しも
        // 細いままでした)
        let mut run: Option<Piece> = None;
        for c in &line.cells {
            let same = run.as_ref().is_some_and(|r: &Piece| {
                (r.size_pt - c.size_pt).abs() < 0.01
                    && r.color == c.fmt.color
                    && r.bold == c.fmt.bold
                    && r.underline == c.fmt.underline
                    && r.strike == c.fmt.strike
                    && r.highlight == c.fmt.highlight
            });
            match &mut run {
                Some(r) if same => {
                    r.text.push(c.ch);
                    r.w_mm += c.w_mm;
                }
                _ => {
                    if let Some(r) = run.take() {
                        p.pieces.push(r);
                    }
                    run = Some(Piece {
                        x_mm: mx + c.x_mm,
                        y_mm: y,
                        size_pt: c.size_pt,
                        text: c.ch.to_string(),
                        color: c.fmt.color.clone(),
                        w_mm: c.w_mm,
                        underline: c.fmt.underline,
                        strike: c.fmt.strike,
                        bold: c.fmt.bold,
                        highlight: c.fmt.highlight.clone(),
                        font: 0,
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
        let k = page_of(offsets, r[1], paper.height_mm);
        let off = offsets.get(k).copied().unwrap_or(0.0);
        let pp = paper_of(k);
        if let Some(p) = pages.get_mut(k) {
            p.rules.push(Rule {
                x1_mm: pp.margin_mm + r[0],
                y1_mm: pp.height_mm - (r[1] - off),
                x2_mm: pp.margin_mm + r[2],
                y2_mm: pp.height_mm - (r[3] - off),
                w_mm: 0.2,
                rgb: (0.0, 0.0, 0.0),
                ..Default::default()
            });
        }
    }

    let mut lost = Vec::new();

    // **表の見出しの行を、2頁目から繰り返します**(2026-08-27)。
    //
    // 表が頁をまたぐと、次の頁は見出しの無い数字の並びになります。事務の
    // 帳票では読めません。頁割りが見出しのぶんだけ頭を下げてあるので、
    // その空いた所に1頁目の見出しの行を写します。
    for &t in &sheet.header_tables {
        // その表の見出しの行(行 0)と、その表が載っている頁
        let mut head: Vec<&kumihan::Line> = Vec::new();
        let mut on: Vec<usize> = Vec::new();
        let mut first = usize::MAX;
        for (i, line) in sheet.lines.iter().enumerate() {
            let Some((tn, ri, _)) = line.cell else { continue };
            if tn != t {
                continue;
            }
            let k = pages_of.get(i).copied().unwrap_or(1).max(1) - 1;
            first = first.min(k);
            if !on.contains(&k) {
                on.push(k);
            }
            if ri == 0 {
                head.push(line);
            }
        }
        if head.is_empty() {
            continue;
        }
        let base = head.iter().map(|l| l.y_mm).fold(f32::MAX, f32::min);
        for k in on {
            // **頁割りが場所を空けた頁にだけ**置きます。空いていない頁に
            // 置くと重なります(2026-08-27 に実物で重ねた)
            let space = full.header_h.get(k).copied().unwrap_or(0.0);
            if k == first || space <= 0.0 {
                continue;
            }
            let pp = paper_of(k);
            let Some(p) = pages.get_mut(k) else { continue };
            for line in &head {
                for c in &line.cells {
                    p.pieces.push(Piece {
                        x_mm: pp.margin_mm + c.x_mm,
                        y_mm: pp.height_mm - (pp.margin_mm + (line.y_mm - base)),
                        size_pt: c.size_pt,
                        text: c.ch.to_string(),
                        color: c.fmt.color.clone(),
                        w_mm: c.w_mm,
                        underline: c.fmt.underline,
                        strike: c.fmt.strike,
                        bold: c.fmt.bold,
                        highlight: c.fmt.highlight.clone(),
                        font: 0,
                    });
                }
            }
        }
    }

    // 塗り。**罫線より先に敷きます**(線を塗り潰さないため)
    for (at, color) in &sheet.fills {
        let k = page_of(offsets, at[1], paper.height_mm);
        let off = offsets.get(k).copied().unwrap_or(0.0);
        let pp = paper_of(k);
        if let Some(p) = pages.get_mut(k) {
            p.fills.push(Fill {
                x_mm: pp.margin_mm + at[0],
                // 紙面は上端の y。PDF は左下からなので、高さのぶん下げます
                y_mm: pp.height_mm - (at[1] - off) - at[3],
                w_mm: at[2],
                h_mm: at[3],
                rgb: rgb(color),
                ..Default::default()
            });
        }
    }

    // **塗りは罫線より先に敷きます**(線を塗り潰さないため)
    for (at, color) in &sheet.fills {
        let k = page_of(offsets, at[1], paper.height_mm);
        let off = offsets.get(k).copied().unwrap_or(0.0);
        let pp = paper_of(k);
        if let Some(p) = pages.get_mut(k) {
            p.fills.push(Fill {
                x_mm: pp.margin_mm + at[0],
                // 紙面は上端の y。PDF は左下からなので、高さのぶん下げます
                y_mm: pp.height_mm - (at[1] - off) - at[3],
                w_mm: at[2],
                h_mm: at[3],
                rgb: rgb(color),
                ..Default::default()
            });
        }
    }

    // 画像。どの頁に載るかは上端の y で決めます
    let mut bad = 0;
    for (data, at) in &sheet.images {
        let k = page_of(offsets, at[1], paper.height_mm);
        let off = offsets.get(k).copied().unwrap_or(0.0);
        let pp = paper_of(k);
        if image::load_from_memory(data).is_err() {
            bad += 1;
            continue;
        }
        if let Some(p) = pages.get_mut(k) {
            p.images.push(Image {
                x_mm: pp.margin_mm + at[0],
                // 紙面は上端の y。PDF は左下からなので、高さのぶん下げます
                y_mm: pp.height_mm - (at[1] - off) - at[3],
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
        let pp = full.papers.get(k).copied().unwrap_or(paper);
        for line in page_decor(k + 1) {
            for c in &line.cells {
                p.pieces.push(Piece {
                    x_mm: pp.margin_mm + c.x_mm,
                    y_mm: pp.height_mm - line.y_mm,
                    size_pt: c.size_pt,
                    text: c.ch.to_string(),
                    color: c.fmt.color.clone(),
                    w_mm: c.w_mm,
                    underline: c.fmt.underline,
                    strike: c.fmt.strike,
                    bold: c.fmt.bold,
                    highlight: c.fmt.highlight.clone(),
                    font: 0,
                });
            }
        }
    }
    // **手描きの筆。** 蛍光ペンは太く・薄く・字の下、ペンは細く・濃く・字の上。
    // 2026-08-29 まで「この書き手ではまだ載りません」と数えるだけでした —
    // 紙面の色に透明度が無く、蛍光ペンが出せなかったためです
    if !dress.ink.is_empty() {
        for (k, leaf) in pages.iter_mut().enumerate() {
            let h = leaf.size_mm.map(|(_, h)| h).unwrap_or(paper.height_mm);
            for st in dress.ink.iter().filter(|s| s.page == k) {
                let (w_mm, rgb, a) = if st.highlighter {
                    (3.0, (1.0, 0.89, 0.36), 0.45)
                } else {
                    (0.45, (0.11, 0.23, 0.32), 1.0)
                };
                let saki = if st.highlighter { &mut leaf.rules } else { &mut leaf.rules_top };
                for w in st.points.windows(2) {
                    saki.push(Rule {
                        x1_mm: w[0].0,
                        y1_mm: h - w[0].1,
                        x2_mm: w[1].0,
                        y2_mm: h - w[1].1,
                        w_mm,
                        rgb,
                        a,
                    });
                }
            }
        }
    }
    // **ページに貼り付く図形。** 字と罫線の上に置きます(Word も同じで、
    // `behindDoc="0"` は本文の上です)
    if !dress.shapes.is_empty() {
        for (k, leaf) in pages.iter_mut().enumerate() {
            let h = leaf.size_mm.map(|(_, h)| h).unwrap_or(paper.height_mm);
            let kono: Vec<kumihan::DocShape> =
                dress.shapes.iter().filter(|s| s.page == k).cloned().collect();
            if !kono.is_empty() {
                crate::grid::doc_shapes(leaf, &kono, h);
            }
        }
    }
    (pages, lost)
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

/// 濃さを 0〜255 の目盛りに。**同じ濃さは同じ資源**を使い回すためです
fn usu_key(a: f32) -> u8 {
    (a.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// 濃さが変わったときだけ書きます。255(不透明)は既定なので何も書きません
fn usu(c: &mut Content, ima: &mut Option<u8>, a: f32) {
    let k = usu_key(a);
    if *ima == Some(k) || (ima.is_none() && k == 255) {
        return;
    }
    c.set_parameters(Name(format!("A{k}").as_bytes()));
    *ima = Some(k);
}
