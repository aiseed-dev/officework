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


impl Render for Writer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let us = self.ui_scale;
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
        // 空いている所だけを取っ手にすると、タブが多い窓では幅がゼロになり
        // 掴む場所が無くなる(踏んで直した)。ボタンの類いは stop_propagation で
        // 取っ手より先に効く
        let (ready, all) = ribbon::progress_for(ribbon::App::Writer);
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
        // デスクトップ版の画面の組み立て: 1行目がクイックアクセス+文書名(=取っ手)、
        // 2段目が下線つきのタブ(現在地は青い下線)、3段目がリボンのボタン
        let th_top_bg = if dk { rgb(0x1B1E21) } else { rgb(0xF1F3F5) };
        let th_top_fg = if dk { rgb(0xCFD6DC) } else { rgb(0x444B52) };
        let th_qa_hover = if dk { rgb(0x2C333A) } else { rgb(0xE2E6EA) };
        let qa = |id: &'static str, icon: &'static str| {
            div().id(id).px_2().py_1().rounded_sm().cursor_pointer()
                .hover(move |s| s.bg(th_qa_hover))
                .child(gpui::svg()
                    .path(SharedString::from(format!("icons/{icon}.svg")))
                    .size(px(us * 15.0)).text_color(th_top_fg))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        };
        let title = self
            .path
            .as_ref()
            .and_then(|q| q.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| ui::t!("untitled_document").into());
        let winbtn = |id: &'static str, label: &'static str| {
            div().id(id).px_2p5().py_1().rounded_sm()
                .text_size(px(us * 12.0)).text_color(th_top_fg)
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
            .child(div().text_size(px(us * 12.5)).text_color(th_top_fg)
                .whitespace_nowrap().overflow_hidden()
                .child(SharedString::from(format!(
                    "{}{title}",
                    if self.dirty { "*" } else { "" }
                ))))
            .child(div().flex_1())
            .child(div().pr_2().text_size(px(us * 10.5))
                .text_color(if dk { rgb(0x6E7982) } else { rgb(0x8A949D) })
                .child(SharedString::from(ui::tf!("writer_implemented", ready, all))))
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
        // 押せるボタンが無い段(文脈タブも)は隠す。開いていた段が消えたらホームへ
        let hidden_tabs: Vec<bool> = (0..ribbon::tabs().len()).map(|i| self.tab_hidden_now(i)).collect();
        if hidden_tabs.get(self.tab).copied().unwrap_or(false) {
            self.tab = 1;
        }
        // ---- リボンのタブの行(実装は ui::tabrow に1本。統合の段6の後半)----
        let tabs = ui::tabrow::build(
            cx,
            ui::tabrow::Side::Doc,
            self.tab,
            us,
            dlg_open,
            ui::tabrow::Look {
                row_bg: th_tab_on_bg,
                grey: th_gray_fg,
                on_fg: th_tab_on_fg,
                idle_fg: th_tab_idle,
                hover_fg: th_tab_on_fg,
                find_fg: th_tab_idle,
                underline_on: th_btn,
                ctx_fg: th_gray_fg,
                ctx_bg: th_tab_on_bg,
            },
            self.btn_box.clone(),
            // 文脈タブと、押せるボタンの無い段は隠す(リボンは1つ。2026-09-04)
            move |i| hidden_tabs.get(i).copied().unwrap_or(false),
            |_| false,
            |_| None,
            |this: &mut Writer, i, cx| {
                // タブ切替でも開いている一覧は畳む(他を押したら閉じる)
                this.close_menus();
                if i == 0 && this.tab != 0 {
                    this.prev_tab = this.tab;
                    this.file_view = 0;
                    this.file_field = None;
                }
                this.tab = i;
                // **置き場を見直す**(~/.config/officework/ribbon)。
                // .py を足したのに次に起こすまでボタンが出ない、では手が
                // 止まります。表は定期的に見ていますが、文章は打鍵ごとに
                // 置き場を stat したくないので、タブを替えた時に見ます
                ribbon::refresh_user_cmds();
                cx.notify()
            },
            |this: &mut Writer, cx| {
                this.run_from_ribbon("replace", cx);
                cx.notify()
            },
        );

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
                    // 表の中で効くセルの操作(表の画面と同じボタン。外では灰色)
                    ("‖", None), ("top", None), ("middle", None), ("bottom", None),
                    ("fillparag", None),
                    ("‖", None), ("replace", None),
            ],
        ];
        // 挿入は一段(発注者の画像 2026-08-04)。主要なボタンは名札つきの大ボタン
                // 残りのタブも一段(本家 Web 版の並びから起こした。2026-08-04 発注者)
        const DRAW_ROWS: &[&[LItem]] = &[&[
            ("pen", Some("ペン")), ("highlighter", Some("蛍光ペン")),
            ("eraser", Some("消しゴム")),
        ]];
        const LAYOUT_ROWS: &[&[LItem]] = &[&[
            ("pagemargins", Some("margins")), ("pageorient", Some("orientation")),
            ("pagesize", Some("サイズ")), ("columns", Some("columns")),
            ("‖", None), ("line-numbers", None), ("hyphenation", None),
            // 図形まわりの5つ。**まだ押せません**(図形そのものが入っていない)。
            // 表の側と同じ扱いで、場所だけ取ります(2026-08-21 発注者
            // 「calc と同じようにして」)。灰色は絵の名前で引きます
            ("‖", None), ("img-movefrwd", None), ("img-movebkwd", None),
            ("img-align", None), ("img-group", None), ("shapes-merge", None),
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
            ("form-signature", None), ("‖", None), ("form-name", Some("name")),
        ]];
        const COLLAB_ROWS: &[&[LItem]] = &[&[
            ("coauth-mode", Some("co_editing_mode")), ("‖", None),
            ("co-addcomment", Some("comment")), ("co-delcomment", None),
            ("co-showcomment", None), ("‖", None), ("co-chat", Some("chat")),
            ("‖", None), ("track-changes", Some("変更履歴")), ("‖", None),
            ("co-history", Some("version_history")),
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
                ("zoom100", Some("zoom_100")), ("zoom-in", None),
                ("‖", None), ("darkmode", None),
                // **表にしかありませんでした**(2026-08-21 発注者)
                ("ui-bigger", None), ("ui-smaller", None),
                ("ruler", None),
                ("‖", None), ("show-toolbar", None), ("show-left", None),
            ],
            &[
                ("‖", None),
                ("fit-width", Some("幅に合わせる")),
                ("multipage", Some("複数ページ")),
                ("zoom-out", None),
                ("‖", None), ("‖", None),
                ("‖", None), ("show-statusbar", None), ("show-right", None),
            ],
        ];
        // **マクロの段**(2026-08-16 に「プラグイン」から改名)。
        // 「一覧」は置き場の .py、「ファイルから」は置き場の外の .py。
        // **マクロを書く**(AI)もここ — 置き場に .py を置く仕事で、
        // 会話では代われない(2026-08-15、AI タブの廃止で移した)
                // 挿入とマクロの段は行の表を持たない(骨組みの並びで描く)。前は
        // 小文字の "insert" / "macros" で照合していて届かず、表の方は文言の
        // 鍵が生のまま出る古い物だったので、表を消した(2026-09-02)
        let rows: Option<&[&[LItem]]> = match ribbon::skeleton()[self.tab].name {
            "Home" => Some(HOME_ROWS),
            "Draw" => Some(DRAW_ROWS),
            "Layout" => Some(LAYOUT_ROWS),
            "References" => Some(REF_ROWS),
            "Forms" => Some(FORM_ROWS),
            "Collaboration" => Some(COLLAB_ROWS),
            "Protection" => Some(PROT_ROWS),
            "View" => Some(VIEW_ROWS),
            _ => None,
        };
        if let Some(rows) = rows {
            let size_now = self.size_now();
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
                        row = row.child(div().w(px(us * 1.0))
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
                        // **この欄も場所を控える**(2026-08-20)。一覧を
                        // ボタンの真下に出すのに要ります。前は控えていな
                        // かったので、点検の道具も座標を目分量で当てる
                        // しかありませんでした
                        let mark = {
                            let rec = self.btn_box.clone();
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
                        row = row.child(div()
                            .id(SharedString::from(format!("h-{cid}")))
                            .relative()
                            .child(mark)
                            .flex().flex_row().items_center().gap_1()
                            .px_2().h(px(us * 26.0))
                            .w(px(if cid == "fontname" { 150.0 } else { 56.0 }))
                            .rounded_sm().border_1().border_color(th_cmd_border)
                            .text_size(px(us * 12.0))
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
                            .child(div().text_size(px(us * 9.0)).text_color(th_tab_idle)
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
                    // **リボンは1つ。** この画面が知らないボタンと、的がいま無い
                    // ボタンは、未実装と同じ灰色で同じ場所に出す(2026-09-04)
                    let cmd = ribbon::Cmd { ready: self.usable_here(&cmd), ..cmd };
                    let label = cmd.label;
                    // **入切のボタンは、入っている間ずっと押された形**
                    // (2026-08-21 発注者)。押してみないと分からない、をやめる
                    let on_of = cmd.kind == ribbon::Kind::Toggle && self.is_on(cmd.id);
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
                        // **いまの状況で意味が無いボタンも灰色に**
                        // (2026-08-21 の B-5)。押す前に見て分かるように
                        let fg = if !cmd.ready || dlg_open || !self.can_press(cmd.id) {
                            th_gray_fg
                        } else if on {
                            th_btn
                        } else {
                            th_top_fg
                        };
                        let mut b = div()
                            .id(SharedString::from(format!("h-{icon}")))
                            .px_2().h(px(us * 48.0)).rounded_sm()
                            .when(on_of, |d| d.bg(th_btn_hover))
                            .flex().flex_col().items_center().justify_center()
                            .gap_1()
                            .on_hover(hoverable)
                            // **マウスを置いたらすぐ説明を出す**(2026-08-17
                            // 発注者)。下のステータスバーにも名前は出ますが、
                            // マウスから遠くて気づきません。gpui の既定の
                            // 待ち時間は 500 ミリ秒で、待たされる感じがあります
                            .tooltip(move |_, cx| cx.new(|_| Tip(label.into(), us)).into())
                            .tooltip_show_delay(std::time::Duration::from_millis(150))
                            .children(has_icon.then(|| {
                                gpui::svg()
                                    .path(SharedString::from(format!("icons/{icon}.svg")))
                                    .size(px(us * 20.0))
                                    .text_color(fg)
                            }))
                            .child(div().flex().flex_row().items_center().gap_0p5()
                                .text_size(px(us * 10.5)).text_color(fg)
                                .child(short)
                                .children(marker.map(|m| div()
                                    .text_size(px(us * 8.0)).text_color(th_tab_idle)
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
                        .h(px(us * 26.0)).rounded_sm()
                        .when(on_of, |d| d.bg(th_btn_hover))
                        .flex().items_center().justify_center()
                        .on_hover(hoverable)
                        .tooltip(move |_, cx| cx.new(|_| Tip(label.into(), us)).into())
                        .tooltip_show_delay(std::time::Duration::from_millis(150));
                    b = if has_icon {
                        // 印つきは幅を固定しない(印のぶん広がる)
                        if marker.is_some() { b.px_0p5() } else { b.w(px(us * 26.0)) }
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
                                .size(px(us * 18.0))
                                .text_color(icon_fg)
                        }))
                        .children(has_icon.then_some(marker).flatten().map(|m| {
                            // 開く印(▾=一覧 / …=小窓)
                            div().text_size(px(us * 8.0)).text_color(th_tab_idle).child(m)
                        }))
                        .children((!has_icon).then(|| {
                            div().text_size(px(us * 10.5))
                                .text_color(if usable { th_btn } else { th_gray_fg })
                                .flex().flex_row().items_center().gap_0p5()
                                .child(label)
                                .children(marker.map(|m| div()
                                    .text_size(px(us * 8.0)).text_color(th_tab_idle)
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
            // **利用者が置いたマクロのボタンを、後ろに足す**
            // (~/.config/officework/ribbon。2026-08-20 発注者「ユーザーが
            // 生成したボタン用のマクロをその後ろにおくコードが必要」)。
            //
            // *文章の画面には1つも出ていませんでした。* 表には前からあり
            // ましたが、文章はどのタブも並びの表を通るので、表に載っていない
            // ボタンは描かれません。同じ置き場を見ているのに、片方だけ
            // 使えない状態でした。
            //
            // **組み込みの後ろ**に置きます — 並びが日によって変わらないよう、
            // 置き場の物は必ず後ろです
            let user = ribbon::user_cmds_for(ribbon::App::Writer, self.tab);
            if !user.is_empty() {
                let mut row = div().flex().flex_row().flex_wrap().gap_1().items_center().py_1();
                for cmd in user {
                    let id = cmd.id;
                    row = row.child(
                        div()
                            .id(SharedString::from(cmd.id))
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_size(px(us * 12.0))
                            .text_color(if dlg_open { th_gray_fg } else { th_top_fg })
                            .when(!dlg_open, |d| {
                                d.cursor_pointer().hover(move |s| s.bg(th_btn_hover)).on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.run_from_ribbon(id, cx);
                                        cx.notify()
                                    }),
                                )
                            })
                            .child(SharedString::from(cmd.label)),
                    );
                }
                cmds = cmds.child(row);
            }
        } else {
            let mut row = div().flex().flex_row().flex_wrap().gap_1().items_center().py_1();
            for cmd in ribbon::writer_tabs()[self.tab].cmds {
                // リボンは1つ: 知らないボタンと的の無いボタンは灰色(2026-09-04)
                let cmd = &ribbon::Cmd { ready: self.usable_here(cmd), ..*cmd };
                // 小窓中は ready でも灰色・無反応(未実装と同じ描き方)
                if cmd.ready && !dlg_open {
                    let id = cmd.id;
                    // **この道のボタンも場所を控えます**(2026-09-02)。控えが
                    // 無いと、一覧(日付・記号・図形・SmartArt)がボタンの下で
                    // なく画面の左上に出ます。実機の点検の道具も座標を当てる
                    // しかありませんでした
                    let mark = {
                        let rec = self.btn_box.clone();
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
                        .size_full()
                    };
                    row = row.child(div()
                        .id(SharedString::from(cmd.id))
                        .relative()
                        .child(mark)
                        .px_3().py_1().rounded_md()
                        .border_1().border_color(th_btn).text_color(th_btn)
                        .text_size(px(us * 12.0)).cursor_pointer()
                        .hover(move |s| s.bg(th_btn_hover))
                        .flex().flex_row().items_center().gap_1()
                        .children(ui::icons::find(cmd.icon).map(|_| {
                            gpui::svg()
                                .path(SharedString::from(format!("icons/{}.svg", cmd.icon)))
                                .size(px(us * 15.0))
                                .text_color(th_btn)
                        }))
                        .child(cmd.label)
                        // 開く印(▾=一覧 / …=小窓)
                        .children(marker_of(cmd.id).map(|m| div()
                            .text_size(px(us * 9.0)).text_color(th_tab_idle)
                            .child(m)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run_from_ribbon(id, cx); cx.notify()
                        })));
                } else {
                    // 未実装(と小窓中)。押せるように見せない
                    row = row.child(div().px_3().py_1().rounded_md()
                        .border_1().border_color(th_gray_border)
                        .text_color(th_gray_fg).text_size(px(us * 12.0))
                        .flex().flex_row().items_center().gap_1()
                        .children(ui::icons::find(cmd.icon).map(|_| {
                            gpui::svg()
                                .path(SharedString::from(format!("icons/{}.svg", cmd.icon)))
                                .size(px(us * 15.0))
                                .text_color(th_gray_fg)
                        }))
                        .child(cmd.label)
                        .children(marker_of(cmd.id).map(|m| div()
                            .text_size(px(us * 9.0)).text_color(th_tab_idle)
                            .child(m))));
                }
            }
            cmds = cmds.child(row);
        }
        let bar = if self.tab == 0 || !self.show_toolbar {
            // ファイルのページ(本家の File メニュー)と、畳んだツールバーは
            // リボンのボタンを持たない(タブは残る — 押せば中身へ行ける)
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
                let draft = self.file_dirty(i);
                let mut label_text = self.file_name(i);
                if draft {
                    // **書きかけの印。** 閉じる前に気づけるように
                    label_text.push('*');
                }
                bar = bar.child(div()
                    .id(SharedString::from(format!("file{i}")))
                    .px_2p5().py_0p5().rounded_sm().cursor_pointer()
                    .bg(if on { rgb(0xFFFFFF) } else { gpui::transparent_black().into() })
                    .border_1().border_color(if on { th_cmd_border } else { gpui::transparent_black().into() })
                    .text_size(px(us * 11.5))
                    .text_color(if on { th_top_fg } else { th_status })
                    .child(SharedString::from(label_text))
                    .on_click(cx.listener(move |t, _, _, cx| { t.show_file(i); cx.notify() })));
                // 閉じる(×)。書きかけがあるときは断ります
                bar = bar.child(div()
                    .id(SharedString::from(format!("filex{i}")))
                    .px_1().rounded_sm().cursor_pointer()
                    .text_size(px(us * 11.0)).text_color(th_status)
                    .hover(move |s| s.bg(th_qa_hover))
                    .child("×")
                    .on_click(cx.listener(move |t, _, _, cx| { t.close_file(i); cx.notify() })));
            }
            bar
        });

        // ---- 文書のタブ(1つのファイルに何枚も入っているとき) ----
        //
        // **calc のシートのタブと同じ位置・同じ動き**にしてあります
        // (2026-08-19)。表の画面と文章の画面で、下のタブの意味が揃います。
        // 1枚しか入っていないときは出しません — 何も選べないタブは邪魔です
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
                    .text_size(px(us * 11.5))
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
                .text_size(px(us * 11.5)).text_color(th_top_fg)
                .hover(move |s| s.bg(th_qa_hover))
                .child(label)
        };
        let statusbar = div().flex().flex_row().items_center().gap_3()
            .px_3().py_0p5().bg(th_top_bg)
            .border_t_1().border_color(th_cmd_border)
            .text_size(px(us * 11.0)).text_color(th_status)
            .child(SharedString::from(ui::tf!("page", cur_page, total_pages)))
            .child(SharedString::from(ui::tf!("characters", nchars)))
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
            .child(sb_btn("sb-spell", ui::t!("spell")).on_click(cx.listener(|this, _, _, cx| {
                this.run_cmd("spell", cx);
                cx.notify()
            })))
            .child(sb_btn("sb-zoom-out", "−").on_click(cx.listener(|this, _, _, cx| {
                this.run_cmd("zoom-out", cx);
                cx.notify()
            })))
            .child(div().id("sb-zoom").px_1().rounded_sm().cursor_pointer()
                .text_size(px(us * 11.5)).text_color(th_top_fg)
                .hover(move |s| s.bg(th_qa_hover))
                .child(SharedString::from(ui::tf!("zoom", (self.zoom * 100.0).round() as i32)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.zoom = 1.0;
                    cx.notify()
                })))
            .child(sb_btn("sb-zoom-in", ui::t!("plus_char")).on_click(cx.listener(|this, _, _, cx| {
                this.run_cmd("zoom-in", cx);
                cx.notify()
            })));

        // ---- ファイルのページ(本家の File メニュー。タブ0で全面に出す) ----
        // **埋め込みのときは officework が描きます**(統合の段8。2026-09-04)。
        // 左の列は officework の持ち物で、右側だけ [`Writer::file_pane`] を
        // 呼ばれます。ここで組むと、同じページが2つの持ち主から出ます
        let filepage: Option<gpui::Div> = if self.tab != 0 || self.embedded {
            None
        } else {
            let item_bg = th_qa_hover;
            // **左の列は ui::filemenu が描きます**(統合の段8 の本体)。
            // 前は同じ 40 行が表の画面にも写してあり、片方だけ直る型でした。
            // **場所の控えは渡します** — 点検の道具が座標を当てずに押せる
            // ように(2026-08-17。リボンのボタンと同じ形)
            let sb = ui::filemenu::sidebar(
                &ui::filemenu::SideLook {
                    bg: th_top_bg,
                    border: th_cmd_border,
                    fg: th_top_fg,
                    gray: th_gray_fg,
                    hover: item_bg,
                    scale: us,
                },
                &self.file_menu(),
                Some(self.btn_box.clone()),
                cx,
                |this: &mut Writer, id, cx| this.file_menu_click(id, cx),
            );

            let pane = self.file_pane(cx);
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
        // 紙を1枚ずつ子として敷く(容器は透明)。中身の座標は変えない
        // (容器が原点のまま)ので、他は触らずに済む。紙を先に足すので、
        // あとから足す字や画像はその上に載る。Web の形(区切り=なし)と
        // 縦書きと見開きは1枚の長い紙のまま
        let mut paper = div().absolute()
            .left(px(28.0)).top(px(14.0 - self.scroll_mm * pxmm))
            .w(px(self.paper_w_mm() * pxmm)).h(px(self.content_mm() * pxmm));
        if self.sheets() {
            for (k, top) in self.page_tops.clone().iter().enumerate() {
                let q = self.page_papers.get(k).copied().unwrap_or(paper::Paper::from_page(&self.pg));
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

        // **セルの塗り**(組んだ紙の fills)。前は PDF だけが描き、画面は
        // 落としていた — 表スタイルの帯もセルの塗りも紙にだけ出ていた
        // (2026-09-04、的の順で writer の表にセルの塗りを入れて気づいた)。
        // 罫線と文字より先に敷く
        for (at, c) in &self.page.fills {
            let [x, y, w, h] = *at;
            paper = paper.child(div().absolute()
                .left(px((self.pg.left_mm + x) * pxmm)).top(px(y * pxmm))
                .w(px(w * pxmm)).h(px(h * pxmm))
                .bg(gpui::Rgba { r: hex(c, 0), g: hex(c, 1), b: hex(c, 2), a: 1.0 }));
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

        // 段落の背景色と囲み枠。行の下地として敷く(文字より下に来るよう先に描く)。
        // 元は `lay` が合成後の段落から控えた `para_deco`(テンプレートの背景も含む)
        {
            let deco = &self.para_deco;
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
                // 縦書き: 列の x に1字ずつ正立で置く。選択は縦の下地、
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

        // **ページに貼り付く図形**(2026-08-30)。calc と同じ SVG の道です —
        // 大きさを織り込んで作るので、拡げても鮮明です。
        // 選んでいる図形には枠を出します
        for (i, sp) in self.doc.shapes.iter().enumerate() {
            let oy = self
                .page_offsets
                .get(sp.page)
                .copied()
                .unwrap_or(sp.page as f32 * self.pg.h_mm);
            // 模型は mm、画面は px。図形の見た目は px で作るので直します
            let mut look = sp.look.clone();
            look.width_px = sp.w_mm * PX_PER_MM;
            look.height_px = sp.h_mm * PX_PER_MM;
            let pad = look.pad();
            let svg = look.to_svg();
            let key = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                svg.hash(&mut h);
                h.finish() as usize
            };
            let src = self
                .shape_cache
                .borrow_mut()
                .entry(key)
                .or_insert_with(|| {
                    std::sync::Arc::new(gpui::Image::from_bytes(
                        gpui::ImageFormat::Svg,
                        svg.into_bytes(),
                    ))
                })
                .clone();
            let (x, y) = (sp.x_mm * pxmm, (sp.y_mm + oy) * pxmm);
            let (w, h) = (sp.w_mm * pxmm, sp.h_mm * pxmm);
            let pd = pad / PX_PER_MM * pxmm;
            paper = paper.child(
                gpui::img(src)
                    .absolute()
                    .left(px(x - pd))
                    .top(px(y - pd))
                    .w(px(w + pd * 2.0))
                    .h(px(h + pd * 2.0)),
            );
            // 選んでいる図形には枠を出します。Ctrl+クリックで足した図形も
            // 同じ枠で、主(最後に押した図形)だけ太くします
            let picked = self.shape_pick.contains(&i);
            if self.shape_sel == Some(i) || picked {
                let frame = div()
                    .absolute()
                    .left(px(x - 2.0))
                    .top(px(y - 2.0))
                    .w(px(w + 4.0))
                    .h(px(h + 4.0))
                    .border_color(rgb(0x1B6E3C));
                paper = paper.child(if self.shape_sel == Some(i) {
                    frame.border_2()
                } else {
                    frame.border_1()
                });
            }
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
            tbl_panel, date_panel, export_panel, fl_panel, user_font_panel,
        } = self.panels(dk, th_btn, th_btn_hover, th_cmd_border, th_status, th_top_fg, cx);

        // ---- 右クリックのメニュー ----
        // InputSink より後に描く(bubble は後に登録した方が先に走るので、
        // 項目の stop_propagation がクリック処理より先に効く — calc と同じ)
        let menu = self.menu_at.map(|(mx, my)| {
            let has_sel = self.ed.has_selection();
            // (id, 名前, 付記, 押せるか)。"" は仕切り。
            // 照合は id — 名前は見せる字だけなので訳してよい
            let entries: Vec<(&'static str, &'static str, &'static str, bool)> = vec![
                ("cut", ui::t!("cut"), "Ctrl+X", has_sel),
                ("copy", ui::t!("copy"), "Ctrl+C", has_sel),
                ("paste", ui::t!("paste"), "Ctrl+V", true),
                ("", "", "", false),
                ("selword", ui::t!("select_word"), "", true),
                ("selline", ui::t!("select_line"), "", true),
                ("selall", ui::t!("select_all_2"), "Ctrl+A", true),
                ("", "", "", false),
                ("bold", ui::t!("bold"), "", true),
                ("italic", ui::t!("italic"), "", true),
                ("underline", ui::t!("underline"), "", true),
                ("", "", "", false),
                ("align-left", ui::t!("align_left"), "", true),
                ("align-center", ui::t!("centre"), "", true),
                ("align-right", ui::t!("align_right"), "", true),
                ("align-just", ui::t!("justify"), "", true),
                ("", "", "", false),
                ("replace", ui::t!("find_replace"), "Ctrl+F", true),
                ("comment", ui::t!("comment"), "", true),
                ("wordcount", ui::t!("word_count"), "", true),
                // **読み飛ばした物を見直す口**(2026-08-26)。断りを閉じても
                // ここから見られます。読めなかった物があるときだけ出します
                ("show-notes", ui::t!("skipped_version"), "", !self.notes.is_empty()),
            ];
            let h_est = entries.len() as f32 * 25.0 + 10.0;
            let win_w = f32::from(window.viewport_size().width);
            let mx = mx.min((win_w - 28.0 - 230.0).max(0.0));
            let my = my.min((self.view_h_px - h_est).max(0.0));
            let mut m = div().absolute().left(px(mx)).top(px(my)).w(px(us * 220.0))
                .p_1().rounded_md().bg(rgb(0xFFFFFF))
                .border_1().border_color(rgb(0xC6CDD3)).shadow_lg()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
            for (i, (id, label, hint, ready)) in entries.into_iter().enumerate() {
                if id.is_empty() && label.is_empty() {
                    m = m.child(div().h(px(us * 1.0)).my_1().bg(rgb(0xE1E6EA)));
                    continue;
                }
                if !ready {
                    m = m.child(div()
                        .flex().flex_row().items_center().justify_between().gap_4()
                        .px_3().py_1()
                        .child(div().text_size(px(us * 12.5)).text_color(rgb(0xB6BDC4)).child(label))
                        .child(div().text_size(px(us * 10.5)).text_color(rgb(0xD5DBE0)).child(hint)));
                    continue;
                }
                m = m.child(div()
                    .id(SharedString::from(format!("wm{i}")))
                    .flex().flex_row().items_center().justify_between().gap_4()
                    .px_3().py_1().rounded_sm().cursor_pointer()
                    .hover(|s| s.bg(rgb(0xEAF2F7)))
                    .child(div().text_size(px(us * 12.5)).text_color(rgb(0x1B1B1B)).child(label))
                    .child(div().text_size(px(us * 10.5)).text_color(rgb(0x9AA5AE)).child(hint))
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                        move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.menu_action(id, window, cx);
                        })));
            }
            m
        });

        div().size_full().flex().flex_col().bg(th_desk)
            // **一覧は窓の根に置きます**(2026-08-20。手順2)。編集の面の
            // 中に置くとリボンの高さぶん下から始まるので、押したボタンの
            // 真下に出せません。表の画面が 2026-08-15 に通った道と同じです
            .relative()
            .key_context("jo_doc")
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
                .child(div().flex_1().min_w(px(us * 0.0)).relative().overflow_hidden()
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
                    .children(proof_panel)
                    // 終了確認のパネル(窓の中の中央。rfd はスクリーン中央に出て遠い)
                    .children(self.quit_ask.then(|| {
                        let btn = |id: &'static str, label: String, primary: bool| {
                            div().id(id).px_3().py_1().rounded_sm().text_size(px(us * 12.5))
                                .border_1()
                                .border_color(if primary { rgb(0x165E83) } else { rgb(0xC6CDD3) })
                                .bg(if primary { rgb(0x165E83) } else { rgb(0xFFFFFF) })
                                .text_color(if primary { rgb(0xFFFFFF) } else { rgb(0x1B1B1B) })
                                .cursor_pointer()
                                .child(SharedString::from(label))
                        };
                        div().absolute().inset_0().flex().items_center().justify_center()
                            .child(div().w(px(us * 420.0)).p_3().rounded_md().bg(rgb(0xF7F9FA))
                                .border_1().border_color(rgb(0x165E83)).shadow_lg()
                                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation()
                                })
                                .flex().flex_col().gap_2()
                                .child(div().text_size(px(us * 13.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0x165E83))
                                    .child(ui::t!("there_unsaved_changes")))
                                .child(div().text_size(px(us * 12.0))
                                    .child(ui::t!(
                                        "save_quit_enter_save")))
                                .child(div().flex().flex_row().gap_2().justify_center()
                                    .child(btn("q-save", ui::t!("save_quit").to_string(), true)
                                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                                            |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.quit_ask = false;
                                                this.save(true, cx);
                                                cx.notify();
                                            })))
                                    .child(btn("q-drop", ui::t!("quit_without_saving").to_string(), false)
                                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                                            |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.release_lock();
                                                cx.quit();
                                            })))
                                    .child(btn("q-cancel", ui::t!("cancel").to_string(), false)
                                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                                            |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.quit_ask = false;
                                                this.status = ui::t!("quit_cancelled").into();
                                                cx.notify();
                                            })))))
                    }))
                    .child(InputSink { view: me })
                    .children(menu))
                .children(rp_panel)
            })
            // **文書のタブはステータスバーの上**(calc のシートのタブと同じ位置)
            .children(docs_bar)
            .children(self.show_statusbar.then_some(statusbar))
            // 窓の縁のつかみ(最後に描く = 最初にマウスを受ける)。
            // GNOME の Wayland は外枠を付けないので、これが無いと
            // 大きさを変えられない(calc と共通 — ui::resize_edges)
            // **一覧はいちばん最後に描きます。** 先に描くとリボンが上から
            // 塗ってしまい、真下に出したはずの一覧が隠れます
            // (2026-08-20 に実際にそうなった)
            .children(font_panel)
            .children(size_panel)
            .children(style_panel)
            .children(symbol_panel)
            .children(tbl_panel)
            .children(fl_panel)
            .children(user_font_panel)
            .children(date_panel)
            .children(export_panel)
            .children(ui::resize_edges(window))
    }
}

/// ホバーで出す小さな札。**絵だけの釦には要る** — 左右のパネルの柱は
/// アイコンしか出さないので、これが無いと何の面か分からない(2026-08-15)。
/// calc の `Tip` と同じ作り(2例から抽象は作らない — 部屋が別々のまま)
pub(crate) struct Tip(pub(crate) SharedString, pub(crate) f32);
impl gpui::Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        // 2つ目は画面の文字の大きさ(2026-08-21)。表の `Tip` と同じ形です
        let us = self.1;
        div().px_2().py_1().rounded_md()
            .bg(gpui::rgb(0x2B2F33)).text_color(gpui::rgb(0xF2F5F7))
            .text_size(px(us * 11.0))
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
            // Ctrl+クリックは図形を選択に足す・外す(表の画面と同じ)
            let ctrl = e.modifiers.control;
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
                        w.click_at_ctrl(f32::from(rel.x), f32::from(rel.y), shift, ctrl);
                        // Ctrl+クリックの後は引いても選択を伸ばしません。伸ばすと
                        // 図形の上で動いた拍子に、足した選択が1つに戻ります
                        w.drag_select = !ctrl;
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
                // **図形をつまんでいる間は図形が動きます**(2026-08-30)。
                // 本文の選択より先に見ないと、図形を掴んだまま字が選ばれます
                if w.shape_drag.is_some() {
                    let pxmm = PX_PER_MM * w.zoom;
                    let x = (f32::from(rel.x) - 28.0) / pxmm - w.pg.left_mm;
                    let y = (f32::from(rel.y) - 14.0) / pxmm + w.scroll_mm;
                    w.shape_move(x, y);
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
                w.shape_drag = None;
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
