//! writer の画面(main.rs から純移動 2026-08-08。部屋割りの2歩目)。
//! impl Render(紙面の描画・リボン・パネル)と InputSink(入力とマウスの受け皿)。
//! **純移動** — 挙動と文言は一切変えない

use crate::*;

/// 段の箱の鍵(`&'static str` が要るので表で持つ。calc と同じ綴り)
/// カーソルの上端(文字の大きさに対する割合。ベースラインからの上向き)と高さ。
/// **画面で測って決めた値です**(2026-08-17)。gpui が文字を箱の中で下寄りに
/// 置くぶんを含んでいます。文字の描画位置を変えたら、ここも測り直してください。
const CARET_TOP: f32 = 0.53;
const CARET_H: f32 = 1.00;

const TAB_IDS: &[&str] = &[
    "@tab0", "@tab1", "@tab2", "@tab3", "@tab4", "@tab5",
    "@tab6", "@tab7", "@tab8", "@tab9", "@tab10", "@tab11", "@tab12",
];

impl Render for Writer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me: Entity<Writer> = cx.entity();
        // 画面の倍率(紙のミリは変えず、画素への写像だけ変える)
        let pxmm = PX_PER_MM * self.zoom;
        // 編集領域の高さを実測しておく(キャレット追従・スクロールの止めに使う)。
        // リボンのぶん(約110px)を引いた近似で足りる
        self.view_h_px = (f32::from(window.viewport_size().height) - 136.0).max(100.0);
        self.view_w_px = f32::from(window.viewport_size().width).max(200.0);
        self.win_wh.set((
            f32::from(window.viewport_size().width),
            f32::from(window.viewport_size().height),
        ));
        // **点検の道具へ、ボタンの場所を渡す。** 環境変数が無ければ何もしない
        self.dump_ui();
        let marked = self.ed.marked_range();
        let (cx_mm, cy_mm, caret_pt) = self.caret_xy();

        // ---- リボン(Euro-Office に名前と並びを合わせる) ----
        // **タブの行そのものが窓の取っ手**(掴んで移動・二度押しで最大化)。
        // 空きの帯だけを取っ手にすると、タブが多い窓では幅がゼロになり
        // 掴む場所が無くなる(踏んで直した)。ボタンの類いは stop_propagation で
        // 取っ手より先に効く
        let (ready, all) = ribbon::progress(ribbon::writer_tabs());
        // ダークモードは**紙以外**を暗くする — 紙は白いまま(印刷と同じ)。
        // 文書は何も変わらない(見え方だけ)
        let dk = self.dark;
        let th_tab_on_bg = if dk { rgb(0x22262A) } else { rgb(0xFFFFFF) };
        let th_tab_on_fg = if dk { rgb(0xCFE0EA) } else { rgb(0x165E83) };
        let th_cmd_bg = if dk { rgb(0x22262A) } else { rgb(0xFFFFFF) };
        let th_cmd_border = if dk { rgb(0x33383D) } else { rgb(0xE1E6EA) };
        let th_btn = if dk { rgb(0x7FB2D0) } else { rgb(0x165E83) };
        let th_btn_hover = if dk { rgb(0x2C333A) } else { rgb(0xEAF2F7) };
        let th_gray_border = if dk { rgb(0x2E3338) } else { rgb(0xEDEFF1) };
        let th_gray_fg = if dk { rgb(0x565D64) } else { rgb(0xB6BDC4) };
        let th_status = if dk { rgb(0x9AA5AE) } else { rgb(0x66707A) };
        let th_desk = if dk { rgb(0x191C1F) } else { rgb(0x63686D) };
        // デスクトップ版の額縁: 1段目がクイックアクセス+文書名(=取っ手)、
        // 2段目が下線つきのタブ(現在地は青い下線)、3段目がボタンの帯
        let th_top_bg = if dk { rgb(0x1B1E21) } else { rgb(0xF1F3F5) };
        let th_top_fg = if dk { rgb(0xCFD6DC) } else { rgb(0x444B52) };
        let th_qa_hover = if dk { rgb(0x2C333A) } else { rgb(0xE2E6EA) };
        let qa = |id: &'static str, icon: &'static str| {
            div().id(id).px_2().py_1().rounded_sm().cursor_pointer()
                .hover(move |s| s.bg(th_qa_hover))
                .child(gpui::svg()
                    .path(SharedString::from(format!("icons/{icon}.svg")))
                    .size(px(15.0)).text_color(th_top_fg))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        };
        let title = self
            .path
            .as_ref()
            .and_then(|q| q.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| ui::t!("無題のドキュメント").into());
        let winbtn = |id: &'static str, label: &'static str| {
            div().id(id).px_2p5().py_1().rounded_sm()
                .text_size(px(12.0)).text_color(th_top_fg)
                .cursor_pointer()
                .hover(move |s| if id == "close" { s.bg(rgb(0xC0392B)).text_color(rgb(0xFFFFFF)) }
                                else { s.bg(rgb(0x2C7DA6)).text_color(rgb(0xFFFFFF)) })
                .child(label)
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        };
        let top = div().id("titlebar").flex().flex_row().items_center().gap_0p5()
            .px_2().py_0p5().bg(th_top_bg)
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                |_, e: &gpui::MouseDownEvent, window, _| {
                    if e.click_count >= 2 {
                        window.zoom_window();
                    } else {
                        window.start_window_move();
                    }
                }))
            .child(qa("qa-save", "save").on_click(cx.listener(|this, _, _, cx| {
                this.run_cmd("save", cx);
                cx.notify()
            })))
            .child(qa("qa-print", "print").on_click(cx.listener(|this, _, _, cx| {
                this.run_cmd("pdf", cx);
                cx.notify()
            })))
            .child(qa("qa-undo", "undo").on_click(cx.listener(|this, _, _, cx| {
                this.run_cmd("undo", cx);
                cx.notify()
            })))
            .child(qa("qa-redo", "redo").on_click(cx.listener(|this, _, _, cx| {
                this.run_cmd("redo", cx);
                cx.notify()
            })))
            .child(div().flex_1())
            .child(div().text_size(px(12.5)).text_color(th_top_fg)
                .whitespace_nowrap().overflow_hidden()
                .child(SharedString::from(format!(
                    "{}{title}",
                    if self.dirty { "*" } else { "" }
                ))))
            .child(div().flex_1())
            .child(div().pr_2().text_size(px(10.5))
                .text_color(if dk { rgb(0x6E7982) } else { rgb(0x8A949D) })
                .child(SharedString::from(ui::tf!("writer — 実装済み {}/{}", ready, all))))
            .child(winbtn("min", "─").on_click(cx.listener(|_, _, window, _| {
                window.minimize_window();
            })))
            .child(winbtn("max", "▢").on_click(cx.listener(|_, _, window, _| {
                window.zoom_window();
            })))
            .child(winbtn("close", "✕").on_click(cx.listener(|this, _, _, cx| {
                this.request_quit(cx);
            })));

        let th_tab_idle = if dk { rgb(0x9AA5AE) } else { rgb(0x555E66) };
        // 小窓(…)が開いている間はリボン全体 — タブの切替も — を無効にする。
        // 一覧(▾)は他を押せば閉じる作りなので対象外
        let dlg_open = self.dialog_open();
        // ボタンの印は3値: ▾=一覧が開く / …=小窓が開く / 無印=すぐ効く。
        // id からの導出(リボンの表は触らない)
        let marker_of = |id: &'static str| -> Option<&'static str> {
            if Writer::MENU_IDS.contains(&id) {
                Some("▾")
            } else if Writer::DIALOG_IDS.contains(&id) {
                Some("…")
            } else {
                None
            }
        };
        let mut tabs = div().flex().flex_row().items_end().gap_1()
            .px_2().bg(th_tab_on_bg);
        // **段は 15 段を同じ並びで出します**(2026-08-19 発注者「使わない
        // 場合には灰色にすればいいでしょう」)。文章に無い段(数式・データ・
        // ピボット・表のデザイン)は灰色で、押せません。並びが動かないので、
        // 表の画面と行き来しても段を探し直さずに済みます
        for (位置, 段) in ui::tabs::merged().into_iter().enumerate() {
            let 名 = 段.name;
            let Some(i) = 段.doc else {
                // この画面には無い段。**灰色で出す**(未実装の釦と同じ描き方)
                tabs = tabs.child(div()
                    .id(SharedString::from(format!("tab{位置}")))
                    .px_2p5().pt_1p5()
                    .text_size(px(12.0))
                    .text_color(th_gray_fg)
                    .flex().flex_col().items_center().gap_1()
                    .child(名)
                    .child(div().h(px(2.0)).w_full()));
                continue;
            };
            let tb = &ribbon::writer_tabs()[i];
            let on = i == self.tab;
            tabs = tabs.child(div()
                .id(SharedString::from(format!("tab{i}")))
                .px_2p5().pt_1p5()
                .text_size(px(12.0))
                // 小窓中はタブも灰色・無反応(未実装のボタンと同じ描き方)
                .text_color(if dlg_open { th_gray_fg }
                    else if on { th_tab_on_fg } else { th_tab_idle })
                .font_weight(if on { gpui::FontWeight::BOLD } else { gpui::FontWeight::NORMAL })
                .when(!dlg_open, |d| d.cursor_pointer()
                    .hover(move |s| s.text_color(th_tab_on_fg))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // タブ切替でも開いている一覧は畳む(他を押したら閉じる)
                        this.close_menus();
                        if i == 0 && this.tab != 0 {
                            this.prev_tab = this.tab;
                            this.file_view = 0;
                            this.file_field = None;
                        }
                        this.tab = i;
                        cx.notify()
                    })))
                .flex().flex_col().items_center().gap_1()
                // **段の箱も控える**(id は calc と同じ `@tabN`)。
                // 点検の道具が段を名前で切り替えられるように
                .relative()
                .child({
                    let rec = self.btn_box.clone();
                    let key: &'static str = TAB_IDS[i.min(TAB_IDS.len() - 1)];
                    gpui::canvas(
                        move |b: gpui::Bounds<gpui::Pixels>, _, _| {
                            rec.borrow_mut().insert(key, (
                                f32::from(b.origin.x),
                                f32::from(b.origin.y),
                                f32::from(b.size.width),
                                f32::from(b.size.height),
                            ));
                        },
                        |_, _: (), _, _| {},
                    )
                    .absolute()
                    .size_full()
                })
                .child(tb.name)
                // 現在地の青い下線(デスクトップ版の形)
                .child(div().h(px(2.5)).w_full().rounded_sm()
                    .bg(if on { th_btn } else { th_tab_on_bg })));
        }
        tabs = tabs.child(div().flex_1())
            .child(div().id("tab-find").px_2().pb_1().text_size(px(12.0))
                .text_color(th_tab_idle)
                .when(!dlg_open, |d| d.cursor_pointer()
                    .hover(move |s| s.text_color(th_tab_on_fg))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.run_from_ribbon("replace", cx);
                        cx.notify()
                    })))
                .child("🔍"));

        let mut cmds = div().flex().flex_col().gap_0p5()
            .px_3().py_1().bg(th_cmd_bg)
            .border_b_1().border_color(th_cmd_border);
        // 本家風のタブ配置。(id, 大ボタンの名札)。"‖" は群の区切り線。
        // 名札つきは絵の下に短い名前(本家の言い方)、無印は絵だけのボタン。
        // ボタンの名前は乗ったときに下のステータスバーへ出す(hover_hint)
        type LItem = (&'static str, Option<&'static str>);
        // ホームは2段(発注者の画像 2026-08-04)
        const HOME_ROWS: &[&[LItem]] = &[
            &[
                ("copy", None), ("cut", None), ("‖", None), ("fontname", None),
                ("fontsize", None), ("incfont", None), ("decfont", None),
                ("changecase", None), ("ruby", None),
                // **ふりがな**(AI)はルビの隣。AI タブを廃した 2026-08-15 に
                // ここへ移した — 会話では代われない仕事(入るのがルビの書式)
                ("ai-furigana", None), ("‖", None),
                ("markers", None),
                ("numbering", None), ("multilevels", None), ("decoffset", None),
                ("incoffset", None), ("linespace", None), ("direction", None),
                ("‖", None), ("parastyle", None),
            ],
            &[
                ("paste", None), ("selectall", None), ("‖", None), ("bold", None),
                ("italic", None), ("underline", None), ("strikeout", None),
                ("superscript", None), ("subscript", None), ("highlight", None),
                ("fontcolor", None), ("clearstyle", None), ("‖", None),
                ("align-left", None), ("align-center", None),
                ("align-right", None), ("align-just", None),
                ("align-dist", None),
                ("hidenchars", None), ("paracolor", None), ("borders", None),
                ("‖", None), ("replace", None),
            ],
        ];
        // 挿入は一段(発注者の画像 2026-08-04)。主要なボタンは名札つきの大ボタン
        const INS_ROWS: &[&[LItem]] = &[&[
            ("blankpage", Some("空白ページ")), ("pagebreak", Some("区切り")),
            ("‖", None), ("instable", Some("表")), ("‖", None),
            ("insimage", Some("画像")), ("insshape", Some("図形")),
            ("inssmartart", None), ("inschart", None), ("smartpicker", None),
            ("‖", None), ("instext", None), ("instextart", None),
            ("dropcap", None), ("text-from-file", None), ("‖", None),
            ("edit-header", None), ("edit-footer", None), ("pagenum", None),
            ("numpages", None), ("datetime", None), ("‖", None),
            ("insequation", None), ("inssymbol", None), ("‖", None),
            ("controls", None),
        ]];
        // 残りのタブも一段(本家 Web 版の並びから起こした。2026-08-04 発注者)
        const DRAW_ROWS: &[&[LItem]] = &[&[
            ("pen", Some("ペン")), ("highlighter", Some("蛍光ペン")),
            ("eraser", Some("消しゴム")),
        ]];
        const LAYOUT_ROWS: &[&[LItem]] = &[&[
            ("pagemargins", Some("余白")), ("pageorient", Some("向き")),
            ("pagesize", Some("サイズ")), ("columns", Some("段組み")),
            ("‖", None), ("line-numbers", None), ("hyphenation", None),
            ("‖", None), ("watermark", None), ("pagecolor", None),
            ("‖", None), ("colorschemas", None),
        ]];
        const REF_ROWS: &[&[LItem]] = &[&[
            ("toc", Some("目次")), ("toc-update", None), ("add-text", None),
            ("‖", None), ("bookmarks", None), ("caption", None),
            ("crossref", None), ("footnote", None),
            ("‖", None), ("tof", None), ("tof-update", None),
        ]];
        const FORM_ROWS: &[&[LItem]] = &[&[
            ("form-text", None), ("form-combo", None), ("form-dropdown", None),
            ("form-checkbox", None), ("form-radio", None), ("form-image", None),
            ("form-email", None), ("form-phone", None), ("form-complex", None),
            ("form-signature", None), ("‖", None), ("form-name", Some("名前")),
        ]];
        const COLLAB_ROWS: &[&[LItem]] = &[&[
            ("coauth-mode", Some("共同編集モード")), ("‖", None),
            ("co-addcomment", Some("コメント")), ("co-delcomment", None),
            ("co-showcomment", None), ("‖", None), ("co-chat", Some("チャット")),
            ("‖", None), ("track-changes", Some("変更履歴")), ("‖", None),
            ("co-history", Some("バージョン履歴")),
        ]];
        // **暗号化を掛けるボタンは置きません**(2026-08-18 発注者「暗号化は、
        // 開くだけ残す」)。writer が保存するのは adoc で、zip ではないので
        // 包めません。パスワード付きの docx を開く道は残っています
        const PROT_ROWS: &[&[LItem]] = &[&[
            ("prot-sign", Some("署名")), ("prot-doc", Some("保護")),
        ]];
        // 表示は本家どおり2段(発注者の画像 2026-08-04)
        const VIEW_ROWS: &[&[LItem]] = &[
            &[
                ("nav", Some("ナビゲーション")), ("‖", None),
                ("fit-page", Some("ページに合わせる")),
                ("zoom100", Some("100%に拡大する")), ("zoom-in", None),
                ("‖", None), ("darkmode", None), ("ruler", None),
                ("‖", None), ("show-toolbar", None), ("show-left", None),
            ],
            &[
                ("‖", None),
                ("fit-width", Some("幅に合わせる")),
                ("multipage", Some("複数ページ")), ("printview", None),
                ("zoom-out", None),
                ("‖", None), ("‖", None),
                ("‖", None), ("show-statusbar", None), ("show-right", None),
            ],
        ];
        // **マクロの段**(2026-08-16 に「プラグイン」から改名)。
        // 「一覧」は置き場の .py、「ファイルから」は置き場の外の .py。
        // **マクロを書く**(AI)もここ — 置き場に .py を置く仕事で、
        // 会話では代われない(2026-08-15、AI タブの廃止で移した)
        const PLUG_ROWS: &[&[LItem]] = &[&[
            ("plug-manage", Some("一覧")),
            ("ai-macro", Some("マクロを書く")),
        ]];
        let rows: Option<&[&[LItem]]> = match ribbon::WRITER[self.tab].name {
            "ホーム" => Some(HOME_ROWS),
            "挿入" => Some(INS_ROWS),
            "描画" => Some(DRAW_ROWS),
            "レイアウト" => Some(LAYOUT_ROWS),
            "参考資料" => Some(REF_ROWS),
            "フォーム" => Some(FORM_ROWS),
            "共同編集" => Some(COLLAB_ROWS),
            "保護" => Some(PROT_ROWS),
            "表示" => Some(VIEW_ROWS),
            "マクロ" => Some(PLUG_ROWS),
            _ => None,
        };
        if let Some(rows) = rows {
            let size_now = self.doc.size_at(self.ed.selection()).unwrap_or(SIZE_PT);
            let size_disp = if size_now.fract() == 0.0 {
                format!("{}", size_now as i32)
            } else {
                format!("{size_now}")
            };
            for ids in rows {
                let tall = ids.iter().any(|(_, b)| b.is_some());
                let mut row = div().flex().flex_row().items_center().gap_0p5();
                for &(id, big) in *ids {
                    if id == "‖" {
                        row = row.child(div().w(px(1.0))
                            .h(px(if tall { 40.0 } else { 22.0 }))
                            .bg(th_cmd_border).mx_1());
                        continue;
                    }
                    // コンボ風(フォント名と大きさは今の値を見せる)
                    if id == "fontname" || id == "fontsize" {
                        let cid = id;
                        let text = if cid == "fontname" {
                            self.font_name.to_string()
                        } else {
                            size_disp.clone()
                        };
                        row = row.child(div()
                            .id(SharedString::from(format!("h-{cid}")))
                            .flex().flex_row().items_center().gap_1()
                            .px_2().h(px(26.0))
                            .w(px(if cid == "fontname" { 150.0 } else { 56.0 }))
                            .rounded_sm().border_1().border_color(th_cmd_border)
                            .text_size(px(12.0))
                            // 小窓中は欄も灰色・無反応(リボン全体を無効にする約束)
                            .text_color(if dlg_open { th_gray_fg } else { th_top_fg })
                            .when(!dlg_open, |d| d.cursor_pointer()
                                .hover(move |st| st.bg(th_btn_hover))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run_from_ribbon(cid, cx);
                                    cx.notify()
                                })))
                            .child(div().flex_1().whitespace_nowrap()
                                .overflow_hidden().child(SharedString::from(text)))
                            .child(div().text_size(px(9.0)).text_color(th_tab_idle)
                                .child("▼")));
                        continue;
                    }
                    let Some(cmd) = ribbon::writer_tabs()[self.tab]
                        .cmds
                        .iter()
                        .find(|c| c.id == id || (!c.ready && c.icon == id))
                        .copied()
                    else {
                        continue;
                    };
                    let label = cmd.label;
                    // 名札の短い形は ja 向け — 他の言語では表の語を使う
                    let big = if ui::settings::language() == "ja" {
                        big
                    } else {
                        big.map(|_| cmd.label)
                    };
                    let icon = cmd.icon;
                    let hoverable = cx.listener(move |this: &mut Writer, on: &bool, _, cx| {
                        if *on {
                            this.hover_hint = Some(label);
                        } else if this.hover_hint == Some(label) {
                            this.hover_hint = None;
                        }
                        cx.notify()
                    });
                    let has_icon = ui::icons::find(icon).is_some();
                    // 開く印(▾=一覧 / …=小窓)。無印はすぐ効くボタン
                    let marker = marker_of(cmd.id);
                    // **押せるボタンは自分の場所を控える**(実機の点検のため。
                    // calc の btn_box と同じ形)。描くたびに上書きする
                    let mark = {
                        let rec = self.btn_box.clone();
                        let cid = cmd.id;
                        gpui::canvas(
                            move |b: gpui::Bounds<gpui::Pixels>, _, _| {
                                rec.borrow_mut().insert(cid, (
                                    f32::from(b.origin.x),
                                    f32::from(b.origin.y),
                                    f32::from(b.size.width),
                                    f32::from(b.size.height),
                                ));
                            },
                            |_, _: (), _, _| {},
                        )
                        .absolute()
                        .size_full()
                    };
                    if let Some(short) = big {
                        // 名札つきの大ボタン(絵の下に短い名前。本家の言い方)。
                        // 開くボタンは名札の横に小さな印。小窓中は灰色・無反応
                        let on = cmd.ready && self.toggled(cmd.id);
                        let fg = if !cmd.ready || dlg_open {
                            th_gray_fg
                        } else if on {
                            th_btn
                        } else {
                            th_top_fg
                        };
                        let mut b = div()
                            .id(SharedString::from(format!("h-{icon}")))
                            .px_2().h(px(48.0)).rounded_sm()
                            .flex().flex_col().items_center().justify_center()
                            .gap_1()
                            .on_hover(hoverable)
                            // **マウスを置いたらすぐ説明を出す**(2026-08-17
                            // 発注者)。下のステータスバーにも名前は出ますが、
                            // マウスから遠くて気づきません。gpui の既定の
                            // 待ち時間は 500 ミリ秒で、待たされる感じがあります
                            .tooltip(move |_, cx| cx.new(|_| Tip(label.into())).into())
                            .tooltip_show_delay(std::time::Duration::from_millis(150))
                            .children(has_icon.then(|| {
                                gpui::svg()
                                    .path(SharedString::from(format!("icons/{icon}.svg")))
                                    .size(px(20.0))
                                    .text_color(fg)
                            }))
                            .child(div().flex().flex_row().items_center().gap_0p5()
                                .text_size(px(10.5)).text_color(fg)
                                .child(short)
                                .children(marker.map(|m| div()
                                    .text_size(px(8.0)).text_color(th_tab_idle)
                                    .child(m))));
                        if on {
                            b = b.bg(th_btn_hover).border_1().border_color(th_btn);
                        }
                        if cmd.ready && !dlg_open {
                            let cid = cmd.id;
                            b = b.relative().child(mark).cursor_pointer()
                                .hover(move |st| st.bg(th_btn_hover))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run_from_ribbon(cid, cx);
                                    cx.notify()
                                }));
                        }
                        row = row.child(b);
                        continue;
                    }
                    let on = cmd.ready && self.toggled(cmd.id);
                    // 小窓中は ready でも灰色・無反応(未実装と同じ描き方)
                    let usable = cmd.ready && !dlg_open;
                    let mut b = div()
                        .id(SharedString::from(format!("h-{icon}")))
                        .h(px(26.0)).rounded_sm()
                        .flex().items_center().justify_center()
                        .on_hover(hoverable)
                        .tooltip(move |_, cx| cx.new(|_| Tip(label.into())).into())
                        .tooltip_show_delay(std::time::Duration::from_millis(150));
                    b = if has_icon {
                        // 印つきは幅を固定しない(印のぶん広がる)
                        if marker.is_some() { b.px_0p5() } else { b.w(px(26.0)) }
                    } else {
                        b.px_1p5()
                    };
                    if on {
                        // 入っている印(押した結果が画面に残るもの)
                        b = b.bg(th_btn_hover).border_1().border_color(th_btn);
                    }
                    let icon_fg = if !usable {
                        th_gray_fg
                    } else if on {
                        th_btn
                    } else {
                        th_top_fg
                    };
                    b = b
                        .children(has_icon.then(|| {
                            gpui::svg()
                                .path(SharedString::from(format!("icons/{icon}.svg")))
                                .size(px(18.0))
                                .text_color(icon_fg)
                        }))
                        .children(has_icon.then_some(marker).flatten().map(|m| {
                            // 開く印(▾=一覧 / …=小窓)
                            div().text_size(px(8.0)).text_color(th_tab_idle).child(m)
                        }))
                        .children((!has_icon).then(|| {
                            div().text_size(px(10.5))
                                .text_color(if usable { th_btn } else { th_gray_fg })
                                .flex().flex_row().items_center().gap_0p5()
                                .child(label)
                                .children(marker.map(|m| div()
                                    .text_size(px(8.0)).text_color(th_tab_idle)
                                    .child(m)))
                        }));
                    if usable {
                        let cid = cmd.id;
                        b = b.relative().child(mark).cursor_pointer()
                            .hover(move |st| st.bg(th_btn_hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.run_from_ribbon(cid, cx);
                                cx.notify()
                            }));
                    }
                    row = row.child(b);
                }
                cmds = cmds.child(row);
            }
        } else {
            let mut row = div().flex().flex_row().flex_wrap().gap_1().items_center().py_1();
            for cmd in ribbon::writer_tabs()[self.tab].cmds {
                // 小窓中は ready でも灰色・無反応(未実装と同じ描き方)
                if cmd.ready && !dlg_open {
                    let id = cmd.id;
                    row = row.child(div()
                        .id(SharedString::from(cmd.id))
                        .px_3().py_1().rounded_md()
                        .border_1().border_color(th_btn).text_color(th_btn)
                        .text_size(px(12.0)).cursor_pointer()
                        .hover(move |s| s.bg(th_btn_hover))
                        .flex().flex_row().items_center().gap_1()
                        .children(ui::icons::find(cmd.icon).map(|_| {
                            gpui::svg()
                                .path(SharedString::from(format!("icons/{}.svg", cmd.icon)))
                                .size(px(15.0))
                                .text_color(th_btn)
                        }))
                        .child(cmd.label)
                        // 開く印(▾=一覧 / …=小窓)
                        .children(marker_of(cmd.id).map(|m| div()
                            .text_size(px(9.0)).text_color(th_tab_idle)
                            .child(m)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run_from_ribbon(id, cx); cx.notify()
                        })));
                } else {
                    // 未実装(と小窓中)。押せるように見せない
                    row = row.child(div().px_3().py_1().rounded_md()
                        .border_1().border_color(th_gray_border)
                        .text_color(th_gray_fg).text_size(px(12.0))
                        .flex().flex_row().items_center().gap_1()
                        .children(ui::icons::find(cmd.icon).map(|_| {
                            gpui::svg()
                                .path(SharedString::from(format!("icons/{}.svg", cmd.icon)))
                                .size(px(15.0))
                                .text_color(th_gray_fg)
                        }))
                        .child(cmd.label)
                        .children(marker_of(cmd.id).map(|m| div()
                            .text_size(px(9.0)).text_color(th_tab_idle)
                            .child(m))));
                }
            }
            cmds = cmds.child(row);
        }
        let bar = if self.tab == 0 || !self.show_toolbar {
            // ファイルのページ(本家の File メニュー)と、畳んだツールバーは
            // ボタンの帯を持たない(タブは残る — 押せば中身へ行ける)
            div().flex().flex_col().child(top).child(tabs)
        } else {
            div().flex().flex_col().child(top).child(tabs).child(cmds)
        };

        // ---- ファイルのタブ(何枚も開いているとき) ----
        //
        // **Zed と同じように何枚も開いて行き来します**(2026-08-19 発注者)。
        // 1枚しか開いていないときは出しません
        let files_bar = (self.file_count() > 1).then(|| {
            let mut bar = div().flex().flex_row().items_center().gap_1()
                .px_2().py_0p5().bg(th_top_bg)
                .border_b_1().border_color(th_cmd_border);
            for i in 0..self.file_count() {
                let on = i == self.file_at;
                let 書きかけ = self.file_dirty(i);
                let mut 札 = self.file_name(i);
                if 書きかけ {
                    // **書きかけの印。** 閉じる前に気づけるように
                    札.push('*');
                }
                bar = bar.child(div()
                    .id(SharedString::from(format!("file{i}")))
                    .px_2p5().py_0p5().rounded_sm().cursor_pointer()
                    .bg(if on { rgb(0xFFFFFF) } else { gpui::transparent_black().into() })
                    .border_1().border_color(if on { th_cmd_border } else { gpui::transparent_black().into() })
                    .text_size(px(11.5))
                    .text_color(if on { th_top_fg } else { th_status })
                    .child(SharedString::from(札))
                    .on_click(cx.listener(move |t, _, _, cx| { t.show_file(i); cx.notify() })));
                // 閉じる(×)。書きかけがあるときは断ります
                bar = bar.child(div()
                    .id(SharedString::from(format!("filex{i}")))
                    .px_1().rounded_sm().cursor_pointer()
                    .text_size(px(11.0)).text_color(th_status)
                    .hover(move |s| s.bg(th_qa_hover))
                    .child("×")
                    .on_click(cx.listener(move |t, _, _, cx| { t.close_file(i); cx.notify() })));
            }
            bar
        });

        // ---- 文書の耳(1つのファイルに何枚も入っているとき) ----
        //
        // **calc のシートの耳と同じ位置・同じ動き**にしてあります
        // (2026-08-19)。表の画面と文章の画面で、下の耳の意味が揃います。
        // 1枚しか入っていないときは出しません — 何も選べない耳は邪魔です
        let docs_bar = (self.doc_count() > 1).then(|| {
            let mut bar = div().flex().flex_row().items_center().gap_1()
                .px_3().py_1().bg(rgb(0xF1F3F5))
                .border_t_1().border_color(rgb(0xD5DBE0));
            for i in 0..self.doc_count() {
                let on = i == self.doc_at;
                bar = bar.child(div()
                    .id(SharedString::from(format!("doc{i}")))
                    .px_3().py_1().rounded_sm().cursor_pointer()
                    .bg(if on { rgb(0xFFFFFF) } else { rgb(0xEFF2F4) })
                    .border_1().border_color(if on { rgb(0x1B6E3C) } else { rgb(0xD5DBE0) })
                    .text_size(px(11.5))
                    .text_color(if on { rgb(0x1B6E3C) } else { rgb(0x4A5560) })
                    .child(SharedString::from(self.doc_name(i)))
                    .on_click(cx.listener(move |t, _, _, cx| { t.show_doc(i); cx.notify() })));
            }
            bar
        });

        // ---- 下のステータスバー(デスクトップ版: ページ・文字数・ズーム) ----
        let total_pages = self.page_offsets.len().max(1);
        let cur_page = self
            .page_offsets
            .iter()
            .rposition(|o| self.scroll_mm >= *o - 0.01)
            .unwrap_or(0)
            + 1;
        let nchars = self
            .doc
            .body_text()
            .chars()
            .filter(|c| !c.is_whitespace())
            .count();
        let sb_btn = |id: &'static str, label: &'static str| {
            div().id(id).px_1p5().py_0p5().rounded_sm().cursor_pointer()
                .text_size(px(11.5)).text_color(th_top_fg)
                .hover(move |s| s.bg(th_qa_hover))
                .child(label)
        };
        let statusbar = div().flex().flex_row().items_center().gap_3()
            .px_3().py_0p5().bg(th_top_bg)
            .border_t_1().border_color(th_cmd_border)
            .text_size(px(11.0)).text_color(th_status)
            .child(SharedString::from(ui::tf!("{}/{} ページ", cur_page, total_pages)))
            .child(SharedString::from(ui::tf!("文字数 {}", nchars)))
            // **いまどちらの形式か。** 形式によって出来ることが違います
            // (ネイティブでは直接書式を封じてスタイルへ誘導します)。
            // それが画面のどこにも出ていませんでした(2026-08-17 発注者
            // 「そもそも、docx か .adoc かわからない」)。
            // 形式の名前はどの言語でも同じなので、翻訳の表は通しません
            .child(
                div()
                    .px_1p5()
                    .rounded_sm()
                    .bg(if self.native { rgb(0xE3F0F6) } else { th_cmd_bg })
                    .text_color(if self.native { rgb(0x165E83) } else { th_status })
                    .child(if self.native { "adoc" } else { "docx" }),
            )
            .child(div().flex_1().whitespace_nowrap().overflow_hidden()
                .child(SharedString::from(match self.hover_hint {
                    Some(h) => h.to_string(),
                    None => format!(
                        "{}{}",
                        if self.dirty { "● " } else { "" },
                        self.status
                    ),
                })))
            .child(sb_btn("sb-spell", ui::t!("スペル")).on_click(cx.listener(|this, _, _, cx| {
                this.run_cmd("spell", cx);
                cx.notify()
            })))
            .child(sb_btn("sb-zoom-out", "−").on_click(cx.listener(|this, _, _, cx| {
                this.run_cmd("zoom-out", cx);
                cx.notify()
            })))
            .child(div().id("sb-zoom").px_1().rounded_sm().cursor_pointer()
                .text_size(px(11.5)).text_color(th_top_fg)
                .hover(move |s| s.bg(th_qa_hover))
                .child(SharedString::from(ui::tf!("ズーム{}%", (self.zoom * 100.0).round() as i32)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.zoom = 1.0;
                    cx.notify()
                })))
            .child(sb_btn("sb-zoom-in", ui::t!("＋")).on_click(cx.listener(|this, _, _, cx| {
                this.run_cmd("zoom-in", cx);
                cx.notify()
            })));

        // ---- ファイルのページ(本家の File メニュー。タブ0で全面に出す) ----
        let filepage: Option<gpui::Div> = if self.tab != 0 {
            None
        } else {
            let item_bg = th_qa_hover;
            // **ファイルの面の項目も場所を控える**(2026-08-17。点検の道具が
            // 座標を当てずに押せるように。リボンのボタンと同じ形)
            let boxes = self.btn_box.clone();
            let mk = move |id: &'static str, label: &'static str, ready: bool| {
                let rec = boxes.clone();
                // **控えは最初の子に**(calc の mark と同じ形)。最後に置くと
                // 流れの中で label の下に置かれ、**1項目ぶん下の箱**を控えて
                // いた(2026-08-17 実機で踏んだ — 押すと1つ下が反応した)
                let d = div()
                    .id(id)
                    .relative()
                    .child(
                        gpui::canvas(
                            move |b: gpui::Bounds<gpui::Pixels>, _, _| {
                                rec.borrow_mut().insert(id, (
                                    f32::from(b.origin.x),
                                    f32::from(b.origin.y),
                                    f32::from(b.size.width),
                                    f32::from(b.size.height),
                                ));
                            },
                            |_, _: (), _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .px_4()
                    .py_1p5()
                    .text_size(px(13.0));
                if ready {
                    d.text_color(th_top_fg)
                        .cursor_pointer()
                        .hover(move |s| s.bg(item_bg))
                } else {
                    d.text_color(th_gray_fg)
                }
                .child(label)
            };
            let sb = div().w(px(280.0)).bg(th_top_bg)
                .border_r_1().border_color(th_cmd_border)
                .flex().flex_col().py_2()
                .child(mk("f-back", ui::t!("‹ 戻る"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.tab = this.prev_tab;
                        cx.notify()
                    })))
                .child(div().h(px(10.0)))
                .child(mk("f-new", ui::t!("新規作成"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        if this.new_doc() {
                            this.tab = this.prev_tab;
                        }
                        cx.notify()
                    })))
                .child(mk("f-tpl", ui::t!("テンプレートから作成"), false))
                .child(mk("f-open", ui::t!("開く"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.tab = this.prev_tab;
                        this.open_dialog(cx);
                        cx.notify()
                    })))
                .child(mk("f-url", ui::t!("URLを開く"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.tab = this.prev_tab;
                        this.url_open = true;
                        this.url_ed = Editor::new("http://127.0.0.1:8765/");
                        this.status =
                            ui::t!("URL を打って Enter(JS なしの閲覧と記入。http のみ)").into();
                        cx.notify()
                    })))
                .child(mk("f-recent", ui::t!("最近開いた"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.file_view = 1;
                        cx.notify()
                    })))
                // **フォルダから探す**(2026-08-17 発注者。SFIND の写真)。
                // 複数のファイルを串刺しで探し、選ぶと下に見え、
                // 下の「読み込み」で初めて開く
                .child(mk("f-find", ui::t!("フォルダから探す"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.file_view = 3;
                        cx.notify()
                    })))
                .child(div().h(px(10.0)))
                .child(mk("f-save", ui::t!("保存"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.save(false, cx);
                        cx.notify()
                    })))
                .child(mk("f-saveas", ui::t!("名前を付けて保存"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.save_as(cx);
                        cx.notify()
                    })))
                .child(mk("f-print", ui::t!("印刷"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.save_pdf(cx);
                        cx.notify()
                    })))
                .child(mk("f-merge", ui::t!("データを差し込む(CSV)"), true).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.merge_csv(cx);
                        cx.notify()
                    })))
                .child(mk("f-html", ui::t!("Web の形で書き出す(HTML)"), true).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.save_html(cx);
                        cx.notify()
                    })))
                .child(mk("f-protect", ui::t!("保護する"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        if let Some(i) =
                            ribbon::WRITER.iter().position(|t| t.name == "保護")
                        {
                            this.tab = i;
                        }
                        cx.notify()
                    })))
                // **adoc 形式にする**(2026-08-16。SEKKEI 段階D)。受け取った docx を
                // 意味だけ+テンプレートに変える。**非可逆なので明示の1手** —
                // 開いただけでは何も起きない
                .child(mk("f-distill", ui::t!("adoc 形式にする(本文と書式を分ける)"), !self.native).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.tab = this.prev_tab;
                        this.distill_now();
                        cx.notify()
                    }),
                ))
                .child(div().h(px(10.0)))
                .child({
                    let d = mk("f-info", ui::t!("詳細情報"), true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.file_view = 0;
                            cx.notify()
                        }));
                    if self.file_view == 0 { d.bg(item_bg) } else { d }
                })
                .child(mk("f-place", ui::t!("ファイルの場所を開く"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        match this.path.as_ref().and_then(|p| p.parent()) {
                            Some(dir) => {
                                this.status = match ui::open_outside(&dir.display().to_string()) {
                                    ui::Opened::Yes => ui::tf!("開きます: {}",
                                        dir.display().to_string()).into(),
                                    ui::Opened::JustNow => ui::t!(
                                        "さっき開きました(窓が出るまで少し待ってください)").into(),
                                    ui::Opened::Failed => ui::tf!(
                                        "開けません(xdg-open がありません): {}",
                                        dir.display().to_string()).into(),
                                };
                            }
                            None => {
                                this.status = ui::t!("まだファイルになっていません").into();
                            }
                        }
                        cx.notify()
                    })))
                .child(div().h(px(10.0)))
                .child(mk("f-quit", ui::t!("終了"), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.request_quit(cx);
                        cx.notify()
                    })))
                .child(div().flex_1())
                .child({
                    let d = mk("f-opts", ui::t!("詳細設定"), true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.file_view = 2;
                            cx.notify()
                        }));
                    if self.file_view == 2 { d.bg(item_bg) } else { d }
                })
                .child(mk("f-help", ui::t!("ヘルプ"), false))
                .child(mk("f-req", ui::t!("機能のリクエスト"), false));

            let mut pane = div().flex_1().bg(th_cmd_bg).p_8()
                .flex().flex_col().gap_3().text_size(px(12.5))
                .text_color(th_top_fg);
            if self.file_view == 3 {
                // **フォルダから探す**(2026-08-17 発注者。SFIND の写真)。
                // 上に欄、真ん中に当たりの一覧、下に見せる窓と「読み込み」
                let 欄 = |this: &Writer, i: usize, ed: &Editor, w: f32, ph: &'static str| {
                    let mut s = ed.text().to_string();
                    if this.fd_field == i && this.file_view == 3 {
                        let c = ed.cursor().min(s.len());
                        s.insert(c, '|');
                    }
                    div().id(SharedString::from(format!("fd-{i}")))
                        .w(px(w)).px_2().py_1().rounded_sm().cursor_text()
                        .border_1()
                        .border_color(if this.fd_field == i { th_btn } else { th_cmd_border })
                        .bg(gpui::white())
                        .text_size(px(12.5)).whitespace_nowrap().overflow_hidden()
                        .child(SharedString::from(if s.is_empty() { ph.to_string() } else { s }))
                        .on_click(cx.listener(move |t, _, _, cx| { t.fd_field = i; cx.notify() }))
                };
                let 押し = |id: &'static str, 札: SharedString| {
                    div().id(id).px_3().py_1().rounded_sm().cursor_pointer()
                        .border_1().border_color(th_btn).text_color(th_btn)
                        .text_size(px(12.0))
                        .hover(move |s| s.bg(th_btn_hover))
                        .child(札)
                };
                pane = pane
                    .child(div().text_size(px(16.0)).font_weight(gpui::FontWeight::BOLD)
                        .child(ui::t!("フォルダから探す")))
                    .child(div().flex().flex_row().items_center().gap_2()
                        .child(欄(self, 0, &self.fd_term, 280.0, "探す字"))
                        .child(欄(self, 1, &self.fd_glob, 120.0, "*.txt"))
                        .child(押し("fd-dir", ui::t!("場所を選ぶ").into()).on_click(
                            cx.listener(|t, _, _, cx| { t.find_dir_dialog(cx); cx.notify() })))
                        .child(押し("fd-go", ui::t!("探す (Enter)").into()).on_click(
                            cx.listener(|t, _, _, cx| { t.find_in_folder(); cx.notify() }))))
                    .child(div().text_size(px(11.5)).text_color(th_status)
                        .child(SharedString::from(match self.find_dir() {
                            Some(d) => ui::tf!("場所: {}", d.display()).to_string(),
                            None => ui::t!("場所がまだ決まっていません(「場所を選ぶ」)").to_string(),
                        })));
                // 当たりの一覧(ファイルごとに見出し + 行番号つきの行)
                let mut 一覧 = div().id("fd-list")
                    .flex_none().h(px(320.0)).overflow_y_scroll()
                    .p_2().rounded_sm().bg(gpui::white())
                    .border_1().border_color(th_cmd_border)
                    .flex().flex_col().gap_0p5().text_size(px(12.0));
                if self.fd_hits.is_empty() {
                    一覧 = 一覧.child(div().text_color(th_status)
                        .child(ui::t!("(まだ探していません)")));
                }
                self.fd_box.borrow_mut().clear();
                for (fi, f) in self.fd_hits.iter().enumerate() {
                    一覧 = 一覧.child(div().mt_1().text_color(th_btn)
                        .child(SharedString::from(format!(
                            "{}   {}   {}",
                            f.path.file_name().unwrap_or_default().to_string_lossy(),
                            ui::search::human_size(f.size),
                            f.path.parent().map(|d| d.display().to_string()).unwrap_or_default()
                        ))));
                    for (hi, h) in f.hits.iter().enumerate() {
                        let on = self.fd_at == Some((fi, hi));
                        // 長い行は縮める(一覧が横に流れない)
                        let line: String = h.text.chars().take(120).collect();
                        let rec = self.fd_box.clone();
                        一覧 = 一覧.child(div()
                            .id(SharedString::from(format!("fd-h-{fi}-{hi}")))
                            .relative()
                            .child(gpui::canvas(
                                move |b: gpui::Bounds<gpui::Pixels>, _, _| {
                                    rec.borrow_mut().push((
                                        fi,
                                        hi,
                                        f32::from(b.origin.x),
                                        f32::from(b.origin.y),
                                        f32::from(b.size.width),
                                        f32::from(b.size.height),
                                    ));
                                },
                                |_, _: (), _, _| {},
                            ).absolute().size_full())
                            .px_1().rounded_sm().cursor_pointer()
                            .bg(if on { th_btn_hover } else { gpui::transparent_black().into() })
                            .hover(move |s| s.bg(th_btn_hover))
                            .whitespace_nowrap().overflow_hidden()
                            .child(SharedString::from(format!("{:05} {line}", h.line)))
                            .on_click(cx.listener(move |t, _, _, cx| {
                                t.find_peek(fi, hi);
                                cx.notify()
                            })));
                    }
                }
                pane = pane.child(一覧);
                // **下の窓と「読み込み」**(発注者 2026-08-17
                // 「下に読み込みボタンを置くのはどうか」)。見て、これだと
                // 分かってから開く — 押し間違いで文書が入れ替わらない
                pane = pane.child(div().flex().flex_row().items_center().gap_2()
                    .child(押し("fd-load", ui::t!("読み込み").into()).on_click(
                        cx.listener(|t, _, _, cx| { t.find_load(); cx.notify() })))
                    .child(div().text_size(px(11.5)).text_color(th_status)
                        .child(ui::t!("選んだ当たりの文書を開いて、その場所へ移ります"))));
                pane = pane.child(div().id("fd-peek")
                    .flex_1().min_h(px(120.0)).overflow_y_scroll()
                    .p_2().rounded_sm().bg(gpui::white())
                    .border_1().border_color(th_cmd_border)
                    .text_size(px(12.0)).font_family(crate::doc::MONO)
                    .child(SharedString::from(if self.fd_peek.is_empty() {
                        ui::t!("(当たりを選ぶと、ここに前後が出ます)").to_string()
                    } else {
                        self.fd_peek.clone()
                    })));
            } else if self.file_view == 2 {
                // 詳細設定 — 器は ~/.config/officework/settings.toml
                // (SEKKEI「設定 — 器と言語」。環境変数が一時上書きで優先)
                let lang_now = ui::settings::get("language").unwrap_or_else(|| "ja".into());
                let row = |label: &'static str, value: String| {
                    div().flex().flex_row().items_center().gap_2()
                        .child(div().w(px(200.0)).text_color(th_status).child(label))
                        .child(div().child(SharedString::from(value)))
                };
                pane = pane
                    .child(div().text_size(px(16.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(ui::t!("詳細設定")))
                    .child(div().text_color(th_status).child(SharedString::from(
                        ui::tf!("置き場: {}", ui::settings::path().display()))))
                    .child(div().h(px(6.0)))
                    .child(div().flex().flex_row().items_center().gap_2()
                        .child(div().w(px(200.0)).text_color(th_status)
                            .child(ui::t!("言語(リボンと文言)")))
                        .child(div().id("set-lang")
                            .px_3().py_1().rounded_sm().cursor_pointer()
                            .bg(item_bg)
                            // 札ではなく**その言語自身の名前**を出す。
                            // `pt` と `pt-br` は札のままでは見分けられない
                            .child(SharedString::from(
                                ui::language_label(&lang_now).to_string()))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let cur = ui::settings::get("language")
                                    .unwrap_or_else(|| "ja".into());
                                let all = ui::languages();
                                let i = all.iter().position(|l| **l == cur).unwrap_or(0);
                                let next = all[(i + 1) % all.len()];
                                ui::settings::set("language", next);
                                this.status = ui::t!("言語を控えました(次の起動から効きます。環境変数 OFFICE_LANG があればそちらが優先)").into();
                                cx.notify()
                            }))))
                    // ── AI ────────────────────────────────────────────
                    // **宛先を覚えるのはここ**(発注者 2026-08-15
                    // 「AI の設定を設定メニューに追加して」)。calc と同じ形
                    .child(div().h(px(10.0)))
                    .child(div().flex().flex_row().items_center().gap_2()
                        .child(div().w(px(200.0)).text_color(th_status)
                            .child(ui::t!("AI の宛先")))
                        .child(div().id("set-ai")
                            .px_3().py_1().rounded_sm().cursor_pointer().bg(item_bg)
                            .child(SharedString::from(ui::ai::backend().label().to_string()))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run_cmd("ai-where", cx);
                                cx.notify()
                            }))))
                    // **使えないなら理由を出す。** 鍵そのものは出さない。
                    // 手元のモデルだけは「使えます」と言わない(繋がるか
                    // 確かめずに言えば嘘になる)
                    .child(row(ui::t!("いま使えるか"), {
                        let b = ui::ai::backend();
                        match ui::ai::ready(b) {
                            _ if b == ui::ai::Backend::Local =>
                                ui::t!("頼んでみるまで分かりません(下の宛先へ繋ぎます)").to_string(),
                            Ok(()) => ui::t!("使えます").to_string(),
                            Err(e) => e,
                        }
                    }))
                    .child(row(ui::t!("AI のモデル(JO_AI_MODEL)"),
                        std::env::var("JO_AI_MODEL")
                            .unwrap_or_else(|_| ui::t!("(宛先の既定)").into())))
                    .child(div().h(px(10.0)))
                    .child(row(ui::t!("書体(OFFICE_FONT)"),
                        std::env::var("OFFICE_FONT")
                            .unwrap_or_else(|_| ui::t!("(文書に従う)").into())))
                    // **手元のモデルと校正の宛先。** 会のサーバーへ向けられる
                    // ので(2026-08-15)、外に出るかどうかもここに出す —
                    // 「外に出ない」は宛先を変えたら嘘になる
                    .child(row(ui::t!("手元のモデルの宛先"), {
                        let ep = ui::Endpoint::default();
                        format!(
                            "{}({})",
                            ep.shown(),
                            if ep.is_local() {
                                ui::t!("この機械の中だけ")
                            } else {
                                ui::t!("外へ出ます")
                            }
                        )
                    }))
                    .child(row(ui::t!("宛先の決め方"),
                        ui::t!("settings.toml の ai_url / ai_model(環境変数 OFFICE_URL が優先)")
                            .to_string()))
                    .child(row(ui::t!("Python の経路"),
                        std::env::var("JO_PYTHON")
                            .unwrap_or_else(|_| ui::t!("(自動: .venv → python3)").into())))
                    .child(row(ui::t!("名前(ロック・チャット・署名)"), lock_identity()));
            } else if self.file_view == 1 {
                pane = pane.child(div().text_size(px(16.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("最近開いた")));
                let list = Self::recent_list();
                if list.is_empty() {
                    pane = pane.child(div().text_color(th_status)
                        .child(ui::t!("(まだありません。開く・保存すると残ります)")));
                }
                for (i, q) in list.into_iter().enumerate() {
                    let name = q.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let dir = q.parent()
                        .map(|d| d.to_string_lossy().to_string())
                        .unwrap_or_default();
                    pane = pane.child(div()
                        .id(SharedString::from(format!("recent-{i}")))
                        .px_2().py_1().rounded_sm().cursor_pointer()
                        .hover(move |s| s.bg(item_bg))
                        .flex().flex_row().items_center().gap_2()
                        .child(div().text_size(px(13.0))
                            .child(SharedString::from(name)))
                        .child(div().text_size(px(11.0)).text_color(th_status)
                            .child(SharedString::from(dir)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tab = this.prev_tab;
                            this.open(q.clone());
                            cx.notify()
                        })));
                }
            } else {
                let text = self.doc.body_text();
                let words = text.split_whitespace().count();
                let chars_all = text.chars().filter(|c| *c != '\n').count();
                let paras = self.doc.paragraphs().count();
                pane = pane.child(div().text_size(px(16.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(ui::t!("文書の情報")))
                    .child(div().text_size(px(13.5))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(ui::t!("統計")));
                for (k, v) in [
                    (ui::t!("ページ"), total_pages),
                    (ui::t!("段落"), paras),
                    (ui::t!("単語"), words),
                    (ui::t!("文字数"), nchars),
                    (ui::t!("文字数 (スペースを含む)"), chars_all),
                ] {
                    pane = pane.child(div().flex().flex_row()
                        .child(div().w(px(220.0)).text_color(th_status).child(k))
                        .child(SharedString::from(format!("{v}"))));
                }
                pane = pane.child(div().h(px(6.0)))
                    .child(div().text_size(px(13.5))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(ui::t!("プロパティ")));
                let pr = self.doc.props.clone();
                let vals: [(&'static str, String, &'static str); 5] = [
                    (ui::t!("作成者"), pr.creator, ui::t!("著者を追加")),
                    (ui::t!("タイトル"), pr.title, ui::t!("テキストの追加")),
                    (ui::t!("タグ"), pr.keywords, ui::t!("テキストの追加")),
                    (ui::t!("件名"), pr.subject, ui::t!("テキストの追加")),
                    (ui::t!("コメント"), pr.description, ui::t!("テキストの追加")),
                ];
                for (i, (k, v, ph)) in vals.into_iter().enumerate() {
                    let editing = self.file_field == Some(i as u8);
                    let shown = if editing {
                        let mut t = self.prop_ed.text().to_string();
                        let cur = self.prop_ed.cursor().min(t.len());
                        t.insert(cur, '|');
                        t
                    } else {
                        v.clone()
                    };
                    let empty = !editing && v.is_empty();
                    pane = pane.child(div().flex().flex_row().items_center()
                        .child(div().w(px(220.0)).text_color(th_status).child(k))
                        .child(div()
                            .id(SharedString::from(format!("prop-{i}")))
                            .w(px(320.0)).px_2().py_1().rounded_sm()
                            .border_1()
                            .border_color(if editing {
                                rgb(0x1B6E3C)
                            } else {
                                th_cmd_border
                            })
                            .cursor_pointer()
                            .whitespace_nowrap().overflow_hidden()
                            .text_color(if empty { th_gray_fg } else { th_top_fg })
                            .child(SharedString::from(if empty {
                                ph.to_string()
                            } else {
                                shown
                            }))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let cur = match i {
                                    0 => this.doc.props.creator.clone(),
                                    1 => this.doc.props.title.clone(),
                                    2 => this.doc.props.keywords.clone(),
                                    3 => this.doc.props.subject.clone(),
                                    _ => this.doc.props.description.clone(),
                                };
                                this.prop_ed = Editor::new(&cur);
                                this.file_field = Some(i as u8);
                                cx.notify()
                            }))));
                }
                pane = pane.child(div().text_size(px(11.5)).text_color(th_status)
                    .child(ui::t!("欄を押して打ち、Enter で控える(保存で docx の情報に入ります)")));
            }
            Some(div().flex_1().relative().overflow_hidden()
                .child(div().absolute().inset_0().flex().flex_row()
                    .child(sb)
                    .child(pane))
                .child(InputSink { view: me.clone() }))
        };

        // 紙。スクロールは紙ごと上へずらすだけ(中身は全部この容器の子)。
        // ページの色は文書の設定(紙も同じ色に塗られる)
        let paper_bg = match self.dress_page.1.as_deref() {
            Some(c) => gpui::Rgba { r: hex(c, 0), g: hex(c, 1), b: hex(c, 2), a: 1.0 },
            None => gpui::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
        };
        // 印刷モードでは容器を透明にして、**紙を1枚ずつ**子として敷く。
        // 中身の座標は変えない(容器が原点のまま)ので、他は触らずに済む。
        // 紙を先に足すので、あとから足す字や画像はその上に載る
        let mut paper = div().absolute()
            .left(px(28.0)).top(px(14.0 - self.scroll_mm * pxmm))
            .w(px(self.paper_w_mm() * pxmm)).h(px(self.content_mm() * pxmm));
        if self.paged {
            for (k, top) in self.page_tops.clone().iter().enumerate() {
                let q = self.page_papers.get(k).copied().unwrap_or(paper::Paper {
                    width_mm: self.pg.w_mm,
                    height_mm: self.pg.h_mm,
                    margin_mm: self.pg.left_mm,
                });
                paper = paper.child(div().absolute()
                    .left(px(0.0)).top(px(top * pxmm))
                    .w(px(q.width_mm * pxmm)).h(px(q.height_mm * pxmm))
                    .bg(paper_bg).shadow_lg());
            }
        } else {
            paper = paper.bg(paper_bg).shadow_lg();
        }

        // ルーラー(10mm ごとの目盛り。余白の位置が分かる)
        if self.ruler {
            let mut n = 0;
            loop {
                let mm = n as f32 * 10.0;
                if mm > self.pg.w_mm {
                    break;
                }
                let major = n % 5 == 0;
                paper = paper.child(div().absolute()
                    .left(px(mm * pxmm)).top(px(0.0))
                    .w(px(1.0)).h(px(if major { 10.0 } else { 5.0 }))
                    .bg(rgb(0xAABBC6)));
                if major && n > 0 {
                    paper = paper.child(div().absolute()
                        .left(px(mm * pxmm + 2.0)).top(px(0.0))
                        .text_size(px(8.5)).text_color(rgb(0x8899A6))
                        .child(SharedString::from(format!("{}", mm as u32))));
                }
                n += 1;
            }
            // 余白の線(本文の左右端)
            for x in [self.pg.left_mm, self.pg.w_mm - self.pg.right_mm] {
                paper = paper.child(div().absolute()
                    .left(px(x * pxmm)).top(px(0.0))
                    .w(px(1.0)).h(px(14.0)).bg(rgb(0x1B6E3C)));
            }
        }

        // 画像。組版が置いた位置に、そのまま出す
        for (i, (bytes, [x, top, w_mm, h_mm])) in self.page.images.iter().enumerate() {
            let src = self.image_cache.entry(std::sync::Arc::as_ptr(bytes) as usize)
                .or_insert_with(|| {
                    let format = match bytes.get(..4) {
                        Some([0x89, b'P', b'N', b'G']) => gpui::ImageFormat::Png,
                        Some([0xFF, 0xD8, ..]) => gpui::ImageFormat::Jpeg,
                        _ => gpui::ImageFormat::Png,
                    };
                    std::sync::Arc::new(gpui::Image::from_bytes(format, bytes.to_vec()))
                })
                .clone();
            let _ = i;
            paper = paper.child(
                gpui::img(src)
                    .absolute()
                    .left(px((self.pg.left_mm + x) * pxmm))
                    .top(px(top * pxmm))
                    .w(px(w_mm * pxmm))
                    .h(px(h_mm * pxmm)),
            );
        }

        // 表の罫線。紙面の座標をそのまま引く
        for r in &self.page.rules {
            let [x1, y1, x2, y2] = *r;
            let (x1, y1) = ((self.pg.left_mm + x1) * pxmm, y1 * pxmm);
            let (x2, y2) = ((self.pg.left_mm + x2) * pxmm, y2 * pxmm);
            paper = paper.child(div().absolute()
                .left(px(x1.min(x2))).top(px(y1.min(y2)))
                .w(px((x2 - x1).abs().max(1.0))).h(px((y2 - y1).abs().max(1.0)))
                .bg(rgb(0x444B52)));
        }

        // 段落の背景色と囲み枠。行の帯として敷く(文字より下に来るよう先に描く)
        {
            let mut deco: Vec<(std::ops::Range<usize>, Option<String>, bool)> = Vec::new();
            let mut at = 0usize;
            for p in self.doc.paragraphs() {
                let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
                if p.shade.is_some() || p.boxed {
                    deco.push((at..at + len, p.shade.clone(), p.boxed));
                }
                at += len + 1;
            }
            if !deco.is_empty() {
                let (bx0, bx1) = (self.pg.left_mm, self.pg.w_mm - self.pg.right_mm);
                for line in self.page.lines.iter().filter(|l| l.from_body) {
                    let Some((r, shade, boxed)) = deco
                        .iter()
                        .find(|(r, ..)| r.start <= line.byte0 && line.byte0 <= r.end)
                        .map(|(r, sh, b)| (r.clone(), sh.clone(), *b))
                    else {
                        continue;
                    };
                    let band_top = (line.y_mm - LINE_MM * 0.75) * pxmm;
                    let band_h = LINE_MM * pxmm;
                    if let Some(c) = &shade {
                        paper = paper.child(div().absolute()
                            .left(px(bx0 * pxmm)).top(px(band_top))
                            .w(px((bx1 - bx0) * pxmm)).h(px(band_h))
                            .bg(gpui::Rgba {
                                r: hex(c, 0), g: hex(c, 1), b: hex(c, 2), a: 1.0,
                            }));
                    }
                    if boxed {
                        let ink = rgb(0x444B52);
                        for x in [bx0, bx1] {
                            paper = paper.child(div().absolute()
                                .left(px(x * pxmm)).top(px(band_top))
                                .w(px(1.0)).h(px(band_h)).bg(ink));
                        }
                        if line.byte0 == r.start {
                            paper = paper.child(div().absolute()
                                .left(px(bx0 * pxmm)).top(px(band_top))
                                .w(px((bx1 - bx0) * pxmm)).h(px(1.0)).bg(ink));
                        }
                        if line.byte_end() >= r.end {
                            paper = paper.child(div().absolute()
                                .left(px(bx0 * pxmm)).top(px(band_top + band_h))
                                .w(px((bx1 - bx0) * pxmm)).h(px(1.0)).bg(ink));
                        }
                    }
                }
            }
        }

        // ページの境の薄い線(積み上げたページの切れ目が分かるように)
        {
            let mut pno = 1;
            loop {
                let y = pno as f32 * self.pg.h_mm;
                if y >= self.content_mm() {
                    break;
                }
                paper = paper.child(div().absolute()
                    .left(px(0.0)).top(px(y * pxmm))
                    .w(px(self.pg.w_mm * pxmm)).h(px(1.0))
                    .bg(rgb(0xD5DBE0)));
                pno += 1;
            }
        }

        // 透かし。1字ずつ対角線に沿って置く(画面の近似。紙は回転した字)
        if let Some(text) = self.dress_page.0.as_deref().filter(|t| !t.is_empty()) {
            let n = text.chars().count().max(1) as f32;
            let wpt = (520.0 / n).clamp(36.0, 120.0);
            let em_mm = wpt * 25.4 / 72.0;
            let k = std::f32::consts::FRAC_1_SQRT_2;
            let (cx0, cy0) = (self.pg.w_mm / 2.0, self.pg.h_mm / 2.0);
            for (i, ch) in text.chars().enumerate() {
                let t = (i as f32 - (n - 1.0) / 2.0) * em_mm;
                let x = cx0 + t * k - em_mm / 2.0;
                let y = cy0 - t * k - em_mm / 2.0;
                paper = paper.child(div().absolute()
                    .left(px(x * pxmm)).top(px(y * pxmm))
                    .text_size(px(wpt * 96.0 / 72.0 * self.zoom))
                    .font_family(self.font_name.clone())
                    .text_color(gpui::Rgba { r: 0.62, g: 0.62, b: 0.62, a: 0.5 })
                    .child(SharedString::from(ch.to_string())));
            }
        }

        // 変更履歴の記録中: 変わった段落の左に橙の棒(Word の変更バー)
        if self.track {
            if let Some(base) = &self.track_base {
                let base_set: std::collections::HashSet<&str> =
                    base.iter().map(|s| s.as_str()).collect();
                let mut starts: Vec<(usize, bool)> = Vec::new();
                let mut at = 0usize;
                for p in self.doc.paragraphs() {
                    let t = para_text(p);
                    starts.push((at, !base_set.contains(t.as_str())));
                    at += t.len() + 1;
                }
                for line in self.page.lines.iter().filter(|l| l.from_body) {
                    let changed = starts
                        .iter()
                        .rev()
                        .find(|(b, _)| *b <= line.byte0)
                        .map(|(_, c)| *c)
                        .unwrap_or(false);
                    if changed {
                        paper = paper.child(div().absolute()
                            .left(px((self.pg.left_mm - 5.0).max(0.5) * pxmm))
                            .top(px((line.y_mm - LINE_MM * 0.7) * pxmm))
                            .w(px(2.0)).h(px(LINE_MM * pxmm))
                            .bg(rgb(0xE08A00)));
                    }
                }
            }
        }

        // コメントの印。付いた段落の1行目の右余白にオレンジの角を出す
        if self.show_comments {
            let mut at = 0usize;
            let mut heads: Vec<usize> = Vec::new(); // コメント付き段落の頭のバイト
            for p in self.doc.paragraphs() {
                let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
                if !p.comments.is_empty() {
                    heads.push(at);
                }
                at += len + 1;
            }
            for s0 in heads {
                if let Some(line) = self.page.lines.iter()
                    .filter(|l| l.from_body)
                    .find(|l| l.byte0 == s0)
                {
                    paper = paper.child(div().absolute()
                        .left(px((self.pg.w_mm - self.pg.right_mm + 2.0) * pxmm))
                        .top(px(line.y_mm * pxmm - 8.0))
                        .w(px(6.0)).h(px(6.0)).rounded_sm()
                        .bg(rgb(0xE08A00)));
                }
            }
        }

        // 行番号。本文の(見た目の)行を数え、左の余白に出す
        if self.line_numbers {
            let mut n = 0usize;
            for line in self.page.lines.iter().filter(|l| l.from_body) {
                n += 1;
                paper = paper.child(div().absolute()
                    .left(px((self.pg.left_mm - 9.0).max(1.0) * pxmm))
                    .top(px(line.y_mm * pxmm - 8.5 * self.zoom))
                    .text_size(px(8.5 * self.zoom))
                    .text_color(rgb(0x9DB8C8))
                    .child(SharedString::from(n.to_string())));
            }
        }

        // 未確定(変換中)の下線は、行が持つバイト位置(byte0)で結ぶ
        for (li, line) in self.page.lines.iter().enumerate() {
            if line.cells.is_empty() {
                continue;
            }
            if self.page.vertical {
                // 縦書き: 列の x に1字ずつ正立で置く。選択は縦の帯、
                // キャレットは横棒(変換下線は初版では出さない)
                let colx = self.page.vert_x.get(li).copied().unwrap_or(0.0);
                let mine = match self.target {
                    Target::Body => line.from_body,
                    Target::Cell { table, row, col } => {
                        line.cell == Some((table, row, col))
                    }
                };
                let (ls, le) = (line.byte0, line.byte_end());
                let base = line.cells.iter().map(|c| c.off).min().unwrap_or(0);
                let yr = |upto: usize| -> f32 {
                    line.cells.iter()
                        .find(|c| c.off - base >= upto)
                        .map(|c| c.x_mm)
                        .or_else(|| line.cells.last().map(|c| c.x_mm + c.w_mm))
                        .unwrap_or(0.0)
                };
                let selr = self.ed.selection();
                if mine && !selr.is_empty() && selr.start < le && selr.end > ls {
                    let a = selr.start.max(ls) - ls;
                    let b = selr.end.min(le) - ls;
                    paper = paper.child(div().absolute()
                        .left(px(colx * pxmm))
                        .top(px((line.y_mm + yr(a)) * pxmm))
                        .w(px(LINE_MM * 0.9 * pxmm))
                        .h(px((yr(b) - yr(a)).max(1.5) * pxmm))
                        .bg(gpui::Rgba { r: 0.40, g: 0.60, b: 0.85, a: 0.35 }));
                }
                if mine {
                    let cur = self.ed.cursor();
                    if cur >= ls && cur <= le {
                        paper = paper.child(div().absolute()
                            .left(px(colx * pxmm))
                            .top(px((line.y_mm + yr(cur - ls)) * pxmm))
                            .w(px(LINE_MM * 0.9 * pxmm))
                            .h(px(1.5))
                            .bg(rgb(0x165E83)));
                    }
                }
                for c in &line.cells {
                    let spt = c.size_pt * 96.0 / 72.0 * self.zoom;
                    let mut d = div().absolute()
                        .left(px(colx * pxmm))
                        .top(px((line.y_mm + c.x_mm) * pxmm))
                        .text_size(px(spt))
                        .font_family(c.font.clone().map(SharedString::from)
                            .unwrap_or_else(|| self.font_name.clone()))
                        .whitespace_nowrap()
                        .child(SharedString::from(c.ch.to_string()));
                    if c.fmt.bold {
                        d = d.font_weight(gpui::FontWeight::BOLD);
                    }
                    d = match &c.fmt.color {
                        Some(cl) => d.text_color(gpui::Rgba {
                            r: hex(cl, 0), g: hex(cl, 1), b: hex(cl, 2), a: 1.0,
                        }),
                        None => d.text_color(rgb(0x1B1B1B)),
                    };
                    paper = paper.child(d);
                }
                continue;
            }
            let pt = line.cells[0].size_pt;
            let sz = pt * 96.0 / 72.0 * self.zoom;
            let x0 = self.pg.left_mm + line.cells[0].x_mm;
            let top = line.y_mm * pxmm - sz * 0.88;

            if let Some(m) = &marked {
                let mine = match self.target {
                    Target::Body => line.from_body,
                    Target::Cell { table, row, col } => line.cell == Some((table, row, col)),
                };
                if !mine {
                    // 編集していない行に変換下線は出さない
                } else {
                let (ls, le) = (line.byte0, line.byte_end());
                if m.start < le && m.end > ls {
                    let a = m.start.max(ls) - ls;
                    let b = m.end.min(le) - ls;
                    let base = line.cells.iter().map(|c| c.off).min().unwrap_or(0);
                    // 幅は x 位置から出す(均等割付で字間が広がってもずれない)
                    let xr = |upto: usize| -> f32 {
                        line.cells.iter()
                            .find(|c| c.off - base >= upto)
                            .map(|c| c.x_mm)
                            .or_else(|| line.cells.last().map(|c| c.x_mm + c.w_mm))
                            .unwrap_or(0.0)
                            - line.cells[0].x_mm
                    };
                    paper = paper.child(div().absolute()
                        .left(px((x0 + xr(a)) * pxmm))
                        .top(px(top + sz * (1.05 + HALF_LEADING)))
                        .w(px((xr(b) - xr(a)).max(1.0) * pxmm))
                        .h(px(2.0)).bg(rgb(0x165E83)));
                }
                }
            }
            // 選択の色。**選択が見えないと、コピーも切り取りも信用できない**
            // (ドラッグで選べるようにしても、色が出なければ「できない」に見える)
            let selr = self.ed.selection();
            if !selr.is_empty() {
                let mine = match self.target {
                    Target::Body => line.from_body,
                    Target::Cell { table, row, col } => line.cell == Some((table, row, col)),
                };
                let (ls, le) = (line.byte0, line.byte_end());
                if mine && selr.start < le && selr.end > ls {
                    let a = selr.start.max(ls) - ls;
                    let b = selr.end.min(le) - ls;
                    let base = line.cells.iter().map(|c| c.off).min().unwrap_or(0);
                    let xr = |upto: usize| -> f32 {
                        line.cells.iter()
                            .find(|c| c.off - base >= upto)
                            .map(|c| c.x_mm)
                            .or_else(|| line.cells.last().map(|c| c.x_mm + c.w_mm))
                            .unwrap_or(0.0)
                            - line.cells[0].x_mm
                    };
                    paper = paper.child(div().absolute()
                        .left(px((x0 + xr(a)) * pxmm))
                        .top(px(top + sz * HALF_LEADING))
                        .w(px((xr(b) - xr(a)).max(1.5) * pxmm))
                        .h(px(sz * 1.2))
                        // 半透明の青。文字より下・蛍光ペンより上に敷く
                        .bg(gpui::Rgba { r: 0.40, g: 0.60, b: 0.85, a: 0.35 }));
                }
            }
            // 文字は**同じ書式の連なり**ごとに描く(部分書式。太字・大きさ・
            // 書体・色が行の中で混ざっても、その通りに出る)
            let mut i = 0usize;
            while i < line.cells.len() {
                let c0 = &line.cells[i];
                let mut j = i + 1;
                while j < line.cells.len()
                    && line.cells[j].fmt == c0.fmt
                    && line.cells[j].size_pt == c0.size_pt
                    && line.cells[j].font == c0.font
                    // 字間が広げられた行(均等割付)は1本で描けない —
                    // x が飛んだら連なりを切る
                    && (line.cells[j].x_mm
                        - line.cells[j - 1].x_mm
                        - line.cells[j - 1].w_mm)
                        .abs()
                        < 0.05
                {
                    j += 1;
                }
                let seg = &line.cells[i..j];
                let text: String = seg.iter().map(|c| c.ch).collect();
                let w_mm: f32 = seg.iter().map(|c| c.w_mm).sum();
                let f = &c0.fmt;
                let sx = self.pg.left_mm + c0.x_mm;
                let spt = c0.size_pt * 96.0 / 72.0 * self.zoom;
                let stop = line.y_mm * pxmm - spt * 0.88;
                // 上付き・下付きは小さく描き、少し上下へずらす
                let (spt, stop) = if f.superscript {
                    (spt * 0.7, stop - spt * 0.25)
                } else if f.subscript {
                    (spt * 0.7, stop + spt * 0.25)
                } else {
                    (spt, stop)
                };
                // 記入欄(コンテンツコントロール)は薄い箱で囲む。
                // 「ここは書き込む場所」と分かるように(Word の作法)
                if f.sdt.is_some() {
                    paper = paper.child(div().absolute()
                        .left(px(sx * pxmm)).top(px(stop + spt * HALF_LEADING))
                        .w(px(w_mm * pxmm)).h(px(spt * 1.15))
                        .border_1().border_color(rgb(0x8FB8CE))
                        .bg(gpui::Rgba { r: 0.55, g: 0.75, b: 0.9, a: 0.10 }));
                }
                // 参照(フィールド)はうっすら網掛け(Word の作法)。
                // 「ここは計算された値」と分かるように
                if f.field.is_some() {
                    paper = paper.child(div().absolute()
                        .left(px(sx * pxmm)).top(px(stop + spt * HALF_LEADING))
                        .w(px(w_mm * pxmm)).h(px(spt * 1.15))
                        .bg(gpui::Rgba { r: 0.55, g: 0.6, b: 0.65, a: 0.16 }));
                }
                // 蛍光ペン。字の下に色を敷く
                if let Some(h) = &f.highlight {
                    let bg = match h.as_str() {
                        "green" => rgb(0xC9F0C9),
                        "cyan" => rgb(0xC9EEF0),
                        _ => rgb(0xF7EFA8),
                    };
                    paper = paper.child(div().absolute()
                        .left(px(sx * pxmm)).top(px(stop + spt * HALF_LEADING))
                        .w(px(w_mm * pxmm)).h(px(spt * 1.15))
                        .bg(bg));
                }
                let mut d = div().absolute()
                    .left(px(sx * pxmm)).top(px(stop))
                    .text_size(px(spt))
                    .font_family(c0.font.clone().map(SharedString::from)
                        .unwrap_or_else(|| self.font_name.clone()))
                    .whitespace_nowrap()
                    .child(SharedString::from(text));
                if f.bold {
                    d = d.font_weight(gpui::FontWeight::BOLD);
                }
                if f.italic {
                    d = d.italic();
                }
                d = match &f.color {
                    Some(c) => d.text_color(gpui::Rgba {
                        r: hex(c, 0), g: hex(c, 1), b: hex(c, 2), a: 1.0,
                    }),
                    None => d.text_color(rgb(0x1B1B1B)),
                };
                paper = paper.child(d);
                // 下線・取り消し線は連なりごとに引く(gpui の text に無い)
                for (on, dy) in [
                    (f.underline, spt * (1.05 + HALF_LEADING)),
                    (f.strike, spt * (0.35 + HALF_LEADING)),
                ] {
                    if on {
                        paper = paper.child(div().absolute()
                            .left(px(sx * pxmm)).top(px(stop + dy))
                            .w(px(w_mm * pxmm)).h(px(1.0))
                            .bg(rgb(0x1B1B1B)));
                    }
                }
                i = j;
            }
            // 編集記号。空白は・、段落の終わりは ↵(見え方だけ。文書は変わらない)
            if self.show_marks && line.from_body {
                for c in &line.cells {
                    if c.ch == ' ' || c.ch == '\u{3000}' {
                        paper = paper.child(div().absolute()
                            .left(px((self.pg.left_mm + c.x_mm + c.w_mm * 0.3) * pxmm))
                            .top(px(top + sz * 0.35))
                            .text_size(px(sz * 0.6)).text_color(rgb(0x9DB8C8))
                            .child(SharedString::from(if c.ch == ' ' { "·" } else { "□" })));
                    }
                }
                let end_x = line.cells.last().map(|c| c.x_mm + c.w_mm).unwrap_or(0.0);
                paper = paper.child(div().absolute()
                    .left(px((self.pg.left_mm + end_x) * pxmm)).top(px(top))
                    .text_size(px(sz * 0.8)).text_color(rgb(0x9DB8C8))
                    .child("↵"));
            }
        }
        // ヘッダー・フッター。画面の紙は巻物なので、ヘッダーは紙の頭、
        // フッターは紙の末尾の頁の位置に出す(番号は1ページ目のもの。
        // 各ページの本当の番号は PDF で入る)。編集中は青、普段は灰色
        let foot_shift = (self.content_mm() - self.pg.h_mm).max(0.0);
        for (lines, dy, active) in [
            (&self.header_lines, 0.0, self.hf_edit == Some(false)),
            (&self.footer_lines, foot_shift, self.hf_edit == Some(true)),
        ] {
            for line in lines.iter() {
                if line.cells.is_empty() {
                    continue;
                }
                let pt = line.cells[0].size_pt;
                let sz = pt * 96.0 / 72.0 * self.zoom;
                let x0 = self.pg.left_mm + line.cells[0].x_mm;
                let top = (line.y_mm + dy) * pxmm - sz * 0.88;
                paper = paper.child(div().absolute()
                    .left(px(x0 * pxmm)).top(px(top))
                    .text_size(px(sz))
                    .font_family(self.font_name.clone())
                    .whitespace_nowrap()
                    .text_color(if active { rgb(0x165E83) } else { rgb(0x8899A6) })
                    .child(SharedString::from(line.text())));
            }
        }
        // 脚注。**紙(PDF)と同じ割り当て**で、そのページの下に仕切り線とともに出す。
        // 割り当ては paginate_full から受け取っているので、画面と紙で
        // 脚注の出るページが食い違わない
        for (k, idx) in self.page_notes.iter().enumerate() {
            if idx.is_empty() {
                continue;
            }
            let Some(off) = self.page_offsets.get(k).copied() else { continue };
            let total: f32 = idx.iter()
                .filter_map(|i| self.page.notes.get(*i))
                .map(|n| n.h_mm)
                .sum();
            // ページの上端からの深さ。紙と同じ勘定(下余白のすぐ上に積む)
            let block_top = self.pg.h_mm - self.pg.left_mm - total;
            // 仕切り線。紙と同じく三分の一の長さ
            paper = paper.child(div().absolute()
                .left(px(self.pg.left_mm * pxmm))
                .top(px((off + block_top - paper::NOTE_GAP_MM * 0.5) * pxmm))
                .w(px((self.pg.w_mm - self.pg.left_mm * 2.0) / 3.0 * pxmm))
                .h(px(1.0))
                .bg(rgb(0x99A5AE)));
            let mut up = 0.0f32;
            for i in idx {
                let Some(nb) = self.page.notes.get(*i) else { continue };
                for nl in &nb.lines {
                    if nl.cells.is_empty() {
                        continue;
                    }
                    let pt = nl.cells[0].size_pt;
                    let sz = pt * 96.0 / 72.0 * self.zoom;
                    let y = off + block_top + up + nl.y_mm;
                    paper = paper.child(div().absolute()
                        .left(px((self.pg.left_mm + nl.cells[0].x_mm) * pxmm))
                        .top(px(y * pxmm - sz * 0.88))
                        .text_size(px(sz))
                        .font_family(self.font_name.clone())
                        .whitespace_nowrap()
                        .text_color(rgb(0x1C1C1C))
                        .child(SharedString::from(
                            nl.cells.iter().map(|c| c.ch).collect::<String>())));
                }
                up += nb.h_mm;
            }
        }

        // カーソル。その場の文字の大きさに合わせて描きます(縦書きは行の側)。
        //
        // **位置は実際に画面で測って決めました**(2026-08-17。発注者
        // 「カーソルの位置が上すぎる」)。文字を描く箱の上端は
        // `y_mm - 0.88 * 大きさ` ですが、gpui はその箱の中で文字をさらに下に
        // 置くので、同じ式をカーソルに使うと上へ 0.4 文字ぶんずれます。
        // 実測では上端が 16px 高く、下端が 7px 足りませんでした。
        //
        // 点滅は 530 ミリ秒ごと(Windows の既定と同じ間隔)。打っている間は
        // 消しません — 消えると打ち間違いに気づきにくくなります。
        if !self.page.vertical && self.caret_on {
            let sz = caret_pt * 96.0 / 72.0 * self.zoom;
            paper = paper.child(div().absolute()
                .left(px(cx_mm * pxmm))
                .top(px(cy_mm * pxmm - sz * CARET_TOP))
                .w(px(1.5)).h(px(sz * CARET_H))
                .bg(rgb(0x165E83)));
        }

        // 手描きの線。gpui の Path は「塗り」なので、折れ線を
        // 幅のある四角形の連なりとして塗る(画面も紙も同じ座標)
        {
            let mut strokes: Vec<(bool, Vec<(f32, f32)>)> = Vec::new();
            for st in self.doc.ink.iter().chain(self.ink_cur.iter()) {
                let oy = self
                    .page_offsets
                    .get(st.page)
                    .copied()
                    .unwrap_or(st.page as f32 * self.pg.h_mm);
                strokes.push((
                    st.highlighter,
                    st.points.iter().map(|(x, y)| (x * pxmm, (y + oy) * pxmm)).collect(),
                ));
            }
            if !strokes.is_empty() {
                let pxmm2 = pxmm;
                paper = paper.child(
                    gpui::canvas(|_, _, _| (), move |bounds, _, window, _| {
                        for (hl, pts) in &strokes {
                            let w_px = if *hl { 3.0 } else { 0.45 } * pxmm2;
                            let color = if *hl {
                                gpui::Rgba { r: 1.0, g: 0.9, b: 0.35, a: 0.35 }
                            } else {
                                gpui::Rgba { r: 0.11, g: 0.23, b: 0.32, a: 1.0 }
                            };
                            let o = bounds.origin;
                            let mut path: Option<gpui::Path<gpui::Pixels>> = None;
                            for seg in pts.windows(2) {
                                let (x1, y1) = seg[0];
                                let (x2, y2) = seg[1];
                                let (dx, dy) = (x2 - x1, y2 - y1);
                                let len = (dx * dx + dy * dy).sqrt().max(0.01);
                                let (nx, ny) = (-dy / len * w_px / 2.0, dx / len * w_px / 2.0);
                                let a = gpui::point(o.x + px(x1 + nx), o.y + px(y1 + ny));
                                let b = gpui::point(o.x + px(x2 + nx), o.y + px(y2 + ny));
                                let c = gpui::point(o.x + px(x2 - nx), o.y + px(y2 - ny));
                                let d = gpui::point(o.x + px(x1 - nx), o.y + px(y1 - ny));
                                let p = path.get_or_insert_with(|| gpui::Path::new(a));
                                p.move_to(a);
                                p.line_to(b);
                                p.line_to(c);
                                p.line_to(d);
                            }
                            if let Some(p) = path {
                                window.paint_path(p, color);
                            }
                        }
                    })
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .size_full(),
                );
            }
        }

        // パネル群は部屋割りで panels.rs へ(純移動 2026-08-12)。名前も順も元のまま
        let Panels {
            find_panel, hf_panel, cmt_panel, wm_panel, bm_panel, style_new_panel, hist_panel,
            chat_panel, pw_panel, url_panel, fm_panel, nav_panel, rp_panel,
            lk_panel, ai_panel, sd_panel, rb_panel, eq_panel, plug_panel, xr_panel,
            font_panel, size_panel, style_panel, symbol_panel, proof_panel,
        } = self.panels(dk, th_btn, th_btn_hover, th_cmd_border, th_status, th_top_fg, cx);

        // ---- 右クリックのメニュー ----
        // InputSink より後に描く(bubble は後に登録した方が先に走るので、
        // 項目の stop_propagation がクリック処理より先に効く — calc と同じ)
        let menu = self.menu_at.map(|(mx, my)| {
            let has_sel = self.ed.has_selection();
            // (id, 名前, 付記, 押せるか)。"" は仕切り。
            // 照合は id — 名前は見せる字だけなので訳してよい
            let entries: Vec<(&'static str, &'static str, &'static str, bool)> = vec![
                ("cut", ui::t!("切り取り"), "Ctrl+X", has_sel),
                ("copy", ui::t!("コピー"), "Ctrl+C", has_sel),
                ("paste", ui::t!("貼り付け"), "Ctrl+V", true),
                ("", "", "", false),
                ("selword", ui::t!("語を選択"), "", true),
                ("selline", ui::t!("行を選択"), "", true),
                ("selall", ui::t!("すべて選択"), "Ctrl+A", true),
                ("", "", "", false),
                ("bold", ui::t!("太字"), "", true),
                ("italic", ui::t!("斜体"), "", true),
                ("underline", ui::t!("下線"), "", true),
                ("", "", "", false),
                ("align-left", ui::t!("左揃え"), "", true),
                ("align-center", ui::t!("中央揃え"), "", true),
                ("align-right", ui::t!("右揃え"), "", true),
                ("align-just", ui::t!("両端揃え"), "", true),
                ("", "", "", false),
                ("replace", ui::t!("検索と置換"), "Ctrl+F", true),
                ("comment", ui::t!("コメント"), "", true),
                ("wordcount", ui::t!("文字数を数える"), "", true),
            ];
            let h_est = entries.len() as f32 * 25.0 + 10.0;
            let win_w = f32::from(window.viewport_size().width);
            let mx = mx.min((win_w - 28.0 - 230.0).max(0.0));
            let my = my.min((self.view_h_px - h_est).max(0.0));
            let mut m = div().absolute().left(px(mx)).top(px(my)).w(px(220.0))
                .p_1().rounded_md().bg(rgb(0xFFFFFF))
                .border_1().border_color(rgb(0xC6CDD3)).shadow_lg()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
            for (i, (id, label, hint, ready)) in entries.into_iter().enumerate() {
                if id.is_empty() && label.is_empty() {
                    m = m.child(div().h(px(1.0)).my_1().bg(rgb(0xE1E6EA)));
                    continue;
                }
                if !ready {
                    m = m.child(div()
                        .flex().flex_row().items_center().justify_between().gap_4()
                        .px_3().py_1()
                        .child(div().text_size(px(12.5)).text_color(rgb(0xB6BDC4)).child(label))
                        .child(div().text_size(px(10.5)).text_color(rgb(0xD5DBE0)).child(hint)));
                    continue;
                }
                m = m.child(div()
                    .id(SharedString::from(format!("wm{i}")))
                    .flex().flex_row().items_center().justify_between().gap_4()
                    .px_3().py_1().rounded_sm().cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF2F7)))
                    .child(div().text_size(px(12.5)).text_color(rgb(0x1B1B1B)).child(label))
                    .child(div().text_size(px(10.5)).text_color(rgb(0x9AA5AE)).child(hint))
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                        move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.menu_action(id, window, cx);
                        })));
            }
            m
        });

        let notes = if self.notes.is_empty() { None } else {
            let mut n = div().absolute().right(px(16.0)).top(px(14.0)).w(px(270.0))
                .p_3().rounded_md().bg(rgb(0xFFF6E6))
                .border_1().border_color(rgb(0xE8D5A8))
                .child(div().text_size(px(11.5)).font_weight(gpui::FontWeight::BOLD)
                       .text_color(rgb(0x8A4B00)).child(ui::t!("この版で読み飛ばしたもの")));
            for x in &self.notes {
                n = n.child(div().text_size(px(11.0)).text_color(rgb(0x8A4B00))
                            .child(x.clone()));
            }
            Some(n)
        };

        div().size_full().flex().flex_col().bg(th_desk)
            .key_context("jo_edit")
            .track_focus(&self.focus)
            .on_action(cx.listener(Writer::backspace))
            .on_action(cx.listener(Writer::delete))
            .on_action(cx.listener(Writer::left))
            .on_action(cx.listener(Writer::right))
            .on_action(cx.listener(Writer::select_left))
            .on_action(cx.listener(Writer::select_right))
            .on_action(cx.listener(Writer::select_all))
            .on_action(cx.listener(Writer::up))
            .on_action(cx.listener(Writer::down))
            .on_action(cx.listener(Writer::select_up))
            .on_action(cx.listener(Writer::select_down))
            .on_action(cx.listener(Writer::word_left))
            .on_action(cx.listener(Writer::word_right))
            .on_action(cx.listener(Writer::select_word_left))
            .on_action(cx.listener(Writer::select_word_right))
            .on_action(cx.listener(Writer::a_tab))
            .on_action(cx.listener(Writer::a_shift_tab))
            .on_action(cx.listener(Writer::page_up))
            .on_action(cx.listener(Writer::page_down))
            .on_action(cx.listener(Writer::do_find))
            .on_action(cx.listener(Writer::do_print))
            .on_action(cx.listener(Writer::do_fullscreen))
            .on_action(cx.listener(Writer::do_save_as_key))
            .on_action(cx.listener(Writer::do_zoom_reset))
            .on_action(cx.listener(Writer::do_help))
            .on_action(cx.listener(Writer::do_ins_date))
            .on_action(cx.listener(Writer::do_ins_time))
            .on_action(cx.listener(Writer::do_bold))
            .on_action(cx.listener(Writer::do_italic))
            .on_action(cx.listener(Writer::do_underline))
            .on_action(cx.listener(Writer::do_strikeout))
            .on_action(cx.listener(Writer::a_context_menu))
            .on_action(cx.listener(Writer::a_cancel))
            .on_action(cx.listener(Writer::doc_home))
            .on_action(cx.listener(Writer::doc_end))
            .on_action(cx.listener(Writer::home))
            .on_action(cx.listener(Writer::end))
            .on_action(cx.listener(Writer::enter))
            .on_action(cx.listener(Writer::copy))
            .on_action(cx.listener(Writer::cut))
            .on_action(cx.listener(Writer::paste))
            .on_action(cx.listener(Writer::undo))
            .on_action(cx.listener(Writer::redo))
            .on_action(cx.listener(Writer::do_save))
            .on_action(cx.listener(Writer::do_open))
            .on_action(cx.listener(Writer::do_quit))
            // 定番の増強(2026-08-14)
            .on_action(cx.listener(Writer::do_align_left))
            .on_action(cx.listener(Writer::do_align_center))
            .on_action(cx.listener(Writer::do_align_right))
            .on_action(cx.listener(Writer::do_align_justify))
            .on_action(cx.listener(Writer::do_page_break))
            .on_action(cx.listener(Writer::do_font_bigger))
            .on_action(cx.listener(Writer::do_font_smaller))
            .child(bar)
            // **ファイルのタブはリボンのすぐ下**(Zed と同じ位置)
            .children(files_bar)
            .child(if let Some(fp) = filepage {
                fp
            } else {
                // 左右のパネルは**場所を取る**(重ねない)。紙の上に被せると
                // 本文が隠れる — calc で 2026-08-15 に同じ物を直した。
                // 紙の側に min_w(0) が要る(flex の既定 min-width:auto だと
                // 紙が縮まず、右のパネルが枠の外へ押し出されて消える)
                div().flex_1().overflow_hidden().flex().flex_row()
                .children(nav_panel)
                .child(div().flex_1().min_w(px(0.0)).relative().overflow_hidden()
                    .on_scroll_wheel(cx.listener(|this, e: &gpui::ScrollWheelEvent, _, cx| {
                        // 上に回すと delta は正 → 紙は頭の方へ戻る
                        let dy = match e.delta {
                            gpui::ScrollDelta::Pixels(p) => f32::from(p.y),
                            gpui::ScrollDelta::Lines(l) => l.y * 40.0,
                        };
                        this.scroll_px(-dy);
                        cx.notify();
                    }))
                    .child(paper)
                    .children(notes)
                    .children(find_panel)
                    .children(hf_panel)
                    .children(cmt_panel)
                    .children(wm_panel)
                    .children(bm_panel)
                    .children(style_new_panel)
                    .children(xr_panel)
                    .children(hist_panel)
                    .children(chat_panel)
                    .children(plug_panel)
                    .children(pw_panel)
                    .children(rb_panel)
                    .children(eq_panel)
                    .children(sd_panel)
                    .children(ai_panel)
                    .children(url_panel)
                    .children(fm_panel)
                    .children(lk_panel)
                    .children(font_panel)
                    .children(size_panel)
                    .children(style_panel)
                    .children(symbol_panel)
                    .children(proof_panel)
                    // 終了確認のパネル(窓の中の中央。rfd はスクリーン中央に出て遠い)
                    .children(self.quit_ask.then(|| {
                        let btn = |id: &'static str, label: String, primary: bool| {
                            div().id(id).px_3().py_1().rounded_sm().text_size(px(12.5))
                                .border_1()
                                .border_color(if primary { rgb(0x165E83) } else { rgb(0xC6CDD3) })
                                .bg(if primary { rgb(0x165E83) } else { rgb(0xFFFFFF) })
                                .text_color(if primary { rgb(0xFFFFFF) } else { rgb(0x1B1B1B) })
                                .cursor_pointer()
                                .child(SharedString::from(label))
                        };
                        div().absolute().inset_0().flex().items_center().justify_center()
                            .child(div().w(px(420.0)).p_3().rounded_md().bg(rgb(0xF7F9FA))
                                .border_1().border_color(rgb(0x165E83)).shadow_lg()
                                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation()
                                })
                                .flex().flex_col().gap_2()
                                .child(div().text_size(px(13.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0x165E83))
                                    .child(ui::t!("保存していない変更があります")))
                                .child(div().text_size(px(12.0))
                                    .child(ui::t!(
                                        "保存して終了しますか?(Enter = 保存して終了 / Esc = やめる)")))
                                .child(div().flex().flex_row().gap_2().justify_center()
                                    .child(btn("q-save", ui::t!("保存して終了").to_string(), true)
                                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                                            |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.quit_ask = false;
                                                this.save(true, cx);
                                                cx.notify();
                                            })))
                                    .child(btn("q-drop", ui::t!("保存せず終了").to_string(), false)
                                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                                            |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.release_lock();
                                                cx.quit();
                                            })))
                                    .child(btn("q-cancel", ui::t!("キャンセル").to_string(), false)
                                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                                            |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.quit_ask = false;
                                                this.status = ui::t!("終了をやめました").into();
                                                cx.notify();
                                            })))))
                    }))
                    .child(InputSink { view: me })
                    .children(menu))
                .children(rp_panel)
            })
            // **文書の耳はステータスバーの上**(calc のシートの耳と同じ位置)
            .children(docs_bar)
            .children(self.show_statusbar.then_some(statusbar))
            // 窓の縁のつかみ(最後に描く = 最初にマウスを受ける)。
            // GNOME の Wayland は外枠を付けないので、これが無いと
            // 大きさを変えられない(calc と共通 — ui::resize_edges)
            .children(ui::resize_edges(window))
    }
}

/// ホバーで出す小さな札。**絵だけの釦には要る** — 左右のパネルの柱は
/// アイコンしか出さないので、これが無いと何の面か分からない(2026-08-15)。
/// calc の `Tip` と同じ作り(2例から抽象は作らない — 部屋が別々のまま)
pub(crate) struct Tip(pub(crate) SharedString);
impl gpui::Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        div().px_2().py_1().rounded_md()
            .bg(gpui::rgb(0x2B2F33)).text_color(gpui::rgb(0xF2F5F7))
            .text_size(px(11.0))
            .border_1().border_color(gpui::rgb(0x14161A))
            .shadow_md()
            .child(self.0.clone())
    }
}

/// 入力ハンドラは **paint のときに窓へ差す**(GPUI の作法)。
/// 何も描かない要素だが、これが無いと IME もキー入力も届かない。
struct InputSink {
    view: Entity<Writer>,
}

impl IntoElement for InputSink {
    type Element = Self;
    fn into_element(self) -> Self { self }
}

impl gpui::Element for InputSink {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> { None }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> { None }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, ()) {
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<gpui::Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {}

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<gpui::Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.view.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        // クリックでカーソルを置く。編集領域の座標を知っているのはここだけ
        let view = self.view.clone();
        window.on_mouse_event(move |e: &gpui::MouseDownEvent, phase, _w, cx| {
            if phase != gpui::DispatchPhase::Bubble
                || e.button != gpui::MouseButton::Left
                || !bounds.contains(&e.position)
            {
                return;
            }
            let rel = e.position - bounds.origin;
            let clicks = e.click_count;
            let shift = e.modifiers.shift;
            view.update(cx, |w, cx| {
                if w.tab == 0 {
                    // ファイルのページ。紙は無いのでキャレットも筆も動かさない
                    return;
                }
                w.menu_at = None;
                if w.tool.is_some() {
                    // 道具の間、マウスは筆になる(文字は選ばない)
                    let pxmm = PX_PER_MM * w.zoom;
                    let x = (f32::from(rel.x) - 28.0) / pxmm;
                    let y = (f32::from(rel.y) - 14.0) / pxmm + w.scroll_mm;
                    w.ink_begin(x, y);
                    cx.notify();
                    return;
                }
                match clicks {
                    // 二度押しは語、三度押しは行を選ぶ
                    2 => {
                        w.click_at(f32::from(rel.x), f32::from(rel.y), false);
                        w.select_word();
                        w.drag_select = false;
                    }
                    c if c >= 3 => {
                        w.click_at(f32::from(rel.x), f32::from(rel.y), false);
                        w.select_line();
                        w.drag_select = false;
                    }
                    _ => {
                        w.click_at(f32::from(rel.x), f32::from(rel.y), shift);
                        w.drag_select = true;
                    }
                }
                cx.notify();
            });
        });
        // 押したまま動かすと選択が伸びる(文字の選択の通り相場)
        let view = self.view.clone();
        window.on_mouse_event(move |e: &gpui::MouseMoveEvent, phase, _w, cx| {
            if phase != gpui::DispatchPhase::Bubble
                || e.pressed_button != Some(gpui::MouseButton::Left)
            {
                return;
            }
            let rel = e.position - bounds.origin;
            view.update(cx, |w, cx| {
                if w.tool.is_some() {
                    let pxmm = PX_PER_MM * w.zoom;
                    let x = (f32::from(rel.x) - 28.0) / pxmm;
                    let y = (f32::from(rel.y) - 14.0) / pxmm + w.scroll_mm;
                    w.ink_move(x, y);
                    cx.notify();
                    return;
                }
                if w.drag_select {
                    w.click_at(f32::from(rel.x), f32::from(rel.y), true);
                    cx.notify();
                }
            });
        });
        let view = self.view.clone();
        window.on_mouse_event(move |e: &gpui::MouseUpEvent, phase, _w, cx| {
            if phase != gpui::DispatchPhase::Bubble || e.button != gpui::MouseButton::Left {
                return;
            }
            view.update(cx, |w, cx| {
                if w.tool.is_some() {
                    w.ink_end();
                    cx.notify();
                }
                w.drag_select = false;
            });
        });
        // 右クリックでメニュー。選択があれば選択への操作、無ければ押した所へ
        let view = self.view.clone();
        window.on_mouse_event(move |e: &gpui::MouseDownEvent, phase, _w, cx| {
            if phase != gpui::DispatchPhase::Bubble
                || e.button != gpui::MouseButton::Right
                || !bounds.contains(&e.position)
            {
                return;
            }
            let rel = e.position - bounds.origin;
            view.update(cx, |w, cx| {
                if !w.ed.has_selection() {
                    w.click_at(f32::from(rel.x), f32::from(rel.y), false);
                }
                w.menu_at = Some((f32::from(rel.x), f32::from(rel.y)));
                cx.notify();
            });
        });
    }
}
