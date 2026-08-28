//! 組版の試験。

use super::doc::*;
use super::layout::*;
use crate::font;

#[cfg(test)]
/// **試験で使う既定の書体。**
///
/// 既定の書体は画面の言語で決まります。言語を替える試験(テンプレートの
/// 各国語版)と並んで走ると、別の書体が返って字幅の試験が落ちます
/// (2026-08-28。25 回に1回ほど落ちるのを捕まえました)。
///
/// ここで**言語を日本語に据えて**から取ります。錠は返る前に放すので、
/// 言語を替える試験と噛み合いません。
#[cfg(test)]
pub(crate) fn test_font() -> Vec<u8> {
    // **1度だけ選んで、1度だけ読みます。**
    //
    // 呼ぶ所が 31 あるので、そのたびに読み直すと数十 MB × 31 になります。
    // 錠の中で読むようにしたら、試験が 2.4 秒から 3分20秒 になりました
    // (2026-08-28。測って気づきました)。
    //
    // 選ぶときだけ言語の錠を取ります。既定の書体は画面の言語で決まるので、
    // 言語を替える試験(テンプレートの各国語版)と並ぶと別の書体が返り、
    // 字幅の試験が落ちます(25 回に1回ほど)。
    static FONT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    FONT.get_or_init(|| {
        let f = {
            let _lang = crate::font::lang_lock();
            crate::font::set_default_language("ja");
            crate::font::for_document(None).expect("日本語フォントが要る").0
        };
        crate::font::load(f).expect("読めない")
    })
    .clone()
}

mod kihon {
    use super::*;

    fn font() -> Vec<u8> {
        // **同梱しない。** システムのフォントを使う
        super::test_font()
    }

    fn sheet_of(text: &str, measure: f32) -> Sheet {
        let data = font();
        let m = Metrics::new(&data).unwrap();
        let doc = Document::plain(text);
        layout(&doc, &m, &Frame { measure_mm: measure, line_height_mm: 6.0, y0_mm: 20.0 })
    }

    const SAMPLE: &str = "日本の事務の実態は、文書ではなく様式です。その様式の定義をテキストにして、記入用の帳票・検証・データベースを全部そこから派生させます。「原本はテキスト。」と、私たちは Rust で書きます。";

    #[test]
    fn the_extent_of_a_form_field_can_be_read() {
        // 「氏名: 」(8バイト)+ 欄「山田　太郎」(15バイト)= 8..23
        let mut d = Document::plain("氏名: 山田　太郎\n次の行");
        d.apply_char_format(8..23, |f| {
            f.sdt = Some(Box::new(Sdt { tag: "氏名".into(), ..Default::default() }))
        });
        // 太字で run を割っても、欄は一つに繋がって返る
        d.apply_char_format(8..14, |f| f.bold = true);
        assert_eq!(d.sdt_range_at(12), Some(8..23), "割れた run が繋がらない");
        assert_eq!(d.sdt_range_at(23), Some(8..23), "欄の直後(直前の字の慣習)");
        assert_eq!(d.sdt_range_at(3), None, "欄の外");
        assert_eq!(d.sdt_range_at(26), None, "次の段落");
    }

    #[test]
    fn no_punctuation_or_closing_bracket_starts_a_line() {
        for measure in [30.0, 40.0, 55.0, 70.0, 90.0] {
            let s = sheet_of(SAMPLE, measure);
            for l in &s.lines {
                let c = l.cells[0].ch;
                assert!(!is_gyoto_kinsoku(c),
                    "行長{measure}mm で行頭が「{c}」: {}", l.text());
            }
        }
    }

    #[test]
    fn no_opening_bracket_is_left_at_the_line_end() {
        for measure in [30.0, 40.0, 55.0, 70.0, 90.0] {
            let s = sheet_of(SAMPLE, measure);
            for l in &s.lines {
                let c = l.cells.last().unwrap().ch;
                assert!(!is_gyomatsu_kinsoku(c),
                    "行長{measure}mm で行末が「{c}」: {}", l.text());
            }
        }
    }

    #[test]
    fn latin_words_do_not_break_mid_line() {
        for measure in [30.0, 40.0, 55.0, 70.0] {
            let s = sheet_of(SAMPLE, measure);
            let joined: Vec<String> = s.lines.iter().map(|l| l.text()).collect();
            // "Rust" がどこかの行に丸ごとある(行またぎで割れていない)
            assert!(joined.iter().any(|t| t.contains("Rust")),
                "行長{measure}mm で Rust が割れた: {joined:?}");
        }
    }

    #[test]
    fn does_not_overshoot_the_line_length() {
        // 追い出しで短くなるのは良い。超えるのは駄目(はみ出し)
        for measure in [40.0, 55.0, 70.0] {
            let s = sheet_of(SAMPLE, measure);
            for l in &s.lines {
                assert!(l.width_mm() <= measure + 0.1,
                    "行長{measure}mm を超過: {:.2}mm 「{}」", l.width_mm(), l.text());
            }
        }
    }

    #[test]
    fn not_a_single_character_is_lost() {
        let want: String = SAMPLE.chars().filter(|c| *c != ' ').collect();
        let s = sheet_of(SAMPLE, 55.0);
        let got: String = s.lines.iter().flat_map(|l| l.cells.iter())
            .map(|c| c.ch).filter(|c| *c != ' ').collect();
        assert_eq!(got, want);
    }

    #[test]
    fn layout_uses_the_real_font_widths() {
        let data = font();
        let m = Metrics::new(&data).unwrap();
        let zen = m.advance_mm('あ', 10.5);
        let han = m.advance_mm('i', 10.5);
        assert!(zen > 3.0 && zen < 4.5, "全角の送りが不自然: {zen}mm");
        assert!(han < zen * 0.6, "半角が全角より十分細くない: {han}mm vs {zen}mm");
    }
}

#[cfg(test)]
mod format_tests {
    use super::*;

    fn doc(text: &str) -> Document {
        Document::plain(text)
    }

    #[test]
    fn typing_does_not_clear_the_format() {
        // 以前は set_body_text が段落を作り直していたので、打つたびに太字が消えた
        let mut d = doc("表題\n本文");
        d.apply_char_format(0..2, |f| f.bold = true);
        d.apply_align(0..2, Align::Center);
        // 1文字打った、のつもり
        d.set_body_text("表題あ\n本文");
        let p = d.paragraphs().next().unwrap();
        assert!(p.runs[0].fmt.bold, "太字が消えた");
        assert_eq!(p.align, Align::Center, "揃えが消えた");
    }

    #[test]
    fn adding_paragraphs_keeps_the_earlier_format() {
        let mut d = doc("表題");
        d.apply_char_format(0..2, |f| f.bold = true);
        d.set_body_text("表題\n新しい段落");
        let ps: Vec<_> = d.paragraphs().collect();
        assert!(ps[0].runs[0].fmt.bold);
        assert!(!ps[1].runs[0].fmt.bold, "新しい段落まで太字になった");
    }

    #[test]
    fn applies_only_to_the_selected_paragraphs() {
        let mut d = doc("一行目\n二行目\n三行目");
        // 「二行目」は 4..7(一行目=9バイト+改行)
        let start = "一行目\n".len();
        d.apply_char_format(start..start + 3, |f| f.bold = true);
        let ps: Vec<_> = d.paragraphs().collect();
        assert!(!ps[0].runs[0].fmt.bold, "上の段落まで太字になった");
        assert!(ps[1].runs[0].fmt.bold, "選んだ段落が太字にならない");
        assert!(!ps[2].runs[0].fmt.bold, "下の段落まで太字になった");
    }

    #[test]
    fn a_selection_spanning_several_paragraphs() {
        let mut d = doc("一行目\n二行目\n三行目");
        let end = "一行目\n二行目".len();
        d.apply_align(0..end, Align::Center);
        let ps: Vec<_> = d.paragraphs().collect();
        assert_eq!(ps[0].align, Align::Center);
        assert_eq!(ps[1].align, Align::Center);
        assert_eq!(ps[2].align, Align::Left, "選んでいない段落まで動いた");
    }

    #[test]
    fn reads_the_current_format() {
        // ボタンを押した状態に見せるために要る
        let mut d = doc("表題\n本文");
        d.apply_char_format(0..2, |f| f.bold = true);
        d.apply_align(0..2, Align::Right);
        assert!(d.char_format_at(0..2).bold);
        assert_eq!(d.align_at(0..2), Align::Right);
        let second = "表題\n".len();
        assert!(!d.char_format_at(second..second).bold, "別の段落の書式を返した");
    }

    #[test]
    fn the_table_is_not_lost() {
        let mut d = doc("本文");
        d.blocks.push(Block::Table(Table { col_mm: vec![], rows: vec![vec![Cellbox::default()]],
        ..Default::default()
    }));
        d.set_body_text("本文を直した");
        assert_eq!(d.tables().count(), 1, "表が消えた");
    }
}

#[cfg(test)]
mod align_tests {
    use super::*;

    fn sheet(text: &str, a: Align) -> Sheet {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain(text);
        d.apply_align(0..text.len(), a);
        layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 })
    }

    #[test]
    fn centering_leaves_equal_space_on_both_sides() {
        let s = sheet("表題", Align::Center);
        let line = &s.lines[0];
        let left = line.cells[0].x_mm;
        let right = 100.0 - (line.cells.last().unwrap().x_mm + line.cells.last().unwrap().w_mm);
        assert!((left - right).abs() < 0.01, "左 {left}mm / 右 {right}mm");
        assert!(left > 1.0, "中央に寄っていない");
    }

    #[test]
    fn right_alignment_reaches_the_line_end() {
        let s = sheet("表題", Align::Right);
        let last = s.lines[0].cells.last().unwrap();
        assert!((last.x_mm + last.w_mm - 100.0).abs() < 0.01, "右端に着いていない");
    }

    #[test]
    fn left_alignment_starts_at_zero() {
        assert_eq!(sheet("表題", Align::Left).lines[0].cells[0].x_mm, 0.0);
    }

    #[test]
    fn the_format_reaches_the_characters() {
        // 画面と紙が同じものを見るので、片方だけ太字になることが起きない
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("太字");
        d.apply_char_format(0..6, |f| f.bold = true);
        let s = layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 });
        assert!(s.lines[0].cells.iter().all(|c| c.fmt.bold), "字に書式が届いていない");
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;

    #[test]
    fn typing_does_not_reset_the_size() {
        let mut d = Document::plain("表題\n本文");
        d.apply_size(0..2, |s| s + 6.0);
        d.set_body_text("表題あ\n本文");
        assert_eq!(d.size_at(0..2), Some(16.5), "大きさが既定に戻った");
        let second = "表題あ\n".len();
        assert_eq!(d.size_at(second..second), Some(10.5), "他の段落まで変わった");
    }

    #[test]
    fn does_not_grow_without_bound() {
        // 0pt にすると本文が消えて、原因が分からなくなる
        let mut d = Document::plain("本文");
        for _ in 0..100 { d.apply_size(0..2, |s| s - 10.0) }
        assert!(d.size_at(0..2).unwrap() >= 4.0, "小さくしすぎた");
        for _ in 0..100 { d.apply_size(0..2, |s| s * 2.0) }
        assert!(d.size_at(0..2).unwrap() <= 400.0, "大きくしすぎた");
    }

    #[test]
    fn a_font_can_be_applied_to_a_paragraph() {
        let mut d = Document::plain("表題\n本文");
        d.apply_font(0..2, Some("BIZ UDPゴシック".into()));
        assert_eq!(d.paragraphs().next().unwrap().runs[0].font.as_deref(), Some("BIZ UDPゴシック"));
        assert_eq!(d.paragraphs().nth(1).unwrap().runs[0].font, None, "他の段落まで変わった");
    }
}

#[cfg(test)]
mod list_tests {
    use super::*;

    fn sheet(setup: impl Fn(&mut Document)) -> Sheet {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("一つ目\n二つ目\n三つ目");
        setup(&mut d);
        layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 })
    }

    fn text(s: &Sheet, i: usize) -> String {
        s.lines.get(i).map(|l| l.text()).unwrap_or_default()
    }

    #[test]
    fn the_bullet_mark_renders_before_the_text() {
        let s = sheet(|d| {
            for b in &mut d.blocks {
                if let Block::Para(p) = b { p.list = ListKind::Bullet }
            }
        });
        assert!(text(&s, 0).starts_with('・'), "印が出ていない: {:?}", text(&s, 0));
    }

    #[test]
    fn paragraph_numbers_run_in_sequence() {
        let s = sheet(|d| {
            for b in &mut d.blocks {
                if let Block::Para(p) = b { p.list = ListKind::Number }
            }
        });
        assert!(text(&s, 0).starts_with("1."), "{:?}", text(&s, 0));
        assert!(text(&s, 1).starts_with("2."), "{:?}", text(&s, 1));
        assert!(text(&s, 2).starts_with("3."), "{:?}", text(&s, 2));
    }

    #[test]
    fn marker_and_number_shape_change_with_the_level() {
        let mut p = Paragraph { list: ListKind::Bullet, ..Default::default() };
        assert_eq!(p.marker(0).as_deref(), Some("・"));
        p.indent = 1;
        assert_eq!(p.marker(0).as_deref(), Some("○"), "レベル2の印が変わらない");
        p.list = ListKind::Number;
        assert_eq!(p.marker(2).as_deref(), Some("(3) "), "レベル2の番号の形が違う");
    }

    /// **箇条書きの後の番号付きは1から。** 別のリストなので続けて数えない
    /// (2026-08-18、見本を実機で開いて「3.」から始まっているのを見つけた)。
    #[test]
    fn a_change_of_kind_restarts_the_numbering() {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("あ\nい\n一つ目\n二つ目");
        for (i, kind) in [
            (0usize, ListKind::Bullet),
            (1, ListKind::Bullet),
            (2, ListKind::Number),
            (3, ListKind::Number),
        ] {
            if let Block::Para(p) = &mut d.blocks[i] {
                p.list = kind;
            }
        }
        let s = layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 });
        let texts: Vec<String> = s.lines.iter().map(|l| l.text()).collect();
        assert!(texts[2].starts_with("1."), "番号が1から始まらない: {:?}", texts);
        assert!(texts[3].starts_with("2."), "2番目が違う: {:?}", texts);
    }

    #[test]
    fn deep_numbering_restarts_when_a_shallow_level_advances() {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("一\n一の一\n一の二\n二\n二の一");
        for (i, ind) in [(0usize, 0u8), (1, 1), (2, 1), (3, 0), (4, 1)] {
            if let Block::Para(p) = &mut d.blocks[i] {
                p.list = ListKind::Number;
                p.indent = ind;
            }
        }
        let s = layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 });
        let texts: Vec<String> = s.lines.iter().map(|l| l.text()).collect();
        assert!(texts[1].starts_with("(1) "), "{:?}", texts[1]);
        assert!(texts[2].starts_with("(2) "), "{:?}", texts[2]);
        assert!(texts[3].starts_with("2. "), "{:?}", texts[3]);
        assert!(texts[4].starts_with("(1) "), "深い数えが振り出しに戻らない: {:?}", texts[4]);
    }

    #[test]
    fn the_mark_does_not_rewrite_the_text() {
        // 編集中の文字位置とずれると、カーソルが合わなくなる
        let mut d = Document::plain("一つ目");
        if let Block::Para(p) = &mut d.blocks[0] { p.list = ListKind::Bullet }
        assert_eq!(d.body_text(), "一つ目", "本文に印が混ざった");
    }

    #[test]
    fn indenting_moves_text_right() {
        let plain = sheet(|_| {});
        let ind = sheet(|d| {
            for b in &mut d.blocks {
                if let Block::Para(p) = b { p.indent = 2 }
            }
        });
        assert!(ind.lines[0].cells[0].x_mm > plain.lines[0].cells[0].x_mm + 5.0,
                "インデントが効いていない");
    }

    #[test]
    fn line_spacing_separates_the_lines() {
        let plain = sheet(|_| {});
        let wide = sheet(|d| {
            for b in &mut d.blocks {
                if let Block::Para(p) = b { p.line_spacing = 2.0 }
            }
        });
        let gap = |s: &Sheet| s.lines[1].y_mm - s.lines[0].y_mm;
        assert!((gap(&wide) - gap(&plain) * 2.0).abs() < 0.1,
                "行間が倍になっていない: {} → {}", gap(&plain), gap(&wide));
    }

    #[test]
    fn indenting_shortens_the_line_length() {
        // 右端がはみ出さないこと
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let long = "あ".repeat(60);
        let mut d = Document::plain(&long);
        if let Block::Para(p) = &mut d.blocks[0] { p.indent = 3 }
        let s = layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 });
        for l in &s.lines {
            let right = l.cells.last().map(|c| c.x_mm + c.w_mm).unwrap_or(0.0);
            assert!(right <= 100.5, "行長を超えた: {right}mm");
        }
    }
}

#[cfg(test)]
mod vertical_tests {
    use super::*;

    #[test]
    fn vertical_writing_runs_right_to_left_and_top_to_bottom() {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let d = Document::plain("一行目の文。\n二行目。");
        let pg = PageSetup::default();
        let y0 = pg.top_mm + 4.0;
        let measure = pg.h_mm - pg.top_mm - pg.bottom_mm - 8.0;
        let mut sheet = layout(&d, &m,
            &Frame { measure_mm: measure, line_height_mm: 6.0, y0_mm: y0 });
        fold_vertical(&mut sheet, &pg, y0, 6.0);
        assert!(sheet.vertical);
        assert_eq!(sheet.vert_x.len(), sheet.lines.len());
        // 1列目は右端の近く、2列目はその左
        let right = pg.w_mm - pg.right_mm;
        assert!((sheet.vert_x[0] - (right - 6.0)).abs() < 0.5,
            "1列目が右端に無い: {}", sheet.vert_x[0]);
        assert!(sheet.vert_x[1] < sheet.vert_x[0], "2列目が左に来ていない");
        // 字は上から下(Cell.x_mm が増える)
        let cs = &sheet.lines[0].cells;
        assert!(cs[0].x_mm < cs[1].x_mm, "字が上から下に並んでいない");
        // 約物が縦用に置き換わる
        assert!(cs.iter().any(|c| c.ch == '︒'), "句点が縦用でない");
    }
}

#[cfg(test)]
mod ruby_tests {
    use super::*;

    #[test]
    fn the_ruby_line_sits_above_the_base_at_half_size() {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("組版の話");
        // 「組版」にだけルビを振る
        d.apply_char_format(0..6, |f| f.ruby = Some("くみはん".into()));
        let sheet = layout(&d, &m,
            &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 });
        let body: Vec<&Line> = sheet.lines.iter().filter(|l| l.from_body).collect();
        let ruby: Vec<&Line> = sheet.lines.iter().filter(|l| !l.from_body).collect();
        assert_eq!(body.len(), 1);
        assert_eq!(ruby.len(), 1, "ルビの行が無い");
        assert_eq!(ruby[0].text(), "くみはん");
        assert!(ruby[0].y_mm < body[0].y_mm, "ルビが基底より下にある");
        assert!((ruby[0].cells[0].size_pt - 5.25).abs() < 0.01, "半分の大きさでない");
        let bx0 = body[0].cells[0].x_mm;
        let bx1 = body[0].cells[1].x_mm + body[0].cells[1].w_mm;
        let rx0 = ruby[0].cells[0].x_mm;
        let rlast = ruby[0].cells.last().unwrap();
        let rx1 = rlast.x_mm + rlast.w_mm;
        let (bc, rc) = ((bx0 + bx1) / 2.0, (rx0 + rx1) / 2.0);
        assert!((bc - rc).abs() < 1.0, "ルビが基底の中央に来ていない: {bc} vs {rc}");
    }
}

#[cfg(test)]
mod distribute_tests {
    use super::*;

    #[test]
    fn distributed_alignment_stretches_the_last_line_too() {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("氏名");
        if let Some(Block::Para(p)) = d.blocks.first_mut() {
            p.align = Align::Distribute;
        }
        let sheet = layout(&d, &m,
            &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 });
        let line = &sheet.lines[0];
        let last = line.cells.last().unwrap();
        assert!(
            (last.x_mm + last.w_mm - 100.0).abs() < 0.5,
            "右端に届いていない: {}",
            last.x_mm + last.w_mm
        );
        assert!(line.cells[0].x_mm < 0.5, "左端から始まっていない");
    }
}

#[cfg(test)]
mod table_layout_tests {
    use super::*;

    fn doc_with_table() -> Document {
        let cell = |s: &str| Cellbox {
            paragraphs: vec![Paragraph {
                runs: vec![Run {
                    text: s.into(), size_pt: Some(10.5), font: None, fmt: Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut d = Document::plain("前の本文");
        d.blocks.push(Block::Table(Table {
            col_mm: vec![],
            rows: vec![
                vec![cell("品名"), cell("金額")],
                vec![cell("防火戸"), cell("120,000")],
            ],
        ..Default::default()
    }));
        d
    }

    fn sheet() -> Sheet {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        layout(&doc_with_table(), &m,
               &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 })
    }

    #[test]
    fn table_contents_reach_the_page() {
        let s = sheet();
        let all: String = s.lines.iter().map(|l| l.text()).collect();
        assert!(all.contains("品名"), "表のセルが描かれていない");
        assert!(all.contains("防火戸"));
    }

    #[test]
    fn table_rows_do_not_come_from_the_body_text() {
        // カーソルの位置合わせを壊さないための区別
        let s = sheet();
        let body: Vec<&Line> = s.lines.iter().filter(|l| l.from_body).collect();
        assert_eq!(body.len(), 1, "本文の行数が違う: {}", body.len());
        assert!(body[0].text().contains("前の本文"));
        assert!(s.lines.iter().any(|l| !l.from_body), "表の行が無い");
    }

    #[test]
    fn borders_are_drawn() {
        let s = sheet();
        // 2行の表: 横線3本 + 縦線(3本×2行) = 9本
        assert_eq!(s.rules.len(), 9, "罫線の数が違う: {}", s.rules.len());
        // 横線は行長いっぱい
        let h: Vec<_> = s.rules.iter().filter(|r| r[1] == r[3]).collect();
        assert_eq!(h.len(), 3);
        assert!(h.iter().all(|r| (r[2] - r[0] - 100.0).abs() < 0.01));
    }

    #[test]
    fn wraps_inside_a_cell() {
        let cell = |s: &str| Cellbox {
            paragraphs: vec![Paragraph {
                runs: vec![Run {
                    text: s.into(), size_pt: Some(10.5), font: None, fmt: Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut d = Document { note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), size_pt: None, endnote_fmt: Default::default(), font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false, blocks: vec![] };
        d.blocks.push(Block::Table(Table {
            col_mm: vec![],
            rows: vec![vec![cell(&"あ".repeat(30)), cell("短い")]],
        ..Default::default()
    }));
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let s = layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 });
        // 50mm の列に 30文字(約110mm)は3行になる
        let cell_lines = s.lines.iter().filter(|l| !l.from_body).count();
        assert!(cell_lines >= 3, "セルの中で折り返していない: {cell_lines} 行");
        // 右のセルにはみ出さない
        for l in s.lines.iter().filter(|l| !l.from_body) {
            if l.text().starts_with('あ') {
                let right = l.cells.last().map(|c| c.x_mm + c.w_mm).unwrap_or(0.0);
                assert!(right <= 50.0 + 0.5, "隣のセルへはみ出した: {right}mm");
            }
        }
    }
}

#[cfg(test)]
mod merge_layout_tests {
    use super::*;

    fn cell(s: &str) -> Cellbox {
        Cellbox {
            paragraphs: vec![Paragraph {
                runs: vec![Run {
                    text: s.into(), size_pt: Some(10.5), font: None, fmt: Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn sheet_of(rows: Vec<Vec<Cellbox>>) -> Sheet {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let d = Document { note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), size_pt: None, endnote_fmt: Default::default(),
            font: None, page: None, sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false,
            blocks: vec![Block::Table(Table { col_mm: vec![], rows,
        ..Default::default()
    })],
        };
        layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 })
    }

    #[test]
    fn a_horizontal_merge_spans_columns() {
        // 1行目: 見出しが2列ぶん。2行目: 普通の2列
        let mut head = cell("見出し");
        head.col_span = 2;
        let s = sheet_of(vec![
            vec![head],
            vec![cell("左"), cell("右")],
        ]);
        let b0 = s.cell_boxes.iter().find(|b| b.row == 0 && b.col == 0).unwrap();
        assert!((b0.w_mm - 100.0).abs() < 0.01, "結合したのに幅が広がらない: {}", b0.w_mm);
        // 結合の中(x=50)を縦線が横切らない(1行目の帯だけを見る)
        let mid_crosses = s.rules.iter().any(|r| {
            r[0] == r[2] && (r[0] - 50.0).abs() < 0.01 && r[1] < b0.top_mm + b0.h_mm - 0.1
                && r[3] > b0.top_mm + 0.1
        });
        assert!(!mid_crosses, "結合の中を縦線が横切った");
        // 2行目には x=50 の縦線がある
        let b1 = s.cell_boxes.iter().find(|b| b.row == 1 && b.col == 1).unwrap();
        assert!((b1.x_mm - 50.0).abs() < 0.01, "2行目の右セルの位置が違う: {}", b1.x_mm);
    }

    #[test]
    fn a_vertical_merge_spans_rows() {
        let mut start = cell("項目");
        start.v_merge = VMerge::Start;
        let mut cont = cell("");
        cont.v_merge = VMerge::Continue;
        let s = sheet_of(vec![
            vec![start, cell("1行目")],
            vec![cont, cell("2行目")],
        ]);
        // 呑まれたセルには当たり判定が無く、始まりのセルが2行ぶんに延びる
        assert!(s.cell_boxes.iter().all(|b| !(b.row == 1 && b.col == 0)),
            "呑まれたセルに当たり判定が残っている");
        let b0 = s.cell_boxes.iter().find(|b| b.row == 0 && b.col == 0).unwrap();
        let b1 = s.cell_boxes.iter().find(|b| b.row == 1 && b.col == 1).unwrap();
        let merged_bottom = b0.top_mm + b0.h_mm;
        let row1_bottom = b1.top_mm + b1.h_mm;
        assert!((merged_bottom - row1_bottom).abs() < 0.01,
            "結合が2行目の下端まで延びていない: {merged_bottom} vs {row1_bottom}");
        // 行の境の横線が、結合の中(左半分)を横切らない
        let boundary = b1.top_mm;
        for r in s.rules.iter().filter(|r| r[1] == r[3] && (r[1] - boundary).abs() < 0.01) {
            assert!(r[0] >= 50.0 - 0.01,
                "結合の中を横線が横切った: x {}..{}", r[0], r[2]);
        }
    }
}

#[cfg(test)]
mod gridcol_tests {
    use super::*;

    fn cell(s: &str) -> Cellbox {
        Cellbox {
            paragraphs: vec![Paragraph {
                runs: vec![Run {
                    text: s.into(), size_pt: Some(10.5), font: None, fmt: Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn rules_of(col_mm: Vec<f32>) -> Vec<[f32; 4]> {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let d = Document { note_ids_taken: Vec::new(), template: None, attrs: Vec::new(), styles: Vec::new(), styles_new: Vec::new(),  footnote_fmt: Default::default(), size_pt: None, endnote_fmt: Default::default(),
            font: None,
            page: None,
            sect_raw: None, footnotes: Vec::new(), header: Default::default(), footer: Default::default(), page_color: None, watermark: None, ink: Vec::new(), track_author: None, hyphenate: false, protection: None, props: Default::default(), vertical: false,
            blocks: vec![Block::Table(Table {
                col_mm,
                rows: vec![vec![cell("項目"), cell("値")]],
        ..Default::default()
    })],
        };
        layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 }).rules
    }

    #[test]
    fn column_width_specs_take_effect() {
        // 30mm + 70mm の2列。縦線が 0, 30, 100 に立つ
        let rules = rules_of(vec![30.0, 70.0]);
        let mut vx: Vec<f32> = rules.iter().filter(|r| r[0] == r[2]).map(|r| r[0]).collect();
        vx.sort_by(f32::total_cmp);
        vx.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        assert_eq!(vx.len(), 3, "{vx:?}");
        assert!((vx[1] - 30.0).abs() < 0.01, "指定した列幅で立っていない: {vx:?}");
    }

    #[test]
    fn specs_over_the_line_length_shrink_proportionally() {
        // 120+80=200mm を 100mm に。比率 3:2 のまま 60/40 になる
        let rules = rules_of(vec![120.0, 80.0]);
        let mut vx: Vec<f32> = rules.iter().filter(|r| r[0] == r[2]).map(|r| r[0]).collect();
        vx.sort_by(f32::total_cmp);
        vx.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        assert!((vx[1] - 60.0).abs() < 0.1, "比率が守られていない: {vx:?}");
        assert!((vx[2] - 100.0).abs() < 0.1, "右へはみ出した: {vx:?}");
    }
}

#[cfg(test)]
mod empty_line_tests {
    use super::*;

    #[test]
    fn an_empty_paragraph_still_holds_a_line() {
        // 持たないと、後ろの行のバイト勘定がずれてカーソルが合わなくなる
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let d = Document::plain("一行目\n\n三行目");
        let s = layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 });
        let body: Vec<&Line> = s.lines.iter().filter(|l| l.from_body).collect();
        assert_eq!(body.len(), 3, "空行が消えた: {} 行", body.len());
        assert!(body[1].cells.is_empty());
        // 3行目は2行ぶん下にある
        assert!((body[2].y_mm - body[0].y_mm - 12.0).abs() < 0.01);
    }
}

#[cfg(test)]
mod byte0_tests {
    use super::*;

    fn lines(text: &str, measure: f32) -> Vec<Line> {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let d = Document::plain(text);
        layout(&d, &m, &Frame { measure_mm: measure, line_height_mm: 6.0, y0_mm: 20.0 })
            .lines
    }

    #[test]
    fn wrapping_keeps_line_byte_positions_matching_the_text() {
        let _lang = crate::font::lang_lock();
        // 「行の文字数 + 1」で数えると、折り返した行の数だけずれていた
        let text = "あ".repeat(40); // 100mm に入らないので折り返す
        let ls = lines(&text, 100.0);
        assert!(ls.len() >= 2, "折り返していない");
        for l in &ls {
            // byte0 の位置の字が、その行の先頭の字と一致する
            let head = text[l.byte0..].chars().next().unwrap();
            assert_eq!(head, l.cells[0].ch, "byte0 がずれている");
        }
        // 連結すると本文に戻る(空白落ちのない文)
        let total: usize = ls.iter().map(|l| l.byte_end() - l.byte0).sum();
        assert_eq!(total, text.len());
    }

    #[test]
    fn dropped_whitespace_does_not_shift_positions() {
        // 行末で捨てた空白のぶん、次の行の byte0 が進んでいること
        let text = format!("{} {}", "a".repeat(40), "b".repeat(40));
        let ls = lines(&text, 60.0);
        assert!(ls.len() >= 2);
        let l2 = &ls[1];
        let head = text[l2.byte0..].chars().next().unwrap();
        assert_eq!(head, l2.cells[0].ch, "落ちた空白の勘定が入っていない");
    }

    #[test]
    fn still_matches_across_paragraphs() {
        let text = "一つ目\n二つ目の段落\n三";
        let ls = lines(text, 100.0);
        for l in &ls {
            if l.cells.is_empty() { continue }
            let head = text[l.byte0..].chars().next().unwrap();
            assert_eq!(head, l.cells[0].ch);
        }
    }

    #[test]
    fn the_bullet_mark_is_not_counted_in_byte_positions() {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain("項目");
        if let Block::Para(p) = &mut d.blocks[0] { p.list = ListKind::Bullet }
        let s = layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 });
        let l = &s.lines[0];
        assert_eq!(l.byte0, 0);
        // 印(・)ぶんが byte_end に乗っていない
        assert_eq!(l.byte_end(), "項目".len(), "印が本文のバイトに混ざった");
    }
}

#[cfg(test)]
mod run_edit_tests {
    use super::*;

    fn bold_spans(d: &Document) -> Vec<(String, bool)> {
        d.paragraphs()
            .flat_map(|p| p.runs.iter())
            .map(|r| (r.text.clone(), r.fmt.bold))
            .collect()
    }

    #[test]
    fn bold_can_cover_only_part_of_a_paragraph() {
        let mut d = Document::plain("防火戸の仕様を確認");
        let s = "防火戸の".len();
        let e = "防火戸の仕様".len();
        d.apply_char_format(s..e, |f| f.bold = true);
        assert_eq!(
            bold_spans(&d),
            vec![
                ("防火戸の".into(), false),
                ("仕様".into(), true),
                ("を確認".into(), false)
            ],
            "選択の字だけが太字になっていない"
        );
    }

    #[test]
    fn partial_formatting_survives_typing() {
        let mut d = Document::plain("防火戸の仕様を確認");
        let s = "防火戸の".len();
        let e = "防火戸の仕様".len();
        d.apply_char_format(s..e, |f| f.bold = true);
        // 「仕様」の後ろに「書」を打った(1回の編集 = 1箇所の置き換え)
        d.set_body_text("防火戸の仕様書を確認");
        assert_eq!(
            bold_spans(&d),
            vec![
                ("防火戸の".into(), false),
                ("仕様書".into(), true),
                ("を確認".into(), false)
            ],
            "太字の中に打った字が太字にならない・境が流された"
        );
        // 頭に打っても境は動かない
        d.set_body_text("この防火戸の仕様書を確認");
        assert_eq!(bold_spans(&d)[1], ("仕様書".into(), true), "頭への挿入で境がずれた");
    }

    #[test]
    fn clearing_inside_the_selection_keeps_the_boundaries() {
        let mut d = Document::plain("あいうえお");
        d.apply_char_format(3..12, |f| f.bold = true); // いうえ
        d.set_body_text("あいえお"); // 「う」を消した
        assert_eq!(
            bold_spans(&d),
            vec![("あ".into(), false), ("いえ".into(), true), ("お".into(), false)],
            "削除で境が流された"
        );
    }

    #[test]
    fn pressing_enter_mid_document_does_not_shift_later_properties() {
        // 旧方式(段落番号で写す)の持病: 段落の増減で下の段落の性質がずれた
        let mut d = Document::plain("一\n二\n三");
        let start = "一\n二".len();
        d.apply_align(start + 1..start + 1, Align::Center); // 「三」を中央に
        if let Block::Para(p) = &mut d.blocks[2] {
            p.shade = Some("FFF2CC".into());
        }
        // 「二」の後ろで Enter
        d.set_body_text("一\n二\n\n三");
        let ps: Vec<&Paragraph> = d.paragraphs().collect();
        assert_eq!(ps.len(), 4);
        assert_eq!(ps[3].align, Align::Center, "下の段落の揃えがずれた");
        assert_eq!(ps[3].shade.as_deref(), Some("FFF2CC"), "下の段落の帯がずれた");
        // 割った両方が段落の性質を持つ(Word と同じ)。下へ「ずれる」のとは違う
        assert_eq!(ps[2].shade.as_deref(), Some("FFF2CC"));
        // undo(= 逆向きの1回の編集)でも戻る
        d.set_body_text("一\n二\n三");
        let ps: Vec<&Paragraph> = d.paragraphs().collect();
        assert_eq!(ps[2].align, Align::Center, "undo で揃えが消えた");
    }

    #[test]
    fn editing_does_not_move_the_table() {
        // 旧方式は打鍵のたびに表が末尾へ動いていた
        let mut d = Document::plain("前\n後");
        d.blocks.insert(1, Block::Table(Table {
            col_mm: vec![],
            rows: vec![vec![Cellbox::default()]],
        ..Default::default()
    }));
        d.set_body_text("前に足す\n後");
        let kinds: Vec<&str> = d.blocks.iter().map(|b| match b {
            Block::Para(_) => "段落",
            Block::Table(_) => "表",
        }).collect();
        assert_eq!(kinds, vec!["段落", "表", "段落"], "表が動いた: {kinds:?}");
    }

    #[test]
    fn merging_paragraphs_keeps_the_first_ones_properties() {
        let mut d = Document::plain("一\n二");
        d.apply_align(0..0, Align::Center);
        // 「一」と「二」の間の改行を消した
        d.set_body_text("一二");
        let ps: Vec<&Paragraph> = d.paragraphs().collect();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].align, Align::Center, "合流で頭の性質が消えた");
        assert_eq!(ps[0].runs[0].text, "一二");
    }

    #[test]
    fn size_and_font_apply_only_to_the_selection() {
        let mut d = Document::plain("見出しと本文");
        d.apply_size(0.."見出し".len(), |_| 16.0);
        d.apply_font(0.."見出し".len(), Some("ゴシック".into()));
        let runs: Vec<&Run> = d.paragraphs().flat_map(|p| p.runs.iter()).collect();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].size_pt, Some(16.0));
        assert_eq!(runs[0].font.as_deref(), Some("ゴシック"));
        assert_eq!(runs[1].size_pt, None, "選択の外まで変わった");
        assert_eq!(runs[1].font, None);
    }

    #[test]
    fn reads_the_format_at_the_caret() {
        let mut d = Document::plain("あ太字い");
        d.apply_char_format(3..9, |f| f.bold = true);
        assert!(!d.char_format_at(0..0).bold, "頭で太字と言った");
        assert!(d.char_format_at(9..9).bold, "太字の直後で太字と言わない");
        assert!(!d.char_format_at(12..12).bold, "太字の外で太字と言った");
    }
}

#[cfg(test)]
mod ref_field_tests {
    use super::*;

    fn field_doc() -> Document {
        let mut d = Document::plain("仕様は3ページを見る");
        let s = "仕様は".len();
        let e = "仕様は3ページ".len();
        d.apply_field(s..e, Some(RefField { name: "様式".into(), page: false }));
        d
    }

    #[test]
    fn a_reference_survives_editing_and_typing_around_it() {
        let mut d = field_doc();
        // 参照の前に打つ
        d.set_body_text("この仕様は3ページを見る");
        let f: Vec<_> = d.paragraphs().flat_map(|p| p.runs.iter())
            .filter(|r| r.fmt.field.is_some())
            .map(|r| r.text.clone())
            .collect();
        assert_eq!(f, vec!["3ページ"], "参照が前の打鍵で壊れた");
        // 参照の直後に打っても、参照は伸びない
        let e = "この仕様は3ページ".len();
        let mut t = d.body_text();
        t.insert(e, '目');
        d.set_body_text(&t);
        let f: Vec<_> = d.paragraphs().flat_map(|p| p.runs.iter())
            .filter(|r| r.fmt.field.is_some())
            .map(|r| r.text.clone())
            .collect();
        assert_eq!(f, vec!["3ページ"], "打った字が参照に呑まれた");
    }

    #[test]
    fn editing_inside_a_reference_turns_it_into_plain_text() {
        let mut d = field_doc();
        // 「3ページ」の中の「ペ」を消した
        d.set_body_text("仕様は3ージを見る");
        assert!(d.paragraphs().flat_map(|p| p.runs.iter())
            .all(|r| r.fmt.field.is_none()),
            "壊れた参照が参照のまま残った");
        assert_eq!(d.body_text(), "仕様は3ージを見る", "本文まで変わった");
    }

    #[test]
    fn a_references_value_can_be_recalculated() {
        let mut d = field_doc();
        let n = d.refresh_fields(|name, page| {
            assert_eq!(name, "様式");
            assert!(!page);
            Some("5ページ".into())
        });
        assert_eq!(n, 1);
        assert_eq!(d.body_text(), "仕様は5ページを見る");
        // 同じ値ならもう数えない
        assert_eq!(d.refresh_fields(|_, _| Some("5ページ".into())), 0);
    }

    #[test]
    fn bolding_over_a_reference_keeps_the_reference() {
        let mut d = field_doc();
        d.apply_char_format(0..d.body_text().len(), |f| f.bold = true);
        let r: Vec<_> = d.paragraphs().flat_map(|p| p.runs.iter())
            .filter(|r| r.fmt.field.is_some()).collect();
        assert_eq!(r.len(), 1, "太字で参照が消えた");
        assert!(r[0].fmt.bold);
    }
}

#[cfg(test)]
mod hyphen_tests {
    use super::*;

    #[test]
    fn english_words_break_at_syllables_with_a_hyphen() {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let text = "The quick information hyphenation representation communication demonstration";
        let mut d = Document::plain(text);
        d.hyphenate = true;
        let s = layout(&d, &m, &Frame { measure_mm: 45.0, line_height_mm: 6.0, y0_mm: 20.0 });
        let joined: Vec<String> = s.lines.iter().map(|l| l.text()).collect();
        assert!(joined.iter().any(|l| l.ends_with('-')),
            "どの行末にもハイフンが無い: {joined:?}");
        for l in &s.lines {
            assert!(l.width_mm() <= 45.1, "行長を超えた: {}", l.width_mm());
        }
        // ハイフンを除けば、文字は一つも失われない
        let got: String = s.lines.iter().flat_map(|l| l.cells.iter())
            .map(|c| c.ch).filter(|c| *c != '-' && *c != ' ').collect();
        let want: String = text.chars().filter(|c| *c != ' ').collect();
        assert_eq!(got, want, "ハイフネーションで字が消えた");
    }

    #[test]
    fn no_split_means_no_change() {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let d = Document::plain("The quick information hyphenation");
        let s = layout(&d, &m, &Frame { measure_mm: 45.0, line_height_mm: 6.0, y0_mm: 20.0 });
        assert!(s.lines.iter().all(|l| !l.text().ends_with('-')),
            "設定していないのに折った");
    }
}

#[cfg(test)]
mod dropcap_tests {
    use super::*;

    #[test]
    fn the_first_character_is_large_and_the_rest_is_set_narrow() {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let mut d = Document::plain(&format!("春{}", "はあけぼの。".repeat(8)));
        if let Block::Para(p) = &mut d.blocks[0] {
            p.dropcap = true;
        }
        let s = layout(&d, &m, &Frame { measure_mm: 100.0, line_height_mm: 6.0, y0_mm: 20.0 });
        let cap = &s.lines[0];
        assert_eq!(cap.text(), "春");
        assert!(cap.cells[0].size_pt > 25.0, "頭の字が大きくない: {}", cap.cells[0].size_pt);
        // 本文は頭の字の右から始まり、バイト位置は「春」の後ろから
        let body = &s.lines[1];
        assert!(body.cells[0].x_mm > cap.cells[0].w_mm - 0.5, "本文が頭の字に重なる");
        assert_eq!(body.byte0, "春".len(), "バイト勘定がずれた");
        // 文字は一つも失われない
        let got: String = s.lines.iter().flat_map(|l| l.cells.iter()).map(|c| c.ch).collect();
        assert_eq!(got.chars().count(), format!("春{}", "はあけぼの。".repeat(8)).chars().count());
    }
}

#[cfg(test)]
mod column_tests {
    use super::*;

    fn folded(n_lines: usize, cols: u8) -> Sheet {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let text = (1..=n_lines).map(|i| format!("{i} 行目")).collect::<Vec<_>>().join("\n");
        let d = Document::plain(&text);
        let pg = PageSetup { columns: cols, ..Default::default() };
        let mut s = layout(&d, &m, &Frame {
            measure_mm: pg.column_measure_mm(),
            line_height_mm: 6.4,
            y0_mm: 24.0,
        });
        fold_columns(&mut s, &pg, 24.0);
        s
    }

    #[test]
    fn two_column_layout_continues_into_the_right_column() {
        // A4 の1段は約40行。60行なら 1ページ目の左40行 + 右20行
        let s = folded(60, 2);
        let pg = PageSetup { columns: 2, ..Default::default() };
        let left: Vec<&Line> = s.lines.iter()
            .filter(|l| l.cells[0].x_mm < pg.column_measure_mm()).collect();
        let right: Vec<&Line> = s.lines.iter()
            .filter(|l| l.cells[0].x_mm >= pg.column_measure_mm()).collect();
        assert!(!right.is_empty(), "右の段に何も行かない");
        assert!(left.len() > right.len(), "左から詰まっていない");
        // 右の段の頭は左の段の頭と同じ高さ(ページの頭)
        assert!((right[0].y_mm - s.lines[0].y_mm).abs() < 0.01,
            "右の段がページの頭から始まらない: {} vs {}", right[0].y_mm, s.lines[0].y_mm);
        // ページは1枚に収まる(60行 = 2段ぶん以内)
        assert!(s.breaks.is_empty(), "1ページに収まるのに頁が割れた");
    }

    #[test]
    fn two_columns_overflow_to_the_next_page() {
        let s = folded(100, 2);
        assert!(!s.breaks.is_empty(), "2段×1ページを超えたのに頁が割れない");
        let pg = PageSetup::default();
        let last = s.lines.last().unwrap();
        assert!(last.y_mm > pg.h_mm, "2ページ目の座標が積み上がっていない");
        // どの行も段の中に収まる(x が紙からはみ出さない)
        for l in &s.lines {
            let right = l.cells.last().map(|c| c.x_mm + c.w_mm).unwrap_or(0.0);
            assert!(right <= pg.measure_mm() + 0.5, "段からはみ出した: {right}mm");
        }
    }

    #[test]
    fn a_single_column_changes_nothing() {
        let a = folded(30, 1);
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let text = (1..=30).map(|i| format!("{i} 行目")).collect::<Vec<_>>().join("\n");
        let d = Document::plain(&text);
        let b = layout(&d, &m, &Frame {
            measure_mm: PageSetup::default().column_measure_mm(),
            line_height_mm: 6.4,
            y0_mm: 24.0,
        });
        assert_eq!(a.lines.len(), b.lines.len());
        for (x, y) in a.lines.iter().zip(&b.lines) {
            assert!((x.y_mm - y.y_mm).abs() < 0.001, "1段なのに動いた");
        }
    }
}

#[cfg(test)]
mod hf_layout_tests {
    use super::*;

    fn metrics() -> Vec<u8> {
        test_font()
    }

    fn hf(text: &str) -> HeadFoot {
        HeadFoot {
            paragraphs: Document::plain(text).paragraphs().cloned().collect(),
            part: None,
        }
    }

    #[test]
    fn the_page_number_field_becomes_the_number() {
        let data = metrics();
        let m = Metrics::new(&data).unwrap();
        let pg = PageSetup::default();
        let f = hf(&format!("- {PAGE_MARK} -"));
        assert_eq!(layout_hf(&f, &m, &pg, 6.4, 1, 9, true, DEFAULT_PT)[0].text(), "- 1 -");
        assert_eq!(layout_hf(&f, &m, &pg, 6.4, 12, 9, true, DEFAULT_PT)[0].text(), "- 12 -");
    }

    #[test]
    fn header_goes_in_the_top_margin_footer_in_the_bottom() {
        let data = metrics();
        let m = Metrics::new(&data).unwrap();
        let pg = PageSetup::default();
        let h = layout_hf(&hf("頭"), &m, &pg, 6.4, 1, 1, false, DEFAULT_PT);
        assert!(h[0].y_mm < pg.top_mm, "ヘッダーが本文域に食い込む: {}", h[0].y_mm);
        assert!(h[0].y_mm > 0.0);
        let f = layout_hf(&hf("足"), &m, &pg, 6.4, 1, 1, true, DEFAULT_PT);
        assert!(f[0].y_mm > pg.h_mm - pg.bottom_mm,
            "フッターが本文域に食い込む: {}", f[0].y_mm);
        assert!(f[0].y_mm < pg.h_mm, "紙の外に出た: {}", f[0].y_mm);
    }

    #[test]
    fn footer_centering_works() {
        let data = metrics();
        let m = Metrics::new(&data).unwrap();
        let pg = PageSetup::default();
        let mut f = hf(&PAGE_MARK.to_string());
        f.paragraphs[0].align = Align::Center;
        let lines = layout_hf(&f, &m, &pg, 6.4, 1, 9, true, DEFAULT_PT);
        assert!(lines[0].cells[0].x_mm > pg.measure_mm() * 0.3,
            "中央に寄っていない: {}", lines[0].cells[0].x_mm);
    }

    #[test]
    fn the_page_count_field_becomes_the_total() {
        let data = metrics();
        let m = Metrics::new(&data).unwrap();
        let pg = PageSetup::default();
        let f = hf(&format!("{PAGE_MARK} / {PAGES_MARK}"));
        assert_eq!(layout_hf(&f, &m, &pg, 6.4, 2, 7, true, DEFAULT_PT)[0].text(), "2 / 7");
    }

    #[test]
    fn nothing_renders_when_there_is_nothing() {
        let data = metrics();
        let m = Metrics::new(&data).unwrap();
        assert!(layout_hf(&HeadFoot::default(), &m, &PageSetup::default(), 6.4, 1, 1, false,
                          DEFAULT_PT)
            .is_empty());
    }

    #[test]
    fn converting_to_plain_text_and_back_keeps_the_format() {
        // パネルでの編集は paras_text / set_paras_text を通る
        let mut ps: Vec<Paragraph> =
            Document::plain("社外秘").paragraphs().cloned().collect();
        ps[0].align = Align::Right;
        ps[0].runs[0].fmt.bold = true;
        set_paras_text(&mut ps, "社外秘・控");
        assert_eq!(paras_text(&ps), "社外秘・控");
        assert_eq!(ps[0].align, Align::Right, "揃えが消えた");
        assert!(ps[0].runs[0].fmt.bold, "太字が消えた");
    }
}

#[cfg(test)]
mod shade_carry_tests {
    use super::*;

    #[test]
    fn editing_keeps_the_paragraph_band_and_border() {
        // set_body_text は段落をまるごと写すので、新しい性質も自動で残る
        let mut d = Document::plain("見出し\n本文");
        if let Block::Para(p) = &mut d.blocks[0] {
            p.shade = Some("DEEAF6".into());
            p.boxed = true;
        }
        d.set_body_text("見出しに追記\n本文");
        let p = d.paragraphs().next().unwrap();
        assert_eq!(p.shade.as_deref(), Some("DEEAF6"), "1文字打つだけで帯が消えた");
        assert!(p.boxed, "枠が消えた");
    }
}

/// 節が途中で変わる文書の組版。**`w:sectPr` は節の「終わり」に置かれる**ので、
/// どの段落がどの節かを1つ取り違えると全部ずれる。そこを釘で打つ試験。
#[cfg(test)]
mod section_layout_tests {
    use super::*;

    fn paper(w: f32, h: f32) -> PageSetup {
        PageSetup { w_mm: w, h_mm: h, left_mm: 20.0, right_mm: 20.0,
                    top_mm: 20.0, bottom_mm: 20.0, columns: 1 }
    }

    fn tab(text: &str, sect: Option<PageSetup>) -> Block {
        tab_c(text, sect, false)
    }

    /// 節の種類まで指定する版(continuous = 改ページしない)
    fn tab_c(text: &str, sect: Option<PageSetup>, continuous: bool) -> Block {
        Block::Para(Paragraph {
            runs: vec![Run { text: text.into(), size_pt: Some(10.5), font: None,
                             fmt: Default::default() }],
            line_spacing: 1.0,
            sect: sect.map(|page| SectionBreak { raw: String::new(), page, continuous }),
            ..Default::default()
        })
    }

    /// **真ん中だけ横向きの3節。** 節の境目の手前の段落と、最後の節
    /// (`Document::page` から来るほう)が要注意 — 発注者 2026-08-10
    fn three_sections() -> Document {
        let vertical = paper(210.0, 297.0);
        let sideways = paper(297.0, 210.0);
        Document {
            // 最後の節は Document::page が持つ(docx がそう書く)
            page: Some(vertical),
            blocks: vec![
                tab("第一節の本文", None),
                tab("第一節の終わり", Some(vertical)),   // ここまでが縦
                tab("第二節の本文", None),
                tab("第二節の終わり", Some(sideways)),   // ここまでが横
                tab("第三節の本文", None),          // ここは Document::page = 縦
            ],
            ..Default::default()
        }
    }

    #[test]
    fn the_last_paragraph_belongs_to_its_own_section() {
        let geo = section_geometry(&three_sections());
        let w: Vec<f32> = geo.iter().map(|g| g.w_mm).collect();
        // **1つずれていないか。** 節末の段落(添字1・3)は自分の節の紙で組む
        assert_eq!(w, vec![210.0, 210.0, 297.0, 297.0, 210.0],
            "節の割り当てがずれている: {w:?}");
    }

    #[test]
    fn a_new_section_changes_the_line_length() {
        let d = three_sections();
        let geo = section_geometry(&d);
        // 横の節は紙が広いぶん行長も広い(折り返しがやり直しになる所)
        assert!(geo[2].column_measure_mm() > geo[0].column_measure_mm() + 50.0,
            "横の節で行長が広がっていない: {} / {}",
            geo[2].column_measure_mm(), geo[0].column_measure_mm());
    }


    #[test]
    fn continuous_sections_do_not_split_pages() {
        // 段組みを変えるためだけの節が実物には多い。そこで改ページすると
        // **見た目が大きく変わる**(2026-08-10、pyoffice の指摘)
        let vertical = paper(210.0, 297.0);
        let d = Document {
            page: Some(vertical),
            blocks: vec![
                tab_c("一段の所", Some(vertical), true),   // continuous: 紙は同じ
                tab("二段の所", None),
            ],
            ..Default::default()
        };
        let s = layout_for_test(&d);
        assert!(s.breaks.is_empty(), "continuous で頁を割った: {:?}", s.breaks);
        // 紙が変わらないので、節ごとの紙も増えない(先頭の1つだけ)
        assert_eq!(s.sect_pages.len(), 1, "紙が変わらないのに増えた: {:?}", s.sect_pages);
    }

    #[test]
    fn continuous_still_splits_when_the_paper_size_differs() {
        // 1枚の紙は1つの大きさしか取れない。continuous でも従えない所
        let d = Document {
            page: Some(paper(297.0, 210.0)),                 // 横
            blocks: vec![
                tab_c("縦の所", Some(paper(210.0, 297.0)), true), // continuous だが縦→横
                tab("横の所", None),
            ],
            ..Default::default()
        };
        let s = layout_for_test(&d);
        assert_eq!(s.breaks.len(), 1, "紙が変わるのに割らなかった: {:?}", s.breaks);
        assert_eq!(s.sect_pages.len(), 2, "紙の切り替えが無い: {:?}", s.sect_pages);
    }

    #[test]
    fn nextpage_sections_still_split() {
        let vertical = paper(210.0, 297.0);
        let d = Document {
            page: Some(vertical),
            blocks: vec![tab_c("前", Some(vertical), false), tab("後", None)],
            ..Default::default()
        };
        assert_eq!(layout_for_test(&d).breaks.len(), 1, "nextPage で割らなかった");
    }

    /// 試験用に組む(フォントは実体を使う)
    fn layout_for_test(d: &Document) -> Sheet {
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        layout(d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 20.0 })
    }

    #[test]
    fn a_single_section_still_carries_nothing() {
        let d = Document {
            page: Some(paper(210.0, 297.0)),
            blocks: vec![tab("本文", None)],
            ..Default::default()
        };
        assert!(section_geometry(&d).is_empty(), "節が1つなのに節ごとの紙を作った");
    }
}

#[cfg(test)]
mod footnote_mark_tests {
    use super::*;

    /// **字を持たない run は均しで落とされる。** 脚注の印はそこに乗るので、
    /// 守っていないと「読めているのに、編集や組版を一度通っただけで消える」
    /// という形になる(2026-08-10)
    #[test]
    fn justifying_keeps_the_footnote_mark() {
        let mark = |id: &str| Run {
            text: String::new(), size_pt: Some(10.5), font: None,
            fmt: CharFormat {
                footnote: Some(FootnoteRef { id: id.into(), endnote: false }),
                ..Default::default()
            },
        };
        let text = |t: &str| Run {
            text: t.into(), size_pt: Some(10.5), font: None, fmt: CharFormat::default(),
        };
        let mut runs = vec![text("本文"), mark("20"), text("の続き"), mark("21")];
        normalize_runs(&mut runs);
        assert_eq!(runs.iter().filter(|r| r.fmt.footnote.is_some()).count(), 2,
            "印が落ちた: {runs:?}");
        // 印を挟んだ字どうしは繋げない(繋ぐと印が字の外へ出る)
        let order: Vec<&str> = runs.iter()
            .map(|r| if r.fmt.footnote.is_some() { "印" } else { r.text.as_str() })
            .collect();
        assert_eq!(order, vec!["本文", "印", "の続き", "印"], "並びが変わった");
    }

    /// 印の付いていない空の run は今までどおり落とす(増やさない)
    #[test]
    fn an_empty_run_with_no_mark_is_still_dropped() {
        let mut runs = vec![
            Run { text: "あ".into(), size_pt: Some(10.5), font: None, fmt: CharFormat::default() },
            Run { text: String::new(), size_pt: Some(10.5), font: None, fmt: CharFormat::default() },
        ];
        normalize_runs(&mut runs);
        assert_eq!(runs.len(), 1, "空の run が残った: {runs:?}");
    }
}

#[cfg(test)]
mod footnote_layout_tests {
    use super::*;

    fn build(d: &Document) -> Sheet {
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        layout(d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 20.0 })
    }

    fn mark(id: &str) -> Run {
        Run { text: String::new(), size_pt: Some(10.5), font: None,
              fmt: CharFormat {
                  footnote: Some(FootnoteRef { id: id.into(), endnote: false }),
                  ..Default::default() } }
    }
    fn text(t: &str) -> Run {
        Run { text: t.into(), size_pt: Some(10.5), font: None, fmt: CharFormat::default() }
    }
    fn tab(runs: Vec<Run>) -> Block {
        Block::Para(Paragraph { runs, line_spacing: 1.0, ..Default::default() })
    }
    fn tail_mark(id: &str) -> Run {
        Run { text: String::new(), size_pt: Some(10.5), font: None,
              fmt: CharFormat {
                  footnote: Some(FootnoteRef { id: id.into(), endnote: true }),
                  ..Default::default() } }
    }
    fn note(id: &str, endnote: bool, t: &str) -> Footnote {
        Footnote { added: false, id: id.into(), endnote,
                   paragraphs: vec![Paragraph { runs: vec![text(t)], line_spacing: 1.0,
                                                ..Default::default() }] }
    }

    /// **脚注と文末脚注は id が衝突する。** docx は `footnotes.xml` と
    /// `endnotes.xml` を別々に番号付けするので、どちらも 1・2・3… から始まる。
    /// id だけで引くと、印に別の注の文章が付く(2026-08-10 に踏んだ)
    #[test]
    fn colliding_footnote_and_endnote_ids_are_not_confused() {
        let d = Document {
            blocks: vec![tab(vec![text("あ"), mark("2"), text("い"), tail_mark("2")])],
            footnotes: vec![
                note("2", false, "これは脚注"),
                note("2", true, "これは文末脚注"),
            ],
            ..Default::default()
        };
        let s = build(&d);
        // **紙の下に来るのは脚注だけ。** 文末脚注は文書の末尾へ回るので
        // ここには入らない(2026-08-11 に置き場を分けた)
        assert_eq!(s.notes.len(), 1, "紙の下に文末脚注まで来た");
        let sentence = |n: &NoteBlock| -> String {
            n.lines.iter().flat_map(|l| l.cells.iter()).map(|c| c.ch).collect()
        };
        assert!(sentence(&s.notes[0]).contains("これは脚注"),
            "脚注の印に別の注が付いた: {:?}", sentence(&s.notes[0]));
        let full_text: String = s.lines.iter()
            .flat_map(|l| l.cells.iter()).map(|c| c.ch).collect();
        assert!(full_text.contains("これは文末脚注"), "文末脚注が末尾に出ていない");
        assert!(!full_text.contains("これは脚注"), "脚注まで本文へ流れた");
    }

    /// **番号は出てくる順**。docx の id は書き手ごとにばらばら
    /// (LibreOffice は 2・3・4、pandoc は 20・21・22)なので、
    /// id をそのまま出すと 2 から始まる脚注になってしまう
    #[test]
    fn footnote_numbers_follow_the_order_of_appearance() {
        let d = Document {
            blocks: vec![
                tab(vec![text("あ"), mark("20"), text("い"), mark("21")]),
                tab(vec![text("う"), mark("22")]),
            ],
            ..Default::default()
        };
        let s = build(&d);
        let sup: String = s.lines.iter()
            .flat_map(|l| l.cells.iter())
            .filter(|c| c.fmt.superscript)
            .map(|c| c.ch)
            .collect();
        assert_eq!(sup, "123", "番号が出てくる順でない: {sup:?}");
    }

    /// 印は**本文の字ではない**。カーソルが本文とずれないよう、
    /// 番号を出しても後ろの字のバイト位置は動かない
    #[test]
    fn showing_numbers_does_not_move_text_byte_positions() {
        let d = Document {
            blocks: vec![tab(vec![text("あい"), mark("2"), text("うえ")])],
            ..Default::default()
        };
        let s = build(&d);
        let body: Vec<(char, usize)> = s.lines[0].cells.iter()
            .filter(|c| !c.fmt.superscript)
            .map(|c| (c.ch, c.off))
            .collect();
        // あ=0 い=3 う=6 え=9(いずれも3バイト)。印は挟まっても動かさない
        assert_eq!(body, vec![('あ', 0), ('い', 3), ('う', 6), ('え', 9)],
            "印のせいで本文のバイト位置がずれた: {body:?}");
    }

    /// 番号は上付きで、本文より小さい
    #[test]
    fn the_number_is_superscript_and_small() {
        let d = Document {
            blocks: vec![tab(vec![text("あ"), mark("2")])],
            ..Default::default()
        };
        let s = build(&d);
        let c = s.lines[0].cells.iter().find(|c| c.fmt.superscript).expect("番号が無い");
        assert_eq!(c.ch, '1');
        assert!(c.size_pt < 10.5, "本文と同じ大きさ: {}", c.size_pt);
        assert!(c.fmt.footnote.is_some(), "番号が脚注の印を持っていない");
    }

    /// 表のセルの中の印も同じ流れで数える(番号が飛ばない)
    #[test]
    fn marks_inside_a_table_are_counted_in_the_same_run() {
        let cell = |runs: Vec<Run>| Cellbox {
            paragraphs: vec![Paragraph { runs, line_spacing: 1.0, ..Default::default() }],
            ..Default::default()
        };
        let d = Document {
            blocks: vec![
                tab(vec![text("前"), mark("2")]),
                Block::Table(Table {
                    col_mm: vec![80.0],
                    rows: vec![vec![cell(vec![text("表"), mark("3")])]],
        ..Default::default()
    }),
                tab(vec![text("後"), mark("4")]),
            ],
            ..Default::default()
        };
        let s = build(&d);
        let sup: String = s.lines.iter()
            .flat_map(|l| l.cells.iter())
            .filter(|c| c.fmt.superscript)
            .map(|c| c.ch)
            .collect();
        assert_eq!(sup, "123", "表を挟むと番号が飛ぶ: {sup:?}");
    }
}


#[cfg(test)]
mod endnote_tests {
    use crate::*;

    fn build(d: &Document) -> Sheet {
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        layout(d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 20.0 })
    }
    fn mark(id: &str, endnote: bool) -> Run {
        Run { text: String::new(), size_pt: Some(10.5), font: None,
              fmt: CharFormat { footnote: Some(FootnoteRef { id: id.into(), endnote }),
                                ..Default::default() } }
    }
    fn text(t: &str) -> Run {
        Run { text: t.into(), size_pt: Some(10.5), font: None, fmt: CharFormat::default() }
    }
    fn tab(runs: Vec<Run>) -> Block {
        Block::Para(Paragraph { runs, line_spacing: 1.0, ..Default::default() })
    }
    fn note(id: &str, endnote: bool, t: &str) -> Footnote {
        Footnote { added: false, id: id.into(), endnote,
                   paragraphs: vec![Paragraph { runs: vec![text(t)], line_spacing: 1.0,
                                                ..Default::default() }] }
    }

    /// **id は必ず衝突する。** docx は footnotes.xml と endnotes.xml を
    /// 別々に番号付けするので、両方を含む文書ではどちらも 1・2・3… になる。
    /// 実物(both-notes.docx)は脚注 2・3 と文末脚注 2・3 だった
    fn mixed_kinds() -> Document {
        Document {
            blocks: vec![tab(vec![text("あ"), mark("2", false), text("い"), mark("2", true)])],
            footnotes: vec![
                note("2", false, "脚注のほう"),
                note("2", true, "文末脚注のほう"),
            ],
            endnote_fmt: NoteNumFmt::LowerRoman,
            ..Default::default()
        }
    }

    #[test]
    fn the_same_id_does_not_confuse_footnote_and_endnote() {
        let s = build(&mixed_kinds());
        let below: Vec<String> = s.notes.iter()
            .map(|n| n.lines.iter().flat_map(|l| l.cells.iter()).map(|c| c.ch).collect())
            .collect();
        assert_eq!(below.len(), 1, "紙の下に文末脚注まで出た: {below:?}");
        assert!(below[0].contains("脚注のほう"), "紙の下が取り違えている: {below:?}");
        let full_text: String = s.lines.iter()
            .flat_map(|l| l.cells.iter()).map(|c| c.ch).collect();
        assert!(full_text.contains("文末脚注のほう"), "文末脚注が出ていない: {full_text:?}");
    }

    /// **番号は別々に数える。** 1本の連番にすると
    /// 脚注が「1・3」文末脚注が「2・4」と飛んで見える
    #[test]
    fn footnotes_and_endnotes_are_separate々counts_as() {
        let d = Document {
            blocks: vec![tab(vec![
                text("あ"), mark("2", false), mark("2", true),
                text("い"), mark("3", false), mark("3", true),
            ])],
            footnotes: vec![
                note("2", false, "脚1"), note("3", false, "脚2"),
                note("2", true, "文末1"), note("3", true, "文末2"),
            ],
            endnote_fmt: NoteNumFmt::LowerRoman,
            ..Default::default()
        };
        let s = build(&d);
        let mark_text: String = s.lines[0].cells.iter()
            .filter(|c| c.fmt.superscript).map(|c| c.ch).collect();
        // 脚注 1・2(算用数字)と 文末脚注 i・ii(ローマ数字)が交互に出る
        assert_eq!(mark_text, "1i2ii", "番号の振り方が違う: {mark_text:?}");
    }

    /// 文末脚注は**紙の下ではなく文書の末尾**。置き場が違う
    #[test]
    fn endnotes_flow_after_the_body() {
        let s = build(&mixed_kinds());
        assert!(s.notes.iter().all(|n| !n.lines.is_empty()), "紙の下が空");
        // 本文の最後の行より下に、文末脚注の行が来る
        let body_bottom = s.lines.iter()
            .filter(|l| l.cells.iter().any(|c| c.ch == 'あ' || c.ch == 'い'))
            .map(|l| l.y_mm).fold(0.0f32, f32::max);
        let sentence_end_head = s.lines.iter()
            .filter(|l| l.cells.iter().map(|c| c.ch).collect::<String>().contains("文末脚注のほう"))
            .map(|l| l.y_mm).fold(f32::MAX, f32::min);
        assert!(sentence_end_head > body_bottom,
            "文末脚注が本文より上に来た: 本文の底={body_bottom} 文末={sentence_end_head}");
    }

    #[test]
    fn renders_the_number_format_as_text() {
        assert_eq!(NoteNumFmt::Decimal.label(4), "4");
        assert_eq!(NoteNumFmt::LowerRoman.label(4), "iv");
        assert_eq!(NoteNumFmt::UpperRoman.label(9), "IX");
        assert_eq!(NoteNumFmt::LowerLetter.label(1), "a");
        assert_eq!(NoteNumFmt::UpperLetter.label(27), "AA");
        // 知らない書式は算用数字に落とす(知った顔をしない)
        assert_eq!(NoteNumFmt::from_docx("chicago"), NoteNumFmt::Decimal);
        assert_eq!(NoteNumFmt::from_docx("lowerRoman"), NoteNumFmt::LowerRoman);
    }
}


/// 選んだ字を脚注にする(`make_footnote`)。
///
/// 空の注を作って別の窓で打たせる形にはしていない — 注を打つ場所を
/// まだ持っていないので、**持っていない物を持っている顔をしない**。
#[cfg(test)]
mod make_footnote_tests {
    use crate::*;

    fn doc(text: &str) -> Document {
        Document::plain(text)
    }

    #[test]
    fn the_selected_text_moves_into_the_note_leaving_a_mark() {
        let mut d = doc("あいうえお");
        // 「いう」を脚注にする(あ=0..3、いう=3..9)
        let fr = d.make_footnote(3..9, false).expect("脚注にできなかった");
        assert_eq!(d.body_text(), "あえお", "字が本文から抜けていない");
        assert_eq!(d.footnotes.len(), 1, "注が作られていない");
        let t: String = d.footnotes[0].paragraphs.iter()
            .flat_map(|p| p.runs.iter().map(|r| r.text.as_str())).collect();
        assert_eq!(t, "いう", "注の中身が違う");
        assert!(d.footnotes[0].added, "足した注の印が立っていない");
        // 跡に**字を持たない印の run**が残る
        let p = d.paragraphs().next().unwrap();
        let mark = p.runs.iter().find(|r| r.fmt.footnote.is_some()).expect("印が無い");
        assert!(mark.text.is_empty(), "印の run が字を持っている");
        assert_eq!(mark.fmt.footnote.as_ref().unwrap(), &fr);
    }

    #[test]
    fn the_mark_stays_where_it_was_placed() {
        let mut d = doc("あいうえお");
        d.make_footnote(3..9, false).unwrap();
        let p = d.paragraphs().next().unwrap();
        // 「あ」の後ろ、「え」の前
        let mut seen = String::new();
        let mut at = None;
        for r in &p.runs {
            if r.fmt.footnote.is_some() {
                at = Some(seen.clone());
            }
            seen.push_str(&r.text);
        }
        assert_eq!(at.as_deref(), Some("あ"), "印の位置が違う: {at:?}");
    }

    /// **段落をまたぐ範囲は受けない。** どう畳むかに正解が無いので、
    /// 決められないことを黙って決めない
    #[test]
    fn a_range_spanning_paragraphs_is_refused() {
        let mut d = doc("あい\nうえ");
        let before = d.body_text();
        assert!(d.make_footnote(3..12, false).is_none(), "またぐ範囲を受けてしまった");
        assert_eq!(d.body_text(), before, "断ったのに本文が変わった");
        assert!(d.footnotes.is_empty(), "断ったのに注ができた");
    }

    #[test]
    fn an_empty_range_is_refused() {
        let mut d = doc("あいう");
        assert!(d.make_footnote(3..3, false).is_none(), "空の範囲を受けてしまった");
        assert!(d.footnotes.is_empty());
    }

    /// 二つ作っても id がぶつからない
    #[test]
    fn two_instances_do_not_collide_on_ids() {
        let mut d = doc("あいうえおかきくけこ");
        let a = d.make_footnote(0..3, false).unwrap();
        let b = d.make_footnote(6..9, false).unwrap();
        assert_ne!(a.id, b.id, "同じ id を二度使った");
        assert_eq!(d.footnotes.len(), 2);
    }

    /// 組むと、注が紙の下に出て番号が振られる
    #[test]
    fn the_created_note_appears_at_the_bottom_of_the_page() {
        let mut d = doc("あいうえお");
        d.make_footnote(3..9, false).unwrap();
        let (fam, _) = font::for_document(None).unwrap();
        let data = font::load(fam).unwrap();
        let m = Metrics::new(&data).unwrap();
        let s = layout(&d, &m, &Frame { measure_mm: 170.0, line_height_mm: 6.4, y0_mm: 20.0 });
        assert_eq!(s.notes.len(), 1, "紙の下に出ていない");
        let t: String = s.notes[0].lines.iter()
            .flat_map(|l| l.cells.iter()).map(|c| c.ch).collect();
        assert!(t.contains("いう"), "注の中身が出ていない: {t:?}");
        assert!(t.starts_with('1'), "番号が振られていない: {t:?}");
    }
}


/// 印刷モードの折り方(`fold_print`)。
///
/// **編集の画面は切れ目の無い巻物**で、頁の間隔は紙の高さより詰まっている
/// (実測で紙 297mm に対し間隔 260mm — 余白ぶん)。だから紙の絵を後ろに
/// 敷くだけでは重なる。中身を折り直して初めてページが見える。
#[cfg(test)]
mod fold_print_tests {
    use crate::*;

    fn paper(w: f32, h: f32) -> PageSetup {
        PageSetup { w_mm: w, h_mm: h, left_mm: 20.0, right_mm: 20.0,
                    top_mm: 20.0, bottom_mm: 20.0, columns: 1 }
    }
    fn line(y: f32) -> Line {
        Line { cells: vec![Cell { ch: 'あ', x_mm: 0.0, w_mm: 4.0, size_pt: 10.5,
                                  off: 0, fmt: Default::default(), font: None }],
               y_mm: y, from_body: true, byte0: 0, cell: None }
    }

    #[test]
    fn stacks_by_paper_height_per_page() {
        let mut s = Sheet { lines: vec![line(20.0), line(270.0), line(530.0)], ..Default::default() };
        // 巻物では 260mm 間隔でも、折れば紙の高さ(+隙間)で積まれる
        let tops = fold_print(&mut s, &[paper(210.0, 297.0); 3], &[0.0, 260.0, 522.0],
                               &[f32::NEG_INFINITY, 270.0, 530.0], 8.0);
        assert_eq!(tops, vec![0.0, 305.0, 610.0], "頁の上端が紙の高さで積まれていない");
        assert_eq!(s.lines[0].y_mm, 20.0, "1頁目の中の位置がずれた");
        assert_eq!(s.lines[1].y_mm, 305.0 + 10.0, "2頁目の中の位置がずれた");
        assert_eq!(s.lines[2].y_mm, 610.0 + 8.0, "3頁目の中の位置がずれた");
    }

    /// **頁ごとに紙が違ってよい。** 節で縦から横に変わる文書がこれ
    #[test]
    fn paper_height_may_differ_per_page() {
        let mut s = Sheet { lines: vec![line(20.0), line(270.0)], ..Default::default() };
        let tops = fold_print(&mut s, &[paper(210.0, 297.0), paper(297.0, 210.0)],
                              &[0.0, 260.0], &[f32::NEG_INFINITY, 270.0], 8.0);
        assert_eq!(tops, vec![0.0, 305.0], "縦の紙の高さで積んでいない");
        assert_eq!(s.lines[1].y_mm, 315.0);
    }

    /// 1頁だけの文書は中身を動かさない(折る必要が無い)
    #[test]
    fn a_single_page_is_left_alone() {
        let mut s = Sheet { lines: vec![line(20.0), line(100.0)], ..Default::default() };
        let tops = fold_print(&mut s, &[paper(210.0, 297.0)], &[0.0], &[f32::NEG_INFINITY], 8.0);
        assert_eq!(tops, vec![0.0]);
        assert_eq!(s.lines[0].y_mm, 20.0);
        assert_eq!(s.lines[1].y_mm, 100.0);
    }

    /// 脚注の**印のある行**も一緒に折る。折らないと、紙の下に出す位置が
    /// 巻物のままになって別の頁に出る
    #[test]
    fn the_footnote_mark_is_laid_out_with_the_text() {
        let mut s = Sheet {
            lines: vec![line(20.0), line(270.0)],
            notes: vec![NoteBlock { no: 1, at_y: 270.0, lines: vec![], h_mm: 5.0 }],
            ..Default::default()
        };
        fold_print(&mut s, &[paper(210.0, 297.0); 2], &[0.0, 260.0],
                   &[f32::NEG_INFINITY, 270.0], 8.0);
        assert_eq!(s.notes[0].at_y, 315.0, "脚注の目印が巻物のまま");
    }

    /// **頁は「その頁の最初の行」で分ける** — 紙の上端(offsets)は最初の行より
    /// 余白ぶん上にあるので、境に使うと**前の頁の末尾が次の頁へ化ける**。
    /// 巻物は空きを詰めて流れるので、上端は前の頁の終わりより手前に来る
    /// (2026-08-17、発表の組み方で踏んだ)
    #[test]
    fn pages_split_at_the_first_line_not_the_paper_top() {
        // 1頁目は 20 と 270、2頁目は 280 から。2頁目の紙の上端は 260 で、
        // 270 の行より**上**にある
        let mut s = Sheet { lines: vec![line(20.0), line(270.0), line(280.0)], ..Default::default() };
        let tops = fold_print(&mut s, &[paper(210.0, 297.0); 2], &[0.0, 260.0],
                              &[f32::NEG_INFINITY, 280.0], 8.0);
        assert_eq!(s.lines[1].y_mm, 270.0, "1頁目の末尾が次の頁へ化けた");
        assert_eq!(s.lines[2].y_mm, tops[1] + 20.0, "2頁目の頭がずれた");
    }

    /// 折ったら**頁の切れ目**もその位置に置き直す(紙に写す側が見る)
    #[test]
    fn moves_the_break() {
        let mut s = Sheet { lines: vec![line(20.0), line(270.0)], ..Default::default() };
        fold_print(&mut s, &[paper(210.0, 297.0); 2], &[0.0, 260.0],
                   &[f32::NEG_INFINITY, 270.0], 8.0);
        assert_eq!(s.breaks, vec![305.0], "切れ目が折った後の位置になっていない");
    }
}

#[cfg(test)]
mod fill_tests {
    use crate::{adoc, fill};
    use std::collections::BTreeMap;

    fn line(items: &[(&str, &str)]) -> BTreeMap<String, String> {
        items.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    const TMPL_OF: &str = "= 請求書\n\n\
        {{宛名}} 御中\n\n\
        |===\n\
        | 品名 | 数量\n\
        | {{明細.品名}} | {{明細.数量}}\n\
        |===\n\n\
        合計 {{合計}} 円\n";

    /// **空のデータで通すと、穴の名前が全部出て、本文は変わらない。**
    ///
    /// MCP の `doc_merge_fields`(2026-08-21 の C-2)がこの性質に頼って
    /// います。穴の名前を並べる関数は無いので、空のデータで1度通して
    /// `unknown` を読み、返ってきた文書は捨てます。
    ///
    /// *埋まらない穴をそのまま残す*作りをやめると、道具が黙って壊れます。
    /// 表の群の行はデータが無いと消えるので、**返り値は使いません**が、
    /// 元の文書が変わらないことも合わせて見ます。
    #[test]
    fn empty_data_reveals_every_field_name() {
        let d = adoc::parse(TMPL_OF).expect("雛形が読めない");
        let from = d.paragraphs().count();
        let (_, rep) = fill::fill(&d, &fill::Data::new());
        // **表の群は、列ごとではなく群の名前で出ます**(「明細.品名」では
        // なく「明細」)。行を増やす仕掛けなので、埋めるときも群の単位です
        for name in ["宛名", "合計", "明細"] {
            assert!(rep.unknown.iter().any(|x| x == name), "{name} が出ていない: {:?}", rep.unknown);
        }
        // 同じ名前は1度だけ(道具はこれをそのまま一覧に出します)
        let mut order = rep.unknown.clone();
        order.sort();
        let src_count = order.len();
        order.dedup();
        assert_eq!(order.len(), src_count, "同じ名前が2度出ている: {:?}", rep.unknown);
        // 元の文書は触られていない
        assert_eq!(d.paragraphs().count(), from, "元の文書が変わっている");
        let text: String = d
            .paragraphs()
            .flat_map(|p| p.runs.iter())
            .map(|r| r.text.as_str())
            .collect();
        assert!(text.contains("{{宛名}}"), "穴が消えている: {text}");
    }

    /// **明細の行がデータの数だけ増える。** ここが帳票の芯です。
    #[test]
    fn detail_rows_grow() {
        let d = adoc::parse(TMPL_OF).expect("雛形が読めない");
        let mut data = fill::Data::new();
        data.set("宛名", "みほん商事").set("合計", "3,000");
        data.push_row("明細", line(&[("品名", "鉛筆"), ("数量", "10")]));
        data.push_row("明細", line(&[("品名", "消しゴム"), ("数量", "5")]));
        data.push_row("明細", line(&[("品名", "定規"), ("数量", "2")]));

        let (out, rep) = fill::fill(&d, &data);
        let t = out.tables().next().expect("表が無い");
        // 見出しの1行 + 明細3行
        assert_eq!(t.rows.len(), 4, "行が増えていない: {}", t.rows.len());
        let text = |r: usize, c: usize| -> String {
            t.rows[r][c].paragraphs.iter()
                .flat_map(|p| p.runs.iter()).map(|x| x.text.as_str()).collect()
        };
        assert_eq!(text(1, 0), "鉛筆");
        assert_eq!(text(3, 1), "2");
        assert_eq!(rep.expanded.get("明細"), Some(&3));
        assert!(rep.unknown.is_empty(), "分からない名前があった: {:?}", rep.unknown);

        // 表の外も差し込まれる
        let body: String = out.paragraphs().flat_map(|p| p.runs.iter())
            .map(|r| r.text.as_str()).collect();
        assert!(body.contains("みほん商事 御中"), "宛名が入っていない: {body}");
        assert!(body.contains("合計 3,000 円"), "合計が入っていない: {body}");
    }

    /// **分からない名前を黙って空にしない。** 空にすると「金額が空欄の
    /// 請求書」が黙って出来上がります。
    #[test]
    fn unknown_names_are_kept_and_reported() {
        let d = adoc::parse(TMPL_OF).expect("雛形が読めない");
        let mut data = fill::Data::new();
        data.set("宛名", "みほん商事"); // 合計を入れ忘れた
        data.push_row("明細", line(&[("品名", "鉛筆"), ("数量", "10")]));

        let (out, rep) = fill::fill(&d, &data);
        let body: String = out.paragraphs().flat_map(|p| p.runs.iter())
            .map(|r| r.text.as_str()).collect();
        assert!(body.contains("{{合計}}"), "空にしてしまった: {body}");
        assert_eq!(rep.unknown, vec!["合計".to_string()]);
        assert!(rep.summary().contains("合計"), "報告に出ていない: {}", rep.summary());
    }

    /// データが1行も無いときは、明細の行が消えます(見出しは残る)。
    #[test]
    fn an_empty_detail_list_produces_no_rows() {
        let d = adoc::parse(TMPL_OF).expect("雛形が読めない");
        let mut data = fill::Data::new();
        data.set("宛名", "-").set("合計", "0");
        data.rows.insert("明細".into(), vec![]);
        let (out, rep) = fill::fill(&d, &data);
        assert_eq!(out.tables().next().unwrap().rows.len(), 1, "見出しだけ残るはず");
        assert_eq!(rep.expanded.get("明細"), Some(&0));
    }

    /// CSV 1枚で、1つだけの値と明細の両方をまかないます。
    #[test]
    fn reads_from_csv() {
        let src = "宛名,品名,数量\nみほん商事,鉛筆,10\nみほん商事,消しゴム,5\n";
        let d = fill::from_csv(src, "明細");
        assert_eq!(d.values.get("宛名"), Some(&"みほん商事".to_string()));
        assert_eq!(d.rows["明細"].len(), 2);
        assert_eq!(d.rows["明細"][1]["品名"], "消しゴム");
    }

    /// 囲みの中の改行とカンマを読み違えないこと。
    #[test]
    fn reads_csv_quoting() {
        let src = "品名,備考\n\"鉛筆, HB\",\"1行目\n2行目\"\n";
        let d = fill::from_csv(src, "明細");
        assert_eq!(d.rows["明細"][0]["品名"], "鉛筆, HB");
        assert_eq!(d.rows["明細"][0]["備考"], "1行目\n2行目");
    }

    /// 差し込む所を見つけられること(画面から使うときの判断に要ります)。
    #[test]
    fn counts_the_merge_fields() {
        let d = adoc::parse(TMPL_OF).expect("雛形が読めない");
        assert_eq!(fill::groups(&d), vec!["明細".to_string()]);
        let plain = adoc::parse("= 題\n\nただの本文。\n").unwrap();
        assert!(fill::groups(&plain).is_empty(), "無い所を有ると言った");
    }

    /// **雛形は何度でも使える**(原本を書き換えない)。
    #[test]
    fn the_template_is_not_modified() {
        let d = adoc::parse(TMPL_OF).expect("雛形が読めない");
        let mut data = fill::Data::new();
        data.set("宛名", "一回目").set("合計", "1");
        data.push_row("明細", line(&[("品名", "あ"), ("数量", "1")]));
        let before = adoc::write(&d);
        let _ = fill::fill(&d, &data);
        // 表の書き方は読むときに空白を許し、書くときは詰めるので、
        // 元の字ではなく**書き出した字どうし**で比べます
        assert_eq!(adoc::write(&d), before, "雛形が書き換わった");
        // 2回目も同じ結果になること
        let (a, _) = fill::fill(&d, &data);
        let (b, _) = fill::fill(&d, &data);
        assert_eq!(adoc::write(&a), adoc::write(&b), "2回目が違う");
    }
}

#[cfg(test)]
mod indent_tests {
    use super::*;
    use crate::theme;

    /// **1行目の字下げが紙面に出る。** 日本語の本文は1字下げるのが普通です。
    /// テンプレートに `字下げ = 1` と書くと、その段落の1行目だけが下がります
    /// (2026-08-18。それまで模型は値を持つだけで、紙面では使っていませんでした)。
    #[test]
    fn indents_only_the_first_line() {
        let data = test_font();
        let m = Metrics::new(&data).unwrap();
        let doc = crate::adoc::parse(
            "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめも
",
        )
        .unwrap();
        let th = theme::parse("[文書]\n大きさ = 10.5\n\n[スタイル.本文]\n字下げ = 1\n").unwrap();
        let c = theme::compose(&doc, &th);
        // 10.5pt の1字 = 210 twips
        let p0 = c.paragraphs().next().unwrap();
        assert_eq!(p0.first_line_twips, 210, "合成で字下げが乗らない");

        let s = layout(&c, &m, &Frame { measure_mm: 60.0, line_height_mm: 6.0, y0_mm: 20.0 });
        assert!(s.lines.len() >= 2, "2行以上に折れていない");
        // 1行目は下がり、2行目は下がらない
        let head = |i: usize| s.lines[i].cells[0].x_mm;
        let text = 10.5 * 25.4 / 72.0;
        assert!((head(0) - head(1) - text).abs() < 0.2,
                "1行目だけが1字ぶん下がっていない: {} と {}", head(0), head(1));
    }

    /// **行の後ろの覚え書きを落とす。** TOML の普通の書き方で、これが読めないと
    /// 手引きに載せた見本がそのまま落ちます(2026-08-18 に踏みました)。
    #[test]
    fn drops_the_trailing_note_on_a_line() {
        let th = theme::parse(
            "[組み方]\n横幅 = \"可変\"     # 窓の幅で組む\n\n\
             [スタイル.本文]\n字下げ = 1   # 1字下げ\n色 = \"C0392B\"\n",
        )
        .expect("覚え書きつきが読めない");
        assert!(th.setting.fluid, "横幅の指定が読めていない");
        assert_eq!(th.style("本文").unwrap().first_line_chars, Some(1.0));
        // 囲みの中の # は字として残る
        let th2 = theme::parse("[スタイル.本文]\n色 = \"#C0392B\"\n").unwrap();
        assert_eq!(th2.style("本文").unwrap().color.as_deref(), Some("#C0392B"));
    }

    /// **字下げは Web にも出る。** テンプレートの鍵は、通る道が全部揃って
    /// いないと「効かない」だけが残ります(2026-08-18、条件を当てて見つけた —
    /// 字下げは紙には出るのに CSS に出ていませんでした)。
    #[test]
    fn the_indent_reaches_the_css() {
        let th = theme::parse("[スタイル.本文]\n字下げ = 1\n").unwrap();
        let css = crate::html_write::css(&th, false);
        assert!(css.contains("text-indent:1em"), "CSS に字下げが出ていない:\n{css}");
    }

    /// 書いた物を読み直すと同じになる(テンプレートの往復)。
    #[test]
    fn the_indent_round_trips() {
        let th = theme::parse("[スタイル.本文]\n字下げ = 1\n").unwrap();
        let back = theme::parse(&theme::write(&th)).unwrap();
        assert_eq!(back.style("本文").unwrap().first_line_chars, Some(1.0));
    }
}

#[cfg(test)]
mod title_tests {
    use crate::{adoc, html_write, theme};

    /// **表題が紙面に出る。** 2026-08-18 まで文書の情報にしか入らず、
    /// 開くと題名が消えて見えていました。
    #[test]
    fn the_title_becomes_a_body_paragraph() {
        let src = "= 月次報告\n:template: 型\n\n== まとめ\n\n本文です。\n";
        let d = adoc::parse(src).expect("読めない");
        assert_eq!(adoc::write(&d), src, "往復していない");
        // 文書の情報にも入る(docx の往復に要る)
        assert_eq!(d.props.title, "月次報告");
        // 本文の先頭の段落にもなる
        let ps: Vec<_> = d.paragraphs().collect();
        assert_eq!(ps[0].style, crate::doc::ParaStyle::Title);
        assert_eq!(ps[0].runs[0].text, "月次報告");
        // テンプレートの「表題」が当たる(既定は 20pt の太字)
        let c = theme::compose(&d, &theme::default_theme());
        let t = c.paragraphs().next().unwrap();
        assert_eq!(t.runs[0].size_pt, Some(20.0), "表題に書式が当たらない");
        assert!(t.runs[0].fmt.bold);
        // HTML では h1.title(2回出ない)
        let h = html_write::body(&d);
        assert_eq!(h.matches("<h1").count(), 1, "h1 が2つある:\n{h}");
        assert!(h.contains("<h1 class=\"title\">月次報告</h1>"), "{h}");
    }

    /// docx から来た文書(表題の段落が無く、文書の情報にだけ題名がある)でも
    /// HTML には題名が出る。
    #[test]
    fn a_title_that_only_exists_in_the_metadata_still_shows() {
        let mut d = crate::doc::Document::plain("本文だけ。");
        d.props.title = "受け取った文書".into();
        let h = html_write::body(&d);
        assert!(h.contains("<h1 class=\"title\">受け取った文書</h1>"), "{h}");
    }
}

#[cfg(test)]
mod fill_honke_tests {
    use crate::{adoc, fill};

    /// **差し込みは本家の `{member}` で書けます**(2026-08-18)。
    /// AsciiDoc は属性の参照を `{member}` と書くので、そちらに寄せました。
    /// 前からの `{{member}}` も受け続けます(手引きと見本がその形で出ています)。
    #[test]
    fn mail_merge_accepts_the_vendor_notation() {
        for src in ["請求先: {宛名} 様\n", "請求先: {{宛名}} 様\n"] {
            let d = adoc::parse(src).expect("読めない");
            let mut data = fill::Data::new();
            data.set("宛名", "山田太郎");
            let (out, rep) = fill::fill(&d, &data);
            let text: String = out
                .paragraphs()
                .flat_map(|p| p.runs.iter())
                .map(|r| r.text.as_str())
                .collect();
            assert_eq!(text, "請求先: 山田太郎 様", "差し込めていない: {src:?}");
            assert!(rep.unknown.is_empty(), "分からない名前が出た: {rep:?}");
        }
    }

    /// **普通の文の中括弧は差し込みの穴にしません。** 名前に空白が入る物は
    /// 穴ではない、と見ます(`{ x + y }` のような字を巻き込まないため)。
    #[test]
    fn braces_containing_a_space_are_not_fields() {
        let d = adoc::parse("式は { x + y } です。\n").expect("読めない");
        let (out, rep) = fill::fill(&d, &fill::Data::new());
        let text: String = out
            .paragraphs()
            .flat_map(|p| p.runs.iter())
            .map(|r| r.text.as_str())
            .collect();
        assert_eq!(text, "式は { x + y } です。");
        assert!(rep.unknown.is_empty(), "普通の文を穴と見た: {rep:?}");
    }
}

#[cfg(test)]
mod adoc_honke_tests {
    use crate::adoc;

    /// **表のセルは次の行に続く**(本家の作法)。2026-08-18 まで断っていたので、
    /// 本家の手引き 176 枚のうち 11 枚が開けませんでした。
    #[test]
    fn a_table_cell_continues_on_the_next_line() {
        // 1行目のセルの数が桁の数(ここでは2桁)。以降は流れで切られる
        let src = "|===\n|あ |い\n|一つ目のセル\nその続きの行\n|二つ目\n|===\n";
        let d = adoc::parse(src).expect("読めない");
        let t = d.tables().next().expect("表が無い");
        let text = |r: usize, c: usize| -> String {
            t.rows[r][c].paragraphs.iter().flat_map(|p| p.runs.iter())
                .map(|x| x.text.as_str()).collect()
        };
        assert_eq!(t.rows.len(), 2, "2行にならない: {:?}", t.rows.len());
        assert_eq!(text(1, 0), "一つ目のセルその続きの行", "続きの行が繋がっていない");
        assert_eq!(text(1, 1), "二つ目", "1行に1セルずつ書いた分が流れていない");
    }

    /// **セルの指定を読み飛ばす**(`h|` 見出し・`^|` 中央・`a|` など)。
    /// 効かせるのは結合だけで、残りは指定として捨てます。
    #[test]
    fn skips_cell_specs() {
        let d = adoc::parse("|===\nh|見出し ^|中央 a|中身\n|===\n").expect("読めない");
        let t = d.tables().next().expect("表が無い");
        assert_eq!(t.rows[0].len(), 3, "セルが3つに割れていない");
        let text: String = t.rows[0][0].paragraphs.iter().flat_map(|p| p.runs.iter())
            .map(|x| x.text.as_str()).collect();
        assert_eq!(text, "見出し", "指定が字に混ざった");
    }

    /// **文書の頭は空行までが頭。** 著者の行で打ち切ると、その後ろの属性が
    /// 本文に落ちて、書き戻しで消えます(2026-08-18 に本家の README で発覚)。
    #[test]
    fn the_head_runs_to_the_blank_line() {
        let src = "= 題\n著者 <mail>\n// 覚え書き\n:idprefix:\n:idseparator: -\n\n本文。\n";
        let d = adoc::parse(src).expect("読めない");
        assert_eq!(adoc::write(&d), src, "頭が往復していない");
        assert_eq!(d.paragraphs().count(), 2, "頭が本文に落ちた");
    }

    /// **行を継ぐときの空白。** 日本語は入れず、英語は入れます。
    #[test]
    fn the_joint_space_is_dropped_only_for_japanese() {
        let d = adoc::parse("plain CSS.\nThe build minifies it.\n").expect("読めない");
        let text: String = d.paragraphs().flat_map(|p| p.runs.iter())
            .map(|r| r.text.as_str()).collect();
        assert_eq!(text, "plain CSS. The build minifies it.", "英語の語がくっついた");
        let d2 = adoc::parse("日本語の文を\n行で折った。\n").expect("読めない");
        let text2: String = d2.paragraphs().flat_map(|p| p.runs.iter())
            .map(|r| r.text.as_str()).collect();
        assert_eq!(text2, "日本語の文を行で折った。", "日本語に空白が入った");
    }
}

#[cfg(test)]
mod adoc_notes_tests {
    use crate::adoc;

    /// **表に取り込んだ `[cols=]` は帳簿に出さない**(2026-08-19)。
    ///
    /// 桁の割合は表の物として読んでいるので、「読み飛ばした」と言うと嘘に
    /// なります。表の題(`.題`)と同じ作法です。実機で calc に `.adoc` を
    /// 開かせたとき、下の帳簿に「塊の指定」が出ていて気づきました。
    #[test]
    fn column_specs_absorbed_into_the_table_are_not_logged() {
        let (d, ledger) = adoc::parse_full(
            ".売上\n[cols=\"1,1\"]\n|===\n|月 |額\n\n|4月 |100\n|===\n",
        )
        .expect("読めない");
        assert!(!ledger.iter().any(|x| x.contains("塊の指定")), "取り込んだのに帳簿に出た: {ledger:?}");
        let t = d.tables().next().expect("表が無い");
        assert_eq!(t.col_ratio, vec![1.0, 1.0], "桁の割合を取り込んでいない");

        // 表と関係のない `[…]` は、いままでどおり帳簿に出る
        let (_, ledger2) = adoc::parse_full("[source,python]\n----\nx = 1\n----\n").expect("読めない");
        assert!(ledger2.iter().any(|x| x.contains("塊の指定")), "{ledger2:?}");
    }

    /// **本家にあってうちに無い書き方は、帳簿に出す。**
    ///
    /// 字は本文として残りますが、意味は落ちています。2026-08-18 まで8つ試して
    /// 8つとも黙って本文に化けていました(手引きには「読めないと言う」と
    /// 書いてあったので、文書のほうが嘘でした)。
    #[test]
    fn unsupported_notation_is_logged() {
        for (what, src) in [
            ("コードの塊", "----\nlet x = 1;\n----\n"),
            ("塊の題", ".表の題\n\nふつうの段落。\n"),
            ("取り込み", "include::別の.adoc[]\n"),
            ("属性の参照", "宛名は {宛名} です。\n"),
            // **見出し4・5 は 2026-08-18 に読めるようになりました。**
            // ここには置きません(帳簿に出ないのが正しい)
            ("塊の指定", "[source,python]\n----\nprint(1)\n----\n"),
            ("チェックの箇条書き", "* [x] 済み\n"),
        ] {
            let (_, notes) = adoc::parse_full(src).expect("読めない");
            assert!(!notes.is_empty(), "{what}を黙って本文に化けさせた");
        }
    }

    /// **うちの書き方では帳簿に何も出ない。** 毎回出ると、本当に落ちたときに
    /// 気づけなくなります。
    #[test]
    fn our_own_notation_leaves_the_log_empty() {
        let src = "= 題\n:template: 型\n\n== 見出し\n\n本文と*強調*。\
                   ruby:漢字[かんじ]。footnote:[注]\n\n* あ\n* い\n\n\
                   . 一\n. 二\n\n____\n引用。\n____\n\n<<<\n\n\
                   |===\n2+|見出し\n|あ |い\n|===\n\nstem:[x^2]\n\n\
                   image::images/図1.png[]\n\nfield:名前[お名前]\n\n\
                   宛名は {{宛名}} です。\n";
        let (_, notes) = adoc::parse_full(src).expect("読めない");
        assert!(notes.is_empty(), "うちの書き方で帳簿が出た: {notes:?}");
    }
}

#[cfg(test)]
mod adoc_dropped_tests {
    use crate::adoc;
    use crate::doc::{CharFormat, Document, Paragraph, Run};

    /// **adoc で保存すると消える物を、消える前に数える。**
    ///
    /// 消すこと自体は決めたとおりですが、黙って消すと、人は開き直したときに
    /// 初めて気づきます(2026-08-17)。
    #[test]
    fn counts_and_reports_what_is_lost() {
        let mut d = Document::default();
        let mut p = Paragraph::default();
        p.runs.push(Run {
            text: "下線つき".into(),
            size_pt: Some(16.0),
            font: None,
            fmt: CharFormat { underline: true, ..Default::default() },
        });
        p.align = crate::doc::Align::Center;
        d.push_para(p);
        d.watermark = Some("社外秘".into());
        d.header.paragraphs.push(Paragraph::default());
        d.header.paragraphs[0].runs.push(Run {
            text: "ヘッダーの字".into(),
            size_pt: None,
            font: None,
            fmt: CharFormat::default(),
        });

        let got = adoc::dropped(&d);
        for what in ["下線", "字の大きさ", "段落の揃え", "透かし", "ヘッダー"] {
            assert!(got.contains(&what), "「{what}」が挙がっていない: {got:?}");
        }
    }

    /// 意味だけの文書では**何も消えません。** ここが空でないと、毎回
    /// 「消えました」と出て、本当に消える時に気づけなくなります。
    #[test]
    fn a_semantics_only_document_loses_nothing() {
        let src = "= 題\n\n== 章\n\n本文と*強調*。\n\n* あ\n* い\n";
        let d = adoc::parse(src).expect("読めない");
        assert!(adoc::dropped(&d).is_empty(), "何も無いのに挙がった: {:?}", adoc::dropped(&d));
    }
}

#[cfg(test)]
mod html_write_tests {
    use crate::{adoc, html_write, theme};

    fn doc(src: &str) -> crate::doc::Document {
        adoc::parse(src).expect("adoc が読めない")
    }

    /// **本文は意味だけ。** 見た目は CSS の側に出て、HTML には入りません。
    #[test]
    fn the_look_goes_to_css_the_text_to_html() {
        let d = doc("= 題\n\n== 章の名前\n\n本文です。*ここ*が大事。\n");
        let th = theme::parse("[スタイル.見出し1]\n大きさ = 20\n太字 = true\n").unwrap();
        let p = html_write::page(&d, &th);

        // 意味はタグで出る
        assert!(p.html.contains("<h2>章の名前</h2>"), "題名があるので h2 のはず:\n{}", p.html);
        assert_eq!(p.html.matches("<h1").count(), 1, "h1 が2つ以上ある:\n{}", p.html);
        assert!(p.html.contains("<strong>ここ</strong>"), "強調が strong になっていない");
        // 見た目は HTML に入らない
        assert!(!p.html.contains("font-size:20pt\""), "見た目が本文に埋まった");
        // 見た目は CSS に出る
        assert!(p.css.contains("h2 {"), "見出し1 の規則が h2 に当たっていない:\n{}", p.css);
        assert!(p.css.contains("font-size:20pt"), "大きさが CSS に出ていない");
    }

    /// **テンプレートを替えると CSS だけが変わる。** これが「同じ本文で
    /// Web にも帳票にもなる」の根拠です。
    #[test]
    fn changing_the_template_leaves_the_text() {
        let d = doc("= 題\n\n本文です。\n");
        let web = theme::parse("[組み方]\n横幅 = \"可変\"\n区切り = \"なし\"\n").unwrap();
        let paper = theme::parse("[スタイル.本文]\n大きさ = 10.5\n").unwrap();
        let a = html_write::page(&d, &web);
        let b = html_write::page(&d, &paper);
        assert_eq!(html_write::body(&d), html_write::body(&d), "本文が安定しない");
        assert_ne!(a.css, b.css, "テンプレートを替えても CSS が同じ");
        assert!(a.css.contains("max-width"), "横幅可変が効いていない:\n{}", a.css);
    }

    /// 箇条書きは `ul` / `ol` にまとめます(HTML の入れ物の作法)。
    #[test]
    fn bullets_are_wrapped_together() {
        let d = doc("= 題\n\n* あ\n* い\n* う\n");
        let h = html_write::body(&d);
        assert_eq!(h.matches("<ul>").count(), 1, "ul が1つでない:\n{h}");
        assert_eq!(h.matches("<li>").count(), 3, "項目が3つでない:\n{h}");
    }

    /// 出来た HTML が**形として正しい**か(閉じ忘れ・入れ子の乱れが無いか)。
    #[test]
    fn the_produced_html_is_well_formed() {
        let d = doc("= 題\n\n== 章\n\n本文。*強調*と_斜体_。\n\n* あ\n* い\n\n____\n引用です。\n____\n");
        let th = theme::parse("[スタイル.見出し1]\n大きさ = 18\n").unwrap();
        let p = html_write::page(&d, &th);
        // 開いた数と閉じた数が合うこと
        for tag in ["h1", "h2", "p", "ul", "li", "strong", "em", "blockquote", "style", "body"] {
            let open = p.html.matches(&format!("<{tag}")).count();
            let close = p.html.matches(&format!("</{tag}>")).count();
            assert_eq!(open, close, "{tag} の開閉が合わない({open} 対 {close}):\n{}", p.html);
        }
        assert!(p.html.starts_with("<!DOCTYPE html>"), "宣言が無い");
    }

    /// **記入欄が adoc で往復する。** 意味だけの本文に書けなければ、
    /// アプリの形にできません(2026-08-17)。
    #[test]
    fn form_fields_round_trip_through_adoc() {
        use crate::{adoc, doc::SdtKind as K};
        let src = "= 申し込み\n\n\
                   field:name[お名前]\n\n\
                   field:addr[ご住所,複数行]\n\n\
                   field:kind[参加区分,選ぶ:一般|学生]\n\n\
                   field:day[希望日,日付]\n";
        let d = adoc::parse(src).expect("読めない");
        let got: Vec<_> = html_write::fields(&d)
            .into_iter()
            .map(|s| (s.tag, s.alias, s.kind, s.items))
            .collect();
        assert_eq!(
            got,
            vec![
                ("name".into(), "お名前".into(), K::Text, vec![]),
                ("addr".into(), "ご住所".into(), K::Complex, vec![]),
                ("kind".into(), "参加区分".into(), K::Dropdown,
                 vec!["一般".to_string(), "学生".to_string()]),
                ("day".into(), "希望日".into(), K::Date, vec![]),
            ]
        );
        // 書き戻すと元の字に戻る
        assert_eq!(adoc::write(&d), src, "往復していない");
    }

    /// **ラベル付きリストの値に入れた記入欄が消えない**(2026-08-18)。
    /// 申込用紙は「氏名:: field:氏名[お名前]」の形で書くので、ここが
    /// 落ちると用紙が空になる
    #[test]
    fn form_fields_in_labeled_list_values_survive() {
        let (doc, _) = crate::adoc::parse_full(
            "= 申込\n\n氏名:: field:氏名[お名前]\n人数:: field:人数[人数]\n",
        )
        .expect("読めない");
        let html = crate::html_write::body(&doc);
        assert!(html.contains("<dl>"), "ラベル付きリストになっていない: {html}");
        assert!(html.contains("name=\"氏名\""), "記入欄が消えた: {html}");
        assert!(html.contains("name=\"人数\""), "記入欄が消えた: {html}");
        assert_eq!(crate::html_write::fields(&doc).len(), 2);
    }

    /// **記入欄は form になる**(アプリビルダーの土台)。
    #[test]
    fn form_fields_become_a_form() {
        use crate::doc::{CharFormat, Document, Paragraph, Run, Sdt, SdtKind};
        let mut d = Document::default();
        let mut p = Paragraph::default();
        for (alias, tag, kind, items) in [
            ("お名前", "name", SdtKind::Text, vec![]),
            ("ご住所", "addr", SdtKind::Complex, vec![]),
            ("参加区分", "kind", SdtKind::Dropdown, vec!["一般".to_string(), "学生".to_string()]),
            ("希望日", "date", SdtKind::Date, vec![]),
        ] {
            p.runs.push(Run {
                text: String::new(),
                size_pt: None,
                font: None,
                fmt: CharFormat {
                    sdt: Some(Box::new(Sdt {
                        kind,
                        alias: alias.into(),
                        tag: tag.into(),
                        items,
                    })),
                    ..Default::default()
                },
            });
        }
        d.push_para(p);

        // 送り先があれば form で包む
        let th = theme::parse(
            "[送り先]\n宛先 = \"https://例.jp/受付\"\n送り方 = \"post\"\nボタン = \"申し込む\"\n",
        )
        .unwrap();
        let h = html_write::page(&d, &th).html;
        assert!(h.contains("<form action=\"https://例.jp/受付\" method=\"post\">"), "form が無い:\n{h}");
        assert!(h.contains("name=\"name\""), "名前の欄が無い");
        assert!(h.contains("<textarea"), "複数行の欄が無い");
        assert!(h.contains("<select"), "選ぶ欄が無い");
        assert!(h.contains("type=\"date\""), "日付の欄が無い");
        assert!(h.contains("申し込む</button>"), "送るボタンが無い");

        // **送り先が無ければ包まない。** 押しても何も起きない form は出さない
        let plain = theme::parse("[スタイル.本文]\n大きさ = 11\n").unwrap();
        let h2 = html_write::page(&d, &plain).html;
        assert!(!h2.contains("<form"), "送り先が無いのに form を出した:\n{h2}");
        assert!(h2.contains("name=\"name\""), "欄そのものは出るはず");
    }

    /// **adoc に書けるものは HTML にも出る。**
    ///
    /// 発注者 2026-08-17「AsciiDoc の文法とリボンのボタンと HTML/CSS を
    /// 合わせていくものでしょう」。片方にしか無い書き方があると、書いた人は
    /// 書き出して初めて消えたことに気づきます。ここが見張りです。
    ///
    /// 意味の単位を1つ足したら、この表にも1行足してください。
    #[test]
    fn what_adoc_can_write_also_reaches_html() {
        let src = "= 題\n\n\
                   [[しるし]]\n== 章\n\n\
                   本文と<<しるし>>への参照。\nfootnote:[注の文章]\n\n\
                   <<<\n\n\
                   改ページの後。\n\n\
                   image::images/図1.png[]\n\n\
                   stem:[x^2]\n\n\
                   ruby:漢字[かんじ]と https://例.jp[リンク]。\n\n\
                   x^2^ と H~2~O。\n\n\
                   |===\n\
                   2+|左右で1つ\n\
                   .2+|上下で1つ |ふつう\n\
                   |下の行\n\
                   |===\n";
        let d = doc(src);
        assert_eq!(adoc::write(&d), src, "adoc の往復が崩れた");
        let h = html_write::body(&d);

        for (what, mark) in [
            ("しおり", "id=\"しるし\""),
            ("相互参照", "href=\"#しるし\""),
            ("脚注の印", "href=\"#fn1\""),
            ("脚注の文章", "注の文章"),
            ("改ページ", "class=\"pagebreak\""),
            ("画像", "<img src=\"images/図1.png\""),
            ("数式", "data-tex=\"x^2\""),
            ("ルビ", "<ruby>漢字<rt>かんじ</rt></ruby>"),
            ("リンク", "href=\"https://例.jp\""),
            ("上付き", "<sup>2</sup>"),
            ("下付き", "<sub>2</sub>"),
            ("横の結合", "colspan=\"2\""),
            ("縦の結合", "rowspan=\"2\""),
        ] {
            assert!(h.contains(mark), "{what}が HTML に出ていない({mark}):\n{h}");
        }
        // 縦結合の続きのセルは書かない(rowspan が占めるので、書くと桁が増える)
        assert_eq!(h.matches("<td").count(), 4, "セルの数が合わない:\n{h}");
    }

    /// 画像の中身は**HTML に埋め込まず、別のファイルとして返します。**
    #[test]
    fn images_come_back_as_separate_files() {
        use crate::doc::{Document, InlineImage, Paragraph};
        let mut d = Document::default();
        let mut p = Paragraph::default();
        p.images_new.push(InlineImage {
            bytes: std::sync::Arc::new(vec![0xFF, 0xD8, 1, 2, 3]), // jpeg の頭
            w_mm: 40.0,
            h_mm: 30.0,
            tex: None,
            src: None,
        });
        d.push_para(p);
        let page = html_write::page(&d, &theme::default_theme());
        assert_eq!(page.assets.len(), 1, "画像が返ってこない");
        assert_eq!(page.assets[0].0, "images/図1.jpg", "拡張子を中身から見ていない");
        assert!(page.html.contains("src=\"images/図1.jpg\""), "本文が参照していない:\n{}", page.html);
        assert!(!page.html.contains("base64"), "画像を埋め込んだ");
    }

    /// **目次は nav にまとめます**(2026-08-18)。前は普通の段落として並び、
    /// Web では本文と見分けが付きませんでした。
    #[test]
    fn the_table_of_contents_is_wrapped_in_nav() {
        use crate::doc::{Document, Paragraph, ParaStyle, Run};
        let mut d = Document::default();
        let mut line = |style: ParaStyle, text: &str| {
            let mut p = Paragraph { style, line_spacing: 1.0, ..Default::default() };
            p.runs.push(Run {
                text: text.into(),
                size_pt: None,
                font: None,
                fmt: Default::default(),
            });
            d.push_para(p);
        };
        line(ParaStyle::Toc(1), "はじめに … 1");
        line(ParaStyle::Toc(2), "その1 … 2");
        line(ParaStyle::Body, "本文です。");
        let h = html_write::body(&d);
        assert_eq!(h.matches("<nav class=\"toc\">").count(), 1, "nav が1つでない:\n{h}");
        assert_eq!(h.matches("</nav>").count(), 1, "nav の閉じが1つでない:\n{h}");
        assert!(h.contains("class=\"toc1\""), "段の深さが class に出ていない:\n{h}");
        // 本文は nav の外
        let after = h.split("</nav>").nth(1).unwrap_or("");
        assert!(after.contains("本文です。"), "本文が nav の中に入った:\n{h}");
    }

    /// 逃がし忘れると、本文の `<` でページが壊れます。
    #[test]
    fn escapes_special_characters() {
        let d = doc("= 題\n\n1 < 2 & 3 > 0\n");
        let h = html_write::body(&d);
        assert!(h.contains("1 &lt; 2 &amp; 3 &gt; 0"), "逃がせていない:\n{h}");
    }
}

#[cfg(test)]
mod char_format_tests {
    use crate::doc::{CharFormat, Document, Paragraph, Run};

    fn paras(runs: &[(&str, bool)]) -> Document {
        let mut d = Document::default();
        let mut p = Paragraph::default();
        for (s, bold) in runs {
            p.runs.push(Run {
                text: (*s).into(),
                size_pt: None,
                font: None,
                fmt: CharFormat { bold: *bold, ..Default::default() },
            });
        }
        d.push_para(p);
        d
    }

    /// **選んだ字の書式を返す。** ここが1つ手前を見ていたせいで、太字の語を
    /// 選んで太字のボタンを押しても外れなかった(2026-08-17 発注者
    /// 「書式設定が戻せない」)。
    #[test]
    fn with_a_selection_the_selected_texts_format_is_used() {
        // 「ここは」は普通、「大事」は太字(どちらも 9 バイトと 6 バイト)
        let d = paras(&[("ここは", false), ("大事", true), ("なところ。", false)]);
        assert!(d.char_format_at(9..15).bold, "選んだ字が太字なのに拾えていない");
        assert!(!d.char_format_at(0..9).bold, "普通の字を太字と言った");
    }

    /// カーソルが1点のときは今までどおり**直前の字**を見る
    /// (打つとその書式が続く、という慣習)。
    #[test]
    fn a_caret_with_no_selection_looks_at_the_char_before() {
        let d = paras(&[("ここは", false), ("大事", true), ("なところ。", false)]);
        assert!(!d.char_format_at(9..9).bold, "境目の手前は普通の字のはず");
        assert!(d.char_format_at(15..15).bold, "太字の直後は太字を継ぐはず");
    }
}

#[cfg(test)]
mod midashi_tests {
    use super::*;

    fn font() -> Vec<u8> {
        super::test_font()
    }

    /// **見出しは見出しに見える**(2026-08-15)。
    ///
    /// 前は `ParaStyle::Heading` を組版が一度も見ておらず、docx の見出しが
    /// 本文と同じ大きさ・太さで組まれていた(実機で報告書.docx を開いて
    /// 気づいた)。読み書きは前から正しかったので、直したのは見えだけ。
    #[test]
    fn headings_are_laid_out_larger_and_bolder_than_the_body() {
        let data = font();
        let m = Metrics::new(&data).unwrap();
        let frame = Frame { measure_mm: 120.0, line_height_mm: 6.0, y0_mm: 20.0 };
        let mk = |style: ParaStyle| {
            let mut d = Document::plain("");
            d.blocks = vec![Block::Para(Paragraph {
                runs: vec![Run { text: "見出し".into(), size_pt: None, font: None,
                                 fmt: Default::default() }],
                style,
                ..Default::default()
            })];
            d
        };
        let body = layout(&mk(ParaStyle::Body), &m, &frame);
        let h1 = layout(&mk(ParaStyle::Heading(1)), &m, &frame);
        let h2 = layout(&mk(ParaStyle::Heading(2)), &m, &frame);

        let size = |s: &Sheet| s.lines[0].cells[0].size_pt;
        assert!(size(&h1) > size(&body), "H1 が本文より大きくない");
        assert!(size(&h2) > size(&body), "H2 が本文より大きくない");
        assert!(size(&h1) > size(&h2), "H1 が H2 より大きくない");
        assert!(h1.lines[0].cells[0].fmt.bold, "見出しが太字でない");
        assert!(!body.lines[0].cells[0].fmt.bold, "本文まで太字になった");
    }

    /// **run が自分で大きさを言っていればそちらが勝つ**(docx の作法)。
    /// 04_月次報告.docx は見出しの run に `w:sz` を持っており、Word でも
    /// その大きさで出る
    #[test]
    fn a_runs_size_beats_the_heading() {
        let data = font();
        let m = Metrics::new(&data).unwrap();
        let frame = Frame { measure_mm: 120.0, line_height_mm: 6.0, y0_mm: 20.0 };
        let mut d = Document::plain("");
        d.blocks = vec![Block::Para(Paragraph {
            runs: vec![Run { text: "見出し".into(), size_pt: Some(9.0), font: None,
                             fmt: Default::default() }],
            style: ParaStyle::Heading(1),
            ..Default::default()
        })];
        let s = layout(&d, &m, &frame);
        assert_eq!(s.lines[0].cells[0].size_pt, 9.0, "run の指定が負けた");
        // 大きさは run が勝つが、太字は見出しの見え方として掛かる
        assert!(s.lines[0].cells[0].fmt.bold, "見出しの太字が効いていない");
    }

    /// 行の高さも見出しに追従する。**しないと次の行と重なる**
    #[test]
    fn a_heading_line_follows_in_height_too() {
        let frame = Frame { measure_mm: 120.0, line_height_mm: 6.0, y0_mm: 20.0 };
        let p = |style: ParaStyle| Paragraph { style, ..Default::default() };
        assert!(lh_of(&p(ParaStyle::Heading(1)), &frame) > lh_of(&p(ParaStyle::Body), &frame),
                "H1 の行が本文と同じ高さ(重なる)");
        assert_eq!(lh_of(&p(ParaStyle::Body), &frame), 6.0, "本文の高さが変わった");
    }
/// **塊の種類ごとに、正しい要素で出るか。**
///
/// 2026-08-25 まで、コード以外の塊も全部 `<pre><code>` に落ちていました。
/// 例も傍注も註記も、Web ではコードに見えていたということです。
/// 横の区切り線は印の字がそのまま出ていました。
#[test]
fn blocks_render_as_one_element_per_kind() {
    let stroke = "\u{27}\u{27}\u{27}";      // 横の区切り線の印
    let sample = format!(
        "= 題\n\nNOTE: 註記。\n\nWARNING: 警告。\n\n\
         [source,python]\n----\nprint(1)\n----\n\n\
         ....\n字のまま\n....\n\n====\n例。\n====\n\n\
         ****\n傍注。\n****\n\n{stroke}\n");
    let d = crate::adoc::parse(&sample).expect("読めない");
    let h = crate::html_write::body(&d);
    // **開きのタグそのもので見ます。** `class="example"` だけ見ると、
    // `<pre><code class="example">` でも通ってしまいます
    for (what, mark) in [
        ("註記", "<aside class=\"admonition note\""),
        ("警告", "<aside class=\"admonition warning\""),
        ("コード", "<code>"),
        ("字のまま", "字のまま"),
        ("例", "<div class=\"example\""),
        ("傍注", "<aside class=\"sidebar\""),
        ("横の区切り線", "<hr"),
    ] {
        assert!(h.contains(mark), "{what} が {mark} で出ていません:\n{h}");
    }
    // **`pre` はコードと字のままの2つだけ。** 例も傍注も文章です
    assert_eq!(h.matches("<pre").count(), 2, "pre が多すぎます:\n{h}");
    assert_eq!(h.matches("<code>").count(), 1, "コード以外まで code になっています:\n{h}");
    // 印の字が本文に漏れていないこと
    assert!(!h.contains("****"), "傍注の印が本文に出ています:\n{h}");
    assert!(!h.contains("===="), "例の印が本文に出ています:\n{h}");
}

/// **CSS を外しても見た目が残るか**(2026-08-25 発注者
/// 「CSS がなくてもフレットやフラッターのように全部指定するように」)。
///
/// 本文だけを他所へ貼っても、註記が註記に見える必要があります。
#[test]
fn blocks_carry_their_own_look() {
    let d = crate::adoc::parse("= 題\n\nWARNING: 危ない。\n\n----\nprint(1)\n----\n")
        .expect("読めない");
    let h = crate::html_write::body(&d);
    assert!(h.contains("border-left:4px solid"), "註記に線がありません:\n{h}");
    assert!(h.contains("monospace"), "コードが等幅になっていません:\n{h}");
}

/// **そのまま通す塊は逃がさない。** 生の HTML を書くための塊なので、
/// 逃がすと `<details>` のような Web の仕掛けが使えません。
#[test]
fn passthrough_blocks_are_not_escaped() {
    let d = crate::adoc::parse(
        "= 題\n\n++++\n<details><summary>開く</summary>中身</details>\n++++\n")
        .expect("読めない");
    let h = crate::html_write::body(&d);
    assert!(h.contains("<details>"), "生の HTML が逃がされています:\n{h}");
}

/// **多段のリストが、入れ子で出るか。**
///
/// 2026-08-25 まで、`**` で深くした段が Web では平らに並んでいました。
/// 模型は `indent` で段を持っているのに、書き出しが捨てていました。
#[test]
fn nested_lists_render_nested() {
    let d = crate::adoc::parse("= 題\n\n* 1段目\n** 2段目\n*** 3段目\n* また1段目\n")
        .expect("読めない");
    let h = crate::html_write::body(&d);
    assert_eq!(h.matches("<ul").count(), 3, "段の数だけ ul が要ります:\n{h}");
    assert_eq!(h.matches("</ul>").count(), 3, "開いた分だけ閉じていません:\n{h}");
    // **深い段は親の項目の中**。`</li>` の前に `<ul` が来ます
    let i = h.find("1段目").expect("1段目が無い");
    let behind = &h[i..];
    let j = behind.find("<ul").expect("入れ子の ul が無い");
    let k = behind.find("</li>").expect("項目の閉じが無い");
    assert!(j < k, "深い段が親の項目の外に出ています:\n{h}");
}

/// 番号付きも同じように入れ子になるか。
#[test]
fn nested_ordered_lists_render_nested() {
    let d = crate::adoc::parse("= 題\n\n. 番号1\n.. 番号2\n").expect("読めない");
    let h = crate::html_write::body(&d);
    assert_eq!(h.matches("<ol").count(), 2, "段の数だけ ol が要ります:\n{h}");
    assert_eq!(h.matches("</ol>").count(), 2, "開いた分だけ閉じていません:\n{h}");
}

/// **作業のリスト**(`* [ ]` / `* [x]`)がチェックボックスで出るか。
///
/// 前は本文の段落になり、印の `* [ ]` がそのままページに出ていました。
#[test]
fn task_lists_render_as_checkboxes() {
    let d = crate::adoc::parse("= 題\n\n* [ ] まだの作業\n* [x] 済んだ作業\n")
        .expect("読めない");
    let h = crate::html_write::body(&d);
    assert!(h.contains("<input type=\"checkbox\" disabled"), "空の箱がありません:\n{h}");
    assert!(h.contains("<input type=\"checkbox\" checked disabled"), "済みの箱がありません:\n{h}");
    assert!(h.contains("まだの作業"), "本文が消えています:\n{h}");
    assert!(!h.contains("[ ]"), "印の字がページに出ています:\n{h}");
    assert!(!h.contains("[x]"), "印の字がページに出ています:\n{h}");
    // **点は出しません。** 箱と点が二重になります
    assert!(h.contains("list-style:none"), "点を消していません:\n{h}");
}

/// **事務の様式の番号**(1 →(1)→ ア →(ア))。
///
/// 役所の文書はこの順です。Word の日本語の既定も同じで、
/// 2026-08-25 まで3段目が `1)` でした。
#[test]
fn numbers_follow_the_template_order() {
    let mut p = crate::Paragraph { list: crate::ListKind::Number, ..Default::default() };
    p.indent = 0;
    assert_eq!(p.marker(0).as_deref(), Some("1. "));
    p.indent = 1;
    assert_eq!(p.marker(0).as_deref(), Some("(1) "));
    p.indent = 2;
    assert_eq!(p.marker(0).as_deref(), Some("ア "), "3段目はカタカナ");
    assert_eq!(p.marker(2).as_deref(), Some("ウ "));
    p.indent = 3;
    assert_eq!(p.marker(0).as_deref(), Some("(ア) "), "4段目は括弧つきのカタカナ");
    // 五十音を使い切っても番号が消えないこと
    p.indent = 2;
    assert_eq!(p.marker(45).as_deref(), Some("ア1 "), "45 を超えたら数を足す");
}

/// **番号の付け方の指定**(`[loweralpha]` `[start=5]`)。
///
/// 前は読み捨てられ、しかも*指定の行でリストが切れず*、
/// 指定の違うリストが1つに繋がっていました。
#[test]
fn the_numbering_style_spec_takes_effect() {
    let d = crate::adoc::parse(
        "= 題\n\n[loweralpha]\n. あ\n. い\n\n[upperroman]\n. 一\n\n[start=5]\n. 五\n")
        .expect("読めない");
    let h = crate::html_write::body(&d);
    assert!(h.contains("<ol type=\"a\">"), "小文字の英字になっていません:\n{h}");
    assert!(h.contains("<ol type=\"I\">"), "大文字のローマ数字になっていません:\n{h}");
    assert!(h.contains("start=\"5\""), "始めの数が効いていません:\n{h}");
    // **指定ごとに別のリスト**。3つに切れているはず
    assert_eq!(h.matches("<ol").count(), 3, "指定の行でリストが切れていません:\n{h}");
}

/// HTML の番号も、紙と同じ段の並びにするか。
#[test]
fn nested_numbering_renders_in_the_same_order_as_paper() {
    let d = crate::adoc::parse("= 題\n\n. 1段\n.. 2段\n... 3段\n").expect("読めない");
    let h = crate::html_write::body(&d);
    assert!(h.contains("list-style-type:katakana"), "3段目がカタカナになっていません:\n{h}");
}

/// **問答形式**(`[qanda]`)。手続きの案内は問いと答えで書きます。
///
/// 2026-08-25 まで、指定は読み捨てられて普通の用語の一覧になっていました。
#[test]
fn qanda_renders_as_question_and_answer() {
    let d = crate::adoc::parse(
        "= 題\n\n[qanda]\n申請はいつまでですか:: 3月31日までです。\n\
         手数料はいくらですか:: 300円です。\n")
        .expect("読めない");
    let h = crate::html_write::body(&d);
    assert!(h.contains("<ol class=\"qanda\">"), "問答の一覧になっていません:\n{h}");
    assert!(!h.contains("<dl>"), "用語の一覧のまま出ています:\n{h}");
    assert!(h.contains("申請はいつまでですか"), "問いが消えています:\n{h}");
    assert!(h.contains("3月31日までです。"), "答えが消えています:\n{h}");
    // **問いは太く。** CSS を外しても問いと答えが見分けられること
    assert!(h.contains("font-weight:600"), "問いが太くありません:\n{h}");
}

/// **空行で切れた2つの一覧が、1つに繋がらないか。**
///
/// 印が無かったころは、書き戻しで空行が消えて別々の一覧が呑まれ、
/// HTML でも1つの `dl` になっていました。
#[test]
fn lists_separated_by_a_blank_line_are_distinct々becomes() {
    let from = "= 題\n\n[qanda]\n問い:: 答え\n\n用語:: 普通の説明\n";
    let d = crate::adoc::parse(from).expect("読めない");
    // **書き戻しで空行が残ること**(ここが本体)
    assert_eq!(crate::adoc::write(&d), from, "書き戻しで空行が消えています");
    let h = crate::html_write::body(&d);
    assert!(h.contains("<ol class=\"qanda\">"), "問答が出ていません:\n{h}");
    assert!(h.contains("<dl>"), "後ろの用語の一覧が出ていません:\n{h}");
    // 内輪の印が字に漏れていないこと
    assert!(!h.contains("説明のリストの始め"), "内輪の印がページに出ています:\n{h}");
}

/// **紙の上の塊と註記**(2026-08-25)。
///
/// 手引きは「コードの塊は等幅」「註記は種類が分かる」と書いてあるのに、
/// 紙の側は 3つとも外れていました。
///
/// * `[source,python]` と `----` が本文としてそのまま印刷される
/// * `NOTE: ` の印が読むときに外れるので、紙では普通の段落に見える
/// * コードが本文と同じ書体で組まれる
#[test]
fn blocks_and_admonitions_are_distinguishable_on_paper_too() {
    let d = crate::adoc::parse(
        "= 題\n\nNOTE: 気をつけて。\n\n[source,python]\n----\nprint(1)\n----\n\n普通の段落。\n")
        .expect("読めない");
    let (fam, _) = crate::font::for_document(None).expect("フォントが無い");
    let data = crate::font::load(fam).expect("読めない");
    let m = crate::Metrics::new(&data).expect("読めない");
    let sheet = crate::layout(
        &d, &m, &crate::Frame { measure_mm: 160.0, line_height_mm: 6.0, y0_mm: 20.0 });
    let line: Vec<String> = sheet.lines.iter()
        .map(|l| l.cells.iter().map(|c| c.ch).collect::<String>())
        .filter(|t| !t.trim().is_empty())
        .collect();
    let all = line.join("\n");
    // **印の行は紙に出しません**
    assert!(!all.contains("----"), "塊の印が印刷されています:\n{all}");
    assert!(!all.contains("[source"), "塊の指定が印刷されています:\n{all}");
    // **註記は種類が分かること**
    assert!(all.contains("メモ"), "註記の見出しがありません:\n{all}");
    // **等幅の書体が入っているのに探していない、を捕まえます。**
    // 「機械に無いから」で素通りすると、探す所を壊しても気づけません
    let is_set = ["Noto Sans Mono CJK JP", "Noto Sans Mono", "DejaVu Sans Mono",
                      "Liberation Mono", "IPAGothic", "MS Gothic", "BIZ UDGothic",
                      "Osaka-Mono", "Courier New"]
        .iter()
        .any(|n| crate::font::for_document(Some(n)).is_ok_and(|(_, genuine)| genuine));
    assert_eq!(is_set, crate::font::monospace().is_some(),
               "等幅の書体が入っているのに monospace() が見つけていません");
    // **コードは等幅**(この機械に等幅の書体があるときだけ見ます)
    if crate::font::monospace().is_some() {
        let code_line = sheet.lines.iter()
            .find(|l| l.cells.iter().map(|c| c.ch).collect::<String>().contains("print"))
            .expect("コードの行がありません");
        assert!(code_line.cells[0].font.is_some(), "コードが本文と同じ書体です");
    }
}

/// **作業のリストが紙でもチェックボックスで出るか**(2026-08-25)。
///
/// HTML だけ直して紙を見ていなかったので、`* [ ]` の印がそのまま
/// 印刷されていました。記入欄と同じ ☐ / ☑ で出します。
#[test]
fn task_lists_render_as_boxes_on_paper_too() {
    let d = crate::adoc::parse("= 題\n\n* [ ] やること\n* [x] 済んだこと\n")
        .expect("読めない");
    let (fam, _) = crate::font::for_document(None).expect("フォントが無い");
    let data = crate::font::load(fam).expect("読めない");
    let m = crate::Metrics::new(&data).expect("読めない");
    let sheet = crate::layout(
        &d, &m, &crate::Frame { measure_mm: 160.0, line_height_mm: 6.0, y0_mm: 20.0 });
    let all: String = sheet.lines.iter()
        .map(|l| l.cells.iter().map(|c| c.ch).collect::<String>())
        .collect::<Vec<_>>().join("\n");
    assert!(all.contains("☐ やること"), "空の箱が出ていません:\n{all}");
    assert!(all.contains("☑ 済んだこと"), "済みの箱が出ていません:\n{all}");
    assert!(!all.contains("[ ]"), "印の字が印刷されています:\n{all}");
    assert!(!all.contains("[x]"), "印の字が印刷されています:\n{all}");
    // **書き戻しは元のまま。** 紙の見た目のために字を変えていないこと
    assert_eq!(crate::adoc::write(&d), "= 題\n\n* [ ] やること\n* [x] 済んだこと\n");

    // **段も効くこと。** `**` なら1段下がって組まれます
    let d2 = crate::adoc::parse("= 題\n\n* [ ] 親\n** [ ] 子\n").expect("読めない");
    let s2 = crate::layout(
        &d2, &m, &crate::Frame { measure_mm: 160.0, line_height_mm: 6.0, y0_mm: 20.0 });
    let search_for = |text: &str| -> f32 {
        s2.lines.iter()
            .find(|l| l.cells.iter().map(|c| c.ch).collect::<String>().contains(text))
            .map(|l| l.cells[0].x_mm)
            .unwrap_or_else(|| panic!("{text} の行がありません"))
    };
    assert!(search_for("子") > search_for("親"), "2段目が下がっていません");

    // **`-` の書き方**(Markdown)も作業のリストです
    let d3 = crate::adoc::parse("= 題\n\n- [ ] 別の書き方\n").expect("読めない");
    let h = crate::html_write::body(&d3);
    assert!(h.contains("type=\"checkbox\""), "- の書き方が箱になっていません:\n{h}");
    assert!(!h.contains("- [ ]"), "印の字が残っています:\n{h}");
}

/// **多段の作業のリスト**(2026-08-25)。
///
/// 読み手が1段目しか拾っておらず、`** [ ]` は普通の箇条書きになって
/// `[ ]` が字として残っていました。
#[test]
fn nested_task_lists_also_render_as_boxes() {
    let from = "= 題\n\n* [ ] 親\n** [x] 子\n";
    let d = crate::adoc::parse(from).expect("読めない");
    assert_eq!(crate::adoc::write(&d), from, "書き戻しで字が変わっています");
    let h = crate::html_write::body(&d);
    assert_eq!(h.matches("type=\"checkbox\"").count(), 2, "箱が2つ要ります:\n{h}");
    assert_eq!(h.matches("<ul").count(), 2, "入れ子になっていません:\n{h}");
    assert!(!h.contains("[ ]") && !h.contains("[x]"), "印の字が残っています:\n{h}");
}

}

/// **段落と升の背景色が紙面に落ちる**(2026-08-27 発注者「セルの塗りが
/// どうして文書にないのですか」)。
///
/// 模型は前から持っていて、画面も塗っていました。**組む所で落としていた**
/// ので、紙と PDF に出ていません。註記の帯も見出しの背景も印刷で消えます。
#[cfg(test)]
mod shade_tests {
    #[test]
    fn a_paragraph_shade_reaches_the_page() {
        let mut d = crate::adoc::parse("NOTE: 註記です。\n").expect("読めない");
        let t = crate::theme::default_theme();
        d = crate::theme::compose(&d, &t);
        let m = crate::Metrics::new(&[]).ok();
        let _ = m;
        let f = crate::font::default_family("ja").expect("書体");
        let bytes = crate::font::load(f).expect("読めない");
        let m = crate::Metrics::new(&bytes).expect("測れない");
        let s = crate::layout(
            &d,
            &m,
            &crate::Frame { measure_mm: 170.0, line_height_mm: crate::LINE_MM, y0_mm: 24.0 },
        );
        assert!(!s.fills.is_empty(), "註記の帯が紙面に落ちていない");
        assert_eq!(s.fills[0].1, "FFF6E0", "既定のテンプレートの色でない");
    }

    /// **升の塗り**も同じ道です
    #[test]
    fn a_cell_shade_reaches_the_page() {
        let mut d = crate::Document::default();
        let mut cell = crate::Cellbox::default();
        let mut p = crate::Document::plain("見出し").paragraphs().next().cloned().expect("段落");
        p.shade = Some("4472C4".into());
        cell.paragraphs = vec![p];
        d.blocks.push(crate::Block::Table(crate::Table {
            rows: vec![vec![cell]],
            ..Default::default()
        }));
        let f = crate::font::default_family("ja").expect("書体");
        let bytes = crate::font::load(f).expect("読めない");
        let m = crate::Metrics::new(&bytes).expect("測れない");
        let s = crate::layout(
            &d,
            &m,
            &crate::Frame { measure_mm: 170.0, line_height_mm: crate::LINE_MM, y0_mm: 24.0 },
        );
        assert!(
            s.fills.iter().any(|(_, c)| c == "4472C4"),
            "升の塗りが紙面に落ちていない: {:?}",
            s.fills
        );
    }
}
