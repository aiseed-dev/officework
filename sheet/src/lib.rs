//! sheet: exchanging spreadsheet files. Independent of the UI.
//!
//! This crate reads and writes xlsx and reads ods (2026-08-26, SEKKEI
//! "エンジンは3つに分ける"; ods added 2026-10-02, "決め: ODF を先にする").
//! The cell model, formula calculation and `.adoc` live in `kumihan`, and
//! this crate does not re-export them, so it never looks as if a file format
//! owned the model.
//!
//! - [`xlsx`] reading and writing xlsx (`styles.xml` and `theme1.xml` included)
//! - [`ods`] reading ods
//!
//! Macros are not implemented, by design: a document never carries code with
//! the right to run, so "opening is running" does not exist as a way in
//! (the same idea as aiseed-migration-kit DESIGN.md §5).

pub mod ods;
pub mod xlsx;
