//! **An .adoc beside its pages** (docs/sekkei/hyouji-e.ja.adoc, step 6).
//!
//! The text of an .adoc is what the author writes, so it is edited as it
//! is: the left side is the writer's own editing surface holding the file's
//! characters in a fixed-width face, flowing to the width of the pane, and
//! saving writes those characters back unchanged. The right side shows the
//! pages the text makes, laid out the way the PDF lays them out (template,
//! forms, table formulas) and drawn as pictures. The pages are made again
//! on another thread a moment after the typing stops.
//!
//! `adoc_code = "0"` in settings.toml opens an .adoc the older way (edited
//! on its pages).

use crate::*;
use std::sync::Arc;

/// How long the typing must pause before the pages are made again
const PAUSE_MS: u64 = 350;
/// The gap between two pages in the right pane (mm)
const GAP_MM: f32 = 6.0;

/// The pages made from the text at one moment
pub(crate) struct Preview {
    /// Each page and the document (of the file) it belongs to
    pub pages: Vec<(paper::pdfw::Leaf, usize)>,
    /// The fonts of each document, in the order its leaves name them
    pub fonts: Vec<Vec<(String, Vec<u8>)>>,
    /// The paper of a page that does not carry its own size
    pub paper: (f32, f32),
}

/// The state of the split view
#[derive(Default)]
pub(crate) struct CodeView {
    /// Counts changes to the text; `built` is the one the pages show
    pub edits: u64,
    pub built: u64,
    pub edited_at: Option<std::time::Instant>,
    pub busy: bool,
    pub waiting: bool,
    pub pages: Option<Arc<Preview>>,
    /// Why the text could not be made into pages, shown above them
    pub err: Option<String>,
    pub cache: std::collections::HashMap<(usize, u32), Arc<gpui::RenderImage>>,
    pub scroll_px: f32,
    /// The width the text was last laid out to (px)
    pub laid_w: f32,
}

/// Whether .adoc files open beside their pages
pub(crate) fn code_on_open() -> bool {
    ui::settings::get("adoc_code").is_none_or(|v| v.trim() != "0")
}

/// Everything the other thread needs to make the pages
pub(crate) struct Job {
    docs: Vec<kumihan::Document>,
    theme: kumihan::theme::Theme,
    iter: Option<(u32, f64)>,
}

pub(crate) fn make_pages(job: Job) -> Result<Preview, String> {
    let mut pages = Vec::new();
    let mut fonts = Vec::new();
    let mut size = (210.0, 297.0);
    for (i, doc) in job.docs.iter().enumerate() {
        // The same steps as the writer's layout of an .adoc and the PDF:
        // the template, the forms, the table formulas, then the fonts
        let mut composed = paper::compose_doc(doc, Some(&job.theme));
        kumihan::theme::apply_forms(&mut composed, &job.theme);
        if ops::table::has_formula(&composed) {
            ops::table::fill_with(&mut composed, job.iter);
        }
        let run_fonts = paper::resolve_run_fonts(&mut composed);
        let laid = paper::layout_doc(&composed, &paper::DocOpts::default(), &run_fonts)?;
        let src = paper::PageSource {
            doc: composed,
            sheet: laid.sheet,
            page: laid.page,
            family: laid.family.clone(),
            font: Arc::new(laid.font.clone()),
            run_fonts: run_fonts.clone(),
            bg: None,
        };
        if i == 0 {
            let p = paper::Paper::from_page(&laid.page);
            size = (p.width_mm, p.height_mm);
        }
        let mut f = vec![(laid.family, laid.font)];
        f.extend(run_fonts);
        for leaf in paper::page_leaves(&src)? {
            pages.push((leaf, i));
        }
        fonts.push(f);
    }
    Ok(Preview { pages, fonts, paper: size })
}

impl Writer {
    /// The document the left side edits: the text as it is, one paragraph
    /// a line. The face and size are put on the laid-out copy (`lay`)
    pub(crate) fn enter_code(&mut self, p: &std::path::Path, text: &str) {
        self.native = false;
        self.target = Target::Body;
        self.hf_edit = None;
        self.track = false;
        self.track_base = None;
        self.encrypt_pw = None;
        self.docs.clear();
        self.doc_at = 0;
        self.code = Some(CodeView { edits: 1, ..Default::default() });
        self.pg = kumihan::PageSetup {
            left_mm: 6.0,
            right_mm: 6.0,
            top_mm: 6.0,
            bottom_mm: 6.0,
            ..Default::default()
        };
        self.set_doc(Document::plain(text));
        self.adopt_font();
        self.path = Some(p.to_path_buf());
        self.dirty = false;
    }

    /// What the other thread needs to make the pages of the text now
    pub(crate) fn job_now(&self) -> Result<Job, String> {
        let text = self.doc.body_text();
        let (docs, _) = kumihan::adoc::parse_many_full(&text).map_err(|e| e.to_string())?;
        let path = self.path.clone().unwrap_or_default();
        let (theme, _, _) = self.load_template(docs.first().and_then(|d| d.template.as_deref()), &path);
        Ok(Job { docs, theme, iter: ui::calc_iter_setting() })
    }

    /// The PDF of the text: the pages the right pane shows. The fonts of
    /// the documents in the file go into one list, and each letter is
    /// pointed at its face in that list
    pub(crate) fn code_pdf(&mut self, p: &std::path::Path) -> Result<(), String> {
        let pv = make_pages(self.job_now()?)?;
        let mut fonts: Vec<&[u8]> = Vec::new();
        let mut first = Vec::new();
        for f in &pv.fonts {
            first.push(fonts.len());
            fonts.extend(f.iter().map(|(_, b)| b.as_slice()));
        }
        let mut leaves = Vec::new();
        for (mut leaf, d) in pv.pages {
            let at = first[d];
            for piece in &mut leaf.pieces {
                piece.font = (piece.font as usize + at).min(255) as u8;
            }
            leaves.push(leaf);
        }
        kumihan::atomic::save(p, |f| {
            paper::pdfw::write_pages_fonts(&leaves, pv.paper.0, pv.paper.1, &fonts, std::io::BufWriter::new(f))
        })
    }

    /// The width of the right pane (px)
    pub(crate) fn code_preview_w(&self) -> f32 {
        (self.view_w_px * 0.5).round()
    }

    /// The width the left side lays its lines to (px)
    pub(crate) fn code_text_w(&self) -> f32 {
        (self.view_w_px - self.code_preview_w() - 60.0).max(200.0)
    }

    /// Called after every layout: the text may have changed
    pub(crate) fn code_touched(&mut self) {
        let w = self.code_text_w();
        if let Some(c) = self.code.as_mut() {
            c.laid_w = w;
            c.edits += 1;
            c.edited_at = Some(std::time::Instant::now());
        }
    }

    /// Make the pages again when the text has changed and the typing has
    /// paused. Called from `render`
    pub(crate) fn code_tick(&mut self, cx: &mut Context<Self>) {
        // The text flows to the pane: lay it out again when the window's
        // width has changed (the first layout runs before the window has one)
        if self.code.as_ref().is_some_and(|c| (c.laid_w - self.code_text_w()).abs() > 2.0) {
            self.relayout_keep();
        }
        let Some(c) = self.code.as_mut() else { return };
        if c.busy || c.built == c.edits {
            return;
        }
        let quiet = c.edited_at.is_none_or(|t| t.elapsed().as_millis() as u64 >= PAUSE_MS);
        if !quiet {
            if !c.waiting {
                c.waiting = true;
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(PAUSE_MS)).await;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(c) = this.code.as_mut() {
                            c.waiting = false;
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            return;
        }
        let edits = c.edits;
        let job = match self.job_now() {
            Ok(j) => j,
            Err(e) => {
                let c = self.code.as_mut().expect("code view");
                c.built = edits;
                c.err = Some(e);
                return;
            }
        };
        let c = self.code.as_mut().expect("code view");
        c.busy = true;
        let work = cx.background_executor().spawn(async move { make_pages(job) });
        cx.spawn(async move |this, cx| {
            let r = work.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(c) = this.code.as_mut() {
                    c.busy = false;
                    c.built = edits;
                    match r {
                        Ok(p) => {
                            c.pages = Some(Arc::new(p));
                            c.cache.clear();
                            c.err = None;
                        }
                        // The last pages stay; the reason is shown above them
                        Err(e) => c.err = Some(e),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The right pane: the pages fitted to its width, one under another
    pub(crate) fn code_pane(&mut self, scale: f32, cx: &mut Context<Self>) -> gpui::AnyElement {
        let w_px = self.code_preview_w();
        let view_h = self.view_h_px;
        let Some(c) = self.code.as_mut() else { return div().into_any_element() };
        let mut pane = div()
            .flex_none()
            .w(px(w_px))
            .h_full()
            .relative()
            .overflow_hidden()
            .bg(rgb(0x5A6068))
            .on_scroll_wheel(cx.listener(|this, e: &gpui::ScrollWheelEvent, _, cx| {
                let dy = match e.delta {
                    gpui::ScrollDelta::Pixels(p) => f32::from(p.y),
                    gpui::ScrollDelta::Lines(l) => l.y * 40.0,
                };
                if let Some(c) = this.code.as_mut() {
                    c.scroll_px = (c.scroll_px - dy).max(0.0);
                }
                cx.notify();
            }));
        let Some(pv) = c.pages.clone() else {
            return pane.child(div().p_4().text_color(rgb(0xFFFFFF)).child(ui::t!("code_pages_making"))).into_any_element();
        };
        // Fit the widest page to the pane, less a margin either side
        let widest = pv.pages.iter().map(|(l, _)| l.size_mm.map(|s| s.0).unwrap_or(pv.paper.0)).fold(pv.paper.0, f32::max);
        let pxmm = ((w_px - 24.0) / widest).max(0.5);
        let bai = pxmm * scale;
        let total: f32 = pv.pages.iter().map(|(l, _)| l.size_mm.map(|s| s.1).unwrap_or(pv.paper.1) + GAP_MM).sum::<f32>() * pxmm;
        c.scroll_px = c.scroll_px.min((total - view_h * 0.5).max(0.0));
        let mut y = 12.0 - c.scroll_px;
        for (k, (leaf, d)) in pv.pages.iter().enumerate() {
            let (w, h) = leaf.size_mm.unwrap_or(pv.paper);
            let hp = h * pxmm;
            if y + hp >= -view_h && y <= view_h * 2.0 {
                let key = (k, (bai * 100.0).round() as u32);
                let pic = match c.cache.get(&key) {
                    Some(p) => Some(p.clone()),
                    None => {
                        let fonts: Vec<&[u8]> = pv.fonts[*d].iter().map(|(_, b)| b.as_slice()).collect();
                        let p = crate::pages::picture(leaf, w, h, bai, &fonts);
                        if let Some(p) = &p {
                            c.cache.insert(key, p.clone());
                        }
                        p
                    }
                };
                if let Some(pic) = pic {
                    pane = pane.child(
                        gpui::img(pic).absolute().left(px(12.0)).top(px(y)).w(px(w * pxmm)).h(px(hp)).shadow_lg(),
                    );
                }
            }
            y += hp + GAP_MM * pxmm;
        }
        if let Some(e) = &c.err {
            pane = pane.child(
                div().absolute().left(px(0.0)).top(px(0.0)).w_full().p_2()
                    .bg(rgb(0xFFF4E5)).text_color(rgb(0x8A4B00))
                    .child(SharedString::from(ui::tf!("code_pages_cannot", e).to_string())),
            );
        }
        pane.into_any_element()
    }
}
