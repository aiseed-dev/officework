//! View settings of an ods (`settings.xml`): frozen panes.
//!
//! LibreOffice keeps what a window shows in `ooo:view-settings`: a list of
//! views, each with a `Tables` map that holds one entry per sheet. A frozen
//! pane is `HorizontalSplitMode` / `VerticalSplitMode` 2 with the first
//! column / row that scrolls as `HorizontalSplitPosition` /
//! `VerticalSplitPosition` (sc/inc/ViewSettingsSequenceDefines.hxx, and
//! sc/source/filter/oox/viewsettings.cxx for how an xlsx pane becomes these).
//! Mode 1 is a split window, whose positions are lengths; the model has no
//! split window, so it is not read.

use std::collections::HashMap;
use std::fmt::Write as _;

use book::{Book, FreezePane};
use quick_xml::events::Event;
use quick_xml::Reader;

use super::read::attr;

/// Frozen panes by sheet name, from the first view
pub(super) fn parse(xml: &str) -> HashMap<String, FreezePane> {
    let mut out = HashMap::new();
    let mut r = Reader::from_str(xml);
    // The map we are in: depth of `Tables`, the sheet entry, the item name
    let mut in_tables = false;
    let mut views = 0;
    let mut sheet: Option<String> = None;
    let mut item: Option<String> = None;
    let mut vals: HashMap<String, i64> = HashMap::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"config:config-item-map-named" if attr(&e, "config:name").as_deref() == Some("Tables") => in_tables = views == 1,
                b"config:config-item-map-entry" => {
                    if in_tables {
                        sheet = attr(&e, "config:name");
                        vals.clear();
                    } else {
                        views += 1;
                    }
                }
                b"config:config-item" if sheet.is_some() => item = attr(&e, "config:name"),
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if let (Some(_), Some(k)) = (&sheet, &item) {
                    if let Ok(v) = t.unescape().unwrap_or_default().trim().parse::<i64>() {
                        vals.insert(k.clone(), v);
                    }
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"config:config-item" => item = None,
                b"config:config-item-map-entry" => {
                    if let Some(name) = sheet.take() {
                        let frozen = |mode: &str, pos: &str| {
                            if vals.get(mode) == Some(&2) {
                                vals.get(pos).copied().unwrap_or(0).max(0) as u32
                            } else {
                                0
                            }
                        };
                        let cols = frozen("HorizontalSplitMode", "HorizontalSplitPosition");
                        let rows = frozen("VerticalSplitMode", "VerticalSplitPosition");
                        if cols > 0 || rows > 0 {
                            out.insert(name, FreezePane { frozen_rows: rows, frozen_columns: cols });
                        }
                    }
                }
                b"config:config-item-map-named" => in_tables = false,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// settings.xml for the frozen panes of the book, None when no sheet has one
pub(super) fn write(book: &Book) -> Option<String> {
    let frozen: Vec<_> = book.sheets.iter().filter_map(|s| s.freeze.as_ref().map(|f| (s.name.as_str(), f))).collect();
    if frozen.iter().all(|(_, f)| f.frozen_rows == 0 && f.frozen_columns == 0) {
        return None;
    }
    let item = |s: &mut String, name: &str, ty: &str, v: &str| {
        let _ = write!(s, r#"<config:config-item config:name="{name}" config:type="{ty}">{v}</config:config-item>"#);
    };
    let mut s = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-settings xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:config="urn:oasis:names:tc:opendocument:xmlns:config:1.0" xmlns:ooo="http://openoffice.org/2004/office" office:version="1.3"><office:settings><config:config-item-set config:name="ooo:view-settings"><config:config-item-map-indexed config:name="Views"><config:config-item-map-entry>"#,
    );
    item(&mut s, "ViewId", "string", "view1");
    s.push_str(r#"<config:config-item-map-named config:name="Tables">"#);
    for (name, f) in &frozen {
        let (rows, cols) = (f.frozen_rows, f.frozen_columns);
        let _ = write!(s, r#"<config:config-item-map-entry config:name="{}">"#, super::write::esc(name));
        item(&mut s, "CursorPositionX", "int", &cols.to_string());
        item(&mut s, "CursorPositionY", "int", &rows.to_string());
        item(&mut s, "HorizontalSplitMode", "short", if cols > 0 { "2" } else { "0" });
        item(&mut s, "VerticalSplitMode", "short", if rows > 0 { "2" } else { "0" });
        item(&mut s, "HorizontalSplitPosition", "int", &cols.to_string());
        item(&mut s, "VerticalSplitPosition", "int", &rows.to_string());
        // The pane with the cursor: bottom left with rows only, bottom
        // right otherwise (the choice viewsettings.cxx makes for an xlsx)
        item(&mut s, "ActiveSplitRange", "short", if cols > 0 { "3" } else { "2" });
        item(&mut s, "PositionLeft", "int", "0");
        item(&mut s, "PositionRight", "int", &cols.to_string());
        item(&mut s, "PositionTop", "int", "0");
        item(&mut s, "PositionBottom", "int", &rows.to_string());
        s.push_str("</config:config-item-map-entry>");
    }
    s.push_str("</config:config-item-map-named>");
    if let Some(first) = book.sheets.iter().find(|s| !s.hidden) {
        item(&mut s, "ActiveTable", "string", &super::write::esc(&first.name));
    }
    s.push_str("</config:config-item-map-entry></config:config-item-map-indexed></config:config-item-set></office:settings></office:document-settings>");
    Some(s)
}
