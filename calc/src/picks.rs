//! main.rs からの純移動(2026-08-06 の分割)。挙動は変えない。

use crate::*;

impl Calc {
    /// 一覧を閉じる。**絞り込みの検索欄と選択も必ず一緒に片づける** —
    /// pick だけ消して pick_filter を残すと、次に開いた素の一覧へ古い打鍵が流れる。
    pub(crate) fn close_pick(&mut self) {
        self.pick = None;
        self.pick_note = None;
        self.pick_filter = None;
        self.pick_sel = 0;
    }

    /// 絞り込みつきの一覧([`ui::combo::Kind::Filter`])を開く。
    ///
    /// 書体・入力規則など、数が増える一覧に使う。打鍵は検索欄へ流れ、打つほど
    /// 絞られる。**開くとき今の値([`current`])の位置へ選択を送る**(絞り込み前の
    /// 全体の中の位置。合致が無ければ先頭)。素の一覧を開くときは pick を直に組む。
    pub(crate) fn open_combo(
        &mut self,
        kind: &'static str,
        items: Vec<(String, String)>,
        at: (f32, f32),
        current: &str,
    ) {
        let borrowed: Vec<(&str, &str)> =
            items.iter().map(|(k, l)| (k.as_str(), l.as_str())).collect();
        self.pick_sel = ui::combo::current_index(&borrowed, current);
        self.pick_kind = kind;
        self.pick_filter = Some(Editor::new(""));
        self.pick = Some((items, at));
    }

    /// いま画面に出ている(=絞り込み後の)一覧の項。検索欄が無ければ全部。
    /// **添字は絞り込み後の並び**で数える(↑↓・Enter・マウスがこれを見る)。
    pub(crate) fn pick_visible(&self) -> Vec<(String, String)> {
        let Some((items, _)) = &self.pick else { return Vec::new() };
        let Some(ed) = &self.pick_filter else { return items.clone() };
        let borrowed: Vec<(&str, &str)> =
            items.iter().map(|(k, l)| (k.as_str(), l.as_str())).collect();
        ui::combo::filter(&borrowed, ed.text())
            .into_iter()
            .map(|i| items[i].clone())
            .collect()
    }

    /// 絞り込みつきの一覧が開いていて、打鍵がその検索欄へ流れるべきか。
    pub(crate) fn pick_filtering(&self) -> bool {
        self.pick.is_some() && self.pick_filter.is_some()
    }

    /// 検索欄を打ち替えたときの後始末(選択を先頭へ戻す)。
    pub(crate) fn pick_filter_edited(&mut self) {
        self.pick_sel = 0;
    }

    /// 一覧の中の選択を1つ動かす(↑↓)。**絞り込み後の件数**で端を止める。
    pub(crate) fn pick_move(&mut self, down: bool) {
        let n = self.pick_visible().len();
        if n == 0 {
            self.pick_sel = 0;
            return;
        }
        self.pick_sel = if down {
            (self.pick_sel + 1).min(n - 1)
        } else {
            self.pick_sel.saturating_sub(1)
        };
    }

    /// いま選んでいる項を確定する(Enter)。
    ///
    /// 絞り込みつきで**一覧に合致が無い**ときは、打った字そのものを確定する
    /// (書体は一覧に無い名前も打てる・入力規則は規則側の決めに従う)。
    pub(crate) fn pick_confirm(&mut self, cx: &mut Context<Self>) {
        let vis = self.pick_visible();
        let chosen = if let Some((_, label)) = vis.get(self.pick_sel) {
            label.clone()
        } else if let Some(ed) = &self.pick_filter {
            // 合致なし — 打った字を確定(空なら何もしない)
            let t = ed.text().trim().to_string();
            if t.is_empty() {
                return;
            }
            t
        } else {
            return;
        };
        self.close_pick();
        self.apply_pick(&chosen, cx);
    }

    /// 一覧から選んだものを適用する(pick_kind で意味が変わる)。
    pub(crate) fn apply_pick(&mut self, v: &str, cx: &mut Context<Self>) {
        // 「✓ 」は「今これが効いている」、「☑ /☐ 」は入切の印
        // (値そのものではないので、ここで剥がしてから照合する)
        let v = v.strip_prefix("✓ ").unwrap_or(v);
        let v = v.strip_prefix("☑ ").or_else(|| v.strip_prefix("☐ ")).unwrap_or(v);
        match self.pick_kind {
            "font" => {
                let name = v.to_string();
                self.fmt(move |f| f.font = Some(name.clone()));
                // 最近使った書体を新しい順に最大12(recent_symbols と同じ運び)
                self.recent_fonts.retain(|x| x != v);
                self.recent_fonts.insert(0, v.to_string());
                self.recent_fonts.truncate(12);
                self.status = ui::tf!("font_set", v).into();
            }
            "size" => {
                if let Ok(pt) = v.parse::<f32>() {
                    // 自由入力(打った数)は 4〜409pt・0.5 刻みに黙って丸める。
                    // **丸めは画面の入力だけ** — 模型と Python の口には掛けない
                    // (round_size を通すのはここ=画面から入る道の1箇所)
                    let pt = ui::combo::round_size(pt);
                    self.fmt(move |f| f.size_c = Some((pt * 100.0) as u32));
                    self.status =
                        ui::tf!("font_size_set_pt", ui::combo::size_label(pt)).into();
                }
            }
            "symbol" => {
                // 打ちかけの続きに差し込む(セルを置き換えない)
                self.input.insert(v);
                self.dirty = true;
                // **次に同じ物を探させない。** 新しい順に最大12
                self.recent_symbols.retain(|x| x != v);
                self.recent_symbols.insert(0, v.to_string());
                self.recent_symbols.truncate(12);
                self.status = ui::tf!("inserted_enter_commits", v).into();
            }
            // 名前の適用範囲(2段目)。**ここで初めて名前を入れる** —
            // 途中でやめたら何も残らない
            "name-scope" => {
                let Some((name, range)) = self.name_new.take() else { return };
                let scoped = v == "sheet_only";
                let s = &mut self.book.sheets[self.active];
                s.names.retain(|d| d.name != name);
                s.names.push(kumihan::book::DefinedName {
                    name: name.clone(),
                    range: range.clone(),
                    scoped,
                });
                recalc_book(&mut self.book, self.active);
                self.dirty = true;
                self.status = if scoped {
                    ui::tf!("name_usable_sheet_only", name, range).into()
                } else {
                    ui::tf!("name_usable_any_sheet", name, range).into()
                };
            }
            // 分類を選んだ段。**2段目は1段目と同じ場所に重ねる**(目が飛ばない)
            "shape-cat" => {
                let items = shape_gallery(v);
                if items.is_empty() {
                    return;
                }
                let at = self
                    .pick
                    .as_ref()
                    .map(|(_, at)| *at)
                    .unwrap_or_else(|| self.pop_anchor());
                self.pick_kind = "shape";
                self.pick_note = Some(SharedString::from(shape_cat_label(v).to_string()));
                self.pick = Some((menu(&items), at));
                return; // pick_kind を戻さない(2段目へ)
            }
            "shape" => {
                // **鍵をそのまま文に差し込まない。** v は日本語の鍵なので、
                // 訳した文の中に日本語が残ってしまう(一覧は ui::item! で組んである)
                let (kind, name) = shape_kind(v);
                // 自由な形は**点を持って生まれる**(点が無いと何も描けない)。
                // 三角に置いて、そこからポイント編集で好きな形にする
                let pts = if kind == "path" {
                    vec![
                        kumihan::book::PathPoint::at(0.05, 0.9),
                        kumihan::book::PathPoint::at(0.5, 0.1),
                        kumihan::book::PathPoint::at(0.95, 0.9),
                    ]
                } else {
                    Vec::new()
                };
                self.checkpoint();
                let at = self.cursor;
                self.sheet_mut().shapes_new.push(kumihan::book::SheetShape {
                    at,
                    width_px: 160.0,
                    height_px: 100.0,
                    kind: kind.into(),
                    fill: None,
                    line: Some("1B6E3C".into()),
                    points: pts,
                    ..Default::default()
                });
                self.shape_sel = Some(self.sheet().shapes_new.len() - 1);
                self.dirty = true;
                self.status = ui::tf!("placed_drag_move_bottom", name, at.a1())
                .into();
            }
            "sa-cat" => {
                let cats = smartart();
                if let Some(ci) = cats.iter().position(|(k, _, _)| *k == v) {
                    self.sa_cat = ci;
                    let names: Vec<(String, String)> =
                        cats[ci].2.iter().map(|(k, l, _)| (k.to_string(), l.to_string())).collect();
                    // 2段目は1段目と同じ場所に重ねる(目が飛ばない)
                    let at = self.pick.as_ref().map(|(_, at)| *at)
                        .unwrap_or_else(|| self.pop_anchor());
                    self.pick_kind = "sa-item";
                    self.pick = Some((names, at));
                    // 鍵ではなく見出しを見せる(訳した文に日本語を混ぜない)
                    self.status = ui::tf!("smartart_pick_layout_inserted", cats[ci].1)
                    .into();
                    return; // pick_kind を "value" に戻さない(2段目へ)
                }
            }
            "sa-item" => {
                let cats = smartart();
                let hit = cats
                    .get(self.sa_cat)
                    .and_then(|(_, _, items)| items.iter().find(|(k, _, _)| *k == v));
                if let Some((name, _, key)) = hit {
                    let (name, key) = (name.to_string(), key.to_string());
                    self.insert_smartart(&name, &key);
                }
            }
            "scheme" => {
                if let Some((_, cols)) = kumihan::book::theme::SCHEMES.iter().find(|(n, _)| *n == v) {
                    self.checkpoint_book();
                    self.book.theme = cols.iter().map(|c| c.to_string()).collect();
                    // テーマ由来の色を持つセルを解き直す(配色に追従させる)
                    let theme = self.book.theme.clone();
                    let mut n = 0usize;
                    for sh in &mut self.book.sheets {
                        for cell in sh.cells.values_mut() {
                            if let Some((i, t)) = cell.fmt.color_theme {
                                cell.fmt.color =
                                    Some(kumihan::book::theme::resolve(&theme, i, t as f32 / 1000.0));
                                n += 1;
                            }
                            if let Some((i, t)) = cell.fmt.fill_theme {
                                cell.fmt.fill =
                                    Some(kumihan::book::theme::resolve(&theme, i, t as f32 / 1000.0));
                                n += 1;
                            }
                        }
                    }
                    self.dirty = true;
                    let label = crate::util::color_scheme_label(v);
                    self.status = ui::tf!("colour_scheme_applied_colours", label, n)
                    .into();
                }
            }
            // 直入力の補完: 打ちかけの名前を選んだ関数に置き換えて ( まで入れる
            "fn-complete" => {
                let t = self.input.text().to_string();
                let cur = self.input.cursor().min(t.len());
                let tok_len: usize = t[..cur]
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '.')
                    .map(|c| c.len_utf8())
                    .sum();
                let start = cur - tok_len;
                let mut t2 = t.clone();
                t2.replace_range(start..cur, &format!("{v}("));
                self.input = Editor::new(&t2);
                self.input.move_to(start + v.len() + 1, false);
                self.edit_armed = true;
                self.formula_assist();
            }
            "func-cat" => {
                let id = util::fn_group_cmd(v);
                self.run_cmd(id, cx);
            }
            // 保護中に許す操作。押すたびに入切して、一覧をその場で描き直す
            // (閉じない — 何個も入切したいので)
            "prot-allow" => {
                let name = v.trim_start_matches(['☑', '☐']).trim().to_string();
                self.commit();
                self.dirty = true;
                self.sheet_mut().protect_allow.toggle(&name);
                let a = self.sheet().protect_allow.clone();
                // 印は見出しだけに付ける(鍵は名前そのまま — sheet 側の照合に渡る)
                let items: Vec<(String, String)> = a
                    .items()
                    .iter()
                    .map(|(n, on)| {
                        let l = crate::util::protect_allow_label(n);
                        (n.to_string(), format!("{} {}", if *on { "☑" } else { "☐" }, l))
                    })
                    .collect();
                let at = self.pick.as_ref().map(|(_, at)| *at).unwrap_or_else(|| self.pop_anchor());
                self.pick_kind = "prot-allow";
                self.pick = Some((items, at));
                let on = a.items().iter().find(|(n, _)| *n == name).map(|(_, o)| *o);
                let label = crate::util::protect_allow_label(&name);
                self.status = match on {
                    Some(true) => ui::tf!("allowed", label).into(),
                    _ => ui::tf!("forbade", label).into(),
                };
                return; // pick_kind を "value" に戻さない(続けて入切する)
            }
            // 名前を式へ差し込む。**打っている所に入れる**(末尾ではない)。
            // まだ式を始めていなければ「=」から始めてあげる
            // 記号: 組を選ぶ → その組の字を一字ずつ並べ直す
            "symbol-group" => {
                if v.starts_with("type_unicode") {
                    self.prompt = Some(("symbol-hex", Editor::new("")));
                    return;
                }
                // 鍵は `symbols:recent` か `symbols:組の名`。字の並びは組から引き直す
                let chars: String = if v == "symbols:recent" {
                    self.recent_symbols.join(" ")
                } else {
                    let k = v.strip_prefix("symbols:").unwrap_or(v);
                    symbol_groups()
                        .iter()
                        .find(|(key, _, _)| *key == k)
                        .map(|(_, _, c)| (*c).to_string())
                        .unwrap_or_default()
                };
                let at = self.pick.as_ref().map(|(_, a)| *a).unwrap_or_else(|| self.pop_anchor());
                self.pick_kind = "symbol";
                self.pick_note = Some(ui::t!("pick_character_put_formula").into());
                self.pick = Some((
                    // 字そのものの一覧 — 訳す物ではない
                    plain(chars.split_whitespace().collect::<String>().chars()
                        .map(|c| c.to_string())),
                    at,
                ));
                return; // 2段目へ(閉じない)
            }
            "paste-name" => {
                // 鍵は `name:名前` か `table:名前` — 名前は鍵から取り出す
                let name = v
                    .strip_prefix("name:")
                    .or_else(|| v.strip_prefix("table:"))
                    .unwrap_or(v)
                    .to_string();
                if self.input.text().is_empty() {
                    self.input = Editor::new("=");
                }
                self.input.insert(&name);
                self.edit_armed = true;
                self.status = ui::tf!("put_formula", name).into();
            }
            // **書き出す形を選ぶ**(手引き
            // `docs/ja/commands/ファイル/エクスポート.adoc`)。
            // 形ごとに、もとからある書き出しの道へ渡します
            "f-export" => match v {
                "xlsx" => self.run_cmd("f-saveas", cx),
                "csv" => self.export_csv_dialog(cx),
                "html" => self.export_html_dialog(cx),
                _ => self.run_cmd("pdf", cx),
            },
            "csv-kind" => {
                // ✓ は見出しだけに付く — 鍵はそのまま引き当てられる
                if let Some((k, l, _)) = Calc::csv_kinds().iter().find(|(k, _, _)| *k == v) {
                    self.csv_kind = k;
                    self.status =
                        ui::tf!("csv_written_now", l).into();
                }
            }
            // 自動復旧: 控えを開く。**原本ではなく控えを開く** — 中身を見て、
            // よければ名前を付けて保存する(黙って原本へ戻さない)
            "recover" => {
                let Some((name, path)) = self.pick_paths.iter().find(|(n, _)| n == v).cloned()
                else {
                    return;
                };
                self.open(path.clone());
                // 控えを原本と取り違えないよう、道は持たせない
                self.set_path(None);
                self.dirty = true;
                self.status = ui::tf!(
                    "opened_backup_original_check",
                    name
                )
                .into();
            }
            "recover-every" => {
                // **鍵をそのまま文に差し込まない。** v は日本語の鍵なので、
                // 訳した文の中に日本語が残ってしまう(zh-tw の訳者が見つけた)
                let (secs, every) = match v {
                    "every_minute" => (60, ui::t!("every_minute")),
                    "every_5_minutes" => (300, ui::t!("every_5_minutes")),
                    "every_10_minutes" => (600, ui::t!("every_10_minutes")),
                    _ => (0, ""),
                };
                self.recover_secs = secs;
                self.status = if self.recover_secs == 0 {
                    ui::t!("not_keeping_auto_recovery").into()
                } else {
                    ui::tf!("backing_up_every_original", every).into()
                };
            }
            // 改ページの3択(横 / 縦 / すべて外す)
            "pagebreak" => {
                self.commit();
                self.checkpoint();
                let (r, c) = (self.cursor.row, self.cursor.col);
                let cn = col_name(c);
                let sh = self.sheet_mut();
                if v == "pagebreak:all" {
                    let n = sh.row_breaks.len() + sh.col_breaks.len();
                    sh.row_breaks.clear();
                    sh.col_breaks.clear();
                    self.status = ui::tf!("removed_all_page_breaks", n).into();
                } else if v == "pagebreak:col" {
                    if let Some(i) = sh.col_breaks.iter().position(|b| *b == c) {
                        sh.col_breaks.remove(i);
                        self.status = ui::tf!("removed_page_break_column", cn).into();
                    } else if c == 0 {
                        self.undo_stack.pop();
                        self.status = ui::t!("nothing_break_before_column").into();
                        return;
                    } else {
                        sh.col_breaks.push(c);
                        self.status = ui::tf!("new_sheet_paper_column", cn).into();
                    }
                } else if let Some(i) = sh.row_breaks.iter().position(|b| *b == r) {
                    sh.row_breaks.remove(i);
                    self.status = ui::tf!("removed_page_break_row", r + 1).into();
                } else if r == 0 {
                    self.undo_stack.pop();
                    self.status = ui::t!("no_page_break_before").into();
                    return;
                } else {
                    sh.row_breaks.push(r);
                    self.status = ui::tf!("new_sheet_paper_row", r + 1).into();
                }
                self.dirty = true;
            }
            // 紙 N 枚に収める
            "fit-pages" => {
                self.commit();
                self.checkpoint();
                let sh = self.sheet_mut();
                // **鍵をそのまま文に差し込まない。** 見出しを一緒に持って回る
                let (w, h, label) = match v {
                    "fit_all_columns_one" => {
                        (Some(1), None, ui::t!("fit_all_columns_one"))
                    }
                    "fit_all_rows_one" => {
                        (None, Some(1), ui::t!("fit_all_rows_one"))
                    }
                    "fit_sheet_one_page" => (Some(1), Some(1), ui::t!("fit_sheet_one_page")),
                    "2_pages_wide_1" => {
                        (Some(2), Some(1), ui::t!("2_pages_wide_1"))
                    }
                    _ => (None, None, ""),
                };
                sh.fit_to_w = w;
                sh.fit_to_h = h;
                self.dirty = true;
                self.status = if w.is_none() && h.is_none() {
                    ui::t!("no_longer_fitting_paper").into()
                } else {
                    ui::tf!("set_applies_pdf_saving", label).into()
                };
            }
            // コメントの見せ方を選んだ(2026-08-13)。一覧の板と、セルに出る
            // 付記は別の物 — 片方だけ切っても、もう片方は残る
            "comment-show" => match v {
                "list" => {
                    self.comment_list = match self.comment_list {
                        Some(_) => None,
                        None => Some(CommentList::default()),
                    };
                    self.status = if self.comment_list.is_some() {
                        ui::t!("comment_list_opened_whole").into()
                    } else {
                        ui::t!("comment_list_closed").into()
                    };
                }
                _ => {
                    self.show_comments = !self.show_comments;
                    self.status = if self.show_comments {
                        ui::t!("comments_shown").into()
                    } else {
                        ui::t!("comments_hidden_still_there").into()
                    };
                }
            },
            // どこまで消すかを選んだ(本家の「現在/自分/すべて」)
            "comment-del" => self.delete_comments(v),
            // 表のスタイルを選んだ(2026-08-12、台帳「テンプレート選択ギャラリー」)
            // スライサーにする列を選んだ(2026-08-13)
            "slicer-col" => {
                let Some(col) = (0..256).find(|c| crate::col_name(*c) == v) else { return };
                // 同じ列をもう一度選んだら**その板を閉じる**(☑ を外す)
                if let Some(i) = self.slicers.iter().position(|s| s.col == col) {
                    self.slicers.remove(i);
                    self.slicer_sel = self.slicer_sel.min(self.slicers.len().saturating_sub(1));
                    self.slicer_cfg &= !self.slicers.is_empty();
                    self.status = ui::tf!("closed_slicer_column", crate::col_name(col)).into();
                    return;
                }
                self.slicers.push(Slicer { col, ..Default::default() });
                self.slicer_sel = self.slicers.len() - 1;
                self.status = ui::tf!(
                    "slicer_tap_value_column",
                    crate::col_name(col)
                )
                .into();
            }
            "table-style" => {
                if let Some((_, label, st)) =
                    crate::util::table_styles().iter().find(|(k, _, _)| *k == v)
                {
                    let (st, label) = (*st, *label);
                    self.make_table(st, Some(label));
                }
            }
            "cell-style" => {
                if let Some((_, label, f)) = cell_styles().iter().find(|(k, _, _)| *k == v) {
                    let (f, label) = (*f, *label);
                    self.fmt(f);
                    // 鍵ではなく見出し(訳した文に日本語を混ぜない)
                    self.status = ui::tf!("cell_style_applied", label).into();
                }
            }
            // **マクロの一覧から選んだ .py は走る。** 打たずに選べる道 —
            // 日本語の名前は IME を挟むので `@名前` の Enter が変換に
            // 食われて辿り着けないことがある(2026-08-09 の理由がまだ生きている)
            "py-run" => {
                if let Some((name, _)) = self.pick_paths.iter().find(|(n, _)| n == v).cloned() {
                    // 一覧から選んで走らせた分も記録に残す(2026-08-16)
                    self.rec_line(format!("xw.run_macro({name:?})"));
                    self.run_plugin(&name, None, cx);
                }
                self.pick_paths.clear();
            }
            // Python タブの一覧から選んだ .py(打たずに選べる道)
            // 一覧から選んだ .py は**編集の道具で開く**(発注者 2026-08-15。
            // プログラムの編集は表計算の仕事ではない)。順は
            // settings の editor → 隣の writer → 機械の既定
            // リボンのマクロの一覧。3種が混ざる — 名乗ったマクロ(編集)、
            // 見本を作る、同梱の台本を読む(2026-08-16)
            "ribbon-macro" => {
                let dir = pyrun::ribbon_dir();
                if let Some(n) = v.strip_prefix('\u{2}') {
                    // **同梱の台本は控えを書き出して開く。** 直しても効かない —
                    // 契約が違う(指図を受け取ってブックに触らない純関数)ので、
                    // 置き場に移しても同じようには走らない
                    let Some((_, src)) = pyrun::BUNDLED.iter().find(|(k, _)| *k == n) else {
                        return;
                    };
                    let out = std::env::temp_dir().join(format!("officework-同梱-{n}.py"));
                    self.status = match std::fs::write(&out, src)
                        .map_err(|e| e.to_string())
                        .and_then(|_| ui::open_for_edit(&out.display().to_string()))
                    {
                        Ok(tool) => ui::tf!(
                            "opened_copy_read_only",
                            n,
                            tool
                        )
                        .into(),
                        Err(e) => ui::tf!("cant_open", e).into(),
                    };
                } else if v == "\u{1}見本" {
                    // 見本は**そのまま動く物**を書く。動かない見本は、動かない
                    // ことに気づくまでの時間を丸ごと無駄にする
                    let _ = std::fs::create_dir_all(&dir);
                    let mut path = dir.join("見本.py");
                    let mut i = 2;
                    while path.exists() {
                        path = dir.join(format!("見本{i}.py"));
                        i += 1;
                    }
                    let src = "リボン = {\"札\": \"見本\", \"絵\": \"py-run\", \"段\": \"マクロ\"}\n\
                               \n\
                               from officework import calc as xw\n\
                               \n\
                               s = xw.Book.attach().sheets.active\n\
                               s[\"A1\"].value = \"見本が走りました\"\n";
                    self.status = match std::fs::write(&path, src)
                        .map_err(|e| e.to_string())
                        .and_then(|_| ui::open_for_edit(&path.display().to_string()))
                    {
                        Ok(tool) => {
                            ui::tf!("created_opened", path.display().to_string(), tool)
                                .into()
                        }
                        Err(e) => ui::tf!("cant_open", e).into(),
                    };
                } else if let Some((_, path)) = self.pick_paths.iter().find(|(n, _)| n == v).cloned()
                {
                    self.status = match ui::open_for_edit(&path.display().to_string()) {
                        Ok(tool) => ui::tf!("opened", v, tool).into(),
                        Err(e) => ui::tf!("cant_open", e).into(),
                    };
                }
                self.pick_paths.clear();
            }
            "py-edit" => {
                if let Some((_, path)) = self.pick_paths.iter().find(|(n, _)| n == v).cloned() {
                    self.status = match ui::open_for_edit(&path.display().to_string()) {
                        Ok(tool) => ui::tf!("opened", v, tool).into(),
                        Err(e) => ui::tf!("cant_open", e).into(),
                    };
                }
                self.pick_paths.clear();
            }
            "unhide" => {
                if let Some((_, path)) = self.pick_paths.iter().find(|(n, _)| n == v).cloned() {
                    if let Ok(i) = path.to_string_lossy().parse::<usize>() {
                        if i < self.book.sheets.len() {
                            self.checkpoint_book();
                            self.book.sheets[i].hidden = false;
                            self.switch_sheet(i);
                            self.dirty = true;
                            self.status = ui::tf!("sheet_unhidden", v).into();
                        }
                    }
                }
                self.pick_paths.clear();
            }
            "freeze" => {
                match v {
                    "unfreeze" => {
                        self.frozen = None;
                        self.status = ui::t!("panes_unfrozen").into();
                    }
                    "freeze_top_row" => {
                        self.split = None;
                        self.frozen = Some(Pos::new(1, 0));
                        self.status = ui::t!("top_row_frozen").into();
                    }
                    "freeze_first_column" => {
                        self.split = None;
                        self.frozen = Some(Pos::new(0, 1));
                        self.status = ui::t!("first_column_frozen").into();
                    }
                    "shadow_frozen_edge" => {
                        self.freeze_shadow = !self.freeze_shadow;
                        self.status = if self.freeze_shadow {
                            ui::t!("frozen_panes_get_shadow").into()
                        } else {
                            ui::t!("frozen_pane_shadow_removed").into()
                        };
                    }
                    _ => {
                        // いまの位置で固定(その上と左が留まる)
                        if self.cursor.row == 0 && self.cursor.col == 0 {
                            self.status = ui::t!("put_cursor_where_panes").into();
                        } else {
                            // 固定と分割は同時に立てません(帯が二重になります)
                            self.split = None;
                            self.frozen = Some(self.cursor);
                            self.status = ui::tf!("frozen_row_column", self.cursor.row, self.cursor.col).into();
                        }
                    }
                }
            }
            // ピボットの聞き取り(クリックで入切 → 決定で次へ)。
            // 行 → 列 → 値 → 集計の4段。Esc でいつでもやめられる
            // 罫線: 辺の選択(ペンの線種・色で掛ける)
            // 名前マネージャー: 名前を選ぶ → 移動/打ち直し/削除
            "names-pick" => {
                if v.starts_with("new_name_current_selection") {
                    self.prompt = Some(("name", Editor::new("")));
                    return; // パネルの確定まで
                }
                // 鍵は `name:名前` か `table:名前` — どちらかは鍵の頭で分かる
                if let Some(name) = v.strip_prefix("table:") {
                    // テーブル名は表オブジェクトの持ち物 — ここでは消さない
                    let hit = self
                        .sheet()
                        .tables
                        .iter()
                        .find(|t| t.name == name)
                        .map(|t| (t.a, t.b));
                    if let Some((a, b)) = hit {
                        self.anchor = Some(a);
                        self.cursor = b;
                        self.sync_input();
                        self.status = ui::t!("moved_table_rename_delete").into();
                    }
                    return;
                }
                let name = v.strip_prefix("name:").unwrap_or(v).to_string();
                if self.sheet().names.iter().any(|d| d.name == name) {
                    let at = self.pop_anchor();
                    self.name_pend = Some(name.clone());
                    self.pick_note = Some(ui::tf!("what", name).into());
                    self.pick_kind = "name-act-pick";
                    self.pick = Some((
                        menu(&[
                            ui::item!("go_there"),
                            ui::item!("retype_contents"),
                            ui::item!("delete_name"),
                        ]),
                        at,
                    ));
                    return;
                }
            }
            "name-act-pick" => {
                let Some(name) = self.name_pend.take() else { return };
                let range = self
                    .sheet()
                    .names
                    .iter()
                    .find(|d| d.name == name)
                    .map(|d| d.range.clone())
                    .unwrap_or_default();
                match v {
                    "go_there" => {
                        let mut it = range.split(':');
                        let a = it.next().and_then(Pos::parse);
                        let b = it.next().and_then(Pos::parse);
                        if let Some(a) = a {
                            self.anchor = b.map(|_| a);
                            self.cursor = b.unwrap_or(a);
                            if b.is_some() {
                                self.anchor = Some(a);
                            }
                            self.sync_input();
                            self.status = ui::tf!("moved_2", name, range).into();
                        } else {
                            self.status = ui::tf!("content_not_readable_location", name, range).into();
                        }
                    }
                    "retype_contents" => {
                        self.name_pend = Some(name);
                        self.prompt = Some(("name-range", Editor::new(&range)));
                        return; // パネルの確定まで
                    }
                    _ => {
                        // 名前を消す
                        self.checkpoint();
                        self.book.sheets[self.active].names.retain(|d| d.name != name);
                        recalc_book(&mut self.book, self.active);
                        self.dirty = true;
                        self.status = ui::tf!("name_deleted_formulas_becomes", name, name).into();
                    }
                }
            }
            // ヘッダー/フッター: 6つの区分から選んでパネルで打つ
            "hf-pick" => {
                if v == "clear_all" {
                    self.checkpoint();
                    self.sheet_mut().header = None;
                    self.sheet_mut().footer = None;
                    self.dirty = true;
                    self.status = ui::t!("header_footer_removed").into();
                } else {
                    let name = v.split(':').next().unwrap_or(v).trim();
                    let (footer, slot) = match name {
                        "header_left" => (false, 0u8),
                        "header_centre" => (false, 1),
                        "header_right" => (false, 2),
                        "footer_left" => (true, 0),
                        "footer_centre" => (true, 1),
                        _ => (true, 2),
                    };
                    let raw = if footer {
                        self.sheet().footer.clone()
                    } else {
                        self.sheet().header.clone()
                    };
                    let (l, c, r) = kumihan::book::hf_split(raw.as_deref().unwrap_or(""));
                    let cur = match slot { 0 => l, 1 => c, _ => r };
                    self.hf_pend = Some((footer, slot));
                    self.prompt = Some(("hf-edit", Editor::new(&cur)));
                    return; // パネルの確定まで
                }
            }
            "border-pick" => {
                match v {
                    "→ 線のスタイル…" => {
                        let at = self.pop_anchor();
                        // 「✓ 」は今のペンの印 — 見出しにだけ付ける(鍵は素のまま)
                        let items: Vec<(String, String)> = border_styles()
                            .iter()
                            .map(|(k, l, b)| {
                                let label = if *b == self.pen_style {
                                    format!("✓ {l}")
                                } else {
                                    l.to_string()
                                };
                                (k.to_string(), label)
                            })
                            .collect();
                        self.pick_note = Some(ui::t!("line_style_goes_into").into());
                        self.pick_kind = "border-style-pick";
                        self.pick = Some((items, at));
                        return;
                    }
                    "→ 線の色…" => {
                        let at = self.pop_anchor();
                        let mut items: Vec<(String, String)> = font_colors()
                            .iter()
                            .map(|(k, l, _)| (k.to_string(), l.to_string()))
                            .collect();
                        items.extend(menu(&[ui::item!("other_type_rrggbb")]));
                        self.pick_note = Some(ui::t!("line_colour_goes_into").into());
                        self.pick_kind = "border-color-pick";
                        self.pick = Some((items, at));
                        return;
                    }
                    _ => {
                        self.apply_borders(v);
                        // パネルは開いたまま — 外枠→内側…と連打で組み立てる
                        // (閉じるのは Esc かパネルの外。発注者報告 2026-08-08)
                        self.run_cmd("borders", cx);
                        return;
                    }
                }
            }
            "border-style-pick" => {
                if let Some((_, label, b)) = border_styles().iter().find(|(k, _, _)| *k == v) {
                    self.pen_style = *b;
                    // 鍵ではなく見出し(訳した文に日本語を混ぜない)
                    self.status =
                        ui::tf!("line_style_applies_when", label).into();
                }
            }
            "border-color-pick" => {
                if v.starts_with("other") {
                    self.prompt = Some(("border-color-rgb", Editor::new("")));
                    return; // パネルの確定まで pick_kind を戻さない
                }
                if let Some((_, label, hx)) = font_colors().iter().find(|(k, _, _)| *k == v) {
                    self.pen_color =
                        hx.and_then(|h| u32::from_str_radix(h, 16).ok());
                    // 鍵ではなく見出し(訳した文に日本語を混ぜない)
                    self.status =
                        ui::tf!("line_colour_applies_when", label).into();
                }
            }
            // 変更履歴の一覧。選んだらその場所へ跳ぶ(戻す機能ではない)
            "changes-pick" => {
                // 「日時 シート!A1 …」の形からシート名と番地を取る
                if let Some(tok) = v.split_whitespace().nth(2) {
                    if let Some((sh, a1)) = tok.rsplit_once('!') {
                        if let Some(i) = self.book.sheets.iter().position(|s| s.name == sh) {
                            self.active = i;
                        }
                        if let Some(p) = Pos::parse(a1) {
                            self.anchor = None;
                            self.cursor = p;
                            self.follow();
                            self.sync_input();
                            self.status = ui::tf!("jumped_2", tok).into();
                        }
                    }
                }
            }
            "pivot-sort-pick" => {
                if let Some(i) = self.pivot_at(self.cursor).or_else(|| self.pivot_flt.as_ref().map(|(p, _, _)| *p)) {
                    let so = match v {
                        "labels_z" | "labels_z_2" | "largest_value_first" | "smallest_value_first" => {
                            v.to_string()
                        }
                        _ => String::new(), // そのまま
                    };
                    if let Some(d) = self.book.pivots.get_mut(i) {
                        d.sort = so;
                        let nd = d.clone();
                        self.spawn_pivot(nd, Some(i), cx);
                    }
                }
            }
            "pivot-showas-pick" => {
                if let Some(i) = self.pivot_at(self.cursor) {
                    let sa = match v {
                        "total" | "running_total" | "difference" => v.to_string(),
                        _ => String::new(), // そのまま
                    };
                    if let Some(d) = self.book.pivots.get_mut(i) {
                        d.show_as = sa.clone();
                        // 累計と差は積み上げなので、小計・総計を落とす
                        // (途中に総計が挟まると読み違えるため)
                        if sa == "running_total" || sa == "difference" {
                            d.totals = false;
                            d.subtotals = false;
                        }
                        let nd = d.clone();
                        self.spawn_pivot(nd, Some(i), cx);
                    }
                }
            }
            "pivot-style-pick" => {
                if let Some(i) = self.pivot_at(self.cursor) {
                    let style = match v {
                        "green" | "orange" | "grey" => v.to_string(),
                        _ => String::new(), // 青(既定)
                    };
                    if let Some(d) = self.book.pivots.get_mut(i) {
                        d.style = style;
                        let nd = d.clone();
                        self.spawn_pivot(nd, Some(i), cx);
                    }
                }
            }
            "pivot-filter-pick" => {
                if v == "show_everything_again" {
                    if let Some((pi, field, _)) = self.pivot_flt.take() {
                        if let Some(d) = self.book.pivots.get_mut(pi) {
                            d.hide.retain(|(f, _)| *f != field);
                            let nd = d.clone();
                            self.spawn_pivot(nd, Some(pi), cx);
                        }
                    }
                    return;
                }
                if v == "filter_label" {
                    // 含む/で始まる/で終わる 語 — 合う値以外を hide に落とす
                    self.prompt = Some(("pivot-label", Editor::new("")));
                    return; // pivot_flt はパネルの確定まで持つ
                }
                if v == "filter_value" {
                    let cur = self
                        .pivot_flt
                        .as_ref()
                        .and_then(|(pi, _, _)| self.book.pivots.get(*pi))
                        .and_then(|d| d.vfilter.as_ref())
                        .map(|(op, th)| format!("{op} {th}"))
                        .unwrap_or_default();
                    self.prompt = Some(("pivot-vfilter", Editor::new(&cur)));
                    return;
                }
                if v == "sort" {
                    let at = self.pop_anchor();
                    let Some(pi) = self.pivot_flt.as_ref().map(|(p, _, _)| *p) else { return };
                    // **小計・空行を出している間は掛けない。** 区切りの塊の
                    // 中身を並べ替えると、区切りと中身の対応が崩れる。
                    // 黙って崩さずに、できない理由を言う(2026-08-13)
                    if self.book.pivots.get(pi).is_some_and(|d| d.subtotals || d.blank_rows) {
                        self.status = ui::t!(
                            "cannot_sort_while_subtotals"
                        )
                        .into();
                        return;
                    }
                    let cur = self.book.pivots.get(pi).map(|d| d.sort.clone()).unwrap_or_default();
                    let items: Vec<(String, String)> = [
                        // **「そのまま」を流用しない。** あちらは計算の種類の
                        // 選択肢で、全言語で「計算しない」と訳されている —
                        // 並べ替えの一覧に置くと意味が変わる(2026-08-13、
                        // 訳す人が気づいた)
                        ui::item!("not_sort"),
                        ui::item!("labels_z"),
                        ui::item!("labels_z_2"),
                        ui::item!("largest_value_first"),
                        ui::item!("smallest_value_first"),
                    ]
                    .iter()
                    .map(|(k, l)| {
                        let key = if *k == "not_sort" { "" } else { *k };
                        (k.to_string(), if key == cur { format!("✓ {l}") } else { l.to_string() })
                    })
                    .collect();
                    self.pick_note = Some(ui::t!("sort_value_means_leftmost").into());
                    self.pick_kind = "pivot-sort-pick";
                    self.pick = Some((items, at));
                    return;
                }
                if v == "group" {
                    let at = self.pop_anchor();
                    let field = self.pivot_flt.as_ref().map(|(_, f, _)| f.clone()).unwrap_or_default();
                    self.pick_note =
                        Some(ui::tf!("group_pick_unit", field).into());
                    self.pick_kind = "pivot-group-pick";
                    self.pick = Some((
                        menu(&[
                            ui::item!("months"),
                            ui::item!("quarters"),
                            ui::item!("years"),
                            ui::item!("number_range"),
                            ui::item!("ungroup"),
                        ]),
                        at,
                    ));
                    return;
                }
                if v == "apply_filter" {
                    let Some((pi, field, hidden)) = self.pivot_flt.take() else { return };
                    if let Some(d) = self.book.pivots.get_mut(pi) {
                        d.hide.retain(|(f, _)| *f != field);
                        if !hidden.is_empty() {
                            d.hide.push((field, hidden.into_iter().collect()));
                        }
                        let nd = d.clone();
                        self.spawn_pivot(nd, Some(pi), cx);
                    }
                    return;
                }
                if let Some((_, _, hidden)) = &mut self.pivot_flt {
                    let v = v.to_string();
                    if !hidden.remove(&v) {
                        hidden.insert(v);
                    }
                }
                self.pivot_filter_pick();
                return;
            }
            "cond-manage-pick" => {
                let Some(i) = v.split(')').next().and_then(|n| n.trim().parse::<usize>().ok())
                else { return };
                let i = i - 1;
                if i >= self.book.sheets[self.active].cond.len() { return }
                self.cond_pend = Some(i);
                let at = self.pop_anchor();
                self.pick_note = Some(ui::tf!("what_rule", i + 1).into());
                self.pick_kind = "cond-act-pick";
                self.pick = Some((
                    menu(&[ui::item!("go_there"), ui::item!("delete_rule")]),
                    at,
                ));
                return;
            }
            "cond-act-pick" => {
                let Some(i) = self.cond_pend.take() else { return };
                let Some(rule) = self.book.sheets[self.active].cond.get(i).cloned() else {
                    return;
                };
                if v == "go_there" {
                    let (a, b) = rule.range;
                    self.anchor = Some(a);
                    self.cursor = b;
                    self.sync_input();
                    self.status = ui::tf!("jumped", a.a1(), b.a1()).into();
                } else {
                    self.checkpoint();
                    self.book.sheets[self.active].cond.remove(i);
                    self.dirty = true;
                    self.status = ui::tf!(
                        "removed_rule",
                        rule.range.0.a1(), rule.range.1.a1(), cond_kind_name(&rule.kind)
                    )
                    .into();
                }
            }
            "csv-import-pick" => {
                if v.starts_with("→ ") {
                    // 取り込む(下ごしらえ済みの grid を流し込む)
                    let Some(pend) = self.import_pend.take() else { return };
                    if pend.grid.is_empty() {
                        self.status = ui::t!("no_rows_read_check").into();
                        self.import_pend = Some(pend);
                        self.import_pick();
                        return;
                    }
                    self.checkpoint();
                    let n_rows = pend.grid.len();
                    let n = crate::util::paste_values_text(
                        &mut self.book.sheets[self.active],
                        pend.dest,
                        &pend.grid,
                    );
                    recalc_book(&mut self.book, self.active);
                    self.dirty = true;
                    self.sync_input();
                    self.status = ui::tf!(
                        "imported_rows_fields_values",
                        n_rows, n, pend.dest.a1()
                    )
                    .into();
                    return;
                }
                // PDF: 表を次のものへ回す
                let table_head = format!("{}: ", ui::t!("table_2"));
                if v.starts_with(&table_head) {
                    if let Some(p) = &mut self.import_pend {
                        if !p.pdf.is_empty() {
                            p.pdf_at = (p.pdf_at + 1) % p.pdf.len();
                            p.grid = p.pdf[p.pdf_at].2.clone();
                        }
                    }
                    self.import_pick();
                    return;
                }
                let enc_head = format!("{}: ", ui::t!("encoding"));
                let delim_head = format!("{}: ", ui::t!("delimiter"));
                let dest_head = format!("{}: ", ui::t!("destination"));
                if v.starts_with(&enc_head) {
                    if let Some(pend) = &mut self.import_pend {
                        pend.enc = (pend.enc + 1) % crate::py::import_encs().len();
                    }
                    self.import_reparse(cx);
                    return;
                }
                if v.starts_with(&delim_head) {
                    let mut ask_custom = false;
                    if let Some(pend) = &mut self.import_pend {
                        pend.delim = (pend.delim + 1) % crate::py::import_delims().len();
                        // 実体は3つ目へ移った(1つ目=鍵, 2つ目=見出し)
                        ask_custom =
                            crate::py::import_delims()[pend.delim].2 == "other_2";
                    }
                    if ask_custom {
                        self.prompt = Some(("csv-delim", Editor::new("")));
                    } else {
                        self.import_reparse(cx);
                    }
                    return;
                }
                if v.starts_with(&dest_head) {
                    let cur = self
                        .import_pend
                        .as_ref()
                        .map(|p| p.dest.a1())
                        .unwrap_or_default();
                    self.prompt = Some(("csv-dest", Editor::new(&cur)));
                    return;
                }
                // プレビューの行は何もしない(パネルは開いたまま)
                self.import_pick();
                return;
            }
            "spark-kind-pick" => {
                let kind = match v {
                    "column_2" => "spark-col",
                    "win_loss" => "spark-wl",
                    _ => "spark",
                };
                self.insert_sparkline(kind);
            }
            "dedup-pick" => {
                let header_label = ui::t!("first_row_header_keep").to_string();
                if v == format!("→ {}", ui::t!("delete")) {
                    let Some((list, header)) = self.dedup_pend.take() else { return };
                    let cols: Vec<u32> =
                        list.iter().filter(|(_, _, on)| *on).map(|(c, _, _)| *c).collect();
                    if cols.is_empty() {
                        self.status = ui::t!("pick_least_one_column").into();
                        self.dedup_pend = Some((list, header));
                        self.dedup_pick();
                        return;
                    }
                    self.checkpoint();
                    let all = cols.len() == list.len();
                    let n = self.book.sheets[self.active]
                        .remove_duplicate_rows_in(header, if all { &[] } else { &cols });
                    self.dirty = true;
                    recalc_book(&mut self.book, self.active);
                    self.sync_input();
                    // 何件消したかを黙らない
                    self.status = ui::tf!("duplicate_rows_removed", n).into();
                } else if let Some((list, header)) = &mut self.dedup_pend {
                    if v == header_label {
                        *header = !*header;
                    } else if let Some(item) = list.iter_mut().find(|(_, n, _)| *n == v) {
                        item.2 = !item.2;
                    }
                    self.dedup_pick();
                    return;
                }
            }
            "pivot-group-pick" => {
                let Some((pi, field, _)) = self.pivot_flt.clone() else { return };
                if v == "number_range" {
                    self.prompt = Some(("pivot-group-width", Editor::new("")));
                    return;
                }
                let Some(d) = self.book.pivots.get_mut(pi) else { return };
                d.group_by.retain(|(f, _)| *f != field);
                if v != "ungroup" {
                    d.group_by.push((field, v.to_string()));
                }
                let nd = d.clone();
                self.pivot_flt = None;
                self.spawn_pivot(nd, Some(pi), cx);
            }
            // **壊れたブックの逃げ道**(開いて修復。2026-08-22)
            "repair" => {
                let Some((path, bytes)) = self.repair_pend.take() else { return };
                // **矢印では分けられません**(2026-08-26)。鍵が記号に
                // なったので、拾う操作の鍵と名指しで比べます。控えの名前は
                // ファイル名なので、この鍵と重なりません
                if v != "salvage_what_can_read" {
                    // 控えから開く。**元のファイルは触りません**
                    let record = ops::history::list(Some(&path));
                    if let Some((_, q)) = record.into_iter().find(|(n, _)| *n == v) {
                        self.open_version(&q);
                    }
                    return;
                }
                let s = sheet::xlsx::salvage(&bytes);
                if !s.any() {
                    self.status =
                        ui::t!("nothing_salvaged_no_part").into();
                    return;
                }
                match sheet::xlsx::read(std::io::Cursor::new(s.bytes)) {
                    Ok((mut book, _rep)) => {
                        kumihan::calc::recalc_all(&mut book);
                        // **読めなかった部品を一件ずつ並べます。**
                        // 「修復しました」だけでは、どこに穴が空いたか分かりません
                        let mut notes: Vec<gpui::SharedString> = s
                            .lost
                            .iter()
                            .map(|(n, why)| {
                                gpui::SharedString::from(
                                    ui::tf!("not_read_3", n.clone(), why.clone())
                                        .to_string(),
                                )
                            })
                            .collect();
                        if notes.is_empty() {
                            notes.push(gpui::SharedString::from(
                                ui::t!("every_part_salvaged_only")
                                    .to_string(),
                            ));
                        }
                        let status = ui::tf!(
                            "opened_salvaged_parts_not",
                            s.kept.len(),
                            s.lost.len()
                        )
                        .to_string();
                        self.adopt_salvaged(path, book, notes, status);
                    }
                    Err(e) => {
                        self.status =
                            ui::tf!("not_open_even_salvaged", e).into()
                    }
                }
            }
            // シナリオ。名前を押すとその値を書き戻す
            "scenario" => {
                if v.starts_with("new_scenario_current_values") {
                    let (a, b) = self.sel_rect();
                    let n = (b.row - a.row + 1) as usize * (b.col - a.col + 1) as usize;
                    if n > 64 {
                        self.status = ui::t!(
                            "too_many_cells_selected"
                        )
                        .into();
                        return;
                    }
                    self.prompt = Some(("scenario-name", Editor::new("")));
                    self.status =
                        ui::tf!("type_name_scenario_press", n)
                            .into();
                    return;
                }
                if v.starts_with("delete_scenario") {
                    let at = self.pop_anchor();
                    let items: Vec<(String, String)> = self
                        .sheet()
                        .scenarios
                        .iter()
                        .map(|s| (s.name.clone(), s.name.clone()))
                        .collect();
                    self.pick_note = Some(ui::t!("choose_scenario_delete").into());
                    self.pick_kind = "scenario-del";
                    self.pick = Some((items, at));
                    return;
                }
                let name = v.to_string();
                let Some(sc) = self.sheet().scenarios.iter().find(|s| s.name == name).cloned()
                else {
                    return;
                };
                self.checkpoint();
                for (p, val) in &sc.cells {
                    self.sheet_mut().set(*p, kumihan::book::Cell::input(val));
                }
                crate::recalc_book(&mut self.book, self.active);
                self.dirty = true;
                self.status =
                    ui::tf!("applied_scenario_cells_ctrl", name, sc.cells.len())
                        .into();
            }
            "scenario-del" => {
                let name = v.to_string();
                let before = self.sheet().scenarios.len();
                self.sheet_mut().scenarios.retain(|s| s.name != name);
                if self.sheet().scenarios.len() < before {
                    self.dirty = true;
                    self.status = ui::tf!("deleted_scenario", name).into();
                }
            }
            // レポートの接続。押すたびに入切して、一覧は開いたまま
            "slicer-refs" => {
                let name = v.to_string();
                let existed = self.book.pivots.iter().any(|d| d.name == name);
                if existed {
                    if let Some(sl) = self.slicers.get_mut(self.slicer_sel) {
                        if let Some(i) = sl.pivots.iter().position(|x| *x == name) {
                            sl.pivots.remove(i);
                        } else {
                            sl.pivots.push(name);
                        }
                    }
                    // 繋いだ(外した)その場で、いまの絞りを合わせる
                    self.slicer_push_to_pivots(self.slicer_sel, cx);
                }
                self.slicer_refs_pick();
                return;
            }
            // おすすめを1つ選んだ。**ここで初めて作ります**
            "pivot-suggest" => {
                if v.starts_with("choose_fields_myself_rows") {
                    self.status =
                        ui::t!("click_headers_lay_out").into();
                    self.pivot_pick("pivot-rows-pick");
                    return;
                }
                let Some(i) = v.strip_prefix('#').and_then(|x| x.parse::<usize>().ok()) else {
                    return;
                };
                let Some(sg) = self.pivot_suggests.get(i).cloned() else { return };
                let Some(mut pend) = self.pivot_pend.take() else { return };
                pend.rows_sel = sg.rows_sel;
                pend.cols_sel = sg.cols_sel;
                let value = sg.value;
                self.insert_pivot(pend, value, sg.agg, cx);
            }
            "pivot-rows-pick" => {
                if v == "next_choose_columns" {
                    let ok = self
                        .pivot_pend
                        .as_ref()
                        .map(|p| !p.rows_sel.is_empty())
                        .unwrap_or(false);
                    if !ok {
                        self.status =
                            ui::t!("pick_least_one_header").into();
                    } else {
                        self.status = ui::t!("headers_spread_columns_optional").into();
                        self.pivot_pick("pivot-cols-pick");
                        return;
                    }
                } else if let Some(p) = &mut self.pivot_pend {
                    let h = v.to_string();
                    if let Some(i) = p.rows_sel.iter().position(|x| *x == h) {
                        p.rows_sel.remove(i);
                    } else if p.headers.contains(&h) {
                        p.rows_sel.push(h);
                    }
                }
                self.pivot_pick("pivot-rows-pick");
                return;
            }
            "pivot-cols-pick" => {
                if v == "done_columns_optional" {
                    self.status = ui::t!("click_header_use_values").into();
                    self.pivot_pick("pivot-val-pick");
                    return;
                }
                if let Some(p) = &mut self.pivot_pend {
                    let h = v.to_string();
                    if let Some(i) = p.cols_sel.iter().position(|x| *x == h) {
                        p.cols_sel.remove(i);
                    } else if p.headers.contains(&h) {
                        p.cols_sel.push(h);
                    }
                }
                self.pivot_pick("pivot-cols-pick");
                return;
            }
            "pivot-val-pick" => {
                let known = self
                    .pivot_pend
                    .as_ref()
                    .map(|p| p.headers.contains(&v.to_string()))
                    .unwrap_or(false);
                if known {
                    if let Some(p) = &mut self.pivot_pend {
                        p.val_sel = v.to_string();
                    }
                    self.status = ui::tf!("how_should_aggregated", v).into();
                    self.pivot_pick("pivot-agg-pick");
                    return;
                }
            }
            "pivot-agg-pick" => {
                let Some(pend) = self.pivot_pend.take() else { return };
                let agg =
                    pivot_aggs().iter().find(|(k, _)| *k == v).map(|(k, _)| *k).unwrap_or("sum");
                let value = pend.val_sel.clone();
                self.insert_pivot(pend, value, agg, cx);
            }
            // 並べ替えの「拡張しますか」(選択の横にデータが続いているとき)
            "sort-expand" => {
                let asc = self.sort_pend.take().unwrap_or(true);
                if v.starts_with("expand_selection_neighbouring_columns") {
                    // 表全体をカーソル列で(見出しは据え置き — 従来の道)
                    self.sort_col(self.cursor.col, asc);
                } else if v.starts_with("sort_selection_only_fall") {
                    let (a, b) = self.sel_rect();
                    self.sort_range_now(a, b, asc);
                } else {
                    self.status = ui::t!("sort_cancelled").into();
                }
            }
            // 結合の4択(本家のドロップダウン)
            "merge-pick" => {
                let kind = match v {
                    "merge_across_row_row" => "横方向",
                    "merge_cells_leave_alignment" => "結合だけ",
                    "unmerge" => "解除",
                    _ => "centre",
                };
                self.merge_selection(kind);
                if self.pick.is_some() {
                    return; // 値の確認へ(pick_kind を戻さない)
                }
            }
            // 通貨を選んだ。**記号は選んだもの、並びは画面の言語**
            "currency" => {
                let Some((_, label, sym, dec)) =
                    currencies().iter().find(|(k, _, _, _)| *k == v).cloned()
                else {
                    return;
                };
                let pattern = kumihan::datetime_names::names(ui::language()).currency_pattern;
                let code = currency_code(sym, dec, pattern);
                let c = code.clone();
                self.fmt(move |f| f.number_format = Some(c.clone()));
                self.status =
                    ui::tf!("currency_set_code", label, code).into();
            }
            // 日付の形を選んだ
            "datefmt" => {
                let Some((_, label, code)) =
                    date_formats().into_iter().find(|(k, _, _)| *k == v)
                else {
                    return;
                };
                let c = code.clone();
                self.fmt(move |f| f.number_format = Some(c.clone()));
                self.status = ui::tf!("date_style_set_code", label, code).into();
            }
            "numfmt-pick" => {
                // 「日付…」も例を約束しない — 日付の形を選ぶ一覧へ渡す
                if v == "date_2" {
                    self.run_cmd("datefmt", cx);
                    return;
                }
                // 「通貨…」は記号を約束しない — 通貨を選ぶ一覧へ渡す
                if v == "currency_2" {
                    self.run_cmd("currency", cx);
                    return;
                }
                if v.starts_with("other") {
                    // 書式コードの直打ち(カスタム書式)。今のコードを下敷きに
                    let cur = self
                        .sheet()
                        .get(self.cursor)
                        .and_then(|c| c.fmt.number_format.clone())
                        .unwrap_or_default();
                    self.prompt = Some(("numfmt-custom", Editor::new(&cur)));
                    return; // pick_kind を戻さない(パネルの確定まで)
                }
                if let Some((_, label, code)) = numfmts().iter().find(|(k, _, _)| *k == v) {
                    let c = code.map(|s| s.to_string());
                    self.fmt(move |f| f.number_format = c.clone());
                    // 鍵ではなく見出し(コードの方は書式コードそのもの — 訳さない)
                    self.status = match code {
                        Some(c) => {
                            ui::tf!("number_format_set_code", label, c).into()
                        }
                        None => ui::t!("number_format_reset_general").into(),
                    };
                }
            }
            "changecase" => {
                self.checkpoint();
                let (a, b) = self.sel_rect();
                let mut n = 0usize;
                for r in a.row..=b.row {
                    for c in a.col..=b.col {
                        let p = Pos::new(r, c);
                        let Some(cell) = self.sheet().get(p).cloned() else { continue };
                        if cell.formula.is_some() {
                            continue; // 式の結果は触らない(次の計算で戻ってしまう)
                        }
                        let kumihan::book::Value::Text(t) = &cell.value else { continue };
                        let new_t = change_case(t, v);
                        if new_t != *t {
                            let mut cell = cell;
                            cell.value = kumihan::book::Value::Text(new_t);
                            self.sheet_mut().set(p, cell);
                            n += 1;
                        }
                    }
                }
                if n == 0 {
                    self.undo_stack.pop();
                    self.status = ui::t!("nothing_selection_changes_case").into();
                } else {
                    self.dirty = true;
                    self.sync_input();
                    self.status = ui::tf!("changed_case_cells", n).into();
                }
            }
            "orient-pick" => {
                // **鍵をそのまま文に差し込まない。** 見出しを一緒に持って回る
                let (deg, label): (Option<i32>, &'static str) = match v {
                    "no_rotation" => (Some(0), ui::t!("no_rotation")),
                    "rotate_up_45" => (Some(45), ui::t!("rotate_up_45")),
                    "rotate_down_45" => (Some(135), ui::t!("rotate_down_45")),
                    "rotate_up_90" => (Some(90), ui::t!("rotate_up_90")),
                    "rotate_down_90" => (Some(180), ui::t!("rotate_down_90")),
                    "vertical_stack_one_character" => (Some(255), ui::t!("vertical_stack_one_character")),
                    _ => (None, ""),
                };
                match deg {
                    Some(0) => {
                        self.fmt(|f| f.rotation = None);
                        self.status = ui::t!("text_orientation_reset").into();
                    }
                    Some(d) => {
                        self.fmt(move |f| f.rotation = Some(d));
                        self.status = if d == 255 {
                            ui::t!("text_stacked_vertically").into()
                        } else {
                            ui::tf!("text_set", label).into()
                        };
                    }
                    None => {
                        // その他 = 任意の角度(-90〜90)
                        self.prompt = Some(("text-angle", Editor::new("")));
                        return; // パネルの確定まで
                    }
                }
            }
            "font-color" => {
                if v.starts_with("other") {
                    self.prompt = Some(("font-color-rgb", Editor::new("")));
                    return; // パネルの確定まで
                }
                if let Some((_, label, hx)) = font_colors().iter().find(|(k, _, _)| *k == v) {
                    let c = hx.map(|h| h.to_string());
                    self.fmt(move |f| f.color = c.clone());
                    // 鍵ではなく見出し(訳した文に日本語を混ぜない)
                    self.status = if hx.is_some() {
                        ui::tf!("font_colour_set", label).into()
                    } else {
                        ui::t!("font_colour_reset_automatic").into()
                    };
                }
            }
            "fill-color" => {
                if v.starts_with("other") {
                    self.prompt = Some(("fill-color-rgb", Editor::new("")));
                    return; // パネルの確定まで
                }
                // 2段目へ(柄・グラデーション)。1段目と同じ場所に重ねる
                if v == "apply_pattern" || v == "gradient" {
                    let at = self
                        .pick
                        .as_ref()
                        .map(|(_, at)| *at)
                        .unwrap_or_else(|| self.pop_anchor());
                    let grad = v.starts_with("gradient_2");
                    self.pick_kind = if grad { "fill-grad" } else { "fill-pattern" };
                    self.pick_note = Some(
                        if grad { ui::t!("gradient_2") } else { ui::t!("pattern") }.into(),
                    );
                    self.pick = Some((menu(&if grad { grad_dirs() } else { fill_patterns() }), at));
                    return;
                }
                if v == "pattern_background_colour" {
                    // **柄が無いときは効かない。そう言う** — 押して何も
                    // 起きないと、鍵が効かないのか柄が無いのか分からない
                    let has = self
                        .sheet()
                        .get(self.cursor)
                        .map(|c| c.fmt.fill_pattern.is_some())
                        .unwrap_or(false);
                    if !has {
                        self.status =
                            ui::t!("cell_no_pattern_pick").into();
                        return;
                    }
                    self.prompt = Some(("fill-bg-rgb", Editor::new("")));
                    return;
                }
                if let Some((_, label, hx)) = fill_colors().iter().find(|(k, _, _)| *k == v) {
                    let c = hx.map(|h| h.to_string());
                    // **色を選ぶ = べた塗り。** 柄と虹は外す — 単色を選んだのに
                    // 前の網目が残っていたら、選んだ物と見える物が食い違う
                    self.fmt(move |f| {
                        f.fill = c.clone();
                        f.fill_pattern = None;
                        f.fill_bg = None;
                        f.fill_grad = None;
                    });
                    // 鍵ではなく見出し(訳した文に日本語を混ぜない)
                    self.status = if hx.is_some() {
                        ui::tf!("fill_set", label).into()
                    } else {
                        ui::t!("fill_removed").into()
                    };
                }
            }
            // 柄を掛ける。**いまの塗りの色を前景に**、地は白から始める
            // (地は「柄の地の色…」で後から変えられる)
            "fill-pattern" => {
                let Some(p) = pattern_kind(v) else { return };
                let now = self.sheet().get(self.cursor).map(|c| c.fmt.clone()).unwrap_or_default();
                // 色が無いまま柄だけ掛けると、白地に白の柄=何も見えない
                let fg = now.fill.clone().unwrap_or_else(|| "808080".into());
                let bg = now.fill_bg.clone().unwrap_or_else(|| "FFFFFF".into());
                self.fmt(move |f| {
                    f.fill = Some(fg.clone());
                    f.fill_pattern = Some(p.to_string());
                    f.fill_bg = Some(bg.clone());
                    f.fill_grad = None; // 柄と虹は排他(xlsx も塗りの要素は一つ)
                });
                let label = fill_patterns().iter().find(|(k, _)| *k == v).map(|(_, l)| *l).unwrap_or("");
                self.status =
                    ui::tf!("pattern_set_foreground_current", label).into();
            }
            "fill-grad" => {
                let Some((deg, path)) = grad_dir_of(v) else { return };
                let now = self.sheet().get(self.cursor).map(|c| c.fmt.clone()).unwrap_or_default();
                // いまの塗りの色から白へ。色が無ければ薄い青から白へ
                let from = now.fill.clone().unwrap_or_else(|| "DEEAF6".into());
                self.fmt(move |f| {
                    f.fill_grad = Some(kumihan::book::Gradient {
                        degree_c: deg,
                        stops: vec![(0, from.clone()), (1000, "FFFFFF".into())],
                        path: path.then(|| "path".to_string()),
                    });
                    f.fill_pattern = None;
                    f.fill_bg = None;
                });
                let label = grad_dirs().iter().find(|(k, _)| *k == v).map(|(_, l)| *l).unwrap_or("");
                self.status =
                    ui::tf!("gradient_set_current_fill", label).into();
            }
            "sheet-menu" => {
                self.sheet_menu_action(v);
                if self.pick.is_some() || self.prompt.is_some() {
                    return; // 2段目(色・改名・再表示)へ。pick_kind を戻さない
                }
            }
            "tab-color" => self.set_tab_color(v),
            "history" | "plugin" => {
                let plugin = self.pick_kind == "plugin";
                let hit = self.pick_paths.iter().find(|(n, _)| n == v).cloned();
                if let Some((_, path)) = hit {
                    if plugin {
                        match std::fs::read_to_string(&path) {
                            Ok(code) => self.run_python(code, cx),
                            Err(e) => self.status = ui::tf!("cant_read", e).into(),
                        }
                    } else {
                        self.open_version(&path);
                    }
                }
                self.pick_paths.clear();
            }
            _ => self.pick_value(v),
        }
        self.pick_kind = "value";
        self.pick_note = None;
    }

    /// 一覧から選んだ値をセルに入れる(書式は据え置き)。
    pub(crate) fn pick_value(&mut self, v: &str) {
        self.checkpoint();
        let p = self.cursor;
        let fmt = self.sheet().get(p).map(|c| c.fmt.clone()).unwrap_or_default();
        let mut cell = Cell::input(v);
        cell.fmt = fmt;
        self.book.sheets[self.active].set(p, cell);
        recalc_book(&mut self.book, self.active);
        self.dirty = true;
        self.sync_input();
        self.status = ui::tf!("entered_into", p.a1()).into();
    }

    pub(crate) fn menu_action(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let menu_was_at = self.menu_at.take();
        self.menu_sub = None;
        match id {
            "cut" => self.a_cut(&ui::Cut, window, cx),
            "copy" => self.a_copy(&ui::Copy, window, cx),
            "paste" => self.a_paste(&ui::Paste, window, cx),
            // 図形の専用メニュー(実体は main.rs の shape_menu_action)
            "sh-cut" | "sh-copy" | "sh-paste" | "sh-del" | "sh-front" | "sh-forward"
            | "sh-backward" | "sh-back" | "sh-rot-r" | "sh-rot-l" | "sh-flip-h"
            | "sh-flip-v" | "sh-save" | "sh-settings" | "sh-points" => {
                self.shape_menu_action(id)
            }
            "sh-b-union" => self.shapes_boolean(kumihan::book::BoolOp::Union),
            "sh-b-inter" => self.shapes_boolean(kumihan::book::BoolOp::Intersect),
            "sh-b-sub" => self.shapes_boolean(kumihan::book::BoolOp::Subtract),
            "sh-al-l" | "sh-al-c" | "sh-al-r" | "sh-al-t" | "sh-al-m" | "sh-al-b"
            | "sh-dist-h" | "sh-dist-v" => self.shape_align(id),
            "ps-values" => self.paste_special("values", cx),
            "ps-formulas" => self.paste_special("formulas", cx),
            "ps-formats" => self.paste_special("formats", cx),
            "ps-transpose" => self.paste_special("transpose", cx),
            // 消去。Euro-Office の「消去 ▸」に対応する3段
            "clear-all" => {
                self.commit();
                self.checkpoint();
                let (a, b) = self.sel_rect();
                let mut n = 0usize;
                for r in a.row..=b.row {
                    for c in a.col..=b.col {
                        n += self.book.sheets[self.active]
                            .cells
                            .remove(&Pos::new(r, c))
                            .is_some() as usize;
                    }
                }
                recalc_book(&mut self.book, self.active);
                self.dirty = true;
                self.sync_input();
                self.status = ui::tf!("cells_cleared_contents_formatting", n).into();
            }
            "clear-text" => {
                self.checkpoint();
                let n = self.clear_range();
                self.status = ui::tf!("contents_cells_cleared_formatting", n).into();
            }
            "clear-fmt" => self.run_cmd("clear", cx),
            // コメントとハイパーリンクだけを消す(本家の消去は5択)
            "clear-comment" => {
                self.checkpoint();
                let (a, b) = self.sel_rect();
                let sh = &mut self.book.sheets[self.active];
                let before = sh.comments.len();
                sh.comments.retain(|p, _| {
                    p.row < a.row || p.row > b.row || p.col < a.col || p.col > b.col
                });
                let n = before - sh.comments.len();
                if n == 0 {
                    self.undo_stack.pop();
                    self.status = ui::t!("no_comments_range").into();
                } else {
                    self.dirty = true;
                    self.status = ui::tf!("removed_comments", n).into();
                }
            }
            "clear-link" => {
                self.checkpoint();
                let (a, b) = self.sel_rect();
                let sh = &mut self.book.sheets[self.active];
                let before = sh.links.len();
                sh.links.retain(|p, _| {
                    p.row < a.row || p.row > b.row || p.col < a.col || p.col > b.col
                });
                let n = before - sh.links.len();
                if n == 0 {
                    self.undo_stack.pop();
                    self.status = ui::t!("no_hyperlinks_range").into();
                } else {
                    self.dirty = true;
                    self.status = ui::tf!("removed_hyperlinks", n).into();
                }
            }
            "insrow" => {
                self.rowcol(|s, p| s.insert_row(p.row));
                self.status = ui::t!("row_inserted_formula_references").into();
            }
            "delrow" => {
                self.rowcol(|s, p| s.remove_row(p.row));
                self.status = ui::t!("row_deleted").into();
            }
            "inscol" => {
                self.rowcol(|s, p| s.insert_col(p.col));
                self.status = ui::t!("column_inserted").into();
            }
            "delcol" => {
                self.rowcol(|s, p| s.remove_col(p.col));
                self.status = ui::t!("column_deleted").into();
            }
            "sort-asc" | "sort-desc" => self.sort_active(id == "sort-asc"),
            "sort-fill-top" | "sort-font-top" => self.sort_color_top(id == "sort-fill-top"),
            id if id.starts_with("subt-") => self.set_subtotal_kind(&id["subt-".len()..]),
            // 選んだ値で絞り込む = その列で「選んだ値以外」を隠す
            // (オートフィルタの1操作。▼で選び直せる)
            "filter-set" => {
                let p = self.cursor;
                let v = self.sheet().get(p).map(|c| c.value.display()).unwrap_or_default();
                if self.auto_filter.is_none() {
                    self.run_cmd("setfilter", cx);
                }
                if self.auto_filter.is_none() {
                    return; // 張れなかった(空の表)。理由は setfilter が言っている
                }
                let (vals, _) = self.filter_values(p.col);
                let hide: std::collections::BTreeSet<String> =
                    vals.into_iter().map(|(s, _)| s).filter(|s| *s != v).collect();
                let f = self.auto_filter.as_mut().unwrap();
                if hide.is_empty() {
                    f.hide.remove(&p.col);
                } else {
                    f.hide.insert(p.col, hide);
                }
                let label = if v.is_empty() { ui::t!("blank").to_string() } else { v };
                self.status = ui::tf!("showing_only_change_header", label).into();
            }
            "filter-clear" => self.run_cmd("clear-filter", cx),
            "numfmt-more" => self.run_cmd("format", cx),
            "reapply" => {
                // 値は動的に見ているので掛け直しは常に済んでいる — 数を言い直す
                if let Some((total, shown)) = self.filter_counts() {
                    self.status = ui::tf!("filter_reapplied_rows_shown", total, shown).into();
                }
            }
            // セル単位のシフト(挿入・削除)。結合をまたぐときは断られる
            "inscell-right" | "inscell-down" | "delcell-left" | "delcell-up" => {
                self.commit();
                self.checkpoint();
                let (a, b) = self.sel_rect();
                let r = match id {
                    "inscell-right" => self.book.sheets[self.active].insert_cells(a, b, true),
                    "inscell-down" => self.book.sheets[self.active].insert_cells(a, b, false),
                    "delcell-left" => self.book.sheets[self.active].delete_cells(a, b, true),
                    _ => self.book.sheets[self.active].delete_cells(a, b, false),
                };
                match r {
                    Ok(n) => {
                        recalc_book(&mut self.book, self.active);
                        self.dirty = true;
                        self.anchor = None;
                        self.sync_input();
                        self.status = ui::tf!("cells_shifted_references_moved", n)
                        .into();
                    }
                    Err(e) => {
                        // 何も変えていないので、積んだ控えは戻す
                        self.undo_stack.pop();
                        self.status = e.into();
                    }
                }
            }
            "cond-neg" => {
                self.commit();
                self.checkpoint();
                let range = self.sel_rect();
                self.book.sheets[self.active].cond.push(kumihan::book::CondRule {
                    range,
                    kind: kumihan::book::CondKind::Cmp(kumihan::book::CondOp::Lt, 0.0),
                    look: kumihan::book::CondLook {
                        color: Some("C00000".into()),
                        ..Default::default()
                    },
                });
                self.dirty = true;
                self.status = ui::tf!("values_below_0_shown", range.0.a1(), range.1.a1()).into();
            }
            "cond-gt" => {
                self.commit();
                self.prompt = Some(("cond-gt", Editor::new("")));
            }
            "cond-lt" => {
                self.commit();
                self.prompt = Some(("cond-lt", Editor::new("")));
            }
            "cond-between" => {
                self.commit();
                self.prompt = Some(("cond-between", Editor::new("")));
            }
            "cond-text" => {
                self.commit();
                self.prompt = Some(("cond-text", Editor::new("")));
            }
            "cond-top" | "cond-bottom" => {
                let bottom = id == "cond-bottom";
                self.commit();
                self.prompt = Some((
                    if bottom { "cond-bottom" } else { "cond-top" },
                    Editor::new("10"),
                ));
            }
            // パネルの要らない規則はその場で掛ける
            // 第2版: バー/スケール/アイコン(範囲の最小〜最大が物差し)
            "cond-bar" | "cond-scale" | "cond-icons" => self.cond_visual(id),
            "cond-manage" => {
                self.commit();
                let rules = &self.book.sheets[self.active].cond;
                if rules.is_empty() {
                    self.status = ui::t!("sheet_no_conditional_formatting").into();
                } else {
                    let at = self.pop_anchor();
                    // 番号・番地・規則の中身 — 帳票の側の値なので訳さない
                    let items = plain(rules.iter().enumerate().map(|(i, r)| {
                        format!(
                            "{}) {}:{} — {}",
                            i + 1,
                            r.range.0.a1(),
                            r.range.1.a1(),
                            cond_kind_name(&r.kind)
                        )
                    }));
                    self.pick_note = Some(ui::t!("manage_rules_click_rule").into());
                    self.pick_kind = "cond-manage-pick";
                    self.pick = Some((items, at));
                }
            }
            "cond-dup" | "cond-uniq" | "cond-avg-above" | "cond-avg-below" => {
                self.commit();
                self.checkpoint();
                let range = self.sel_rect();
                use kumihan::book::{CondKind, CondLook, CondRule};
                let (kind, color, fill, said) = match id {
                    "cond-dup" => (
                        CondKind::Dup(false),
                        Some("9C0006".to_string()),
                        Some("FFC7CE".to_string()),
                        ui::t!("duplicate_values_shown_red").to_string(),
                    ),
                    "cond-uniq" => (
                        CondKind::Dup(true),
                        None,
                        Some("E2EFDA".to_string()),
                        ui::t!("unique_values_filled").to_string(),
                    ),
                    "cond-avg-above" => (
                        CondKind::Avg(false),
                        None,
                        Some("E2EFDA".to_string()),
                        ui::t!("above_average_values_filled").to_string(),
                    ),
                    _ => (
                        CondKind::Avg(true),
                        None,
                        Some("FCE4D6".to_string()),
                        ui::t!("below_average_values_filled").to_string(),
                    ),
                };
                self.book.sheets[self.active]
                    .cond
                    .push(CondRule {
                        range,
                        kind,
                        look: CondLook { color, fill, ..Default::default() },
                    });
                self.dirty = true;
                self.status =
                    format!("{}:{} — {}", range.0.a1(), range.1.a1(), said).into();
            }
            "cond-clear" => {
                self.commit();
                self.checkpoint();
                let (a, b) = self.sel_rect();
                let before = self.book.sheets[self.active].cond.len();
                self.book.sheets[self.active].cond.retain(|r| {
                    let (ra, rb) = r.range;
                    // 選んだ範囲と重なる規則を消す
                    !(ra.row <= b.row && rb.row >= a.row && ra.col <= b.col && rb.col >= a.col)
                });
                let n = before - self.book.sheets[self.active].cond.len();
                self.dirty = true;
                self.status = ui::tf!("rules_removed", n).into();
            }
            // 見出しの右クリック: 行・列の非表示と再表示
            "hide-rows" | "hide-cols" | "unhide-rows" | "unhide-cols" => self.hide_lines(id),
            // 見出しの右クリック: 幅・高さの数値指定(選んだ列・行ぶん)
            "colw" => {
                let cur = self
                    .sheet()
                    .col_width
                    .get(&self.cursor.col)
                    .map(|w| format!("{w:.2}"))
                    .unwrap_or_default();
                self.prompt = Some(("col-width", Editor::new(&cur)));
            }
            "rowh" => {
                let cur = self
                    .sheet()
                    .row_height
                    .get(&self.cursor.row)
                    .map(|h| format!("{h:.1}"))
                    .unwrap_or_default();
                self.prompt = Some(("row-height", Editor::new(&cur)));
            }
            "picklist" => self.open_pick_list(),
            "defname" => {
                self.commit();
                self.prompt = Some(("name", Editor::new("")));
            }
            // コメントの一覧の板(ブック全体)。並べ替えと跳び先はこの板が持つ
            "comment-list" => {
                self.comment_list = match self.comment_list {
                    Some(_) => None,
                    None => Some(CommentList::default()),
                };
                self.status = if self.comment_list.is_some() {
                    ui::t!("comment_list_opened_whole").into()
                } else {
                    ui::t!("comment_list_closed").into()
                };
            }
            // コメントの筋に返信を足す(本家の「返信を追加」)
            "comment-reply" => {
                self.commit();
                if !self.sheet().comments.contains_key(&self.cursor) {
                    self.status = ui::t!("cell_no_comment").into();
                    return;
                }
                self.prompt = Some(("comment-reply", Editor::new("")));
            }
            // 解決の印を入切する。**筋ごと**に立つ(返信1つだけは解決できない)
            "comment-done" => {
                self.commit();
                let p = self.cursor;
                if !self.sheet().comments.contains_key(&p) {
                    self.status = ui::t!("cell_no_comment").into();
                    return;
                }
                self.checkpoint();
                let th = self.book.sheets[self.active].comments.get_mut(&p).unwrap();
                th.done = !th.done;
                let now = th.done;
                self.dirty = true;
                self.status = if now {
                    ui::t!("marked_resolved").into()
                } else {
                    ui::t!("resolved_mark_removed").into()
                };
            }
            "co-addcomment" => {
                self.commit();
                // 打ち直すのは**筋の頭の文**(返信は別の口で足す)
                let cur = self
                    .sheet()
                    .comments
                    .get(&self.cursor)
                    .map(|t| t.text().to_string())
                    .unwrap_or_default();
                self.prompt = Some(("comment", Editor::new(&cur)));
            }
            "hyperlink" => {
                self.commit();
                let cur = self.sheet().links.get(&self.cursor).cloned().unwrap_or_default();
                self.prompt = Some(("link", Editor::new(&cur)));
            }
            "fmtcells" => {
                // メニューの出ていた場所の近くに小窓を開く
                self.fmt_panel = Some(menu_was_at.unwrap_or((HEAD_W + 24.0, ROW_H + 24.0)));
            }
            "freeze" => self.run_cmd("freeze", cx),
            // 数値の書式・関数はリボンと同じ配線を通す
            "comma" | "currency" | "percents" | "digit-inc" | "digit-dec"
            | "sum" | "average" | "count" | "max" | "min" => self.run_cmd(id, cx),
            _ => {}
        }
        cx.notify();
    }

    /// 第2版の条件付き書式: バー/スケール/アイコン(範囲の最小〜最大が物差し)
    pub(crate) fn cond_visual(&mut self, id: &str) {
        self.commit();
        self.checkpoint();
        let range = self.sel_rect();
        use kumihan::book::{CondKind, CondLook, CondRule};
        let (kind, said) = match id {
            "cond-bar" => (
                CondKind::Bar("638EC6".into()),
                ui::t!("data_bars_added_bar").to_string(),
            ),
            "cond-scale" => (
                CondKind::Scale("F8696B".into(), Some("FFEB84".into()), "63BE7B".into()),
                ui::t!("colour_scale_applied_low").to_string(),
            ),
            _ => (
                CondKind::Icons("3Arrows".into()),
                ui::t!("three_arrows_added_low").to_string(),
            ),
        };
        self.book.sheets[self.active]
            .cond
            .push(CondRule { range, kind, look: CondLook::default() });
        self.dirty = true;
        self.status = format!("{}:{} — {}", range.0.a1(), range.1.a1(), said).into();
    }

    /// 子メニューの中身 (id, 名前, 押せるか)。
    /// **並びと名前は Euro-Office に合わせ、未実装は灰色**(リボンと同じ方針)。
    pub(crate) fn menu_sub_entries(&self, sub: &str) -> Vec<(&'static str, &'static str, bool)> {
        match sub {
            "ins" => vec![
                ("inscell-right", "セルを右にシフト", true),
                ("inscell-down", "セルを下にシフト", true),
                ("insrow", "行全体", true),
                ("inscol", "列全体", true),
            ],
            "del" => vec![
                ("delcell-left", "セルを左にシフト", true),
                ("delcell-up", "セルを上にシフト", true),
                ("delrow", "行全体", true),
                ("delcol", "列全体", true),
            ],
            // 図形の配置(重なり順)と回転
            "sh-order" => vec![
                ("sh-front", "最前面へ移動", true),
                ("sh-forward", "前面へ移動", true),
                ("sh-backward", "背面へ移動", true),
                ("sh-back", "最背面へ移動", true),
            ],
            "sh-rotate" => vec![
                ("sh-rot-r", "右へ90度回転", true),
                ("sh-rot-l", "左へ90度回転", true),
                ("sh-flip-h", "左右に反転", true),
                ("sh-flip-v", "上下に反転", true),
            ],
            // 整列は2個から、分布は3個から(Ctrl+クリックで束ねる)
            "sh-bool" => vec![
                ("sh-b-union", "union", true),
                ("sh-b-inter", "intersect", true),
                ("sh-b-sub", "減算(主から控えを引く)", true),
            ],
            "sh-align" => {
                let n = self.shape_sel.is_some() as usize + self.shape_multi.len();
                vec![
                    ("sh-al-l", "align_left", n >= 2),
                    ("sh-al-c", "左右中央揃え", n >= 2),
                    ("sh-al-r", "align_right", n >= 2),
                    ("sh-al-t", "上揃え", n >= 2),
                    ("sh-al-m", "align_middle", n >= 2),
                    ("sh-al-b", "下揃え", n >= 2),
                    ("sh-dist-h", "横に分布", n >= 3),
                    ("sh-dist-v", "縦に分布", n >= 3),
                ]
            }
            "clr" => vec![
                // 本家の消去は5択(すべて/テキスト/書式/コメント/ハイパーリンク)
                ("clear-all", "all", true),
                ("clear-text", "テキスト(書式は残す)", true),
                ("clear-fmt", "書式(中身は残す)", true),
                ("clear-comment", "comment", !self.sheet().comments.is_empty()),
                ("clear-link", "ハイパーリンク", !self.sheet().links.is_empty()),
            ],
            // 本家の合計行のセル右の▼と同じ8択(SUBTOTAL の集計番号)
            "subtotal" => vec![
                ("subt-9", "sum", true),
                ("subt-1", "average", true),
                ("subt-3", "count", true),
                ("subt-4", "maximum", true),
                ("subt-5", "minimum", true),
                ("subt-7", "stddev", true),
                ("subt-10", "variance", true),
                ("subt-none", "なし(式を消す)", true),
            ],
            "sort" => {
                let f = self.sheet().get(self.cursor).map(|c| c.fmt.clone()).unwrap_or_default();
                vec![
                    ("sort-asc", "ascending", true),
                    ("sort-desc", "descending", true),
                    ("sort-fill-top", "選択したセルの色を上に", f.fill.is_some()),
                    ("sort-font-top", "選択したフォントの色を上に", f.color.is_some()),
                ]
            }
            "filter" => vec![
                ("filter-set", "選択した値で絞り込む", true),
                ("filter-clear", "絞り込みを解く", self.auto_filter.is_some()),
            ],
            "pastesp" => vec![
                ("ps-values", "値だけ(Ctrl+Shift+V)", true),
                ("ps-formulas", "式をそのまま(ずらさない)", true),
                ("ps-formats", "書式だけ", self.clip_cells.is_some()),
                ("ps-transpose", "行と列を入れ替えて(値を)", true),
            ],
            "cond" => vec![
                ("cond-neg", "0未満を赤字にする", true),
                ("cond-gt", "値より大きいと薄緑の塗り…", true),
                ("cond-lt", "値より小さいと薄赤の塗り…", true),
                ("cond-between", "値の間なら薄黄の塗り…", true),
                ("cond-text", "文字を含むと薄黄の塗り…", true),
                ("cond-dup", "重複する値を赤く", true),
                ("cond-uniq", "一意の値を薄緑に", true),
                ("cond-top", "上位Nを薄青に…", true),
                ("cond-bottom", "下位Nを薄赤に…", true),
                ("cond-avg-above", "平均より上を薄緑に", true),
                ("cond-avg-below", "平均より下を薄赤に", true),
                ("cond-bar", "データバー(青の棒)", true),
                ("cond-scale", "カラースケール(赤→黄→緑)", true),
                ("cond-icons", "アイコン(3つの矢印)", true),
                ("cond-manage", "ルールの管理…", true),
                ("cond-clear", "この範囲の条件を消す", true),
            ],
            "numfmt" => vec![
                ("comma", "桁区切り(1,000)", true),
                ("currency", "通貨(¥)", true),
                ("percents", "パーセント(%)", true),
                ("digit-inc", "小数を増やす", true),
                ("digit-dec", "小数を減らす", true),
                ("numfmt-more", "その他の表示形式…", true),
            ],
            "func" => vec![
                ("sum", "SUM(合計)", true),
                ("average", "AVERAGE(平均)", true),
                ("count", "COUNT(個数)", true),
                ("max", "MAX(最大)", true),
                ("min", "MIN(最小)", true),
            ],
            _ => vec![],
        }
    }

    pub(crate) fn a_context_menu(&mut self, _: &ui::ContextMenu, _: &mut Window, cx: &mut Context<Self>) {
        // キーボードから: カーソルのセルのそば(セルが画面の外なら左上)に出す
        let (x, y) = self
            .cell_origin_px(self.cursor)
            .map(|(x, y)| (x + 16.0, y + 16.0))
            .unwrap_or((HEAD_W + 16.0, ROW_H + 16.0));
        self.menu_at = Some((x, y));
        self.menu_sub = None;
        cx.notify();
    }

    /// 名前ボックスの Enter。番地(B12)・範囲(A1:C9)・定義済みの名前なら
    /// そこへ飛ぶ。知らない名前なら**いまの選択に名前を付ける**(Excel と同じ)
    pub(crate) fn commit_name_box(&mut self) {
        let Some(ed) = self.name_edit.take() else { return };
        let t = ed.text().trim().to_string();
        if t.is_empty() {
            return;
        }
        let up = t.to_uppercase();
        let jump = |this: &mut Self, a: Pos, b: Option<Pos>| {
            this.commit();
            this.cursor = b.unwrap_or(a);
            this.anchor = b.is_some().then_some(a);
            this.sync_input();
            this.follow();
        };
        if let Some((a, b)) = up.split_once(':') {
            if let (Some(pa), Some(pb)) = (Pos::parse(a), Pos::parse(b)) {
                jump(self, pa, Some(pb));
                self.status = ui::tf!("selected_2", up).into();
                return;
            }
        }
        if let Some(p) = Pos::parse(&up) {
            jump(self, p, None);
            self.status = ui::tf!("moved", p.a1()).into();
            return;
        }
        // 定義済みの名前ならそこへ
        if let Some(r) = self
            .sheet()
            .names
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(&t))
            .cloned()
        {
            let up = r.range.to_uppercase();
            if let Some((a, b)) = up.split_once(':') {
                if let (Some(pa), Some(pb)) = (Pos::parse(a), Pos::parse(b)) {
                    jump(self, pa, Some(pb));
                    self.status = ui::tf!("selected_name", t, up).into();
                    return;
                }
            }
            if let Some(p) = Pos::parse(&up) {
                jump(self, p, None);
                self.status = ui::tf!("moved_name", t, up).into();
                return;
            }
        }
        // 新しい名前 = いまの選択に付ける
        let range = if self.anchor.is_some() {
            let (a, b) = self.sel_rect();
            format!("{}:{}", a.a1(), b.a1())
        } else {
            self.cursor.a1()
        };
        self.checkpoint();
        self.sheet_mut().names.push(kumihan::book::DefinedName::new(t.clone(), range.clone()));
        self.dirty = true;
        self.status = ui::tf!("assigned_name_recall_name", t, range).into();
    }

    /// 式の直入力の支援。=を打っている間だけ:
    /// - 打ちかけの関数名(2字以上)には**補完の一覧**(セルの下。押すと入る)
    /// - 開いた括弧の中では、**いま打っている引数のヒント**をステータスバーに
    pub(crate) fn formula_assist(&mut self) {
        let t = self.input.text().to_string();
        if !t.starts_with('=') {
            if self.pick_kind == "fn-complete" {
                self.pick = None;
                self.pick_note = None;
            }
            return;
        }
        let cur = self.input.cursor().min(t.len());
        // --- 補完: カーソルの直前の識別子(英字はじまり・2字以上) ---
        let token: String = {
            let rev: String = t[..cur]
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '.')
                .collect();
            rev.chars().rev().collect()
        };
        let mut showed = false;
        if token.len() >= 2 && token.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            let up = token.to_uppercase();
            let cands: Vec<String> = funcs::FUNCS
                .iter()
                .filter(|f| f.name.starts_with(&up) && f.name != up)
                .map(|f| f.name.to_string())
                .take(12)
                .collect();
            if !cands.is_empty() {
                if let Some((x, y)) = self.cell_origin_px(self.cursor) {
                    let h = self.row_px(self.cursor.row);
                    self.pick_kind = "fn-complete";
                    // 関数名は式に打ち込む字そのもの — **絶対に訳さない**
                    self.pick = Some((plain(cands), (x, y + h)));
                    showed = true;
                }
            }
        }
        if !showed && self.pick_kind == "fn-complete" {
            self.pick = None;
            self.pick_note = None;
        }
        // --- 引数のヒント: いちばん内側の閉じていない関数と、何番目の引数か ---
        let mut stack: Vec<(String, usize)> = Vec::new();
        let mut in_str = false;
        let mut ident = String::new();
        for ch in t[..cur].chars() {
            match ch {
                '"' => in_str = !in_str,
                _ if in_str => {}
                '(' => {
                    stack.push((ident.to_uppercase(), 0));
                    ident.clear();
                }
                ')' => {
                    stack.pop();
                    ident.clear();
                }
                ',' => {
                    if let Some((_, n)) = stack.last_mut() {
                        *n += 1;
                    }
                    ident.clear();
                }
                c if c.is_ascii_alphanumeric() || c == '.' => ident.push(c),
                _ => ident.clear(),
            }
        }
        if let Some((name, argi)) = stack.last() {
            if let Some(f) = funcs::FUNCS.iter().find(|f| f.name == name) {
                let hint = f
                    .arg_desc()
                    .get(*argi)
                    .or(f.arg_desc().last())
                    .copied()
                    .unwrap_or("");
                let names = parse_fn_args(f.args());
                let arg_name = names
                    .get(*argi)
                    .or(names.last())
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default();
                self.status =
                    format!("{}{} — {}{}", f.name, f.args(), arg_name, hint).into();
            }
        }
    }

    /// 「関数を挿入」の次へ = 選んだ関数の**引数の画面**へ進む(本家の第2段)
    pub(crate) fn fn_next(&mut self) {
        let Some(d) = self.fn_dlg.take() else { return };
        let list = fn_filtered(d.search.text(), d.group);
        let Some(f) = list.get(d.sel.min(list.len().saturating_sub(1))).copied() else {
            self.status = ui::t!("no_function_matches").into();
            return;
        };
        let names = parse_fn_args(f.args());
        let eds = (0..names.len()).map(|_| Editor::new("")).collect();
        self.fn_args = Some(FnArgs {
            f,
            names,
            eds,
            focus: 0,
            result: String::new(),
            pick_from: None,
        });
        self.fn_args_recalc();
        self.status = ui::t!(
            "function_arguments_tab_next")
        .into();
    }

    /// 引数の画面の中身から式の文字を組む(埋めた欄まで)
    pub(crate) fn fn_args_formula(&self) -> Option<String> {
        let a = self.fn_args.as_ref()?;
        let vals: Vec<String> = a.eds.iter().map(|e| e.text().trim().to_string()).collect();
        let mut last = 0;
        for (i, v) in vals.iter().enumerate() {
            if !v.is_empty() {
                last = i + 1;
            }
        }
        Some(format!("{}({})", a.f.name, vals[..last].join(", ")))
    }

    /// 関数の結果の下見。**表の複製**の空きセルで計算する(ゴールシークと
    /// 同じ流儀 — 本物の表は触らない)
    pub(crate) fn fn_args_recalc(&mut self) {
        let Some(fstr) = self.fn_args_formula() else { return };
        let mut s = self.sheet().clone();
        let (rows, _) = s.extent();
        let p = Pos::new(rows + 2, 0);
        s.set(p, Cell::input(&format!("={fstr}")));
        recalc(&mut s);
        let out = s.get(p).map(|c| c.value.display()).unwrap_or_default();
        if let Some(a) = &mut self.fn_args {
            a.result = out;
        }
    }

    /// 引数の画面の OK。組んだ式をセルへ(編集中ならカーソルに差し込み)
    pub(crate) fn fn_args_ok(&mut self) {
        let Some(fstr) = self.fn_args_formula() else {
            self.fn_args = None;
            return;
        };
        self.fn_args = None;
        if self.editing() || self.edit_armed {
            self.input.insert(&fstr);
        } else {
            self.input = Editor::new(&format!("={fstr}"));
            let end = self.input.text().len();
            self.input.move_to(end, false);
        }
        self.edit_armed = true;
        self.status = ui::t!("formula_inserted_enter_confirm").into();
    }

    /// F2 = このセルを編集(次の打鍵が**追記**になる。Excel と同じ)
    pub(crate) fn a_edit_cell(&mut self, _: &ui::EditCell, _: &mut Window, cx: &mut Context<Self>) {
        if self.prompt.is_some() || self.solver.is_some() {
            return;
        }
        self.edit_armed = true;
        self.input.move_to(self.input.text().len(), false);
        self.status = ui::t!("editing_typing_appends_cell").into();
        cx.notify();
    }

    /// 元の表から、ある見出しの値の候補(重複は畳む。上限は自衛)。
    pub(crate) fn pivot_field_values(&self, pi: usize, field: &str) -> Vec<String> {
        let Some(d) = self.book.pivots.get(pi) else { return Vec::new() };
        let (a, b) = d.src;
        let Some(fc) = (a.col..=b.col).position(|c| {
            self.sheet()
                .get(Pos::new(a.row, c))
                .map(|x| x.value.display() == field)
                .unwrap_or(false)
        }) else { return Vec::new() };
        let fc = a.col + fc as u32;
        let mut vals: Vec<String> = Vec::new();
        for r in a.row + 1..=b.row {
            let v = self
                .sheet()
                .get(Pos::new(r, fc))
                .map(|x| x.value.display())
                .unwrap_or_default();
            if !v.is_empty() && !vals.contains(&v) {
                vals.push(v);
            }
            if vals.len() >= 1000 {
                break;
            }
        }
        vals
    }

    /// ピボットの絞り込みの一覧(見出しの ▼)。☑ = 表示、☐ = 隠す。
    /// 組み直しては出し直す(クリックのたび)
    pub(crate) fn pivot_filter_pick(&mut self) {
        let Some((pi, field, hidden)) = self.pivot_flt.clone() else { return };
        let Some(d) = self.book.pivots.get(pi) else { return };
        // 値の候補は元の表から(見出しの列の値。重複は畳む。上限は自衛)
        let (a, b) = d.src;
        let Some(fc) = (a.col..=b.col).position(|c| {
            self.sheet()
                .get(Pos::new(a.row, c))
                .map(|x| x.value.display() == field)
                .unwrap_or(false)
        }) else { return };
        let fc = a.col + fc as u32;
        let mut vals: Vec<String> = Vec::new();
        for r in a.row + 1..=b.row {
            let v = self
                .sheet()
                .get(Pos::new(r, fc))
                .map(|x| x.value.display())
                .unwrap_or_default();
            if !v.is_empty() && !vals.contains(&v) {
                vals.push(v);
            }
            if vals.len() >= 1000 {
                break;
            }
        }
        // 値は帳票の中身 — 訳さない。入切の印は見出しにだけ付ける
        let mut items: Vec<(String, String)> = vals
            .iter()
            .map(|v| {
                let label =
                    if hidden.contains(v) { format!("☐ {v}") } else { format!("☑ {v}") };
                (v.clone(), label)
            })
            .collect();
        items.extend(menu(&[
            ui::item!("apply_filter"),
            ui::item!("show_everything_again"),
            ui::item!("filter_label"),
            ui::item!("filter_value"),
            ui::item!("group"),
            ui::item!("sort"),
        ]));
        let at = self.pop_anchor();
        let pname = self.book.pivots.get(pi).map(|d| d.name.clone()).unwrap_or_default();
        self.pick_note = Some(
            ui::tf!("filter_shown_hidden", pname, field).into(),
        );
        self.pick_kind = "pivot-filter-pick";
        self.pick = Some((items, at));
    }

    /// ピボットの聞き取りの一覧を(いまの控えから)組み直して開く。
    /// クリックのたびに呼ばれる — ✓ の付け外しはここで反映される
    /// 合計行のセルの集計のしかたを替える(=SUM/=SUBTOTAL → =SUBTOTAL(n, 範囲))。
    pub(crate) fn set_subtotal_kind(&mut self, kind: &str) {
        let p = self.cursor;
        let Some(f) = self.sheet().get(p).and_then(|c| c.formula.clone()) else { return };
        // いまの式から範囲を取り出す(SUM(範囲) / SUBTOTAL(番号, 範囲))
        let inner = f
            .find('(')
            .and_then(|i| f.rfind(')').map(|j| &f[i + 1..j]))
            .unwrap_or("");
        let range = inner.split_once(',').map(|(_, r)| r).unwrap_or(inner).trim().to_string();
        if range.is_empty() {
            self.status = ui::t!("not_read_range_formula").into();
            return;
        }
        self.commit();
        self.checkpoint();
        let mut cell = self.sheet().get(p).cloned().unwrap_or_default();
        if kind == "none" {
            cell.formula = None;
            cell.value = kumihan::book::Value::Empty;
            self.book.sheets[self.active].set(p, cell);
            self.status = ui::t!("removed_total_formula_format").into();
        } else {
            let v = kumihan::book::Cell::input(&format!("=SUBTOTAL({kind},{range})"));
            cell.formula = v.formula;
            cell.value = v.value;
            self.book.sheets[self.active].set(p, cell);
            let name = match kind {
                "1" => ui::t!("average"),
                "3" => ui::t!("count"),
                "4" => ui::t!("maximum"),
                "5" => ui::t!("minimum"),
                "7" => ui::t!("stddev"),
                "10" => ui::t!("variance"),
                _ => ui::t!("sum"),
            };
            self.status =
                ui::tf!("changed_over_filtered_out", p.a1(), range, name).into();
        }
        recalc_book(&mut self.book, self.active);
        self.dirty = true;
        self.sync_input();
    }

    /// スパークラインを置く(kind: spark=折れ線 / spark-col=縦棒 / spark-wl=勝ち負け)。
    /// その時の値で描く固定の絵 — データに追従しない(文言でそう言う)
    pub(crate) fn insert_sparkline(&mut self, kind: &str) {
        let (a, b) = self.sel_rect();
        let mut vals: Vec<f64> = Vec::new();
        for r in a.row..=b.row {
            for c in a.col..=b.col {
                if let Some(cell) = self.sheet().get(Pos::new(r, c)) {
                    if let kumihan::book::Value::Number(n) = cell.value {
                        vals.push(n);
                    }
                }
            }
        }
        if vals.len() < 2 {
            self.status = ui::t!("least_two_numbers_needed").into();
            return;
        }
        let (lo, hi) = vals
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
        let n = vals.len();
        let (points, base): (Vec<(f32, f32)>, f32) = match kind {
            // 縦棒: 0 を物差しに入れて、底(0 の高さ)から棒を立てる
            "spark-col" => {
                let lo2 = lo.min(0.0);
                let hi2 = hi.max(0.0);
                let span = (hi2 - lo2).max(1e-9);
                let base = (1.0 - ((0.0 - lo2) / span)) as f32;
                (
                    vals.iter()
                        .enumerate()
                        .map(|(i, v)| {
                            (
                                (i as f32 + 0.5) / n as f32,
                                (1.0 - ((v - lo2) / span)) as f32,
                            )
                        })
                        .collect(),
                    base,
                )
            }
            // 勝ち負け: 符号だけ(正は上へ・負は下へ同じ長さ。0 は底のまま)
            "spark-wl" => (
                vals.iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let y = if *v > 0.0 { 0.1 } else if *v < 0.0 { 0.9 } else { 0.5 };
                        ((i as f32 + 0.5) / n as f32, y)
                    })
                    .collect(),
                0.5,
            ),
            // 折れ線(従来)
            _ => {
                let span = (hi - lo).max(1e-9);
                (
                    vals.iter()
                        .enumerate()
                        .map(|(i, v)| {
                            (
                                i as f32 / (n - 1) as f32,
                                (1.0 - ((v - lo) / span)) as f32,
                            )
                        })
                        .collect(),
                    0.0,
                )
            }
        };
        // 置き場所はいまのセル(選択の中なら右のセル)、大きさはそのセル
        let at = if (a.row..=b.row).contains(&self.cursor.row)
            && (a.col..=b.col).contains(&self.cursor.col)
        {
            Pos::new(a.row, b.col + 1)
        } else {
            self.cursor
        };
        self.checkpoint();
        let (w, h) = (self.col_px(at.col) - 2.0, self.row_px(at.row) - 2.0);
        self.sheet_mut().shapes_new.push(kumihan::book::SheetShape {
            at,
            width_px: w,
            height_px: h,
            kind: kind.into(),
            fill: None,
            line: Some("1B6E3C".into()),
            // 組でこしらえた点を、形の点(制御点は無し=折れ線)へ包む。
            // **包むのは一番外で1回** — 分岐ごとに包むと読みにくい
            points: points
                .into_iter()
                .map(|(x, y)| kumihan::book::PathPoint::at(x, y))
                .collect(),
            base,
            ..Default::default()
        });
        self.dirty = true;
        let said = match kind {
            "spark-col" => ui::t!("column_sparkline"),
            "spark-wl" => ui::t!("win_loss_sparkline"),
            _ => ui::t!("line_sparkline"),
        };
        self.status = ui::tf!(
            "placed_fixed_picture_current",
            said, at.a1()
        )
        .into();
    }

    /// 重複の削除のパネル — 比べる列の入切と「先頭行は見出し」。
    pub(crate) fn dedup_pick(&mut self) {
        let Some((list, header)) = &self.dedup_pend else { return };
        let at = self.pop_anchor();
        // 列の名は帳票の見出しの字 — 訳さない。入切の印は見出しにだけ
        let mut items: Vec<(String, String)> = Vec::new();
        for (_, name, on) in list {
            items.push((
                name.clone(),
                format!("{} {}", if *on { "☑" } else { "☐" }, name),
            ));
        }
        // 下の2つは受け側(apply_pick)も同じ ui::t! で組み直して照合する —
        // 鍵と見出しを分けず、両端で同じ字にしておく
        let header_label = ui::t!("first_row_header_keep").to_string();
        items.push((
            header_label.clone(),
            format!("{} {}", if *header { "☑" } else { "☐" }, header_label),
        ));
        let del = format!("→ {}", ui::t!("delete"));
        items.push((del.clone(), del));
        self.pick_note = Some(ui::t!("remove_duplicates_columns_compare").into());
        self.pick_kind = "dedup-pick";
        self.pick = Some((items, at));
    }

    /// **スライサーの絞りを、繋いだピボットへ押し出す**(レポートの接続。
    /// 2026-08-21 の D群)。
    ///
    /// ピボットの絞りは「隠す値」で持っています。スライサーは逆に「選んだ値」
    /// なので、**その列に実際にある値から選んだ分を引いて**隠す値にします。
    /// 空の選択は素通しなので、隠す値も空にします。
    ///
    /// 見出しの字が繋いだピボットに無ければ、その1枚は飛ばします。元の表を
    /// 差し替えた後などに起こります — 黙って全部隠したピボットを作らないためです。
    pub(crate) fn slicer_push_to_pivots(&mut self, si: usize, cx: &mut Context<Self>) {
        let Some(sl) = self.slicers.get(si) else { return };
        if sl.pivots.is_empty() {
            return;
        }
        let col = sl.col;
        let sel = sl.sel.clone();
        let grain = sl.grain.clone();
        let names = sl.pivots.clone();
        let heading = self
            .sheet()
            .get(Pos::new(0, col))
            .map(|c| c.value.display())
            .unwrap_or_default();
        if heading.is_empty() {
            return;
        }
        let mut pressed = 0usize;
        for name in names {
            let Some(pi) = self.book.pivots.iter().position(|d| d.name == name) else { continue };
            // その見出しをピボットが使っていなければ触らない
            let in_use = {
                let d = &self.book.pivots[pi];
                d.rows_sel.contains(&heading) || d.cols_sel.contains(&heading)
            };
            if !in_use {
                continue;
            }
            // **値はピボットの元の表から集めます。** シート全体から集めると、
            // ピボットを置いた先の空欄まで「(空白)」として拾います
            // (2026-08-21 に試験で出ました)
            let (a, b) = self.book.pivots[pi].src;
            let Some(si2) = self.book.sheets.iter().position(|x| x.name == self.book.pivots[pi].sheet)
            else {
                continue;
            };
            let sh = &self.book.sheets[si2];
            let Some(c2) = (a.col..=b.col).find(|&c| {
                sh.get(Pos::new(a.row, c)).map(|x| x.value.display()).unwrap_or_default() == heading
            }) else {
                continue;
            };
            // ピボットの絞りは**生の値**で持ちます。粒(月・四半期・年)で
            // まとめているときは、選んだ束に入らない生の値を全部隠します
            let mut every: Vec<String> = (a.row + 1..=b.row)
                .map(|r| match sh.get(Pos::new(r, c2)).map(|x| x.value.display()) {
                    Some(v) if !v.is_empty() => v,
                    _ => ui::t!("blank").to_string(),
                })
                .collect();
            every.sort();
            every.dedup();
            let hide: Vec<String> = if sel.is_empty() {
                Vec::new()
            } else if grain.is_empty() {
                every.into_iter().filter(|v| !sel.contains(v)).collect()
            } else {
                let d1904 = self.book.date1904;
                every
                    .into_iter()
                    .filter(|v| {
                        let n = v.parse::<f64>().ok();
                        match crate::util::date_bucket(n, v, &grain, d1904) {
                            Some(b) => !sel.contains(&b),
                            None => true, // 日付として読めない行は隠す
                        }
                    })
                    .collect()
            };
            let d = &mut self.book.pivots[pi];
            d.hide.retain(|(f, _)| *f != heading);
            if !hide.is_empty() {
                d.hide.push((heading.clone(), hide));
            }
            let nd = d.clone();
            self.spawn_pivot(nd, Some(pi), cx);
            pressed += 1;
        }
        if pressed > 0 {
            self.status = ui::tf!("connected_pivottables_filtered_same", pressed).into();
        }
    }

    /// **日付の単位を次のものへ回す**(タイムライン)。
    ///
    /// 粒が変わると札の意味が変わるので、**選びは捨てます** — 「2026-08」を
    /// 選んだまま年に切り替えると、どの札にも当たらず全部消えます。
    pub(crate) fn slicer_cycle_grain(&mut self, cx: &mut Context<Self>) {
        // 順繰り: 値そのもの → 月 → 四半期 → 年 → 値そのもの
        let grains = crate::util::slicer_grains();
        let Some(sl) = self.slicers.get_mut(self.slicer_sel) else { return };
        let next_of = match grains.iter().position(|(k, _)| *k == sl.grain) {
            Some(i) if i + 1 < grains.len() => grains[i + 1].0,
            Some(_) => "",
            None => grains[0].0,
        };
        let k = next_of;
        sl.grain = k.to_string();
        sl.sel.clear();
        let grain = crate::util::slicer_grain_label(k);
        self.status = if k.is_empty() {
            ui::t!("listing_values_themselves_filter").into()
        } else {
            ui::tf!("grouping_dates_filter_cleared", grain).into()
        };
        self.slicer_push_to_pivots(self.slicer_sel, cx);
    }

    /// **レポートの接続** — このスライサーをどのピボットにつなぐかを選ぶ一覧。
    pub(crate) fn slicer_refs_pick(&mut self) {
        let Some(sl) = self.slicers.get(self.slicer_sel) else {
            self.status = ui::t!("select_slicer_first").into();
            return;
        };
        if self.book.pivots.is_empty() {
            self.status =
                ui::t!("workbook_no_pivottable_make").into();
            return;
        }
        let joint = sl.pivots.clone();
        let at = self.pop_anchor();
        // 鍵はピボットの名前。印は見出しにだけ付ける(照合は鍵で)
        let items: Vec<(String, String)> = self
            .book
            .pivots
            .iter()
            .map(|d| {
                let on = joint.contains(&d.name);
                (d.name.clone(), format!("{} {}", if on { "☑" } else { "☐" }, d.name))
            })
            .collect();
        self.pick_note =
            Some(ui::t!("filtering_slicer_filters_connected").into());
        self.pick_kind = "slicer-refs";
        self.pick = Some((items, at));
    }

    /// **おすすめのピボットを並べる。**候補が1つでもあれば真を返します。
    ///
    /// 候補の作り方は `util::pivot_suggestions` に純関数で置いてあります。
    /// 乱数も学習も使いません — 同じ表からは毎回同じ候補が出ます。
    pub(crate) fn pivot_suggest_pick(&mut self) -> bool {
        let Some(pend) = &self.pivot_pend else { return false };
        let (a, b) = (pend.a, pend.b);
        let headers = pend.headers.clone();
        let sh = self.sheet();
        let cols: Vec<Vec<String>> = (a.col..=b.col)
            .map(|c| {
                (a.row + 1..=b.row)
                    .map(|r| {
                        sh.get(Pos::new(r, c)).map(|x| x.value.display()).unwrap_or_default()
                    })
                    .collect()
            })
            .collect();
        let cands = crate::util::pivot_suggestions(&headers, &cols);
        if cands.is_empty() {
            return false;
        }
        let at = self.pop_anchor();
        // 鍵は番号。見出しの字は帳票の中身なので、鍵にすると訳の照合に紛れます
        let mut items: Vec<(String, String)> = cands
            .iter()
            .enumerate()
            .map(|(i, s)| (format!("#{i}"), crate::util::pivot_suggest_label(s)))
            .collect();
        items.extend(menu(&[ui::item!("choose_fields_myself_rows")]));
        self.pivot_suggests = cands;
        self.pick_note =
            Some(ui::t!("layouts_table_can_make").into());
        self.pick_kind = "pivot-suggest";
        self.pick = Some((items, at));
        true
    }

    pub(crate) fn pivot_pick(&mut self, kind: &'static str) {
        let Some(pend) = &self.pivot_pend else { return };
        let at = self.pop_anchor();
        // 見出しの字は帳票の中身 — 鍵も見出しもそのまま。印は見出しにだけ付ける
        let mut items: Vec<(String, String)> = Vec::new();
        let note: SharedString = match kind {
            "pivot-rows-pick" => {
                for h in &pend.headers {
                    let on = pend.rows_sel.contains(h);
                    items.push((
                        h.clone(),
                        format!("{} {}", if on { "☑" } else { "☐" }, h),
                    ));
                }
                items.extend(menu(&[ui::item!("next_choose_columns")]));
                ui::t!("pivot_1_4_headers").into()
            }
            "pivot-cols-pick" => {
                for h in pend.headers.iter().filter(|h| !pend.rows_sel.contains(h)) {
                    let on = pend.cols_sel.contains(h);
                    items.push((
                        h.clone(),
                        format!("{} {}", if on { "☑" } else { "☐" }, h),
                    ));
                }
                items.extend(menu(&[ui::item!("done_columns_optional")]));
                ui::t!("pivot_2_4_headers").into()
            }
            "pivot-val-pick" => {
                items.extend(plain(pend.headers.clone()));
                ui::t!("pivot_3_4_one").into()
            }
            _ => {
                // 集計の名はピボットの定義に書き込む字 — 鍵は訳さず、画面は見出し
                items.extend(menu(&pivot_aggs()));
                ui::tf!("pivot_4_4_how", pend.val_sel).into()
            }
        };
        self.pick_note = Some(note);
        self.pick_kind = kind;
        self.pick = Some((items, at));
    }

    /// 罫線を選択に掛ける(ペンの線種・色で)。which は一覧の項目名
    /// 線のスタイルのパネル(ペンに入る)。罫線パレットからも来る
    pub(crate) fn open_border_style_pick(&mut self) {
        let at = self.pop_anchor();
        // 「✓ 」は今のペンの印 — 見出しにだけ付ける(鍵は素のまま)
        let items: Vec<(String, String)> = border_styles()
            .iter()
            .map(|(k, l, b)| {
                let label =
                    if *b == self.pen_style { format!("✓ {l}") } else { l.to_string() };
                (k.to_string(), label)
            })
            .collect();
        self.pick_note = Some(ui::t!("line_style_goes_into").into());
        self.pick_kind = "border-style-pick";
        self.pick = Some((items, at));
    }

    /// 線の色のパネル(ペンに入る)。罫線パレットからも来る
    pub(crate) fn open_border_color_pick(&mut self) {
        let at = self.pop_anchor();
        let mut items: Vec<(String, String)> =
            font_colors().iter().map(|(k, l, _)| (k.to_string(), l.to_string())).collect();
        items.extend(menu(&[ui::item!("other_type_rrggbb")]));
        self.pick_note = Some(ui::t!("line_colour_goes_into").into());
        self.pick_kind = "border-color-pick";
        self.pick = Some((items, at));
    }

    pub(crate) fn apply_borders(&mut self, which: &str) {
        let (a, b) = self.sel_rect();
        let e = kumihan::book::Edge::line(self.pen_style, self.pen_color);
        self.checkpoint();
        let sh = &mut self.book.sheets[self.active];
        for r in a.row..=b.row {
            for c in a.col..=b.col {
                let p = Pos::new(r, c);
                let mut cell = sh.get(p).cloned().unwrap_or_default();
                let bd = &mut cell.fmt.borders;
                match which {
                    "bottom_border" => {
                        if r == b.row { bd.bottom = e }
                    }
                    "top_border" => {
                        if r == a.row { bd.top = e }
                    }
                    "left_border" => {
                        if c == a.col { bd.left = e }
                    }
                    "right_border" => {
                        if c == b.col { bd.right = e }
                    }
                    "outline" => {
                        if r == a.row { bd.top = e }
                        if r == b.row { bd.bottom = e }
                        if c == a.col { bd.left = e }
                        if c == b.col { bd.right = e }
                    }
                    "all_borders_grid" => {
                        *bd = kumihan::book::Borders {
                            top: e, bottom: e, left: e, right: e,
                        };
                    }
                    // 内側だけ(外周には引かない)— 帳票の中身の区切り
                    "inside_vertical_border" => {
                        if c > a.col { bd.left = e }
                        if c < b.col { bd.right = e }
                    }
                    "inside_horizontal_border" => {
                        if r > a.row { bd.top = e }
                        if r < b.row { bd.bottom = e }
                    }
                    _ => *bd = kumihan::book::Borders::NONE, // 罫線を消す
                }
                sh.set(p, cell);
            }
        }
        self.dirty = true;
        // 引き当ては鍵で済んだ。報せるのは**見出し**(訳された名前)
        let label = crate::util::border_kind_label(which);
        self.status = ui::tf!("borders_applied_ctrl_z", label, a.a1(), b.a1()).into();
    }

    /// 「データの入力規則」のパネルを開く(いまの規則を下敷きに)
    pub(crate) fn dv_open(&mut self) {
        let v = self.sheet().validation_at(self.cursor).cloned();
        let mut d = DvDlg {
            tab: 0,
            kind: 0,
            op: 0,
            allow_blank: true,
            apply_same: false,
            hide_arrow: false,
            err_style: 0,
            eds: std::array::from_fn(|_| Editor::new("")),
            focus: 0,
            menu: 0,
            keep: None,
            was: v.clone(),
        };
        if let Some(v) = &v {
            d.allow_blank = v.allow_blank;
            d.hide_arrow = v.hide_arrow;
            if let Some((t, m)) = &v.input_msg {
                d.eds[2] = Editor::new(t);
                d.eds[3] = Editor::new(m);
            }
            if let Some((s, t, m)) = &v.error_msg {
                d.err_style = dv_styles().iter().position(|(k, _)| k == s).unwrap_or(0);
                d.eds[4] = Editor::new(t);
                d.eds[5] = Editor::new(m);
            }
            match v.kind.as_str() {
                "" => d.kind = 0,
                "whole" => d.kind = 1,
                "decimal" => d.kind = 2,
                "list" => d.kind = 3,
                "textLength" => d.kind = 4,
                // 日付・時刻・カスタムは判定できない — 開いても壊さない(保持)
                _ => {
                    d.kind = 5;
                    d.keep = Some(v.clone());
                }
            }
            if d.kind == 3 {
                // 直書きは中身、参照は = 付き(従来の書き方)
                let f = v.formula.trim();
                let init = match f.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                    Some(inner) => inner.to_string(),
                    None if f.is_empty() => String::new(),
                    None => format!("={f}"),
                };
                d.eds[0] = Editor::new(&init);
            } else if matches!(d.kind, 1 | 2 | 4) {
                d.op = dv_ops().iter().position(|(k, _)| *k == v.op).unwrap_or(0);
                d.eds[0] = Editor::new(&v.formula);
                d.eds[1] = Editor::new(&v.formula2);
            }
        }
        self.dv_dlg = Some(d);
    }

    /// 「データの入力規則」の OK。選択の範囲に規則を掛ける(重なる規則は
    /// 入れ替え)。読めない条件はパネルを開いたまま言い返す
    /// **入力規則の小窓のドロップダウンを鍵で動かす**(2026-08-20。手順3)。
    ///
    /// 前はマウスでしか選べませんでした。リボンの一覧は前から ↑↓・Enter・
    /// Esc で完結するので、小窓だけ取り残されていました。
    ///
    /// 開いているドロップダウンの選択を1つ送ります。動かしたら真。
    pub(crate) fn dv_menu_move(&mut self, downward: bool) -> bool {
        let Some(d) = &mut self.dv_dlg else { return false };
        let n = match d.menu {
            1 => crate::util::dv_kinds().len(),
            2 => crate::util::dv_ops().len(),
            3 => crate::util::dv_styles().len(),
            _ => return false,
        };
        let now = match d.menu {
            1 => &mut d.kind,
            2 => &mut d.op,
            _ => &mut d.err_style,
        };
        // 端では止めます(巡回しません — どちらが端か分からなくなるため)
        *now = if downward { (*now + 1).min(n - 1) } else { now.saturating_sub(1) };
        true
    }

    /// **開いているドロップダウンを Enter で閉じる**(選んだ物はもう入っている)。
    /// 閉じたら真。
    pub(crate) fn dv_menu_enter(&mut self) -> bool {
        let Some(d) = &mut self.dv_dlg else { return false };
        if d.menu == 0 {
            return false;
        }
        d.menu = 0;
        true
    }

    /// **Esc で閉じるのはドロップダウンだけ**(小窓は残す)。閉じたら真。
    pub(crate) fn dv_menu_esc(&mut self) -> bool {
        self.dv_menu_enter()
    }

    pub(crate) fn dv_ok(&mut self, cx: &mut Context<Self>) {
        let Some(mut d) = self.dv_dlg.take() else { return };
        let f = |i: usize| -> String { d.eds[i].text().trim().to_string() };
        let (a, b) = self.sel_rect();
        let input_msg = {
            let (t, m) = (f(2), f(3));
            (!t.is_empty() || !m.is_empty()).then_some((t, m))
        };
        let error_msg = {
            let (t, m) = (f(4), f(5));
            (!t.is_empty() || !m.is_empty())
                .then_some((dv_styles()[d.err_style].0.to_string(), t, m))
        };
        let new_v: Option<kumihan::book::Validation> = match d.kind {
            // 読めない種類(日付など)はそのまま保つ — 文言と空白の扱いだけ更新
            5 => d.keep.take().map(|mut v| {
                v.range = (a, b);
                v.input_msg = input_msg.clone();
                v.error_msg = error_msg.clone();
                v.allow_blank = d.allow_blank;
                v
            }),
            // すべての値: 条件なし。文言も何も無ければ「規則なし」= 外す
            0 => (input_msg.is_some() || error_msg.is_some() || !d.allow_blank).then(|| {
                let mut v = kumihan::book::Validation::list((a, b), String::new());
                v.kind = String::new();
                v.input_msg = input_msg.clone();
                v.error_msg = error_msg.clone();
                v.allow_blank = d.allow_blank;
                v
            }),
            // リスト: 直書き(甲,乙,丙)か範囲の参照(=D2:D5)
            3 => {
                let text = f(0);
                if text.is_empty() {
                    self.status =
                        ui::t!("write_source_values_e").into();
                    self.dv_dlg = Some(d);
                    cx.notify();
                    return;
                }
                let formula = match text.strip_prefix('=') {
                    Some(r) => r.trim().to_string(),
                    None => format!("\"{}\"", text.replace('"', "")),
                };
                let mut v = kumihan::book::Validation::list((a, b), formula);
                if v.options(self.sheet()).is_empty() {
                    // 読めない規則を作らない(できないものを、できるように見せない)
                    self.status =
                        ui::t!("cant_read_choices_e").into();
                    self.dv_dlg = Some(d);
                    cx.notify();
                    return;
                }
                v.input_msg = input_msg.clone();
                v.error_msg = error_msg.clone();
                v.allow_blank = d.allow_blank;
                v.hide_arrow = d.hide_arrow;
                Some(v)
            }
            // 整数 / 小数 / 文字数
            k => {
                let ops = dv_ops();
                let (opk, _) = ops[d.op.min(ops.len() - 1)];
                let need2 = matches!(opk, "between" | "notBetween");
                // IME の全角の数・記号は半角にならす(打ち直させない)
                let norm = |t: String| -> String {
                    t.chars()
                        .map(|c| match c {
                            '\u{FF10}'..='\u{FF19}' => {
                                char::from(b'0' + (c as u32 - 0xFF10) as u8)
                            }
                            '\u{FF0E}' => '.',
                            '\u{FF0D}' | '\u{2212}' => '-',
                            _ => c,
                        })
                        .collect()
                };
                let (f1, f2) = (norm(f(0)), norm(f(1)));
                if f1.is_empty() || (need2 && f2.is_empty()) {
                    self.status = ui::t!("write_condition_value_ascii").into();
                    self.dv_dlg = Some(d);
                    cx.notify();
                    return;
                }
                Some(kumihan::book::Validation {
                    range: (a, b),
                    formula: f1,
                    kind: DV_KIND_XLSX[k].into(),
                    op: opk.into(),
                    formula2: if need2 { f2 } else { String::new() },
                    input_msg: input_msg.clone(),
                    error_msg: error_msg.clone(),
                    allow_blank: d.allow_blank,
                    hide_arrow: d.hide_arrow,
                })
            }
        };
        self.checkpoint();
        // 「同じ設定の他のすべてのセル」: 開いたときの規則と同じ条件の規則を
        // 先に差し替える(範囲は据え置き)
        if d.apply_same {
            if let (Some(was), Some(nv)) = (&d.was, &new_v) {
                for x in self.book.sheets[self.active].validations.iter_mut() {
                    if x.kind == was.kind
                        && x.op == was.op
                        && x.formula == was.formula
                        && x.formula2 == was.formula2
                    {
                        let range = x.range;
                        *x = nv.clone();
                        x.range = range;
                    }
                }
            }
        }
        // 選択に重なる規則は入れ替える(重ね掛けは分かりにくい)
        let overlap = |x: &kumihan::book::Validation| {
            let (ra, rb) = x.range;
            ra.row <= b.row && rb.row >= a.row && ra.col <= b.col && rb.col >= a.col
        };
        let had = self.sheet().validations.iter().any(&overlap);
        self.book.sheets[self.active].validations.retain(|x| !overlap(x));
        match new_v {
            Some(v) => {
                let said = if v.kind == "list" {
                    ui::tf!("options", v.options(self.sheet()).join(" / ")).to_string()
                } else if v.kind.is_empty() {
                    ui::t!("message_only_values_not").to_string()
                } else {
                    v.describe()
                };
                self.book.sheets[self.active].validations.push(v);
                self.status = ui::tf!(
                    "validation_applied_kept_xlsx",
                    a.a1(), b.a1(), said
                )
                .into();
            }
            None if had => {
                self.status = ui::t!("validation_removed_range").into();
            }
            None => {
                self.undo_stack.pop();
                self.status = ui::t!("no_validation_rule_nothing").into();
            }
        }
        self.dirty = true;
        cx.notify();
    }

    /// ▼のパネルの開閉(見出しのボタンから。同じ列ならしまう)
    pub(crate) fn toggle_filter_panel(&mut self, col: u32) {
        match &self.filter_panel {
            Some((c, _)) if *c == col => self.filter_panel = None,
            _ => self.filter_panel = Some((col, Editor::new(""))),
        }
    }

    /// ▼のパネル: 値ひとつの入切。空になったらその列は素通しに戻す
    pub(crate) fn filter_toggle_value(&mut self, col: u32, v: &str) {
        let Some(f) = &mut self.auto_filter else { return };
        let set = f.hide.entry(col).or_default();
        if !set.remove(v) {
            set.insert(v.to_string());
        }
        if set.is_empty() {
            f.hide.remove(&col);
        }
        self.filter_note();
    }

    /// ▼のパネル: (すべて選択)。全部見えていれば全部隠し、そうでなければ全部見せる
    pub(crate) fn filter_toggle_all(&mut self, col: u32, all: Vec<String>) {
        let Some(f) = &mut self.auto_filter else { return };
        if f.hide.remove(&col).is_none() {
            f.hide.insert(col, all.into_iter().collect());
        }
        self.filter_note();
    }

    /// ▼のパネル: この列の絞り込みを解く
    pub(crate) fn filter_clear_col(&mut self, col: u32) {
        if let Some(f) = &mut self.auto_filter {
            f.hide.remove(&col);
        }
        self.filter_note();
    }

    /// 絞り込みの操作のたびに、いま何行見えているかを状態行で言う
    fn filter_note(&mut self) {
        self.status = match self.filter_counts() {
            Some((total, shown)) => {
                ui::tf!("filtering_rows_shown_display", total, shown).into()
            }
            None => ui::t!("no_filter_everything_visible_2").into(),
        };
    }

    pub(crate) fn a_cancel(&mut self, _: &ui::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        // **開いているドロップダウンだけ閉じる**(小窓は残す。手順3)。
        // Esc で小窓ごと消えると、打ち込んだ設定まで捨てることになります
        if self.dv_menu_esc() {
            cx.notify();
            return;
        }
        // .py の編集面。書きかけがあれば一度断る(黙って捨てない)
        if self.py_edit.is_some() {
            self.close_py_edit();
            cx.notify();
            return;
        }
        if self.quit_ask {
            self.quit_ask = false;
            self.status = ui::t!("quit_cancelled").into();
            cx.notify();
            return;
        }
        // 名前ボックス・関数の小窓は最優先で閉じる
        if self.name_edit.take().is_some()
            || self.fn_args.take().is_some()
            || self.fn_dlg.take().is_some()
        {
            cx.notify();
            return;
        }
        // 会話の欄は Esc で焦点を返す(パネルは開いたまま — 表へ戻るだけ)
        if self.chat_focus {
            self.chat_focus = false;
            self.status = ui::t!("back_sheet_conversation_panel").into();
            cx.notify();
            return;
        }
        // キーヒントはいちばん先に畳む(重なっている物の最前)
        if self.key_hint.take().is_some() {
            self.status = ui::t!("key_hints_closed").into();
            cx.notify();
            return;
        }
        // 絞り込みつきの一覧(書体・入力規則)は Esc で検索欄ごと閉じる。
        // 素の一覧は下の `||` の列で従来どおり閉じる
        if self.pick_filtering() {
            self.close_pick();
            cx.notify();
            return;
        }
        // 入力のパネル → 一覧 → 子メニュー → 親メニュー → 書式の小窓 → コピーの破線、
        // の順で閉じる
        self.pivot_pend = None; // 聞き取り途中のピボット・小計は Esc でやめる
        self.sub_pend = None;
        self.sort_pend = None; // 並べ替えの「拡張しますか」も
        self.pivot_flt = None; // ピボットの絞り込みの聞き取りも
        self.hf_pend = None; // ヘッダー/フッターの聞き取りも
        self.name_pend = None; // 名前マネージャーの選択も
        if self.brush.take().is_some() {
            self.status = ui::t!("format_painting_cancelled").into();
        }
        self.menu_head = None; // 見出しメニューの印も畳む
        // 親を通らずに開いた子の品書きは、子と親をまとめて閉じる。
        // 片方ずつ閉じると押した覚えのない親が出てくる
        if self.menu_direct && self.menu_sub.take().is_some() {
            self.menu_at = None;
            self.menu_direct = false;
            cx.notify();
            return;
        }
        self.dedup_pend = None;
        self.cond_pend = None;
        self.import_pend = None;
        self.border_pal = None;

        self.pw_pending = None; // パスワード待ちも Esc でやめる(開かない)
        self.pw_first = None; // 初回の控えも捨てる(途中でやめたら残さない)
        // 入力規則のパネル: 開いたドロップダウン → パネル、の順で閉じる
        if let Some(d) = &mut self.dv_dlg {
            if d.menu != 0 {
                d.menu = 0;
            } else {
                self.dv_dlg = None;
                self.status = ui::t!("validation_cancelled").into();
            }
            cx.notify();
            return;
        }
        if self.tool.take().is_some() {
            self.ink_cur = None;
            self.status = ui::t!("back_cell_operations").into();
        }
        self.shape_multi.clear();
        if self.filter_panel.take().is_some()
            || self.solver.take().is_some()
            // Esc は**設定の板 → スライサーの板を1枚**の順で閉じる。
            // 何枚でも開ける造りなので、まとめて畳むと押し間違いで全部消える
            || std::mem::take(&mut self.slicer_cfg)
            || self.close_slicer()
            || self.prompt.take().is_some()
            || self.pick.take().is_some()
            || self.menu_sub.take().is_some()
            || self.menu_at.take().is_some()
            || self.fmt_panel.take().is_some()
            || self.clip_range.take().is_some()
            || self.shape_sel.take().is_some()
            || self.img_sel.take().is_some()
        {
            // 一覧・パネルを閉じたら意味づけも戻す(タブのメニューの狙い先も)
            self.pick_kind = "value";
            self.pick_note = None;
            self.sheet_menu_at = None;
            cx.notify();
        } else if self.editing() {
            // 打ちかけを捨てて、セルの保存内容に戻す
            // (入力規則で堰き止められたときの逃げ道でもある)
            self.sync_input();
            self.status = ui::t!("input_cancelled").into();
            cx.notify();
        } else if self.edit_armed {
            // F2 だけ押して何も打っていない — 編集をやめる
            self.edit_armed = false;
            cx.notify();
        }
    }

    /// 入力のパネルを確定する(Enter)。
    pub(crate) fn finish_prompt(&mut self, cx: &mut Context<Self>) {
        let Some((kind, ed)) = self.prompt.take() else { return };
        let text = ed.text().trim().to_string();
        match kind {
            // テキスト取り込み: その他の区切り(1文字)
            "csv-delim" => {
                let t = text.trim();
                let Some(c0) = t.chars().next() else {
                    self.status = ui::t!("type_one_delimiter_character").into();
                    self.prompt = Some((kind, Editor::new("")));
                    return;
                };
                if let Some(pend) = &mut self.import_pend {
                    pend.custom = c0.to_string();
                }
                self.import_reparse(cx);
            }
            // テキスト取り込み: 置き場所(A1 の形)
            "csv-dest" => {
                let Some(p2) = Pos::parse(text.trim()) else {
                    self.status = ui::t!("not_valid_cell_use").into();
                    self.prompt = Some((kind, Editor::new(text.trim())));
                    return;
                };
                if let Some(pend) = &mut self.import_pend {
                    pend.dest = p2;
                }
                self.import_pick();
            }
            // 反復計算の入切(回数 変化量。空 Enter = 切)
            "calc-iter" => {
                let t = text.trim();
                if t.is_empty() {
                    self.book.calc_iter = None;
                    self.dirty = true;
                    self.status = ui::t!("iterative_calculation_off_circular").into();
                    return;
                }
                let mut it = t.split_whitespace();
                let n: Option<u32> = it.next().and_then(|v| v.parse().ok());
                let d: Option<f64> = match it.next() {
                    Some(v) => v.parse().ok(),
                    None => Some(0.001),
                };
                match (n, d) {
                    (Some(n), Some(d)) if n >= 1 && d >= 0.0 => {
                        self.book.calc_iter = Some((n, d));
                        self.dirty = true;
                        recalc_book(&mut self.book, self.active);
                        self.sync_input();
                        self.status = ui::tf!(
                            "iterative_calculation_max_passes",
                            n, d
                        )
                        .into();
                    }
                    _ => {
                        self.status = ui::t!("use_form_100_0").into();
                        self.prompt = Some((kind, Editor::new(t)));
                    }
                }
            }
            // ピボット: ラベルで絞る(含む/で始まる/で終わる 語)。
            // 合う値**以外**を hide に落とす — 既存の絞り込み機構に乗せる
            "pivot-label" => {
                let Some((pi, field, _)) = self.pivot_flt.take() else { return };
                let t = text.trim();
                if t.is_empty() {
                    self.status = ui::t!("condition_empty_e_g").into();
                    return;
                }
                let (op, word) = match t.split_once(char::is_whitespace) {
                    Some((a, b)) if ["含む", "で始まる", "で終わる"].contains(&a) => {
                        (a.to_string(), b.trim().to_string())
                    }
                    _ => ("含む".into(), t.to_string()),
                };
                let ok = |v: &str| match op.as_str() {
                    "で始まる" => v.starts_with(&word),
                    "で終わる" => v.ends_with(&word),
                    _ => v.contains(&word),
                };
                let vals = self.pivot_field_values(pi, &field);
                let hidden: Vec<String> = vals.into_iter().filter(|v| !ok(v)).collect();
                if let Some(d) = self.book.pivots.get_mut(pi) {
                    d.hide.retain(|(f, _)| *f != field);
                    if !hidden.is_empty() {
                        d.hide.push((field, hidden));
                    }
                    let nd = d.clone();
                    self.spawn_pivot(nd, Some(pi), cx);
                }
            }
            // ピボット: 値で絞る(> 1000 の形。空 Enter = 解除)
            "pivot-vfilter" => {
                let Some((pi, _, _)) = self.pivot_flt.take() else { return };
                let t = text.trim();
                let Some(d) = self.book.pivots.get_mut(pi) else { return };
                if t.is_empty() {
                    d.vfilter = None;
                } else {
                    let (op, num) = match t.split_once(char::is_whitespace) {
                        Some((a, b)) if [">", ">=", "<", "<=", "="].contains(&a) => {
                            (a.to_string(), b.trim())
                        }
                        _ => {
                            self.status =
                                ui::t!("use_form_1000_operators").into();
                            self.prompt = Some((kind, Editor::new(t)));
                            self.pivot_flt = Some((pi, String::new(), Default::default()));
                            return;
                        }
                    };
                    let Ok(th) = num.parse::<f64>() else {
                        self.status = ui::t!("threshold_not_number").into();
                        self.prompt = Some((kind, Editor::new(t)));
                        self.pivot_flt = Some((pi, String::new(), Default::default()));
                        return;
                    };
                    d.vfilter = Some((op, th));
                }
                let nd = d.clone();
                self.spawn_pivot(nd, Some(pi), cx);
            }
            // ピボット: 数の幅でグループ化(例: 100)
            "pivot-group-width" => {
                let Some((pi, field, _)) = self.pivot_flt.take() else { return };
                let Ok(w) = text.trim().parse::<f64>() else {
                    self.status = ui::t!("width_not_number_e").into();
                    self.prompt = Some((kind, Editor::new(text.trim())));
                    self.pivot_flt = Some((pi, field, Default::default()));
                    return;
                };
                if w <= 0.0 {
                    self.status = ui::t!("width_must_greater_than").into();
                    self.prompt = Some((kind, Editor::new(text.trim())));
                    self.pivot_flt = Some((pi, field, Default::default()));
                    return;
                }
                let Some(d) = self.book.pivots.get_mut(pi) else { return };
                d.group_by.retain(|(f, _)| *f != field);
                d.group_by.push((field, format!("幅:{w}")));
                let nd = d.clone();
                self.spawn_pivot(nd, Some(pi), cx);
            }
            // 列の幅・行の高さの数値指定(選んだ列・行ぶん。空 = 既定に戻す)
            "col-width" | "row-height" => {
                let is_col = kind == "col-width";
                let (a, b) = self.sel_rect();
                let t = text.trim();
                if t.is_empty() {
                    self.checkpoint();
                    if is_col {
                        for c in a.col..=b.col {
                            self.sheet_mut().col_width.remove(&c);
                        }
                    } else {
                        for r in a.row..=b.row {
                            self.sheet_mut().row_height.remove(&r);
                        }
                    }
                    self.dirty = true;
                    self.status = ui::t!("reset_default_size").into();
                    return;
                }
                let Ok(v) = t.parse::<f32>() else {
                    self.status = ui::t!("use_ascii_digits_e").into();
                    self.prompt = Some((kind, Editor::new(t)));
                    return;
                };
                let ok = if is_col { (0.0..=255.0).contains(&v) } else { (0.0..=409.0).contains(&v) };
                if !ok {
                    self.status = if is_col {
                        ui::t!("column_width_must_0").into()
                    } else {
                        ui::t!("row_height_must_0").into()
                    };
                    self.prompt = Some((kind, Editor::new(t)));
                    return;
                }
                self.checkpoint();
                if is_col {
                    for c in a.col..=b.col {
                        self.sheet_mut().col_width.insert(c, v);
                    }
                    self.status = ui::tf!("column_width_set_columns", v, b.col - a.col + 1).into();
                } else {
                    for r in a.row..=b.row {
                        self.sheet_mut().row_height.insert(r, v);
                    }
                    self.status = ui::tf!("row_height_set_pt", v, b.row - a.row + 1).into();
                }
                self.dirty = true;
            }
            // 名前の中身の打ち直し(A1 か A1:C9 の形)
            "name-range" => {
                let Some(name) = self.name_pend.take() else { return };
                let t = text.trim().to_uppercase();
                let ok = match t.split_once(':') {
                    Some((a, b)) => Pos::parse(a).is_some() && Pos::parse(b).is_some(),
                    None => Pos::parse(&t).is_some(),
                };
                if !ok {
                    self.status = ui::t!("cant_read_location_b12").into();
                    self.name_pend = Some(name);
                    self.prompt = Some(("name-range", Editor::new(&t)));
                    return;
                }
                self.checkpoint();
                let s = &mut self.book.sheets[self.active];
                if let Some(e) = s.names.iter_mut().find(|d| d.name == name) {
                    e.range = t.clone();
                }
                recalc_book(&mut self.book, self.active);
                self.dirty = true;
                self.status = ui::tf!("name_set", name, t).into();
            }
            // ヘッダー/フッターの1区分(空 Enter = その区分を消す)
            "hf-edit" => {
                let Some((footer, slot)) = self.hf_pend.take() else { return };
                self.checkpoint();
                let raw = if footer {
                    self.sheet().footer.clone()
                } else {
                    self.sheet().header.clone()
                };
                let (mut l, mut c, mut r) =
                    kumihan::book::hf_split(raw.as_deref().unwrap_or(""));
                match slot { 0 => l = text.clone(), 1 => c = text.clone(), _ => r = text.clone() }
                let joined = kumihan::book::hf_join(&l, &c, &r);
                let val = if joined.is_empty() { None } else { Some(joined) };
                if footer {
                    self.sheet_mut().footer = val;
                } else {
                    self.sheet_mut().header = val;
                }
                self.dirty = true;
                self.status = if text.is_empty() {
                    ui::t!("section_removed").into()
                } else {
                    ui::t!("added_header_footer_visible").into()
                };
            }
            // 文字の色・塗りの直指定(RRGGBB)。空 Enter = 自動/塗りなし
            // Unicode の 16 進で記号を入れる(本家の「文字コード」欄)
            "symbol-hex" => {
                let t = text.trim().trim_start_matches("U+").trim_start_matches("u+");
                match u32::from_str_radix(t, 16).ok().and_then(char::from_u32) {
                    Some(ch) => {
                        let s = ch.to_string();
                        self.input.insert(&s);
                        self.dirty = true;
                        self.recent_symbols.retain(|x| *x != s);
                        self.recent_symbols.insert(0, s.clone());
                        self.recent_symbols.truncate(12);
                        self.status =
                            ui::tf!("inserted_u", s, t.to_uppercase()).into();
                    }
                    None => {
                        // **黙って何も入れない、をしない**
                        self.status =
                            ui::t!("cant_read_unicode_hexadecimal").into();
                        self.prompt = Some(("symbol-hex", Editor::new(t)));
                    }
                }
            }
            "font-color-rgb" | "fill-color-rgb" => {
                let is_font = kind == "font-color-rgb";
                let t = text.trim().trim_start_matches('#').to_uppercase();
                if t.is_empty() {
                    if is_font {
                        self.fmt(|f| f.color = None);
                        self.status = ui::t!("font_colour_reset_automatic").into();
                    } else {
                        self.fmt(|f| f.fill = None);
                        self.status = ui::t!("fill_removed").into();
                    }
                } else if t.len() == 6 && u32::from_str_radix(&t, 16).is_ok() {
                    let c = Some(t.clone());
                    if is_font {
                        self.fmt(move |f| f.color = c.clone());
                        self.status = ui::tf!("font_colour_set", format!("#{t}")).into();
                    } else {
                        self.fmt(move |f| f.fill = c.clone());
                        self.status = ui::tf!("fill_set", format!("#{t}")).into();
                    }
                } else {
                    self.status = ui::t!("cant_read_colour_6").into();
                    self.prompt = Some((kind, Editor::new(&t)));
                }
            }
            // コメントに書き残す名乗り。器は settings.toml(言語と同じ所)
            "user-name" => {
                let t = text.trim();
                ui::settings::set("user_name", t);
                self.status = if t.is_empty() {
                    ui::t!("stay_anonymous_no_name").into()
                } else {
                    ui::tf!("comments_write_now_signed", t).into()
                };
            }
            // コメントへの返信。**筋の後ろに足す**(頭の文は書き換えない)
            "comment-reply" => {
                let t = text.trim().to_string();
                if t.is_empty() {
                    self.status = ui::t!("reply_empty_nothing_added").into();
                    return;
                }
                let p = self.cursor;
                let Some(th) = self.book.sheets[self.active].comments.get_mut(&p) else {
                    self.status = ui::t!("cell_no_comment").into();
                    return;
                };
                // 名乗りは共同編集の名前を使う。無ければ空のまま —
                // **「不明」のような名前を作らない**
                let who = ui::comment_author();
                th.entries.push(kumihan::book::CommentEntry { who, when: String::new(), text: t });
                self.dirty = true;
                self.status =
                    ui::tf!("replied_comment_kept_save", p.a1()).into();
            }
            // 柄の地の色(patternFill の bgColor)。柄が掛かっているときだけ意味を持つ
            "fill-bg-rgb" => {
                let t = text.trim().trim_start_matches('#').to_uppercase();
                if t.is_empty() {
                    self.fmt(|f| f.fill_bg = Some("FFFFFF".into()));
                    self.status = ui::t!("pattern_background_back_white").into();
                } else if t.len() == 6 && u32::from_str_radix(&t, 16).is_ok() {
                    let c = Some(t.clone());
                    self.fmt(move |f| f.fill_bg = c.clone());
                    self.status = ui::tf!("pattern_background_set", format!("#{t}")).into();
                } else {
                    self.status = ui::t!("cant_read_colour_6").into();
                    self.prompt = Some((kind, Editor::new(&t)));
                }
            }
            // 文字の角度の直指定(-90〜90。xlsx の encode は負を 90+|d| で)
            "text-angle" => {
                let t = text.trim().replace('°', "");
                match t.parse::<i32>() {
                    Ok(d) if (-90..=90).contains(&d) => {
                        let enc: Option<i32> = if d == 0 {
                            None
                        } else if d > 0 {
                            Some(d)
                        } else {
                            Some(90 - d) // -30 → 120(xlsx の encode)
                        };
                        self.fmt(move |f| f.rotation = enc);
                        self.status = if d == 0 {
                            ui::t!("text_orientation_reset").into()
                        } else {
                            ui::tf!("text_rotated_degrees_positive", d).into()
                        };
                    }
                    _ => {
                        self.status =
                            ui::t!("cant_read_angle_number").into();
                        self.prompt = Some(("text-angle", Editor::new(&t)));
                    }
                }
            }
            // 罫線の色の直指定(RRGGBB)。空 Enter = 自動(黒)
            "border-color-rgb" => {
                let t = text.trim().trim_start_matches('#').to_string();
                if t.is_empty() {
                    self.pen_color = None;
                    self.status = ui::t!("line_colour_automatic_black").into();
                } else if t.len() == 6 {
                    if let Ok(v) = u32::from_str_radix(&t, 16) {
                        self.pen_color = Some(v);
                        self.status = ui::tf!("line_colour_applies_when_draw", t.to_uppercase()).into();
                    } else {
                        self.status = ui::t!("cant_read_colour_6").into();
                        self.prompt = Some(("border-color-rgb", Editor::new(&t)));
                    }
                } else {
                    self.status = ui::t!("cant_read_colour_6").into();
                    self.prompt = Some(("border-color-rgb", Editor::new(&t)));
                }
            }
            // カスタムの数値書式(xlsx のコードをそのまま)。空 Enter = 一般に戻す
            "numfmt-custom" => {
                if text.is_empty() {
                    self.fmt(|f| f.number_format = None);
                    self.status = ui::t!("number_format_reset_general").into();
                } else {
                    let code = text.clone();
                    self.fmt(move |f| f.number_format = Some(code.clone()));
                    self.status = ui::tf!(
                        "number_format_code_set",
                        text
                    )
                    .into();
                }
            }
            // 並べ替えの基準(複数可)。「見出し名か列の字 [昇順|降順]」を
            // カンマ区切りで。向きを省けば昇順
            "sort-by" => {
                if text.is_empty() {
                    self.status = ui::t!("sort_cancelled").into();
                    return;
                }
                let (_, cols) = self.sheet().extent();
                let heads: Vec<String> = (0..cols)
                    .map(|c| {
                        self.sheet()
                            .get(Pos::new(0, c))
                            .map(|x| x.value.display())
                            .unwrap_or_default()
                    })
                    .collect();
                let mut keys: Vec<(u32, bool)> = Vec::new();
                let mut names: Vec<String> = Vec::new();
                for raw in text.split([',', '、']) {
                    let t = raw.trim();
                    if t.is_empty() {
                        continue;
                    }
                    let low = t.to_lowercase();
                    // **打つのは利用者なので、鍵ではなく訳と比べます**
                    // (2026-08-26)。日本語の画面では「区分 降順」と打ちます
                    let (name, asc) = if let Some(n) = t.strip_suffix(ui::t!("descending")) {
                        (n.trim(), false)
                    } else if let Some(n) = t.strip_suffix(ui::t!("ascending")) {
                        (n.trim(), true)
                    } else if low.ends_with("desc") {
                        (t[..t.len() - 4].trim_end(), false)
                    } else if low.ends_with("asc") {
                        (t[..t.len() - 3].trim_end(), true)
                    } else {
                        (t, true)
                    };
                    let col = heads
                        .iter()
                        .position(|h| h == name)
                        .map(|i| i as u32)
                        .or_else(|| {
                            // 列の字(A・B・AA…)でも指せる
                            if !name.is_empty()
                                && name.chars().all(|c| c.is_ascii_alphabetic())
                            {
                                Pos::parse(&format!("{}1", name.to_uppercase())).map(|p| p.col)
                            } else {
                                None
                            }
                        });
                    let Some(col) = col else {
                        // 打ち直せるようにパネルを開いたまま返す
                        self.prompt = Some(("sort-by", ed));
                        self.status = ui::tf!(
                            "no_header_named_available",
                            name,
                            heads.iter().filter(|h| !h.is_empty()).cloned()
                                .collect::<Vec<_>>().join(" / ")
                        )
                        .into();
                        return;
                    };
                    keys.push((col, asc));
                    names.push(format!(
                        "{} {}",
                        if heads.get(col as usize).map(|h| !h.is_empty()).unwrap_or(false) {
                            heads[col as usize].clone()
                        } else {
                            Pos::new(0, col).a1().trim_end_matches('1').to_string()
                        },
                        if asc { ui::t!("ascending") } else { ui::t!("descending") }
                    ));
                }
                if keys.is_empty() {
                    self.status = ui::t!("sort_cancelled").into();
                    return;
                }
                self.checkpoint();
                self.book.sheets[self.active].sort_by_columns(&keys, true);
                recalc_book(&mut self.book, self.active);
                self.dirty = true;
                self.sync_input();
                self.status = ui::tf!(
                    "sorted_header_row_kept",
                    names.join(" → ")
                )
                .into();
            }
            "sheet-rename" => {
                let Some(t) = self.sheet_menu_at.take() else { return };
                if t >= self.book.sheets.len() {
                    return;
                }
                let old = self.book.sheets[t].name.clone();
                if text.is_empty() || text == old {
                    self.status = ui::t!("name_unchanged").into();
                    return;
                }
                // xlsx のシート名の決まり: 31字まで・: \\ / ? * [ ] は使えない
                if text.chars().count() > 31
                    || text.contains([':', '\\', '/', '?', '*', '[', ']'])
                {
                    self.status = ui::tf!("cannot_sheet_name_up", text)
                    .into();
                    return;
                }
                if self.book.sheets.iter().enumerate().any(|(i, s)| i != t && s.name == text) {
                    self.status = ui::tf!("already_exists", text).into();
                    return;
                }
                self.checkpoint_book(); // 名前と式の書き換えを1手で戻せる
                // 文字列の中(INDIRECT("古!A1") 等)は**書き換えない** —
                // Excel も追随させないし、文字列は data であって参照ではない。
                // ただし黙って壊さない: 残る数を数えて言う
                let stale = stale_in_strings(&self.book, &old);
                let n = rename_sheet_refs(&mut self.book, &old, &text);
                self.book.sheets[t].name = text.clone();
                recalc_book(&mut self.book, t);
                self.dirty = true;
                let head = if n > 0 {
                    ui::tf!("renamed_formula_references_updated", old, text, n)
                        .to_string()
                } else {
                    ui::tf!("renamed_2", old, text).to_string()
                };
                self.status = if stale > 0 {
                    ui::tf!(
                        "but_inside_strings_indirect",
                        head, old, stale
                    )
                    .into()
                } else {
                    head.into()
                };
            }
            "name" => {
                if text.is_empty() {
                    self.status = ui::t!("no_name_given").into();
                    return;
                }
                let ok = text.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && !text.chars().next().unwrap().is_ascii_digit()
                    && Pos::parse(&text).is_none();
                if !ok {
                    self.status = ui::tf!("cant_name_letters_digits", text)
                    .into();
                    return;
                }
                let (a, b) = self.sel_rect();
                let range = if self.anchor.is_some() {
                    format!("{}:{}", a.a1(), b.a1())
                } else {
                    a.a1()
                };
                // 適用範囲を訊く2段目へ(本家の「新しい名前」も範囲を選ばせる)
                self.name_new = Some((text.clone(), range.clone()));
                self.pick_kind = "name-scope";
                self.pick_note = Some(ui::tf!("scope_name", text, range).into());
                let at = self.pop_anchor();
                self.pick = Some((
                    menu(&[
                        ui::item!("whole_workbook_use_any"),
                        ui::item!("sheet_only"),
                    ]),
                    at,
                ));
            }
            // **ピボットの元の表の差し替え**(2026-08-21 の D群)
            "pivot-src" => {
                let Some(i) = self.pivot_at(self.cursor) else {
                    self.status = ui::t!("put_cursor_pivot").into();
                    return;
                };
                // `Sheet1!A1:C20` か `A1:C20`(いまのシート)
                let (name, range_of) = match text.rsplit_once('!') {
                    Some((s0, r)) => (s0.trim().to_string(), r.trim().to_string()),
                    None => (self.sheet().name.clone(), text.trim().to_string()),
                };
                let Some((a0, b0)) = range_of.split_once(':') else {
                    self.status = ui::t!("write_range_like_a1_c20").into();
                    return;
                };
                let (Some(a0), Some(b0)) = (
                    Pos::parse(&a0.replace('$', "").to_uppercase()),
                    Pos::parse(&b0.replace('$', "").to_uppercase()),
                ) else {
                    self.status = ui::tf!("cant_read_range", range_of).into();
                    return;
                };
                let Some(si) = self.book.sheets.iter().position(|s| s.name == name) else {
                    self.status = ui::tf!("there_no_sheet", name).into();
                    return;
                };
                if b0.row <= a0.row {
                    self.status = ui::t!("data_rows_needed_below").into();
                    return;
                }
                // **いま使っている見出しが新しい範囲にあるか。** 無ければ
                // 作り直しても空になります — 黙って空にせず、先に言います
                let heading: Vec<String> = (a0.col..=b0.col)
                    .map(|c| {
                        self.book.sheets[si]
                            .get(Pos::new(a0.row, c))
                            .map(|x| x.value.display())
                            .unwrap_or_default()
                    })
                    .collect();
                let d = self.book.pivots[i].clone();
                let needed: Vec<&String> = d
                    .rows_sel
                    .iter()
                    .chain(d.cols_sel.iter())
                    .chain(std::iter::once(&d.value))
                    .filter(|h| !h.is_empty())
                    .collect();
                let missing: Vec<String> =
                    needed.iter().filter(|h| !heading.contains(h)).map(|h| h.to_string()).collect();
                if !missing.is_empty() {
                    self.status = ui::tf!(
                        "new_range_no_such",
                        missing.join("・")
                    )
                    .into();
                    return;
                }
                self.checkpoint();
                self.book.pivots[i].sheet = name.clone();
                self.book.pivots[i].src = (a0, b0);
                let d = self.book.pivots[i].clone();
                self.dirty = true;
                self.status = ui::tf!(
                    "source_table_repointed_rebuilding",
                    name,
                    a0.a1(),
                    b0.a1()
                )
                .into();
                self.spawn_pivot(d, Some(i), cx);
            }
            "comment" => {
                let p = self.cursor;
                if text.is_empty() {
                    if self.book.sheets[self.active].comments.remove(&p).is_some() {
                        self.dirty = true;
                        self.status = ui::tf!("comment_removed", p.a1()).into();
                    }
                } else {
                    // 頭の文だけ差し替える(返信と解決の印は残す)
                    let sh = &mut self.book.sheets[self.active];
                    match sh.comments.get_mut(&p) {
                        Some(th) if !th.entries.is_empty() => th.entries[0].text = text,
                        _ => {
                            sh.comments.insert(p, text.into());
                        }
                    }
                    self.dirty = true;
                    self.status = ui::tf!("comment_added_kept_save", p.a1()).into();
                }
            }
            "cond-gt" | "cond-lt" => {
                let Ok(value) = text.parse::<f64>() else {
                    self.status = ui::tf!("not_number", text).into();
                    return;
                };
                self.checkpoint();
                let range = self.sel_rect();
                let gt = kind == "cond-gt";
                self.book.sheets[self.active].cond.push(kumihan::book::CondRule {
                    range,
                    kind: kumihan::book::CondKind::Cmp(
                        if gt { kumihan::book::CondOp::Gt } else { kumihan::book::CondOp::Lt },
                        value,
                    ),
                    look: kumihan::book::CondLook {
                        fill: Some(if gt { "E2EFDA".into() } else { "FCE4D6".into() }),
                        ..Default::default()
                    },
                });
                self.dirty = true;
                // 比べ方は**先に1つの句に組んで**から差し込む。trf の {} は
                // 左から順に埋まるだけなので、「100 より大きい値」を数と語に
                // 割ると語順を変えられない言語(独・西・伊・葡・尼)が壊れる
                let what = if gt {
                    ui::tf!("values_greater_than", value)
                } else {
                    ui::tf!("values_less_than", value)
                };
                self.status = ui::tf!("coloring", range.0.a1(), range.1.a1(), what).into();
            }
            // 条件付き書式のパネル(間・文字・上位/下位N)
            "cond-between" => {
                let t = text.replace('~', "〜");
                let Some((a1, b1)) = t.split_once('〜') else {
                    self.status = ui::t!("use_form_8_15").into();
                    self.prompt = Some(("cond-between", Editor::new(&text)));
                    return;
                };
                let (Ok(lo), Ok(hi)) = (a1.trim().parse::<f64>(), b1.trim().parse::<f64>())
                else {
                    self.status = ui::t!("use_form_8_15").into();
                    self.prompt = Some(("cond-between", Editor::new(&text)));
                    return;
                };
                self.checkpoint();
                let range = self.sel_rect();
                self.book.sheets[self.active].cond.push(kumihan::book::CondRule {
                    range,
                    kind: kumihan::book::CondKind::Between(lo.min(hi), lo.max(hi), false),
                    look: kumihan::book::CondLook {
                        fill: Some("FFF2CC".into()),
                        ..Default::default()
                    },
                });
                self.dirty = true;
                self.status = ui::tf!("filling_values_between", range.0.a1(), range.1.a1(), lo.min(hi), lo.max(hi)).into();
            }
            "cond-text" => {
                if text.is_empty() {
                    self.status = ui::t!("enter_text_look").into();
                    return;
                }
                self.checkpoint();
                let range = self.sel_rect();
                self.book.sheets[self.active].cond.push(kumihan::book::CondRule {
                    range,
                    kind: kumihan::book::CondKind::Text(text.clone()),
                    look: kumihan::book::CondLook {
                        fill: Some("FFF2CC".into()),
                        ..Default::default()
                    },
                });
                self.dirty = true;
                self.status = ui::tf!("filling_cells_containing", range.0.a1(), range.1.a1(), text).into();
            }
            "cond-top" | "cond-bottom" => {
                let Ok(n) = text.trim().parse::<u32>() else {
                    self.status = ui::t!("enter_count_ascii_digits").into();
                    self.prompt = Some((kind, Editor::new(&text)));
                    return;
                };
                let bottom = kind == "cond-bottom";
                self.checkpoint();
                let range = self.sel_rect();
                self.book.sheets[self.active].cond.push(kumihan::book::CondRule {
                    range,
                    kind: kumihan::book::CondKind::Top(n.max(1), bottom),
                    look: kumihan::book::CondLook {
                        fill: Some(if bottom { "FCE4D6".into() } else { "D9E1F2".into() }),
                        ..Default::default()
                    },
                });
                self.dirty = true;
                // ここも句ごと1つの引数にする。区切りは**日本語の型の中**に
                // 入れた(「上位 10 件」)— 訳の末尾に空白を仕込ませない
                let what = if bottom {
                    ui::tf!("bottom_3", n.max(1))
                } else {
                    ui::tf!("top_3", n.max(1))
                };
                self.status = ui::tf!("coloring", range.0.a1(), range.1.a1(), what).into();
            }
            "py" => {
                let t = text.trim().to_string();
                if t.is_empty() {
                    // **空 Enter = 置き場の一覧。** 前はここからファイル選択が
                    // 出て、どこの .py でも走らせられた(2026-08-16 に閉じた —
                    // 発注者「calc, writer から起動できるのは、置き場を固定する
                    // のがいい」)。外から来た物は置き場に置いてから選ぶ
                    self.run_cmd("py-list", cx);
                } else if t == "@計算" || t == "@calc" {
                    self.run_py_calc(cx);
                } else if t == "@" || t == "@list" {
                    // コードは置き場の .py にしかない(ブックは運ばない)。
                    // **置き場は2つ** — funcs=式から呼ぶ関数、plugins=人が押す
                    // マクロ(2026-08-16 に割った)。片方だけ出すと、もう片方が
                    // 「消えた」ように見える
                    let old: Vec<&str> =
                        self.book.scripts.iter().map(|(n, _)| n.as_str()).collect();
                    let line = |o: Vec<(String, Vec<String>)>| {
                        o.iter()
                            .map(|(m, defs)| format!("{m}: {}", defs.join(" ")))
                            .collect::<Vec<_>>()
                            .join(" / ")
                    };
                    let funcs = line(pyrun::outline_in(&pyrun::funcs_dir()));
                    let plugs = line(plugin_outline());
                    let mut msg = if funcs.is_empty() && plugs.is_empty() {
                        ui::tf!(
                            "no_py_files_yet",
                            pyrun::funcs_dir().display().to_string()
                        )
                        .to_string()
                    } else {
                        let mut s = String::new();
                        if !funcs.is_empty() {
                            s.push_str(&ui::tf!("functions_funcs", funcs));
                        }
                        if !plugs.is_empty() {
                            if !s.is_empty() {
                                s.push_str("  ");
                            }
                            s.push_str(&ui::tf!("macros_plugins", plugs));
                        }
                        s
                    };
                    if !old.is_empty() {
                        msg.push_str(&ui::tf!(
                            "old_code_carried_book",
                            old.join(" ")
                        ));
                    }
                    self.status = msg.into();
                } else if t.starts_with("@save") {
                    // 2026-08-09 発注者確定: データとプログラムを1つのファイルに
                    // しない。関数(UDF)もブックには載せない — 置き場は plugins だけ
                    self.status = ui::tf!(
                        "books_not_carry_code",
                        plugins_dir().display().to_string()
                    )
                    .into();
                } else if let Some(name) = t.strip_prefix("@del ") {
                    // 古いブックに載っていたコードを、保存を待たずに外す
                    let name = name.trim();
                    let before = self.book.scripts.len();
                    self.book.scripts.retain(|(n, _)| n != name);
                    if self.book.scripts.len() < before {
                        self.dirty = true;
                        self.status = ui::tf!("removed_workbook", name).into();
                    } else {
                        self.status = ui::tf!("not_exist", name).into();
                    }
                } else if let Some(name) = t.strip_prefix("@edit") {
                    // plugins の .py を calc の中で開く(zed 側の半分)
                    self.open_py_edit(name.trim());
                } else if let Some(name) = t.strip_prefix("@export ") {
                    // 古いブックに載っていたコードの取り出し口(実行はしない。
                    // 中身を見て、良ければ自分で plugins へ置く — それが取り込みの門)
                    self.export_python_dialog(name.trim().to_string(), cx);
                } else if let Some(rest) = t.strip_prefix('@') {
                    // 実行するのは**手元(plugins)の .py だけ**(発注者確定
                    // 2026-08-08 → 2026-08-09 に関数まで拡張)。ブックに載って
                    // 旅してきたコードは実行しない — ファイルは実行の起点になれない。
                    // サンドボックスは従来どおり必須、網は既定で閉じ
                    // 「@名前 net」とその場で打ったときだけ開く
                    // サンドボックスを着せなくなったので網の区別は無くなった。
                    // 黙って受けると「網ありで動いた」と誤解されるので、断って言う
                    if let Some(n) = rest.trim().strip_suffix(" net") {
                        self.status = ui::tf!(
                            "net_no_longer_needed",
                            n.trim()
                        )
                        .into();
                        return;
                    }
                    let name = rest.trim();
                    // 「モジュール.関数」なら、その関数だけを呼ぶ
                    let (module, func) = match name.rsplit_once('.') {
                        Some((m, f)) if plugins_dir().join(format!("{m}.py")).exists() => {
                            (m, Some(f))
                        }
                        _ => (name, None),
                    };
                    if plugins_dir().join(format!("{module}.py")).exists() {
                        self.run_plugin(module, func, cx);
                    } else if self.book.scripts.iter().any(|(n, _)| n == name) {
                        // 古いブックに載っているコードは、関数も手続きも実行しない
                        self.status = ui::tf!(
                            "code_carried_book_not",
                            name,
                            name,
                            plugins_dir().display().to_string()
                        )
                        .into();
                    } else {
                        self.status = ui::tf!(
                            "there_no_py_files",
                            name,
                            plugins_dir().display().to_string()
                        )
                        .into();
                    }
                } else {
                    self.run_python(t, cx);
                }
            }
            "shape-text" => {
                let Some(i) = self.shape_sel else { return };
                if self.sheet().shapes_new.len() <= i {
                    return;
                }
                self.checkpoint();
                self.sheet_mut().shapes_new[i].text =
                    (!text.is_empty()).then(|| text.clone());
                self.dirty = true;
                self.status = if text.is_empty() {
                    ui::t!("text_removed").into()
                } else {
                    ui::t!("text_set_shape_goes").into()
                };
            }
            // 図形の塗り・線の色の直指定(RRGGBB)。空 Enter = なし
            "shape-fill-rgb" | "shape-line-rgb" => {
                let is_fill = kind == "shape-fill-rgb";
                let t = text.trim().trim_start_matches('#').to_uppercase();
                if t.is_empty() {
                    self.shape_edit(|sp| {
                        if is_fill {
                            sp.fill = None;
                        } else {
                            sp.line = None;
                        }
                    });
                    self.status = if is_fill {
                        ui::t!("fill_removed").into()
                    } else {
                        ui::t!("removed_line").into()
                    };
                } else if t.len() == 6 && u32::from_str_radix(&t, 16).is_ok() {
                    let c = t.clone();
                    self.shape_edit(move |sp| {
                        if is_fill {
                            sp.fill = Some(c);
                        } else {
                            sp.line = Some(c);
                        }
                    });
                    self.status = if is_fill {
                        ui::tf!("fill_set", format!("#{t}")).into()
                    } else {
                        ui::tf!("made_line", format!("#{t}")).into()
                    };
                } else {
                    self.status = ui::t!("cant_read_colour_6").into();
                    self.prompt = Some((kind, Editor::new(&t)));
                }
            }
            // 図形の回転の直指定(度・時計回り)。空 Enter = 0 に戻す
            "shape-rot" => {
                let t = text.trim().replace('°', "");
                if t.is_empty() {
                    self.shape_edit(|sp| sp.rot = 0.0);
                    self.status = ui::t!("rotation_reset").into();
                } else {
                    match t.parse::<f32>() {
                        Ok(d) if d.is_finite() => {
                            let d = d.rem_euclid(360.0);
                            self.shape_edit(move |sp| sp.rot = d);
                            self.status =
                                ui::tf!("rotated_clockwise", format!("{d:.0}")).into();
                        }
                        _ => {
                            self.status =
                                ui::t!("cant_read_angle_one").into();
                            self.prompt = Some(("shape-rot", Editor::new(&t)));
                        }
                    }
                }
            }
            // **一覧の仕事**(作る・名前を変える・消す。2026-08-26)
            "fl-name" => {
                self.fl_commit(text);
                recalc_book(&mut self.book, self.active);
            }
            "split-delim" => {
                let delim = if text.is_empty() { ",".to_string() } else { text };
                let (a, b) = self.sel_rect();
                let col = a.col;
                let targets: Vec<(Pos, String)> = (a.row..=b.row)
                    .filter_map(|r| {
                        let p = Pos::new(r, col);
                        match self.sheet().get(p).map(|c| &c.value) {
                            Some(kumihan::book::Value::Text(t)) if t.contains(&delim) => {
                                Some((p, t.clone()))
                            }
                            _ => None,
                        }
                    })
                    .collect();
                if targets.is_empty() {
                    self.status = ui::tf!("no_cell_selection_splits", delim).into();
                    return;
                }
                self.checkpoint();
                let mut n = 0usize;
                for (p, t) in targets {
                    for (k, part) in t.split(&delim).enumerate() {
                        let q = Pos::new(p.row, p.col + k as u32);
                        let fmt = self.sheet().get(q).map(|c| c.fmt.clone()).unwrap_or_default();
                        // **字を割った結果は字のまま**(2026-08-25 発注者)。
                        // `Cell::input` を通すと「090-1234-5678」を「-」で
                        // 割った先が 90・1234・5678 になり、電話番号と
                        // 郵便番号の頭の 0 が落ちます。割る前が字なら、
                        // 割った後も字です — Excel のように列ごとの型を
                        // 選ばせるのではなく、規則1つで塞ぎます
                        let mut cell = Cell {
                            formula: None,
                            value: kumihan::book::Value::Text(part.to_string()),
                            fmt: Default::default(),
                        };
                        cell.fmt = fmt;
                        self.sheet_mut().set(q, cell);
                        n += 1;
                    }
                }
                recalc_book(&mut self.book, self.active);
                self.dirty = true;
                self.sync_input();
                self.status =
                    ui::tf!("split_into_columns_cells", n).into();
            }
            "goal-target" => {
                // 「D6=765600」の形
                let Some((cell_s, val_s)) = text.split_once('=') else {
                    self.status = ui::t!("use_form_cell_target").into();
                    return;
                };
                let (Some(p), Ok(v)) = (Pos::parse(cell_s), val_s.trim().parse::<f64>()) else {
                    self.status = ui::t!("cant_parse_e_g").into();
                    return;
                };
                self.goal = Some((p, v));
                self.prompt = Some(("goal-var", Editor::new("")));
            }
            // データテーブル 1/2 — 列の入力セル(空 Enter = やめる)
            "dt-col" => {
                let t = text.trim().to_string();
                if t.is_empty() {
                    self.status = ui::t!("data_table_cancelled").into();
                    return;
                }
                let Some(p) = Pos::parse(&t) else {
                    self.status = ui::t!("cant_read_input_cell").into();
                    self.prompt = Some(("dt-col", Editor::new(&t)));
                    return;
                };
                self.dt_col = Some(p);
                self.prompt = Some(("dt-row", Editor::new("")));
            }
            // データテーブル 2/2 — 行の入力セル(空 Enter = 1変数)
            "dt-row" => {
                let Some(ci) = self.dt_col.take() else { return };
                let t = text.trim().to_string();
                if t.is_empty() {
                    self.data_table(Some(ci), None);
                    return;
                }
                match Pos::parse(&t) {
                    Some(ri) => self.data_table(Some(ci), Some(ri)),
                    None => {
                        self.status = ui::t!("cant_read_row_input").into();
                        self.dt_col = Some(ci);
                        self.prompt = Some(("dt-row", Editor::new(&t)));
                    }
                }
            }
            "goal-var" => {
                let Some((target, goal)) = self.goal.take() else { return };
                let Some(var) = Pos::parse(&text) else {
                    self.status = ui::t!("cant_parse_variable_cell").into();
                    return;
                };
                self.goal_seek(target, goal, var);
            }
            // パスワードのパネル。開き待ちがあれば解いて開き、
            // 無ければ「次の保存から暗号化」を決める(空なら解除)
            "pw-open" => {
                let Some(p) = self.pw_pending.take() else { return };
                let bytes = match std::fs::read(&p) {
                    Ok(b) => b,
                    Err(e) => {
                        self.status = ui::tf!("cant_open", e).into();
                        return;
                    }
                };
                match ooxml::crypt::decrypt(&bytes, &text) {
                    Ok(plain) => {
                        self.open_plain(p.clone(), plain);
                        if self.path.as_deref() == Some(p.as_path()) {
                            self.encrypt_pw = Some(text);
                            self.status = ui::tf!("saving_keeps_same_password", self.status)
                            .into();
                        }
                    }
                    Err(e) => {
                        // パネルは開いたまま。打ち直せる
                        self.pw_pending = Some(p);
                        self.pw_show = false;
                        self.prompt = Some(("pw-open", Editor::new("")));
                        self.status = e.into();
                    }
                }
            }
            // **2回聞きます**(2026-08-21 の D群)。打ち間違えたパスワードで
            // 包むと、そのファイルは誰にも開けません。元に戻す手がありません
            "pw-set" => {
                if text.is_empty() {
                    self.pw_first = None;
                    self.encrypt_pw = None;
                    self.status = ui::t!("encryption_off_next_save").into();
                } else {
                    self.pw_first = Some(text);
                    self.pw_show = false;
                    self.prompt = Some(("pw-set2", Editor::new("")));
                    self.status = ui::t!("lets_check_type_same").into();
                }
            }
            "pw-set2" => {
                let first_time = self.pw_first.take();
                if first_time.as_deref() == Some(text.as_str()) {
                    self.encrypt_pw = first_time;
                    self.dirty = true;
                    self.status =
                        ui::t!("next_save_encrypted_password").into();
                } else {
                    // **前の設定は触りません。** 半端に掛かった状態を作らない
                    self.status =
                        ui::t!("two_passwords_differ_encryption").into();
                }
            }
            // 予測シート — 何期先まで
            "forecast-h" => {
                let h = text.trim().parse::<usize>().unwrap_or(6).clamp(1, 60);
                self.forecast_run(h, cx);
            }
            // シナリオの名前。**いま選んでいるセルの値をそのまま控えます**
            "scenario-name" => {
                let name = text.trim().to_string();
                if name.is_empty() {
                    self.status = ui::t!("name_empty_nothing_made").into();
                    return;
                }
                let (a, b) = self.sel_rect();
                let mut cells: Vec<(Pos, String)> = Vec::new();
                for r in a.row..=b.row {
                    for c in a.col..=b.col {
                        let p = Pos::new(r, c);
                        // **式のセルは入れません。** 式を字で書き戻すと、
                        // 当てた瞬間に式が消えます(比べる先が壊れる)
                        let cell = self.sheet().get(p);
                        if cell.is_some_and(|x| x.formula.is_some()) {
                            continue;
                        }
                        let v = cell.map(|x| x.value.display()).unwrap_or_default();
                        if !v.is_empty() {
                            cells.push((p, v));
                        }
                    }
                }
                if cells.is_empty() {
                    self.status = ui::t!(
                        "there_no_values_keep"
                    )
                    .into();
                    return;
                }
                let n_cells = cells.len();
                let before = self.sheet().scenarios.len();
                self.sheet_mut().scenarios.retain(|s| s.name != name);
                let overwrite = self.sheet().scenarios.len() < before;
                self.sheet_mut().scenarios.push(kumihan::book::Scenario {
                    name: name.clone(),
                    cells,
                    comment: String::new(),
                });
                self.dirty = true;
                self.status = if overwrite {
                    ui::tf!("scenario_kept_again_cells", name, n_cells).into()
                } else {
                    ui::tf!("scenario_kept_cells_saving", name, n_cells)
                        .into()
                };
            }
            "equation" => {
                if text.is_empty() {
                    self.status = ui::t!("formula_empty_nothing_placed").into();
                } else {
                    self.insert_py_image(EQ_PY, "eq", text, cx);
                }
            }
            "textart" => {
                if text.is_empty() {
                    self.status = ui::t!("text_empty_nothing_placed").into();
                } else {
                    self.insert_py_image(TEXTART_PY, "textart", text, cx);
                }
            }
            // 著者を1人足す(dc:creator は `;` 区切りで何人でも入る)
            "prop-author-add" => {
                let name = text.trim().to_string();
                if name.is_empty() {
                    return;
                }
                if self.book.props.creators.contains(&name) {
                    self.status = ui::tf!("already_one_authors", name).into();
                    return;
                }
                self.book.props.creators.push(name);
                self.dirty = true;
                self.status =
                    ui::t!("author_added_goes_into").into();
            }
            // カスタムプロパティ 1/3 — 名前。2段目(型)へ送る
            "prop-add-name" => {
                let name = text.trim().to_string();
                if name.is_empty() {
                    self.status = ui::t!("name_empty_nothing_added").into();
                    return;
                }
                self.prop_add = Some((name, PropKind::Text));
                self.prompt = Some(("prop-add-type", Editor::new("")));
            }
            // カスタムプロパティ 2/3 — 型。空 Enter は文字
            "prop-add-type" => {
                let Some((name, _)) = self.prop_add.take() else { return };
                self.prop_add = Some((name, PropKind::parse(&text)));
                self.prompt = Some(("prop-add-value", Editor::new("")));
            }
            // カスタムプロパティ 3/3 — 値。**ここで初めて足す**
            "prop-add-value" => {
                let Some((name, kind)) = self.prop_add.take() else { return };
                use kumihan::book::{CustomProp, CustomVal};
                let value = match kind {
                    PropKind::Text => CustomVal::Text(text),
                    PropKind::Number => match text.trim().parse::<f64>() {
                        Ok(n) => CustomVal::Number(n),
                        Err(_) => {
                            self.status =
                                ui::tf!("not_read_number_nothing", text)
                                    .into();
                            return;
                        }
                    },
                    PropKind::Date => CustomVal::Date(text.trim().to_string()),
                    PropKind::Bool => CustomVal::Bool(matches!(
                        text.trim(),
                        "yes" | "true" | "TRUE" | "1" | "○"
                    )),
                };
                // 名前はブックの中で一意。同じ名前があれば差し替える
                let props = &mut self.book.props.custom;
                match props.iter().position(|p| p.name == name) {
                    Some(i) => props[i].value = value,
                    None => props.push(CustomProp { name: name.clone(), value, link: None }),
                }
                self.dirty = true;
                self.status =
                    ui::tf!("property_recorded_goes_into", name).into();
            }
            // ブックの情報(保存で docProps/core.xml へ)
            "prop-title" | "prop-keywords" | "prop-subject" | "prop-desc" => {
                let f = match kind {
                    "prop-title" => &mut self.book.props.title,
                    "prop-keywords" => &mut self.book.props.keywords,
                    "prop-subject" => &mut self.book.props.subject,
                    _ => &mut self.book.props.description,
                };
                *f = text;
                self.dirty = true;
                self.status =
                    ui::t!("workbook_info_recorded_goes").into();
            }
            "table-resize" => {
                let p = self.cursor;
                let Some(i) = self.sheet().tables.iter().position(|t| t.contains(p)) else {
                    return;
                };
                let parse = |t: &str| -> Option<(Pos, Pos)> {
                    let (x, y) = t.split_once(':')?;
                    Some((Pos::parse(x.trim())?, Pos::parse(y.trim())?))
                };
                match parse(&text) {
                    None => {
                        self.status = ui::t!("write_range_like_a1").into();
                        self.prompt = Some(("table-resize", Editor::new(&text)));
                    }
                    Some((a, b)) if b.row < a.row || b.col < a.col => {
                        self.status = ui::t!("top_left_bottom_right").into();
                        self.prompt = Some(("table-resize", Editor::new(&text)));
                    }
                    Some((a, b)) => {
                        self.checkpoint();
                        {
                            let t = &mut self.book.sheets[self.active].tables[i];
                            t.a = a;
                            t.b = b;
                        }
                        self.dirty = true;
                        self.status = ui::tf!("table_resized_formatting_not", a.a1(), b.a1())
                        .into();
                    }
                }
            }
            "chat" => {
                if text.is_empty() {
                    self.status = ui::t!("no_message_left").into();
                } else if let Some(cp) = self.chat_path() {
                    let stamp = ui::now_stamp();
                    let line = format!("[{stamp}] {}: {text}\n", lock_identity());
                    use std::io::Write as _;
                    let r = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&cp)
                        .and_then(|mut f| f.write_all(line.as_bytes()));
                    self.status = match r {
                        Ok(_) => ui::tf!("message_left", cp.file_name().unwrap_or_default().to_string_lossy())
                        .into(),
                        Err(e) => ui::tf!("cant_write", e).into(),
                    };
                }
            }
            // 小計の聞き取り(区切りの見出し → 合計する見出し)
            "subtotal-by" => {
                let Some(mut pend) = self.sub_pend.take() else { return };
                let t = text.trim().to_string();
                if !pend.headers.contains(&t) {
                    self.status =
                        ui::tf!("not_among_headings_2", t, pend.headers.join(" / "))
                            .into();
                    self.sub_pend = Some(pend);
                    self.prompt = Some(("subtotal-by", Editor::new(&text)));
                    return;
                }
                pend.rows_sel = vec![t];
                self.status =
                    ui::t!("columns_total_comma_separated").into();
                self.sub_pend = Some(pend);
                self.prompt = Some(("subtotal-vals", Editor::new("")));
            }
            "subtotal-vals" => {
                let Some(pend) = self.sub_pend.take() else { return };
                let by_off =
                    pend.headers.iter().position(|h| *h == pend.rows_sel[0]).unwrap_or(0);
                let by = pend.a.col + by_off as u32;
                let sel = split_fields(&text);
                let mut vals: Vec<u32> = Vec::new();
                if sel.is_empty() {
                    // 数の列を自動で拾う(基準の列は除く)
                    let sh = self.sheet();
                    for i in 0..pend.headers.len() {
                        let c = pend.a.col + i as u32;
                        if c == by {
                            continue;
                        }
                        let numeric = (pend.a.row + 1..=pend.b.row).any(|r| {
                            matches!(
                                sh.get(Pos::new(r, c)).map(|x| &x.value),
                                Some(Value::Number(_))
                            )
                        });
                        if numeric {
                            vals.push(c);
                        }
                    }
                    if vals.is_empty() {
                        self.status =
                            ui::t!("no_numeric_column_found").into();
                        self.sub_pend = Some(pend);
                        self.prompt = Some(("subtotal-vals", Editor::new("")));
                        return;
                    }
                } else {
                    for name in &sel {
                        match pend.headers.iter().position(|h| h == name) {
                            Some(i) => vals.push(pend.a.col + i as u32),
                            None => {
                                self.status =
                                    ui::tf!("not_among_headings", name).into();
                                self.sub_pend = Some(pend);
                                self.prompt = Some(("subtotal-vals", Editor::new(&text)));
                                return;
                            }
                        }
                    }
                }
                self.checkpoint();
                let n = apply_subtotals(
                    &mut self.book.sheets[self.active],
                    pend.a,
                    pend.b,
                    by,
                    &vals,
                );
                recalc_book(&mut self.book, self.active);
                self.dirty = true;
                self.sync_input();
                self.status = ui::tf!("subtotals_grand_total_added", n)
                .into();
            }
            "find" => {
                if text.is_empty() {
                    self.status = ui::t!("type_search_term").into();
                    return;
                }
                self.find_term = Some(text);
                self.prompt = Some(("replace-with", Editor::new("")));
            }
            "replace-with" => {
                let Some(find) = self.find_term.take() else { return };
                if text.is_empty() {
                    // 検索だけ
                    self.find_next(&find);
                    return;
                }
                // 全て置き換え(シート全体。式の中も)
                let targets: Vec<(Pos, String)> = self
                    .sheet()
                    .cells
                    .iter()
                    .filter(|(_, c)| c.editable().contains(&find))
                    .map(|(p, c)| (*p, c.editable()))
                    .collect();
                if targets.is_empty() {
                    self.status = ui::tf!("not_found", find).into();
                    self.find_term = Some(find);
                    return;
                }
                self.checkpoint();
                let mut n = 0usize;
                for (p, src) in targets {
                    n += src.matches(find.as_str()).count();
                    let dst = src.replace(find.as_str(), &text);
                    let fmt = self.sheet().get(p).map(|c| c.fmt.clone()).unwrap_or_default();
                    let mut cell = Cell::input(&dst);
                    cell.fmt = fmt;
                    self.sheet_mut().set(p, cell);
                }
                recalc_book(&mut self.book, self.active);
                self.dirty = true;
                self.sync_input();
                self.find_term = Some(find.clone());
                self.status =
                    ui::tf!("replacements_ctrl_z_undoes", find, text, n)
                        .into();
            }
            "link" => {
                let p = self.cursor;
                if text.is_empty() {
                    if self.book.sheets[self.active].links.remove(&p).is_some() {
                        self.dirty = true;
                        self.status = ui::tf!("unlinked", p.a1()).into();
                    }
                } else {
                    self.book.sheets[self.active].links.insert(p, text);
                    self.dirty = true;
                    self.status =
                        ui::tf!("added_link_ctrl_click", p.a1()).into();
                    // 続けて表示テキスト(セルに見せる文字)。本家のリンク設定の欄と同じ
                    let cur = self.sheet().get(p).map(|c| c.value.display()).unwrap_or_default();
                    self.prompt = Some(("link-text", Editor::new(&cur)));
                }
            }
            // リンクの表示テキスト。空 Enter = セルはそのまま
            "link-text" => {
                let p = self.cursor;
                if !text.is_empty() {
                    let cur = self.sheet().get(p).map(|c| c.value.display()).unwrap_or_default();
                    if text != cur {
                        self.checkpoint();
                        let mut cell = self.sheet().get(p).cloned().unwrap_or_default();
                        let v = kumihan::book::Cell::input(&text);
                        cell.formula = v.formula;
                        cell.value = v.value;
                        self.book.sheets[self.active].set(p, cell);
                        recalc_book(&mut self.book, self.active);
                        self.dirty = true;
                        self.sync_input();
                    }
                }
            }
            _ => {}
        }
    }

    /// 選んだ範囲の**外周だけ**に罫線(帳票の枠)。
    pub(crate) fn border_outline(&mut self) {
        self.commit();
        self.checkpoint();
        let (a, b) = self.sel_rect();
        for r in a.row..=b.row {
            for c in a.col..=b.col {
                let p = Pos::new(r, c);
                let mut cell = self.sheet().get(p).cloned().unwrap_or_default();
                if r == a.row { cell.fmt.borders.top = kumihan::book::Edge::THIN }
                if r == b.row { cell.fmt.borders.bottom = kumihan::book::Edge::THIN }
                if c == a.col { cell.fmt.borders.left = kumihan::book::Edge::THIN }
                if c == b.col { cell.fmt.borders.right = kumihan::book::Edge::THIN }
                self.book.sheets[self.active].set(p, cell);
            }
        }
        self.dirty = true;
        self.status = ui::t!("outer_border_drawn").into();
    }

    /// 書式の小窓のボタン。
    pub(crate) fn fmt_panel_action(&mut self, id: &str, cx: &mut Context<Self>) {
        match id {
            "close" => self.fmt_panel = None,
            "b-all" => {
                self.fmt(|f| f.borders = Borders::ALL);
                self.status = ui::t!("grid_borders_drawn").into();
            }
            "b-out" => self.border_outline(),
            "b-none" => {
                self.fmt(|f| f.borders = Borders::NONE);
                self.status = ui::t!("borders_removed").into();
            }
            "numfmt-none" => {
                self.fmt(|f| f.number_format = None);
                self.status = ui::t!("number_format_cleared").into();
            }
            id if id.starts_with("fill-") => {
                let v = id.trim_start_matches("fill-").to_string();
                if v == "none" {
                    self.fmt(|f| f.fill = None);
                } else {
                    self.fmt(move |f| f.fill = Some(v.clone()));
                }
            }
            id if id.starts_with("color-") => {
                let v = id.trim_start_matches("color-").to_string();
                if v == "none" {
                    self.fmt(|f| f.color = None);
                } else {
                    self.fmt(move |f| f.color = Some(v.clone()));
                }
            }
            other => self.run_cmd(other, cx),
        }
    }

    /// 「ドロップダウンリストから選択」。同じ列に既にある値の一覧を出す
    /// (Excel の Alt+↓ と同じ発想。入力規則が無くても、列の値は候補になる)。
    pub(crate) fn open_pick_list(&mut self) {
        // 入力規則があればその候補(規則に書かれた順のまま)。無ければ同じ列の値
        let from_rule = self
            .sheet()
            .validation_at(self.cursor)
            .map(|v| v.options(self.sheet()))
            .filter(|o| !o.is_empty());
        let mut vals: Vec<String> = from_rule.clone().unwrap_or_default();
        if vals.is_empty() {
            let col = self.cursor.col;
            let (rows, _) = self.sheet().extent();
            for r in 0..rows {
                if r == self.cursor.row {
                    continue;
                }
                if let Some(c) = self.sheet().get(Pos::new(r, col)) {
                    // 式の結果ではなく「打つもの」を候補にする(文字の値だけ)
                    if c.formula.is_none() {
                        let v = c.value.display();
                        if !v.is_empty() && !vals.contains(&v) {
                            vals.push(v);
                        }
                    }
                }
            }
            if vals.is_empty() {
                self.status = ui::t!("column_no_values_yet").into();
                return;
            }
            vals.sort();
        }
        let at = self.pop_anchor();
        let cur = self.input.text().to_string();
        // 打つほど絞られるコンボ。**数百件でも切り捨てない** — 絞り込みで足りる。
        // 一覧に無い値の扱いは入力規則側の決めに従う(コンボでは丸めない・撥ねない)。
        // 同じ列に打たれている値そのもの — 訳す物ではない
        self.open_combo("value", plain(vals), at, &cur);
    }

    /// シートを切り替える。いまの編集を確定し、場所はシートごとに覚えている。
    /// 絞り込みは解く(別のシートの列で絞ったままは意味を持たない)。
    pub(crate) fn switch_sheet(&mut self, i: usize) {
        if i >= self.book.sheets.len() || i == self.active {
            return;
        }
        if !self.commit() {
            return; // 入力規則で戻された。切り替えると打った文字が消える
        }
        self.remember_ui();
        self.active = i;
        self.restore_ui();
        self.anchor = None;
        self.auto_filter = None;
        self.filter_panel = None;
        self.sync_input();
        self.status = ui::tf!("sheet", self.sheet().name).into();
    }

    /// シートを1枚足して、そこへ移る。
    pub(crate) fn add_sheet(&mut self) {
        let name = unique_sheet_name(&self.book);
        self.book.sheets.push(kumihan::book::Sheet::new(&name));
        self.dirty = true;
        self.switch_sheet(self.book.sheets.len() - 1);
    }

    /// タブの右クリックメニュー(本家「シートの管理」の並び)。
    /// 出す場所はタブに近い左下 — パネルを遠くに出さない(終了確認と同じ判断)
    pub(crate) fn open_sheet_menu(&mut self, i: usize) {
        self.sheet_menu_at = Some(i);
        self.pick_kind = "sheet-menu";
        let y = (self.view_h_px - 420.0).max(ROW_H + 16.0);
        self.pick = Some((
            {
                // 保護は**そのシートの今の状態で言い分を変える**。
                // 「保護する/解除する」が分かれていないと、押すまで
                // どちらになるか分からない
                let prot = if self.book.sheets.get(i).map(|s| s.protected).unwrap_or(false) {
                    ui::item!("unprotect_sheet")
                } else {
                    ui::item!("protect_sheet")
                };
                menu(&[
                    ui::item!("insert"),
                    ui::item!("delete"),
                    ui::item!("rename"),
                    ui::item!("duplicate"),
                    ui::item!("move_left"),
                    ui::item!("move_right"),
                    ui::item!("hide"),
                    ui::item!("unhide"),
                    ui::item!("tab_colour"),
                    prot,
                ])
            },
            (HEAD_W + 24.0, y),
        ));
    }

    /// シートの構成が変わった(挿入・削除・移動・複製)。**表の控えの束は
    /// シートの番号で結ばれている**ので、番号が振り直されると意味を失う —
    /// 黙って別のシートへ書き戻すより「元に戻せない」と言う(Excel と同じ)
    pub(crate) fn sheets_restructured(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.clip_range = None;
        self.dirty = true;
    }

    /// シートを削除する(タブのメニューと橋(rpc)の共有 — 同じ作法で断る)。
    /// 返りは消したシートの名前。undo は消える(sheets_restructured)。
    pub(crate) fn delete_sheet_at(&mut self, t: usize) -> Result<String, String> {
        if t >= self.book.sheets.len() {
            return Err(ui::t!("there_no_such_sheet").to_string());
        }
        if self.book.sheets.len() <= 1 {
            return Err(ui::t!("cant_delete_last_sheet").to_string());
        }
        if self.book.sheets.iter().enumerate().filter(|(i, s)| *i != t && !s.hidden).count()
            == 0
        {
            return Err(ui::t!(
                "cant_delete_no_visible"
            )
            .to_string());
        }
        if !self.commit() {
            return Err(ui::t!("what_typing_rejected_validation").to_string());
        }
        self.remember_ui();
        let name = self.book.sheets[t].name.clone();
        self.book.sheets.remove(t);
        self.sheet_ui.remove(t);
        self.watch.retain(|w| w.0 != t);
        for w in self.watch.iter_mut() {
            if w.0 > t {
                w.0 -= 1;
            }
        }
        if self.active >= t && self.active > 0 {
            self.active -= 1;
        }
        if self.book.sheets[self.active].hidden {
            if let Some(i) = self.book.sheets.iter().position(|s| !s.hidden) {
                self.active = i;
            }
        }
        self.sheets_restructured();
        self.restore_ui();
        self.sync_input();
        recalc_book(&mut self.book, self.active);
        Ok(name)
    }

    /// シートを複製する(タブのメニューと橋(rpc)の共有)。写しは元の右隣に
    /// 入り、そこへ移る。名前は省略なら「名前 (2)」の流儀、指定ならシート名の
    /// 決まり(31字・: \\ / ? * [ ] 不可・重複不可)で検査。返りは写しの名前。
    pub(crate) fn copy_sheet_at(&mut self, t: usize, name: Option<&str>) -> Result<String, String> {
        if t >= self.book.sheets.len() {
            return Err(ui::t!("there_no_such_sheet").to_string());
        }
        let new_name = match name {
            None => copy_sheet_name(&self.book, &self.book.sheets[t].name),
            Some(n) => {
                if n.is_empty()
                    || n.chars().count() > 31
                    || n.contains([':', '\\', '/', '?', '*', '[', ']'])
                {
                    return Err(ui::tf!(
                        "cannot_sheet_name_up",
                        n
                    )
                    .to_string());
                }
                if self.book.sheets.iter().any(|s| s.name == n) {
                    return Err(ui::tf!("already_exists", n).to_string());
                }
                n.to_string()
            }
        };
        if !self.commit() {
            return Err(ui::t!("what_typing_rejected_validation").to_string());
        }
        self.remember_ui();
        let mut copy = self.book.sheets[t].clone();
        copy.name = new_name.clone();
        copy.hidden = false;
        self.book.sheets.insert(t + 1, copy);
        self.sheet_ui.insert(t + 1, self.sheet_ui[t]);
        for w in self.watch.iter_mut() {
            if w.0 > t {
                w.0 += 1;
            }
        }
        self.sheets_restructured();
        self.active = t + 1;
        self.restore_ui();
        self.sync_input();
        recalc_book(&mut self.book, self.active);
        Ok(new_name)
    }

    /// シートを隠す・戻す(タブのメニューと橋(rpc)の共有)。
    /// 最後の見えている1枚は隠せない。隠したのがいまのシートなら見える所へ移る。
    pub(crate) fn set_sheet_hidden(&mut self, t: usize, hidden: bool) -> Result<(), String> {
        if t >= self.book.sheets.len() {
            return Err(ui::t!("there_no_such_sheet").to_string());
        }
        if hidden
            && self.book.sheets.iter().enumerate().filter(|(i, s)| *i != t && !s.hidden).count()
                == 0
        {
            return Err(ui::t!("last_sheet_cant_hidden").to_string());
        }
        self.remember_ui();
        self.book.sheets[t].hidden = hidden;
        if hidden && self.active == t {
            if let Some(i) = self.book.sheets.iter().position(|s| !s.hidden) {
                self.active = i;
                self.restore_ui();
                self.sync_input();
            }
        }
        self.dirty = true;
        Ok(())
    }

    /// タブのメニューの実行。t = メニューが指しているシート
    pub(crate) fn sheet_menu_action(&mut self, v: &str) {
        let Some(t) = self.sheet_menu_at else { return };
        if t >= self.book.sheets.len() {
            self.sheet_menu_at = None;
            return;
        }
        self.remember_ui(); // sheet_ui をシート数まで育てておく(挿し外しの前提)
        match v {
            "insert" => {
                let name = unique_sheet_name(&self.book);
                self.book.sheets.insert(t + 1, kumihan::book::Sheet::new(&name));
                self.sheet_ui.insert(t + 1, (Pos::new(0, 0), Pos::new(0, 0), None));
                for w in self.watch.iter_mut() {
                    if w.0 > t {
                        w.0 += 1;
                    }
                }
                self.sheets_restructured();
                self.active = t + 1;
                self.restore_ui();
                self.sync_input();
                self.status = ui::tf!("inserted_sheet", name).into();
            }
            "delete" => {
                self.status = match self.delete_sheet_at(t) {
                    Ok(name) => {
                        ui::tf!("deleted_sheet_cant_undone", name)
                            .into()
                    }
                    Err(e) => e.into(),
                };
            }
            "rename" => {
                let cur = self.book.sheets[t].name.clone();
                self.prompt = Some(("sheet-rename", Editor::new(&cur)));
                return; // sheet_menu_at はパネルの確定まで持ち越す
            }
            "duplicate" => {
                self.status = match self.copy_sheet_at(t, None) {
                    Ok(name) => ui::tf!("created_2", name).into(),
                    Err(e) => e.into(),
                };
            }
            "move_left" | "move_right" => {
                let to = if v == "move_left" {
                    t.checked_sub(1)
                } else {
                    (t + 1 < self.book.sheets.len()).then_some(t + 1)
                };
                let Some(to) = to else {
                    self.status = ui::t!("cant_move_way_already").into();
                    self.sheet_menu_at = None;
                    return;
                };
                self.book.sheets.swap(t, to);
                self.sheet_ui.swap(t, to);
                for w in self.watch.iter_mut() {
                    w.0 = if w.0 == t { to } else if w.0 == to { t } else { w.0 };
                }
                if self.active == t {
                    self.active = to;
                } else if self.active == to {
                    self.active = t;
                }
                self.sheets_restructured();
                self.status = ui::tf!("moved_sheet", self.book.sheets[to].name)
                    .into();
            }
            "hide" => {
                let name = self.book.sheets[t].name.clone();
                self.status = match self.set_sheet_hidden(t, true) {
                    Ok(()) => ui::tf!(
                        "hid_sheet_unhide_brings",
                        name
                    )
                    .into(),
                    Err(e) => e.into(),
                };
            }
            "unhide" => {
                let hidden: Vec<(usize, String)> = self
                    .book
                    .sheets
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.hidden)
                    .map(|(i, s)| (i, s.name.clone()))
                    .collect();
                if hidden.is_empty() {
                    self.status = ui::t!("no_hidden_sheets").into();
                } else {
                    self.pick_kind = "unhide";
                    self.pick_paths = hidden
                        .iter()
                        .map(|(i, n)| (n.clone(), PathBuf::from(i.to_string())))
                        .collect();
                    let y = (self.view_h_px - 420.0).max(ROW_H + 16.0);
                    self.pick = Some((
                        // シート名は帳票の持ち物 — 訳さない
                        plain(hidden.into_iter().map(|(_, n)| n)),
                        (HEAD_W + 24.0, y),
                    ));
                    self.status = ui::t!("hidden_sheets_pick_one").into();
                    self.sheet_menu_at = None;
                    return; // 2段目の一覧へ(pick_kind を戻さない)
                }
            }
            // シートのタブから保護を掛け外し。**そのシートを開いてから**掛ける —
            // いま見ているのと違うシートに黙って掛けない
            "protect_sheet" | "unprotect_sheet" => {
                if t < self.book.sheets.len() {
                    self.commit();
                    self.checkpoint();
                    let on = !self.book.sheets[t].protected;
                    self.book.sheets[t].protected = on;
                    let name = self.book.sheets[t].name.clone();
                    self.dirty = true;
                    self.status = if on {
                        ui::tf!(
                            "protected_sheet_only_unlocked",
                            name
                        )
                        .into()
                    } else {
                        ui::tf!("released_protection_sheet", name).into()
                    };
                }
            }
            "tab_colour" => {
                self.pick_kind = "tab-color";
                let y = (self.view_h_px - 420.0).max(ROW_H + 16.0);
                self.pick = Some((
                    menu(&[
                        ui::item!("no_colour"),
                        ui::item!("red"),
                        ui::item!("orange"),
                        ui::item!("yellow"),
                        ui::item!("green"),
                        ui::item!("blue"),
                        ui::item!("purple"),
                        ui::item!("grey"),
                    ]),
                    (HEAD_W + 24.0, y),
                ));
                return; // sheet_menu_at は色の決定まで持ち越す
            }
            _ => {}
        }
        self.sheet_menu_at = None;
    }

    /// シート見出しの色の決定(タブの色の2段目)
    pub(crate) fn set_tab_color(&mut self, v: &str) {
        let Some(t) = self.sheet_menu_at.take() else { return };
        if t >= self.book.sheets.len() {
            return;
        }
        // **鍵をそのまま文に差し込まない。** 見出しを一緒に持って回る
        let (hex, label) = match v {
            "red" => (Some("FFC00000"), ui::t!("red")),
            "orange" => (Some("FFED7D31"), ui::t!("orange")),
            "yellow" => (Some("FFFFC000"), ui::t!("yellow")),
            "green" => (Some("FF70AD47"), ui::t!("green")),
            "blue" => (Some("FF4472C4"), ui::t!("blue")),
            "purple" => (Some("FF7030A0"), ui::t!("purple")),
            "grey" => (Some("FF7F7F7F"), ui::t!("grey")),
            _ => (None, ""),
        };
        // 1手で戻せる(シート見出しの色もシートの中身 — checkpoint と同じ作法で番号つき)
        self.undo_stack.push(vec![(t, self.book.sheets[t].clone())]);
        self.redo_stack.clear();
        self.book.sheets[t].tab_color = hex.map(|h| h.to_string());
        self.dirty = true;
        self.status = if hex.is_some() {
            ui::tf!("tab_colour_set_kept", label).into()
        } else {
            ui::t!("tab_colour_removed").into()
        };
    }
}
