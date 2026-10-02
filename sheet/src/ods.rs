//! ods (OpenDocument spreadsheet) reading.
//!
//! Started 2026-10-02 (SEKKEI "決め: ODF を先にする"). The way LibreOffice
//! reads and writes ods is the reference; its API and tests are copied, its
//! code is not. Unread parts are counted in the same [`Report`] the xlsx
//! reader uses.

mod formula;
mod numfmt;
mod read;
mod styles;

pub use crate::xlsx::Report;
pub use formula::to_a1;
pub use read::read;

#[cfg(test)]
mod tests;
