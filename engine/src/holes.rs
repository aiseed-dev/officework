//! **`.sheet.adoc` の往復で落ちる物を数える。**
//!
//! ブックの正本を `.adoc` にする(SEKKEI「エンジンは3つに分ける」)なら、
//! 利用者が画面でやったことは往復で戻らないといけません。戻らない物は
//! **黙って落とさず、名前で数えて言う**のが家の作法です。
//!
//! [`write_report`](super::adoc::write_report) は5種類しか見ていませんでした。
//! `Sheet` の持ち物は 55 あります。**見ていない 50 は、落ちても何も言いません。**
//! ここはその穴を機械で数える所です。
//!
//! # 使い方
//!
//! [`round_trip_holes`] が「埋めたのに戻らなかった持ち物の名前」を返します。
//! 穴が埋まるたびに返る数が減ります。
//!
//! # 名前が漏れない仕掛け
//!
//! `Sheet` に持ち物を足したとき、ここの表に足し忘れると**穴が見えなく
//! なります**。それでは道具の意味がないので、[`super::tests`] の
//! `every_sheet_field_is_watched` が `types.rs` を読んで、`pub` の持ち物が
//! 全部この表に載っていることを確かめます。

use crate::book_adoc as adoc;
use book::{
    Book, Cell, CellFormat, CondKind, CondLook, CondOp, CondRule, DefinedName, FreezePane, Pos,
    Scenario, Sheet, SheetImage, SheetShape, TableDef, Validation, Value,
};

/// **見張っている `Sheet` の持ち物の名前。** `types.rs` の `pub` と揃えます。
///
/// 3列目は「往復で戻るか」ではなく「**この道具が埋めるか**」です。
/// 埋めない物には理由を書いてあります。
pub const WATCHED: &[(&str, Watch)] = &[
    ("name", Watch::Body),
    ("cells", Watch::Body),
    ("merges", Watch::Body),
    ("col_width", Watch::Look),
    ("default_col_width", Watch::Look),
    ("default_row_height", Watch::Look),
    ("row_collapsed", Watch::Look),
    ("col_collapsed", Watch::Look),
    ("row_height", Watch::Look),
    ("row_outline", Watch::Look),
    ("col_outline", Watch::Look),
    ("row_hidden", Watch::Body),
    ("col_hidden", Watch::Body),
    ("tables", Watch::Body),
    ("style_of", Watch::Skip("xlsx の <c s=\"…\"> の控え。原本の据え置きに使う物で、\n    // adoc は書式を名前で持つ")),
    ("rtl", Watch::Look),
    ("freeze", Watch::Look),
    ("show_gridlines", Watch::Look),
    ("show_formulas", Watch::Look),
    ("zoom_scale", Watch::Look),
    ("hidden", Watch::Look),
    ("tab_color", Watch::Look),
    ("protected", Watch::Look),
    ("protect_allow", Watch::Look),
    // 範囲ごとの保護(2026-08-30)
    ("protect_ranges", Watch::Look),
    ("names", Watch::Body),
    ("links", Watch::Body),
    ("comments", Watch::Body),
    ("cond", Watch::Body),
    ("validations", Watch::Body),
    ("scenarios", Watch::Body),
    ("landscape", Watch::Look),
    ("paper_size", Watch::Look),
    ("margins_mm", Watch::Look),
    ("print_areas", Watch::Body),
    ("print_scale", Watch::Look),
    ("fit_to_w", Watch::Look),
    ("fit_to_h", Watch::Look),
    ("row_breaks", Watch::Look),
    ("col_breaks", Watch::Look),
    ("print_gridlines", Watch::Look),
    ("print_headings", Watch::Look),
    ("print_title_rows", Watch::Look),
    ("print_title_cols", Watch::Look),
    ("header", Watch::Look),
    ("footer", Watch::Look),
    ("header_even", Watch::Look),
    ("footer_even", Watch::Look),
    ("header_first", Watch::Look),
    ("footer_first", Watch::Look),
    ("hf_diff_odd_even", Watch::Look),
    ("hf_diff_first", Watch::Look),
    ("shapes", Watch::Aside),
    ("images", Watch::Aside),
    ("phonetics", Watch::Body),
    // ここから下は**埋めません**。理由つき。
    ("shapes_new", Watch::Skip("shapes と同じ物。保存で shapes に合流する")),
    ("images_new", Watch::Skip("images と同じ物。保存で images に合流する")),
    ("py_stamp", Watch::Skip("Python を走らせた印。開き直せば計算し直す")),
    ("spills", Watch::Skip("計算の跡。式から作り直る")),
    ("dim", Watch::Skip("読んだときの大きさ。中身から作り直る")),
    ("seen", Watch::Skip("読んだときの大きさ。中身から作り直る")),
    ("cse", Watch::Skip("昔ながらの配列数式の跡。式から作り直る")),
];

/// 見張り方 — この持ち物を**どこへ持つか**。
///
/// 決めは「**意味は `.sheet.adoc`・見た目は `.tmpl.adoc`**」です
/// (docs/sekkei/calc.ja.adoc「calc の adoc 形式」2026-08-18 発注者)。
/// 分け方に迷ったら「別のブックにこの設定を使い回せるか」で決めます —
/// 使い回せるなら見た目、そのブック固有なら意味です。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Watch {
    /// `.sheet.adoc` が持つ(意味)
    Body,
    /// `.tmpl.adoc` が持つ(見た目)
    Look,
    /// 隣のファイルに出す(絵の実体)
    Aside,
    /// 埋めない。計算の跡や重複など、往復で戻らなくてよい物
    Skip(&'static str),
}

impl Watch {
    /// 埋めて確かめる物か
    pub fn is_filled(self) -> bool {
        !matches!(self, Watch::Skip(_))
    }

    /// 帳簿に出すときの行き先の名前
    pub fn where_to(self) -> &'static str {
        match self {
            Watch::Body => ".sheet.adoc(意味)",
            Watch::Look => ".tmpl.adoc(見た目)",
            Watch::Aside => "隣のファイル(絵の実体)",
            Watch::Skip(_) => "持たない",
        }
    }
}

/// **`Book` の持ち物。** シートの外側にある物です。
pub const WATCHED_BOOK: &[(&str, Watch)] = &[
    ("sheets", Watch::Body),
    ("props", Watch::Body),
    ("theme", Watch::Look),
    ("names_raw", Watch::Skip("読めなかった definedName の xlsx の原文。\n    // 理解しないまま持ち越す控えで、adoc に居場所は無い")),
    ("named_styles", Watch::Look),
    ("named_styles_new", Watch::Look),
    ("scripts", Watch::Skip("古いブックに載っていた Python。**保存では書き戻さない**\n    // 決め(2026-08-09 発注者)なので、往復しないのが正しい")),
    ("pivots", Watch::Body),
    ("calc_manual", Watch::Body),
    ("calc_iter", Watch::Body),
    ("r1c1", Watch::Look),
    ("read_only_rec", Watch::Body),
    // ブックの構造の保護(2026-08-30)
    ("lock_structure", Watch::Body),
    ("date1904", Watch::Body),
    ("changes", Watch::Body),
    ("path", Watch::Skip("開いた場所。ファイルの中身ではない")),
];

/// **持ち物を全部埋めたブック。** 往復で何が落ちるかを測る材料です。
pub fn filled_book() -> Book {
    let mut b = Book::new();
    b.sheets.clear();
    b.sheets.push(filled_sheet("売上台帳"));

    b.props.title = "四月の売上".into();
    b.props.creators = vec!["総務課".into()];
    b.theme = book::theme::OFFICE.iter().map(|s| s.to_string()).collect();
    b.names_raw = vec!["<definedName name=\"税率\">0.1</definedName>".into()];
    b.named_styles = vec![("見出し".into(), Some(3), CellFormat { bold: true, ..Default::default() })];
    b.named_styles_new = vec![("合計".into(), CellFormat { bold: true, ..Default::default() })];
    b.scripts = vec![("集計.py".into(), "print(1)".into())];
    b.calc_manual = true;
    b.calc_iter = Some((50, 0.001));
    b.r1c1 = true;
    b.read_only_rec = true;
    b.lock_structure = true;
    b.date1904 = true;
    b
}

/// 持ち物を全部埋めた1枚。
pub fn filled_sheet(name: &str) -> Sheet {
    let at = |a: &str| Pos::parse(a).expect("番地");
    let mut s = Sheet::new(name);

    s.set(at("A1"), Cell { value: Value::Text("品名".into()), ..Default::default() });
    s.set(at("B1"), Cell { value: Value::Text("金額".into()), ..Default::default() });
    s.set(at("A2"), Cell { value: Value::Text("ボールペン".into()), ..Default::default() });
    s.set(at("B2"), Cell { value: Value::Number(1200.0), ..Default::default() });
    s.set(at("B3"), Cell { formula: Some("SUM(B2:B2)".into()), ..Default::default() });
    // **書式つきのセル。** いまいちばん大きい落とし物
    s.set(at("A1"), Cell {
        value: Value::Text("品名".into()),
        fmt: CellFormat { bold: true, ..Default::default() },
        ..Default::default()
    });

    s.merges = vec![(at("D1"), at("E1"))];
    s.col_width.insert(0, 18.5);
    s.default_col_width = Some(8.43);
    s.default_row_height = Some(13.5);
    s.row_collapsed.insert(4);
    s.col_collapsed.insert(4);
    s.row_height.insert(0, 24.0);
    s.row_outline.insert(3, 1);
    s.col_outline.insert(3, 1);
    s.row_hidden.insert(5);
    s.col_hidden.insert(5);
    s.tables = vec![TableDef {
        name: "売上".into(), a: at("A1"), b: at("B3"), header: true, ..Default::default()
    }];
    s.style_of.insert(at("A2"), 3);
    s.rtl = true;
    s.freeze = Some(FreezePane { frozen_rows: 1, frozen_columns: 0 });
    s.show_gridlines = Some(false);
    s.show_formulas = Some(true);
    s.zoom_scale = Some(120);
    s.hidden = true;
    s.tab_color = Some("4472C4".into());
    s.protected = true;
    s.protect_allow.sort = true;
    s.protect_ranges = vec![("入力欄".into(), "B2:D10".into())];
    s.names = vec![DefinedName { name: "税率".into(), range: "$A$1".into(), scoped: true }];
    s.links.insert(at("A2"), "https://example.jp".into());
    s.cond = vec![CondRule {
        range: (at("B2"), at("B3")),
        kind: CondKind::Cmp(CondOp::Gt, 1000.0),
        look: CondLook { bold: Some(true), ..Default::default() },
    }];
    s.validations = vec![Validation {
        range: (at("B2"), at("B3")),
        kind: "whole".into(),
        op: "between".into(),
        formula: "0".into(),
        formula2: "10000".into(),
        input_msg: None,
        error_msg: None,
        allow_blank: true,
        hide_arrow: false,
    }];
    s.scenarios = vec![Scenario {
        name: "強気".into(), cells: vec![(at("B2"), "2000".into())], comment: String::new(),
    }];
    s.landscape = true;
    s.paper_size = Some(9);
    s.margins_mm = Some((20.0, 20.0, 25.0, 25.0));
    s.print_areas = vec![(at("A1"), at("B3"))];
    s.print_scale = Some(90);
    s.fit_to_w = Some(1);
    s.fit_to_h = Some(0);
    s.row_breaks = vec![10];
    s.col_breaks = vec![4];
    s.print_gridlines = true;
    s.print_headings = true;
    s.print_title_rows = Some((0, 0));
    s.print_title_cols = Some((0, 0));
    s.header = Some("&C四月の売上".into());
    s.footer = Some("&C&P / &N".into());
    s.header_even = Some("&L偶数".into());
    s.footer_even = Some("&L偶数".into());
    s.header_first = Some("&L初頁".into());
    s.footer_first = Some("&L初頁".into());
    s.hf_diff_odd_even = true;
    s.hf_diff_first = true;
    s.shapes = vec![SheetShape { kind: "rect".into(), ..Default::default() }];
    s.images = vec![SheetImage {
        at: at("D5"), dx_px: 0.0, dy_px: 0.0, width_px: 96.0, height_px: 96.0,
        data: vec![0x89, b'P', b'N', b'G'],
    }];
    s.phonetics.insert(at("A2"), "ボールペン".into());
    s
}

/// **往復で戻らなかった持ち物を、行き先ごとにまとめて日本語で出す。**
///
/// 作業の順を決めるための一覧です。`cargo test -p kumihan -- --nocapture
/// holes_count` で読めます。
pub fn report() -> String {
    let holes = round_trip_holes();
    let mut out = String::new();
    for (dest, title) in [
        (Watch::Body, ".sheet.adoc が持つべき物(意味)"),
        (Watch::Look, ".tmpl.adoc が持つべき物(見た目)"),
        (Watch::Aside, "隣のファイルに出す物(絵の実体)"),
    ] {
        let mine: Vec<&str> = holes
            .iter()
            .copied()
            .filter(|n| where_of(n) == dest)
            .collect();
        if mine.is_empty() {
            continue;
        }
        out.push_str(&format!("{title} — {} 件\n", mine.len()));
        for n in mine {
            out.push_str(&format!("  {n}\n"));
        }
    }
    if out.is_empty() {
        out.push_str("落ちる物はありません。\n");
    }
    out
}

/// その持ち物の行き先。表に無ければ「持たない」。
pub fn where_of(name: &str) -> Watch {
    WATCHED
        .iter()
        .chain(WATCHED_BOOK.iter())
        .find(|(n, _)| *n == name)
        .map(|(_, w)| *w)
        .unwrap_or(Watch::Skip("表に無い"))
}

/// **往復で戻らなかった持ち物の名前。** テンプレートも通します。
///
/// 返る並びは `WATCHED` の順です。`Watch::Skip` の物は見ません。
pub fn round_trip_holes() -> Vec<&'static str> {
    let before = filled_book();
    let tmpl = crate::booktmpl::from_book(&before);

    let src = adoc::write(&before);
    let (mut after, _) = adoc::parse(&src).expect("自分で書いた adoc が読めない");
    let tsrc = crate::booktmpl::write(&tmpl);
    let back = crate::booktmpl::parse(&tsrc).expect("自分で書いたテンプレートが読めない");
    crate::booktmpl::apply(&back, &mut after);

    let mut out = Vec::new();
    for (name, w) in WATCHED_BOOK {
        if !w.is_filled() || name == &"sheets" {
            continue;
        }
        if !same_book_field(name, &before, &after) {
            out.push(*name);
        }
    }
    let (a, b) = match (before.sheets.first(), after.sheets.first()) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            out.push("sheets");
            return out;
        }
    };
    for (name, w) in WATCHED {
        if !w.is_filled() {
            continue;
        }
        if !same_sheet_field(name, a, b) {
            out.push(*name);
        }
    }
    out
}

fn same_book_field(name: &str, a: &Book, b: &Book) -> bool {
    match name {
        // BookProps は PartialEq を持たないので、画面に出る欄だけ比べます
        "props" => a.props.title == b.props.title && a.props.creators == b.props.creators,
        "theme" => a.theme == b.theme,
        // **2つは xlsx の索引を持つかどうかで分かれているだけ**なので、
        // 名前と書式の組として合わせて比べます
        "named_styles" | "named_styles_new" => {
            let set = |b: &Book| {
                let mut v: Vec<(String, CellFormat)> = b
                    .named_styles
                    .iter()
                    .map(|(n, _, f)| (n.clone(), f.clone()))
                    .chain(b.named_styles_new.iter().cloned())
                    .collect();
                v.sort_by(|x, y| x.0.cmp(&y.0));
                v
            };
            set(a) == set(b)
        }
        "pivots" => a.pivots.len() == b.pivots.len(),
        "calc_manual" => a.calc_manual == b.calc_manual,
        "calc_iter" => a.calc_iter == b.calc_iter,
        "r1c1" => a.r1c1 == b.r1c1,
        "read_only_rec" => a.read_only_rec == b.read_only_rec,
        "lock_structure" => a.lock_structure == b.lock_structure,
        "date1904" => a.date1904 == b.date1904,
        "changes" => a.changes.len() == b.changes.len(),
        _ => panic!("見張りの表にあるのに比べ方が無い: {name}"),
    }
}

fn same_sheet_field(name: &str, a: &Sheet, b: &Sheet) -> bool {
    match name {
        "name" => a.name == b.name,
        // **式と値の両方**。式が正本なので、式が戻れば値は計算で作れます
        "cells" => {
            a.cells.len() == b.cells.len()
                && a.cells.iter().all(|(p, c)| {
                    b.cells.get(p).is_some_and(|d| c.formula == d.formula && c.fmt == d.fmt)
                })
        }
        "merges" => a.merges == b.merges,
        "col_width" => a.col_width == b.col_width,
        "default_col_width" => a.default_col_width == b.default_col_width,
        "default_row_height" => a.default_row_height == b.default_row_height,
        "row_collapsed" => a.row_collapsed == b.row_collapsed,
        "col_collapsed" => a.col_collapsed == b.col_collapsed,
        "row_height" => a.row_height == b.row_height,
        "row_outline" => a.row_outline == b.row_outline,
        "col_outline" => a.col_outline == b.col_outline,
        "row_hidden" => a.row_hidden == b.row_hidden,
        "col_hidden" => a.col_hidden == b.col_hidden,
        "tables" => a.tables == b.tables,
        "rtl" => a.rtl == b.rtl,
        "freeze" => a.freeze == b.freeze,
        "show_gridlines" => a.show_gridlines == b.show_gridlines,
        "show_formulas" => a.show_formulas == b.show_formulas,
        "zoom_scale" => a.zoom_scale == b.zoom_scale,
        "hidden" => a.hidden == b.hidden,
        "tab_color" => a.tab_color == b.tab_color,
        "protected" => a.protected == b.protected,
        "protect_allow" => a.protect_allow == b.protect_allow,
        "protect_ranges" => a.protect_ranges == b.protect_ranges,
        "names" => a.names == b.names,
        "links" => a.links == b.links,
        "comments" => a.comments.len() == b.comments.len(),
        "cond" => a.cond == b.cond,
        "validations" => a.validations == b.validations,
        "scenarios" => a.scenarios == b.scenarios,
        "landscape" => a.landscape == b.landscape,
        "paper_size" => a.paper_size == b.paper_size,
        "margins_mm" => a.margins_mm == b.margins_mm,
        "print_areas" => a.print_areas == b.print_areas,
        "print_scale" => a.print_scale == b.print_scale,
        "fit_to_w" => a.fit_to_w == b.fit_to_w,
        "fit_to_h" => a.fit_to_h == b.fit_to_h,
        "row_breaks" => a.row_breaks == b.row_breaks,
        "col_breaks" => a.col_breaks == b.col_breaks,
        "print_gridlines" => a.print_gridlines == b.print_gridlines,
        "print_headings" => a.print_headings == b.print_headings,
        "print_title_rows" => a.print_title_rows == b.print_title_rows,
        "print_title_cols" => a.print_title_cols == b.print_title_cols,
        "header" => a.header == b.header,
        "footer" => a.footer == b.footer,
        "header_even" => a.header_even == b.header_even,
        "footer_even" => a.footer_even == b.footer_even,
        "header_first" => a.header_first == b.header_first,
        "footer_first" => a.footer_first == b.footer_first,
        "hf_diff_odd_even" => a.hf_diff_odd_even == b.hf_diff_odd_even,
        "hf_diff_first" => a.hf_diff_first == b.hf_diff_first,
        "shapes" => a.shapes.len() == b.shapes.len(),
        "images" => a.images.len() == b.images.len(),
        "phonetics" => a.phonetics == b.phonetics,
        _ => panic!("見張りの表にあるのに比べ方が無い: {name}"),
    }
}
