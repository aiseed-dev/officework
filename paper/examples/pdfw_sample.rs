//! 新しい書き手で1枚出す(いまの道と大きさを見比べる用)。
fn main() {
    let f = kumihan::font::default_family("ja").expect("書体");
    let bytes = kumihan::font::load(f).expect("読めない");
    let pages = vec![vec![
        paper::pdfw::Piece { x_mm: 20.0, y_mm: 270.0, size_pt: 18.0, text: "四月の売上".into(), ..Default::default() },
        paper::pdfw::Piece { x_mm: 20.0, y_mm: 255.0, size_pt: 10.5,
            text: "本文です。日本語の行組みもエンジンが折ります。".into(), ..Default::default() },
        paper::pdfw::Piece { x_mm: 20.0, y_mm: 245.0, size_pt: 10.5,
            text: "ボールペン    1,200 円".into(), ..Default::default() },
    ]];
    let out = paper::pdfw::write(&pages, 210.0, 297.0, &bytes).expect("PDF が出ない");
    std::fs::write("test/out/pdfw_sample.pdf", &out).expect("置けない");
    println!("書体 {:>12} バイト", bytes.len());
    println!("PDF  {:>12} バイト", out.len());
}
