//! xlsx(SpreadsheetML)の読み書き。
//! 読めないものは黙って落とさず `Report` に積む(ooxml と同じ作法)。
use std::io::{Cursor, Read, Seek, Write};

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};

use crate::model::{Book, Cell, Pos, Sheet, Value};

#[derive(Debug, Default, Clone)]
pub struct Report {
    pub unsupported: Vec<(String, usize)>,
    pub sheets: usize,
    pub cells: usize,
}
impl Report {
    fn note(&mut self, n: &str) {
        match self.unsupported.iter_mut().find(|(x, _)| x == n) {
            Some(e) => e.1 += 1,
            None => self.unsupported.push((n.to_string(), 1)),
        }
    }
    pub fn is_lossless(&self) -> bool { self.unsupported.is_empty() }
}

fn local(n: &[u8]) -> &[u8] {
    match n.iter().position(|b| *b == b':') { Some(i) => &n[i + 1..], None => n }
}
fn attr(e: &BytesStart, want: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (local(a.key.as_ref()) == want.as_bytes())
            .then(|| String::from_utf8_lossy(&a.value).to_string())
    })
}

/// attr の実体参照(&lt; 等)を戻す版。自由な文字が入る属性(名前の類い)用
fn attr_un(e: &BytesStart, want: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (local(a.key.as_ref()) == want.as_bytes())
            .then(|| a.unescape_value().map(|v| v.to_string()).unwrap_or_default())
    })
}

/// sharedStrings.xml → 文字列表と、そのふりがな。
///
/// 日本語の xlsx には**ふりがな**(`<rPh>`)が入る。その中にも `<t>` があるので、
/// 素直に全部の `<t>` を拾うと「提案見積書テイアンミツモリショ」になる。
/// 欧米の実装が落としがちな箇所。ふりがなは本文には混ぜず、**別に持って**
/// 保存で書き戻す(PHONETIC 関数もこれを読む)。
fn parse_shared(xml: &str) -> (Vec<String>, Vec<Option<String>>) {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);
    let (mut out, mut cur) = (Vec::new(), String::new());
    let (mut rubies, mut ruby) = (Vec::new(), String::new());
    let (mut in_t, mut in_si, mut in_rph) = (false, false, false);
    let mut in_rt = false; // rPh の中の <t>
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"si" => {
                    in_si = true;
                    cur.clear();
                    ruby.clear();
                }
                b"rPh" => in_rph = true,
                b"t" if in_si && !in_rph => in_t = true,
                b"t" if in_rph => in_rt = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_t => cur.push_str(&t.unescape().unwrap_or_default()),
            Ok(Event::Text(t)) if in_rt => ruby.push_str(&t.unescape().unwrap_or_default()),
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                b"t" => {
                    in_t = false;
                    in_rt = false;
                }
                b"rPh" => in_rph = false,
                b"si" => {
                    in_si = false;
                    out.push(std::mem::take(&mut cur));
                    rubies.push(if ruby.is_empty() {
                        None
                    } else {
                        Some(std::mem::take(&mut ruby))
                    });
                }
                _ => {}
            },
            _ => {}
        }
        buf.clear();
    }
    (out, rubies)
}

/// `<mergeCell ref="A1:B2"/>` を結合として持つ(読み飛ばすと保存で消える)。
fn merge(e: &quick_xml::events::BytesStart, sh: &mut Sheet) {
    if let Some(r) = attr(e, "ref") {
        if let Some((a, b)) = r.split_once(':') {
            if let (Some(a), Some(b)) = (Pos::parse(a), Pos::parse(b)) {
                sh.merges.push((a, b));
            }
        }
    }
}

/// `<sheetView rightToLeft="1" showGridLines="0" zoomScale="85" …>` —
/// 画面の見え方。**Start と Empty の両方から呼ぶ** — Excel が書く sheetView は
/// 中に `<selection/>` や `<pane/>` を抱えるので Start で来る。Empty でしか
/// 見ていなかったので、**実物の xlsx では rtl すら読めていなかった**。
fn sheet_view(e: &quick_xml::events::BytesStart, sh: &mut Sheet) {
    let on = |k: &str| match attr(e, k).as_deref() {
        Some("1") | Some("true") => Some(true),
        Some(_) => Some(false),
        None => None,
    };
    sh.rtl = on("rightToLeft") == Some(true);
    sh.show_gridlines = on("showGridLines");
    sh.show_formulas = on("showFormulas");
    sh.zoom_scale = attr(e, "zoomScale").and_then(|v| v.parse().ok()).filter(|z| *z > 0);
}

/// `<pane xSplit="1" ySplit="1" topLeftCell="B2" activePane="bottomRight" state="frozen"/>` —
/// 固定枠。**xSplit が列、ySplit が行**(取り違えると縦横が入れ替わる)。
///
/// `state="split"` は掴んで動かす**分割**で固定ではないので捨てる。
/// 分割のときの xSplit は「何列ぶん」ではなく 1/20 ポイントの座標なので、
/// 固定として読むと途方もない列数になる — 撥ねるのが正しい。
fn pane(e: &quick_xml::events::BytesStart, sh: &mut Sheet) {
    if !matches!(attr(e, "state").as_deref(), Some("frozen") | Some("frozenSplit")) {
        return;
    }
    // schema は小数(xsd:double)なので一度 f64 で受けてから切り捨てる
    let n = |k: &str| {
        attr(e, k).and_then(|v| v.parse::<f64>().ok()).filter(|v| *v > 0.0).unwrap_or(0.0) as u32
    };
    let (frozen_columns, frozen_rows) = (n("xSplit"), n("ySplit"));
    // 両方 0 の pane は「固定していない」— 空の固定枠を持ち越さない
    if frozen_rows > 0 || frozen_columns > 0 {
        sh.freeze = Some(crate::model::FreezePane { frozen_rows, frozen_columns });
    }
}

/// `<row r="3" ht="27.5" customHeight="1" outlineLevel="1" hidden="1">` —
/// 指定のある行だけ持つ(高さ・グループ化の深さ・畳み)。
fn row_height(e: &quick_xml::events::BytesStart, sh: &mut Sheet) {
    let Some(r) = attr(e, "r").and_then(|v| v.parse::<u32>().ok()) else { return };
    if r < 1 {
        return;
    }
    let r0 = r - 1;
    if attr(e, "customHeight").as_deref() == Some("1") {
        if let Some(h) = attr(e, "ht").and_then(|v| v.parse::<f32>().ok()) {
            sh.row_height.insert(r0, h);
        }
    }
    if let Some(l) = attr(e, "outlineLevel").and_then(|v| v.parse::<u8>().ok()) {
        if l > 0 {
            sh.row_outline.insert(r0, l);
        }
    }
    if matches!(attr(e, "hidden").as_deref(), Some("1") | Some("true")) {
        sh.row_hidden.insert(r0);
    }
}

/// `<col min="1" max="3" width="12.5"/>` — min..=max は1始まり。
///
/// 全列に近い指定(既定幅)は展開しない。1列ずつに割ると
/// 16,384 個の col になって保存が肥大する。
fn col_width(e: &quick_xml::events::BytesStart, sh: &mut Sheet) {
    let g = |k: &str| attr(e, k).and_then(|v| v.parse::<f32>().ok());
    let (Some(min), Some(max)) = (g("min"), g("max")) else { return };
    if let Some(w) = g("width") {
        if max - min > 1000.0 {
            sh.default_col_width = Some(w);
            return;
        }
        for c in (min as u32)..=(max as u32) {
            if c >= 1 {
                sh.col_width.insert(c - 1, w);
            }
        }
    }
    // グループ化の深さと畳み(幅の指定が無い col でも来る)
    let level = attr(e, "outlineLevel").and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
    let hidden = matches!(attr(e, "hidden").as_deref(), Some("1") | Some("true"));
    if (level > 0 || hidden) && max - min <= 1000.0 {
        for c in (min as u32)..=(max as u32) {
            if c >= 1 {
                if level > 0 {
                    sh.col_outline.insert(c - 1, level);
                }
                if hidden {
                    sh.col_hidden.insert(c - 1);
                }
            }
        }
    }
}

/// styles.xml の dxfs(条件付き書式の見た目)→ (文字色, 塗り) の列。
fn parse_dxfs(xml: &str) -> Vec<(Option<String>, Option<String>)> {
    let mut r = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut buf = Vec::new();
    let (mut in_dxfs, mut in_dxf, mut in_font, mut in_fill) = (false, false, false, false);
    let mut cur: (Option<String>, Option<String>) = (None, None);
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"dxfs" => in_dxfs = true,
                b"dxf" if in_dxfs => {
                    in_dxf = true;
                    cur = (None, None);
                }
                b"font" if in_dxf => in_font = true,
                b"fill" if in_dxf => in_fill = true,
                b"color" if in_font => {
                    cur.0 = attr(&e, "rgb").map(|v| {
                        // 先頭の FF は 8桁(AARRGGBB)のときだけ透過(FFF2CC を壊さない)
                        if v.len() == 8 { v[2..].to_string() } else { v }
                    });
                }
                // 塗りの色は書き手によって置き場所が違う。
                //   LibreOffice  <patternFill><bgColor rgb="FFDDEBF7"/>
                //   openpyxl     <patternFill patternType="solid"><fgColor rgb="00DDEBF7"/>
                //   Excel        両方書き、片方は indexed="64"(rgb 無し)
                // **rgb を持っているほうを採る。** bgColor を先に見て、
                // それが rgb を持たないときだけ fgColor に落ちる
                b"bgColor" | b"fgColor" if in_fill => {
                    let c = attr(&e, "rgb").map(|v| {
                        if v.len() == 8 { v[2..].to_string() } else { v }
                    });
                    if c.is_some() && (local(e.name().as_ref()) == b"bgColor" || cur.1.is_none()) {
                        cur.1 = c;
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                b"dxfs" => in_dxfs = false,
                b"dxf" => {
                    if in_dxf {
                        out.push(std::mem::take(&mut cur));
                    }
                    in_dxf = false;
                }
                b"font" => in_font = false,
                b"fill" => in_fill = false,
                _ => {}
            },
            _ => {}
        }
        buf.clear();
    }
    out
}

/// _rels/*.rels → (Id, Type, Target, 外部か)
fn parse_rels(xml: &str) -> Vec<(String, String, String, bool)> {
    let mut r = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"Relationship" =>
            {
                out.push((
                    attr(&e, "Id").unwrap_or_default(),
                    attr(&e, "Type").unwrap_or_default(),
                    attr(&e, "Target").unwrap_or_default(),
                    attr(&e, "TargetMode").as_deref() == Some("External"),
                ));
            }
            _ => {}
        }
        buf.clear();
    }
    out
}

/// xl/worksheets/ からの相対の的を zip の中の道に直す("../comments1.xml" → "xl/comments1.xml")。
/// drawing のアンカーの中身(画像か図形か)。
enum DrawKind {
    /// 画像(r:embed)
    Image(String),
    /// 図形。中身(種類・色・文字・回転・線幅…)は詰めてあり、
    /// 置き場所と大きさ(at / width / height / dx / dy)は受け手が埋める
    Shape(Box<crate::model::SheetShape>),
}

/// drawing(xl/drawings/drawingN.xml)から、画像と図形のアンカーを拾う。
/// 返すのは (置き場所のセル, 幅EMU, 高さEMU, 中身)。
/// `xl/tables/tableN.xml` を読む。範囲が読めなければ None(黙って作らない)。
fn parse_table(xml: &str) -> Option<crate::model::TableDef> {
    let attr_of = |elem: &str, key: &str| -> Option<String> {
        let i = xml.find(&format!("<{elem}"))?;
        let rest = &xml[i..];
        let e = rest.find('>')?;
        let tag = &rest[..e];
        let k = format!("{key}=\"");
        let a = tag.find(&k)? + k.len();
        let b = tag[a..].find('"')? + a;
        Some(tag[a..b].to_string())
    };
    let r = attr_of("table", "ref")?;
    let (a, b) = match r.split_once(':') {
        Some((x, y)) => (Pos::parse(x)?, Pos::parse(y)?),
        None => {
            let p = Pos::parse(&r)?;
            (p, p)
        }
    };
    let num = |elem: &str, k: &str, d: u32| -> u32 {
        attr_of(elem, k).and_then(|v| v.parse().ok()).unwrap_or(d)
    };
    let on = |k: &str| -> bool {
        matches!(attr_of("tableStyleInfo", k).as_deref(), Some("1") | Some("true"))
    };
    Some(crate::model::TableDef {
        name: attr_of("table", "displayName")
            .or_else(|| attr_of("table", "name"))
            .unwrap_or_else(|| "テーブル".into()),
        a,
        b,
        header: num("table", "headerRowCount", 1) > 0,
        totals: num("table", "totalsRowCount", 0) > 0,
        banded_rows: on("showRowStripes"),
        banded_cols: on("showColumnStripes"),
        first_col: on("showFirstColumn"),
        last_col: on("showLastColumn"),
        filter: xml.contains("<autoFilter"),
    })
}

fn parse_drawing_anchors(xml: &str) -> Vec<(Pos, i64, i64, i64, i64, DrawKind)> {
    let mut r = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let (mut col, mut row) = (None::<u32>, None::<u32>);
    let (mut off_x, mut off_y) = (0i64, 0i64);
    let (mut cx, mut cy) = (None::<i64>, None::<i64>);
    let mut embed = None::<String>;
    let mut prst = None::<String>;
    // 図形の色: solidFill の1つ目が塗り、a:ln の中のものが線
    let (mut fill, mut line) = (None::<String>, None::<String>);
    // 図形の中の文字(a:t)と、custGeom の折れ線
    let mut text = String::new();
    let mut in_t = false;
    let mut pts: Vec<(f32, f32)> = Vec::new();
    let mut sp_name: Option<String> = None;
    let (mut path_w, mut path_h) = (1000.0f32, 1000.0f32);
    let mut has_custom = false;
    let mut in_from = false;
    let mut in_ln = false;
    let mut in_sp = false;
    // 回転・反転・線幅・不透明度・影(xfrm / a:ln w / a:alpha / outerShdw)
    let mut rot = 0.0f32;
    let (mut flip_h, mut flip_v) = (false, false);
    let mut line_w = 1.5f32;
    let mut alpha: Option<f32> = None;
    let mut shadow = false;
    // effectLst の中の色や alpha を塗りと取り違えない
    let mut in_effect = false;
    let mut cur: Vec<u8> = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"oneCellAnchor" | b"twoCellAnchor" | b"absoluteAnchor" => {
                    (col, row, cx, cy, embed, prst, fill, line) =
                        (None, None, None, None, None, None, None, None);
                    (off_x, off_y) = (0, 0);
                    text.clear();
                    pts.clear();
                    sp_name = None;
                    has_custom = false;
                    (path_w, path_h) = (1000.0, 1000.0);
                    in_sp = false;
                    in_ln = false;
                    rot = 0.0;
                    (flip_h, flip_v) = (false, false);
                    line_w = 1.5;
                    alpha = None;
                    shadow = false;
                    in_effect = false;
                }
                b"from" => in_from = true,
                t @ (b"col" | b"row" | b"colOff" | b"rowOff") if in_from => {
                    cur = t.to_vec()
                }
                b"sp" => in_sp = true,
                b"cNvPr" if sp_name.is_none() => sp_name = attr(&e, "name"),
                b"xfrm" if in_sp => {
                    rot = attr(&e, "rot")
                        .and_then(|v| v.parse::<i64>().ok())
                        .map(|v| v as f32 / 60000.0)
                        .unwrap_or(0.0);
                    flip_h = attr(&e, "flipH").as_deref() == Some("1");
                    flip_v = attr(&e, "flipV").as_deref() == Some("1");
                }
                b"effectLst" => in_effect = true,
                b"outerShdw" if in_sp => shadow = true,
                b"ln" => {
                    in_ln = true;
                    if let Some(w) = attr(&e, "w").and_then(|v| v.parse::<f32>().ok()) {
                        line_w = w / 12700.0;
                    }
                }
                b"blip" => {
                    if embed.is_none() {
                        embed = attr(&e, "embed");
                    }
                }
                b"prstGeom" => {
                    if prst.is_none() {
                        prst = attr(&e, "prst");
                    }
                }
                b"custGeom" => has_custom = true,
                // alpha を子に持つ色は Start で来る(<a:srgbClr><a:alpha/></a:srgbClr>)
                b"srgbClr" if in_sp && !in_effect => {
                    let v = attr(&e, "val");
                    if in_ln {
                        if line.is_none() {
                            line = v;
                        }
                    } else if fill.is_none() {
                        fill = v;
                    }
                }
                b"path" if has_custom => {
                    path_w = attr(&e, "w").and_then(|v| v.parse().ok()).unwrap_or(1000.0);
                    path_h = attr(&e, "h").and_then(|v| v.parse().ok()).unwrap_or(1000.0);
                }
                b"t" if in_sp => in_t = true,
                _ => cur.clear(),
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"ext" => {
                    if cx.is_none() {
                        cx = attr(&e, "cx").and_then(|v| v.parse().ok());
                        cy = attr(&e, "cy").and_then(|v| v.parse().ok());
                    }
                }
                b"blip" => {
                    if embed.is_none() {
                        embed = attr(&e, "embed");
                    }
                }
                b"cNvPr" if sp_name.is_none() => sp_name = attr(&e, "name"),
                b"pt" if has_custom => {
                    let x = attr(&e, "x").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                    let y = attr(&e, "y").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                    pts.push((x / path_w.max(1.0), y / path_h.max(1.0)));
                }
                b"srgbClr" if in_sp && !in_effect => {
                    let v = attr(&e, "val");
                    if in_ln {
                        if line.is_none() {
                            line = v;
                        }
                    } else if fill.is_none() {
                        fill = v;
                    }
                }
                b"alpha" if in_sp && !in_effect && alpha.is_none() => {
                    alpha = attr(&e, "val")
                        .and_then(|v| v.parse::<f32>().ok())
                        .map(|v| v / 100_000.0);
                }
                b"outerShdw" if in_sp => shadow = true,
                b"ln" if in_sp => {
                    if let Some(w) = attr(&e, "w").and_then(|v| v.parse::<f32>().ok()) {
                        line_w = w / 12700.0;
                    }
                }
                b"xfrm" if in_sp => {
                    rot = attr(&e, "rot")
                        .and_then(|v| v.parse::<i64>().ok())
                        .map(|v| v as f32 / 60000.0)
                        .unwrap_or(0.0);
                    flip_h = attr(&e, "flipH").as_deref() == Some("1");
                    flip_v = attr(&e, "flipV").as_deref() == Some("1");
                }
                _ => {}
            },
            Ok(Event::Text(t)) if in_t => {
                text.push_str(&t.unescape().unwrap_or_default());
            }
            Ok(Event::Text(t)) if !cur.is_empty() => {
                let raw = t.unescape().unwrap_or_default();
                let v: i64 = raw.trim().parse().unwrap_or(0);
                match cur.as_slice() {
                    b"col" => col = Some(v.max(0) as u32),
                    b"row" => row = Some(v.max(0) as u32),
                    b"colOff" => off_x = v,
                    _ => off_y = v,
                }
            }
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                b"from" => {
                    in_from = false;
                    cur.clear();
                }
                b"col" | b"row" | b"colOff" | b"rowOff" => cur.clear(),
                b"ln" => in_ln = false,
                b"effectLst" => in_effect = false,
                b"t" => in_t = false,
                b"oneCellAnchor" | b"twoCellAnchor" | b"absoluteAnchor" => {
                    // 図形の雛形(場所と大きさは受け手が埋める)
                    let tpl = crate::model::SheetShape {
                        fill: fill.take(),
                        line: line.take(),
                        text: (!text.is_empty()).then(|| text.clone()),
                        rot,
                        flip_h,
                        flip_v,
                        line_w,
                        alpha: alpha.unwrap_or(1.0),
                        shadow,
                        ..Default::default()
                    };
                    let kind = match (embed.take(), prst.take(), has_custom) {
                        (Some(em), _, _) => Some(DrawKind::Image(em)),
                        (None, Some(pr), _) => Some(DrawKind::Shape(Box::new(
                            crate::model::SheetShape { kind: pr, ..tpl },
                        ))),
                        (None, None, true) if !pts.is_empty() => {
                            // 自作の札(jo:spark-col:底)があれば棒に組み直す。
                            // 棒は4点1組の閉じた小道 — 先頭2点の中点が中心、
                            // 1点目の y が先端
                            let marker = sp_name
                                .as_deref()
                                .and_then(|n| n.strip_prefix("jo:"))
                                .and_then(|n| n.split_once(':'))
                                .filter(|(k, _)| *k == "spark-col" || *k == "spark-wl");
                            match marker {
                                Some((k, b)) if pts.len() >= 4 => {
                                    let base: f32 = b.parse().unwrap_or(1.0);
                                    let tops: Vec<(f32, f32)> = pts
                                        .chunks(4)
                                        .filter(|c| c.len() == 4)
                                        .map(|c| ((c[0].0 + c[1].0) / 2.0, c[0].1))
                                        .collect();
                                    Some(DrawKind::Shape(Box::new(
                                        crate::model::SheetShape {
                                            kind: k.into(),
                                            points: tops,
                                            base,
                                            ..tpl
                                        },
                                    )))
                                }
                                _ => Some(DrawKind::Shape(Box::new(
                                    crate::model::SheetShape {
                                        kind: "spark".into(),
                                        points: std::mem::take(&mut pts),
                                        ..tpl
                                    },
                                ))),
                            }
                        }
                        _ => None,
                    };
                    if let (Some(c), Some(rr), Some(k)) = (col, row, kind) {
                        out.push((
                            Pos::new(rr, c),
                            off_x,
                            off_y,
                            cx.unwrap_or(300 * 9525),
                            cy.unwrap_or(200 * 9525),
                            k,
                        ));
                    }
                }
                _ => {}
            },
            _ => {}
        }
        buf.clear();
    }
    out
}

/// 挿した図形1枚のアンカー(oneCellAnchor の xdr:sp)。Excel でも図形として開ける。
fn shape_anchor_xml(sp: &crate::model::SheetShape, id: u32) -> String {
    let (cx, cy) = ((sp.width_px * 9525.0) as i64, (sp.height_px * 9525.0) as i64);
    // 不透明度は srgbClr の子 a:alpha(10万分率)。1.0 なら書かない
    let alpha = if sp.alpha < 0.999 {
        format!(
            "<a:alpha val=\"{}\"/>",
            (sp.alpha.clamp(0.0, 1.0) * 100_000.0) as i64
        )
    } else {
        String::new()
    };
    let fill = match &sp.fill {
        Some(c) => format!("<a:solidFill><a:srgbClr val=\"{c}\">{alpha}</a:srgbClr></a:solidFill>"),
        None => "<a:noFill/>".to_string(),
    };
    let line = match &sp.line {
        Some(c) => format!(
            "<a:ln w=\"{w}\"><a:solidFill><a:srgbClr val=\"{c}\">{alpha}</a:srgbClr></a:solidFill></a:ln>",
            w = (sp.line_w.max(0.1) * 12700.0) as i64
        ),
        None => String::new(),
    };
    // 影(右下への落ち影)。色は固定の灰 — 画面の描き方と揃える
    let effect = if sp.shadow {
        concat!(
            "<a:effectLst><a:outerShdw blurRad=\"50800\" dist=\"50800\" ",
            "dir=\"2700000\" algn=\"tl\" rotWithShape=\"0\">",
            "<a:srgbClr val=\"9E9E9E\"><a:alpha val=\"35000\"/></a:srgbClr>",
            "</a:outerShdw></a:effectLst>"
        )
    } else {
        ""
    };
    // 回転(6万分の1度)と反転は xfrm の属性
    let mut xfrm_attrs = String::new();
    let rot = sp.rot.rem_euclid(360.0);
    if rot != 0.0 {
        xfrm_attrs.push_str(&format!(" rot=\"{}\"", (rot * 60000.0) as i64));
    }
    if sp.flip_h {
        xfrm_attrs.push_str(" flipH=\"1\"");
    }
    if sp.flip_v {
        xfrm_attrs.push_str(" flipV=\"1\"");
    }
    // 形: 折れ線(spark)は custGeom、他は prstGeom。
    // 縦棒・勝ち負けも custGeom(棒ごとに4点の閉じた小道 — Excel でも棒に見える)
    let bars = matches!(sp.kind.as_str(), "spark-col" | "spark-wl");
    let poly = matches!(sp.kind.as_str(), "spark" | "ink" | "marker");
    let geom = if bars && !sp.points.is_empty() {
        let n = sp.points.len().max(1) as f32;
        let bw = (10000.0 / n * 0.7).max(120.0);
        let base = (sp.base * 10000.0) as i64;
        let mut path = String::new();
        for (cx_, ty) in &sp.points {
            let (l, r) = (
                ((cx_ * 10000.0) - bw / 2.0) as i64,
                ((cx_ * 10000.0) + bw / 2.0) as i64,
            );
            let t = (ty * 10000.0) as i64;
            path.push_str(&format!(
                concat!(
                    "<a:moveTo><a:pt x=\"{l}\" y=\"{t}\"/></a:moveTo>",
                    "<a:lnTo><a:pt x=\"{r}\" y=\"{t}\"/></a:lnTo>",
                    "<a:lnTo><a:pt x=\"{r}\" y=\"{b}\"/></a:lnTo>",
                    "<a:lnTo><a:pt x=\"{l}\" y=\"{b}\"/></a:lnTo>",
                    "<a:close/>"
                ),
                l = l, r = r, t = t, b = base
            ));
        }
        format!(
            concat!(
                "<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>",
                "<a:rect l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>",
                "<a:pathLst><a:path w=\"10000\" h=\"10000\">{}</a:path></a:pathLst>",
                "</a:custGeom>"
            ),
            path
        )
    } else if poly && !sp.points.is_empty() {
        let mut path = String::new();
        for (i, (x, y)) in sp.points.iter().enumerate() {
            let (px_, py_) = ((x * 10000.0) as i64, (y * 10000.0) as i64);
            if i == 0 {
                path.push_str(&format!(
                    "<a:moveTo><a:pt x=\"{px_}\" y=\"{py_}\"/></a:moveTo>"
                ));
            } else {
                path.push_str(&format!(
                    "<a:lnTo><a:pt x=\"{px_}\" y=\"{py_}\"/></a:lnTo>"
                ));
            }
        }
        format!(
            concat!(
                "<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>",
                "<a:rect l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>",
                "<a:pathLst><a:path w=\"10000\" h=\"10000\" fill=\"none\">{}</a:path></a:pathLst>",
                "</a:custGeom>"
            ),
            path
        )
    } else {
        format!("<a:prstGeom prst=\"{}\"><a:avLst/></a:prstGeom>", sp.kind)
    };
    // 中の文字(テキストボックス)
    let txt = match &sp.text {
        Some(t) => format!(
            concat!(
                "<xdr:txBody><a:bodyPr wrap=\"square\"/><a:lstStyle/>",
                "<a:p><a:r><a:rPr lang=\"ja-JP\" sz=\"1100\"/><a:t>{}</a:t></a:r></a:p>",
                "</xdr:txBody>"
            ),
            esc(t)
        ),
        None => String::new(),
    };
    format!(
        concat!(
            "<xdr:oneCellAnchor>",
            "<xdr:from><xdr:col>{col}</xdr:col><xdr:colOff>{dx}</xdr:colOff>",
            "<xdr:row>{row}</xdr:row><xdr:rowOff>{dy}</xdr:rowOff></xdr:from>",
            "<xdr:ext cx=\"{cx}\" cy=\"{cy}\"/>",
            "<xdr:sp macro=\"\" textlink=\"\">",
            "<xdr:nvSpPr><xdr:cNvPr id=\"{id}\" name=\"{name}\"/><xdr:cNvSpPr/></xdr:nvSpPr>",
            "<xdr:spPr><a:xfrm{xfrm}><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>",
            "{geom}{fill}{line}{effect}</xdr:spPr>{txt}",
            "</xdr:sp><xdr:clientData/></xdr:oneCellAnchor>"
        ),
        col = sp.at.col,
        row = sp.at.row,
        dx = (sp.dx_px * 9525.0) as i64,
        dy = (sp.dy_px * 9525.0) as i64,
        cx = cx,
        cy = cy,
        id = id,
        // 縦棒・勝ち負けは name に自作の札を残す — 開き直しで棒に組み直せる
        name = if matches!(sp.kind.as_str(), "spark-col" | "spark-wl") {
            format!("jo:{}:{:.4}", sp.kind, sp.base)
        } else {
            format!("図形 {id}")
        },
        geom = geom,
        fill = fill,
        line = line,
        effect = effect,
        xfrm = xfrm_attrs,
        txt = txt
    )
}

/// 挿した画像1枚のアンカー(oneCellAnchor)。大きさは px → EMU(9525 EMU = 1px)。
fn image_anchor_xml(im: &crate::model::SheetImage, rid: &str, id: u32) -> String {
    let (cx, cy) = ((im.width_px * 9525.0) as i64, (im.height_px * 9525.0) as i64);
    format!(
        concat!(
            "<xdr:oneCellAnchor>",
            "<xdr:from><xdr:col>{col}</xdr:col><xdr:colOff>{cox}</xdr:colOff>",
            "<xdr:row>{row}</xdr:row><xdr:rowOff>{roy}</xdr:rowOff></xdr:from>",
            "<xdr:ext cx=\"{cx}\" cy=\"{cy}\"/>",
            "<xdr:pic><xdr:nvPicPr><xdr:cNvPr id=\"{id}\" name=\"画像 {id}\"/><xdr:cNvPicPr/></xdr:nvPicPr>",
            "<xdr:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill>",
            "<xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>",
            "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic>",
            "<xdr:clientData/></xdr:oneCellAnchor>"
        ),
        col = im.at.col,
        row = im.at.row,
        cox = (im.dx_px * 9525.0) as i64,
        roy = (im.dy_px * 9525.0) as i64,
        cx = cx,
        cy = cy,
        id = id,
        rid = rid
    )
}

/// `_xlnm.Print_Titles` の行の部($1:$4)を(シート番号, (先頭行, 末尾行))に解く。
/// 列の繰り返し($A:$B)や混在は None(原文のまま持ち越す)。
fn parse_print_titles(raw: &str) -> Option<(usize, (u32, u32))> {
    let sid = raw
        .split(SID_ATTR)
        .nth(1)
        .and_then(|r| r.split('"').next())
        .and_then(|v| v.parse::<usize>().ok())?;
    let body = raw.split('>').nth(1).and_then(|r| r.split('<').next())?;
    let range = body.rsplit('!').next()?.replace('$', "");
    let (a, b) = range.split_once(':')?;
    let (a, b) = (a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?);
    if a == 0 || b == 0 {
        return None;
    }
    Some((sid, (a - 1, b - 1)))
}

fn resolve_target(t: &str) -> String {
    if let Some(rest) = t.strip_prefix("../") {
        format!("xl/{rest}")
    } else if let Some(rest) = t.strip_prefix('/') {
        rest.to_string()
    } else {
        format!("xl/worksheets/{t}")
    }
}

/// `xl/_rels/workbook.xml.rels` の的を zip の中の道に直す。
/// 相対は `xl/` から数える("worksheets/sheet3.xml" → "xl/worksheets/sheet3.xml")。
/// 先頭の `/` は入れ物の根から、`../` は一つ上へ。
fn resolve_book_target(t: &str) -> String {
    if let Some(rest) = t.strip_prefix('/') {
        return rest.to_string();
    }
    let mut dirs: Vec<&str> = vec!["xl"];
    let mut rest = t;
    loop {
        if let Some(r) = rest.strip_prefix("../") {
            dirs.pop();
            rest = r;
        } else if let Some(r) = rest.strip_prefix("./") {
            rest = r;
        } else {
            break;
        }
    }
    dirs.push(rest);
    dirs.join("/")
}

/// `xl/worksheets/sheet23.xml` → 23。**数として**並べ替えるための鍵。
/// 文字列のままだと `sheet10.xml` が `sheet2.xml` より前に来る。
/// 番号の読めない部品は最後へ回す
fn sheet_part_no(n: &str) -> u32 {
    n.rsplit_once("/sheet")
        .and_then(|(_, r)| r.strip_suffix(".xml"))
        .and_then(|d| d.parse().ok())
        .unwrap_or(u32::MAX)
}

/// `workbook.xml` の `<sheet>` の並びを、それぞれの本体の部品名に直す。
///
/// **`r:id` を `xl/_rels/workbook.xml.rels` で解くのが正道。** 部品の番号は
/// `<sheet>` の並びとも rId の順とも一致しない — Excel でシートを消したり
/// 並べ替えたりすると離れる。`r:id` か rels の項が無いときだけ、部品を
/// **数として**並べ替えて位置で対にする(昔のやり方)。
///
/// 返すのは `<sheet>` と同じ長さの列。解けなかった所は空文字
/// (中身は空になるが、**名前と並びは狂わない**)。
///
/// `entries` は zip にある部品の全部(的が実在するかを確かめる)、
/// `parts` は控えに使う `xl/worksheets/sheetN.xml` を数で並べた列
fn sheet_parts(
    rids: &[Option<String>],
    rels: &[(String, String)],
    entries: &[String],
    parts: &[String],
) -> Vec<String> {
    rids.iter()
        .enumerate()
        .map(|(i, rid)| {
            rid.as_deref()
                .and_then(|r| rels.iter().find(|(id, _)| id == r))
                .map(|(_, target)| target.clone())
                .filter(|p| entries.iter().any(|n| n == p))
                .or_else(|| parts.get(i).cloned())
                .unwrap_or_default()
        })
        .collect()
}

/// `<definedName>` が「単純」か — こちらのモデルで往復できる名前か。
///
/// **数ではなく意味で見る。** 前は属性の**数**を数えて、知っている分
/// (`name` と `localSheetId`)にちょうど合うときだけ単純としていた。
/// LibreOffice は名前の定義すべてに `function="false" hidden="false"
/// vbProcedure="false"` を**既定値でも書く**ので数が合わず、中身は Excel と
/// 同じ(`式!$A$1:$A$5`)なのに全部「理解できない名前」へ落ちていた。
/// 式から引くと `#NAME?` になる(2026-08-09 第2便)。
///
/// 見方:
/// - `name` / `localSheetId` — 既に理解して往復させている。単純のまま
/// - 真偽の属性 — **偽(`0` / `false`)は無いのと同じ**。真なら隠し名前か
///   マクロなので、こちらでは扱わず原文で持ち越す
/// - それ以外(`comment` / `description` など中身を持つ属性)— 単純と見ると
///   保存で書き戻せず**黙って落ちる**ので、原文で持ち越す側へ回す
fn defined_name_plain(e: &BytesStart) -> bool {
    /// 既定が偽で、立っていたら単純ではなくなる属性
    const FLAGS: [&str; 6] = [
        "hidden",
        "function",
        "vbProcedure",
        "xlm",
        "publishToServer",
        "workbookParameter",
    ];
    e.attributes().flatten().all(|a| {
        let k = String::from_utf8_lossy(local(a.key.as_ref())).to_string();
        let v = String::from_utf8_lossy(&a.value).to_string();
        match k.as_str() {
            "name" => true,
            // 読めない番号は「理解した」と言えない(シート限定が消える)
            "localSheetId" => v.parse::<usize>().is_ok(),
            _ if FLAGS.contains(&k.as_str()) => !matches!(v.as_str(), "1" | "true" | "TRUE"),
            _ => false,
        }
    })
}

/// commentsN.xml → (セル参照, 本文) の列
fn parse_comments(xml: &str) -> Vec<(Pos, String)> {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);
    let mut out = Vec::new();
    let mut buf = Vec::new();
    let mut cur: Option<Pos> = None;
    let mut text = String::new();
    let mut in_t = false;
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"comment" => {
                    cur = attr(&e, "ref").and_then(|s| Pos::parse(&s));
                    text.clear();
                }
                b"t" if cur.is_some() => in_t = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_t => text.push_str(&t.unescape().unwrap_or_default()),
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                b"t" => in_t = false,
                b"comment" => {
                    if let Some(p) = cur.take() {
                        out.push((p, std::mem::take(&mut text)));
                    }
                }
                _ => {}
            },
            _ => {}
        }
        buf.clear();
    }
    out
}

fn parse_sheet(xml: &str, shared: &[String], rubies: &[Option<String>],
               styles: &[crate::model::CellFormat], name: &str, rep: &mut Report) -> Sheet {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);
    let mut sh = Sheet::new(name);
    let (mut pos, mut ty) = (None::<Pos>, String::new());
    // いま <colBreaks> の中か(<rowBreaks> と <brk> の形が同じため)
    let mut in_col_breaks = false;
    // いま <customSheetView> の中か。**あそこにも <pane> がぶら下がる** —
    // 誰かが昔しまい込んだ表示設定を、いまの固定枠として読んでしまわないため
    // (pageSetup など他の元素も同じ形で入るが、そちらは元からの持ち越し)
    let mut in_custom_view = false;
    let (mut in_v, mut in_f, mut in_is) = (false, false, false);
    let (mut v, mut f) = (String::new(), String::new());
    // 印刷のヘッダー/フッター(Some(true)=oddHeader の中)
    let mut hf_side: Option<bool> = None;
    let mut style: Option<usize> = None;
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                // 印刷のヘッダー/フッター(文字は子の Text で拾う)
                b"oddHeader" | b"oddFooter" => {
                    hf_side = Some(local(e.name().as_ref()) == b"oddHeader");
                }
                b"row" => row_height(&e, &mut sh),
                b"c" => {
                    pos = attr(&e, "r").and_then(|s| Pos::parse(&s));
                    ty = attr(&e, "t").unwrap_or_default();
                    // s は styles.xml の cellXfs の索引。書式はそちらにある
                    style = attr(&e, "s").and_then(|s| s.parse::<usize>().ok());
                    v.clear(); f.clear();
                }
                b"v" => in_v = true,
                b"f" => {
                    in_f = true;
                    // 昔ながらの配列数式(CSE)。**覆う範囲は式でなく人が
                    // 決める**ので、その範囲をここで覚える
                    if attr(&e, "t").as_deref() == Some("array") {
                        if let (Some(p), Some(r)) = (pos, attr(&e, "ref")) {
                            let mut it = r.split(':');
                            let a = it.next().and_then(Pos::parse);
                            let b = it.next().and_then(Pos::parse).or(a);
                            if let (Some(a), Some(b)) = (a, b) {
                                let h = b.row.saturating_sub(a.row) + 1;
                                let w = b.col.saturating_sub(a.col) + 1;
                                sh.cse.insert(p, (h, w));
                            }
                        }
                    }
                }
                b"is" => in_is = true,
                b"mergeCell" => merge(&e, &mut sh),
                b"col" => col_width(&e, &mut sh),
                // 画面の見え方と固定枠。**Excel の sheetView は子を抱えるので
                // ここ(Start)に来る** — Empty 側にも同じ組を置いてある
                b"sheetView" => sheet_view(&e, &mut sh),
                b"pane" if !in_custom_view => pane(&e, &mut sh),
                b"customSheetView" => in_custom_view = true,
                // 改ページの束。**中の <brk> は縦横で形が同じ**なので、
                // どちらの中にいるかをここで覚える(Start でしか来ない)
                b"rowBreaks" => in_col_breaks = false,
                b"colBreaks" => in_col_breaks = true,
                _ => {}
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"col" => col_width(&e, &mut sh),
                b"c" => {
                    // 値の無い自己完結のセル。書式だけなら、それは帳票の枠 —
                    // 落とすと保存で罫線が消える(Excel 以外の道具が書く形)
                    if let (Some(p), Some(si)) = (
                        attr(&e, "r").and_then(|s| Pos::parse(&s)),
                        attr(&e, "s").and_then(|s| s.parse::<usize>().ok()),
                    ) {
                        let fmt = styles.get(si).cloned().unwrap_or_default();
                        if !fmt.is_plain() {
                            rep.cells += 1;
                            sh.set(p, Cell { formula: None, value: Value::Empty, fmt });
                            sh.style_of.insert(p, si as u32);
                        }
                    }
                    pos = None;
                }
                b"mergeCell" => merge(&e, &mut sh),
                // 印刷の設定。読むだけ(保存は原文持ち越しが正)— PDF が従う
                b"pageSetup" => {
                    sh.landscape = attr(&e, "orientation").as_deref() == Some("landscape");
                    sh.paper_size = attr(&e, "paperSize").and_then(|v| v.parse().ok());
                    sh.print_scale = attr(&e, "scale").and_then(|v| v.parse().ok());
                    // 紙 N 枚に収める。0 は「合わせない」なので None に倒す
                    let n = |k: &str| {
                        attr(&e, k).and_then(|v| v.parse::<u32>().ok()).filter(|v| *v > 0)
                    };
                    sh.fit_to_w = n("fitToWidth");
                    sh.fit_to_h = n("fitToHeight");
                }
                b"printOptions" => {
                    let on = |k: &str| {
                        matches!(attr(&e, k).as_deref(), Some("1") | Some("true"))
                    };
                    sh.print_gridlines = on("gridLines");
                    sh.print_headings = on("headings");
                }
                // 画面の見え方(右から左・格子線・倍率)と固定枠。
                // 子を持たない sheetView はこちら(Start 側にも同じ組がある)
                b"sheetView" => sheet_view(&e, &mut sh),
                b"pane" if !in_custom_view => pane(&e, &mut sh),
                // 耳(タブ)の色。rgb 指定だけ拾う(theme 指定は色に解けない)
                b"tabColor" => {
                    sh.tab_color = attr(&e, "rgb");
                }
                // シートの保護。sheet="0" と書く道具は保護していない扱い
                b"sheetProtection" => {
                    sh.protected =
                        !matches!(attr(&e, "sheet").as_deref(), Some("0") | Some("false"));
                    // **xlsx は「禁じる」向きで書く**(formatCells="1" = 禁じる)。
                    // こちらは「許す」向きで持つので裏返す。属性が無いときの
                    // 既定も向きが違う: 選択は許す(false=禁じない)、
                    // 他は禁じる(true)— Excel が保護を掛けたときの初期値
                    let deny = |k: &str, when_absent: bool| -> bool {
                        match attr(&e, k).as_deref() {
                            Some("0") | Some("false") => false,
                            Some(_) => true,
                            None => when_absent,
                        }
                    };
                    let a = &mut sh.protect_allow;
                    a.select_locked = !deny("selectLockedCells", false);
                    a.select_unlocked = !deny("selectUnlockedCells", false);
                    a.format_cells = !deny("formatCells", true);
                    a.format_cols = !deny("formatColumns", true);
                    a.format_rows = !deny("formatRows", true);
                    a.insert_cols = !deny("insertColumns", true);
                    a.insert_rows = !deny("insertRows", true);
                    a.insert_links = !deny("insertHyperlinks", true);
                    a.delete_cols = !deny("deleteColumns", true);
                    a.delete_rows = !deny("deleteRows", true);
                    a.sort = !deny("sort", true);
                    a.autofilter = !deny("autoFilter", true);
                    a.pivot = !deny("pivotTables", true);
                }
                // 改ページ。**縦(colBreaks)と横(rowBreaks)を取り違えない** —
                // どちらの中にいるかを見る。縦は読んでいなかったので、Excel で
                // 入れた列の区切りが開いて保存するだけで消えていた
                b"rowBreaks" => in_col_breaks = false,
                b"colBreaks" => in_col_breaks = true,
                b"brk" => {
                    if let Some(id) = attr(&e, "id").and_then(|v| v.parse().ok()) {
                        if in_col_breaks {
                            sh.col_breaks.push(id);
                        } else {
                            sh.row_breaks.push(id);
                        }
                    }
                }
                b"pageMargins" => {
                    let g = |k: &str| {
                        attr(&e, k).and_then(|v| v.parse::<f32>().ok()).map(|inch| inch * 25.4)
                    };
                    if let (Some(l), Some(r), Some(t), Some(b)) =
                        (g("left"), g("right"), g("top"), g("bottom"))
                    {
                        sh.margins_mm = Some((l, r, t, b));
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) if hf_side.is_some() => {
                let s = t.unescape().unwrap_or_default().to_string();
                if !s.is_empty() {
                    if hf_side == Some(true) {
                        sh.header = Some(s);
                    } else {
                        sh.footer = Some(s);
                    }
                }
            }
            Ok(Event::Text(t)) if in_v || in_f || in_is => {
                let s = t.unescape().unwrap_or_default();
                if in_f { f.push_str(&s) } else { v.push_str(&s) }
            }
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                b"oddHeader" | b"oddFooter" => hf_side = None,
                b"customSheetView" => in_custom_view = false,
                b"v" => in_v = false,
                b"f" => in_f = false,
                b"is" => in_is = false,
                b"c" => {
                    if let Some(p) = pos.take() {
                        let value = match ty.as_str() {
                            "s" => {
                                let i = v.trim().parse::<usize>().ok();
                                // ふりがな(rPh)はセルに紐づけて持つ
                                if let Some(r) =
                                    i.and_then(|i| rubies.get(i).cloned()).flatten()
                                {
                                    sh.phonetics.insert(p, r);
                                }
                                i.and_then(|i| shared.get(i).cloned())
                                    .map(Value::Text).unwrap_or(Value::Empty)
                            }
                            "b" => Value::Bool(v.trim() == "1"),
                            "e" => Value::Error(v.trim().to_string()),
                            "str" | "inlineStr" => Value::Text(v.trim().to_string()),
                            _ => match v.trim().parse::<f64>() {
                                Ok(n) => Value::Number(n),
                                Err(_) if v.trim().is_empty() => Value::Empty,
                                Err(_) => Value::Text(v.trim().to_string()),
                            },
                        };
                        let fmt = style
                            .and_then(|i| styles.get(i).cloned())
                            .unwrap_or_default();
                        let cell = Cell {
                            formula: (!f.is_empty()).then(|| f.clone()),
                            value,
                            fmt,
                        };
                        // **罫線だけのセル**も帳票では意味を持つので落とさない
                        if cell.formula.is_some() || !cell.value.is_empty()
                            || !cell.fmt.is_plain() {
                            rep.cells += 1;
                            sh.set(p, cell);
                            // 原本の書式索引を控える(保存で据え置くため)
                            if let Some(si) = style {
                                sh.style_of.insert(p, si as u32);
                            }
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
        buf.clear();
    }
    sh
}

pub fn read<R: Read + Seek>(src: R) -> Result<(Book, Report), String> {
    let mut zip = zip::ZipArchive::new(src).map_err(|e| format!("zipを開けません: {e}"))?;
    let mut rep = Report::default();

    // 書式表を先に読む。セルの s= はこの索引
    let mut styles: Vec<crate::model::CellFormat> = Vec::new();
    let mut dxfs: Vec<(Option<String>, Option<String>)> = Vec::new();
    // テーマの色(styles より先に読む — 色を解くのに要る)
    let theme_colors: Vec<String> = {
        let mut tx = String::new();
        if let Ok(mut f) = zip.by_name("xl/theme/theme1.xml") {
            let _ = f.read_to_string(&mut tx);
        }
        crate::theme::parse(&tx)
    };
    let mut named_styles: Vec<(String, Option<u32>, crate::model::CellFormat)> = Vec::new();
    if let Ok(mut f) = zip.by_name("xl/styles.xml") {
        let mut s = String::new();
        let _ = f.read_to_string(&mut s);
        styles = crate::styles::parse(&s, &theme_colors);
        dxfs = parse_dxfs(&s);
        // 名前付きセルスタイル(「見出し 1」など)。マークダウンの見出しの
        // 書式はここから引く — 型紙に定義しておけば全ブックに効く
        named_styles = crate::styles::parse_named(&s, &theme_colors);
    }

    let (shared, rubies) = {
        let mut s = String::new();
        match zip.by_name("xl/sharedStrings.xml") {
            Ok(mut f) => {
                let _ = f.read_to_string(&mut s);
                parse_shared(&s)
            }
            Err(_) => (Vec::new(), Vec::new()),
        }
    };
    // シート名(workbook.xml の並び順)と、名前の定義
    let mut names = Vec::new();
    let mut hiddens: Vec<bool> = Vec::new();
    // 各 `<sheet>` の r:id。本体の部品はこれを rels で解いて突き止める
    let mut rids: Vec<Option<String>> = Vec::new();
    // (名前, 中身) — 中身は 'Sheet1'!$A$1:$B$2 の形
    // (名前, 中身, シート限定ならその番号)
    let mut defined: Vec<(String, String, Option<usize>)> = Vec::new();
    // 理解できなかった definedName の原文(hidden 属性つき等)。捨てない
    let mut defined_raw: Vec<String> = Vec::new();
    let mut calc_manual = false;
    let mut calc_iter: Option<(u32, f64)> = None;
    let mut r1c1 = false;
    let mut read_only_rec = false;
    if let Ok(mut f) = zip.by_name("xl/workbook.xml") {
        let mut s = String::new();
        let _ = f.read_to_string(&mut s);
        let mut r = Reader::from_str(&s);
        let mut buf = Vec::new();
        let mut in_defined: Option<(String, bool, usize, Option<usize>)> = None; // (name, 属性が単純か, 原文の頭)
        let mut text = String::new();
        let mut last = r.buffer_position() as usize;
        loop {
            let ev = r.read_event_into(&mut buf);
            let start_pos = last;
            last = r.buffer_position() as usize;
            match ev {
                Ok(Event::Eof) | Err(_) => break,
                Ok(Event::Start(e)) | Ok(Event::Empty(e))
                    if local(e.name().as_ref()) == b"sheet" =>
                {
                    names.push(attr(&e, "name").unwrap_or_else(|| "Sheet".into()));
                    hiddens.push(matches!(
                        attr(&e, "state").as_deref(),
                        Some("hidden") | Some("veryHidden")
                    ));
                    // `r:id`(local は "id"。同居する sheetId とは別物)
                    rids.push(attr(&e, "id"));
                }
                // 計算方法(calcPr)。manual を落とすと開き直しで勝手に自動へ戻る
                // 1904年の日付系(古い Mac の Excel)。保存は原文持ち越しで
                // 守られるが、表示は 1900 系のまま=4年ずれる。黙らない
                // 読み取り専用のお願い(鍵ではない)
                Ok(Event::Start(e)) | Ok(Event::Empty(e))
                    if local(e.name().as_ref()) == b"workbookProtection" =>
                {
                    read_only_rec = matches!(
                        attr(&e, "readOnlyRecommended").as_deref(),
                        Some("1") | Some("true")
                    );
                }
                Ok(Event::Start(e)) | Ok(Event::Empty(e))
                    if local(e.name().as_ref()) == b"workbookPr" =>
                {
                    if matches!(attr(&e, "date1904").as_deref(), Some("1") | Some("true")) {
                        rep.note("1904年の日付系(このブックの日付表示は4年ずれます。保存では保たれます)");
                    }
                }
                Ok(Event::Start(e)) | Ok(Event::Empty(e))
                    if local(e.name().as_ref()) == b"calcPr" =>
                {
                    calc_manual = attr(&e, "calcMode").as_deref() == Some("manual");
                    r1c1 = attr(&e, "refMode").as_deref() == Some("R1C1");
                    if attr(&e, "iterate").as_deref() == Some("1") {
                        let n = attr(&e, "iterateCount")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(100);
                        let d = attr(&e, "iterateDelta")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0.001);
                        calc_iter = Some((n, d));
                    }
                }
                Ok(Event::Start(e)) if local(e.name().as_ref()) == b"definedName" => {
                    // **localSheetId は理解する。** Excel の「このシートだけ」の
                    // 名前で、素通しにしていたので式から使えなかった。
                    // 単純かどうかは属性の**意味**で決める(defined_name_plain)。
                    // 数で数えると、既定値まで書く書き手(LibreOffice)の名前が
                    // 丸ごと使えなくなる
                    let sid = attr(&e, "localSheetId").and_then(|v| v.parse::<usize>().ok());
                    let nm = attr(&e, "name").unwrap_or_default();
                    // **`_xlnm.` で始まる名前(印刷範囲・タイトル行)はここで
                    // 拾わない。** 別の道でモデルへ入れているので、こちらでも
                    // 拾うと二重になって印刷の設定が壊れる(踏んで直した)
                    let simple = defined_name_plain(&e) && !nm.starts_with("_xlnm.");
                    in_defined = Some((nm, simple, start_pos, sid));
                    text.clear();
                }
                Ok(Event::Text(t)) if in_defined.is_some() => {
                    text.push_str(&t.unescape().unwrap_or_default());
                }
                Ok(Event::End(e)) if local(e.name().as_ref()) == b"definedName" => {
                    if let Some((nm, simple, at, sid)) = in_defined.take() {
                        if simple {
                            defined.push((nm, std::mem::take(&mut text), sid));
                        } else {
                            defined_raw.push(s[at..last].to_string());
                        }
                    }
                }
                _ => {}
            }
            buf.clear();
        }
    }
    // zip にある部品の全部(rels の的が実在するかを確かめるのに使う)
    let entries: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();
    // 控え(r:id が無いか rels に項の無いとき)。**数として**並べ替える —
    // 文字列だと sheet10.xml が sheet2.xml より前に来て、シートが 10 枚以上
    // ある帳面は中身が丸ごと入れ替わる
    let mut parts: Vec<String> = entries
        .iter()
        .filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
        .cloned()
        .collect();
    parts.sort_by(|a, b| sheet_part_no(a).cmp(&sheet_part_no(b)).then_with(|| a.cmp(b)));
    // ブックの rels(rId → 部品)。**シートの割り当てはこれが正**
    let book_rels: Vec<(String, String)> = {
        let mut s = String::new();
        if let Ok(mut f) = zip.by_name("xl/_rels/workbook.xml.rels") {
            let _ = f.read_to_string(&mut s);
        }
        parse_rels(&s)
            .into_iter()
            .filter(|(id, _, _, ext)| !id.is_empty() && !ext)
            .map(|(id, _, target, _)| (id, resolve_book_target(&target)))
            .collect()
    };
    // シートの並びは `<sheet>` の順。workbook.xml が読めなかったときだけ部品順
    let paths: Vec<String> = if names.is_empty() {
        parts.clone()
    } else {
        sheet_parts(&rids, &book_rels, &entries, &parts)
    };

    let mut book = Book {
        sheets: Vec::new(),
        names_raw: defined_raw,
        theme: theme_colors.clone(),
        named_styles,
        calc_manual,
        calc_iter,
        r1c1,
        read_only_rec,
        ..Default::default()
    };
    // ブックの情報(docProps/core.xml)。読んで見せる。保存は原文持ち越し
    // なので、開いたファイルの情報は保存で消えない
    if let Ok(mut f) = zip.by_name("docProps/core.xml") {
        let mut s = String::new();
        let _ = f.read_to_string(&mut s);
        let unesc = |t: &str| {
            t.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&")
        };
        let grab = |tag: &str| -> String {
            let open = format!("<{tag}");
            let close = format!("</{tag}>");
            s.find(&open)
                .and_then(|i| {
                    let rest = &s[i..];
                    let a = rest.find('>')? + 1;
                    // <tag/> の自己完結は空欄
                    if rest.as_bytes().get(a - 2) == Some(&b'/') {
                        return None;
                    }
                    let b = rest.find(&close)?;
                    (b >= a).then(|| unesc(&rest[a..b]))
                })
                .unwrap_or_default()
        };
        book.props = crate::model::BookProps {
            creator: grab("dc:creator"),
            title: grab("dc:title"),
            subject: grab("dc:subject"),
            keywords: grab("cp:keywords"),
            description: grab("dc:description"),
        };
    }
    for (i, path) in paths.iter().enumerate() {
        let mut s = String::new();
        if let Ok(mut f) = zip.by_name(path) { let _ = f.read_to_string(&mut s); }
        let name = names.get(i).cloned().unwrap_or_else(|| format!("Sheet{}", i + 1));
        let mut sh = parse_sheet(&s, &shared, &rubies, &styles, &name, &mut rep);
        sh.hidden = hiddens.get(i).copied().unwrap_or(false);
        // このシートの rels(ハイパーリンクの先・コメントの部品への道)
        let rels_path = {
            let base = path.rsplit_once('/').map(|(_, b)| b).unwrap_or(path);
            format!("xl/worksheets/_rels/{base}.rels")
        };
        let mut rels = Vec::new();
        if let Ok(mut f) = zip.by_name(&rels_path) {
            let mut rs = String::new();
            let _ = f.read_to_string(&mut rs);
            rels = parse_rels(&rs);
        }

            // cfRule の頭を読む(種類ごと。第1版 2026-08-07 で拡張)。
            // 読めない種類は今までどおり報告して落とす
            fn parse_cf_start(
                e: &quick_xml::events::BytesStart,
                rule: &mut Option<(String, Option<usize>)>,
                formula: &mut String,
                rep: &mut Report,
            ) {
                let ty = attr(e, "type").unwrap_or_default();
                let dxf: Option<usize> = attr(e, "dxfId").and_then(|v| v.parse().ok());
                *rule = match ty.as_str() {
                    "cellIs" => Some((attr(e, "operator").unwrap_or_default(), dxf)),
                    "containsText" => {
                        Some((format!("text:{}", attr(e, "text").unwrap_or_default()), dxf))
                    }
                    "duplicateValues" => Some(("dup".into(), dxf)),
                    "uniqueValues" => Some(("uniq".into(), dxf)),
                    "top10" => {
                        if attr(e, "percent").as_deref() == Some("1") {
                            rep.note("条件付き書式(上位/下位のパーセント。保存で失われる)");
                            None
                        } else {
                            let n = attr(e, "rank")
                                .and_then(|v| v.parse::<u32>().ok())
                                .unwrap_or(10);
                            let bottom = attr(e, "bottom").as_deref() == Some("1");
                            Some((format!("top:{n}:{}", bottom as u8), dxf))
                        }
                    }
                    "aboveAverage" => {
                        let below = attr(e, "aboveAverage").as_deref() == Some("0");
                        Some((format!("avg:{}", below as u8), dxf))
                    }
                    "dataBar" => Some(("bar".into(), dxf)),
                    "colorScale" => Some(("scale".into(), dxf)),
                    "iconSet" => Some(("icons".into(), dxf)),
                    // 数式で指定。中身は <formula> の子にある(finish_cf で取る)
                    "expression" => Some(("expr".into(), dxf)),
                    _ => {
                        rep.note("条件付き書式(読めない種類。保存で失われる)");
                        None
                    }
                };
                formula.clear();
            }

            // 1本の cfRule を確定して sh に積む(<cfRule …/> の自己閉じは
            // End が来ない — Empty のその場でもここを通す)
            #[allow(clippy::too_many_arguments)]
            fn finish_cf(
                sh: &mut Sheet,
                sqref: Option<(Pos, Pos)>,
                taken: Option<(String, Option<usize>)>,
                formula: &str,
                dxfs: &[(Option<String>, Option<String>)],
                cf_colors: &[String],
                icon_name: Option<&str>,
                rep: &mut Report,
            ) {

                if let (Some(range), Some((tag, dxf))) = (sqref, taken) {
                    use crate::model::CondKind;
                    // formula は 1〜2 本(between は改行区切りで貯まる)
                    let nums: Vec<f64> = formula
                        .split('\u{1f}')
                        .filter_map(|t| t.trim().parse::<f64>().ok())
                        .collect();
                    let kind: Option<CondKind> = if let Some(t) = tag.strip_prefix("text:") {
                        Some(CondKind::Text(t.to_string()))
                    } else if tag == "dup" {
                        Some(CondKind::Dup(false))
                    } else if tag == "uniq" {
                        Some(CondKind::Dup(true))
                    } else if let Some(rest) = tag.strip_prefix("top:") {
                        let mut it = rest.split(':');
                        let n = it.next().and_then(|v| v.parse().ok()).unwrap_or(10);
                        let bottom = it.next() == Some("1");
                        Some(CondKind::Top(n, bottom))
                    } else if let Some(rest) = tag.strip_prefix("avg:") {
                        Some(CondKind::Avg(rest == "1"))
                    } else if tag == "bar" {
                        Some(CondKind::Bar(
                            cf_colors.first().cloned().unwrap_or_else(|| "638EC6".into()),
                        ))
                    } else if tag == "scale" {
                        match cf_colors {
                            [lo, hi] => Some(CondKind::Scale(lo.clone(), None, hi.clone())),
                            [lo, m, hi] => {
                                Some(CondKind::Scale(lo.clone(), Some(m.clone()), hi.clone()))
                            }
                            _ => {
                                rep.note("条件付き書式(カラースケールの色が読めない。保存で失われる)");
                                None
                            }
                        }
                    } else if tag == "icons" {
                        Some(CondKind::Icons(
                            icon_name.unwrap_or("3TrafficLights1").to_string(),
                        ))
                    } else if tag == "expr" {
                        // 数式で指定。<formula> は1本(2本目以降は Excel も見ない)。
                        // 式は範囲の左上を錨にした原文のまま貯める
                        match formula.split('\u{1f}').next().map(str::trim) {
                            Some(f) if !f.is_empty() => Some(CondKind::Formula(f.to_string())),
                            _ => {
                                rep.note("条件付き書式(数式で指定だが式が空。保存で失われる)");
                                None
                            }
                        }
                    } else {
                        match (crate::model::CondOp::from_xlsx(&tag), nums.as_slice()) {
                            (Some(op), [v, ..]) => Some(CondKind::Cmp(op, *v)),
                            (None, [lo, hi]) if tag == "between" => {
                                Some(CondKind::Between(*lo, *hi, false))
                            }
                            (None, [lo, hi]) if tag == "notBetween" => {
                                Some(CondKind::Between(*lo, *hi, true))
                            }
                            _ => None,
                        }
                    };
                    match kind {
                        Some(kind) => {
                            let (color, fill) = dxf
                                .and_then(|i| dxfs.get(i).cloned())
                                .unwrap_or((None, None));
                            sh.cond.push(crate::model::CondRule {
                                range, kind, color, fill,
                            });
                        }
                        None => rep.note(
                            "条件付き書式(読めない条件。保存で失われる)",
                        ),
                    }
                }
                        
            }
        // 条件付き書式。cellIs(値との比較)だけ理解し、他は報告
        {
            let mut r = Reader::from_str(&s);
            let mut buf = Vec::new();
            let mut sqref: Option<(Pos, Pos)> = None;
            let mut rule: Option<(String, Option<usize>)> = None; // (operator, dxfId)
            let mut in_formula = false;
            let mut formula = String::new();
            // バー/スケール/アイコンの中身(cfRule の子から拾う)
            let mut cf_colors: Vec<String> = Vec::new();
            let mut icon_name: Option<String> = None;
            loop {
                match r.read_event_into(&mut buf) {
                    Ok(Event::Eof) | Err(_) => break,
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if local(e.name().as_ref()) == b"conditionalFormatting" =>
                    {
                        sqref = attr(&e, "sqref").and_then(|v| {
                            let v = v.split_whitespace().next()?.to_string();
                            match v.split_once(':') {
                                Some((a, b)) => Some((Pos::parse(a)?, Pos::parse(b)?)),
                                None => {
                                    let p = Pos::parse(&v)?;
                                    Some((p, p))
                                }
                            }
                        });
                    }
                    Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"cfRule" => {
                        parse_cf_start(&e, &mut rule, &mut formula, &mut rep);
                        cf_colors.clear();
                        icon_name = None;
                        // 自己閉じは End が来ない — その場で確定
                        finish_cf(&mut sh, sqref, rule.take(), &formula, &dxfs,
                            &cf_colors, icon_name.as_deref(), &mut rep);
                    }
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"cfRule" => {
                        parse_cf_start(&e, &mut rule, &mut formula, &mut rep);
                        cf_colors.clear();
                        icon_name = None;
                    }
                    // cfRule の子: <color rgb>(バー/スケール)と <iconSet iconSet=…>
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if rule.is_some() && local(e.name().as_ref()) == b"color" =>
                    {
                        if let Some(rgb) = attr(&e, "rgb") {
                            // 頭の FF(不透明)は1回だけ剥がす — trim_start_matches は
                            // 繰り返し剥がすので FFFFEB84(黄)が EB84 に化ける
                            let c = if rgb.len() == 8 { rgb[2..].to_string() } else { rgb };
                            cf_colors.push(c);
                        }
                    }
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if rule.is_some() && local(e.name().as_ref()) == b"iconSet" =>
                    {
                        icon_name = attr(&e, "iconSet").or(Some("3TrafficLights1".into()));
                    }
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"formula" => {
                        in_formula = true;
                    }
                    Ok(Event::Text(t)) if in_formula => {
                        formula.push_str(&t.unescape().unwrap_or_default());
                    }
                    Ok(Event::End(e)) => match local(e.name().as_ref()) {
                        b"formula" => {
                            in_formula = false;
                            formula.push('\u{1f}'); // 2本目との区切り(between)
                        }
                        b"cfRule" => {
                            finish_cf(&mut sh, sqref, rule.take(), &formula, &dxfs,
                                &cf_colors, icon_name.as_deref(), &mut rep);
                        }
                        b"conditionalFormatting" => sqref = None,
                        _ => {}
                    },
                    _ => {}
                }
                buf.clear();
            }
        }
        // データの入力規則。list(候補から選ぶ)だけ理解し、他は報告
        {
            let mut r = Reader::from_str(&s);
            let mut buf = Vec::new();
            // (sqref の原文, list か)。formula1 は子要素なので End まで貯める
            // 種類・比較・第2式・文言まで全部持ち越す(知らない種類も落とさない)
            let mut dv: Option<crate::model::Validation> = None;
            let mut dv_sq = String::new();
            let mut in_f: u8 = 0; // 1=formula1 2=formula2
            let read_attrs = |e: &quick_xml::events::BytesStart| -> (crate::model::Validation, String) {
                let a = |k: &str| attr(e, k).unwrap_or_default();
                let input = {
                    let (t, b) = (a("promptTitle"), a("prompt"));
                    (!t.is_empty() || !b.is_empty()).then_some((t, b))
                };
                let error = {
                    let (t, b) = (a("errorTitle"), a("error"));
                    let style = attr(e, "errorStyle").unwrap_or_else(|| "stop".into());
                    (!t.is_empty() || !b.is_empty()).then_some((style, t, b))
                };
                (
                    crate::model::Validation {
                        range: (Pos::new(0, 0), Pos::new(0, 0)),
                        formula: String::new(),
                        kind: a("type"),
                        op: a("operator"),
                        formula2: String::new(),
                        input_msg: input,
                        error_msg: error,
                        allow_blank: attr(e, "allowBlank").as_deref() != Some("0"),
                        hide_arrow: attr(e, "showDropDown").as_deref() == Some("1"),
                    },
                    a("sqref"),
                )
            };
            let push = |sh: &mut crate::model::Sheet, v: crate::model::Validation, sq: &str| {
                // sqref は空白区切りで複数の範囲を持てる
                for part in sq.split_whitespace() {
                    let range = match part.split_once(':') {
                        Some((a, b)) => Pos::parse(a).zip(Pos::parse(b)),
                        None => Pos::parse(part).map(|p| (p, p)),
                    };
                    if let Some(range) = range {
                        let mut v = v.clone();
                        v.range = range;
                        sh.validations.push(v);
                    }
                }
            };
            loop {
                match r.read_event_into(&mut buf) {
                    Ok(Event::Eof) | Err(_) => break,
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"dataValidation" => {
                        let (v, sq) = read_attrs(&e);
                        dv = Some(v);
                        dv_sq = sq;
                    }
                    // 自己閉じ = 式を持たない規則(文言だけ等)。それも持ち越す
                    Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"dataValidation" => {
                        let (v, sq) = read_attrs(&e);
                        push(&mut sh, v, &sq);
                    }
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"formula1" => in_f = 1,
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"formula2" => in_f = 2,
                    Ok(Event::Text(t)) if in_f > 0 => {
                        let s = t.unescape().unwrap_or_default();
                        if let Some(v) = &mut dv {
                            if in_f == 1 {
                                v.formula.push_str(&s);
                            } else {
                                v.formula2.push_str(&s);
                            }
                        }
                    }
                    Ok(Event::End(e)) => match local(e.name().as_ref()) {
                        b"formula1" | b"formula2" => in_f = 0,
                        b"dataValidation" => {
                            if let Some(mut v) = dv.take() {
                                v.formula = v.formula.trim().to_string();
                                v.formula2 = v.formula2.trim().to_string();
                                push(&mut sh, v, &dv_sq);
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
                buf.clear();
            }
        }
        // ハイパーリンク。r:id の付いた外部URLだけ理解し、文書内の場所は報告
        {
            let mut r = Reader::from_str(&s);
            let mut buf = Vec::new();
            loop {
                match r.read_event_into(&mut buf) {
                    Ok(Event::Eof) | Err(_) => break,
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if local(e.name().as_ref()) == b"hyperlink" =>
                    {
                        let p = attr(&e, "ref").and_then(|v| Pos::parse(&v));
                        let rid = attr(&e, "id");
                        match (p, rid) {
                            (Some(p), Some(rid)) => {
                                if let Some((_, _, target, _)) = rels
                                    .iter()
                                    .find(|(id, ty, _, ext)| {
                                        *id == rid && ty.ends_with("/hyperlink") && *ext
                                    })
                                {
                                    sh.links.insert(p, target.clone());
                                }
                            }
                            // id が無く location だけ=帳面の中の場所。# 付きで持つ
                            (Some(p), None) if attr(&e, "location").is_some() => {
                                sh.links.insert(p, format!("#{}", attr(&e, "location").unwrap()));
                            }
                            _ => rep.note("ハイパーリンク(読めない形)"),
                        }
                    }
                    _ => {}
                }
                buf.clear();
            }
        }
        // コメント(commentsN.xml)。rels の type で結ばれている
        if let Some((_, _, target, _)) =
            rels.iter().find(|(_, ty, _, _)| ty.ends_with("/comments"))
        {
            if let Ok(mut f) = zip.by_name(&resolve_target(target)) {
                let mut cs = String::new();
                let _ = f.read_to_string(&mut cs);
                for (p, t) in parse_comments(&cs) {
                    sh.comments.insert(p, t);
                }
            }
        }
        // 表オブジェクト(xlsx の table)。範囲に変換・サイズ変更のために持つ
        for (_, _ty, target, _) in rels.iter().filter(|(_, t, _, _)| t.ends_with("/table")) {
            let tpath = resolve_target(target);
            let mut tx = String::new();
            if let Ok(mut f) = zip.by_name(&tpath) {
                let _ = f.read_to_string(&mut tx);
            }
            if let Some(t) = parse_table(&tx) {
                sh.tables.push(t);
            } else {
                rep.note("表オブジェクト(範囲が読めない)");
            }
        }
        // 画像(drawing)。**表示のために**読む — 保存は原文の持ち越しが担うので、
        // ここで読んだ絵を書き直すことはしない(図形など理解しない部品を壊さない)
        if let Some((_, _, target, _)) =
            rels.iter().find(|(_, ty, _, _)| ty.ends_with("/drawing"))
        {
            let dpath = resolve_target(target);
            let mut dx = String::new();
            if let Ok(mut f) = zip.by_name(&dpath) {
                let _ = f.read_to_string(&mut dx);
            }
            let drels = {
                let (dir, base) = dpath.rsplit_once('/').unwrap_or(("xl/drawings", &dpath));
                format!("{dir}/_rels/{base}.rels")
            };
            let mut rx = String::new();
            if let Ok(mut f) = zip.by_name(&drels) {
                let _ = f.read_to_string(&mut rx);
            }
            let dmap = parse_rels(&rx);
            for (at, ox_emu, oy_emu, cx_emu, cy_emu, kind) in parse_drawing_anchors(&dx) {
                let (width_px, height_px) =
                    (cx_emu as f32 / 9525.0, cy_emu as f32 / 9525.0);
                match kind {
                    DrawKind::Image(embed) => {
                        let Some((_, _, t, _)) =
                            dmap.iter().find(|(id, _, _, _)| *id == embed)
                        else {
                            rep.note("画像(実体への参照が無い)");
                            continue;
                        };
                        let mpath = resolve_target(t);
                        let mut data = Vec::new();
                        if let Ok(mut f) = zip.by_name(&mpath) {
                            let _ = f.read_to_end(&mut data);
                        }
                        if data.is_empty() {
                            rep.note("画像(実体が見つからない)");
                            continue;
                        }
                        sh.images.push(crate::model::SheetImage {
                            at,
                            dx_px: 0.0,
                            dy_px: 0.0,
                            width_px,
                            height_px,
                            data,
                        });
                    }
                    DrawKind::Shape(mut sp) => {
                        sp.at = at;
                        sp.width_px = width_px;
                        sp.height_px = height_px;
                        // ずらし(colOff/rowOff)も読む — SmartArt の
                        // 図形の集まりが保存後も同じ場所に見える
                        sp.dx_px = ox_emu as f32 / 9525.0;
                        sp.dy_px = oy_emu as f32 / 9525.0;
                        sh.shapes.push(*sp);
                    }
                }
            }
        }
        book.sheets.push(sh);
        rep.sheets += 1;
    }
    if book.sheets.is_empty() {
        return Err("worksheet がありません(xlsxではない可能性)".into());
    }
    // 名前の定義をシートへ配る。'Sheet1'!$A$1:$B$2 の形だけ理解し、
    // それ以外(複数範囲・行列全体・_xlnm 系)は原文のまま持ち越す
    for (nm, target, sid) in defined {
        match split_defined(&target) {
            Some((sheet_name, r)) => {
                // シート限定の印があればその番号を信じる(同じ名前が
                // 何枚にもあるとき、名前だけでは行き先が決まらない)
                let idx = sid
                    .filter(|i| *i < book.sheets.len())
                    .or_else(|| book.sheets.iter().position(|s| s.name == sheet_name));
                match idx.map(|i| &mut book.sheets[i]) {
                    Some(sh) => sh.names.push((nm, r)),
                    None => book.names_raw.push(format!(
                        "<definedName name=\"{}\">{}</definedName>",
                        esc(&nm),
                        esc(&target)
                    )),
                }
            }
            None => book.names_raw.push(format!(
                "<definedName name=\"{}\">{}</definedName>",
                esc(&nm),
                esc(&target)
            )),
        }
    }
    // ブックに載せた Python(独自部品 xl/joPython.xml)。**読むだけで実行しない**
    {
        let mut sx = String::new();
        if let Ok(mut f) = zip.by_name("xl/joPython.xml") {
            let _ = f.read_to_string(&mut sx);
        }
        if !sx.is_empty() {
            let mut r = Reader::from_str(&sx);
            let mut buf = Vec::new();
            let mut name = None::<String>;
            let mut code = String::new();
            let mut in_s = false;
            loop {
                match r.read_event_into(&mut buf) {
                    Ok(Event::Eof) | Err(_) => break,
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"script" => {
                        name = attr(&e, "name");
                        code.clear();
                        in_s = true;
                    }
                    Ok(Event::Text(t)) if in_s => {
                        code.push_str(&t.unescape().unwrap_or_default());
                    }
                    Ok(Event::End(e)) if local(e.name().as_ref()) == b"script" => {
                        if let Some(n) = name.take() {
                            // 古いブックに載っている Python は**関数(UDF)も含めて
                            // 読むだけ**。実行せず、保存でブックから消える
                            // (2026-08-08 発注者確定 → 2026-08-09 に UDF まで拡張:
                            // データとプログラムを1つのファイルにしない)。
                            // 黙って落とさない: 開くときの報告で言う。
                            // 取り出しは @export 名前 → 中を確かめて plugins へ
                            rep.note(
                                "ブックに載っていた Python(実行しません。@export で取り出して plugins へ。保存でブックから消えます)",
                            );
                            book.scripts.push((n, code.clone()));
                        }
                        in_s = false;
                    }
                    _ => {}
                }
                buf.clear();
            }
        }
    }
    // 変更履歴(独自部品 xl/joChanges.xml)。読んで持ち、保存で書き戻す
    {
        let mut cx = String::new();
        if let Ok(mut f) = zip.by_name("xl/joChanges.xml") {
            let _ = f.read_to_string(&mut cx);
        }
        if !cx.is_empty() {
            let mut r = Reader::from_str(&cx);
            let mut buf = Vec::new();
            loop {
                match r.read_event_into(&mut buf) {
                    Ok(Event::Eof) | Err(_) => break,
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if local(e.name().as_ref()) == b"c" =>
                    {
                        let at = attr_un(&e, "at").and_then(|v| Pos::parse(&v));
                        if let Some(at) = at {
                            book.changes.push(crate::model::ChangeRec {
                                who: attr_un(&e, "who").unwrap_or_default(),
                                when: attr_un(&e, "when").unwrap_or_default(),
                                sheet: attr_un(&e, "sheet").unwrap_or_default(),
                                at,
                                before: attr_un(&e, "before").unwrap_or_default(),
                                after: attr_un(&e, "after").unwrap_or_default(),
                            });
                        }
                    }
                    _ => {}
                }
                buf.clear();
            }
        }
    }
    // ピボットの指図(独自部品 xl/joPivot.xml)。読むだけ — 更新は明示の操作
    {
        let mut sx = String::new();
        if let Ok(mut f) = zip.by_name("xl/joPivot.xml") {
            let _ = f.read_to_string(&mut sx);
        }
        if !sx.is_empty() {
            let mut r = Reader::from_str(&sx);
            let mut buf = Vec::new();
            let mut cur: Option<crate::model::PivotDef> = None;
            let mut field = 0u8; // 1 = <r> 行の見出し / 2 = <c> 列の見出し
            loop {
                match r.read_event_into(&mut buf) {
                    Ok(Event::Eof) | Err(_) => break,
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"pivot" => {
                        let range = attr(&e, "src").unwrap_or_default();
                        let mut it = range.split(':');
                        let a = it.next().and_then(Pos::parse);
                        let b = it.next().and_then(Pos::parse);
                        let dest = attr(&e, "dest").and_then(|d| Pos::parse(&d));
                        if let (Some(a), Some(b), Some(dest)) = (a, b, dest) {
                            cur = Some(crate::model::PivotDef {
                                sheet: attr_un(&e, "sheet").unwrap_or_default(),
                                src: (a, b),
                                rows_sel: Vec::new(),
                                cols_sel: Vec::new(),
                                value: attr_un(&e, "value").unwrap_or_default(),
                                agg: attr_un(&e, "agg").unwrap_or_else(|| "合計".into()),
                                totals: attr(&e, "totals").as_deref() == Some("1"),
                                subtotals: attr(&e, "subtotals").as_deref() == Some("1"),
                                blank_rows: attr(&e, "blank").as_deref() == Some("1"),
                                compact: attr(&e, "compact").as_deref() == Some("1"),
                                dest,
                                size: (
                                    attr(&e, "h").and_then(|v| v.parse().ok()).unwrap_or(0),
                                    attr(&e, "w").and_then(|v| v.parse().ok()).unwrap_or(0),
                                ),
                                hide: Vec::new(),
                                style: attr_un(&e, "style").unwrap_or_default(),
                                name: attr_un(&e, "name").unwrap_or_default(),
                                vfilter: None,
                                group_by: Vec::new(),
                                show_as: String::new(),
                            });
                        }
                    }
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"r" => field = 1,
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"c" => field = 2,
                    // 絞り込み(隠す値)。<f name="見出し"><v>値</v>…</f>
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"f" => {
                        if let Some(d) = cur.as_mut() {
                            d.hide.push((attr_un(&e, "name").unwrap_or_default(), Vec::new()));
                        }
                    }
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if local(e.name().as_ref()) == b"vf" =>
                    {
                        if let Some(d) = cur.as_mut() {
                            let op = attr_un(&e, "op").unwrap_or_default();
                            let th = attr(&e, "v").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                            d.vfilter = Some((op, th));
                        }
                    }
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if local(e.name().as_ref()) == b"g" =>
                    {
                        if let Some(d) = cur.as_mut() {
                            d.group_by.push((
                                attr_un(&e, "name").unwrap_or_default(),
                                attr_un(&e, "unit").unwrap_or_default(),
                            ));
                        }
                    }
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if local(e.name().as_ref()) == b"sa" =>
                    {
                        if let Some(d) = cur.as_mut() {
                            d.show_as = attr_un(&e, "v").unwrap_or_default();
                        }
                    }
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"v" => field = 3,
                    Ok(Event::Text(t)) if field > 0 => {
                        if let Some(d) = cur.as_mut() {
                            let v = t.unescape().unwrap_or_default().to_string();
                            match field {
                                1 => d.rows_sel.push(v),
                                2 => d.cols_sel.push(v),
                                _ => {
                                    if let Some((_, vs)) = d.hide.last_mut() {
                                        vs.push(v);
                                    }
                                }
                            }
                        }
                    }
                    Ok(Event::End(e))
                        if local(e.name().as_ref()) == b"r"
                            || local(e.name().as_ref()) == b"c"
                            || local(e.name().as_ref()) == b"v" =>
                    {
                        field = 0;
                    }
                    Ok(Event::End(e)) if local(e.name().as_ref()) == b"pivot" => {
                        if let Some(d) = cur.take() {
                            book.pivots.push(d);
                        }
                    }
                    _ => {}
                }
                buf.clear();
            }
        }
    }
    // スピルの記録(独自部品 xl/joSpill.xml)。これが無いと、開き直したとき
    // 自分のスピル跡が他人のデータに見えて偽の #SPILL! になる
    {
        let mut sx = String::new();
        if let Ok(mut f) = zip.by_name("xl/joSpill.xml") {
            let _ = f.read_to_string(&mut sx);
        }
        if !sx.is_empty() {
            let mut r = Reader::from_str(&sx);
            let mut buf = Vec::new();
            loop {
                match r.read_event_into(&mut buf) {
                    Ok(Event::Eof) | Err(_) => break,
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if local(e.name().as_ref()) == b"s" =>
                    {
                        let sheet = attr_un(&e, "sheet").unwrap_or_default();
                        let at = attr(&e, "at").and_then(|v| Pos::parse(&v));
                        let h: u32 =
                            attr(&e, "h").and_then(|v| v.parse().ok()).unwrap_or(0);
                        let w: u32 =
                            attr(&e, "w").and_then(|v| v.parse().ok()).unwrap_or(0);
                        if let Some(at) = at.filter(|_| h > 0 && w > 0) {
                            if let Some(s) =
                                book.sheets.iter_mut().find(|s| s.name == sheet)
                            {
                                s.spills.insert(at, (h, w));
                            }
                        }
                    }
                    _ => {}
                }
                buf.clear();
            }
        }
    }
    // 印刷範囲は編集の対象なのでモデルへ(他の definedName は原文のまま)。
    // 読めない形だけ原文に残す — 黙って捨てない
    let mut rest = Vec::new();
    for raw in std::mem::take(&mut book.names_raw) {
        if raw.contains("_xlnm.Print_Area") {
            if let Some((sid, areas)) = parse_print_area(&raw) {
                if let Some(sh) = book.sheets.get_mut(sid) {
                    sh.print_areas.extend(areas);
                    continue;
                }
            }
        }
        if raw.contains("_xlnm.Print_Titles") {
            // 行の部($1:$4)だけ読む。列の繰り返しはまだ(原文のまま残す)
            if let Some((sid, rows)) = parse_print_titles(&raw) {
                if let Some(sh) = book.sheets.get_mut(sid) {
                    sh.print_title_rows = Some(rows);
                    continue;
                }
            }
        }
        rest.push(raw);
    }
    book.names_raw = rest;
    Ok((book, rep))
}

/// `_xlnm.Print_Area` の definedName を(シート番号, 範囲の列)に解く。
/// `,` 区切りの複数の域も受ける。読めなければ None。
fn parse_print_area(raw: &str) -> Option<(usize, Vec<(Pos, Pos)>)> {
    let sid = raw
        .split(SID_ATTR)
        .nth(1)
        .and_then(|r| r.split('"').next())
        .and_then(|v| v.parse::<usize>().ok())?;
    let body = raw.split('>').nth(1).and_then(|r| r.split('<').next())?;
    let mut out = Vec::new();
    for part in body.split(',') {
        let range = part.rsplit('!').next().unwrap_or(part);
        let parsed = match range.split_once(':') {
            Some((x, y)) => Pos::parse(x).zip(Pos::parse(y)),
            None => Pos::parse(range).map(|p| (p, p)),
        };
        out.push(parsed?);
    }
    if out.is_empty() {
        return None;
    }
    Some((sid, out))
}

/// localSheetId 属性の頭(引用符の入れ子を避けるため定数で持つ)
const SID_ATTR: &str = "localSheetId=\"";

// ---------- 書く ----------

const CT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>__SHEETS__<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;

const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const RNS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
     .replace('"', "&quot;")
}

/// definedName の中身を (シート名, "A1" か "A1:B2") に分ける。
/// 'Sheet 1'!$A$1 の引用も解く。理解できない形なら None(原文で持ち越す側)。
fn split_defined(target: &str) -> Option<(String, String)> {
    let (sheet, r) = target.split_once('!')?;
    let sheet = sheet.trim();
    let sheet = if let Some(q) = sheet.strip_prefix('\'') {
        q.strip_suffix('\'')?.replace("''", "'")
    } else {
        sheet.to_string()
    };
    let plain: String = r.chars().filter(|c| *c != '$').collect();
    // A1 か A1:B2 の形だけ。複数範囲(カンマ)や行・列全体は理解しない
    let ok = match plain.split_once(':') {
        Some((a, b)) => Pos::parse(a).is_some() && Pos::parse(b).is_some(),
        None => Pos::parse(&plain).is_some(),
    };
    ok.then_some((sheet, plain))
}

/// "A1" / "A1:B2" → "$A$1" / "$A$1:$B$2"
fn dollars(r: &str) -> String {
    let one = |s: &str| -> String {
        let split = s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len());
        let (c, n) = s.split_at(split);
        format!("${c}${n}")
    };
    match r.split_once(':') {
        Some((a, b)) => format!("{}:{}", one(a), one(b)),
        None => one(r),
    }
}

/// 原本の workbook.xml の definedNames を、こちらの塊に置き換える。
/// 原文の workbook.xml の <sheet> に state="hidden" を差し替える。
/// **知らない属性は残す** — 名前・sheetId・r:id はそのまま。
/// 原本の workbook.xml の計算方法(calcPr calcMode)をこちらのモデルに合わせる。
/// 他の属性(calcId 等)は据え置く。calcPr が無い原本に手動を書くときは
/// definedNames の後(スキーマの順)に差し込む
/// calcPr の1本(新規保存)。手動と反復のどちらも無ければ空
fn calc_pr_xml(book: &Book) -> String {
    let mut attrs = String::new();
    if book.calc_manual {
        attrs.push_str(r#" calcMode="manual""#);
    }
    if let Some((n, d)) = book.calc_iter {
        attrs.push_str(&format!(r#" iterate="1" iterateCount="{n}" iterateDelta="{d}""#));
    }
    if book.r1c1 {
        attrs.push_str(r#" refMode="R1C1""#);
    }
    if attrs.is_empty() { String::new() } else { format!("<calcPr{attrs}/>") }
}

/// calcPr の iterate 系3属性を差し替える(付ける/外す)。
/// calcPr の refMode 属性を差し替える(付ける/外す)。
fn patch_refmode(tag: &str, r1c1: bool) -> String {
    let mut t = tag.to_string();
    while let Some(a) = t.find(" refMode=\"") {
        let vstart = a + " refMode=\"".len();
        let Some(vend) = t[vstart..].find('"') else { break };
        t.replace_range(a..vstart + vend + 1, "");
    }
    if r1c1 {
        if let Some(stripped) = t.strip_suffix("/>") {
            t = format!("{stripped} refMode=\"R1C1\"/>");
        } else if let Some(stripped) = t.strip_suffix('>') {
            t = format!("{stripped} refMode=\"R1C1\">");
        }
    }
    t
}

fn patch_iterate(tag: &str, iter: Option<(u32, f64)>) -> String {
    let mut t = tag.to_string();
    for name in ["iterate", "iterateCount", "iterateDelta"] {
        while let Some(a) = t.find(&format!(" {name}=\"")) {
            let vstart = a + name.len() + 3;
            let Some(vend) = t[vstart..].find('"') else { break };
            t.replace_range(a..vstart + vend + 1, "");
        }
    }
    if let Some((n, d)) = iter {
        let ins = format!(r#" iterate="1" iterateCount="{n}" iterateDelta="{d}""#);
        if let Some(stripped) = t.strip_suffix("/>") {
            t = format!("{stripped}{ins}/>");
        } else if let Some(stripped) = t.strip_suffix('>') {
            t = format!("{stripped}{ins}>");
        }
    }
    t
}

fn patch_calc_pr(workbook: &str, manual: bool) -> String {
    let mode = if manual { "manual" } else { "auto" };
    if let Some(start) = workbook.find("<calcPr") {
        let Some(len) = workbook[start..].find('>') else { return workbook.into() };
        let tag = &workbook[start..start + len + 1];
        let new_tag = if let Some(a) = tag.find("calcMode=\"") {
            // 既にある calcMode の値だけ差し替える
            let vstart = a + "calcMode=\"".len();
            match tag[vstart..].find('"') {
                Some(vend) => format!("{}{}{}", &tag[..vstart], mode, &tag[vstart + vend..]),
                None => return workbook.into(),
            }
        } else if manual {
            tag.replacen("<calcPr", r#"<calcPr calcMode="manual""#, 1)
        } else {
            return workbook.into(); // calcMode 無し=自動。触らない
        };
        format!("{}{}{}", &workbook[..start], new_tag, &workbook[start + len + 1..])
    } else if manual {
        // calcPr が無い原本。definedNames の後(無ければ sheets の後)に差し込む
        let ins = r#"<calcPr calcMode="manual"/>"#;
        if let Some(p) = workbook.find("</definedNames>") {
            let at = p + "</definedNames>".len();
            format!("{}{}{}", &workbook[..at], ins, &workbook[at..])
        } else if let Some(p) = workbook.find("</sheets>") {
            let at = p + "</sheets>".len();
            format!("{}{}{}", &workbook[..at], ins, &workbook[at..])
        } else {
            workbook.into()
        }
    } else {
        workbook.into()
    }
}

/// workbook.xml の `<sheet>` から `r:id` を**並び順に**拾う。
/// 書き出しで「どの部品がどのシートの物か」を読みと同じ道理で解くのに使う
fn sheet_rids(xml: &str) -> Vec<Option<String>> {
    let mut r = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sheet" => {
                out.push(attr(&e, "id"));
            }
            _ => {}
        }
        buf.clear();
    }
    out
}

/// 原本のブックの rels を、書き出す番号へ向け直す。
///
/// 本体は `<sheet>` の並び順に `sheet1..N` と振り直して書くので、
/// **原本の的を持ち越すと `<sheet>` が別の部品を指す** — 消した跡や
/// 並べ替えで、rId の順と部品の番号は離れているため。
/// `rids` は原本の `<sheet>` の並び順の `r:id`
fn patch_book_rels(rels: &str, rids: &[Option<String>], n_sheets: usize) -> String {
    let mut inner = String::new();
    for (id, ty, target, ext) in parse_rels(rels) {
        let at = rids.iter().take(n_sheets).position(|r| r.as_deref() == Some(id.as_str()));
        let (ty, target) = match at {
            // 書き出す並びの番号へ。中身も worksheet として書くので型も揃える
            Some(k) => (format!("{RNS}/worksheet"), format!("worksheets/sheet{}.xml", k + 1)),
            // `<sheet>` から指されていないシートの項は落とす。
            // 書き出す部品に無いものを残すと Excel が「修復」に入る
            None if ty.ends_with("/worksheet") || ty.ends_with("/chartsheet") => continue,
            None => (ty, target),
        };
        inner.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}"{}/>"#,
            esc(&id),
            esc(&ty),
            esc(&target),
            if ext { r#" TargetMode="External""# } else { "" }
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         {inner}</Relationships>"
    )
}

fn patch_sheet_states(workbook: &str, book: &Book) -> String {
    let mut out = String::new();
    let mut rest = workbook;
    let mut i = 0usize;
    while let Some(p) = rest.find("<sheet ") {
        let Some(e) = rest[p..].find('>') else { break };
        let tag = &rest[p..p + e + 1];
        out.push_str(&rest[..p]);
        // 既存の state= を落として、必要なら付け直す
        let mut t = tag.to_string();
        while let Some(a) = t.find(" state=\"") {
            if let Some(b) = t[a + 8..].find('"') {
                t.replace_range(a..a + 8 + b + 1, "");
            } else {
                break;
            }
        }
        if book.sheets.get(i).map(|s| s.hidden).unwrap_or(false) {
            let cut = t.len() - if t.ends_with("/>") { 2 } else { 1 };
            t.insert_str(cut, " state=\"hidden\"");
        }
        out.push_str(&t);
        rest = &rest[p + e + 1..];
        i += 1;
    }
    out.push_str(rest);
    out
}

/// 読み取り専用のお願いを workbook.xml に織り込む(無ければ足し、
/// 外したら消す)。**鍵ではないので password は書かない** — 掛けた振りをしない
fn patch_read_only(workbook: &str, on: bool) -> String {
    let mut s = workbook.to_string();
    // 既存の workbookProtection は取り除いてから置き直す
    if let Some(i) = s.find("<workbookProtection") {
        if let Some(j) = s[i..].find("/>") {
            s.replace_range(i..i + j + 2, "");
        } else if let Some(j) = s[i..].find("</workbookProtection>") {
            s.replace_range(i..i + j + "</workbookProtection>".len(), "");
        }
    }
    if !on {
        return s;
    }
    // 位置は fileVersion/workbookPr の後・bookViews の前(スキーマの並び)。
    // 手近な目印として <sheets> の前に置く
    match s.find("<bookViews").or_else(|| s.find("<sheets")) {
        Some(i) => {
            s.insert_str(i, r#"<workbookProtection readOnlyRecommended="1"/>"#);
            s
        }
        None => s,
    }
}

fn patch_defined_names(workbook: &str, block: &str) -> String {
    let mut s = workbook.to_string();
    if let Some(i) = s.find("<definedNames>") {
        if let Some(j) = s[i..].find("</definedNames>") {
            s.replace_range(i..i + j + "</definedNames>".len(), "");
        }
    } else if let Some(i) = s.find("<definedNames/>") {
        s.replace_range(i..i + "<definedNames/>".len(), "");
    }
    if block.is_empty() {
        return s;
    }
    // 位置は sheets の直後(スキーマの並び)
    match s.find("</sheets>") {
        Some(i) => {
            let at = i + "</sheets>".len();
            s.insert_str(at, block);
            s
        }
        None => s,
    }
}

/// A1 を絶対参照($A$1)にする。Print_Area は絶対参照で書くのが通り相場。
fn abs_a1(p: Pos) -> String {
    let a1 = p.a1();
    let split = a1.find(|c: char| c.is_ascii_digit()).unwrap_or(0);
    format!("${}${}", &a1[..split], &a1[split..])
}

/// 全シートの名前の定義 + 印刷範囲 + 理解しなかった原文を definedNames の塊にする。
fn defined_names_xml(book: &Book) -> String {
    let mut inner = String::new();
    for raw in &book.names_raw {
        inner.push_str(raw);
    }
    // タイトル行(モデルが正)
    for (i, sh) in book.sheets.iter().enumerate() {
        if let Some((a, b)) = sh.print_title_rows {
            inner.push_str(&format!(
                "<definedName name=\"_xlnm.Print_Titles\" localSheetId=\"{i}\">{}</definedName>",
                esc(&format!("'{}'!${}:${}", sh.name.replace('\'', "''"), a + 1, b + 1))
            ));
        }
    }
    // 印刷範囲(モデルが正)。シート名は常に引用符で包む(空白・記号に安全)
    for (i, sh) in book.sheets.iter().enumerate() {
        if sh.print_areas.is_empty() {
            continue;
        }
        let refs: Vec<String> = sh
            .print_areas
            .iter()
            .map(|(a, b)| {
                format!("'{}'!{}:{}", sh.name.replace('\'', "''"), abs_a1(*a), abs_a1(*b))
            })
            .collect();
        inner.push_str(&format!(
            "<definedName name=\"_xlnm.Print_Area\" localSheetId=\"{i}\">{}</definedName>",
            esc(&refs.join(","))
        ));
    }
    // 人が付けた名前。**同じ名前が2枚以上のシートにあるときだけ
    // localSheetId を付ける。**
    //
    // こちらのモデルは名前を「指す先のシート」に持たせていて、Excel の
    // 「適用範囲」(ブック全体 / このシートだけ)は持っていない。全部に
    // localSheetId を付けるとブック全体の名前がシート限定に落ちて、他の
    // シートの式が壊れる。逆に一つも付けないと、同じ名前が2枚にあるとき
    // **ブック全体の名前が2つ**になって開けないファイルになる。
    // 重なったときだけシート限定にするのが、どちらも壊さない線。
    let mut seen: std::collections::HashMap<&str, usize> = Default::default();
    for s in &book.sheets {
        for (n, _) in &s.names {
            *seen.entry(n.as_str()).or_insert(0) += 1;
        }
    }
    for (i, s) in book.sheets.iter().enumerate() {
        for (n, r) in &s.names {
            let scoped = seen.get(n.as_str()).copied().unwrap_or(0) > 1;
            let sid = if scoped { format!(" localSheetId=\"{i}\"") } else { String::new() };
            inner.push_str(&format!(
                "<definedName name=\"{}\"{}>'{}'!{}</definedName>",
                esc(n),
                sid,
                s.name.replace('\'', "''"),
                dollars(r)
            ));
        }
    }
    if inner.is_empty() {
        String::new()
    } else {
        format!("<definedNames>{inner}</definedNames>")
    }
}

/// 自己閉じ要素の属性を差し替える(無ければ足す)。他の属性は触らない。
fn set_attr(el: &str, name: &str, value: &str) -> String {
    let pat = format!("{name}=\"");
    if let Some(i) = el.find(&pat) {
        let vstart = i + pat.len();
        if let Some(vend) = el[vstart..].find('"') {
            let mut out = String::with_capacity(el.len() + value.len());
            out.push_str(&el[..vstart]);
            out.push_str(value);
            out.push_str(&el[vstart + vend..]);
            return out;
        }
    }
    el.replacen("/>", &format!(" {name}=\"{value}\"/>"), 1)
}

/// 印刷まわりの塊(pageMargins → pageSetup → drawing の順 = schema の順)。
/// 原文があれば**属性だけ差し替える**(拡大縮小など知らない属性を残す)。
/// 無ければモデルの値から最小の要素を作る。
fn print_extra_xml(orig: &str, sh: &Sheet) -> String {
    let take = |pat: &str| -> Option<String> {
        let i = orig.find(pat)?;
        let j = orig[i..].find("/>")? + i + 2;
        Some(orig[i..j].to_string())
    };
    let inch = |mm: f32| format!("{:.5}", mm / 25.4);
    // printOptions(枠線・見出しの印刷)。モデルの真偽を原文へ織り込む
    let popts = {
        let el = take("<printOptions").unwrap_or_else(|| "<printOptions/>".to_string());
        let el = set_attr(&el, "gridLines", if sh.print_gridlines { "1" } else { "0" });
        let el = set_attr(&el, "headings", if sh.print_headings { "1" } else { "0" });
        if !sh.print_gridlines && !sh.print_headings && !orig.contains("<printOptions") {
            None
        } else {
            Some(el)
        }
    };
    let margins = match (sh.margins_mm, take("<pageMargins")) {
        (Some((l, r, t, b)), Some(el)) => {
            let el = set_attr(&el, "left", &inch(l));
            let el = set_attr(&el, "right", &inch(r));
            let el = set_attr(&el, "top", &inch(t));
            Some(set_attr(&el, "bottom", &inch(b)))
        }
        (Some((l, r, t, b)), None) => Some(format!(
            "<pageMargins left=\"{}\" right=\"{}\" top=\"{}\" bottom=\"{}\" header=\"0.3\" footer=\"0.3\"/>",
            inch(l), inch(r), inch(t), inch(b)
        )),
        (None, el) => el,
    };
    let setup = {
        let orig_el = take("<pageSetup");
        if !sh.landscape && sh.paper_size.is_none() && sh.print_scale.is_none()
            && sh.fit_to_w.is_none() && sh.fit_to_h.is_none()
            && orig_el.is_none()
        {
            None
        } else {
            let el = orig_el.unwrap_or_else(|| "<pageSetup/>".to_string());
            let el = set_attr(
                &el,
                "orientation",
                if sh.landscape { "landscape" } else { "portrait" },
            );
            let el = match sh.paper_size {
                Some(c) => set_attr(&el, "paperSize", &c.to_string()),
                None => el,
            };
            let el = match sh.print_scale {
                Some(sc) => set_attr(&el, "scale", &sc.to_string()),
                None => el,
            };
            // 紙 N 枚に収める。**片方だけ指定でも両方書く**(0 = 合わせない)
            // — 書かないと読み手の既定(1)が効いて意図しない縮小になる
            let el = if sh.fit_to_w.is_some() || sh.fit_to_h.is_some() {
                let el = set_attr(&el, "fitToWidth", &sh.fit_to_w.unwrap_or(0).to_string());
                set_attr(&el, "fitToHeight", &sh.fit_to_h.unwrap_or(0).to_string())
            } else {
                el
            };
            Some(el)
        }
    };
    let mut out = String::new();
    if let Some(po) = popts {
        out.push_str(&po);
    }
    if let Some(m) = margins {
        out.push_str(&m);
    }
    if let Some(su) = setup {
        out.push_str(&su);
    }
    // 印刷のヘッダー/フッター(schema では pageSetup の後・rowBreaks の前)
    if sh.header.is_some() || sh.footer.is_some() {
        let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;");
        out.push_str("<headerFooter>");
        if let Some(h) = &sh.header {
            out.push_str(&format!("<oddHeader>{}</oddHeader>", esc(h)));
        }
        if let Some(f) = &sh.footer {
            out.push_str(&format!("<oddFooter>{}</oddFooter>", esc(f)));
        }
        out.push_str("</headerFooter>");
    }
    // 改ページ(モデルが正。原文の rowBreaks は読みでモデルへ入っている)
    if !sh.row_breaks.is_empty() {
        let mut sorted = sh.row_breaks.clone();
        sorted.sort_unstable();
        sorted.dedup();
        out.push_str(&format!(
            r#"<rowBreaks count="{}" manualBreakCount="{}">"#,
            sorted.len(),
            sorted.len()
        ));
        for r in sorted {
            out.push_str(&format!(r#"<brk id="{r}" max="16383" man="1"/>"#));
        }
        out.push_str("</rowBreaks>");
    }
    // 縦の改ページ(schema では rowBreaks の後)
    if !sh.col_breaks.is_empty() {
        let mut sorted = sh.col_breaks.clone();
        sorted.sort_unstable();
        sorted.dedup();
        out.push_str(&format!(
            r#"<colBreaks count="{}" manualBreakCount="{}">"#,
            sorted.len(),
            sorted.len()
        ));
        for c in sorted {
            out.push_str(&format!(r#"<brk id="{c}" max="1048575" man="1"/>"#));
        }
        out.push_str("</colBreaks>");
    }
    if let Some(d) = take("<drawing") {
        out.push_str(&d);
    }
    out
}

const CORE_REL: &str = r#"<Relationship Id="rIdCore" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>"#;

const CORE_XML_EMPTY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"></cp:coreProperties>";

/// core.xml の1つのタグを差し替える(無ければ足す)。原文の他の欄は残す。
fn set_core_tag(s: &str, tag: &str, val: &str) -> String {
    let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let repl = if val.is_empty() {
        format!("<{tag}/>")
    } else {
        format!("<{tag}>{}</{tag}>", esc(val))
    };
    if let Some(i) = s.find(&open) {
        let rest = &s[i..];
        let gt = rest.find('>').unwrap_or(0);
        if gt > 0 && rest.as_bytes().get(gt - 1) == Some(&b'/') {
            // <tag/> 自己完結
            return format!("{}{}{}", &s[..i], repl, &rest[gt + 1..]);
        }
        if let Some(c) = rest.find(&close) {
            return format!("{}{}{}", &s[..i], repl, &rest[c + close.len()..]);
        }
        s.to_string()
    } else if let Some(i) = s.rfind("</cp:coreProperties>") {
        format!("{}{}{}", &s[..i], repl, &s[i..])
    } else {
        s.to_string()
    }
}

/// docProps/core.xml をブックの情報で差し替える。
fn patch_core_props(orig: &str, p: &crate::model::BookProps) -> String {
    let mut s = orig.to_string();
    for (tag, v) in [
        ("dc:creator", &p.creator),
        ("dc:title", &p.title),
        ("dc:subject", &p.subject),
        ("cp:keywords", &p.keywords),
        ("dc:description", &p.description),
    ] {
        s = set_core_tag(&s, tag, v);
    }
    s
}

/// **書いた xlsx を型紙(XLTX)に仕立て直す。**
///
/// 中身は xlsx と同じで、違うのは `[Content_Types].xml` の宣言ひとつだけ
/// (`...spreadsheetml.sheet.main+xml` → `...template.main+xml`)。
/// 開くと「この型紙から新しいブック」になる。
///
/// 書き手を二重に持たないよう、**出来上がった zip を作り直す**形にした。
pub fn to_template(bytes: &[u8]) -> Result<Vec<u8>, String> {
    const FROM: &str = "spreadsheetml.sheet.main+xml";
    const TO: &str = "spreadsheetml.template.main+xml";
    let mut zin = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut zout = zip::ZipWriter::new(&mut out);
        let opts: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut swapped = false;
        for i in 0..zin.len() {
            let mut f = zin.by_index(i).map_err(|e| e.to_string())?;
            let name = f.name().to_string();
            let mut buf = Vec::new();
            std::io::copy(&mut f, &mut buf).map_err(|e| e.to_string())?;
            if name == "[Content_Types].xml" {
                let t = String::from_utf8_lossy(&buf).to_string();
                if t.contains(FROM) {
                    swapped = true;
                }
                buf = t.replace(FROM, TO).into_bytes();
            }
            zout.start_file(name, opts).map_err(|e| e.to_string())?;
            zout.write_all(&buf).map_err(|e| e.to_string())?;
        }
        if !swapped {
            // **黙って xlsx のまま出さない。** 宣言を書き換えられなければ
            // 型紙ではないので、そう言って止める
            return Err("型紙の宣言が見つかりません(xlsx の作りが変わった?)".into());
        }
        zout.finish().map_err(|e| e.to_string())?;
    }
    Ok(out.into_inner())
}

pub fn write<W: Write + Seek>(book: &Book, dst: W) -> Result<(), String> {
    write_with(book, None::<std::io::Cursor<Vec<u8>>>, dst)
}

/// 保存する。`original` に開いた元のファイルを渡すと、こちらが作り直す部品
/// (シート・共有文字列・書式)以外 — **図形・テーマ・印刷設定・文書情報** —
/// を原本から持ち越す。渡さないと消える。
///
/// calcChain.xml だけは意図して捨てる(位置が古いままだと Excel が
/// 誤った計算順で開くことがある。無ければ Excel が作り直す)。
pub fn write_with<R: Read + Seek, W: Write + Seek>(
    book: &Book,
    original: Option<R>,
    dst: W,
) -> Result<(), String> {
    // 原本の部品と、各シートの引き継ぎ要素(印刷まわり・図形)を先に読む
    let mut carried: Vec<(String, Vec<u8>)> = Vec::new();
    let mut sheet_extras: Vec<String> = Vec::new();
    // [Content_Types] とシートの rels は「そのまま」ではなく、
    // リンク・コメントのぶんを織り込んで作り直す
    let mut orig_ct: Option<String> = None;
    let mut orig_sheet_rels: Vec<Option<String>> = Vec::new();
    let mut orig_styles: Option<String> = None;
    if let Some(src) = original {
        if let Ok(mut z) = zip::ZipArchive::new(src) {
            // **どの部品がどのシートの物かは r:id で解く**(読みと同じ道理)。
            // 部品の番号は `<sheet>` の並びとは一致しない。ここを部品の番号で
            // 数えていたので、引き継ぐ印刷設定と図形が別のシートへ付いていた
            let mut orig_wb = String::new();
            if let Ok(mut f) = z.by_name("xl/workbook.xml") {
                let _ = f.read_to_string(&mut orig_wb);
            }
            let mut orig_wb_rels = String::new();
            if let Ok(mut f) = z.by_name("xl/_rels/workbook.xml.rels") {
                let _ = f.read_to_string(&mut orig_wb_rels);
            }
            let orig_rids = sheet_rids(&orig_wb);
            let orig_parts: Vec<String> = {
                let entries: Vec<String> = (0..z.len())
                    .filter_map(|i| z.by_index(i).ok().map(|f| f.name().to_string()))
                    .collect();
                let mut parts: Vec<String> = entries
                    .iter()
                    .filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
                    .cloned()
                    .collect();
                parts.sort_by(|a, b| {
                    sheet_part_no(a).cmp(&sheet_part_no(b)).then_with(|| a.cmp(b))
                });
                let rels: Vec<(String, String)> = parse_rels(&orig_wb_rels)
                    .into_iter()
                    .filter(|(id, _, _, ext)| !id.is_empty() && !ext)
                    .map(|(id, _, target, _)| (id, resolve_book_target(&target)))
                    .collect();
                sheet_parts(&orig_rids, &rels, &entries, &parts)
            };
            // 部品名 → そのシートの並び順
            let book_at = |part: &str| orig_parts.iter().position(|p| p == part);
            for i in 0..z.len() {
                let Ok(mut f) = z.by_index(i) else { continue };
                let name = f.name().to_string();
                let regenerated = name.starts_with("xl/worksheets/sheet")
                    && name.ends_with(".xml")
                    || name == "xl/sharedStrings.xml"
                    || name == "xl/styles.xml"
                    || name == "xl/calcChain.xml"
                    // コメントの部品はこちらが作り直す
                    || name.starts_with("xl/comments")
                    || name.starts_with("xl/drawings/vmlDrawing");
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_err() {
                    continue;
                }
                if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
                    // シート本体は作り直すが、印刷まわりと図形の参照は引き継ぐ
                    let s = String::from_utf8_lossy(&buf);
                    let mut extra = String::new();
                    for pat in ["<printOptions", "<pageMargins", "<pageSetup", "<drawing"] {
                        if let Some(i) = s.find(pat) {
                            if let Some(j) = s[i..].find("/>") {
                                extra.push_str(&s[i..i + j + 2]);
                            }
                        }
                    }
                    // 部品の番号ではなく**このシートの並び順**へ入れる
                    if let Some(k) = book_at(&name) {
                        while sheet_extras.len() <= k {
                            sheet_extras.push(String::new());
                        }
                        sheet_extras[k] = extra;
                    }
                }
                if name == "xl/workbook.xml" {
                    // 名前の定義はこちらの帳簿(モデル+原文持ち越し)が正。
                    // 原本の definedNames を置き換えて持ち越す
                    let s = String::from_utf8_lossy(&buf).to_string();
                    let patched = patch_defined_names(&s, &defined_names_xml(book));
                    // 隠しシートの state はこちらのモデルが正(原文へ属性差し替え)
                    let patched = patch_sheet_states(&patched, book);
                    // 計算方法もこちらが正(F9 で手動にしたら残す)
                    let patched = patch_calc_pr(&patched, book.calc_manual);
                    // 読み取り専用のお願い(鍵ではない)
                    let patched = patch_read_only(&patched, book.read_only_rec);
                    // 反復計算も原本の calcPr に織り込む(無ければ足し、切っていれば外す)
                    let patched = if let Some(start) = patched.find("<calcPr") {
                        match patched[start..].find('>') {
                            Some(len) => {
                                let tag = &patched[start..start + len + 1];
                                format!(
                                    "{}{}{}",
                                    &patched[..start],
                                    patch_refmode(&patch_iterate(tag, book.calc_iter), book.r1c1),
                                    &patched[start + len + 1..]
                                )
                            }
                            None => patched,
                        }
                    } else if book.calc_iter.is_some() || book.r1c1 {
                        // calcPr が無い原本に反復だけ足す(manual と同じ差し込み場所)
                        let ins = calc_pr_xml(book);
                        if let Some(p) = patched.find("</definedNames>") {
                            let at = p + "</definedNames>".len();
                            format!("{}{}{}", &patched[..at], ins, &patched[at..])
                        } else if let Some(p) = patched.find("</sheets>") {
                            let at = p + "</sheets>".len();
                            format!("{}{}{}", &patched[..at], ins, &patched[at..])
                        } else {
                            patched
                        }
                    } else {
                        patched
                    };
                    carried.push((name, patched.into_bytes()));
                    continue;
                }
                if name == "xl/theme/theme1.xml" {
                    continue; // テーマの色はモデルが正(配色の変更が効く)
                }
                if name == "xl/styles.xml" {
                    // 原本の書式表は**据え置き合成の土台**として持つ
                    // (作り直すと、読みで拾えない書式が消える — 発注者 2026-08-06)
                    orig_styles = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                if name.starts_with("xl/tables/") {
                    continue; // 表オブジェクトはモデルから作り直す
                }
                if name == "docProps/core.xml" {
                    // ブックの情報はこちらのモデルが正。原文の他の欄は残す
                    let s = String::from_utf8_lossy(&buf).to_string();
                    carried.push((name, patch_core_props(&s, &book.props).into_bytes()));
                    continue;
                }
                if name == "[Content_Types].xml" {
                    orig_ct = Some(String::from_utf8_lossy(&buf).to_string());
                    continue;
                }
                if let Some(base) = name
                    .strip_prefix("xl/worksheets/_rels/")
                    .and_then(|r| r.strip_suffix(".rels"))
                {
                    // この rels の持ち主の部品を、並び順へ直して置く
                    if let Some(k) = book_at(&format!("xl/worksheets/{base}")) {
                        while orig_sheet_rels.len() <= k {
                            orig_sheet_rels.push(None);
                        }
                        orig_sheet_rels[k] = Some(String::from_utf8_lossy(&buf).to_string());
                    }
                    continue;
                }
                if name == "xl/_rels/workbook.xml.rels" {
                    // 本体は並び順の番号で書き出すので、的もそこへ向け直す
                    let s = String::from_utf8_lossy(&buf).to_string();
                    let fixed = patch_book_rels(&s, &orig_rids, book.sheets.len());
                    carried.push((name, fixed.into_bytes()));
                    continue;
                }
                if !regenerated {
                    carried.push((name, buf));
                }
            }
        }
    }

    let mut zip = zip::ZipWriter::new(dst);
    let o: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 共有文字列を集める(ふりがなも添える — 落とすと日本語の宝が消える)。
    // 同じ字で違う読みの2セルは、先に出た読みで代表(表は字で引くため)
    let mut shared: Vec<String> = Vec::new();
    let mut shared_ruby: Vec<Option<String>> = Vec::new();
    let mut idx = std::collections::HashMap::new();
    for sh in &book.sheets {
        for (p, c) in &sh.cells {
            if let Value::Text(t) = &c.value {
                let ruby = sh.phonetics.get(p);
                match idx.get(t) {
                    None => {
                        idx.insert(t.clone(), shared.len());
                        shared.push(t.clone());
                        shared_ruby.push(ruby.cloned());
                    }
                    Some(&i) => {
                        if shared_ruby[i].is_none() {
                            shared_ruby[i] = ruby.cloned();
                        }
                    }
                }
            }
        }
    }

    // 原本の書式表を読み戻す(据え置き合成の照合用)。
    // 読みと同じ関数で解くので、読みで拾えない書式も**同じように**落ち、
    // 「触っていないセル」は必ず一致して原本の索引のまま書き戻る
    let orig_fmts: Option<Vec<crate::model::CellFormat>> =
        orig_styles.as_ref().map(|xml| crate::styles::parse(xml, &book.theme));
    // このセルは原本の書式のままか(なら索引ごと据え置く)
    let kept_style = |sh: &Sheet, p: &Pos, fmt: &crate::model::CellFormat| -> Option<u32> {
        let fmts = orig_fmts.as_ref()?;
        let i = *sh.style_of.get(p)?;
        (fmts.get(i as usize)? == fmt).then_some(i)
    };
    // 使われている書式を集めて表にする(据え置きのセルは除く)。
    // 索引を <c s="…"> に配る
    let used: Vec<crate::model::CellFormat> = {
        let mut v = Vec::new();
        for sh in &book.sheets {
            for (p, c) in &sh.cells {
                if kept_style(sh, p, &c.fmt).is_some() {
                    continue;
                }
                if !c.fmt.is_plain() && !v.contains(&c.fmt) {
                    v.push(c.fmt.clone());
                }
            }
        }
        v
    };
    let (styles_xml, style_idx) = match orig_styles
        .as_ref()
        .and_then(|xml| crate::styles::append_to(xml, &used))
    {
        Some(r) => r,
        // 原本が無い(新規)か、節の見つからない styles.xml なら作り直し
        None => crate::styles::build(&used),
    };
    // 条件付き書式の見た目(dxfs)。全シートの規則から集めて番号を振る
    let dxf_list: Vec<(Option<String>, Option<String>)> = {
        let mut v = Vec::new();
        for sh in &book.sheets {
            for r in &sh.cond {
                let pair = (r.color.clone(), r.fill.clone());
                if !v.contains(&pair) {
                    v.push(pair);
                }
            }
        }
        v
    };
    let styles_xml = if dxf_list.is_empty() {
        styles_xml
    } else {
        let mut dx = format!("<dxfs count=\"{}\">", dxf_list.len());
        for (color, fill) in &dxf_list {
            dx.push_str("<dxf>");
            if let Some(c) = color {
                dx.push_str(&format!("<font><color rgb=\"FF{c}\"/></font>"));
            }
            if let Some(f) = fill {
                dx.push_str(&format!(
                    "<fill><patternFill><bgColor rgb=\"FF{f}\"/></patternFill></fill>"
                ));
            }
            dx.push_str("</dxf>");
        }
        dx.push_str("</dxfs>");
        let mut s = styles_xml;
        // 原本に dxfs の節があれば置き換える(二重の節は不正)。無ければ挿す
        if let Some(i) = s.find("<dxfs") {
            let end = match s[i..].find("</dxfs>") {
                Some(j) => i + j + "</dxfs>".len(),
                // <dxfs count="0"/> の自己完結形
                None => i + s[i..].find("/>").map(|j| j + 2).unwrap_or(0),
            };
            if end > i {
                s.replace_range(i..end, &dx);
            }
        } else if let Some(p) = s.rfind("</styleSheet>") {
            s.insert_str(p, &dx);
        }
        s
    };

    let overrides: String = (1..=book.sheets.len())
        .map(|i| format!(r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#))
        .collect();
    // このアプリで挿した画像(グラフ)の部品。原本に drawing のあるシートは
    // **その部品の中へアンカーと rels を継ぎ足す**(drawing は1シート1部品の決まり)。
    // 無いシートは drawingC{N}.xml を新しく作る
    let mut media_out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut fresh_parts: Vec<(String, String)> = Vec::new();
    // 連番は原本にある imageC の続きから(前の保存で足した分と衝突しない)
    let mut media_n = 0usize;
    for (name, _) in &carried {
        if let Some(rest) = name.strip_prefix("xl/media/imageC") {
            if let Some(num) = rest.split('.').next().and_then(|v| v.parse::<usize>().ok()) {
                media_n = media_n.max(num);
            }
        }
    }
    for (i, sh) in book.sheets.iter().enumerate() {
        if sh.images_new.is_empty() && sh.shapes_new.is_empty() {
            continue;
        }
        let mut anchors = String::new();
        let mut rels_add = String::new();
        for (k, spn) in sh.shapes_new.iter().enumerate() {
            anchors.push_str(&shape_anchor_xml(spn, (i as u32) * 100 + k as u32 + 50));
        }
        for (k, im) in sh.images_new.iter().enumerate() {
            media_n += 1;
            let ext = if im.data.starts_with(&[0xFF, 0xD8]) {
                "jpeg"
            } else if im.data.starts_with(b"GIF8") {
                "gif"
            } else if im.data.starts_with(b"BM") {
                "bmp"
            } else {
                "png"
            };
            let _ = k;
            let rid = format!("rIdC{media_n}");
            media_out.push((format!("xl/media/imageC{media_n}.{ext}"), im.data.clone()));
            rels_add.push_str(&format!(
                r#"<Relationship Id="{rid}" Type="{RNS}/image" Target="../media/imageC{media_n}.{ext}"/>"#
            ));
            anchors.push_str(&image_anchor_xml(im, &rid, (i as u32) * 100 + k as u32 + 2));
        }
        let orig_target = orig_sheet_rels.get(i).cloned().flatten().and_then(|onr| {
            parse_rels(&onr)
                .into_iter()
                .find(|(_, ty, _, _)| ty.ends_with("/drawing"))
                .map(|(_, _, t, _)| resolve_target(&t))
        });
        match orig_target {
            Some(dpath) => {
                for (name, buf) in carried.iter_mut() {
                    if *name == dpath {
                        let mut xml = String::from_utf8_lossy(buf).to_string();
                        if let Some(p) = xml.rfind("</xdr:wsDr>") {
                            xml.insert_str(p, &anchors);
                            *buf = xml.into_bytes();
                        }
                    }
                }
                let drels = {
                    let (dir, base) = dpath.rsplit_once('/').unwrap_or(("xl/drawings", &dpath));
                    format!("{dir}/_rels/{base}.rels")
                };
                let mut found = false;
                for (name, buf) in carried.iter_mut() {
                    if *name == drels {
                        let mut xml = String::from_utf8_lossy(buf).to_string();
                        if let Some(p) = xml.rfind("</Relationships>") {
                            xml.insert_str(p, &rels_add);
                            *buf = xml.into_bytes();
                        }
                        found = true;
                    }
                }
                if !found {
                    fresh_parts.push((drels, format!(
                        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels_add}</Relationships>"
                    )));
                }
            }
            None => {
                fresh_parts.push((
                    format!("xl/drawings/drawingC{}.xml", i + 1),
                    format!(
                        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">{anchors}</xdr:wsDr>"
                    ),
                ));
                fresh_parts.push((
                    format!("xl/drawings/_rels/drawingC{}.xml.rels", i + 1),
                    format!(
                        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels_add}</Relationships>"
                    ),
                ));
            }
        }
    }

    // ブックに載せた Python・ピボット・スピルの記録はモデルが正(古い部品は写さない)
    carried.retain(|(name, _)| {
        name != "xl/joPython.xml" && name != "xl/joPivot.xml" && name != "xl/joSpill.xml"
        && name != "xl/joChanges.xml"
    });
    let carry = !carried.is_empty();
    // ブックの情報。原本に core.xml が無い・新規ブックでも、書いた情報は残す
    let pr = &book.props;
    let props_any = !(pr.creator.is_empty()
        && pr.title.is_empty()
        && pr.subject.is_empty()
        && pr.keywords.is_empty()
        && pr.description.is_empty());
    let had_core = carried.iter().any(|(n, _)| n == "docProps/core.xml");
    let core_fresh = !had_core && props_any;
    if core_fresh {
        carried.push((
            "docProps/core.xml".to_string(),
            patch_core_props(CORE_XML_EMPTY, pr).into_bytes(),
        ));
        // 持ち越した .rels に core の関係が無ければ足す
        if let Some((_, buf)) = carried.iter_mut().find(|(n, _)| n == "_rels/.rels") {
            let s = String::from_utf8_lossy(buf).to_string();
            if !s.contains("core-properties") {
                if let Some(i) = s.rfind("</Relationships>") {
                    let mut s2 = s.clone();
                    s2.insert_str(i, CORE_REL);
                    *buf = s2.into_bytes();
                }
            }
        }
    }
    for (name, buf) in &carried {
        zip.start_file(name.as_str(), o).map_err(|e| e.to_string())?;
        zip.write_all(buf).map_err(|e| e.to_string())?;
    }
    for (name, buf) in &media_out {
        zip.start_file(name.as_str(), o).map_err(|e| e.to_string())?;
        zip.write_all(buf).map_err(|e| e.to_string())?;
    }
    let mut put = |name: &str, data: &str| -> Result<(), String> {
        zip.start_file(name, o).map_err(|e| e.to_string())?;
        zip.write_all(data.as_bytes()).map_err(|e| e.to_string())
    };
    // [Content_Types]。コメントの部品を持つときは、その宣言も要る
    {
        let mut ct = match &orig_ct {
            Some(s) => s.clone(),
            None => CT.replace("__SHEETS__", &overrides),
        };
        // 表オブジェクトの宣言は作り直す(減ったときに空の宣言を残さない)
        while let Some(i) = ct.find(r#"<Override PartName="/xl/tables/"#) {
            if let Some(j) = ct[i..].find("/>") {
                ct.replace_range(i..i + j + 2, "");
            } else {
                break;
            }
        }
        // シート本体の宣言も作り直す。**原本の部品の番号は飛んでいることが
        // ある**(消した跡)が、こちらは並び順に sheet1..N と書き出すので、
        // 原本の宣言をそのまま持ち越すと有る部品が漏れ、無い部品を宣言する
        if orig_ct.is_some() {
            for pat in [
                r#"<Override PartName="/xl/worksheets/"#,
                r#"<Override PartName="/xl/chartsheets/"#,
            ] {
                while let Some(i) = ct.find(pat) {
                    match ct[i..].find("/>") {
                        Some(j) => ct.replace_range(i..i + j + 2, ""),
                        None => break,
                    }
                }
            }
            // 宣言の並び順は問われないので最後へ足す
            if let Some(p) = ct.rfind("</Types>") {
                ct.insert_str(p, &overrides);
            }
        }
        let n_tables: usize = book.sheets.iter().map(|s| s.tables.len()).sum();
        let has_comments = book.sheets.iter().any(|s| !s.comments.is_empty());
        let mut add = String::new();
        for n in 1..=n_tables {
            add.push_str(&format!(
                r#"<Override PartName="/xl/tables/table{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>"#
            ));
        }
        if has_comments && !ct.contains("Extension=\"vml\"") {
            add.push_str(r#"<Default Extension="vml" ContentType="application/vnd.openxmlformats-officedocument.vmlDrawing"/>"#);
        }
        for (i, sh) in book.sheets.iter().enumerate() {
            let part = format!("/xl/comments{}.xml", i + 1);
            if !sh.comments.is_empty() && !ct.contains(&part) {
                add.push_str(&format!(r#"<Override PartName="{part}" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml"/>"#));
            }
        }
        // 挿した画像の部品の宣言(絵の拡張子と、新しく作った drawing)
        if media_out.iter().any(|(n, _)| n.ends_with(".png")) && !ct.contains("Extension=\"png\"") {
            add.push_str(r#"<Default Extension="png" ContentType="image/png"/>"#);
        }
        if media_out.iter().any(|(n, _)| n.ends_with(".jpeg")) && !ct.contains("Extension=\"jpeg\"") {
            add.push_str(r#"<Default Extension="jpeg" ContentType="image/jpeg"/>"#);
        }
        if media_out.iter().any(|(n, _)| n.ends_with(".gif")) && !ct.contains("Extension=\"gif\"") {
            add.push_str(r#"<Default Extension="gif" ContentType="image/gif"/>"#);
        }
        if media_out.iter().any(|(n, _)| n.ends_with(".bmp")) && !ct.contains("Extension=\"bmp\"") {
            add.push_str(r#"<Default Extension="bmp" ContentType="image/bmp"/>"#);
        }
        for (name, _) in &fresh_parts {
            if name.starts_with("xl/drawings/drawingC") && name.ends_with(".xml") {
                add.push_str(&format!(
                    r#"<Override PartName="/{name}" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/>"#
                ));
            }
        }
        if !book.theme.is_empty() && !ct.contains("/xl/theme/theme1.xml") {
            add.push_str(r#"<Override PartName="/xl/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>"#);
        }
        if core_fresh && !ct.contains("core-properties") {
            add.push_str(r#"<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>"#);
        }
        if !add.is_empty() {
            if let Some(p) = ct.rfind("</Types>") {
                ct.insert_str(p, &add);
            }
        }
        put("[Content_Types].xml", &ct)?;
    }
    for (name, xml) in &fresh_parts {
        put(name, xml)?;
    }
    // **ブックには Python を一切書かない**(発注者確定 2026-08-09:
    // データとプログラムを1つのファイルにしない — xlsm の逆)。
    // 関数(UDF)も手続きも plugins の .py にある。古いブックから読んだ
    // コードは保存で消える(開くときの報告でそう言う。取り出しは @export)
    // 変更履歴(独自部品 xl/joChanges.xml)。Excel は読まない — 正直な劣化
    if !book.changes.is_empty() {
        let mut cx = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<joChanges>",
        );
        for c in &book.changes {
            cx.push_str(&format!(
                "<c who=\"{}\" when=\"{}\" sheet=\"{}\" at=\"{}\" before=\"{}\" after=\"{}\"/>",
                esc(&c.who), esc(&c.when), esc(&c.sheet), c.at.a1(),
                esc(&c.before), esc(&c.after)
            ));
        }
        cx.push_str("</joChanges>");
        put("xl/joChanges.xml", &cx)?;
    }
    if !book.pivots.is_empty() {
        let mut px = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<joPivot>",
        );
        for d in &book.pivots {
            px.push_str(&format!(
                "<pivot sheet=\"{}\" src=\"{}:{}\" dest=\"{}\" h=\"{}\" w=\"{}\" value=\"{}\" agg=\"{}\" totals=\"{}\" subtotals=\"{}\" blank=\"{}\" compact=\"{}\" style=\"{}\" name=\"{}\">",
                esc(&d.sheet),
                d.src.0.a1(),
                d.src.1.a1(),
                d.dest.a1(),
                d.size.0,
                d.size.1,
                esc(&d.value),
                esc(&d.agg),
                d.totals as u8,
                d.subtotals as u8,
                d.blank_rows as u8,
                d.compact as u8,
                esc(&d.style),
                esc(&d.name),
            ));
            for r in &d.rows_sel {
                px.push_str(&format!("<r>{}</r>", esc(r)));
            }
            for c in &d.cols_sel {
                px.push_str(&format!("<c>{}</c>", esc(c)));
            }
            for (f, vs) in &d.hide {
                px.push_str(&format!("<f name=\"{}\">", esc(f)));
                for v in vs {
                    px.push_str(&format!("<v>{}</v>", esc(v)));
                }
                px.push_str("</f>");
            }
            // 値のフィルターとグループ化(第2版)
            if let Some((op, th)) = &d.vfilter {
                px.push_str(&format!("<vf op=\"{}\" v=\"{}\"/>", esc(op), th));
            }
            for (f, unit) in &d.group_by {
                px.push_str(&format!("<g name=\"{}\" unit=\"{}\"/>", esc(f), esc(unit)));
            }
            if !d.show_as.is_empty() {
                px.push_str(&format!("<sa v=\"{}\"/>", esc(&d.show_as)));
            }
            px.push_str("</pivot>");
        }
        px.push_str("</joPivot>");
        put("xl/joPivot.xml", &px)?;
    }
    if book.sheets.iter().any(|s| !s.spills.is_empty()) {
        let mut sx = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<joSpill>",
        );
        for s in &book.sheets {
            for (at, (h, w)) in &s.spills {
                sx.push_str(&format!(
                    "<s sheet=\"{}\" at=\"{}\" h=\"{h}\" w=\"{w}\"/>",
                    esc(&s.name),
                    at.a1()
                ));
            }
        }
        sx.push_str("</joSpill>");
        put("xl/joSpill.xml", &sx)?;
    }
    if !carry {
        if core_fresh {
            put(
                "_rels/.rels",
                &RELS.replace("</Relationships>", &format!("{CORE_REL}</Relationships>")),
            )?;
        } else {
            put("_rels/.rels", RELS)?;
        }
    }

    let sheets_xml: String = book.sheets.iter().enumerate()
        .map(|(i, s)| format!(r#"<sheet name="{}" sheetId="{}"{} r:id="rId{}"/>"#,
                              esc(&s.name), i + 1,
                              if s.hidden { r#" state="hidden""# } else { "" },
                              i + 1))
        .collect();
    if !carry {
    put("xl/workbook.xml", &format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="{NS}" xmlns:r="{RNS}">{}<sheets>{sheets_xml}</sheets>{}{}</workbook>"#,
        // 読み取り専用のお願い(スキーマでは sheets の前)
        if book.read_only_rec { r#"<workbookProtection readOnlyRecommended="1"/>"# } else { "" },
        defined_names_xml(book),
        // 手動計算をファイルに残す(自動は既定なので書かない)
        calc_pr_xml(book).as_str()))?;

    let wrels: String = (1..=book.sheets.len())
        .map(|i| format!(r#"<Relationship Id="rId{i}" Type="{RNS}/worksheet" Target="worksheets/sheet{i}.xml"/>"#))
        .collect();
    put("xl/_rels/workbook.xml.rels", &format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{wrels}<Relationship Id="rIdSS" Type="{RNS}/sharedStrings" Target="sharedStrings.xml"/><Relationship Id="rIdST" Type="{RNS}/styles" Target="styles.xml"/><Relationship Id="rIdTH" Type="{RNS}/theme" Target="theme/theme1.xml"/></Relationships>"#))?;
    }

    // テーマの色。読んだものをそのまま返し、配色を変えたときは新しい組を書く
    if !book.theme.is_empty() {
        put("xl/theme/theme1.xml", &crate::theme::to_xml(&book.theme))?;
    }
    put("xl/styles.xml", &styles_xml)?;

    let si: String = shared
        .iter()
        .zip(&shared_ruby)
        .map(|(s, ruby)| match ruby {
            Some(r) => format!(
                "<si><t xml:space=\"preserve\">{}</t>\
                 <rPh sb=\"0\" eb=\"{}\"><t>{}</t></rPh>\
                 <phoneticPr fontId=\"0\"/></si>",
                esc(s),
                s.chars().count(),
                esc(r)
            ),
            None => format!("<si><t xml:space=\"preserve\">{}</t></si>", esc(s)),
        })
        .collect();
    put("xl/sharedStrings.xml", &format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="{NS}" count="{}" uniqueCount="{}">{si}</sst>"#, shared.len(), shared.len()))?;

    for (i, sh) in book.sheets.iter().enumerate() {
        let mut w = Writer::new(Cursor::new(Vec::new()));
        let mut ws = BytesStart::new("worksheet");
        ws.push_attribute(("xmlns", NS));
        ws.push_attribute(("xmlns:r", RNS));
        w.write_event(Event::Start(ws)).unwrap();
        // 耳(タブ)の色。schema では worksheet の先頭(sheetPr)
        if let Some(c) = &sh.tab_color {
            w.write_event(Event::Start(BytesStart::new("sheetPr"))).unwrap();
            let mut tc = BytesStart::new("tabColor");
            tc.push_attribute(("rgb", c.as_str()));
            w.write_event(Event::Empty(tc)).unwrap();
            w.write_event(Event::End(BytesEnd::new("sheetPr"))).unwrap();
        }
        // 画面の見え方(schema では sheetPr の次)。右から左・固定枠・格子線・
        // 倍率がここに集まる。**rtl のときだけ書いていたので固定枠を置く場所が
        // なかった** — 常に書き、中身は持っているものだけを載せる
        {
            w.write_event(Event::Start(BytesStart::new("sheetViews"))).unwrap();
            let mut sv = BytesStart::new("sheetView");
            if sh.rtl {
                sv.push_attribute(("rightToLeft", "1"));
            }
            // 読んだときだけ返す(None は原文に無かった = Excel の既定)。
            // 既定を書き足さないので、触っていない帳票の差分が増えない
            let b10 = |v: bool| if v { "1" } else { "0" };
            if let Some(v) = sh.show_formulas {
                sv.push_attribute(("showFormulas", b10(v)));
            }
            if let Some(v) = sh.show_gridlines {
                sv.push_attribute(("showGridLines", b10(v)));
            }
            if let Some(z) = sh.zoom_scale {
                sv.push_attribute(("zoomScale", z.to_string().as_str()));
            }
            sv.push_attribute(("workbookViewId", "0"));
            match sh.freeze {
                None => w.write_event(Event::Empty(sv)).unwrap(),
                Some(f) => {
                    w.write_event(Event::Start(sv)).unwrap();
                    let mut p = BytesStart::new("pane");
                    // **xSplit が列、ySplit が行。** 0 の側は書かない
                    // (Excel は書かない。書くと「0列を固定」の余計な指定になる)
                    if f.frozen_columns > 0 {
                        p.push_attribute(("xSplit", f.frozen_columns.to_string().as_str()));
                    }
                    if f.frozen_rows > 0 {
                        p.push_attribute(("ySplit", f.frozen_rows.to_string().as_str()));
                    }
                    // 止めた枠のすぐ右下のセル = 繰る側の左上
                    let tl = Pos::new(f.frozen_rows, f.frozen_columns);
                    p.push_attribute(("topLeftCell", tl.a1().as_str()));
                    // 動く側の枠。行だけ止めれば下、列だけ止めれば右、両方なら右下
                    p.push_attribute((
                        "activePane",
                        match (f.frozen_rows > 0, f.frozen_columns > 0) {
                            (true, true) => "bottomRight",
                            (true, false) => "bottomLeft",
                            _ => "topRight",
                        },
                    ));
                    p.push_attribute(("state", "frozen"));
                    w.write_event(Event::Empty(p)).unwrap();
                    w.write_event(Event::End(BytesEnd::new("sheetView"))).unwrap();
                }
            }
            w.write_event(Event::End(BytesEnd::new("sheetViews"))).unwrap();
        }
        // グループ化があるときは sheetFormatPr に深さの最大を書く
        // (Excel のアウトライン欄の 1 2 3 ボタンがこれを見る)。cols より前が作法
        if !sh.row_outline.is_empty() || !sh.col_outline.is_empty() {
            let mut fp = BytesStart::new("sheetFormatPr");
            fp.push_attribute(("defaultRowHeight", "15"));
            if let Some(m) = sh.row_outline.values().max() {
                fp.push_attribute(("outlineLevelRow", m.to_string().as_str()));
            }
            if let Some(m) = sh.col_outline.values().max() {
                fp.push_attribute(("outlineLevelCol", m.to_string().as_str()));
            }
            w.write_event(Event::Empty(fp)).unwrap();
        }
        // 列幅・列のグループ化。読んだものを返す(捨てると帳票の形が変わる)。
        // 同じ指定が並ぶ区間は1つの col にまとめる
        if !sh.col_width.is_empty()
            || sh.default_col_width.is_some()
            || !sh.col_outline.is_empty()
            || !sh.col_hidden.is_empty()
        {
            w.write_event(Event::Start(BytesStart::new("cols"))).unwrap();
            if let Some(dw) = sh.default_col_width {
                let mut e = BytesStart::new("col");
                e.push_attribute(("min", "1"));
                e.push_attribute(("max", "16384"));
                e.push_attribute(("width", dw.to_string().as_str()));
                w.write_event(Event::Empty(e)).unwrap();
            }
            // 列ごとの指定(幅・深さ・畳み)をひとつの走査にまとめる
            let mut marks: std::collections::BTreeSet<u32> =
                sh.col_width.keys().copied().collect();
            marks.extend(sh.col_outline.keys().copied());
            marks.extend(sh.col_hidden.iter().copied());
            let spec = |c: u32| {
                (
                    sh.col_width.get(&c).copied(),
                    sh.col_outline.get(&c).copied(),
                    sh.col_hidden.contains(&c),
                )
            };
            let same = |a: &(Option<f32>, Option<u8>, bool), b: &(Option<f32>, Option<u8>, bool)| {
                a.1 == b.1
                    && a.2 == b.2
                    && match (a.0, b.0) {
                        (Some(x), Some(y)) => (x - y).abs() < 1e-6,
                        (None, None) => true,
                        _ => false,
                    }
            };
            let cols: Vec<u32> = marks.into_iter().collect();
            let mut i = 0;
            while i < cols.len() {
                let c0 = cols[i];
                let sp = spec(c0);
                let mut c1 = c0;
                while i + 1 < cols.len() && cols[i + 1] == c1 + 1 && same(&spec(cols[i + 1]), &sp)
                {
                    c1 = cols[i + 1];
                    i += 1;
                }
                let mut e = BytesStart::new("col");
                e.push_attribute(("min", (c0 + 1).to_string().as_str()));
                e.push_attribute(("max", (c1 + 1).to_string().as_str()));
                if let Some(wd) = sp.0 {
                    e.push_attribute(("width", wd.to_string().as_str()));
                    e.push_attribute(("customWidth", "1"));
                }
                if let Some(l) = sp.1 {
                    e.push_attribute(("outlineLevel", l.to_string().as_str()));
                }
                if sp.2 {
                    e.push_attribute(("hidden", "1"));
                }
                w.write_event(Event::Empty(e)).unwrap();
                i += 1;
            }
            w.write_event(Event::End(BytesEnd::new("cols"))).unwrap();
        }
        w.write_event(Event::Start(BytesStart::new("sheetData"))).unwrap();

        let mut rows: std::collections::BTreeMap<u32, Vec<(&Pos, &Cell)>> = Default::default();
        for (p, c) in &sh.cells { rows.entry(p.row).or_default().push((p, c)); }
        // 中身が無くてもグループ化・畳みのある行は <row> を出す(捨てない)
        for r in sh.row_outline.keys().chain(sh.row_hidden.iter()) {
            rows.entry(*r).or_default();
        }
        for (r, cells) in rows {
            let mut row = BytesStart::new("row");
            row.push_attribute(("r", (r + 1).to_string().as_str()));
            if let Some(h) = sh.row_height.get(&r) {
                row.push_attribute(("ht", h.to_string().as_str()));
                row.push_attribute(("customHeight", "1"));
            }
            if let Some(l) = sh.row_outline.get(&r) {
                row.push_attribute(("outlineLevel", l.to_string().as_str()));
            }
            if sh.row_hidden.contains(&r) {
                row.push_attribute(("hidden", "1"));
            }
            w.write_event(Event::Start(row)).unwrap();
            for (p, c) in cells {
                let mut ce = BytesStart::new("c");
                ce.push_attribute(("r", p.a1().as_str()));
                let (ty, text) = match &c.value {
                    Value::Text(t) => ("s", idx[t].to_string()),
                    Value::Number(n) => ("", n.to_string()),
                    Value::Bool(b) => ("b", (*b as u8).to_string()),
                    Value::Error(e) => ("e", e.clone()),
                    Value::Empty => ("", String::new()),
                };
                if !ty.is_empty() { ce.push_attribute(("t", ty)); }
                // 書式は styles.xml 側にあり、ここは索引だけ。
                // 触っていないセルは**原本の索引のまま**(書式の据え置き)
                if let Some(i) = kept_style(sh, p, &c.fmt) {
                    if i > 0 {
                        ce.push_attribute(("s", i.to_string().as_str()));
                    }
                } else if let Some(s) = style_idx.get(&c.fmt).filter(|i| **i > 0) {
                    ce.push_attribute(("s", s.to_string().as_str()));
                }
                w.write_event(Event::Start(ce)).unwrap();
                if let Some(f) = &c.formula {
                    let mut fe = BytesStart::new("f");
                    // 昔ながらの配列数式は t="array" と覆う範囲を添えて返す。
                    // **返さないと、開いて保存しただけで普通の式に落ちる**
                    if let Some((h, wd)) = sh.cse.get(p) {
                        let end = Pos::new(p.row + h - 1, p.col + wd - 1);
                        fe.push_attribute(("t", "array"));
                        fe.push_attribute((
                            "ref",
                            format!("{}:{}", p.a1(), end.a1()).as_str(),
                        ));
                    }
                    w.write_event(Event::Start(fe)).unwrap();
                    w.write_event(Event::Text(BytesText::new(f))).unwrap();
                    w.write_event(Event::End(BytesEnd::new("f"))).unwrap();
                }
                if !text.is_empty() {
                    w.write_event(Event::Start(BytesStart::new("v"))).unwrap();
                    w.write_event(Event::Text(BytesText::new(&text))).unwrap();
                    w.write_event(Event::End(BytesEnd::new("v"))).unwrap();
                }
                w.write_event(Event::End(BytesEnd::new("c"))).unwrap();
            }
            w.write_event(Event::End(BytesEnd::new("row"))).unwrap();
        }
        w.write_event(Event::End(BytesEnd::new("sheetData"))).unwrap();
        // シートの保護(パスワード無し。効き目はアプリが守る)。
        // 作法どおり sheetData の直後・mergeCells の前
        if sh.protected {
            let mut pr = BytesStart::new("sheetProtection");
            pr.push_attribute(("sheet", "1"));
            pr.push_attribute(("objects", "1"));
            pr.push_attribute(("scenarios", "1"));
            // **既定に頼らず全部書く。** 属性ごとに既定の向きが違うので、
            // 省くと読み手によって解釈が割れる
            let a = &sh.protect_allow;
            let d = |allow: bool| if allow { "0" } else { "1" };
            for (k, v) in [
                ("selectLockedCells", a.select_locked),
                ("selectUnlockedCells", a.select_unlocked),
                ("formatCells", a.format_cells),
                ("formatColumns", a.format_cols),
                ("formatRows", a.format_rows),
                ("insertColumns", a.insert_cols),
                ("insertRows", a.insert_rows),
                ("insertHyperlinks", a.insert_links),
                ("deleteColumns", a.delete_cols),
                ("deleteRows", a.delete_rows),
                ("sort", a.sort),
                ("autoFilter", a.autofilter),
                ("pivotTables", a.pivot),
            ] {
                pr.push_attribute((k, d(v)));
            }
            w.write_event(Event::Empty(pr)).unwrap();
        }
        // 結合を返す。読めたのに書かないと、開いて保存しただけで帳票が壊れる
        if !sh.merges.is_empty() {
            let mut mc = BytesStart::new("mergeCells");
            mc.push_attribute(("count", sh.merges.len().to_string().as_str()));
            w.write_event(Event::Start(mc)).unwrap();
            for (a, b) in &sh.merges {
                let mut m = BytesStart::new("mergeCell");
                m.push_attribute(("ref", format!("{}:{}", a.a1(), b.a1()).as_str()));
                w.write_event(Event::Empty(m)).unwrap();
            }
            w.write_event(Event::End(BytesEnd::new("mergeCells"))).unwrap();
        }
        w.write_event(Event::End(BytesEnd::new("worksheet"))).unwrap();
        let mut body = String::from_utf8(w.into_inner().into_inner()).unwrap();
        // 条件付き書式(schema では mergeCells の後・hyperlinks の前)
        if !sh.cond.is_empty() {
            let mut cf = String::new();
            for (n, r) in sh.cond.iter().enumerate() {
                let dxf = dxf_list
                    .iter()
                    .position(|p| *p == (r.color.clone(), r.fill.clone()))
                    .unwrap_or(0);
                let (a, b) = r.range;
                let sq = if a == b {
                    a.a1()
                } else {
                    format!("{}:{}", a.a1(), b.a1())
                };
                use crate::model::CondKind;
                let inner = match &r.kind {
                    CondKind::Cmp(op, v) => format!(
                        r#"<cfRule type="cellIs" dxfId="{dxf}" priority="{}" operator="{}"><formula>{v}</formula></cfRule>"#,
                        n + 1, op.as_xlsx()
                    ),
                    CondKind::Between(lo, hi, out) => format!(
                        r#"<cfRule type="cellIs" dxfId="{dxf}" priority="{}" operator="{}"><formula>{lo}</formula><formula>{hi}</formula></cfRule>"#,
                        n + 1, if *out { "notBetween" } else { "between" }
                    ),
                    CondKind::Text(t) => format!(
                        r#"<cfRule type="containsText" dxfId="{dxf}" priority="{}" operator="containsText" text="{}"/>"#,
                        n + 1,
                        t.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
                    ),
                    CondKind::Dup(false) => format!(
                        r#"<cfRule type="duplicateValues" dxfId="{dxf}" priority="{}"/>"#,
                        n + 1
                    ),
                    CondKind::Dup(true) => format!(
                        r#"<cfRule type="uniqueValues" dxfId="{dxf}" priority="{}"/>"#,
                        n + 1
                    ),
                    CondKind::Top(k, bottom) => format!(
                        r#"<cfRule type="top10" dxfId="{dxf}" priority="{}" rank="{k}"{}/>"#,
                        n + 1,
                        if *bottom { r#" bottom="1""# } else { "" }
                    ),
                    CondKind::Avg(below) => format!(
                        r#"<cfRule type="aboveAverage" dxfId="{dxf}" priority="{}"{}/>"#,
                        n + 1,
                        if *below { r#" aboveAverage="0""# } else { "" }
                    ),
                    // バー/スケール/アイコンは dxf を使わない(色は中身に持つ)
                    CondKind::Bar(color) => format!(
                        r#"<cfRule type="dataBar" priority="{}"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF{color}"/></dataBar></cfRule>"#,
                        n + 1
                    ),
                    CondKind::Scale(lo, mid, hi) => {
                        let (vo, cols) = match mid {
                            Some(m) => (
                                r#"<cfvo type="min"/><cfvo type="percentile" val="50"/><cfvo type="max"/>"#.to_string(),
                                format!(r#"<color rgb="FF{lo}"/><color rgb="FF{m}"/><color rgb="FF{hi}"/>"#),
                            ),
                            None => (
                                r#"<cfvo type="min"/><cfvo type="max"/>"#.to_string(),
                                format!(r#"<color rgb="FF{lo}"/><color rgb="FF{hi}"/>"#),
                            ),
                        };
                        format!(
                            r#"<cfRule type="colorScale" priority="{}"><colorScale>{vo}{cols}</colorScale></cfRule>"#,
                            n + 1
                        )
                    }
                    CondKind::Icons(name) => {
                        // 区切りはアイコン数で等分(3つなら 0/33/67%)
                        let k: u32 = name
                            .chars()
                            .next()
                            .and_then(|c| c.to_digit(10))
                            .unwrap_or(3)
                            .max(2);
                        let vo: String = (0..k)
                            .map(|i| {
                                format!(r#"<cfvo type="percent" val="{}"/>"#, i * 100 / k)
                            })
                            .collect();
                        format!(
                            r#"<cfRule type="iconSet" priority="{}"><iconSet iconSet="{name}">{vo}</iconSet></cfRule>"#,
                            n + 1
                        )
                    }
                    // 数式で指定。**持っている原文をそのまま返す** —
                    // 画面で解くときにずらした式は、間違っても書かない
                    CondKind::Formula(f) => format!(
                        r#"<cfRule type="expression" dxfId="{dxf}" priority="{}"><formula>{}</formula></cfRule>"#,
                        n + 1,
                        f.replace('&', "&amp;").replace('<', "&lt;")
                    ),
                };
                cf.push_str(&format!(
                    r#"<conditionalFormatting sqref="{sq}">{inner}</conditionalFormatting>"#
                ));
            }
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, &cf);
            }
        }
        // データの入力規則(schema では conditionalFormatting の後・hyperlinks の前)
        if !sh.validations.is_empty() {
            // 属性は " も守る(文言が入るため)。本文は & と < だけ
            let ea = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
            let et = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;");
            let mut dv = format!(r#"<dataValidations count="{}">"#, sh.validations.len());
            for v in &sh.validations {
                let (a, b) = v.range;
                let sq = if a == b { a.a1() } else { format!("{}:{}", a.a1(), b.a1()) };
                let mut attrs = String::new();
                if !v.kind.is_empty() {
                    attrs.push_str(&format!(r#" type="{}""#, ea(&v.kind)));
                }
                if !v.op.is_empty() {
                    attrs.push_str(&format!(r#" operator="{}""#, ea(&v.op)));
                }
                if let Some((style, t, m)) = &v.error_msg {
                    attrs.push_str(&format!(
                        r#" errorStyle="{}" errorTitle="{}" error="{}""#,
                        ea(style), ea(t), ea(m)
                    ));
                }
                if let Some((t, m)) = &v.input_msg {
                    attrs.push_str(&format!(
                        r#" promptTitle="{}" prompt="{}""#,
                        ea(t), ea(m)
                    ));
                }
                if v.hide_arrow {
                    attrs.push_str(r#" showDropDown="1""#);
                }
                let mut fs = String::new();
                if !v.formula.is_empty() {
                    fs.push_str(&format!("<formula1>{}</formula1>", et(&v.formula)));
                }
                if !v.formula2.is_empty() {
                    fs.push_str(&format!("<formula2>{}</formula2>", et(&v.formula2)));
                }
                dv.push_str(&format!(
                    r#"<dataValidation{attrs} allowBlank="{}" showInputMessage="1" showErrorMessage="1" sqref="{sq}">{fs}</dataValidation>"#,
                    if v.allow_blank { "1" } else { "0" },
                ));
            }
            dv.push_str("</dataValidations>");
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, &dv);
            }
        }
        // ハイパーリンク(schema では mergeCells の後・印刷まわりの前)
        if !sh.links.is_empty() {
            let mut hl = String::from("<hyperlinks>");
            for (n, (p, url)) in sh.links.iter().enumerate() {
                if let Some(loc) = url.strip_prefix('#') {
                    hl.push_str(&format!(
                        r#"<hyperlink ref="{}" location="{}"/>"#, p.a1(), esc(loc)));
                } else {
                    hl.push_str(&format!(
                        r#"<hyperlink ref="{}" r:id="rIdHL{}"/>"#, p.a1(), n + 1));
                }
            }
            hl.push_str("</hyperlinks>");
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, &hl);
            }
        }
        // 印刷まわり(原本の原文にモデルの向き・用紙・余白を織り込む)と
        // 図形の参照を、schema の位置(hyperlinks の後)へ
        {
            let orig = sheet_extras.get(i).map(|s| s.as_str()).unwrap_or("");
            let extra = print_extra_xml(orig, sh);
            if !extra.is_empty() {
                if let Some(pos) = body.rfind("</worksheet>") {
                    body.insert_str(pos, &extra);
                }
            }
        }
        // このアプリで挿した画像。原本に drawing が無ければ新しい部品への参照を足す
        // (原本に有るときは、その部品の中へアンカーを継ぎ足す — 部品は1シート1つの決まり)
        if (!sh.images_new.is_empty() || !sh.shapes_new.is_empty())
            && !body.contains("<drawing ")
        {
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, r#"<drawing r:id="rIdDRW"/>"#);
            }
        }
        // 表オブジェクトへの参照(schema では最後の方)
        if !sh.tables.is_empty() {
            let base: usize = book.sheets[..i].iter().map(|s| s.tables.len()).sum();
            let mut tp = format!(r#"<tableParts count="{}">"#, sh.tables.len());
            for k in 0..sh.tables.len() {
                tp.push_str(&format!(r#"<tablePart r:id="rIdTBL{}"/>"#, base + k + 1));
            }
            tp.push_str("</tableParts>");
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, &tp);
            }
        }
        // コメントの図形(VML)への参照は一番後ろ
        if !sh.comments.is_empty() {
            if let Some(pos) = body.rfind("</worksheet>") {
                body.insert_str(pos, r#"<legacyDrawing r:id="rIdVML"/>"#);
            }
        }
        put(&format!("xl/worksheets/sheet{}.xml", i + 1),
            &format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n{body}"))?;

        // このシートの rels。原本のもの(図形など)は残し、
        // リンク・コメントのぶんはこちらが作り直す
        let orig = orig_sheet_rels.get(i).cloned().flatten();
        if !sh.links.is_empty() || !sh.comments.is_empty() || orig.is_some()
            || !sh.images_new.is_empty() || !sh.shapes_new.is_empty()
            || !sh.tables.is_empty()
        {
            let mut inner = String::new();
            if let Some(o) = &orig {
                for (id, ty, target, ext) in parse_rels(o) {
                    if ty.ends_with("/hyperlink")
                        || ty.ends_with("/comments")
                        || ty.ends_with("/vmlDrawing")
                        || ty.ends_with("/table")
                    {
                        continue;
                    }
                    inner.push_str(&format!(
                        r#"<Relationship Id="{}" Type="{}" Target="{}"{}/>"#,
                        esc(&id), esc(&ty), esc(&target),
                        if ext { r#" TargetMode="External""# } else { "" }
                    ));
                }
            }
            for (n, (_, url)) in sh.links.iter().enumerate() {
                if url.starts_with('#') {
                    continue; // 帳面の中の場所は location 属性だけで足りる
                }
                inner.push_str(&format!(
                    r#"<Relationship Id="rIdHL{}" Type="{RNS}/hyperlink" Target="{}" TargetMode="External"/>"#,
                    n + 1, esc(url)
                ));
            }
            let had_drawing = orig.as_deref().is_some_and(|o| o.contains("/drawing\""));
            if (!sh.images_new.is_empty() || !sh.shapes_new.is_empty()) && !had_drawing {
                inner.push_str(&format!(
                    r#"<Relationship Id="rIdDRW" Type="{RNS}/drawing" Target="../drawings/drawingC{}.xml"/>"#,
                    i + 1
                ));
            }
            {
                let base: usize = book.sheets[..i].iter().map(|s| s.tables.len()).sum();
                for k in 0..sh.tables.len() {
                    let n = base + k + 1;
                    inner.push_str(&format!(
                        r#"<Relationship Id="rIdTBL{n}" Type="{RNS}/table" Target="../tables/table{n}.xml"/>"#
                    ));
                }
            }
            if !sh.comments.is_empty() {
                inner.push_str(&format!(
                    r#"<Relationship Id="rIdCM" Type="{RNS}/comments" Target="../comments{}.xml"/>"#,
                    i + 1
                ));
                inner.push_str(&format!(
                    r#"<Relationship Id="rIdVML" Type="{RNS}/vmlDrawing" Target="../drawings/vmlDrawing{}.vml"/>"#,
                    i + 1
                ));
            }
            put(&format!("xl/worksheets/_rels/sheet{}.xml.rels", i + 1), &format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{inner}</Relationships>"))?;
        }
        // 表オブジェクトの部品
        {
            let base: usize = book.sheets[..i].iter().map(|s| s.tables.len()).sum();
            for (k, t) in sh.tables.iter().enumerate() {
                let n = base + k + 1;
                let r = if t.a == t.b {
                    t.a.a1()
                } else {
                    format!("{}:{}", t.a.a1(), t.b.a1())
                };
                // 列の名前は見出し行から。空なら「列N」(Excel は空名を嫌う)
                let mut cols = String::new();
                for (ci, c) in (t.a.col..=t.b.col).enumerate() {
                    let nm = if t.header {
                        sh.get(Pos::new(t.a.row, c))
                            .map(|x| x.value.display())
                            .filter(|v| !v.is_empty())
                            .unwrap_or_else(|| format!("列{}", ci + 1))
                    } else {
                        format!("列{}", ci + 1)
                    };
                    cols.push_str(&format!(
                        r#"<tableColumn id="{}" name="{}"/>"#,
                        ci + 1,
                        esc(&nm)
                    ));
                }
                let b01 = |v: bool| if v { "1" } else { "0" };
                let xml = format!(
                    concat!(
                        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
                        r#"<table xmlns="{ns}" id="{n}" name="{nm}" displayName="{nm}" ref="{r}""#,
                        r#" headerRowCount="{hdr}" totalsRowCount="{tot}">"#,
                        r#"{af}<tableColumns count="{cnt}">{cols}</tableColumns>"#,
                        r#"<tableStyleInfo name="TableStyleMedium2" showFirstColumn="{fc}""#,
                        r#" showLastColumn="{lc}" showRowStripes="{rs}" showColumnStripes="{cs}"/>"#,
                        r#"</table>"#
                    ),
                    ns = NS,
                    n = n,
                    nm = esc(&t.name),
                    r = r,
                    hdr = if t.header { 1 } else { 0 },
                    tot = if t.totals { 1 } else { 0 },
                    af = if t.filter {
                        format!(r#"<autoFilter ref="{r}"/>"#)
                    } else {
                        String::new()
                    },
                    cnt = (t.b.col - t.a.col + 1),
                    cols = cols,
                    fc = b01(t.first_col),
                    lc = b01(t.last_col),
                    rs = b01(t.banded_rows),
                    cs = b01(t.banded_cols),
                );
                put(&format!("xl/tables/table{n}.xml"), &xml)?;
            }
        }
        // コメントの本体と、Excel がコメントに使う最小の VML 図形
        if !sh.comments.is_empty() {
            let mut cl = String::new();
            for (p, t) in &sh.comments {
                cl.push_str(&format!(
                    r#"<comment ref="{}" authorId="0"><text><r><t xml:space="preserve">{}</t></r></text></comment>"#,
                    p.a1(), esc(t)
                ));
            }
            put(&format!("xl/comments{}.xml", i + 1), &format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<comments xmlns="{NS}"><authors><author></author></authors><commentList>{cl}</commentList></comments>"#))?;
            let mut shapes = String::new();
            for (n, (p, _)) in sh.comments.iter().enumerate() {
                shapes.push_str(&format!(
                    r##"<v:shape id="_x0000_s{}" type="#_x0000_t202" style="position:absolute;margin-left:80pt;margin-top:2pt;width:120pt;height:60pt;z-index:{};visibility:hidden" fillcolor="#ffffe1" o:insetmode="auto"><v:fill color2="#ffffe1"/><x:ClientData ObjectType="Note"><x:MoveWithCells/><x:SizeWithCells/><x:AutoFill>False</x:AutoFill><x:Row>{}</x:Row><x:Column>{}</x:Column></x:ClientData></v:shape>"##,
                    1025 + n, n + 1, p.row, p.col
                ));
            }
            put(&format!("xl/drawings/vmlDrawing{}.vml", i + 1), &format!(
                r#"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel"><o:shapelayout v:ext="edit"><o:idmap v:ext="edit" data="1"/></o:shapelayout><v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe"><v:stroke joinstyle="miter"/><v:path gradientshapeok="t" o:connecttype="rect"/></v:shapetype>{shapes}</xml>"#))?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod fmt_round {
    use crate::model::{Borders, Cell, CellFormat, Edge, HAlign, Pos, Value};
    use crate::{Book, Sheet};

    fn book(fmt: CellFormat) -> Book {
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.set(Pos { row: 0, col: 0 }, Cell {
            formula: None, value: Value::Text("品名".into()), fmt: fmt.clone() });
        s.set(Pos { row: 0, col: 1 }, Cell {
            formula: None, value: Value::Number(1200.0), fmt });
        Book { sheets: vec![s], ..Default::default() }
    }

    fn roundtrip(b: &Book) -> Book {
        let mut buf = Vec::new();
        crate::xlsx::write(b, std::io::Cursor::new(&mut buf)).unwrap();
        crate::xlsx::read(std::io::Cursor::new(&buf)).unwrap().0
    }

    #[test]
    fn 罫線が往復する() {
        // 日本の帳票の本体。落とすと書類として通らない
        let f = CellFormat { borders: Borders::ALL, ..Default::default() };
        let back = roundtrip(&book(f.clone()));
        let c = back.sheets[0].get(Pos { row: 0, col: 0 }).unwrap();
        assert_eq!(c.fmt.borders, Borders::ALL, "罫線が消えた: {:?}", c.fmt);
    }

    #[test]
    fn 太字と塗りと揃えが往復する() {
        let f = CellFormat {
            bold: true,
            fill: Some("FFFF00".into()),
            align: HAlign::Center,
            borders: Borders { bottom: Edge::THIN, ..Borders::NONE },
            ..Default::default()
        };
        let back = roundtrip(&book(f.clone()));
        let c = back.sheets[0].get(Pos { row: 0, col: 0 }).unwrap();
        assert_eq!(c.fmt, f, "書式が変わった");
    }

    #[test]
    fn 表示形式が往復する() {
        let f = CellFormat { number_format: Some("#,##0".into()), ..Default::default() };
        let back = roundtrip(&book(f.clone()));
        let c = back.sheets[0].get(Pos { row: 0, col: 1 }).unwrap();
        assert_eq!(c.fmt.number_format.as_deref(), Some("#,##0"));
        assert_eq!(c.value, Value::Number(1200.0), "値が壊れた");
    }

    #[test]
    fn 素の書式なら索引を付けない() {
        // 余計な索引を書かない(他の道具が読むときの雑音になる)
        let mut buf = Vec::new();
        crate::xlsx::write(&book(CellFormat::default()), std::io::Cursor::new(&mut buf)).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&buf)).unwrap();
        let mut s = String::new();
        use std::io::Read;
        z.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut s).unwrap();
        assert!(!s.contains(" s=\""), "素の書式に索引を付けた");
    }

    #[test]
    fn 罫線だけのセルも残る() {
        // 値が無くても、罫線が引いてあれば帳票では意味を持つ
        let mut sh = Sheet { name: "枠".into(), ..Default::default() };
        sh.set(Pos { row: 2, col: 2 }, Cell {
            formula: None,
            value: Value::Empty,
            fmt: CellFormat { borders: Borders::ALL, ..Default::default() },
        });
        let back = roundtrip(&Book { sheets: vec![sh], ..Default::default() });
        let c = back.sheets[0].get(Pos { row: 2, col: 2 });
        assert!(c.is_some(), "値の無い罫線セルが消えた");
        assert_eq!(c.unwrap().fmt.borders, Borders::ALL);
    }
}

#[cfg(test)]
mod merge_round {
    use crate::model::{Cell, Pos, Value};
    use crate::{Book, Sheet};

    fn roundtrip(b: &Book) -> Book {
        let mut buf = Vec::new();
        crate::xlsx::write(b, std::io::Cursor::new(&mut buf)).unwrap();
        crate::xlsx::read(std::io::Cursor::new(&buf)).unwrap().0
    }

    #[test]
    fn セル結合が往復する() {
        // 開いて保存しただけで帳票の枠組みが壊れてはいけない
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.set(Pos::parse("A1").unwrap(), Cell {
            formula: None, value: Value::Text("見出し".into()), fmt: Default::default() });
        s.merges.push((Pos::parse("A1").unwrap(), Pos::parse("C1").unwrap()));
        s.merges.push((Pos::parse("A2").unwrap(), Pos::parse("A4").unwrap()));
        let back = roundtrip(&Book { sheets: vec![s], ..Default::default() });
        assert_eq!(back.sheets[0].merges.len(), 2, "結合が消えた");
        assert_eq!(back.sheets[0].merges[0],
                   (Pos::parse("A1").unwrap(), Pos::parse("C1").unwrap()));
    }

    #[test]
    fn 行の出し入れで結合も動く() {
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.merges.push((Pos::parse("A3").unwrap(), Pos::parse("C3").unwrap()));
        s.insert_row(1);
        assert_eq!(s.merges[0], (Pos::parse("A4").unwrap(), Pos::parse("C4").unwrap()),
                   "結合が置き去りになった");
        s.remove_row(1);
        assert_eq!(s.merges[0], (Pos::parse("A3").unwrap(), Pos::parse("C3").unwrap()));
    }

    #[test]
    fn 潰れた結合は消える() {
        // A1:A2 の縦結合で2行目を抜くと、1セルになる。1セルの結合は結合ではない
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.merges.push((Pos::parse("A1").unwrap(), Pos::parse("A2").unwrap()));
        s.remove_row(1);
        assert!(s.merges.is_empty(), "1セルの結合が残った: {:?}", s.merges);
    }

    #[test]
    fn 呑まれた位置が分かる() {
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.merges.push((Pos::parse("A1").unwrap(), Pos::parse("B2").unwrap()));
        assert!(!s.covered_by_merge(Pos::parse("A1").unwrap()), "左上まで呑んだ");
        assert!(s.covered_by_merge(Pos::parse("B2").unwrap()));
        assert!(!s.covered_by_merge(Pos::parse("C1").unwrap()));
    }
}

#[cfg(test)]
mod colwidth_round {
    use crate::model::{Cell, Pos, Value};
    use crate::{Book, Sheet};

    #[test]
    fn 列幅が往復する() {
        // 読み飛ばして保存すると帳票の形が変わる
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.set(Pos::parse("A1").unwrap(), Cell {
            formula: None, value: Value::Text("品".into()), fmt: Default::default() });
        s.col_width.insert(0, 3.5);
        s.col_width.insert(2, 24.0);
        let mut buf = Vec::new();
        crate::xlsx::write(&Book { sheets: vec![s], ..Default::default() }, std::io::Cursor::new(&mut buf)).unwrap();
        let back = crate::xlsx::read(std::io::Cursor::new(&buf)).unwrap().0;
        let cw = &back.sheets[0].col_width;
        assert_eq!(cw.get(&0), Some(&3.5), "列幅が消えた: {cw:?}");
        assert_eq!(cw.get(&2), Some(&24.0));
        assert_eq!(cw.get(&1), None, "指定していない列に幅が付いた");
    }

    #[test]
    fn 列の出し入れで幅も動く() {
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.col_width.insert(1, 20.0);
        s.insert_col(0);
        assert_eq!(s.col_width.get(&2), Some(&20.0), "幅が置き去り: {:?}", s.col_width);
        s.remove_col(0);
        assert_eq!(s.col_width.get(&1), Some(&20.0));
    }

    #[test]
    fn 実物の様式の列幅を読める() {
        let p = "/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki/実施要領様式7_提案見積書.xlsx";
        let Ok(f) = std::fs::File::open(p) else { return }; // 無い機械では飛ばす
        let (book, _) = crate::xlsx::read(f).unwrap();
        let n: usize = book.sheets.iter().map(|s| s.col_width.len()).sum();
        assert!(n > 0, "実物の列幅を1つも読めていない");
    }
}

#[cfg(test)]
mod rowheight_round {
    use crate::model::{Cell, Pos, Value};
    use crate::{Book, Sheet};

    #[test]
    fn 行の高さが往復する() {
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.set(Pos::parse("A3").unwrap(), Cell {
            formula: None, value: Value::Text("高い行".into()), fmt: Default::default() });
        s.row_height.insert(2, 27.5);
        let mut buf = Vec::new();
        crate::xlsx::write(&Book { sheets: vec![s], ..Default::default() }, std::io::Cursor::new(&mut buf)).unwrap();
        let back = crate::xlsx::read(std::io::Cursor::new(&buf)).unwrap().0;
        assert_eq!(back.sheets[0].row_height.get(&2), Some(&27.5), "行の高さが消えた");
    }

    #[test]
    fn 行の出し入れで高さも動く() {
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.row_height.insert(3, 30.0);
        s.insert_row(0);
        assert_eq!(s.row_height.get(&4), Some(&30.0), "{:?}", s.row_height);
        s.remove_row(0);
        assert_eq!(s.row_height.get(&3), Some(&30.0));
    }
}

#[cfg(test)]
mod carry_tests {
    use crate::model::{Cell, Pos, Value};
    use crate::{Book, Sheet};
    use std::io::{Cursor, Read, Write};

    fn xlsx_with_parts() -> Vec<u8> {
        let mut book = Book::default();
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.set(Pos::parse("A1").unwrap(), Cell::input("品名"));
        book.sheets.push(s);
        let mut base = Vec::new();
        crate::xlsx::write(&book, Cursor::new(&mut base)).unwrap();
        // 原本に「こちらが知らない部品」を足し、シートに印刷設定と図形を差す
        let mut z = zip::ZipArchive::new(Cursor::new(&base)).unwrap();
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o: zip::write::FileOptions<'_, ()> = Default::default();
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).unwrap();
            if name == "xl/worksheets/sheet1.xml" {
                let s = String::from_utf8(buf).unwrap().replace(
                    "</worksheet>",
                    r#"<pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/><pageSetup paperSize="9" orientation="landscape"/><drawing r:id="rId9"/></worksheet>"#,
                );
                buf = s.into_bytes();
            }
            out.start_file(name, o).unwrap();
            out.write_all(&buf).unwrap();
        }
        out.start_file("xl/theme/theme1.xml", o).unwrap();
        out.write_all(b"<theme/>").unwrap();
        out.start_file("xl/drawings/drawing1.xml", o).unwrap();
        out.write_all(b"<wsDr/>").unwrap();
        out.start_file("xl/printerSettings/printerSettings1.bin", o).unwrap();
        out.write_all(b"\x01\x02printer").unwrap();
        out.finish().unwrap().into_inner()
    }

    #[test]
    fn 開いて保存しても部品が残る() {
        let src = xlsx_with_parts();
        let (book, _) = crate::xlsx::read(Cursor::new(&src)).unwrap();
        let mut out = Vec::new();
        crate::xlsx::write_with(&book, Some(Cursor::new(&src)), Cursor::new(&mut out)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
        let names: Vec<String> =
            (0..z.len()).map(|i| z.by_index(i).unwrap().name().into()).collect();
        for want in ["xl/theme/theme1.xml", "xl/drawings/drawing1.xml",
                     "xl/printerSettings/printerSettings1.bin"] {
            assert!(names.iter().any(|n| n == want), "{want} が消えた: {names:?}");
        }
        // 印刷の向きと図形の参照がシートに戻っている
        let mut s = String::new();
        z.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut s).unwrap();
        assert!(s.contains("landscape"), "印刷の向きが消えた");
        assert!(s.contains("<drawing"), "図形の参照が消えた");
        // 値も生きている
        let (back, _) = crate::xlsx::read(Cursor::new(&out)).unwrap();
        assert_eq!(back.sheets[0].get(Pos::parse("A1").unwrap()).map(|c| c.value.display()),
                   Some("品名".into()));
    }

    #[test]
    fn 古い計算順は持ち越さない() {
        // calcChain が古いままだと Excel が誤った順で開くことがある
        let src = xlsx_with_parts();
        let mut with_chain = Vec::new();
        {
            let mut z = zip::ZipArchive::new(Cursor::new(&src)).unwrap();
            let mut out = zip::ZipWriter::new(Cursor::new(&mut with_chain));
            let o: zip::write::FileOptions<'_, ()> = Default::default();
            for i in 0..z.len() {
                let mut f = z.by_index(i).unwrap();
                let name = f.name().to_string();
                let mut buf = Vec::new();
                f.read_to_end(&mut buf).unwrap();
                out.start_file(name, o).unwrap();
                out.write_all(&buf).unwrap();
            }
            out.start_file("xl/calcChain.xml", o).unwrap();
            out.write_all(b"<calcChain/>").unwrap();
            out.finish().unwrap();
        }
        let (book, _) = crate::xlsx::read(Cursor::new(&with_chain)).unwrap();
        let mut out = Vec::new();
        crate::xlsx::write_with(&book, Some(Cursor::new(&with_chain)), Cursor::new(&mut out)).unwrap();
        let z = zip::ZipArchive::new(Cursor::new(&out)).unwrap();
        let names: Vec<String> = z.file_names().map(String::from).collect();
        assert!(!names.iter().any(|n| n == "xl/calcChain.xml"), "古い計算順を持ち越した");
    }
}

#[cfg(test)]
mod name_roundtrip_tests {
    use super::*;
    use crate::model::Cell;
    use crate::recalc;

    #[test]
    fn 名前の定義が往復して式で効く() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("100"));
        b.sheets[0].set(Pos::parse("B1").unwrap(), Cell::input("=単価*2"));
        b.sheets[0].names.push(("単価".into(), "A1".into()));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (mut back, _) = read(buf).expect("読めない");
        assert_eq!(back.sheets[0].names, vec![("単価".to_string(), "A1".to_string())],
            "名前が往復しない");
        recalc(&mut back.sheets[0]);
        assert_eq!(back.sheets[0].value(Pos::parse("B1").unwrap()), Value::Number(200.0));
    }

    #[test]
    fn 実物のprint_areaを壊さない() {
        let src = "/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki/実施要領様式7_提案見積書.xlsx";
        let Ok(bytes) = std::fs::read(src) else { return };
        let (book, _) = read(Cursor::new(&bytes)).expect("読めない");
        let mut out = Cursor::new(Vec::new());
        write_with(&book, Some(Cursor::new(&bytes)), &mut out).expect("書けない");
        out.set_position(0);
        let mut z = zip::ZipArchive::new(out).expect("zipでない");
        let mut s = String::new();
        use std::io::Read as _;
        z.by_name("xl/workbook.xml").expect("workbookが無い")
            .read_to_string(&mut s).unwrap();
        assert!(s.contains("_xlnm.Print_Area"),
            "印刷範囲(Print_Area)が保存で消えた");
    }
}

#[cfg(test)]
mod link_comment_tests {
    use super::*;
    use crate::model::Cell;

    fn roundtrip(b: &Book) -> Book {
        let mut buf = Cursor::new(Vec::new());
        write(b, &mut buf).expect("書けない");
        buf.set_position(0);
        read(buf).expect("読めない").0
    }

    #[test]
    fn ハイパーリンクが往復する() {
        let mut b = Book::new();
        let p = Pos::parse("B2").unwrap();
        b.sheets[0].set(p, Cell::input("会社サイト"));
        b.sheets[0].links.insert(p, "https://example.co.jp/".into());
        let back = roundtrip(&b);
        assert_eq!(back.sheets[0].links.get(&p).map(|s| s.as_str()),
            Some("https://example.co.jp/"), "リンクが往復しない");
    }

    #[test]
    fn 帳面の中へのリンクがlocationで往復する() {
        let mut b = Book::new();
        b.sheets.push(crate::model::Sheet::new("集計"));
        let p = Pos::parse("B2").unwrap();
        b.sheets[0].set(p, Cell::input("集計へ"));
        b.sheets[0].links.insert(p, "#集計!B5".into());
        let back = roundtrip(&b);
        assert_eq!(back.sheets[0].links.get(&p).map(|s| s.as_str()),
            Some("#集計!B5"), "帳面の中へのリンクが往復しない");
    }

    #[test]
    fn バーとスケールとアイコンの条件付き書式が往復する() {
        use crate::model::{CondKind, CondRule};
        let mut b = Book::new();
        for (i, v) in ["10", "20", "30"].iter().enumerate() {
            b.sheets[0].set(Pos::new(i as u32, 0), Cell::input(v));
        }
        let range = (Pos::new(0, 0), Pos::new(2, 0));
        b.sheets[0].cond.push(CondRule {
            range, kind: CondKind::Bar("638EC6".into()), color: None, fill: None });
        b.sheets[0].cond.push(CondRule {
            range,
            kind: CondKind::Scale("F8696B".into(), Some("FFEB84".into()), "63BE7B".into()),
            color: None, fill: None });
        b.sheets[0].cond.push(CondRule {
            range, kind: CondKind::Icons("3Arrows".into()), color: None, fill: None });
        let back = roundtrip(&b);
        let cond = &back.sheets[0].cond;
        assert_eq!(cond.len(), 3, "本数が違う: {cond:?}");
        assert_eq!(cond[0].kind, CondKind::Bar("638EC6".into()), "バーが往復しない");
        assert_eq!(
            cond[1].kind,
            CondKind::Scale("F8696B".into(), Some("FFEB84".into()), "63BE7B".into()),
            "スケールが往復しない(FF の剥がし過ぎに注意)"
        );
        assert_eq!(cond[2].kind, CondKind::Icons("3Arrows".into()), "アイコンが往復しない");
    }

    #[test]
    fn 縦棒のスパークラインが棒のまま往復する() {
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(crate::model::SheetShape {
            at: Pos::parse("C2").unwrap(),
            width_px: 90.0,
            height_px: 22.0,
            kind: "spark-col".into(),
            line: Some("1B6E3C".into()),
            points: vec![(0.17, 0.0), (0.5, 0.9), (0.83, 0.25)],
            base: 0.75,
            ..Default::default()
        });
        let back = roundtrip(&b);
        let sp = back.sheets[0]
            .shapes
            .iter()
            .find(|s| s.kind == "spark-col")
            .expect("棒が折れ線に化けた(jo: の札が読めていない)");
        assert!((sp.base - 0.75).abs() < 1e-3, "底が違う: {}", sp.base);
        assert_eq!(sp.points.len(), 3, "棒の本数が違う: {:?}", sp.points);
        assert!((sp.points[1].0 - 0.5).abs() < 0.02, "中心が違う: {:?}", sp.points[1]);
        assert!((sp.points[1].1 - 0.9).abs() < 0.02, "先端が違う: {:?}", sp.points[1]);
    }

    #[test]
    fn コメントが往復する() {
        let mut b = Book::new();
        let p = Pos::parse("C3").unwrap();
        b.sheets[0].set(p, Cell::input("単価"));
        b.sheets[0].comments.insert(p, "去年の実績から仮置き。要確認".into());
        let back = roundtrip(&b);
        assert_eq!(back.sheets[0].comments.get(&p).map(|s| s.as_str()),
            Some("去年の実績から仮置き。要確認"), "コメントが往復しない");
    }

    #[test]
    fn 実物にコメントを足しても部品が揃う() {
        let src = "/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki/実施要領様式7_提案見積書.xlsx";
        let Ok(bytes) = std::fs::read(src) else { return };
        let (mut book, _) = read(Cursor::new(&bytes)).expect("読めない");
        let p = Pos::parse("A30").unwrap();
        book.sheets[0].comments.insert(p, "ここに社名を書く".into());
        book.sheets[0].links.insert(p, "https://example.co.jp/".into());
        let mut out = Cursor::new(Vec::new());
        write_with(&book, Some(Cursor::new(&bytes)), &mut out).expect("書けない");
        out.set_position(0);
        // 読み直せて中身が残る
        let (back, _) = read(Cursor::new(out.get_ref().clone())).expect("読み直せない");
        assert_eq!(back.sheets[0].comments.get(&p).map(|s| s.as_str()),
            Some("ここに社名を書く"));
        assert!(back.sheets[0].links.contains_key(&p), "実物でリンクが消えた");
        // 部品の宣言も揃っている
        let mut z = zip::ZipArchive::new(out).unwrap();
        let mut ct = String::new();
        use std::io::Read as _;
        z.by_name("[Content_Types].xml").unwrap().read_to_string(&mut ct).unwrap();
        assert!(ct.contains("/xl/comments1.xml"), "コメントの宣言が無い");
        assert!(ct.contains("Extension=\"vml\""), "VML の宣言が無い");
    }
}

#[cfg(test)]
mod cond_tests {
    use super::*;
    use crate::model::{Cell, CondAux, CondKind, CondOp, CondRule};

    #[test]
    fn 塗りはfgcolorでもbgcolorでも読める() {
        // 書き手ごとに置き場所が違う。片方しか見ないと、条件付き書式の
        // 色が**黙って消える**(規則は残るので気付きにくい)
        let dxf = |body: &str| {
            super::parse_dxfs(&format!(
                r#"<styleSheet><dxfs count="1"><dxf>{body}</dxf></dxfs></styleSheet>"#
            ))
            .first()
            .cloned()
            .map(|(_, fill)| fill)
            .unwrap_or_default()
        };
        assert_eq!(
            dxf(r#"<fill><patternFill><bgColor rgb="FFDDEBF7"/></patternFill></fill>"#),
            Some("DDEBF7".into()),
            "LibreOffice の書き方(bgColor)が読めない"
        );
        assert_eq!(
            dxf(r#"<fill><patternFill patternType="solid"><fgColor rgb="00DDEBF7"/></patternFill></fill>"#),
            Some("DDEBF7".into()),
            "openpyxl の書き方(solid + fgColor)が読めない"
        );
        // 両方あるとき: rgb を持っている bgColor が勝つ
        assert_eq!(
            dxf(r#"<fill><patternFill patternType="solid"><fgColor indexed="64"/><bgColor rgb="FFFFC7CE"/></patternFill></fill>"#),
            Some("FFC7CE".into()),
            "Excel の書き方(indexed の fgColor + bgColor)が読めない"
        );
    }

    fn roundtrip(b: &Book) -> Book {
        let mut buf = Cursor::new(Vec::new());
        write(b, &mut buf).expect("書けない");
        buf.set_position(0);
        read(buf).expect("読めない").0
    }

    #[test]
    fn 条件付き書式が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("-5"));
        b.sheets[0].cond.push(CondRule {
            range: (Pos::parse("A1").unwrap(), Pos::parse("A9").unwrap()),
            kind: CondKind::Cmp(CondOp::Lt, 0.0),
            color: Some("C00000".into()),
            fill: None,
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let r = &back.sheets[0].cond;
        assert_eq!(r.len(), 1, "規則が往復しない");
        assert_eq!(r[0].kind, CondKind::Cmp(CondOp::Lt, 0.0));
        assert_eq!(r[0].color.as_deref(), Some("C00000"), "見た目(dxf)が往復しない");
        // 効き方
        let aux = CondAux::default();
        assert!(r[0].hits(Pos::parse("A1").unwrap(), &Value::Number(-5.0), &aux));
        assert!(!r[0].hits(Pos::parse("A1").unwrap(), &Value::Number(5.0), &aux));
        assert!(
            !r[0].hits(Pos::parse("B1").unwrap(), &Value::Number(-5.0), &aux),
            "範囲の外に効いた"
        );
    }

    #[test]
    fn 数式で指定した縞模様が往復して効く() {
        // 実物の帳票でいちばん多い条件付き書式。読めないでは済まない
        let mut b = Book::new();
        for i in 0..10u32 {
            b.sheets[0].set(Pos::new(i, 0), Cell::input(&format!("{}", i + 1)));
        }
        b.sheets[0].cond.push(CondRule {
            range: (Pos::parse("A1").unwrap(), Pos::parse("B10").unwrap()),
            kind: CondKind::Formula("MOD(ROW(),2)=0".into()),
            color: None,
            fill: Some("DDEBF7".into()),
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        assert!(
            rep.unsupported.is_empty(),
            "読めたのに報告が出た: {:?}",
            rep.unsupported
        );
        let r = &back.sheets[0].cond;
        assert_eq!(r.len(), 1, "規則が往復しない: {r:?}");
        assert_eq!(
            r[0].kind,
            CondKind::Formula("MOD(ROW(),2)=0".into()),
            "式の原文が往復しない"
        );
        assert_eq!(r[0].fill.as_deref(), Some("DDEBF7"), "見た目(dxf)が往復しない");
        // 効き方 — ROW() は1から数えるので、偶数行(A2/A4…)が当たる
        let sh = &back.sheets[0];
        let aux = r[0].aux(sh);
        for (a1, want) in [("A1", false), ("A2", true), ("A3", false), ("B4", true)] {
            let p = Pos::parse(a1).unwrap();
            assert_eq!(r[0].hits(p, &sh.value(p), &aux), want, "{a1} の縞が違う");
        }
        assert!(
            !r[0].hits(Pos::parse("C2").unwrap(), &sh.value(Pos::parse("C2").unwrap()), &aux),
            "範囲の外に効いた"
        );
    }

    #[test]
    fn 数式で指定は左上を錨に相対参照をずらす() {
        // **ここが静かに狂う所。** 式は範囲の左上のことを書いたものとして
        // 貯まっているので、他のセルではずらして解かないと1行ずれる
        let mut b = Book::new();
        let sh = &mut b.sheets[0];
        sh.set(Pos::parse("C2").unwrap(), Cell::input("あ"));
        sh.set(Pos::parse("C3").unwrap(), Cell::input("ああああ"));
        sh.set(Pos::parse("C4").unwrap(), Cell::input("いい"));
        // $ で列を固定した、行まるごとの色分け(実物でよく使う形)
        sh.set(Pos::parse("A2").unwrap(), Cell::input("済"));
        sh.set(Pos::parse("A3").unwrap(), Cell::input("未"));
        sh.cond.push(CondRule {
            range: (Pos::parse("C2").unwrap(), Pos::parse("C4").unwrap()),
            kind: CondKind::Formula("LEN(C2)>3".into()),
            color: Some("C00000".into()),
            fill: None,
        });
        sh.cond.push(CondRule {
            range: (Pos::parse("B2").unwrap(), Pos::parse("C3").unwrap()),
            kind: CondKind::Formula(r#"$A2="済""#.into()),
            color: None,
            fill: Some("FFF2CC".into()),
        });
        let back = roundtrip(&b);
        let sh = &back.sheets[0];
        let r = &sh.cond;
        assert_eq!(r.len(), 2, "規則が往復しない: {r:?}");

        let aux = r[0].aux(sh);
        for (a1, want) in [("C2", false), ("C3", true), ("C4", false)] {
            let p = Pos::parse(a1).unwrap();
            assert_eq!(
                r[0].hits(p, &sh.value(p), &aux),
                want,
                "{a1}: 錨がずれている(左上の式をそのまま使っていないか)"
            );
        }

        // $A は列を固定 — B列でも C列でも A列を見る。行だけがずれる
        let aux = r[1].aux(sh);
        for (a1, want) in [("B2", true), ("C2", true), ("B3", false), ("C3", false)] {
            let p = Pos::parse(a1).unwrap();
            assert_eq!(
                r[1].hits(p, &sh.value(p), &aux),
                want,
                "{a1}: $ で固定した列が動いている"
            );
        }
    }

    #[test]
    fn 数式で指定は解けなくても原文を落とさない() {
        // 評価に失敗しても**ファイルは減らない** — 保存はいつも原文を返す
        let mut b = Book::new();
        let f = "COUNTIF(知らない表!A:A,A1)>0";
        b.sheets[0].cond.push(CondRule {
            range: (Pos::parse("A1").unwrap(), Pos::parse("A3").unwrap()),
            kind: CondKind::Formula(f.into()),
            color: None,
            fill: Some("FCE4D6".into()),
        });
        let back = roundtrip(&b);
        let sh = &back.sheets[0];
        assert_eq!(
            sh.cond.first().map(|r| &r.kind),
            Some(&CondKind::Formula(f.into())),
            "解けない式が保存で失われた"
        );
        // 解けない式は当たらない側へ倒す(見当違いの色を付けない)
        let aux = sh.cond[0].aux(sh);
        let p = Pos::parse("A1").unwrap();
        assert!(!sh.cond[0].hits(p, &sh.value(p), &aux), "解けない式で色が付いた");
    }

    #[test]
    fn 新しい規則の種類も往復して効く() {
        let mut b = Book::new();
        let s = &mut b.sheets[0];
        for (i, v) in ["10", "20", "20", "5"].iter().enumerate() {
            s.set(Pos::new(i as u32, 0), Cell::input(v));
        }
        let range = (Pos::new(0, 0), Pos::new(3, 0));
        s.cond.push(CondRule { range, kind: CondKind::Between(8.0, 15.0, false), color: None, fill: Some("FFF2CC".into()) });
        s.cond.push(CondRule { range, kind: CondKind::Text("2".into()), color: None, fill: Some("E2EFDA".into()) });
        s.cond.push(CondRule { range, kind: CondKind::Dup(false), color: Some("9C0006".into()), fill: None });
        s.cond.push(CondRule { range, kind: CondKind::Top(2, false), color: None, fill: Some("D9E1F2".into()) });
        s.cond.push(CondRule { range, kind: CondKind::Avg(false), color: None, fill: None });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let sh = &back.sheets[0];
        let r = &sh.cond;
        assert_eq!(r.len(), 5, "規則が往復しない: {r:?}");
        assert_eq!(r[0].kind, CondKind::Between(8.0, 15.0, false));
        assert_eq!(r[1].kind, CondKind::Text("2".into()));
        assert_eq!(r[2].kind, CondKind::Dup(false));
        assert_eq!(r[3].kind, CondKind::Top(2, false));
        assert_eq!(r[4].kind, CondKind::Avg(false));
        // 効き方(下ごしらえ込み)
        let p0 = Pos::new(0, 0);
        let aux = r[2].aux(sh);
        assert!(r[2].hits(Pos::new(1, 0), &Value::Number(20.0), &aux), "重複が効かない");
        assert!(!r[2].hits(p0, &Value::Number(10.0), &aux), "重複でない値に効いた");
        let aux = r[3].aux(sh);
        assert!(r[3].hits(Pos::new(1, 0), &Value::Number(20.0), &aux), "上位2が効かない");
        assert!(!r[3].hits(Pos::new(3, 0), &Value::Number(5.0), &aux));
        let aux = r[4].aux(sh);
        // 平均 = 13.75 → 20 は上
        assert!(r[4].hits(Pos::new(1, 0), &Value::Number(20.0), &aux));
        assert!(!r[4].hits(p0, &Value::Number(10.0), &aux));
        let aux = CondAux::default();
        assert!(r[0].hits(p0, &Value::Number(10.0), &aux), "間が効かない");
        assert!(r[1].hits(Pos::new(1, 0), &Value::Number(20.0), &aux), "文字を含むが効かない");
    }
}

#[cfg(test)]
mod validation_roundtrip_tests {
    use super::*;
    use crate::model::{Cell, Validation};

    #[test]
    fn 入力規則が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("D2").unwrap(), Cell::input("東京"));
        b.sheets[0].set(Pos::parse("D3").unwrap(), Cell::input("大阪"));
        b.sheets[0].validations.push(Validation::list(
            (Pos::parse("B2").unwrap(), Pos::parse("B10").unwrap()),
            r#""甲,乙,丙""#.into(),
        ));
        b.sheets[0].validations.push(Validation::list(
            (Pos::parse("C2").unwrap(), Pos::parse("C2").unwrap()),
            "$D$2:$D$3".into(),
        ));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, rep) = read(buf).expect("読めない");
        let v = &back.sheets[0].validations;
        assert_eq!(v.len(), 2, "規則が往復しない: {v:?}");
        assert_eq!(v[0].formula, r#""甲,乙,丙""#, "直書きの原文が変わった");
        assert_eq!(v[0].range, (Pos::parse("B2").unwrap(), Pos::parse("B10").unwrap()));
        assert_eq!(v[1].formula, "$D$2:$D$3", "範囲参照の原文が変わった");
        // 候補も引ける
        assert_eq!(v[0].options(&back.sheets[0]), vec!["甲", "乙", "丙"]);
        assert_eq!(v[1].options(&back.sheets[0]), vec!["東京", "大阪"]);
        assert!(rep.unsupported.is_empty(), "全部読めるのに報告が出た: {:?}", rep.unsupported);
    }

    #[test]
    fn list以外の規則も持ち越す() {
        // 手書きの最小 xlsx を作るのは大掛かりなので、書いた xlsx の
        // dataValidation の type を書き換えて読み直す
        let mut b = Book::new();
        b.sheets[0].validations.push(Validation::list(
            (Pos::parse("A1").unwrap(), Pos::parse("A1").unwrap()),
            r#""x""#.into(),
        ));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        // zip の中の sheet1.xml を直に書き換える
        let mut z = zip::ZipArchive::new(Cursor::new(buf.get_ref().clone())).unwrap();
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        use std::io::{Read as _, Write as _};
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut s = Vec::new();
            f.read_to_end(&mut s).unwrap();
            if name.ends_with("sheet1.xml") {
                let t = String::from_utf8(s).unwrap()
                    .replace(r#"type="list""#, r#"type="whole""#);
                s = t.into_bytes();
            }
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&s).unwrap();
        }
        let out = w.finish().unwrap();
        let (back, _rep) = read(Cursor::new(out.into_inner())).expect("読めない");
        // 2026-08-06 改訂: list 以外も落とさず、種類ごと持ち越す
        assert_eq!(back.sheets[0].validations.len(), 1, "規則が消えた");
        assert_eq!(back.sheets[0].validations[0].kind, "whole", "種類が持ち越せない");
    }

    #[test]
    fn 画像のずらしが往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].images_new.push(crate::model::SheetImage {
            at: Pos::parse("B2").unwrap(),
            dx_px: 30.0,
            dy_px: 12.0,
            width_px: 100.0,
            height_px: 50.0,
            data: vec![0x89, 0x50, 0x4E, 0x47],
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        // 読み側は images(読んだ画像)に入る。位置と大きさが保たれている
        assert_eq!(back.sheets[0].images.len(), 1, "画像が往復しない");
        let im = &back.sheets[0].images[0];
        assert_eq!(im.at, Pos::parse("B2").unwrap());
        assert_eq!(im.width_px.round(), 100.0);
    }

    #[test]
    fn ヘッダーとフッターが往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].header = Some("&C月次売上&R&P / &N".into());
        b.sheets[0].footer = Some("&L社外秘".into());
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert_eq!(back.sheets[0].header.as_deref(), Some("&C月次売上&R&P / &N"));
        assert_eq!(back.sheets[0].footer.as_deref(), Some("&L社外秘"));
    }

    #[test]
    fn 罫線の線種と色が往復する() {
        use crate::model::{BStyle, Edge};
        let mut b = Book::new();
        let mut cell = Cell::input("x");
        cell.fmt.borders.bottom = Edge::line(BStyle::MediumDashed, Some(0x00B050));
        cell.fmt.borders.top = Edge::line(BStyle::Double, None);
        b.sheets[0].set(Pos::parse("B2").unwrap(), cell);
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let bd = back.sheets[0].get(Pos::parse("B2").unwrap()).unwrap().fmt.borders;
        assert_eq!(bd.bottom.style, BStyle::MediumDashed, "線種が往復しない");
        assert_eq!(bd.bottom.color, Some(0x00B050), "線の色が往復しない");
        assert_eq!(bd.top.style, BStyle::Double);
        assert_eq!(bd.top.color, None, "自動(黒)が色付きに化けた");
        assert!(!bd.left.on);
    }

    #[test]
    fn ピボットの絞り込みが往復する() {
        let mut b = Book::new();
        b.pivots.push(crate::model::PivotDef {
            sheet: "Sheet1".into(),
            src: (Pos::parse("A1").unwrap(), Pos::parse("C4").unwrap()),
            rows_sel: vec!["区分".into()],
            cols_sel: vec!["月".into()],
            value: "金額".into(),
            agg: "合計".into(),
            totals: true,
            subtotals: false,
            blank_rows: false,
            compact: false,
            dest: Pos::parse("E1").unwrap(),
            size: (3, 3),
            hide: vec![("区分".into(), vec!["紙製品".into(), "その他".into()])],
            style: String::new(),
            name: String::new(),
            vfilter: Some((">=".into(), 1000.0)),
            group_by: vec![("日付".into(), "四半期".into()), ("金額".into(), "幅:100".into())],
            show_as: "累計".into(),
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert_eq!(back.pivots.len(), 1);
        assert_eq!(
            back.pivots[0].hide,
            vec![("区分".to_string(), vec!["紙製品".to_string(), "その他".to_string()])],
            "絞り込みが往復しない"
        );
        assert_eq!(
            back.pivots[0].vfilter,
            Some((">=".to_string(), 1000.0)),
            "値のフィルターが往復しない"
        );
        assert_eq!(
            back.pivots[0].group_by,
            vec![
                ("日付".to_string(), "四半期".to_string()),
                ("金額".to_string(), "幅:100".to_string())
            ],
            "グループ化が往復しない"
        );
        assert_eq!(back.pivots[0].show_as, "累計", "計算の種類が往復しない");
    }

    #[test]
    fn 手動計算が往復する() {
        // 手動(calcPr calcMode="manual")を落とすと、開き直しで勝手に自動へ戻る
        let mut b = Book::new();
        b.calc_manual = true;
        b.calc_iter = Some((50, 0.01));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert!(back.calc_manual, "手動計算が往復しない");
        assert_eq!(back.calc_iter, Some((50, 0.01)), "反復計算が往復しない");
        let mut b2 = Book::new();
        b2.r1c1 = true;
        let mut buf = Cursor::new(Vec::new());
        write(&b2, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back2, _) = read(buf).expect("読めない");
        assert!(back2.r1c1, "R1C1 が往復しない");
        // 自動(既定)は calcPr を書かない → 読みも false
        let b2 = Book::new();
        let mut buf2 = Cursor::new(Vec::new());
        write(&b2, &mut buf2).expect("書けない");
        buf2.set_position(0);
        let (back2, _) = read(buf2).expect("読めない");
        assert!(!back2.calc_manual);
    }

    #[test]
    fn 原本のcalcPrはcalcModeだけ差し替える() {
        // calcId 等の他の属性は据え置き
        let src = r#"<workbook><sheets/><calcPr calcId="191029"/></workbook>"#;
        let out = patch_calc_pr(src, true);
        assert!(out.contains(r#"calcMode="manual""#), "{out}");
        assert!(out.contains(r#"calcId="191029""#), "calcId が消えた: {out}");
        // 手動 → 自動へ戻すときは calcMode の値だけ書き換える
        let back = patch_calc_pr(&out, false);
        assert!(back.contains(r#"calcMode="auto""#), "{back}");
        // calcPr が無い原本に手動を差し込む(スキーマの順 = sheets の後)
        let none = r#"<workbook><sheets><sheet name="a"/></sheets></workbook>"#;
        let ins = patch_calc_pr(none, true);
        assert!(ins.contains(r#"</sheets><calcPr calcMode="manual"/>"#), "{ins}");
    }

    #[test]
    fn 整数の規則と文言が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        let mut v = Validation::list(
            (Pos::parse("B2").unwrap(), Pos::parse("B9").unwrap()),
            "1".into(),
        );
        v.kind = "whole".into();
        v.op = "between".into();
        v.formula2 = "100".into();
        v.input_msg = Some(("数量".into(), "1 から 100 の整数で".into()));
        v.error_msg = Some(("stop".into(), "".into(), "その数は使えません".into()));
        v.allow_blank = false; // 「空白を無視」を外した形も往復する
        v.hide_arrow = true; // ▾ を出さない指定(showDropDown)も往復する
        b.sheets[0].validations.push(v);
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let v = &back.sheets[0].validations[0];
        assert_eq!(v.kind, "whole");
        assert_eq!(v.op, "between");
        assert_eq!((v.formula.as_str(), v.formula2.as_str()), ("1", "100"));
        assert_eq!(
            v.input_msg,
            Some(("数量".to_string(), "1 から 100 の整数で".to_string()))
        );
        assert_eq!(
            v.error_msg,
            Some(("stop".to_string(), String::new(), "その数は使えません".to_string()))
        );
        assert!(!v.allow_blank, "allowBlank が往復しない");
        assert!(v.hide_arrow, "showDropDown が往復しない");
        // 判定も一緒に確かめる
        let s = &back.sheets[0];
        assert!(v.passes(s, "50"));
        assert!(!v.passes(s, "0"), "範囲の外が通った");
        assert!(!v.passes(s, "2.5"), "小数が整数の規則を通った");
        assert!(!v.passes(s, "あ"), "文字が数の規則を通った");
    }
}

#[cfg(test)]
mod page_setup_tests {
    use super::*;

    #[test]
    fn 印刷の設定が読める() {
        // 最小の xlsx を書き、sheet1.xml に pageSetup / pageMargins を差して読み直す
        let b = Book::new();
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let mut z = zip::ZipArchive::new(Cursor::new(buf.get_ref().clone())).unwrap();
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        use std::io::{Read as _, Write as _};
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut s = Vec::new();
            f.read_to_end(&mut s).unwrap();
            if name.ends_with("sheet1.xml") {
                let t = String::from_utf8(s).unwrap().replace(
                    "</worksheet>",
                    r#"<pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/><pageSetup paperSize="8" orientation="landscape"/></worksheet>"#,
                );
                s = t.into_bytes();
            }
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&s).unwrap();
        }
        let out = w.finish().unwrap();
        let (back, _) = read(Cursor::new(out.into_inner())).expect("読めない");
        let sh = &back.sheets[0];
        assert!(sh.landscape, "横向きが読めない");
        assert_eq!(sh.paper_size, Some(8), "用紙コードが読めない");
        let (l, _, t, _) = sh.margins_mm.expect("余白が読めない");
        assert!((l - 17.78).abs() < 0.01, "0.7インチ = 17.78mm でない: {l}");
        assert!((t - 19.05).abs() < 0.01, "{t}");
    }
}

#[cfg(test)]
mod print_setup_roundtrip_tests {
    use super::*;

    #[test]
    fn 印刷設定と印刷範囲がモデル経由で往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].landscape = true;
        b.sheets[0].paper_size = Some(12);
        b.sheets[0].margins_mm = Some((10.0, 10.0, 20.0, 20.0));
        b.sheets[0]
            .print_areas
            .push((Pos::parse("A1").unwrap(), Pos::parse("G30").unwrap()));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let sh = &back.sheets[0];
        assert!(sh.landscape, "向きが往復しない");
        assert_eq!(sh.paper_size, Some(12), "用紙が往復しない");
        let (l, _, t, _) = sh.margins_mm.expect("余白が往復しない");
        assert!((l - 10.0).abs() < 0.05, "{l}");
        assert!((t - 20.0).abs() < 0.05, "{t}");
        assert_eq!(
            sh.print_areas,
            vec![(Pos::parse("A1").unwrap(), Pos::parse("G30").unwrap())],
            "印刷範囲が往復しない"
        );
    }

    #[test]
    fn 原文の知らない属性を消さずに向きだけ変わる() {
        // 拡大縮小(scale)付きの原本を読み、向きだけ変えて保存する
        let b0 = Book::new();
        let mut buf = Cursor::new(Vec::new());
        write(&b0, &mut buf).expect("書けない");
        let mut z = zip::ZipArchive::new(Cursor::new(buf.get_ref().clone())).unwrap();
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        use std::io::Write as _;
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut s = Vec::new();
            f.read_to_end(&mut s).unwrap();
            if name.ends_with("sheet1.xml") {
                let t = String::from_utf8(s).unwrap().replace(
                    "</worksheet>",
                    r#"<pageSetup paperSize="9" scale="85" orientation="landscape"/></worksheet>"#,
                );
                s = t.into_bytes();
            }
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&s).unwrap();
        }
        let original = w.finish().unwrap().into_inner();
        let (mut book, _) = read(Cursor::new(original.clone())).expect("読めない");
        assert!(book.sheets[0].landscape, "原本の向きが読めていない");
        book.sheets[0].landscape = false; // 縦に変える
        let mut out = Cursor::new(Vec::new());
        write_with(&book, Some(Cursor::new(original)), &mut out).expect("書けない");
        let mut z = zip::ZipArchive::new(Cursor::new(out.into_inner())).unwrap();
        let mut s = String::new();
        z.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut s).unwrap();
        assert!(s.contains(r#"scale="85""#), "知らない属性(scale)が消えた");
        assert!(s.contains(r#"orientation="portrait""#), "変えた向きが書かれていない");
        assert!(!s.contains("landscape"), "古い向きが残った");
    }
}

#[cfg(test)]
mod image_roundtrip_tests {
    use super::*;
    use crate::model::SheetImage;

    fn png() -> Vec<u8> {
        // 実体は問わない(読みは復号しない)。PNG の魔法数だけ本物
        let mut d = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        d.extend_from_slice(&[0; 32]);
        d
    }

    #[test]
    fn 挿した画像が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].images_new.push(SheetImage {
            at: Pos::new(2, 3),
            dx_px: 0.0,
            dy_px: 0.0,
            width_px: 300.0,
            height_px: 200.0,
            data: png(),
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let ims = &back.sheets[0].images;
        assert_eq!(ims.len(), 1, "画像が往復しない");
        assert_eq!(ims[0].at, Pos::new(2, 3), "アンカーのセルが違う");
        assert!((ims[0].width_px - 300.0).abs() < 1.0, "幅が違う: {}", ims[0].width_px);
        assert_eq!(ims[0].data, png(), "実体が化けた");
        assert!(back.sheets[0].images_new.is_empty(), "読んだ画像が「挿した側」に入った");
    }

    #[test]
    fn 画像入りの原本に足しても両方残る() {
        // 1枚入りを作る → それを原本にもう1枚足して保存 → 2枚とも読める
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].images_new.push(SheetImage {
            at: Pos::new(0, 0),
            dx_px: 0.0,
            dy_px: 0.0,
            width_px: 100.0,
            height_px: 50.0,
            data: png(),
        });
        let mut buf1 = Cursor::new(Vec::new());
        write(&b, &mut buf1).expect("書けない");
        buf1.set_position(0);
        let (mut b2, _) = read(buf1.clone()).expect("読めない");
        assert_eq!(b2.sheets[0].images.len(), 1);
        b2.sheets[0].images_new.push(SheetImage {
            at: Pos::new(5, 5),
            dx_px: 0.0,
            dy_px: 0.0,
            width_px: 200.0,
            height_px: 100.0,
            data: png(),
        });
        let mut buf2 = Cursor::new(Vec::new());
        buf1.set_position(0);
        write_with(&b2, Some(buf1), &mut buf2).expect("書けない");
        buf2.set_position(0);
        let (b3, _) = read(buf2).expect("読めない");
        assert_eq!(b3.sheets[0].images.len(), 2, "継ぎ足しで枚数が合わない");
        assert!(
            b3.sheets[0].images.iter().any(|im| im.at == Pos::new(5, 5)),
            "足した方のアンカーが無い"
        );
    }
}

#[cfg(test)]
mod print_extras_roundtrip_tests {
    use super::*;

    #[test]
    fn 拡大縮小と改ページとタイトル行が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].print_scale = Some(80);
        b.sheets[0].row_breaks = vec![10, 30];
        b.sheets[0].print_gridlines = true;
        b.sheets[0].print_headings = true;
        b.sheets[0].print_title_rows = Some((0, 1));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let sh = &back.sheets[0];
        assert_eq!(sh.print_scale, Some(80), "scale が往復しない");
        assert_eq!(sh.row_breaks, vec![10, 30], "改ページが往復しない");
        assert!(sh.print_gridlines && sh.print_headings, "printOptions が往復しない");
        assert_eq!(sh.print_title_rows, Some((0, 1)), "タイトル行が往復しない");
    }

    #[test]
    fn 昔ながらの配列数式が往復して正しく計算される() {
        // **これが読めないと古い帳票が静かに違う値になる。**
        // =SUM(A1:A3*B1:B3) は普通に計算すると配列にならない
        let mut b = Book::new();
        for (i, (x, y)) in [(1.0, 10.0), (2.0, 20.0), (3.0, 30.0)].iter().enumerate() {
            b.sheets[0].set(Pos::new(i as u32, 0), Cell::input(&x.to_string()));
            b.sheets[0].set(Pos::new(i as u32, 1), Cell::input(&y.to_string()));
        }
        let at = Pos::parse("D1").unwrap();
        b.sheets[0].set(at, Cell::input("=SUM(A1:A3*B1:B3)"));
        b.sheets[0].cse.insert(at, (1, 1));
        crate::recalc(&mut b.sheets[0]);
        // 1*10 + 2*20 + 3*30 = 140
        assert_eq!(b.sheets[0].get(at).unwrap().value.display(), "140",
                   "配列として計算されていない");

        // xlsx を往復しても配列数式のままか(落ちると次の計算で値が変わる)
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let bytes = buf.into_inner();
        let x = {
            let mut z = zip::ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
            let mut f = z.by_name("xl/worksheets/sheet1.xml").unwrap();
            let mut out = String::new();
            std::io::Read::read_to_string(&mut f, &mut out).unwrap();
            out
        };
        assert!(x.contains(r#"t="array""#), "t=\"array\" が書かれていない");
        assert!(x.contains(r#"ref="D1:D1""#), "覆う範囲が書かれていない");
        let (back, _) = read(Cursor::new(bytes)).expect("読めない");
        assert_eq!(back.sheets[0].cse.get(&at), Some(&(1, 1)), "配列数式の印が往復しない");
        let mut b2 = back;
        crate::recalc(&mut b2.sheets[0]);
        assert_eq!(b2.sheets[0].get(at).unwrap().value.display(), "140",
                   "往復したら値が変わった");
    }

    #[test]
    fn 配列数式は決められた範囲に収まり足りない席はNAになる() {
        let mut b = Book::new();
        for i in 0..3u32 {
            b.sheets[0].set(Pos::new(i, 0), Cell::input(&((i + 1) * 2).to_string()));
        }
        // 3つしか返らない式を5つぶんの範囲に入れた(Excel は #N/A で埋める)
        let at = Pos::parse("C1").unwrap();
        b.sheets[0].set(at, Cell::input("=A1:A3*10"));
        b.sheets[0].cse.insert(at, (5, 1));
        crate::recalc(&mut b.sheets[0]);
        assert_eq!(b.sheets[0].get(at).unwrap().value.display(), "20");
        assert_eq!(b.sheets[0].get(Pos::parse("C3").unwrap()).unwrap().value.display(), "60");
        assert_eq!(
            b.sheets[0].get(Pos::parse("C4").unwrap()).unwrap().value.display(),
            "#N/A",
            "足りない席が埋まっていない"
        );
    }

    #[test]
    fn 読み取り専用の勧めが往復する() {
        // **鍵ではなくお願い。** password は書かない(掛けた振りをしない)
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.read_only_rec = true;
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let bytes = buf.into_inner();
        let wb = {
            let mut z = zip::ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
            let mut f = z.by_name("xl/workbook.xml").unwrap();
            let mut out = String::new();
            std::io::Read::read_to_string(&mut f, &mut out).unwrap();
            out
        };
        assert!(wb.contains(r#"readOnlyRecommended="1""#), "勧めが書かれていない");
        assert!(!wb.contains("workbookPassword"), "掛けてもいない鍵を書いた");
        let (back, _) = read(Cursor::new(bytes)).expect("読めない");
        assert!(back.read_only_rec, "勧めが往復しない");

        // 外したら消える(残ると開くたびに言い続ける)
        let mut b2 = back;
        b2.read_only_rec = false;
        let mut buf2 = Cursor::new(Vec::new());
        write(&b2, &mut buf2).expect("書けない");
        let (back2, _) = read(Cursor::new(buf2.into_inner())).expect("読めない");
        assert!(!back2.read_only_rec, "外したのに残っている");
    }

    #[test]
    fn 同じ名前が二枚にあるときだけシート限定で書く() {
        // **付けないと「ブック全体の名前が2つ」になって開けないファイルに
        // なる。全部に付けるとブック全体の名前がシート限定に落ちる**
        let mut b = Book::new();
        b.sheets.push(crate::Sheet::new("Sheet2".into()));
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[1].set(Pos::parse("A1").unwrap(), Cell::input("y"));
        b.sheets[0].names.push(("売上".into(), "A1:A3".into()));
        b.sheets[1].names.push(("売上".into(), "A1:A5".into())); // 同じ名前
        b.sheets[0].names.push(("税率".into(), "B1".into())); // こちらは1枚だけ

        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let bytes = buf.into_inner();
        let wb = {
            let mut z = zip::ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
            let mut f = z.by_name("xl/workbook.xml").unwrap();
            let mut out = String::new();
            std::io::Read::read_to_string(&mut f, &mut out).unwrap();
            out
        };
        assert_eq!(wb.matches(r#"name="売上""#).count(), 2, "重なった名前が両方書かれていない");
        assert_eq!(
            wb.matches(r#"name="売上" localSheetId="#).count(),
            2,
            "重なった名前にシート限定の印が付いていない"
        );
        assert!(
            wb.contains(r#"name="税率">"#),
            "1枚だけの名前にまで印が付いた(ブック全体の名前が壊れる)"
        );
        // 読み返しても両方が元のシートに戻る
        let (back, _) = read(Cursor::new(bytes)).expect("読めない");
        assert_eq!(back.sheets[0].names.iter().filter(|(n, _)| n == "売上").count(), 1);
        assert_eq!(back.sheets[1].names.iter().filter(|(n, _)| n == "売上").count(), 1);
    }

    #[test]
    fn 型紙は宣言だけが違い中身は読める() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("見積書"));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let x = buf.into_inner();
        let t = to_template(&x).expect("型紙にできない");
        // 宣言が型紙になっている
        let ct = {
            let mut z = zip::ZipArchive::new(Cursor::new(t.clone())).unwrap();
            let mut f = z.by_name("[Content_Types].xml").unwrap();
            let mut out = String::new();
            std::io::Read::read_to_string(&mut f, &mut out).unwrap();
            out
        };
        assert!(ct.contains("spreadsheetml.template.main+xml"), "型紙の宣言が無い");
        assert!(!ct.contains("spreadsheetml.sheet.main+xml"), "ブックの宣言が残っている");
        // **中身は同じ** — 型紙もこちらで開けること
        let (back, _) = read(Cursor::new(t)).expect("型紙が読めない");
        assert_eq!(
            back.sheets[0].get(Pos::parse("A1").unwrap()).unwrap().value.display(),
            "見積書"
        );
    }

    #[test]
    fn 紙に収める指定と縦の改ページが往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].fit_to_w = Some(1);
        b.sheets[0].fit_to_h = None; // 横だけ合わせる(縦は何枚でもよい)
        b.sheets[0].col_breaks = vec![3, 7];
        b.sheets[0].row_breaks = vec![20];
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let sh = &back.sheets[0];
        assert_eq!(sh.fit_to_w, Some(1), "横の枚数が往復しない");
        assert_eq!(sh.fit_to_h, None, "「合わせない」(0)が枚数に化けた");
        // **縦と横を取り違えない。** どちらも <brk> なので混ざりやすい
        assert_eq!(sh.col_breaks, vec![3, 7], "縦の改ページが往復しない");
        assert_eq!(sh.row_breaks, vec![20], "横の改ページに縦が混ざった");
    }
}

#[cfg(test)]
mod shape_roundtrip_tests {
    use super::*;
    use crate::model::SheetShape;

    #[test]
    fn 挿した図形が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].shapes_new.push(SheetShape {
            at: Pos::new(1, 2),
            width_px: 160.0,
            height_px: 100.0,
            kind: "rightArrow".into(),
            fill: Some("FFF2CC".into()),
            line: Some("1B6E3C".into()),
            ..Default::default()
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let sp = &back.sheets[0].shapes;
        assert_eq!(sp.len(), 1, "図形が往復しない");
        assert_eq!(sp[0].kind, "rightArrow");
        assert_eq!(sp[0].at, Pos::new(1, 2));
        assert_eq!(sp[0].fill.as_deref(), Some("FFF2CC"));
        assert_eq!(sp[0].line.as_deref(), Some("1B6E3C"), "線の色が塗りと混ざった");
        assert!((sp[0].width_px - 160.0).abs() < 1.0);
        assert!(back.sheets[0].shapes_new.is_empty());
    }

    #[test]
    fn 回転と反転と線幅と不透明度と影が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].shapes_new.push(SheetShape {
            at: Pos::new(1, 1),
            width_px: 120.0,
            height_px: 80.0,
            kind: "roundRect".into(),
            fill: Some("FFF2CC".into()),
            line: Some("1B6E3C".into()),
            rot: 30.0,
            flip_h: true,
            line_w: 3.0,
            alpha: 0.5,
            shadow: true,
            ..Default::default()
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let sp = &back.sheets[0].shapes;
        assert_eq!(sp.len(), 1, "図形が往復しない");
        assert!((sp[0].rot - 30.0).abs() < 0.01, "回転が往復しない: {}", sp[0].rot);
        assert!(sp[0].flip_h && !sp[0].flip_v, "反転が往復しない");
        assert!((sp[0].line_w - 3.0).abs() < 0.01, "線幅が往復しない: {}", sp[0].line_w);
        assert!((sp[0].alpha - 0.5).abs() < 0.01, "不透明度が往復しない: {}", sp[0].alpha);
        assert!(sp[0].shadow, "影が往復しない");
        // 影の色や alpha が塗り・線に化けていない
        assert_eq!(sp[0].fill.as_deref(), Some("FFF2CC"));
        assert_eq!(sp[0].line.as_deref(), Some("1B6E3C"));
        // 素の図形は既定のまま(余計な性質が付かない)
        let mut b2 = Book::new();
        b2.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b2.sheets[0].shapes_new.push(SheetShape {
            at: Pos::new(0, 0),
            width_px: 100.0,
            height_px: 50.0,
            kind: "rect".into(),
            line: Some("1B6E3C".into()),
            ..Default::default()
        });
        let mut buf2 = Cursor::new(Vec::new());
        write(&b2, &mut buf2).expect("書けない");
        buf2.set_position(0);
        let (back2, _) = read(buf2).expect("読めない");
        let q = &back2.sheets[0].shapes[0];
        assert!(q.rot == 0.0 && !q.flip_h && !q.flip_v && !q.shadow);
        assert!((q.alpha - 1.0).abs() < 0.01 && (q.line_w - 1.5).abs() < 0.01);
    }
}

#[cfg(test)]
mod textbox_spark_roundtrip_tests {
    use super::*;
    use crate::model::SheetShape;

    #[test]
    fn 文字入りの図形と折れ線が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].shapes_new.push(SheetShape {
            at: Pos::new(0, 5),
            width_px: 200.0,
            height_px: 80.0,
            kind: "rect".into(),
            line: Some("7F7F7F".into()),
            text: Some("注意: 締切は8/10 <厳守>".into()),
            ..Default::default()
        });
        b.sheets[0].shapes_new.push(SheetShape {
            at: Pos::new(3, 5),
            width_px: 108.0,
            height_px: 24.0,
            kind: "spark".into(),
            line: Some("1B6E3C".into()),
            points: vec![(0.0, 1.0), (0.5, 0.0), (1.0, 0.6)],
            ..Default::default()
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let sp = &back.sheets[0].shapes;
        assert_eq!(sp.len(), 2, "図形が往復しない: {sp:?}");
        let tb = sp.iter().find(|s| s.kind == "rect").expect("文字箱が無い");
        assert_eq!(tb.text.as_deref(), Some("注意: 締切は8/10 <厳守>"), "文字が化けた");
        let sk = sp.iter().find(|s| s.kind == "spark").expect("折れ線が無い");
        assert_eq!(sk.points.len(), 3);
        assert!((sk.points[1].0 - 0.5).abs() < 0.01 && sk.points[1].1.abs() < 0.01);
    }
}

#[cfg(test)]
mod style_keep_tests {
    use super::*;

    /// `<c r=… s=…>` の対応表(セル → 書式索引)を抜く
    fn smap(xml: &str) -> std::collections::BTreeMap<String, String> {
        let mut m = std::collections::BTreeMap::new();
        for part in xml.split("<c ").skip(1) {
            // 頭に空白を足して、最初の属性も「 名前="」で引けるようにする
            let tag = format!(" {}", &part[..part.find('>').unwrap_or(0)]);
            let g = |k: &str| {
                tag.split(&format!(" {k}=\""))
                    .nth(1)
                    .and_then(|r| r.split('"').next())
                    .map(str::to_string)
            };
            if let (Some(r), Some(s)) = (g("r"), g("s")) {
                m.insert(r, s);
            }
        }
        m
    }

    fn part(zip_bytes: &[u8], name: &str) -> String {
        let mut z = zip::ZipArchive::new(Cursor::new(zip_bytes.to_vec())).unwrap();
        let mut s = String::new();
        use std::io::Read as _;
        z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    /// **実物の様式を開いて保存しただけなら、書式は1字も変わらない。**
    /// styles.xml は据え置き、セルの書式索引も原本のまま
    /// (勝手な書式設定をするな — 発注者 2026-08-06)。
    /// 様式が無い環境では黙って飛ばす
    #[test]
    fn 実物の様式は保存で書式表が変わらない() {
        let src = std::path::Path::new(
            "/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki/実施要領様式7_提案見積書.xlsx",
        );
        let Ok(bytes) = std::fs::read(src) else { return };
        let (book, _) = read(Cursor::new(bytes.clone())).unwrap();
        let mut out = Cursor::new(Vec::new());
        write_with(&book, Some(Cursor::new(bytes.clone())), &mut out).unwrap();
        let out = out.into_inner();
        assert_eq!(
            part(&bytes, "xl/styles.xml"),
            part(&out, "xl/styles.xml"),
            "開いて保存しただけで styles.xml が変わった"
        );
        // セルの書式索引も原本のまま(消えたセルも無い)
        let orig = smap(&part(&bytes, "xl/worksheets/sheet1.xml"));
        let now = smap(&part(&out, "xl/worksheets/sheet1.xml"));
        for (r, s) in &orig {
            assert_eq!(now.get(r), Some(s), "セル {r} の書式索引が変わった");
        }
    }

    /// 書式を1つ触ったら、原本の表はそのままで**末尾に追記**される
    #[test]
    fn 触った書式は追記で受ける() {
        let src = std::path::Path::new(
            "/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki/実施要領様式7_提案見積書.xlsx",
        );
        let Ok(bytes) = std::fs::read(src) else { return };
        let (mut book, _) = read(Cursor::new(bytes.clone())).unwrap();
        // A1 を太字にする(書式を1つだけ触る)
        let p = Pos::parse("A1").unwrap();
        let mut c = book.sheets[0].get(p).cloned().unwrap_or_default();
        c.fmt.bold = true;
        book.sheets[0].set(p, c);
        let mut out = Cursor::new(Vec::new());
        write_with(&book, Some(Cursor::new(bytes.clone())), &mut out).unwrap();
        let out = out.into_inner();
        let orig_styles = part(&bytes, "xl/styles.xml");
        let now_styles = part(&out, "xl/styles.xml");
        // 原本の cellXfs の中身がそっくり残っている(据え置き+追記)
        let orig_xfs = {
            let a = orig_styles.find("<cellXfs").unwrap();
            let a = a + orig_styles[a..].find('>').unwrap() + 1;
            let b = orig_styles.find("</cellXfs>").unwrap();
            orig_styles[a..b].to_string()
        };
        assert!(
            now_styles.contains(&orig_xfs),
            "原本の xf が書き換わった(追記でなく作り直しになっている)"
        );
        // 触っていないセルの索引は変わらない
        let orig_map = smap(&part(&bytes, "xl/worksheets/sheet1.xml"));
        let now_map = smap(&part(&out, "xl/worksheets/sheet1.xml"));
        for (r, s) in &orig_map {
            if r == "A1" {
                continue;
            }
            assert_eq!(now_map.get(r), Some(s), "触っていないセル {r} の索引が動いた");
        }
    }
}

#[cfg(test)]
mod script_roundtrip_tests {
    use super::*;

    #[test]
    fn ブックには関数も手続きも書かない() {
        // 発注者確定 2026-08-09: データとプログラムを1つのファイルにしない。
        // 関数(UDF)も手続きも plugins の .py にある — ブックは何も運ばない
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.scripts.push((
            "関数集計".into(),
            "def 集計(x):\n    return 1 < 2 and x".into(),
        ));
        b.scripts.push(("取り込み".into(), "print('手続き')".into()));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert!(back.scripts.is_empty(), "コードがブックに残った(ファイルが実行の起点になる)");
    }

    #[test]
    fn 古いブックのコードは読めて報告が出て保存で消える() {
        // 黙って落とさない: 開くときに報告し、@export で取り出せる状態にはする
        let mut old = Book::new();
        old.scripts.push(("関数集計".into(), "def 集計(x):\n    return 1 < 2 and x".into()));
        // 古い形の xlsx を手で組む(いまの write はもう joPython を書かないため)
        let mut buf = Cursor::new(Vec::new());
        write(&old, &mut buf).expect("書けない");
        buf.set_position(0);
        let with_py = 古い形にjoPythonを足す(buf.into_inner(), &old.scripts);

        let (back, rep) = read(Cursor::new(with_py.clone())).expect("読めない");
        assert_eq!(back.scripts.len(), 1, "古いブックのコードが読めない(@export できない)");
        assert!(back.scripts[0].1.contains("1 < 2"), "コードの逃がしが壊れた");
        assert!(
            rep.unsupported.iter().any(|(n, _)| n.contains("ブックに載っていた Python")),
            "黙って落とした(報告が無い): {:?}",
            rep.unsupported
        );
        // 保存し直すと消える(原本を渡しても持ち越さない)
        let mut buf2 = Cursor::new(Vec::new());
        write_with(&back, Some(Cursor::new(with_py)), &mut buf2).expect("書けない");
        buf2.set_position(0);
        let (b3, _) = read(buf2).expect("読めない");
        assert!(b3.scripts.is_empty(), "保存し直してもコードが残った");
    }

    /// 試験のための小道具 — zip に xl/joPython.xml を足した「古い形」を作る
    #[allow(non_snake_case)]
    fn 古い形にjoPythonを足す(bytes: Vec<u8>, scripts: &[(String, String)]) -> Vec<u8> {
        let mut zin = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip が読めない");
        let mut out = Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut out);
            let opts: zip::write::FileOptions<()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for i in 0..zin.len() {
                let mut f = zin.by_index(i).expect("項目が読めない");
                let name = f.name().to_string();
                let mut v = Vec::new();
                f.read_to_end(&mut v).expect("中身が読めない");
                zw.start_file(name, opts).expect("書けない");
                zw.write_all(&v).expect("書けない");
            }
            let mut sx = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<joPython>",
            );
            for (n, code) in scripts {
                sx.push_str(&format!("<script name=\"{}\">{}</script>", esc(n), esc(code)));
            }
            sx.push_str("</joPython>");
            zw.start_file("xl/joPython.xml", opts).expect("書けない");
            zw.write_all(sx.as_bytes()).expect("書けない");
            zw.finish().expect("閉じられない");
        }
        out.into_inner()
    }

    #[test]
    fn ブックの情報が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.props.creator = "日本フネン".into();
        b.props.title = "見積 <2026>".into();
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert_eq!(back.props.creator, "日本フネン", "作成者が往復しない");
        assert_eq!(back.props.title, "見積 <2026>", "逃がしが往復しない");
        assert_eq!(back.props.subject, "", "空欄は空欄のまま");
    }

    #[test]
    fn 図形のずらしが往復する() {
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(crate::model::SheetShape {
            at: Pos::parse("B2").unwrap(),
            width_px: 100.0,
            height_px: 50.0,
            kind: "rect".into(),
            fill: None,
            line: Some("1B6E3C".into()),
            dx_px: 30.0,
            dy_px: 12.0,
            ..Default::default()
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let sp = &back.sheets[0].shapes[0];
        assert!((sp.dx_px - 30.0).abs() < 0.2, "colOff が往復しない: {}", sp.dx_px);
        assert!((sp.dy_px - 12.0).abs() < 0.2, "rowOff が往復しない: {}", sp.dy_px);
    }

    #[test]
    fn テーマ色が往復し配色を変えると追従する() {
        let mut b = Book::new();
        b.theme = crate::theme::OFFICE.iter().map(|s| s.to_string()).collect();
        let p = Pos::parse("A1").unwrap();
        let mut c = Cell::input("色");
        // アクセント1(4番)を明るくした色を、由来つきで持つ
        c.fmt.color_theme = Some((4, 400));
        c.fmt.color = Some(crate::theme::resolve(&b.theme, 4, 0.4));
        b.sheets[0].set(p, c);
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let f = &back.sheets[0].get(p).unwrap().fmt;
        assert_eq!(f.color_theme, Some((4, 400)), "テーマ由来が往復しない");
        assert_eq!(f.color.as_deref(), Some(crate::theme::resolve(&back.theme, 4, 0.4).as_str()), "色が解けない");
        // 配色を変えると、同じ由来から別の色が出る(追従の土台)
        let warm = crate::theme::SCHEMES[1].1;
        let after = crate::theme::resolve(
            &warm.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            4,
            0.4,
        );
        assert_ne!(after, f.color.clone().unwrap(), "配色を変えても色が変わらない");
    }

    #[test]
    fn 表オブジェクトと右横書きが往復する() {
        let mut b = Book::new();
        for (r, row) in [["部署", "金額"], ["営業", "100"]].iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                b.sheets[0].set(Pos::new(r as u32, c as u32), Cell::input(v));
            }
        }
        b.sheets[0].tables.push(crate::model::TableDef {
            name: "売上表".into(),
            a: Pos::new(0, 0),
            b: Pos::new(1, 1),
            totals: true,
            banded_cols: true,
            first_col: true,
            ..Default::default()
        });
        b.sheets[0].rtl = true;
        let p = Pos::parse("A1").unwrap();
        let mut c = b.sheets[0].get(p).cloned().unwrap();
        c.fmt.rtl_text = true;
        b.sheets[0].set(p, c);
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let t = back.sheets[0].tables.first().expect("表が往復しない");
        assert_eq!(t.name, "売上表");
        assert_eq!((t.a, t.b), (Pos::new(0, 0), Pos::new(1, 1)), "範囲が違う");
        assert!(t.header && t.totals && t.first_col && t.banded_cols, "性質が往復しない");
        assert!(back.sheets[0].rtl, "右から左が往復しない");
        assert!(back.sheets[0].get(p).unwrap().fmt.rtl_text, "右横書きが往復しない");
    }

    #[test]
    fn 固定枠と画面の見え方が往復する() {
        use crate::model::FreezePane;
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("見出し"));
        // 見出しの1行と左の1列を止める。右から左と重ねて、同じ sheetView に
        // 両方が載ること(片方が片方を追い出さないこと)も見る
        b.sheets[0].freeze = Some(FreezePane { frozen_rows: 1, frozen_columns: 1 });
        b.sheets[0].rtl = true;
        b.sheets[0].show_gridlines = Some(false);
        b.sheets[0].show_formulas = Some(true);
        b.sheets[0].zoom_scale = Some(85);
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let sh = &back.sheets[0];
        assert_eq!(
            sh.freeze,
            Some(FreezePane { frozen_rows: 1, frozen_columns: 1 }),
            "固定枠が往復しない"
        );
        assert!(sh.rtl, "固定枠と一緒だと右から左が落ちる");
        assert_eq!(sh.show_gridlines, Some(false), "格子線が往復しない");
        assert_eq!(sh.show_formulas, Some(true), "式の表示が往復しない");
        assert_eq!(sh.zoom_scale, Some(85), "表示倍率が往復しない");
    }

    #[test]
    fn 見出し行を固定した実物の形を読める() {
        // **Excel が書く sheetView は `<selection>` や `<pane>` を抱えるので
        // Start で来る。** Empty でしか見ていなかったので、固定枠だけでなく
        // rtl も実物では読めていなかった — その形を型紙にして押さえる
        let b = Book::new();
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let mut z = zip::ZipArchive::new(Cursor::new(buf.get_ref().clone())).unwrap();
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        use std::io::{Read as _, Write as _};
        let mut replaced = false;
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut s = Vec::new();
            f.read_to_end(&mut s).unwrap();
            if name.ends_with("sheet1.xml") {
                let t = String::from_utf8(s).unwrap().replace(
                    r#"<sheetViews><sheetView workbookViewId="0"/></sheetViews>"#,
                    r#"<sheetViews><sheetView tabSelected="1" rightToLeft="1" showGridLines="0" zoomScale="85" workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/><selection pane="bottomLeft" activeCell="A2" sqref="A2"/></sheetView></sheetViews>"#,
                );
                replaced = true;
                s = t.into_bytes();
            }
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&s).unwrap();
        }
        assert!(replaced, "型紙を差す先が無い(書き出しの形が変わった)");
        let out = w.finish().unwrap();
        let (back, _) = read(Cursor::new(out.into_inner())).expect("読めない");
        let sh = &back.sheets[0];
        assert_eq!(
            sh.freeze,
            Some(crate::model::FreezePane { frozen_rows: 1, frozen_columns: 0 }),
            "見出し行の固定が読めない"
        );
        assert!(sh.rtl, "子を持つ sheetView の rtl が読めない");
        assert_eq!(sh.show_gridlines, Some(false), "格子線が読めない");
        assert_eq!(sh.zoom_scale, Some(85), "表示倍率が読めない");
    }

    #[test]
    fn 掴んで動かす分割は固定枠にしない() {
        // state="split" の pane は仕切りであって固定ではない。しかも xSplit は
        // 列数ではなく 1/20 ポイントの座標なので、固定として読むと
        // 途方もない列数になる — 撥ねていることを押さえる
        let b = Book::new();
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let mut z = zip::ZipArchive::new(Cursor::new(buf.get_ref().clone())).unwrap();
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        use std::io::{Read as _, Write as _};
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut s = Vec::new();
            f.read_to_end(&mut s).unwrap();
            if name.ends_with("sheet1.xml") {
                let t = String::from_utf8(s).unwrap().replace(
                    r#"<sheetView workbookViewId="0"/>"#,
                    r#"<sheetView workbookViewId="0"><pane xSplit="2310" ySplit="1170" topLeftCell="C4" activePane="bottomRight"/></sheetView>"#,
                );
                s = t.into_bytes();
            }
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&s).unwrap();
        }
        let out = w.finish().unwrap();
        let (back, _) = read(Cursor::new(out.into_inner())).expect("読めない");
        assert_eq!(back.sheets[0].freeze, None, "分割を固定枠として読んでいる");
    }

    #[test]
    fn しまい込んだ表示設定の固定枠は拾わない() {
        // customSheetView は「誰かが昔しまい込んだ表示設定」で、そこにも pane が
        // ぶら下がる。いまの画面の固定枠として読むと、開いた人が設定した覚えの
        // ない場所で表が止まる
        let b = Book::new();
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let mut z = zip::ZipArchive::new(Cursor::new(buf.get_ref().clone())).unwrap();
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        use std::io::{Read as _, Write as _};
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut s = Vec::new();
            f.read_to_end(&mut s).unwrap();
            if name.ends_with("sheet1.xml") {
                let t = String::from_utf8(s).unwrap().replace(
                    "</worksheet>",
                    r#"<customSheetViews><customSheetView guid="{00000000-0000-0000-0000-000000000001}"><pane xSplit="3" ySplit="7" topLeftCell="D8" activePane="bottomRight" state="frozen"/></customSheetView></customSheetViews></worksheet>"#,
                );
                s = t.into_bytes();
            }
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&s).unwrap();
        }
        let out = w.finish().unwrap();
        let (back, _) = read(Cursor::new(out.into_inner())).expect("読めない");
        assert_eq!(back.sheets[0].freeze, None, "しまい込んだ表示設定の固定枠を拾っている");
    }

    #[test]
    fn 表を外すと部品も宣言も消える() {
        // 表つきで書いたものを読み、表を外して書き直す(範囲に変換の道)
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].tables.push(crate::model::TableDef {
            a: Pos::new(0, 0),
            b: Pos::new(1, 1),
            ..Default::default()
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).unwrap();
        buf.set_position(0);
        let (mut back, _) = read(buf).unwrap();
        assert_eq!(back.sheets[0].tables.len(), 1);
        back.sheets[0].tables.clear();
        // 原本を持ち越しながら書き直す(実際の保存と同じ道)
        let orig = {
            let mut b2 = Cursor::new(Vec::new());
            write(&b, &mut b2).unwrap();
            b2.set_position(0);
            b2
        };
        let mut out = Cursor::new(Vec::new());
        write_with(&back, Some(orig), &mut out).unwrap();
        let bytes = out.into_inner();
        let (again, _) = read(Cursor::new(bytes.clone())).unwrap();
        assert!(again.sheets[0].tables.is_empty(), "外した表が残っている");
        // 宣言も残っていない(残ると Excel が壊れたと言う)
        let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut ct = String::new();
        use std::io::Read as _;
        z.by_name("[Content_Types].xml").unwrap().read_to_string(&mut ct).unwrap();
        assert!(!ct.contains("/xl/tables/"), "Content_Types に宣言が残っている");
    }

    #[test]
    fn 隠しシートと下付きと回転が往復する() {
        let mut b = Book::new();
        b.sheets.push(crate::Sheet::new("裏"));
        b.sheets[1].hidden = true;
        let p = Pos::parse("A1").unwrap();
        let mut c = Cell::input("x");
        c.fmt.subscript = true;
        c.fmt.rotation = Some(255);
        c.fmt.align = crate::model::HAlign::Justify;
        b.sheets[0].set(p, c);
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert!(back.sheets[1].hidden, "隠しシートが往復しない");
        let f = &back.sheets[0].get(p).unwrap().fmt;
        assert!(f.subscript, "下付きが往復しない");
        assert_eq!(f.rotation, Some(255), "回転が往復しない");
        assert_eq!(f.align, crate::model::HAlign::Justify, "両端揃えが往復しない");
    }

    #[test]
    fn シートの保護が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].protected = true;
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert!(back.sheets[0].protected, "保護が往復しない");
    }

    #[test]
    fn 耳の色が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].tab_color = Some("FFC00000".into());
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert_eq!(
            back.sheets[0].tab_color.as_deref(),
            Some("FFC00000"),
            "耳の色が往復しない"
        );
    }

    #[test]
    fn グループ化と畳みが往復する() {
        let mut b = Book::new();
        let s = &mut b.sheets[0];
        s.set(Pos::parse("A1").unwrap(), Cell::input("見出し"));
        s.set(Pos::parse("A5").unwrap(), Cell::input("x"));
        s.row_outline.insert(1, 1);
        s.row_outline.insert(2, 2);
        s.row_outline.insert(3, 1); // 行4: 中身の無い行(それでも消えない)
        s.row_hidden.insert(2);
        s.col_outline.insert(2, 1);
        s.col_outline.insert(3, 1);
        s.col_hidden.insert(3);
        s.col_width.insert(2, 20.0);
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let s = &back.sheets[0];
        assert_eq!(s.row_outline.get(&1), Some(&1));
        assert_eq!(s.row_outline.get(&2), Some(&2));
        assert_eq!(s.row_outline.get(&3), Some(&1), "中身の無い行の深さが消えた");
        assert!(s.row_hidden.contains(&2), "畳んだ行が開いてしまう");
        assert_eq!(s.col_outline.get(&2), Some(&1));
        assert!(s.col_hidden.contains(&3));
        assert_eq!(s.col_width.get(&2), Some(&20.0), "幅と深さの同居で幅が消えた");
    }

    #[test]
    fn ピボットの指図が往復する() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.pivots.push(crate::model::PivotDef {
            sheet: "Sheet1".into(),
            src: (Pos::parse("A1").unwrap(), Pos::parse("C5").unwrap()),
            rows_sel: vec!["部署".into(), "係".into()],
            cols_sel: vec!["月".into()],
            value: "金額 <税込>".into(),
            agg: "平均".into(),
            totals: true,
            subtotals: false,
            blank_rows: true,
            compact: false,
            dest: Pos::parse("E1").unwrap(),
            show_as: String::new(),
            size: (4, 3),
            hide: Vec::new(),
            style: "緑".into(),
            name: "ピボットテーブル1".into(),
            vfilter: None,
            group_by: Vec::new(),
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf.clone()).expect("読めない");
        assert_eq!(back.pivots.len(), 1, "指図が往復しない");
        assert_eq!(back.pivots[0], b.pivots[0], "中身が変わった: {:?}", back.pivots[0]);
        // もう一往復(古い部品と二重にならない)
        let mut buf2 = Cursor::new(Vec::new());
        buf.set_position(0);
        write_with(&back, Some(buf), &mut buf2).expect("書けない");
        buf2.set_position(0);
        let (b3, _) = read(buf2).expect("読めない");
        assert_eq!(b3.pivots.len(), 1, "二往復で二重になった");
    }
    /// 名前の定義に属性を差し込んだ xlsx を作って読み直す。
    /// 既定値まで書く書き手(LibreOffice)を真似るための道具
    fn 名前に属性をつけて読み直す(extra: &str) -> Book {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("1"));
        b.sheets[0].names.push(("名前つき".into(), "A1:A5".into()));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        // zip の中の workbook.xml の definedName に属性を差し込む
        let mut z = zip::ZipArchive::new(Cursor::new(buf.get_ref().clone())).unwrap();
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        use std::io::{Read as _, Write as _};
        let mut hit = false;
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut s = Vec::new();
            f.read_to_end(&mut s).unwrap();
            if name == "xl/workbook.xml" {
                let t = String::from_utf8(s).unwrap().replace(
                    "<definedName name=\"名前つき\"",
                    &format!("<definedName {extra} name=\"名前つき\""),
                );
                hit = t.contains(extra);
                s = t.into_bytes();
            }
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&s).unwrap();
        }
        assert!(hit, "属性を差し込めなかった(書き出しの形が変わった?)");
        let out = w.finish().unwrap();
        read(Cursor::new(out.into_inner())).expect("読めない").0
    }

    #[test]
    fn 既定値つきの名前が式から引ける() {
        // LibreOffice は名前の定義すべてに真偽の属性を**既定値でも**書く。
        // 属性の数で「単純か」を決めていたので、中身は Excel と同じなのに
        // 全部「理解できない名前」へ落ち、式から引くと #NAME? だった
        let back = 名前に属性をつけて読み直す(r#"function="false" hidden="false" vbProcedure="false""#);
        assert_eq!(
            back.sheets[0].names,
            vec![("名前つき".to_string(), "A1:A5".to_string())],
            "偽の属性で名前が使えなくなった(names_raw: {:?})",
            back.names_raw
        );
        assert!(back.names_raw.is_empty(), "単純な名前が原文へ回った: {:?}", back.names_raw);
    }

    #[test]
    fn 隠し名前は原文のまま持ち越す() {
        // hidden="1" は**立っている**ので単純ではない。式からは引かせず、
        // 捨てもせず原文で持ち越す(今までどおり)
        let back = 名前に属性をつけて読み直す(r#"hidden="1""#);
        assert!(back.sheets[0].names.is_empty(), "隠し名前が式から引けてしまう");
        assert_eq!(back.names_raw.len(), 1, "隠し名前を落とした: {:?}", back.names_raw);
        assert!(
            back.names_raw[0].contains("hidden=\"1\""),
            "原文が変わった: {}",
            back.names_raw[0]
        );
    }
}
/// シートの割り当て — `<sheet>` の `r:id` を rels で解いているか。
///
/// **2026-08-09 の [大]。** 部品を文字列で並べ替えて位置で対にしていたので、
/// `sheet10.xml` が `sheet2.xml` より前に来て、シートが 10 枚以上ある帳面は
/// 中身が丸ごと入れ替わっていた(日銀の資金循環統計 30 枚で発覚)。
/// **黙って別のシートの中身を返す**のがいちばん悪い型なので、受入試験を置く。
///
/// 自分で書く xlsx は `sheet1..9` しか作らないので、この形は
/// **こちらの答案では永久に出ない** — 型紙を手で組む
#[cfg(test)]
mod sheet_rid {
    use crate::model::{Pos, Value};
    use std::io::Write;

    /// 12 枚。`<sheet>` の並びと部品の番号を**わざと食い違わせる**。
    ///
    /// `<sheet name="表{i}" r:id="rId{i}"/>` を i=1..12 の順に並べ、
    /// rels では `rId{i}` → `sheet{13-i}.xml`(逆順)へ向ける。
    /// 各部品の A1 には**自分の部品番号**を書いてあるので、
    /// 取り違えれば値で分かる
    fn 型紙() -> Vec<u8> {
        const N: usize = 12;
        let mut buf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let o: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            let mut put = |z: &mut zip::ZipWriter<_>, name: &str, s: &str| {
                z.start_file(name, o).unwrap();
                z.write_all(s.as_bytes()).unwrap();
            };
            let ct: String = (1..=N)
                .map(|i| format!(r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#))
                .collect();
            put(&mut z, "[Content_Types].xml", &super::CT.replace("__SHEETS__", &ct));
            put(&mut z, "_rels/.rels", super::RELS);
            let sheets: String = (1..=N)
                .map(|i| format!(r#"<sheet name="表{i}" sheetId="{i}" r:id="rId{i}"/>"#))
                .collect();
            put(&mut z, "xl/workbook.xml", &format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="{}" xmlns:r="{}"><sheets>{sheets}</sheets></workbook>"#,
                super::NS, super::RNS));
            // **逆順に向ける** — rId の順も部品の番号も当てにならない形
            let rels: String = (1..=N)
                .map(|i| format!(
                    r#"<Relationship Id="rId{i}" Type="{}/worksheet" Target="worksheets/sheet{}.xml"/>"#,
                    super::RNS, N + 1 - i))
                .collect();
            put(&mut z, "xl/_rels/workbook.xml.rels", &format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#));
            for p in 1..=N {
                put(&mut z, &format!("xl/worksheets/sheet{p}.xml"), &format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="{}"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>部品{p}</t></is></c></row></sheetData></worksheet>"#,
                    super::NS));
            }
            z.finish().unwrap();
        }
        buf
    }

    fn a1(sh: &crate::Sheet) -> String {
        match sh.get(Pos { row: 0, col: 0 }).map(|c| c.value.clone()) {
            Some(Value::Text(t)) => t,
            v => panic!("A1 が文字列でない: {v:?}"),
        }
    }

    #[test]
    fn r_idで解いた部品を読む() {
        let (book, _) = crate::xlsx::read(std::io::Cursor::new(型紙())).unwrap();
        assert_eq!(book.sheets.len(), 12, "シートの枚数");
        for (i, sh) in book.sheets.iter().enumerate() {
            // 並びは `<sheet>` の順のまま
            assert_eq!(sh.name, format!("表{}", i + 1), "{i} 枚目の名前");
            // 中身は rels の指す部品(逆順)
            assert_eq!(a1(sh), format!("部品{}", 12 - i), "{} の中身が別のシートの物", sh.name);
        }
    }

    #[test]
    fn 文字列の並べ替えに戻っていない() {
        // 文字列で並べると sheet10 が sheet2 より前に来る。
        // その狂い方(表2 に 部品10 系の中身)を名指しで撥ねる
        let (book, _) = crate::xlsx::read(std::io::Cursor::new(型紙())).unwrap();
        assert_eq!(a1(&book.sheets[1]), "部品11", "表2 が文字列の並べ替えの中身を掴んでいる");
    }

    #[test]
    fn 往復してもシートの中身が動かない() {
        // 書き出しは部品を並び順に振り直すので、**ブックの rels の的も
        // 向け直さないと**、開き直したときに別のシートを指す
        let 原本 = 型紙();
        let (book, _) = crate::xlsx::read(std::io::Cursor::new(原本.clone())).unwrap();
        let mut out = Vec::new();
        crate::xlsx::write_with(&book, Some(std::io::Cursor::new(&原本)), std::io::Cursor::new(&mut out))
            .unwrap();
        let (back, _) = crate::xlsx::read(std::io::Cursor::new(&out)).unwrap();
        assert_eq!(back.sheets.len(), book.sheets.len(), "枚数が変わった");
        for (before, after) in book.sheets.iter().zip(&back.sheets) {
            assert_eq!(after.name, before.name, "名前の並びが変わった");
            assert_eq!(a1(after), a1(before), "{} の中身が別のシートへ移った", before.name);
        }
    }

    #[test]
    fn 往復した帳面の部品と宣言が食い違わない() {
        // 的の向け直しで、宣言(Content_Types)と rels と部品の三つが揃うこと。
        // ずれていると Excel が「修復」に入る
        let 原本 = 型紙();
        let (book, _) = crate::xlsx::read(std::io::Cursor::new(原本.clone())).unwrap();
        let mut out = Vec::new();
        crate::xlsx::write_with(&book, Some(std::io::Cursor::new(&原本)), std::io::Cursor::new(&mut out))
            .unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&out)).unwrap();
        let mut ct = String::new();
        let mut rels = String::new();
        {
            use std::io::Read;
            z.by_name("[Content_Types].xml").unwrap().read_to_string(&mut ct).unwrap();
            z.by_name("xl/_rels/workbook.xml.rels").unwrap().read_to_string(&mut rels).unwrap();
        }
        for i in 1..=12 {
            let part = format!("xl/worksheets/sheet{i}.xml");
            assert!(z.by_name(&part).is_ok(), "{part} が無い");
            assert!(ct.contains(&format!(r#"PartName="/{part}""#)), "{part} の宣言が無い");
            // `<sheet>` の i 枚目(rId{i})は i 番の部品を指すこと
            assert!(
                rels.contains(&format!(r#"Id="rId{i}" Type="{}/worksheet" Target="worksheets/sheet{i}.xml""#, super::RNS)),
                "rId{i} の的が sheet{i}.xml へ向いていない: {rels}"
            );
        }
        // 宣言が余っていない(原本の番号を持ち越していない)
        assert_eq!(ct.matches(r#"PartName="/xl/worksheets/"#).count(), 12, "シートの宣言の数");
    }

    #[test]
    fn r_idが無ければ数として並べ替える() {
        // 控えの道。**文字列**で並べると sheet10 が sheet2 より前に来る
        let mut buf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let o: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            let mut put = |z: &mut zip::ZipWriter<_>, name: &str, s: &str| {
                z.start_file(name, o).unwrap();
                z.write_all(s.as_bytes()).unwrap();
            };
            put(&mut z, "_rels/.rels", super::RELS);
            // r:id を書かない(古い書き手や壊れた帳面の形)
            let sheets: String =
                (1..=12).map(|i| format!(r#"<sheet name="表{i}" sheetId="{i}"/>"#)).collect();
            put(&mut z, "xl/workbook.xml", &format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="{}" xmlns:r="{}"><sheets>{sheets}</sheets></workbook>"#,
                super::NS, super::RNS));
            for p in 1..=12 {
                put(&mut z, &format!("xl/worksheets/sheet{p}.xml"), &format!(
                    r#"<worksheet xmlns="{}"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>部品{p}</t></is></c></row></sheetData></worksheet>"#,
                    super::NS));
            }
            z.finish().unwrap();
        }
        let (book, _) = crate::xlsx::read(std::io::Cursor::new(buf)).unwrap();
        assert_eq!(book.sheets.len(), 12);
        for (i, sh) in book.sheets.iter().enumerate() {
            assert_eq!(a1(sh), format!("部品{}", i + 1), "{} が数の順で対になっていない", sh.name);
        }
    }
}
