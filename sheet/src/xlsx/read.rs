//! **xlsx を読む。** 原本の形をそのまま模型へ写す。
//!
//! 読めないものは黙って落とさず `Report` に積む(ooxml と同じ作法)。

use std::io::{Read, Seek};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::model::{Book, Cell, Pos, Sheet, Value};

use super::write::{esc, split_defined, SID_ATTR};

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

pub(super) fn local(n: &[u8]) -> &[u8] {
    match n.iter().position(|b| *b == b':') { Some(i) => &n[i + 1..], None => n }
}
pub(super) fn attr(e: &BytesStart, want: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (local(a.key.as_ref()) == want.as_bytes())
            .then(|| String::from_utf8_lossy(&a.value).to_string())
    })
}

/// attr の実体参照(&lt; 等)を戻す版。自由な文字が入る属性(名前の類い)用
pub(super) fn attr_un(e: &BytesStart, want: &str) -> Option<String> {
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
pub(super) fn parse_shared(xml: &str) -> (Vec<String>, Vec<Option<String>>) {
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
pub(super) fn merge(e: &quick_xml::events::BytesStart, sh: &mut Sheet) {
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
pub(super) fn sheet_view(e: &quick_xml::events::BytesStart, sh: &mut Sheet) {
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
pub(super) fn pane(e: &quick_xml::events::BytesStart, sh: &mut Sheet) {
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

/// **`<c>` がそこに置かれていたことだけを控える**([`Sheet::seen`])。
///
/// 値も書式も無いセル(`<c r="D1" s="0"/>`)は `cells` に入れない — 入れると
/// 「中身のある範囲」を意味する `extent` が狂う。だがシートの大きさとしては
/// 数える。**要素があるのは、書き手がそこまで書いたということ。**
///
/// 落とすと、呼ぶ側が正しく要求した範囲を「シートの外」と断ることになる。
pub(super) fn saw_cell(sh: &mut Sheet, p: Option<Pos>) {
    let Some(p) = p else { return };
    let (r, c) = sh.seen.unwrap_or((0, 0));
    sh.seen = Some((r.max(p.row + 1), c.max(p.col + 1)));
}

/// `<row r="3" ht="27.5" customHeight="1" outlineLevel="1" hidden="1">` —
/// 指定のある行だけ持つ(高さ・グループ化の深さ・畳み)。
pub(super) fn row_height(e: &quick_xml::events::BytesStart, sh: &mut Sheet) {
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
    // **畳んである(アウトラインの「−」)。** hidden とは別で、
    // 畳んだ親の行そのものは見えている
    if matches!(attr(e, "collapsed").as_deref(), Some("1") | Some("true")) {
        sh.row_collapsed.insert(r0);
    }
}

/// `<col min="1" max="3" width="12.5"/>` — min..=max は1始まり。
///
/// 全列に近い指定(既定幅)は展開しない。1列ずつに割ると
/// 16,384 個の col になって保存が肥大する。
pub(super) fn col_width(e: &quick_xml::events::BytesStart, sh: &mut Sheet) {
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
    // **畳んである(アウトラインの「−」)。** hidden とは別
    let collapsed = matches!(attr(e, "collapsed").as_deref(), Some("1") | Some("true"));
    if (level > 0 || hidden || collapsed) && max - min <= 1000.0 {
        for c in (min as u32)..=(max as u32) {
            if c >= 1 {
                if level > 0 {
                    sh.col_outline.insert(c - 1, level);
                }
                if hidden {
                    sh.col_hidden.insert(c - 1);
                }
                if collapsed {
                    sh.col_collapsed.insert(c - 1);
                }
            }
        }
    }
}

/// styles.xml の dxfs(条件付き書式の見た目)→ `CondLook` の列。
///
/// **飾りは三択。** `<b/>` は太字にする、`<b val="0"/>` は太字を外す、
/// 書いていなければ触らない。`val` の既定は true(xlsx の約束)なので、
/// 属性が無ければ Some(true)。
///
/// 下線 `<u/>` は `val="none"` のときだけ「外す」— `single`・`double` は
/// どれも「引く」に畳む(こちらは太さの別を持たない)
pub(super) fn parse_dxfs(xml: &str) -> Vec<crate::model::CondLook> {
    use crate::model::CondLook;
    let mut r = Reader::from_str(xml);
    let mut out: Vec<CondLook> = Vec::new();
    let mut buf = Vec::new();
    let (mut in_dxfs, mut in_dxf, mut in_font, mut in_fill) = (false, false, false, false);
    let mut cur = CondLook::default();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"dxfs" => in_dxfs = true,
                b"dxf" if in_dxfs => {
                    in_dxf = true;
                    cur = CondLook::default();
                }
                b"font" if in_dxf => in_font = true,
                b"fill" if in_dxf => in_fill = true,
                b"color" if in_font => {
                    cur.color = attr(&e, "rgb").map(|v| {
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
                    if c.is_some() && (local(e.name().as_ref()) == b"bgColor" || cur.fill.is_none()) {
                        cur.fill = c;
                    }
                }
                // 飾り。`val` が無ければ true(xlsx の既定)
                b"b" | b"i" | b"strike" if in_font => {
                    let on = attr(&e, "val").map(|v| v != "0" && v != "false").unwrap_or(true);
                    match local(e.name().as_ref()) {
                        b"b" => cur.bold = Some(on),
                        b"i" => cur.italic = Some(on),
                        _ => cur.strike = Some(on),
                    }
                }
                b"u" if in_font => {
                    // 太さの別(single/double/…)は持たない。none だけが「外す」
                    let on = attr(&e, "val").map(|v| v != "none").unwrap_or(true);
                    cur.underline = Some(on);
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
pub(super) fn parse_rels(xml: &str) -> Vec<(String, String, String, bool)> {
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
pub(super) enum DrawKind {
    /// 画像(r:embed)
    Image(String),
    /// 図形。中身(種類・色・文字・回転・線幅…)は詰めてあり、
    /// 置き場所と大きさ(at / width / height / dx / dy)は受け手が埋める
    Shape(Box<crate::model::SheetShape>),
    /// **グラフ。中身は持たない。**
    ///
    /// officework はグラフの模型を持たない — 描くのは matplotlib で、
    /// 出来上がりは画像として置く(発注者確定)。だから系列も軸も読まない。
    ///
    /// **それでも、在ったことは言う。** 家訓は「読めなかった物は黙って
    /// 落とさない」で、**「持たない」と「黙って捨てる」は別のこと**
    /// (2026-08-11。リッチテキストで同じ区別をしたのと同じ形)。
    /// 保存では原本の drawing がそのまま持ち越されるので、**壊れはしない。**
    Chart,
}

/// drawing(xl/drawings/drawingN.xml)から、画像と図形のアンカーを拾う。
/// 返すのは (置き場所のセル, 幅EMU, 高さEMU, 中身)。
/// `xl/tables/tableN.xml` を読む。範囲が読めなければ None(黙って作らない)。
pub(super) fn parse_table(xml: &str) -> Option<crate::model::TableDef> {
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
        // **`table/@name` と `tableStyleInfo/@name` は別物。** 前者は
        // `Table1` のような識別子、後者は `TableStyleMedium2` のような見た目
        style: attr_of("tableStyleInfo", "name"),
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

/// テキストボックスの組み方を1つの要素から拾う(`bodyPr` / `pPr` / `rPr` /
/// 箇条書きの印)。**Start と Empty の両方から呼ぶ**ので1箇所にまとめた。
pub(super) fn text_fmt_attr(e: &BytesStart, tf: &mut crate::model::TextFmt) {
    match local(e.name().as_ref()) {
        b"bodyPr" => {
            tf.anchor = match attr(e, "anchor").as_deref() {
                Some("ctr") => crate::model::TextAnchor::Middle,
                Some("b") => crate::model::TextAnchor::Bottom,
                _ => crate::model::TextAnchor::Top,
            };
            // 縦組みは vert が横以外のとき。**種類は問わない** —
            // eaVert も vert270 もこちらは1つの縦組みで見せる
            tf.vertical = attr(e, "vert").is_some_and(|v| v != "horz");
        }
        b"pPr" => {
            tf.align = match attr(e, "algn").as_deref() {
                Some("ctr") => crate::model::HAlign::Center,
                Some("r") => crate::model::HAlign::Right,
                Some("just") => crate::model::HAlign::Justify,
                _ => crate::model::HAlign::General,
            };
        }
        b"buChar" => tf.bullet = Some(false),
        b"buAutoNum" => tf.bullet = Some(true),
        b"buNone" => tf.bullet = None,
        b"rPr" => {
            tf.strike = attr(e, "strike").is_some_and(|v| v != "noStrike");
            // baseline は千分率。正が上付き、負が下付き
            let base = attr(e, "baseline").and_then(|v| v.parse::<i32>().ok());
            tf.sup = base.is_some_and(|b| b > 0);
            tf.sub = base.is_some_and(|b| b < 0);
        }
        _ => {}
    }
}

pub(super) fn parse_drawing_anchors(xml: &str) -> Vec<(Pos, i64, i64, i64, i64, DrawKind)> {
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
    // テキストボックスの組み方(bodyPr / pPr / rPr から拾う)
    let mut tfmt = crate::model::TextFmt::default();
    let mut pts: Vec<crate::model::PathPoint> = Vec::new();
    // 曲線の3つ組を貯める場所(cubicBezTo の中だけ)
    let mut in_bez = false;
    let mut bez: Vec<(f32, f32)> = Vec::new();
    // 次に来る点が新しい輪郭の始まりか(moveTo の直後)
    let mut next_starts = false;
    let mut sp_name: Option<String> = None;
    let (mut path_w, mut path_h) = (1000.0f32, 1000.0f32);
    let mut has_custom = false;
    let mut in_from = false;
    let mut in_ln = false;
    let mut in_sp = false;
    let mut is_chart = false;
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
                    // **組み方も1つずつ畳む。** 畳まないと、前の図形の
                    // 揃えや箇条書きが次の箱に漏れる — 1つだけの試験では
                    // 出ず、6つ並べた実物の見本で初めて出た(2026-08-13)
                    tfmt = crate::model::TextFmt::default();
                    pts.clear();
                    sp_name = None;
                    has_custom = false;
                    (path_w, path_h) = (1000.0, 1000.0);
                    in_sp = false;
                    in_ln = false;
                    is_chart = false;
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
                // グラフの入れ物。**中に入らない** — 在ったことだけ控える
                b"graphicFrame" => is_chart = true,
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
                b"bodyPr" | b"pPr" | b"buChar" | b"buAutoNum" | b"buNone" | b"rPr"
                    if in_sp =>
                {
                    text_fmt_attr(&e, &mut tfmt);
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
                // **Start と Empty の両方で来る。** bodyPr も pPr も rPr も
                // 中に何か持てば Start、持たなければ Empty — 片方だけ見ていると
                // 「箇条書きを付けた途端に揃えが読めなくなる」ような穴になる
                // (sheet_view で一度踏んだのと同じ道理)
                b"bodyPr" | b"pPr" | b"buChar" | b"buAutoNum" | b"buNone" | b"rPr"
                    if in_sp =>
                {
                    text_fmt_attr(&e, &mut tfmt);
                }
                // **小道の区間。中に <a:pt/> を持つので Start で来る**
                // (Empty 側に置いて一度取り逃がした — bodyPr で踏んだのと
                // 同じ道理。3つ組かどうかはここで決まる)
                b"cubicBezTo" if has_custom => {
                    in_bez = true;
                    bez.clear();
                }
                b"lnTo" if has_custom => in_bez = false,
                // 2本目以降の moveTo は**輪郭の切れ目**(穴など)
                b"moveTo" if has_custom => {
                    in_bez = false;
                    next_starts = true;
                }
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
                    let at = (x / path_w.max(1.0), y / path_h.max(1.0));
                    // **曲線の中では3つ組**(制御点2つ → 着地点)。
                    // 制御点は前の点の c_out / この点の c_in へ振り分ける
                    if in_bez {
                        bez.push(at);
                        if bez.len() == 3 {
                            if let Some(prev) = pts.last_mut() {
                                prev.c_out = Some(bez[0]);
                            }
                            pts.push(crate::model::PathPoint {
                                at: bez[2],
                                start: false,
                                c_in: Some(bez[1]),
                                c_out: None,
                            });
                            bez.clear();
                        }
                    } else if std::mem::take(&mut next_starts) && !pts.is_empty() {
                        pts.push(crate::model::PathPoint::start_at(at.0, at.1));
                    } else {
                        pts.push(crate::model::PathPoint::at(at.0, at.1));
                    }
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
                // テキストボックスの組み方。**Start と Empty の両方で来る** —
                // bodyPr も pPr も rPr も、中に何か持てば Start、持たなければ
                // Empty。片方だけ見ていると「箇条書きを付けた途端に揃えが
                // 読めなくなる」種類の穴になる(sheet_view で一度踏んだ道理)
                b"bodyPr" | b"pPr" | b"buChar" | b"buAutoNum" | b"buNone" | b"rPr"
                    if in_sp =>
                {
                    text_fmt_attr(&e, &mut tfmt);
                }
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
                        text_fmt: tfmt.clone(),
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
                            // 札は `jo:種類:底[:印]`。**印は後から足した欄**なので
                            // 無い古い札も読める(split_once で2欄に割ってから、
                            // 残りをさらに割る)
                            let marker = sp_name
                                .as_deref()
                                .and_then(|n| n.strip_prefix("jo:"))
                                .and_then(|n| n.split_once(':'))
                                .filter(|(k, _)| *k == "spark-col" || *k == "spark-wl");
                            let marks = sp_name
                                .as_deref()
                                .and_then(|n| n.strip_prefix("jo:"))
                                .and_then(|n| n.splitn(3, ':').nth(2))
                                .map(crate::model::SparkMarks::parse)
                                .unwrap_or_default();
                            match marker {
                                Some((k, b)) if pts.len() >= 4 => {
                                    // 底は2欄目まで(3欄目の印を巻き込まない)
                                    let base: f32 =
                                        b.split(':').next().unwrap_or(b).parse().unwrap_or(1.0);
                                    let tops: Vec<crate::model::PathPoint> = pts
                                        .chunks(4)
                                        .filter(|c| c.len() == 4)
                                        .map(|c| {
                                            crate::model::PathPoint::at(
                                                (c[0].at.0 + c[1].at.0) / 2.0,
                                                c[0].at.1,
                                            )
                                        })
                                        .collect();
                                    Some(DrawKind::Shape(Box::new(
                                        crate::model::SheetShape {
                                            kind: k.into(),
                                            points: tops,
                                            base,
                                            spark_marks: marks,
                                            ..tpl
                                        },
                                    )))
                                }
                                _ => Some(DrawKind::Shape(Box::new(
                                    crate::model::SheetShape {
                                        kind: "spark".into(),
                                        points: std::mem::take(&mut pts),
                                        spark_marks: marks,
                                        ..tpl
                                    },
                                ))),
                            }
                        }
                        _ if is_chart => Some(DrawKind::Chart),
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

/// `_xlnm.Print_Titles` を(シート番号, 行の部, 列の部)に解く。
///
/// 中身は `'表'!$1:$4`(行)・`'表'!$A:$B`(列)・その両方を `,` で
/// 並べた形。**行と列は別々に持つ** — 片方だけの帳票が普通にある
/// (2026-08-13 に列も持つようにした。前は行だけで、列は原文で持ち越し)。
/// どちらも解けなければ None(原文のまま持ち越す側)。
#[allow(clippy::type_complexity)]
pub(super) fn parse_print_titles(
    raw: &str,
) -> Option<(usize, Option<(u32, u32)>, Option<(u32, u32)>)> {
    let sid = raw
        .split(SID_ATTR)
        .nth(1)
        .and_then(|r| r.split('"').next())
        .and_then(|v| v.parse::<usize>().ok())?;
    let body = raw.split('>').nth(1).and_then(|r| r.split('<').next())?;
    let mut rows = None;
    let mut cols = None;
    for part in body.split(',') {
        let Some(range) = part.rsplit('!').next().map(|r| r.replace('$', "")) else {
            continue;
        };
        let Some((a, b)) = range.split_once(':') else { continue };
        let (a, b) = (a.trim().to_string(), b.trim().to_string());
        if let (Ok(x), Ok(y)) = (a.parse::<u32>(), b.parse::<u32>()) {
            if x > 0 && y > 0 {
                rows = Some((x.min(y) - 1, x.max(y) - 1));
            }
        } else if !a.is_empty() && a.chars().all(|c| c.is_ascii_alphabetic()) {
            // 列は字("A":"B")。行を足して Pos に解かせる
            if let (Some(x), Some(y)) = (
                crate::model::Pos::parse(&format!("{a}1")),
                crate::model::Pos::parse(&format!("{b}1")),
            ) {
                cols = Some((x.col.min(y.col), x.col.max(y.col)));
            }
        }
    }
    (rows.is_some() || cols.is_some()).then_some((sid, rows, cols))
}

pub(super) fn resolve_target(t: &str) -> String {
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
pub(super) fn resolve_book_target(t: &str) -> String {
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
pub(super) fn sheet_part_no(n: &str) -> u32 {
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
pub(super) fn sheet_parts(
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
pub(super) fn defined_name_plain(e: &BytesStart) -> bool {
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
/// 古い `commentsN.xml`(著者の一覧 + セルごとの1件)を読む。
///
/// **著者も拾う。** 前は `<authors>` と `authorId` を捨てていたので、
/// 誰が書いたコメントか分からなくなっていた。
pub(super) fn parse_comments(xml: &str) -> Vec<(Pos, crate::model::CommentThread)> {
    use crate::model::{CommentEntry, CommentThread};
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);
    let mut out: Vec<(Pos, CommentThread)> = Vec::new();
    let mut buf = Vec::new();
    let mut cur: Option<Pos> = None;
    let mut text = String::new();
    let mut in_t = false;
    // 著者の一覧。comment@authorId がこの並びを指す
    let mut authors: Vec<String> = Vec::new();
    let mut in_authors = false;
    let mut in_author = false;
    let mut who = String::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"authors" => in_authors = true,
                b"author" if in_authors => in_author = true,
                b"comment" => {
                    cur = attr(&e, "ref").and_then(|s| Pos::parse(&s));
                    text.clear();
                    who = attr(&e, "authorId")
                        .and_then(|i| i.parse::<usize>().ok())
                        .and_then(|i| authors.get(i).cloned())
                        .unwrap_or_default();
                }
                b"t" if cur.is_some() => in_t = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_t => text.push_str(&t.unescape().unwrap_or_default()),
            Ok(Event::Text(t)) if in_author => {
                who.push_str(&t.unescape().unwrap_or_default())
            }
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                b"t" => in_t = false,
                b"author" => {
                    in_author = false;
                    authors.push(std::mem::take(&mut who));
                }
                b"authors" => in_authors = false,
                b"comment" => {
                    if let Some(p) = cur.take() {
                        out.push((
                            p,
                            CommentThread {
                                done: false,
                                entries: vec![CommentEntry {
                                    who: std::mem::take(&mut who),
                                    when: String::new(),
                                    text: std::mem::take(&mut text),
                                }],
                            },
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

/// `xl/threadedComments/threadedCommentN.xml` を読む。**こちらが本体** —
/// 古い `commentsN.xml` はその写しなので、両方あればこちらを採る。
///
/// 1件は `<threadedComment ref dT personId id parentId done><text>…</text>`。
/// `parentId` を持つものが返信で、親の後ろに並ぶ。`personId` は
/// `xl/persons/person.xml` の表示名を指す。
pub(super) fn parse_threaded_comments(
    xml: &str,
    persons: &std::collections::BTreeMap<String, String>,
) -> Vec<(Pos, crate::model::CommentThread)> {
    use crate::model::{CommentEntry, CommentThread};
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(false);
    // (場所, 自分のid, 親のid, 解決済み, 発言)
    let mut items: Vec<(Pos, String, Option<String>, bool, CommentEntry)> = Vec::new();
    let mut buf = Vec::new();
    let mut cur: Option<(Pos, String, Option<String>, bool, CommentEntry)> = None;
    let mut in_text = false;
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"threadedComment" => {
                    let p = attr(&e, "ref").and_then(|s| Pos::parse(&s));
                    let who = attr(&e, "personId")
                        .and_then(|id| persons.get(&id).cloned())
                        .unwrap_or_default();
                    cur = p.map(|p| {
                        (
                            p,
                            attr(&e, "id").unwrap_or_default(),
                            attr(&e, "parentId"),
                            matches!(attr(&e, "done").as_deref(), Some("1") | Some("true")),
                            CommentEntry {
                                who,
                                when: attr(&e, "dT").unwrap_or_default(),
                                text: String::new(),
                            },
                        )
                    });
                }
                b"text" if cur.is_some() => in_text = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_text => {
                if let Some(c) = &mut cur {
                    c.4.text.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                b"text" => in_text = false,
                b"threadedComment" => {
                    if let Some(c) = cur.take() {
                        items.push(c);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        buf.clear();
    }
    // 場所ごとに束ねる。**親を先に、返信をその後ろに** — 原文の並びが
    // すでにその順なので、親を持たないものを先に流し込む
    let mut out: Vec<(Pos, CommentThread)> = Vec::new();
    for (p, _id, parent, done, entry) in items {
        match out.iter_mut().find(|(q, _)| *q == p) {
            Some((_, th)) => {
                th.done |= done;
                th.entries.push(entry);
            }
            None => {
                // 親が先に来なかった筋(壊れた原文)でも落とさない
                let _ = &parent;
                out.push((p, CommentThread { done, entries: vec![entry] }));
            }
        }
    }
    out
}

pub(super) fn parse_sheet(xml: &str, shared: &[String], rubies: &[Option<String>],
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
    // 印刷のヘッダー/フッター。いまどの区分の中か
    // ("oddHeader" などの元素の名前をそのまま持つ)
    let mut hf_side: Option<Vec<u8>> = None;
    let mut style: Option<usize> = None;
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                // 印刷のヘッダー/フッター(文字は子の Text で拾う)
                b"oddHeader" | b"oddFooter" | b"evenHeader" | b"evenFooter"
                | b"firstHeader" | b"firstFooter" => {
                    hf_side = Some(local(e.name().as_ref()).to_vec());
                }
                // 奇数偶数・先頭頁で分ける旗(headerFooter の属性)。
                // **持たないと保存で偶数・先頭のヘッダーが消える**
                b"headerFooter" => {
                    let on = |k: &str| {
                        matches!(attr(&e, k).as_deref(), Some("1") | Some("true"))
                    };
                    sh.hf_diff_odd_even = on("differentOddEven");
                    sh.hf_diff_first = on("differentFirst");
                }
                b"row" => row_height(&e, &mut sh),
                b"c" => {
                    pos = attr(&e, "r").and_then(|s| Pos::parse(&s));
                    saw_cell(&mut sh, pos);
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
                // **全行の既定の高さ。** 書いてはいたが読んでいなかった
                // (2026-08-10)。無い行はこれで描くので、落とすと行間が変わる
                b"sheetFormatPr" => {
                    sh.default_row_height =
                        attr(&e, "defaultRowHeight").and_then(|v| v.parse::<f32>().ok());
                    // **全列の既定幅もここにある。** `<col>` の無い列はこの幅
                    if sh.default_col_width.is_none() {
                        sh.default_col_width =
                            attr(&e, "defaultColWidth").and_then(|v| v.parse::<f32>().ok());
                    }
                }
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
                // **全行の既定の高さと全列の既定幅。** 原本は
                // `<sheetFormatPr defaultRowHeight="15" customHeight="1"/>` と
                // **自己終了形**で書かれるので、Empty の枝にも要る。
                // **同じ形の穴は4度目**(sheetView・docx の <w:p/>・<row/>・これ)
                b"sheetFormatPr" => {
                    sh.default_row_height =
                        attr(&e, "defaultRowHeight").and_then(|v| v.parse::<f32>().ok());
                    if sh.default_col_width.is_none() {
                        sh.default_col_width =
                            attr(&e, "defaultColWidth").and_then(|v| v.parse::<f32>().ok());
                    }
                }
                // **書き手が申告した大きさ** `<dimension ref="A1:CN46"/>`。
                // 単独では信じない — `Sheet::size` が実際と大きいほうを採る
                b"dimension" => {
                    if let Some(r) = attr(&e, "ref") {
                        let end = r.rsplit(':').next().unwrap_or(&r);
                        if let Some(p) = Pos::parse(end) {
                            sh.dim = Some((p.row + 1, p.col + 1));
                        }
                    }
                }
                // **中身の無い行 `<row r="71" ht="23.1" customHeight="1"/>`。**
                // 高さだけ決めた空行で、帳票では行間の調整によく使う。
                // Start の枝にしか置いていなかったので、**高さが落ちていた**
                // (日銀の資金循環で 115 箇所。2026-08-10)。
                //
                // **同じ形の穴は3度目。** xlsx の sheetView(Empty の枝にしか
                // 無かった)・docx の `<w:p/>`(Start の枝にしか無かった)。
                // quick-xml で読む所は、**Start と Empty の両方に置いたか**を
                // 要素ごとに確かめること
                b"row" => row_height(&e, &mut sh),
                b"c" => {
                    // 値の無い自己完結のセル。書式だけなら、それは帳票の枠 —
                    // 落とすと保存で罫線が消える(Excel 以外の道具が書く形)
                    saw_cell(&mut sh, attr(&e, "r").and_then(|s| Pos::parse(&s)));
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
                // シート見出し(タブ)の色。rgb 指定だけ拾う(theme 指定は色に解けない)
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
                    // **既定は禁止**(objects を書かない古い版は「図形も保護」)
                    a.objects = !deny("objects", true);
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
                    match hf_side.as_deref() {
                        Some(b"oddHeader") => sh.header = Some(s),
                        Some(b"oddFooter") => sh.footer = Some(s),
                        Some(b"evenHeader") => sh.header_even = Some(s),
                        Some(b"evenFooter") => sh.footer_even = Some(s),
                        Some(b"firstHeader") => sh.header_first = Some(s),
                        Some(b"firstFooter") => sh.footer_first = Some(s),
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(t)) if in_v || in_f || in_is => {
                let s = t.unescape().unwrap_or_default();
                if in_f { f.push_str(&s) } else { v.push_str(&s) }
            }
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                b"oddHeader" | b"oddFooter" | b"evenHeader" | b"evenFooter"
                | b"firstHeader" | b"firstFooter" => hf_side = None,
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

/// `dc:creator` の1つの文字列を著者の列に割る。
///
/// Excel は複数の著者を**1つの要素に `;` 区切り**で入れる。前後の空白は
/// 落とし、空の断片は数えない — 「山田;」は1人であって、名無しの2人目は
/// いない。区切りが無ければそのまま1人。
pub(super) fn split_creators(raw: &str) -> Vec<String> {
    raw.split(';').map(|t| t.trim()).filter(|t| !t.is_empty()).map(String::from).collect()
}

/// `docProps/custom.xml` を読む。
///
/// 一件は `<property fmtid="…" pid="…" name="…"><vt:型>値</vt:型></property>`。
/// **`fmtid` と `pid` は読まない** — この部品の `fmtid` は規格が1つに
/// 定めていて(D5CDD505-…)、`pid` は2からの連番。書くときに振り直すので
/// 原文の番号を持ち歩いても使い道がない。ただし `linkTarget` は意味を持つ
/// ので抱える。知らない型も捨てずに `Other` で抱える。
pub(super) fn parse_custom_props(xml: &str) -> Vec<crate::model::CustomProp> {
    use crate::model::{CustomProp, CustomVal};
    let unesc = |t: &str| {
        t.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
            .replace("&apos;", "'").replace("&amp;", "&")
    };
    let mut out: Vec<CustomProp> = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<property") {
        rest = &rest[i..];
        let Some(head_end) = rest.find('>') else { break };
        let head = &rest[..head_end];
        // name="…" を取る。無名の property は Excel も作らないので飛ばす
        let grab_attr = |want: &str| -> Option<String> {
            let pat = format!("{want}=\"");
            head.find(&pat).and_then(|a| {
                let s = &head[a + pat.len()..];
                s.find('"').map(|b| unesc(&s[..b]))
            })
        };
        let name = grab_attr("name").unwrap_or_default();
        let link = grab_attr("linkTarget").filter(|t| !t.is_empty());
        let body_end = rest.find("</property>").unwrap_or(rest.len());
        let body = &rest[head_end + 1..body_end.max(head_end + 1)];
        rest = &rest[body_end.min(rest.len())..];
        if name.is_empty() {
            continue;
        }
        // 中身は `<vt:型>値</vt:型>` ひとつ。接頭辞は原本によって変わる
        // (`vt:` が既定だが、根で既定名前空間にしている物もある)ので
        // 接頭辞は捨てて**局所名で見る**
        let Some(a) = body.find('<') else { continue };
        let Some(b) = body[a..].find('>').map(|k| a + k) else { continue };
        let tag_full = &body[a + 1..b];
        let tag = tag_full.split(':').next_back().unwrap_or(tag_full);
        let inner = body[b + 1..]
            .find('<')
            .map(|k| &body[b + 1..b + 1 + k])
            .unwrap_or("");
        let raw = unesc(inner);
        let value = match tag {
            "lpwstr" | "lpstr" => CustomVal::Text(raw),
            "r8" | "r4" => match raw.trim().parse::<f64>() {
                Ok(n) => CustomVal::Number(n),
                Err(_) => CustomVal::Other(tag.to_string(), raw),
            },
            "filetime" | "date" => CustomVal::Date(raw),
            // XML の真偽は "true"/"false" と "1"/"0" の両方が来る
            "bool" => match raw.trim() {
                "true" | "1" => CustomVal::Bool(true),
                "false" | "0" => CustomVal::Bool(false),
                _ => CustomVal::Other(tag.to_string(), raw),
            },
            _ => CustomVal::Other(tag.to_string(), raw),
        };
        // 名前はブックの中で一意。同じ名前が二度来たら後を採る
        if let Some(k) = out.iter().position(|p: &CustomProp| p.name == name) {
            out[k].value = value;
            out[k].link = link;
        } else {
            out.push(CustomProp { name, value, link });
        }
    }
    out
}

pub fn read<R: Read + Seek>(src: R) -> Result<(Book, Report), String> {
    let mut zip = zip::ZipArchive::new(src).map_err(|e| format!("zipを開けません: {e}"))?;
    let mut rep = Report::default();

    // 書式表を先に読む。セルの s= はこの索引
    let mut styles: Vec<crate::model::CellFormat> = Vec::new();
    let mut dxfs: Vec<crate::model::CondLook> = Vec::new();
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
    let mut date1904 = false;
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
                        date1904 = true;
                        rep.note("1904年の日付系(日付の計算と表示は 1904-01-01 起点で扱います。保存でも保たれます)");
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
        date1904,
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
            // **`;` 区切りで複数の著者**(Excel の慣習)。区切りが無ければ1人。
            // 空欄は0人 — 「空の名前が1人いる」ことにしない
            creators: split_creators(&grab("dc:creator")),
            title: grab("dc:title"),
            subject: grab("dc:subject"),
            keywords: grab("cp:keywords"),
            description: grab("dc:description"),
            custom: Vec::new(),
        };
    }
    // カスタムプロパティ(docProps/custom.xml)。core.xml とは**別の部品**
    if let Ok(mut f) = zip.by_name("docProps/custom.xml") {
        let mut s = String::new();
        let _ = f.read_to_string(&mut s);
        book.props.custom = parse_custom_props(&s);
    }
    // コメントを書いた人の表示名(xl/persons/*.xml)。**ブックに1つ**なので
    // シートを回る前に一度だけ読む。threadedComment@personId がここを指す
    let persons: std::collections::BTreeMap<String, String> = {
        let mut m = std::collections::BTreeMap::new();
        let names: Vec<String> = (0..zip.len())
            .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
            .filter(|n| n.starts_with("xl/persons/") && n.ends_with(".xml"))
            .collect();
        for n in names {
            let mut s = String::new();
            if let Ok(mut f) = zip.by_name(&n) {
                let _ = f.read_to_string(&mut s);
            }
            let mut r = Reader::from_str(&s);
            let mut buf = Vec::new();
            loop {
                match r.read_event_into(&mut buf) {
                    Ok(Event::Eof) | Err(_) => break,
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if local(e.name().as_ref()) == b"person" =>
                    {
                        if let (Some(id), Some(name)) =
                            (attr_un(&e, "id"), attr_un(&e, "displayName"))
                        {
                            m.insert(id, name);
                        }
                    }
                    _ => {}
                }
                buf.clear();
            }
        }
        m
    };
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
                dxfs: &[crate::model::CondLook],
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
                            let look = dxf
                                .and_then(|i| dxfs.get(i).cloned())
                                .unwrap_or_default();
                            sh.cond.push(crate::model::CondRule { range, kind, look });
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
        // シナリオ(入力セルの組に名前を付けたもの)。名前・覚え書き・
        // 入力セルの位置と値をそのまま持つ
        {
            let mut r = Reader::from_str(&s);
            let mut buf = Vec::new();
            let mut cur: Option<crate::model::Scenario> = None;
            loop {
                match r.read_event_into(&mut buf) {
                    Ok(Event::Eof) | Err(_) => break,
                    Ok(Event::Start(e)) if local(e.name().as_ref()) == b"scenario" => {
                        cur = Some(crate::model::Scenario {
                            name: attr(&e, "name").unwrap_or_default(),
                            cells: Vec::new(),
                            comment: attr(&e, "comment").unwrap_or_default(),
                        });
                    }
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if local(e.name().as_ref()) == b"inputCells" =>
                    {
                        if let (Some(sc), Some(p)) =
                            (cur.as_mut(), attr(&e, "r").and_then(|v| Pos::parse(&v)))
                        {
                            sc.cells.push((p, attr(&e, "val").unwrap_or_default()));
                        }
                    }
                    Ok(Event::End(e)) if local(e.name().as_ref()) == b"scenario" => {
                        if let Some(sc) = cur.take() {
                            if !sc.name.is_empty() {
                                sh.scenarios.push(sc);
                            }
                        }
                    }
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
        // **スレッドのコメントが本体。** 古い commentsN.xml はその写しなので、
        // 両方あるときはこちらで上書きする — 写しだけ読んでいると返信が
        // 見えず、直した内容も Excel 側へ届かない(2026-08-13 に実測)
        for (_, _, target, _) in
            rels.iter().filter(|(_, t, _, _)| t.ends_with("/threadedComment"))
        {
            if let Ok(mut f) = zip.by_name(&resolve_target(target)) {
                let mut ts = String::new();
                let _ = f.read_to_string(&mut ts);
                for (p, t) in parse_threaded_comments(&ts, &persons) {
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
                        // **描けない形を黙って四角にしない。** 保存では
                        // prstGeom の名前をそのまま返すので原本は壊れないが、
                        // 画面では四角に見える — 見える物が違うなら言う
                        if !crate::model::can_draw(&sp.kind) {
                            rep.note("図形(描けない形。四角で見せます。保存では元の形のまま)");
                        }
                        sp.at = at;
                        sp.width_px = width_px;
                        sp.height_px = height_px;
                        // ずらし(colOff/rowOff)も読む — SmartArt の
                        // 図形の集まりが保存後も同じ場所に見える
                        sp.dx_px = ox_emu as f32 / 9525.0;
                        sp.dy_px = oy_emu as f32 / 9525.0;
                        sh.shapes.push(*sp);
                    }
                    // **持たないが、黙らない。** グラフの模型は持たない
                    // 決めなので描かないが、在ったことは帳簿に載せる。
                    // 原本の drawing は保存で持ち越されるので壊れはしない
                    DrawKind::Chart => rep.note("グラフ(chart)"),
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
                    // **localSheetId が付いていれば「このシートだけ」。**
                    // 前は付いていることを重なりから当てていた(推測)
                    Some(sh) => sh.names.push(crate::model::DefinedName {
                        name: nm,
                        range: r,
                        scoped: sid.is_some(),
                    }),
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
                                sort: String::new(),
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
                    Ok(Event::Start(e)) | Ok(Event::Empty(e))
                        if local(e.name().as_ref()) == b"so" =>
                    {
                        if let Some(d) = cur.as_mut() {
                            d.sort = attr_un(&e, "v").unwrap_or_default();
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
            // 行の部($1:$4)と列の部($A:$B)。片方だけの帳票も普通にある
            if let Some((sid, rows, cols)) = parse_print_titles(&raw) {
                if let Some(sh) = book.sheets.get_mut(sid) {
                    sh.print_title_rows = rows;
                    sh.print_title_cols = cols;
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
pub(super) fn parse_print_area(raw: &str) -> Option<(usize, Vec<(Pos, Pos)>)> {
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
