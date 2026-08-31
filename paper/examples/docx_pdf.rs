//! docx を1つ読んで PDF に写します。元の PDF と見比べるための道具です。
//!
//! ```text
//! cargo run -q -p paper --example docx_pdf -- 入力.docx 出力.pdf
//! ```

fn main() -> Result<(), String> {
    let mut a = std::env::args().skip(1);
    let (moto, saki) = match (a.next(), a.next()) {
        (Some(m), Some(s)) => (m, s),
        _ => return Err("使い方: docx_pdf <入力.docx> <出力.pdf>".into()),
    };
    let f = std::fs::File::open(&moto).map_err(|e| e.to_string())?;
    let (doc, _) = ooxml::read(std::io::BufReader::new(f))?;
    let g = std::fs::File::create(&saki).map_err(|e| e.to_string())?;
    paper::doc_to_pdf(&doc, None, std::io::BufWriter::new(g))?;
    println!("{saki} を書きました");
    Ok(())
}
