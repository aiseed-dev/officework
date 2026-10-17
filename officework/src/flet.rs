//! **officework as the window of a Flet program** (SEKKEI "検討: officework を
//! Flet の画面側にする"; the rules are in docs/sekkei/flet-client.ja.adoc).
//!
//! Flet starts its client as `flet <page url> <pid file> [assets]`. When
//! officework is started that way (Flet finds it through `FLET_VIEW_PATH`,
//! under the name `flet`), it writes the pid file, connects to the page as
//! the client, keeps the control tree the Python side sends, and ends when
//! the Python side closes the socket.
//!
//! What is drawn (step 3, 2026-10-17): Page, View, Column, Row, Text,
//! Button and TextField, with the properties the Dart client reads
//! (`packages/flet/lib/src/controls/*.dart`). A button's click, a field's
//! change and submit go to Python as events; a field's value is written back
//! with UPDATE_CONTROL_PROPS; the window's size is written to the Page. A
//! control of any other type is drawn as its name, so that a program using
//! one sees what is missing rather than nothing.

use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gpui::{
    div, prelude::*, px, size, AnyElement, App, Bounds, Context, ElementInputHandler, Entity, EntityInputHandler,
    FocusHandle, FontWeight, KeyDownEvent, SharedString, UTF16Selection, Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;
use kumihan::Editor;
use ui::{handler, HasEditor};

use fletview::client::{apply, page_props};
use fletview::message::RegisterResponse;
use fletview::{Client, Control, ControlId, Incoming, Sender, Tree, Value};

/// The arguments Flet starts the client with
pub struct Args {
    pub url: String,
    pub pid_file: PathBuf,
    /// The assets folder; used once images are drawn
    #[allow(dead_code)]
    pub assets: Option<PathBuf>,
}

#[cfg(unix)]
fn is_socket(path: &str) -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::metadata(path).map(|m| m.file_type().is_socket()).unwrap_or(false)
}

#[cfg(not(unix))]
fn is_socket(_path: &str) -> bool {
    false
}

/// Recognises `officework <url> <pid file> [assets]`: the first argument is
/// `tcp://host:port` or a Unix socket, and a pid file follows. A document
/// path never looks like that.
pub fn parse(args: &[String]) -> Option<Args> {
    let url = args.get(1)?;
    let pid_file = args.get(2)?;
    if !(url.starts_with("tcp://") || is_socket(url)) {
        return None;
    }
    Some(Args { url: url.clone(), pid_file: PathBuf::from(pid_file), assets: args.get(3).map(PathBuf::from) })
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

/// What the socket thread hands to the window
enum Event {
    Registered(RegisterResponse, Sender),
    In(Incoming),
    /// The socket closed or failed; the text says which
    Ended(String),
}

type Queue = Arc<Mutex<Vec<Event>>>;

const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;
/// Material's default body text, which Flet's Text uses
const TEXT_PX: f32 = 14.0;
/// Material's primary colour, which Flet's Button uses by default
const PRIMARY: u32 = 0x6750A4;

/// What a control sits in. Flutter gives a child bounded or unbounded room
/// per axis: a Column bounds the width and leaves the height unbounded, a
/// Row the other way round, a View (the page) bounds both and is a column
/// for `expand`, a Button's content is bounded both ways with no `expand`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Parent {
    Bounded,
    View,
    Column,
    Row,
}

impl Parent {
    fn bounds_width(self) -> bool {
        self != Parent::Row
    }
    fn bounds_height(self) -> bool {
        self != Parent::Column
    }
    /// The axis `expand` grows along
    fn expand_axis(self) -> Option<Parent> {
        match self {
            Parent::Bounded => None,
            Parent::View | Parent::Column => Some(Parent::Column),
            Parent::Row => Some(Parent::Row),
        }
    }
}

/// A TextField being edited here: the editor (cursor, selection, undo, IME)
/// and the value Python last saw
struct Field {
    ed: Editor,
    focus: FocusHandle,
    sent: String,
}

/// The window of a Flet page
struct FletView {
    tree: Tree,
    sender: Option<Sender>,
    status: String,
    title: String,
    fields: HashMap<ControlId, Field>,
    /// The field that has the keyboard
    focused: Option<ControlId>,
    /// An editor to hand out when no field is focused
    scratch: Editor,
    /// The window size last written to the Page
    reported: Option<(f32, f32)>,
}

impl FletView {
    fn new() -> FletView {
        FletView {
            tree: Tree::new(),
            sender: None,
            status: "つないでいます".into(),
            title: "Flet".into(),
            fields: HashMap::new(),
            focused: None,
            scratch: Editor::new(""),
            reported: None,
        }
    }

    fn take(&mut self, ev: Event, cx: &mut Context<Self>) {
        match ev {
            Event::Registered(r, sender) => {
                match self.tree.set_root(&r.page_patch) {
                    Ok(_) => self.status = format!("登録しました(session {})", r.session_id.unwrap_or_default()),
                    Err(e) => self.status = format!("Page が読めません: {e}"),
                }
                self.sender = Some(sender);
            }
            Event::In(incoming) => match apply(&mut self.tree, &incoming) {
                Ok(Some(changed)) => self.status = format!("差分を当てました({} 個の部品)", changed.len()),
                Ok(None) => match &incoming {
                    Incoming::Crashed(m) => self.status = format!("Python が落ちました: {m}"),
                    Incoming::Invoke(req) => {
                        // No method is implemented yet; say so rather than let Python wait
                        if let Some(s) = &self.sender {
                            let _ = s.invoke_result(req, Value::Nil, Some(format!("{} は未対応です", req.name)));
                        }
                    }
                    _ => {}
                },
                Err(e) => self.status = format!("差分が当てられません: {e}"),
            },
            Event::Ended(why) => {
                self.status = why;
                // Flet's own client closes with the program; so does this window
                cx.quit();
            }
        }
        if let Some(t) = self.tree.root.and_then(|r| self.tree.get(r)).and_then(|p| p.get_str("title")) {
            self.title = t.to_string();
        }
        self.sync_fields(cx);
        cx.notify();
    }

    /// Gives every TextField an editor, and takes a value Python changed
    fn sync_fields(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<(ControlId, String)> = self
            .tree
            .controls()
            .filter(|c| c.ctype == "TextField")
            .map(|c| (c.id, c.get_str("value").unwrap_or("").to_string()))
            .collect();
        for (id, value) in ids {
            match self.fields.get_mut(&id) {
                Some(f) => {
                    if f.sent != value {
                        f.ed = Editor::new(&value);
                        f.sent = value;
                    }
                }
                None => {
                    self.fields.insert(id, Field { ed: Editor::new(&value), focus: cx.focus_handle(), sent: value });
                }
            }
        }
    }

    /// After typing: the field's value goes to Python's control, and to its
    /// `on_change` when it has one
    fn flush(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.focused else { return };
        let Some(f) = self.fields.get_mut(&id) else { return };
        let value = f.ed.text().to_string();
        if value == f.sent {
            return;
        }
        f.sent = value.clone();
        let _ = self.tree.update(id, &Value::map(vec![("value", Value::str(&value))]));
        if let Some(s) = &self.sender {
            let _ = s.update_props(id, Value::map(vec![("value", Value::str(&value))]));
            if self.tree.get(id).is_some_and(|c| c.has_event("change")) {
                let _ = s.send_event(id, "change", Value::str(&value));
            }
        }
        cx.notify();
    }

    fn event(&self, id: ControlId, name: &str, data: Value) {
        if let (Some(s), Some(c)) = (&self.sender, self.tree.get(id)) {
            if c.has_event(name) {
                let _ = s.send_event(id, name, data);
            }
        }
    }

    /// The window's size, written to the Page when it changes
    fn report_size(&mut self, window: &Window) {
        let s = window.viewport_size();
        let now = (f32::from(s.width), f32::from(s.height));
        if self.reported == Some(now) {
            return;
        }
        // Before the page is registered there is nothing to write it to; the
        // size is reported once the page exists
        let Some(root) = self.tree.root else { return };
        self.reported = Some(now);
        let props = Value::map(vec![("width", Value::F64(now.0 as f64)), ("height", Value::F64(now.1 as f64))]);
        let _ = self.tree.update(root, &props);
        if let Some(s) = &self.sender {
            let _ = s.update_props(root, props.clone());
            if self.tree.get(root).is_some_and(|c| c.has_event("resize")) {
                let _ = s.send_event(root, "resize", props);
            }
        }
    }

    // ---- drawing ----------------------------------------------------------

    /// Draws a control. `parent` is the axis of the Column or Row it sits in
    /// (None when the parent is bounded both ways, like a View): there
    /// `expand` means something, and the other axis is bounded.
    fn element(&self, id: ControlId, parent: Parent, cx: &mut Context<Self>) -> AnyElement {
        let Some(c) = self.tree.get(id) else { return div().into_any_element() };
        let el: AnyElement = match c.ctype.as_str() {
            "View" => self.view(c, cx),
            "Column" => self.column(c, true, cx),
            "Row" => self.column(c, false, cx),
            "Text" => self.text(c),
            "Button" => self.button(c, cx),
            "TextField" => self.text_field(c, cx),
            other => div()
                .px_2()
                .py_1()
                .border_1()
                .border_dashed()
                .border_color(gpui::rgb(0xC0C0C0))
                .text_color(gpui::rgb(0x888888))
                .child(format!("{other}(未対応)"))
                .into_any_element(),
        };
        self.layout(div().child(el), c, parent).into_any_element()
    }

    /// Whether a control takes all the width it is offered: a View (the
    /// page fills the window), a Row (unless tight), a TextField, and a
    /// Column holding one of those (Flutter sizes a Column to its widest child)
    fn wants_full_width(&self, c: &Control) -> bool {
        let tight = c.get_bool("tight").unwrap_or(false);
        match c.ctype.as_str() {
            "View" => true,
            "Row" => !tight,
            "TextField" => true,
            "Column" => self.tree.children(c.id, "controls").iter().any(|k| self.tree.get(*k).is_some_and(|k| self.wants_full_width(k))),
            _ => false,
        }
    }

    /// Whether a control takes all the height it is offered: a View, a
    /// Column (unless tight), and a Row holding one
    fn wants_full_height(&self, c: &Control) -> bool {
        let tight = c.get_bool("tight").unwrap_or(false);
        match c.ctype.as_str() {
            "View" => true,
            "Column" => !tight,
            "Row" => self.tree.children(c.id, "controls").iter().any(|k| self.tree.get(*k).is_some_and(|k| self.wants_full_height(k))),
            _ => false,
        }
    }

    /// The properties every control has (`LayoutControl`): size, expand,
    /// opacity, visibility, and Flutter's sizing: a control takes the room
    /// it is offered on an axis only when that room is bounded
    fn layout(&self, mut d: gpui::Div, c: &Control, parent: Parent) -> gpui::Div {
        d = d.flex().flex_col();
        if parent.bounds_width() && self.wants_full_width(c) {
            d = d.w_full();
        }
        if parent.bounds_height() && self.wants_full_height(c) {
            d = d.h_full();
        }
        if let Some(w) = c.get_f64("width") {
            d = d.w(px(w as f32)).flex_shrink_0();
        }
        if let Some(h) = c.get_f64("height") {
            d = d.h(px(h as f32)).flex_shrink_0();
        }
        if parent.expand_axis().is_some() {
            let expand = match c.get_value("expand") {
                Some(Value::Bool(true)) => Some(1.0),
                Some(v) => v.as_f64().filter(|n| *n > 0.0),
                None => None,
            };
            if let Some(n) = expand {
                d = d.flex_basis(px(0.0)).flex_grow(n as f32).min_h_0().min_w_0();
            }
        }
        if let Some(o) = c.get_f64("opacity") {
            d = d.opacity(o as f32);
        }
        if c.get_bool("disabled") == Some(true) {
            d = d.opacity(0.5);
        }
        d
    }

    fn children(&self, id: ControlId, prop: &str, parent: Parent, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.tree.children(id, prop).into_iter().map(|cid| self.element(cid, parent, cx)).collect()
    }

    /// A View: the page's root column, padded (10 by default)
    fn view(&self, c: &Control, cx: &mut Context<Self>) -> AnyElement {
        let pad = c.get("padding").and_then(|p| p.as_value()).map(edge_insets).unwrap_or([10.0; 4]);
        let spacing = c.get_f64("spacing").unwrap_or(10.0) as f32;
        let mut d = div()
            .id(name("view", c.id))
            .flex()
            .flex_col()
            .size_full()
            .pt(px(pad[0]))
            .pr(px(pad[1]))
            .pb(px(pad[2]))
            .pl(px(pad[3]))
            .gap(px(spacing));
        // A View scrolls only when told to (Flet's `scroll`); a scroll box
        // would give its children unbounded height and `expand` nothing to fill
        if c.get("scroll").and_then(|p| p.as_value()).is_some_and(|v| !v.is_nil()) {
            d = d.overflow_y_scroll();
        }
        d = main_axis(d, c.get_str("vertical_alignment"));
        d = cross_axis(d, c.get_str("horizontal_alignment"));
        if let Some(bg) = c.get_str("bgcolor").and_then(color) {
            d = d.bg(bg);
        }
        d.children(self.children(c.id, "controls", Parent::View, cx)).into_any_element()
    }

    /// Column (`vertical`) and Row: a flex box with Flet's spacing and
    /// alignments (`column.dart`, `row.dart`)
    fn column(&self, c: &Control, vertical: bool, cx: &mut Context<Self>) -> AnyElement {
        let spacing = c.get_f64("spacing").unwrap_or(10.0) as f32;
        let tight = c.get_bool("tight").unwrap_or(false);
        let mut d = div().id(name("box", c.id)).flex().gap(px(spacing));
        d = if vertical { d.flex_col() } else { d.flex_row() };
        // mainAxisSize.max: fill the wrapper (`layout` decides how much of
        // the parent's room the wrapper gets)
        if !tight {
            d = d.size_full();
        }
        d = main_axis(d, c.get_str("alignment"));
        let cross = if vertical { c.get_str("horizontal_alignment") } else { c.get_str("vertical_alignment").or(Some("center")) };
        d = cross_axis(d, cross);
        if c.get("scroll").and_then(|p| p.as_value()).is_some_and(|v| !v.is_nil()) {
            d = if vertical { d.overflow_y_scroll() } else { d.overflow_x_scroll() };
        }
        let axis = if vertical { Parent::Column } else { Parent::Row };
        d.children(self.children(c.id, "controls", axis, cx)).into_any_element()
    }

    /// Text (`text.dart`): value, size, weight, italic, colours, alignment
    fn text(&self, c: &Control) -> AnyElement {
        let value = c.get_str("value").unwrap_or("").to_string();
        let mut d = div().child(SharedString::from(value));
        if let Some(s) = c.get_f64("size") {
            d = d.text_size(px(s as f32));
        }
        if let Some(w) = c.get_str("weight") {
            d = d.font_weight(weight(w));
        }
        if c.get_bool("italic") == Some(true) {
            d = d.italic();
        }
        if let Some(col) = c.get_str("color").and_then(color) {
            d = d.text_color(col);
        }
        if let Some(bg) = c.get_str("bgcolor").and_then(color) {
            d = d.bg(bg);
        }
        d = match c.get_str("text_align") {
            Some("center") => d.text_center(),
            Some("right") | Some("end") => d.text_right(),
            _ => d,
        };
        if c.get_bool("no_wrap") == Some(true) {
            d = d.whitespace_nowrap();
        }
        d.into_any_element()
    }

    /// Button (`button.dart`): a filled button; `click` goes to Python
    fn button(&self, c: &Control, cx: &mut Context<Self>) -> AnyElement {
        let id = c.id;
        let disabled = c.get_bool("disabled") == Some(true);
        let bg = c.get_str("bgcolor").and_then(color).unwrap_or(gpui::rgb(PRIMARY));
        let fg = c.get_str("color").and_then(color).unwrap_or(gpui::rgb(0xFFFFFF));
        let mut d = div()
            .id(name("btn", id))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap_2()
            .px_6()
            .py_2()
            .rounded_full()
            .bg(bg)
            .text_color(fg)
            .cursor_pointer();
        if !disabled {
            d = d.hover(|s| s.opacity(0.9)).on_click(cx.listener(move |this, _ev, _window, _cx| {
                this.event(id, "click", Value::Nil);
            }));
        }
        let content: AnyElement = match c.get("content") {
            Some(fletview::Prop::Control(cid)) => self.element(*cid, Parent::Bounded, cx),
            _ => div().child(SharedString::from(c.get_str("content").unwrap_or("").to_string())).into_any_element(),
        };
        d.child(content).into_any_element()
    }

    /// TextField (`textfield.dart`): a single line with a label, edited with
    /// the engine's Editor (IME included)
    fn text_field(&self, c: &Control, cx: &mut Context<Self>) -> AnyElement {
        let id = c.id;
        let Some(f) = self.fields.get(&id) else { return div().into_any_element() };
        let focused = self.focused == Some(id);
        let label = c.get_str("label").unwrap_or("").to_string();
        let hint = c.get_str("hint_text").unwrap_or("").to_string();
        let password = c.get_bool("password") == Some(true);
        let read_only = c.get_bool("read_only") == Some(true);
        let text = f.ed.text().to_string();
        let shown: String = if password { "•".repeat(text.chars().count()) } else { text.clone() };
        let cursor = if password { shown.len().min(text[..f.ed.cursor()].chars().count() * "•".len()) } else { f.ed.cursor() };
        let (before, after) = shown.split_at(cursor.min(shown.len()));
        let line_color = if focused { gpui::rgb(PRIMARY) } else { gpui::rgb(0x79747E) };
        let mut d = div()
            .id(name("tf", id))
            .track_focus(&f.focus)
            .flex()
            .flex_col()
            .w_full()
            .pt_2()
            .px_3()
            .pb_1()
            .rounded_t_md()
            .bg(gpui::rgb(0xE6E0E9))
            .border_b(px(if focused { 2.0 } else { 1.0 }))
            .border_color(line_color)
            .cursor_text()
            .on_click(cx.listener(move |this, _ev, window, cx| {
                if let Some(f) = this.fields.get(&id) {
                    window.focus(&f.focus, cx);
                }
                if this.focused != Some(id) {
                    this.focused = Some(id);
                    this.event(id, "focus", Value::Nil);
                }
                cx.notify();
            }))
            .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _window, cx| {
                this.key(id, ev, cx);
            }));
        if !label.is_empty() {
            d = d.child(
                div()
                    .text_size(px(12.0))
                    .text_color(if focused { gpui::rgb(PRIMARY) } else { gpui::rgb(0x49454F) })
                    .child(SharedString::from(label)),
            );
        }
        let mut line = div().flex().flex_row().items_center().h(px(24.0)).text_color(gpui::rgb(0x1D1B20));
        if shown.is_empty() && !hint.is_empty() && !focused {
            line = line.child(div().text_color(gpui::rgb(0x79747E)).child(SharedString::from(hint)));
        } else {
            line = line.child(div().whitespace_nowrap().child(SharedString::from(before.to_string())));
            if focused && !read_only {
                line = line.child(div().w(px(1.5)).h(px(18.0)).bg(gpui::rgb(PRIMARY)));
            }
            line = line.child(div().whitespace_nowrap().child(SharedString::from(after.to_string())));
        }
        d.child(line).into_any_element()
    }

    /// The keys a field answers besides typed text (which arrives through
    /// the input handler): moving, deleting, Enter
    fn key(&mut self, id: ControlId, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let read_only = self.tree.get(id).is_some_and(|c| c.get_bool("read_only") == Some(true));
        let Some(f) = self.fields.get_mut(&id) else { return };
        let k = &ev.keystroke;
        let ctrl = k.modifiers.control || k.modifiers.platform;
        match k.key.as_str() {
            "left" => f.ed.move_char(false, k.modifiers.shift),
            "right" => f.ed.move_char(true, k.modifiers.shift),
            "home" => f.ed.move_to(0, k.modifiers.shift),
            "end" => {
                let n = f.ed.text().len();
                f.ed.move_to(n, k.modifiers.shift);
            }
            "backspace" if !read_only => f.ed.backspace(),
            "delete" if !read_only => f.ed.delete(),
            "a" if ctrl => f.ed.select_all(),
            "z" if ctrl && !read_only => {
                if k.modifiers.shift {
                    f.ed.redo();
                } else {
                    f.ed.undo();
                }
            }
            "enter" => {
                let value = f.ed.text().to_string();
                self.flush(cx);
                self.event(id, "submit", Value::str(&value));
                return;
            }
            _ => return,
        }
        self.flush(cx);
        cx.notify();
    }

    fn focused_handle(&self) -> Option<FocusHandle> {
        self.focused.and_then(|id| self.fields.get(&id)).map(|f| f.focus.clone())
    }
}

fn name(kind: &str, id: ControlId) -> SharedString {
    format!("flet-{kind}-{id}").into()
}

/// Flet's `MainAxisAlignment` names
fn main_axis(d: gpui::Stateful<gpui::Div>, a: Option<&str>) -> gpui::Stateful<gpui::Div> {
    match a {
        Some("center") => d.justify_center(),
        Some("end") => d.justify_end(),
        Some("space_between") | Some("spaceBetween") => d.justify_between(),
        Some("space_around") | Some("spaceAround") => d.justify_around(),
        Some("space_evenly") | Some("spaceEvenly") => d.justify_evenly(),
        _ => d.justify_start(),
    }
}

/// Flet's `CrossAxisAlignment` names
fn cross_axis(d: gpui::Stateful<gpui::Div>, a: Option<&str>) -> gpui::Stateful<gpui::Div> {
    match a {
        Some("center") => d.items_center(),
        Some("end") => d.items_end(),
        Some("stretch") => d.items_stretch(),
        Some("baseline") => d.items_baseline(),
        _ => d.items_start(),
    }
}

/// `{"left", "top", "right", "bottom"}` or one number → top, right, bottom, left
fn edge_insets(v: &Value) -> [f32; 4] {
    if let Some(n) = v.as_f64() {
        return [n as f32; 4];
    }
    let g = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
    [g("top"), g("right"), g("bottom"), g("left")]
}

fn weight(w: &str) -> FontWeight {
    match w.to_ascii_lowercase().as_str() {
        "bold" | "w700" => FontWeight::BOLD,
        "w100" => FontWeight::THIN,
        "w200" => FontWeight::EXTRA_LIGHT,
        "w300" => FontWeight::LIGHT,
        "w500" => FontWeight::MEDIUM,
        "w600" => FontWeight::SEMIBOLD,
        "w800" => FontWeight::EXTRA_BOLD,
        "w900" => FontWeight::BLACK,
        _ => FontWeight::NORMAL,
    }
}

/// A Flet colour: `#RRGGBB`, `#AARRGGBB`, or one of Material's names
fn color(s: &str) -> Option<gpui::Rgba> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return match hex.len() {
            6 => u32::from_str_radix(hex, 16).ok().map(gpui::rgb),
            8 => u32::from_str_radix(hex, 16).ok().map(|argb| gpui::rgba((argb << 8) | (argb >> 24))),
            _ => None,
        };
    }
    let (name, alpha) = match s.split_once(',') {
        Some((n, a)) => (n.trim(), a.trim().parse::<f32>().ok()),
        None => (s, None),
    };
    let rgb = match name.to_ascii_lowercase().as_str() {
        "red" => 0xF44336,
        "pink" => 0xE91E63,
        "purple" => 0x9C27B0,
        "blue" => 0x2196F3,
        "lightblue" | "light_blue" => 0x03A9F4,
        "cyan" => 0x00BCD4,
        "teal" => 0x009688,
        "green" => 0x4CAF50,
        "lightgreen" | "light_green" => 0x8BC34A,
        "lime" => 0xCDDC39,
        "yellow" => 0xFFEB3B,
        "amber" => 0xFFC107,
        "orange" => 0xFF9800,
        "deeporange" | "deep_orange" => 0xFF5722,
        "brown" => 0x795548,
        "grey" | "gray" => 0x9E9E9E,
        "bluegrey" | "blue_grey" => 0x607D8B,
        "black" => 0x000000,
        "white" => 0xFFFFFF,
        "primary" => PRIMARY,
        "onprimary" | "on_primary" => 0xFFFFFF,
        "secondary" => 0x625B71,
        "surface" => 0xFEF7FF,
        "onsurface" | "on_surface" => 0x1D1B20,
        "error" => 0xB3261E,
        "transparent" => return Some(gpui::rgba(0x00000000)),
        _ => return None,
    };
    Some(match alpha {
        Some(a) => gpui::rgba((rgb << 8) | ((a.clamp(0.0, 1.0) * 255.0) as u32)),
        None => gpui::rgb(rgb),
    })
}

impl HasEditor for FletView {
    fn editor(&mut self) -> &mut Editor {
        match self.focused.and_then(|id| self.fields.get_mut(&id)) {
            Some(f) => &mut f.ed,
            None => &mut self.scratch,
        }
    }

    fn editor_ref(&self) -> &Editor {
        match self.focused.and_then(|id| self.fields.get(&id)) {
            Some(f) => &f.ed,
            None => &self.scratch,
        }
    }
}

impl EntityInputHandler for FletView {
    fn text_for_range(&mut self, r: Range<usize>, actual: &mut Option<Range<usize>>, _w: &mut Window, _cx: &mut Context<Self>) -> Option<String> {
        handler::text_for_range(self, r, actual)
    }

    fn selected_text_range(&mut self, _i: bool, _w: &mut Window, _cx: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: handler::selected_range_utf16(self), reversed: false })
    }

    fn marked_text_range(&self, _w: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
        handler::marked_range_utf16(self)
    }

    fn unmark_text(&mut self, _w: &mut Window, _cx: &mut Context<Self>) {
        handler::unmark(self)
    }

    fn replace_text_in_range(&mut self, r: Option<Range<usize>>, text: &str, _w: &mut Window, cx: &mut Context<Self>) {
        let read_only = self.focused.and_then(|id| self.tree.get(id)).is_some_and(|c| c.get_bool("read_only") == Some(true));
        if self.focused.is_none() || read_only {
            return;
        }
        handler::replace(self, r, text);
        self.flush(cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(&mut self, r: Option<Range<usize>>, text: &str, sel: Option<Range<usize>>, _w: &mut Window, cx: &mut Context<Self>) {
        if self.focused.is_none() {
            return;
        }
        handler::replace_and_mark(self, r, text, sel);
        cx.notify();
    }

    fn bounds_for_range(&mut self, _r: Range<usize>, bounds: Bounds<gpui::Pixels>, _w: &mut Window, _cx: &mut Context<Self>) -> Option<Bounds<gpui::Pixels>> {
        Some(bounds)
    }

    fn character_index_for_point(&mut self, _p: gpui::Point<gpui::Pixels>, _w: &mut Window, _cx: &mut Context<Self>) -> Option<usize> {
        None
    }
}

/// An element over the whole window that hands the focused field's keyboard
/// (and the IME) to the view, the way calc's grid does
struct Ime {
    view: Entity<FletView>,
}

impl IntoElement for Ime {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for Ime {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<gpui::ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(&mut self, _: Option<&gpui::GlobalElementId>, _: Option<&gpui::InspectorElementId>, window: &mut Window, cx: &mut App) -> (gpui::LayoutId, ()) {
        let mut s = gpui::Style { position: gpui::Position::Absolute, ..Default::default() };
        s.inset.top = px(0.0).into();
        s.inset.left = px(0.0).into();
        s.size.width = gpui::relative(1.0).into();
        s.size.height = gpui::relative(1.0).into();
        (window.request_layout(s, [], cx), ())
    }
    fn prepaint(&mut self, _: Option<&gpui::GlobalElementId>, _: Option<&gpui::InspectorElementId>, _: Bounds<gpui::Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}
    fn paint(&mut self, _: Option<&gpui::GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<gpui::Pixels>, _: &mut (), _: &mut (), window: &mut Window, cx: &mut App) {
        if let Some(focus) = self.view.read(cx).focused_handle() {
            window.handle_input(&focus, ElementInputHandler::new(bounds, self.view.clone()), cx);
        }
    }
}

impl Render for FletView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&self.title);
        self.report_size(window);
        let root = self.tree.root;
        let page = root.and_then(|r| self.tree.get(r));
        let bg = page.and_then(|p| p.get_str("bgcolor")).and_then(color).unwrap_or(gpui::rgb(0xFFFFFF));
        let mut d = div().relative().size_full().bg(bg).text_color(gpui::rgb(0x1D1B20)).text_size(px(TEXT_PX));
        if let Some(r) = root {
            // The first view is the page being shown; routes come later
            if let Some(v) = self.tree.children(r, "views").into_iter().next() {
                d = d.child(self.element(v, Parent::Bounded, cx));
            } else {
                d = d.child(div().p_4().text_color(gpui::rgb(0x66707A)).child(self.status.clone()));
            }
        } else {
            d = d.child(div().p_4().text_color(gpui::rgb(0x66707A)).child(self.status.clone()));
        }
        d.child(Ime { view: cx.entity() })
    }
}

/// Reads from the socket on its own thread and queues what arrives
fn listen(url: String, queue: Queue) {
    std::thread::spawn(move || {
        let push = |e: Event| {
            if let Ok(mut q) = queue.lock() {
                q.push(e);
            }
        };
        let result = (|| -> std::io::Result<()> {
            let mut c = Client::connect(&url)?;
            let r = c.register(page_props(WIDTH as f64, HEIGHT as f64, platform()))?;
            push(Event::Registered(r, c.sender()));
            loop {
                push(Event::In(c.next()?));
            }
        })();
        push(Event::Ended(match result {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => "Python が終わりました".to_string(),
            Err(e) => format!("つながりが切れました: {e}"),
            Ok(()) => "終わりました".to_string(),
        }));
    });
}

/// Starts as the client of the page and runs until the program ends
pub fn run(args: Args) {
    if let Err(e) = std::fs::write(&args.pid_file, std::process::id().to_string()) {
        eprintln!("pid ファイルに書けません({}): {e}", args.pid_file.display());
    }
    let queue: Queue = Arc::new(Mutex::new(Vec::new()));
    listen(args.url.clone(), queue.clone());
    application().with_assets(ui::Icons).run(move |cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![std::borrow::Cow::Borrowed(ops::font_data())])
            .expect("フォント登録");
        let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
        let queue = queue.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions { title: Some("Flet".into()), ..Default::default() }),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|_| FletView::new());
                // Every 30 ms, what the socket thread queued is applied on the
                // main thread (the same rule as calc's inlet)
                let v = view.clone();
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor().timer(std::time::Duration::from_millis(30)).await;
                        let events: Vec<Event> = match queue.lock() {
                            Ok(mut q) => std::mem::take(&mut *q),
                            Err(_) => Vec::new(),
                        };
                        if events.is_empty() {
                            continue;
                        }
                        let _ = v.update(cx, |this, cx| {
                            for e in events {
                                this.take(e, cx);
                            }
                        });
                    }
                })
                .detach();
                window.on_window_should_close(cx, move |_, cx| {
                    cx.quit();
                    true
                });
                view
            },
        )
        .expect("窓が開けません");
        cx.activate(true);
    });
}
