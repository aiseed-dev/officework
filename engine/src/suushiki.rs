//! **数式を組む。** LaTeX の数式を PNG(と mm の寸法)にします。
//!
//! 2026-09-02 の決め(SEKKEI「数式は Rust で組み、OMML も読む」)。それまでは
//! Python(TeX か matplotlib)に組ませていましたが、エンジンの機能は Python に
//! 頼らないと決めたので、Rust の typst と mitex で組みます。
//!
//! 道は3段です。
//!
//! 1. こちらで括弧の釣り合いを確かめます。mitex は `\frac{a}{` を黙って通して
//!    空の分母で組むので、先に断ります
//! 2. mitex が LaTeX を typst の数式に変えます(`\frac{a}{b}` → `frac(a, b)`)。
//!    `\sqrt` や `\text{}` や行列は mitex の typst 側の定義に頼るので、その
//!    3ファイル(assets/suushiki/mitex、Apache-2.0)を埋め込んで一緒に渡します
//! 3. typst が組み、絵にします。数式の書体は New Computer Modern Math
//!    (assets/suushiki、GUST Font License)を埋め込み、数式の中の日本語の
//!    ために文書の書体を後ろに並べます。typst は数式の書体を show ルールで
//!    決めるので、`#set text(font:)` では効きません(実際に豆腐になりました)
//!
//! 出来上がりの大きさは、文書の字の大きさ(pt)で組んだ自然な寸法です。
//! 絵は 1pt を 4 画素で描きます(印刷でも粗くならない密度)。

/// 組んだ結果。
#[derive(Debug, Clone)]
pub struct Kumitate {
    /// PNG(背景は透明)
    pub png: Vec<u8>,
    pub w_mm: f32,
    pub h_mm: f32,
}

/// **書く前の検査。** 空・括弧の釣り合い・`\begin{}` と `\end{}` の対を見ます。
/// mitex がここを黙って通すので、こちらで先に断ります。
pub fn tashikameru(tex: &str) -> Result<(), String> {
    if tex.trim().is_empty() {
        return Err("空の数式です".into());
    }
    let mut depth = 0i32;
    let mut esc = false;
    for ch in tex.chars() {
        if esc {
            esc = false;
            continue;
        }
        match ch {
            '\\' => esc = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return Err("閉じ括弧 } が多すぎます".into());
                }
            }
            _ => {}
        }
    }
    if depth > 0 {
        return Err(format!("括弧 {{ が {depth} つ閉じていません"));
    }
    // 環境の対。\begin{X} と \end{X} が同じ名前で入れ子になっているか
    let mut stack: Vec<String> = Vec::new();
    let mut rest = tex;
    while let Some(i) = rest.find("\\begin{").or_else(|| rest.find("\\end{")) {
        let (is_begin, skip) = if rest[i..].starts_with("\\begin{") { (true, 7) } else { (false, 5) };
        let after = &rest[i + skip..];
        let Some(close) = after.find('}') else { break };
        let name = after[..close].to_string();
        if is_begin {
            stack.push(name);
        } else {
            match stack.pop() {
                Some(open) if open == name => {}
                Some(open) => return Err(format!("\\begin{{{open}}} が \\end{{{name}}} で閉じられています")),
                None => return Err(format!("\\end{{{name}}} に対する \\begin がありません")),
            }
        }
        rest = &after[close + 1..];
    }
    if let Some(open) = stack.pop() {
        return Err(format!("\\begin{{{open}}} が閉じていません"));
    }
    Ok(())
}

/// **数式を組む。** `tex` は LaTeX の数式(`$` は付けません)。`size_pt` は
/// 文書の字の大きさ。`moji_font` は文書の書体のファイル(日本語の字のため。
/// 無ければ数式の書体だけで組み、日本語は出ません)。
///
/// 組めなければ理由を返します。黙って空の絵を返しません。
pub fn kumu(tex: &str, size_pt: f32, moji_font: Option<&[u8]>) -> Result<Kumitate, String> {
    kumu_iro(tex, size_pt, moji_font, None)
}

/// [`kumu`] の、字の色を指定できる形。`color` は `RRGGBB`(先頭の `#` は
/// あってもなくてもよい)。None は黒
pub fn kumu_iro(tex: &str, size_pt: f32, moji_font: Option<&[u8]>, color: Option<&str>) -> Result<Kumitate, String> {
    tashikameru(tex)?;
    imp::kumu(tex, size_pt, moji_font, color, false).map(|(png, w_mm, h_mm)| Kumitate { png, w_mm, h_mm })
}

/// **数式を SVG に組む。** 字は輪郭になるので、受け手に書体が無くても化けません。
/// 引数は [`kumu_iro`] と同じです
pub fn kumu_svg(tex: &str, size_pt: f32, moji_font: Option<&[u8]>, color: Option<&str>) -> Result<String, String> {
    tashikameru(tex)?;
    imp::kumu(tex, size_pt, moji_font, color, true).map(|(svg, _, _)| String::from_utf8_lossy(&svg).into_owned())
}

#[cfg(feature = "suushiki")]
mod imp {
    use std::collections::HashMap;
    use typst::diag::{FileError, FileResult};
    use typst::foundations::{Bytes, Datetime, Duration};
    use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
    use typst::text::{Font, FontBook};
    use typst::utils::LazyHash;
    use typst::{Library, LibraryExt, World};

    const MATH_FONT: &[u8] = include_bytes!("../suushiki/NewCMMath-Book.otf");
    const MITEX_FILES: &[(&str, &str)] = &[
        ("/specs/mod.typ", include_str!("../suushiki/mitex/mod.typ")),
        ("/specs/prelude.typ", include_str!("../suushiki/mitex/prelude.typ")),
        ("/specs/latex/standard.typ", include_str!("../suushiki/mitex/latex/standard.typ")),
    ];
    /// 1pt を何画素で描くか
    const GASO_PER_PT: f64 = 4.0;
    const MM_PER_PT: f32 = 25.4 / 72.0;

    /// typst に渡す「世界」。ファイルは埋め込んだ物だけ、書体は数式用と文書の物だけ。
    struct Sekai {
        lib: LazyHash<Library>,
        book: LazyHash<FontBook>,
        fonts: Vec<Font>,
        main: FileId,
        sources: HashMap<FileId, Source>,
    }

    fn file_id(vpath: &str) -> FileId {
        let vp = VirtualPath::new(vpath).expect("固定の径路");
        FileId::new(RootedPath::new(VirtualRoot::Project, vp))
    }

    impl World for Sekai {
        fn library(&self) -> &LazyHash<Library> {
            &self.lib
        }
        fn book(&self) -> &LazyHash<FontBook> {
            &self.book
        }
        fn main(&self) -> FileId {
            self.main
        }
        fn source(&self, id: FileId) -> FileResult<Source> {
            self.sources
                .get(&id)
                .cloned()
                .ok_or_else(|| FileError::NotFound(std::path::PathBuf::from(id.vpath().get_without_slash())))
        }
        fn file(&self, id: FileId) -> FileResult<Bytes> {
            self.sources
                .get(&id)
                .map(|s| Bytes::new(s.text().as_bytes().to_vec()))
                .ok_or_else(|| FileError::NotFound(std::path::PathBuf::from(id.vpath().get_without_slash())))
        }
        fn font(&self, index: usize) -> Option<Font> {
            self.fonts.get(index).cloned()
        }
        fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
            None
        }
    }

    /// 組む本体。`svg` が true なら SVG の字、false なら PNG を返します
    pub(super) fn kumu(
        tex: &str,
        size_pt: f32,
        moji_font: Option<&[u8]>,
        color: Option<&str>,
        svg: bool,
    ) -> Result<(Vec<u8>, f32, f32), String> {
        let iro = color.map(|c| c.trim_start_matches('#')).filter(|c| c.len() == 6 && c.chars().all(|ch| ch.is_ascii_hexdigit()));
        let fill = iro.map(|c| format!("#set text(fill: rgb(\"#{c}\"))\n")).unwrap_or_default();
        let math = mitex::convert_math(tex, None).map_err(|e| e.trim().trim_start_matches("error: ").to_string())?;

        // 書体。数式用を先に、文書の書体(あれば)を後ろに
        let mut fonts: Vec<Font> = Font::iter(Bytes::new(MATH_FONT.to_vec())).collect();
        let math_family = fonts.first().map(|f| f.info().family.clone()).unwrap_or_default();
        let mut families = vec![math_family];
        if let Some(bytes) = moji_font {
            let extra: Vec<Font> = Font::iter(Bytes::new(bytes.to_vec())).collect();
            if let Some(f) = extra.first() {
                families.push(f.info().family.clone());
            }
            fonts.extend(extra);
        }
        let font_list = families.iter().map(|f| format!("{f:?}")).collect::<Vec<_>>().join(", ");

        let src = format!(
            "#import \"specs/mod.typ\": mitex-scope\n\
             #set page(width: auto, height: auto, margin: 1pt, fill: none)\n\
             #set text(size: {size_pt}pt)\n{fill}\
             #show math.equation: set text(font: ({font_list}))\n\
             #math.equation(block: true, eval({}, scope: mitex-scope))\n",
            format!("{:?}", format!("$ {math} $"))
        );

        let main = file_id("/main.typ");
        let mut sources = HashMap::new();
        sources.insert(main, Source::new(main, src));
        for (path, text) in MITEX_FILES {
            let id = file_id(path);
            sources.insert(id, Source::new(id, (*text).to_string()));
        }
        let sekai = Sekai {
            lib: LazyHash::new(Library::default()),
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
            main,
            sources,
        };
        let doc: typst_layout::PagedDocument = typst::compile(&sekai).output.map_err(|errs| {
            errs.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; ")
        })?;
        let page = doc.pages().first().ok_or("何も組めませんでした")?;
        let size = page.frame.size();
        let (w_mm, h_mm) = (size.x.to_pt() as f32 * MM_PER_PT, size.y.to_pt() as f32 * MM_PER_PT);
        if svg {
            let s = typst_svg::svg(page, &typst_svg::SvgOptions::default());
            return Ok((s.into_bytes(), w_mm, h_mm));
        }
        let opts = typst_render::RenderOptions {
            pixel_per_pt: typst::utils::Scalar::new(GASO_PER_PT),
            ..Default::default()
        };
        let pix = typst_render::render(page, &opts);
        let png = pix.encode_png().map_err(|e| e.to_string())?;
        Ok((png, w_mm, h_mm))
    }
}

#[cfg(not(feature = "suushiki"))]
mod imp {
    pub(super) fn kumu(_tex: &str, _size_pt: f32, _moji_font: Option<&[u8]>, _color: Option<&str>, _svg: bool) -> Result<(Vec<u8>, f32, f32), String> {
        Err("数式を組む部品(suushiki)が入っていません".into())
    }
}

#[cfg(all(test, feature = "suushiki"))]
mod tests {
    use super::*;

    #[test]
    fn bunsuu_wo_kumu() {
        let k = kumu(r"\frac{a+b}{2}", 11.0, None).unwrap();
        assert!(k.png.starts_with(b"\x89PNG"), "PNG ではない");
        assert!(k.w_mm > 2.0 && k.h_mm > 2.0, "寸法が小さすぎる: {} x {}", k.w_mm, k.h_mm);
        assert!(k.h_mm > k.w_mm * 0.5, "分数は縦に高い: {} x {}", k.w_mm, k.h_mm);
    }

    #[test]
    fn gyouretsu_to_heihoukon() {
        for t in [
            r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
            r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
            r"\sum_{i=1}^{n} i^2",
            r"\int_0^\infty e^{-x^2}\,dx",
            r"f(x) = \begin{cases} x^2 & (x \ge 0) \\ -x & (x < 0) \end{cases}",
            r"\lim_{x \to 0} \frac{\sin x}{x} = 1",
        ] {
            kumu(t, 11.0, None).unwrap_or_else(|e| panic!("{t}: {e}"));
        }
    }

    #[test]
    fn kowareta_shiki_wa_kotowaru() {
        assert!(kumu(r"\frac{a}{", 11.0, None).is_err(), "括弧の不足");
        assert!(kumu(r"a}", 11.0, None).is_err(), "閉じ括弧の過多");
        assert!(kumu(r"\begin{pmatrix} a \end{bmatrix}", 11.0, None).is_err(), "環境の名前違い");
        assert!(kumu(r"\foo{a}", 11.0, None).is_err(), "知らない命令");
        assert!(kumu("   ", 11.0, None).is_err(), "空");
    }

    #[test]
    fn ookisa_wa_ji_no_ookisa_ni_tsuite_kuru() {
        let a = kumu("x+y", 10.0, None).unwrap();
        let b = kumu("x+y", 20.0, None).unwrap();
        assert!((b.w_mm / a.w_mm - 2.0).abs() < 0.3, "倍にならない: {} → {}", a.w_mm, b.w_mm);
    }

    #[test]
    fn svg_to_iro() {
        let svg = kumu_svg(r"\frac{a}{b}", 11.0, None, Some("#1B6E3C")).unwrap();
        assert!(svg.starts_with("<svg"), "SVG ではない: {}", &svg[..40.min(svg.len())]);
        assert!(svg.contains("1b6e3c") || svg.contains("1B6E3C"), "色が効いていない");
        assert!(kumu_iro("x", 11.0, None, Some("赤")).is_ok(), "読めない色は黒で組む");
    }

    #[test]
    fn nihongo_wa_bunsho_no_shotai_de() {
        // 文書の書体が機械に無ければ、この検査は飛ばす
        let Ok((fam, _)) = crate::font::for_document(None) else { return };
        let Ok(bytes) = crate::font::load(fam) else { return };
        let with = kumu(r"\text{売上} = \text{単価} \times \text{数量}", 11.0, Some(&bytes)).unwrap();
        let without = kumu(r"\text{売上} = \text{単価} \times \text{数量}", 11.0, None).unwrap();
        // 豆腐(□)は本物の字より狭い。書体を渡した方が横に長くなる
        assert!(with.w_mm > without.w_mm, "書体が効いていない: {} <= {}", with.w_mm, without.w_mm);
    }
}
