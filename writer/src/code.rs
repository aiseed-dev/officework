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

/// Numbers each split view, so pages made for one view never land in
/// another (a file opened while the pages of the last were being made)
static VIEWS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The buttons and keys that work on the text of an .adoc. The others
/// change the look or the structure of a document (a table, a picture, a
/// style), which the text does not hold, and they would be lost on saving
pub(crate) const CODE_OK: &[&str] = &[
    "open", "save", "undo", "redo", "selectall", "pdf", "replace", "spell", "wordcount",
    "copy", "cut", "paste", "zoom", "zoom-in", "zoom-out", "zoom100", "fit-page", "fit-width",
    "darkmode", "ui-bigger", "ui-smaller", "show-toolbar", "show-statusbar", "show-left",
    "show-right", "nav", "terminal", "ruler", "hidenchars", "ai-where", "py-folder",
];

/// The pages made from the text at one moment
pub(crate) struct Preview {
    /// Each page and the document (of the file) it belongs to
    pub pages: Vec<(paper::pdfw::Leaf, usize)>,
    /// The fonts of all the documents, each face once
    pub fonts: Vec<(String, Arc<Vec<u8>>)>,
    /// For each document, where each of its fonts (in the order its leaves
    /// name them) is in `fonts`
    pub font_at: Vec<Vec<usize>>,
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
        let Some(last) = (at..from.len()).take(len).take_while(|&i| plain(from[i])).last() else { continue };
        let end = last + 1;
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

impl Preview {
    /// The font bytes the leaves of document `d` name, in their order
    pub fn fonts_of(&self, d: usize) -> Vec<&[u8]> {
        self.font_at.get(d).map(|m| m.iter().map(|&i| self.fonts[i].1.as_slice()).collect()).unwrap_or_default()
    }
}

/// An entry of the engine's ledger of markup the pages do not show, for
/// the screen. The engine names each kind in Japanese ("取り込み(include::)(7
/// 行目)"); on a screen in another language the kind is shown by its
/// AsciiDoc markup, which reads the same in every language, and the line
/// in the screen's words
pub(crate) fn note_for_screen(entry: &str, ja: bool) -> String {
    if ja {
        return entry.to_string();
    }
    match note_parts(entry) {
        Some((mark, Some(n), line)) => ui::tf!("code_note_many", mark, n, line).to_string(),
        Some((mark, None, line)) => ui::tf!("code_note", mark, line).to_string(),
        None => entry.to_string(),
    }
}

/// A ledger entry, "kind(7 行目)" or "kind × 3(3 行目ほか)", as the kind's
/// markup, how many times it came (when more than once) and its line
fn note_parts(entry: &str) -> Option<(String, Option<usize>, String)> {
    let open = entry.rfind('(')?;
    let (head, tail) = (&entry[..open], &entry[open + '('.len_utf8()..]);
    let line: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    if line.is_empty() {
        return None;
    }
    let (kind, many) = match head.split_once(" × ") {
        Some((k, n)) => (k, n.parse::<usize>().ok()),
        None => (head, None),
    };
    // The markup inside the kind's own brackets, else the kind's markup
    let inner = kind.rfind('(').map(|i| kind[i + '('.len_utf8()..].trim_end_matches(')').to_string());
    let mark = match inner.as_deref() {
        Some(".題") => ".Title".to_string(),
        Some(":名前: 値") => ":name: value".to_string(),
        Some(m) if !m.is_empty() => m.to_string(),
        _ => match kind {
            "横の区切り線" => "'''".into(),
            "チェックの箇条書き" => "* [x]".into(),
            "コードの塊" => "----".into(),
            "字のまま出す塊" => "....".into(),
            "例の塊" => "====".into(),
            "傍注の塊" => "****".into(),
            "覚え書きの塊" => "////".into(),
            "そのまま通す塊" => "++++".into(),
            k if k.starts_with("開いた塊") || k == "塊の中" => "--".into(),
            k => k.into(),
        },
    };
    Some((mark, many, line))
}

/// The state of the split view
#[derive(Default)]
pub(crate) struct CodeView {
    /// Which view this is (`VIEWS`)
    pub id: u64,
    /// The file's line ends are CR LF, and whether it ends with a newline:
    /// saving gives them back
    pub crlf: bool,
    pub final_nl: bool,
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
    /// Pictures no longer shown, given back to the GPU at the next draw
    pub drop_later: Vec<Arc<gpui::RenderImage>>,
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

/// Write pages made by [`make_pages`] to a PDF: each letter is pointed at
/// its face in the one list of the file's fonts, and the strokes drawn by
/// hand (`ink`) go on the pages of their document
fn write_preview(pv: Preview, ink: Option<(usize, &[kumihan::Stroke])>, p: &std::path::Path) -> Result<(), String> {
    if pv.fonts.len() > 256 {
        return Err(ui::tf!("code_too_many_fonts", pv.fonts.len()).to_string());
    }
    let fonts: Vec<&[u8]> = pv.fonts.iter().map(|(_, b)| b.as_slice()).collect();
    let mut leaves = Vec::new();
    let mut of_doc = Vec::new();
    for (mut leaf, d) in pv.pages {
        let at = &pv.font_at[d];
        for piece in &mut leaf.pieces {
            piece.font = at.get(piece.font as usize).copied().unwrap_or(0) as u8;
        }
        leaves.push(leaf);
        of_doc.push(d);
    }
    if let Some((d, strokes)) = ink.filter(|(_, s)| !s.is_empty()) {
        if let (Some(a), Some(b)) = (of_doc.iter().position(|&x| x == d), of_doc.iter().rposition(|&x| x == d)) {
            paper::pdfw::put_ink(&mut leaves[a..=b], strokes, pv.paper.1);
        }
    }
    kumihan::atomic::save(p, |f| {
        paper::pdfw::write_pages_fonts(&leaves, pv.paper.0, pv.paper.1, &fonts, std::io::BufWriter::new(f))
    })
}

pub(crate) fn make_pages(job: Job) -> Result<Preview, String> {
    let mut pages = Vec::new();
    let mut fonts: Vec<(String, Arc<Vec<u8>>)> = Vec::new();
    let mut font_at = Vec::new();
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
        for leaf in paper::page_leaves(&src)? {
            pages.push((leaf, i));
        }
        // The documents of one file share their faces: keep each once
        let mut at = Vec::new();
        for (name, bytes) in std::iter::once((laid.family, laid.font)).chain(run_fonts) {
            match fonts.iter().position(|(n, _)| *n == name) {
                Some(k) => at.push(k),
                None => {
                    at.push(fonts.len());
                    fonts.push((name, Arc::new(bytes)));
                }
            }
        }
        font_at.push(at);
    }
    let lines = lines_of(&pages, size);
    Ok(Preview { pages, fonts, font_at, paper: size, lines, notes: job.notes })
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
        // Nothing of the file before stays: its look, its template and the
        // lock someone else held on it
        self.tmpl = kumihan::theme::default_theme();
        self.tmpl_path = None;
        self.locked_by = None;
        self.opened += 1;
        let id = VIEWS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.code = Some(CodeView { id, edits: 1, final_nl: true, ..Default::default() });
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

    /// The text of the file as a save writes it: its own line ends, and a
    /// final newline when the file had one
    pub(crate) fn file_text(&self) -> String {
        let mut text = self.doc.body_text();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        if let Some(c) = &self.code {
            if !c.final_nl {
                text.pop();
            }
            if c.crlf {
                text = text.replace('\n', "\r\n");
            }
        }
        text
    }

    /// The AsciiDoc of what is being edited: the file's text in the split
    /// view, else every document of the file written out
    pub(crate) fn adoc_text(&self) -> String {
        if self.code.is_some() {
            self.file_text()
        } else if self.docs.len() > 1 {
            kumihan::adoc::write_many(&self.docs_for_save())
        } else {
            kumihan::adoc::write(&self.doc)
        }
    }

    /// Run `f` with the document the text makes in place of the text, as
    /// an .adoc opened on its pages would be (to write a docx or HTML from
    /// it), then put the text back as it was
    pub(crate) fn as_parsed<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> Result<R, String> {
        self.flush_target();
        let text = self.doc.body_text();
        let (docs, _) = kumihan::adoc::parse_many_full(&text).map_err(|e| e.to_string())?;
        let path = self.path.clone().unwrap_or_default();
        let (theme, tmpl_path, _) = self.load_template(docs.first().and_then(|d| d.template.as_deref()), &path);
        let first = docs.first().cloned().unwrap_or_default();
        let target = self.target;
        let keep = self.take_open();
        self.path = keep.path.clone();
        self.dirty = keep.dirty;
        self.ed = Editor::new(&first.body_text());
        self.doc = first;
        self.docs = if docs.len() > 1 { docs } else { Vec::new() };
        self.doc_at = 0;
        self.native = true;
        // The paper the template gives, as an .adoc opened on its pages has
        self.pg = theme.page.unwrap_or_default();
        self.tmpl = theme;
        self.tmpl_path = tmpl_path;
        self.target = Target::Body;
        let r = f(self);
        self.put_open(keep);
        self.target = target;
        Ok(r)
    }

    /// The PDF of the text: the pages the right pane shows, laid out with
    /// the print template when the folder has one (as the PDF of an .adoc
    /// opened on its pages is). Each letter is pointed at its face in the
    /// one list of the file's fonts
    pub(crate) fn code_pdf(&mut self, p: &std::path::Path) -> Result<Option<String>, String> {
        let mut job = self.job_now()?;
        let (theme, used) = self.as_parsed(|w| w.template_for("印刷"))?;
        job.theme = theme;
        write_preview(make_pages(job)?, None, p)?;
        Ok(used)
    }

    /// The PDF of an .adoc edited on its pages: laid out as the split view
    /// and the engine lay it out (template, forms, table formulas), with
    /// the print template when the folder has one, and the strokes drawn by
    /// hand on the document shown. Returns the print template used
    pub(crate) fn native_pdf(&mut self, p: &std::path::Path) -> Result<Option<String>, String> {
        self.flush_target();
        let (theme, used) = self.template_for("印刷");
        let at = if self.docs.len() > 1 { self.doc_at } else { 0 };
        let job = Job { docs: self.docs_for_save(), theme, iter: ui::calc_iter_setting(), notes: Vec::new() };
        write_preview(make_pages(job)?, Some((at, &self.doc.ink)), p)?;
        Ok(used)
    }

    /// The PDF of a docx whose screen pages cannot be used (vertical text,
    /// or no layout yet): the engine's pages of the document, as
    /// `Doc.save("x.pdf")` writes them, with the strokes drawn by hand
    pub(crate) fn docx_pdf(&mut self, p: &std::path::Path) -> Result<(), String> {
        self.flush_target();
        let mut pages = paper::doc_pages(&self.doc, None)?;
        paper::pdfw::put_ink(&mut pages.leaves, &self.doc.ink, pages.paper.height_mm);
        let fonts: Vec<&[u8]> = pages.fonts.iter().map(|(_, b)| b.as_slice()).collect();
        kumihan::atomic::save(p, |f| {
            paper::pdfw::write_pages_fonts(
                &pages.leaves,
                pages.paper.width_mm,
                pages.paper.height_mm,
                &fonts,
                std::io::BufWriter::new(f),
            )
        })
    }

    /// The width of the right pane (px)
    pub(crate) fn code_preview_w(&self) -> f32 {
        (self.view_w_px * 0.5).round()
    }

    /// The width the left side lays its lines to (px on the screen)
    pub(crate) fn code_text_w(&self) -> f32 {
        let panel = if self.rp_open { crate::panels::RP_PANEL_W } else { 0.0 };
        (self.view_w_px - self.code_preview_w() - panel - 60.0).max(200.0)
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
        let id = c.id;
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
                if let Some(c) = this.code.as_mut().filter(|c| c.id == id) {
                    c.busy = false;
                    c.built = edits;
                    match r {
                        Ok(p) => {
                            c.pages = Some(Arc::new(p));
                            let old: Vec<_> = c.cache.drain().map(|(_, v)| v).collect();
                            c.drop_later.extend(old);
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
        let Some(c) = self.code.as_mut() else { return };
        if c.synced == Some(cur) {
            return;
        }
        let Some(pv) = c.pages.clone() else { return };
        c.synced = Some(cur);
        let text = self.ed.text().to_string();
        let src: Vec<char> = text.chars().collect();
        let cur = cur.min(text.len());
        let at = text[..visible_from(&text, cur)].chars().count();
        let (pt, starts) = page_text(&pv.lines);
        // A caret at the end of a line, or on a line of markings only, has
        // no letter after it: the letters its line starts with are used
        let line_start = text[..cur].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let from_line = text[..visible_from(&text, line_start)].chars().count();
        let Some(j) = match_place(&src, at, &pt).or_else(|| match_place(&src, from_line, &pt)) else { return };
        let Some(c) = self.code.as_mut() else { return };
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
        let now = (bai * 100.0).round() as u32;
        let mut shown = std::collections::HashSet::new();
        for (k, (leaf, d)) in pv.pages.iter().enumerate() {
            let (w, h) = leaf.size_mm.unwrap_or(pv.paper);
            let (y, hp) = (c.tops[k] - c.scroll_px, h * pxmm);
            if y + hp < -view_h || y > view_h * 2.0 {
                continue;
            }
            shown.insert(k);
            let key = (k, now);
            let pic = match c.cache.get(&key) {
                Some(p) => Some(p.clone()),
                None => {
                    let fonts = pv.fonts_of(*d);
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
        // Only the pages near the view stay, at this zoom
        let drop = &mut c.drop_later;
        c.cache.retain(|(k, b), v| {
            let keep = *b == now && shown.contains(k);
            if !keep {
                drop.push(v.clone());
            }
            keep
        });
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
            let ja = ui::settings::language() == "ja";
            let list: Vec<String> = pv.notes.iter().map(|n| note_for_screen(n, ja)).collect();
            notes = notes.child(
                div().p_2().bg(rgb(0xEEF3F7)).text_color(rgb(0x2B4150))
                    .child(SharedString::from(
                        ui::tf!("uses_markup_not_handle", name, list.join(if ja { "・" } else { ", " })).to_string(),
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

    /// The ledger names a kind by its markup on a screen in another
    /// language, and stays as the engine wrote it on a Japanese screen
    #[test]
    fn a_ledger_entry_is_shown_by_its_markup() {
        let one = "取り込み(include::)(7 行目)";
        assert_eq!(note_for_screen(one, true), one);
        assert_eq!(note_parts(one), Some(("include::".into(), None, "7".into())));
        assert_eq!(note_parts("横の区切り線 × 3(2 行目ほか)"), Some(("'''".into(), Some(3), "2".into())));
        assert_eq!(note_parts("塊の題(.題)(4 行目)").map(|p| p.0), Some(".Title".into()));
        assert_eq!(note_parts("字下げの段落(literal)(9 行目)").map(|p| p.0), Some("literal".into()));
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
