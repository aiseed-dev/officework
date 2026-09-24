//! Makes the MHLW resume form (`履歴書-厚労省.form.adoc` and its
//! `.tmpl.adoc`) from the Ministry's own xlsx of the "履歴書様式例"
//! (docs/sekkei/drawlist.ja.adoc).
//!
//!     cargo run -p sheet --example rireki_form -- kouroushourirekishoA4.xlsx sample/rirekisho
//!
//! The Ministry's layout is kept. What changes:
//!
//! - the fonts become BIZ UD (MS Mincho → BIZ UD明朝, MS PMincho → BIZ
//!   UDP明朝, MS PGothic → BIZ UDPゴシック, the title's HG font → BIZ UD明朝)
//! - the row heights are scaled so that rows 1 to 52 fit an A4 page at 100%,
//!   in steps of 0.75pt (a Windows pixel), so Excel prints one page a side
//! - the cells that take data hold marks such as `{氏名}` (book::form)
//!
//! It also makes the JIS form (`履歴書-JIS.form.adoc`) from the same
//! layout ([`jis`]): the fields JIS Z 8303's example had and the Ministry's
//! dropped are put back, and 性別 becomes a choice to circle.

use book::{Cell, Pos, Value};

fn at(a1: &str) -> Pos {
    Pos::parse(a1).expect("a cell reference")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let src = args.next().expect("the MHLW xlsx");
    let out = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    let f = std::fs::File::open(&src).expect("open the xlsx");
    let (mut book, _) = sheet::xlsx::read(std::io::BufReader::new(f)).expect("read the xlsx");
    book.sheets.truncate(1);
    book.props.title = "履歴書".into();
    let s = &mut book.sheets[0];
    s.name = "履歴書".into();

    // Fonts: BIZ UD in place of the MS and HG fonts
    let biz = |name: &str| -> String {
        match name {
            n if n.contains("Ｐ明朝") || n.contains("P明朝") => "BIZ UDP明朝",
            n if n.contains("明朝") || n.starts_with("HG") => "BIZ UD明朝",
            n if n.contains("Ｐゴシック") || n.contains("Pゴシック") => "BIZ UDPゴシック",
            _ => "BIZ UDゴシック",
        }
        .to_string()
    };
    for c in s.cells.values_mut() {
        c.fmt.font = Some(biz(c.fmt.font.as_deref().unwrap_or("ＭＳ Ｐ明朝")));
    }
    for sp in s.shapes.iter_mut() {
        sp.text_fmt.font = Some(biz(sp.text_fmt.font.as_deref().unwrap_or("ＭＳ Ｐ明朝")));
        // The photo box is the field the data's picture goes into
        if sp.text.as_deref().is_some_and(|t| t.contains("写真をはる位置")) {
            sp.field = Some("写真".into());
        }
    }
    // The named styles (標準 and the like) name their faces too
    for (_, _, f) in book.named_styles.iter_mut() {
        if let Some(n) = f.font.as_deref() {
            f.font = Some(biz(n));
        }
    }
    book.default_font = Some(("BIZ UDP明朝".into(), 11.0));

    // Row heights: rows 1..=52 fit the printable height of A4 at 100%
    let s = &mut book.sheets[0];
    let dflt = s.default_row_height.unwrap_or(13.5);
    let h: Vec<f32> = (0..52).map(|r| *s.row_height.get(&r).unwrap_or(&dflt)).collect();
    let (_, _, top, bottom) = s.margins_mm.unwrap_or((15.0, 10.0, 19.0, 10.0));
    let usable_pt = (297.0 - top - bottom) * 72.0 / 25.4;
    let total: f32 = h.iter().sum();
    // a little room below the last row, so a printer's own margin does not
    // push it to a second page
    let k = (usable_pt * 0.97 / total).min(1.0);
    s.row_height.clear();
    s.row_height_auto.clear();
    for (r, v) in h.iter().enumerate() {
        s.row_height.insert(r as u32, ((v * k) / 0.75).round() * 0.75);
    }
    s.print_scale = Some(100);
    eprintln!("rows: {total:.1}pt -> {:.1}pt (x{k:.3}), printable {usable_pt:.1}pt",
        s.row_height.values().sum::<f32>());

    // Marks
    let mut mark = |a1: &str, text: &str| {
        let p = at(a1);
        let fmt = s.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
        s.set(p, Cell { formula: None, value: Value::Text(text.into()), fmt });
    };
    mark("E3", "{日付.年}年　{日付.月}月　{日付.日}日現在");
    mark("C5", "{ふりがな}");
    mark("B7", "{氏名}");
    mark("B10", "　{生年月日.年}年　{生年月日.月}月　{生年月日.日}日生　（満{年齢}歳）");
    mark("I10", "{性別}");
    mark("C12", "{現住所ふりがな}");
    mark("C13", "{郵便番号}");
    mark("B15", "{現住所}");
    mark("J13", "{電話}");
    mark("C17", "{連絡先ふりがな}");
    mark("C19", "{連絡先郵便番号}");
    mark("B20", "{連絡先}");
    mark("J19", "{連絡先電話}");
    // 学歴・職歴: 15 rows on the left page, 7 more on the right
    let left = [26, 28, 30, 32, 34, 35, 37, 39, 40, 42, 44, 46, 47, 48, 49];
    let right = [4, 6, 8, 9, 10, 12, 14];
    for (i, r) in left.iter().chain(right.iter()).enumerate() {
        let (y, m, t) = if i < left.len() { ("B", "C", "D") } else { ("M", "N", "O") };
        let n = i + 1;
        mark(&format!("{y}{r}"), &format!("{{学歴・職歴.{n}.年}}"));
        mark(&format!("{m}{r}"), &format!("{{学歴・職歴.{n}.月}}"));
        mark(&format!("{t}{r}"), &format!("{{学歴・職歴.{n}.内容}}"));
    }
    for (i, r) in [18, 21, 22, 25, 27, 29].iter().enumerate() {
        let n = i + 1;
        mark(&format!("M{r}"), &format!("{{免許・資格.{n}.年}}"));
        mark(&format!("N{r}"), &format!("{{免許・資格.{n}.月}}"));
        mark(&format!("O{r}"), &format!("{{免許・資格.{n}.内容}}"));
    }
    mark("M33", "{志望の動機など}");
    for (i, r) in [46, 47, 48, 49].iter().enumerate() {
        mark(&format!("M{r}"), &format!("{{本人希望.{}}}", i + 1));
    }
    // A long answer wraps inside the box and starts at its top
    if let Some(c) = s.cells.get_mut(&at("M33")) {
        c.fmt.wrap = true;
        c.fmt.valign = book::VAlign::Top;
    }

    save(&book, "履歴書-厚労省", "厚生労働省「履歴書様式例」(2021-04-16)", &out);
    jis(&mut book);
    save(&book, "履歴書-JIS",
         "厚生労働省「履歴書様式例」(2021-04-16)に、JIS Z 8303 の様式例の欄を足しました", &out);
}

/// The two files of a form: the cells, merges and marks, and its look.
fn save(book: &book::Book, name: &str, source: &str, out: &std::path::Path) {
    let theme = kumihan::booktmpl::from_book(book);
    let mut form = kumihan::book_adoc::write(book);
    // The form names its look (SEKKEI: a template is named by :template:)
    if let Some(i) = form.find('\n') {
        form.insert_str(i + 1, &format!(":template: {name}\n:出典: {source}\n"));
    }
    std::fs::create_dir_all(out).unwrap();
    std::fs::write(out.join(format!("{name}.form.adoc")), form).unwrap();
    std::fs::write(out.join(format!("{name}.tmpl.adoc")), kumihan::booktmpl::write(&theme)).unwrap();
    for (file, data) in kumihan::book_meta::image_files(book) {
        std::fs::write(out.join(file), data).unwrap();
    }
    eprintln!("{name}: fields {}", book::form::fields(book).len());
    for r in kumihan::book_adoc::write_report(book) {
        eprintln!("left out: {r}");
    }
}

/// **The JIS layout from the Ministry's.** JIS Z 8303's example (withdrawn
/// in 2020) had fields the Ministry's example of 2021 dropped; they go back
/// in the same frame. The texts follow the JIS example as the Ministry
/// shows it for comparison ("新たな履歴書の様式例の作成について", page 3,
/// "（参考）従来の履歴書様式例（ＪＩＳ規格の履歴書様式例）"):
///
/// - 性別 is circled from ※男・女 (`{性別:男・女}`)
/// - the photo box gives the sizes, 縦 36～40mm and 横 24～30mm
/// - the box of 志望の動機 gives its right side to 通勤時間, 扶養家族数
///   (配偶者を除く), 配偶者 and 配偶者の扶養義務, the last two circled
///   from ※有・無
/// - the note under the first page is JIS's 記入上の注意
fn jis(book: &mut book::Book) {
    use book::{HAlign, VAlign};
    let s = &mut book.sheets[0];
    // The Ministry's note says 性別 may be left out; JIS has its own note
    if let Some(c) = s.cells.get_mut(&at("B51")) {
        c.value = Value::Empty;
    }
    for sp in s.shapes.iter_mut().filter(|sp| sp.field.as_deref() == Some("写真")) {
        if let Some(t) = &sp.text {
            sp.text = Some(t.replace("縦　　", "縦　36～40mm").replace("横　　", "横　24～30mm"));
        }
    }
    let base = s.get(at("M33")).map(|c| c.fmt.clone()).unwrap_or_default();
    let edge = base.borders.top;
    // One box: merged, its text in the top left cell, a line all round
    let mut boxed = |a: &str, z: &str, text: &str, pt: u32, align: HAlign, valign: VAlign, wrap: bool,
                     lined: bool| {
        let (a, z) = (at(a), at(z));
        s.merges.retain(|(p, q)| p.row > z.row || q.row < a.row || p.col > z.col || q.col < a.col);
        if a != z {
            s.merges.push((a, z));
        }
        for r in a.row..=z.row {
            for c in a.col..=z.col {
                let p = Pos::new(r, c);
                let mut f = base.clone();
                f.borders = Default::default();
                f.size_c = Some(pt * 100);
                f.align = align;
                f.valign = valign;
                f.wrap = wrap && p == a;
                if lined {
                    if r == a.row { f.borders.top = edge; }
                    if r == z.row { f.borders.bottom = edge; }
                    if c == a.col { f.borders.left = edge; }
                    if c == z.col { f.borders.right = edge; }
                }
                let value = if p == a { Value::Text(text.into()) } else { Value::Empty };
                s.set(p, Cell { formula: None, value, fmt: f });
            }
        }
    };
    use HAlign::{Center, Left};
    use VAlign::{Middle, Top};
    // First page: 性別 to circle, and JIS's note in place of the Ministry's
    boxed("H10", "J11", "※{性別:男・女}", 12, Center, Middle, false, true);
    // Two lines as JIS sets them: １ and ２, then ３ under １
    boxed("B52", "J52",
          "記入上の注意　１．鉛筆以外の黒又は青の筆記具で記入。　　\
           ２．数字はアラビア数字で、文字はくずさず正確に書く。\n\
           　　　　　　　３．※印のところは、該当するものを○で囲む。",
          8, Left, Middle, false, false);
    // Second page: 志望の動機 on the left, the four JIS fields on the right
    boxed("M32", "O32", "　志望の動機、特技、好きな学科、アピールポイントなど", 10, Left, Middle, false, true);
    boxed("M33", "O42", "{志望の動機など}", 14, Left, Top, true, true);
    boxed("P32", "Q32", "　通勤時間", 10, Left, Middle, false, true);
    boxed("P33", "Q34", "約　{通勤時間.時間}　時間　{通勤時間.分}　分", 12, Center, Middle, false, true);
    boxed("P35", "Q36", "　扶養家族数（配偶者を除く）", 9, Left, Middle, false, true);
    boxed("P37", "Q38", "{扶養家族数}　人", 12, HAlign::Right, Middle, false, true);
    boxed("P39", "P39", "配偶者", 9, Center, Middle, false, true);
    boxed("Q39", "Q39", "配偶者の扶養義務", 9, Center, Middle, false, true);
    boxed("P40", "P42", "※{配偶者:有・無}", 10, Center, Middle, false, true);
    boxed("Q40", "Q42", "※{配偶者の扶養義務:有・無}", 11, Center, Middle, false, true);
    // The note takes two lines
    s.row_height.insert(51, 24.0);
}
