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
    /// The letters on the pages, line by line, to find the text a place
    /// on a page comes from and the other way round
    pub lines: Vec<PLine>,
    /// Markup the pages do not read, with the file's line numbers
    pub notes: Vec<String>,
}

/// One line of letters on a page
pub(crate) struct PLine {
    pub page: usize,
    /// From the top of the page (mm)
    pub top_mm: f32,
    pub h_mm: f32,
    /// Each letter and where it starts (mm from the left of the page)
    pub chars: Vec<(char, f32)>,
}

/// Put the pieces of the pages into lines: pieces on one page that share a
/// baseline, left to right
fn lines_of(pages: &[(paper::pdfw::Leaf, usize)], paper: (f32, f32)) -> Vec<PLine> {
    let mut out: Vec<PLine> = Vec::new();
    for (k, (leaf, _)) in pages.iter().enumerate() {
        let h = leaf.size_mm.map(|s| s.1).unwrap_or(paper.1);
        let mut rows: Vec<(f32, f32, Vec<(char, f32)>)> = Vec::new();
        for p in &leaf.pieces {
            let n = p.text.chars().count();
            if n == 0 || p.rotation.abs() > 0.01 {
                continue;
            }
            let size = p.size_pt * 25.4 / 72.0;
            let step = p.w_mm / n as f32;
            let letters = p.text.chars().enumerate().map(|(i, c)| (c, p.x_mm + step * i as f32));
            match rows.iter_mut().find(|r| (r.0 - p.y_mm).abs() < 0.6) {
                Some(r) => {
                    r.1 = r.1.max(size);
                    r.2.extend(letters);
                }
                None => rows.push((p.y_mm, size, letters.collect())),
            }
        }
        // Top of the page first
        rows.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (y, size, mut chars) in rows {
            chars.sort_by(|a, b| a.1.total_cmp(&b.1));
            out.push(PLine { page: k, top_mm: h - y - size * 0.9, h_mm: size * 1.2, chars });
        }
    }
    out
}

/// A letter that a marking in the text never is
fn plain(c: char) -> bool {
    !matches!(c, '*' | '_' | '`' | '#' | '^' | '~' | '+' | '[' | ']' | '|' | '<' | '>' | '{' | '}' | '\\' | '\n')
}

/// Where `needle` comes for the `n`-th time in `hay` (0 is the first)
fn nth(hay: &[char], needle: &[char], n: usize) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    let mut seen = 0usize;
    for i in 0..=hay.len() - needle.len() {
        if &hay[i..i + needle.len()] == needle {
            if seen == n {
                return Some(i);
            }
            seen += 1;
        }
    }
    None
}

/// How many times `needle` comes in `hay` before `upto`
fn count_before(hay: &[char], needle: &[char], upto: usize) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i + needle.len() <= upto.min(hay.len()) {
        if &hay[i..i + needle.len()] == needle {
            n += 1;
        }
        i += 1;
    }
    n
}

/// Find in `to` the place that `at` is in `from`: the letters from `at`
/// onwards (as many as agree, 8 down to 1, stopping at a marking), the
/// same time they come. When they come fewer times in `to`, the place of
/// the same share of the way through
pub(crate) fn match_place(from: &[char], at: usize, to: &[char]) -> Option<usize> {
    if at >= from.len() || to.is_empty() {
        return None;
    }
    for len in [8usize, 5, 3, 2, 1] {
        let end = (at..from.len()).take(len).take_while(|&i| plain(from[i])).last()? + 1;
        let needle = &from[at..end];
        if needle.len() < len.min(2) || needle.iter().all(|c| c.is_whitespace()) {
            continue;
        }
        let k = count_before(from, needle, at);
        if let Some(i) = nth(to, needle, k) {
            return Some(i);
        }
        // Fewer in `to`: the one nearest the same share of the way
        let share = at as f32 / from.len() as f32;
        let mut best: Option<(f32, usize)> = None;
        let mut i = 0;
        while let Some(j) = nth(&to[i..], needle, 0) {
            let d = ((i + j) as f32 / to.len() as f32 - share).abs();
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, i + j));
            }
            i += j + 1;
        }
        if let Some((_, j)) = best {
            return Some(j);
        }
    }
    None
}

/// The letters of the pages, one line after another, and where each line
/// starts in them
fn page_text(lines: &[PLine]) -> (Vec<char>, Vec<usize>) {
    let mut text = Vec::new();
    let mut starts = Vec::new();
    for l in lines {
        starts.push(text.len());
        text.extend(l.chars.iter().map(|(c, _)| *c));
        text.push('\n');
    }
    (text, starts)
}

/// The first letter of the text from `byte` on that can be seen on a page:
/// the markings at the head of a line (`== `, `* `, `|`) are skipped
fn visible_from(text: &str, byte: usize) -> usize {
    let line_end = text[byte..].find('\n').map(|i| byte + i).unwrap_or(text.len());
    let rest = &text[byte..line_end];
    match rest.char_indices().find(|(_, c)| c.is_alphanumeric()) {
        Some((i, _)) => byte + i,
        None => byte,
    }
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
    /// Where the pane is in the window (px), kept by the pane as it paints
    pub origin: std::rc::Rc<std::cell::Cell<(f32, f32)>>,
    /// Where each page starts in the pane, before scrolling (px), and the
    /// pixels per mm the pages were drawn at
    pub tops: Vec<f32>,
    pub pxmm: f32,
    /// The caret the pages were last brought to
    pub synced: Option<usize>,
    /// The line of a page the caret is on, shown with a band
    pub mark: Option<usize>,
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
    notes: Vec<String>,
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
    let lines = lines_of(&pages, size);
    Ok(Preview { pages, fonts, paper: size, lines, notes: job.notes })
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
        let (docs, notes) = kumihan::adoc::parse_many_full(&text).map_err(|e| e.to_string())?;
        let path = self.path.clone().unwrap_or_default();
        let (theme, _, _) = self.load_template(docs.first().and_then(|d| d.template.as_deref()), &path);
        Ok(Job { docs, theme, iter: ui::calc_iter_setting(), notes })
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
        self.code_sync();
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
                            c.synced = None;
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

    /// Bring the pages to the caret: find the line of a page the caret's
    /// text is on, mark it, and scroll to it when it is out of sight
    pub(crate) fn code_sync(&mut self) {
        let cur = self.ed.cursor();
        let view_h = self.view_h_px;
        let text = self.ed.text().to_string();
        let Some(c) = self.code.as_mut() else { return };
        if c.synced == Some(cur) {
            return;
        }
        let Some(pv) = c.pages.clone() else { return };
        c.synced = Some(cur);
        let src: Vec<char> = text.chars().collect();
        let at = text[..visible_from(&text, cur.min(text.len()))].chars().count();
        let (pt, starts) = page_text(&pv.lines);
        let Some(j) = match_place(&src, at, &pt) else { return };
        let li = starts.partition_point(|&s| s <= j).saturating_sub(1);
        c.mark = Some(li);
        let Some(l) = pv.lines.get(li) else { return };
        let Some(top) = c.tops.get(l.page) else { return };
        let y = top + l.top_mm * c.pxmm;
        if y < c.scroll_px + 20.0 || y > c.scroll_px + view_h - 60.0 {
            c.scroll_px = (y - view_h / 3.0).max(0.0);
        }
    }

    /// A press on the pages puts the caret on the text it comes from
    pub(crate) fn code_click(&mut self, x: f32, y: f32) {
        let text = self.ed.text().to_string();
        let Some(c) = self.code.as_mut() else { return };
        let Some(pv) = c.pages.clone() else { return };
        let (ox, oy) = c.origin.get();
        let (x, y) = (x - ox, y - oy + c.scroll_px);
        let Some(k) = c.tops.iter().rposition(|&t| t <= y) else { return };
        let (x_mm, y_mm) = ((x - 12.0) / c.pxmm, (y - c.tops[k]) / c.pxmm);
        // The line under the press, else the nearest on that page
        let Some((li, l)) = pv
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.page == k && !l.chars.is_empty())
            .min_by(|a, b| {
                let d = |l: &PLine| (l.top_mm + l.h_mm / 2.0 - y_mm).abs();
                d(a.1).total_cmp(&d(b.1))
            })
        else {
            return;
        };
        let ci = l.chars.iter().rposition(|(_, cx)| *cx <= x_mm).unwrap_or(0);
        let (pt, starts) = page_text(&pv.lines);
        let src: Vec<char> = text.chars().collect();
        let Some(j) = match_place(&pt, starts[li] + ci, &src) else { return };
        let byte = text.char_indices().nth(j).map(|(b, _)| b).unwrap_or(text.len());
        c.mark = Some(li);
        c.synced = Some(byte);
        if self.target != Target::Body {
            self.switch_target(Target::Body);
        }
        self.ed.move_to(byte, false);
        self.follow_caret();
    }

    /// The right pane: the pages fitted to its width, one under another
    pub(crate) fn code_pane(&mut self, scale: f32, cx: &mut Context<Self>) -> gpui::AnyElement {
        let w_px = self.code_preview_w();
        let view_h = self.view_h_px;
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some(c) = self.code.as_mut() else { return div().into_any_element() };
        let origin = c.origin.clone();
        let mut pane = div()
            .flex_none()
            .w(px(w_px))
            .h_full()
            .relative()
            .overflow_hidden()
            .bg(rgb(0x5A6068))
            // Where the pane is in the window, for the presses on the pages
            .child(
                gpui::canvas(
                    move |b: gpui::Bounds<gpui::Pixels>, _, _| {
                        origin.set((f32::from(b.origin.x), f32::from(b.origin.y)));
                    },
                    |_, _: (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, e: &gpui::MouseDownEvent, _, cx| {
                    this.code_click(f32::from(e.position.x), f32::from(e.position.y));
                    cx.notify();
                }),
            )
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
        c.pxmm = pxmm;
        c.tops.clear();
        let mut at = 12.0;
        for (leaf, _) in &pv.pages {
            c.tops.push(at);
            at += (leaf.size_mm.map(|s| s.1).unwrap_or(pv.paper.1) + GAP_MM) * pxmm;
        }
        c.scroll_px = c.scroll_px.min((at - view_h * 0.5).max(0.0));
        for (k, (leaf, d)) in pv.pages.iter().enumerate() {
            let (w, h) = leaf.size_mm.unwrap_or(pv.paper);
            let (y, hp) = (c.tops[k] - c.scroll_px, h * pxmm);
            if y + hp < -view_h || y > view_h * 2.0 {
                continue;
            }
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
        // The line the caret's text is on
        if let Some(l) = c.mark.and_then(|i| pv.lines.get(i)) {
            let (w, _) = pv.pages[l.page].0.size_mm.unwrap_or(pv.paper);
            pane = pane.child(
                div()
                    .absolute()
                    .left(px(12.0))
                    .top(px(c.tops[l.page] - c.scroll_px + l.top_mm * pxmm))
                    .w(px(w * pxmm))
                    .h(px(l.h_mm * pxmm))
                    .bg(gpui::Rgba { r: 1.0, g: 0.85, b: 0.2, a: 0.28 }),
            );
        }
        // What went wrong, and the markup the pages do not read, with the
        // lines of the file they are on
        let mut notes = div().absolute().left(px(0.0)).top(px(0.0)).w_full().flex().flex_col();
        if let Some(e) = &c.err {
            notes = notes.child(
                div().p_2().bg(rgb(0xFFF4E5)).text_color(rgb(0x8A4B00))
                    .child(SharedString::from(ui::tf!("code_pages_cannot", e).to_string())),
            );
        }
        if !pv.notes.is_empty() {
            notes = notes.child(
                div().p_2().bg(rgb(0xEEF3F7)).text_color(rgb(0x2B4150))
                    .child(SharedString::from(
                        ui::tf!("uses_markup_not_handle", name, pv.notes.join("・")).to_string(),
                    )),
            );
        }
        pane.child(notes).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    /// A place on the pages finds its text, past the markings, and the
    /// second of two equal words finds the second
    #[test]
    fn a_place_on_the_pages_finds_its_text() {
        let src = chars("= 申請書\n\n== 目的\n\n目的は*太字*です。目的は二つ。\n");
        let page = chars("申請書\n目的\n目的は太字です。目的は二つ。\n");
        // "太字" on the page is inside the markings in the text
        let at = page.iter().position(|&c| c == '太').unwrap();
        let j = match_place(&page, at, &src).unwrap();
        assert_eq!(src[j], '太');
        // The third "目的" on the page is the third in the text
        let third = nth(&page, &chars("目的"), 2).unwrap();
        let j = match_place(&page, third, &src).unwrap();
        assert_eq!(j, nth(&src, &chars("目的"), 2).unwrap());
    }

    /// The caret on a heading's markings is on the heading's words
    #[test]
    fn the_caret_on_a_marking_is_on_the_words_after_it() {
        let text = "== 目的\n本文\n";
        assert_eq!(&text[visible_from(text, 0)..], "目的\n本文\n");
        let src = chars(text);
        let page = chars("目的\n本文\n");
        let at = text[..visible_from(text, 0)].chars().count();
        assert_eq!(match_place(&src, at, &page), Some(0));
    }
}
