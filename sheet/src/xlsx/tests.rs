//! xlsx の読み書きの試験。**往復で確かめる** — 書いて読み直し、
//! 出したものと同じものが返るか。

use std::io::{Cursor, Read, Write};


use book::{Book, Cell, Pos, Value};

use super::read::*;
use super::write::*;

#[cfg(test)]
mod fmt_round {

    /// **図形の塗りに、字の色を使わない。**
    ///
    /// `<a:noFill/>` の図形の中にあるテキストボックスの `<a:rPr>` が
    /// `<a:srgbClr val="000000"/>` を持つと、それを図形の塗りとして拾って
    /// いました。国税庁の消費税の表の縦書きの箱が真っ黒に潰れていました
    /// (2026-08-31 発注者)。
    #[test]
    fn a_shapes_fill_is_not_its_text_colour() {
        // **実物の形そのままです**(国税庁の消費税の表の drawing1.xml)。
        // `<a:noFill/>` が2つ、その後ろの `<a:rPr>` に字の色があります
        let dr = r#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
<xdr:twoCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>
<xdr:to><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>3</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>
<xdr:sp macro="" textlink=""><xdr:nvSpPr><xdr:cNvPr id="1" name="Text Box 9"/><xdr:cNvSpPr txBox="1"/></xdr:nvSpPr>
<xdr:spPr bwMode="auto"><a:xfrm><a:off x="36711" y="4718146"/><a:ext cx="110992" cy="789768"/></a:xfrm>
<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln w="9525"><a:noFill/><a:miter lim="800000"/><a:headEnd/><a:tailEnd/></a:ln><a:effectLst/></xdr:spPr>
<xdr:txBody><a:bodyPr vert="vert" wrap="none" anchor="b"/><a:lstStyle/><a:p><a:pPr algn="l"/><a:r>
<a:rPr lang="en-US" sz="600" b="0" i="0" strike="noStrike"><a:solidFill><a:srgbClr val="000000"/></a:solidFill><a:latin typeface="Century"/></a:rPr>
<a:t>For the current year</a:t></a:r></a:p></xdr:txBody></xdr:sp>
<xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>"#;
        let v = crate::xlsx::read::parse_drawing_anchors(dr);
        let sp = v
            .iter()
            .find_map(|t| match &t.5 {
                crate::xlsx::read::DrawKind::Shape(s) => Some(s.as_ref()),
                _ => None,
            })
            .expect("図形が読めない");
        assert_eq!(sp.fill, None, "塗らない図形に色が付いた");
        assert_eq!(sp.line, None, "線を引かない図形に色が付いた");
        assert!(sp.text_fmt.vertical, "縦組みを読んでいない");
        assert_eq!(sp.text.as_deref(), Some("For the current year"));
    }
    use book::{Borders, Cell, CellFormat, Edge, HAlign, Pos, Value};
    use book::{Book, Sheet};

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
    fn borders_round_trip() {
        // 日本の帳票の本体。落とすと書類として通らない
        let f = CellFormat { borders: Borders::ALL, ..Default::default() };
        let back = roundtrip(&book(f.clone()));
        let c = back.sheets[0].get(Pos { row: 0, col: 0 }).unwrap();
        assert_eq!(c.fmt.borders, Borders::ALL, "罫線が消えた: {:?}", c.fmt);
    }

    #[test]
    fn bold_fill_and_alignment_round_trip() {
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
    fn number_formats_round_trip() {
        let f = CellFormat { number_format: Some("#,##0".into()), ..Default::default() };
        let back = roundtrip(&book(f.clone()));
        let c = back.sheets[0].get(Pos { row: 0, col: 1 }).unwrap();
        assert_eq!(c.fmt.number_format.as_deref(), Some("#,##0"));
        assert_eq!(c.value, Value::Number(1200.0), "値が壊れた");
    }

    #[test]
    fn a_plain_style_gets_no_index() {
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
    fn a_cell_with_only_borders_survives() {
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
    use book::{Cell, Pos, Value};
    use book::{Book, Sheet};

    fn roundtrip(b: &Book) -> Book {
        let mut buf = Vec::new();
        crate::xlsx::write(b, std::io::Cursor::new(&mut buf)).unwrap();
        crate::xlsx::read(std::io::Cursor::new(&buf)).unwrap().0
    }

    #[test]
    fn cell_merges_round_trip() {
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
    fn inserting_and_deleting_rows_moves_the_merges() {
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.merges.push((Pos::parse("A3").unwrap(), Pos::parse("C3").unwrap()));
        s.insert_row(1);
        assert_eq!(s.merges[0], (Pos::parse("A4").unwrap(), Pos::parse("C4").unwrap()),
                   "結合が置き去りになった");
        s.remove_row(1);
        assert_eq!(s.merges[0], (Pos::parse("A3").unwrap(), Pos::parse("C3").unwrap()));
    }

    #[test]
    fn a_collapsed_merge_disappears() {
        // A1:A2 の縦結合で2行目を抜くと、1セルになる。1セルの結合は結合ではない
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.merges.push((Pos::parse("A1").unwrap(), Pos::parse("A2").unwrap()));
        s.remove_row(1);
        assert!(s.merges.is_empty(), "1セルの結合が残った: {:?}", s.merges);
    }

    #[test]
    fn a_swallowed_position_can_be_found() {
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.merges.push((Pos::parse("A1").unwrap(), Pos::parse("B2").unwrap()));
        assert!(!s.covered_by_merge(Pos::parse("A1").unwrap()), "左上まで呑んだ");
        assert!(s.covered_by_merge(Pos::parse("B2").unwrap()));
        assert!(!s.covered_by_merge(Pos::parse("C1").unwrap()));
    }
}

#[cfg(test)]
mod colwidth_round {
    use book::{Cell, Pos, Value};
    use book::{Book, Sheet};

    #[test]
    fn column_widths_round_trip() {
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
    fn inserting_and_deleting_columns_moves_the_widths() {
        let mut s = Sheet { name: "帳票".into(), ..Default::default() };
        s.col_width.insert(1, 20.0);
        s.insert_col(0);
        assert_eq!(s.col_width.get(&2), Some(&20.0), "幅が置き去り: {:?}", s.col_width);
        s.remove_col(0);
        assert_eq!(s.col_width.get(&1), Some(&20.0));
    }

    #[test]
    fn the_column_widths_of_a_real_template_read() {
        let p = "/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki/実施要領様式7_提案見積書.xlsx";
        let Ok(f) = std::fs::File::open(p) else { return }; // 無い機械では飛ばす
        let (book, _) = crate::xlsx::read(f).unwrap();
        let n: usize = book.sheets.iter().map(|s| s.col_width.len()).sum();
        assert!(n > 0, "実物の列幅を1つも読めていない");
    }
}

#[cfg(test)]
mod rowheight_round {
    use book::{Cell, Pos, Value};
    use book::{Book, Sheet};

    #[test]
    fn row_heights_round_trip() {
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
    fn inserting_and_deleting_rows_moves_the_heights() {
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
    use book::{Cell, Pos};
    use book::{Book, Sheet};
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
    fn open_and_save_keeps_the_parts() {
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

    /// 原本の `docProps/custom.xml` を、宣言(Content_Types)と
    /// 関係(_rels/.rels)ごと差した xlsx を作る。
    fn xlsx_with_custom(inner: &str) -> Vec<u8> {
        let mut book = Book::default();
        book.sheets.push(Sheet { name: "帳票".into(), ..Default::default() });
        let mut base = Vec::new();
        crate::xlsx::write(&book, Cursor::new(&mut base)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&base)).unwrap();
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o: zip::write::FileOptions<'_, ()> = Default::default();
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).unwrap();
            if name == "[Content_Types].xml" {
                buf = String::from_utf8(buf).unwrap().replace("</Types>",
                    r#"<Override PartName="/docProps/custom.xml" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/></Types>"#).into_bytes();
            }
            if name == "_rels/.rels" {
                buf = String::from_utf8(buf).unwrap().replace("</Relationships>",
                    r#"<Relationship Id="rIdCustom" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties" Target="docProps/custom.xml"/></Relationships>"#).into_bytes();
            }
            out.start_file(name, o).unwrap();
            out.write_all(&buf).unwrap();
        }
        out.start_file("docProps/custom.xml", o).unwrap();
        out.write_all(format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">{inner}</Properties>"#).as_bytes()).unwrap();
        out.finish().unwrap().into_inner()
    }

    /// 部品・宣言・関係の3つを一度に見る(どれか1つでも欠けたら包みが壊れる)
    fn custom_parts(saved: &[u8]) -> (bool, bool, bool, String) {
        let mut z = zip::ZipArchive::new(Cursor::new(saved)).unwrap();
        let names: Vec<String> =
            (0..z.len()).map(|i| z.by_index(i).unwrap().name().into()).collect();
        let part = names.iter().any(|n| n == "docProps/custom.xml");
        let mut ct = String::new();
        z.by_name("[Content_Types].xml").unwrap().read_to_string(&mut ct).unwrap();
        let mut rels = String::new();
        z.by_name("_rels/.rels").unwrap().read_to_string(&mut rels).unwrap();
        let mut body = String::new();
        if part {
            z.by_name("docProps/custom.xml").unwrap().read_to_string(&mut body).unwrap();
        }
        (part, ct.contains("/docProps/custom.xml"), rels.contains("docProps/custom.xml"), body)
    }

    #[test]
    fn the_originals_custom_properties_survive_open_and_save() {
        let src = xlsx_with_custom(
            r#"<property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="発注番号"><vt:lpwstr>A-1234</vt:lpwstr></property>"#,
        );
        let (book, _) = crate::xlsx::read(Cursor::new(&src)).unwrap();
        assert_eq!(book.props.custom.len(), 1, "読めていない");
        assert_eq!(book.props.custom[0].name, "発注番号");
        let mut saved = Vec::new();
        crate::xlsx::write_with(&book, Some(Cursor::new(&src)), Cursor::new(&mut saved)).unwrap();
        let (part, ct, rels, body) = custom_parts(&saved);
        assert!(part && ct && rels, "部品/宣言/関係のどれかが消えた: {part} {ct} {rels}");
        assert!(body.contains("A-1234") && body.contains("発注番号"), "中身が変わった: {body}");
    }

    #[test]
    fn the_four_custom_property_types_round_trip() {
        use book::{CustomProp, CustomVal};
        let mut b = Book::new();
        let mk = |n: &str, v: CustomVal| CustomProp { name: n.into(), value: v, link: None };
        b.props.custom = vec![
            mk("発注番号", CustomVal::Text("A-1234 <検>".into())),
            mk("数量", CustomVal::Number(12.5)),
            mk("納期", CustomVal::Date("2026-08-13T00:00:00Z".into())),
            mk("承認済み", CustomVal::Bool(true)),
        ];
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        // 新規ブックでも部品・宣言・関係の3つが揃う
        let (part, ct, rels, _) = custom_parts(&buf);
        assert!(part && ct && rels, "3つ揃わない: {part} {ct} {rels}");
        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        assert_eq!(back.props.custom, b.props.custom, "カスタムプロパティが往復しない");
    }

    #[test]
    fn links_to_unknown_types_and_targets_are_kept() {
        // vt:i4 はこちらが型として持たない。linkTarget も繋ぎ直さない。
        // **どちらも保存で同じ姿に戻す**のが約束(黙って落とさない)
        let src = xlsx_with_custom(
            r#"<property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="通し番号"><vt:i4>7</vt:i4></property><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="3" name="部署" linkTarget="部署名"><vt:lpwstr>総務</vt:lpwstr></property>"#,
        );
        let (book, _) = crate::xlsx::read(Cursor::new(&src)).unwrap();
        assert_eq!(book.props.custom.len(), 2);
        assert_eq!(
            book.props.custom[0].value,
            book::CustomVal::Other("i4".into(), "7".into()),
            "知らない型を落とした"
        );
        assert_eq!(book.props.custom[1].link.as_deref(), Some("部署名"));
        let mut saved = Vec::new();
        crate::xlsx::write_with(&book, Some(Cursor::new(&src)), Cursor::new(&mut saved)).unwrap();
        let (_, _, _, body) = custom_parts(&saved);
        assert!(body.contains("<vt:i4>7</vt:i4>"), "知らない型が保存で化けた: {body}");
        assert!(body.contains(r#"linkTarget="部署名""#), "リンクが消えた: {body}");
    }

    #[test]
    fn clearing_every_custom_property_removes_the_part_and_relation() {
        // 部品だけ消して宣言や関係を残すと、包みが「無い先」を指す —
        // Excel はそれを「修復しました」と言って開く
        let src = xlsx_with_custom(
            r#"<property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="発注番号"><vt:lpwstr>A-1234</vt:lpwstr></property>"#,
        );
        let (mut book, _) = crate::xlsx::read(Cursor::new(&src)).unwrap();
        book.props.custom.clear();
        let mut saved = Vec::new();
        crate::xlsx::write_with(&book, Some(Cursor::new(&src)), Cursor::new(&mut saved)).unwrap();
        let (part, ct, rels, _) = custom_parts(&saved);
        assert!(!part, "部品が残っている");
        assert!(!ct, "宣言が残っている");
        assert!(!rels, "関係が残っている(無い先を指す)");
    }

    #[test]
    fn several_authors_round_trip_through_the_separator() {
        let mut b = Book::new();
        b.props.creators = vec!["山田 太郎".into(), "鈴木 花子".into()];
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&buf)).unwrap();
        let mut core = String::new();
        z.by_name("docProps/core.xml").unwrap().read_to_string(&mut core).unwrap();
        assert!(core.contains("山田 太郎; 鈴木 花子"), "`;` で繋がっていない: {core}");
        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        assert_eq!(back.props.creators, ["山田 太郎", "鈴木 花子"], "著者が往復しない");
    }

    #[test]
    fn blank_authors_and_extra_separators_are_not_counted() {
        // 「山田;」は1人。名無しの2人目はいない
        assert_eq!(crate::xlsx::read::split_creators("山田; ; 鈴木 ;"), ["山田", "鈴木"]);
        assert_eq!(crate::xlsx::read::split_creators("").len(), 0);
        // 名前そのものの `;` は繋ぐ前に落とす(開き直して2人に化けさせない)
        let one = vec!["山;田".to_string()];
        assert_eq!(crate::xlsx::read::split_creators(&crate::xlsx::write::join_creators(&one)).len(), 1);
    }


    #[test]
    fn the_name_of_an_added_shape_round_trips() {
        // prstGeom の名前をそのまま種類にしているので、Excel で作った形も
        // ここを通る。**名前を知らない形に丸めない**
        let mut b = Book::new();
        for (i, k) in ["star5", "hexagon", "wedgeEllipseCallout", "flowChartDecision"]
            .iter()
            .enumerate()
        {
            b.sheets[0].shapes_new.push(book::SheetShape {
                at: Pos::new(i as u32, 0),
                width_px: 100.0,
                height_px: 60.0,
                kind: (*k).into(),
                fill: Some("DCE6F1".into()),
                ..Default::default()
            });
        }
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        let got: Vec<String> = back.sheets[0].shapes.iter().map(|s| s.kind.clone()).collect();
        assert_eq!(got, ["star5", "hexagon", "wedgeEllipseCallout", "flowChartDecision"],
                   "図形の種類が往復しない");
    }

    #[test]
    fn an_undrawable_shape_is_reported_not_silently_squared() {
        // 保存では元の prstGeom の名前を返すのでファイルは壊れない。
        // だが画面では四角に見える — **見える物が違うなら言う**
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(book::SheetShape {
            at: Pos::new(0, 0),
            width_px: 80.0,
            height_px: 50.0,
            kind: "cube".into(), // こちらが作図を持たない形
            fill: Some("DCE6F1".into()),
            ..Default::default()
        });
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let (back, rep) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        assert_eq!(back.sheets[0].shapes[0].kind, "cube", "名前まで失っている");
        assert!(
            rep.unsupported.iter().any(|(n, _)| n.contains("描けない形")),
            "描けない形を黙って通した: {:?}",
            rep.unsupported
        );
    }


    #[test]
    fn text_box_layout_round_trips() {
        use book::{HAlign, TextAnchor, TextFmt};
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(book::SheetShape {
            at: Pos::new(0, 0),
            width_px: 200.0,
            height_px: 90.0,
            kind: "rect".into(),
            text: Some("見出し".into()),
            text_fmt: TextFmt {
                align: HAlign::Center,
                anchor: TextAnchor::Middle,
                vertical: true,
                ins_mm: TextFmt::default().ins_mm,
                bullet: Some(true),
                strike: true,
                sup: true,
                sub: false,
                size_pt: Some(14.0),
            },
            ..Default::default()
        });
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        let sp = &back.sheets[0].shapes[0];
        assert_eq!(sp.text.as_deref(), Some("見出し"));
        assert_eq!(sp.text_fmt, b.sheets[0].shapes_new[0].text_fmt, "組み方が往復しない");
    }

    #[test]
    fn the_default_layout_writes_no_attribute() {
        // **書かないことが既定を表す**(xlsx の作法)。既定で属性を書くと、
        // Excel で開いたとき「わざわざ左寄せにした」ことになる
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(book::SheetShape {
            at: Pos::new(0, 0),
            width_px: 100.0,
            height_px: 50.0,
            kind: "rect".into(),
            text: Some("素".into()),
            ..Default::default()
        });
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&buf)).unwrap();
        let mut dx = String::new();
        let name = (0..z.len())
            .map(|i| z.by_index(i).unwrap().name().to_string())
            .find(|n| n.starts_with("xl/drawings/drawing") && n.ends_with(".xml"))
            .expect("drawing が無い");
        z.by_name(&name).unwrap().read_to_string(&mut dx).unwrap();
        for attr in ["anchor=", "vert=", "algn=", "strike=", "baseline=", "buChar", "buAutoNum"] {
            assert!(!dx.contains(attr), "既定なのに {attr} を書いた:\n{dx}");
        }
    }

    #[test]
    fn superscript_and_subscript_are_never_both_on() {
        // baseline は1つの属性。両方立てても書けるのは片方だけ
        use book::TextFmt;
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(book::SheetShape {
            at: Pos::new(0, 0),
            width_px: 100.0,
            height_px: 50.0,
            kind: "rect".into(),
            text: Some("x".into()),
            text_fmt: TextFmt { sup: true, sub: true, ..Default::default() },
            ..Default::default()
        });
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        let tf = &back.sheets[0].shapes[0].text_fmt;
        assert!(tf.sup != tf.sub, "上付きと下付きが両方立っている: {tf:?}");
    }


    #[test]
    fn layout_is_folded_per_shape() {
        // **1つだけの往復では出ない穴。** 読みの途中の控えを図形ごとに
        // 畳んでいないと、前の箱の揃えや箇条書きが次へ漏れる
        // (2026-08-13、6つ並べた実物の見本で見つけた)
        use book::{HAlign, TextAnchor, TextFmt};
        let mut b = Book::new();
        let mk = |row: u32, t: &str, tf: TextFmt| book::SheetShape {
            at: Pos::new(row, 0),
            width_px: 100.0,
            height_px: 50.0,
            kind: "rect".into(),
            text: Some(t.into()),
            text_fmt: tf,
            ..Default::default()
        };
        b.sheets[0].shapes_new.push(mk(0, "飾りつき", TextFmt {
            align: HAlign::Right,
            anchor: TextAnchor::Bottom,
            bullet: Some(true),
            strike: true,
            ..Default::default()
        }));
        b.sheets[0].shapes_new.push(mk(4, "素のまま", TextFmt::default()));
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        assert_eq!(back.sheets[0].shapes.len(), 2);
        assert_eq!(
            back.sheets[0].shapes[1].text_fmt,
            TextFmt::default(),
            "前の図形の組み方が漏れている"
        );
    }


    #[test]
    fn comment_threads_authors_and_resolution_round_trip() {
        use book::{CommentEntry, CommentThread};
        let mut b = Book::new();
        let p = Pos::parse("A1").unwrap();
        b.sheets[0].set(p, Cell::input("値"));
        b.sheets[0].comments.insert(p, CommentThread {
            done: true,
            entries: vec![
                CommentEntry { who: "山田 太郎".into(), when: "2026-08-01T09:00:00Z".into(),
                               text: "元のコメント".into() },
                CommentEntry { who: "鈴木 花子".into(), when: "2026-08-02T10:00:00Z".into(),
                               text: "返信です".into() },
            ],
        });
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        assert_eq!(
            back.sheets[0].comments.get(&p),
            b.sheets[0].comments.get(&p),
            "コメントの筋が往復しない"
        );
    }

    #[test]
    fn an_edited_comment_stays_consistent_with_the_legacy_copy() {
        // **これが直したかった穴。** 近代の Excel はスレッド側を見るので、
        // 古い写しだけ書き換えると、直した文が Excel に映らない
        // (2026-08-13 に実測してから直した)
        use book::CommentThread;
        let mut b = Book::new();
        let p = Pos::parse("A1").unwrap();
        b.sheets[0].comments.insert(p, CommentThread::new("元の文", "山田"));
        let mut src = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut src)).unwrap();

        let (mut back, _) = crate::xlsx::read(Cursor::new(&src)).unwrap();
        back.sheets[0].comments.get_mut(&p).unwrap().entries[0].text = "直した文".into();
        let mut saved = Vec::new();
        crate::xlsx::write_with(&back, Some(Cursor::new(&src)), Cursor::new(&mut saved)).unwrap();

        let mut z = zip::ZipArchive::new(Cursor::new(&saved)).unwrap();
        for n in ["xl/threadedComments/threadedComment1.xml", "xl/comments1.xml"] {
            let mut s = String::new();
            z.by_name(n).unwrap().read_to_string(&mut s).unwrap();
            assert!(s.contains("直した文"), "{n} が古いまま");
            assert!(!s.contains("元の文"), "{n} に古い文が残っている");
        }
    }

    #[test]
    fn comment_authors_read_from_older_workbooks_too() {
        // 前は <authors> と authorId を捨てていたので、誰が書いたか
        // 分からなくなっていた
        let mut b = Book::new();
        let p = Pos::parse("B2").unwrap();
        b.sheets[0].comments.insert(p, book::CommentThread::new("覚書", "サンプル商事"));
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&buf)).unwrap();
        let mut cs = String::new();
        z.by_name("xl/comments1.xml").unwrap().read_to_string(&mut cs).unwrap();
        assert!(cs.contains("<author>サンプル商事</author>"), "写しに著者が無い: {cs}");
        // 写しだけを読む道(スレッドの部品が無い古いブック)でも著者が出る
        let got = crate::xlsx::read::parse_comments(&cs);
        assert_eq!(got[0].1.entries[0].who, "サンプル商事", "著者を読み落とした");
    }


    #[test]
    fn sparkline_point_markers_round_trip() {
        use book::SparkMarks;
        let mut b = Book::new();
        for (kind, base) in [("spark", 0.0f32), ("spark-col", 1.0), ("spark-wl", 0.5)] {
            b.sheets[0].shapes_new.push(book::SheetShape {
                at: Pos::new(0, 0),
                width_px: 90.0,
                height_px: 24.0,
                kind: kind.into(),
                line: Some("1B6E3C".into()),
                points: vec![book::PathPoint::at(0.0, 0.8), book::PathPoint::at(0.33, 0.2), book::PathPoint::at(0.66, 0.6), book::PathPoint::at(1.0, 0.1)],
                base,
                spark_marks: SparkMarks { high: true, low: true, last: true, ..Default::default() },
                ..Default::default()
            });
        }
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        for sp in &back.sheets[0].shapes {
            assert_eq!(
                sp.spark_marks,
                SparkMarks { high: true, low: true, last: true, ..Default::default() },
                "{} の点の印が往復しない",
                sp.kind
            );
        }
    }

    #[test]
    fn an_old_entry_without_the_flag_column_still_reads() {
        // 札は `jo:種類:底` だった。**印は後から足した欄**なので、
        // 古いブックの札を壊さずに読めなければならない
        use book::SparkMarks;
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(book::SheetShape {
            at: Pos::new(0, 0),
            width_px: 90.0,
            height_px: 24.0,
            kind: "spark-col".into(),
            line: Some("1B6E3C".into()),
            points: vec![book::PathPoint::at(0.0, 0.8), book::PathPoint::at(0.5, 0.2)],
            base: 1.0,
            ..Default::default()
        });
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        // 3欄目を削って古い札に戻す
        let s = String::from_utf8_lossy(&buf).to_string();
        assert!(s.contains("jo:spark-col:1.0000:") || !s.contains("jo:"), "札の形が変わった");
        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        assert_eq!(back.sheets[0].shapes[0].kind, "spark-col", "種類を読み違えた");
        assert!((back.sheets[0].shapes[0].base - 1.0).abs() < 0.01, "底を読み違えた");
        assert_eq!(back.sheets[0].shapes[0].spark_marks, SparkMarks::default());
    }


    #[test]
    fn curve_control_points_round_trip() {
        use book::PathPoint;
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(book::SheetShape {
            at: Pos::new(0, 0),
            width_px: 160.0,
            height_px: 100.0,
            kind: "path".into(),
            fill: Some("DCE6F1".into()),
            line: Some("1B6E3C".into()),
            points: vec![
                PathPoint::at(0.0, 1.0),
                PathPoint { at: (0.5, 0.0), start: false, c_in: Some((0.2, 0.0)), c_out: Some((0.8, 0.0)) },
                PathPoint::at(1.0, 1.0),
            ],
            ..Default::default()
        });
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&buf)).unwrap();
        let name = (0..z.len())
            .map(|i| z.by_index(i).unwrap().name().to_string())
            .find(|n| n.starts_with("xl/drawings/drawing"))
            .unwrap();
        let mut dx = String::new();
        z.by_name(&name).unwrap().read_to_string(&mut dx).unwrap();
        assert!(dx.contains("cubicBezTo"), "曲線を書いていない:\n{dx}");

        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        let got = &back.sheets[0].shapes[0].points;
        assert_eq!(got.len(), 3, "点の数が変わった: {got:?}");
        let near = |a: (f32, f32), b: (f32, f32)| {
            (a.0 - b.0).abs() < 0.002 && (a.1 - b.1).abs() < 0.002
        };
        assert!(near(got[1].at, (0.5, 0.0)), "頂点が動いた: {:?}", got[1]);
        assert!(
            got[1].c_in.is_some_and(|c| near(c, (0.2, 0.0))),
            "入りの制御点が往復しない: {:?}",
            got[1]
        );
        assert!(
            got[1].c_out.is_some_and(|c| near(c, (0.8, 0.0))),
            "出の制御点が往復しない: {:?}",
            got[1]
        );
    }

    #[test]
    fn a_polyline_without_control_points_stays_straight() {
        // **スパークラインやペンの線を曲線にしない。** 制御点が無いなら
        // lnTo のまま — 曲線で書くと Excel 側の見た目がわずかに変わる
        use book::PathPoint;
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(book::SheetShape {
            at: Pos::new(0, 0),
            width_px: 90.0,
            height_px: 24.0,
            kind: "spark".into(),
            line: Some("1B6E3C".into()),
            points: vec![PathPoint::at(0.0, 0.8), PathPoint::at(0.5, 0.2), PathPoint::at(1.0, 0.5)],
            ..Default::default()
        });
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&buf)).unwrap();
        let name = (0..z.len())
            .map(|i| z.by_index(i).unwrap().name().to_string())
            .find(|n| n.starts_with("xl/drawings/drawing"))
            .unwrap();
        let mut dx = String::new();
        z.by_name(&name).unwrap().read_to_string(&mut dx).unwrap();
        assert!(!dx.contains("cubicBezTo"), "直線を曲線で書いた:\n{dx}");
        assert!(dx.contains("lnTo"), "直線を書いていない");
    }


    #[test]
    fn the_scope_of_a_name_round_trips() {
        // **重なっていなくてもシート限定にできる。** 前は「同じ名前が
        // 2枚にあるか」から当てていたので、これが作れなかった
        use book::DefinedName;
        let mut b = Book::new();
        b.sheets.push(book::Sheet::new("2枚目"));
        b.sheets[0].names.push(DefinedName { name: "限定".into(), range: "A1".into(), scoped: true });
        b.sheets[0].names.push(DefinedName { name: "全体".into(), range: "B2".into(), scoped: false });
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let (back, _) = crate::xlsx::read(Cursor::new(&buf)).unwrap();
        let g = |n: &str| back.sheets[0].names.iter().find(|d| d.name == n).cloned();
        assert!(g("限定").is_some_and(|d| d.scoped), "シート限定が往復しない");
        assert!(g("全体").is_some_and(|d| !d.scoped), "ブック全体がシート限定に落ちた");
    }

    #[test]
    fn an_overlapping_workbook_wide_name_is_narrowed_on_export() {
        // ブック全体の名前が2つあると Excel は開けない。**印が無くても防ぐ**
        use book::DefinedName;
        let mut b = Book::new();
        b.sheets.push(book::Sheet::new("2枚目"));
        b.sheets[0].names.push(DefinedName::new("売上", "A1"));
        b.sheets[1].names.push(DefinedName::new("売上", "B2"));
        let mut buf = Vec::new();
        crate::xlsx::write(&b, Cursor::new(&mut buf)).unwrap();
        let mut z = zip::ZipArchive::new(Cursor::new(&buf)).unwrap();
        let mut wb = String::new();
        z.by_name("xl/workbook.xml").unwrap().read_to_string(&mut wb).unwrap();
        assert_eq!(wb.matches("localSheetId").count(), 2, "重なりを限定していない: {wb}");
    }

    #[test]
    fn the_old_calculation_chain_is_not_carried_over() {
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
    use book::Cell;
    use book::calc::recalc;

    #[test]
    fn defined_names_round_trip_and_work_in_formulas() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("100"));
        b.sheets[0].set(Pos::parse("B1").unwrap(), Cell::input("=単価*2"));
        b.sheets[0].names.push(book::DefinedName::new("単価", "A1"));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (mut back, _) = read(buf).expect("読めない");
        assert_eq!(back.sheets[0].names, vec![book::DefinedName::new("単価", "A1")],
            "名前が往復しない");
        recalc(&mut back.sheets[0]);
        assert_eq!(back.sheets[0].value(Pos::parse("B1").unwrap()), Value::Number(200.0));
    }

    #[test]
    fn a_real_files_print_area_is_not_broken() {
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
    use book::Cell;

    fn roundtrip(b: &Book) -> Book {
        let mut buf = Cursor::new(Vec::new());
        write(b, &mut buf).expect("書けない");
        buf.set_position(0);
        read(buf).expect("読めない").0
    }

    #[test]
    fn hyperlinks_round_trip() {
        let mut b = Book::new();
        let p = Pos::parse("B2").unwrap();
        b.sheets[0].set(p, Cell::input("会社サイト"));
        b.sheets[0].links.insert(p, "https://example.co.jp/".into());
        let back = roundtrip(&b);
        assert_eq!(back.sheets[0].links.get(&p).map(|s| s.as_str()),
            Some("https://example.co.jp/"), "リンクが往復しない");
    }

    #[test]
    fn an_internal_link_round_trips_through_location() {
        let mut b = Book::new();
        b.sheets.push(book::Sheet::new("集計"));
        let p = Pos::parse("B2").unwrap();
        b.sheets[0].set(p, Cell::input("集計へ"));
        b.sheets[0].links.insert(p, "#集計!B5".into());
        let back = roundtrip(&b);
        assert_eq!(back.sheets[0].links.get(&p).map(|s| s.as_str()),
            Some("#集計!B5"), "帳面の中へのリンクが往復しない");
    }

    #[test]
    fn data_bars_color_scales_and_icon_sets_round_trip() {
        use book::{CondKind, CondRule};
        let mut b = Book::new();
        for (i, v) in ["10", "20", "30"].iter().enumerate() {
            b.sheets[0].set(Pos::new(i as u32, 0), Cell::input(v));
        }
        let range = (Pos::new(0, 0), Pos::new(2, 0));
        b.sheets[0].cond.push(CondRule {
            range, kind: CondKind::Bar("638EC6".into()), look: Default::default() });
        b.sheets[0].cond.push(CondRule {
            range,
            kind: CondKind::Scale("F8696B".into(), Some("FFEB84".into()), "63BE7B".into()),
            look: Default::default() });
        b.sheets[0].cond.push(CondRule {
            range, kind: CondKind::Icons("3Arrows".into()), look: Default::default() });
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
    fn a_column_sparkline_round_trips_as_a_column() {
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(book::SheetShape {
            at: Pos::parse("C2").unwrap(),
            width_px: 90.0,
            height_px: 22.0,
            kind: "spark-col".into(),
            line: Some("1B6E3C".into()),
            points: vec![book::PathPoint::at(0.17, 0.0), book::PathPoint::at(0.5, 0.9), book::PathPoint::at(0.83, 0.25)],
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
        assert!((sp.points[1].at.0 - 0.5).abs() < 0.02, "中心が違う: {:?}", sp.points[1]);
        assert!((sp.points[1].at.1 - 0.9).abs() < 0.02, "先端が違う: {:?}", sp.points[1]);
    }

    #[test]
    fn comments_round_trip() {
        let mut b = Book::new();
        let p = Pos::parse("C3").unwrap();
        b.sheets[0].set(p, Cell::input("単価"));
        b.sheets[0].comments.insert(p, "去年の実績から仮置き。要確認".into());
        let back = roundtrip(&b);
        assert_eq!(back.sheets[0].comments.get(&p).map(|s| s.text()),
            Some("去年の実績から仮置き。要確認"), "コメントが往復しない");
    }

    #[test]
    fn adding_a_comment_to_a_real_file_creates_the_parts() {
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
        assert_eq!(back.sheets[0].comments.get(&p).map(|s| s.text()),
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
    use book::{Cell, CondAux, CondKind, CondLook, CondOp, CondRule};

    /// **dxf は色と塗りだけではない。** 太字・斜体・下線・取り消し線も
    /// 持てる。前は読んでおらず、Excel で「赤字・太字」にした規則が
    /// **規則は残ったまま太字と文字色だけ落ちて**開いていた
    /// (2026-08-10 pyoffice セッションの報告)
    #[test]
    fn reads_decorations() {
        let look = |body: &str| {
            super::parse_dxfs(&format!(
                r#"<styleSheet><dxfs count="1"><dxf>{body}</dxf></dxfs></styleSheet>"#
            ))
            .first()
            .cloned()
            .unwrap_or_default()
        };
        // 向こうの試験が使っている形(xlsx-sidecar.test.ts の雛形)
        let lk = look(
            r#"<font><b/><color rgb="FF9C0006"/></font><fill><patternFill><bgColor rgb="FFFFC7CE"/></patternFill></fill>"#,
        );
        assert_eq!(lk.bold, Some(true), "太字が落ちている");
        assert_eq!(lk.color.as_deref(), Some("9C0006"), "文字色が落ちている");
        assert_eq!(lk.fill.as_deref(), Some("FFC7CE"));

        let lk = look(r#"<font><b/><i/><u/><strike/></font>"#);
        assert_eq!(
            (lk.bold, lk.italic, lk.underline, lk.strike),
            (Some(true), Some(true), Some(true), Some(true)),
            "4つとも読めていない"
        );

        // **三択を潰さない。** 書いていない=触らない、val="0"=外す
        let lk = look(r#"<font><b val="0"/><u val="none"/></font>"#);
        assert_eq!(lk.bold, Some(false), "val=0 は「外す」");
        assert_eq!(lk.underline, Some(false), "u val=none は「外す」");
        assert_eq!(lk.italic, None, "書いていない飾りまで決めている");
        assert_eq!(look("").bold, None, "font が無いのに太字を決めている");
    }

    /// 飾りが保存で消えないこと。**読めたのに書かないと、開いて保存した
    /// だけで帳票が痩せる**
    #[test]
    fn decorations_round_trip() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("-5"));
        b.sheets[0].cond.push(CondRule {
            range: (Pos::parse("A1").unwrap(), Pos::parse("A9").unwrap()),
            kind: CondKind::Cmp(CondOp::Lt, 0.0),
            look: CondLook {
                color: Some("9C0006".into()),
                fill: Some("FFC7CE".into()),
                bold: Some(true),
                italic: Some(true),
                underline: Some(false),
                strike: None,
            },
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let r = read(buf).expect("読めない").0;
        let lk = &r.sheets[0].cond[0].look;
        assert_eq!(lk.color.as_deref(), Some("9C0006"), "文字色が往復しない");
        assert_eq!(lk.fill.as_deref(), Some("FFC7CE"), "塗りが往復しない");
        assert_eq!(lk.bold, Some(true), "太字が往復しない");
        assert_eq!(lk.italic, Some(true), "斜体が往復しない");
        assert_eq!(lk.underline, Some(false), "「下線を外す」が往復しない");
        assert_eq!(lk.strike, None, "触らないはずの取り消し線が決まっている");
    }

    #[test]
    fn fill_reads_from_either_fg_color_or_bg_color() {
        // 書き手ごとに置き場所が違う。片方しか見ないと、条件付き書式の
        // 色が**黙って消える**(規則は残るので気付きにくい)
        let dxf = |body: &str| {
            super::parse_dxfs(&format!(
                r#"<styleSheet><dxfs count="1"><dxf>{body}</dxf></dxfs></styleSheet>"#
            ))
            .first()
            .cloned()
            .map(|lk| lk.fill)
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
    fn conditional_formatting_round_trips() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("-5"));
        b.sheets[0].cond.push(CondRule {
            range: (Pos::parse("A1").unwrap(), Pos::parse("A9").unwrap()),
            kind: CondKind::Cmp(CondOp::Lt, 0.0),
            look: CondLook {
                color: Some("C00000".into()),
                ..Default::default()
            },
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let r = &back.sheets[0].cond;
        assert_eq!(r.len(), 1, "規則が往復しない");
        assert_eq!(r[0].kind, CondKind::Cmp(CondOp::Lt, 0.0));
        assert_eq!(r[0].look.color.as_deref(), Some("C00000"), "見た目(dxf)が往復しない");
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
    fn a_banding_rule_given_by_formula_round_trips_and_works() {
        // 実物の帳票でいちばん多い条件付き書式。読めないでは済まない
        let mut b = Book::new();
        for i in 0..10u32 {
            b.sheets[0].set(Pos::new(i, 0), Cell::input(&format!("{}", i + 1)));
        }
        b.sheets[0].cond.push(CondRule {
            range: (Pos::parse("A1").unwrap(), Pos::parse("B10").unwrap()),
            kind: CondKind::Formula("MOD(ROW(),2)=0".into()),
            look: CondLook {
                fill: Some("DDEBF7".into()),
                ..Default::default()
            },
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
        assert_eq!(r[0].look.fill.as_deref(), Some("DDEBF7"), "見た目(dxf)が往復しない");
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
    fn a_formula_rule_shifts_relative_references_from_the_top_left() {
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
            look: CondLook {
                color: Some("C00000".into()),
                ..Default::default()
            },
        });
        sh.cond.push(CondRule {
            range: (Pos::parse("B2").unwrap(), Pos::parse("C3").unwrap()),
            kind: CondKind::Formula(r#"$A2="済""#.into()),
            look: CondLook {
                fill: Some("FFF2CC".into()),
                ..Default::default()
            },
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
    fn an_unresolvable_formula_rule_keeps_its_source() {
        // 評価に失敗しても**ファイルは減らない** — 保存はいつも原文を返す
        let mut b = Book::new();
        let f = "COUNTIF(知らない表!A:A,A1)>0";
        b.sheets[0].cond.push(CondRule {
            range: (Pos::parse("A1").unwrap(), Pos::parse("A3").unwrap()),
            kind: CondKind::Formula(f.into()),
            look: CondLook {
                fill: Some("FCE4D6".into()),
                ..Default::default()
            },
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
    fn new_rule_kinds_round_trip_and_work() {
        let mut b = Book::new();
        let s = &mut b.sheets[0];
        for (i, v) in ["10", "20", "20", "5"].iter().enumerate() {
            s.set(Pos::new(i as u32, 0), Cell::input(v));
        }
        let range = (Pos::new(0, 0), Pos::new(3, 0));
        s.cond.push(CondRule { range, kind: CondKind::Between(8.0, 15.0, false), look: CondLook { fill: Some("FFF2CC".into()), ..Default::default() } });
        s.cond.push(CondRule { range, kind: CondKind::Text("2".into()), look: CondLook { fill: Some("E2EFDA".into()), ..Default::default() } });
        s.cond.push(CondRule { range, kind: CondKind::Dup(false), look: CondLook { color: Some("9C0006".into()), ..Default::default() } });
        s.cond.push(CondRule { range, kind: CondKind::Top(2, false), look: CondLook { fill: Some("D9E1F2".into()), ..Default::default() } });
        s.cond.push(CondRule { range, kind: CondKind::Avg(false), look: Default::default() });
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
    use book::{Cell, Validation};

    /// **壊れた xlsx から拾う**(2026-08-22。台帳「開いて修復」)。
    ///
    /// 中央目録が読めなくても、局所ヘッダから部品を拾えることを見ます。
    /// 拾えなかった部品は**黙って落とさず名前を並べる**ことも見ます。
    #[test]
    fn a_broken_central_directory_can_still_be_read() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("品名"));
        b.sheets[0].set(Pos::parse("A2").unwrap(), Cell::input("鉛筆"));
        b.sheets[0].set(Pos::parse("B2").unwrap(), Cell::input("100"));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let from = buf.into_inner();

        // 普通の読み手はこれを開けない状態にする — 末尾(中央目録)を落とす
        let broken = &from[..from.len() - 40];
        assert!(read(Cursor::new(broken.to_vec())).is_err(), "壊れていない");

        let s = crate::xlsx::salvage(broken);
        assert!(s.any(), "1つも拾えない: {:?}", s.lost);
        assert!(
            s.kept.iter().any(|n| n.contains("sheet1.xml")),
            "本体を拾えていない: {:?}",
            s.kept
        );
        let (back, _) = read(Cursor::new(s.bytes)).expect("拾った物が読めない");
        assert_eq!(
            back.sheets[0].get(Pos::parse("A2").unwrap()).map(|c| c.value.display()),
            Some("鉛筆".to_string()),
            "中身が拾えていない"
        );
    }

    /// 部品の中身が化けていたら、**その部品だけを捨てて名前を控える**。
    #[test]
    fn a_corrupt_part_is_dropped_and_its_name_recorded() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("あ"));
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let mut raw = buf.into_inner();
        // sharedStrings の中身のどこかを塗り潰す(名前は残す)
        let mark = b"xl/sharedStrings.xml";
        let at = raw.windows(mark.len()).position(|w| w == mark).expect("見つからない");
        for i in at + mark.len() + 8..at + mark.len() + 40 {
            if i < raw.len() {
                raw[i] = 0xFF;
            }
        }
        let s = crate::xlsx::salvage(&raw);
        // 他の部品は拾えている
        assert!(s.kept.iter().any(|n| n.contains("workbook.xml")), "{:?}", s.kept);
        // 捨てた物があるなら、名前が出ている
        for (n, why) in &s.lost {
            assert!(!n.is_empty(), "名前の無い報せがある");
            assert!(!why.is_empty(), "理由の無い報せがある");
        }
    }

    /// 何も拾えない物を渡しても、**慌てず「拾えません」と言う**。
    #[test]
    fn a_non_zip_file_yields_nothing() {
        let s = crate::xlsx::salvage(b"this is not a zip at all");
        assert!(!s.any(), "拾えないはずの物から拾った");
        assert!(s.kept.is_empty());
    }

    #[test]
    fn scenarios_round_trip() {
        let mut b = Book::new();
        b.sheets[0].scenarios.push(book::Scenario {
            name: "強気".into(),
            cells: vec![
                (Pos::parse("B2").unwrap(), "120".into()),
                (Pos::parse("B3").unwrap(), "0.08".into()),
            ],
            comment: "単価を上げた場合".into(),
        });
        b.sheets[0].scenarios.push(book::Scenario {
            name: "弱気".into(),
            cells: vec![(Pos::parse("B2").unwrap(), "90".into())],
            comment: String::new(),
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let sc = &back.sheets[0].scenarios;
        assert_eq!(sc.len(), 2, "シナリオが往復しない: {sc:?}");
        assert_eq!(sc[0].name, "強気");
        assert_eq!(sc[0].comment, "単価を上げた場合");
        assert_eq!(sc[0].cells.len(), 2);
        assert_eq!(sc[0].cells[0], (Pos::parse("B2").unwrap(), "120".to_string()));
        assert_eq!(sc[1].name, "弱気");
        assert_eq!(sc[1].comment, "", "覚え書きが無いのに何か入った");
    }

    #[test]
    fn data_validation_round_trips() {
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
    fn validation_rules_other_than_list_are_carried_over() {
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
    fn image_offsets_round_trip() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].images_new.push(book::SheetImage {
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
    fn headers_and_footers_round_trip() {
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
    fn border_style_and_color_round_trip() {
        use book::{BStyle, Edge};
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
    fn pivot_filters_round_trip() {
        let mut b = Book::new();
        b.pivots.push(book::PivotDef {
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
            chart_at: None,
            dest: Pos::parse("E1").unwrap(),
            size: (3, 3),
            hide: vec![("区分".into(), vec!["紙製品".into(), "その他".into()])],
            style: String::new(),
            name: String::new(),
            vfilter: Some((">=".into(), 1000.0)),
            group_by: vec![("日付".into(), "四半期".into()), ("金額".into(), "幅:100".into())],
            show_as: "累計".into(),
            sort: String::new(),
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
    fn manual_calculation_round_trips() {
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
    // **日本語の試験名は家の作法。** ラテン大文字が混じると non_snake_case が鳴る
    #[allow(non_snake_case)]
    fn only_calc_mode_is_replaced_in_the_original_calc_pr() {
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
    fn the_whole_number_rule_and_its_messages_round_trip() {
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
    fn print_settings_are_read() {
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
    fn print_settings_and_print_area_round_trip_through_the_model() {
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

    /// **「紙 N 枚に収める」は、選ばれているときだけ効く。**
    ///
    /// `fitToWidth` は `<pageSetUpPr fitToPage="1"/>` と組で意味を持ちます。
    /// 印が無ければ Excel は `scale` のほうを使います。国税庁の消費税の表は
    /// `scale="78"` と `fitToWidth="2"` の両方を持っていて、後者を使うと
    /// 倍率が 83.7% に**上がり**、紙が1列ぶん増えていました(2026-08-31)。
    #[test]
    fn fit_to_width_needs_the_fit_to_page_mark() {
        use std::io::Write as _;
        let yomu = |shirushi: &str| {
            let b0 = Book::new();
            let mut buf = Cursor::new(Vec::new());
            write(&b0, &mut buf).expect("書けない");
            let mut z = zip::ZipArchive::new(Cursor::new(buf.get_ref().clone())).unwrap();
            let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for i in 0..z.len() {
                let mut f = z.by_index(i).unwrap();
                let name = f.name().to_string();
                let mut s = Vec::new();
                f.read_to_end(&mut s).unwrap();
                if name.ends_with("sheet1.xml") {
                    let t = String::from_utf8(s).unwrap().replace(
                        "</worksheet>",
                        r#"<pageSetup paperSize="9" scale="78" fitToWidth="2" orientation="portrait"/></worksheet>"#,
                    );
                    // sheetPr は worksheet の最初の子でなければならない
                    let t = t.replacen("><", &format!(">{shirushi}<"), 1);
                    s = t.into_bytes();
                }
                w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
                w.write_all(&s).unwrap();
            }
            read(Cursor::new(w.finish().unwrap().into_inner())).expect("読めない").0
        };
        let nashi = yomu("");
        assert_eq!(nashi.sheets[0].fit_to_w, None, "印が無いのに紙に収めようとした");
        assert_eq!(nashi.sheets[0].print_scale, Some(78), "倍率まで落とした");
        let ari = yomu(r#"<sheetPr><pageSetUpPr fitToPage="1"/></sheetPr>"#);
        assert_eq!(ari.sheets[0].fit_to_w, Some(2), "印があるのに読んでいない");
        // **書いていない側の既定は1枚**です(ISO/IEC 29500 の
        // `fitToHeight` は `default="1"`)。総務省の給与所得の表は印だけを
        // 立てて2つとも書いていないので、縦横1枚の意味になります
        assert_eq!(ari.sheets[0].fit_to_h, Some(1), "書いていない側が1枚になっていない");
        // 保存でも印を書く。書かないと Excel 側で効きません
        let mut buf = Cursor::new(Vec::new());
        write(&ari, &mut buf).expect("書けない");
        let mut z = zip::ZipArchive::new(Cursor::new(buf.into_inner())).unwrap();
        let mut s = String::new();
        z.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut s).unwrap();
        assert!(s.contains(r#"<pageSetUpPr fitToPage="1"/>"#), "保存で印が消えた");
        buf = Cursor::new(Vec::new());
        write(&nashi, &mut buf).expect("書けない");
        let mut z = zip::ZipArchive::new(Cursor::new(buf.into_inner())).unwrap();
        let mut s = String::new();
        z.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut s).unwrap();
        assert!(!s.contains("pageSetUpPr"), "選ばれていない印を書いた");
    }

    #[test]
    fn unknown_source_attributes_survive_while_only_the_direction_changes() {
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
    use book::SheetImage;

    fn png() -> Vec<u8> {
        // 実体は問わない(読みは復号しない)。PNG の魔法数だけ本物
        let mut d = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        d.extend_from_slice(&[0; 32]);
        d
    }

    #[test]
    fn an_inserted_image_round_trips() {
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
    fn adding_to_a_file_that_has_images_keeps_both() {
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
    fn scaling_page_breaks_and_title_rows_round_trip() {
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
    fn a_legacy_array_formula_round_trips_and_computes() {
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
        book::calc::recalc(&mut b.sheets[0]);
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
        book::calc::recalc(&mut b2.sheets[0]);
        assert_eq!(b2.sheets[0].get(at).unwrap().value.display(), "140",
                   "往復したら値が変わった");
    }

    #[test]
    // **日本語の試験名は家の作法。** ラテン大文字が混じると non_snake_case が鳴る
    #[allow(non_snake_case)]
    fn an_array_formula_fits_its_declared_range_and_pads_with_na() {
        let mut b = Book::new();
        for i in 0..3u32 {
            b.sheets[0].set(Pos::new(i, 0), Cell::input(&((i + 1) * 2).to_string()));
        }
        // 3つしか返らない式を5つぶんの範囲に入れた(Excel は #N/A で埋める)
        let at = Pos::parse("C1").unwrap();
        b.sheets[0].set(at, Cell::input("=A1:A3*10"));
        b.sheets[0].cse.insert(at, (5, 1));
        book::calc::recalc(&mut b.sheets[0]);
        assert_eq!(b.sheets[0].get(at).unwrap().value.display(), "20");
        assert_eq!(b.sheets[0].get(Pos::parse("C3").unwrap()).unwrap().value.display(), "60");
        assert_eq!(
            b.sheets[0].get(Pos::parse("C4").unwrap()).unwrap().value.display(),
            "#N/A",
            "足りない席が埋まっていない"
        );
    }

    #[test]
    fn the_read_only_recommendation_round_trips() {
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
    fn a_name_is_scoped_to_a_sheet_only_when_it_exists_on_two() {
        // **付けないと「ブック全体の名前が2つ」になって開けないファイルに
        // なる。全部に付けるとブック全体の名前がシート限定に落ちる**
        let mut b = Book::new();
        b.sheets.push(book::Sheet::new("Sheet2"));
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[1].set(Pos::parse("A1").unwrap(), Cell::input("y"));
        b.sheets[0].names.push(book::DefinedName::new("売上", "A1:A3"));
        b.sheets[1].names.push(book::DefinedName::new("売上", "A1:A5")); // 同じ名前
        b.sheets[0].names.push(book::DefinedName::new("税率", "B1")); // こちらは1枚だけ

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
        assert_eq!(back.sheets[0].names.iter().filter(|d| d.name == "売上").count(), 1);
        assert_eq!(back.sheets[1].names.iter().filter(|d| d.name == "売上").count(), 1);
    }

    #[test]
    fn a_template_differs_only_in_its_declaration_and_still_reads() {
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
    fn fit_to_paper_and_vertical_page_breaks_round_trip() {
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
    use book::SheetShape;

    /// **グラフは持たないが、黙って捨てない。**
    ///
    /// officework はグラフの模型を持たない — 描くのは matplotlib で、
    /// 出来上がりは画像として置く(発注者確定)。だから系列も軸も読まない。
    ///
    /// だが 2026-08-11 まで、`graphicFrame` は**見てすらいなかった**
    /// (`grep graphicFrame` で 0 件)。他人の作ったグラフ入りの帳票を開くと、
    /// **出ないだけでなく、出なかったとも言わなかった** — 家訓に反する。
    ///
    /// **「持たない」と「黙って捨てる」は別のこと。** リッチテキストで
    /// 同じ区別をしたのと同じ形。保存では原本の drawing がそのまま
    /// 持ち越されるので、**壊れはしない。**
    #[test]
    fn charts_are_not_read_but_are_logged() {
        let b = Book::new();
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        // 図形の入った drawing を差し込む。**グラフの入れ物だけ**入れて、
        // 中身(系列・軸)は空にしてある — 読み手はそこへ入らない
        let mut z = zip::ZipArchive::new(Cursor::new(buf.get_ref().clone())).unwrap();
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        use std::io::{Read as _, Write as _};
        let mut patched = false;
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut body = Vec::new();
            f.read_to_end(&mut body).unwrap();
            if name.ends_with("sheet1.xml") {
                let t = String::from_utf8(body).unwrap();
                let t = t.replace("</worksheet>", r#"<drawing r:id="rIdD"/></worksheet>"#);
                patched = true;
                body = t.into_bytes();
            }
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&body).unwrap();
        }
        assert!(patched, "型紙を差す先が無い(書き出しの形が変わった)");
        const XDR: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
        const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
        const PKG: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
        const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
        for (name, body) in [
            (
                "xl/worksheets/_rels/sheet1.xml.rels",
                format!(
                    r#"<Relationships xmlns="{PKG}"><Relationship Id="rIdD" Type="{REL}/drawing" Target="../drawings/drawing1.xml"/></Relationships>"#
                ),
            ),
            (
                "xl/drawings/drawing1.xml",
                format!(
                    r#"<xdr:wsDr xmlns:xdr="{XDR}" xmlns:a="{A}"><xdr:twoCellAnchor>
                    <xdr:from><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>
                    <xdr:to><xdr:col>6</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>12</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>
                    <xdr:graphicFrame><xdr:nvGraphicFramePr><xdr:cNvPr id="2" name="売上"/></xdr:nvGraphicFramePr>
                    <a:graphic><a:graphicData/></a:graphic></xdr:graphicFrame><xdr:clientData/>
                    </xdr:twoCellAnchor></xdr:wsDr>"#
                ),
            ),
        ] {
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        let out = w.finish().unwrap();
        let (back, rep) = read(Cursor::new(out.into_inner())).expect("読めない");

        assert!(back.sheets[0].shapes.is_empty(), "グラフを図形として読んでしまった");
        assert!(back.sheets[0].images.is_empty(), "グラフを画像として読んでしまった");
        assert!(
            rep.unsupported.iter().any(|(w, n)| w.contains("グラフ") && *n == 1),
            "**グラフが黙って消えた** — 帳簿: {:?}",
            rep.unsupported
        );
    }

    #[test]
    fn an_inserted_shape_round_trips() {
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
    fn rotation_flip_line_width_opacity_and_shadow_round_trip() {
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
    use book::SheetShape;

    #[test]
    fn shapes_with_text_and_polylines_round_trip() {
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
            points: vec![book::PathPoint::at(0.0, 1.0), book::PathPoint::at(0.5, 0.0), book::PathPoint::at(1.0, 0.6)],
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
        assert!((sk.points[1].at.0 - 0.5).abs() < 0.01 && sk.points[1].at.1.abs() < 0.01);
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
    fn saving_a_real_template_leaves_the_style_table_alone() {
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
    fn a_touched_style_is_taken_as_an_append() {
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
    fn the_workbook_holds_no_functions_or_procedures() {
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
    fn macro_code_in_an_old_workbook_reads_is_reported_and_is_dropped_on_save() {
        // 黙って落とさない: 開くときに報告し、@export で取り出せる状態にはする
        let mut old = Book::new();
        old.scripts.push(("関数集計".into(), "def 集計(x):\n    return 1 < 2 and x".into()));
        // 古い形の xlsx を手で組む(いまの write はもう joPython を書かないため)
        let mut buf = Cursor::new(Vec::new());
        write(&old, &mut buf).expect("書けない");
        buf.set_position(0);
        let with_py = add_jopython_to_old(buf.into_inner(), &old.scripts);

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
    fn add_jopython_to_old(bytes: Vec<u8>, scripts: &[(String, String)]) -> Vec<u8> {
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
    fn workbook_properties_round_trip() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.props.creators = vec!["サンプル商事".into()];
        b.props.title = "見積 <2026>".into();
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert_eq!(back.props.creators, ["サンプル商事"], "作成者が往復しない");
        assert_eq!(back.props.title, "見積 <2026>", "逃がしが往復しない");
        assert_eq!(back.props.subject, "", "空欄は空欄のまま");
    }

    #[test]
    fn shape_offsets_round_trip() {
        let mut b = Book::new();
        b.sheets[0].shapes_new.push(book::SheetShape {
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
    fn theme_colors_round_trip_and_follow_a_palette_change() {
        let mut b = Book::new();
        b.theme = book::theme::OFFICE.iter().map(|s| s.to_string()).collect();
        let p = Pos::parse("A1").unwrap();
        let mut c = Cell::input("色");
        // アクセント1(4番)を明るくした色を、由来つきで持つ
        c.fmt.color_theme = Some((4, 400));
        c.fmt.color = Some(book::theme::resolve(&b.theme, 4, 0.4));
        b.sheets[0].set(p, c);
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let f = &back.sheets[0].get(p).unwrap().fmt;
        assert_eq!(f.color_theme, Some((4, 400)), "テーマ由来が往復しない");
        assert_eq!(f.color.as_deref(), Some(book::theme::resolve(&back.theme, 4, 0.4).as_str()), "色が解けない");
        // 配色を変えると、同じ由来から別の色が出る(追従の土台)
        let warm = book::theme::SCHEMES[1].1;
        let after = book::theme::resolve(
            &warm.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            4,
            0.4,
        );
        assert_ne!(after, f.color.clone().unwrap(), "配色を変えても色が変わらない");
    }

    #[test]
    fn table_objects_and_right_to_left_round_trip() {
        let mut b = Book::new();
        for (r, row) in [["部署", "金額"], ["営業", "100"]].iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                b.sheets[0].set(Pos::new(r as u32, c as u32), Cell::input(v));
            }
        }
        b.sheets[0].tables.push(book::TableDef {
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

    /// **表の様式の名前を決め打ちにしていた。** `<tableStyleInfo name>` を
    /// 読まず、書くときは必ず `TableStyleMedium2`。淡い緑の表を開いて
    /// 保存すると、**黙って青くなっていた**。
    ///
    /// 表そのものの名前(`Table1`)と様式の名前(`TableStyleLight9`)は
    /// 別物で、同じ `name` という綴りなのが罠(2026-08-10)。
    #[test]
    fn the_table_style_name_round_trips() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::new(0, 0), Cell::input("部署"));
        b.sheets[0].set(Pos::new(1, 0), Cell::input("営業"));
        b.sheets[0].tables.push(book::TableDef {
            name: "売上表".into(),
            style: Some("TableStyleLight9".into()),
            a: Pos::new(0, 0),
            b: Pos::new(1, 0),
            ..Default::default()
        });
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        let t = back.sheets[0].tables.first().expect("表が往復しない");
        assert_eq!(t.style.as_deref(), Some("TableStyleLight9"), "様式の名前が往復しない");
        assert_eq!(t.name, "売上表", "表の名前と様式の名前を取り違えている");

        // 様式の指定が無い表は、書くときに既定へ落ちる(Excel が新しい表に
        // 付けるもの)。**None のまま書いて属性を欠かすと Excel が開けない**
        let mut b2 = Book::new();
        b2.sheets[0].set(Pos::new(0, 0), Cell::input("あ"));
        b2.sheets[0].tables.push(book::TableDef {
            a: Pos::new(0, 0),
            b: Pos::new(0, 0),
            ..Default::default()
        });
        let mut buf2 = Cursor::new(Vec::new());
        write(&b2, &mut buf2).expect("書けない");
        buf2.set_position(0);
        let (back2, _) = read(buf2).expect("読めない");
        assert_eq!(
            back2.sheets[0].tables[0].style.as_deref(),
            Some("TableStyleMedium2"),
            "指定の無い表が既定に落ちない"
        );
    }

    #[test]
    fn freeze_panes_and_view_settings_round_trip() {
        use book::FreezePane;
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
    fn the_height_of_an_empty_row_can_be_read() {
        // **`<row r="71" ht="23.1" customHeight="1"/>`** — 高さだけ決めた空行。
        // 帳票では行間の調整に使う。Start の枝にしか置いていなかったので
        // 高さが落ちていた(日銀の資金循環で 115 箇所。2026-08-10)。
        //
        // **同じ形の穴は3度目。** sheetView は Empty の枝にしか無く、
        // docx の `<w:p/>` は Start の枝にしか無かった。**quick-xml で読む所は
        // Start と Empty の両方に置いたかを要素ごとに確かめること**
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
                    "<sheetData></sheetData>",
                    r#"<sheetData><row r="3" ht="23.1" customHeight="1"/><row r="5" ht="9" customHeight="1" hidden="1"/></sheetData>"#,
                );
                // **本当に置き換わったかを見る。** 名前で真にすると、
                // 差す先の綴りが変わったときに空振りしたまま緑になる
                // (2026-08-10 に踏んだ — `<sheetData/>` を探していたが
                //  書き出しは `<sheetData></sheetData>` だった)
                replaced = t.contains("<row r=\"3\"");
                s = t.into_bytes();
            }
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&s).unwrap();
        }
        assert!(replaced, "型紙を差す先が無い(書き出しの形が変わった)");
        let out = w.finish().unwrap();
        let (back, _) = read(Cursor::new(out.into_inner())).expect("読めない");
        let sh = &back.sheets[0];
        assert_eq!(sh.row_height.get(&2), Some(&23.1), "中身の無い行の高さが落ちている");
        assert_eq!(sh.row_height.get(&4), Some(&9.0), "隠した空行の高さも落ちている");
        assert!(sh.row_hidden.contains(&4), "中身の無い行の hidden が落ちている");
    }

    #[test]
    fn a_real_file_with_a_frozen_header_row_reads() {
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
            Some(book::FreezePane { frozen_rows: 1, frozen_columns: 0 }),
            "見出し行の固定が読めない"
        );
        assert!(sh.rtl, "子を持つ sheetView の rtl が読めない");
        assert_eq!(sh.show_gridlines, Some(false), "格子線が読めない");
        assert_eq!(sh.zoom_scale, Some(85), "表示倍率が読めない");
    }

    /// **中身も書式も無いセルも、シートの大きさには数える。**
    ///
    /// `<c r="D1" s="0"/>` は `cells` には入れない(`extent` は「中身のある
    /// 範囲」の意味で 38 箇所から使われている)。だが要素が置かれているのは
    /// **書き手がそこまで書いたということ**で、`<dimension>` の無いファイル
    /// では、それが唯一の手掛かりになる。
    ///
    /// 落とすと、呼ぶ側が正しく要求した範囲を「シートの外」と断ってしまう
    /// (2026-08-10、genoffice の試験が教えてくれた)。
    #[test]
    fn the_size_counts_up_to_the_last_present_cell_even_if_empty() {
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
                let t = String::from_utf8(s).unwrap();
                // **`<dimension>` ごと消す。** 申告が残っていると、そちらで
                // 大きさが埋まってしまい、何を試しているのか分からなくなる
                let t = match (t.find("<dimension"), t.find("<sheetData")) {
                    (Some(a), Some(_)) => {
                        let e = t[a..].find("/>").expect("dimension が閉じない") + a + 2;
                        format!("{}{}", &t[..a], &t[e..])
                    }
                    _ => t,
                };
                let t = t.replace(
                    "<sheetData></sheetData>",
                    r#"<sheetData><row r="1"><c r="A1" t="str"><v>あ</v></c><c r="D1" s="0"/></row></sheetData>"#,
                );
                assert!(t.contains(r#"<c r="D1""#), "型紙を差せていない");
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
        // 中身があるのは A1 だけ
        assert_eq!(sh.extent(), (1, 1), "extent の意味を変えてしまっている");
        // **見せる大きさは D 列まで。** 要素が置かれていた所まで数える
        assert_eq!(sh.size(), (1, 4), "空でも要素のあるセルを大きさに数えていない");
        assert!(sh.get(Pos::new(0, 3)).is_none(), "中身の無いセルを持ってしまっている");
    }

    #[test]
    fn a_dragged_split_does_not_become_a_freeze() {
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
    fn a_stored_view_setting_freeze_pane_is_ignored() {
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
    fn removing_a_table_removes_its_part_and_declaration() {
        // 表つきで書いたものを読み、表を外して書き直す(範囲に変換の道)
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.sheets[0].tables.push(book::TableDef {
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
    fn hidden_sheets_subscript_and_rotation_round_trip() {
        let mut b = Book::new();
        b.sheets.push(book::Sheet::new("裏"));
        b.sheets[1].hidden = true;
        let p = Pos::parse("A1").unwrap();
        let mut c = Cell::input("x");
        c.fmt.subscript = true;
        c.fmt.rotation = Some(255);
        c.fmt.align = book::HAlign::Justify;
        b.sheets[0].set(p, c);
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = read(buf).expect("読めない");
        assert!(back.sheets[1].hidden, "隠しシートが往復しない");
        let f = &back.sheets[0].get(p).unwrap().fmt;
        assert!(f.subscript, "下付きが往復しない");
        assert_eq!(f.rotation, Some(255), "回転が往復しない");
        assert_eq!(f.align, book::HAlign::Justify, "両端揃えが往復しない");
    }

    /// 横の揃えは6通りとも、開いて保存して開き直しても元のまま。
    ///
    /// **前は畳んでいた** — `centerContinuous` を `center` に、`distributed` を
    /// `justify` に寄せていたので、開くだけで見た目が変わっていた
    /// (日銀の統計表の題を genoffice の読み手と突き合わせて発覚)
    #[test]
    fn all_six_horizontal_alignments_round_trip() {
        use book::HAlign;
        let all = [
            ("A1", HAlign::Left),
            ("A2", HAlign::Center),
            ("A3", HAlign::Right),
            ("A4", HAlign::Justify),
            ("A5", HAlign::CenterContinuous),
            ("A6", HAlign::Distribute),
        ];
        let mut b = Book::new();
        for (a1, al) in all {
            let mut c = Cell::input("氏名");
            c.fmt.align = al;
            b.sheets[0].set(Pos::parse(a1).unwrap(), c);
        }
        let mut buf = Cursor::new(Vec::new());
        write(&b, &mut buf).expect("書けない");
        let bytes = buf.into_inner();

        // xlsx の綴りのまま書けているか。**往復だけ見ても足りない** —
        // 読み書き両方で同じ綴りに畳んでいれば往復は通ってしまう
        let mut z = zip::ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
        let mut s = String::new();
        use std::io::Read as _;
        z.by_name("xl/styles.xml").unwrap().read_to_string(&mut s).unwrap();
        assert!(
            s.contains("horizontal=\"centerContinuous\""),
            "styles.xml に centerContinuous が無い"
        );
        assert!(
            s.contains("horizontal=\"distributed\""),
            "styles.xml に distributed が無い"
        );

        let (back, _) = read(Cursor::new(bytes)).expect("読めない");
        for (a1, al) in all {
            let p = Pos::parse(a1).unwrap();
            assert_eq!(
                back.sheets[0].get(p).unwrap().fmt.align,
                al,
                "{a1} の揃えが往復しない"
            );
        }
    }

    #[test]
    fn sheet_protection_round_trips() {
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
    fn sheet_tab_color_round_trips() {
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
            "シート見出しの色が往復しない"
        );
    }

    #[test]
    fn grouping_and_collapsing_round_trip() {
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
    fn pivot_definitions_round_trip() {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("x"));
        b.pivots.push(book::PivotDef {
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
            chart_at: None,
            dest: Pos::parse("E1").unwrap(),
            show_as: String::new(),
            sort: String::new(),
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
    fn reread_with_attrs(extra: &str) -> Book {
        let mut b = Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("1"));
        b.sheets[0].names.push(book::DefinedName::new("名前つき", "A1:A5"));
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
    fn a_name_with_a_default_resolves_from_a_formula() {
        // LibreOffice は名前の定義すべてに真偽の属性を**既定値でも**書く。
        // 属性の数で「単純か」を決めていたので、中身は Excel と同じなのに
        // 全部「理解できない名前」へ落ち、式から引くと #NAME? だった
        let back = reread_with_attrs(r#"function="false" hidden="false" vbProcedure="false""#);
        assert_eq!(
            back.sheets[0].names,
            vec![book::DefinedName::new("名前つき", "A1:A5")],
            "偽の属性で名前が使えなくなった(names_raw: {:?})",
            back.names_raw
        );
        assert!(back.names_raw.is_empty(), "単純な名前が原文へ回った: {:?}", back.names_raw);
    }

    #[test]
    fn hidden_names_are_carried_over_verbatim() {
        // hidden="1" は**立っている**ので単純ではない。式からは引かせず、
        // 捨てもせず原文で持ち越す(今までどおり)
        let back = reread_with_attrs(r#"hidden="1""#);
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
    use book::{Pos, Value};
    use std::io::Write;

    /// 12 枚。`<sheet>` の並びと部品の番号を**わざと食い違わせる**。
    ///
    /// `<sheet name="表{i}" r:id="rId{i}"/>` を i=1..12 の順に並べ、
    /// rels では `rId{i}` → `sheet{13-i}.xml`(逆順)へ向ける。
    /// 各部品の A1 には**自分の部品番号**を書いてあるので、
    /// 取り違えれば値で分かる
    fn pattern() -> Vec<u8> {
        const N: usize = 12;
        let mut buf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let o: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            let put = |z: &mut zip::ZipWriter<_>, name: &str, s: &str| {
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

    fn a1(sh: &book::Sheet) -> String {
        match sh.get(Pos { row: 0, col: 0 }).map(|c| c.value.clone()) {
            Some(Value::Text(t)) => t,
            v => panic!("A1 が文字列でない: {v:?}"),
        }
    }

    #[test]
    fn reads_the_part_resolved_by_r_id() {
        let (book, _) = crate::xlsx::read(std::io::Cursor::new(pattern())).unwrap();
        assert_eq!(book.sheets.len(), 12, "シートの枚数");
        for (i, sh) in book.sheets.iter().enumerate() {
            // 並びは `<sheet>` の順のまま
            assert_eq!(sh.name, format!("表{}", i + 1), "{i} 枚目の名前");
            // 中身は rels の指す部品(逆順)
            assert_eq!(a1(sh), format!("部品{}", 12 - i), "{} の中身が別のシートの物", sh.name);
        }
    }

    #[test]
    fn the_string_sort_has_not_come_back() {
        // 文字列で並べると sheet10 が sheet2 より前に来る。
        // その狂い方(表2 に 部品10 系の中身)を名指しで撥ねる
        let (book, _) = crate::xlsx::read(std::io::Cursor::new(pattern())).unwrap();
        assert_eq!(a1(&book.sheets[1]), "部品11", "表2 が文字列の並べ替えの中身を掴んでいる");
    }

    #[test]
    fn a_round_trip_does_not_move_the_sheet_contents() {
        // 書き出しは部品を並び順に振り直すので、**ブックの rels の的も
        // 向け直さないと**、開き直したときに別のシートを指す
        let original = pattern();
        let (book, _) = crate::xlsx::read(std::io::Cursor::new(original.clone())).unwrap();
        let mut out = Vec::new();
        crate::xlsx::write_with(&book, Some(std::io::Cursor::new(&original)), std::io::Cursor::new(&mut out))
            .unwrap();
        let (back, _) = crate::xlsx::read(std::io::Cursor::new(&out)).unwrap();
        assert_eq!(back.sheets.len(), book.sheets.len(), "枚数が変わった");
        for (before, after) in book.sheets.iter().zip(&back.sheets) {
            assert_eq!(after.name, before.name, "名前の並びが変わった");
            assert_eq!(a1(after), a1(before), "{} の中身が別のシートへ移った", before.name);
        }
    }

    #[test]
    fn parts_and_declarations_stay_consistent_after_a_round_trip() {
        // 的の向け直しで、宣言(Content_Types)と rels と部品の三つが揃うこと。
        // ずれていると Excel が「修復」に入る
        let original = pattern();
        let (book, _) = crate::xlsx::read(std::io::Cursor::new(original.clone())).unwrap();
        let mut out = Vec::new();
        crate::xlsx::write_with(&book, Some(std::io::Cursor::new(&original)), std::io::Cursor::new(&mut out))
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
    fn without_an_r_id_sheets_sort_numerically() {
        // 控えの道。**文字列**で並べると sheet10 が sheet2 より前に来る
        let mut buf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let o: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            let put = |z: &mut zip::ZipWriter<_>, name: &str, s: &str| {
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

/// 計算の結果が xlsx を往復するか。**式そのものの試験は kumihan の側**に
/// あります(2026-08-26、エンジンを3つに分けたとき)。ここに置くのは
/// 「計算で出来た物が保存で残るか」だけです。
#[cfg(test)]
mod calc_round {
    use book::{Book, Cell, Pos, Value};
    use book::calc::recalc;

    fn book_with(cells: &[(&str, &str)]) -> Book {
        let mut b = Book::new();
        b.sheets[0].name = "Sheet1".into();
        for (at, text) in cells {
            let p = Pos::parse(at).unwrap();
            let c = if let Some(f) = text.strip_prefix('=') {
                Cell { formula: Some(f.to_string()), ..Default::default() }
            } else {
                Cell { value: Value::Text((*text).into()), ..Default::default() }
            };
            b.sheets[0].set(p, c);
        }
        b
    }

    fn roundtrip(b: &Book) -> Book {
        let mut buf = std::io::Cursor::new(Vec::new());
        crate::xlsx::write(b, &mut buf).unwrap();
        crate::xlsx::read(std::io::Cursor::new(buf.into_inner())).unwrap().0
    }

    #[test]
    fn the_spill_record_round_trips_through_xlsx() {
        let mut book = book_with(&[("A1", "=SEQUENCE(3,1)"), ("C1", "=SUM(A1:A3)")]);
        recalc(&mut book.sheets[0]);
        let mut back = roundtrip(&book);
        assert_eq!(back.sheets[0].spills.get(&Pos::parse("A1").unwrap()), Some(&(3, 1)),
            "スピルの記録が往復しない");
        // 開き直して再計算しても、自分の跡を先客と間違えない
        recalc(&mut back.sheets[0]);
        assert_eq!(back.sheets[0].value(Pos::parse("A1").unwrap()), Value::Number(1.0),
            "開き直しで偽の #SPILL! になった");
        assert_eq!(back.sheets[0].value(Pos::parse("A3").unwrap()), Value::Number(3.0));
    }

    #[test]
    fn furigana_round_trips_through_xlsx() {
        // rPh — 欧米の実装が落とす宝
        let mut book = book_with(&[("A1", "日本")]);
        book.sheets[0].phonetics.insert(Pos::parse("A1").unwrap(), "ニホン".into());
        let back = roundtrip(&book);
        assert_eq!(
            back.sheets[0].phonetics.get(&Pos::parse("A1").unwrap()),
            Some(&"ニホン".to_string()),
            "ふりがなが保存で落ちた"
        );
    }
}

/// **束ねた図形が xlsx を往復する。**
///
/// 2026-08-29 発注者「グループ化」。名前で覚える細工ではなく、本物の
/// `xdr:grpSp` で書きます。Excel でも束として開けます。
#[test]
fn grouped_shapes_survive_a_round_trip() {
    let mut b = book::Book::new();
    for (kind, fill, dx, g) in
        [("rect", "DDE7F0", 20.0f32, 1u32), ("ellipse", "C0504D", 160.0, 1), ("diamond", "9BBB59", 20.0, 0)]
    {
        b.sheets[0].shapes_new.push(book::SheetShape {
            at: book::Pos::new(2, 0),
            dx_px: dx,
            dy_px: 20.0,
            width_px: 110.0,
            height_px: 70.0,
            kind: kind.into(),
            fill: Some(fill.into()),
            line: Some("2E5A87".into()),
            line_w: 1.5,
            alpha: 1.0,
            group: g,
            ..Default::default()
        });
    }
    let mut buf = Vec::new();
    super::write(&b, std::io::Cursor::new(&mut buf)).expect("書けない");

    // 束は1つの入れ物、束ねていない図形は別の入れ物
    let x = {
        use std::io::Read;
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&buf)).expect("開けない");
        let na: Vec<String> =
            (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect();
        let d = na.iter().find(|n| n.contains("drawings/") && n.ends_with(".xml")).expect("描画");
        let mut t = String::new();
        z.by_name(d).unwrap().read_to_string(&mut t).unwrap();
        t
    };
    assert_eq!(x.matches("<xdr:grpSp>").count(), 1, "束が grpSp になっていない");
    assert_eq!(x.matches("<xdr:oneCellAnchor>").count(), 2, "入れ物の数が違う");

    let (b2, _) = super::read(std::io::Cursor::new(&buf)).expect("読めない");
    let sp = &b2.sheets[0].shapes;
    assert_eq!(sp.len(), 3, "図形の数が違う");
    assert_eq!(sp[0].group, 1, "束の番号が落ちた");
    assert_eq!(sp[1].group, 1, "束の番号が落ちた");
    assert_eq!(sp[2].group, 0, "束ねていない図形に番号が付いた");
    // 束の子の位置も戻る(束の中の位置 + 入れ物の位置)
    assert!((sp[0].dx_px - 20.0).abs() < 1.0, "1つ目の位置: {}", sp[0].dx_px);
    assert!((sp[1].dx_px - 160.0).abs() < 1.0, "2つ目の位置: {}", sp[1].dx_px);
    assert_eq!(sp[1].kind, "ellipse");
    assert_eq!(sp[1].fill.as_deref(), Some("C0504D"));
}

/// **ブックの構造の保護が往復する。**
///
/// 2026-08-30 発注者「ブックを保護」。鍵ではないので password は書きません
/// — 掛けた振りをしないためです(読み取り専用のお願いと同じ考え)。
#[test]
fn the_workbook_structure_lock_round_trips() {
    let mut b = book::Book::new();
    b.lock_structure = true;
    let mut buf = Vec::new();
    super::write(&b, std::io::Cursor::new(&mut buf)).expect("書けない");

    let x = {
        use std::io::Read;
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&buf)).expect("開けない");
        let mut t = String::new();
        z.by_name("xl/workbook.xml").unwrap().read_to_string(&mut t).unwrap();
        t
    };
    assert!(x.contains(r#"lockStructure="1""#), "構造の保護が書かれていない");
    assert!(!x.contains("password"), "鍵を書いている(掛けた振りをしない)");

    let (b2, _) = super::read(std::io::Cursor::new(&buf)).expect("読めない");
    assert!(b2.lock_structure, "読み戻せていない");

    // 外したら消える
    let mut b3 = b2;
    b3.lock_structure = false;
    let mut buf2 = Vec::new();
    super::write(&b3, std::io::Cursor::new(&mut buf2)).expect("書けない");
    let (b4, _) = super::read(std::io::Cursor::new(&buf2)).expect("読めない");
    assert!(!b4.lock_structure, "外したのに残っている");
}

/// **範囲ごとの保護が往復する。**
///
/// 2026-08-30 発注者「範囲を保護」。鍵は持ちません — password を書かず、
/// 掛けた振りをしないためです(ブックの保護と同じ考え)。
#[test]
fn protected_ranges_round_trip() {
    let mut b = book::Book::new();
    b.sheets[0].protected = true;
    b.sheets[0].protect_ranges = vec![
        ("入力欄".into(), "B2:D10".into()),
        ("合計".into(), "F2:F10".into()),
    ];
    let mut buf = Vec::new();
    super::write(&b, std::io::Cursor::new(&mut buf)).expect("書けない");

    let x = {
        use std::io::Read;
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&buf)).expect("開けない");
        let mut t = String::new();
        z.by_name("xl/worksheets/sheet1.xml").unwrap().read_to_string(&mut t).unwrap();
        t
    };
    assert!(x.contains("<protectedRanges>"), "範囲の保護が書かれていない");
    assert!(x.contains(r#"sqref="B2:D10""#), "範囲が書かれていない");
    assert!(!x.contains("password"), "鍵を書いている(掛けた振りをしない)");
    // **作法どおり sheetProtection の後**(順が違うと Excel が読まない)
    let (i, j) = (x.find("sheetProtection").unwrap(), x.find("protectedRanges").unwrap());
    assert!(i < j, "並びが作法どおりでない");

    let (b2, _) = super::read(std::io::Cursor::new(&buf)).expect("読めない");
    assert_eq!(b2.sheets[0].protect_ranges.len(), 2, "読み戻せていない");
    assert_eq!(b2.sheets[0].protect_ranges[0], ("入力欄".into(), "B2:D10".into()));
    assert_eq!(b2.sheets[0].protect_ranges[1].1, "F2:F10");
}
