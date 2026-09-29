//! **The page pictures** (docs/sekkei/hyouji-e.ja.adoc, step 2).
//!
//! A docx is shown as the pages the PDF prints: `paper::page_leaves` makes
//! them from what the last layout kept (`page_src`), `paper::e` draws each
//! with vello_cpu, and gpui only shows the picture. The editing marks (caret,
//! selection, IME underline, edit marks) are still drawn by the view, over
//! the pictures.

use crate::*;
use std::sync::Arc;

/// A page drawn at `bai` pixels per mm, as gpui keeps pictures (BGRA)
pub(crate) fn picture(
    leaf: &paper::pdfw::Leaf,
    w_mm: f32,
    h_mm: f32,
    bai: f32,
    fonts: &[&[u8]],
) -> Option<Arc<gpui::RenderImage>> {
    let e = paper::e::egaku_fonts(leaf, w_mm, h_mm, bai, fonts);
    let mut bgra = e.rgba;
    for p in bgra.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    let buf = image::RgbaImage::from_raw(e.w, e.h, bgra)?;
    Some(Arc::new(gpui::RenderImage::new(vec![image::Frame::new(buf)])))
}

impl Writer {
    /// Whether the document is shown as page pictures: a docx on stacked
    /// pages (not the flowing view, the two-page spread or vertical text).
    /// `page_pictures = "0"` in settings.toml turns them off
    pub(crate) fn pictures_on(&self) -> bool {
        !self.native
            && self.sheets()
            && self.page_src.is_some()
            && ui::settings::get("page_pictures").is_none_or(|v| v.trim() != "0")
    }

    /// **The PDF of a docx is written from the same pages as the screen**
    /// (step 4): `pdfw` writes the leaves the page pictures are drawn from,
    /// with the handwritten strokes on top, so shapes, text boxes and
    /// cropped pictures come out as they are shown, and only the letters
    /// used are embedded. `None` when this path does not apply (a document
    /// of this app, vertical text, or no layout yet), and the older writer
    /// is used
    pub(crate) fn write_pdf_pages(&self, p: &std::path::Path) -> Option<Result<(), String>> {
        if self.native || self.page.vertical {
            return None;
        }
        let src = self.page_src.as_ref()?;
        let mut leaves = match paper::page_leaves(src) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        let whole = paper::Paper::from_page(&src.page);
        paper::pdfw::put_ink(&mut leaves, &self.doc.ink, whole.height_mm);
        let fonts = src.font_bytes();
        Some(kumihan::atomic::save(p, |f| {
            paper::pdfw::write_pages_fonts(
                &leaves,
                whole.width_mm,
                whole.height_mm,
                &fonts,
                std::io::BufWriter::new(f),
            )
        }))
    }

    /// The pages of the last layout, made once per layout
    fn pages_now(&mut self) -> Option<Arc<Vec<paper::pdfw::Leaf>>> {
        if self.pic_gen != self.layout_gen || self.pic_leaves.is_none() {
            self.pic_gen = self.layout_gen;
            self.pic_cache.clear();
            self.pic_leaves =
                self.page_src.as_ref().and_then(|s| paper::page_leaves(s).ok()).map(Arc::new);
        }
        self.pic_leaves.clone()
    }

    /// Page `k` drawn at `bai` pixels per mm, kept until the next layout
    fn page_picture(&mut self, k: usize, bai: f32) -> Option<(Arc<gpui::RenderImage>, f32, f32)> {
        let leaves = self.pages_now()?;
        let leaf = leaves.get(k)?;
        let src = self.page_src.as_ref()?;
        let whole = paper::Paper::from_page(&src.page);
        let (w, h) = leaf.size_mm.unwrap_or((whole.width_mm, whole.height_mm));
        let key = (k, (bai * 100.0).round() as u32);
        if let Some(p) = self.pic_cache.get(&key) {
            return Some((p.clone(), w, h));
        }
        let pic = picture(leaf, w, h, bai, &src.font_bytes())?;
        self.pic_cache.insert(key, pic.clone());
        Some((pic, w, h))
    }

    /// Put the pictures of the pages that can be seen (and one page either
    /// side) on the paper, each at the top the stacked layout gives it.
    /// `from_mm`..`to_mm` is the section shown
    pub(crate) fn put_page_pictures(
        &mut self,
        mut paper: gpui::Div,
        pxmm: f32,
        scale: f32,
        from_mm: f32,
        to_mm: f32,
    ) -> gpui::Div {
        let tops = self.page_tops.clone();
        let view_from = self.scroll_mm;
        let view_to = self.scroll_mm + self.view_h_px / pxmm;
        for (k, top) in tops.iter().enumerate() {
            if *top < from_mm - 0.01 || *top >= to_mm {
                continue;
            }
            let h = self.page_papers.get(k).map(|q| q.height_mm).unwrap_or(self.pg.h_mm);
            // One page of margin either way, so scrolling finds it ready
            if top + h < view_from - h || *top > view_to + h {
                continue;
            }
            if let Some((pic, w, h)) = self.page_picture(k, pxmm * scale) {
                paper = paper.child(
                    gpui::img(pic).absolute().left(px(0.0)).top(px(top * pxmm)).w(px(w * pxmm)).h(px(h * pxmm)),
                );
            }
        }
        paper
    }
}
