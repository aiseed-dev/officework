//! 画像入りの紙を新しい書き手で出す(実物で見る用)。
fn main() {
    // 小さな PNG を作る(赤と青の格子)
    let mut img = image::RgbImage::new(64, 48);
    for (x, y, p) in image::GenericImageView::pixels(&img.clone()) {
        let _ = p;
        let c = if (x / 16 + y / 16) % 2 == 0 { [220u8, 40, 40] } else { [40, 80, 220] };
        img.put_pixel(x, y, image::Rgb(c));
    }
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut png, image::ImageOutputFormat::Png)
        .expect("PNG");
    let data = std::sync::Arc::new(png.into_inner());

    let f = kumihan::font::default_family("ja").expect("書体");
    let bytes = kumihan::font::load(f).expect("読めない");
    let leaf = paper::pdfw::Leaf {
        pieces: vec![paper::pdfw::Piece {
            x_mm: 20.0, y_mm: 270.0, size_pt: 14.0, w_mm: 40.0,
            text: "絵の入った紙".into(), ..Default::default()
        }],
        rules: vec![],
        images: vec![paper::pdfw::Image {
            x_mm: 20.0, y_mm: 200.0, w_mm: 64.0, h_mm: 48.0, data,
        }],
    };
    let mut out = Vec::new();
    paper::pdfw::write_pages(&[leaf], 210.0, 297.0, &bytes, &mut out).expect("PDF");
    std::fs::write("test/out/pdf_image.pdf", &out).unwrap();
    println!("PDF {} バイト", out.len());
}
