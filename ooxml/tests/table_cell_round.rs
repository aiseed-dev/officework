//! **実物の docx で、表のセルの往復を測る。**
//!
//! 日本の様式は「1つのセルに段落がいくつも入る」形が多く、実物 17 冊の
//! 395 セルのうち **63 セル**がそれでした(2026-08-19)。素のセル(`|`)は中身を
//! 1段落として組むので、そのまま書くと段落の切れ目が消えます。
//!
//! `a|`(AsciiDoc として組むセル)と `{empty}`(空の段落)で往復させます。
//! ここは**字が消えていないこと**を見張ります — 段落の数の細かな違い
//! (空白だけの段落が空の段落になる等)は許します。

/// セルの字を並べる(表の中だけ)
fn cell_text(d: &kumihan::Document) -> Vec<String> {
    d.blocks
        .iter()
        .filter_map(|b| if let kumihan::Block::Table(t) = b { Some(t) } else { None })
        .flat_map(|t| t.rows.iter().flatten())
        .map(|c| kumihan::paras_text(&c.paragraphs))
        .collect()
}

/// 空の段落を落とした「字の芯」。段落の数の違いを無視して中身だけ比べる
fn core(s: &str) -> String {
    s.split('\n').filter(|x| !x.trim().is_empty()).collect::<Vec<_>>().join("\n")
}

#[test]
fn real_table_cells_round_trip_through_adoc() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let dirs = [
        root.join("sample"),
        root.join("sample/writer"),
        // 実物の様式(手元にある機械だけ。無ければ飛ばす)
        std::path::PathBuf::from("/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki"),
    ];
    let (mut volume, mut grid_cell, mut as_is, mut blank_only) = (0, 0, 0, 0);
    let mut content_differs = Vec::new();

    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut ps: Vec<_> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "docx"))
            .collect();
        ps.sort();
        for p in ps {
            let Ok(bytes) = std::fs::read(&p) else { continue };
            let Ok((doc, _)) = ooxml::read(std::io::Cursor::new(bytes)) else { continue };
            let src = kumihan::adoc::write(&doc);
            let back = kumihan::adoc::parse(&src).expect("書いた adoc が読めない");
            volume += 1;
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            for (a, b) in cell_text(&doc).iter().zip(cell_text(&back).iter()) {
                // 段落が2つ以上のセルだけを見る(1段落のセルは前から通っている)
                if !a.contains('\n') {
                    continue;
                }
                grid_cell += 1;
                if a == b {
                    as_is += 1;
                } else if core(a) == core(b) {
                    blank_only += 1;
                } else {
                    content_differs.push(format!("{name}: {a:?} → {b:?}"));
                }
            }
        }
    }

    println!("docx {volume} 冊 / 段落が複数のセル {grid_cell}: そのまま {as_is} / 空の段落だけ違う {blank_only}");
    assert!(volume > 0, "docx を1冊も読めていない");
    // **字が消えたら落とす。** 段落の数の細かな違いは許す
    assert!(content_differs.is_empty(), "セルの字が往復で変わった:\n  {}", content_differs.join("\n  "));
}
