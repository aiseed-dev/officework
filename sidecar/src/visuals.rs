//! **図形と画像を、モデルを通さずに原本の drawing から読む。**
//!
//! 契約が欲しいのは `drawingPath` と `drawingIndex`(向こうが保存で書き戻す
//! ときの位置指定)と、**原本どおりのアンカー**。officework の `SheetImage` は
//! 左上のセルと px の大きさに潰してあるので、どちらも出せない。モデルを
//! 経由すると**契約が欲しい情報をこちらで捨ててから渡す**ことになる。
//!
//! だから ZIP の配管と同じ立ち場で読む — 部品を開き、原文をそのまま写す。

use std::collections::BTreeMap;
use std::io::{Read, Seek};

use serde_json::{Map, Value, json};
use book::Sheet;

use crate::archive::open_zip;

//
// **モデルを通さずに原本の drawing から読む。**
//
// 契約が欲しいのは `drawingPath` と `drawingIndex`(向こうが保存で書き戻す
// ときの位置指定)と、**原本どおりのアンカー**。officework の `SheetImage` は
// 左上のセルと px の大きさに潰してあるので、どちらも出せない。モデルを
// 経由すると**契約が欲しい情報をこちらで捨ててから渡す**ことになる。
//
// だから ZIP の配管と同じ立ち場で読む — 部品を開き、原文をそのまま写す。

/// `xl/worksheets/sheet1.xml` から見た `../drawings/drawing1.xml` を
/// `xl/drawings/drawing1.xml` に直す。**`..` を素直に畳む。**
pub(crate) fn resolve_part(base: &str, target: &str) -> String {
    if let Some(t) = target.strip_prefix('/') {
        return t.to_string();
    }
    let dir = base.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

pub(crate) fn attr_of(e: &quick_xml::events::BytesStart<'_>, k: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == k)
        .map(|a| String::from_utf8_lossy(&a.value).to_string())
}

pub(crate) fn part_text(z: &mut zip::ZipArchive<impl Read + Seek>, part: &str) -> Option<String> {
    let mut f = z.by_name(part).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

/// rels を読んで `Id` → `Target` の表にする。
pub(crate) fn rels_map(z: &mut zip::ZipArchive<impl Read + Seek>, part: &str) -> BTreeMap<String, String> {
    let (dir, file) = part.rsplit_once('/').unwrap_or(("", part));
    let mut out = BTreeMap::new();
    let Some(s) = part_text(z, &format!("{dir}/_rels/{file}.rels")) else { return out };
    let mut r = quick_xml::Reader::from_str(&s);
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Empty(e)) | Ok(quick_xml::events::Event::Start(e))
                if e.name().local_name().as_ref() == b"Relationship" =>
            {
                if let (Some(id), Some(t)) = (attr_of(&e, b"Id"), attr_of(&e, b"Target")) {
                    out.insert(id, t);
                }
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

/// workbook の並び順で、シートの本体の部品の径路を返す。
///
/// **`r:id` を rels で解く。** 名前の番号(`sheet3.xml`)と並びは一致しない
/// (`sheet` 側も 127e762 で同じ直しをしている)。
pub(crate) fn sheet_parts(z: &mut zip::ZipArchive<impl Read + Seek>) -> Vec<String> {
    let rels = rels_map(z, "xl/workbook.xml");
    let Some(s) = part_text(z, "xl/workbook.xml") else { return Vec::new() };
    let mut r = quick_xml::Reader::from_str(&s);
    let (mut buf, mut out) = (Vec::new(), Vec::new());
    loop {
        match r.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Empty(e)) | Ok(quick_xml::events::Event::Start(e))
                if e.name().local_name().as_ref() == b"sheet" =>
            {
                let target = attr_of(&e, b"id").and_then(|id| rels.get(&id).cloned());
                out.push(target.map(|t| resolve_part("xl/workbook.xml", &t)).unwrap_or_default());
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

/// 絵の種類を拡張子から言う。**知らない拡張子には `image/` を付けない** —
/// 向こうの `mediaType` は `^image/` で検査される。
pub(crate) fn media_type(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// `xdr:from` / `xdr:to` の中身。EMU のずらしつき。
#[derive(Default, Clone, Copy)]
pub(crate) struct Corner {
    col: u32,
    col_off: i64,
    row: u32,
    row_off: i64,
}

pub(crate) fn anchor_value(a: &Corner, b: &Corner) -> Value {
    json!({
        "fromRow": a.row, "fromColumn": a.col,
        "fromRowOffset": a.row_off, "fromColumnOffset": a.col_off,
        "toRow": b.row, "toColumn": b.col,
        "toRowOffset": b.row_off, "toColumnOffset": b.col_off,
    })
}

/// `to` の無いアンカー(`oneCellAnchor`)のために、**大きさから右下を当てる**。
///
/// 列の幅・行の高さを EMU に直して、`cx`/`cy` を覆うまで歩く。
/// **原本に無い数字を作っている** — 原本が `to` を書いていればそちらを使う。
pub(crate) fn span_to(from: &Corner, ext: (i64, i64), sh: &Sheet) -> Corner {
    const EMU_PER_PT: f64 = 12700.0;
    // Excel の列幅は「標準の字の数」。px ≒ 幅×7+5、1px = 9525 EMU
    let col_emu = |c: u32| -> i64 {
        let w = sh.col_width.get(&c).copied().or(sh.default_col_width).unwrap_or(8.43);
        ((w as f64 * 7.0 + 5.0) * 9525.0) as i64
    };
    let row_emu = |r: u32| -> i64 {
        let h = sh.row_height.get(&r).copied().or(sh.default_row_height).unwrap_or(15.0);
        (h as f64 * EMU_PER_PT) as i64
    };
    let (mut col, mut left) = (from.col, ext.0 + from.col_off);
    while left > 0 && col < 16_383 {
        left -= col_emu(col).max(1);
        col += 1;
    }
    let (mut row, mut up) = (from.row, ext.1 + from.row_off);
    while up > 0 && row < 1_048_575 {
        up -= row_emu(row).max(1);
        row += 1;
    }
    Corner { col, col_off: 0, row, row_off: 0 }
}

/// **1つのシートの図形と画像。** 原本の drawing をそのまま読む。
///
/// `drawing_index` は**飛ばしたアンカーも数える**(向こうの schema が
/// 「document order, skipped anchors counted」と書いている)。数え落とすと、
/// 向こうが保存で別の図形を書き換える。
pub(crate) fn visuals_of(
    z: &mut zip::ZipArchive<impl Read + Seek>,
    sheet_part: &str,
    sheet_id: &str,
    sh: &Sheet,
    skipped: &mut Vec<String>,
) -> Vec<Value> {
    if sheet_part.is_empty() {
        return Vec::new();
    }
    // **コメントの吹き出しの VML は drawing ではない。** `drawings/drawing` で
    // 引くのは、`vmlDrawing1.vml` を絵と数えないため(2026-08-10 に数え違えた)
    let Some(target) = rels_map(z, sheet_part)
        .into_values()
        .find(|t| t.contains("drawings/drawing"))
        .map(|t| resolve_part(sheet_part, &t))
    else {
        return Vec::new();
    };
    let media = rels_map(z, &target);
    let Some(xml) = part_text(z, &target) else { return Vec::new() };

    let mut r = quick_xml::Reader::from_str(&xml);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let (mut index, mut from, mut to) = (0usize, Corner::default(), None::<Corner>);
    let (mut ext, mut side) = ((0i64, 0i64), 0u8); // side: 1=from 2=to
    let mut field: Vec<u8> = Vec::new();
    let (mut kind, mut embed, mut name) = (None::<&str>, None::<String>, String::new());
    let (mut prst, mut fill, mut label) = (None::<String>, None::<String>, String::new());
    let mut in_txt = false;

    let num = |s: &str| s.trim().parse::<i64>().unwrap_or(0);

    loop {
        let ev = r.read_event_into(&mut buf);
        match ev {
            Ok(quick_xml::events::Event::Start(ref e))
            | Ok(quick_xml::events::Event::Empty(ref e)) => {
                let local = e.name().local_name().as_ref().to_vec();
                match local.as_slice() {
                    b"twoCellAnchor" | b"oneCellAnchor" | b"absoluteAnchor" => {
                        from = Corner::default();
                        to = None;
                        ext = (0, 0);
                        kind = None;
                        embed = None;
                        name = String::new();
                        prst = None;
                        fill = None;
                        label = String::new();
                    }
                    b"from" => side = 1,
                    b"to" => {
                        side = 2;
                        to = Some(Corner::default());
                    }
                    b"col" | b"colOff" | b"row" | b"rowOff" => field = local.clone(),
                    b"ext" => {
                        ext = (
                            attr_of(e, b"cx").map(|v| num(&v)).unwrap_or(0),
                            attr_of(e, b"cy").map(|v| num(&v)).unwrap_or(0),
                        )
                    }
                    b"pic" => kind = Some("image"),
                    b"sp" => kind = Some("shape"),
                    b"graphicFrame" => kind = Some("chart"),
                    b"blip" => embed = attr_of(e, b"embed"),
                    b"cNvPr" => name = attr_of(e, b"name").unwrap_or_default(),
                    b"prstGeom" => prst = attr_of(e, b"prst"),
                    b"srgbClr" => {
                        if fill.is_none() {
                            fill = attr_of(e, b"val").map(|v| format!("#{v}"));
                        }
                    }
                    b"txBody" => in_txt = true,
                    _ => {}
                }
            }
            Ok(quick_xml::events::Event::Text(ref t)) => {
                let s = t.unescape().unwrap_or_default().to_string();
                if in_txt {
                    label.push_str(&s);
                }
                if !field.is_empty() {
                    let c = if side == 2 {
                        to.get_or_insert(Corner::default())
                    } else {
                        &mut from
                    };
                    match field.as_slice() {
                        b"col" => c.col = num(&s).max(0) as u32,
                        b"colOff" => c.col_off = num(&s),
                        b"row" => c.row = num(&s).max(0) as u32,
                        b"rowOff" => c.row_off = num(&s),
                        _ => {}
                    }
                }
            }
            Ok(quick_xml::events::Event::End(ref e)) => {
                let local = e.name().local_name().as_ref().to_vec();
                match local.as_slice() {
                    b"col" | b"colOff" | b"row" | b"rowOff" => field.clear(),
                    b"from" | b"to" => side = 0,
                    b"txBody" => in_txt = false,
                    b"twoCellAnchor" | b"oneCellAnchor" | b"absoluteAnchor" => {
                        // **飛ばしても番号は進める。** 向こうの drawingIndex は
                        // 「飛ばしたアンカーも数える」と決まっている
                        let this = index;
                        index += 1;
                        let corner_to = to.unwrap_or_else(|| span_to(&from, ext, sh));
                        let mut o = Map::new();
                        o.insert("id".into(), json!(format!("{target}#{this}")));
                        o.insert("sheetId".into(), json!(sheet_id));
                        o.insert("anchor".into(), anchor_value(&from, &corner_to));
                        o.insert("drawingPath".into(), json!(target));
                        o.insert("drawingIndex".into(), json!(this));
                        if !name.is_empty() {
                            o.insert("name".into(), json!(name.clone()));
                        }
                        match kind {
                            Some("image") => {
                                let Some(rel) = embed.as_ref().and_then(|id| media.get(id)) else {
                                    skipped.push(format!("{target}#{this} 絵の在り処が引けない"));
                                    continue;
                                };
                                let path = resolve_part(&target, rel);
                                let Some(mt) = media_type(&path) else {
                                    skipped.push(format!("{path} 知らない絵の種類"));
                                    continue;
                                };
                                o.insert("kind".into(), json!("image"));
                                o.insert("mediaPath".into(), json!(path));
                                o.insert("mediaType".into(), json!(mt));
                                out.push(Value::Object(o));
                            }
                            Some("shape") => {
                                o.insert("kind".into(), json!("shape"));
                                if let Some(p) = &prst {
                                    o.insert("shapeType".into(), json!(p));
                                }
                                if let Some(c) = &fill {
                                    o.insert("fillColor".into(), json!(c));
                                }
                                let t = label.trim();
                                if !t.is_empty() {
                                    o.insert("text".into(), json!(t));
                                }
                                out.push(Value::Object(o));
                            }
                            // **グラフは出さない。** 系列まで組み立てないと
                            // 空の枠を描かせることになる。黙って消さずに言う
                            Some("chart") => skipped.push(format!("{target}#{this} グラフ")),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

/// `open` の答えから、絵の id → (原本の中の径路, 種類) を拾う。
///
/// **答えそのものを正とする。** 別に数え直すと、返した物と引ける物が
/// ずれる余地ができる — 今日それで何度も転んだ。
pub(crate) fn media_index(open: &Value) -> BTreeMap<String, (String, String)> {
    open.get("visuals")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| {
                    Some((
                        v.get("id")?.as_str()?.to_string(),
                        (
                            v.get("mediaPath")?.as_str()?.to_string(),
                            v.get("mediaType")?.as_str()?.to_string(),
                        ),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `read_media` — 絵の中身を base64 で返す。**聞かれたときに原本から出す。**
pub(crate) fn read_media(
    path: &str,
    media: &BTreeMap<String, (String, String)>,
    visual_id: &str,
) -> Result<Value, String> {
    let Some((part, mime)) = media.get(visual_id) else {
        return Err(format!("その絵はありません: {visual_id}"));
    };
    let mut z = open_zip(path)?;
    let mut f = z.by_name(part).map_err(|e| format!("{part}: 取り出せません: {e}"))?;
    let mut body = Vec::new();
    f.read_to_end(&mut body).map_err(|e| format!("{part}: 読めません: {e}"))?;
    Ok(json!({ "mediaType": mime, "base64": base64(&body) }))
}

/// **base64。** 依存を1つ増やすほどの話ではない(`uuid_v4` と同じ判断)。
pub(crate) fn base64(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((*c.get(1).unwrap_or(&0) as u32) << 8)
            | (*c.get(2).unwrap_or(&0) as u32);
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { A[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { A[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **部品の径路を素直に畳む。** `xl/worksheets/sheet1.xml` から見た
    /// `../drawings/drawing1.xml` は `xl/drawings/drawing1.xml`。ここを
    /// 間違えると絵が1枚も出ない — しかも**黙って**出ない
    #[test]
    fn part_paths_can_be_resolved() {
        assert_eq!(
            resolve_part("xl/worksheets/sheet1.xml", "../drawings/drawing1.xml"),
            "xl/drawings/drawing1.xml"
        );
        assert_eq!(
            resolve_part("xl/drawings/drawing1.xml", "../media/image1.png"),
            "xl/media/image1.png"
        );
        assert_eq!(
            resolve_part("xl/workbook.xml", "worksheets/sheet1.xml"),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(resolve_part("xl/workbook.xml", "/xl/theme/theme1.xml"), "xl/theme/theme1.xml");
    }

    /// **知らない拡張子に `image/` を付けない。** 向こうの `mediaType` は
    /// `^image/` で検査される。`.emf` を `image/emf` と偽れば通ってしまうが、
    /// 描けない物を描けると言ったことになる
    #[test]
    fn unknown_image_kind_is_not_declared() {
        assert_eq!(media_type("xl/media/image1.PNG"), Some("image/png"));
        assert_eq!(media_type("xl/media/image2.jpeg"), Some("image/jpeg"));
        assert_eq!(media_type("xl/media/image3.emf"), None, "**知らない物を名乗った**");
        assert_eq!(media_type("xl/media/image4"), None);
    }

    /// base64。**詰め物の `=` まで**確かめる — 長さが3の倍数でないときだけ
    /// 出るので、そこを外すと「たいてい合っている」絵が届く
    #[test]
    fn base64_is_correct() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"a"), "YQ==");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64(b"\x89PNG\r\n\x1a\n"), "iVBORw0KGgo=");
    }
}
