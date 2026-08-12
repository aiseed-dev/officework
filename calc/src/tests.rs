//! calc の試験(main.rs から純移動 2026-08-06。分割の1歩目)

#[cfg(test)]
mod freeze_tests {
    use crate::*;

    #[test]
    fn 固定した行は窓が動いても頭に残る() {
        // 見出し行(0)を固定して、窓が10行目に居ても 0 行目が出る
        let rows = grid_rows(Some(Pos::new(1, 1)), Pos::new(10, 5), 5);
        assert_eq!(rows[0], 0, "固定した見出しが消えた: {rows:?}");
        assert_eq!(rows[1], 10, "続きが窓から始まっていない: {rows:?}");
        let cols = grid_cols(Some(Pos::new(1, 1)), Pos::new(10, 5), 4);
        assert_eq!(cols, vec![0, 5, 6, 7], "{cols:?}");
    }

    #[test]
    fn 固定なしなら窓のまま() {
        assert_eq!(grid_rows(None, Pos::new(3, 0), 4), vec![3, 4, 5, 6]);
    }

    #[test]
    fn 窓が固定の中に居ても重複しない() {
        // 窓が先頭にあるとき、固定行と窓の行が二重に出ない
        let rows = grid_rows(Some(Pos::new(2, 0)), Pos::new(0, 0), 5);
        let mut sorted = rows.clone();
        sorted.dedup();
        assert_eq!(rows.len(), sorted.len(), "行が二重に出た: {rows:?}");
    }

    #[gpui::test]
    fn ファイルの固定枠が画面へ出て保存でモデルへ戻る(cx: &mut gpui::TestAppContext) {
        // **画面とファイルが別のことを言わない**ための往復。固定枠は画面の状態
        // (`frozen`)で持つので、開くときに model から移し、保存の前に model へ
        // 戻す。どちらかが欠けると「固定が見えない」か「固定してもファイルに
        // 載らない」かのどちらかになる
        use sheet::model::FreezePane;
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            // ファイルが「見出しの1行を固定」と言っている状態
            this.book.sheets[0].freeze = Some(FreezePane { frozen_rows: 1, frozen_columns: 0 });
            this.freeze_from_book();
            assert_eq!(this.frozen, Some(Pos::new(1, 0)), "ファイルの固定枠が画面へ出ない");
            // 画面で左の2列も足して、モデルへ戻す
            this.frozen = Some(Pos::new(1, 2));
            this.freeze_into_book();
            assert_eq!(
                this.book.sheets[0].freeze,
                Some(FreezePane { frozen_rows: 1, frozen_columns: 2 }),
                "画面の固定枠がモデルへ戻らない"
            );
            // 固定を解いたら model からも消える(空の固定枠を書き残さない)
            this.frozen = None;
            this.freeze_into_book();
            assert_eq!(this.book.sheets[0].freeze, None, "固定を解いてもモデルに残る");
        });
    }
}

#[cfg(test)]
mod size_grip_tests {
    use crate::*;

    #[test]
    fn 境界の近くだけ掴める() {
        // 2列(48px, 108px)が HEAD_W から並ぶ
        let cols = [(0u32, 48.0f32), (1, 108.0)];
        let e1 = HEAD_W + 48.0; // 1本目の境界
        let e2 = e1 + 108.0; // 2本目
        assert_eq!(grip_hit(&cols, HEAD_W, e1), Some(0));
        assert_eq!(grip_hit(&cols, HEAD_W, e1 - GRIP), Some(0), "縁の手前±GRIPで掴めない");
        assert_eq!(grip_hit(&cols, HEAD_W, e1 + GRIP), Some(0));
        assert_eq!(grip_hit(&cols, HEAD_W, e2 - 1.0), Some(1), "2本目の境界が累積位置にない");
        assert_eq!(grip_hit(&cols, HEAD_W, e1 + GRIP + 1.0), None, "境界の外で掴めた");
        assert_eq!(grip_hit(&cols, HEAD_W, HEAD_W + 10.0), None, "列の中ほどで掴めた");
    }

    #[test]
    fn 一覧は押したボタンの真下に出る() {
        // 窓: リボン(高さ96)+ 数式バー(24)の下、y=120 から格子の面。幅1200
        let pane = (0.0, 120.0, 1200.0, 700.0);
        // 「ホーム」の書体の欄: 左端 x=300、下辺 y=70(リボンの中)
        let (x, y) = pop_under((300.0, 48.0, 110.0, 22.0), pane);
        assert_eq!(x, 300.0, "一覧が欄の左端にそろっていない");
        // 面より上へは出せない(一覧の層が面の中でクリップされる)ので、
        // 縦は面の一番上まで。deferred で外に出す手は一覧が全部消えて失敗
        assert_eq!(y, 2.0, "面の上端に収めていない: {y}");

        // 右端のボタン: 一覧が窓の外へ出ないよう内に寄せる
        let (x, _) = pop_under((1150.0, 48.0, 40.0, 22.0), pane);
        assert_eq!(x, 1200.0 - POP_W, "右端で一覧がはみ出す");

        // 面が窓の左端でないとき(左に作業ウィンドウ等)も面の中で数える
        let (x, _) = pop_under((300.0, 48.0, 110.0, 22.0), (200.0, 120.0, 1000.0, 700.0));
        assert_eq!(x, 100.0, "面のずれを引いていない");

        // まだ一度も描いていない(幅0)ときは寄せずにそのまま
        let (x, _) = pop_under((300.0, 48.0, 110.0, 22.0), (0.0, 0.0, 0.0, 0.0));
        assert_eq!(x, 300.0, "幅が分かる前に寄せた");

        // ボタンの場所が分からないときの逃げ道: 押した点のすぐ下
        let (x, y) = pop_at_click(300.0, pane);
        assert_eq!((x, y), (288.0, 2.0), "逃げ道が面の一番上に出ない");
    }

    #[test]
    fn 幅の換算が往復する() {
        // 画面px → xlsxの字数 → 画面px が(丸め2桁でも)崩れない
        let px0 = 108.0f32;
        let w = ((px0 / PX_PER_CHW) * 100.0).round() / 100.0;
        assert!((w - 8.43).abs() < 0.01, "既定幅が 8.43 にならない: {w}");
        assert!((w * PX_PER_CHW - px0).abs() < 0.5, "幅の往復がずれる");
        // 行: 画面px → pt → 画面px。既定 24px = 15pt
        let pt = (24.0f32 * 15.0 / 24.0 * 100.0).round() / 100.0;
        assert_eq!(pt, 15.0);
        assert_eq!(pt * 24.0 / 15.0, 24.0);
    }
}

#[cfg(test)]
mod validation_tests {
    use crate::*;

    #[gpui::test]
    fn パネルから整数の規則を掛けて堰き止める(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // B2:B4 に 1〜100 の整数(本家の形のパネル: 設定タブで組む)
            this.anchor = Some(Pos::parse("B2").unwrap());
            this.cursor = Pos::parse("B4").unwrap();
            this.run_cmd("data-validation", cx);
            {
                let d = this.dv_dlg.as_mut().expect("入力規則のパネルが開かない");
                d.kind = 1; // 整数
                d.op = 0; // 次の値の間
                d.eds[0] = Editor::new("1");
                d.eds[1] = Editor::new("100");
                // エラー警告タブ: 警告にして通して言うだけ
                d.err_style = 1;
                d.eds[5] = Editor::new("大きすぎます");
                // メッセージを入力タブ
                d.eds[2] = Editor::new("数量");
                d.eds[3] = Editor::new("1〜100 で");
            }
            this.dv_ok(cx);
            assert!(this.dv_dlg.is_none(), "OK でパネルが閉じない");
            let v = &this.sheet().validations[0];
            assert_eq!((v.kind.as_str(), v.op.as_str()), ("whole", "between"));
            assert_eq!((v.formula.as_str(), v.formula2.as_str()), ("1", "100"));
            // 警告なので、範囲の外も通して言うだけ
            this.anchor = None;
            this.cursor = Pos::parse("B2").unwrap();
            this.sync_input();
            this.input.insert("200");
            assert!(this.commit(), "警告なのに堰き止めた");
            assert!(this.status.contains("通しました"), "{}", this.status);
            // エラーを「停止」に直すと堰き止める
            this.anchor = Some(Pos::parse("B2").unwrap());
            this.cursor = Pos::parse("B4").unwrap();
            this.run_cmd("data-validation", cx);
            {
                let d = this.dv_dlg.as_mut().unwrap();
                assert_eq!(d.kind, 1, "既存の規則がパネルに読み込まれない");
                assert_eq!(d.eds[0].text(), "1");
                d.err_style = 0; // 停止
            }
            this.dv_ok(cx);
            this.anchor = None;
            this.cursor = Pos::parse("B3").unwrap();
            this.sync_input();
            this.input.insert("999");
            assert!(!this.commit(), "999 が 1〜100 を通った");
            assert!(this.status.contains("入力規則"), "{}", this.status);
            // 範囲の中は入る
            this.input.select_all();
            this.input.insert("50");
            assert!(this.commit());
            // 入力メッセージはセルに乗ると状態行に出る
            this.cursor = Pos::parse("B4").unwrap();
            this.sync_input();
            assert!(this.status.contains("数量"), "{}", this.status);
        });
    }

    #[gpui::test]
    fn 空白を無視を外すと空も堰き止める(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            let b2 = Pos::parse("B2").unwrap();
            this.cursor = b2;
            this.sync_input();
            this.input.insert("5");
            assert!(this.commit());
            this.run_cmd("data-validation", cx);
            {
                let d = this.dv_dlg.as_mut().unwrap();
                d.kind = 1;
                d.op = 0;
                d.eds[0] = Editor::new("1");
                d.eds[1] = Editor::new("100");
                d.allow_blank = false;
            }
            this.dv_ok(cx);
            assert!(!this.sheet().validations[0].allow_blank);
            // 空にするのも堰き止められる
            this.sync_input();
            this.input.select_all();
            this.input.insert("");
            assert!(!this.commit(), "空白を無視を外したのに空が通った");
        });
    }

    #[gpui::test]
    fn 読めない種類の規則はパネルで壊れない(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // 日付の規則(判定できない種類)が既にある
            let b2 = Pos::parse("B2").unwrap();
            let mut v = sheet::model::Validation::list((b2, b2), "40000".into());
            v.kind = "date".into();
            v.op = "greaterThan".into();
            this.book.sheets[this.active].validations.push(v);
            this.cursor = b2;
            this.anchor = None;
            this.run_cmd("data-validation", cx);
            {
                let d = this.dv_dlg.as_mut().unwrap();
                assert_eq!(d.kind, 5, "読めない種類は「このまま保持」で開く");
                // 文言だけ足す
                d.eds[3] = Editor::new("日付を入れてください");
            }
            this.dv_ok(cx);
            let v = &this.sheet().validations[0];
            assert_eq!(v.kind, "date", "日付の規則が壊れた");
            assert_eq!(v.op, "greaterThan");
            assert_eq!(v.formula, "40000");
            assert_eq!(v.input_msg.as_ref().unwrap().1, "日付を入れてください");
        });
    }
}

mod numfmt_tests {
    use crate::*;

    #[gpui::test]
    fn 数値の書式は一覧とコード直打ちで掛かる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            let a1 = Pos::parse("A1").unwrap();
            this.cursor = a1;
            this.sync_input();
            this.input.insert("1234.5");
            assert!(this.commit());
            // 一覧から: パーセント
            this.run_cmd("format", cx);
            assert_eq!(this.pick_kind, "numfmt-pick");
            this.apply_pick("パーセント (12.34%)", cx);
            assert_eq!(
                this.sheet().get(a1).unwrap().fmt.number_format.as_deref(),
                Some("0.00%")
            );
            // 開き直すと今の書式に ✓ が付き、状態行にも出る(本家のコンボの追従の代わり)
            this.run_cmd("format", cx);
            {
                let (items, _) = this.pick.as_ref().expect("一覧が開かない");
                assert!(
                    // 印は**見出し**に付く(鍵は素のまま — 照合が言語で壊れない)
                    items.iter().any(|(_, l)| l == "✓ パーセント (12.34%)"),
                    "今の書式に印が付かない: {items:?}"
                );
                assert!(this.status.contains("今の書式"), "{}", this.status);
            }
            // ✓ 付きのまま選び直しても効く(印は値ではない)
            this.apply_pick("✓ パーセント (12.34%)", cx);
            assert_eq!(
                this.sheet().get(a1).unwrap().fmt.number_format.as_deref(),
                Some("0.00%")
            );
            // その他 → コード直打ち(今のコードが下敷きに入る)
            this.run_cmd("format", cx);
            this.apply_pick("その他(書式コードを打つ)…", cx);
            let (kind, ed) = this.prompt.as_ref().expect("コードのパネルが開かない");
            assert_eq!(*kind, "numfmt-custom");
            assert_eq!(ed.text(), "0.00%", "今のコードが下敷きにならない");
            this.prompt = Some(("numfmt-custom", Editor::new("#,##0.0")));
            this.finish_prompt(cx);
            assert_eq!(
                this.sheet().get(a1).unwrap().fmt.number_format.as_deref(),
                Some("#,##0.0")
            );
            // 一般に戻す
            this.run_cmd("format", cx);
            this.apply_pick("一般", cx);
            assert_eq!(this.sheet().get(a1).unwrap().fmt.number_format, None);
        });
    }
}

mod sort_tests {
    use crate::*;

    #[gpui::test]
    fn 選択の横にデータが続くときは拡張するか聞く(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // A=名前, B=数(隣り合った2列の表)
            for (a1, v) in [
                ("A1", "c"), ("B1", "3"),
                ("A2", "a"), ("B2", "1"),
                ("A3", "b"), ("B3", "2"),
            ] {
                this.cursor = Pos::parse(a1).unwrap();
                this.sync_input();
                this.input.insert(v);
                assert!(this.commit());
            }
            // A列だけ選んで昇順 → 横(B列)にデータが続くので聞かれる
            this.anchor = Some(Pos::parse("A1").unwrap());
            this.cursor = Pos::parse("A3").unwrap();
            this.sync_input();
            this.run_cmd("sort-asc", cx);
            assert_eq!(this.pick_kind, "sort-expand", "拡張の確認が出ない");
            let get = |this: &Calc, p: &str| {
                this.sheet().get(Pos::parse(p).unwrap()).map(|c| c.editable()).unwrap_or_default()
            };
            assert_eq!(get(this, "A1"), "c", "聞く前に並べ替えられた");
            // 「選択した範囲だけ」→ A列だけ並び、B列はそのまま(ずれる)
            this.apply_pick("選択した範囲だけ並べ替え(横の列とはずれます)", cx);
            assert_eq!(
                (get(this, "A1"), get(this, "A2"), get(this, "A3")),
                ("a".into(), "b".into(), "c".into())
            );
            assert_eq!(
                (get(this, "B1"), get(this, "B2"), get(this, "B3")),
                ("3".into(), "1".into(), "2".into()),
                "選択の外まで動いた"
            );
            // 「拡張して」→ 表全体が行ごと動く(1行目は見出しとして据え置き、
            // 残りが A の降順。B が行ごと付いてくる)
            this.run_cmd("sort-desc", cx);
            assert_eq!(this.pick_kind, "sort-expand");
            this.apply_pick("拡張して並べ替え(続きの列も一緒に動く)", cx);
            assert_eq!(get(this, "A2"), "c");
            assert_eq!(get(this, "B2"), "2", "拡張なのに行が付いてこない");
            // 横に何も無い離れ小島は、聞かずに選択だけを並べ替える
            for (a1, v) in [("E1", "2"), ("E2", "1")] {
                this.cursor = Pos::parse(a1).unwrap();
                this.sync_input();
                this.input.insert(v);
                assert!(this.commit());
            }
            this.anchor = Some(Pos::parse("E1").unwrap());
            this.cursor = Pos::parse("E2").unwrap();
            this.sync_input();
            this.run_cmd("sort-asc", cx);
            assert_eq!(get(this, "E1"), "1", "島の並べ替えが効かない");
            assert_eq!(this.pick_kind, "value", "島なのに聞いた");
        });
    }

    #[gpui::test]
    fn 複数の基準で並べ替える(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for (a1, v) in [
                ("A1", "区分"), ("B1", "数"),
                ("A2", "甲"), ("B2", "1"),
                ("A3", "乙"), ("B3", "2"),
                ("A4", "甲"), ("B4", "3"),
                ("A5", "丙"), ("B5", "4"),
            ] {
                this.cursor = Pos::parse(a1).unwrap();
                this.sync_input();
                this.input.select_all();
                this.input.insert(v);
                assert!(this.commit());
            }
            let col_a = |this: &Calc| -> Vec<String> {
                (1..5)
                    .map(|r| this.sheet().value(Pos::new(r, 0)).display())
                    .collect()
            };
            // 見出し名で2基準: 区分 降順 → 同じ区分の中は 数 降順
            this.prompt = Some(("sort-by", Editor::new("区分 降順, 数 降順")));
            this.finish_prompt(cx);
            assert_eq!(col_a(this), ["甲", "甲", "乙", "丙"], "1つ目の基準が効かない");
            assert_eq!(
                this.sheet().value(Pos::parse("B2").unwrap()),
                sheet::Value::Number(3.0),
                "2つ目の基準(数 降順)が効かない"
            );
            // 列の字でも指せる(B 昇順)
            this.prompt = Some(("sort-by", Editor::new("B")));
            this.finish_prompt(cx);
            assert_eq!(col_a(this), ["甲", "乙", "甲", "丙"], "列の字の基準が効かない");
            // 知らない見出しはパネルを開いたまま言い返す
            this.prompt = Some(("sort-by", Editor::new("存在しない列")));
            this.finish_prompt(cx);
            assert!(this.prompt.is_some(), "打ち直せるようにパネルが残るはず");
            assert!(this.status.contains("見つかりません"), "{}", this.status);
        });
    }
}

mod filter_tests {
    use crate::*;

    fn seed(this: &mut Calc) {
        for (a1, v) in [
            ("A1", "区分"), ("B1", "数"),
            ("A2", "甲"), ("B2", "1"),
            ("A3", "乙"), ("B3", "2"),
            ("A4", "甲"), ("B4", "3"),
            ("A5", "丙"), ("B5", "4"),
        ] {
            this.cursor = Pos::parse(a1).unwrap();
            this.sync_input();
            this.input.select_all();
            this.input.insert(v);
            assert!(this.commit());
        }
        this.anchor = None;
        this.cursor = Pos::parse("A1").unwrap();
        this.sync_input();
    }

    #[gpui::test]
    fn 値の入切で行が隠れて数も件数も追随する(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            seed(this);
            this.run_cmd("setfilter", cx); // 表全体 A1:B5 に範囲を張る
            let f = this.auto_filter.as_ref().expect("範囲が張られない");
            assert_eq!(f.range, (Pos::parse("A1").unwrap(), Pos::parse("B5").unwrap()));
            // パネルの一覧: A列の値と件数(BTreeMap の並び=文字順)
            let (vals, cut) = this.filter_values(0);
            assert!(!cut);
            assert_eq!(
                vals,
                vec![("丙".into(), 1), ("乙".into(), 1), ("甲".into(), 2)],
                "値の一覧が違う"
            );
            // 乙と丙を隠す → 見出し+甲の2行が残る
            this.filter_toggle_value(0, "乙");
            this.filter_toggle_value(0, "丙");
            assert!(this.filter_active());
            assert_eq!(this.filter_counts(), Some((4, 2)), "行の数が違う");
            assert_eq!(this.visible_rows(), vec![0, 1, 3], "見える行が違う");
            // 他の列の一覧は絞り込みを効かせたまま: B列は甲の行の値だけ
            let (bv, _) = this.filter_values(1);
            assert_eq!(bv, vec![("1".into(), 1), ("3".into(), 1)]);
            // 入切で戻る(空になったら列ごと素通し)
            this.filter_toggle_value(0, "乙");
            this.filter_toggle_value(0, "丙");
            assert!(!this.filter_active(), "全部見せたのに絞られている");
            // (すべて選択)を切る → 全部隠れる → もう一度で全部戻る
            let all: Vec<String> =
                this.filter_values(0).0.into_iter().map(|(v, _)| v).collect();
            this.filter_toggle_all(0, all.clone());
            assert_eq!(this.filter_counts(), Some((4, 0)));
            this.filter_toggle_all(0, all);
            assert!(!this.filter_active());
            // もう一度 setfilter で範囲ごと外れる
            this.run_cmd("setfilter", cx);
            assert!(this.auto_filter.is_none(), "トグルで外れない");
        });
    }

    #[gpui::test]
    fn 絞り込みは生きた値にも効く(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            seed(this);
            this.run_cmd("setfilter", cx);
            this.filter_toggle_value(0, "乙");
            this.filter_toggle_value(0, "丙");
            // B2:B5 を選ぶと、見えている甲の行(1と3)だけ数える
            this.anchor = Some(Pos::parse("B2").unwrap());
            this.cursor = Pos::parse("B5").unwrap();
            let s = this.sel_stats().expect("生きた値が出ない");
            assert!(s.contains("合計 4"), "隠れた行を数えている: {s}");
            assert!(s.contains("個数 2"), "個数が違う: {s}");
        });
    }
}

#[cfg(test)]
mod sheet_name_tests {
    use crate::*;

    #[test]
    fn 足すシートの名前がぶつからない() {
        let mut b = Book::new(); // Sheet1
        assert_eq!(unique_sheet_name(&b), "Sheet2");
        b.sheets.push(sheet::Sheet::new("Sheet2"));
        b.sheets.push(sheet::Sheet::new("Sheet3"));
        assert_eq!(unique_sheet_name(&b), "Sheet4");
        // 歯抜け(途中の名前が消えた等)でも重複しない
        b.sheets.remove(1);
        let n = unique_sheet_name(&b);
        assert!(!b.sheets.iter().any(|s| s.name == n), "重複した: {n}");
    }
}

#[cfg(test)]
mod clipboard_tests {
    use crate::*;

    fn table() -> sheet::Sheet {
        let mut s = sheet::Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::new(0, 0), Cell::input("品名"));
        s.set(Pos::new(0, 1), Cell::input("金額"));
        s.set(Pos::new(1, 0), Cell::input("甲"));
        s.set(Pos::new(1, 1), Cell::input("=A2&\"円\""));
        s
    }

    #[test]
    fn コピーはtsvで式が残る() {
        let s = table();
        let tsv = range_tsv(&s, Pos::new(0, 0), Pos::new(1, 1));
        assert_eq!(tsv, "品名\t金額\n甲\t=A2&\"円\"", "TSV の形が違う: {tsv:?}");
    }

    #[test]
    fn 空セルは空欄として出る() {
        let s = table();
        let tsv = range_tsv(&s, Pos::new(0, 0), Pos::new(2, 1));
        assert!(tsv.ends_with("\n\t"), "空行の形が違う: {tsv:?}");
    }

    #[test]
    fn アプリ内の貼り付けは式がずれる() {
        let mut s = table();
        // B2 の式(=A2&"円")を B4 へ: 2行下 → =A4&"円"
        let grid = vec![vec!["=A2&\"円\"".to_string()]];
        paste_grid(&mut s, Pos::new(3, 1), &grid, Some((2, 0)));
        assert_eq!(
            s.get(Pos::new(3, 1)).and_then(|c| c.formula.clone()).as_deref(),
            Some("A4&\"円\""),
            "式の参照がずれていない"
        );
    }

    #[test]
    fn 外から来たtsvは式をずらさない() {
        let mut s = sheet::Sheet { name: "表".into(), ..Default::default() };
        let grid = tsv_grid("甲\t100\r\n乙\t=A1*2\n");
        let n = paste_grid(&mut s, Pos::new(0, 0), &grid, None);
        assert_eq!(n, 4);
        assert_eq!(s.value(Pos::new(0, 1)), Value::Number(100.0));
        assert_eq!(
            s.get(Pos::new(1, 1)).and_then(|c| c.formula.clone()).as_deref(),
            Some("A1*2"),
            "外来の式を勝手にずらした"
        );
    }

    #[test]
    fn 貼り付けても書式は据え置き() {
        // 帳票の枠(罫線)の上に値を貼っても枠が残る
        let mut s = sheet::Sheet { name: "枠".into(), ..Default::default() };
        s.set(Pos::new(0, 0), Cell {
            formula: None,
            value: Value::Empty,
            fmt: CellFormat { borders: Borders::ALL, ..Default::default() },
        });
        paste_grid(&mut s, Pos::new(0, 0), &[vec!["100".to_string()]], None);
        let c = s.get(Pos::new(0, 0)).unwrap();
        assert_eq!(c.value, Value::Number(100.0));
        assert_eq!(c.fmt.borders, Borders::ALL, "貼り付けで罫線が消えた");
    }

    #[test]
    fn 値だけの貼り付けで式が値になる() {
        let mut s = table();
        recalc(&mut s);
        // B2(=A2&"円")を控えて、値だけを B4 へ
        let cells = vec![vec![s.get(Pos::new(1, 1)).cloned()]];
        paste_values_cells(&mut s, Pos::new(3, 1), &cells);
        let c = s.get(Pos::new(3, 1)).unwrap();
        assert!(c.formula.is_none(), "式が残っている");
        assert_eq!(c.value, Value::Text("甲円".into()), "計算結果の値になっていない");
    }

    #[test]
    fn 外来の式もどきは文字として貼る() {
        let mut s = sheet::Sheet { name: "表".into(), ..Default::default() };
        paste_values_text(&mut s, Pos::new(0, 0), &[vec!["=A1*2".to_string()]]);
        let c = s.get(Pos::new(0, 0)).unwrap();
        assert!(c.formula.is_none(), "外の式を黙って式にした");
        assert_eq!(c.value, Value::Text("=A1*2".into()));
    }

    #[test]
    fn 書式だけの貼り付けで中身は残る() {
        let mut s = sheet::Sheet { name: "枠".into(), ..Default::default() };
        s.set(Pos::new(0, 0), Cell::input("100"));
        let src = Some(Cell {
            formula: None,
            value: Value::Empty,
            fmt: CellFormat { borders: Borders::ALL, ..Default::default() },
        });
        paste_formats(&mut s, Pos::new(0, 0), &[vec![src]]);
        let c = s.get(Pos::new(0, 0)).unwrap();
        assert_eq!(c.value, Value::Number(100.0), "書式だけのはずが中身が消えた");
        assert_eq!(c.fmt.borders, Borders::ALL, "書式が写っていない");
    }

    #[test]
    fn 転置で行と列が入れ替わる() {
        let g = vec![
            vec!["a".to_string(), "b".into(), "c".into()],
            vec!["1".to_string(), "2".into()],
        ];
        let t = transpose(&g);
        assert_eq!(t.len(), 3, "列の数が行にならない");
        assert_eq!(t[0], vec!["a".to_string(), "1".into()]);
        assert_eq!(t[2], vec!["c".to_string(), "".into()], "歯抜けが埋まらない");
    }

    #[test]
    fn 改行コードと末尾改行を受け流す() {
        assert_eq!(tsv_grid("a\tb\r\nc\td\r\n"),
                   vec![vec!["a".to_string(), "b".into()], vec!["c".into(), "d".into()]]);
        assert_eq!(tsv_grid("1"), vec![vec!["1".to_string()]]);
    }
}

#[cfg(test)]
mod table_design_tests {
    use crate::*;

    #[test]
    fn 合計行は見出しを外して数の列だけ足す() {
        let mut s = sheet::Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::new(0, 0), Cell::input("品名"));
        s.set(Pos::new(0, 1), Cell::input("金額"));
        s.set(Pos::new(1, 0), Cell::input("甲"));
        s.set(Pos::new(1, 1), Cell::input("100"));
        s.set(Pos::new(2, 0), Cell::input("乙"));
        s.set(Pos::new(2, 1), Cell::input("50"));
        add_total_row(&mut s, Pos::new(0, 0), Pos::new(2, 1));
        recalc(&mut s);
        let label = s.get(Pos::new(3, 0)).unwrap();
        assert_eq!(label.value.display(), "合計", "文字の列の先頭は札");
        assert!(label.fmt.bold && label.fmt.borders.top.on, "合計行の書式が付かない");
        let sum = s.get(Pos::new(3, 1)).unwrap();
        assert_eq!(
            sum.formula.as_deref(),
            Some("SUM(B2:B3)"),
            "見出しが合計に混ざった: {:?}",
            sum.formula
        );
        assert_eq!(sum.value.display(), "150");
    }

    #[test]
    fn 見出しの無い表は全行を合計する() {
        let mut s = sheet::Sheet { name: "表".into(), ..Default::default() };
        for (r, v) in [(0, "10"), (1, "20")] {
            s.set(Pos::new(r, 0), Cell::input(v));
        }
        add_total_row(&mut s, Pos::new(0, 0), Pos::new(1, 0));
        recalc(&mut s);
        let sum = s.get(Pos::new(2, 0)).unwrap();
        assert_eq!(sum.formula.as_deref(), Some("SUM(A1:A2)"));
        assert_eq!(sum.value.display(), "30");
    }
}

#[cfg(test)]
mod subtotal_tests {
    use crate::*;

    #[test]
    fn 小計と総計が入り明細だけ畳まれる() {
        let mut s = sheet::Sheet { name: "表".into(), ..Default::default() };
        for (r, row) in [
            ["部署", "月", "金額"],
            ["営業", "1月", "100"],
            ["営業", "1月", "50"],
            ["営業", "2月", "70"],
            ["総務", "1月", "30"],
        ]
        .iter()
        .enumerate()
        {
            for (c, v) in row.iter().enumerate() {
                s.set(Pos::new(r as u32, c as u32), Cell::input(v));
            }
        }
        let n = apply_subtotals(&mut s, Pos::new(0, 0), Pos::new(4, 2), 0, &[2]);
        recalc(&mut s);
        assert_eq!(n, 2, "区切りの数が違う");
        // 並び: 1見出し 2-4営業明細 5営業小計 6総務明細 7総務小計 8総計
        let d = |r: u32, c: u32| s.get(Pos::new(r, c)).map(|x| x.value.display()).unwrap_or_default();
        assert_eq!(d(4, 0), "営業 小計");
        assert_eq!(d(4, 2), "220", "営業の小計が違う");
        assert_eq!(
            s.get(Pos::new(4, 2)).and_then(|c| c.formula.clone()).as_deref(),
            Some("SUM(C2:C4)"),
            "小計が式でない"
        );
        assert_eq!(d(6, 0), "総務 小計");
        assert_eq!(d(6, 2), "30");
        assert_eq!(d(7, 0), "総計");
        assert_eq!(d(7, 2), "250", "総計が違う");
        // 明細だけグループ化(小計・総計はされない → 畳んでも残る)
        for r in [1, 2, 3, 5] {
            assert_eq!(s.row_outline.get(&r), Some(&1), "明細 {r} が畳めない");
        }
        for r in [0, 4, 6, 7] {
            assert!(!s.row_outline.contains_key(&r), "行 {r} まで畳まれてしまう");
        }
    }

    #[test]
    fn 行の挿抜でグループ化が付いてくる() {
        let mut s = sheet::Sheet { name: "表".into(), ..Default::default() };
        s.row_outline.insert(5, 1);
        s.row_hidden.insert(5);
        s.insert_row(2);
        assert_eq!(s.row_outline.get(&6), Some(&1), "挿入で深さが置き去り");
        assert!(s.row_hidden.contains(&6), "挿入で畳みが置き去り");
        s.remove_row(0);
        assert_eq!(s.row_outline.get(&5), Some(&1), "削除で深さが置き去り");
        assert!(s.row_hidden.contains(&5));
    }
}

#[cfg(test)]
mod solver_tests {
    use crate::*;

    #[test]
    fn セルと範囲の列挙が読める() {
        let v = parse_cell_list("B2:B4", 64).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0], Pos::new(1, 1));
        let v = parse_cell_list("$A$1, C3", 64).unwrap();
        assert_eq!(v, vec![Pos::new(0, 0), Pos::new(2, 2)]);
        assert!(parse_cell_list("ほげ", 64).is_none(), "読めないものは None");
        assert!(parse_cell_list("A1:Z99", 10).is_none(), "上限を超えたら None");
        assert!(parse_cell_list("", 64).is_none());
    }

    #[test]
    fn 台本が実際にscipyで回る() {
        // .venv が無い機械では黙って飛ぶ(HIKITSUGI の作法)
        let py = ["../.venv/bin/python", ".venv/bin/python"]
            .iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.exists());
        let Some(py) = py else { return };
        // max x+2y  s.t. x+y<=4, x<=2, x,y>=0 → x=0,y=4(目的8)
        let dir = std::env::temp_dir().join(format!("jo-solver-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let spec = "{\"c\":[-1,-2],\"aub\":[[1,1],[1,0]],\"bub\":[4,2],\"aeq\":[],\"beq\":[],\"nonneg\":true}";
        let json_path = dir.join("solver.json");
        let py_path = dir.join("solver.py");
        std::fs::write(&json_path, spec).unwrap();
        std::fs::write(&py_path, SOLVER_PY).unwrap();
        let o = std::process::Command::new(&py)
            .arg(&py_path)
            .arg(&json_path)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let out = String::from_utf8_lossy(&o.stdout).to_string();
        let xs: Vec<f64> =
            out.split('\u{1f}').filter_map(|v| v.trim().parse().ok()).collect();
        assert_eq!(xs.len(), 2, "答えの形が違う: {out:?}");
        assert!(xs[0].abs() < 1e-6 && (xs[1] - 4.0).abs() < 1e-6,
                "最適解が違う: {xs:?}");
    }
}

#[cfg(test)]
mod equation_tests {
    use crate::*;

    #[test]
    fn 台本が実際にmathtextで清書する() {
        // .venv が無い機械では黙って飛ぶ(HIKITSUGI の作法)
        let py = ["../.venv/bin/python", ".venv/bin/python"]
            .iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.exists());
        let Some(py) = py else { return };
        let dir = std::env::temp_dir().join(format!("jo-eq-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("eq.png");
        let spec = format!(
            "{{\"tex\":\"\\\\frac{{a}}{{b}}+\\\\sqrt{{x^2+1}}\",\"font\":\"\",\"out\":\"{}\"}}",
            out.to_string_lossy()
        );
        let json_path = dir.join("eq.json");
        let py_path = dir.join("eq.py");
        std::fs::write(&json_path, spec).unwrap();
        std::fs::write(&py_path, EQ_PY).unwrap();
        let o = std::process::Command::new(&py)
            .arg(&py_path)
            .arg(&json_path)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let data = std::fs::read(&out).unwrap();
        assert!(data.starts_with(&[0x89, b'P', b'N', b'G']), "PNG が出ていない");
        let (w, h) = image_px(&data).expect("大きさが読めない");
        assert!(w > 40 && h > 20, "清書が小さすぎる: {w}x{h}");
        // テキストアートも同じ道(飾り文字が PNG になる)
        let ta = format!(
            "{{\"tex\":\"見積書\",\"font\":\"\",\"out\":\"{}\"}}",
            out.to_string_lossy()
        );
        std::fs::write(&json_path, ta).unwrap();
        std::fs::write(&py_path, TEXTART_PY).unwrap();
        let o = std::process::Command::new(&py)
            .arg(&py_path)
            .arg(&json_path)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let data = std::fs::read(&out).unwrap();
        assert!(data.starts_with(&[0x89, b'P', b'N', b'G']), "テキストアートが PNG でない");
        // 読めない式は黙って白紙にせず、ちゃんと失敗する(台本を式のものに戻す)
        std::fs::write(&py_path, EQ_PY).unwrap();
        let bad = format!(
            "{{\"tex\":\"\\\\frac{{a\",\"font\":\"\",\"out\":\"{}\"}}",
            out.to_string_lossy()
        );
        std::fs::write(&json_path, bad).unwrap();
        let o = std::process::Command::new(&py)
            .arg(&py_path)
            .arg(&json_path)
            .output()
            .unwrap();
        assert!(!o.status.success(), "壊れた式が通ってしまった");
    }
}

#[cfg(test)]
mod pivot_tests {
    use crate::*;

    #[test]
    fn 見出しの列挙はカンマでも読点でも空白でも() {
        assert_eq!(split_fields("部署, 月"), vec!["部署", "月"]);
        assert_eq!(split_fields("部署、月 区分"), vec!["部署", "月", "区分"]);
        assert!(split_fields("  ").is_empty());
    }

    #[gpui::test]
    fn ピボットの行列値は一覧のクリックで選ぶ(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for (a1, v) in [
                ("A1", "区分"), ("B1", "月"), ("C1", "金額"),
                ("A2", "筆記具"), ("B2", "4月"), ("C2", "100"),
                ("A3", "紙製品"), ("B3", "5月"), ("C3", "200"),
            ] {
                this.cursor = Pos::parse(a1).unwrap();
                this.sync_input();
                this.input.insert(v);
                assert!(this.commit());
            }
            // 範囲選択なし・表の中にカーソルだけで開く(発注者指摘 2026-08-07)
            this.anchor = None;
            this.cursor = Pos::parse("B2").unwrap();
            this.sync_input();
            this.run_cmd("pivot-insert", cx);
            assert_eq!(this.pick_kind, "pivot-rows-pick", "カーソルだけで行の一覧が開かない");
            // 見出しを選ばず決定 → 言い返されて一覧のまま
            this.apply_pick("→ 決定(列の選択へ)", cx);
            assert_eq!(this.pick_kind, "pivot-rows-pick", "空のまま先へ進んだ");
            // クリックで入切(✓ 付きでもう一度押すと外れる)
            this.apply_pick("☐ 区分", cx);
            this.apply_pick("☑ 区分", cx);
            assert!(this.pivot_pend.as_ref().unwrap().rows_sel.is_empty(), "入切が効かない");
            this.apply_pick("区分", cx);
            {
                let (items, _) = this.pick.as_ref().unwrap();
                assert!(items.iter().any(|(_, l)| l == "☑ 区分"), "選んだ印が付かない: {items:?}");
            }
            this.apply_pick("→ 決定(列の選択へ)", cx);
            assert_eq!(this.pick_kind, "pivot-cols-pick");
            {
                // 行に使った見出しは列の候補に出ない
                let (items, _) = this.pick.as_ref().unwrap();
                assert!(!items.iter().any(|(k, _)| k.contains("区分")), "{items:?}");
            }
            this.apply_pick("☐ 月", cx);
            this.apply_pick("→ 決定(列は無しでもよい)", cx);
            assert_eq!(this.pick_kind, "pivot-val-pick");
            this.apply_pick("金額", cx);
            assert_eq!(this.pick_kind, "pivot-agg-pick", "集計の一覧が開かない");
            let p = this.pivot_pend.as_ref().unwrap();
            assert_eq!(p.rows_sel, vec!["区分"]);
            assert_eq!(p.cols_sel, vec!["月"]);
            assert_eq!(p.val_sel, "金額");
            // ここでは polars は回さない(集計を選ぶと insert_pivot へ)。
            // Esc でやめられることだけ確かめる
            this.pivot_pend = None;
            this.pick = None;
            this.pick_kind = "value";
        });
    }

    fn def(rows: &[&str], cols: &[&str], value: &str, agg: &str) -> sheet::model::PivotDef {
        sheet::model::PivotDef {
            sheet: "S".into(),
            src: (Pos::new(0, 0), Pos::new(1, 1)),
            rows_sel: rows.iter().map(|s| s.to_string()).collect(),
            cols_sel: cols.iter().map(|s| s.to_string()).collect(),
            value: value.into(),
            agg: agg.into(),
            totals: false,
            subtotals: false,
            blank_rows: false,
            compact: false,
            dest: Pos::new(0, 0),
            size: (0, 0),
            hide: Vec::new(),
            style: String::new(),
            name: String::new(),
            vfilter: None,
            group_by: Vec::new(),
            show_as: String::new(),
            sort: String::new(),
        }
    }

    #[test]
    fn 指図のjsonは逃がしが効く() {
        let json = pivot_spec_json(
            &["部\"署".to_string()],
            &[vec!["営\\業".to_string()]],
            &def(&["部\"署"], &[], "部\"署", "合計"),
        );
        assert!(json.contains("部\\\"署"), "二重引用符が逃げていない: {json}");
        assert!(json.contains("営\\\\業"), "バックスラッシュが逃げていない: {json}");
        assert!(json.contains("\"totals\":false"), "旗が無い: {json}");
    }

    fn run_py(spec: String) -> Option<(Vec<Vec<String>>, Vec<char>)> {
        // .venv が無い機械では黙って飛ぶ(HIKITSUGI の作法)
        let py = ["../.venv/bin/python", ".venv/bin/python"]
            .iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.exists())?;
        // 並走する試験と取り合わないよう、呼び出しごとに番号を振る
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("jo-pivot-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let json_path = dir.join(format!("pivot{n}.json"));
        let py_path = dir.join(format!("pivot{n}.py"));
        std::fs::write(&json_path, spec).unwrap();
        std::fs::write(&py_path, PIVOT_PY).unwrap();
        let o = std::process::Command::new(&py)
            .arg(&py_path)
            .arg(&json_path)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        Some(parse_pivot_grid(&String::from_utf8_lossy(&o.stdout)))
    }

    #[gpui::test]
    fn 罫線のパネルは連打でき表の形が1押しで掛かる(cx: &mut gpui::TestAppContext) {
        use sheet::model::BStyle;
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.anchor = Some(Pos::new(0, 0));
            this.cursor = Pos::new(1, 1);
            this.sync_input();
            this.run_cmd("borders", cx);
            assert!(this.border_pal.is_some(), "パレットが開かない");
            fn bd(this: &Calc, r: u32, c2: u32) -> sheet::model::Borders {
                this.book.sheets[0].get(Pos::new(r, c2)).unwrap().fmt.borders
            }
            // 場所×ペンの直交モデル(Microsoft の型スタンプは持たない —
            // 発注者確定 2026-08-08)。帳票の枠はペンを替えながら連打で組む:
            // 細で格子 → ペンを中太にして外枠 → ペンを二重にして下罫線
            this.apply_borders("すべての罫線(格子)");
            assert!(this.border_pal.is_some(), "パレットが閉じた(連打できない)");
            this.pen_style = BStyle::Medium;
            this.apply_borders("外枠");
            assert_eq!(bd(this, 0, 0).top.style, BStyle::Medium);
            assert_eq!(bd(this, 0, 0).left.style, BStyle::Medium);
            assert_eq!(bd(this, 0, 0).bottom.style, BStyle::Thin, "格子の内側が外枠で潰れた");
            assert_eq!(bd(this, 0, 0).right.style, BStyle::Thin);
            assert_eq!(bd(this, 1, 1).bottom.style, BStyle::Medium);
            assert_eq!(bd(this, 1, 1).right.style, BStyle::Medium);
            this.pen_style = BStyle::Double;
            this.apply_borders("下罫線");
            let _ = cx;
            assert_eq!(bd(this, 1, 0).bottom.style, BStyle::Double);
            assert_eq!(bd(this, 1, 1).bottom.style, BStyle::Double);
        });
    }

    #[gpui::test]
    fn jocalcの口は読み書きと展開ができる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // 書く: 数・文字・式(= から始まる)
            let r = crate::rpc::handle(
                this,
                r#"{"cmd":"set","a1":"A1","values":[["品名","金額"],["鉛筆",100],["ノート",250],["合計","=SUM(B2:B3)"]]}"#,
                cx,
            );
            assert!(r.contains("\"ok\":true"), "set が通らない: {r}");
            // 読む: 式は計算済みの値、数は数のまま
            let r = crate::rpc::handle(this, r#"{"cmd":"get","a1":"B4"}"#, cx);
            assert!(r.contains("[[350]]"), "式が計算されていない: {r}");
            let r = crate::rpc::handle(this, r#"{"cmd":"get","a1":"A1:B2"}"#, cx);
            assert!(r.contains("\"品名\""), "文字が読めない: {r}");
            assert!(r.contains("100"), "数が読めない: {r}");
            // 式そのもの
            let r = crate::rpc::handle(this, r#"{"cmd":"get_formula","a1":"B4"}"#, cx);
            assert!(r.contains("=SUM(B2:B3)"), "式が読めない: {r}");
            // 表の広がり(expand='table')
            let r = crate::rpc::handle(this, r#"{"cmd":"expand","a1":"A1"}"#, cx);
            assert!(r.contains("\"rows\":4") && r.contains("\"cols\":2"), "広がりが違う: {r}");
            // ブックの情報
            let r = crate::rpc::handle(this, r#"{"cmd":"book_info"}"#, cx);
            assert!(r.contains("Sheet1"), "シート名が出ない: {r}");
            // 無いシートは正しく断る
            let r = crate::rpc::handle(this, r#"{"cmd":"get","a1":"A1","sheet":"無い"}"#, cx);
            assert!(r.contains("err"), "無いシートを断らない: {r}");
            // 保護中は書かせない
            this.book.sheets[0].protected = true;
            let r = crate::rpc::handle(this, r#"{"cmd":"set","a1":"A9","values":[[1]]}"#, cx);
            assert!(r.contains("保護"), "保護を破って書けてしまう: {r}");
            this.book.sheets[0].protected = false;
            // 未保存の変更がある間は new/open を断る(黙って捨てない)
            let r = crate::rpc::handle(this, r#"{"cmd":"new"}"#, cx);
            assert!(r.contains("err"), "未保存で new が通った: {r}");
        });
    }

    #[gpui::test]
    fn 左上が空の結合は最初の値を左上へ移す(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            // A1 空、B1 に題 — A1:C1 を結合すると題が左上へ移る
            this.book.sheets[0].set(Pos::new(0, 1), sheet::Cell::input("題"));
            this.merge_do(Pos::new(0, 0), Pos::new(0, 2), "中央");
            assert_eq!(
                this.book.sheets[0].get(Pos::new(0, 0)).unwrap().value.display(),
                "題", "最初の値が左上へ移らない"
            );
            assert!(
                this.book.sheets[0]
                    .get(Pos::new(0, 1))
                    .is_none_or(|c| c.value.is_empty()),
                "元の場所に残っている"
            );
            assert!(this.status.contains("移しました"), "移したことを言わない: {}", this.status);
            // 「セルの結合」: 書式は値があった場所の書式ごと移り、中央揃えにはしない
            let mut src = sheet::Cell::input("題2");
            src.fmt.bold = true;
            src.fmt.align = sheet::model::HAlign::Right;
            this.book.sheets[0].set(Pos::new(5, 1), src);
            this.merge_do(Pos::new(5, 0), Pos::new(5, 2), "結合だけ");
            let got = this.book.sheets[0].get(Pos::new(5, 0)).unwrap().clone();
            assert_eq!(got.value.display(), "題2");
            assert!(got.fmt.bold, "書式(太字)が移らない");
            assert_eq!(got.fmt.align, sheet::model::HAlign::Right, "揃えまで変えられた");
            // 「結合して中央に配置」だけが中央を掛ける
            let mut src = sheet::Cell::input("題3");
            src.fmt.bold = true;
            this.book.sheets[0].set(Pos::new(7, 1), src);
            this.merge_do(Pos::new(7, 0), Pos::new(7, 2), "中央");
            let got = this.book.sheets[0].get(Pos::new(7, 0)).unwrap().clone();
            assert!(got.fmt.bold, "中央配置で書式が落ちた");
            assert_eq!(got.fmt.align, sheet::model::HAlign::Center, "中央配置が中央でない");
            // 横方向: 行ごとに同じ扱い(2行目は C2 の値が左端へ)
            this.book.sheets[0].set(Pos::new(2, 2), sheet::Cell::input("乙"));
            this.merge_do(Pos::new(2, 0), Pos::new(2, 2), "横方向");
            assert_eq!(
                this.book.sheets[0].get(Pos::new(2, 0)).unwrap().value.display(),
                "乙", "横方向で行の左端へ移らない"
            );
        });
    }

    #[gpui::test]
    fn 結合は1つのセルとして歩ける(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            // B1:C2 を結合
            this.merge_do(Pos::new(0, 1), Pos::new(1, 2), "中央");
            // 右へ: A1 → B1(左上)→ D1(結合を飛び越す)
            this.cursor = Pos::new(0, 0);
            this.sync_input();
            this.move_cursor(0, 1);
            assert_eq!(this.cursor, Pos::new(0, 1), "結合に入ると左上に立つ");
            this.move_cursor(0, 1);
            assert_eq!(this.cursor, Pos::new(0, 3), "結合を1つとして飛び越す");
            // 左へ戻る: D1 → B1(左上)→ A1
            this.move_cursor(0, -1);
            assert_eq!(this.cursor, Pos::new(0, 1));
            this.move_cursor(0, -1);
            assert_eq!(this.cursor, Pos::new(0, 0));
            // 下から入っても左上へ(C3 の上=C2 は呑まれている)
            this.cursor = Pos::new(2, 2);
            this.sync_input();
            this.move_cursor(-1, 0);
            assert_eq!(this.cursor, Pos::new(0, 1), "下から入っても左上に立つ");
            // merge_of がクリックの吸い付け先を返す
            assert_eq!(
                this.merge_of(Pos::new(1, 2)),
                Some((Pos::new(0, 1), Pos::new(1, 2)))
            );
        });
    }

    #[gpui::test]
    fn 一覧が開くボタンは押した所に一覧を出す(cx: &mut gpui::TestAppContext) {
        // **位置の直書きの見張り。** 一覧を出す命令が pop_anchor を通さず
        // 座標を直に書いていると、どのボタンから開いても画面の左端に出る。
        // 実機の一巡点検(tools/ribbon_sweep.py)がこれを6箇所見つけたので、
        // 画面なしでも捕まえられるようここに固定する
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.book.sheets[0].set(Pos::new(0, 0), sheet::Cell::input("1"));
            this.book.sheets[0].set(Pos::new(1, 0), sheet::Cell::input("2"));
            let mark = (777.0, 55.0);
            let mut seen = 0;
            for id in Calc::DROP_IDS {
                this.pick = None;
                this.cursor = Pos::new(0, 0);
                this.anchor = None;
                this.pop_at = Some(mark);
                this.run_cmd(id, cx);
                this.pop_at = None;
                if let Some((_, at)) = this.pick.clone() {
                    assert_eq!(at, mark, "{id} の一覧が押した所でなく {at:?} に出た");
                    seen += 1;
                }
                this.pick = None;
                this.menu_at = None;
                this.fmt_panel = None;
                this.border_pal = None;
                this.prompt = None;
            }
            assert!(seen >= 10, "一覧が開いた命令が {seen} 件しかない — 見張りになっていない");
        });
    }

    #[gpui::test]
    fn ホームの全ボタンを一巡り点検(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            use sheet::model::{HAlign, VAlign};
            this.book.sheets[0].set(Pos::new(0, 0), sheet::Cell::input("abc"));
            this.cursor = Pos::new(0, 0);
            this.sync_input();
            let f = |this: &Calc| this.book.sheets[0].get(Pos::new(0, 0)).unwrap().fmt.clone();
            // --- 書式の掛かり(モデル) ---
            for id in ["bold", "italic", "underline", "strikeout", "subscript", "wrap", "direction"] {
                this.run_cmd(id, cx);
            }
            let g = f(this);
            assert!(g.bold && g.italic && g.underline && g.strike && g.subscript && g.wrap && g.rtl_text,
                "文字飾りが掛からない: {g:?}");
            this.run_cmd("top", cx);
            assert_eq!(f(this).valign, VAlign::Top);
            this.run_cmd("middle", cx);
            assert_eq!(f(this).valign, VAlign::Middle);
            this.run_cmd("bottom", cx);
            assert_eq!(f(this).valign, VAlign::Bottom);
            this.run_cmd("align-left", cx);
            assert_eq!(f(this).align, HAlign::Left);
            this.run_cmd("align-center", cx);
            assert_eq!(f(this).align, HAlign::Center);
            this.run_cmd("align-right", cx);
            assert_eq!(f(this).align, HAlign::Right);
            this.run_cmd("align-just", cx);
            assert_eq!(f(this).align, HAlign::Justify);
            this.run_cmd("incfont", cx);
            assert_eq!(f(this).size_c, Some(1200), "文字が大きくならない");
            this.run_cmd("decfont", cx);
            assert_eq!(f(this).size_c, Some(1100));
            // **通貨は押した瞬間には掛からない** — 通貨を選ぶ一覧が開く
            // (2026-08-10 発注者確定: お金は帳票のものなので選ばせる)
            this.run_cmd("currency", cx);
            assert_eq!(this.pick_kind, "currency", "通貨の一覧が開かない");
            assert_eq!(f(this).number_format, None, "選ぶ前に掛かっている");
            this.apply_pick("円 (¥)", cx);
            assert_eq!(f(this).number_format.as_deref(), Some("\"¥\"#,##0"));
            this.run_cmd("percents", cx);
            assert_eq!(f(this).number_format.as_deref(), Some("0%"));
            this.run_cmd("comma", cx);
            assert_eq!(f(this).number_format.as_deref(), Some("#,##0"));
            this.run_cmd("digit-inc", cx);
            assert_eq!(f(this).number_format.as_deref(), Some("#,##0.0"));
            this.run_cmd("digit-dec", cx);
            assert_eq!(f(this).number_format.as_deref(), Some("#,##0"));
            this.run_cmd("clear", cx);
            assert_eq!(f(this), Default::default(), "書式のクリアが効かない");
            // --- パネル・小窓が開く系 ---
            let close = |this: &mut Calc| {
                this.pick = None;
                this.pick_kind = "value";
                this.pick_note = None;
                this.menu_sub = None;
                this.prompt = None;
                this.fn_dlg = None;
                this.fmt_panel = None;
            };
            this.run_cmd("borders", cx);
            assert!(this.border_pal.is_some(), "罫線のパレットが開かない");
            this.border_pal = None;
            for (id, kind) in [
                ("changecase", "changecase"),
                ("fontname", "font"),
                ("fontcolor", "font-color"),
                ("fillparag", "fill-color"),

                ("text-orient", "orient-pick"),
                ("merge", "merge-pick"), // 結合は範囲が要る(下で anchor を張る)
                ("format", "numfmt-pick"),
                ("cell-styles", "cell-style"),
                ("defname", "names-pick"),
            ] {
                if id == "merge" {
                    this.anchor = Some(Pos::new(0, 0));
                    this.cursor = Pos::new(0, 1);
                    this.sync_input();
                }
                this.run_cmd(id, cx);
                assert_eq!(this.pick_kind, kind, "{id} のパネルが開かない");
                assert!(this.pick.is_some(), "{id} のパネルが無い");
                close(this);
                if id == "merge" {
                    this.anchor = None;
                    this.cursor = Pos::new(0, 0);
                    this.sync_input();
                }
            }
            this.run_cmd("fontsize", cx);
            assert!(this.pick.is_some(), "fontsize のパネルが開かない");
            close(this);
            this.run_cmd("condformat", cx);
            assert_eq!(this.menu_sub, Some("cond"), "条件付き書式の一覧が開かない");
            close(this);
            this.run_cmd("insert-function", cx);
            assert!(this.fn_dlg.is_some(), "関数の挿入の小窓が開かない");
            close(this);
            this.run_cmd("cell-format", cx);
            assert!(this.fmt_panel.is_some(), "書式の小窓が開かない");
            close(this);
            this.run_cmd("replace", cx);
            assert!(matches!(this.prompt, Some(("find", _))), "置換のパネルが開かない");
            close(this);
            // --- 行動系 ---
            this.run_cmd("copystyle", cx);
            assert!(this.brush.is_some(), "書式のコピーの刷毛が持てない");
            this.brush = None;
            this.run_cmd("copy", cx);
            this.cursor = Pos::new(4, 0);
            this.sync_input();
            this.run_cmd("paste", cx);
            assert_eq!(
                this.book.sheets[0].get(Pos::new(4, 0)).unwrap().value.display(),
                "abc", "コピー→貼り付けが効かない"
            );
            this.run_cmd("selectall", cx);
            assert!(this.anchor.is_some(), "すべて選択が効かない");
            this.anchor = None;
            // フィル(先頭行を下へ)
            this.book.sheets[0].set(Pos::new(9, 0), sheet::Cell::input("7"));
            this.anchor = Some(Pos::new(9, 0));
            this.cursor = Pos::new(11, 0);
            this.sync_input();
            this.run_cmd("fill-num", cx);
            assert_eq!(
                this.book.sheets[0].get(Pos::new(11, 0)).unwrap().value.display(),
                "7", "フィルが効かない"
            );
            this.anchor = None;
            // 絞り込みの張り外し
            this.book.sheets[0].set(Pos::new(20, 0), sheet::Cell::input("見出し"));
            this.book.sheets[0].set(Pos::new(21, 0), sheet::Cell::input("1"));
            this.run_cmd("setfilter", cx);
            assert!(this.auto_filter.is_some(), "絞り込みが張れない");
            this.run_cmd("clear-filter", cx);
            assert!(this.auto_filter.is_none(), "絞り込みが解けない");
            // 行の出し入れ(cell-ins/cell-del は行に固定 — 台帳の既知の控え)
            let rows0 = this.book.sheets[0].extent().0;
            this.cursor = Pos::new(0, 0);
            this.sync_input();
            this.run_cmd("cell-ins", cx);
            assert_eq!(this.book.sheets[0].extent().0, rows0 + 1, "行の挿入が効かない");
            this.run_cmd("cell-del", cx);
            assert_eq!(this.book.sheets[0].extent().0, rows0, "行の削除が効かない");
            // 表にする。**道が2つある**(2026-08-12、台帳「テンプレート選択
            // ギャラリー」) — `instable` は既定の色ですぐ、`table-tpl` は
            // 色の一覧を出してから
            this.anchor = Some(Pos::new(30, 0));
            this.cursor = Pos::new(32, 1);
            this.sync_input();
            this.run_cmd("instable", cx);
            assert!(!this.book.sheets[0].tables.is_empty(), "表にならない");
            let n = this.book.sheets[0].tables.len();

            // **一覧が出るだけでは表にならない。** 選んで初めて掛かる
            this.anchor = Some(Pos::new(34, 0));
            this.cursor = Pos::new(36, 1);
            this.sync_input();
            this.run_cmd("table-tpl", cx);
            assert!(this.pick.is_some(), "表のスタイルの一覧が出ない");
            assert_eq!(this.book.sheets[0].tables.len(), n, "選ぶ前に表になっている");
            this.apply_pick("青", cx);
            assert_eq!(this.book.sheets[0].tables.len(), n + 1, "選んでも表にならない");
            assert_eq!(
                this.book.sheets[0].get(Pos::new(34, 0)).map(|c| c.fmt.fill.clone()),
                Some(Some("D6E4F0".into())),
                "選んだ色が見出しに掛からない"
            );
        });
    }

    #[gpui::test]
    fn ホームの文字飾り4種が掛かる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.book.sheets[0].set(Pos::new(0, 0), sheet::Cell::input("字"));
            this.cursor = Pos::new(0, 0);
            this.sync_input();
            for id in ["bold", "italic", "underline", "strikeout"] {
                this.run_cmd(id, cx);
            }
            let f = &this.book.sheets[0].get(Pos::new(0, 0)).unwrap().fmt;
            assert!(f.bold, "太字が掛からない");
            assert!(f.italic, "斜体が掛からない");
            assert!(f.underline, "下線が掛からない");
            assert!(f.strike, "取り消し線が掛からない");
        });
    }

    #[gpui::test]
    fn テキスト取り込みのパネルは置き場所と取り込みが効く(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.import_pend = Some(crate::py::ImportPend {
                path: std::path::PathBuf::from("試.csv"),
                enc: 0,
                delim: 0,
                custom: String::new(),
                dest: Pos::new(0, 0),
                grid: vec![
                    vec!["品名".into(), "金額".into()],
                    vec!["鉛筆".into(), "100".into()],
                ],
                used: ("utf-8-sig".into(), ",".into()),
            });
            this.import_pick();
            assert_eq!(this.pick_kind, "csv-import-pick");
            // 置き場所を B3 に
            this.prompt = Some(("csv-dest", Editor::new("B3")));
            this.finish_prompt(cx);
            assert_eq!(this.import_pend.as_ref().unwrap().dest, Pos::new(2, 1));
            // 取り込む
            this.apply_pick("→ 取り込む(2 行)", cx);
            assert!(this.import_pend.is_none(), "パネルが閉じない");
            let got = this.book.sheets[0].get(Pos::new(3, 1)).unwrap().value.display();
            assert_eq!(got, "鉛筆", "置き場所に流し込まれていない");
            let got = this.book.sheets[0].get(Pos::new(3, 2)).unwrap().value.display();
            assert_eq!(got, "100");
        });
    }

    #[test]
    fn csvの台本は文字コードと区切りの指定が効く() {
        // .venv が無い機械では黙って飛ぶ(HIKITSUGI の作法)
        let Some(py) = ["../.venv/bin/python", ".venv/bin/python"]
            .iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.exists())
        else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("jo-csvwiz-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let py_path = dir.join("jo_csv.py");
        std::fs::write(&py_path, crate::py::CSV_PY).unwrap();
        // CP932 のセミコロン区切り(自動では , を想定しがちな中身)
        let csv_path = dir.join("試.csv");
        let sjis: Vec<u8> = [
            0x95i32, 0x69, 0x96, 0xbc, 0x3b, 0x8b, 0xe0, 0x8a, 0x7a, 0x0a, // 品名;金額
            0x89, 0x94, 0x95, 0x4d, 0x3b, 0x31, 0x30, 0x30, // 鉛筆;100
        ]
        .iter()
        .map(|b| *b as u8)
        .collect();
        std::fs::write(&csv_path, sjis).unwrap();
        let o = std::process::Command::new(&py)
            .arg(&py_path)
            .arg(&csv_path)
            .arg("cp932")
            .arg(";")
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let out = String::from_utf8_lossy(&o.stdout).to_string();
        let mut rows = out.split('\u{1e}');
        let meta = rows.next().unwrap();
        assert!(meta.starts_with('\u{01}'), "下ごしらえの報告が無い: {meta:?}");
        assert!(meta.contains("cp932"), "使った文字コードの報告が無い: {meta:?}");
        let first: Vec<&str> = rows.next().unwrap().split('\u{1f}').collect();
        assert_eq!(first, vec!["品名", "金額"], "CP932+セミコロンで読めない: {first:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[gpui::test]
    fn スパークラインは3種を選んで置ける(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            for (i, v) in ["3", "-1", "2"].iter().enumerate() {
                this.book.sheets[0].set(Pos::new(0, i as u32), sheet::Cell::input(v));
            }
            this.anchor = Some(Pos::new(0, 0));
            this.cursor = Pos::new(0, 2);
            this.sync_input();
            this.insert_sparkline("spark-col");
            let sp = this.book.sheets[0].shapes_new.last().unwrap().clone();
            assert_eq!(sp.kind, "spark-col");
            assert_eq!(sp.points.len(), 3);
            // 底は 0 の高さ(範囲は -1..3 → 0 は 3/4 の位置 = y 0.75)
            assert!((sp.base - 0.75).abs() < 1e-4, "底が違う: {}", sp.base);
            // 負の値の棒の先端は底より下
            assert!(sp.points[1].1 > sp.base, "負の棒が下に伸びていない");
            this.insert_sparkline("spark-wl");
            let sp = this.book.sheets[0].shapes_new.last().unwrap().clone();
            assert_eq!(sp.kind, "spark-wl");
            assert_eq!(sp.base, 0.5);
            assert_eq!(sp.points.iter().map(|p| p.1).collect::<Vec<_>>(), vec![0.1, 0.9, 0.1]);
        });
    }

    #[gpui::test]
    fn 図形の設定パネルの経路で性質が変わる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.book.sheets[0].shapes_new.push(sheet::model::SheetShape {
                at: Pos::new(0, 0),
                width_px: 100.0,
                height_px: 60.0,
                kind: "rect".into(),
                line: Some("1B6E3C".into()),
                ..Default::default()
            });
            this.shape_sel = Some(0);
            // 塗りの直指定(小文字でも通る)
            this.prompt = Some(("shape-fill-rgb", Editor::new("ff0000")));
            this.finish_prompt(cx);
            assert_eq!(
                this.book.sheets[0].shapes_new[0].fill.as_deref(),
                Some("FF0000"),
                "塗りの直指定が効かない"
            );
            // 回転の直指定(負は 360 に折り返す)
            this.prompt = Some(("shape-rot", Editor::new("-30")));
            this.finish_prompt(cx);
            assert!(
                (this.book.sheets[0].shapes_new[0].rot - 330.0).abs() < 0.01,
                "回転の直指定が効かない: {}",
                this.book.sheets[0].shapes_new[0].rot
            );
            // 太さ・不透明度・影・反転(パネルのボタンの実体は shape_edit)
            this.shape_edit(|sp| {
                sp.line_w = 3.0;
                sp.alpha = 0.5;
                sp.shadow = true;
                sp.flip_h = true;
            });
            let sp = &this.book.sheets[0].shapes_new[0];
            assert!(sp.shadow && sp.flip_h);
            assert!((sp.line_w - 3.0).abs() < 0.01 && (sp.alpha - 0.5).abs() < 0.01);
            // 空 Enter = 塗りなし
            this.prompt = Some(("shape-fill-rgb", Editor::new("")));
            this.finish_prompt(cx);
            assert!(this.book.sheets[0].shapes_new[0].fill.is_none());
            // 読めない色はパネルが残る(黙って捨てない)
            this.prompt = Some(("shape-line-rgb", Editor::new("赤")));
            this.finish_prompt(cx);
            assert!(this.prompt.is_some(), "読めない色でパネルが閉じた");
            this.prompt = None;
            // 選択が無ければ何も起きない(shape_edit の守り)
            this.shape_sel = None;
            this.shape_edit(|sp| sp.rot = 10.0);
            assert!((this.book.sheets[0].shapes_new[0].rot - 330.0).abs() < 0.01);
        });
    }

    #[gpui::test]
    fn 図形メニューで重なり順と切り貼りができる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            for k in ["rect", "ellipse", "diamond"] {
                this.book.sheets[0].shapes_new.push(sheet::model::SheetShape {
                    at: Pos::new(0, 0),
                    width_px: 50.0,
                    height_px: 50.0,
                    kind: k.into(),
                    line: Some("1B6E3C".into()),
                    ..Default::default()
                });
            }
            let kinds = |this: &Calc| -> Vec<String> {
                this.book.sheets[0].shapes_new.iter().map(|s| s.kind.clone()).collect()
            };
            // 重なり順: 後ろの方が前。rect(0) を最前面へ
            this.shape_sel = Some(0);
            this.shape_menu_action("sh-front");
            assert_eq!(kinds(this), vec!["ellipse", "diamond", "rect"]);
            assert_eq!(this.shape_sel, Some(2), "選択が付いて行かない");
            // 1つ背面へ
            this.shape_menu_action("sh-backward");
            assert_eq!(kinds(this), vec!["ellipse", "rect", "diamond"]);
            // 最背面へ
            this.shape_menu_action("sh-back");
            assert_eq!(kinds(this), vec!["rect", "ellipse", "diamond"]);
            // コピー → 貼り付け(カーソルの位置に、少しずらして)
            this.cursor = Pos::new(5, 3);
            this.shape_menu_action("sh-copy");
            this.shape_menu_action("sh-paste");
            assert_eq!(this.book.sheets[0].shapes_new.len(), 4);
            let pasted = this.book.sheets[0].shapes_new.last().unwrap();
            assert_eq!(pasted.kind, "rect");
            assert_eq!(pasted.at, Pos::new(5, 3));
            // 切り取り → 数が減り、貼り付けで戻る
            this.shape_sel = Some(0);
            this.shape_menu_action("sh-cut");
            assert_eq!(this.book.sheets[0].shapes_new.len(), 3);
            assert!(this.shape_sel.is_none());
            this.shape_menu_action("sh-paste");
            assert_eq!(this.book.sheets[0].shapes_new.len(), 4);
            // 右回転90度が2度で180
            this.shape_menu_action("sh-rot-r");
            this.shape_menu_action("sh-rot-r");
            let sp = this.book.sheets[0].shapes_new.last().unwrap();
            assert!((sp.rot - 180.0).abs() < 0.01);
            // 貼る物が無いときは黙って何も足さない
            this.shape_clip = None;
            this.shape_menu_action("sh-paste");
            assert_eq!(this.book.sheets[0].shapes_new.len(), 4);
        });
    }

    #[gpui::test]
    fn 回転ハンドルはポインタの向きへ回りshiftで15度刻み(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            this.book.sheets[0].shapes_new.push(sheet::model::SheetShape {
                at: Pos::new(2, 2),
                width_px: 100.0,
                height_px: 60.0,
                kind: "rect".into(),
                line: Some("1B6E3C".into()),
                ..Default::default()
            });
            this.shape_sel = Some(0);
            this.shape_rot = Some(0);
            // 図形の中心(格子px)
            let sp = &this.book.sheets[0].shapes_new[0];
            let (sx, sy) = this.cell_origin_px(sp.at).unwrap();
            let (ccx, ccy) = (sx + 50.0, sy + 30.0);
            // 真右へ引く = 90度
            this.shape_rotate_at(ccx + 80.0, ccy, false);
            assert!(
                (this.book.sheets[0].shapes_new[0].rot - 90.0).abs() < 0.5,
                "右で90度にならない: {}",
                this.book.sheets[0].shapes_new[0].rot
            );
            // 真下 = 180度
            this.shape_rotate_at(ccx, ccy + 80.0, false);
            assert!((this.book.sheets[0].shapes_new[0].rot - 180.0).abs() < 0.5);
            // Shift: 中途半端な向き(100度あたり)が15度刻みに丸まる
            let t = 100.0f32.to_radians();
            this.shape_rotate_at(ccx + 80.0 * t.sin(), ccy - 80.0 * t.cos(), true);
            let r = this.book.sheets[0].shapes_new[0].rot;
            assert!((r - 105.0).abs() < 0.5, "15度刻みに丸まらない: {r}");
            // 取っ手は折れ線もの(スパークライン)には無い
            this.book.sheets[0].shapes_new.push(sheet::model::SheetShape {
                at: Pos::new(0, 0),
                width_px: 80.0,
                height_px: 20.0,
                kind: "spark".into(),
                line: Some("1B6E3C".into()),
                points: vec![(0.0, 0.5), (1.0, 0.5)],
                ..Default::default()
            });
            assert!(this.shape_rot_handle(1).is_none(), "折れ線に回転の取っ手が出た");
            assert!(this.shape_rot_handle(0).is_some());
        });
    }

    #[gpui::test]
    fn 図形の整列と分布は束の外接の箱が基準(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            // 3つを別々の場所に(全部画面の中)
            for (r, cc, w, h) in [(1u32, 1u32, 40.0f32, 20.0f32), (3, 3, 60.0, 30.0), (5, 5, 20.0, 40.0)] {
                this.book.sheets[0].shapes_new.push(sheet::model::SheetShape {
                    at: Pos::new(r, cc),
                    width_px: w,
                    height_px: h,
                    kind: "rect".into(),
                    line: Some("1B6E3C".into()),
                    ..Default::default()
                });
            }
            let pos = |this: &Calc, i: usize| -> (f32, f32, f32, f32) {
                let sp = &this.book.sheets[0].shapes_new[i];
                let (x, y) = this.cell_origin_px(sp.at).unwrap();
                (x + sp.dx_px, y + sp.dy_px, sp.width_px, sp.height_px)
            };
            // 2個未満は動かず、状態行で案内する
            this.shape_sel = Some(0);
            let before = pos(this, 0);
            this.shape_align("sh-al-l");
            assert_eq!(pos(this, 0), before, "1個で整列が動いた");
            // 左揃え: いちばん左の x に揃う
            this.shape_multi = vec![1, 2];
            let min_x = (0..3).map(|i| pos(this, i).0).fold(f32::MAX, f32::min);
            this.shape_align("sh-al-l");
            for i in 0..3 {
                assert!((pos(this, i).0 - min_x).abs() < 1.5, "左に揃わない: {i}");
            }
            // 下揃え: いちばん下の底に揃う
            let max_b = (0..3).map(|i| { let p = pos(this, i); p.1 + p.3 }).fold(f32::MIN, f32::max);
            this.shape_align("sh-al-b");
            for i in 0..3 {
                let p = pos(this, i);
                assert!((p.1 + p.3 - max_b).abs() < 1.5, "下に揃わない: {i}");
            }
            // 横に分布: 隙間が等しい(端の2つは留まる)
            // まず縦に揃えてから横へ広げ直す
            this.book.sheets[0].shapes_new[1].at = Pos::new(3, 6);
            this.shape_align("sh-dist-h");
            let mut ps: Vec<(f32, f32)> = (0..3).map(|i| { let p = pos(this, i); (p.0, p.2) }).collect();
            ps.sort_by(|a, b| a.0.total_cmp(&b.0));
            let g1 = ps[1].0 - (ps[0].0 + ps[0].1);
            let g2 = ps[2].0 - (ps[1].0 + ps[1].1);
            assert!((g1 - g2).abs() < 2.0, "隙間が等しくない: {g1} vs {g2}");
            // Del は束ごと消す
            this.shape_sel = Some(0);
            this.shape_multi = vec![1];
            this.shape_menu_action("sh-del");
            assert_eq!(this.book.sheets[0].shapes_new.len(), 1);
            assert!(this.shape_multi.is_empty());
        });
    }

    #[gpui::test]
    fn ブックに載っているコードは実行しない(cx: &mut gpui::TestAppContext) {
        // 発注者確定 2026-08-09: データとプログラムを1つのファイルにしない。
        // 関数(UDF)も手続きも plugins の .py だけ — ブックからは何も実行しない
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.book.scripts.push(("取り込み試験".into(), "print(1)".into()));
            this.book.scripts.push(("関数集計".into(), "def f(x):\n    return x".into()));
            // ブックに載っているものは、手続きも関数も断る(取り出しの道を案内)
            for name in ["@取り込み試験", "@関数集計"] {
                this.prompt = Some(("py", Editor::new(name)));
                this.finish_prompt(cx);
                assert!(
                    this.status.contains("実行しません") && this.status.contains("@export"),
                    "{name} を断っていない: {}",
                    this.status
                );
            }
            // @save はもう無い(ブックにコードは載せない)
            this.prompt = Some(("py", Editor::new("@save 関数集計")));
            this.finish_prompt(cx);
            assert!(
                this.status.contains("載せません"),
                "@save の門が働いていない: {}",
                this.status
            );
            // サンドボックスを外したので「net」の区別はもう無い(黙って受けない)
            this.prompt = Some(("py", Editor::new("@居ない手続きxyz net")));
            this.finish_prompt(cx);
            assert!(
                this.status.contains("要らなく"),
                "net の始末を言っていない: {}",
                this.status
            );
            // 無い名前は plugins の置き場を案内
            this.prompt = Some(("py", Editor::new("@居ない手続きxyz")));
            this.finish_prompt(cx);
            assert!(this.status.contains("ありません"), "{}", this.status);
        });
    }

    #[gpui::test]
    fn r1c1では見せ方が変わり中身はa1のまま(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.book.sheets[0].set(Pos::new(0, 0), sheet::Cell::input("10"));
            this.book.sheets[0].set(Pos::new(4, 4), sheet::Cell::input("=A1*2"));
            recalc_book(&mut this.book, 0);
            this.run_cmd("ref-style", cx);
            assert!(this.book.r1c1);
            // 数式バーは R1C1 で見える
            this.cursor = Pos::new(4, 4);
            this.sync_input();
            assert_eq!(this.input.text(), "=R[-4]C[-4]*2", "見せ方が変わらない");
            // R1C1 で打っても中身は A1 で仕舞われる
            this.input = Editor::new("=R[-4]C[-4]+RC[-1]");
            this.commit();
            let f = this.book.sheets[0].get(Pos::new(4, 4)).unwrap().formula.clone();
            assert_eq!(f.as_deref(), Some("A1+D5"), "中身が A1 になっていない: {f:?}");
            // 動かず確定しても値が壊れない(往復の対称性)
            this.sync_input();
            assert!(this.commit(), "対称でない(見せた式が別物として書き戻る)");
            let f2 = this.book.sheets[0].get(Pos::new(4, 4)).unwrap().formula.clone();
            assert_eq!(f2.as_deref(), Some("A1+D5"));
            // 切り替えれば A1 に戻る
            this.run_cmd("ref-style", cx);
            this.sync_input();
            assert_eq!(this.input.text(), "=A1+D5");
        });
    }

    #[gpui::test]
    fn ラベルと値とグループの指図がパネルから入る(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // 元の表
            for (r, row) in [["区分", "金額"], ["東京", "100"], ["大阪", "50"], ["東村山", "30"]]
                .iter()
                .enumerate()
            {
                for (cc, v) in row.iter().enumerate() {
                    this.book.sheets[0].set(Pos::new(r as u32, cc as u32), sheet::Cell::input(v));
                }
            }
            let mut d = def(&["区分"], &[], "金額", "合計");
            // 試験では polars を飛ばさない: spawn_pivot はシート名で早期に
            // 止まる(指図の欄が入ることだけを確かめる)
            d.sheet = "試験では無いシート".into();
            d.src = (Pos::new(0, 0), Pos::new(3, 1));
            this.book.pivots.push(d);
            // ラベルで絞る: 「で始まる 東」→ 大阪だけ hide に落ちる
            this.pivot_flt = Some((0, "区分".into(), Default::default()));
            this.prompt = Some(("pivot-label", Editor::new("で始まる 東")));
            this.finish_prompt(cx);
            let hide = &this.book.pivots[0].hide;
            assert_eq!(hide.len(), 1, "hide が入らない: {hide:?}");
            assert_eq!(hide[0].0, "区分");
            assert_eq!(hide[0].1, vec!["大阪".to_string()], "落ちる値が違う: {hide:?}");
            // 値で絞る
            this.pivot_flt = Some((0, "区分".into(), Default::default()));
            this.prompt = Some(("pivot-vfilter", Editor::new("> 40")));
            this.finish_prompt(cx);
            assert_eq!(this.book.pivots[0].vfilter, Some((">".into(), 40.0)));
            // 数の幅でグループ化
            this.pivot_flt = Some((0, "金額".into(), Default::default()));
            this.prompt = Some(("pivot-group-width", Editor::new("50")));
            this.finish_prompt(cx);
            assert_eq!(this.book.pivots[0].group_by, vec![("金額".into(), "幅:50".into())]);
        });
    }

    #[test]
    fn グループ化と値のフィルターがpolarsで回る() {
        let headers: Vec<String> =
            ["日付", "区分", "金額"].iter().map(|s| s.to_string()).collect();
        let rows: Vec<Vec<String>> = [
            ["2026-01-10", "A", "100"],
            ["2026-01-25", "A", "50"],
            ["2026-02-05", "B", "30"],
            ["2026-04-01", "B", "70"],
        ]
        .iter()
        .map(|r| r.iter().map(|s| s.to_string()).collect())
        .collect();
        // 日付を月でグループ化して合計
        let mut d = def(&["日付"], &[], "金額", "合計");
        d.group_by.push(("日付".into(), "月".into()));
        let spec = pivot_spec_json(&headers, &rows, &d);
        let Some((g, _)) = run_py(spec) else { return };
        assert_eq!(g[1], vec!["2026-01", "150"], "月のグループが効かない: {g:?}");
        assert_eq!(g[2], vec!["2026-02", "30"]);
        assert_eq!(g[3], vec!["2026-04", "70"]);
        // 四半期
        let mut d = def(&["日付"], &[], "金額", "合計");
        d.group_by.push(("日付".into(), "四半期".into()));
        let spec = pivot_spec_json(&headers, &rows, &d);
        let Some((g, _)) = run_py(spec) else { return };
        assert_eq!(g[1], vec!["2026年Q1", "180"], "四半期が効かない: {g:?}");
        assert_eq!(g[2], vec!["2026年Q2", "70"]);
        // 数の幅(金額を 50 刻みで束ね、区分を数える)
        let mut d = def(&["金額"], &[], "区分", "個数");
        d.group_by.push(("金額".into(), "幅:50".into()));
        let spec = pivot_spec_json(&headers, &rows, &d);
        let Some((g, _)) = run_py(spec) else { return };
        assert_eq!(g[1], vec!["  0〜 49", "1"], "幅の帯が違う: {g:?}");
        assert_eq!(g[2], vec![" 50〜 99", "2"], "帯の並びが数字順でない: {g:?}");
        assert_eq!(g[3], vec!["100〜149", "1"]);
        // 値のフィルター(合計 >= 70 の行だけ)+ 総計はフィルター後
        let mut d = def(&["区分"], &[], "金額", "合計");
        d.vfilter = Some((">=".into(), 70.0));
        d.totals = true;
        let spec = pivot_spec_json(&headers, &rows, &d);
        let Some((g, _k)) = run_py(spec) else { return };
        // A=150, B=100 → 両方残る。しきい値を上げると片方だけに
        assert_eq!(g[1], vec!["A", "150"]);
        assert_eq!(g[2], vec!["B", "100"]);
        let mut d = def(&["区分"], &[], "金額", "合計");
        d.vfilter = Some((">".into(), 120.0));
        d.totals = true;
        let spec = pivot_spec_json(&headers, &rows, &d);
        let Some((g, k)) = run_py(spec) else { return };
        assert_eq!(g[1], vec!["A", "150"], "値のフィルターが効かない: {g:?}");
        let ti = k.iter().position(|c| *c == 't').expect("総計が無い");
        assert_eq!(g[ti], vec!["総計", "150"], "総計が絞り込み後になっていない: {g:?}");
        assert_eq!(g.len(), ti + 1, "余計な行がある: {g:?}");
    }

    /// **並べ替えが実際に効くこと**(2026-08-13、台帳「ピボットの並べ替え」)。
    /// 台本を通して回すので、書いたつもりで効いていない、が起きない
    #[test]
    fn ピボットの並べ替えが効く() {
        let headers: Vec<String> =
            ["区分", "金額"].iter().map(|s| s.to_string()).collect();
        let rows: Vec<Vec<String>> = [
            ["B", "50"],
            ["A", "150"],
            ["C", "100"],
        ]
        .iter()
        .map(|r| r.iter().map(|s| s.to_string()).collect())
        .collect();
        let 並び = |so: &str| -> Option<Vec<String>> {
            let mut d = def(&["区分"], &[], "金額", "合計");
            d.sort = so.to_string();
            let (g, _) = run_py(pivot_spec_json(&headers, &rows, &d))?;
            Some(g.iter().skip(1).map(|r| r[0].clone()).collect())
        };
        // **黙って飛ばさない。** `.venv` があるのに動かないなら、それは
        // 「試験が無い」と同じ — 2026-08-13、壊しても通ることに気づいた
        let Some(素) = 並び("") else {
            assert!(
                !std::path::Path::new("../.venv/bin/python").exists()
                    && !std::path::Path::new(".venv/bin/python").exists(),
                ".venv があるのにピボットの台本が回りません(試験が飛んでいます)"
            );
            return;
        };
        // **昇順は polars の素の並びと同じ**なので、この試験だけでは
        // 効いている証拠にならない(2026-08-13、壊しても通ることを確認した)。
        // 見出しの側は降順が、値の側は両方が、実際に並びを変える
        assert_eq!(並び("見出しの昇順").unwrap(), vec!["A", "B", "C"], "見出しの昇順が効かない");
        assert_eq!(並び("見出しの降順").unwrap(), vec!["C", "B", "A"], "見出しの降順が効かない");
        assert_eq!(並び("値の大きい順").unwrap(), vec!["A", "C", "B"], "値の大きい順が効かない");
        assert_eq!(並び("値の小さい順").unwrap(), vec!["B", "C", "A"], "値の小さい順が効かない");
        // **知らない指定は素通し。** 黙って別の順に並べない
        assert_eq!(並び("よくわからない順").unwrap(), 素, "知らない指定で並びが変わっている");
    }

    #[test]
    fn 台本が実際にpolarsで回る() {
        let headers: Vec<String> =
            ["部署", "月", "金額"].iter().map(|s| s.to_string()).collect();
        let rows: Vec<Vec<String>> = [
            ["営業", "1月", "100"],
            ["営業", "1月", "50"],
            ["総務", "1月", "30"],
            ["営業", "2月", "70"],
        ]
        .iter()
        .map(|r| r.iter().map(|s| s.to_string()).collect())
        .collect();
        // 部署×月の合計(クロス表)
        let spec = pivot_spec_json(&headers, &rows, &def(&["部署"], &["月"], "金額", "合計"));
        let Some((g, k)) = run_py(spec) else { return };
        // 1行目は Excel と同じ札(合計 / 金額 と、列に広げた見出し)
        assert_eq!(k[0], 'l');
        assert_eq!(g[0], vec!["合計 / 金額", "月", ""], "札の形が違う: {g:?}");
        assert_eq!(k[1], 'h');
        assert_eq!(g[1], vec!["部署", "1月", "2月"], "見出しの形が違う: {g:?}");
        assert_eq!(g[2], vec!["営業", "150", "70"]);
        // 無い組み合わせ: 合計は 0(空の合計)。平均などは null → 空欄になる
        assert_eq!(g[3], vec!["総務", "30", "0"]);
        // 部署ごとの個数(列に広げない)— 値の列の見出しは「個数 / 金額」
        let spec = pivot_spec_json(&headers, &rows, &def(&["部署"], &[], "金額", "個数"));
        let Some((g, _)) = run_py(spec) else { return };
        assert_eq!(g[0], vec!["部署", "個数 / 金額"]);
        assert_eq!(g[1], vec!["営業", "3"]);
        assert_eq!(g[2], vec!["総務", "1"]);
    }

    #[test]
    fn 総計と小計と空行が付く() {
        let headers: Vec<String> =
            ["部署", "係", "月", "金額"].iter().map(|s| s.to_string()).collect();
        let rows: Vec<Vec<String>> = [
            ["営業", "一", "1月", "100"],
            ["営業", "二", "1月", "50"],
            ["営業", "一", "2月", "70"],
            ["総務", "一", "1月", "30"],
        ]
        .iter()
        .map(|r| r.iter().map(|s| s.to_string()).collect())
        .collect();
        let mut d = def(&["部署", "係"], &["月"], "金額", "合計");
        d.totals = true;
        d.subtotals = true;
        d.blank_rows = true;
        let spec = pivot_spec_json(&headers, &rows, &d);
        let Some((g, k)) = run_py(spec) else { return };
        assert_eq!(g[0], vec!["合計 / 金額", "", "月", "", ""], "札: {g:?}");
        assert_eq!(g[1], vec!["部署", "係", "1月", "2月", "総計"], "見出し: {g:?}");
        assert_eq!(g[2], vec!["営業", "一", "100", "70", "170"]);
        assert_eq!(g[3], vec!["営業", "二", "50", "0", "50"]);
        assert_eq!(
            g[4],
            vec!["営業 小計", "", "150", "70", "220"],
            "小計が違う: {g:?}"
        );
        assert_eq!(k[4], 's', "小計の種別が違う");
        assert_eq!(k[5], 'b', "空行が無い");
        assert_eq!(g[7], vec!["総務 小計", "", "30", "0", "30"]);
        let last = g.last().unwrap();
        assert_eq!(last, &vec!["総計", "", "180", "70", "250"], "総計が違う: {g:?}");
        assert_eq!(*k.last().unwrap(), 't');
        // コンパクト形式: 繰り返しの見出しが空欄になる
        d.subtotals = false;
        d.blank_rows = false;
        d.totals = false;
        d.compact = true;
        let spec = pivot_spec_json(&headers, &rows, &d);
        let Some((g, _)) = run_py(spec) else { return };
        assert_eq!(g[3][0], "", "繰り返しの部署が空欄にならない: {g:?}");
        assert_eq!(g[3][1], "二");
    }
}

/// 計算方法(自動/手動)とセル内改行の試験
#[cfg(test)]
mod recalc_tests {
    use crate::*;

    #[gpui::test]
    fn 手動計算は確定で計算せずf9相当で計算する(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            // A1=5 → B1==A1*2。自動のうちは確定で計算される
            this.cursor = Pos::parse("A1").unwrap();
            this.sync_input();
            this.input.insert("5");
            assert!(this.commit());
            this.cursor = Pos::parse("B1").unwrap();
            this.sync_input();
            this.input.insert("=A1*2");
            assert!(this.commit());
            assert_eq!(
                this.sheet().value(Pos::parse("B1").unwrap()),
                sheet::Value::Number(10.0),
                "自動のうちは確定で計算されるはず"
            );
            // 手動にして A1 を書き換えると、B1 は古いまま
            this.auto_calc = false;
            this.cursor = Pos::parse("A1").unwrap();
            this.sync_input();
            this.input.select_all();
            this.input.insert("7");
            assert!(this.commit());
            assert_eq!(
                this.sheet().value(Pos::parse("B1").unwrap()),
                sheet::Value::Number(10.0),
                "手動なのに確定で計算された(手動が効いていない)"
            );
            // F9 の実体(recalc_book)で計算される
            recalc_book(&mut this.book, this.active);
            assert_eq!(
                this.sheet().value(Pos::parse("B1").unwrap()),
                sheet::Value::Number(14.0),
                "F9 相当の再計算が効かない"
            );
        });
    }

    #[gpui::test]
    fn 固定はプリセットの一覧から選ぶ(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.cursor = Pos::parse("B2").unwrap();
            this.run_cmd("freeze", cx);
            assert_eq!(this.pick_kind, "freeze", "固定の一覧が開かない");
            this.apply_pick("最上行の固定", cx);
            assert_eq!(this.frozen, Some(Pos::new(1, 0)), "最上行が固定されない");
            this.run_cmd("freeze", cx);
            this.apply_pick("最初の列の固定", cx);
            assert_eq!(this.frozen, Some(Pos::new(0, 1)), "最初の列が固定されない");
            this.run_cmd("freeze", cx);
            this.apply_pick("いまの位置で固定(上と左が留まる)", cx);
            assert_eq!(this.frozen, Some(Pos::parse("B2").unwrap()), "いまの位置で固定されない");
            this.run_cmd("freeze", cx);
            this.apply_pick("固定の解除", cx);
            assert_eq!(this.frozen, None, "固定が解けない");
            // 影の入切(本家の「固定された枠に影を付ける」)。✓ 付きでも効く
            this.run_cmd("freeze", cx);
            this.apply_pick("固定した枠に影を付ける", cx);
            assert!(this.freeze_shadow, "影が入らない");
            this.run_cmd("freeze", cx);
            this.apply_pick("✓ 固定した枠に影を付ける", cx);
            assert!(!this.freeze_shadow, "影が切れない");
        });
    }

    #[gpui::test]
    fn 画像は選んで動かして大きさを変えて消せる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            // 1x1 の PNG を B2 に置く(挿した画像の体)
            let png: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47];
            this.sheet_mut().images_new.push(sheet::model::SheetImage {
                at: Pos::parse("B2").unwrap(),
                dx_px: 0.0,
                dy_px: 0.0,
                width_px: 120.0,
                height_px: 60.0,
                data: png,
            });
            // 当たり判定(B2 の原点 + 中ほど)
            let (ox, oy) = this.cell_origin_px(Pos::parse("B2").unwrap()).unwrap();
            let hit = this.image_at(ox + 10.0, oy + 10.0);
            assert!(hit.is_some(), "画像に当たらない");
            let (i, _, corner) = hit.unwrap();
            assert!(!corner);
            // 右下は大きさの掴み
            let (_, _, corner) = this.image_at(ox + 115.0, oy + 55.0).unwrap();
            assert!(corner, "右下の掴みにならない");
            // 移動(ドラッグの実体を直接)
            this.img_drag = Some((i, (ox + 10.0, oy + 10.0), (ox, oy), false));
            this.image_drag_at(ox + 40.0, oy + 15.0, false);
            let im = &this.sheet().images_new[0];
            assert!(im.dx_px > 0.0 || im.at != Pos::parse("B2").unwrap(), "動かない");
            // 大きさ(比を保つ)
            this.img_drag = Some((i, (0.0, 0.0), (ox, oy), true));
            this.image_drag_at(ox + 240.0, oy + 999.0, false);
            let im = &this.sheet().images_new[0];
            assert!((im.height_px / im.width_px - 0.5).abs() < 0.01, "比が崩れた: {}x{}", im.width_px, im.height_px);
            // 削除
            this.img_drag = None;
            this.img_sel = Some(0);
            assert!(this.delete_selected_image());
            assert!(this.sheet().images_new.is_empty());
        });
    }

    #[gpui::test]
    fn 条件付き書式のパネルの規則が掛かる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for (a1, v) in [("A1", "10"), ("A2", "20"), ("A3", "20"), ("A4", "5")] {
                this.cursor = Pos::parse(a1).unwrap();
                this.sync_input();
                this.input.insert(v);
                assert!(this.commit());
            }
            this.anchor = Some(Pos::parse("A1").unwrap());
            this.cursor = Pos::parse("A4").unwrap();
            this.sync_input();
            // 間(パネル)
            this.prompt = Some(("cond-between", Editor::new("8〜15")));
            this.finish_prompt(cx);
            // 上位N(パネル)
            this.prompt = Some(("cond-top", Editor::new("2")));
            this.finish_prompt(cx);
            let rules = &this.sheet().cond;
            assert_eq!(rules.len(), 2, "{}", this.status);
            assert_eq!(
                rules[0].kind,
                sheet::model::CondKind::Between(8.0, 15.0, false)
            );
            assert_eq!(rules[1].kind, sheet::model::CondKind::Top(2, false));
            // 効き方(下ごしらえ込み)
            let aux = rules[1].aux(this.sheet());
            assert!(rules[1].hits(Pos::parse("A2").unwrap(), &sheet::Value::Number(20.0), &aux));
            assert!(!rules[1].hits(Pos::parse("A4").unwrap(), &sheet::Value::Number(5.0), &aux));
            // 読めない入力は言い返す
            this.prompt = Some(("cond-between", Editor::new("abc")));
            this.finish_prompt(cx);
            assert!(this.prompt.is_some(), "読めない間の形が通った");
        });
    }

    #[gpui::test]
    fn グループ化は7段で頭打ち_基底と合わせて8レベル(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for r in 0..4 {
                this.book.sheets[0].set(Pos::new(r, 0), sheet::Cell::input("x"));
            }
            // 同じ2〜3行目を9回まとめる → outlineLevel は 7 で止まる
            // (ECMA-376 の上限。本家の「最大8レベル」= 基底+7段と同じ意味)
            this.anchor = Some(Pos::new(1, 0));
            this.cursor = Pos::new(2, 0);
            this.sync_input();
            for _ in 0..9 {
                this.run_cmd("group", cx);
            }
            assert_eq!(this.sheet().row_outline.get(&1), Some(&7), "7段で止まらない");
            // 1段ほどく → 6
            this.run_cmd("ungroup", cx);
            assert_eq!(this.sheet().row_outline.get(&1), Some(&6), "ほどけない");
        });
    }

    #[gpui::test]
    fn 値だけをcsvに書き出せる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            this.book.sheets[0].set(Pos::new(0, 0), sheet::Cell::input("品名"));
            this.book.sheets[0].set(Pos::new(0, 1), sheet::Cell::input("値,段"));
            this.book.sheets[0].set(Pos::new(1, 0), sheet::Cell::input("鉛筆"));
            this.book.sheets[0].set(Pos::new(1, 1), sheet::Cell::input("=1+2"));
            recalc_book(&mut this.book, 0);
            let dir = std::env::temp_dir().join("calc-csv-test");
            std::fs::create_dir_all(&dir).unwrap();
            let p = dir.join("out.csv");
            this.write_csv(&p);
            let got = std::fs::read_to_string(&p).unwrap();
            assert!(got.starts_with('\u{feff}'), "BOM が無い");
            assert!(got.contains("品名,\"値,段\""), "区切りを含む欄が囲われていない: {got}");
            assert!(got.contains("鉛筆,3"), "式が計算値になっていない: {got}");
            std::fs::remove_dir_all(&dir).ok();
        });
    }

    #[gpui::test]
    fn ルールの管理で規則を選んで消せる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for (i, v) in ["10", "20"].iter().enumerate() {
                this.book.sheets[0].set(Pos::new(i as u32, 0), sheet::Cell::input(v));
            }
            this.anchor = Some(Pos::new(0, 0));
            this.cursor = Pos::new(1, 0);
            this.sync_input();
            this.cond_visual("cond-bar");
            this.cond_visual("cond-scale");
            assert_eq!(this.book.sheets[0].cond.len(), 2);
            // 1本目を選んで消す
            this.pick_kind = "cond-manage-pick";
            this.apply_pick("1) A1:A2 — データバー", cx);
            assert_eq!(this.pick_kind, "cond-act-pick", "2択が開かない");
            this.apply_pick("この規則を消す", cx);
            assert_eq!(this.book.sheets[0].cond.len(), 1, "消えていない");
            assert!(matches!(
                this.book.sheets[0].cond[0].kind,
                sheet::model::CondKind::Scale(..)
            ), "残る方が違う");
        });
    }

    #[gpui::test]
    fn データバーとスケールとアイコンをメニューから掛けられる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            for (i, v) in ["10", "20", "30"].iter().enumerate() {
                this.book.sheets[0].set(Pos::new(i as u32, 0), sheet::Cell::input(v));
            }
            this.anchor = Some(Pos::new(0, 0));
            this.cursor = Pos::new(2, 0);
            this.sync_input();
            this.cond_visual("cond-bar");
            this.cond_visual("cond-scale");
            this.cond_visual("cond-icons");
            let cond = &this.book.sheets[0].cond;
            assert_eq!(cond.len(), 3, "3本入らない: {cond:?}");
            use sheet::model::CondKind;
            assert!(matches!(cond[0].kind, CondKind::Bar(_)));
            assert!(matches!(cond[1].kind, CondKind::Scale(..)));
            assert!(matches!(cond[2].kind, CondKind::Icons(_)));
        });
    }

    #[gpui::test]
    fn 合計行の集計のしかたを替えられる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            this.book.sheets[0].set(Pos::new(0, 0), sheet::Cell::input("10"));
            this.book.sheets[0].set(Pos::new(1, 0), sheet::Cell::input("30"));
            this.book.sheets[0].set(Pos::new(2, 0), sheet::Cell::input("=SUM(A1:A2)"));
            recalc_book(&mut this.book, 0);
            this.cursor = Pos::new(2, 0);
            this.sync_input();
            // 平均に替える
            this.set_subtotal_kind("1");
            let cell = this.book.sheets[0].get(Pos::new(2, 0)).unwrap();
            assert_eq!(cell.formula.as_deref(), Some("SUBTOTAL(1,A1:A2)"), "式が替わらない");
            assert_eq!(cell.value.display(), "20", "平均が出ない");
            // なし → 式が消えて書式は残る
            this.set_subtotal_kind("none");
            let gone = this.book.sheets[0]
                .get(Pos::new(2, 0))
                .is_none_or(|c| c.formula.is_none() && c.value.is_empty());
            assert!(gone, "式が消えない");
        });
    }

    #[gpui::test]
    fn 重複の削除は列と見出しの有無を選べる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // 品名は同じでも金額が違う2行 — 品名の列だけで比べれば重複
            for (r, (name, amt)) in
                [("品名", "金額"), ("鉛筆", "100"), ("鉛筆", "120"), ("消しゴム", "80")]
                    .iter()
                    .enumerate()
            {
                this.book.sheets[0].set(Pos::new(r as u32, 0), sheet::Cell::input(name));
                this.book.sheets[0].set(Pos::new(r as u32, 1), sheet::Cell::input(amt));
            }
            this.run_cmd("rem-duplicates", cx);
            assert_eq!(this.pick_kind, "dedup-pick", "選ぶパネルが開かない");
            assert!(this.dedup_pend.is_some());
            // 「金額」を外す → 品名だけで比べる
            this.apply_pick("金額", cx);
            assert_eq!(this.pick_kind, "dedup-pick", "入切でパネルが閉じた");
            this.apply_pick("→ 削除する", cx);
            let s = &this.book.sheets[0];
            assert_eq!(s.get(Pos::new(1, 0)).unwrap().value.display(), "鉛筆");
            assert_eq!(s.get(Pos::new(2, 0)).unwrap().value.display(), "消しゴム");
            assert!(s.get(Pos::new(3, 0)).is_none(), "重複が消えていない");
            // 残るのは先に出てきた 100 の行
            assert_eq!(s.get(Pos::new(1, 1)).unwrap().value.display(), "100");
        });
    }

    #[gpui::test]
    fn リンクの後に表示テキストを聞かれてセルに入る(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.cursor = Pos::new(1, 1);
            this.sync_input();
            this.prompt = Some(("link", Editor::new("https://example.co.jp/")));
            this.finish_prompt(cx);
            assert!(this.sheet().links.contains_key(&Pos::new(1, 1)), "リンクが付かない");
            // 続けて表示テキストのパネルが開く
            assert_eq!(this.prompt.as_ref().map(|(k, _)| *k), Some("link-text"), "表示テキストのパネルが開かない");
            this.prompt = Some(("link-text", Editor::new("会社サイト")));
            this.finish_prompt(cx);
            let got = this.sheet().get(Pos::new(1, 1)).map(|c| c.value.display());
            assert_eq!(got.as_deref(), Some("会社サイト"), "表示テキストがセルに入らない");
            // 空 Enter ならそのまま
            this.prompt = Some(("link", Editor::new("#B9")));
            this.finish_prompt(cx);
            this.prompt = Some(("link-text", Editor::new("")));
            this.finish_prompt(cx);
            let got = this.sheet().get(Pos::new(1, 1)).map(|c| c.value.display());
            assert_eq!(got.as_deref(), Some("会社サイト"), "空 Enter でセルが変わった");
        });
    }

    #[gpui::test]
    fn 列の幅と行の高さを数で指定して既定にも戻せる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // B〜C 列に 12.5
            this.anchor = Some(Pos::new(0, 1));
            this.cursor = Pos::new(0, 2);
            this.sync_input();
            this.prompt = Some(("col-width", Editor::new("12.5")));
            this.finish_prompt(cx);
            assert_eq!(this.sheet().col_width.get(&1), Some(&12.5));
            assert_eq!(this.sheet().col_width.get(&2), Some(&12.5));
            // 範囲外は言い返す
            this.prompt = Some(("col-width", Editor::new("999")));
            this.finish_prompt(cx);
            assert!(this.prompt.is_some(), "範囲外の幅が通った");
            this.prompt = None;
            // 行の高さ
            this.anchor = None;
            this.cursor = Pos::new(3, 0);
            this.sync_input();
            this.prompt = Some(("row-height", Editor::new("30")));
            this.finish_prompt(cx);
            assert_eq!(this.sheet().row_height.get(&3), Some(&30.0));
            // 空 Enter = 既定に戻す
            this.anchor = Some(Pos::new(0, 1));
            this.cursor = Pos::new(0, 2);
            this.sync_input();
            this.prompt = Some(("col-width", Editor::new("")));
            this.finish_prompt(cx);
            assert!(!this.sheet().col_width.contains_key(&1), "既定に戻らない");
        });
    }

    #[gpui::test]
    fn 名前マネージャーで移動と打ち直しと削除(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // 名前を1つ付ける(B2:C3 = 単価表)
            this.anchor = Some(Pos::parse("B2").unwrap());
            this.cursor = Pos::parse("C3").unwrap();
            this.sync_input();
            this.prompt = Some(("name", Editor::new("単価表")));
            this.finish_prompt(cx);
            assert_eq!(this.sheet().names, vec![("単価表".into(), "B2:C3".into())]);
            // 一覧に出る
            this.anchor = None;
            this.cursor = Pos::parse("A1").unwrap();
            this.sync_input();
            this.run_cmd("defname", cx);
            assert_eq!(this.pick_kind, "names-pick");
            {
                let (items, _) = this.pick.as_ref().unwrap();
                // 鍵は `name:名前`、見出しは範囲つき — 両方見て割れ方を書き残す
                assert!(
                    items.iter().any(|(k, l)| k == "name:単価表" && l == "単価表 = B2:C3"),
                    "{items:?}"
                );
            }
            // 移動
            this.apply_pick("name:単価表", cx);
            assert_eq!(this.pick_kind, "name-act-pick");
            this.apply_pick("そこへ移動", cx);
            assert_eq!(this.cursor, Pos::parse("C3").unwrap());
            assert_eq!(this.anchor, Some(Pos::parse("B2").unwrap()));
            // 打ち直し
            this.run_cmd("defname", cx);
            this.apply_pick("name:単価表", cx);
            this.apply_pick("中身を打ち直す…", cx);
            this.prompt = Some(("name-range", Editor::new("B2:D9")));
            this.finish_prompt(cx);
            assert_eq!(this.sheet().names[0].1, "B2:D9");
            // 削除
            this.run_cmd("defname", cx);
            this.apply_pick("name:単価表", cx);
            this.apply_pick("名前を消す", cx);
            assert!(this.sheet().names.is_empty(), "名前が消えない");
        });
    }

    #[gpui::test]
    fn ヘッダーとフッターをパネルから入れて消す(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.run_cmd("editheader", cx);
            assert_eq!(this.pick_kind, "hf-pick", "一覧が開かない");
            this.apply_pick("ヘッダー中", cx);
            assert!(this.prompt.is_some(), "パネルが開かない");
            this.prompt = Some(("hf-edit", Editor::new("月次売上")));
            this.finish_prompt(cx);
            assert_eq!(this.sheet().header.as_deref(), Some("&C月次売上"));
            // フッター右に頁(既存の値が一覧に見える)
            this.run_cmd("editheader", cx);
            {
                let (items, _) = this.pick.as_ref().unwrap();
                // 鍵は欄の名前だけ。打った値は見出しにだけ付く
                assert!(
                    items.iter().any(|(k, l)| k == "ヘッダー中" && l == "ヘッダー中: 月次売上"),
                    "{items:?}"
                );
            }
            this.apply_pick("フッター右", cx); // 鍵は欄の名前(値は見出しの側)
            this.prompt = Some(("hf-edit", Editor::new("&P / &N")));
            this.finish_prompt(cx);
            assert_eq!(this.sheet().footer.as_deref(), Some("&R&P / &N"));
            // 全部消す
            this.run_cmd("editheader", cx);
            this.apply_pick("全部消す", cx);
            assert!(this.sheet().header.is_none() && this.sheet().footer.is_none());
        });
    }

    #[gpui::test]
    fn 色のその他と文字の角度の直指定(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            let a1 = Pos::parse("A1").unwrap();
            this.cursor = a1;
            this.sync_input();
            this.input.insert("x");
            assert!(this.commit());
            // 文字の色: その他 → RRGGBB
            this.run_cmd("fontcolor", cx);
            this.apply_pick("その他(RRGGBB を打つ)…", cx);
            this.prompt = Some(("font-color-rgb", Editor::new("00B050")));
            this.finish_prompt(cx);
            assert_eq!(
                this.sheet().get(a1).unwrap().fmt.color.as_deref(),
                Some("00B050"),
                "文字の色の直指定が効かない"
            );
            // 塗り: その他 → RRGGBB
            this.run_cmd("fillparag", cx);
            this.apply_pick("その他(RRGGBB を打つ)…", cx);
            this.prompt = Some(("fill-color-rgb", Editor::new("FFF2CC")));
            this.finish_prompt(cx);
            assert_eq!(
                this.sheet().get(a1).unwrap().fmt.fill.as_deref(),
                Some("FFF2CC")
            );
            // 角度: 一覧のプリセット
            this.run_cmd("text-orient", cx);
            assert_eq!(this.pick_kind, "orient-pick");
            this.apply_pick("左上がり 45度", cx);
            assert_eq!(this.sheet().get(a1).unwrap().fmt.rotation, Some(45));
            // 任意の角度(負は xlsx の encode で 90+|d|)
            this.run_cmd("text-orient", cx);
            this.apply_pick("その他(角度を打つ)…", cx);
            this.prompt = Some(("text-angle", Editor::new("-30")));
            this.finish_prompt(cx);
            assert_eq!(
                this.sheet().get(a1).unwrap().fmt.rotation,
                Some(120),
                "負の角度の encode が違う"
            );
            // 範囲外は言い返す
            this.prompt = Some(("text-angle", Editor::new("200")));
            this.finish_prompt(cx);
            assert!(this.prompt.is_some(), "範囲外の角度が通った");
            assert!(this.status.contains("角度が読めません"));
        });
    }

    #[gpui::test]
    fn 罫線はペンの線種と色で掛かる(cx: &mut gpui::TestAppContext) {
        use sheet::model::BStyle;
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // 一覧が開き、ペンを 中太の実線・赤 にする
            this.anchor = Some(Pos::parse("B2").unwrap());
            this.cursor = Pos::parse("C3").unwrap();
            this.sync_input();
            this.run_cmd("borders", cx);
            assert!(this.border_pal.is_some(), "罫線のパレットが開かない");
            this.open_border_style_pick();
            assert_eq!(this.pick_kind, "border-style-pick");
            this.apply_pick("中太の実線", cx);
            assert_eq!(this.pen_style, BStyle::Medium);
            this.open_border_color_pick();
            this.apply_pick("その他(RRGGBB を打つ)…", cx);
            this.prompt = Some(("border-color-rgb", Editor::new("FF0000")));
            this.finish_prompt(cx);
            assert_eq!(this.pen_color, Some(0xFF0000), "RGB 直指定が効かない");
            // 外枠を掛ける(パレットの1押しと同じ実体)
            this.apply_borders("外枠");
            let bd = |this: &Calc, p: &str| {
                this.sheet().get(Pos::parse(p).unwrap()).unwrap().fmt.borders
            };
            let b2 = bd(this, "B2");
            assert!(b2.top.on && b2.left.on, "外枠の左上が付かない");
            assert!(!b2.bottom.on && !b2.right.on, "外枠なのに内側に付いた");
            assert_eq!(b2.top.style, BStyle::Medium);
            assert_eq!(b2.top.color, Some(0xFF0000));
            let c3 = bd(this, "C3");
            assert!(c3.bottom.on && c3.right.on && !c3.top.on);
            // 格子 → 全辺。消す → 全部消える
            this.apply_borders("すべての罫線(格子)");
            assert!(bd(this, "B2").right.on && bd(this, "C3").top.on);
            this.apply_borders("罫線を消す");
            // 素に戻ったセルは片づけられる(get は None)— どちらでも「無い」
            let off = |this: &Calc, p: &str| {
                this.sheet()
                    .get(Pos::parse(p).unwrap())
                    .map(|c| c.fmt.borders.any())
                    .unwrap_or(false)
            };
            assert!(!off(this, "B2") && !off(this, "C3"), "罫線が消えない");
        });
    }

    #[gpui::test]
    fn セルの上のbackspaceとdeleteは中身を消す(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            let a1 = Pos::parse("A1").unwrap();
            this.cursor = a1;
            this.sync_input();
            this.input.insert("こんにちは");
            assert!(this.commit());
            this.fmt(|f| f.bold = true);
            this.sync_input();
            // セルの上(編集していない)= まるごと消す。書式は残る
            assert!(!this.editing() && !this.edit_armed);
            this.clear_selection_now();
            let cell = this.sheet().get(a1).cloned().unwrap_or_default();
            assert_eq!(cell.editable(), "", "中身が消えない");
            assert!(cell.fmt.bold, "書式まで消えた");
            assert_eq!(this.input.text(), "", "数式バーが残っている");
            // 編集中の1文字削除は従来どおり(こちらは Editor の仕事)
            this.input.insert("abc");
            this.edit_armed = true;
            this.input.backspace();
            assert_eq!(this.input.text(), "ab");
        });
    }

    #[gpui::test]
    fn 結合は聞かずに掛かり左上以外の値は消える(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for (p, v) in [("A1", "甲"), ("B2", "乙")] {
                this.cursor = Pos::parse(p).unwrap();
                this.sync_input();
                this.input.insert(v);
                assert!(this.commit());
            }
            // 聞かずに結合し、左上以外の値は**消す**(発注者 2026-08-08 —
            // 残すと見えない値が SUM などの式に効く)。Ctrl+Z で戻せる
            this.anchor = Some(Pos::parse("A1").unwrap());
            this.cursor = Pos::parse("B2").unwrap();
            this.run_cmd("merge", cx);
            assert_eq!(this.pick_kind, "merge-pick", "4択が出ない");
            this.apply_pick("結合して中央に配置", cx);
            assert_eq!(this.sheet().merges.len(), 1, "確認を挟まず結合されるべき");
            assert!(this.status.contains("左上以外の値は消しました"), "案内が無い: {}", this.status);
            assert!(
                this.sheet()
                    .get(Pos::parse("B2").unwrap())
                    .is_none_or(|c| c.value.is_empty()),
                "呑まれた値が残っている(式に効いてしまう)"
            );
            // SUM が隠れた値を数えない
            this.cursor = Pos::parse("D5").unwrap();
            this.sync_input();
            this.input = Editor::new("=SUM(A1:B2)");
            assert!(this.commit());
            assert_eq!(
                this.sheet().get(Pos::parse("D5").unwrap()).unwrap().value.display(),
                "0", "隠れた値が SUM に効いている"
            );
            // Ctrl+Z で結合前に戻る(乙も戻る)
            this.undo_sheet(); // SUM の確定ぶん
            this.undo_sheet(); // 結合ぶん
            assert_eq!(
                this.sheet().get(Pos::parse("B2").unwrap()).unwrap().editable(),
                "乙", "Ctrl+Z で値が戻らない"
            );
            // やり直して続きの検査へ
            this.anchor = Some(Pos::parse("A1").unwrap());
            this.cursor = Pos::parse("B2").unwrap();
            this.run_cmd("merge", cx);
            this.apply_pick("結合して中央に配置", cx);
            // 空の範囲も同じくそのまま
            this.anchor = Some(Pos::parse("D1").unwrap());
            this.cursor = Pos::parse("E2").unwrap();
            this.run_cmd("merge", cx);
            this.apply_pick("結合して中央に配置", cx);
            assert_eq!(this.sheet().merges.len(), 2);
            // 横方向: 行ごとに1本ずつ
            this.anchor = Some(Pos::parse("G1").unwrap());
            this.cursor = Pos::parse("H3").unwrap();
            this.sync_input();
            this.run_cmd("merge", cx);
            this.apply_pick("横方向に結合(行ごと)", cx);
            assert_eq!(this.sheet().merges.len(), 5, "横方向が行ごとにならない");
            // 解除: 選択に重なる結合をまとめて外す
            this.run_cmd("merge", cx);
            this.apply_pick("結合の解除", cx);
            assert_eq!(this.sheet().merges.len(), 2, "解除で消えない");
        });
    }

    #[gpui::test]
    fn ピボットの上では表を壊す操作を締める(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            let name = this.sheet().name.clone();
            this.book.pivots.push(sheet::model::PivotDef {
                sheet: name,
                src: (Pos::parse("A1").unwrap(), Pos::parse("B4").unwrap()),
                rows_sel: vec!["品名".into()],
                cols_sel: vec![],
                value: "金額".into(),
                agg: "合計".into(),
                totals: true,
                subtotals: false,
                blank_rows: false,
                compact: true,
                dest: Pos::parse("D1").unwrap(),
                size: (3, 2), // D1:E3 に置いてある体
                hide: Vec::new(),
                style: String::new(),
                name: "ピボットテーブル1".into(),
                vfilter: None,
                group_by: Vec::new(),
                show_as: String::new(),
                sort: String::new(),
            });
            // ピボットに乗ると状態行が「タブで操作」と案内する
            this.cursor = Pos::parse("D2").unwrap();
            this.anchor = None;
            this.sync_input();
            assert!(this.status.contains("ピボットテーブル"), "{}", this.status);
            // レイアウトは行の見出しが1つだと効かない — 正直に言う
            this.run_cmd("pivot-layout", cx);
            assert!(this.status.contains("2つ以上"), "{}", this.status);
            // ピボットの上(D2)では結合も入力規則も断られる
            this.anchor = Some(Pos::parse("E3").unwrap());
            this.run_cmd("merge", cx);
            assert!(this.sheet().merges.is_empty(), "ピボットの上で結合できてしまう");
            assert!(this.status.contains("ピボット"), "{}", this.status);
            this.run_cmd("data-validation", cx);
            assert!(this.dv_dlg.is_none(), "ピボットの上で入力規則のパネルが開いた");
            // 外(A1)なら普通に通る
            this.anchor = None;
            this.cursor = Pos::parse("A1").unwrap();
            this.run_cmd("data-validation", cx);
            assert!(this.dv_dlg.is_some(), "ピボットの外まで締めている");
            this.dv_dlg = None;
        });
    }

    #[gpui::test]
    fn 画面の文字の大きさは段階で動き両端で止まる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // 実利用者の settings.toml の値に左右されない(試験は素の 100% から)
            this.ui_scale = 1.0;
            let base = this.ui_scale;
            this.run_cmd("ui-bigger", cx);
            assert!(this.ui_scale > base, "大きくならない");
            for _ in 0..30 {
                this.run_cmd("ui-bigger", cx);
            }
            assert_eq!(this.ui_scale, 1.5, "上の端(150%)で止まらない");
            for _ in 0..30 {
                this.run_cmd("ui-smaller", cx);
            }
            assert_eq!(this.ui_scale, 0.8, "下の端(80%)で止まらない");
        });
    }

    #[test]
    fn 大文字小文字の5つの変え方() {
        let t = "hello WORLD こんにちは 3rd";
        assert_eq!(change_case(t, "すべて大文字"), "HELLO WORLD こんにちは 3RD");
        assert_eq!(change_case(t, "すべて小文字"), "hello world こんにちは 3rd");
        assert_eq!(change_case(t, "文の先頭だけ大文字"), "Hello world こんにちは 3rd");
        assert_eq!(change_case(t, "単語の先頭を大文字"), "Hello World こんにちは 3rd");
        assert_eq!(
            change_case(t, "大文字と小文字を入れ替え"),
            "HELLO world こんにちは 3RD"
        );
    }

    #[gpui::test]
    fn 結合すると中央に揃う(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            this.cursor = Pos::parse("A1").unwrap();
            this.sync_input();
            this.input.insert("見出し");
            assert!(this.commit());
            this.anchor = Some(Pos::parse("A1").unwrap());
            this.cursor = Pos::parse("C1").unwrap();
            this.merge_selection("中央");
            let f = &this.sheet().get(Pos::parse("A1").unwrap()).unwrap().fmt;
            assert_eq!(f.align, sheet::model::HAlign::Center, "横が中央にならない");
            assert_eq!(f.valign, sheet::model::VAlign::Middle, "縦が中央にならない");
            assert_eq!(this.sheet().merges.len(), 1, "結合が積まれていない");
        });
    }

    #[gpui::test]
    fn セル内改行の確定で折り返しが立つ(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            this.cursor = Pos::parse("A1").unwrap();
            this.sync_input();
            this.input.insert("上の行\n下の行");
            assert!(this.commit());
            let cell = this.sheet().get(Pos::parse("A1").unwrap()).unwrap();
            assert!(cell.fmt.wrap, "改行入りの確定で折り返しが立たない");
            assert_eq!(
                cell.value,
                sheet::Value::Text("上の行\n下の行".into()),
                "改行が中身に残らない"
            );
        });
    }
}

/// **メニューのボタンを全部おして、落ちないか・繋がっているかを見る。**
/// writer の menu_run_tests と同じ作法 — リボンに ready で並ぶものは
/// ここで実際に run_cmd を通す(ダイアログを開くものだけは外す)。
/// GUI は起こさない — gpui の試験用の場で Calc を作って叩く
#[cfg(test)]
mod menu_run_tests {
    use crate::*;

    /// ファイル選択の窓を開くボタン。**試験では押さない** —
    /// rfd は実際に窓を出しに行くので、画面の無い試験では返ってこない
    /// (writer で踏んで確かめた轍。実機での確認に回す)
    const DIALOG: &[&str] = &[
        "open", "save", "pdf", "plug-macros", "insimage", "data-from-text",
        "data-external-links",
    ];

    /// 空の表だと何も起きないボタンがあるので、見本の小さな表を入れて選ぶ
    fn seed(this: &mut Calc) {
        if this.sheet().cells.is_empty() {
            for (a1, v) in [
                ("A1", "品名"), ("B1", "数量"), ("C1", "単価"),
                ("A2", "防火戸"), ("B2", "4"), ("C2", "125000"),
                ("A3", "点検口"), ("B3", "2"), ("C3", "8000"),
                ("D2", "=B2*C2"), ("D3", "=B3*C3"),
            ] {
                this.sheet_mut().set(Pos::parse(a1).unwrap(), Cell::input(v));
            }
            recalc(this.sheet_mut());
        }
        this.cursor = Pos::parse("A1").unwrap();
        this.anchor = Some(Pos::parse("D3").unwrap());
        // バーとセルを揃える(実機ではカーソル移動が必ず呼ぶ。ずれたままだと
        // 最初の commit() が A1 を空で潰し、種の表が崩れる)
        this.sync_input();
    }

    #[gpui::test]
    fn 全部のボタンが落ちずに通る(cx: &mut gpui::TestAppContext) {
        // AI の宛先は覚える設定なので、試験で変えたら戻す
        let keep_ai = ui::ai::backend();
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        for tab in ui::ribbon::CALC {
            for cmd in tab.cmds {
                if !cmd.ready || DIALOG.contains(&cmd.id) {
                    continue;
                }
                let (id, label) = (cmd.id, cmd.label);
                c.update(cx, |this, cx| {
                    seed(this);
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

    /// リボンの「すべて選択」は**セル**に効く(バーの文字選択に化けない —
    /// Ctrl+A と同じ実体を通ることの検査。2026-08-05 に別実装のサボりを直した)
    #[gpui::test]
    fn 全選択はセルに効く(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            seed(this);
            this.anchor = None;
            this.sync_input(); // 実機ではカーソル移動が必ず呼ぶ
            this.run_cmd("selectall", cx);
            let (rows, cols) = this.sheet().extent();
            assert_eq!(this.anchor, Some(Pos::parse("A1").unwrap()), "起点が A1 でない");
            assert_eq!(
                this.cursor,
                Pos::new(rows - 1, cols - 1),
                "使われている範囲の端まで選べていない"
            );
        });
    }

    /// 押すと入切するボタンは、2回押すと元に戻る(1手で戻せる方針)
    #[gpui::test]
    fn 入切のボタンは二度おすと戻る(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        let state = |this: &Calc, id: &str| -> bool {
            match id {
                "show-formulas" => this.show_formulas,
                "show-gridlines" => this.gridlines,
                "co-showcomment" => this.show_comments,
                "formula-bar" => this.show_formula_bar,
                "show-headings" => this.show_headers,
                "show-zeros" => this.show_zeros,
                "rtl-sheet" => this.sheet().rtl,
                _ => unreachable!(),
            }
        };
        for id in [
            "show-formulas", "show-gridlines", "co-showcomment", "formula-bar",
            "show-headings", "show-zeros", "rtl-sheet",
        ] {
            c.update(cx, |this, cx| {
                seed(this);
                // freeze は A1 では効かない仕様(固定する位置が要る)
                this.cursor = Pos::parse("B2").unwrap();
                this.anchor = None;
                let before = state(this, id);
                this.run_cmd(id, cx);
                assert_ne!(before, state(this, id), "「{id}」を押しても変わらない");
                this.run_cmd(id, cx);
                assert_eq!(before, state(this, id), "「{id}」が元に戻らない");
            });
        }
    }

    /// **見本のブックを開いた状態でも**全部のボタンが通る。
    /// 空のブックと違い、式・結合・列幅・条件付き書式が入っているので
    /// 「前提があるときの道」も通る(sample/*.xlsx が検査の材料)。
    /// 見本は写しを開く — 署名やチャットが隣にファイルを添えるため、
    /// 追跡している見本の隣を汚さない
    #[gpui::test]
    fn 見本を開いても全部のボタンが通る(cx: &mut gpui::TestAppContext) {
        let dir = std::path::Path::new("../sample");
        let dir = if dir.exists() {
            dir.to_path_buf()
        } else {
            std::path::Path::new("sample").to_path_buf()
        };
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return; // 見本が無い環境では黙って飛ばす(失敗にはしない)
        };
        let mut files: Vec<std::path::PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("xlsx"))
            .collect();
        files.sort();
        assert!(!files.is_empty(), "見本が無い: {}", dir.display());
        let work = std::env::temp_dir().join(format!("jo-menu-{}", std::process::id()));
        std::fs::create_dir_all(&work).unwrap();
        let keep_ai = ui::ai::backend();
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        for f in files {
            let copy = work.join(f.file_name().unwrap());
            std::fs::copy(&f, &copy).unwrap();
            c.update(cx, |this, _| this.open(copy.clone()));
            for tab in ui::ribbon::CALC {
                for cmd in tab.cmds {
                    if !cmd.ready || DIALOG.contains(&cmd.id) {
                        continue;
                    }
                    let (id, label) = (cmd.id, cmd.label);
                    let name = f.file_name().unwrap().to_string_lossy().to_string();
                    c.update(cx, |this, cx| {
                        this.run_cmd(id, cx);
                        let st = this.status.to_string();
                        assert!(
                            !st.contains("未配線"),
                            "{name} で「{label}」({id}) が未配線: {st}"
                        );
                    });
                }
            }
            c.update(cx, |this, _| this.release_lock());
        }
        ui::ai::set_backend(keep_ai);
        let _ = std::fs::remove_dir_all(&work);
    }
}

#[cfg(test)]
mod wiring_tests {
    #[test]
    fn リボンのreadyは全部配線されている() {
        for tab in ui::ribbon::CALC {
            for cmd in tab.cmds {
                if cmd.ready {
                    assert!(
                        crate::Calc::HANDLED.contains(&cmd.id),
                        "「{}」({}) は ready なのに run_cmd が知らない",
                        cmd.label, cmd.id
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod paper_tests {
    use crate::*;

    #[test]
    fn 用紙コードはjisのbで引く() {
        assert_eq!(paper_mm(9), Some((210.0, 297.0, "A4")));
        assert_eq!(paper_mm(12), Some((257.0, 364.0, "B4")), "B4 は JIS の紙");
        assert_eq!(paper_mm(99), None, "知らないコードを黙って A4 にしない");
    }
}

#[cfg(test)]
mod index_at_tests {
    use crate::*;

    #[test]
    fn 位置から列が引ける() {
        let cols = [(0u32, 108.0f32), (1, 54.0), (2, 108.0)];
        assert_eq!(index_at(&cols, HEAD_W, HEAD_W + 1.0), Some(0));
        assert_eq!(index_at(&cols, HEAD_W, HEAD_W + 107.9), Some(0));
        assert_eq!(index_at(&cols, HEAD_W, HEAD_W + 108.0), Some(1), "境界は次の区分");
        assert_eq!(index_at(&cols, HEAD_W, HEAD_W + 200.0), Some(2));
        assert_eq!(index_at(&cols, HEAD_W, HEAD_W + 400.0), None, "並びの外");
        assert_eq!(index_at(&cols, HEAD_W, 10.0), None, "start より手前");
    }
}

#[cfg(test)]
mod goal_seek_tests {
    use crate::*;

    #[test]
    fn 合計を目標に数量が逆算できる() {
        // 見本の表: D2=B2*C2, D4=SUM, D6=D4+D5(消費税は固定にして単純化)
        let mut s = sheet::Sheet { name: "表".into(), ..Default::default() };
        s.set(Pos::parse("B2").unwrap(), Cell::input("4"));
        s.set(Pos::parse("C2").unwrap(), Cell::input("125000"));
        s.set(Pos::parse("D2").unwrap(), Cell::input("=B2*C2"));
        recalc(&mut s);
        // D2 を 800000 にする B2 は 6.4
        let x = solve_goal(&s, Pos::parse("D2").unwrap(), 800000.0, Pos::parse("B2").unwrap())
            .expect("見つからない");
        assert!((x - 6.4).abs() < 1e-6, "6.4 のはず: {x}");
        // 効かないセルでは正直に None
        assert!(
            solve_goal(&s, Pos::parse("D2").unwrap(), 800000.0, Pos::parse("F9").unwrap())
                .is_none(),
            "効かないセルで見つかったことにした"
        );
    }
}

#[cfg(test)]
mod lock_tests {
    use crate::*;

    #[test]
    fn 先客のロックが見え_自分のは先客に数えない() {
        let dir = std::env::temp_dir().join(format!("jo-lock-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let book = dir.join("台帳.xlsx");
        std::fs::write(&book, b"x").unwrap();
        let lp = lock_path_for(&book);
        assert!(lp.file_name().unwrap().to_string_lossy().starts_with(".~lock.台帳"));
        // 誰も居ない
        assert!(foreign_lock(&book).is_none());
        // 先客
        std::fs::write(&lp, "yamada@jimusho,;").unwrap();
        assert_eq!(foreign_lock(&book).as_deref(), Some("yamada@jimusho"));
        // 自分のロックは先客ではない
        std::fs::write(&lp, format!("{},;", lock_identity())).unwrap();
        assert!(foreign_lock(&book).is_none(), "自分を先客と間違えた");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod udf_tests {
    use crate::*;

    #[test]
    fn 台本の出力が解けてスピルが効く() {
        // 出力形式: セル \x1e 行 \x1e 行 … / 行の中は \x1f
        let raw = "B2\u{1e}10\u{1f}20\u{1e}30\u{1f}40\u{1c}D1\u{1e}こんにちは";
        let results = parse_udf_output(raw);
        assert_eq!(results.len(), 2);
        let mut sh = sheet::Sheet { name: "表".into(), ..Default::default() };
        let mut py = Cell::input("=PY(\"f\",A1)");
        py.value = sheet::Value::Error("#PY?".into());
        sh.set(Pos::parse("B2").unwrap(), py);
        let (spills, n, c) = apply_py_results(&mut sh, &results, &Default::default());
        assert_eq!((n, c), (2, 0));
        // アンカーは式を保ったまま値が入る
        let b2 = sh.get(Pos::parse("B2").unwrap()).unwrap();
        assert!(b2.formula.is_some(), "式が消えた");
        assert_eq!(b2.value, sheet::Value::Number(10.0));
        // スピル面
        assert_eq!(sh.value(Pos::parse("C3").unwrap()), sheet::Value::Number(40.0));
        assert_eq!(spills.get(&Pos::parse("B2").unwrap()), Some(&(2, 2)));
        assert_eq!(sh.value(Pos::parse("D1").unwrap()), sheet::Value::Text("こんにちは".into()));
    }

    #[test]
    fn スピル先に他人のデータがあれば止まる() {
        let mut sh = sheet::Sheet { name: "表".into(), ..Default::default() };
        sh.set(Pos::parse("B2").unwrap(), Cell::input("=PY(\"f\")"));
        sh.set(Pos::parse("C3").unwrap(), Cell::input("大事なメモ"));
        let raw = "B2\u{1e}1\u{1f}2\u{1e}3\u{1f}4";
        let (spills, n, c) =
            apply_py_results(&mut sh, &parse_udf_output(raw), &Default::default());
        assert_eq!((n, c), (0, 1));
        assert_eq!(
            sh.value(Pos::parse("B2").unwrap()),
            sheet::Value::Error("#SPILL!".into())
        );
        assert_eq!(
            sh.value(Pos::parse("C3").unwrap()),
            sheet::Value::Text("大事なメモ".into()),
            "他人のデータを潰した"
        );
        assert!(spills.is_empty());
    }

    #[test]
    fn 縮んだスピルの残骸は消える() {
        let mut sh = sheet::Sheet { name: "表".into(), ..Default::default() };
        sh.set(Pos::parse("A1").unwrap(), Cell::input("=PY(\"f\")"));
        // 前回 1x3 で展開していた
        sh.set(Pos::parse("B1").unwrap(), Cell::input("古い"));
        sh.set(Pos::parse("C1").unwrap(), Cell::input("残骸"));
        let mut prev = std::collections::HashMap::new();
        prev.insert(Pos::parse("A1").unwrap(), (1u32, 3u32));
        // 今回はスカラー
        let raw = "A1\u{1e}9";
        let (_, n, c) = apply_py_results(&mut sh, &parse_udf_output(raw), &prev);
        assert_eq!((n, c), (1, 0));
        assert_eq!(sh.value(Pos::parse("A1").unwrap()), sheet::Value::Number(9.0));
        assert!(sh.value(Pos::parse("C1").unwrap()).is_empty(), "残骸が残った");
    }

    #[test]
    fn 台本が実際にpythonで回る() {
        // .venv が無い機械では黙って飛ぶ(HIKITSUGI の作法)。
        // cargo test の cwd は calc/ なので、リポジトリ直下の .venv も見る
        let py = ["../.venv/bin/python", ".venv/bin/python"]
            .iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.exists());
        let Some(py) = py else { return };
        let dir = std::env::temp_dir().join(format!("jo-udf-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("out.txt");
        let mods = vec![(
            "道具".to_string(),
            "def 倍(x):\n    return x * 2\ndef 表(r):\n    return [[v * 10 for v in row] for row in r]"
                .to_string(),
        )];
        let calls = vec![
            (
                "B1".to_string(),
                "道具".to_string(),
                "倍".to_string(),
                vec![sheet::calc::PyArg::One(sheet::Value::Number(21.0))],
            ),
            (
                "D1".to_string(),
                "道具".to_string(),
                "表".to_string(),
                vec![sheet::calc::PyArg::Rect(
                    2,
                    vec![
                        sheet::Value::Number(1.0),
                        sheet::Value::Number(2.0),
                        sheet::Value::Number(3.0),
                        sheet::Value::Number(4.0),
                    ],
                )],
            ),
        ];
        let script = build_udf_script(&mods, &calls, &out);
        let py_path = dir.join("t.py");
        std::fs::write(&py_path, script).unwrap();
        let o = std::process::Command::new(&py).arg(&py_path).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let raw = std::fs::read_to_string(&out).unwrap();
        let results = parse_udf_output(&raw);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1[0][0], "42", "倍(21) が違う: {raw:?}");
        assert_eq!(results[1].1[1][1], "40", "表の2x2が違う");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[gpui::test]
    fn 見出しを打つと行が広がる(cx: &mut gpui::TestAppContext) {
        // 2026-08-09 発注者:「## 等 h1, h2, h3 の指定をした場合は、セルの高さも
        // 変更して。あらかじめ書式を決めておくといいですね」
        // 大きさの表は sheet::markdown::HEADINGS が正(画面の文字と同じ所を見る)
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            let base = 15.0_f32;
            fn h(this: &Calc, r: u32) -> f32 {
                *this.book.sheets[0].row_height.get(&r).unwrap_or(&15.0)
            }

            // 普通の文字では触らない
            this.cursor = Pos::new(0, 0);
            this.input = Editor::new("普通の文字");
            assert!(this.commit());
            assert_eq!(h(this, 0), base, "見出しでないのに行が動いた");

            // # は一番大きく、### に向かって小さくなる
            let mut heights = Vec::new();
            for (r, text) in [(1u32, "# 大"), (2, "## 中"), (3, "### 小")] {
                this.cursor = Pos::new(r, 0);
                this.input = Editor::new(text);
                assert!(this.commit());
                let got = h(this, r);
                assert!(got > base, "{text} で行が広がっていない({got}pt)");
                heights.push(got);
            }
            assert!(
                heights[0] > heights[1] && heights[1] > heights[2],
                "見出しの段で高さが並んでいない: {heights:?}"
            );
            // 既定のとおりか(画面の文字と同じ所を見ていること)
            assert!((heights[0] - base * sheet::markdown::DEFAULT_HEADINGS[0].scale).abs() < 0.01);

            // **型紙が正**: ブックに名前付きスタイル「見出し 1」があれば
            // そちらが勝つ(2026-08-09 発注者「テンプレートに設定できませんか?」)
            let mut big = sheet::model::CellFormat::default();
            big.size_c = Some(2200); // 22pt = 普通の 11pt の2倍
            big.bold = true;
            this.book.named_styles.push(("見出し 1".into(), Some(16), big));
            this.cursor = Pos::new(8, 0);
            this.input = Editor::new("# 型紙で決めた大きさ");
            assert!(this.commit());
            assert!(
                (h(this, 8) - base * 2.0).abs() < 0.01,
                "型紙の「見出し 1」が効いていない({}pt)",
                h(this, 8)
            );

            // **狭めはしない** — 手で決めた高さを打ち直しで壊さない
            this.sheet_mut().row_height.insert(5, 60.0);
            this.cursor = Pos::new(5, 0);
            this.input = Editor::new("### 小");
            assert!(this.commit());
            assert_eq!(h(this, 5), 60.0, "手で決めた行の高さを縮めてしまった");
        });
    }

    #[gpui::test]
    fn 日本語の名前でも編集面が開く(cx: &mut gpui::TestAppContext) {
        // 発注者報告 2026-08-09:「道具.py は、編集できません」
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // 無い名前は下書きを作る(disk には触らない — 開くだけ)
            this.prompt = Some(("py", Editor::new("@edit 試し用モジュール")));
            this.finish_prompt(cx);
            let p = this.py_edit.as_ref().expect("編集面が開かない");
            assert_eq!(p.name, "試し用モジュール", "日本語の名前が渡っていない");
            assert!(p.ed.text().contains("def"), "下書きが入っていない");

            // 開いている間、打鍵は**コード**へ行く(表ではない)
            let before = this.py_edit.as_ref().unwrap().ed.text().to_string();
            ui::handler::replace(this, None, "x");
            let after = this.py_edit.as_ref().unwrap().ed.text().to_string();
            assert_ne!(before, after, "打った字がコードに入らない");

            // 名前を書かなければ、そう言う
            this.py_edit = None;
            this.prompt = Some(("py", Editor::new("@edit")));
            this.finish_prompt(cx);
            assert!(this.py_edit.is_none());
            assert!(this.status.contains("@edit 名前"), "案内が出ない: {}", this.status);
        });
    }

    #[test]
    fn pluginsの関数は裸の名前で式に書ける() {
        // 2026-08-09 発注者確定: 交換されるファイルはデータだけ。関数は各自の
        // plugins にあり、式には `=倍(A1)` と普通に書く(=PY("倍", A1) も残す)
        sheet::calc::set_udf_names(["倍".to_string(), "XWSPLIT".to_string()]);
        let mut s = sheet::Sheet::default();
        s.set(Pos::new(0, 0), sheet::Cell::input("21"));
        assert!(sheet::calc::is_py_formula("倍(A1)"), "裸の名前が UDF と見なされない");
        // 字句解析は ASCII を大文字にする — 小文字で書いても結ばれる
        assert!(sheet::calc::is_py_formula("xwsplit(A1)"), "小文字の名前が結ばれない");
        assert!(sheet::calc::is_py_formula("PY(\"倍\", A1)"), "古い書き方が壊れた");
        // 登録簿に無い名前はただの関数(#NAME? になる) — 勝手に UDF にしない
        assert!(!sheet::calc::is_py_formula("知らない関数(A1)"));
        // 複合式は UDF のセルではない(値だけを置く場所なので)
        assert!(!sheet::calc::is_py_formula("倍(A1)+1"));
        // 引数は普通の式として評価されて渡る
        let (name, args) = sheet::calc::eval_py_call(&s, "倍(A1)").expect("解けない");
        assert_eq!(name, "倍");
        assert!(matches!(&args[0], sheet::calc::PyArg::One(sheet::Value::Number(n)) if *n == 21.0));

        // --- 指紋(py_stamp)が動いたときだけ裏で計算し直す ---
        // (登録簿は機械にひとつなので、名前を使う試験はここに集めてある)
        let mut s = sheet::Sheet::default();
        s.set(Pos::new(0, 0), sheet::Cell::input("21"));
        s.set(Pos::new(0, 1), sheet::Cell::input("=倍(A1)"));
        sheet::calc::recalc(&mut s);
        let a = s.py_stamp;
        assert_ne!(a, 0, "UDF のセルがあるのに指紋が立たない");
        sheet::calc::recalc(&mut s);
        assert_eq!(a, s.py_stamp, "同じ中身で指紋が動いた(計算し直しが止まらない)");
        s.set(Pos::new(0, 0), sheet::Cell::input("22"));
        sheet::calc::recalc(&mut s);
        assert_ne!(a, s.py_stamp, "引数が変わったのに指紋が動かない");
        // UDF のセルが無いブックでは 0(見張りは何もしない)
        let mut t = sheet::Sheet::default();
        t.set(Pos::new(0, 0), sheet::Cell::input("=1+1"));
        sheet::calc::recalc(&mut t);
        assert_eq!(t.py_stamp, 0);
        sheet::calc::set_udf_names(Vec::new());
    }

    #[test]
    fn シート名の変更が式の参照に追随する() {
        // 素の参照と '…' 付きの両方を書き換える
        assert_eq!(
            rename_refs_in("Sheet2!A1+SUM('Sheet2'!B1:B9)", "Sheet2", "集計").as_deref(),
            Some("集計!A1+SUM(集計!B1:B9)")
        );
        // 別の語の続き(合計! の中の 計!)は書き換えない
        assert_eq!(rename_refs_in("合計!A1", "計", "x"), None);
        // 文字列の中は触らない
        assert_eq!(rename_refs_in("IF(A1=\"Sheet2!\",1,2)", "Sheet2", "x"), None);
        // 空白入りの新しい名前は '…' で包む
        assert_eq!(
            rename_refs_in("Sheet2!A1", "Sheet2", "売 上").as_deref(),
            Some("'売 上'!A1")
        );
        // ブック全体: 式の数を数え、名前の定義も追随する
        let mut b = Book::new();
        b.sheets.push(sheet::Sheet::new("Sheet2"));
        b.sheets[0].set(Pos::parse("A1").unwrap(), Cell::input("=Sheet2!B1*2"));
        b.sheets[0].names.push(("単価".into(), "Sheet2!B2".into()));
        let n = rename_sheet_refs(&mut b, "Sheet2", "集計");
        assert_eq!(n, 1);
        assert_eq!(
            b.sheets[0].get(Pos::parse("A1").unwrap()).unwrap().formula.as_deref(),
            Some("集計!B1*2") // 式は = 抜きで持つ
        );
        assert_eq!(b.sheets[0].names[0].1, "集計!B2");
    }

    #[test]
    fn 複製の名前はexcelの流儀() {
        let mut b = Book::new();
        let base = b.sheets[0].name.clone();
        assert_eq!(copy_sheet_name(&b, &base), format!("{base} (2)"));
        b.sheets.push(sheet::Sheet::new(&format!("{base} (2)")));
        assert_eq!(copy_sheet_name(&b, &base), format!("{base} (3)"));
    }
}

#[cfg(test)]
mod pivot_e2e_tests {
    use crate::*;

    /// 実物の python+polars で端から端まで(挿入 → 置かれる → pivot_at →
    /// ピボット上のロック)。.venv が見つからない環境では飛ばす
    #[gpui::test]
    async fn ピボットは挿入から締めまで通しで効く(cx: &mut gpui::TestAppContext) {
        if !std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../.venv/bin/python")
            .exists()
        {
            eprintln!("skip: .venv が無い(polars の端到端は飛ばす)");
            return;
        }
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for (a1, v) in [
                ("A1", "区分"), ("B1", "月"), ("C1", "金額"),
                ("A2", "筆記具"), ("B2", "4月"), ("C2", "100"),
                ("A3", "紙製品"), ("B3", "5月"), ("C3", "200"),
                ("A4", "筆記具"), ("B4", "5月"), ("C4", "50"),
            ] {
                this.cursor = Pos::parse(a1).unwrap();
                this.sync_input();
                this.input.insert(v);
                assert!(this.commit());
            }
            this.anchor = None;
            this.cursor = Pos::parse("B2").unwrap();
            this.sync_input();
            this.run_cmd("pivot-insert", cx);
            this.apply_pick("☐ 区分", cx);
            this.apply_pick("→ 決定(列の選択へ)", cx);
            this.apply_pick("→ 決定(列は無しでもよい)", cx);
            this.apply_pick("金額", cx);
            this.apply_pick("合計", cx);
        });
        // polars の子プロセスが返るまで(background executor を回す)
        cx.executor().advance_clock(std::time::Duration::from_secs(30));
        cx.run_until_parked();
        c.update(cx, |this, cx| {
            assert_eq!(this.book.pivots.len(), 1, "ピボットが置かれない: {}", this.status);
            let d = this.book.pivots[0].clone();
            assert!(d.size.0 > 0, "大きさが入らない");
            // 出力の頭(見出しの下)に合計が入っている
            let val = |p: Pos| {
                this.book.sheets[0].get(p).map(|c| c.value.display()).unwrap_or_default()
            };
            let body: Vec<String> = (0..d.size.0)
                .map(|r| val(Pos::new(d.dest.row + r, d.dest.col + d.size.1 - 1)))
                .collect();
            assert!(
                body.iter().any(|v| v == "150"),
                "筆記具の合計 150 が出ない: {body:?}"
            );
            // 総計は既定で入り(本家と同じ)、見出しには本家風の帯が掛かる
            let all: Vec<String> = (0..d.size.0)
                .flat_map(|r| (0..d.size.1).map(move |c| (r, c)))
                .map(|(r, c)| val(Pos::new(d.dest.row + r, d.dest.col + c)))
                .collect();
            assert!(all.iter().any(|v| v == "総計"), "総計が無い: {all:?}");
            assert!(all.iter().any(|v| v == "350"), "総計の値が無い: {all:?}");
            let head = this.book.sheets[0].get(d.dest).unwrap().fmt.clone();
            assert_eq!(head.fill.as_deref(), Some("4472C4"), "見出しの帯が無い");
            assert!(head.bold);
            // 置いた直後にカーソルが集計へ移り、ピボットのタブが開いている
            assert_eq!(this.cursor, d.dest, "カーソルが集計へ移らない");
            let ti = ribbon::calc_tabs()
                .iter()
                .position(|t| t.cmds.iter().any(|c| c.id == "pivot-layout"))
                .unwrap();
            assert_eq!(this.tab, ti, "ピボットテーブルのタブが開かない");
            // ピボットの上では締まる(文脈タブと同じ判定 pivot_at)
            assert!(this.pivot_at(this.cursor).is_some(), "pivot_at が効かない");
            this.run_cmd("data-validation", cx);
            assert!(this.dv_dlg.is_none(), "ピボットの上で入力規則が開いた");
            assert!(this.status.contains("ピボット"), "{}", this.status);
            // フィールドリスト: いまの指図が ✓ 入りで読み込まれる
            this.run_cmd("pivot-fields", cx);
            assert_eq!(this.pick_kind, "pivot-rows-pick", "フィールドリストが開かない");
            {
                let (items, _) = this.pick.as_ref().unwrap();
                assert!(items.iter().any(|(_, l)| l == "☑ 区分"), "既存の行が ✓ にならない: {items:?}");
            }
            // 月を「列」へ広げて置き直す(Excel の形 — 1行目に札が出る)
            this.apply_pick("→ 決定(列の選択へ)", cx);
            this.apply_pick("☐ 月", cx);
            this.apply_pick("→ 決定(列は無しでもよい)", cx);
            this.apply_pick("金額", cx);
            this.apply_pick("合計", cx);
        });
        cx.executor().advance_clock(std::time::Duration::from_secs(30));
        cx.run_until_parked();
        c.update(cx, |this, cx| {
            assert_eq!(this.book.pivots.len(), 1, "組み替えで増殖した: {}", this.status);
            let d = &this.book.pivots[0];
            assert_eq!(d.rows_sel, vec!["区分".to_string()], "組み替えが効かない");
            assert_eq!(d.cols_sel, vec!["月".to_string()], "列への組み替えが効かない");
            assert!(d.totals, "総計の性質が引き継がれない");
            // Excel と同じ1行目の札(合計 / 金額 と 月)
            let d = this.book.pivots[0].clone();
            let label = this.book.sheets[0]
                .get(d.dest)
                .map(|x| x.value.display())
                .unwrap_or_default();
            assert_eq!(label, "合計 / 金額", "1行目の札が無い");
            let month_label = this.book.sheets[0]
                .get(Pos::new(d.dest.row, d.dest.col + 1))
                .map(|x| x.value.display())
                .unwrap_or_default();
            assert_eq!(month_label, "月", "列の見出しの札が無い");
            // 絞り込み(▼ 相当): 紙製品を隠して置き直す
            this.pivot_flt = Some((
                0,
                "区分".into(),
                std::iter::once("紙製品".to_string()).collect(),
            ));
            this.pick_kind = "pivot-filter-pick";
            this.apply_pick("→ 決定(絞り込む)", cx);
        });
        cx.executor().advance_clock(std::time::Duration::from_secs(30));
        cx.run_until_parked();
        c.update(cx, |this, _| {
            let d = this.book.pivots[0].clone();
            assert_eq!(d.hide, vec![("区分".to_string(), vec!["紙製品".to_string()])]);
            let all: Vec<String> = (0..d.size.0)
                .flat_map(|r| (0..d.size.1).map(move |c| (r, c)))
                .map(|(r, c)| {
                    this.book.sheets[0]
                        .get(Pos::new(d.dest.row + r, d.dest.col + c))
                        .map(|x| x.value.display())
                        .unwrap_or_default()
                })
                .collect();
            assert!(!all.iter().any(|v| v == "紙製品"), "隠したのに出ている: {all:?}");
            assert!(all.iter().any(|v| v == "筆記具"), "残るはずの値が消えた: {all:?}");
        });
        // スタイルギャラリー: 緑を選ぶと帯が掛け替わる
        c.update(cx, |this, cx| {
            let d = this.book.pivots[0].clone();
            this.anchor = None;
            this.cursor = d.dest;
            this.sync_input();
            this.run_cmd("pivot-style", cx);
            assert_eq!(this.pick_kind, "pivot-style-pick", "スタイルの一覧が開かない");
            this.apply_pick("緑", cx);
        });
        cx.executor().advance_clock(std::time::Duration::from_secs(30));
        cx.run_until_parked();
        c.update(cx, |this, _| {
            let d = &this.book.pivots[0];
            assert_eq!(d.style, "緑");
            let head = this.book.sheets[0].get(d.dest).unwrap().fmt.clone();
            assert_eq!(head.fill.as_deref(), Some("548235"), "緑の帯にならない");
        });
    }
}

#[cfg(test)]
mod hide_lines_tests {
    use crate::*;

    #[gpui::test]
    fn 行と列を隠して戻せる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            for r in 0..5u32 {
                for col in 0..3u32 {
                    this.book.sheets[0].set(Pos::new(r, col), sheet::Cell::input("x"));
                }
            }
            // 2〜3行目(索引 1〜2)を選んで隠す
            this.select_rows(1, 2);
            this.hide_lines("hide-rows");
            assert_eq!(this.book.sheets[0].row_hidden.len(), 2);
            assert!(this.book.sheets[0].row_hidden.contains(&1));
            // 隠れた分を挟むように選んで戻す
            this.select_rows(0, 3);
            this.hide_lines("unhide-rows");
            assert!(this.book.sheets[0].row_hidden.is_empty(), "戻っていない");
            // 列も同じ器
            this.select_cols(1, 1);
            this.hide_lines("hide-cols");
            assert!(this.book.sheets[0].col_hidden.contains(&1));
            // Ctrl+Z で1手ずつ戻る
            this.undo_sheet();
            assert!(this.book.sheets[0].col_hidden.is_empty(), "undo で戻らない");
        });
    }

    #[gpui::test]
    fn 使っている行を全部は隠せない(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            for r in 0..3u32 {
                this.book.sheets[0].set(Pos::new(r, 0), sheet::Cell::input("x"));
            }
            this.select_rows(0, 2);
            this.hide_lines("hide-rows");
            assert!(this.book.sheets[0].row_hidden.is_empty(), "全部隠れてしまった");
            assert!(this.status.contains("全部は隠せません"), "{}", this.status);
            // 隠れていない所で「再表示」を押しても、黙って何もしない旨を言う
            this.select_rows(0, 1);
            this.hide_lines("unhide-rows");
            assert!(this.status.contains("隠れた"), "{}", this.status);
        });
    }
}

#[cfg(test)]
mod data_edge_tests {
    use crate::*;

    /// A1:A3 に中身、A4〜A6 は空、A7 に中身(飛び石の縦一列)
    fn setup(this: &mut Calc) {
        for a1 in ["A1", "A2", "A3", "A7"] {
            this.book.sheets[0].set(Pos::parse(a1).unwrap(), sheet::Cell::input("x"));
        }
    }

    #[gpui::test]
    fn 塊の終わりと次の塊へ飛ぶ(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            setup(this);
            // A1 から下 = 塊の終わり A3
            this.cursor = Pos::parse("A1").unwrap();
            assert_eq!(this.data_edge(1, 0), Pos::parse("A3").unwrap());
            // A3 から下 = 隣が空なので次の中身 A7
            this.cursor = Pos::parse("A3").unwrap();
            assert_eq!(this.data_edge(1, 0), Pos::parse("A7").unwrap());
            // A7 から上 = 隣が空なので次の中身 A3
            this.cursor = Pos::parse("A7").unwrap();
            assert_eq!(this.data_edge(-1, 0), Pos::parse("A3").unwrap());
            // A1 から上 = もう行けないのでそのまま
            this.cursor = Pos::parse("A1").unwrap();
            assert_eq!(this.data_edge(-1, 0), Pos::parse("A1").unwrap());
            // 中身の無い向きは使っている範囲の端で止まる(表の最果てへ飛ばない)
            this.cursor = Pos::parse("A1").unwrap();
            let e = this.data_edge(0, 1);
            assert_eq!(e.col, 0, "使っている範囲の外へ出た: {}", e.a1());
        });
    }

    #[gpui::test]
    fn 端まで選択は起点を保つ(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            setup(this);
            // Ctrl+Shift+↓ の実体: 起点を置いてから端へ動く
            this.cursor = Pos::parse("A1").unwrap();
            this.anchor = Some(this.cursor);
            this.cursor = this.data_edge(1, 0);
            let (a, b) = this.sel_rect();
            assert_eq!((a.a1(), b.a1()), ("A1".to_string(), "A3".to_string()));
            // もう一度伸ばすと次の塊まで(起点は動かない)
            this.cursor = this.data_edge(1, 0);
            let (a, b) = this.sel_rect();
            assert_eq!((a.a1(), b.a1()), ("A1".to_string(), "A7".to_string()));
        });
    }
}

#[cfg(test)]
mod data_table_tests {
    use crate::*;

    /// B1=単価 B2=数量 / B4 = B1*B2 の式。A5:A7 に数量の候補
    fn setup(this: &mut Calc) {
        let s = &mut this.book.sheets[0];
        s.set(Pos::parse("B1").unwrap(), sheet::Cell::input("100"));
        s.set(Pos::parse("B2").unwrap(), sheet::Cell::input("1"));
        s.set(Pos::parse("B4").unwrap(), sheet::Cell::input("=B1*B2"));
        // 1変数の表: A4 が角(空)・B4 が式・A5:A7 が入力値
        for (a1, v) in [("A5", "2"), ("A6", "3"), ("A7", "10")] {
            s.set(Pos::parse(a1).unwrap(), sheet::Cell::input(v));
        }
        sheet::recalc_all(&mut this.book);
    }

    #[gpui::test]
    fn 一変数の感度表が埋まる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            setup(this);
            // A4:B7 を選び、列の入力セル = B2(数量)
            this.anchor = Some(Pos::parse("A4").unwrap());
            this.cursor = Pos::parse("B7").unwrap();
            this.data_table(Some(Pos::parse("B2").unwrap()), None);
            let v = |a1: &str| this.book.sheets[0].value(Pos::parse(a1).unwrap()).as_number();
            assert_eq!(v("B5"), 200.0, "数量2 の答えが違う");
            assert_eq!(v("B6"), 300.0);
            assert_eq!(v("B7"), 1000.0);
            // 元の入力セルは荒らさない(複製の上で回したか)
            assert_eq!(v("B2"), 1.0, "入力セルが書き換わっている");
            // Ctrl+Z の1手で戻る(`drop(v)` と書いていたが v は Copy で、
            // 何も起きていなかった — 借りを外す必要はそもそも無い)
            this.undo_sheet();
            assert_eq!(
                this.book.sheets[0].value(Pos::parse("B5").unwrap()).as_number(),
                0.0,
                "undo で戻らない"
            );
        });
    }

    #[gpui::test]
    fn 二変数は角の式を使う(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            setup(this);
            let s = &mut this.book.sheets[0];
            // D4 が角の式、D5:D6 が列の入力(数量)、E4:F4 が行の入力(単価)
            s.set(Pos::parse("D4").unwrap(), sheet::Cell::input("=B1*B2"));
            s.set(Pos::parse("D5").unwrap(), sheet::Cell::input("2"));
            s.set(Pos::parse("D6").unwrap(), sheet::Cell::input("5"));
            s.set(Pos::parse("E4").unwrap(), sheet::Cell::input("10"));
            s.set(Pos::parse("F4").unwrap(), sheet::Cell::input("20"));
            sheet::recalc_all(&mut this.book);
            this.anchor = Some(Pos::parse("D4").unwrap());
            this.cursor = Pos::parse("F6").unwrap();
            this.data_table(
                Some(Pos::parse("B2").unwrap()), // 列 = 数量
                Some(Pos::parse("B1").unwrap()), // 行 = 単価
            );
            let v = |a1: &str| this.book.sheets[0].value(Pos::parse(a1).unwrap()).as_number();
            assert_eq!(v("E5"), 20.0, "数量2×単価10");
            assert_eq!(v("F5"), 40.0, "数量2×単価20");
            assert_eq!(v("E6"), 50.0, "数量5×単価10");
            assert_eq!(v("F6"), 100.0);
        });
    }

    #[gpui::test]
    fn 形が合わなければ正直に断る(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            setup(this);
            // 1セルだけの選択
            this.anchor = None;
            this.cursor = Pos::parse("A4").unwrap();
            this.data_table(Some(Pos::parse("B2").unwrap()), None);
            assert!(this.status.contains("四角い範囲"), "{}", this.status);
            // 2変数なのに角が式でない
            this.anchor = Some(Pos::parse("A4").unwrap());
            this.cursor = Pos::parse("B7").unwrap();
            this.data_table(
                Some(Pos::parse("B2").unwrap()),
                Some(Pos::parse("B1").unwrap()),
            );
            assert!(this.status.contains("角"), "{}", this.status);
        });
    }
}

#[cfg(test)]
mod track_changes_tests {
    use crate::*;

    #[gpui::test]
    fn 記録の入切で差分が刻まれる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            let s = &mut this.book.sheets[0];
            s.set(Pos::parse("A1").unwrap(), sheet::Cell::input("10"));
            s.set(Pos::parse("A2").unwrap(), sheet::Cell::input("消える"));
            // 記録を始める
            this.track_changes();
            assert!(this.track_from.is_some(), "記録が始まっていない");
            // 直す・足す・消す
            let s = &mut this.book.sheets[0];
            s.set(Pos::parse("A1").unwrap(), sheet::Cell::input("20"));
            s.set(Pos::parse("B1").unwrap(), sheet::Cell::input("=A1*2"));
            s.cells.remove(&Pos::parse("A2").unwrap());
            // 止めると差分が刻まれる
            this.track_changes();
            assert!(this.track_from.is_none(), "記録が止まっていない");
            let ch = &this.book.changes;
            assert_eq!(ch.len(), 3, "刻んだ数が違う: {ch:?}");
            let find = |a1: &str| {
                ch.iter().find(|c| c.at == Pos::parse(a1).unwrap()).expect(a1)
            };
            assert_eq!((find("A1").before.as_str(), find("A1").after.as_str()), ("10", "20"));
            // 増えたセルは before が空。**式は式のまま**刻む
            assert_eq!((find("B1").before.as_str(), find("B1").after.as_str()), ("", "=A1*2"));
            // 消えたセルは after が空
            assert_eq!((find("A2").before.as_str(), find("A2").after.as_str()), ("消える", ""));
            // 誰が・いつ が入っている
            assert!(!find("A1").who.is_empty() && find("A1").when.len() >= 16);
        });
    }

    #[gpui::test]
    fn 変わっていなければ何も刻まない(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _cx| {
            this.book.sheets[0].set(Pos::parse("A1").unwrap(), sheet::Cell::input("10"));
            this.track_changes();
            this.track_changes();
            assert!(this.book.changes.is_empty(), "変えていないのに刻まれた");
            assert!(this.status.contains("ありません"), "{}", this.status);
        });
    }

    #[test]
    fn 変更履歴がxlsxを往復する() {
        let mut b = sheet::Book::new();
        b.sheets[0].set(Pos::parse("A1").unwrap(), sheet::Cell::input("x"));
        b.changes.push(sheet::model::ChangeRec {
            who: "dev@機械".into(),
            when: "2026-08-08 15:30".into(),
            sheet: "Sheet1".into(),
            at: Pos::parse("B2").unwrap(),
            before: "10".into(),
            after: "=A1&\"<>\"".into(),
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        sheet::xlsx::write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = sheet::xlsx::read(buf).expect("読めない");
        assert_eq!(back.changes.len(), 1, "変更履歴が往復しない");
        let c = &back.changes[0];
        assert_eq!(c.who, "dev@機械");
        assert_eq!(c.at, Pos::parse("B2").unwrap());
        assert_eq!(c.after, "=A1&\"<>\"", "記号の逃がしが壊れた");
    }
}

#[cfg(test)]
mod tab_zoom_tests {
    use crate::*;

    #[gpui::test]
    fn 耳の品書きは保護の今の状態で言い分を変える(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            // 掛かっていなければ「シートを保護」
            this.open_sheet_menu(0);
            let (items, _) = this.pick.clone().expect("耳の品書きが出ない");
            assert!(items.iter().any(|(k, _)| k == "シートを保護"), "{items:?}");
            assert!(!items.iter().any(|(k, _)| k == "保護を解除"));

            // 押すと掛かる
            this.apply_pick("シートを保護", cx);
            assert!(this.book.sheets[0].protected, "保護が掛かっていない");
            assert!(this.status.contains("保護しました"), "{}", this.status);

            // 掛かっていれば「保護を解除」に変わる(押すまで分からない、を避ける)
            this.open_sheet_menu(0);
            let (items, _) = this.pick.clone().unwrap();
            assert!(items.iter().any(|(k, _)| k == "保護を解除"), "{items:?}");
            this.apply_pick("保護を解除", cx);
            assert!(!this.book.sheets[0].protected, "保護が外れていない");
        });
    }

    #[gpui::test]
    fn ズームは上下の端で止まり百に戻せる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for _ in 0..30 {
                this.run_cmd("zoom-in", cx);
            }
            assert!(this.zoom <= 2.0 + 1e-6, "上に抜けた: {}", this.zoom);
            for _ in 0..30 {
                this.run_cmd("zoom-out", cx);
            }
            assert!(this.zoom >= 0.5 - 1e-6, "下に抜けた: {}", this.zoom);
            // 右下の倍率を押したときと同じ戻し方
            this.zoom = 1.0;
            assert_eq!(this.zoom, 1.0);
        });
    }
}

#[cfg(test)]
mod flash_fill_tests {
    use crate::*;

    #[test]
    fn 見本から作り方を読み取る() {
        // 姓 + 空白 + 名
        let ex = vec![(vec!["山田".into(), "太郎".into()], "山田 太郎".into())];
        let r = flash_recipe(&ex).expect("読み取れない");
        assert_eq!(
            flash_apply(&r, &["鈴木".into(), "花子".into()]).unwrap(),
            "鈴木 花子"
        );
        // 一部だけ切り出す(頭文字)
        let ex2 = vec![(vec!["2026-08-09".into()], "2026".into())];
        let r2 = flash_recipe(&ex2).expect("読み取れない");
        assert_eq!(flash_apply(&r2, &["1999-01-02".into()]).unwrap(), "1999");
    }

    #[test]
    fn 見本を作り直せない作り方は採らない() {
        // 2つの見本が食い違う(1つ目は姓+名、2つ目は名だけ)→ 諦める
        let ex = vec![
            (vec!["山田".into(), "太郎".into()], "山田 太郎".into()),
            (vec!["鈴木".into(), "花子".into()], "花子".into()),
        ];
        assert!(flash_recipe(&ex).is_none(), "当てずっぽうで作り方を作った");
    }

    #[gpui::test]
    fn 見本の下だけを埋めて既にある所は触らない(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for (i, (a, b)) in [("山田", "太郎"), ("鈴木", "花子"), ("佐藤", "一郎")]
                .iter()
                .enumerate()
            {
                this.book.sheets[0].set(Pos::new(i as u32, 0), sheet::Cell::input(a));
                this.book.sheets[0].set(Pos::new(i as u32, 1), sheet::Cell::input(b));
            }
            // C1 に見本、C3 は先に埋まっている
            this.book.sheets[0].set(Pos::parse("C1").unwrap(), sheet::Cell::input("山田 太郎"));
            this.book.sheets[0].set(Pos::parse("C3").unwrap(), sheet::Cell::input("触るな"));
            this.cursor = Pos::parse("C1").unwrap();
            this.anchor = None;
            this.sync_input();
            this.run_cmd("flash-fill", cx);

            let g = |a1: &str| {
                this.book.sheets[0]
                    .get(Pos::parse(a1).unwrap())
                    .map(|x| x.value.display())
                    .unwrap_or_default()
            };
            assert_eq!(g("C2"), "鈴木 花子", "書き方どおりに埋まっていない");
            assert_eq!(g("C3"), "触るな", "既にある所を上書きした");
        });
    }

    #[gpui::test]
    fn 読み取れなければ黙って埋めない(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.book.sheets[0].set(Pos::new(0, 0), sheet::Cell::input("あ"));
            this.book.sheets[0].set(Pos::new(1, 0), sheet::Cell::input("い"));
            // 元と何の関係も無い見本
            this.book.sheets[0].set(Pos::parse("B1").unwrap(), sheet::Cell::input("XYZ"));
            this.book.sheets[0].set(Pos::parse("B2").unwrap(), sheet::Cell::input("123"));
            this.cursor = Pos::parse("B1").unwrap();
            this.anchor = None;
            this.sync_input();
            this.run_cmd("flash-fill", cx);
            assert!(
                this.status.contains("読み取れませんでした") || this.status.contains("埋める所"),
                "当てずっぽうで埋めた: {}",
                this.status
            );
        });
    }
}

#[cfg(test)]
mod symbol_watch_tests {
    use crate::*;

    #[gpui::test]
    fn 記号は組で選んでから字を選び最近使った分が先に出る(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.run_cmd("inssymbol", cx);
            let (items, _) = this.pick.clone().expect("組の一覧が出ない");
            // 鍵は `symbols:組の名`、見出しは「組の名: 字たち」
            assert!(items.iter().any(|(k, _)| k == "symbols:帳票でよく使う"), "組が出ていない");
            assert!(items.iter().any(|(k, _)| k.starts_with("Unicode")), "16進の口が無い");
            assert!(!items.iter().any(|(k, _)| k == "symbols:recent"), "まだ何も使っていない");

            // 組を選ぶと字が一つずつ並ぶ
            this.apply_pick("symbols:しるし", cx);
            let (chars, _) = this.pick.clone().expect("字の一覧が出ない");
            assert_eq!(chars[0].0, "○", "一字ずつになっていない: {chars:?}");

            // 選ぶと式に入り、最近使った分に積まれる
            this.apply_pick("★", cx);
            assert!(this.input.text().contains('★'), "差し込まれていない");
            assert_eq!(this.recent_symbols.first().map(|s| s.as_str()), Some("★"));

            this.run_cmd("inssymbol", cx);
            let (items, _) = this.pick.clone().unwrap();
            assert_eq!(items[0].0, "symbols:recent", "最近使った分が先に出ない");
        });
    }

    // 家の作法の日本語の試験名。ラテン大文字で始まるので non_snake_case が
    // 鳴る — **その場で許す**(まとめて消すと製品の命名まで見なくなる)
    #[allow(non_snake_case)]
    #[gpui::test]
    fn Unicodeの16進で記号を入れられて読めなければ断る(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.prompt = Some(("symbol-hex", Editor::new("3012")));
            this.finish_prompt(cx);
            assert!(this.input.text().contains('〒'), "U+3012 が入っていない");

            this.prompt = Some(("symbol-hex", Editor::new("ゆうびん")));
            this.finish_prompt(cx);
            assert!(this.status.contains("Unicode が読めません"), "黙って流した: {}", this.status);
            assert!(this.prompt.is_some(), "打ち直させていない");
        });
    }

    #[gpui::test]
    fn 見張りは一つずつ外せて押せば飛ぶ(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _| {
            let (a, b) = (Pos::parse("A1").unwrap(), Pos::parse("D9").unwrap());
            this.watch.push((0, a));
            this.watch.push((0, b));

            // 札を押すとそこへ飛ぶ(遠くても窓が追いつく)
            this.watch_goto(0, b);
            assert_eq!(this.cursor, b, "飛んでいない");

            // × は1つだけ外す
            this.watch_remove(0, a);
            assert_eq!(this.watch, vec![(0, b)], "1つだけ外せていない");
            assert!(this.status.contains("外しました"), "言っていない: {}", this.status);

            // もう無いものを外そうとしたら、そう言う(黙って成功にしない)
            this.watch_remove(0, a);
            assert!(this.status.contains("もうありません"), "黙って流した: {}", this.status);
        });
    }
}

#[cfg(test)]
mod names_tests {
    use crate::*;

    #[gpui::test]
    fn 名前を式の打っている所へ差し込む(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.book.sheets[0].names.push(("売上".into(), "B2:B10".into()));
            this.cursor = Pos::parse("D1").unwrap();
            this.sync_input();

            // 式がまだ空なら「=」から始めてくれる
            this.pick_kind = "paste-name";
            this.apply_pick("name:売上", cx);
            assert_eq!(this.input.text(), "=売上", "式に入っていない");

            // 続きを打ってから、また差し込む(末尾でなく打っている所へ)
            this.input.insert("+");
            this.pick_kind = "paste-name";
            this.apply_pick("name:売上", cx);
            assert_eq!(this.input.text(), "=売上+売上", "打っている所に入らない");
        });
    }

}

#[cfg(test)]
mod autofit_tests {
    use crate::*;

    #[gpui::test]
    fn 幅を中身に合わせるとはみ出さなくなる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            let p = Pos::parse("A1").unwrap();
            this.book.sheets[0].set(p, sheet::Cell::input("とても長い見出しの文字列です"));
            this.cursor = p;
            this.anchor = None;
            this.sync_input(); // 直に置いた中身を編集欄へ(commit が空で潰さないよう)
            let before = this.col_px(0);
            this.run_cmd("autofit-col", cx);
            let after = this.col_px(0);
            assert!(after > before, "幅が広がっていない({before} → {after})");
            // **はみ出しの判定と同じ物差し**で測って、収まっていること
            let need = text_px("とても長い見出しの文字列です", 12.5);
            assert!(after + 0.5 >= need, "合わせたのにまだはみ出す({after} < {need})");
        });
    }

    #[gpui::test]
    fn 折り返すセルは高さが行数ぶんになる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            let p = Pos::parse("A1").unwrap();
            let mut cell = sheet::Cell::input("あいうえおかきくけこさしすせそたちつてと");
            cell.fmt.wrap = true;
            this.book.sheets[0].set(p, cell);
            this.book.sheets[0].col_width.insert(0, 6.0); // わざと狭く
            this.cursor = p;
            this.anchor = None;
            this.sync_input();
            this.run_cmd("autofit-row", cx);
            let h = *this.book.sheets[0].row_height.get(&0).unwrap();
            assert!(h > 15.0 * 2.0, "折り返しぶん高くなっていない({h} pt)");
        });
    }

    #[gpui::test]
    fn 中身が無ければ何もしないと言う(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.cursor = Pos::parse("A1").unwrap();
            this.anchor = None;
            this.run_cmd("autofit-col", cx);
            assert!(
                this.status.contains("中身が無い"),
                "黙って何もしていない: {}",
                this.status
            );
        });
    }
}

#[cfg(test)]
mod color_tests {
    use crate::*;

    #[gpui::test]
    fn 色は16進で直に指定できて読めない字は断る(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            let p = Pos::parse("A1").unwrap();
            this.book.sheets[0].set(p, sheet::Cell::input("色"));
            this.cursor = p;

            // 「その他」を選ぶと打ち込みのパネルが開く
            this.pick_kind = "font-color";
            this.apply_pick("その他(RRGGBB を打つ)…", cx);
            assert!(this.prompt.is_some(), "打ち込みのパネルが開かない");

            // #付きでも大文字小文字でも通る
            this.prompt = Some(("font-color-rgb", Editor::new("#ff8800")));
            this.finish_prompt(cx);
            assert_eq!(
                this.book.sheets[0].get(p).unwrap().fmt.color.as_deref(),
                Some("FF8800"),
                "16進の色が入っていない"
            );

            // 読めない字は**黙って黒にせず**、断って打ち直させる
            this.prompt = Some(("fill-color-rgb", Editor::new("みどり")));
            this.finish_prompt(cx);
            assert!(this.book.sheets[0].get(p).unwrap().fmt.fill.is_none(), "でたらめな色が入った");
            assert!(this.status.contains("色が読めません"), "理由を言っていない: {}", this.status);
            assert!(this.prompt.is_some(), "打ち直させていない");
        });
    }
}

#[cfg(test)]
mod cse_tests {
    use crate::*;

    #[gpui::test]
    fn 範囲を選んで配列数式を入れると範囲いっぱいに答えが入る(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for i in 0..3u32 {
                this.book.sheets[0]
                    .set(Pos::new(i, 0), sheet::Cell::input(&((i + 1) * 2).to_string()));
            }
            // C1:C3 を選んで =A1:A3*10 を配列で入れる
            this.cursor = Pos::parse("C1").unwrap();
            this.anchor = Some(Pos::parse("C3").unwrap());
            this.set_array_formula("=A1:A3*10", cx);

            let at = Pos::parse("C1").unwrap();
            assert_eq!(this.book.sheets[0].cse.get(&at), Some(&(3, 1)), "配列の印が付かない");
            for (a1, want) in [("C1", "20"), ("C2", "40"), ("C3", "60")] {
                let p = Pos::parse(a1).unwrap();
                assert_eq!(
                    this.book.sheets[0].get(p).unwrap().value.display(),
                    want,
                    "{a1} が違う"
                );
            }
            // 数式バーでは { } で囲んで見せる(普通の式と見分けられるように)
            this.cursor = at;
            this.sync_input();
            assert_eq!(this.input.text(), "{=A1:A3*10}", "配列数式の印が見えない");
        });
    }

    #[gpui::test]
    fn 配列数式の一部は書き換えられない(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            for i in 0..3u32 {
                this.book.sheets[0].set(Pos::new(i, 0), sheet::Cell::input("2"));
            }
            this.cursor = Pos::parse("C1").unwrap();
            this.anchor = Some(Pos::parse("C3").unwrap());
            this.set_array_formula("=A1:A3*10", cx);

            // 真ん中のセルを普通に書き換えようとする → 断られる
            this.cursor = Pos::parse("C2").unwrap();
            this.input = Editor::new("999");
            assert!(!this.commit(), "配列の一部が書き換えられてしまった");
            assert_eq!(
                this.book.sheets[0].get(Pos::parse("C2").unwrap()).unwrap().value.display(),
                "20",
                "値が変わってしまった"
            );
            assert!(this.status.contains("配列数式の一部"), "理由を言っていない: {}", this.status);

            // **範囲ぜんぶを選べば消せる**
            this.cursor = Pos::parse("C1").unwrap();
            this.anchor = Some(Pos::parse("C3").unwrap());
            this.sync_input();
        });
    }
}

#[cfg(test)]
mod csv_out_tests {
    use crate::*;

    // 家の作法の日本語の試験名。ラテン大文字で始まるので non_snake_case が
    // 鳴る — **その場で許す**(まとめて消すと製品の命名まで見なくなる)
    #[allow(non_snake_case)]
    #[gpui::test]
    fn CSVはShift_JISでも書けて落ちた字を数える(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        let dir = std::env::temp_dir().join(format!("jo-csv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        c.update(cx, |this, _| {
            this.book.sheets[0].set(Pos::parse("A1").unwrap(), sheet::Cell::input("売上"));
            this.book.sheets[0].set(Pos::parse("B1").unwrap(), sheet::Cell::input("𠮟る"));

            // 既定は UTF-8 BOM 付き・カンマ
            let p = dir.join("u8.csv");
            this.write_csv(&p);
            let b = std::fs::read(&p).unwrap();
            assert_eq!(&b[..3], &[0xEF, 0xBB, 0xBF], "BOM が付いていない");
            assert!(String::from_utf8_lossy(&b).contains("売上,"), "カンマ区切りでない");

            // Shift_JIS。**CP932 に無い字(𠮟)は落ちるので数えて言う**
            this.csv_kind = "Shift_JIS(CP932)・カンマ";
            let p2 = dir.join("sjis.csv");
            this.write_csv(&p2);
            let b2 = std::fs::read(&p2).unwrap();
            assert!(b2.starts_with(&[0x94, 0x84]), "Shift_JIS になっていない(売=0x9484)");
            assert!(
                this.status.contains("Shift_JIS に無く"),
                "落ちた字を黙っている: {}",
                this.status
            );

            // タブ区切り
            this.csv_kind = "UTF-8(BOM付き)・タブ";
            let p3 = dir.join("tab.csv");
            this.write_csv(&p3);
            let t = std::fs::read_to_string(&p3).unwrap();
            assert!(t.contains("売上\t"), "タブ区切りになっていない");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod recover_tests {
    use crate::*;

    #[gpui::test]
    fn 自動復旧の控えは原本を上書きしない(cx: &mut gpui::TestAppContext) {
        // **これがこの機能の肝。** 控えを取るたびに原本を書き換えていたら、
        // 「保存していないつもりの変更」が原本に入り Ctrl+Z でも戻せない
        let dir = std::env::temp_dir().join(format!("jo-recover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let orig = dir.join("原本.xlsx");
        {
            let mut b = sheet::Book::new();
            b.sheets[0].set(Pos::parse("A1").unwrap(), sheet::Cell::input("保存した値"));
            let mut f = std::fs::File::create(&orig).unwrap();
            sheet::xlsx::write(&b, &mut f).unwrap();
        }
        let before = std::fs::read(&orig).unwrap();

        let c = cx.update(|cx| cx.new(|cx| Calc::new(Some(orig.clone()), cx)));
        c.update(cx, |this, cx| {
            this.book.sheets[0].set(Pos::parse("A1").unwrap(), sheet::Cell::input("打ちかけ"));
            this.dirty = true;
            this.write_recover(cx);
        });
        cx.run_until_parked();

        // 原本は1バイトも変わっていない
        assert_eq!(std::fs::read(&orig).unwrap(), before, "自動復旧が原本を書き換えた");
        // 控えは別の場所にできている
        let rp = Calc::recover_path_for(Some(&orig));
        assert!(rp.exists(), "控えができていない: {}", rp.display());
        // 控えの中身は打ちかけの方
        let (back, _) = sheet::xlsx::read(std::io::Cursor::new(std::fs::read(&rp).unwrap()))
            .expect("控えが読めない");
        assert_eq!(
            back.sheets[0].get(Pos::parse("A1").unwrap()).unwrap().value.display(),
            "打ちかけ",
            "控えに打ちかけが入っていない"
        );
        // 元の道が添えてある(どのファイルの控えかを言えるように)
        let side = std::fs::read_to_string(rp.with_extension("path")).unwrap();
        assert_eq!(side, orig.to_string_lossy(), "元の道が添えられていない");

        // 無事に保存できたら控えは消える(残すと次の起動で嘘を言う)
        c.update(cx, |this, _| this.drop_recover());
        assert!(!rp.exists(), "保存しても控えが残っている");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[gpui::test]
    fn 控えから開いても原本の道は持たない(cx: &mut gpui::TestAppContext) {
        // 控えを開いて Ctrl+S を押したら原本が上書きされる、では意味がない
        let dir = std::env::temp_dir().join(format!("jo-recover2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stale = dir.join("控え.xlsx");
        {
            let mut b = sheet::Book::new();
            b.sheets[0].set(Pos::parse("A1").unwrap(), sheet::Cell::input("控えの値"));
            let mut f = std::fs::File::create(&stale).unwrap();
            sheet::xlsx::write(&b, &mut f).unwrap();
        }
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            this.pick_paths = vec![("/どこか/原本.xlsx".into(), stale.clone())];
            this.pick_kind = "recover";
            this.apply_pick("/どこか/原本.xlsx", cx);
            assert_eq!(
                this.book.sheets[0].get(Pos::parse("A1").unwrap()).unwrap().value.display(),
                "控えの値",
                "控えの中身が開けていない"
            );
            assert!(this.path.is_none(), "控えを開いたのに道を持っている(Ctrl+S で原本を潰す)");
            assert!(this.dirty, "保存を促していない");
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod protect_tests {
    use crate::*;

    #[test]
    fn セルのロックと許可する操作がxlsxを往復する() {
        let mut b = sheet::Book::new();
        // A1 はロックのまま、B2 はロックを外す(帳票の記入欄)
        b.sheets[0].set(Pos::parse("A1").unwrap(), sheet::Cell::input("見出し"));
        let mut c = sheet::Cell::input("");
        c.fmt.unlocked = true;
        b.sheets[0].set(Pos::parse("B2").unwrap(), c);
        b.sheets[0].protected = true;
        b.sheets[0].protect_allow.format_cells = true;
        b.sheets[0].protect_allow.sort = true;
        b.sheets[0].protect_allow.select_locked = false;

        let mut buf = std::io::Cursor::new(Vec::new());
        sheet::xlsx::write(&b, &mut buf).expect("書けない");
        buf.set_position(0);
        let (back, _) = sheet::xlsx::read(buf).expect("読めない");
        let sh = &back.sheets[0];
        assert!(sh.protected, "保護が往復しない");
        assert!(
            !sh.get(Pos::parse("A1").unwrap()).unwrap().fmt.unlocked,
            "ロックしたセルが往復で外れた"
        );
        assert!(
            sh.get(Pos::parse("B2").unwrap()).unwrap().fmt.unlocked,
            "ロックを外したセルが往復で掛かった"
        );
        // **向きが裏返らないこと。** xlsx は「禁じる」で書き、こちらは
        // 「許す」で持つので、往復のどこかで逆になりやすい
        assert!(sh.protect_allow.format_cells, "許した書式が禁止に化けた");
        assert!(sh.protect_allow.sort, "許した並べ替えが禁止に化けた");
        assert!(!sh.protect_allow.select_locked, "禁じた選択が許可に化けた");
        assert!(!sh.protect_allow.insert_rows, "禁じたままのはずの行挿入が許可に化けた");
        assert!(sh.protect_allow.select_unlocked, "既定で許すはずの選択が禁止になった");
    }

    #[gpui::test]
    fn 保護中もロックを外したセルには書ける(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            let head = Pos::parse("A1").unwrap();
            let entry = Pos::parse("B2").unwrap();
            this.book.sheets[0].set(head, sheet::Cell::input("見出し"));
            // B2 のロックを外す(選んでから「セルのロック」)
            this.cursor = entry;
            this.anchor = None;
            this.run_cmd("cell-lock", cx);
            assert!(
                this.book.sheets[0].get(entry).map(|c| c.fmt.unlocked).unwrap_or(false),
                "ロックが外れていない"
            );
            this.book.sheets[0].protected = true;

            // 見出しは書けない
            this.cursor = head;
            this.sync_input();
            assert!(this.cell_locked(head), "ロックしたセルが素通りする");
            // 記入欄は書ける
            this.cursor = entry;
            assert!(!this.cell_locked(entry), "ロックを外したセルまで堰き止めた");
        });
    }

    #[gpui::test]
    fn 許した操作だけが保護中に通る(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, cx| {
            let p = Pos::parse("A1").unwrap();
            this.book.sheets[0].set(p, sheet::Cell::input("あ"));
            this.cursor = p;
            this.book.sheets[0].protected = true;

            // 既定では書式も禁じる
            this.run_cmd("bold", cx);
            assert!(!this.book.sheets[0].get(p).unwrap().fmt.bold, "禁じた書式が通った");

            // 「セルの書式設定」を許すと通る
            this.book.sheets[0].protect_allow.format_cells = true;
            this.run_cmd("bold", cx);
            assert!(this.book.sheets[0].get(p).unwrap().fmt.bold, "許した書式が通らない");
        });
    }
}

#[cfg(test)]
mod stale_string_tests {
    use crate::*;

    #[test]
    fn 文字列の中の古いシート名を数える() {
        let mut b = sheet::Book::new();
        b.sheets[0].name = "表紙".into();
        let s = &mut b.sheets[0];
        // 追随する(文字列の外)
        s.set(Pos::parse("A1").unwrap(), sheet::Cell::input("=4月!B2"));
        // 追随しない(文字列の中)— これを数える
        s.set(Pos::parse("A2").unwrap(), sheet::Cell::input("=INDIRECT(\"4月!B2\")"));
        s.set(Pos::parse("A3").unwrap(), sheet::Cell::input("=SUM(INDIRECT(\"4月!B1:B9\"))"));
        // ただの文字(参照の形でない)は数えない
        s.set(Pos::parse("A4").unwrap(), sheet::Cell::input("=\"4月の売上\""));
        // 別の語の続きは別物(「決算4月!」の中の「4月!」)
        s.set(Pos::parse("A5").unwrap(), sheet::Cell::input("=INDIRECT(\"決算4月!B2\")"));
        assert_eq!(stale_in_strings(&b, "4月"), 2, "数え方が違う");
        // 改名しても文字列の中は変わらない(Excel と同じ)
        rename_sheet_refs(&mut b, "4月", "April");
        let f = |a1: &str| {
            b.sheets[0].get(Pos::parse(a1).unwrap()).unwrap().editable().to_string()
        };
        assert_eq!(f("A1"), "=April!B2", "文字列の外は追随する");
        assert_eq!(f("A2"), "=INDIRECT(\"4月!B2\")", "文字列の中を書き換えてしまった");
    }
}

#[cfg(test)]
mod cycle_ref_tests {
    use crate::util::cycle_ref_at;

    /// F4 = 参照の $ を回す。**一巡して元に戻る**ことまで見る —
    /// 途中で止まると「戻せない」になり、押すのが怖い鍵になる
    #[test]
    fn 参照のドルを一巡させる() {
        let c = |t: &str| cycle_ref_at(t, t.len());
        assert_eq!(c("=A1"), Some(("=$A$1".into(), 5)));
        assert_eq!(c("=$A$1"), Some(("=A$1".into(), 4)));
        assert_eq!(c("=A$1"), Some(("=$A1".into(), 4)));
        assert_eq!(c("=$A1"), Some(("=A1".into(), 3)));
        // 4回押すと元通り
        let mut t = "=SUM(B12:C20)".to_string();
        let mut cur = 12; // C20 の直後
        for _ in 0..4 {
            let (n, p) = cycle_ref_at(&t, cur).expect("参照が見つからない");
            t = n;
            cur = p;
        }
        assert_eq!(t, "=SUM(B12:C20)", "一巡して元に戻らない");
    }

    #[test]
    fn 参照でないものには効かない() {
        // 関数名(直後が丸かっこ)
        assert_eq!(cycle_ref_at("=LOG10(", 6), None);
        // 数だけ・文字だけ
        assert_eq!(cycle_ref_at("=123", 4), None);
        assert_eq!(cycle_ref_at("=ABC", 4), None);
        // 列が4文字以上
        assert_eq!(cycle_ref_at("=ABCD1", 6), None);
    }

    #[test]
    fn シート名つきの参照はセルの側だけ回す() {
        let c = |t: &str| cycle_ref_at(t, t.len());
        assert_eq!(c("=4月!B2"), Some(("=4月!$B$2".into(), "=4月!$B$2".len())));
        assert_eq!(c("='売上 表'!B2"), Some(("='売上 表'!$B$2".into(), "='売上 表'!$B$2".len())));
    }

    /// カーソルが参照の**途中**にいても、その参照を回す
    /// (打っている最中に押すのが普通)
    #[test]
    fn 参照の途中で押しても効く() {
        // "=A12" の A と 1 の間
        assert_eq!(cycle_ref_at("=A12", 2), Some(("=$A$12".into(), 6)));
    }
}

#[cfg(test)]
mod slicer_tests {
    use crate::util::{slicer_items, slicer_cmp};

    fn rows(vs: &[(&str, bool)]) -> Vec<(String, bool)> {
        vs.iter().map(|(v, l)| (v.to_string(), *l)).collect()
    }

    /// **数だけの値は数として並べる。** 文字として並べると 10 が 2 より前に
    /// 来て、伝票番号の列が読めなくなる
    #[test]
    fn 数の列は数の順に並ぶ() {
        let r = rows(&[("10", true), ("2", true), ("100", true), ("9", true)]);
        let (up, _) = slicer_items(&r, false, false);
        assert_eq!(up, vec!["2", "9", "10", "100"]);
        let (down, _) = slicer_items(&r, true, false);
        assert_eq!(down, vec!["100", "10", "9", "2"]);
    }

    #[test]
    fn 文字は符号位置の順に並ぶ() {
        let r = rows(&[("う", true), ("あ", true), ("い", true)]);
        assert_eq!(slicer_items(&r, false, false).0, vec!["あ", "い", "う"]);
        assert_eq!(slicer_items(&r, true, false).0, vec!["う", "い", "あ"]);
        // 数と文字が混じったら文字として比べる(数が先に来る)
        assert_eq!(slicer_cmp("2", "あ"), std::cmp::Ordering::Less);
    }

    /// 空欄は値ではないので**並べ替えの外・いちばん最後**。
    /// 降順にしても最後のまま(先頭に来ると値の一つに見える)
    #[test]
    fn 空白はいつも最後() {
        let r = rows(&[("い", true), ("", true), ("あ", true)]);
        assert_eq!(slicer_items(&r, false, false).0, vec!["あ", "い", "(空白)"]);
        assert_eq!(slicer_items(&r, true, false).0, vec!["い", "あ", "(空白)"]);
    }

    /// ⊘ = 他の絞りで一行も残っていない値を並べない。
    /// **同じ値の行が一つでも生きていれば残す**
    #[test]
    fn 行の無い値を外せる() {
        let r = rows(&[("あ", false), ("い", true), ("あ", true), ("う", false)]);
        assert_eq!(slicer_items(&r, false, false).0, vec!["あ", "い", "う"]);
        assert_eq!(slicer_items(&r, false, true).0, vec!["あ", "い"], "生きた行がある値まで消えた");
        // 空白も同じ扱い
        let r = rows(&[("あ", true), ("", false)]);
        assert_eq!(slicer_items(&r, false, true).0, vec!["あ"]);
        assert_eq!(slicer_items(&r, false, false).0, vec!["あ", "(空白)"]);
    }

    /// 64 を超えたら切るが、**何件切ったかを返す**(黙って切らない)
    #[test]
    fn 多すぎる値は数を添えて切る() {
        let v: Vec<(String, bool)> = (0..70).map(|i| (format!("{i:03}"), true)).collect();
        let (items, cut) = slicer_items(&v, false, false);
        assert_eq!(items.len(), 64);
        assert_eq!(cut, 6);
        // 切っていないときは 0
        assert_eq!(slicer_items(&v[..10], false, false).1, 0);
    }
}

/// sheet が持つ名前と、calc が持つ訳の対応表がずれていないか。
///
/// 名前の**本体は sheet 側**(ProtectAllow / SCHEMES)にある。sheet は zip と
/// quick-xml しか要らない器なので訳を持たせず、見出しは calc の表で当てる —
/// つまり同じ日本語が2箇所に書かれる。片方だけ増えると画面に日本語が混じるか、
/// 死んだ訳が残るので、**両方向**を見張る
#[cfg(test)]
mod sheet_name_table_tests {
    use crate::util::{color_schemes, protect_allows};

    #[test]
    fn 保護中に許す操作の名前は両方の表に揃っている() {
        let a = sheet::model::ProtectAllow::default();
        let mine = protect_allows();
        for (n, _) in a.items() {
            assert!(
                mine.iter().any(|(k, _)| *k == n),
                "sheet の ProtectAllow::items にあって calc の protect_allows に無い: 「{n}」\
                 (calc/src/util.rs に ui::item!(\"{n}\") を足す)"
            );
        }
        let theirs = a.items();
        for (k, _) in &mine {
            assert!(
                theirs.iter().any(|(n, _)| n == k),
                "calc の protect_allows にあって sheet の ProtectAllow::items に無い: 「{k}」\
                 (訳が宙に浮いている — 消すか、sheet 側に足す)"
            );
        }
        assert_eq!(mine.len(), theirs.len(), "並びの数が食い違う");
    }

    #[test]
    fn 配色の名前は両方の表に揃っている() {
        let mine = color_schemes();
        for (n, _) in sheet::theme::SCHEMES {
            assert!(
                mine.iter().any(|(k, _)| k == n),
                "sheet の theme::SCHEMES にあって calc の color_schemes に無い: 「{n}」\
                 (calc/src/util.rs に ui::item!(\"{n}\") を足す)"
            );
        }
        for (k, _) in &mine {
            assert!(
                sheet::theme::SCHEMES.iter().any(|(n, _)| n == k),
                "calc の color_schemes にあって sheet の theme::SCHEMES に無い: 「{k}」\
                 (訳が宙に浮いている — 消すか、sheet 側に足す)"
            );
        }
        assert_eq!(mine.len(), sheet::theme::SCHEMES.len(), "並びの数が食い違う");
    }
}

#[cfg(test)]
mod currency_tests {
    use crate::util::{currencies, currency_code};

    /// **記号は帳票のお金、並びは読む人の言語。** 独語(pattern 3)は
    /// 記号が後ろ、日本語(pattern 0)は前
    #[test]
    fn 記号は選び並びは言語で決まる() {
        assert_eq!(currency_code("¥", 0, 0), "\"¥\"#,##0", "日本語の並び");
        assert_eq!(currency_code("€", 2, 3), "#,##0.00 \"€\"", "独語の並び");
        assert_eq!(currency_code("$", 2, 0), "\"$\"#,##0.00");
        assert_eq!(currency_code("₩", 0, 1), "#,##0\"₩\"");
        assert_eq!(currency_code("£", 2, 2), "\"£\" #,##0.00");
    }

    /// **円に小数は付けない。** 「¥1,234.00」は日本の帳票では見ない
    #[test]
    fn 小数の桁は通貨で決まる() {
        let by = |k: &str| {
            currencies().iter().find(|(key, _, _, _)| *key == k).map(|(_, _, s, d)| (*s, *d)).unwrap()
        };
        assert_eq!(by("円 (¥)"), ("¥", 0));
        assert_eq!(by("ウォン (₩)"), ("₩", 0));
        assert_eq!(by("ドル ($)"), ("$", 2));
        assert_eq!(by("ユーロ (€)"), ("€", 2));
    }

    /// 記号なしはただの桁区切り
    #[test]
    fn 記号なしを選べる() {
        assert_eq!(currency_code("", 0, 0), "#,##0");
        assert_eq!(currency_code("", 2, 3), "#,##0.00");
    }

    /// **組んだコードを描き手が読めること。** 引用符つきの記号は
    /// 2026-08-10 まで落ちていたので、往復で確かめる
    #[test]
    fn 組んだコードが描ける() {
        let f = |code: &str| {
            // 起点は 1900(この試験は日付ではなく通貨の書式を見ている)
            sheet::model::format_value(&sheet::Value::Number(1234.0), Some(code), false)
        };
        assert_eq!(f(&currency_code("¥", 0, 0)), "¥1,234");
        assert_eq!(f(&currency_code("€", 2, 3)), "1,234.00 €");
        assert_eq!(f(&currency_code("", 0, 0)), "1,234");
    }
}

#[cfg(test)]
mod datefmt_tests {
    use crate::util::date_formats;

    /// **見出しは、その書式で描いた結果そのもの。**
    /// 「長い日付 (2026年8月6日)」のように例を焼き付けると、独語の人に
    /// 日本語の日付を約束することになる — 描いた物を出せば嘘のつきようがない
    #[test]
    fn 見出しが描いた結果と一致する() {
        for (_, label, code) in date_formats() {
            let drawn = sheet::model::format_value(
                &sheet::Value::Number(46240.0),
                Some(&code),
                false,
            );
            assert!(
                label.ends_with(&drawn),
                "見出しが結果と違う: {label} / 描くと {drawn}(コード {code})"
            );
        }
    }

    /// **日付の書式には地域を書き込む。** 残さないと、開いた人の環境しだいで
    /// 別の月名が出る。時刻だけは言語に関わらないので付けない
    #[test]
    fn 日付には地域が入り時刻には入らない() {
        let f = date_formats();
        let by = |k: &str| f.iter().find(|(key, _, _)| *key == k).unwrap().2.clone();
        for k in ["短い日付", "長い日付", "年と月", "曜日だけ"] {
            assert!(by(k).starts_with("[$-"), "{k} に地域が無い: {}", by(k));
        }
        assert_eq!(by("時刻"), "h:mm:ss", "時刻に地域は要らない");
    }

    /// 日本語で動かしているので、既定は日本語の並びで出る
    #[test]
    fn 日本語では日本語の日付が出る() {
        let f = date_formats();
        let label = |k: &str| f.iter().find(|(key, _, _)| *key == k).unwrap().1.clone();
        assert!(label("長い日付").ends_with("2026年8月6日"), "{}", label("長い日付"));
        assert!(label("曜日だけ").ends_with("木曜日"), "{}", label("曜日だけ"));
    }

}

#[cfg(test)]
mod fnhelp_tests {
    use crate::*;

    /// **分類の綴りが3箇所で揃っていること。**
    ///
    /// 分類の名前は `FN_GROUPS`(耳の並び)・`funcs.rs` の `group`(絞り込みの
    /// 照合)・`fn_group_cmd`(族の一覧を開くコマンド)の3箇所で使う。
    /// どれか1つの綴りがずれても**画面は出る** — 耳を押しても何も絞られない、
    /// あるいは黙って別の一覧が開くだけで、誰も落ちない。
    /// 2026-08-11 に「日付」を「日付・時刻」へ広げたとき、この形で
    /// `picks.rs` だけが取り残されていた。
    #[test]
    fn 関数の分類の綴りが揃っている() {
        use std::collections::BTreeSet;
        let 耳: BTreeSet<&str> = FN_GROUPS.iter().skip(1).copied().collect();
        let 表: BTreeSet<&str> = crate::funcs::FUNCS.iter().map(|f| f.group).collect();
        assert_eq!(耳, 表, "耳の並びと funcs.rs の分類が食い違っています");

        // 族の一覧を開くコマンド。**既定に落ちてよいのはこの2つだけ**
        let 既定でよい = ["検索/行列", "情報"];
        for g in &耳 {
            let id = util::fn_group_cmd(g);
            if 既定でよい.contains(g) {
                assert_eq!(id, "fn-lookup", "{g}");
            } else {
                assert_ne!(
                    id, "fn-lookup",
                    "{g} が既定に落ちています — fn_group_cmd の綴りがずれていませんか"
                );
            }
        }
    }

    /// **どの言語でも、分類の耳が9つとも別の語になること。**
    /// 同じ語が2つ並ぶと、押す人には区別がつかない
    #[test]
    fn 分類の耳が重ならない() {
        let mut seen = std::collections::HashMap::new();
        for g in FN_GROUPS {
            let label = util::fn_group_label(g);
            if let Some(prev) = seen.insert(label, *g) {
                panic!("{prev} と {g} が同じ語 {label:?} で並びます");
            }
        }
    }

    /// **どの言語の関数の言葉も、素の表と1対1で揃っていること。**
    ///
    /// 引き当ては名前の二分探索なので、並びが名前順でなければ**静かに
    /// 別の関数の説明が出る**(落ちない・警告も出ない)。数と並びの
    /// 両方をここで見る。
    #[test]
    fn 関数の言葉がどの言語も揃っている() {
        let 素: Vec<&str> = crate::funcs::FUNCS.iter().map(|f| f.name).collect();
        assert!(素.windows(2).all(|w| w[0] < w[1]), "素の表が名前順に並んでいません");
        let mut 見た = 0;
        for lang in ui::languages() {
            let Some(t) = crate::funcs_tables::text(lang) else {
                // ja は素の表そのものなので登録簿に無い
                assert_eq!(lang, "ja", "{lang} の関数の言葉が登録されていません");
                continue;
            };
            見た += 1;
            let names: Vec<&str> = t.iter().map(|r| r.name).collect();
            assert_eq!(names, 素, "{lang}: 関数の並びが素の表と違います");
            for r in t {
                assert!(!r.desc.is_empty(), "{lang}: {} の説明が空です", r.name);
                assert!(r.args.starts_with('('), "{lang}: {} の引数が変です: {}", r.name, r.args);
            }
        }
        assert!(見た >= 14, "言語が減っています({見た} 件)");
    }

    /// **説明がその言語で書かれていること。** 穴を日本語で埋めると
    /// 「英語で開いたのに1行だけ日本語」になる。仮名が混じっていたら落とす
    /// (中国語・韓国語は漢字を使うので、仮名だけを見る)
    #[test]
    fn 関数の説明に日本語が残っていない() {
        let かな = |s: &str| s.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c));
        for lang in ui::languages() {
            if lang == "ja" {
                continue;
            }
            let Some(t) = crate::funcs_tables::text(lang) else { continue };
            let 残り: Vec<&str> = t
                .iter()
                .filter(|r| かな(r.desc) || かな(r.args))
                .map(|r| r.name)
                .collect();
            assert!(残り.is_empty(), "{lang}: 日本語のまま残っている関数 {残り:?}");
        }
    }
}

#[cfg(test)]
mod prompt_tests {

    /// **パスワード欄が落ちない。**
    ///
    /// 伏せ字は `●`(3バイト)、キャレットの位置は打った字への**バイト**位置。
    /// そのまま `String::insert` すると文字の途中を割って Rust が落ちる。
    /// 1文字打っただけで calc が落ちていた(2026-08-12)— 3の倍数のときだけ
    /// 偶然通るので、試しに何文字か打った人だけが踏む。
    #[test]
    fn 伏せ字にキャレットを差し込んでも落ちない() {
        // view.rs のパスワード欄と同じ式。**文字数で置く**
        let caret = |raw: &str, cursor: usize, mask: bool| -> String {
            let before = raw[..cursor.min(raw.len())].chars().count();
            let mut text =
                if mask { "●".repeat(raw.chars().count()) } else { raw.to_string() };
            let at = text.char_indices().nth(before).map_or(text.len(), |(i, _)| i);
            text.insert(at, '|');
            text
        };
        for n in 0..8 {
            let raw = "a".repeat(n);
            assert_eq!(caret(&raw, n, true), format!("{}|{}", "●".repeat(n), ""), "{n} 文字");
        }
        // 途中にキャレットがあるとき
        assert_eq!(caret("abcd", 2, true), "●●|●●");
        // 伏せない欄は素の字のまま(日本語でも割らない)
        assert_eq!(caret("あい", 3, false), "あ|い");
    }
}

#[cfg(test)]
mod shape_nudge_tests {
    use crate::*;

    fn 図形を1つ置く(this: &mut Calc) -> (f32, f32) {
        this.sheet_mut().shapes_new.push(sheet::model::SheetShape {
            at: Pos::new(5, 3),
            width_px: 80.0,
            height_px: 40.0,
            kind: "rect".into(),
            ..Default::default()
        });
        this.shape_sel = Some(0);
        this.cell_origin_px(Pos::new(5, 3)).unwrap()
    }

    /// **Shift を押すと縦横の比を保つ。** 押していなければ自由
    #[gpui::test]
    fn shift_を押した大きさ変更は比を保つ(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _| {
            let (ox, oy) = 図形を1つ置く(this);
            // 右下の掴み。比は 40/80 = 0.5
            this.shape_drag = Some((0, (ox + 80.0, oy + 40.0), (ox, oy), true));
            this.shape_drag_at(ox + 200.0, oy + 41.0, true);
            let sp = &this.sheet().shapes_new[0];
            assert_eq!(sp.width_px, 200.0);
            assert_eq!(sp.height_px, 100.0, "比 0.5 が保たれていない");

            // Shift 無しなら縦は掴んだ位置のまま
            this.shape_drag_at(ox + 200.0, oy + 41.0, false);
            let sp = &this.sheet().shapes_new[0];
            assert_eq!(sp.height_px, 41.0, "Shift 無しで比を保っている");
        });
    }

    /// **Shift を押した移動は横か縦だけ。** 動かした量の大きいほうへ
    #[gpui::test]
    fn shift_を押した移動は縦横のどちらかに縛られる(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _| {
            let (ox, oy) = 図形を1つ置く(this);
            let 位置 = |this: &Calc| {
                let sp = &this.sheet().shapes_new[0];
                let (x, y) = this.cell_origin_px(sp.at).unwrap();
                (x + sp.dx_px, y + sp.dy_px)
            };
            // 横に大きく、縦に少し → 縦は動かない
            this.shape_drag = Some((0, (ox, oy), (ox, oy), false));
            this.shape_drag_at(ox + 60.0, oy + 5.0, true);
            let (_, y) = 位置(this);
            assert!((y - oy).abs() < 0.6, "縦に動いています({} → {})", oy, y);
        });
    }

    /// **図形を選んでいる間だけ Ctrl+矢印を奪う**(2026-08-13 発注者)。
    /// 選んでいなければ従来どおり「データの端へ」でカーソルが動く
    #[gpui::test]
    fn ctrl矢印は図形を選んでいる間だけ図形を動かす(cx: &mut gpui::TestAppContext) {
        let c = cx.update(|cx| cx.new(|cx| Calc::new(None, cx)));
        c.update(cx, |this, _| {
            let (ox, oy) = 図形を1つ置く(this);
            let 位置 = |this: &Calc| {
                let sp = &this.sheet().shapes_new[0];
                let (x, y) = this.cell_origin_px(sp.at).unwrap();
                (x + sp.dx_px, y + sp.dy_px)
            };
            assert!(this.nudge_shape(1.0, 0.0), "選んでいるのに動かない");
            let (x, _) = 位置(this);
            assert!((x - (ox + 1.0)).abs() < 0.6, "1px 動いていない({ox} → {x})");
            assert!(this.nudge_shape(0.0, 1.0));
            let (_, y) = 位置(this);
            assert!((y - (oy + 1.0)).abs() < 0.6, "縦に 1px 動いていない");

            // **選んでいなければ奪わない。** ここが false でないと、表の
            // 「データの端へ」が図形を置いた瞬間から使えなくなる
            this.shape_sel = None;
            assert!(!this.nudge_shape(1.0, 0.0), "選んでいないのに奪っています");
        });
    }
}
