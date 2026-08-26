//! writer の試験(main.rs から純移動 2026-08-08。分割の1歩目 —
//! calc と同じ作法: 純移動だけ、挙動と文言は一切変えない)。
//! 中の `use crate::*` は移動先でも同じ意味になるよう `use crate::*` に直した

#[cfg(test)]
mod cell_edit_tests {
    use crate::*;

    fn doc_with_table() -> Document {
        let cell = |s: &str| kumihan::Cellbox {
            paragraphs: vec![kumihan::Paragraph {
                runs: vec![kumihan::Run {
                    text: s.into(), size_pt: None, font: None, fmt: Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut d = Document::plain("本文");
        d.blocks.push(kumihan::Block::Table(kumihan::Table {
            col_mm: vec![],
            rows: vec![vec![cell("品名"), cell("金額")]],
            ..Default::default()
        }));
        d
    }

    #[test]
    fn セルの文章を読み書きできる() {
        let d = doc_with_table();
        let t = d.tables().next().unwrap();
        assert_eq!(cell_text(&t.rows[0][0]), "品名");
        let mut c = t.rows[0][0].clone();
        set_cell_text(&mut c, "型式\n数量");
        assert_eq!(c.paragraphs.len(), 2, "段落に割れていない");
        assert_eq!(cell_text(&c), "型式\n数量");
    }

    #[test]
    fn セルの書式は書き戻しで残る() {
        let d = doc_with_table();
        let mut c = d.tables().next().unwrap().rows[0][0].clone();
        c.paragraphs[0].align = kumihan::Align::Center;
        c.paragraphs[0].runs[0].fmt.bold = true;
        set_cell_text(&mut c, "直した");
        assert_eq!(c.paragraphs[0].align, kumihan::Align::Center, "揃えが消えた");
        assert!(c.paragraphs[0].runs[0].fmt.bold, "太字が消えた");
    }
}

#[cfg(test)]
mod find_tests {
    use crate::*;

    fn w(text: &str) -> (Editor, Editor, Editor) {
        (Editor::new(text), Editor::new(""), Editor::new(""))
    }

    // find_next/replace の中身はエディタ操作の列なので、
    // ここでは検索の規則(後ろから・一周する)だけを関数で確かめる
    fn next_hit(text: &str, term: &str, from: usize) -> Option<usize> {
        text[from..].find(term).map(|i| from + i).or_else(|| text.find(term))
    }

    #[test]
    fn カーソルの後ろから探す() {
        let t = "誤りを直す。誤りは残さない。";
        let first = next_hit(t, "誤り", 0).unwrap();
        let second = next_hit(t, "誤り", first + "誤り".len()).unwrap();
        assert!(second > first);
    }

    #[test]
    fn 末尾まで無ければ頭から一周() {
        let t = "誤りを直す。";
        // 「直」の後ろ(文字境界)から探す。実物の from はカーソル位置なので常に境界
        let from = "誤りを直".len();
        let hit = next_hit(t, "誤り", from);
        assert_eq!(hit, Some(0), "一周していない");
    }

    #[test]
    fn 無ければ無いと言える() {
        assert_eq!(next_hit("本文", "存在しない", 0), None);
        let _ = w("x");
    }
}

/// **メニューのボタンを全部おして、落ちないか・繋がっているかを見る。**
/// リボンに ready で並ぶものは、ここで実際に run_cmd を通す
/// (ダイアログを開くものだけは、開いた窓が閉じられないので外す)。
/// GUI は起こさない — gpui の試験用の場で Writer を作って叩く
#[cfg(test)]
mod menu_run_tests {
    use crate::*;

    /// ファイル選択の窓を開くボタン。**試験では押さない** —
    /// rfd は実際に窓を出しに行くので、画面の無い試験では返ってこない
    /// (踏んで確かめた。実機での確認に回す)
    pub(super) const DIALOG: &[&str] = &[
        "open", "save", "pdf", "plug-macros", "insimage", "text-from-file",
        "insshape", "inssmartart", "inschart", "instextart",
        "insequation",
    ];

    /// AI の宛先は共有の設定(~/.config/officework/ai.txt)。触る試験が並走すると
    /// 保存と復元が交錯して稀に落ちるので、同時には走らせない
    static AI_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// **4つの一覧が開いて畳める**(2026-08-22 に旗4つを鍵1つにした)。
    ///
    /// 前は bool が4つあり、開くたびに残り3つを倒す行が要りました。
    /// 1つにしたので「倒し忘れ」が書けません。それを見ます。
    #[gpui::test]
    fn 一覧は多くて1つしか開かない(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            // **意味だけの文書では書体と大きさは押せません**(look_guard。
            // スタイルの一覧へ案内する道になっています)。互換の文書にして
            // から見ます — ここで見たいのは一覧の開け閉めだけです
            this.native = false;
            for id in ["fontname", "fontsize", "parastyle", "inssymbol"] {
                this.run_cmd(id, cx);
                assert_eq!(this.open_list, Some(id), "{id} が開かない");
                // もう一度押すと畳む(トグル)
                this.run_cmd(id, cx);
                assert_eq!(this.open_list, None, "{id} がトグルで畳めない");
            }
            // **別の一覧を押したら、前のは畳まれる**
            this.run_cmd("fontname", cx);
            this.run_cmd("parastyle", cx);
            assert_eq!(this.open_list, Some("parastyle"), "乗り換えられない");
            // 一覧でないボタンを押したら畳む
            this.run_cmd("bold", cx);
            assert_eq!(this.open_list, None, "他のボタンで畳まれない");
        });
    }

    #[gpui::test]
    fn 全部のボタンが落ちずに通る(cx: &mut gpui::TestAppContext) {
        let _ai = AI_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // AI の宛先は覚える設定なので、試験で変えたら戻す
        let keep_ai = ui::ai::backend();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        for tab in ui::ribbon::WRITER {
            for cmd in tab.cmds {
                if !cmd.ready || DIALOG.contains(&cmd.id) {
                    continue;
                }
                let id = cmd.id;
                let label = cmd.label;
                w.update(cx, |this, cx| {
                    // 本文が空だと何も起きないボタンがあるので、毎回中身を入れておく
                    if this.ed.text().is_empty() {
                        this.set_doc(Document::plain("見出し\n本文の字。"));
                    }
                    this.ed.select_all();
                    this.run_cmd(id, cx);
                    let st = this.status.to_string();
                    assert!(
                        !st.contains("未配線"),
                        "「{label}」({id}) が未配線: {st}"
                    );
                });
            }
        }
        ui::ai::set_backend(keep_ai);
    }

    /// 押すと入切するボタンは、2回押すと元に戻る(1手で戻せる方針)
    #[gpui::test]
    fn 入切のボタンは二度おすと戻る(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        for id in [
            "ruler", "darkmode", "hidenchars", "line-numbers", "nav",
            "show-statusbar", "show-right", "co-showcomment", "direction",
            "multipage", "prot-doc",
        ] {
            w.update(cx, |this, cx| {
                let before = this.toggled(id);
                this.run_cmd(id, cx);
                let mid = this.toggled(id);
                assert_ne!(before, mid, "「{id}」を押しても変わらない");
                this.run_cmd(id, cx);
                assert_eq!(before, this.toggled(id), "「{id}」が元に戻らない");
            });
        }
    }

    /// **見本の文書を開いた状態でも**全部のボタンが通る。
    /// 空の文書と違い、表・見出し・記入欄・縦書きが入っているので、
    /// 「前提があるときの道」も通る(sample/writer が検査の材料)
    #[gpui::test]
    fn 見本を開いても全部のボタンが通る(cx: &mut gpui::TestAppContext) {
        let dir = std::path::Path::new("../sample/writer");
        let dir = if dir.exists() {
            dir.to_path_buf()
        } else {
            std::path::Path::new("sample/writer").to_path_buf()
        };
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return; // 見本が無い環境では黙って飛ばす(失敗にはしない)
        };
        let mut files: Vec<std::path::PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("docx"))
            .collect();
        files.sort();
        assert!(!files.is_empty(), "見本が無い: {}", dir.display());
        let _ai = AI_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let keep_ai = ui::ai::backend();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        for f in files {
            w.update(cx, |this, _| this.open(f.clone()));
            for tab in ui::ribbon::WRITER {
                for cmd in tab.cmds {
                    if !cmd.ready || DIALOG.contains(&cmd.id) {
                        continue;
                    }
                    let (id, label) = (cmd.id, cmd.label);
                    let name = f.file_name().unwrap().to_string_lossy().to_string();
                    w.update(cx, |this, cx| {
                        this.run_cmd(id, cx);
                        let st = this.status.to_string();
                        assert!(
                            !st.contains("未配線"),
                            "{name} で「{label}」({id}) が未配線: {st}"
                        );
                    });
                }
            }
        }
        ui::ai::set_backend(keep_ai);
    }

    /// **押した結果が本当に文書に出るか。** status だけ見ても
    /// 「押せるのに何も起きない」は捕まらないので、モデルを見る
    #[gpui::test]
    fn 主なボタンは文書を実際に変える(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        let fresh = |this: &mut Writer| {
            // **docx を開いた状態で見ます。** adoc 形式では見た目のボタンが
            // スタイルの面へ案内するので、直に掛かるかを見るのはこちら
            this.native = false;
            this.set_doc(Document::plain("あいうえお\nかきくけこ"));
            this.ed.move_to(0, false);
            this.ed.move_to(15, true); // 1段落目を選ぶ
        };
        // 文字書式
        for (id, get) in [
            ("bold", (|f: &kumihan::CharFormat| f.bold) as fn(&kumihan::CharFormat) -> bool),
            ("italic", |f| f.italic),
            ("underline", |f| f.underline),
            ("strikeout", |f| f.strike),
            ("superscript", |f| f.superscript),
        ] {
            w.update(cx, |this, cx| {
                fresh(this);
                this.run_cmd(id, cx);
                let f = this.doc.char_format_at(0..15);
                assert!(get(&f), "「{id}」が字に効いていない");
            });
        }
        // 段落の性質
        w.update(cx, |this, cx| {
            fresh(this);
            this.run_cmd("align-center", cx);
            assert_eq!(this.doc.paragraphs().next().unwrap().align, Align::Center);
            this.run_cmd("align-dist", cx);
            assert_eq!(this.doc.paragraphs().next().unwrap().align, Align::Distribute);
            this.run_cmd("markers", cx);
            assert_eq!(this.doc.paragraphs().next().unwrap().list, ListKind::Bullet);
            this.run_cmd("incoffset", cx);
            assert!(this.doc.paragraphs().next().unwrap().indent >= 1);
            this.run_cmd("paracolor", cx);
            assert!(this.doc.paragraphs().next().unwrap().shade.is_some());
            this.run_cmd("borders", cx);
            assert!(this.doc.paragraphs().next().unwrap().boxed);
            this.run_cmd("dropcap", cx);
            assert!(this.doc.paragraphs().next().unwrap().dropcap);
        });
        // 文書ぜんたい
        w.update(cx, |this, cx| {
            fresh(this);
            let n0 = this.doc.paragraphs().count();
            this.run_cmd("instable-go", cx);
            assert_eq!(this.doc.tables().count(), 1, "表が入らない");
            this.run_cmd("blankpage", cx);
            assert!(this.doc.paragraphs().count() > n0, "空白ページが入らない");
            this.run_cmd("watermark", cx);
            assert!(this.wm_edit, "透かしのパネルが開かない");
            this.run_cmd("watermark", cx);
            this.run_cmd("pagecolor", cx);
            assert!(this.doc.page_color.is_some(), "紙の色が変わらない");
            this.run_cmd("columns", cx);
            assert!(this.pg.cols() > 1, "段組みにならない");
            this.run_cmd("pageorient", cx);
            assert!(this.pg.w_mm > this.pg.h_mm, "向きが変わらない");
            this.run_cmd("hyphenation", cx);
            assert!(this.doc.hyphenate, "ハイフネーションが入らない");
        });
        // 見出し → 目次 → 相互参照の的(しおり)
        w.update(cx, |this, cx| {
            this.set_doc(Document::plain("章のはじめ\n本文です。"));
            this.ed.move_to(0, false);
            // 見出しにするのは「テキストの追加」(parastyle は一覧のパネルを開く)
            this.run_cmd("add-text", cx);
            assert!(
                matches!(
                    this.doc.paragraphs().next().unwrap().style,
                    kumihan::ParaStyle::Heading(_)
                ),
                "見出しにならない"
            );
            this.run_cmd("parastyle", cx);
            assert_eq!(this.open_list, Some("parastyle"), "段落のスタイルの一覧が開かない");
            this.run_cmd("parastyle", cx);
            this.run_cmd("toc", cx);
            assert!(
                this.doc
                    .paragraphs()
                    .any(|p| matches!(p.style, kumihan::ParaStyle::Toc(_))),
                "目次が入らない"
            );
            // 「図 」と直に書かない — 訳の入る言語では雛形が変わる。
            // **2つ目が 2 番になる**ことまで見る(1つ目を数えそこねると、
            // どの図も 1 番のままになる。日本語では気づけない不具合だった)
            this.run_cmd("caption", cx);
            assert!(
                this.doc.body_text().contains(&ui::tf!("figure", 1)),
                "図表番号が入らない: {}",
                this.doc.body_text()
            );
            this.run_cmd("caption", cx);
            assert!(
                this.doc.body_text().contains(&ui::tf!("figure", 2)),
                "2つ目の図表番号が 2 にならない(1つ目を数えそこねている): {}",
                this.doc.body_text()
            );
        });
        // 配色(見出しの色が付く)
        w.update(cx, |this, cx| {
            this.set_doc(Document::plain("題"));
            this.ed.move_to(0, false);
            this.run_cmd("add-text", cx);
            this.run_cmd("colorschemas", cx);
            let colored = this
                .doc
                .paragraphs()
                .flat_map(|p| &p.runs)
                .any(|r| r.fmt.color.is_some());
            assert!(colored, "配色で色が付かない");
        });
        // ペン(描いた筆が文書に残る)
        w.update(cx, |this, cx| {
            this.set_doc(Document::plain("紙"));
            this.run_cmd("pen", cx);
            assert!(this.tool.is_some(), "ペンにならない");
            this.ink_begin(10.0, 10.0);
            this.ink_move(20.0, 20.0);
            this.ink_end();
            assert!(!this.doc.ink.is_empty(), "筆が残らない");
        });
    }

    /// AI は**宛先が使えないときに正直に断る**(黙って空にしない)。
    /// 実際にモデルへ繋ぐ試験はしない(手元に居るとは限らないので)。
    ///
    /// **ふりがなでは見られなくなりました**(2026-08-20)。辞書で振るように
    /// したので、宛先が無くてもふりがなは通ります。残っている AI の仕事は
    /// マクロを書く分だけなので、そちらで見ます
    #[gpui::test]
    fn aiは宛先が無ければ理由を言う(cx: &mut gpui::TestAppContext) {
        let _ai = AI_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let keep = ui::ai::backend();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        ui::ai::set_backend(ui::ai::Backend::ClaudeApi);
        if ui::ai::ready(ui::ai::Backend::ClaudeApi).is_err() {
            w.update(cx, |this, cx| {
                this.set_doc(Document::plain("本文です。"));
                this.ai_go(crate::AiJob::Macro("並べ替える".into()), cx);
                let st = this.status.to_string();
                assert!(st.starts_with("AI:"), "断りの言葉が出ない: {st}");
                assert!(!this.ai_busy, "断ったのに考え中のまま");
            });
        }
        ui::ai::set_backend(keep);
    }

    /// **ふりがなは辞書で振る**(2026-08-20 発注者「取り敢えずは辞書で」)。
    /// 宛先が無くても通り、外にも出ません。
    /// 辞書の無い機械では、いままでどおりモデルに回ります(その場合は飛ばす)
    #[gpui::test]
    fn ふりがなは宛先が無くても辞書で振れる(cx: &mut gpui::TestAppContext) {
        if !ui::dict::available() {
            return;
        }
        let _ai = AI_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let keep = ui::ai::backend();
        ui::ai::set_backend(ui::ai::Backend::ClaudeApi);
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.set_doc(Document::plain("路地を歩く。"));
            this.run_cmd("ai-furigana", cx);
            let st = this.status.to_string();
            assert!(st.contains("ふりがな"), "ふりがなの報せが出ない: {st}");
            assert!(!st.starts_with("AI:"), "辞書があるのにモデルへ回った: {st}");
            assert!(!this.ai_busy, "辞書なのに考え中のまま");
        });
        ui::ai::set_backend(keep);
    }

    /// 記入欄(フォーム)は押した種類の欄が本当に入る
    #[gpui::test]
    fn フォームのボタンが記入欄を入れる(cx: &mut gpui::TestAppContext) {
        use kumihan::SdtKind as K;
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        for (id, want) in [
            ("form-text", K::Text),
            ("form-image", K::Picture),
            ("form-email", K::Email),
            ("form-phone", K::Phone),
            ("form-complex", K::Complex),
            ("form-signature", K::Signature),
            ("controls", K::Text),
        ] {
            w.update(cx, |this, cx| {
                this.set_doc(Document::plain(""));
                this.run_cmd(id, cx);
                let kinds: Vec<_> = this
                    .doc
                    .paragraphs()
                    .flat_map(|p| &p.runs)
                    .filter_map(|r| r.fmt.sdt.as_ref().map(|s| s.kind))
                    .collect();
                assert!(kinds.contains(&want), "「{id}」で {want:?} が入らない: {kinds:?}");
            });
        }
    }

    /// マクロの fill(名前, 値) が名前つき記入欄に本当に書く。
    /// サンドボックスの外で同じ台本を回す(bwrap の無い試験環境でも通る)。
    /// python-docx が無い環境では黙って飛ばす
    #[test]
    fn マクロのfillが名前の記入欄に書く() {
        let py = if std::path::Path::new("../.venv/bin/python").exists() {
            std::path::PathBuf::from("../.venv/bin/python")
        } else {
            find_python()
        };
        let ok = std::process::Command::new(&py)
            .args(["-c", "import docx"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !ok {
            eprintln!("python-docx が無いので飛ばす");
            return;
        }
        // 名前「氏名」の記入欄(8..17)と、独自種類メールに名前「連絡先」を
        // 付けた欄(26..35。w:tag は jo:email:連絡先 に合成される)を持つ docx
        let mut doc = Document::plain("氏名: 未記入\n宛先: 未記入");
        doc.apply_char_format(8..17, |f| {
            f.sdt = Some(Box::new(kumihan::Sdt {
                kind: kumihan::SdtKind::Text,
                alias: "氏名".into(),
                tag: "氏名".into(),
                items: Vec::new(),
            }))
        });
        doc.apply_char_format(26..35, |f| {
            f.sdt = Some(Box::new(kumihan::Sdt {
                kind: kumihan::SdtKind::Email,
                alias: "連絡先".into(),
                tag: "連絡先".into(),
                items: Vec::new(),
            }))
        });
        let dir =
            std::env::temp_dir().join(format!("jo-fill-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let (in_d, out_d) = (dir.join("in.docx"), dir.join("out.docx"));
        let f = std::fs::File::create(&in_d).unwrap();
        ooxml::write_with(&doc, None::<std::io::Cursor<Vec<u8>>>, std::io::BufWriter::new(f))
            .unwrap();
        let py_path = dir.join("run.py");
        let script = macro_script(
            &in_d,
            &out_d,
            "fill(\"氏名\", \"山田太郎\")\nfill(\"連絡先\", \"y@example.jp\")",
        );
        std::fs::write(&py_path, script).unwrap();
        let o = std::process::Command::new(&py).arg(&py_path).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let (doc2, _) =
            ooxml::read(std::io::Cursor::new(std::fs::read(&out_d).unwrap())).unwrap();
        let body: String = doc2
            .paragraphs()
            .flat_map(|p| &p.runs)
            .map(|r| r.text.as_str())
            .collect();
        assert!(body.contains("山田太郎"), "記入されていない: {body}");
        assert!(body.contains("y@example.jp"), "合成 tag の欄に書けない: {body}");
        // 記入しても欄(w:tag)は生きている — もう一度 fill できる
        let tags: Vec<_> = doc2
            .paragraphs()
            .flat_map(|p| &p.runs)
            .filter_map(|r| r.fmt.sdt.as_ref().map(|s| s.tag.clone()))
            .collect();
        assert!(tags.contains(&"氏名".to_string()), "欄が消えた: {tags:?}");
        // 無い名前は黙って空振りせず、ことばで断る
        let script = macro_script(&in_d, &out_d, "fill(\"住所\", \"x\")");
        std::fs::write(&py_path, script).unwrap();
        let o = std::process::Command::new(&py).arg(&py_path).output().unwrap();
        assert!(!o.status.success(), "無い名前で通ってしまう");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains("住所"), "断りに名前が出ない: {err}");
        // 吸い上げ(入口): 記入済みから extract で値を読み、fields で一覧
        let script = macro_script(
            &out_d,
            &dir.join("out2.docx"),
            "print(extract(\"氏名\"))\nfor n, v in fields():\n    print(n, v)",
        );
        std::fs::write(&py_path, script).unwrap();
        let o = std::process::Command::new(&py).arg(&py_path).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let out = String::from_utf8_lossy(&o.stdout).to_string();
        assert!(out.contains("山田太郎"), "extract が読めない: {out}");
        assert!(
            out.contains("連絡先 y@example.jp"),
            "fields に合成 tag の名前が出ない: {out}"
        );
        // extract も無い名前は断る
        let script = macro_script(&out_d, &dir.join("out3.docx"), "extract(\"住所\")");
        std::fs::write(&py_path, script).unwrap();
        let o = std::process::Command::new(&py).arg(&py_path).output().unwrap();
        assert!(!o.status.success(), "extract が無い名前で通ってしまう");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// render(辞書)= docxtpl の雛形差し込みが台本から使える。
    /// 雛形は Word の編集を模して {{担当者}} を run 分断で割っておき、
    /// writer の読み書き(heal_runs)を通してから差し込む。
    /// docxtpl が無い環境では黙って飛ばす
    #[test]
    fn 雛形のrenderが差し込む() {
        let py = if std::path::Path::new("../.venv/bin/python").exists() {
            std::path::PathBuf::from("../.venv/bin/python")
        } else {
            find_python()
        };
        let ok = std::process::Command::new(&py)
            .args(["-c", "import docx, docxtpl"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !ok {
            eprintln!("docxtpl が無いので飛ばす");
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("jo-render-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let tpl = dir.join("tpl.docx");
        // 雛形: 差し込み口3つ+行くり返しの表。{{担当者}} は割っておく
        let mk = format!(
            r#"import docx
d = docx.Document()
d.add_paragraph("{{{{顧客名}}}} 様")
p = d.add_paragraph("")
p.add_run("担当: {{{{担当")
p.add_run("者}}}}")
tb = d.add_table(rows=4, cols=2)
tb.cell(0, 0).text = "品名"; tb.cell(0, 1).text = "数量"
tb.cell(1, 0).text = "{{%tr for m in 明細 %}}"
tb.cell(2, 0).text = "{{{{m.品名}}}}"; tb.cell(2, 1).text = "{{{{m.数量}}}}"
tb.cell(3, 0).text = "{{%tr endfor %}}"
d.save({tpl:?})
"#,
            tpl = tpl.to_string_lossy()
        );
        let mk_py = dir.join("mk.py");
        std::fs::write(&mk_py, mk).unwrap();
        let o = std::process::Command::new(&py).arg(&mk_py).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        // writer の読み書きを通す(分断は heal_runs が繋ぐ)
        let bytes = std::fs::read(&tpl).unwrap();
        let (tdoc, _) = ooxml::read(std::io::Cursor::new(bytes.clone())).unwrap();
        let healed = dir.join("healed.docx");
        let f = std::fs::File::create(&healed).unwrap();
        ooxml::write_with(
            &tdoc,
            Some(std::io::Cursor::new(bytes)),
            std::io::BufWriter::new(f),
        )
        .unwrap();
        // 台本: 差し込み口を報告してから差し込む
        let code = r#"print(tpl_fields())
render({"顧客名": "青森県庁", "担当者": "山田",
        "明細": [{"品名": "防火戸", "数量": 3}, {"品名": "枠", "数量": 6}]})
"#;
        let out_d = dir.join("out.docx");
        let py_path = dir.join("run.py");
        std::fs::write(&py_path, macro_script(&healed, &out_d, code)).unwrap();
        let o = std::process::Command::new(&py).arg(&py_path).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let stdout = String::from_utf8_lossy(&o.stdout).to_string();
        for name in ["顧客名", "担当者", "明細"] {
            assert!(stdout.contains(name), "tpl_fields に{name}が無い: {stdout}");
        }
        // 差し込み結果を writer 側で読んで確かめる
        let (doc2, _) =
            ooxml::read(std::io::Cursor::new(std::fs::read(&out_d).unwrap())).unwrap();
        let body = doc2.body_text();
        assert!(body.contains("青森県庁 様"), "顧客名が入らない: {body}");
        assert!(body.contains("担当: 山田"), "割れた欄に入らない: {body}");
        let cells: Vec<String> = doc2
            .tables()
            .flat_map(|t| &t.rows)
            .flatten()
            .flat_map(|c| &c.paragraphs)
            .flat_map(|p| &p.runs)
            .map(|r| r.text.clone())
            .collect();
        let joined = cells.join("|");
        assert!(
            joined.contains("防火戸") && joined.contains("枠"),
            "行くり返しが効かない: {joined}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 実物様式(機構の実施要領様式)が writer の読み書きに通り、
    /// python-docx と docxtpl がそのまま読めることを見る。
    /// 様式や道具が無い環境では黙って飛ばす(失敗にはしない)
    #[test]
    fn 実物様式が読み書きと雛形の道具に通る() {
        let src = std::path::Path::new(
            "/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki",
        );
        let Ok(rd) = std::fs::read_dir(src) else { return };
        let py = if std::path::Path::new("../.venv/bin/python").exists() {
            std::path::PathBuf::from("../.venv/bin/python")
        } else {
            find_python()
        };
        let ok = std::process::Command::new(&py)
            .args(["-c", "import docx, docxtpl"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !ok {
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("jo-yoshiki-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut n = 0;
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|e| e.to_str()) != Some("docx") {
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let bytes = std::fs::read(&p).unwrap();
            let (doc, _) = ooxml::read(std::io::Cursor::new(bytes.clone()))
                .unwrap_or_else(|e| panic!("{name} が読めない: {e}"));
            let out = dir.join(&name);
            let f = std::fs::File::create(&out).unwrap();
            ooxml::write_with(
                &doc,
                Some(std::io::Cursor::new(bytes)),
                std::io::BufWriter::new(f),
            )
            .unwrap_or_else(|e| panic!("{name} が書けない: {e}"));
            // 保存し直したものを python-docx と docxtpl で開く
            let check = format!(
                "import docx\nfrom docxtpl import DocxTemplate\n\
                 docx.Document({out:?})\n\
                 print(sorted(DocxTemplate({out:?}).get_undeclared_template_variables()))",
                out = out.to_string_lossy()
            );
            let py_path = dir.join("check.py");
            std::fs::write(&py_path, check).unwrap();
            let o = std::process::Command::new(&py).arg(&py_path).output().unwrap();
            assert!(
                o.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&o.stderr)
            );
            n += 1;
        }
        eprintln!("実物様式 {n} 件が通った");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// モデルが書きがちなコードフェンスは受け側で剥がす
    #[test]
    fn コードフェンスを剥がす() {
        assert_eq!(strip_code_fence("print(1)"), "print(1)");
        assert_eq!(strip_code_fence("```python\nprint(1)\n```"), "print(1)");
        assert_eq!(strip_code_fence("```\nprint(1)\n```\n"), "print(1)");
        assert_eq!(
            strip_code_fence("```python\nfill(\"氏名\", \"x\")\nprint('done')\n```"),
            "fill(\"氏名\", \"x\")\nprint('done')"
        );
    }

    /// 「マクロを書く」はパネルを開き、宛先が無ければ正直に断る。
    /// 台本が文書に入ることは決してない(置き場に置くだけ)
    #[gpui::test]
    fn マクロを書くはパネルを開き宛先が無ければ断る(cx: &mut gpui::TestAppContext) {
        let _ai = AI_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let keep = ui::ai::backend();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.set_doc(Document::plain("本文です。"));
            this.run_cmd("ai-macro", cx);
            assert!(this.ai_open && this.ai_macro, "マクロのパネルが開かない");
            this.run_cmd("ai-macro", cx); // もう一度押すと閉じる
            assert!(!this.ai_open && !this.ai_macro, "パネルが閉じない");
        });
        ui::ai::set_backend(ui::ai::Backend::ClaudeApi);
        if ui::ai::ready(ui::ai::Backend::ClaudeApi).is_err() {
            w.update(cx, |this, cx| {
                let before = this.ed.text().to_string();
                this.ai_go(AiJob::Macro("氏名を記入して".into()), cx);
                let st = this.status.to_string();
                assert!(st.starts_with("AI:"), "断りの言葉が出ない: {st}");
                assert!(!this.ai_busy, "断ったのに考え中のまま");
                assert_eq!(this.ed.text(), before, "文書が変わってしまった");
            });
        }
        ui::ai::set_backend(keep);
    }

    /// **実物様式の一本通し**: 様式1(参加表明 — ラベル段落型・同じラベルが
    /// 3組ある)に名前つき記入欄を付け、fill で記入 → extract で吸い上げる。
    /// 様式や道具が無い環境では黙って飛ばす
    #[test]
    fn 実物様式1で名前付けから吸い上げまで通る() {
        let src = std::path::Path::new(
            "/mnt/sdb/home/dev/ドキュメント/機構/yoryou-yoshiki/実施要領様式1_参加表明.docx",
        );
        if !src.exists() {
            return;
        }
        let py = if std::path::Path::new("../.venv/bin/python").exists() {
            std::path::PathBuf::from("../.venv/bin/python")
        } else {
            find_python()
        };
        if !std::process::Command::new(&py)
            .args(["-c", "import docx"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return;
        }
        let bytes = std::fs::read(src).unwrap();
        let (mut doc, _) = ooxml::read(std::io::Cursor::new(bytes.clone())).unwrap();
        // ラベル段落の末尾に、節ごとの名前で記入欄を挿す
        // (「…印」で終わる段落は、印の前に挿す)
        let labels: &[(&str, &str)] = &[
            ("住所", "住所"),
            ("商号または名称", "商号"),
            ("代表者職氏名", "代表者"),
            ("所属", "所属"),
            ("氏名", "氏名"),
            ("電話番号", "電話"),
            ("E-mail", "メール"),
        ];
        let mut section = String::new();
        let mut kyoryoku = 0;
        let mut named = 0usize;
        for b in &mut doc.blocks {
            let kumihan::Block::Para(p) = b else { continue };
            let text: String = p.runs.iter().map(|r| r.text.as_str()).collect();
            let t = text.trim();
            if t.contains("代表事業者") {
                section = "代表".into();
                continue;
            }
            if t.contains("協力事業者") {
                kyoryoku += 1;
                section = format!("協力{kyoryoku}");
                continue;
            }
            if t.contains("担当者代表") {
                section = "担当".into();
                continue;
            }
            let Some((_, suffix)) =
                labels.iter().find(|(l, _)| t == *l || t.starts_with(*l)).copied()
            else {
                continue;
            };
            if section.is_empty() {
                continue;
            }
            let name = format!("{section}・{suffix}");
            let base = p.runs.first().cloned();
            let mut field = kumihan::Run {
                text: "　　　　　　　　".into(),
                size_pt: base.as_ref().and_then(|r| r.size_pt),
                font: base.as_ref().and_then(|r| r.font.clone()),
                fmt: Default::default(),
            };
            field.fmt.sdt = Some(Box::new(kumihan::Sdt {
                kind: kumihan::SdtKind::Text,
                alias: name.clone(),
                tag: name,
                items: Vec::new(),
            }));
            // 「印」で終わる段落(代表者職氏名 … 印)は印の前に挿す
            let sz = field.size_pt;
            let last = p.runs.last_mut().unwrap();
            if let Some(at) = last.text.rfind('印') {
                let tail = last.text[at..].to_string();
                last.text.truncate(at);
                p.runs.push(field);
                p.runs.push(kumihan::Run {
                    text: tail,
                    size_pt: sz,
                    font: None,
                    fmt: Default::default(),
                });
            } else {
                p.runs.push(field);
            }
            named += 1;
        }
        assert_eq!(named, 13, "名前を付けた欄の数が想定と違う: {named}");
        let dir =
            std::env::temp_dir().join(format!("jo-yoshiki1-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let named_d = dir.join("named.docx");
        let f = std::fs::File::create(&named_d).unwrap();
        ooxml::write_with(
            &doc,
            Some(std::io::Cursor::new(bytes)),
            std::io::BufWriter::new(f),
        )
        .unwrap();
        // 台本: 名前で記入し、読み戻して報告する(同名ラベル3組でも誤爆しない)
        let code = r#"fill("代表・住所", "徳島県徳島市山城町東浜傍示1番地1")
fill("代表・商号", "日本フネン株式会社")
fill("担当・氏名", "山田太郎")
fill("担当・メール", "yamada@example.jp")
assert extract("代表・商号") == "日本フネン株式会社"
assert extract("協力1・住所") .strip() == ""
print(len(fields()))
"#;
        let out_d = dir.join("out.docx");
        let py_path = dir.join("run.py");
        std::fs::write(&py_path, macro_script(&named_d, &out_d, code)).unwrap();
        let o = std::process::Command::new(&py).arg(&py_path).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let stdout = String::from_utf8_lossy(&o.stdout);
        assert_eq!(stdout.trim(), "13", "fields の数が違う: {stdout}");
        // 記入済みを writer で読み直しても、値と欄が生きている
        let (doc2, _) =
            ooxml::read(std::io::Cursor::new(std::fs::read(&out_d).unwrap())).unwrap();
        let body = doc2.body_text();
        assert!(body.contains("日本フネン株式会社"), "記入が残らない");
        assert!(body.contains("山城町東浜傍示"), "住所が残らない");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 「名前」ボタンで記入欄に名前が付く(docx の w:tag。マクロの fill の鍵)
    #[gpui::test]
    fn 記入欄に名前を付けられる(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.set_doc(Document::plain(""));
            this.run_cmd("form-text", cx); // 欄が入りカーソルは欄の直後
            this.run_cmd("form-name", cx); // 名前のパネルが開く
            assert!(this.sd_open && this.sd_naming, "名前のパネルが開かない");
            this.sd_ed = Editor::new("氏名");
            this.sd_commit();
            let tags: Vec<_> = this
                .doc
                .paragraphs()
                .flat_map(|p| &p.runs)
                .filter_map(|r| r.fmt.sdt.as_ref().map(|s| s.tag.clone()))
                .collect();
            assert!(tags.contains(&"氏名".to_string()), "名前が付かない: {tags:?}");
            // 欄の外で押すとパネルは開かず、ことばで断る
            this.set_doc(Document::plain("ただの字"));
            this.ed.move_to(0, false);
            this.run_cmd("form-name", cx);
            assert!(!this.sd_open, "欄が無いのにパネルが開く");
        });
    }
}

#[cfg(test)]
mod ruby_mark_tests {
    use crate::strip_ruby_marks;

    #[test]
    fn ルビ記法をほどいて位置と読みが出る() {
        let (plain, rubies) = strip_ruby_marks("今日は|組版《くみはん》の話。", 0);
        assert_eq!(plain, "今日は組版の話。");
        assert_eq!(rubies.len(), 1);
        let (r, yomi) = &rubies[0];
        assert_eq!(yomi, "くみはん");
        assert_eq!(&plain[r.clone()], "組版", "位置がずれている");
    }

    #[test]
    fn 全角の縦棒も受ける() {
        let (plain, rubies) = strip_ruby_marks("｜漢字《かんじ》です", 0);
        assert_eq!(plain, "漢字です");
        assert_eq!(rubies[0].1, "かんじ");
    }

    #[test]
    fn 差し込む先の頭からの位置になる() {
        let (_, rubies) = strip_ruby_marks("|漢字《かんじ》", 100);
        assert_eq!(rubies[0].0.start, 100, "base が効いていない");
    }

    #[test]
    fn 記法が壊れていても本文を落とさない() {
        for src in ["|語《よみ", "ただの|棒", "《よみ》だけ", "|"] {
            let (plain, _) = strip_ruby_marks(src, 0);
            for c in src.chars().filter(|c| !"|｜".contains(*c)) {
                assert!(plain.contains(c), "字が落ちた: {src} → {plain}");
            }
        }
    }
}

#[cfg(test)]
mod url_tests {
    use crate::resolve_url;

    #[test]
    fn 相対urlが今の場所から解ける() {
        let b = "http://ex.jp/a/b.html";
        assert_eq!(resolve_url(b, "c.html"), "http://ex.jp/a/c.html");
        assert_eq!(resolve_url(b, "/x/y"), "http://ex.jp/x/y");
        assert_eq!(resolve_url(b, "https://o.jp/z"), "https://o.jp/z");
        assert_eq!(resolve_url(b, "//o.jp/z"), "http://o.jp/z");
        assert_eq!(resolve_url("http://ex.jp", "p.html"), "http://ex.jp/p.html");
    }
}

#[cfg(test)]
mod wiring_tests {
    #[test]
    fn リボンのreadyは全部配線されている() {
        // 「押せるのに何も起きない」を仕組みで止める
        for tab in ui::ribbon::WRITER {
            for cmd in tab.cmds {
                if cmd.ready {
                    assert!(
                        crate::Writer::HANDLED.contains(&cmd.id),
                        "「{}」({}) は ready なのに run_cmd が知らない",
                        cmd.label, cmd.id
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod page_setup_tests {
    use crate::*;

    #[test]
    fn 用紙の変更が保存で残る() {
        // 画面で変えただけで docx に書かれないなら、それは飾り
        let mut d = Document::plain("本文");
        let mut pg = kumihan::PageSetup::default();
        std::mem::swap(&mut pg.w_mm, &mut pg.h_mm); // 横向き
        d.page = Some(pg);
        d.sect_raw = Some(format!(
            "<w:sectPr><w:pgSz w:w=\"{}\" w:h=\"{}\" w:orient=\"landscape\"/>\
             <w:pgMar w:top=\"1134\" w:right=\"1134\" w:bottom=\"1134\" w:left=\"1134\"/></w:sectPr>",
            (pg.w_mm * 56.6929) as i64, (pg.h_mm * 56.6929) as i64));
        let mut buf = Vec::new();
        ooxml::write(&d, std::io::Cursor::new(&mut buf)).unwrap();
        let (back, _) = ooxml::read(std::io::Cursor::new(&buf)).unwrap();
        let bp = back.page.expect("用紙が消えた");
        assert!(bp.w_mm > bp.h_mm, "横向きが消えた: {}×{}", bp.w_mm, bp.h_mm);
    }

    #[test]
    fn ヘッダーの参照は用紙を変えても残る() {
        // set_page は pgSz/pgMar だけ作り替え、他は原文から引き継ぐ
        let raw = r#"<w:sectPr><w:headerReference r:id="rId8"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134"/></w:sectPr>"#;
        // set_page 内の引き継ぎと同じ処理を直接なぞる
        let mut out = String::new();
        let mut skip = false;
        for part in raw.split_inclusive('>') {
            let t = part.trim_start();
            if t.starts_with("<w:sectPr") || t.starts_with("</w:sectPr") {
                continue;
            }
            if t.starts_with("<w:pgSz") || t.starts_with("<w:pgMar") {
                skip = !part.trim_end().ends_with("/>");
                continue;
            }
            if skip {
                continue;
            }
            out.push_str(part);
        }
        assert!(out.contains("headerReference"), "ヘッダーの参照が落ちた: {out}");
        assert!(!out.contains("pgSz"), "古い用紙が残った: {out}");
    }
}

#[cfg(test)]
mod lock_tests {
    use crate::*;

    #[test]
    fn ロックの置き場所と先客の判定() {
        let dir = std::env::temp_dir().join(format!("jolock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let doc = dir.join("文書.docx");
        std::fs::write(&doc, b"x").unwrap();
        let lp = lock_path_for(&doc);
        assert!(lp.file_name().unwrap().to_string_lossy().starts_with(".~lock."),
            "LibreOffice と同じ場所でない: {lp:?}");
        // 先客のロック
        std::fs::write(&lp, "花子@dev2,;").unwrap();
        assert_eq!(foreign_lock(&doc).as_deref(), Some("花子@dev2"), "先客が見えない");
        // 自分のロックは先客と見ない
        std::fs::write(&lp, format!("{},;", lock_identity())).unwrap();
        assert_eq!(foreign_lock(&doc), None, "自分を先客と見た");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod track_tests {
    use crate::*;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn 変わった段落と増えた段落が分かる() {
        let base = v(&["一", "二", "三"]);
        let cur = v(&["一", "二を直した", "追加", "三"]);
        let (marks, deleted) = track_diff(&base, &cur);
        assert_eq!(marks[0], PMark::Same);
        assert_eq!(marks[1], PMark::Changed(1), "変わった段落が組みにならない");
        assert_eq!(marks[2], PMark::New, "増えた段落が新規にならない");
        assert_eq!(marks[3], PMark::Same);
        assert!(deleted.is_empty());
    }

    #[test]
    fn 消えた段落は次の段落の前に付く() {
        let base = v(&["一", "二", "三"]);
        let cur = v(&["一", "三"]);
        let (marks, deleted) = track_diff(&base, &cur);
        assert_eq!(marks, vec![PMark::Same, PMark::Same]);
        assert_eq!(deleted, vec![(1, 1)], "消えた段落の場所が違う");
    }

    #[test]
    fn 文字の差分は頭と尻尾を残す() {
        let (pre, del, ins, suf) = split_diff("防火戸の仕様", "防火ドアの仕様");
        assert_eq!((pre.as_str(), del.as_str(), ins.as_str(), suf.as_str()),
            ("防火", "戸", "ドア", "の仕様"));
        let (pre, del, ins, suf) = split_diff("同じ", "同じ");
        assert_eq!((pre.as_str(), del.as_str(), ins.as_str(), suf.as_str()),
            ("同じ", "", "", ""));
    }
}

#[cfg(test)]
mod word_tests {
    use crate::*;

    #[test]
    fn 英語は空白と語の境で止まる() {
        let t = "hello world  foo";
        assert_eq!(word_boundary(t, 0, true), 6, "次の語の頭に行かない");
        assert_eq!(word_boundary(t, 6, true), 13);
        assert_eq!(word_boundary(t, 13, false), 6, "前の語の頭に戻らない");
        assert_eq!(word_boundary(t, 6, false), 0);
        assert_eq!(word_boundary(t, t.len(), true), t.len(), "末尾で止まらない");
    }

    #[test]
    fn 日本語は文字種の変わり目で止まる() {
        // 漢字の連なり→ひらがな→カタカナ→英数、の境で切れる
        let t = "防火戸のカタログをPDFで";
        let b = |s: &str| t.find(s).unwrap();
        assert_eq!(word_boundary(t, 0, true), b("の"), "漢字の連なりを1語にしない");
        assert_eq!(word_boundary(t, b("の"), true), b("カタログ"));
        assert_eq!(word_boundary(t, b("カタログ"), true), b("を"),
            "カタカナの連なりが1語にならない");
        assert_eq!(word_boundary(t, b("PDF"), false), b("を"));
    }

    #[test]
    fn 端で壊れない() {
        assert_eq!(word_boundary("", 0, true), 0);
        assert_eq!(word_boundary("", 0, false), 0);
        assert_eq!(word_boundary("あ", 0, false), 0);
    }
}

#[cfg(test)]
mod image_px_tests {
    use crate::*;

    #[test]
    fn pngの画素数が読める() {
        // 署名 + IHDR(幅640, 高さ480)
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        b.extend_from_slice(&[0, 0, 0, 13]);
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&640u32.to_be_bytes());
        b.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(image_px(&b), Some((640, 480)));
    }

    #[test]
    fn jpegの画素数が読める() {
        // SOI + APP0(空) + SOF0(高さ300, 幅200)
        let mut b = vec![0xFF, 0xD8];
        b.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x02]); // APP0 長さ2(中身なし)
        b.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
        b.extend_from_slice(&300u16.to_be_bytes()); // 高さ
        b.extend_from_slice(&200u16.to_be_bytes()); // 幅
        b.extend_from_slice(&[0x03, 0x01, 0x01, 0x00]);
        assert_eq!(image_px(&b), Some((200, 300)), "SOF0 の(幅, 高さ)が読めない");
    }

    #[test]
    fn 画像でないものは断る() {
        assert_eq!(image_px(b"not an image"), None);
    }
}

/// 図表番号の頭は、**貼る字と探す字が同じ雛形から出ている**か。
///
/// 番号を付けるのは `ui::tf!("figure", n)`、次の番号を決めるのと図表目次を
/// 作るのは段落の頭の照合。雛形は訳されるので、探す側に生の「図 」を書くと
/// 日本語以外では一度も当たらず、図がすべて 1 番になり目次も空になる
/// (2026-08-10 に見つけた)。二つを [`crate::caption_head`] に寄せたので、
/// ここではその一致だけを見張る
#[cfg(test)]
mod caption_head_tests {
    #[test]
    fn 図表番号は貼る字と探す字が同じ雛形から出る() {
        let head = crate::caption_head();
        assert!(!head.is_empty(), "頭が空だと strip_prefix が全段落に当たる");
        let label = ui::tf!("figure", 7);
        assert!(
            label.starts_with(head),
            "貼る字「{label}」が探す頭「{head}」で始まらない — 番号を数え直せない"
        );
        assert_eq!(
            label.strip_prefix(head).map(|r| r.trim().parse::<usize>()),
            Some(Ok(7)),
            "頭を外した残りが番号として読めない"
        );
    }

    /// ja では**1バイトも変わらない**(いままでの文書が読めなくならないこと)。
    /// ja かどうかは tr が鍵をそのまま返すかで見る(表に無い言語も同じ扱い)
    #[test]
    fn 日本語のときの頭はこれまでと同じ() {
        if ui::tr("図 {}") == "図 {}" {
            assert_eq!(crate::caption_head(), "図 ");
        }
    }
}

/// 画面の脚注が**紙と同じ割り当て**になっているか。
///
/// 画面は長らく `paper::paginate`(頁の位置だけ)を別に呼んでいた。
/// 脚注はその頁の本文の底を上げるので、別々に数えると
/// **画面と PDF で脚注の出る頁が食い違う**。同じ `paginate_full` から
/// 受け取っていることを、値そのもので突き合わせる。
/// **現物には依らせない**(corpus は取り直す物なので、型紙は手で組む)
#[cfg(test)]
mod screen_note_tests {
    use crate::*;

    fn 脚注のある文書() -> Document {
        let 字 = |t: &str| kumihan::Run {
            text: t.into(), size_pt: None, font: None, fmt: Default::default() };
        let 印 = kumihan::Run {
            text: String::new(), size_pt: None, font: None,
            fmt: kumihan::CharFormat {
                footnote: Some(kumihan::FootnoteRef { id: "2".into(), endnote: false }),
                ..Default::default() } };
        let 長文 = "いろはにほへとちりぬるを。".repeat(120);
        let mut d = Document::plain("");
        d.blocks = vec![kumihan::Block::Para(kumihan::Paragraph {
            runs: vec![字(&長文), 印],
            line_spacing: 1.0,
            ..Default::default()
        })];
        d.footnotes = vec![kumihan::Footnote {
            id: "2".into(), endnote: false,
            paragraphs: vec![kumihan::Paragraph {
                runs: vec![字("これは脚注の文章。")],
                line_spacing: 1.0,
                ..Default::default() }],
            added: false,
        }];
        d
    }

    #[gpui::test]
    fn 画面と紙で脚注の出る頁が同じ(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.doc = 脚注のある文書();
            this.relayout();

            assert!(!this.page.notes.is_empty(), "脚注が組まれていない");
            // 画面が持っている割り当てと、紙が出す割り当てが同じであること
            let 紙 = paper::paginate_full(&this.page, paper::Paper {
                width_mm: this.pg.w_mm,
                height_mm: this.pg.h_mm,
                margin_mm: this.pg.left_mm,
            });
            assert_eq!(this.page_notes, 紙.notes,
                "画面と紙で脚注の割り当てが違う");
            assert_eq!(this.page_offsets, 紙.offsets,
                "画面と紙で頁の切れ目が違う");
            assert!(this.page_notes.iter().any(|v| !v.is_empty()),
                "どの頁にも脚注が付いていない");
        });
    }

    /// 脚注が無ければ今までどおり(画面の割り当ても空)
    #[gpui::test]
    fn 脚注が無ければ画面も空のまま(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.doc = Document::plain("本文だけ");
            this.relayout();
            assert!(this.page.notes.is_empty(), "脚注が無いのに組まれた");
            assert!(this.page_notes.iter().all(|v| v.is_empty()),
                "脚注が無いのに頁に付いた");
        });
    }
}


/// 脚注にする操作の取り消し。
///
/// **前は戻せなかった。** 取り消しが `Editor` の平文しか見ておらず、
/// 字を模型(注)へ移すこの操作は巻き戻せなかった。文書ごとの取り消しに
/// 繋いだので(2026-08-13)、いまは本文も注も一手で戻る。
///
/// **Ctrl+Z の道(`undo_step`)で見る。** 前はここが `ed.undo()` を見ていて、
/// 繋ぎ換えても**落ちなかった** — 下の層を見ていたので、直った瞬間に
/// 気づけなかった。使い手の押す道で見る
#[cfg(test)]
mod footnote_undo_tests {
    use crate::*;

    #[gpui::test]
    fn 脚注は一手で戻る(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.doc = Document::plain("あいうえお");
            this.ed = Editor::new(&this.doc.body_text());
            this.relayout();
            // 1手打っておく(打鍵の道は before_edit を通る)
            this.before_edit(true);
            this.ed.insert("か");
            this.on_edited();
            assert_eq!(this.doc.body_text(), "あいうえおか");

            this.ed.move_to(3, false);
            this.ed.move_to(9, true);
            this.run_cmd("footnote", cx);
            assert_eq!(this.doc.body_text(), "あえおか", "字が注へ移っていない");
            assert_eq!(this.doc.footnotes.len(), 1);

            // **戻せる**(2026-08-13、文書ごとの取り消しに繋いだ)
            this.undo_step();
            assert_eq!(this.doc.body_text(), "あいうえおか", "脚注が取り消せない");
            assert!(this.doc.footnotes.is_empty(), "本文は戻ったのに注が残った");
            // やり直しも効く
            this.redo_step();
            assert_eq!(this.doc.body_text(), "あえおか");
            assert_eq!(this.doc.footnotes.len(), 1);
        });
    }

    /// 選択が無ければ何も起きない(履歴も消さない)
    #[gpui::test]
    fn 選択が無ければ履歴を壊さない(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.doc = Document::plain("あいうえお");
            this.ed = Editor::new(&this.doc.body_text());
            this.relayout();
            this.before_edit(true);
            this.ed.insert("か");
            this.on_edited();
            this.ed.move_to(3, false); // 選択なし
            this.run_cmd("footnote", cx);
            assert!(this.doc.footnotes.is_empty(), "選択が無いのに注ができた");
            this.undo_step();
            assert_eq!(this.doc.body_text(), "あいうえお", "何もしていないのに履歴が壊れた");
        });
    }
}

/// **文書ごとの取り消し。**
///
/// 前は「平文の取り消し(`Editor`)」と「マクロ用に文書を1枚控える」の
/// 二本立てで、**書式を変える操作はどちらにも乗っていなかった** —
/// 太字も揃えも Ctrl+Z で戻らなかった(2026-08-13 に測って分かった)。
/// 一本にまとめ、打鍵も書式も同じ山へ積む。
#[cfg(test)]
mod doc_undo_tests {
    use crate::*;

    fn 開く(cx: &mut gpui::TestAppContext) -> gpui::Entity<Writer> {
        cx.update(|cx| cx.new(|cx| Writer::new(None, cx)))
    }
    fn 太字(w: &Writer) -> bool {
        w.doc.paragraphs().next().unwrap().runs.iter().any(|r| r.fmt.bold)
    }

    #[gpui::test]
    fn 太字が取り消せる(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, cx| {
            this.doc = Document::plain("あいうえお");
            this.ed = Editor::new(&this.doc.body_text());
            this.relayout();
            this.ed.move_to(0, false);
            this.ed.move_to(9, true);
            this.run_cmd("bold", cx);
            assert!(太字(this), "太字がかかっていない");

            this.undo_step();
            assert!(!太字(this), "取り消しても太字のまま");

            this.redo_step();
            assert!(太字(this), "やり直しても太字が戻らない");
        });
    }

    /// **一本の履歴。** 打鍵と書式が同じ山に積まれ、新しい順に戻る。
    /// 二本立てのままだと「打ってから太字」の後の Ctrl+Z が
    /// 打鍵のほうを戻してしまい、使い手に順が読めない
    #[gpui::test]
    fn 打鍵と書式が同じ順で戻る(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, cx| {
            this.doc = Document::plain("あいうえお");
            this.ed = Editor::new(&this.doc.body_text());
            this.relayout();

            // 1手目: 打鍵
            this.before_edit(true);
            this.ed.move_to(15, false);
            this.ed.insert("か");
            this.on_edited();
            assert_eq!(this.doc.body_text(), "あいうえおか");

            // 2手目: 太字
            this.ed.move_to(0, false);
            this.ed.move_to(9, true);
            this.run_cmd("bold", cx);
            assert!(太字(this));

            // 戻す → **新しい順**。まず太字が外れ、字は残る
            this.undo_step();
            assert!(!太字(this), "先に打鍵が戻った(順が違う)");
            assert_eq!(this.doc.body_text(), "あいうえおか", "字まで戻った");

            // もう一手 → 打鍵が戻る
            this.undo_step();
            assert_eq!(this.doc.body_text(), "あいうえお", "打鍵が戻らない");

            // やり直しは逆順
            this.redo_step();
            assert_eq!(this.doc.body_text(), "あいうえおか");
            this.redo_step();
            assert!(太字(this), "やり直しで太字が戻らない");
        });
    }

    /// 続けて打った分は**1手にまとめる**(1字ごとに戻らない)
    #[gpui::test]
    fn 続けた打鍵は一手にまとまる(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, _cx| {
            this.doc = Document::plain("");
            this.ed = Editor::new("");
            this.relayout();
            for c in ["あ", "い", "う"] {
                this.before_edit(true);
                this.ed.insert(c);
                this.on_edited();
            }
            assert_eq!(this.doc.body_text(), "あいう");
            this.undo_step();
            assert_eq!(this.doc.body_text(), "", "続けた打鍵が1手になっていない");
        });
    }

    /// 戻したあとに打つと、やり直しの先は捨てる(枝分かれしない)
    #[gpui::test]
    fn 戻したあとに打つとやり直しは消える(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, cx| {
            this.doc = Document::plain("あ");
            this.ed = Editor::new("あ");
            this.relayout();
            this.ed.move_to(0, false);
            this.ed.move_to(3, true);
            this.run_cmd("bold", cx);
            this.undo_step();
            assert!(!太字(this));
            // ここで別の一手
            this.before_edit(true);
            this.ed.move_to(3, false);
            this.ed.insert("い");
            this.on_edited();
            this.redo_step();
            assert!(!太字(this), "捨てたはずのやり直しが効いた");
            assert_eq!(this.doc.body_text(), "あい");
        });
    }
}




/// **文書を変える命令は、押した一手で戻せる。**
///
/// 「押せるのに何も起きない」を `wiring_tests` が止めるのと同じ形で、
/// 「変えたのに戻せない」をここで止める。命令を足したときに控えを
/// 入れ忘れても、この試験が落ちる。
///
/// 実際に2つ見つけた(2026-08-13): **脚注**と**縦書き**。どちらも
/// 文書を変えるのに控えを取っていなかった。
#[cfg(test)]
mod undo_coverage_tests {
    use crate::*;

    /// 文書の姿を1本の字にする(戻ったかを見るための指紋)。
    /// **見る欄が足りないと、戻っていなくても気づけない** — 縦書きは
    /// 最初この指紋に入っておらず、取りこぼしを隠していた
    fn sig(w: &Writer) -> String {
        let mut out = String::new();
        out.push_str(&w.doc.body_text());
        out.push_str(&format!("|blocks={} notes={} vert={} ",
            w.doc.blocks.len(), w.doc.footnotes.len(), w.doc.vertical as u8));
        for p in w.doc.paragraphs() {
            out.push_str(&format!(
                "[a={:?} l={:?} i={} s={:.2} st={:?} pb={}",
                p.align, p.list, p.indent, p.line_spacing, p.style,
                p.page_break_before as u8));
            for r in &p.runs {
                out.push_str(&format!(
                    " (b{} i{} u{} s{} sup{} sub{} sz{:?} c{:?} h{:?} fn{})",
                    r.fmt.bold as u8, r.fmt.italic as u8, r.fmt.underline as u8,
                    r.fmt.strike as u8, r.fmt.superscript as u8, r.fmt.subscript as u8,
                    r.size_pt, r.fmt.color, r.fmt.highlight, r.fmt.footnote.is_some() as u8));
            }
            out.push(']');
        }
        out.push_str(&format!("|pg={:?}", w.doc.page));
        out
    }

    /// 試験の形を整える。**前の命令が開いた欄は閉じる** — 欄が開いていると
    /// 控えを取らない約束なので、閉じ忘れると「戻せない」が全部に伝染する
    /// (最初これで 33 件が偽の赤になった)
    fn 仕切り直す(this: &mut Writer) {
        // **docx を開いた状態で見ます。** adoc 形式では見た目のボタンが
        // スタイルの面へ案内するだけで文書を変えないので、戻せるかを見るのは
        // こちら(2026-08-17、新規を adoc にしたときに合わせた)
        this.native = false;
        this.rp_open = false;
        this.doc = Document::plain("あいうえお\nかきくけこ");
        this.ed = Editor::new(&this.doc.body_text());
        this.pg = Default::default();
        this.undo_stack.clear();
        this.redo_stack.clear();
        this.target = Target::Body;
        this.pw_open = false; this.file_field = None; this.find_open = false;
        this.hf_edit = None; this.cmt_edit = false; this.wm_edit = false;
        this.bm_open = false; this.url_open = false; this.fm_field = None;
        this.rb_open = false; this.sd_open = false; this.ai_open = false;
        this.chat_open = false; this.tool = None;
        this.relayout();
        this.ed.move_to(0, false);
        this.ed.move_to(9, true);
    }

    /// 押さない命令と、その理由。**一覧を手で持たない**ので、
    /// ここに無い命令は全部試される — 命令を足したら自動で見張りに入る
    fn 押さない(id: &str) -> bool {
        // 窓を開ける物(試験では返ってこない)。理由は menu_run_tests と同じ
        super::menu_run_tests::DIALOG.contains(&id)
            // 外の世界に出る物
            || id.starts_with("ai-") || id.starts_with("plug-")
            // 取り消しそのもの・クリップボード・校正
            || matches!(id, "undo" | "redo" | "copy" | "cut" | "paste" | "spell")
    }

    #[gpui::test]
    fn 文書を変える命令は一手で戻せる(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        let mut 見た = 0usize;
        for id in Writer::HANDLED.iter().filter(|i| !押さない(i)) {
            w.update(cx, |this, cx| {
                仕切り直す(this);
                let before = sig(this);
                this.run_cmd(id, cx);
                if sig(this) == before {
                    return; // この形では文書を変えない命令(欄を開くだけ等)
                }
                見た += 1;
                this.undo_step();
                assert_eq!(sig(this), before,
                    "「{id}」が一手で戻らない(控えを取っていない)");
            });
        }
        // **数えておく。** 形が壊れて「どれも文書を変えない」になったら、
        // 中身が空でも緑になってしまう。
        // **39 になったのは 2026-08-25** — 表の挿入と日付の挿入が、
        // 押すとマス目や一覧が開くだけの形になり、その場では文書を変えなく
        // なったためです(挿すのは `instable-go` と一覧から選んだとき)
        assert!(見た >= 39, "文書を変える命令が {見た} 件しか無い — 試験の形が壊れている");
    }

    /// やり直しも効く(戻すだけで進めないと片道になる)
    #[gpui::test]
    fn 戻した一手はやり直せる(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        for id in Writer::HANDLED.iter().filter(|i| !押さない(i)) {
            w.update(cx, |this, cx| {
                仕切り直す(this);
                let before = sig(this);
                this.run_cmd(id, cx);
                let after = sig(this);
                if after == before {
                    return;
                }
                this.undo_step();
                this.redo_step();
                assert_eq!(sig(this), after, "「{id}」がやり直せない");
            });
        }
    }
}




/// 印刷モード(紙を1枚ずつ積む)。
///
/// **見た目は私には確かめられない**(この機械では GPUI が起動しない)。
/// ここで守れるのは数字だけ — 画面が使う紙が紙(PDF)と同じであること、
/// 字がその頁の紙の中に収まっていること。
#[cfg(test)]
mod paged_view_tests {
    use crate::*;

    fn 開く(cx: &mut gpui::TestAppContext) -> gpui::Entity<Writer> {
        cx.update(|cx| cx.new(|cx| Writer::new(None, cx)))
    }

    #[gpui::test]
    fn 節ごとに紙が変わる(cx: &mut gpui::TestAppContext) {
        let f = std::path::PathBuf::from("/home/dev/docx-corpus/all3.docx");
        if !f.exists() {
            return;
        }
        let w = 開く(cx);
        w.update(cx, |this, _| {
            this.open(f);
            this.paged = true;
            this.relayout();
            let sizes: Vec<(f32, f32)> = this.page_papers.iter()
                .map(|q| (q.width_mm.round(), q.height_mm.round())).collect();
            assert!(sizes.len() >= 2, "頁が2つ以上無い: {sizes:?}");
            assert_ne!(sizes[0], sizes[1], "頁ごとに紙が変わっていない: {sizes:?}");
            let widest = sizes.iter().map(|s| s.0).fold(0.0f32, f32::max);
            assert!(this.paper_w_mm() >= widest - 0.5,
                "広い紙がはみ出す: 紙の幅 {} < {widest}", this.paper_w_mm());
        });
    }

    #[gpui::test]
    fn 画面と紙で頁ごとの紙が同じ(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, _| {
            this.doc = Document::plain(&"いろはにほへとちりぬるを。".repeat(400));
            this.ed = Editor::new(&this.doc.body_text());
            this.paged = true;
            this.relayout();
            let kami = paper::paginate_full(&this.page, paper::Paper {
                width_mm: this.pg.w_mm,
                height_mm: this.pg.h_mm,
                margin_mm: this.pg.left_mm,
            });
            assert_eq!(this.page_papers.len(), kami.papers.len(), "頁の数が違う");
        });
    }

    #[gpui::test]
    fn 字がその頁の紙の中に収まる(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, _| {
            this.doc = Document::plain(&"いろはにほへとちりぬるを。".repeat(400));
            this.ed = Editor::new(&this.doc.body_text());
            this.paged = true;
            this.relayout();
            assert!(this.page_tops.len() >= 3, "頁が足りず試験にならない");
            let mita = this.page.lines.iter().filter(|l| !l.cells.is_empty()).count();
            assert!(mita > 0, "行が無い");
            for l in this.page.lines.iter().filter(|l| !l.cells.is_empty()) {
                let k = this.page_tops.iter().rposition(|t| l.y_mm >= *t - 0.01).unwrap_or(0);
                let top = this.page_tops[k];
                let h = this.page_papers.get(k).map(|q| q.height_mm).unwrap_or(this.pg.h_mm);
                assert!(l.y_mm >= top - 0.5 && l.y_mm <= top + h + 0.5,
                    "字が紙の外: y={} 頁{k} は {top}..{}", l.y_mm, top + h);
            }
        });
    }

    /// **挿した絵は一度だけ描かれる。** 組版は images(読み込んだ絵)と
    /// images_new(このアプリで足した絵)の**両方**を描くので、足すときに
    /// 両方へ入れると画面と紙で二重になる — 2026-08-13 に数式で踏み、
    /// 画像の挿入にも同じ形があった(7feb1e6)。両方の道をここで縛る
    #[gpui::test]
    fn 挿した絵は一度だけ描かれる(cx: &mut gpui::TestAppContext) {
        // 1x1 の PNG(この試験のためだけ。読めればよい)
        const PNG: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
            0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00,
            0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
            0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
            0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let dir = std::env::temp_dir().join("officework-image-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("ten.png");
        std::fs::write(&p, PNG).unwrap();

        let w = 開く(cx);
        w.update(cx, |this, _| {
            this.set_doc(Document::plain("本文"));
            this.insert_image(&p);
            // 模型の側: 足した絵は images_new にだけ居る
            let (old, new) = this
                .doc
                .paragraphs()
                .map(|q| (q.images.len(), q.images_new.len()))
                .fold((0usize, 0usize), |a, b| (a.0 + b.0, a.1 + b.1));
            assert_eq!(new, 1, "足した絵が images_new に無い");
            assert_eq!(old, 0, "足した絵を images にも入れている(二重描画の元)");
            // 組版の側: 紙に出るのは一度だけ
            this.relayout();
            assert_eq!(this.page.images.len(), 1,
                "紙に {} 回描かれた(一度であるべき)", this.page.images.len());
        });
        let _ = std::fs::remove_file(&p);
    }

    /// 押す口が実際に効くこと。**機能があってもボタンが繋がっていなければ
    /// 誰にも届かない** — ここが無いと配線の切れに気づけない
    #[gpui::test]
    fn 印刷レイアウトの釦で切り替わる(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, cx| {
            assert!(!this.paged, "既定が印刷レイアウトになっている");
            this.run_cmd("printview", cx);
            assert!(this.paged, "押しても印刷レイアウトにならない");
            this.run_cmd("printview", cx);
            assert!(!this.paged, "もう一度押しても戻らない");
        });
    }

    /// 画面だけの折り方どうし、両立させない(どちらを押しても他方が下りる)
    #[gpui::test]
    fn 印刷レイアウトと見開きは排他(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, cx| {
            this.run_cmd("multipage", cx);
            assert!(this.multipage && !this.paged, "見開きにならない");
            this.run_cmd("printview", cx);
            assert!(this.paged && !this.multipage, "印刷レイアウトで見開きが下りない");
            this.run_cmd("multipage", cx);
            assert!(this.multipage && !this.paged, "見開きで印刷レイアウトが下りない");
        });
    }

    /// 縦書きは初版の約束で断る。**黙って何もしないのではなく、言って断る**
    #[gpui::test]
    fn 縦書きでは印刷レイアウトにしない(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, cx| {
            this.doc.vertical = true;
            this.run_cmd("printview", cx);
            assert!(!this.paged, "縦書きなのに印刷レイアウトにした");
            assert!(!this.status.is_empty(), "断ったことを言っていない");
        });
    }

    /// 数式の口。**組むのは Python** なので、ここで見るのは配線と断り方だけ —
    /// 組んだ絵の良し悪しは実機と officework.tex の検査(test_tex.py)の持ち場
    #[gpui::test]
    fn 数式の釦でパネルが開く(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, cx| {
            assert!(!this.eq_open, "はじめから開いている");
            this.run_cmd("insequation", cx);
            assert!(this.eq_open, "押してもパネルが開かない");
            assert!(this.eq_ed.text().is_empty(), "前の字が残っている");
            // 取りやめても何も置かない
            this.eq_open = false;
            let n: usize = this.doc.paragraphs().map(|p| p.images_new.len()).sum();
            assert_eq!(n, 0, "開いただけで絵を置いた");
        });
    }

    /// 空で Enter は**何も起きない**(空の絵を置かない)
    #[gpui::test]
    fn 空の数式は置かない(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, _| {
            this.set_doc(Document::plain("本文"));
            this.eq_open = true;
            this.eq_ed = Editor::new("   ");
            this.eq_commit();
            assert!(!this.eq_open, "パネルが閉じない");
            let n: usize = this.doc.paragraphs().map(|p| p.images_new.len()).sum();
            assert_eq!(n, 0, "空なのに絵を置いた");
        });
    }

    /// 組めない式は**黙って何も起きない、をしない**。理由を状態行に出す
    #[gpui::test]
    fn 組めない数式は理由を言う(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, _| {
            this.set_doc(Document::plain("本文"));
            this.eq_open = true;
            this.eq_ed = Editor::new(r"\begin{tikzpicture}x\end{tikzpicture}");
            this.eq_commit();
            let n: usize = this.doc.paragraphs().map(|p| p.images_new.len()).sum();
            assert_eq!(n, 0, "組めないのに絵を置いた");
            assert!(!this.status.is_empty(), "断ったことを言っていない");
        });
    }

    #[gpui::test]
    fn 編集モードは折らない(cx: &mut gpui::TestAppContext) {
        let w = 開く(cx);
        w.update(cx, |this, _| {
            this.doc = Document::plain(&"いろはにほへとちりぬるを。".repeat(400));
            this.ed = Editor::new(&this.doc.body_text());
            this.relayout();
            assert!(!this.paged, "既定が印刷モードになっている");
            assert_eq!(this.page_tops, vec![0.0], "編集モードなのに折った");
            let d = this.page_offsets.windows(2).map(|w| w[1] - w[0]).next().unwrap_or(0.0);
            assert!(d > 0.0 && d < this.pg.h_mm,
                "編集モードの頁の間隔が紙の高さになっている: {d}");
        });
    }
}

/// リボンのボタンの印(▾=一覧 / …=小窓 / 無印=すぐ効く)の見張り
#[cfg(test)]
mod marker_tests {
    use crate::*;

    #[test]
    fn 印の一覧は互いに素でリボンに実在する() {
        // ▾(一覧)と …(小窓)の両方に居る id は印が決められない。
        // リボンの表に無い id は印の付けようが無い
        let ribbon_ids: std::collections::HashSet<&str> = ui::ribbon::WRITER
            .iter()
            .flat_map(|t| t.cmds.iter().map(|c| c.id))
            .collect();
        for id in Writer::MENU_IDS {
            assert!(
                !Writer::DIALOG_IDS.contains(id),
                "{id} が一覧(▾)と小窓(…)の両方に居る"
            );
            assert!(ribbon_ids.contains(id), "{id}(▾)がリボンの表に無い");
        }
        for id in Writer::DIALOG_IDS {
            assert!(ribbon_ids.contains(id), "{id}(…)がリボンの表に無い");
        }
    }

    #[gpui::test]
    fn 小窓の印のボタンは小窓の旗を立てる(cx: &mut gpui::TestAppContext) {
        // DIALOG_IDS(…)を1つずつ叩き、dialog_open() が真になる物の数の
        // 下限を見る。**1件ずつの強制はしない** — 前提の要る id
        // (form-name は記入欄の中でしか開かない)があるため。
        // 旗が立ったのに dialog_open() が偽なら、印と無効化がずれている
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            let mut seen = 0;
            for id in Writer::DIALOG_IDS {
                this.set_doc(Document::plain("見出し\n本文の字。"));
                this.ed.select_all();
                this.run_cmd(id, cx);
                if this.dialog_open() {
                    seen += 1;
                }
                // 一覧が開いたら、それは … でなく ▾ の仲間
                assert!(
                    this.open_list.is_none(),
                    "{id}(…)が一覧を開いた — MENU_IDS の側では"
                );
                // 次の id のために全部畳む
                this.find_open = false;
                this.wm_edit = false;
                this.bm_open = false;
                this.cmt_edit = false;
                this.hist_open = false;
                this.plug_open = false;
                this.pw_open = false;
                this.sd_open = false;
                this.ai_open = false;
                this.rb_open = false;
                this.eq_open = false;
                this.chat_open = false;
            }
            // 11 件。暗号化を掛けるボタンを 2026-08-18 に外して1つ減った
            assert!(seen >= 11, "小窓が開いた命令が {seen} 件しかない — 見張りになっていない");
        });
    }

    #[gpui::test]
    fn 一覧は他のボタンを押すと閉じて操作は効く(cx: &mut gpui::TestAppContext) {
        // 一覧(▾)の閉じ方の約束: 他のボタンを押すと畳まれ、押した操作は
        // そのまま効く
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            // 一覧は docx のときに開く(adoc ではスタイルの面へ案内する)
            this.native = false;
            this.set_doc(Document::plain("本文の字。"));
            this.ed.select_all();
            this.run_cmd("fontsize", cx);
            assert_eq!(this.open_list, Some("fontsize"), "fontsize の一覧が開いていない(前提が崩れた)");
            this.run_from_ribbon("bold", cx);
            assert!(this.open_list.is_none(), "他のボタンを押しても一覧が畳まれない");
            assert!(
                this.doc.char_format_at(this.ed.selection()).bold,
                "畳んだだけで太字が効いていない — 押した操作はそのまま効く約束"
            );
        });
    }

    #[gpui::test]
    fn 小窓中はリボンが効かない(cx: &mut gpui::TestAppContext) {
        // 小窓(…)が開いている間はリボン全体が無効。閉じる道(Esc・
        // 小窓の中のボタン)は run_cmd 直呼びなので今のまま
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.set_doc(Document::plain("本文の字。"));
            this.ed.select_all();
            this.run_cmd("replace", cx);
            assert!(this.dialog_open(), "置き換えで小窓が開いていない(前提が崩れた)");
            this.run_from_ribbon("bold", cx);
            assert!(
                !this.doc.char_format_at(this.ed.selection()).bold,
                "小窓中にリボンの bold が通った — リボンは無効の約束"
            );
            assert!(this.find_open, "弾いただけのつもりが小窓まで消えた");
        });
    }

    #[gpui::test]
    fn pyを開いて保存しても素の文字のまま(cx: &mut gpui::TestAppContext) {
        // 発注者 2026-08-14「pyedit は使うな、writer を使え」。
        // **docx に化けさせない** — 化けたら plugins から読めなくなる
        let dir = std::env::temp_dir().join(format!("ow-py-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("見本.py");
        let src = "def 税込(x):\n    return round(float(x) * 1.1)\n";
        std::fs::write(&f, src).unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(f.clone());
            // 字下げが保たれていること(Python は字下げが構文)
            assert!(this.doc.body_text().contains("    return"), "字下げが消えた");
            assert!(this.status.contains("文字だけ"), "{}", this.status);
            // そのまま保存 → 中身が変わらない
            this.save_to(f.clone());
            let back = std::fs::read_to_string(&f).unwrap();
            assert_eq!(back, src, "往復で中身が変わった");
            // zip(docx)になっていないこと
            assert!(!back.starts_with("PK"), "docx に化けている");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 素の文字の拡張子を見分ける() {
        use crate::doc::is_plain_ext;
        for e in ["py", "PY", "txt", "md", "toml", "json", "csv"] {
            assert!(is_plain_ext(e), "{e} は素の文字のはず");
        }
        for e in ["docx", "html", "xlsx", "png"] {
            assert!(!is_plain_ext(e), "{e} は素の文字ではない");
        }
    }

    /// **表の行と列を足す・消すが模型に届く。**
    ///
    /// 2026-08-15 まで writer には「3×3 を末尾に置く」しか無く、
    /// **行の足し方が無かった** — 帳票は必ず行が増えるので、右パネルに
    /// 出すと同時にここで見張る
    #[gpui::test]
    fn 表の行と列を足して消せる(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.run_cmd("instable-go", cx);
            let (r0, c0) = {
                let t = this.doc.tables().next().expect("表がある");
                (t.rows.len(), t.rows[0].len())
            };
            assert_eq!((r0, c0), (3, 3), "3×3 で始まる");
            // カーソルを表の中へ(右パネルはここでだけ表の面を出す)
            this.switch_target(Target::Cell { table: 0, row: 1, col: 1 });
            assert!(this.cursor_table().is_some(), "表の中と分からない");
            let 本文 = this.doc.body_text();

            this.table_add_row(true);
            assert_eq!(this.doc.tables().next().unwrap().rows.len(), 4, "行が増えない");
            this.table_add_col(false);
            assert_eq!(this.doc.tables().next().unwrap().rows[0].len(), 4, "列が増えない");
            this.table_del_row();
            assert_eq!(this.doc.tables().next().unwrap().rows.len(), 3, "行が減らない");
            this.table_del_col();
            assert_eq!(this.doc.tables().next().unwrap().rows[0].len(), 3, "列が減らない");
            // **本文を巻き添えにしない。** 消したあとに編集先を移すとき、
            // 手元に残ったセルの字を書き戻すと本文の1段落目が潰れる
            // (2026-08-15 実機で踏んだ)。段落の数と字が変わらないこと
            assert_eq!(
                this.doc.body_text(),
                本文,
                "行や列を消したら本文が書き換わった(セルの字が本文へ流れた)"
            );
        });
    }

    /// **最後の1行・1列は消せない**(消せると表が消えたように見える)
    #[gpui::test]
    fn 表の最後の行と列は残る(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.run_cmd("instable-go", cx);
            this.switch_target(Target::Cell { table: 0, row: 0, col: 0 });
            for _ in 0..5 {
                this.table_del_row();
            }
            assert_eq!(this.doc.tables().next().unwrap().rows.len(), 1, "最後の1行まで消えた");
            for _ in 0..5 {
                this.table_del_col();
            }
            assert_eq!(this.doc.tables().next().unwrap().rows[0].len(), 1, "最後の1列まで消えた");
            assert!(this.status.contains("最後の1列"), "断りの言葉が出ない: {}", this.status);
        });
    }

    /// **ネイティブ文書は意味だけを往復する**(2026-08-16。段階C の門番)。
    /// 開く → 打つ → 保存 → 開き直す、で意味が同じこと
    #[gpui::test]
    fn adoc_を開いて保存すると意味が往復する(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-adoc-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("見本.adoc");
        let src = "= 月次報告\n\n== まとめ\n\n**要点**だけ書く。\n";
        std::fs::write(&path, src).unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(path.clone());
            assert!(this.native, "ネイティブとして開いていない: {}", this.status);
            assert_eq!(this.doc.props.title, "月次報告");
            let ps: Vec<_> = this.doc.paragraphs().collect();
            // **表題も本文の段落**(2026-08-18)。見出しはその次
            assert_eq!(ps[0].style, kumihan::ParaStyle::Title);
            assert_eq!(ps[1].style, kumihan::ParaStyle::Heading(1));
            // **意味だけ** — 見た目は本文に入らない(見出しの 16pt は
            // テンプレートの側で、合成のときに乗る)
            assert_eq!(ps[1].runs[0].size_pt, None, "本文に見た目が焼き付いた");

            this.save_to(path.clone());
            let back = std::fs::read_to_string(&path).unwrap();
            assert_eq!(back, src, "保存で意味が崩れた");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **手描きの線は adoc の保存で SVG の絵になる**(2026-08-18)。
    /// 前は黙って消えていた。独自の書き方を足さず `image::` で置く
    #[gpui::test]
    fn 筆はadocの保存でsvgになる(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-ink-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("筆の見本.adoc");

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.set_doc(Document::plain("本文です。"));
            this.native = true;
            this.run_cmd("pen", cx);
            this.ink_begin(10.0, 10.0);
            this.ink_move(20.0, 18.0);
            this.ink_move(30.0, 12.0);
            this.ink_end();
            assert!(!this.doc.ink.is_empty(), "筆が残らない");

            this.save_to(path.clone());
            assert!(this.doc.ink.is_empty(), "保存しても筆が模型に残っている");

            let svg = dir.join("images/筆1.svg");
            assert!(svg.is_file(), "SVG が作られない: {}", this.status);
            let 中身 = std::fs::read_to_string(&svg).unwrap();
            assert!(中身.starts_with("<svg "), "SVG の形でない: {中身}");
            assert!(中身.contains("stroke=\"#1C3B52\""), "ペンの色が入らない: {中身}");

            // 本文は image:: で指す(独自の書き方を足していない)
            let adoc = std::fs::read_to_string(&path).unwrap();
            assert!(adoc.contains("image::images/筆1.svg[]"), "image:: が無い: {adoc}");
            // **黙って変えない** — 状態行で言う
            assert!(this.status.contains("SVG"), "状態行が言っていない: {}", this.status);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **見出し4・5 をボタンから掛けられる**(2026-08-18)。
    /// 掛けた段落は adoc で `=====` `======` になる
    #[gpui::test]
    fn 見出し4と5を掛けて保存できる(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-h45-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("見本.adoc");
        std::fs::write(&path, "= 題\n\n一つ目です。\n\n二つ目です。\n").unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(path.clone());
            // 「一つ目です。」の中にカーソルを置いて見出し4に
            let at = this.doc.body_text().find("一つ目").expect("本文が無い");
            this.ed.move_to(at + 3, false);
            this.set_para_style(4);
            this.save_to(path.clone());
            let back = std::fs::read_to_string(&path).unwrap();
            assert!(back.contains("===== 一つ目です。"), "見出し4 にならない:\n{back}");
            assert!(back.contains("二つ目です。"), "他の段落が変わった:\n{back}");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 筆を絵にしても、**本文の字は1文字も変わらない**
    #[gpui::test]
    fn 筆を絵にしても本文は変わらない(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-ink2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("見本.adoc");
        let src = "= 題\n\n一つ目の段落です。\n\n二つ目の段落です。\n\n三つ目の段落です。\n";
        std::fs::write(&path, src).unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.open(path.clone());
            this.run_cmd("pen", cx);
            this.ink_begin(30.0, 120.0);
            this.ink_move(60.0, 130.0);
            this.ink_end();
            this.save_to(path.clone());
            let back = std::fs::read_to_string(&path).unwrap();
            let 字 = |s: &str| s.replace("\n", "").replace(" ", "");
            let 絵ぬき: String = back
                .lines()
                .filter(|l| !l.starts_with("image::"))
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(字(&絵ぬき), 字(src), "本文が変わった:\n{back}");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **adoc から docx を書き出すと、テンプレートがスタイル定義になる**
    /// (2026-08-18)。本文の側には見た目を焼き付けない。
    /// 書き出しなので、原稿の保存先は adoc のままにする —
    /// docx に移ると、次の Ctrl+S が原稿ではなく docx を上書きする
    #[gpui::test]
    fn docxの書き出しはテンプレートを通す(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-dtmpl-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let adoc = dir.join("見本.adoc");
        std::fs::write(&adoc, "= 題\n\n== 節\n\n本文です。\n").unwrap();
        let docx = dir.join("見本.docx");

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(adoc.clone());
            assert!(this.native, "ネイティブとして開いていない");
            this.save_to(docx.clone());

            // 原稿の保存先は動かない
            assert_eq!(this.path.as_deref(), Some(adoc.as_path()),
                       "書き出しで保存先が docx に移った: {}", this.status);

            let bytes = std::fs::read(&docx).unwrap();
            let 部品 = |name: &str| {
                let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).unwrap();
                let mut f = z.by_name(name).unwrap();
                let mut s = String::new();
                std::io::Read::read_to_string(&mut f, &mut s).unwrap();
                s
            };
            // 既定のテンプレートの見出し1 は 16pt の太字
            let styles = 部品("word/styles.xml");
            assert!(styles.contains(r#"w:styleId="Heading1""#), "見出しの定義が無い");
            assert!(styles.contains(r#"<w:sz w:val="32"/>"#), "16pt が定義に入らない");
            // 本文は名乗るだけ
            let body = 部品("word/document.xml");
            assert!(body.contains(r#"w:val="Heading1""#), "pStyle が無い");
            assert!(!body.contains(r#"<w:sz w:val="32"/>"#), "本文に大きさが焼き付いた");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 開いた直後に「変更あり」の印が付かない(付くと、触っていないのに
    /// 保存を促されて、上書きの事故に繋がる)
    #[gpui::test]
    fn 開いた直後は変更ありにならない(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-adoc3-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("見本3.adoc");
        std::fs::write(&path, "== 見出し\n\n本文。\n").unwrap();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(path.clone());
            assert!(!this.dirty, "開いた直後に変更ありになった");
            this.relayout();
            assert!(!this.dirty, "組み直しただけで変更ありになった");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **ネイティブでは見た目を直に変えさせない**(2026-08-16。C-2 の門番)。
    /// 押すと名前を付ける道に入り、決めるとテンプレートへ入る
    #[gpui::test]
    fn ネイティブでは見た目がスタイルの新設になる(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-c2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("見本.adoc");
        std::fs::write(&path, ":template: 見本の型\n\n本文の字。\n").unwrap();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.open(path.clone());
            assert!(this.native);

            // 字を大きく → 直には掛からず、右のスタイルの面が開く
            this.run_cmd("incfont", cx);
            assert!(this.rp_open && this.rp_tab == 2, "スタイルの面が開かない: {}", this.status);
            let ps: Vec<_> = this.doc.paragraphs().collect();
            assert_eq!(ps[0].runs[0].size_pt, None, "直接書式が本文に入った");

            // そこから名前を付ける → **本文に名前が付くだけ**。
            // テンプレートは書き替えない(2026-08-18 発注者の決め)
            this.style_new = Some(kumihan::theme::StyleDef {
                size_pt: Some(16.0),
                ..Default::default()
            });
            this.style_ed = kumihan::Editor::new("大見出し");
            this.style_commit();
            assert!(this.style_new.is_none());
            assert!(this.tmpl.style("大見出し").is_none(), "テンプレートを書き替えた");
            assert!(this.status.to_string().contains("テンプレートにまだありません"),
                    "見た目が付かないことを言っていない: {}", this.status);
            let ps: Vec<_> = this.doc.paragraphs().collect();
            assert_eq!(ps[0].style_id.as_deref(), Some("大見出し"));
            assert_eq!(ps[0].runs[0].size_pt, None, "決めた後も本文は意味だけ");

            // **テンプレートのファイルは作られない。** 見た目を決めるのは
            // テンプレートを書く人の仕事です
            assert!(!dir.join("見本の型.toml").exists(), "テンプレートを書いた");

            // 保存しても本文は意味だけ(**スタイルの名前は載る**)
            this.save_to(path.clone());
            let back = std::fs::read_to_string(&path).unwrap();
            assert!(!back.contains("pt"), "見た目が本文に漏れた: {back}");
            // 2026-08-16 に実機で見つけた穴 — 段落のスタイル名が黙って
            // 消えていた(試験は合成しか見ていなかった)
            assert!(back.contains("[.大見出し]"), "段落のスタイル名が保存で消えた: {back}");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **押したときだけ変わる**(2026-08-16。段階D の門番)。押すまで docx は
    /// docx のまま。押すとネイティブになり、見た目はテンプレートへ移る
    #[gpui::test]
    fn adoc形式にすると本文と書式に分かれる(cx: &mut gpui::TestAppContext) {
        // **直接書式を持つ見本を選ぶ。** よく出来た docx はスタイル任せで
        // 直接書式を持たない(報告書.docx がそうだった)— この操作の効き目を
        // 見るには、泥のある物で測る
        let src = ["../sample/カタログ.docx", "../sample/議事録.docx", "../sample/送付状.docx"]
            .iter()
            .map(std::path::Path::new)
            .find(|p| p.exists());
        let Some(src) = src else {
            eprintln!("見本の docx が無いので飛ばす");
            return;
        };
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(src.to_path_buf());
            assert!(!this.native, "開いただけで変わった(押したときだけのはず)");
            let 前 = this
                .doc
                .paragraphs()
                .flat_map(|p| p.runs.iter())
                .filter(|r| r.size_pt.is_some() || r.font.is_some())
                .count();

            this.distill_now();
            assert!(this.native, "adoc 形式にならない");
            let 後 = this
                .doc
                .paragraphs()
                .flat_map(|p| p.runs.iter())
                .filter(|r| r.size_pt.is_some() || r.font.is_some())
                .count();
            assert_eq!(後, 0, "本文に見た目が残った({前} → {後})");
            let _ = 前;
            assert!(this.doc.template.is_some(), "テンプレートの名前が付かない");
            // 見た目はテンプレートの側にある
            assert!(
                this.tmpl.size_pt.is_some() || !this.tmpl.styles.is_empty(),
                "テンプレートが空(見た目がどこにも行っていない)"
            );
            // 二度押しは断る
            this.distill_now();
            assert!(this.status.contains("もう adoc"), "二度目を断らない: {}", this.status);
        });
    }

    /// **大きさはテンプレートで決める。** writer は本文を書く道具で、
    /// 見た目はテンプレートを書く人の仕事です(発注者 2026-08-18
    /// 「テンプレートの編集はできないと割り切ったほうがいいのでは」)。
    /// 押しても黙って何も起きない、にはしません — どのファイルを直せばよいかを言います。
    #[gpui::test]
    fn 大きさはテンプレートで決めると言う(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-c3-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("テンプレート.toml"), "[スタイル.見出し1]\n大きさ = 16\n").unwrap();
        let path = dir.join("見本.adoc");
        std::fs::write(&path, "== ひとつめ\n\n本文。\n").unwrap();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(path.clone());
            let 前 = this.tmpl.style("見出し1").and_then(|d| d.size_pt);
            this.ed.move_to(0, false);
            this.tweak_style(1);
            assert_eq!(this.tmpl.style("見出し1").and_then(|d| d.size_pt), 前,
                       "テンプレートを書き替えた");
            let s = this.status.to_string();
            assert!(s.contains("テンプレート"), "言い分がない: {s}");
            assert!(s.contains("テンプレート.toml"), "直すファイルを言っていない: {s}");
        });
        // ファイルも変わっていない
        assert_eq!(std::fs::read_to_string(dir.join("テンプレート.toml")).unwrap(),
                   "[スタイル.見出し1]\n大きさ = 16\n", "テンプレートが書き替わった");
        let _ = std::fs::remove_dir_all(&dir);
    }


    /// 着替えは役割と名前を使い分ける — 役割で出る名前は二重に名乗らない
    #[gpui::test]
    fn スタイルの着替えは役割と名前を使い分ける(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-c3b-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("見本.adoc");
        std::fs::write(&path, "本文。\n").unwrap();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(path.clone());
            this.wear_style("見出し2");
            let p = this.doc.paragraphs().next().unwrap();
            assert_eq!(p.style, kumihan::ParaStyle::Heading(2));
            assert_eq!(p.style_id, None, "役割で出る名前を二重に名乗った");

            this.wear_style("注意書き");
            let p = this.doc.paragraphs().next().unwrap();
            assert_eq!(p.style_id.as_deref(), Some("注意書き"));
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **選んでいれば字に、選んでいなければ段落に**(2026-08-16。文字単位の
    /// スタイルの門番)。語を1つ選んで直したのに段落ぜんぶが変わる、では
    /// 直接書式の手軽さに勝てない
    /// **見た目のボタンはスタイルの一覧へ案内する**(2026-08-17)。
    ///
    /// 前は押すたびに「名前を付けてください」と聞いていたので、同じ見た目を
    /// 使い回せず、外す方法もありませんでした。
    #[gpui::test]
    fn ネイティブでは見た目のボタンがスタイルの一覧を開く(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-toggle-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("見本.adoc");
        std::fs::write(&path, ":template: 見本の型\n\nここは大事なところ。\n").unwrap();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.open(path.clone());
            this.ed.move_to(9, false);
            this.ed.move_to(15, true);

            this.run_cmd("fontcolor", cx);
            assert!(this.rp_open, "右のパネルが開かない");
            assert_eq!(this.rp_tab, 2, "スタイルの面が出ていない");
            assert!(this.style_new.is_none(), "いきなり新設の画面が出た: {}", this.status);

            // 一覧から着る → 外す、が通ること
            this.tmpl.styles.push(kumihan::theme::StyleDef {
                name: "注意".into(),
                color: Some("C00000".into()),
                ..Default::default()
            });
            this.ed.move_to(9, false);
            this.ed.move_to(15, true);
            this.wear_style("注意");
            let 付いた = |this: &Writer| {
                this.doc.paragraphs().next().unwrap().runs.iter()
                    .filter(|r| r.fmt.style_id.as_deref() == Some("注意")).count()
            };
            assert!(付いた(this) > 0 || this.doc.paragraphs().next().unwrap().style_id.is_some(),
                    "着られていない");

            this.ed.move_to(9, false);
            this.ed.move_to(15, true);
            this.strip_style();
            assert_eq!(付いた(this), 0, "外れていない: {}", this.status);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[gpui::test]
    fn 選んだ字だけにスタイルが付く(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-char-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("見本.adoc");
        std::fs::write(&path, ":template: 見本の型\n\nここは大事なところ。\n").unwrap();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.open(path.clone());
            // 「大事」だけを選ぶ(バイト位置。「ここは」は 9 バイト)
            this.ed.move_to(9, false);
            this.ed.move_to(15, true);
            this.run_cmd("fontcolor", cx);
            assert!(this.rp_open && this.rp_tab == 2, "スタイルの面が開かない: {}", this.status);
            // その面から新しく作る
            this.style_new = Some(kumihan::theme::StyleDef {
                color: Some("C00000".into()),
                ..Default::default()
            });
            this.style_ed = kumihan::Editor::new("注意");
            this.style_commit();

            let p = this.doc.paragraphs().next().unwrap();
            assert_eq!(p.style_id, None, "段落に名前が付いた(選んだのは字だけ)");
            let 付いた: Vec<&str> = p
                .runs
                .iter()
                .filter(|r| r.fmt.style_id.as_deref() == Some("注意"))
                .map(|r| r.text.as_str())
                .collect();
            assert_eq!(付いた, vec!["大事"], "選んだ字だけに付いていない");

            // 保存すると [.注意]#大事# で残る
            this.save_to(path.clone());
            let back = std::fs::read_to_string(&path).unwrap();
            assert!(back.contains("[.注意]#大事#"), "文字スタイルが保存されない: {back}");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **フォルダから探す**(2026-08-17 発注者。SFIND の写真)。
    /// 素の字も docx も串刺しで、選んでも開かず、「読み込み」で初めて開く
    #[gpui::test]
    fn フォルダから探して読み込む(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("下")).unwrap();
        std::fs::write(dir.join("一.txt"), "あ\nunstructured covariance\nい\n").unwrap();
        std::fs::write(dir.join("下/二.md"), "# 題\n何もない\n").unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            // **場所は既定で決まる** — 開いている文書の隣(2026-08-17)。
            // ここでは文書を開いていないので明に渡す
            this.fd_dir = Some(dir.clone());
            this.fd_term = kumihan::Editor::new("unstructured");
            this.find_in_folder();
            assert_eq!(this.fd_hits.len(), 1, "当たりが1件でない: {}", this.status);
            assert_eq!(this.fd_hits[0].hits[0].line, 2);
            assert!(this.fd_tally.looked >= 2, "見た数が足りない");

            // 選んでも**開かない**(下に見せるだけ)
            let 前 = this.path.clone();
            this.find_peek(0, 0);
            assert_eq!(this.path, 前, "選んだだけで開いた");
            assert!(this.fd_peek.contains("unstructured"), "下に出ない: {}", this.fd_peek);

            // 「読み込み」で初めて開き、その位置へ行く
            this.find_load();
            assert_eq!(
                this.path.as_ref().and_then(|p| p.file_name()),
                Some(std::ffi::OsStr::new("一.txt")),
                "読み込めていない: {}",
                this.status
            );
            assert!(this.ed.cursor() >= 4, "当たりの位置へ飛んでいない");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **場所は開いている文書の隣が既定**(2026-08-17)。毎回「場所を選ぶ」を
    /// 押させない
    #[gpui::test]
    fn 探す場所は文書の隣が既定(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-finddir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("覚え.txt"), "unstructured\n").unwrap();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(dir.join("覚え.txt"));
            assert_eq!(this.fd_dir, None, "まだ選んでいない");
            assert_eq!(this.find_dir().as_deref(), Some(dir.as_path()), "隣が既定になっていない");
            this.fd_term = kumihan::Editor::new("unstructured");
            this.find_in_folder();
            assert_eq!(this.fd_hits.len(), 1, "既定の場所で探せない: {}", this.status);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **docx の中身も探せる**(写真の道具は一度 txt に落としていた)
    #[gpui::test]
    fn docx_の中身も串刺しで探せる(cx: &mut gpui::TestAppContext) {
        let src = std::path::Path::new("../sample/カタログ.docx");
        if !src.exists() {
            eprintln!("見本の docx が無いので飛ばす");
            return;
        }
        let dir = std::env::temp_dir().join(format!("writer-find2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(src, dir.join("見本.docx")).unwrap();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.fd_dir = Some(dir.clone());
            this.fd_term = kumihan::Editor::new("注文書");
            this.find_in_folder();
            assert_eq!(this.fd_hits.len(), 1, "docx の中身が探せない: {}", this.status);
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **組み方の2値**(2026-08-17)。Web のテンプレート(横幅可変・区切り
    /// なし)を着せると、紙の幅で折らず1本の流れになる
    #[gpui::test]
    fn 組み方でwebの流し組みになる(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-flow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // 何ページにもなる長さの本文
        let mut body = String::from("= 題\n:template: web\n\n");
        for i in 0..120 {
            body.push_str(&format!("これは {i} 段落目の本文です。ながながと書きます。\n\n"));
        }
        std::fs::write(dir.join("長い.adoc"), &body).unwrap();
        std::fs::write(dir.join("web.toml"), "[組み方]\n横幅 = \"可変\"\n区切り = \"なし\"\n")
            .unwrap();
        // 比べる相手(紙のまま)
        std::fs::write(dir.join("紙.adoc"), body.replace(":template: web", ":template: 紙"))
            .unwrap();
        std::fs::write(dir.join("紙.toml"), "[スタイル.本文]\n").unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(dir.join("紙.adoc"));
            assert!(this.tmpl.setting == Default::default(), "紙のはず");
            let 紙 = this.total_pages();
            assert!(紙 > 1, "紙で1ページに収まった(前提が崩れた): {紙}");

            this.open(dir.join("長い.adoc"));
            assert!(this.tmpl.setting.endless(), "組み方が読めていない: {}", this.status);
            assert_eq!(this.total_pages(), 1, "区切りなしなのに折れた");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **発表の組み方**(2026-08-17)。1節=1枚で、段落が枚を跨がない
    #[gpui::test]
    fn 発表の組み方は節ごとに1枚になる(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-slide-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // 3つの節。どれも1枚に収まる短さ
        let body = "= 題\n:template: 発表\n\n\
                    == ひとつめ\n\n短い本文。\n\n\
                    == ふたつめ\n\n短い本文。\n\n\
                    == みっつめ\n\n短い本文。\n";
        std::fs::write(dir.join("話.adoc"), body).unwrap();
        std::fs::write(dir.join("発表.toml"), "[組み方]\n区切り = \"節\"\n跨ぎ = false\n")
            .unwrap();
        // 比べる相手(紙)。同じ中身なら1枚に収まる
        std::fs::write(dir.join("紙.adoc"), body.replace(":template: 発表", ":template: 紙"))
            .unwrap();
        std::fs::write(dir.join("紙.toml"), "[スタイル.本文]\n").unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(dir.join("紙.adoc"));
            assert_eq!(this.total_pages(), 1, "紙では1枚に収まる(前提)");

            this.open(dir.join("話.adoc"));
            assert!(this.tmpl.setting.per_section(), "組み方が読めていない: {}", this.status);
            assert!(this.tmpl.setting.keep, "跨がないが読めていない");
            // **1枚目は表題**(2026-08-18 に表題が本文の段落になった)。
            // 発表なら題の枚があるのが普通の形です。節は3つなので合わせて4枚
            assert_eq!(this.total_pages(), 4, "表題の枚 + 節ごとに1枚になっていない");

            // **見出しと本文が同じ枚に載る。** 枚数だけ合っていても、
            // 見出しだけの枚と本文だけの枚に割れていたら発表にならない
            // (2026-08-17、実機で1枚目が見出しだけに見えたので数え直した)
            let mut 枚 = vec![String::new(); this.total_pages()];
            for line in &this.page.lines {
                if !line.from_body {
                    continue;
                }
                let (p, _) = this.page_of_roll(line.y_mm);
                枚[p].extend(line.cells.iter().map(|c| c.ch));
            }
            assert!(枚[0].contains("題"), "1枚目が表題でない: {枚:?}");
            for (i, 節) in ["ひとつめ", "ふたつめ", "みっつめ"].iter().enumerate() {
                assert!(枚[i + 1].contains(節), "{}枚目に見出し「{節}」が無い: {枚:?}", i + 2);
                assert!(枚[i + 1].contains("短い本文。"), "{}枚目に本文が無い: {枚:?}", i + 2);
            }
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **新しい文書は adoc 形式で始まる**(2026-08-17 発注者「もう、adoc から
    /// はじめましょう」)。docx は本文と書式が混ざるので、後から機械で構造を
    /// 拾い直すことになります。
    #[gpui::test]
    fn 新しい文書はadoc形式(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            assert!(this.native, "新規が docx になっている");
            // 書式は本文に入らず、テンプレートの側にある
            assert!(!this.tmpl.styles.is_empty(), "テンプレートが空");
            for p in this.doc.paragraphs() {
                for r in &p.runs {
                    assert_eq!(r.size_pt, None, "本文に大きさが焼き付いている");
                }
            }
            // 見た目のボタンは右のスタイルへ案内する(docx とは違う扱い)
            this.run_cmd("underline", cx);
            assert!(this.rp_open && this.rp_tab == 2, "スタイルの面が開かない");
        });
    }

    /// 互換(docx)では今までどおり直に掛かる — 封じるのはネイティブだけ
    #[gpui::test]
    fn 互換の文書では直接書式が今までどおり効く(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            // 新規は adoc 形式なので、docx を開いた状態を作る
            this.native = false;
            this.run_cmd("incfont", cx);
            assert!(this.style_new.is_none(), "docx で誘導が出た");
            assert!(!this.rp_open, "docx でスタイルの面が開いた");
        });
    }

    /// **フォルダに書式のファイルを1つ置けば、そのフォルダの文書が使う。**
    ///
    /// 発注者 2026-08-18「原則は、ディレクトリーの書式用のファイルをひとつ
    /// おく。それがテンプレート」。本文に `:template:` と書かなくても効きます。
    #[gpui::test]
    fn フォルダの書式のファイルを使う(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-folder-tmpl-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("テンプレート.toml"),
            "[スタイル.見出し1]\n大きさ = 30\n",
        )
        .unwrap();
        let doc = dir.join("案内.adoc");
        std::fs::write(&doc, "== 見出し\n\n本文。\n").unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(doc.clone());
            // 名指しは無いのに、フォルダの書式が効いている
            assert_eq!(this.doc.template, None, "本文が名前を持ってしまった");
            let c = kumihan::theme::compose(&this.doc, &this.tmpl);
            let ps: Vec<_> = c.paragraphs().collect();
            assert_eq!(ps[0].runs[0].size_pt, Some(30.0), "フォルダの書式が効いていない");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **名指しがあれば、そちらが勝つ。** 書いてあることが決まりより強い。
    #[gpui::test]
    fn 名指しはフォルダの書式より強い(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-tmpl-win-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("テンプレート.toml"), "[スタイル.見出し1]\n大きさ = 30\n").unwrap();
        std::fs::write(dir.join("特別.toml"), "[スタイル.見出し1]\n大きさ = 40\n").unwrap();
        let doc = dir.join("案内.adoc");
        std::fs::write(&doc, ":template: 特別\n\n== 見出し\n\n本文。\n").unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(doc.clone());
            let c = kumihan::theme::compose(&this.doc, &this.tmpl);
            let ps: Vec<_> = c.paragraphs().collect();
            assert_eq!(ps[0].runs[0].size_pt, Some(40.0), "名指しが負けた");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 合成は**写しの上**で行う — 紙面には見出しの大きさが乗るが、
    /// 保存される意味の側は無指定のまま
    #[gpui::test]
    fn 合成は紙面にだけ効いて本文を汚さない(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-adoc2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("見本2.adoc");
        std::fs::write(&path, "== 見出し\n\n本文。\n").unwrap();
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(path.clone());
            let ps: Vec<_> = this.doc.paragraphs().collect();
            assert_eq!(ps[0].runs[0].size_pt, None, "意味の側は無指定のまま");
            // 合成した写しでは見出しが 16pt(既定テンプレート)
            let c = kumihan::theme::compose(&this.doc, &this.tmpl);
            let cps: Vec<_> = c.paragraphs().collect();
            assert_eq!(cps[0].runs[0].size_pt, Some(16.0), "合成で見た目が乗らない");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **配られたテンプレートは書き替えない。**
    ///
    /// **配られたテンプレートは書き替えない。** そもそも writer は
    /// テンプレートを書き替えません(2026-08-18)。押したときは、直すべき
    /// ファイルの場所を言います。
    #[gpui::test]
    fn 配られたテンプレートは書き替えない(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-tmpl-{}", std::process::id()));
        let 配り元 = dir.join("配り元");
        let _ = std::fs::create_dir_all(&配り元);
        std::fs::write(配り元.join("社内標準.toml"), "[スタイル.本文]\n大きさ = 11\n").unwrap();
        let doc = dir.join("報告.adoc");
        std::fs::write(&doc, "= 報告\n:template: 社内標準\n\n本文です。\n").unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(doc.clone());
            this.tmpl_path = Some(配り元.join("社内標準.toml"));
            this.ed.move_to(0, false);
            this.tweak_style(1);
            let s = this.status.to_string();
            assert!(s.contains("配り元"), "直す場所を言っていない: {s}");
        });
        // 配り元も、文書の隣も、何も書かれていない
        assert_eq!(std::fs::read_to_string(配り元.join("社内標準.toml")).unwrap(),
                   "[スタイル.本文]\n大きさ = 11\n", "配られた側が書き替わった");
        assert!(!dir.join("社内標準.toml").exists(), "隣に写しを作った");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **adoc で保存すると、画像も隣に並ぶ。**
    ///
    /// adoc は画像を径路で指すので、径路を与えないと保存で絵が消えます
    /// (画面から挿した画像は径路を持っていません)。
    #[gpui::test]
    fn adocで保存すると画像も隣に並ぶ(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-adocimg-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        let out = dir.join("絵入り.adoc");
        w.update(cx, |this, _cx| {
            let mut d = kumihan::Document::default();
            let mut p = kumihan::Paragraph::default();
            p.images_new.push(kumihan::InlineImage {
                bytes: std::sync::Arc::new(vec![0x89, b'P', b'N', b'G', 9]),
                w_mm: 30.0,
                h_mm: 20.0,
                tex: None,
                src: None,
            });
            d.push_para(p);
            this.set_doc(d);
            this.save_adoc_to(&out).expect("保存できない");
        });
        let src = std::fs::read_to_string(&out).expect("adoc が無い");
        assert!(src.contains("image::images/図1.png[]"), "画像が本文に出ていない:\n{src}");
        assert!(dir.join("images/図1.png").is_file(), "画像のファイルが隣に無い");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **目次のページ番号は紙で数える。** 画面と紙が違ってもよい形にしたので
    /// (2026-08-18)、数える所だけは紙に合わせないと嘘の目次になります。
    #[gpui::test]
    fn 目次のページ番号は紙で数える(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-toc-print-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // 画面は A4、紙は小さい(B5 の半分ほど)ので、紙のほうが枚数が増える
        std::fs::write(dir.join("テンプレート.toml"), "[ページ]\n用紙 = \"A4\"\n").unwrap();
        std::fs::write(
            dir.join("テンプレート-印刷.toml"),
            "[ページ]\n用紙 = \"A4\"\n余白 = 90\n",
        )
        .unwrap();
        let doc = dir.join("章.adoc");
        let mut src = String::from("== 一つ目\n\n");
        for _ in 0..40 {
            src.push_str("本文の行です。ここは紙の枚数を増やすための行です。\n\n");
        }
        src.push_str("== 二つ目\n\n終わり。\n");
        std::fs::write(&doc, &src).unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(doc.clone());
            let 画面 = this.total_pages();
            let 紙 = this.print_layout().expect("印刷用が読めない");
            let (pages, _) = paper::paginate(&紙.0, paper::Paper {
                width_mm: 紙.1.w_mm, height_mm: 紙.1.h_mm, margin_mm: 紙.1.left_mm });
            let 紙の枚数 = pages.iter().copied().max().unwrap_or(1);
            assert!(紙の枚数 > 画面, "紙のほうが枚数が多い形になっていない({紙の枚数} と {画面})");
            // 最後の見出しのページ番号は**紙の**枚数(画面の枚数ではない)
            let 末尾 = this.doc.body_text().len();
            assert_eq!(this.page_of_byte()(末尾), 紙の枚数,
                       "目次が画面の枚数で数えている");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **ページの飾りはテンプレートが持てる**(2026-08-18)。
    /// ヘッダー・フッター・透かし・ページの色・縦書きを adoc の文書に付けられます。
    #[gpui::test]
    fn テンプレートのページの飾りが効く(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-kazari-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("テンプレート.toml"),
            "[ページ]\nヘッダー = \"社内資料\"\nフッター = \"- {ページ} -\"\n\
             透かし = \"社外秘\"\nページの色 = \"FFFDF5\"\n",
        )
        .unwrap();
        let doc = dir.join("報告.adoc");
        std::fs::write(&doc, "本文です。\n").unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(doc.clone());
            // 飾りは画面に出る
            let h: String = this.header_lines.iter()
                .flat_map(|l| l.cells.iter()).map(|c| c.ch).collect();
            assert!(h.contains("社内資料"), "ヘッダーが出ていない: {h:?}");
            assert_eq!(this.dress_page.0.as_deref(), Some("社外秘"), "透かしが効いていない");
            assert_eq!(this.dress_page.1.as_deref(), Some("FFFDF5"), "ページの色が効いていない");
            // **本文は意味だけのまま**(保存に漏れない)
            assert!(this.doc.header.paragraphs.is_empty(), "本文に飾りが入った");
            assert!(this.doc.watermark.is_none(), "本文に透かしが入った");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **書き出し先ごとにテンプレートを持てる。** 混ぜないので、一度に効くのは
    /// 1枚のまま(発注者 2026-08-18「表示用、印刷用、Web用、アプリ用と複数の
    /// テンプレートを持つのも悪くないのでは」)。
    #[gpui::test]
    fn 書き出し先ごとの書式を使う(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-web-tmpl-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // 画面は紙の幅、Web は横幅可変
        std::fs::write(dir.join("テンプレート.toml"), "[スタイル.見出し1]\n大きさ = 18\n").unwrap();
        std::fs::write(
            dir.join("テンプレート-web.toml"),
            "[組み方]\n横幅 = \"可変\"\n区切り = \"なし\"\n\n[スタイル.見出し1]\n大きさ = 30\n",
        )
        .unwrap();
        let doc = dir.join("案内.adoc");
        std::fs::write(&doc, "== 見出し\n\n本文。\n").unwrap();
        let out = dir.join("案内.html");

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.open(doc.clone());
            // 画面はフォルダの書式(18pt)
            let c = kumihan::theme::compose(&this.doc, &this.tmpl);
            assert_eq!(c.paragraphs().next().unwrap().runs[0].size_pt, Some(18.0));
            this.write_html(&out);
        });
        let html = std::fs::read_to_string(&out).expect("HTML が無い");
        // 書き出しは Web 用(30pt・横幅可変)
        assert!(html.contains("font-size:30pt"), "Web 用の書式が効いていない:\n{html}");
        assert!(html.contains("max-width"), "横幅可変が効いていない");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **HTML に書き出すと、画像も隣に並ぶ。**
    ///
    /// HTML は画像を相対の径路で参照するので、HTML だけ書いても絵が出ません
    /// (2026-08-17、4つの面を揃えるときに足しました)。
    #[gpui::test]
    fn htmlに書き出すと画像も隣に並ぶ(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("writer-html-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        let out = dir.join("文書.html");
        w.update(cx, |this, _cx| {
            let mut d = kumihan::Document::default();
            let mut p = kumihan::Paragraph::default();
            p.images_new.push(kumihan::InlineImage {
                bytes: std::sync::Arc::new(vec![0x89, b'P', b'N', b'G', 1, 2]),
                w_mm: 30.0,
                h_mm: 20.0,
                tex: None,
                src: None,
            });
            d.push_para(p);
            this.set_doc(d);
            this.write_html(&out);
        });
        let html = std::fs::read_to_string(&out).expect("HTML が無い");
        assert!(html.contains("src=\"images/図1.png\""), "画像を参照していない:\n{html}");
        assert!(dir.join("images/図1.png").is_file(), "画像のファイルが隣に無い");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// **文章の中の表でセル関数が使える**(2026-08-19、エンジンの統一 3段目)。
///
/// 画面には答えが出て、`self.doc`(意味の正本)には式が残る。
/// 保存されるのは意味だけ、という決めをここで縛る
#[cfg(test)]
#[allow(non_snake_case)]
mod 表の式 {
    use crate::*;

    fn 台帳のある文書() -> Document {
        let cell = |s: &str| kumihan::Cellbox {
            paragraphs: Document::plain(s).paragraphs().cloned().collect(),
            ..Default::default()
        };
        let mut d = Document::plain("売上のまとめ");
        d.blocks.push(kumihan::Block::Table(kumihan::Table {
            rows: vec![
                vec![cell("品名"), cell("金額")],
                vec![cell("机"), cell("1200")],
                vec![cell("椅子"), cell("800")],
                vec![cell("合計"), cell("=SUM(B2:B3)")],
            ],
            header_row: true,
            ..Default::default()
        }));
        d
    }

    /// 組んだ紙面に**答えの字**が出ていること(式の字ではなく)
    #[gpui::test]
    fn 画面に答えが出る(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.doc = 台帳のある文書();
            this.relayout();

            let 字: String = this.page.lines.iter().flat_map(|l| l.cells.iter().map(|c| c.ch)).collect();
            assert!(字.contains("2000"), "答えが紙面に出ていない: {字}");
            assert!(!字.contains("SUM"), "式の字がそのまま出ている: {字}");
        });
    }

    /// **正本は式のまま。** 画面に答えを出しても、保存される意味は変わらない
    #[gpui::test]
    fn 正本は式のまま(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.doc = 台帳のある文書();
            this.relayout();

            let t = this.doc.tables().next().expect("表が消えた");
            assert_eq!(
                kumihan::paras_text(&t.rows[3][1].paragraphs),
                "=SUM(B2:B3)",
                "正本まで答えで塗り潰した"
            );
            // 書き出す adoc にも式が残る
            let src = kumihan::adoc::write(&this.doc);
            assert!(src.contains("=SUM(B2:B3)"), "保存の字に式が残っていない:\n{src}");
        });
    }
}

/// **1つのファイルに文書を何枚も**(2026-08-19 発注者「同時に送付する
/// 請求書の原稿をまとめて保存する場合につかいます」)。
#[cfg(test)]
#[allow(non_snake_case)]
mod 請求書をまとめる {
    use crate::*;

    fn 三枚() -> String {
        "[discrete]\n= 請求書 山田商店\n\n金額 12,000 円\n\n\
         [discrete]\n= 請求書 鈴木工業\n\n金額 8,400 円\n\n\
         [discrete]\n= 請求書 佐藤商会\n\n金額 3,300 円\n".into()
    }

    #[gpui::test]
    fn 三枚を開いて行き来できる(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("jo-many-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("請求書.adoc");
        std::fs::write(&p, 三枚()).unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.open(p.clone());
            assert_eq!(this.doc_count(), 3, "文書の枚数が合わない");
            assert_eq!(this.doc_name(0), "請求書 山田商店");
            assert_eq!(this.doc_name(2), "請求書 佐藤商会");
            // いまは1枚目
            assert!(this.doc.body_text().contains("12,000"), "{:?}", this.doc.body_text());

            // 3枚目へ行く
            this.show_doc(2);
            assert_eq!(this.doc_at, 2);
            assert!(this.doc.body_text().contains("3,300"), "{:?}", this.doc.body_text());
            // 戻る
            this.show_doc(0);
            assert!(this.doc.body_text().contains("12,000"));
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **保存すると3枚とも残る。** 見ていない文書を落とさない
    #[gpui::test]
    fn 保存で三枚とも残る(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("jo-many2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("請求書.adoc");
        std::fs::write(&p, 三枚()).unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.open(p.clone());
            this.show_doc(1); // 2枚目を見ている状態で保存
            let 並び = this.docs_for_save();
            assert_eq!(並び.len(), 3, "保存の並びが3枚でない");
            assert_eq!(並び[0].props.title, "請求書 山田商店");
            assert_eq!(並び[1].props.title, "請求書 鈴木工業");
            assert_eq!(並び[2].props.title, "請求書 佐藤商会");
            // いま見ている2枚目の中身が入っていること
            assert!(並び[1].body_text().contains("8,400"));
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 1枚だけのファイルはタブを出さない(何も選べないタブは邪魔)
    #[gpui::test]
    fn 一枚ならタブは出ない(cx: &mut gpui::TestAppContext) {
        let dir = std::env::temp_dir().join(format!("jo-many3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("報告.adoc");
        std::fs::write(&p, "= 報告書\n\n本文です。\n").unwrap();

        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.open(p.clone());
            assert_eq!(this.doc_count(), 1, "1枚のはずが {} 枚", this.doc_count());
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// **ファイルを何枚も開く**(2026-08-19 発注者「Zed と同じように複数
/// ファイルを開くことができるようにして」)。
#[cfg(test)]
#[allow(non_snake_case)]
mod ファイルを何枚も開く {
    use crate::*;

    /// 試験ごとに**別のフォルダ**を作ります。同じ名前にすると、片方の
    /// 後片づけがもう片方の足元を消します(2026-08-19 に踏みました)
    fn 場所(名: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jo-tabs-{}-{名}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (n, body) in [("甲.adoc", "= 甲\n\nあ。\n"), ("乙.adoc", "= 乙\n\nい。\n")] {
            std::fs::write(dir.join(n), body).unwrap();
        }
        dir
    }

    #[gpui::test]
    fn 二枚開いて行き来できる(cx: &mut gpui::TestAppContext) {
        let dir = 場所("go");
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.open(dir.join("甲.adoc"));
            assert_eq!(this.file_count(), 1, "1枚目はタブを増やさない");

            this.open_in_tab(dir.join("乙.adoc"));
            assert_eq!(this.file_count(), 2);
            assert_eq!(this.file_at, 1);
            assert_eq!(this.file_name(0), "甲");
            assert_eq!(this.file_name(1), "乙");
            assert!(this.doc.body_text().contains("い。"), "{:?}", this.doc.body_text());

            // 1枚目へ戻る
            this.show_file(0);
            assert!(this.doc.body_text().contains("あ。"), "{:?}", this.doc.body_text());
            assert_eq!(this.path.as_deref(), Some(dir.join("甲.adoc").as_path()));
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **同じファイルを二重に開かない。** どちらを保存したのか分からなくなる
    #[gpui::test]
    fn 同じファイルは二重に開かない(cx: &mut gpui::TestAppContext) {
        let dir = 場所("nijuu");
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.open(dir.join("甲.adoc"));
            this.open_in_tab(dir.join("乙.adoc"));
            this.open_in_tab(dir.join("甲.adoc"));
            assert_eq!(this.file_count(), 2, "二重に開いた");
            assert_eq!(this.file_at, 0, "先に開いていたタブへ行っていない");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **書きかけのタブは閉じない。** 黙って捨てない
    #[gpui::test]
    fn 書きかけのタブは閉じない(cx: &mut gpui::TestAppContext) {
        let dir = 場所("kakikake");
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.open(dir.join("甲.adoc"));
            this.open_in_tab(dir.join("乙.adoc"));
            this.dirty = true;
            assert!(!this.close_file(1), "書きかけなのに閉じた");
            assert_eq!(this.file_count(), 2);
            // 書きかけでなければ閉じる
            this.dirty = false;
            assert!(this.close_file(1));
            assert_eq!(this.file_count(), 1);
            assert!(this.doc.body_text().contains("あ。"), "残った側が違う");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// タブごとに**別の書きかけの印**を持つ
    #[gpui::test]
    fn 書きかけの印はタブごと(cx: &mut gpui::TestAppContext) {
        let dir = 場所("shirushi");
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _| {
            this.open(dir.join("甲.adoc"));
            this.open_in_tab(dir.join("乙.adoc"));
            this.dirty = true; // 乙 が書きかけ
            this.show_file(0);
            assert!(!this.dirty, "甲 まで書きかけになった");
            assert!(this.file_dirty(1), "乙 の書きかけが消えた");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod file_menu_tests {
    use crate::*;

    /// **いま押せるかが、状況で変わる**(2026-08-21 の B-5)。表の同じ試験と対。
    #[gpui::test]
    fn 押せるかは状況で変わる(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            // 目次を入れていなければ「目次の更新」は押せない
            assert!(!this.押せるか("toc-update"), "目次が無いのに押せる");
            // 見出しを立ててから目次を入れると押せる
            this.set_doc(kumihan::adoc::parse("= 題\n\n== 第1章\n\n本文。\n").expect("読めない"));
            this.run_cmd("toc", cx);
            assert!(this.押せるか("toc-update"), "目次を入れたのに押せない");
            // コメントの削除は、いまの段落にコメントが付いているときだけ
            assert!(!this.押せるか("co-delcomment"), "コメントが無いのに押せる");
        });
    }

    /// **並びと押せるかを縛る**(統合の段8 の1)。calc の同じ試験と対。
    #[gpui::test]
    fn ファイルの項目の並びと押せるかが変わらない(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            let 品 = this.file_menu();
            let ids: Vec<&str> = 品.iter().map(|i| i.id).collect();
            assert_eq!(ids, vec![
                "f-back", "f-new", "f-tpl", "f-open",
                // **フォルダを開き直す**(2026-08-25)。綴りはフォルダなので、
                // 仕事を替えるとはフォルダを替えること。前は起動時だけでした
                "f-folder",
                "f-url", "f-recent", "f-find",
                // **前に落ちた跡から開き直す**(2026-08-21 の B-3)。
                // 控えが無ければ灰色(押しても何も無い、をやめる)
                "f-recover",
                "f-save", "f-saveas", "f-print",
                // **形を選んで書き出す1つの入り口**(2026-08-25)
                "f-export",
                "f-merge", "f-html", "f-protect",
                "f-distill",
                // **書式の標準**(2026-08-26)。3段のどれが効いているかを見て直す
                "f-style",
                "f-info", "f-place", "f-quit", "f-opts", "f-help", "f-req",
            ]);
            let 下: Vec<&str> = 品.iter().filter(|i| i.tail).map(|i| i.id).collect();
            assert_eq!(下, vec!["f-opts", "f-help", "f-req"]);
        });
    }

    /// **17 個は表の画面と同じ id。** ここがずれると officework が
    /// 1枚のページを描けない(段8 の2)
    #[gpui::test]
    fn 共通の項目は表と同じ番号(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            let 文章: Vec<&str> = this.file_menu().iter().map(|i| i.id).collect();
            for id in ["f-back", "f-new", "f-tpl", "f-open", "f-recent", "f-find",
                       "f-save", "f-saveas", "f-print", "f-html", "f-protect",
                       "f-info", "f-place", "f-quit", "f-opts", "f-help", "f-req"] {
                assert!(文章.contains(&id), "共通のはずの {id} が無い");
            }
        });
    }

    #[gpui::test]
    fn 出している面の項目に印が付く(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.file_menu_click("f-recent", cx);
            let on: Vec<&str> =
                this.file_menu().iter().filter(|i| i.on).map(|i| i.id).collect();
            assert_eq!(on, vec!["f-recent"]);
        });
    }
}

#[cfg(test)]
mod autocorrect_tests {
    use crate::*;

    /// **数学オートコレクトは文章でも効く**(2026-08-20 発注者「双方でできる
    /// ようにしたいです」)。仕掛けは前から `ui::handler` の共通の物で、
    /// 表だけが名乗り出ていた
    #[gpui::test]
    fn 本文で綴りが記号に替わる(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            assert!(this.autocorrect, "既定で入になっていない");
            assert!(ui::HasEditor::math_autocorrect(this), "本文で掛からない");
        });
    }

    /// **掛けない所**。検索の欄で替わると探せなくなり、数式の小窓で替わると
    /// TeX の綴りが壊れる
    #[gpui::test]
    fn 探す欄と数式の小窓では掛からない(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            this.find_open = true;
            assert!(!ui::HasEditor::math_autocorrect(this), "探す欄で掛かる");
            this.find_open = false;
            this.eq_open = true;
            assert!(!ui::HasEditor::math_autocorrect(this), "数式の小窓で掛かる");
        });
    }

    /// **画面の明暗も双方でできる**(2026-08-20 発注者)。
    ///
    /// 文章は `dark` の欄を前から持っていたのに**設定に行が無く、控えても
    /// いなかった**ので、開き直すと明るさが戻っていた。表と同じ器
    /// (`theme`)を見る。
    ///
    /// *控えるかどうかは呼ぶ側が渡す* — `cfg!(test)` はクレートごとに
    /// 決まるので、`ui` の中で見ても**アプリの試験中は偽**になり、
    /// 本物の `settings.toml` を書き換えてしまう(2026-08-20 に実際にやった)
    #[gpui::test]
    fn 明暗は表と同じ器を見る(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            assert_eq!(this.dark, ui::dark_at_start(), "起動のときに器を見ていない");
            // **控えない**で入切だけを見る(本物の設定に触らない)
            let (on, msg) = ui::toggle_dark(this.dark, false);
            assert_ne!(on, this.dark, "入切が効かない");
            assert!(!msg.is_empty(), "何をしたか言っていない");
        });
    }

    /// **器は表と同じ1つの綴り**(`math_autocorrect`)。
    ///
    /// *入切そのものは試験しません。* `ui::toggle_math_autocorrect` は
    /// `settings.toml` に書くので、試験から呼ぶと**発注者の本物の設定を
    /// 書き換えます**(2026-08-20 に実際にやった)。ここでは
    /// 「読む側が表と同じ綴りを見ている」ことだけを確かめます
    #[gpui::test]
    fn 器は表と同じ綴り(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            // 設定が無いときの既定は「入」(表と同じ)
            let 器 = ui::settings::get("math_autocorrect");
            let 期待 = 器.map(|v| v != "0").unwrap_or(true);
            assert_eq!(this.autocorrect, 期待, "表と違う既定になっている");
        });
    }
}

#[cfg(test)]
mod docx_formula_tests {
    use crate::*;

    /// **docx には値を焼く**(2026-08-20 発注者。SEKKEI「エンジンの統一」3段目)。
    ///
    /// 前は `=SUM(B2:B4)` の字がそのまま docx に出ていた。Word で開いた相手には
    /// **答えでなく式が見える**。画面・HTML・紙は写しの値を見せているので、
    /// docx だけが素通しでずれていた。
    ///
    /// 確かめ方はメモのとおり2つ — docx 側が値であること、`.adoc` の正本は
    /// 式のままであること。
    #[gpui::test]
    fn 式は値で出て正本は式のまま(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            // 2 と 3 の表に =SUM(A1:B1)
            let cell = |x: &str| kumihan::Cellbox {
                paragraphs: Document::plain(x).paragraphs().cloned().collect(),
                ..Default::default()
            };
            this.doc.blocks.push(kumihan::Block::Table(kumihan::Table {
                rows: vec![vec![cell("2"), cell("3"), cell("=SUM(A1:B1)")]],
                ..Default::default()
            }));

            let 出 = this.doc_for_save(None);
            let 字 = |d: &kumihan::Document| -> Vec<String> {
                d.blocks
                    .iter()
                    .filter_map(|b| match b {
                        kumihan::Block::Table(t) => Some(t.clone()),
                        _ => None,
                    })
                    .flat_map(|t| {
                        t.rows
                            .into_iter()
                            .flatten()
                            .map(|c| kumihan::paras_text(&c.paragraphs))
                            .collect::<Vec<_>>()
                    })
                    .collect()
            };
            // docx へ出す写しは**値**
            assert!(字(&出).contains(&"5".to_string()), "docx に値が焼けていない: {:?}", 字(&出));
            // **正本は式のまま**(開き直せばまた計算される)
            assert!(
                字(&this.doc).iter().any(|x| x.starts_with("=SUM")),
                "正本の式が消えた: {:?}",
                字(&this.doc)
            );
        });
    }

    /// 式が無ければ写しも作らない(倹約)。触っていないことを字で見る
    #[gpui::test]
    fn 式が無ければ表を触らない(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            let cell = |x: &str| kumihan::Cellbox {
                paragraphs: Document::plain(x).paragraphs().cloned().collect(),
                ..Default::default()
            };
            this.doc.blocks.push(kumihan::Block::Table(kumihan::Table {
                rows: vec![vec![cell("りんご"), cell("100")]],
                ..Default::default()
            }));
            assert!(!ops::table::has_formula(&this.doc), "式が無いのに有ると言う");
            let 出 = this.doc_for_save(None);
            let ある = 出.blocks.iter().any(|b| matches!(b, kumihan::Block::Table(_)));
            assert!(ある, "表が消えた");
        });
    }
    #[gpui::test]
    /// **表は行数と列数を打ってから挿します**(2026-08-25 発注者
    /// 「行×列を選ぶ画面は、数値入力にしないと選択ではだめでしょう」)。
    ///
    /// 前は 64 個の組を一覧に並べていました。4×6 を出すのに 64 個から
    /// 目で探すことになり、使えませんでした。
    fn 表は行数と列数を打って挿す(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            // 押すと欄が出るだけ。**まだ表は入りません**
            this.run_cmd("instable", cx);
            assert!(this.tbl_open, "打つ欄が出ていません");
            assert!(!this.doc.blocks.iter().any(|b| matches!(b, kumihan::Block::Table(_))),
                    "押しただけで表が入っています");
            // 2行5列と打つ
            this.tbl_ed = Editor::new("2,5");
            this.tbl_commit(cx);
            let t = this.doc.blocks.iter().find_map(|b| match b {
                kumihan::Block::Table(t) => Some(t),
                _ => None,
            }).expect("表が入っていません");
            assert_eq!(t.rows.len(), 2, "行の数が打ったとおりではありません");
            assert_eq!(t.rows[0].len(), 5, "列の数が打ったとおりではありません");
            assert!(!this.tbl_open, "挿した後も欄が開いたままです");
            // **横幅は文章の幅。** 列の指定を持たないので、組む側が行長を割ります
            assert!(t.col_mm.is_empty(), "列の幅を勝手に決めています");

            // `3x4` や `3 4` も同じに読みます — 打ち方で断らないため
            for 打つ in ["3x4", "3 4", "3、4"] {
                this.tbl_ed = Editor::new(打つ);
                this.tbl_open = true;
                this.tbl_commit(cx);
                assert!(!this.tbl_open, "「{打つ}」が読めていません");
            }
            // **数が読めなければ断ります。** 黙って 3×3 を入れません
            let 前 = this.doc.blocks.len();
            this.tbl_ed = Editor::new("あ");
            this.tbl_open = true;
            this.tbl_commit(cx);
            assert!(this.tbl_open, "読めない字を受け付けています");
            assert_eq!(this.doc.blocks.len(), 前, "読めないのに表が入りました");
            // **大きすぎるものも断ります**(打ち間違いで固まらないように)
            this.tbl_ed = Editor::new("999,999");
            this.tbl_commit(cx);
            assert!(this.tbl_open, "大きすぎる数を受け付けています");
        });
    }

    #[gpui::test]
    /// **日付は形式の一覧から選びます**(2026-08-25 発注者「形式の一覧は必要」)。
    ///
    /// 前は西暦の1つだけを固定で挿していました。事務の様式は和暦で書く
    /// ものが多く、毎回打ち直すことになっていました。
    /// **自動更新は作りません** — 入るのは固定の字です。
    fn 日付は形式を選んで入る(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            this.run_cmd("datetime", cx);
            assert_eq!(this.open_list, Some("datetime"), "一覧が開いていません");
            let 形 = this.一覧の中身("datetime");
            assert!(形.len() >= 5, "形が {} 個しかありません", 形.len());
            // 西暦と和暦が両方あること
            assert!(形.iter().any(|(k, _)| k.contains("年") && k.contains("月")),
                    "西暦の形がありません: {形:?}");
            assert!(形.iter().any(|(k, _)| k.starts_with("令和") || k.starts_with("平成")),
                    "和暦の形がありません: {形:?}");
            // 選ぶと本文に固定の字で入る
            let 選ぶ = 形[0].0.clone();
            this.一覧を選ぶ("datetime", &選ぶ, cx);
            assert!(this.ed.text().contains(&選ぶ), "選んだ形が入っていません");
            assert_eq!(this.open_list, None, "選んだ後も一覧が開いたままです");
        });
    }

    /// 和暦の境目。**改元の日から新しい元号**になります
    #[test]
    fn 和暦は改元の日で変わる() {
        use crate::cmds::和暦;
        assert_eq!(和暦(2026, 8, 25), Some(("令和", "R", 8)));
        assert_eq!(和暦(2019, 5, 1), Some(("令和", "R", 1)), "改元の当日は令和");
        assert_eq!(和暦(2019, 4, 30), Some(("平成", "H", 31)), "改元の前日は平成");
        assert_eq!(和暦(1989, 1, 8), Some(("平成", "H", 1)));
        assert_eq!(和暦(1989, 1, 7), Some(("昭和", "S", 64)), "改元の前日は昭和");
        assert_eq!(和暦(1900, 1, 1), None, "昭和より前は元号を出しません");
    }

    #[gpui::test]
    /// **フォルダを開くと、一覧にフォルダの中身が出ます**
    /// (手引き `docs/ja/commands/ファイル/フォルダーを開く.adoc`)。
    ///
    /// 2026-08-25 発注者「どうしてフォルダーを開くがないのだ」。
    /// 前は起動のときにしか選べず、動かしている間は替えられませんでした。
    fn フォルダを開くと一覧が出る(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, _cx| {
            // ファイルのページに口があること
            let ids: Vec<&str> = this.file_menu().iter().map(|i| i.id).collect();
            assert!(ids.contains(&"f-folder"), "ファイルのページに口がありません");
            let d = std::env::temp_dir();
            this.show_folder(d.clone());
            assert!(this.rp_open, "一覧が開いていません");
            assert_eq!(this.rp_tab, 3, "フォルダの中身の面になっていません");
            // **綴りの .venv を Python の第一候補にします**
            assert_eq!(pyrun::work_dir(), Some(d), "綴りが Python に伝わっていません");
        });
    }

    #[gpui::test]
    /// **エクスポートは形を選んでから出します**
    /// (手引き `docs/ja/commands/ファイル/エクスポート.adoc`)。
    ///
    /// 前は「印刷」と「Web の形で書き出す」に分かれていて、
    /// どこから何が出せるのかが探しにくい形でした。
    fn エクスポートは形を選ぶ(cx: &mut gpui::TestAppContext) {
        let w = cx.update(|cx| cx.new(|cx| Writer::new(None, cx)));
        w.update(cx, |this, cx| {
            let ids: Vec<&str> = this.file_menu().iter().map(|i| i.id).collect();
            assert!(ids.contains(&"f-export"), "ファイルのページに口がありません");
            // **ファイルのページのボタンは `file_menu_click`** を通ります
            this.file_menu_click("f-export", cx);
            assert_eq!(this.open_list, Some("f-export"), "形の一覧が開いていません");
            let 形: Vec<String> =
                this.一覧の中身("f-export").into_iter().map(|(k, _)| k).collect();
            // **文章の節から出せるのは4つ**(手引きの表)
            assert_eq!(形, vec!["docx", "html", "pdf", "text"], "出せる形が表と違います");
            // **`.adoc` は出しません** — 保存の側だからです
            assert!(!形.iter().any(|k| k == "adoc"), "adoc が書き出しに出ています");
        });
    }


}
