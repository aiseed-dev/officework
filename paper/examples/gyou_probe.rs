//! **docx を組んだ行の位置を出す確認用のスクリプト**(2026-09-09)。Word の PDF と
//! 行の y を並べて、どこで折れ方や送りが変わるかを見るための物です。
//!
//! ```text
//! cargo run --release -p paper --example gyou_probe -- 元.docx 探す字 [前後の行数]
//! ```
//!
//! 「探す字」を含む最初の行の前後を、y(mm)・x(mm)・セル番号(表番号, 行, 列。
//! 中の表は `n` 付き)・字で出します。改ページの位置(`breaks`)も先頭に出します
fn main() -> Result<(), String> {
    let mut a = std::env::args().skip(1);
    let moto = a.next().ok_or("使い方: gyou_probe <docx> <探す字> [前後の行数]")?;
    let key = a.next().unwrap_or_default();
    let n: usize = a.next().and_then(|v| v.parse().ok()).unwrap_or(60);
    let f = std::fs::File::open(&moto).map_err(|e| e.to_string())?;
    let (doc, _) = ooxml::read(std::io::BufReader::new(f))?;
    // FONT=name: print how the engine resolves a font name on this machine
    if let Ok(name) = std::env::var("FONT") {
        match kumihan::font::resolve(&name) {
            Some(f) => println!("font {name:?} -> {:?} index {} ({}) okuri={:?} agari={:?}", f.path, f.index, f.name, kumihan::font::okuri_em(Some(&name)), kumihan::font::agari_em(Some(&name))),
            None => println!("font {name:?} -> none; substitute {:?}",
                kumihan::font::substitute(&name).map(|f| (f.path.clone(), f.index))),
        }
    }
    // RUNS=1: print the paragraphs of every table with their style, the
    // size and font the style resolves to, every run, and the images
    if std::env::var("RUNS").is_ok() {
        for (ti, b) in doc.blocks.iter().enumerate() {
            if let kumihan::Block::Table(t) = b {
                for (ri, row) in t.rows.iter().enumerate() {
                    for (ci, c) in row.iter().enumerate() {
                        for p in &c.paragraphs {
                            let sid = p.style_id.as_deref();
                            println!("table {ti} ({ri},{ci}) style={sid:?} pt={:?} font={:?} latin={:?} images={}",
                                doc.style_pt(sid), doc.style_font(sid), doc.style_font_latin(sid), p.images.len());
                            for r in &p.runs {
                                println!("    run pt={:?} font={:?} bold={} {:?}", r.size_pt, r.font, r.fmt.bold, r.text.chars().take(30).collect::<String>());
                            }
                        }
                    }
                }
            }
        }
    }
    // TABLE=1: print every table's grid and cells as read from the docx
    if std::env::var("TABLE").is_ok() {
        for (ti, b) in doc.blocks.iter().enumerate() {
            if let kumihan::Block::Table(t) = b {
                let mm: Vec<String> = t.col_mm.iter().map(|w| format!("{w:.1}")).collect();
                println!("table {ti}: cols {} = [{}] fixed={}", t.col_mm.len(), mm.join(" "), t.fixed_layout);
                for (ri, r) in t.rows.iter().enumerate() {
                    let cells: Vec<String> = r.iter().map(|c| {
                        let txt: String = c.paragraphs.iter().flat_map(|p| p.runs.iter())
                            .map(|run| run.text.as_str()).collect::<String>().chars().take(8).collect();
                        format!("{}{}:{txt:?}", c.span(), match c.v_merge {
                            kumihan::VMerge::None => "", kumihan::VMerge::Start => "S", kumihan::VMerge::Continue => "C" })
                    }).collect();
                    println!("  row {ri:2} spans={:2} | {}", r.iter().map(|c| c.span()).sum::<usize>(), cells.join(" "));
                }
            }
        }
    }
    let (sheet, page, _) = paper::doc_to_sheet(&doc, None)?;
    println!(
        "用紙 {}x{}mm 上 {} 下 {} / 行 {} / 改ページ {:?}",
        page.w_mm, page.h_mm, page.top_mm, page.bottom_mm, sheet.lines.len(), sheet.breaks
    );
    for (at, pg) in &sheet.sect_pages {
        println!("節 y={at:.1} 用紙 {}x{} 上 {} 下 {}", pg.w_mm, pg.h_mm, pg.top_mm, pg.bottom_mm);
    }
    let pn = paper::paginate_full(&sheet, paper::Paper::from_page(&page));
    // ANCHORS=1: floating drawings as read, as placed, and as shapes
    if std::env::var("ANCHORS").is_ok() {
        for (ti, b) in doc.blocks.iter().enumerate() {
            let paras: Vec<&kumihan::Paragraph> = match b {
                kumihan::Block::Para(p) => vec![p],
                kumihan::Block::Table(t) => t.all_paragraphs(),
            };
            for p in paras {
                for a in &p.anchors {
                    let fs = ooxml::foreign_shapes_in(a, &doc.theme_colors);
                    println!("block {ti} anchor len={} inline={} anchor={} wgp={} -> {} shapes", a.len(),
                        a.contains("<wp:inline"), a.contains("<wp:anchor"), a.contains("<wpg:wgp>"), fs.len());
                    for f in fs {
                        println!("    {} fill={:?} line={:?} w={:.1} h={:.1} dx={:.1} dy={:.1} from={}/{} off={:.1}/{:.1}",
                            f.look.kind, f.look.fill, f.look.line, f.w_mm, f.h_mm, f.dx_mm, f.dy_mm, f.h_from, f.v_from, f.x_mm, f.y_mm);
                    }
                }
            }
        }
        for cb in sheet.cell_boxes.iter().filter(|c| c.table == 0) {
            println!("cell ({},{}) x={:.1} top={:.1} w={:.1} h={:.1}", cb.row, cb.col, cb.x_mm, cb.top_mm, cb.w_mm, cb.h_mm);
        }
        println!("sheet.anchors_at = {}", sheet.anchors_at.len());
        for (a, x, y) in &sheet.anchors_at { println!("    at x={x:.1} y={y:.1} len={}", a.len()); }
        println!("sheet.inline_shapes = {}", sheet.inline_shapes.len());
        let shapes = paper::foreign_shapes(&doc, &sheet, page);
        println!("doc shapes = {}", shapes.len());
        for s in &shapes { println!("    p{} {} x={:.1} y={:.1} w={:.1} h={:.1} fill={:?}", s.page, s.look.kind, s.x_mm, s.y_mm, s.w_mm, s.h_mm, s.look.fill); }
    }
    let at = sheet.lines.iter().position(|l| l.text().contains(&key)).unwrap_or(0);
    for (i, l) in sheet.lines.iter().enumerate() {
        if i + 5 < at || i > at + n {
            continue;
        }
        let t: String = l.text().chars().take(16).collect();
        let x = l.cells.first().map(|c| c.x_mm).unwrap_or(-1.0);
        let cell = l.cell.map(|(t, r, c)| {
            let t = if t > usize::MAX / 2 { format!("n{}", t - usize::MAX / 2) } else { t.to_string() };
            format!("({t},{r},{c})")
        });
        let pg = pn.pages.get(i).copied().unwrap_or(0);
        println!("{i:5} p{pg:<3} y={:8.2} x={:6.1} {:>12} {:?} pt={:.1} font={:?}", l.y_mm, x, cell.unwrap_or_default(), t, l.cells.first().map_or(0.0, |c| c.size_pt), l.cells.first().and_then(|c| c.font.as_deref()).unwrap_or(""));
        // 探す字の行だけ、字ごとの送り(mm)と大きさ(pt)も出す
        if i == at && std::env::var("HABA").is_ok() {
            let v: Vec<String> = l.cells.iter().take(12).map(|c| format!("{}:{:.2}/{:.1}", c.ch, c.w_mm, c.size_pt)).collect();
            println!("      送り {}", v.join(" "));
        }
    }
    Ok(())
}
