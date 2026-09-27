//! **The page pictures** (docs/sekkei/hyouji-e.ja.adoc, step 2).
//!
//! A docx is shown as the pages the PDF prints: `paper::page_leaves` makes
//! them from what the last layout kept (`page_src`), `paper::e` draws each
//! with vello_cpu, and gpui only shows the picture. The editing marks (caret,
//! selection, IME underline, edit marks) are still drawn by the view, over
//! the pictures.

use crate::*;
use std::sync::Arc;

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
        let e = paper::e::egaku_fonts(leaf, w, h, bai, &src.font_bytes());
        // gpui keeps pictures as BGRA
        let mut bgra = e.rgba;
        for p in bgra.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        let buf = image::RgbaImage::from_raw(e.w, e.h, bgra)?;
        let pic = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(buf)]));
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
