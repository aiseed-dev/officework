//! ods (OpenDocument spreadsheet) reading and writing.
//!
//! Started 2026-10-02 (SEKKEI "決め: ODF を先にする"). The way LibreOffice
//! reads and writes ods is the reference; its API and tests are copied, its
//! code is not. Unread parts are counted in the same [`Report`] the xlsx
//! reader uses.

mod cond;
mod drawing;
mod formula;
mod numfmt;
mod numfmt_write;
mod page;
mod read;
mod settings;
mod styles;
mod valid;
mod write;

pub use crate::xlsx::Report;
pub use formula::{to_a1, to_of};
pub use read::read;
pub use write::{write, WriteReport};

#[cfg(test)]
mod tests;
