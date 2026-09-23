//! **Custom functions written in Rust** (2026-09-24).
//!
//! Excel lets a workbook define cell functions in VBA. officework does not
//! put code in files; custom functions are written in Rust (here) or in
//! Python (`~/.config/officework/funcs/*.py`, see calc's `py.rs`). A cell
//! calls either kind like any function, `=ZONED("2026-10-01 10:00",
//! "Asia/Tokyo")`, and the formula is what is saved, in xlsx and in adoc.
//!
//! A custom function gets the arguments already calculated (ranges are
//! flattened, left to right and top to bottom) and returns one value.
//! Built-in functions come first: a custom function with the name of a
//! built-in one is never called.
//!
//! The functions that come with officework are in [`BUILT_IN`]. Others can be
//! added while the program runs with [`register`].

use crate::Value;
use std::sync::RwLock;

mod zoned;

/// What a custom function may need to know about the workbook.
pub struct Ctx {
    /// The workbook counts dates from 1904-01-01 (`workbookPr@date1904`)
    pub date1904: bool,
}

/// A custom function: the calculated arguments in, one value out.
pub type CustomFn = fn(&[Value], &Ctx) -> Value;

/// The custom functions that come with officework.
pub const BUILT_IN: &[(&str, CustomFn)] = &[
    // Moments with a time zone (docs/sekkei/time-zone.ja.adoc)
    ("ZONED", zoned::zoned),
    ("TO_ZONE", zoned::to_zone),
];

static REGISTERED: RwLock<Vec<(String, CustomFn)>> = RwLock::new(Vec::new());

/// Adds a custom function. The name is matched without regard to case, the
/// way formulas are read. A second function with the same name replaces
/// the first.
pub fn register(name: &str, f: CustomFn) {
    let up = name.to_ascii_uppercase();
    if let Ok(mut g) = REGISTERED.write() {
        g.retain(|(n, _)| *n != up);
        g.push((up, f));
    }
}

/// The names of all custom functions, the ones that come with officework
/// first.
pub fn names() -> Vec<String> {
    let mut out: Vec<String> = BUILT_IN.iter().map(|(n, _)| n.to_string()).collect();
    if let Ok(g) = REGISTERED.read() {
        out.extend(g.iter().map(|(n, _)| n.clone()).filter(|n| !out.contains(n)).collect::<Vec<_>>());
    }
    out
}

/// The custom function called `name` (already upper case), if there is one.
pub(super) fn find(name: &str) -> Option<CustomFn> {
    if let Ok(g) = REGISTERED.read() {
        if let Some((_, f)) = g.iter().find(|(n, _)| n == name) {
            return Some(*f);
        }
    }
    BUILT_IN.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
}
