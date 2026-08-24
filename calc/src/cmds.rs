//! main.rs からの純移動(2026-08-06 の分割)。挙動は変えない。

use crate::*;

/// Python の真偽(True / False)
fn py_bool(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

/// セル1つを Python の値として書く(記録の `value = [[…]]` の材料)。
/// **式は "=…" の字で書く** — 値を焼き付けず、入れ直せば式のまま入る
fn py_cell(c: Option<&sheet::Cell>) -> String {
    let Some(c) = c else { return "None".into() };
    if let Some(f) = &c.formula {
        return format!("{:?}", format!("={f}"));
    }
    match &c.value {
        sheet::Value::Empty => "None".into(),
        sheet::Value::Number(n) => {
            if (n - n.round()).abs() < f64::EPSILON && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        sheet::Value::Bool(b) => py_bool(*b).into(),
        // 誤りの値は**字として**入れる(=DIV/0! を書き戻すと式になってしまう)
        sheet::Value::Error(e) => format!("{e:?}"),
        sheet::Value::Text(s) => format!("{s:?}"),
    }
}


/// 共通の命令(`ui::appcmd`)が触る面。**欄はここから増やさない** —
/// 命令を1つ移すたびに、あちらの trait と一緒に1つずつ増やす
/// ファイルのページの共通の腕が触る面(統合の段8 の3)。
impl ui::filemenu::FileScreen for Calc {
    fn tab_to_prev(&mut self) {
        self.tab = self.prev_tab;
    }
    fn set_file_view(&mut self, v: u8) {
        self.file_view = v;
    }
    fn protect_page(&mut self) -> bool {
        self.file_view = 5;
        true
    }
    fn opened(&self) -> Option<std::path::PathBuf> {
        self.path.clone()
    }
    fn new_file(&mut self) -> bool {
        self.new_book()
    }
    fn open_dialog_now(&mut self, cx: &mut Context<Self>) {
        self.open_dialog(cx);
    }
    fn save_now(&mut self, cx: &mut Context<Self>) {
        self.save(false, cx);
    }
    fn save_as_now(&mut self, cx: &mut Context<Self>) {
        self.save_as(cx);
    }
    fn quit_now(&mut self, cx: &mut Context<Self>) {
        self.request_quit(cx);
    }
    fn goto_tab_named(&mut self, name: &str) {
        if let Some(i) = ribbon::CALC.iter().position(|t| t.name == name) {
            self.prev_tab = i;
            self.tab = i;
        }
    }
}

impl ui::appcmd::Screen for Calc {
    fn zoom_mut(&mut self) -> &mut f32 {
        &mut self.zoom
    }
    fn dark_mut(&mut self) -> &mut bool {
        &mut self.dark
    }
    fn ui_scale_mut(&mut self) -> &mut f32 {
        &mut self.ui_scale
    }
    fn say(&mut self, msg: String) {
        self.status = msg.into();
    }
}

impl Calc {
    /// run_cmd が処理できる id。**リボンの ready はこの表の中に限る**
    /// (試験で突き合わせる。合っていないボタンは「押せるのに何もしない」嘘になる)
    #[allow(dead_code)] // wiring_tests(cfg(test))が使う
    pub(crate) const HANDLED: &'static [&'static str] = &[
        "open", "save", "undo", "redo", "selectall", "pdf",
        "copy", "cut", "paste",
        "bold", "italic", "underline", "borders", "fillparag", "fontcolor", "copystyle",
        "align-left", "align-center", "align-right",
        "comma", "currency", "percents", "digit-inc", "digit-dec", "clear",
        "strikeout", "top", "middle", "bottom", "wrap", "incfont", "decfont",
        "cell-ins", "cell-del", "insrow", "inscol",
        "merge", "custom-sort", "sort-asc", "sort-desc",
        "rem-duplicates", "setfilter", "clear-filter",
        "fill-num", "freeze", "show-formulas", "show-gridlines",
        "fn-math", "fn-text", "fn-logical", "fn-recent",
        "sum", "average", "count", "max", "min",
        "data-validation", "dv-mark", "condformat", "defname", "split", "scenario",
        "final-mark", "forecast", "pivot-chart",
        "pageorient", "pagesize", "pagemargins", "printarea",
        "inschart", "insimage", "inshyperlink", "replace",
        "changecase", "format", "cell-format", "fontname", "fontsize",
        "fn-datetime", "fn-lookup", "fn-financial", "fn-more",
        "scale", "pagebreak", "fit-pages", "printarea-add", "show-breaks", "printtitles", "print-gridlines", "print-headings",
        "data-from-text", "text-column", "goal-seek", "data-external-links",
        "insshape", "instext", "inssparkline", "python", "co-addcomment",
        "trace-prec", "trace-dep", "remove-arrows", "insrecommend",
        "instable", "table-tpl", "inssymbol", "pivot-insert", "pivot-fields",
        "pivot-refresh", "pivot-refresh-all", "pivot-source", "pivot-select", "pivot-chart",
        "pivot-totals", "pivot-subtotals", "pivot-blank", "pivot-layout", "pivot-style",
        "pivot-showas", "datatable", "track-changes",
        "td-header", "td-total", "td-band-row", "td-band-col",
        "td-first", "td-last", "td-filter",
        "group", "ungroup", "hide-details", "show-details", "subtotal", "solver",
        "inssmartart", "insequation", "insslicer", "inscheckbox", "instextart",
        "coauth-mode", "co-delcomment", "co-showcomment", "co-chat",
        "co-history",
        // Python タブ(2026-08-09)
        "rec-toggle", "py-new", "py-list", "py-folder", "func-list", "ribbon-list",
        "prot-doc", "prot-encrypt", "prot-sign",
        "zoom-in", "zoom-out", "zoom100", "ui-bigger", "ui-smaller", "formula-bar", "show-headings", "show-zeros",
        // 左右のパネル(2026-08-15)
        "show-left", "show-right",
        "subscript", "align-just", "align-dist", "text-orient", "calc-mode",
        "td-torange", "td-resize", "rtl-sheet", "direction",
        "colorschemas", "darkmode",
        "ai-where",
        "insert-function", "cell-styles", "sheet-view", "watch", "edit-header",
        "cell-lock", "prot-allow", "recover", "recover-every", "csv-kind",
        "autofit-col", "autofit-row", "paste-name", "flash-fill",
        "read-only-rec",
        "pen", "highlighter", "eraser", "draw-select",
    ];

    /// 範囲の呼び名(1セルなら G4、広ければ G4:H8)。埋めの報せは
    /// **数ではなく場所を言う** — 「4 セル」だけでは、5つ選んだ人が
    /// 1つ漏れたと読む(発注者 2026-08-14。元のセルは数えないため)
    fn range_label(a: Pos, b: Pos) -> String {
        if a == b { a.a1() } else { format!("{}:{}", a.a1(), b.a1()) }
    }

    /// フィルハンドルの実体 — 元の選択 (a,b) を to まで写す(下か右)。
    /// 元の塊を繰り返し写し、式は写した距離ぶんずれる(本家と同じ)。
    /// `invert` = Ctrl を押しながら引いた(裏返し): 連続データ ↔ 写し。
    /// **既定は連続データ**(1 を引けば 2,3,4 — 発注者確定 2026-08-14)
    pub(crate) fn fill_handle_apply(&mut self, a: Pos, b: Pos, to: Pos, invert: bool) {
        self.commit();
        self.checkpoint();
        let sh = &mut self.book.sheets[self.active];
        let mut n = 0usize;
        // **連続データ(オートフィル)**: 元が全部数で2つ以上あり、間隔が
        // 一定なら、写すのではなく**続きを作る**(1,2 → 3,4,5…。本家と同じ)。
        // 1つだけ・式・文字は写す(本家の既定と同じ)
        let series_of = |vals: &[Option<sheet::Cell>]| -> Option<(f64, f64)> {
            let nums: Vec<f64> = vals
                .iter()
                .map(|c| match c {
                    Some(c) if c.formula.is_none() => match c.value {
                        Value::Number(x) => Some(x),
                        _ => None,
                    },
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            match nums.len() {
                0 => None,
                // 数のドラッグは**連続データが既定**(発注者確定 2026-08-14 —
                // 1 を引けば 2,3,4)。写したいときは Ctrl(裏返し)
                1 => (!invert).then_some((nums[0], 1.0)),
                _ => {
                    let step = nums[1] - nums[0];
                    let ok = nums.windows(2).all(|w| (w[1] - w[0] - step).abs() < 1e-9);
                    (ok && !invert).then(|| (*nums.last().unwrap(), step))
                }
            }
        };
        if to.row > b.row {
            let h = b.row - a.row + 1;
            for c in a.col..=b.col {
                let srcs: Vec<Option<sheet::Cell>> =
                    (a.row..=b.row).map(|r| sh.get(Pos::new(r, c)).cloned()).collect();
                let series = series_of(&srcs);
                for r in b.row + 1..=to.row {
                    let src_r = a.row + (r - a.row) % h;
                    let src = srcs[(src_r - a.row) as usize].clone();
                    let p = Pos::new(r, c);
                    let mut cell = match &src {
                        Some(s) => s.clone(),
                        None => {
                            let fmt = sh.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
                            Cell { formula: None, value: Value::Empty, fmt }
                        }
                    };
                    if let Some((last, step)) = series {
                        cell.value = Value::Number(last + step * (r - b.row) as f64);
                        cell.formula = None;
                    } else if let Some(f) = src.as_ref().and_then(|s| s.formula.as_ref()) {
                        cell.formula =
                            Some(sheet::model::offset_refs(f, (r - src_r) as i64, 0));
                    }
                    sh.set(p, cell);
                    n += 1;
                }
            }
        } else if to.col > b.col {
            let w = b.col - a.col + 1;
            for r in a.row..=b.row {
                let srcs: Vec<Option<sheet::Cell>> =
                    (a.col..=b.col).map(|c| sh.get(Pos::new(r, c)).cloned()).collect();
                let series = series_of(&srcs);
                for c in b.col + 1..=to.col {
                    let src_c = a.col + (c - a.col) % w;
                    let src = srcs[(src_c - a.col) as usize].clone();
                    let p = Pos::new(r, c);
                    let mut cell = match &src {
                        Some(s) => s.clone(),
                        None => {
                            let fmt = sh.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
                            Cell { formula: None, value: Value::Empty, fmt }
                        }
                    };
                    if let Some((last, step)) = series {
                        cell.value = Value::Number(last + step * (c - b.col) as f64);
                        cell.formula = None;
                    } else if let Some(f) = src.as_ref().and_then(|s| s.formula.as_ref()) {
                        cell.formula =
                            Some(sheet::model::offset_refs(f, 0, (c - src_c) as i64));
                    }
                    sh.set(p, cell);
                    n += 1;
                }
            }
        }
        recalc_book(&mut self.book, self.active);
        self.dirty = true;
        // 埋めた所までが選択になる(本家と同じ — 続けてもう一度引ける)
        self.anchor = Some(a);
        self.cursor = to;
        self.sync_input();
        let _ = n;
        let dst = if to.row > b.row {
            (Pos::new(b.row + 1, a.col), Pos::new(to.row, b.col))
        } else {
            (Pos::new(a.row, b.col + 1), Pos::new(b.row, to.col))
        };
        self.status = ui::tf!(
            "{} を {} に写しました",
            Self::range_label(a, b),
            Self::range_label(dst.0, dst.1)
        )
        .into();
    }

    /// フィルハンドルのダブルクリック — 隣の列の長さに合わせて下へ写す。
    /// 左隣(空なら右隣)の途切れる手前まで(本家と同じ)
    pub(crate) fn fill_handle_auto(&mut self) {
        let (a, b) = self.sel_rect();
        let sh = &self.book.sheets[self.active];
        let has = |r: u32, c: u32| {
            sh.get(Pos::new(r, c))
                .map(|x| x.formula.is_some() || !matches!(x.value, Value::Empty))
                .unwrap_or(false)
        };
        let probe = |col: u32| -> Option<u32> {
            if !has(a.row, col) {
                return None;
            }
            let mut r = a.row;
            while has(r + 1, col) {
                r += 1;
            }
            (r > b.row).then_some(r)
        };
        let target = if a.col > 0 { probe(a.col - 1) } else { None };
        let target = target.or_else(|| probe(b.col + 1));
        match target {
            Some(last) => self.fill_handle_apply(a, b, Pos::new(last, b.col), false),
            None => {
                self.status =
                    ui::t!("隣の列に長さの手掛かりがありません(左右どちらも空)").into();
            }
        }
    }

    /// 見た目だけを変える操作(保護の「セルの書式設定」を許すと通る)
    pub(crate) const FORMAT_CMDS: &'static [&'static str] = &[
        "bold", "italic", "underline", "strikeout", "subscript",
        "fontname", "fontsize", "incfont", "decfont", "fontcolor", "fillparag",
        "borders", "align-left", "align-center", "align-right", "align-just", "align-dist",
        "top", "middle", "bottom", "wrap", "text-orient",
        "comma", "currency", "percents", "digit-inc", "digit-dec",
        "numfmt", "format", "cell-format", "cell-styles", "copystyle",
    ];

    /// **一覧・パレットが開くボタン。** リボンは ▾ を添え、押すと
    /// [`Calc::pop_anchor`] の場所(押したボタンの真下)に開く。試験もこの
    /// 一覧を使って「どのボタンから開いても押した所に出るか」を確かめる
    /// (位置の直書きが6箇所残っていた。2026-08-08 実機の一巡点検で発見)。
    ///
    /// 腕の目印: `self.pick =` / `menu_at` / `border_pal` を立てる物だけ。
    /// 何も開かない即時動作に ▾ を出さない(嘘の ▾ が16個あった。
    /// 2026-08-14 の数え直しで除去)。小窓が開く物は [`Self::DIALOG_IDS`]。
    pub(crate) const MENU_IDS: &'static [&'static str] = &[
        "fontname", "fontsize", "changecase", "format", "fontcolor",
        "fillparag", "freeze", "condformat", "colorschemas", "insshape",
        "inssmartart", "table-tpl", "inssymbol", "pagebreak", "fit-pages",
        "recover", "recover-every", "text-orient", "sheet-view", "cell-styles",
        // コメントは「表示のしかた」も「どこまで消すか」も一覧で選ぶ
        "co-showcomment", "co-delcomment",
        "pivot-style", "pivot-fields",
        // 格子パレット(border_pal)。真下に落ちるので ▾ の側
        "borders",
        "merge", "prot-allow", "co-history", "py-list",
        "insslicer", "edit-header", "paste-name", "csv-kind", "defname",
        "currency", "scenario",
    ];

    /// **小窓が開くボタン。** リボンは … を添える(メニュー項目末尾の
    /// 「…」=「続きの画面がある」の古い約束と同じ)。押すと入力や複数項目の
    /// 画面が開き、続きの操作が要る。
    ///
    /// 腕の目印: `self.prompt =` / `fn_dlg` / `dv_dlg` / `fmt_panel` /
    /// `solver` を立てる物だけ。この旗の総和が [`Self::dialog_open`] —
    /// 印と「小窓中はリボン無効」が同じ一覧を見る(ずれると嘘になる)。
    pub(crate) const DIALOG_IDS: &'static [&'static str] = &[
        "insert-function", "cell-format", "data-validation", "custom-sort",
        "python", "prot-encrypt", "co-chat", "instext", "insequation",
        "td-resize", "subtotal", "datatable", "forecast",
        "co-addcomment", "text-column", "goal-seek", "replace", "inshyperlink",
        "solver",
    ];

    /// シートの保護中でも通す操作(見るだけ・保存・保護の操作そのもの)
    pub(crate) const PROTECTED_OK: &'static [&'static str] = &[
        "open", "save", "pdf", "selectall", "undo", "redo",
        "freeze", "show-formulas", "show-gridlines",
        "setfilter", "clear-filter",
        "trace-prec", "trace-dep", "remove-arrows", "pivot-select",
        "coauth-mode", "co-showcomment", "co-chat", "co-history",
        "prot-doc", "prot-encrypt", "prot-sign", "ai-where",
        "recover", "recover-every", "csv-kind", "autofit-col", "autofit-row",
        "read-only-rec",
        // 「許可する操作」は保護中にこそ触る。**鍵を掛けていないので
        // 隠す意味も無い** — 保護は事故止めであって錠前ではない(SEKKEI)
        "prot-allow",
    ];

    /// ピボットの上では締める操作(本家 Toolbar.js の editPivot ロックと同じ顔ぶれ:
    /// オートフィルタ・結合・ハイパーリンク・テーブル・入力規則・重複削除)。
    /// ピボットは polars が置いた「その時の値」— この上で表を組み替えると壊れる
    pub(crate) const PIVOT_LOCKED: &'static [&'static str] = &[
        "setfilter", "merge", "inshyperlink", "instable", "data-validation", "rem-duplicates",
    ];

    /// 選んだ範囲を表にする。**見た目は書式として掛ける**(表を外しても残る
    /// — SEKKEI「表そのもの」の節)。`style` は色の組、`label` は状態行に出す
    /// スタイルの名前(既定で作ったときは出さない)。
    ///
    /// `instable`(すぐ作る)と `table-tpl`(色を選んでから作る)の両方から
    /// 呼ぶ。**2箇所に同じ組み立てを書かない** — 片方だけ直る事故を避ける
    pub(crate) fn make_table(&mut self, style: crate::util::TableStyle, label: Option<&str>) {
        if self.anchor.is_none() {
            self.status = ui::t!("表にする範囲を選んでください").into();
            return;
        }
        self.checkpoint();
        let (a, b) = self.sel_rect();
        for r in a.row..=b.row {
            for c in a.col..=b.col {
                let p = Pos::new(r, c);
                let mut cell = self.sheet().get(p).cloned().unwrap_or_default();
                if r == a.row {
                    cell.fmt.bold = true;
                    cell.fmt.fill = style.header.map(|h| h.into());
                    cell.fmt.borders.top = sheet::model::Edge::THIN;
                } else if (r - a.row) % 2 == 0 {
                    cell.fmt.fill = style.band.map(|h| h.into());
                }
                if r == b.row {
                    cell.fmt.borders.bottom = sheet::model::Edge::THIN;
                }
                if c == a.col {
                    cell.fmt.borders.left = sheet::model::Edge::THIN;
                }
                if c == b.col {
                    cell.fmt.borders.right = sheet::model::Edge::THIN;
                }
                self.book.sheets[self.active].set(p, cell);
            }
        }
        let n = self.book.sheets.iter().map(|s| s.tables.len()).sum::<usize>() + 1;
        self.book.sheets[self.active].tables.push(sheet::model::TableDef {
            name: format!("テーブル{n}"),
            a,
            b,
            ..Default::default()
        });
        self.dirty = true;
        self.status = match label {
            Some(l) => ui::tf!(
                "{}:{} を「{}」の表にしました(範囲に変換・サイズ変更もできます。Ctrl+Z で戻せます)",
                a.a1(),
                b.a1(),
                l
            ),
            None => ui::tf!(
                "{}:{} を表にしました(見出し行の色と縞模様。範囲に変換・サイズ変更もできます。Ctrl+Z で戻せます)",
                a.a1(),
                b.a1()
            ),
        }
        .into();
    }

    /// 記録に残す書式の操作(id → Python の1行を作る型)。
    /// **Python の口にある物だけ**を記録する — 記録した台本が走らないと
    /// 意味が無いので、口の無い操作は残さない(下の rec_cmd が None を返す)
    /// 表の飾りの**次の姿**(入っていれば切る)。カーソルの居る表の旗を見る
    /// — 表が無ければ「掛ける」から始める(押して何も起きないより素直)
    pub(crate) fn td_next(&self, what: sheet::tabledesign::Deco) -> bool {
        use sheet::tabledesign::Deco;
        let p = self.cursor;
        let Some(t) = self.sheet().tables.iter().find(|t| t.contains(p)) else {
            return true;
        };
        !match what {
            Deco::Header => t.header,
            Deco::BandRow => t.banded_rows,
            Deco::BandCol => t.banded_cols,
            Deco::FirstCol => t.first_col,
            Deco::LastCol => t.last_col,
        }
    }

    fn rec_cmd(&self, id: &str) -> Option<String> {
        let (a, b) = self.sel_rect();
        let r = if a == b { a.a1() } else { format!("{}:{}", a.a1(), b.a1()) };
        let sheet = self.book.sheets[self.active].name.clone();
        let v = crate::state::sheet_var(&sheet);
        Some(match id {
            // **書式は書かない。** 掛けた後の姿を [`Self::rec_fmt_diff`] が
            // 差分から起こす(2026-08-16)。1つずつ手で書いていたころは
            // 6つしか無く、しかも align の3行は `.api.align` という
            // **Python に無い口**を書いていた — 記録は残るのに走らなかった
            // 表のデザイン。**掛けた後の姿を書く**(入切なので)
            "td-header" | "td-band-row" | "td-band-col" | "td-first" | "td-last" => {
                use sheet::tabledesign::Deco;
                let what = match id {
                    "td-header" => Deco::Header,
                    "td-band-row" => Deco::BandRow,
                    "td-band-col" => Deco::BandCol,
                    "td-first" => Deco::FirstCol,
                    _ => Deco::LastCol,
                };
                format!(
                    "s{v}[{r:?}].table_style({:?}, {})",
                    what.name(),
                    py_bool(self.td_next(what))
                )
            }
            "td-total" => format!("s{v}[{r:?}].add_total_row()"),
            "td-torange" => format!("s{v}[{:?}].unlist()", self.cursor.a1()),
            "read-only-rec" => {
                format!("b.read_only_recommended = {}", py_bool(!self.book.read_only_rec))
            }
            // **利用者がリボンに足したマクロ**も記録する(発注者 2026-08-16
            // 「ユーザー定義のものを含めてすべて記録できないといけない」)
            other if other.starts_with(ribbon::USER_PREFIX) => {
                format!("xw.run_macro({:?})", &other[ribbon::USER_PREFIX.len()..])
            }
            _ => return None,
        })
    }

    /// **書式の差分から記録の行を起こす。** 掛ける前の姿と後の姿を比べ、
    /// 変わった項目だけを Python の1行にする。
    ///
    /// 手で「この命令はこう書く」と並べていくと、必ず抜ける・必ず古くなる。
    /// 差分なら**命令を増やしても記録は勝手に付いてくる** — 太字も
    /// 表示形式も配色の変更も、書式を変えた限りは同じ道を通る。
    ///
    /// 見るのはカーソルのセル1つ(範囲は同じ書式になる前提。範囲の中で
    /// 元がばらばらなら、揃えた結果が記録される — それが掛けた操作の意味)。
    ///
    /// 返りは(行, **全部を言い表せたか**)。表せない項目 — 罫線・字下げ・
    /// 文字の向き・下付き・ロック — が変わっていたら `false` を返す。
    /// **半分書けたものを「書けた」と数えない**(黙って半分だけ走るマクロは、
    /// 走らないマクロより悪い)。判定は field を数えず、**表せた分を写した
    /// 姿と本物を突き合わせる** — 項目が増えても勝手に追いつく
    pub(crate) fn rec_fmt_diff(&self, before: &sheet::model::CellFormat) -> (Vec<String>, bool) {
        let after = self.sheet().get(self.cursor).map(|c| c.fmt.clone()).unwrap_or_default();
        if after == *before {
            return (Vec::new(), true);
        }
        let (a, b) = self.sel_rect();
        let r = if a == b { a.a1() } else { format!("{}:{}", a.a1(), b.a1()) };
        let sheet = self.book.sheets[self.active].name.clone();
        let v = crate::state::sheet_var(&sheet);
        let cell = format!("s{v}[{r:?}]");
        let mut out = Vec::new();
        let mut boolean = |name: &str, was: bool, now: bool, font: bool| {
            if was != now {
                let dot = if font { ".font" } else { "" };
                out.push(format!("{cell}{dot}.{name} = {}", py_bool(now)));
            }
        };
        boolean("bold", before.bold, after.bold, true);
        boolean("italic", before.italic, after.italic, true);
        boolean("underline", before.underline, after.underline, true);
        boolean("strike", before.strike, after.strike, true);
        boolean("wrap_text", before.wrap, after.wrap, false);
        boolean("shrink", before.shrink, after.shrink, false);
        boolean("subscript", before.subscript, after.subscript, true);
        boolean("rtl_text", before.rtl_text, after.rtl_text, false);
        boolean("formula_hidden", before.formula_hidden, after.formula_hidden, false);
        // ロックは**裏返して**持っている(既定が「掛かっている」)
        if before.unlocked != after.unlocked {
            out.push(format!("{cell}.locked = {}", py_bool(!after.unlocked)));
        }
        if before.indent != after.indent {
            out.push(format!("{cell}.indent = {}", after.indent));
        }
        if before.rotation != after.rotation {
            match after.rotation {
                Some(d) => out.push(format!("{cell}.rotation = {d}")),
                None => out.push(format!("{cell}.rotation = None")),
            }
        }
        if before.font != after.font {
            match &after.font {
                Some(f) => out.push(format!("{cell}.font.name = {f:?}")),
                None => out.push(format!("{cell}.font.name = None")),
            }
        }
        if before.size_c != after.size_c {
            match after.size_c {
                Some(c) => {
                    let pt = c as f32 / 100.0;
                    let s = if (pt - pt.round()).abs() < 0.005 {
                        format!("{}", pt.round() as i64)
                    } else {
                        format!("{pt}")
                    };
                    out.push(format!("{cell}.font.size = {s}"));
                }
                None => out.push(format!("{cell}.font.size = None")),
            }
        }
        if before.color != after.color {
            match &after.color {
                Some(c) => out.push(format!("{cell}.font.color = {c:?}")),
                None => out.push(format!("{cell}.font.color = None")),
            }
        }
        if before.fill != after.fill {
            match &after.fill {
                Some(c) => out.push(format!("{cell}.color = {c:?}")),
                None => out.push(format!("{cell}.color = None")),
            }
        }
        if before.number_format != after.number_format {
            match &after.number_format {
                Some(f) => out.push(format!("{cell}.number_format = {f:?}")),
                None => out.push(format!("{cell}.number_format = \"General\"")),
            }
        }
        if before.align != after.align {
            match after.align.as_xlsx() {
                Some(x) => out.push(format!("{cell}.align = {x:?}")),
                None => out.push(format!("{cell}.align = None")),
            }
        }
        if before.valign != after.valign {
            match after.valign.as_xlsx() {
                Some(x) => out.push(format!("{cell}.valign = {x:?}")),
                None => out.push(format!("{cell}.valign = None")),
            }
        }
        // **言い表せたかを機械で確かめる。** 書ける項目だけを before に写し、
        // それで after と同じになるなら全部言えている。ならなければ、
        // 言えない項目(罫線・字下げ・回転・下付き・ロック…)が動いている
        let mut wrote = before.clone();
        wrote.bold = after.bold;
        wrote.italic = after.italic;
        wrote.underline = after.underline;
        wrote.strike = after.strike;
        wrote.wrap = after.wrap;
        wrote.shrink = after.shrink;
        wrote.font = after.font.clone();
        wrote.size_c = after.size_c;
        wrote.color = after.color.clone();
        wrote.fill = after.fill.clone();
        wrote.number_format = after.number_format.clone();
        wrote.align = after.align;
        wrote.valign = after.valign;
        wrote.subscript = after.subscript;
        wrote.rtl_text = after.rtl_text;
        wrote.formula_hidden = after.formula_hidden;
        wrote.unlocked = after.unlocked;
        wrote.indent = after.indent;
        wrote.rotation = after.rotation;
        // 色の由来は set_fmt が色そのものに落とすので、比べる前に揃える
        wrote.color_theme = after.color_theme;
        wrote.fill_theme = after.fill_theme;
        let 全部言えた = wrote == after;
        (out, 全部言えた)
    }

    /// **値の差分から記録の行を起こす。** 掛ける前のセルと後のセルを比べ、
    /// 変わった所を1つの `value =` にまとめる。
    ///
    /// 書式の差分([`Self::rec_fmt_diff`])で拾えない操作 — 消去・並べ替え・
    /// フィル・置き換え・貼り付け — は、**何をしたか**を Python の言葉で
    /// 言い直せない。言い直せないなら**結果を書く**。並べ替えを
    /// `sort()` と書けないのは残念だが、書けないふりをして黙って落とすより、
    /// 走って同じ結果になる方がいい(発注者 2026-08-16
    /// 「マクロはそのとおり動作する必要がある」)。
    ///
    /// 広がりすぎたら諦めて `None` を返す(註が残る)。1つの操作が
    /// 数千の欄を書き替えたなら、それは記録ではなくブックの複製になる。
    pub(crate) fn rec_value_diff(&self, before: &sheet::Sheet) -> Option<String> {
        let after = self.sheet();
        let mut lo: Option<(u32, u32, u32, u32)> = None;
        let keys: std::collections::BTreeSet<Pos> =
            before.cells.keys().chain(after.cells.keys()).copied().collect();
        for p in keys {
            let x = before.cells.get(&p).map(|c| (c.value.clone(), c.formula.clone()));
            let y = after.cells.get(&p).map(|c| (c.value.clone(), c.formula.clone()));
            if x == y {
                continue;
            }
            lo = Some(match lo {
                None => (p.row, p.col, p.row, p.col),
                Some((r0, c0, r1, c1)) => {
                    (r0.min(p.row), c0.min(p.col), r1.max(p.row), c1.max(p.col))
                }
            });
        }
        let (r0, c0, r1, c1) = lo?;
        // **広すぎる差分は書かない。** 2000 欄を超えたら、記録ではなく複製
        if (r1 - r0 + 1) as u64 * (c1 - c0 + 1) as u64 > 2000 {
            return None;
        }
        let mut rows = Vec::new();
        for r in r0..=r1 {
            let mut row = Vec::new();
            for c in c0..=c1 {
                row.push(py_cell(after.cells.get(&Pos::new(r, c))));
            }
            rows.push(format!("[{}]", row.join(", ")));
        }
        let sheet = self.book.sheets[self.active].name.clone();
        let v = crate::state::sheet_var(&sheet);
        let a1 = if r0 == r1 && c0 == c1 {
            Pos::new(r0, c0).a1()
        } else {
            format!("{}:{}", Pos::new(r0, c0).a1(), Pos::new(r1, c1).a1())
        };
        Some(format!("s{v}[{a1:?}].value = [{}]", rows.join(", ")))
    }

    /// **本当に変わったか。** 記録が見る所だけを比べる — セルの中身と書式、
    /// 表の性質、印刷の設定。
    ///
    /// `edits` の増分では足りない。あれは `checkpoint()`(戻せるように控えを
    /// 取る)で増えるので、**押しても何も変わらなかった命令まで「変わった」に
    /// 数えていた** — 下揃えのセルにもう一度下揃えを掛ける、空の切り取り板を
    /// 貼る、1セルだけ選んで並べ替える。穴の数え上げで 8 件中 5 件がこれだった
    /// (2026-08-16)。**嘘の宿題は、宿題の一覧を読む気を失わせる。**
    pub(crate) fn sheet_changed(&self, before: &sheet::Sheet) -> bool {
        let a = self.sheet();
        if a.cells.len() != before.cells.len() {
            return true;
        }
        for (p, c) in &a.cells {
            match before.cells.get(p) {
                None => return true,
                Some(d) => {
                    if c.value != d.value || c.formula != d.formula || c.fmt != d.fmt {
                        return true;
                    }
                }
            }
        }
        if a.tables.len() != before.tables.len() {
            return true;
        }
        for (t, u) in a.tables.iter().zip(&before.tables) {
            let key = |x: &sheet::model::TableDef| {
                (
                    x.name.clone(),
                    x.a,
                    x.b,
                    x.header,
                    x.totals,
                    x.banded_rows,
                    x.banded_cols,
                    x.first_col,
                    x.last_col,
                )
            };
            if key(t) != key(u) {
                return true;
            }
        }
        // 図形・画像・コメントは**数だけ**見る。中身までは比べない —
        // 記録はまだそれらを書けないので、有無が分かれば註は出せる
        if (a.shapes_new.len(), a.images.len(), a.images_new.len(), a.comments.len())
            != (
                before.shapes_new.len(),
                before.images.len(),
                before.images_new.len(),
                before.comments.len(),
            )
        {
            return true;
        }
        (
            a.paper_size,
            a.landscape,
            a.print_scale,
            a.fit_to_w,
            a.fit_to_h,
            a.print_gridlines,
            a.print_headings,
            a.margins_mm,
            a.rtl,
            a.protected,
        ) != (
            before.paper_size,
            before.landscape,
            before.print_scale,
            before.fit_to_w,
            before.fit_to_h,
            before.print_gridlines,
            before.print_headings,
            before.margins_mm,
            before.rtl,
            before.protected,
        )
    }

    /// **印刷の設定の差分から記録の行を起こす。** 余白・向き・紙・倍率・
    /// 枠線と見出しの印刷 — レイアウトタブのボタンは巡回するので、
    /// 「押した回数」ではなく**行き着いた姿**を書く
    pub(crate) fn rec_page_diff(&self, before: &sheet::Sheet) -> Option<String> {
        let a = self.sheet();
        let mut kw: Vec<String> = Vec::new();
        if before.paper_size != a.paper_size {
            match a.paper_size {
                Some(c) => kw.push(format!("paper={c}")),
                None => kw.push("paper=None".into()),
            }
        }
        if before.landscape != a.landscape {
            kw.push(format!("landscape={}", py_bool(a.landscape)));
        }
        if before.print_scale != a.print_scale {
            match a.print_scale {
                Some(s) => kw.push(format!("scale={s}")),
                None => kw.push("scale=None".into()),
            }
        }
        if before.fit_to_w != a.fit_to_w {
            kw.push(match a.fit_to_w {
                Some(v) => format!("fit_to_w={v}"),
                None => "fit_to_w=None".into(),
            });
        }
        if before.fit_to_h != a.fit_to_h {
            kw.push(match a.fit_to_h {
                Some(v) => format!("fit_to_h={v}"),
                None => "fit_to_h=None".into(),
            });
        }
        if before.print_gridlines != a.print_gridlines {
            kw.push(format!("print_gridlines={}", py_bool(a.print_gridlines)));
        }
        if before.print_headings != a.print_headings {
            kw.push(format!("print_headings={}", py_bool(a.print_headings)));
        }
        if before.margins_mm != a.margins_mm {
            kw.push(match a.margins_mm {
                Some((l, r, t, b)) => format!("margins_mm=({l}, {r}, {t}, {b})"),
                None => "margins_mm=None".into(),
            });
        }
        if kw.is_empty() {
            return None;
        }
        let v = crate::state::sheet_var(&self.book.sheets[self.active].name);
        Some(format!("s{v}.set_page_setup({})", kw.join(", ")))
    }

    /// リボンの表からこの命令の名札を引く(記録の註に人の言葉で残すため)。
    /// 見つからなければ id をそのまま返す
    pub(crate) fn cmd_label(id: &str) -> &str {
        ribbon::calc_tabs()
            .iter()
            .flat_map(|t| t.cmds.iter())
            .find(|c| c.id == id)
            .map(|c| c.label)
            .unwrap_or(id)
    }

    /// **操作の記録の入口。** 中身は [`Self::run_cmd_inner`]。
    ///
    /// ここが薄い包みになっているのは「**書けなかった操作を黙って落とさない**」
    /// ため(発注者 2026-08-15 の設計「記録の正直さ」)。前は
    /// `rec_cmd` が `None` を返したら何も残らず、記録した .py は
    /// **やった操作より短いのに、そうとは分からない**台本になっていた。
    ///
    /// 判定は手で分類しない。**走らせて測る** — 中身が変わった
    /// (`edits` が増えた)のに記録が伸びていなければ、それが穴。
    /// この形なら Python の口が増えたとき註が自然に消え、**一覧を手で
    /// 保つ必要がない**(手で保つ表は必ず遅れて嘘になる)。
    /// **編集中の選択を印で囲む(もう囲んであれば外す)。** 捌いたら真。
    ///
    /// 式(`=` で始まり空白なし)の中では何もしません — 式の字を太字に
    /// する意味が無く、`*` を足すと式そのものが変わってしまいます。
    pub(crate) fn wrap_marks(&mut self, open: &str, close: &str) -> bool {
        if !self.editing() {
            return false;
        }
        let sel = self.input.selection();
        if sel.is_empty() {
            return false;
        }
        let text = self.input.text().to_string();
        if kumihan::adoc::is_formula_cell(&text) {
            return false;
        }
        let (at, rep, select) = sheet::cellmark::toggle_wrap(&text, sel, open, close);
        // 置き換える範囲を選び直してから挿す(Editor の insert は選択を置き換える)
        self.input.move_to(at.start, false);
        self.input.move_to(at.end, true);
        self.input.insert(&rep);
        // 中身を選び直す(続けて別の印も押せるように)
        self.input.move_to(select.start, false);
        self.input.move_to(select.end, true);
        true
    }

    pub(crate) fn run_cmd(&mut self, id: &str, cx: &mut Context<Self>) {
        // 操作の記録(始めていれば)。**押す前に取る** — 掛けた後の姿を
        // 書くので、いまの状態から次の姿を組み立てる
        let rec_len = self.rec.as_ref().map(|v| v.len());
        // **意図が書けるならそちらが勝つ。** 書けたら差分は取らない —
        // `table_style(…)` の下に `.font.bold = True` が並ぶと、読む人は
        // どちらが本体か分からなくなるし、掛け直しにもなる
        let mut 意図あり = false;
        if self.rec.is_some() {
            if let Some(line) = self.rec_cmd(id) {
                self.rec_line(line);
                意図あり = true;
            }
        }
        // 掛ける前の姿を控える(**掛けた後との差分**が記録の行になる)。
        // 記録していない間は控えない — シートの複製は安くない
        let before = (self.rec.is_some() && !意図あり).then(|| {
            (
                self.sheet().get(self.cursor).map(|c| c.fmt.clone()).unwrap_or_default(),
                self.sheet().clone(),
            )
        });
        let edits_before = self.edits;
        // **本当に変わったか**を控えの姿と突き合わせる。`edits` は
        // `checkpoint()`(戻せるように控えを取る)で増えるので、**押しても
        // 何も変わらなかった命令まで「変わった」に数えていた** — 下揃えの
        // セルにもう一度下揃えを掛ける、空の切り取り板を貼る、1セルだけ
        // 選んで並べ替える。穴の数え上げで 8 件中 5 件がこれだった
        // (2026-08-16。残る本物は図形だけ)
        let 姿0 = self.rec.is_some().then(|| self.sheet().clone());
        self.run_cmd_inner(id, cx);
        let 変わった = match &姿0 {
            Some(s) => self.sheet_changed(s),
            None => self.edits > edits_before,
        };
        if let Some((f0, s0)) = before {
            let (lines, 全部言えた) = self.rec_fmt_diff(&f0);
            for line in lines {
                self.rec_line(line);
            }
            // 値が変わっていれば、書式の行があっても**値も**書く。
            // 消去のように両方変える操作があるため
            if let Some(line) = self.rec_value_diff(&s0) {
                self.rec_line(line);
            }
            // 印刷の設定(余白・向き・紙・倍率・枠線と見出し)も同じ控えから
            if let Some(line) = self.rec_page_diff(&s0) {
                self.rec_line(line);
            }
            // 書式に言い表せない動きがあれば、下の穴の註へ落とす
            self.rec_fmt_partial = !全部言えた;
        }
        if let Some(n) = rec_len {
            let 書けた 
                = self.rec.as_ref().is_some_and(|v| v.len() > n) && !self.rec_fmt_partial;
            self.rec_fmt_partial = false;
            if 変わった && !書けた {
                // **穴の印。** この行がそのまま Python の口の宿題になる
                let line = format!(
                    "# この操作はまだ Python で書けません: {}({id})",
                    Self::cmd_label(id)
                );
                self.rec_line(line);
            }
        }
    }

    fn run_cmd_inner(&mut self, id: &str, cx: &mut Context<Self>) {
        // 前に開いていた一覧の注記を落とす。**注記を出す一覧を鍵で閉じると
        // 残り、次に開いた一覧の見出しに前の説明が出ていた**(書体の一覧に
        // 「ピボット 1/4 …」が出た。2026-08-08 実機で見つけた)
        self.pick_note = None;
        // **共通の命令は1本の捌き手へ**(2026-08-19)。同じ id の腕を
        // ここに残すと死ぬので、移したら消す
        if ui::appcmd::run(self, id) {
            return;
        }
        if self.sheet().protected && !Self::PROTECTED_OK.contains(&id) {
            // **一律に断らない。** 保護のときに何を許すかはシートごとに
            // 決められる(Excel の「許可する操作」)。許した分は通す
            let a = self.sheet().protect_allow.clone();
            let allowed = match id {
                // ロックの掛け外しは**保護を解いてから**。保護中に許すと
                // 「保護されたシートで自分のロックを外して書く」ができる
                "cell-lock" => false,
                _ if Self::FORMAT_CMDS.contains(&id) => a.format_cells,
                "colw" | "hide-col" | "show-col" | "autofit-col" => a.format_cols,
                "rowh" | "hide-row" | "show-row" | "autofit-row" => a.format_rows,
                "inscol" => a.insert_cols,
                "insrow" => a.insert_rows,
                "inshyperlink" | "ins-link" => a.insert_links,
                "delcol" => a.delete_cols,
                "delrow" => a.delete_rows,
                "custom-sort" | "sort-asc" | "sort-desc" => a.sort,
                "setfilter" | "clear-filter" => a.autofilter,
                _ if id.starts_with("pivot-") => a.pivot,
                // 中身を書き換えるものは、選んだ範囲のロックを見る
                _ => !self.sel_locked(),
            };
            if !allowed {
                self.status = Self::protected_msg().into();
                cx.notify();
                return;
            }
        }
        if Self::PIVOT_LOCKED.contains(&id) && self.pivot_at(self.cursor).is_some() {
            self.status =
                ui::t!("ピボットの上ではできません(カーソルをピボットの外に置いてから)").into();
            cx.notify();
            return;
        }
        match id {
            "open" => self.open_dialog(cx),
            "save" => self.save(false, cx),
            "undo" => {
                if !self.input.undo() {
                    self.undo_sheet();
                }
            }
            "redo" => {
                if !self.input.redo() {
                    self.redo_sheet();
                }
            }
            "selectall" => self.select_all_now(),
            "copy" => self.copy_now(cx),
            "cut" => self.cut_now(cx),
            "paste" => self.paste_now(cx),
            // 罫線 — **日本の帳票の本体**。一覧から辺とペン(線種・色)を選ぶ
            "borders" => {
                self.commit();
                let at = self.pop_anchor();
                // アイコンの格子パレット(発注者 2026-08-08)。掛けても閉じない
                self.border_pal = Some(at);
            }
            // 書式のコピー(刷毛)。いまのセルの書式を持ち、次に押した先へ塗る
            "copystyle" => {
                self.commit();
                let f = self
                    .sheet()
                    .get(self.cursor)
                    .map(|c| c.fmt.clone())
                    .unwrap_or_default();
                self.brush = Some(f);
                self.status = ui::t!("書式を持ちました — 次に押したセル(選択)に塗ります(Esc でやめる)").into();
            }
            // **編集中に字を選んでいれば、印を入れます**(2026-08-19 発注者
            // 「セルの中の一部を選択してリボンのボタンをつかえばいいのでは」)。
            // 書き方(**太字**)を覚えなくても、選んで押せば付きます。
            // もう一度押せば外れます。選んでいなければ今までどおりセル全体
            "bold" => {
                if !self.wrap_marks("**", "**") {
                    self.fmt(|f| f.bold = !f.bold)
                }
            }
            "italic" => {
                if !self.wrap_marks("__", "__") {
                    self.fmt(|f| f.italic = !f.italic)
                }
            }
            // 下線はセルの中の書き方に無い(AsciiDoc に印が無い)ので、
            // いつでもセル全体
            "underline" => self.fmt(|f| f.underline = !f.underline),
            "strikeout" => {
                if !self.wrap_marks("[.line-through]##", "##") {
                    self.fmt(|f| f.strike = !f.strike)
                }
            }
            // 縦の揃えと折り返し
            "top" => self.fmt(|f| f.valign = sheet::model::VAlign::Top),
            "middle" => self.fmt(|f| f.valign = sheet::model::VAlign::Middle),
            "bottom" => self.fmt(|f| f.valign = sheet::model::VAlign::Bottom),
            "wrap" => self.fmt(|f| f.wrap = !f.wrap),
            // 文字の大きさの +/−。**一覧の17個を1段ずつ辿る**(±1pt ではない —
            // 本家もそう動く)。半端な値は動く向きの隣の一覧値へ寄り、端では止まる
            "incfont" => self.fmt(|f| {
                let pt = f.size_c.map(|c| c as f32 / 100.0).unwrap_or(11.0);
                let pt = ui::combo::step_size(pt, true);
                f.size_c = Some((pt * 100.0) as u32);
            }),
            "decfont" => self.fmt(|f| {
                let pt = f.size_c.map(|c| c as f32 / 100.0).unwrap_or(11.0);
                let pt = ui::combo::step_size(pt, false);
                f.size_c = Some((pt * 100.0) as u32);
            }),
            "align-left" => self.fmt(|f| f.align = HAlign::Left),
            "align-center" => self.fmt(|f| f.align = HAlign::Center),
            "align-right" => self.fmt(|f| f.align = HAlign::Right),
            // 均等割付(字をセルの幅いっぱいに散らす)。両端揃えと違って
            // 折り返しは要らない — 1行の中で字の間を開ける
            "align-dist" => self.fmt(|f| f.align = HAlign::Distribute),
            // 表示形式
            "comma" => self.fmt(|f| f.number_format = Some("#,##0".into())),
            // 行・列の出し入れ
            "cell-ins" => self.rowcol(|s, p| s.insert_row(p.row)),
            "cell-del" => self.rowcol(|s, p| s.remove_row(p.row)),
            "insrow" => self.rowcol(|s, p| s.insert_row(p.row)),
            "inscol" => self.rowcol(|s, p| s.insert_col(p.col)),
            // 小数点以下の桁
            "digit-inc" => self.decimals(1),
            "digit-dec" => self.decimals(-1),
            // 書式のクリア。値は消さない
            "clear" => self.fmt(|f| *f = CellFormat::default()),
            // フィル(下方向へコピー)。式は相対参照がずれ、$ は止まる。
            // 書式も一緒に写す(帳票の列は書式ごと揃える)
            "fill-num" => {
                let (a, b) = self.sel_rect();
                if a.row == b.row {
                    // 高さ1の選択は**上の行を写す**(本家の Ctrl+D と同じ)。
                    // 端まで選択は空の手前で止まる決めなので、空の最終セルを
                    // 埋める一手はこちらが受け持つ(発注者 2026-08-14)
                    if a.row == 0 {
                        self.status = ui::t!("1行目です(上に写す行がありません)").into();
                    } else {
                        self.commit();
                        self.checkpoint();
                        let sh = &mut self.book.sheets[self.active];
                        let mut n = 0usize;
                        for c in a.col..=b.col {
                            let Some(src) = sh.get(Pos::new(a.row - 1, c)).cloned() else {
                                continue;
                            };
                            let mut cell = src.clone();
                            if let Some(f) = &src.formula {
                                cell.formula = Some(sheet::model::offset_refs(f, 1, 0));
                            }
                            sh.set(Pos::new(a.row, c), cell);
                            n += 1;
                        }
                        recalc_book(&mut self.book, self.active);
                        self.dirty = true;
                    // カーソルのセルは打ちかけの欄を映す — 同期しないと
                    // **書けたのに空に見える**(発注者 2026-08-14。選択の
                    // 最後のセル=カーソルの居場所なので、いつも最後だけ
                    // 空に見えた)
                    self.sync_input();
                        let _ = n;
                        self.status = ui::tf!(
                            "{} を {} に写しました",
                            Self::range_label(Pos::new(a.row - 1, a.col), Pos::new(a.row - 1, b.col)),
                            Self::range_label(a, b)
                        )
                        .into();
                    }
                } else {
                    self.commit();
                    self.checkpoint();
                    let sh = &mut self.book.sheets[self.active];
                    let mut n = 0usize;
                    for c in a.col..=b.col {
                        // 一番上が空でも黙って飛ばさない — 空を配る
                        // (中身を消す。書式は残す — 帳票の枠を壊さない)
                        let src = sh.get(Pos::new(a.row, c)).cloned();
                        for r in a.row + 1..=b.row {
                            let p = Pos::new(r, c);
                            let mut cell = match &src {
                                Some(s) => s.clone(),
                                None => {
                                    let fmt = sh.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
                                    Cell { formula: None, value: Value::Empty, fmt }
                                }
                            };
                            if let Some(f) = src.as_ref().and_then(|s| s.formula.as_ref()) {
                                cell.formula =
                                    Some(sheet::model::offset_refs(f, (r - a.row) as i64, 0));
                            }
                            sh.set(p, cell);
                            n += 1;
                        }
                    }
                    recalc_book(&mut self.book, self.active);
                    self.dirty = true;
                    // カーソルのセルは打ちかけの欄を映す — 同期しないと
                    // **書けたのに空に見える**(発注者 2026-08-14。選択の
                    // 最後のセル=カーソルの居場所なので、いつも最後だけ
                    // 空に見えた)
                    self.sync_input();
                    let _ = n;
                    self.status = ui::tf!(
                        "{} を {} に写しました",
                        Self::range_label(Pos::new(a.row, a.col), Pos::new(a.row, b.col)),
                        Self::range_label(Pos::new(a.row + 1, a.col), b)
                    )
                    .into();
                }
            }
            // 右へコピー(Ctrl+R)。リボンには無い — 鍵盤の道だけ。
            // fill-num(下へ)の姉妹で、先頭列を右へ写す。式は相対参照が
            // 列方向にずれ、$ は止まる(offset_refs が同じ規則で見る)
            "fill-right" => {
                let (a, b) = self.sel_rect();
                if a.col == b.col {
                    // 幅1の選択は**左の列を写す**(本家の Ctrl+R と同じ)
                    if a.col == 0 {
                        self.status = ui::t!("A列です(左に写す列がありません)").into();
                    } else {
                        self.commit();
                        self.checkpoint();
                        let sh = &mut self.book.sheets[self.active];
                        let mut n = 0usize;
                        for r in a.row..=b.row {
                            let Some(src) = sh.get(Pos::new(r, a.col - 1)).cloned() else {
                                continue;
                            };
                            let mut cell = src.clone();
                            if let Some(f) = &src.formula {
                                cell.formula = Some(sheet::model::offset_refs(f, 0, 1));
                            }
                            sh.set(Pos::new(r, a.col), cell);
                            n += 1;
                        }
                        recalc_book(&mut self.book, self.active);
                        self.dirty = true;
                    // カーソルのセルは打ちかけの欄を映す — 同期しないと
                    // **書けたのに空に見える**(発注者 2026-08-14。選択の
                    // 最後のセル=カーソルの居場所なので、いつも最後だけ
                    // 空に見えた)
                    self.sync_input();
                        let _ = n;
                        self.status = ui::tf!(
                            "{} を {} に写しました",
                            Self::range_label(Pos::new(a.row, a.col - 1), Pos::new(b.row, a.col - 1)),
                            Self::range_label(a, b)
                        )
                        .into();
                    }
                } else {
                    self.commit();
                    self.checkpoint();
                    let sh = &mut self.book.sheets[self.active];
                    let mut n = 0usize;
                    for r in a.row..=b.row {
                        // 左端が空でも黙って飛ばさない(下へコピーと同じ決め)
                        let src = sh.get(Pos::new(r, a.col)).cloned();
                        for c in a.col + 1..=b.col {
                            let p = Pos::new(r, c);
                            let mut cell = match &src {
                                Some(s) => s.clone(),
                                None => {
                                    let fmt = sh.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
                                    Cell { formula: None, value: Value::Empty, fmt }
                                }
                            };
                            if let Some(f) = src.as_ref().and_then(|s| s.formula.as_ref()) {
                                cell.formula =
                                    Some(sheet::model::offset_refs(f, 0, (c - a.col) as i64));
                            }
                            sh.set(p, cell);
                            n += 1;
                        }
                    }
                    recalc_book(&mut self.book, self.active);
                    self.dirty = true;
                    // カーソルのセルは打ちかけの欄を映す — 同期しないと
                    // **書けたのに空に見える**(発注者 2026-08-14。選択の
                    // 最後のセル=カーソルの居場所なので、いつも最後だけ
                    // 空に見えた)
                    self.sync_input();
                    let _ = n;
                    self.status = ui::tf!(
                        "{} を {} に写しました",
                        Self::range_label(a, Pos::new(b.row, a.col)),
                        Self::range_label(Pos::new(a.row, a.col + 1), b)
                    )
                    .into();
                }
            }
            // 塗りつぶし。黄 → 水色 → 解除(色を選ぶ小窓がまだ無い)
            // 結合は本家の4択(結合して中央/横方向/結合だけ/解除)
            "merge" => {
                self.commit();
                if self.anchor.is_none() {
                    self.status = ui::t!("結合する範囲を Shift+矢印で選んでください").into();
                } else {
                    let at = self.pop_anchor();
                    self.pick_kind = "merge-pick";
                    self.pick = Some((
                        menu(&[
                            ui::item!("結合して中央に配置"),
                            ui::item!("横方向に結合(行ごと)"),
                            ui::item!("セルの結合(揃えは触らない)"),
                            ui::item!("結合の解除"),
                        ]),
                        at,
                    ));
                }
            }
            // 表示。**値は変えない** — 見え方だけの話
            "show-formulas" => self.show_formulas = !self.show_formulas,
            // 帳票を PDF に。画面に見えているもの(値・書式・罫線)を写す
            "pdf" => self.save_pdf(cx),
            "show-gridlines" => self.gridlines = !self.gridlines,
            // ウィンドウ枠の固定。カーソルの上と左を留める。もう一度で解く
            // オートフィルタ。範囲に▼を張る/外す(トグル)。**中身は変えない**
            "setfilter" => {
                if self.auto_filter.is_some() {
                    self.auto_filter = None;
                    self.filter_panel = None;
                    self.status = ui::t!("絞り込みの範囲を外しました").into();
                } else {
                    let (a, b) = if self.anchor.is_some() {
                        self.sel_rect()
                    } else {
                        let (rows, cols) = self.sheet().extent();
                        if rows < 2 || cols == 0 {
                            self.status = ui::t!("絞り込む表がありません(見出しの下にデータが要ります)").into();
                            return;
                        }
                        (Pos::new(0, 0), Pos::new(rows - 1, cols - 1))
                    };
                    if a.row == b.row {
                        self.status = ui::t!("絞り込む表がありません(見出しの下にデータが要ります)").into();
                        return;
                    }
                    self.auto_filter = Some(AutoFilter { range: (a, b), hide: Default::default() });
                    self.status = ui::tf!(
                        "{}:{} に絞り込みの範囲を張りました — 見出しの ▼ から絞ります(表示だけ。保存はされず、閉じれば消えます)",
                        a.a1(), b.a1()
                    )
                    .into();
                }
            }
            "clear-filter" => {
                // **掛かっていないのに「解きました」と言わない**
                // (2026-08-21 の B-5)。灰色にするからには、押したときの
                // 返事も本当のことを言います
                if self.auto_filter.is_none() {
                    self.status = ui::t!("絞り込みは掛かっていません").into();
                    return;
                }
                self.auto_filter = None;
                self.filter_panel = None;
                self.status = ui::t!("絞り込みを解きました").into();
            }
            // 印刷の設定。モデルに置き、保存で原文へ織り込み、PDF が従う。
            // どれもシートの控えで1手戻せる
            "pageorient" => {
                self.commit();
                self.checkpoint();
                let sh = self.sheet_mut();
                sh.landscape = !sh.landscape;
                let landscape = sh.landscape;
                self.dirty = true;
                self.status = ui::tf!(
                    "印刷の向き: {}(PDF と保存に効きます)",
                    if landscape { ui::t!("横") } else { ui::t!("縦") }
                )
                .into();
            }
            "pagesize" => {
                self.commit();
                self.checkpoint();
                // A4 → A3 → B4 → B5 → A5 → A4 の順で回す
                const CYCLE: [u32; 5] = [9, 8, 12, 13, 11];
                let sh = self.sheet_mut();
                let now = sh.paper_size.unwrap_or(9);
                let i = CYCLE.iter().position(|c| *c == now).unwrap_or(0);
                let next = CYCLE[(i + 1) % CYCLE.len()];
                sh.paper_size = Some(next);
                self.dirty = true;
                let name = paper_mm(next).map(|(_, _, n)| n).unwrap_or("A4");
                self.status = ui::tf!("用紙: {}(B は JIS)", name).into();
            }
            "pagemargins" => {
                self.commit();
                self.checkpoint();
                // 既定(20mm)→ 狭い(10mm)→ 広い(30mm)→ 既定
                let sh = self.sheet_mut();
                let (next, label) = match sh.margins_mm {
                    None => (Some((10.0, 10.0, 10.0, 10.0)), "狭い(10mm)"),
                    Some((l, _, _, _)) if l < 15.0 => {
                        (Some((30.0, 30.0, 30.0, 30.0)), "広い(30mm)")
                    }
                    Some(_) => (None, "既定(20mm)"),
                };
                sh.margins_mm = next;
                self.dirty = true;
                self.status = ui::tf!("印刷の余白: {}", label).into();
            }
            "printarea" => {
                self.commit();
                if self.anchor.is_some() {
                    self.checkpoint();
                    let range = self.sel_rect();
                    let had = self.sheet().print_areas.clone();
                    self.sheet_mut().print_areas = vec![range];
                    self.dirty = true;
                    self.status = if had.is_empty() {
                        format!(
                            "印刷範囲: {}:{}(もう一度押すと解除。足すなら「範囲を足す」)",
                            range.0.a1(),
                            range.1.a1()
                        )
                    } else {
                        format!(
                            "印刷範囲を {}:{} に置き換えました(前の {} 域は外しました)",
                            range.0.a1(),
                            range.1.a1(),
                            had.len()
                        )
                    }
                    .into();
                } else if !self.sheet().print_areas.is_empty() {
                    self.checkpoint();
                    self.sheet_mut().print_areas.clear();
                    self.dirty = true;
                    self.status = ui::t!("印刷範囲を解きました(全域を印刷します)").into();
                } else {
                    self.status =
                        ui::t!("印刷範囲にする範囲を Shift+矢印かドラッグで選んでください").into();
                }
            }
            // 大文字小文字。選択の英字に小文字があれば大文字へ、無ければ小文字へ
            // 大文字小文字の変更。本家は5択のサブメニュー —
            // 大小のトグルだけの仮実装をやめ、一覧から選ぶ
            "changecase" => {
                self.commit();
                let at = self.pop_anchor();
                self.pick_kind = "changecase";
                self.pick = Some((menu(&case_modes()), at));
            }
            // 数値の書式・セルのスタイル: 書式の小窓(道具箱)を開く
            // 数値の書式。本家のドロップダウン相当の一覧(その他=コード直打ち)
            "format" => {
                self.commit();
                let at = self.pop_anchor();
                // 今の書式に ✓ を付け、状態行にも言う(本家はコンボが
                // 選択セルの書式に追従する — その代わり)
                let cur = self
                    .sheet()
                    .get(self.cursor)
                    .and_then(|c| c.fmt.number_format.clone());
                let fmts = numfmts();
                // 今のセルの書式に当たる行(鍵=照合用, 見出し=画面用)
                let cur_row = fmts
                    .iter()
                    .find(|(_, _, code)| code.map(|s| s.to_string()) == cur.as_ref().map(|s| s.to_string()))
                    .map(|(k, l, _)| (*k, *l));
                // **✓ は見出しにだけ付ける** — 鍵は素のまま(照合は鍵で走る)
                let mut items: Vec<(String, String)> = fmts
                    .iter()
                    .map(|(k, l, _)| {
                        if Some(*k) == cur_row.map(|(k, _)| k) {
                            (k.to_string(), format!("✓ {l}"))
                        } else {
                            (k.to_string(), l.to_string())
                        }
                    })
                    .collect();
                items.extend(menu(&[ui::item!("その他(書式コードを打つ)…")]));
                self.status = match (cur_row, &cur) {
                    (Some((_, l)), _) => ui::tf!("今の書式: {}", l).into(),
                    (None, Some(code)) => ui::tf!("今の書式: カスタム(コード: {})", code).into(),
                    (None, None) => ui::t!("今の書式: 一般").into(),
                };
                self.pick_kind = "numfmt-pick";
                self.pick = Some((items, at));
            }
            "cell-format" => {
                let at = self
                    .cell_origin_px(self.cursor)
                    .map(|(x, y)| (x + 16.0, y + 16.0))
                    .unwrap_or((HEAD_W + 24.0, ROW_H + 24.0));
                self.fmt_panel = Some(at);
            }
            // 書体: 打つほど絞られるコンボ(照合は日本語名と英語名の両方)。
            // 一覧に無い書体名も打てる。一覧は各項をその書体で描く(view.rs)
            "fontname" => {
                // (鍵=画面に出す名前, 副名=英語名)。絞り込みは両方を見る。
                // 副名が同じでも鍵は画面の文言ではない — 訳したら別の書体を指す
                let all = kumihan::font::list();
                let mut vals: Vec<(String, String)> = Vec::new();
                // 頭に「最近使った書体」— recent_symbols と同じ運び。案内の行を挟む
                let recent: Vec<String> = self
                    .recent_fonts
                    .iter()
                    .filter(|n| all.iter().any(|f| &f.name == *n))
                    .cloned()
                    .collect();
                if !recent.is_empty() {
                    for n in &recent {
                        let ascii = all.iter().find(|f| &f.name == n)
                            .map(|f| f.ascii.clone()).unwrap_or_else(|| n.clone());
                        vals.push((n.clone(), ascii));
                    }
                }
                for f in all.iter().filter(|f| f.japanese) {
                    vals.push((f.name.clone(), f.ascii.clone()));
                }
                if vals.is_empty() {
                    self.status = ui::t!("日本語の書体が見つかりません").into();
                } else {
                    let at = self.pop_anchor();
                    let cur = self.cur_font_name();
                    self.open_combo("font", vals, at, &cur);
                }
            }
            // 大きさ: 一覧は共通の表(sizes_with)。ブックの標準は見た目の元
            // (booktmpl)が繋がったら差し込む — いまは指定なしで Excel の並び。
            // 自由入力(打った数)は 4〜409pt・0.5 刻みに黙って丸める。
            // 丸めは画面の入力だけ(apply_pick)
            "fontsize" => {
                let at = self.pop_anchor();
                let cur = ui::combo::size_label(self.cur_size_pt());
                let vals: Vec<(String, String)> =
                    plain(ui::combo::sizes_with(None).into_iter().map(ui::combo::size_label));
                self.open_combo("size", vals, at, &cur);
            }
            // データタブ: Python 裏方と道具
            "data-from-text" => {
                self.commit();
                self.import_text_dialog(cx);
            }
            "python" => {
                self.commit();
                self.prompt = Some(("py", Editor::new("")));
            }
            // 参照のトレース。矢印の代わりに**セルを光らせる**(見え方だけ)
            "trace-prec" => {
                self.commit();
                let deps = self
                    .sheet()
                    .get(self.cursor)
                    .and_then(|c| c.formula.as_ref())
                    .map(|f| sheet::calc::deps(f))
                    .unwrap_or_default();
                if deps.is_empty() {
                    self.status = ui::t!("このセルの式は他のセルを参照していません").into();
                } else {
                    self.status = ui::tf!(
                        "{} の参照元 {} セルを光らせました(トレース矢印の削除で消す)",
                        self.cursor.a1(),
                        deps.len()
                    )
                    .into();
                    self.trace = deps.into_iter().map(|p| (p, true)).collect();
                }
            }
            "trace-dep" => {
                self.commit();
                let me = self.cursor;
                let dependents: Vec<Pos> = self
                    .sheet()
                    .cells
                    .iter()
                    .filter(|(_, c)| {
                        c.formula
                            .as_ref()
                            .is_some_and(|f| sheet::calc::deps(f).contains(&me))
                    })
                    .map(|(p, _)| *p)
                    .collect();
                if dependents.is_empty() {
                    self.status = ui::tf!("{} を参照している式はありません", me.a1()).into();
                } else {
                    self.status = ui::tf!(
                        "{} の参照先 {} セルを光らせました(トレース矢印の削除で消す)",
                        me.a1(),
                        dependents.len()
                    )
                    .into();
                    self.trace = dependents.into_iter().map(|p| (p, false)).collect();
                }
            }
            "remove-arrows" => {
                if self.trace.is_empty() {
                    self.status = ui::t!("トレースの矢印は出ていません").into();
                    return;
                }
                self.trace.clear();
                self.status = ui::t!("トレースを消しました").into();
            }
            // 推奨チャート = いまの一手(棒グラフ)をそのまま勧める
            "insrecommend" => {
                self.commit();
                if self.anchor.is_none() {
                    self.status =
                        ui::t!("グラフにする範囲を選んでください(1列目が項目名、2列目からが数)").into();
                } else {
                    let (a, b) = self.sel_rect();
                    self.insert_chart(a, b, cx);
                }
            }
            // ピボットテーブル = polars が裏方。結果は「その時の値」で右に置く
            // (元が変わったら選び直してもう一度 — 開く=再計算の仕掛けは持たない)
            "pivot-insert" => {
                self.commit();
                // 範囲を選んでいなければ表全体を自動検出(オートフィルタと同じ)。
                // カーソルを表に置くだけで挿入できる — 範囲選択は絞りたいときだけ
                let picked = if self.anchor.is_some() {
                    Some(self.sel_rect())
                } else {
                    let (rows, cols) = self.sheet().extent();
                    (rows >= 2 && cols > 0)
                        .then(|| (Pos::new(0, 0), Pos::new(rows - 1, cols - 1)))
                };
                if let Some((a, b)) = picked {
                    if b.row <= a.row {
                        self.status = ui::t!("見出しの下にデータの行が要ります").into();
                    } else {
                        let headers: Vec<String> = (a.col..=b.col)
                            .map(|c| {
                                let v = self
                                    .sheet()
                                    .get(Pos::new(a.row, c))
                                    .map(|x| x.value.display())
                                    .unwrap_or_default();
                                if v.is_empty() { col_name(c) } else { v }
                            })
                            .collect();
                        self.pivot_pend = Some(PivotPend {
                            a,
                            b,
                            headers,
                            rows_sel: Vec::new(),
                            cols_sel: Vec::new(),
                            val_sel: String::new(),
                            replace: None,
                        });
                        // **おすすめを先に並べます**(2026-08-09 発注者確定の
                        // 方針: 先回りして作らず、候補を出して人が選ぶ)。
                        // 候補が1つも無ければ、これまでどおり行の選択から
                        if self.pivot_suggest_pick() {
                            self.status = ui::t!("おすすめの形を選ぶ(自分で選ぶこともできます)").into();
                        } else {
                            self.status = ui::t!("行に並べる見出しをクリックで選ぶ(複数可)。選んだら「決定」").into();
                            self.pivot_pick("pivot-rows-pick");
                        }
                    }
                } else {
                    // **理由を言う。** clippy の指摘を直したとき、ここを丸ごと
                    // 消していた(2026-08-11)。押しても何も起きないボタンに
                    // なっていて、家訓に真正面から反していた —
                    // 見つけたのは `lang` の文言の門番で、「もう使っていない訳が
                    // 1句あります」と鳴った。**別の crate の検査が拾った**
                    self.status =
                        ui::t!("元の表がありません(1行目が見出し、下にデータの行)").into();
                }
            }
            // シートの保護。パスワードは掛けない(掛けた振りもしない)—
            // Excel でも「保護されたシート」に見え、解除も同じ1手でできる
            "prot-doc" => {
                let name = self.sheet().name.clone();
                if self.sheet().protected {
                    self.sheet_mut().protected = false;
                    self.dirty = true;
                    self.status = ui::tf!(
                        "シート「{}」の保護を外しました(編集できます。保存で xlsx にも残ります)",
                        name
                    )
                    .into();
                } else {
                    self.commit();
                    self.sheet_mut().protected = true;
                    self.dirty = true;
                    self.status = ui::tf!(
                        "シート「{}」を保護しました(ロックを外したセルだけ書けます。「許可する操作」で緩められます。同じボタンで解除。パスワードは掛けません — 掛けた振りもしません)",
                        name
                    )
                    .into();
                }
            }
            // セルのロック。**保護の効き目はここで決まる** — 保護中は
            // ロックの掛かったセルだけが堰き止められる。帳票は
            // 「記入欄のロックを外して、シートを保護する」が定石
            "cell-lock" => {
                // いま選んでいる所が全部ロック済みなら外す、でなければ掛ける
                let (a, b) = self.sel_rect();
                let all_locked = (a.row..=b.row).all(|r| {
                    (a.col..=b.col).all(|c| {
                        !self.sheet().get(Pos::new(r, c)).map(|x| x.fmt.unlocked).unwrap_or(false)
                    })
                });
                self.fmt(|f| f.unlocked = all_locked);
                self.status = if all_locked {
                    ui::t!("ロックを外しました(保護中でもここは書けます)").into()
                } else {
                    ui::t!("ロックを掛けました(保護中は書けません)").into()
                };
            }
            // **式を隠す。** ロックと同じ組(xlsx の <protection>)で、
            // こちらは「保護中、このセルの式を数式バーに出さない」。
            // 値は見える — 単価表の掛け率を伏せる、といった使い方
            "cell-hide-formula" => {
                let (a, b) = self.sel_rect();
                let all_shown = (a.row..=b.row).all(|r| {
                    (a.col..=b.col).all(|c| {
                        !self
                            .sheet()
                            .get(Pos::new(r, c))
                            .map(|x| x.fmt.formula_hidden)
                            .unwrap_or(false)
                    })
                });
                self.fmt(|f| f.formula_hidden = all_shown);
                self.status = if all_shown {
                    // **保護していないと効かないことを、その場で言う** —
                    // 押して何も起きないように見えるのが一番悪い
                    if self.sheet().protected {
                        ui::t!("式を隠しました(数式バーに出ません。値は見えます)").into()
                    } else {
                        ui::t!("式を隠す印を付けました(シートを保護すると効きます)").into()
                    }
                } else {
                    ui::t!("式を隠すのをやめました").into()
                };
            }
            // 保護中に何を許すか(本家の「このシートのすべてのユーザーに
            // 許可する操作」)。✓ の一覧を押して入切する
            "prot-allow" => {
                let at = self.pop_anchor();
                let a = self.sheet().protect_allow.clone();
                // ☑/☐ は**見出しだけ**に付ける(鍵は名前そのもの — 入切の照合が走る)。
                // 名前は sheet の表、訳は calc の表 — 引き当ては鍵で
                let items: Vec<(String, String)> = a
                    .items()
                    .iter()
                    .map(|(n, on)| {
                        let l = crate::util::protect_allow_label(n);
                        (n.to_string(), format!("{} {}", if *on { "☑" } else { "☐" }, l))
                    })
                    .collect();
                self.pick_kind = "prot-allow";
                self.pick_note = Some(
                    ui::t!("保護中に許す操作(押して入切。保護していないときは効きません)").into(),
                );
                self.pick = Some((items, at));
            }
            // 暗号化。パスワードを決めると、保存で ECMA-376 Standard
            // (AES-128)の複合ファイルに包む。空 Enter で解除
            "prot-encrypt" => {
                self.pw_pending = None;
                self.pw_show = false;
                self.prompt = Some(("pw-set", Editor::new("")));
                self.status = if self.encrypt_pw.is_some() {
                    ui::t!("暗号化は入っています。新しいパスワードを打って Enter(空のまま Enter で暗号化をやめる)").into()
                } else {
                    ui::t!("暗号化: パスワードを打って Enter(次の保存から効きます)").into()
                };
            }
            // デジタル署名。**隣の .sig への添え書き**(Ed25519)。
            // Excel の署名欄には出ない独自方式 — そう言って出す。
            // 有効なら報告だけ、無効・未署名なら(作り直して)署名する
            "prot-sign" => {
                // **署名の中身は ops に1本**(2026-08-21)。前は同じ 40 行を
                // writer と2つ持っていて、しかも下の2つの文言が `format!` の
                // ままで、14 言語で日本語が出ていました
                let Some(p) = self.path.clone() else {
                    self.status =
                        ui::t!("まだファイルになっていません(先に保存してください)").into();
                    return;
                };
                if self.dirty {
                    self.status =
                        ui::t!("未保存の変更があります。保存してから署名してください").into();
                    return;
                }
                self.status = match ops::sign_or_verify(&p) {
                    Ok(ops::Signed::Verified(signer)) => {
                        ui::tf!("署名は有効です — {} が署名した時のままの中身です", signer)
                    }
                    // 文章とは別の文です(理由は writer/src/cmds.rs の註)
                    Ok(ops::Signed::Wrote(name)) => ui::tf!(
                        "署名しました — 隣の {} に添え書き(独自方式。\
                         Excel の署名欄には出ません。もう一度押すと検めます)",
                        name
                    ),
                    Err(ops::SignErr::Read(e)) => ui::tf!("読めません: {}", e),
                    Err(ops::SignErr::Write(e)) => ui::tf!("署名が置けません: {}", e),
                    Err(ops::SignErr::Key(e)) => {
                        ui::tf!("署名できません: {}", key_err_msg(e))
                    }
                }
                .into();
            }
            // 共同編集モード。実体はファイルの錠(.~lock)による早い者勝ちの
            // 編集権。押すと錠の今を確かめ、先客が去っていれば取り直す
            "coauth-mode" => match self.path.clone() {
                None => {
                    self.status =
                        ui::t!("まだファイルになっていません(保存すると編集権=錠を取ります)").into();
                }
                Some(p) => {
                    if self.my_lock.is_some() {
                        self.status = ui::tf!(
                            "編集権はこちら({})にあります。同じブックは先に開いた人が書け、後の人は読むだけになります(錠は .~lock ファイル)",
                            lock_identity()
                        )
                        .into();
                    } else {
                        self.acquire_lock(&p);
                        self.status = match &self.locked_by {
                            Some(who) => format!(
                                "{who} が編集中です(読めますが上書き保存はできません。相手が閉じたら、またこのボタンで確かめてください)"
                            )
                            .into(),
                            None => ui::t!("先客が居なくなっていたので、編集権を取り直しました").into(),
                        };
                    }
                }
            },
            // 「コメントの表示」は2択の小窓に(2026-08-13、台帳「並べ替え」)。
            // 一覧の板と、セルに出る付記は別の物 — 前は付記の入切だけだった
            "co-showcomment" => {
                let items = vec![
                    (
                        "list".to_string(),
                        if self.comment_list.is_some() {
                            ui::t!("コメントの一覧を閉じる").to_string()
                        } else {
                            ui::t!("コメントの一覧を出す").to_string()
                        },
                    ),
                    (
                        "tips".to_string(),
                        // **札は短く。** 小窓の幅を越えると切れて読めない
                        // (実機で見た)。「付いたままです」は押したあとの
                        // 状態行が言う
                        if self.show_comments {
                            ui::t!("セルの付記を隠す").to_string()
                        } else {
                            ui::t!("セルに付記を出す").to_string()
                        },
                    ),
                ];
                let at = self.pop_anchor();
                self.pick_kind = "comment-show";
                self.pick = Some((items, at));
            }
            // 削除は範囲を選ばせる(本家の「現在/自分/すべて」)。
            // **押した瞬間に消さない** — すべてはブック全体に効く
            "co-delcomment" => {
                let n_all: usize =
                    self.book.sheets.iter().map(|s| s.comments.len()).sum();
                let items = vec![
                    (
                        "here".to_string(),
                        ui::tf!("このセルのコメント({})", self.cursor.a1()),
                    ),
                    ("mine".to_string(), ui::t!("自分のコメント").to_string()),
                    (
                        "all".to_string(),
                        ui::tf!("すべてのコメント(ブック全体の {} 件)", n_all),
                    ),
                ];
                let at = self.pop_anchor();
                self.pick_note =
                    Some(ui::t!("どこまで消すかを選ぶ(どれも Ctrl+Z で戻せます)").into());
                self.pick_kind = "comment-del";
                self.pick = Some((items, at));
            }
            // バージョン履歴。上書き保存のたびに .jo-history へ残る控えの一覧
            "co-history" => {
                if self.path.is_none() {
                    self.status =
                        ui::t!("まだファイルになっていません(保存すると、上書きのたびに控えが残ります)").into();
                } else {
                    let v = self.versions();
                    if v.is_empty() {
                        self.status =
                            ui::t!("控えはまだありません(上書き保存のたびに .jo-history へ残ります)").into();
                    } else {
                        let names: Vec<String> = v.iter().map(|(n, _)| n.clone()).collect();
                        self.pick_paths = v;
                        self.pick_kind = "history";
                        let at = self.pop_anchor();
                        // 控えの名前はファイル名 — 訳さない
                        self.pick = Some((plain(names), at));
                        self.status =
                            ui::t!("バージョン履歴: 選ぶと控えを名無しの複製で開きます(いまの書きかけは要るなら先に保存)").into();
                    }
                }
            }
            // チャット。ブックの隣の申し送り帳(.chat.txt)へ名乗り付きで追記。
            // サーバーは無いので生放送ではない — ファイル越しの言伝
            "co-chat" => match self.chat_path() {
                None => {
                    self.status =
                        ui::t!("まだファイルになっていません(保存すると、隣に申し送り帳ができます)").into();
                }
                Some(cp) => {
                    let tail = std::fs::read_to_string(&cp)
                        .map(|t| {
                            t.lines()
                                .rev()
                                .take(3)
                                .map(|l| l.to_string())
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .collect::<Vec<_>>()
                                .join(" / ")
                        })
                        .unwrap_or_default();
                    self.status = if tail.is_empty() {
                        ui::t!("まだ言伝はありません(打って Enter で書き残します)").into()
                    } else {
                        format!("言伝: {tail}").into()
                    };
                    self.prompt = Some(("chat", Editor::new("")));
                }
            },
            // マクロ = Python in Calc と同じ実体(サンドボックスの中で .py を回す)
            // **操作の記録**(発注者 2026-08-15)。主従が逆転した今、
            // 「何を書けばいいか」を教える唯一の道具 — 手でやった操作が
            // そのまま走る Python の台本になる。押すたびに 始める/止める
            "rec-toggle" => {
                if self.rec.is_none() {
                    self.rec_start();
                    return;
                }
                let Some(script) = self.rec_stop() else { return };
                // **記録は記録の置き場へ**(2026-08-16)。plugins に落とすと、
                // 読む前・直す前の台本がそのままマクロの一覧に並ぶ
                let dir = pyrun::records_dir();
                let _ = std::fs::create_dir_all(&dir);
                let mut n = 1;
                while dir.join(format!("記録{n}.py")).exists() {
                    n += 1;
                }
                let path = dir.join(format!("記録{n}.py"));
                let 行 = script.lines().count();
                match std::fs::write(&path, script) {
                    Ok(()) => {
                        self.status = match ui::open_for_edit(&path.display().to_string()) {
                            Ok(tool) => ui::tf!(
                                "記録を {} に書きました({} 行)— {} で開きます",
                                path.file_name().unwrap_or_default().to_string_lossy(),
                                行, tool
                            )
                            .into(),
                            Err(_) => ui::tf!(
                                "記録を {} に書きました({} 行)",
                                path.display().to_string(), 行
                            )
                            .into(),
                        };
                    }
                    Err(e) => self.status = ui::tf!("記録を書けません: {}", e).into(),
                }
            }
            // **編集は表計算の仕事ではない**(発注者 2026-08-15)。
            // 骨組みだけ書いたファイルを作り、編集の道具に渡す —
            // 利用者の editor(settings.toml)→ 隣の writer → 機械の既定、の順
            // **新しい .py は関数の置き場へ**(2026-08-16)。この腕は
            // 「保存したらセルから呼べます」と言う — それは funcs の話
            "py-new" => {
                let dir = pyrun::funcs_dir();
                let _ = std::fs::create_dir_all(&dir);
                let mut n = 1;
                while dir.join(format!("新しい道具{n}.py")).exists() {
                    n += 1;
                }
                let path = dir.join(format!("新しい道具{n}.py"));
                if let Err(e) = std::fs::write(&path, ui::pyedit::skeleton(&format!("新しい道具{n}"))) {
                    self.status = ui::tf!("作れません: {}", e).into();
                    return;
                }
                self.status = match ui::open_for_edit(&path.display().to_string()) {
                    Ok(tool) => ui::tf!(
                        "{} を作って {} で開きました(保存したらセルから呼べます)",
                        path.file_name().unwrap_or_default().to_string_lossy(), tool
                    )
                    .into(),
                    Err(e) => ui::tf!("{} は作りましたが開けません: {}",
                        path.display().to_string(), e).into(),
                };
            }
            // **式から呼べる Python の関数の一覧**(funcs の置き場)。
            // 押すと名前と、その中の def が並ぶ — セルに `=名前(...)` と
            // 書けるのはここに在る物だけ(2026-08-16)
            "func-list" => {
                self.prompt = None;
                let dir = pyrun::funcs_dir();
                let fs = pyrun::outline_in(&dir);
                if fs.is_empty() {
                    self.status = ui::tf!(
                        "式から呼べる関数はまだありません({} に .py を置いてください)",
                        dir.display().to_string()
                    )
                    .into();
                    return;
                }
                let at = self.pop_anchor();
                self.pick_paths =
                    fs.iter().map(|(m, _)| (m.clone(), dir.join(format!("{m}.py")))).collect();
                self.pick_kind = "py-edit";
                self.pick_note =
                    Some(ui::t!("セルに =名前(...) と書けます。選ぶと編集の道具で開きます").into());
                self.pick = Some((
                    fs.iter()
                        .map(|(m, d)| {
                            // def が無い .py もある(上から下まで走る形)。
                            // 尻の「—」を残すと、見出しが途中で切れたように見える
                            let defs = d.join(" ");
                            let head =
                                if defs.is_empty() { m.clone() } else { format!("{m} — {defs}") };
                            (m.clone(), head)
                        })
                        .collect(),
                    at,
                ));
            }
            // **リボンに出るマクロの一覧**(ribbon の置き場)。plugins との
            // 違いは名乗り — 札・絵・段を名乗った .py だけがボタンになる
            "ribbon-list" => {
                // 一覧に混ぜる印。**値は画面に出ない** — 出るのは見出しの方で、
                // こちらは選ばれた物を見分けるための印
                const MIHON: &str = "\u{1}見本";
                const BUNDLED_MARK: char = '\u{2}';
                self.prompt = None;
                let dir = pyrun::ribbon_dir();
                let decls = pyrun::ribbon_decls(&dir);
                let at = self.pop_anchor();
                // 一覧は3段: 名乗ったマクロ / 見本を作る / 同梱の台本(読むだけ)。
                // **空でも一覧を出す** — 何も無いと言われるより、そこから
                // 作り始められる方が近い
                let mut items: Vec<(String, String)> = decls
                    .iter()
                    .map(|d| (d.module.clone(), format!("{} — {}", d.module, d.label)))
                    .collect();
                self.pick_paths = decls
                    .iter()
                    .map(|d| (d.module.clone(), dir.join(format!("{}.py", d.module))))
                    .collect();
                items.push((MIHON.into(), ui::t!("見本を作る").to_string()));
                for (n, _) in pyrun::BUNDLED {
                    items.push((
                        format!("{BUNDLED_MARK}{n}"),
                        ui::tf!("同梱: {}(読むだけ)", *n).to_string(),
                    ));
                }
                self.pick_kind = "ribbon-macro";
                self.pick_note = Some(
                    ui::t!("名乗った .py がこの段のボタンになります。選ぶと編集の道具で開きます").into(),
                );
                self.pick = Some((items, at));
            }
            // 置いてある .py の一覧。**選ぶと走る。**
            //
            // 2026-08-15 の主従逆転(fb50fbd)で `py-run` の腕が落ち、以来
            // マクロの段に**マクロを走らせる道が無かった**(走るのは
            // データ > Python に `@名前` と打つ1本だけ)。writer は
            // 落ちておらず、calc だけが失っていた(2026-08-16 発注者
            // 「普通のマクロは、ファイルからの実行になりませんか」)。
            // 直すのは「置き場を開く」か `@edit 名前`
            "py-list" => {
                self.prompt = None;
                let dir = plugins_dir();
                let plugs = crate::py::plugin_outline();
                if plugs.is_empty() {
                    self.status =
                        ui::tf!("plugins に .py がありません({})", dir.display().to_string()).into();
                    return;
                }
                let at = self.pop_anchor();
                self.pick_paths = plugs
                    .iter()
                    .map(|(m, _)| (m.clone(), dir.join(format!("{m}.py"))))
                    .collect();
                self.pick_kind = "py-run";
                self.pick_note =
                    Some(ui::t!("選ぶと走ります(直すのは「置き場を開く」か @edit 名前)").into());
                self.pick = Some((
                    plugs
                        .iter()
                        .map(|(m, d)| {
                            // def が無い .py もある(上から下まで走る形)。
                            // 尻の「—」を残すと、見出しが途中で切れたように見える
                            let defs = d.join(" ");
                            let head =
                                if defs.is_empty() { m.clone() } else { format!("{m} — {defs}") };
                            (m.clone(), head)
                        })
                        .collect(),
                    at,
                ));
            }
            "py-line" => {
                self.prompt = Some(("py", Editor::new("")));
            }
            "py-calc" => {
                self.run_py_calc(cx);
            }
            // チェックボックス(セルの部品)。空のセルに FALSE を置くと
            // ☑/☐ で見え、空白キーで切り替わる(Excel では TRUE/FALSE の値)
            "inscheckbox" => {
                self.commit();
                let (a, b) = self.sel_rect();
                let mut empties = Vec::new();
                let mut bools = 0usize;
                let mut skipped = 0usize;
                for r in a.row..=b.row {
                    for c in a.col..=b.col {
                        let p = Pos::new(r, c);
                        match self.sheet().get(p).map(|x| &x.value) {
                            None | Some(Value::Empty) => empties.push(p),
                            Some(Value::Bool(_)) => bools += 1,
                            _ => skipped += 1,
                        }
                    }
                }
                if empties.is_empty() && bools == 0 {
                    self.status =
                        ui::t!("空のセルを選んでください(中身のあるセルは潰しません)").into();
                } else {
                    if !empties.is_empty() {
                        self.checkpoint();
                        for p in &empties {
                            let mut cell =
                                self.sheet().get(*p).cloned().unwrap_or_default();
                            cell.formula = None;
                            cell.value = Value::Bool(false);
                            self.book.sheets[self.active].set(*p, cell);
                        }
                        recalc_book(&mut self.book, self.active);
                        self.dirty = true;
                        self.sync_input();
                    }
                    let skip_note = if skipped > 0 {
                        format!("。中身のある {skipped} セルは触っていません")
                    } else {
                        String::new()
                    };
                    self.status = ui::tf!(
                        "チェックボックスを {} 個置きました(空白キーで切替。Excel では TRUE/FALSE で見えます{})",
                        empties.len(),
                        skip_note
                    )
                    .into();
                }
            }
            // スライサー。カーソルの列の一意な値をボタンで並べ、押して絞る。
            // 絞り込みと同じく**見え方だけ**(保存される中身は変わらない)
            "insslicer" => {
                {
                    self.commit();
                    // **どの列にするかを選ばせる。** 前はカーソルの列を
                    // 黙って使っていたので、押した人は自分がどの列を
                    // スライサーにしたのか分からなかった(2026-08-13、
                    // 台帳「スライサー挿入時の列チェック」)。
                    // **開いている列には ☑ を付け、押すとその板を閉じる** —
                    // 何枚でも開ける造りになったので、開ける口と閉じる口を
                    // 同じ一覧にした(2026-08-13、台帳「複数列」)
                    let (rows, cols) = self.sheet().extent();
                    if rows < 2 {
                        self.status =
                            ui::t!("スライサーにする列を選んでください(見出しの下にデータの行が要ります)").into();
                    } else {
                        let items: Vec<(String, String)> = (0..cols)
                            .map(|c| {
                                let head = self
                                    .sheet()
                                    .get(Pos::new(0, c))
                                    .map(|x| x.value.display())
                                    .unwrap_or_default();
                                let name = col_name(c);
                                // 鍵は列の名前(A, B…)— 見出しは訳さないが、
                                // **空の見出しでも列を選べる**ようにする
                                let label = if head.trim().is_empty() {
                                    name.clone()
                                } else {
                                    format!("{name}: {head}")
                                };
                                // ☑/☐ は apply_pick が剥がしてから照合する
                                let on = self.slicers.iter().any(|s| s.col == c);
                                (name, format!("{} {label}", if on { "☑" } else { "☐" }))
                            })
                            .collect();
                        let at = self.pop_anchor();
                        self.pick_note =
                            Some(ui::t!("スライサーにする列を選ぶ(何枚でも。☑ を押すと閉じる。見え方だけで、中身は変わりません)").into());
                        self.pick_kind = "slicer-col";
                        self.pick = Some((items, at));
                    }
                }
            }
            // テキストアート。文字をパネルに打つと飾り文字を描いて画像で置く
            "instextart" => {
                self.commit();
                self.prompt = Some(("textart", Editor::new("")));
                self.status =
                    ui::t!("テキストアート: 文字を打つと、太字+縁取りの飾り文字を画像で置きます").into();
            }
            // 方程式(数式エディタ)。式をパネルに打つと mathtext が清書して画像で置く
            "insequation" => {
                self.commit();
                self.prompt = Some(("equation", Editor::new("")));
                self.status =
                    ui::t!("方程式: TeX の書き方で(例: \\frac{a}{b} や \\sum_{i=1}^n i^2)。Enter で清書").into();
            }
            // SmartArt。分類 → 形の2段の一覧(分類・並び・名前は本家)
            "inssmartart" => {
                self.commit();
                let names: Vec<(String, String)> =
                    smartart().iter().map(|(k, l, _)| (k.to_string(), l.to_string())).collect();
                self.pick_kind = "sa-cat";
                let at = self.pop_anchor();
                self.pick = Some((names, at));
                self.status =
                    ui::t!("SmartArt: 分類 → 形の順に選ぶ(図形の集まりとして入ります)").into();
            }
            // ソルバー。ONLYOFFICE と同じ小窓を開く(解法も同じ単体法 LP)
            "solver" => {
                if self.solver.take().is_none() {
                    self.commit();
                    let init = if self.anchor.is_some() {
                        self.sel_rect().0.a1()
                    } else {
                        self.cursor.a1()
                    };
                    self.solver = Some(Solver::new(&init));
                    self.status =
                        ui::t!("ソルバー: 欄を押して打つ。目的・変数セル・制約を決めて「解を求める」").into();
                }
            }
            // 下付き(vertAlign subscript)。上付きは本家 calc にも無い
            "subscript" => {
                self.fmt(|f| f.subscript = !f.subscript);
                self.status = ui::t!("下付きを切り替えました").into();
            }
            // 両端揃え(セルの横揃え。折り返した行を左右に伸ばす)
            "align-just" => {
                self.fmt(|f| {
                    f.align = if f.align == sheet::model::HAlign::Justify {
                        sheet::model::HAlign::General
                    } else {
                        sheet::model::HAlign::Justify
                    };
                    f.wrap = true; // 揃えるには折り返しが要る
                });
                self.status = ui::t!("両端揃えにしました(折り返して全体を表示も入れます)").into();
            }
            // 文字の回転(縦書きのセル。90度ずつ回る)
            // 文字の向き: 本家のプリセット+任意の角度(第2便3段)
            "text-orient" => {
                let at = self.pop_anchor();
                let cur = self
                    .sheet()
                    .get(self.cursor)
                    .and_then(|c| c.fmt.rotation)
                    .unwrap_or(0);
                // ✓ は見出しにだけ。鍵は素のまま(picks.rs の角度の引き当てが鍵で走る)
                let items: Vec<(String, String)> = [
                    (ui::item!("角度なし"), 0i32),
                    (ui::item!("左上がり 45度"), 45),
                    (ui::item!("右下がり 45度"), 135),
                    (ui::item!("上向き 90度"), 90),
                    (ui::item!("下向き 90度"), 180),
                    (ui::item!("縦書き(1字ずつ積む)"), 255),
                ]
                .iter()
                .map(|((k, l), deg)| {
                    (
                        k.to_string(),
                        if *deg == cur { format!("✓ {l}") } else { l.to_string() },
                    )
                })
                .chain(menu(&[ui::item!("その他(角度を打つ)…")]))
                .collect();
                self.pick_note = Some(ui::t!("文字の向き(xlsx と同じ数え方 — 上向きが正)").into());
                self.pick_kind = "orient-pick";
                self.pick = Some((items, at));
            }
            // 計算方法(自動 ⇔ 手動)。手動のときは F9 で計算する
            "calc-mode" => {
                self.auto_calc = !self.auto_calc;
                // ファイルにも残す(calcPr)。開き直して勝手に自動へ戻さない
                self.book.calc_manual = !self.auto_calc;
                self.dirty = true;
                self.status = if self.auto_calc {
                    ui::t!("計算方法: 自動(いつもすぐ計算します)").into()
                } else {
                    ui::t!("計算方法: 手動(F9 で計算します — 大きな表で待たされない)").into()
                };
            }
            // 参照の形式(A1 ⇔ R1C1)。式の中身は A1 のまま、見せ方と打ち方が変わる
            "ref-style" => {
                self.commit();
                self.book.r1c1 = !self.book.r1c1;
                self.dirty = true;
                self.sync_input(); // 数式バーの見せ方も切り替える
                self.status = if self.book.r1c1 {
                    ui::t!("参照の形式: R1C1(R[行]C[列] — 列見出しも数字になります)").into()
                } else {
                    ui::t!("参照の形式: A1(いつもの B2 の形)").into()
                };
            }
            // 反復計算(循環参照の反復解決)。入ならパネルで回数と変化量を聞く
            "calc-iter" => {
                let cur = self
                    .book
                    .calc_iter
                    .map(|(n, d)| format!("{n} {d}"))
                    .unwrap_or_else(|| "100 0.001".into());
                self.prompt = Some(("calc-iter", Editor::new(&cur)));
            }
            // 関数の挿入 = 本家と同じ小窓(検索・分類・一覧・説明)。
            // 数式バーの fx と同じ実体
            "insert-function" => {
                self.fn_dlg = Some(FnDlg {
                    search: Editor::new(""),
                    group: 0,
                    sel: 0,
                });
                self.status =
                    ui::t!("関数を挿入: 打って絞り込み、↑↓で選んで Enter(Esc で取消)").into();
            }
            // セルのスタイル(既定の書式の組。押すと一覧から選ぶ)
            "cell-styles" => {
                let at = self.pop_anchor();
                self.pick_kind = "cell-style";
                self.pick = Some((
                    cell_styles().iter().map(|(k, l, _)| (k.to_string(), l.to_string())).collect(),
                    at,
                ));
                self.status = ui::t!("セルのスタイル: 選ぶと選択に掛かります(Ctrl+Z で戻せます)").into();
            }
            // シートの表示(隠したシートを戻す/いまのシートを隠す)
            "sheet-view" => {
                let hidden: Vec<(usize, String)> = self
                    .book
                    .sheets
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.hidden)
                    .map(|(i, s)| (i, s.name.clone()))
                    .collect();
                if hidden.is_empty() {
                    // 隠すほう(最後の1枚は隠さない — 見えるシートがゼロになる)
                    if self.book.sheets.iter().filter(|s| !s.hidden).count() <= 1 {
                        self.status = ui::t!("最後の1枚は隠せません").into();
                    } else {
                        let n = self.sheet().name.clone();
                        self.checkpoint_book();
                        self.sheet_mut().hidden = true;
                        // 見えるシートへ移る
                        if let Some(i) = self.book.sheets.iter().position(|s| !s.hidden) {
                            self.switch_sheet(i);
                        }
                        self.dirty = true;
                        self.status = ui::tf!(
                            "シート「{}」を隠しました(同じボタンで戻せます。保存で xlsx にも残ります)",
                            n
                        )
                        .into();
                    }
                } else {
                    self.pick_kind = "unhide";
                    self.pick_paths = hidden
                        .iter()
                        .map(|(i, n)| (n.clone(), PathBuf::from(i.to_string())))
                        .collect();
                    let at = self.pop_anchor();
                    // シート名は中身 — 訳さない
                    self.pick = Some((plain(hidden.into_iter().map(|(_, n)| n)), at));
                    self.status = ui::t!("隠したシート: 選ぶと表示に戻します").into();
                }
            }
            // ウォッチウィンドウ(見張りの窓)。選んだセルを控えて下に見せる
            "watch" => {
                let (a, b) = self.sel_rect();
                let mut n = 0usize;
                for r in a.row..=b.row {
                    for c in a.col..=b.col {
                        let p = Pos::new(r, c);
                        if (self.sheet().get(p).and_then(|x| x.formula.as_ref()).is_some()
                            || self.anchor.is_none())
                            && !self.watch.contains(&(self.active, p)) {
                                self.watch.push((self.active, p));
                                n += 1;
                            }
                    }
                }
                if n == 0 && !self.watch.is_empty() {
                    self.watch.clear();
                    self.status = ui::t!("見張りを空にしました").into();
                } else {
                    self.status = ui::tf!(
                        "{} 個を見張ります(値は下のステータスバーに出ます。もう一度押すと空に)",
                        n
                    )
                    .into();
                }
            }
            // 昇順/降順(ホーム・データ)。右クリックの並べ替え▸と同じ道
            "sort-asc" | "sort-desc" => self.sort_active(id == "sort-asc"),
            // 描画の「選択」= 道具を措いてセルの操作に戻る(本家の並びの先頭)
            "draw-select" => {
                self.tool = None;
                self.ink_cur = None;
                self.status = ui::t!("セルの操作に戻りました").into();
            }
            // 描画(ペン・蛍光ペン・消しゴム)。writer と同じ形の道具の入切
            "pen" | "highlighter" | "eraser" => {
                let t = match id {
                    "pen" => 0u8,
                    "highlighter" => 1,
                    _ => 2,
                };
                self.tool = if self.tool == Some(t) { None } else { Some(t) };
                self.ink_cur = None;
                self.status = match self.tool {
                    Some(0) => ui::t!("ペン: 表の上をドラッグで描く(もう一度押すか Esc で戻る)").into(),
                    Some(1) => ui::t!("蛍光ペン: ドラッグで引く(セルの上に薄く乗る)").into(),
                    Some(2) => ui::t!("消しゴム: 線をなぞると1筆ずつ消える").into(),
                    _ => ui::t!("セルの操作に戻りました").into(),
                };
            }
            // **AI の宛先。** リボンの AI タブは 2026-08-15 に廃した
            // (発注者「いまの AI のボタンは全部要らない」)ので、この命令は
            // 詳細設定の「AI の宛先」からだけ呼ばれる。9つの動詞は左パネルの
            // 会話に譲った — ボタンでは作れない頼み方も打てば通る
            // 配色の変更(テーマ色の組を入れ替える)。テーマ由来の色を
            // 使っているセルは、色がそのまま追従する
            "colorschemas" => {
                let at = self.pop_anchor();
                self.pick_kind = "scheme";
                // 名前は sheet の表(theme.rs)が持つ = 鍵。訳は calc の表で当てる
                let items: Vec<(String, String)> = sheet::theme::SCHEMES
                    .iter()
                    .map(|(n, _)| (n.to_string(), crate::util::color_scheme_label(n)))
                    .collect();
                self.pick = Some((items, at));
                self.status = ui::t!("配色の変更: 選ぶとテーマ色が入れ替わります").into();
            }
            // インターフェイステーマ(画面の明暗)。**セルは白のまま**
            // 範囲に変換する(表オブジェクトを外す。**書式と式は残る**)
            "td-torange" => {
                self.commit();
                let p = self.cursor;
                self.checkpoint();
                match sheet::tabledesign::to_range(&mut self.book.sheets[self.active], p) {
                    None => {
                        self.status =
                            ui::t!("表の中にカーソルを置いてください(表のない範囲は「表の挿入」で表にできます)").into();
                    }
                    Some(name) => {
                        self.dirty = true;
                        self.status = ui::tf!(
                            "表「{}」を普通の範囲に戻しました(見出し行の色や縞模様の書式と式はそのまま残ります)",
                            name
                        )
                        .into();
                    }
                }
            }
            // テーブルのサイズ変更(範囲を変える)。パネルで新しい範囲を聞く
            "td-resize" => {
                self.commit();
                let p = self.cursor;
                match self.sheet().tables.iter().position(|t| t.contains(p)) {
                    None => self.status = ui::t!("表の中にカーソルを置いてください").into(),
                    Some(i) => {
                        let t = &self.sheet().tables[i];
                        let init = format!("{}:{}", t.a.a1(), t.b.a1());
                        self.status = ui::tf!("表「{}」の新しい範囲は?", t.name).into();
                        self.prompt = Some(("table-resize", Editor::new(&init)));
                    }
                }
            }
            // シートの方向(右から左へ)。**日本語も右から書くことがある**
            "rtl-sheet" => {
                let on = !self.sheet().rtl;
                self.sheet_mut().rtl = on;
                self.dirty = true;
                self.status = if on {
                    ui::t!("右から左へ並べます(右横書き。列は右から A B C…)").into()
                } else {
                    ui::t!("左から右へ戻しました").into()
                };
            }
            // 文字の向き(セルの中を右横書きに)。1字ずつ右から並べる
            "direction" => {
                self.fmt(|f| f.rtl_text = !f.rtl_text);
                self.status =
                    ui::t!("セルの中を右横書きにしました(1字ずつ右から。昔の看板の書き方)").into();
            }
            // 表示タブ(本家のデスクトップ版に合わせる)。どれも見え方だけ
            // 画面の文字の大きさ(リボン・数式バー・メニュー・状態行まで全部)。
            // 格子のズームとは別。設定に覚えて、次回も同じ大きさで開く
            "formula-bar" => {
                self.show_formula_bar = !self.show_formula_bar;
                self.status = if self.show_formula_bar {
                    ui::t!("数式バーを表示します").into()
                } else {
                    ui::t!("数式バーを隠しました(表示タブで戻せます)").into()
                };
            }
            "show-headings" => {
                self.show_headers = !self.show_headers;
                self.status = if self.show_headers {
                    ui::t!("見出しを表示します").into()
                } else {
                    ui::t!("見出しを隠しました(列幅のドラッグ等は見出しと一緒に戻ります)").into()
                };
            }
            "show-zeros" => {
                self.show_zeros = !self.show_zeros;
                self.status = if self.show_zeros {
                    ui::t!("0 を表示します").into()
                } else {
                    ui::t!("0 を隠しました(見え方だけ — 値は 0 のまま)").into()
                };
            }
            // 小計(Excel の集計)。本家のデータタブに無いボタンだが、グループ化を
            // 「畳むと合計が残る」形で使うために要る(発注者指摘 2026-08-04)
            "subtotal" => {
                self.commit();
                if self.anchor.is_none() {
                    self.status = ui::t!("表を範囲で選んでください(1行目が見出し)").into();
                } else {
                    let (a, b) = self.sel_rect();
                    if b.row <= a.row {
                        self.status = ui::t!("見出しの下にデータの行が要ります").into();
                    } else {
                        let headers: Vec<String> = (a.col..=b.col)
                            .map(|c| {
                                let v = self
                                    .sheet()
                                    .get(Pos::new(a.row, c))
                                    .map(|x| x.value.display())
                                    .unwrap_or_default();
                                if v.is_empty() { col_name(c) } else { v }
                            })
                            .collect();
                        self.status = ui::tf!(
                            "何の区切りで集めるか(見出しを1つ): {}",
                            headers.join(" / ")
                        )
                        .into();
                        self.sub_pend = Some(PivotPend {
                            val_sel: String::new(),
                            replace: None,
                            a,
                            b,
                            headers,
                            rows_sel: Vec::new(),
                            cols_sel: Vec::new(),
                        });
                        self.prompt = Some(("subtotal-by", Editor::new("")));
                    }
                }
            }
            // グループ化(アウトライン)。行か列かは選択の形で決める:
            // 見出しから列をまるごと選んでいれば列、それ以外は選択の行。
            // 深さは xlsx の outlineLevel と往復し、畳みも保存に残る
            "group" | "ungroup" => {
                self.commit();
                if self.anchor.is_none() {
                    self.status =
                        ui::t!("まとめたい行(または列)を選んでください(見出しの番号を撫でる)").into();
                } else {
                    let (a, b) = self.sel_rect();
                    let (rows_ext, cols_ext) = self.sheet().extent();
                    let whole_rows = a.row == 0 && b.row + 1 >= rows_ext.max(1);
                    let on_cols = whole_rows && !(a.col == 0 && b.col + 1 >= cols_ext.max(1));
                    self.checkpoint();
                    let add = id == "group";
                    let sh = self.sheet_mut();
                    if on_cols {
                        for c in a.col..=b.col {
                            let l = sh.col_outline.get(&c).copied().unwrap_or(0);
                            let nl = if add { (l + 1).min(7) } else { l.saturating_sub(1) };
                            if nl == 0 {
                                sh.col_outline.remove(&c);
                                sh.col_hidden.remove(&c);
                            } else {
                                sh.col_outline.insert(c, nl);
                            }
                        }
                    } else {
                        for r in a.row..=b.row {
                            let l = sh.row_outline.get(&r).copied().unwrap_or(0);
                            let nl = if add { (l + 1).min(7) } else { l.saturating_sub(1) };
                            if nl == 0 {
                                sh.row_outline.remove(&r);
                                sh.row_hidden.remove(&r);
                            } else {
                                sh.row_outline.insert(r, nl);
                            }
                        }
                    }
                    self.dirty = true;
                    let what = if on_cols {
                        format!("{}〜{}列", col_name(a.col), col_name(b.col))
                    } else {
                        format!("{}〜{}行", a.row + 1, b.row + 1)
                    };
                    self.status = if add {
                        format!(
                            "{what}をグループ化しました(深さ+1。「詳細の非表示」で畳めます。Ctrl+Z で戻せます)"
                        )
                        .into()
                    } else {
                        format!("{what}のグループ化を1段解きました(Ctrl+Z で戻せます)").into()
                    };
                }
            }
            // 詳細の非表示=グループ化した行(列)を畳む / 詳細の表示=開く。
            // 対象は選択、無ければカーソルの行が属するグループのひとつながり
            "hide-details" | "show-details" => {
                self.commit();
                let hide = id == "hide-details";
                let (a, b) = self.sel_rect();
                let (rows_ext, cols_ext) = self.sheet().extent();
                let whole_rows =
                    self.anchor.is_some() && a.row == 0 && b.row + 1 >= rows_ext.max(1);
                let on_cols = whole_rows && !(a.col == 0 && b.col + 1 >= cols_ext.max(1));
                if on_cols {
                    let sh = self.sheet();
                    let targets: Vec<u32> = (a.col..=b.col)
                        .filter(|c| sh.col_outline.contains_key(c))
                        .collect();
                    if targets.is_empty() {
                        self.status =
                            ui::t!("選択にグループ化した列がありません(先にグループ化)").into();
                    } else {
                        self.checkpoint();
                        let sh = self.sheet_mut();
                        for c in &targets {
                            if hide {
                                sh.col_hidden.insert(*c);
                            } else {
                                sh.col_hidden.remove(c);
                            }
                        }
                        self.dirty = true;
                        self.status = ui::tf!(
                            "{} 列を{}(Ctrl+Z で戻せます)",
                            targets.len(),
                            if hide { ui::t!("畳みました") } else { ui::t!("開きました") }
                        )
                        .into();
                    }
                } else {
                    // 行: 選択、または カーソルの行が属するグループのひとつながり
                    let (r0, r1) = if self.anchor.is_some() {
                        (a.row, b.row)
                    } else {
                        let sh = self.sheet();
                        let at = self.cursor.row;
                        if !sh.row_outline.contains_key(&at) {
                            self.status = ui::t!("グループ化した行の上で押してください(先に データ > グループ化)").into();
                            cx.notify();
                            return;
                        }
                        let mut lo = at;
                        while lo > 0 && sh.row_outline.contains_key(&(lo - 1)) {
                            lo -= 1;
                        }
                        let mut hi = at;
                        while sh.row_outline.contains_key(&(hi + 1)) {
                            hi += 1;
                        }
                        (lo, hi)
                    };
                    let sh = self.sheet();
                    let targets: Vec<u32> =
                        (r0..=r1).filter(|r| sh.row_outline.contains_key(r)).collect();
                    if targets.is_empty() {
                        self.status =
                            ui::t!("選択にグループ化した行がありません(先に データ > グループ化)").into();
                    } else {
                        self.checkpoint();
                        let sh = self.sheet_mut();
                        for r in &targets {
                            if hide {
                                sh.row_hidden.insert(*r);
                            } else {
                                sh.row_hidden.remove(r);
                            }
                        }
                        self.dirty = true;
                        self.status = ui::tf!(
                            "{} 行を{}(Ctrl+Z で戻せます)",
                            targets.len(),
                            if hide { ui::t!("畳みました") } else { ui::t!("開きました") }
                        )
                        .into();
                    }
                }
            }
            // ピボットの手入れ: どれも「指図を直して置き直す」だけ。
            // 対象はカーソルの下のピボット(指図はブックに控えてある)
            // **ピボットの元の表を差し替える**(2026-08-21 の D群)。
            //
            // カーソルをピボットに置いて押すと、いまの範囲が入った欄が出ます。
            // 直して Enter で差し替え、そのまま作り直します。
            // *Excel の「データソースの変更」と同じ入り口です。*
            "pivot-source" => {
                self.commit();
                match self.pivot_at(self.cursor) {
                    Some(i) => {
                        let d = &self.book.pivots[i];
                        // シートの名前も添える(別のシートの表も指せます)
                        let 今 = format!("{}!{}:{}", d.sheet, d.src.0.a1(), d.src.1.a1());
                        self.prompt = Some(("pivot-src", Editor::new(&今)));
                        self.status = ui::t!(
                            "元の表の範囲を打って Enter(例: Sheet1!A1:C20。見出しの行を含めます)"
                        )
                        .into();
                    }
                    None => {
                        self.status =
                            ui::t!("差し替えたいピボットの上にカーソルを置いてください").into();
                    }
                }
            }
            "pivot-refresh" => {
                self.commit();
                match self.pivot_at(self.cursor) {
                    Some(i) => {
                        let d = self.book.pivots[i].clone();
                        self.spawn_pivot(d, Some(i), cx);
                    }
                    None => {
                        self.status =
                            ui::t!("更新したいピボットの上にカーソルを置いてください").into();
                    }
                }
            }
            "pivot-refresh-all" => {
                self.commit();
                let n = self.book.pivots.len();
                if n == 0 {
                    self.status = ui::t!("このブックにピボットはありません").into();
                } else {
                    for i in 0..n {
                        let d = self.book.pivots[i].clone();
                        self.spawn_pivot(d, Some(i), cx);
                    }
                    self.status = ui::tf!("{} 件のピボットを更新しています…", n).into();
                }
            }
            // フィールドリスト: いまの指図を ✓ 入りで4段に読み込み、
            // 集計まで選んだら同じ場所に置き直す(作り直しではなく組み替え)
            "pivot-fields" => {
                self.commit();
                match self.pivot_at(self.cursor) {
                    None => {
                        self.status = ui::t!("ピボットの上にカーソルを置いてください").into();
                    }
                    Some(i) => {
                        let d = self.book.pivots[i].clone();
                        let (a, b) = d.src;
                        let headers: Vec<String> = (a.col..=b.col)
                            .map(|c| {
                                let v = self
                                    .sheet()
                                    .get(Pos::new(a.row, c))
                                    .map(|x| x.value.display())
                                    .unwrap_or_default();
                                if v.is_empty() { col_name(c) } else { v }
                            })
                            .collect();
                        self.pivot_pend = Some(PivotPend {
                            a,
                            b,
                            headers,
                            rows_sel: d.rows_sel.clone(),
                            cols_sel: d.cols_sel.clone(),
                            val_sel: d.value.clone(),
                            replace: Some(i),
                        });
                        self.pivot_pick("pivot-rows-pick");
                    }
                }
            }
            // スタイルギャラリー(見出し行の色の組)。一覧から選んで掛け直す
            // 計算の種類(そのまま / 総計に対する比率 / 累計 / 前との差)
            // 変更履歴(校閲の記録)。記録中なら止めて差分を刻み、
            // そうでなければ記録を始める(刻んだ分の一覧は止めた後にもう一度)
            "track-changes" => {
                self.commit();
                if self.track_from.is_some() || self.book.changes.is_empty() {
                    self.track_changes();
                } else {
                    self.show_changes();
                }
            }
            // データテーブル(感度表)。入力セルを2段で聞く
            "datatable" => {
                self.commit();
                self.prompt = Some(("dt-col", Editor::new("")));
            }
            "pivot-showas" => {
                self.commit();
                match self.pivot_at(self.cursor) {
                    None => {
                        self.status = ui::t!("ピボットの上にカーソルを置いてください").into();
                    }
                    Some(i) => {
                        let cur = self.book.pivots[i].show_as.clone();
                        let at = self.pop_anchor();
                        let items: Vec<(String, String)> = [
                            ui::item!("そのまま"),
                            ui::item!("比率"),
                            ui::item!("累計"),
                            ui::item!("差"),
                        ]
                        .iter()
                        .map(|(k, l)| {
                            let key = if *k == "そのまま" { "" } else { *k };
                            // ✓ は見出しにだけ(鍵はそのまま照合に使う)
                            (
                                k.to_string(),
                                if key == cur { format!("✓ {l}") } else { l.to_string() },
                            )
                        })
                        .collect();
                        self.pick_note = Some(
                            ui::t!("計算の種類(比率=総計を100%とする。累計と差は小計・総計を出しません)")
                                .into(),
                        );
                        self.pick_kind = "pivot-showas-pick";
                        self.pick = Some((items, at));
                    }
                }
            }
            "pivot-style" => {
                self.commit();
                match self.pivot_at(self.cursor) {
                    None => {
                        self.status = ui::t!("ピボットの上にカーソルを置いてください").into();
                    }
                    Some(i) => {
                        let cur = self.book.pivots[i].style.clone();
                        let at = self.pop_anchor();
                        let items: Vec<(String, String)> = [
                            ui::item!("青(既定)"),
                            ui::item!("緑"),
                            ui::item!("橙"),
                            ui::item!("灰"),
                        ]
                        .iter()
                        .map(|(k, l)| {
                            let key = if *k == "青(既定)" { "" } else { *k };
                            (
                                k.to_string(),
                                if key == cur { format!("✓ {l}") } else { l.to_string() },
                            )
                        })
                        .collect();
                        self.pick_note =
                            Some(ui::t!("ピボットのスタイル(選ぶと掛け直します)").into());
                        self.pick_kind = "pivot-style-pick";
                        self.pick = Some((items, at));
                    }
                }
            }
            // 印刷のヘッダー/フッター(&P=頁 &N=総頁。紙と PDF に出る)
            "edit-header" | "editheader" => {
                self.commit();
                let at = self.pop_anchor();
                let (hl, hc, hr) =
                    sheet::model::hf_split(self.sheet().header.as_deref().unwrap_or(""));
                let (fl, fc, fr) =
                    sheet::model::hf_split(self.sheet().footer.as_deref().unwrap_or(""));
                // 鍵は欄の名前だけ(picks.rs は ':' の前で切って引き当てる)。
                // 中身は**その人が打った字**なので、見出しの側にだけ添える
                let show = |(k, l): (&'static str, &'static str), v: &str| {
                    (k.to_string(), if v.is_empty() { l.to_string() } else { format!("{l}: {v}") })
                };
                let mut items: Vec<(String, String)> = vec![
                    show(ui::item!("ヘッダー左"), &hl),
                    show(ui::item!("ヘッダー中"), &hc),
                    show(ui::item!("ヘッダー右"), &hr),
                    show(ui::item!("フッター左"), &fl),
                    show(ui::item!("フッター中"), &fc),
                    show(ui::item!("フッター右"), &fr),
                ];
                items.extend(menu(&[ui::item!("全部消す")]));
                self.pick_note = Some(
                    ui::t!("ヘッダー/フッター — 印刷と PDF に出ます(&P=頁 &N=総頁)").into(),
                );
                self.pick_kind = "hf-pick";
                self.pick = Some((items, at));
            }
            "pivot-select" => {
                match self.pivot_at(self.cursor) {
                    Some(i) => {
                        let d = &self.book.pivots[i];
                        self.cursor = d.dest;
                        self.anchor = Some(Pos::new(
                            d.dest.row + d.size.0.saturating_sub(1),
                            d.dest.col + d.size.1.saturating_sub(1),
                        ));
                        self.sync_input();
                        self.status = ui::t!("ピボット全体を選びました").into();
                    }
                    None => {
                        self.status = ui::t!("ピボットの上にカーソルを置いてください").into();
                    }
                }
            }
            "pivot-totals" | "pivot-subtotals" | "pivot-blank" | "pivot-layout" => {
                self.commit();
                match self.pivot_at(self.cursor) {
                    None => {
                        self.status = ui::t!("ピボットの上にカーソルを置いてください").into();
                    }
                    Some(i) => {
                        // レイアウト(コンパクト⇔表形式)も行の見出しが1つだと
                        // 見た目が変わらない — 黙って置き直さず、正直に言う
                        let need_two =
                            matches!(id, "pivot-subtotals" | "pivot-blank" | "pivot-layout");
                        if need_two && self.book.pivots[i].rows_sel.len() < 2 {
                            self.status =
                                ui::t!("行の見出しが2つ以上のピボットで効きます(挿入で複数選ぶ)").into();
                        } else {
                            let d = &mut self.book.pivots[i];
                            let (name, on) = match id {
                                "pivot-totals" => {
                                    d.totals = !d.totals;
                                    (ui::t!("総計"), d.totals)
                                }
                                "pivot-subtotals" => {
                                    d.subtotals = !d.subtotals;
                                    (ui::t!("小計"), d.subtotals)
                                }
                                "pivot-blank" => {
                                    d.blank_rows = !d.blank_rows;
                                    (ui::t!("空行"), d.blank_rows)
                                }
                                _ => {
                                    d.compact = !d.compact;
                                    (ui::t!("コンパクト形式"), d.compact)
                                }
                            };
                            let d = self.book.pivots[i].clone();
                            self.dirty = true;
                            self.status = ui::tf!(
                                "{}を{}にして置き直します…",
                                name,
                                if on { ui::t!("あり") } else { ui::t!("なし") }
                            )
                            .into();
                            self.spawn_pivot(d, Some(i), cx);
                        }
                    }
                }
            }
            // 表のデザイン: 表オブジェクトは持たない。選択に**1手ずつ掛ける道具**
            // (掛けた書式・式が帳面に残るだけ。切り替え式に見せない。
            // まとめて掛けるなら挿入タブの「表の挿入」)
            "td-header" | "td-band-row" | "td-band-col" | "td-first" | "td-last" => {
                // **実装は sheet::tabledesign**(2026-08-16 に移した)。画面と
                // Python の口が同じ所を呼ぶので、記録した行はそのまま走る
                let what = match id {
                    "td-header" => sheet::tabledesign::Deco::Header,
                    "td-band-row" => sheet::tabledesign::Deco::BandRow,
                    "td-band-col" => sheet::tabledesign::Deco::BandCol,
                    "td-first" => sheet::tabledesign::Deco::FirstCol,
                    _ => sheet::tabledesign::Deco::LastCol,
                };
                self.commit();
                if self.anchor.is_none() {
                    self.status = ui::t!("表の範囲を選んでください").into();
                } else {
                    self.checkpoint();
                    let (a, b) = self.sel_rect();
                    let on = self.td_next(what);
                    sheet::tabledesign::deco(&mut self.book.sheets[self.active], a, b, what, on);
                    self.dirty = true;
                    // 文に差し込む字も画面の文言 — 訳さないと日本語だけ混じる
                    let what_ja = match id {
                        "td-header" => ui::t!("1行目を見出し行に"),
                        "td-band-row" => ui::t!("1行おきの縞々に"),
                        "td-band-col" => ui::t!("1列おきの縞々に"),
                        "td-first" => ui::t!("最初の列を太字に"),
                        _ => ui::t!("最後の列を太字に"),
                    };
                    self.status = if on {
                        ui::tf!("{}:{} を{}しました(Ctrl+Z で戻せます)", a.a1(), b.a1(), what_ja)
                    } else {
                        // 塗りは剥がさない(掛ける前の姿を覚えていない)。
                        // **できないことを、できるように見せない**
                        ui::tf!("表の性質から{}を外しました(掛かっている色は「書式のクリア」で消せます)", what_ja)
                    }
                    .into();
                }
            }
            // 合計行 = 選択の下に =SUM(…) の行を足す(式なので元が変われば追従)
            "td-total" => {
                self.commit();
                if self.anchor.is_none() {
                    self.status = ui::t!("合計したい表の範囲を選んでください").into();
                } else {
                    let (a, b) = self.sel_rect();
                    if sheet::tabledesign::below_used(self.sheet(), a, b) {
                        self.status =
                            ui::t!("すぐ下の行に中身があります(空けてから — 黙って上書きしません)").into();
                    } else {
                        self.checkpoint();
                        sheet::tabledesign::add_total_row(&mut self.book.sheets[self.active], a, b);
                        recalc_book(&mut self.book, self.active);
                        self.dirty = true;
                        self.status = ui::tf!(
                            "{} 行目に合計(=SUM)を足しました。式なので元が変われば追従します(Ctrl+Z で戻せます)",
                            (b.row + 2).to_string()
                        )
                        .into();
                    }
                }
            }
            // フィルタのボタン = データタブの絞り込みと同じ実体
            "td-filter" => self.run_cmd("setfilter", cx),
            // 表の挿入 = 選択に表の書式(見出し行の色+縞模様+外枠)を掛ける
            // 表にする。`instable` は既定の色ですぐ、`table-tpl` は
            // **色を選んでから**(2026-08-12、台帳「テンプレート選択ギャラリー」)
            "instable" => {
                self.commit();
                let st = crate::util::table_styles()[0].2;
                self.make_table(st, None);
            }
            "table-tpl" => {
                self.commit();
                if self.anchor.is_none() {
                    self.status = ui::t!("表にする範囲を選んでください").into();
                } else {
                    let at = self.pop_anchor();
                    self.pick_kind = "table-style";
                    self.pick = Some((
                        crate::util::table_styles()
                            .iter()
                            .map(|(k, l, _)| (k.to_string(), l.to_string()))
                            .collect(),
                        at,
                    ));
                    self.status = ui::t!("表のスタイル: 選ぶと表になります(Ctrl+Z で戻せます)").into();
                }
            }
            // 記号を挿入: 一覧から選んで**数式バーへ**差し込む(セルは置き換えない)
            // 記号。**分類で選んでから字を選ぶ**(平らに並べると探せない)。
            // 最近使った分は先頭に出す。無い字は 16 進で打てる
            "inssymbol" => {
                let at = self.pop_anchor();
                // **鍵はどの組かだけ** — 字の並びは picks.rs が組から引き直す
                let mut items: Vec<(String, String)> = Vec::new();
                if !self.recent_symbols.is_empty() {
                    let chars = self.recent_symbols.join(" ");
                    items.push(("symbols:recent".into(), ui::tf!("最近使った: {}", chars)));
                }
                for (key, label, chars) in symbol_groups() {
                    items.push((format!("symbols:{key}"), format!("{label}: {chars}")));
                }
                items.extend(menu(&[ui::item!("Unicode を打つ(例: 3012 → 〒)…")]));
                self.pick_kind = "symbol-group";
                self.pick_note = Some(ui::t!("記号(組を選ぶと一字ずつ出ます)").into());
                self.pick = Some((items, at));
            }
            // 文章と同じ id。表は挿入タブと共同編集タブの両方から呼びます
            "co-addcomment" | "addcomment" => {
                self.commit();
                let cur = self
                    .sheet()
                    .comments
                    .get(&self.cursor)
                    .map(|t| t.text().to_string())
                    .unwrap_or_default();
                self.prompt = Some(("comment", Editor::new(&cur)));
            }
            "text-column" => {
                self.commit();
                if self.anchor.is_none() {
                    self.status =
                        ui::t!("割りたいセルを選んでください(選択した列の文字を右へ割ります)").into();
                } else {
                    self.prompt = Some(("split-delim", Editor::new("")));
                }
            }
            "goal-seek" => {
                self.commit();
                // 目標セルの初期値はいまのセル(式のセルの上で押すのが自然)
                let init = if self.sheet().get(self.cursor).and_then(|c| c.formula.as_ref()).is_some()
                {
                    format!("{}=", self.cursor.a1())
                } else {
                    String::new()
                };
                self.goal = None;
                self.prompt = Some(("goal-target", Editor::new(&init)));
            }
            "data-external-links" => {
                // 他のブックを**値として**取り込む(リンクは張らない —
                // リンク切れの帳票を作らない。SEKKEI の分業どおり)
                self.commit();
                let ask = cx.background_executor().spawn(async {
                    let p = rfd::FileDialog::new()
                        .add_filter("Excelブック", &["xlsx"])
                        .pick_file()?;
                    Some(
                        std::fs::File::open(&p)
                            .map_err(|e| e.to_string())
                            .and_then(sheet::xlsx::read)
                            .map(|(b, _)| (p, b)),
                    )
                });
                cx.spawn(async move |this, cx| {
                    let r = ask.await;
                    let _ = this.update(cx, |this, cx| {
                        match r {
                            None => {}
                            Some(Ok((p, mut other))) => {
                                this.checkpoint();
                                sheet::recalc_all(&mut other);
                                let mut n = 0usize;
                                for mut sh in other.sheets.drain(..) {
                                    // 式は計算結果の値に(他所の参照を持ち込まない)
                                    for c in sh.cells.values_mut() {
                                        c.formula = None;
                                    }
                                    sh.name = format!(
                                        "{}({})",
                                        sh.name,
                                        p.file_stem().unwrap_or_default().to_string_lossy()
                                    );
                                    while this.book.sheets.iter().any(|x| x.name == sh.name) {
                                        sh.name.push('+');
                                    }
                                    this.book.sheets.push(sh);
                                    n += 1;
                                }
                                this.dirty = true;
                                this.status = ui::tf!(
                                    "{} シートを値として取り込みました(リンクは張りません)",
                                    n
                                )
                                .into();
                            }
                            Some(Err(e)) => this.status = ui::tf!("取り込めません: {}", e).into(),
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            // 拡大縮小印刷: 100→90→80→70→50→100
            "scale" => {
                self.commit();
                self.checkpoint();
                let sh = self.sheet_mut();
                let next = match sh.print_scale.unwrap_or(100) {
                    100 => 90,
                    90 => 80,
                    80 => 70,
                    70 => 50,
                    _ => 100,
                };
                sh.print_scale = if next == 100 { None } else { Some(next) };
                self.dirty = true;
                self.status = ui::tf!("拡大縮小印刷: {}%(PDF と保存に効きます)", next).into();
            }
            // 改ページ: いまの行から新しい紙を始める(もう一度で解除)
            // 改ページ。本家は「挿入 / 解除 / すべてリセット」の3択なので
            // 一覧で選ばせる。**縦(列の区切り)も入れられる**
            "pagebreak" => {
                let at = self.pop_anchor();
                let sh = self.sheet();
                let (r, c) = (self.cursor.row, self.cursor.col);
                let has_row = sh.row_breaks.contains(&r);
                let has_col = sh.col_breaks.contains(&c);
                // **できないことは並べない。** 1行目・A列の「前」には紙の
                // 切れ目を置けない(そこが既に紙の頭)ので項目ごと出さない
                // **鍵は短い合図**(文ではない)。picks.rs はこの合図で引き当てる —
                // 同じ日本語の文を鍵と見出しに二度書けば、いつかずれる
                let mut items: Vec<(String, String)> = Vec::new();
                if has_row {
                    items.push((
                        "pagebreak:row".into(),
                        ui::tf!("この行({})の改ページを外す", r + 1),
                    ));
                } else if r > 0 {
                    items.push((
                        "pagebreak:row".into(),
                        ui::tf!("{} 行から新しい紙にする(横の区切り)", r + 1),
                    ));
                }
                if has_col {
                    items.push((
                        "pagebreak:col".into(),
                        ui::tf!("この列({})の改ページを外す", col_name(c)),
                    ));
                } else if c > 0 {
                    items.push((
                        "pagebreak:col".into(),
                        ui::tf!("{} 列から新しい紙にする(縦の区切り)", col_name(c)),
                    ));
                }
                let n = sh.row_breaks.len() + sh.col_breaks.len();
                if n > 0 {
                    items.push((
                        "pagebreak:all".into(),
                        ui::tf!("すべての改ページを外す({} 個)", n),
                    ));
                }
                if items.is_empty() {
                    // A1 の上で、まだ1つも改ページが無いとき
                    self.status = ui::t!(
                        "改ページは紙の切れ目です。切りたい行(または列)にカーソルを置いてから押してください"
                    )
                    .into();
                } else {
                    self.pick_kind = "pagebreak";
                    self.pick_note = Some(ui::t!("改ページ(紙の切れ目)").into());
                    self.pick = Some((items, at));
                }
            }
            // 印刷範囲を**足す**(本家の「印刷範囲に追加」)。
            // 域はそれぞれ別の紙に刷る
            "printarea-add" => {
                self.commit();
                if self.anchor.is_none() {
                    self.status =
                        ui::t!("足す範囲を Shift+矢印かドラッグで選んでください").into();
                } else {
                    self.checkpoint();
                    let range = self.sel_rect();
                    let sh = self.sheet_mut();
                    if sh.print_areas.contains(&range) {
                        self.undo_stack.pop();
                        self.status = ui::t!("その範囲はもう印刷範囲に入っています").into();
                    } else {
                        sh.print_areas.push(range);
                        let n = sh.print_areas.len();
                        self.dirty = true;
                        self.status = ui::tf!(
                            "印刷範囲に {}:{} を足しました(全 {} 域。域ごとに別の紙に刷ります)",
                            range.0.a1(),
                            range.1.a1(),
                            n
                        )
                        .into();
                    }
                }
            }
            // 読み取り専用を勧める / 最終版の札。**どちらも鍵ではない** —
            // 掛けた振りをしないのがこちらの作法(SEKKEI)
            "read-only-rec" => {
                self.commit();
                self.checkpoint();
                self.book.read_only_rec = !self.book.read_only_rec;
                self.dirty = true;
                self.status = if self.book.read_only_rec {
                    ui::t!("開いた人に「見るだけ」を勧めます(鍵ではありません — 直せます)").into()
                } else {
                    ui::t!("読み取り専用の勧めをやめました").into()
                };
            }
            // フラッシュフィル(本家 Ctrl+E)。**見本から作り方を推し量って
            // 下を埋める。** 推し量りを外したら黙って埋めず、そう言う
            "flash-fill" => {
                self.commit();
                let (a, b) = self.sel_rect();
                let col = a.col;
                if col == 0 {
                    self.status =
                        ui::t!("左に元の列が要ります(A 列では推し量れません)").into();
                    cx.notify();
                    return;
                }
                let (rows, _) = self.sheet().extent();
                let last = if b.row > a.row { b.row } else { rows.saturating_sub(1) };
                // 見本 = この列に既に入っている行。元 = その左ぜんぶ
                let src_of = |this: &Self, r: u32| -> Vec<String> {
                    (0..col)
                        .map(|c| {
                            this.sheet().get(Pos::new(r, c)).map(|x| x.value.display())
                                .unwrap_or_default()
                        })
                        .collect()
                };
                // **見本はカーソルの行から続いている分だけ。** 下の方に
                // 関係のない値が入っていても、それを見本と取り違えない
                let mut examples: Vec<(Vec<String>, String)> = Vec::new();
                for r in a.row..=last {
                    let v = self.sheet().get(Pos::new(r, col)).map(|x| x.value.display())
                        .unwrap_or_default();
                    if v.is_empty() {
                        break;
                    }
                    examples.push((src_of(self, r), v));
                }
                if examples.is_empty() {
                    self.status = ui::t!(
                        "先に1つ書いてください(その書き方を見て下を埋めます)"
                    )
                    .into();
                    cx.notify();
                    return;
                }
                let Some(recipe) = flash_recipe(&examples) else {
                    // **当てずっぽうで埋めない**
                    self.status = ui::t!(
                        "書き方が読み取れませんでした(もう1つ見本を書くと当たりやすくなります)"
                    )
                    .into();
                    cx.notify();
                    return;
                };
                self.checkpoint();
                let mut n = 0usize;
                for r in a.row..=last {
                    let p = Pos::new(r, col);
                    let now = self.sheet().get(p).map(|x| x.value.display()).unwrap_or_default();
                    if !now.is_empty() {
                        continue; // 既に書いてある所は触らない
                    }
                    let src = src_of(self, r);
                    if src.iter().all(|s| s.is_empty()) {
                        continue;
                    }
                    let Some(v) = flash_apply(&recipe, &src) else { continue };
                    self.book.sheets[self.active].set(p, sheet::Cell::input(&v));
                    n += 1;
                }
                if n == 0 {
                    self.undo_stack.pop();
                    self.status = ui::t!("埋める所がありませんでした").into();
                } else {
                    self.dirty = true;
                    recalc_book(&mut self.book, self.active);
                    self.status = ui::tf!(
                        "{} 個を書き方に合わせて埋めました(Ctrl+Z で戻せます — 中身は必ず見てください)",
                        n
                    )
                    .into();
                }
            }
            // 名前の貼り付け(本家の「数式で使用」)。**式を書いている途中に
            // 名前を差し込む** — 覚えていなくても使える
            "paste-name" => {
                let at = self.pop_anchor();
                // 鍵は「何を指しているか」だけ(`name:` / `table:` + 名前)。
                // 名前と範囲は**中身**なので見出しにそのまま出す(訳さない)
                let mut items: Vec<(String, String)> = self
                    .sheet()
                    .names
                    .iter()
                    .map(|d| {
                        // 適用範囲も見せる — 同じ名前が2つ並ぶことがある
                        let scope = if d.scoped { " (このシート)" } else { "" };
                        (format!("name:{}", d.name), format!("{} = {}{scope}", d.name, d.range))
                    })
                    .collect();
                for t in &self.sheet().tables {
                    items.push((
                        format!("table:{}", t.name),
                        ui::tf!("{} = {}:{}(テーブル)", t.name, t.a.a1(), t.b.a1()),
                    ));
                }
                if items.is_empty() {
                    self.status = ui::t!(
                        "名前がまだありません(数式タブの「名前の管理」で、選んだ範囲に付けられます)"
                    )
                    .into();
                } else {
                    self.pick_kind = "paste-name";
                    self.pick_note =
                        Some(ui::t!("式に差し込む名前(いま打っている所に入ります)").into());
                    self.pick = Some((items, at));
                }
            }
            // **中身に合わせる**(本家の「列の幅の自動調整」「行の高さの
            // 自動調整」)。見出しの境界を両押しでも同じことが起きる
            "autofit-col" | "autofit-row" => {
                self.commit();
                let col = id == "autofit-col";
                let (a, b) = self.sel_rect();
                self.checkpoint();
                let n = self.autofit_at(a, b, col);
                self.dirty = true;
                self.status = if n == 0 {
                    ui::t!("中身が無いので合わせようがありません").into()
                } else if col {
                    ui::tf!("{} 列の幅を中身に合わせました(Ctrl+Z で戻せます)", n).into()
                } else {
                    ui::tf!("{} 行の高さを中身に合わせました(Ctrl+Z で戻せます)", n).into()
                };
            }
            // CSV の形(文字コードと区切り)。**日本の会計ソフトは
            // まだ CP932 のものがある** — UTF-8 固定では渡せない
            // **左右のパネル**(2026-08-15)。writer と同じ id・同じ札。
            // 開け閉めは「すぐ効く」ので印は無印(▾ も … も付けない)
            "show-left" => {
                self.left_open = !self.left_open;
                self.status = if self.left_open {
                    ui::t!("左パネル(AI と相談する)を開きました").into()
                } else {
                    ui::t!("左パネルを閉じました").into()
                };
            }
            "show-right" => {
                self.right_open = !self.right_open;
                self.status = if self.right_open {
                    ui::t!("右パネル(セルの設定)を開きました").into()
                } else {
                    ui::t!("右パネルを閉じました").into()
                };
            }
            "csv-kind" => {
                let at = self.pop_anchor();
                self.pick_kind = "csv-kind";
                self.pick_note =
                    Some(ui::t!("CSV に書き出すときの文字コードと区切り").into());
                // 引き当ては鍵、画面は見出し(io.rs の表)。✓ は見出しだけに
                self.pick = Some((
                    Self::csv_kinds()
                        .iter()
                        .map(|(k, l, _)| {
                            (
                                (*k).to_string(),
                                if *k == self.csv_kind {
                                    format!("✓ {l}")
                                } else {
                                    (*l).to_string()
                                },
                            )
                        })
                        .collect(),
                    at,
                ));
            }
            // 自動復旧: 残っている控えの一覧。選ぶとその中身を開く
            "recover" => {
                let at = self.pop_anchor();
                let list = Self::stale_recovers();
                if list.is_empty() {
                    self.status = ui::tf!(
                        "復旧する控えはありません(いまは {} 秒ごとに控えています)",
                        self.recover_secs
                    )
                    .into();
                } else {
                    self.pick_paths = list.clone();
                    self.pick_kind = "recover";
                    self.pick_note = Some(
                        ui::t!("保存できずに終わったブックの控え(選ぶと開きます)").into(),
                    );
                    // 控えのファイル名 — 訳さない
                    self.pick = Some((plain(list.into_iter().map(|(n, _)| n)), at));
                }
            }
            // 自動復旧の間隔
            "recover-every" => {
                let at = self.pop_anchor();
                self.pick_kind = "recover-every";
                self.pick_note =
                    Some(ui::t!("自動復旧の控えを取る間隔(原本は上書きしません)").into());
                self.pick = Some((
                    menu(&[
                        ui::item!("取らない"),
                        ui::item!("1分ごと"),
                        ui::item!("5分ごと"),
                        ui::item!("10分ごと"),
                    ]),
                    at,
                ));
            }
            // 紙の切れ目を画面に見せる(本家の改ページプレビューの破線)
            "show-breaks" => {
                self.show_breaks = !self.show_breaks;
                let (r, c) = self.page_breaks_now();
                self.status = if self.show_breaks {
                    if r.is_empty() && c.is_empty() {
                        ui::t!("紙の切れ目を出します(いまは1枚に収まっています)").into()
                    } else {
                        format!(
                            "紙の切れ目を出します(横 {} 本・縦 {} 本。手で入れた区切りは濃い線)",
                            r.len(),
                            c.len()
                        )
                        .into()
                    }
                } else {
                    ui::t!("紙の切れ目を消しました").into()
                };
            }
            // 紙 N 枚に収める。本家の「拡大縮小印刷」の選択肢と同じ顔ぶれ
            "fit-pages" => {
                let at = self.pop_anchor();
                self.pick_kind = "fit-pages";
                self.pick_note = Some(
                    ui::t!("紙に収める(選ぶと拡大縮小印刷の % より優先します)").into(),
                );
                self.pick = Some((
                    menu(&[
                        ui::item!("拡大縮小しない"),
                        ui::item!("すべての列を1ページに"),
                        ui::item!("すべての行を1ページに"),
                        ui::item!("シートを1ページに"),
                        ui::item!("横2ページ×縦1ページ"),
                    ]),
                    at,
                ));
            }
            // タイトルを印刷: 選んだ行を各ページの頭で繰り返す。選択なしで解除
            "printtitles" => {
                self.commit();
                if self.anchor.is_some() {
                    self.checkpoint();
                    let (a, b) = self.sel_rect();
                    self.sheet_mut().print_title_rows = Some((a.row, b.row));
                    self.dirty = true;
                    self.status = ui::tf!(
                        "{}〜{} 行を各ページの頭で繰り返します(選択なしで押すと解除)",
                        a.row + 1,
                        b.row + 1
                    )
                    .into();
                } else if self.sheet().print_title_rows.is_some() {
                    self.checkpoint();
                    self.sheet_mut().print_title_rows = None;
                    self.dirty = true;
                    self.status = ui::t!("タイトル行を解除しました").into();
                } else {
                    self.status =
                        ui::t!("繰り返す行を選んでから押してください(行の見出しをクリック)").into();
                }
            }
            "print-gridlines" => {
                self.commit();
                self.checkpoint();
                let sh = self.sheet_mut();
                sh.print_gridlines = !sh.print_gridlines;
                let on = sh.print_gridlines;
                self.dirty = true;
                self.status = ui::tf!(
                    "枠線の印刷: {}",
                    if on {
                        ui::t!("する(表の薄い線が紙に出ます)")
                    } else {
                        ui::t!("しない")
                    }
                )
                .into();
            }
            "print-headings" => {
                self.commit();
                self.checkpoint();
                let sh = self.sheet_mut();
                sh.print_headings = !sh.print_headings;
                let on = sh.print_headings;
                self.dirty = true;
                self.status = ui::tf!(
                    "見出しの印刷: {}",
                    if on {
                        ui::t!("する(行番号と列名が余白に出ます)")
                    } else {
                        ui::t!("しない")
                    }
                )
                .into();
            }
            // 検索と置換(ホーム > 置き換え)。パネルを2枚続けて使う
            "replace" => {
                self.commit();
                let init = self.find_term.clone().unwrap_or_default();
                self.prompt = Some(("find", Editor::new(&init)));
            }
            // グラフ(matplotlib)と画像。挿入タブ
            "inschart" => {
                self.commit();
                if self.anchor.is_none() {
                    self.status =
                        ui::t!("グラフにする範囲を選んでください(1列目が項目名、2列目からが数)").into();
                } else {
                    let (a, b) = self.sel_rect();
                    self.insert_chart(a, b, cx);
                }
            }
            "insimage" => {
                self.commit();
                self.insert_image_dialog(cx);
            }
            "instext" => {
                // テキストボックス = 枠の図形 + 文字。すぐ文字のパネルを開く
                self.checkpoint();
                let at = self.cursor;
                self.sheet_mut().shapes_new.push(sheet::model::SheetShape {
                    at,
                    width_px: 200.0,
                    height_px: 80.0,
                    kind: "rect".into(),
                    fill: None,
                    line: Some("7F7F7F".into()),
                    ..Default::default()
                });
                self.shape_sel = Some(self.sheet().shapes_new.len() - 1);
                self.dirty = true;
                self.prompt = Some(("shape-text", Editor::new("")));
            }
            "inssparkline" => {
                self.commit();
                if self.anchor.is_none() {
                    self.status =
                        ui::t!("スパークラインにする数の範囲を選んでください(置き場所はいまのセル)").into();
                } else {
                    // 本家と同じ3種から選ぶ(折れ線・縦棒・勝ち負け)
                    let at = self.pop_anchor();
                    self.pick_note = Some(ui::t!("スパークラインの種類").into());
                    self.pick_kind = "spark-kind-pick";
                    self.pick = Some((
                        menu(&[
                            ui::item!("折れ線"),
                            ui::item!("縦棒(カラム)"),
                            ui::item!("勝ち負け(正負)"),
                        ]),
                        at,
                    ));
                }
            }
            // 図形は**分類の段が先**(本家と同じ2段)。1段に 30 以上並べると
            // 探せない — 分類は本家の並びに合わせた
            "insshape" => {
                let at = self.pop_anchor();
                self.pick_kind = "shape-cat";
                self.pick_note = Some(ui::t!("図形の分類").into());
                self.pick = Some((
                    menu(&[
                        ui::item!("基本図形"),
                        ui::item!("ブロック矢印"),
                        ui::item!("数式図形"),
                        ui::item!("フローチャート"),
                        ui::item!("星とリボン"),
                        ui::item!("吹き出し"),
                        ui::item!("線"),
                    ]),
                    at,
                ));
            }
            "inshyperlink" => {
                self.commit();
                let cur = self.sheet().links.get(&self.cursor).cloned().unwrap_or_default();
                self.prompt = Some(("link", Editor::new(&cur)));
            }
            // データの入力規則。選んだ範囲に候補を付ける(パネルで受ける)
            // データの入力規則。本家は 設定/入力メッセージ/エラーアラートの
            // 3タブのダイアログ — calc は種類の一覧 → 聞き取りのパネルの2段
            // **入力規則に合っていない値を洗い出す**(2026-08-21 の D群)。
            //
            // 規則は*これから打つ字*を堰き止めますが、**先に入っていた値**や
            // 貼り付けで入った値は素通りします。押すと今の値を全部見て、
            // 合っていないセルを丸で囲みます。もう一度押すと消えます。
            //
            // *判定は `Validation::passes` の1本*(打つときと同じ物)。
            // ここで別の判定を書くと、「打てるのに印が付く」が起きます。
            "dv-mark" => {
                self.commit();
                if !self.dv_marks.is_empty() {
                    self.dv_marks.clear();
                    self.status = ui::t!("印を消しました").into();
                    cx.notify();
                    return;
                }
                let si = self.active;
                let sh = &self.book.sheets[si];
                if sh.validations.is_empty() {
                    self.status =
                        ui::t!("このシートに入力規則はありません(データタブの「データの入力規則」で付けます)")
                            .into();
                    cx.notify();
                    return;
                }
                let mut 見つけた: Vec<(usize, Pos)> = Vec::new();
                for v in &sh.validations {
                    let (a, b) = v.range;
                    for r in a.row..=b.row {
                        for c in a.col..=b.col {
                            let p = Pos::new(r, c);
                            let 字 = sh.get(p).map(|x| x.value.display()).unwrap_or_default();
                            // 空欄は規則の `allow_blank` に従う(打つときと同じ)
                            if 字.is_empty() {
                                if !v.allow_blank {
                                    見つけた.push((si, p));
                                }
                                continue;
                            }
                            if !v.passes(sh, &字) {
                                見つけた.push((si, p));
                            }
                        }
                    }
                }
                let n = 見つけた.len();
                self.dv_marks = 見つけた;
                self.status = if n == 0 {
                    ui::t!("入力規則に合わない値はありません").into()
                } else {
                    ui::tf!("{} 個に印を付けました(もう一度押すと消えます)", n).into()
                };
            }
            "data-validation" => {
                self.commit();
                // 本家の3タブのダイアログと同じ形のパネル(発注者 2026-08-07)
                self.dv_open();
            }
            // 条件付き書式。右クリックメニューと同じ一覧を開く(道は1本)
            "condformat" => {
                self.menu_at = Some(self.pop_anchor());
                self.menu_sub = Some("cond");
                // 親を通らずに子を開いた。Esc はまとめて閉じる
                self.menu_direct = true;
            }
            // 名前の管理。右クリックの「名前の定義」と同じパネル
            // 名前マネージャー(本家の一覧+新規/編集/削除に相当)
            "defname" => {
                self.commit();
                let at = self.pop_anchor();
                // 鍵は「何を指しているか」だけ(`name:` / `table:` + 名前)。
                // 名前と範囲は中身なので見出しにそのまま出す — 訳さない
                let mut items: Vec<(String, String)> = self
                    .sheet()
                    .names
                    .iter()
                    .map(|d| {
                        let scope = if d.scoped { " (このシート)" } else { "" };
                        (format!("name:{}", d.name), format!("{} = {}{scope}", d.name, d.range))
                    })
                    .collect();
                for t in &self.sheet().tables {
                    items.push((
                        format!("table:{}", t.name),
                        ui::tf!("{} = {}:{}(テーブル)", t.name, t.a.a1(), t.b.a1()),
                    ));
                }
                // 「→ 」は鍵の一部(picks.rs が starts_with で見る)
                items.extend(menu(&[ui::item!("→ 新しい名前(いまの選択に付ける)…")]));
                self.pick_note = Some(
                    ui::t!("名前の管理 — 名前を選ぶと 移動/打ち直し/削除。式の中で使えます").into(),
                );
                self.pick_kind = "names-pick";
                self.pick = Some((items, at));
            }
            // ピボットグラフ。**ピボットに連動する図** — ピボットを作り直す
            // たびに描き直します。図だけが古いまま残ることがありません
            "pivot-chart" => {
                self.commit();
                let Some(pi) = self.pivot_at(self.cursor) else {
                    self.status =
                        ui::t!("ピボットの上にカーソルを置いてください").into();
                    return;
                };
                let d = self.book.pivots[pi].clone();
                if let Some(at) = d.chart_at {
                    // もう1枚増やさない。**同じピボットの図は1枚**です
                    let si = self.active;
                    self.book.sheets[si].images_new.retain(|im| im.at != at);
                    self.book.pivots[pi].chart_at = None;
                    self.dirty = true;
                    self.status = ui::t!("ピボットグラフを外しました").into();
                    return;
                }
                // 置き場はピボットの下(表と重ならない所)
                let at = Pos::new(d.dest.row + d.size.0 + 1, d.dest.col);
                self.book.pivots[pi].chart_at = Some(at);
                self.dirty = true;
                self.pivot_chart_redraw(pi, cx);
                self.status = ui::tf!(
                    "ピボットグラフを {} に置きました(ピボットを更新すると図も描き直します。同じボタンで外せます)",
                    at.a1()
                )
                .into();
            }
            // 予測シート。指数平滑で先を出し、新しいシートに置く
            "forecast" => {
                self.commit();
                self.prompt = Some(("forecast-h", Editor::new("6")));
                self.status =
                    ui::t!("何期先まで予測しますか(数を打って Enter。空 Enter = 6 期)").into();
            }
            // 最終版の札。**鍵ではありません** — 読み取り専用の勧めと同じ扱いで、
            // 開いた人に「もう直さないでください」と伝えるだけです
            "final-mark" => {
                self.commit();
                self.toggle_final_mark();
            }
            // シナリオ(入力セルの組に名前を付けて、切り替えて比べる)。
            // **値を書き戻すだけ**で、式も書式も触りません。1手で戻せます
            "scenario" => {
                self.commit();
                let at = self.pop_anchor();
                let mut items: Vec<(String, String)> = self
                    .sheet()
                    .scenarios
                    .iter()
                    .map(|s| {
                        let n = s.cells.len();
                        (s.name.clone(), ui::tf!("{}({} セル)", s.name.clone(), n).to_string())
                    })
                    .collect();
                items.extend(menu(&[ui::item!("→ 新しいシナリオ(選んだセルのいまの値)…")]));
                if !self.sheet().scenarios.is_empty() {
                    items.extend(menu(&[ui::item!("→ シナリオを削除…")]));
                }
                self.pick_note = Some(if self.sheet().scenarios.is_empty() {
                    ui::t!("まだ1つもありません。変えて比べたいセルを選んでから作ってください").into()
                } else {
                    ui::t!("押すとその値をセルに書き戻します(Ctrl+Z で戻せます)").into()
                });
                self.pick_kind = "scenario";
                self.pick = Some((items, at));
            }
            // ウィンドウの分割。固定と似ていますが、**上と左の帯も動きます** —
            // 離れた2か所を並べて見比べるための操作です。カーソルの左上で
            // 分かれるのは Excel と同じ。もう一度押すと元に戻ります
            "split" => {
                if self.split.is_some() {
                    self.split = None;
                    self.status = ui::t!("分割をやめました").into();
                } else {
                    let r = self.cursor.row.saturating_sub(self.view.row);
                    let c = self.cursor.col.saturating_sub(self.view.col);
                    if r == 0 && c == 0 {
                        // 画面の左上そのものでは、分ける場所がありません
                        self.status = ui::t!(
                            "分ける場所にカーソルを置いてください(カーソルの上と左で分かれます)"
                        )
                        .into();
                    } else {
                        // 固定と分割は同時に立てません(帯が二重になります)
                        self.frozen = None;
                        self.split = Some(Pos::new(r, c));
                        self.split_view = self.view;
                        self.view = self.cursor;
                        self.status = ui::t!(
                            "分割しました(下と右はホイールで、上と左はその上でホイールを回すと動きます)"
                        )
                        .into();
                    }
                }
            }
            // ウィンドウ枠の固定。本家はドロップダウンで「最上行」「最初の列」を
            // 個別に選べる — トグルだけの形をやめ、一覧から選ぶ
            "freeze" => {
                let at = self.pop_anchor();
                let mut items: Vec<(String, String)> = Vec::new();
                if self.frozen.is_some() {
                    items.extend(menu(&[ui::item!("固定の解除")]));
                }
                items.extend(menu(&[
                    ui::item!("いまの位置で固定(上と左が留まる)"),
                    ui::item!("最上行の固定"),
                    ui::item!("最初の列の固定"),
                ]));
                // 本家の「固定された枠に影を付ける」(viewtab:freezeshadow)。
                // **✓ は見出しだけ** — 鍵は素のまま(picks.rs がこの字で引き当てる)
                let (k, l) = ui::item!("固定した枠に影を付ける");
                items.push((
                    k.to_string(),
                    if self.freeze_shadow { format!("✓ {l}") } else { l.to_string() },
                ));
                self.pick_kind = "freeze";
                self.pick = Some((items, at));
            }
            // 塗りつぶしの色。本家はパレット — 一覧から選ぶ
            // (順繰りの2色は仮実装だった。発注者指摘 2026-08-06)
            "fillparag" => {
                let at = self.pop_anchor();
                self.pick_kind = "fill-color";
                let mut items: Vec<(String, String)> =
                    fill_colors().iter().map(|(k, l, _)| (k.to_string(), l.to_string())).collect();
                items.extend(menu(&[
                    ui::item!("その他(RRGGBB を打つ)…"),
                    // 単色以外(台帳 第2便の [中])。**色を選ぶとべた塗りに戻る** —
                    // 柄と虹はここから掛け直す
                    ui::item!("柄を掛ける…"),
                    ui::item!("グラデーション…"),
                    ui::item!("柄の地の色…"),
                ]));
                self.pick = Some((items, at));
            }
            // フォントの色。同じくパレット
            "fontcolor" => {
                let at = self.pop_anchor();
                self.pick_kind = "font-color";
                let mut items: Vec<(String, String)> =
                    font_colors().iter().map(|(k, l, _)| (k.to_string(), l.to_string())).collect();
                items.extend(menu(&[ui::item!("その他(RRGGBB を打つ)…")]));
                self.pick = Some((items, at));
            }
            // 並べ替えは**見出しを据え置き、行はまるごと動かす**
            // ユーザー設定の並べ替え。本家は複数基準のダイアログ —
            // calc は小計・ピボットと同じ聞き取りのパネルで複数基準を受ける
            "custom-sort" => {
                self.commit();
                let (rows, cols) = self.sheet().extent();
                if rows < 2 {
                    self.status = ui::t!("並べ替える表がありません(見出しの下にデータが要ります)").into();
                    return;
                }
                let heads: Vec<String> = (0..cols)
                    .map(|c| {
                        self.sheet()
                            .get(Pos::new(0, c))
                            .map(|x| x.value.display())
                            .unwrap_or_default()
                    })
                    .filter(|h| !h.is_empty())
                    .collect();
                self.prompt = Some(("sort-by", Editor::new("")));
                self.status = ui::tf!(
                    "基準を左から強い順に(例: 金額 降順, 品名)。使える見出し: {}",
                    heads.join(" / ")
                )
                .into();
            }
            "rem-duplicates" => {
                self.commit();
                // 本家のダイアログと同じく、比べる列と見出しの有無を選んでから消す
                let (rows, cols) = self.sheet().extent();
                if rows == 0 {
                    self.status = ui::t!("表がありません").into();
                    return;
                }
                let mut list: Vec<(u32, String, bool)> = Vec::new();
                for col in 0..cols.max(1) {
                    let head = self
                        .sheet()
                        .get(sheet::Pos::new(0, col))
                        .map(|x| x.value.display())
                        .unwrap_or_default();
                    let name = if head.is_empty() {
                        ui::tf!("{} 列", crate::util::col_name(col)).to_string()
                    } else {
                        head
                    };
                    list.push((col, name, true)); // 既定は「すべて選択」
                }
                self.dedup_pend = Some((list, true));
                self.dedup_pick();
            }
            // **通貨は選ばせる。** 言語から決めない(お金は帳票のもの)。
            // 二段の一覧 — 押すと通貨が並び、選ぶと書式が掛かる
            "currency" => {
                let at = self.pop_anchor();
                self.pick_kind = "currency";
                self.pick_note =
                    Some(ui::t!("通貨(記号は帳票のお金。並びは画面の言語に従います)").into());
                let items: Vec<(String, String)> = currencies()
                    .iter()
                    .map(|(k, l, _, _)| (k.to_string(), l.to_string()))
                    .collect();
                self.pick = Some((items, at));
            }
            // **日付も選ばせる。** 見出しはその書式で描いた見本そのものなので、
            // 何語で開いても嘘にならない
            "datefmt" => {
                let at = self.pop_anchor();
                self.pick_kind = "datefmt";
                self.pick_note = Some(
                    ui::t!("日付の形(選ぶと、その形で書いたことがファイルに残ります)").into(),
                );
                let items: Vec<(String, String)> = date_formats()
                    .into_iter()
                    .map(|(k, label, _)| (k.to_string(), label))
                    .collect();
                self.pick = Some((items, at));
            }
            "percents" => self.fmt(|f| f.number_format = Some("0%".into())),
            // 関数の一覧。**使える名前だけを出す** — 無いものを並べない
            f @ ("fn-math" | "fn-text" | "fn-logical" | "fn-recent" | "fn-datetime"
            | "fn-lookup" | "fn-financial" | "fn-more") => {
                let names: &str = match f {
                    "fn-math" => "SUM AVERAGE ROUND ROUNDUP ROUNDDOWN INT ABS MOD POWER SQRT \
                                  PRODUCT SUMPRODUCT SUMSQ CEILING FLOOR MROUND EVEN ODD SIGN \
                                  FACT COMBIN PERMUT GCD LCM PI SIN COS TAN ASIN ACOS ATAN ATAN2 \
                                  SINH COSH TANH EXP LN LOG LOG10 DEGREES RADIANS RAND RANDBETWEEN \
                                  SEQUENCE(隣へあふれる。=SEQUENCE(3)+1 のような式も可)",
                    "fn-text" => "LEN LEFT RIGHT MID TRIM UPPER LOWER CONCATENATE CONCAT TEXT \
                                  SUBSTITUTE FIND SEARCH VALUE TEXTJOIN REPT CHAR CODE \
                                  UNICHAR UNICODE PROPER EXACT CLEAN FIXED YEN NUMBERVALUE \
                                  LENB LEFTB RIGHTB MIDB ASC JIS DATESTRING(和暦) \
                                  PHONETIC(ふりがな — 読んだ xlsx の rPh を引く)",
                    "fn-logical" => "IF IFS SWITCH AND OR NOT TRUE FALSE ISBLANK ISERROR IFERROR \
                                     IFNA ISNA ISERR ISLOGICAL ISNONTEXT ISNUMBER ISTEXT NA",
                    "fn-datetime" => "TODAY NOW DATE DATEVALUE YEAR MONTH DAY WEEKDAY \
                                      TIME HOUR MINUTE SECOND EDATE EOMONTH DATEDIF \
                                      WORKDAY NETWORKDAYS DAYS DAYS360 YEARFRAC \
                                      WEEKNUM ISOWEEKNUM(値は通し番号)",
                    "fn-lookup" => "VLOOKUP HLOOKUP XLOOKUP LOOKUP INDEX MATCH CHOOSE \
                                    ROW COLUMN ROWS COLUMNS OFFSET INDIRECT ADDRESS HYPERLINK \
                                    FILTER SORT UNIQUE TRANSPOSE(照合は完全一致。\
                                    FILTER 等は隣へあふれ、四則と組み合わせても効く)",
                    "fn-financial" => "PMT PV FV NPER NPV IRR RATE(IRR と RATE は挟み撃ちの反復解)",
                    "fn-more" => "SUMIF SUMIFS COUNTIF COUNTIFS AVERAGEIF AVERAGEIFS \
                                  MINIFS MAXIFS COUNTA COUNTBLANK TRUNC \
                                  RANK RANK.EQ RANK.AVG LARGE SMALL \
                                  MEDIAN MODE STDEV STDEVP VAR VARP PERCENTILE QUARTILE \
                                  CORREL SLOPE INTERCEPT FORECAST AVERAGEA MAXA MINA \
                                  SUBTOTAL QUOTIENT CEILING.MATH FLOOR.MATH \
                                  ISEVEN ISODD T N TYPE — 一覧は各族のボタンで",
                    _ => "SUM AVERAGE COUNT MAX MIN IF SUMIF COUNTIF VLOOKUP TODAY",
                };
                self.status = ui::tf!("使える関数: {}", names).into();
            }
            f @ ("sum" | "average" | "count" | "max" | "min") => {
                // 上の連続した数値をまとめる(表計算の当たり前の動き)
                let name = f.to_uppercase();
                let (r, c) = (self.cursor.row, self.cursor.col);
                let mut top = r;
                while top > 0 && self.sheet().get(Pos::new(top - 1, c))
                    .map(|x| matches!(x.value, Value::Number(_)) || x.formula.is_some())
                    .unwrap_or(false) { top -= 1 }
                let text = if top < r {
                    format!("={name}({}:{})", Pos::new(top, c).a1(), Pos::new(r - 1, c).a1())
                } else {
                    format!("={name}()")
                };
                self.input = Editor::new(&text);
                self.commit();
                self.sync_input();
            }
            // **利用者がリボンに足したマクロ**(~/.config/officework/ribbon/名前.py)。
            // 静的な表に無い id はここだけを通る — 名乗った .py と1対1で、
            // 名前を作れるのは置き場に .py を置いた人だけ(2026-08-16)
            other if other.starts_with(ribbon::USER_PREFIX) => {
                let m = other[ribbon::USER_PREFIX.len()..].to_string();
                self.run_plugin_in(pyrun::ribbon_dir(), &m, None, cx);
            }
            other => {
                // ここに来たら結線漏れ。黙らず画面に出す
                self.status = ui::tf!("未配線のコマンド: {}(不具合です)", other).into();
            }
        }
    }
}

impl Calc {
    /// 行・列の非表示と再表示(見出しの右クリック)。土台は行と列の
    /// `hidden`(xlsx と往復する = 隠したまま次の人に渡る)で、
    /// データタブの「グループ化」と同じ器を使う。
    /// **全部は隠さない** — 隠したものを戻す道が見えなくなるため
    pub(crate) fn hide_lines(&mut self, id: &str) {
        let cols = id.ends_with("cols");
        let hide = id.starts_with("hide");
        let (a, b) = self.sel_rect();
        let (lo, hi) = if cols { (a.col, b.col) } else { (a.row, b.row) };
        // 「列」「行」を文の中へ差し込むのはやめた。差し込まれた語は語形を
        // 変えられないので、独・西・伊・露は5つの文のどれかが必ず崩れる。
        // **列の文と行の文を丸ごと持つ** — 訳す側は文として訳せる
        // 使っている分を全部隠すと、戻す道(見出しを跨いで選ぶ)が消える
        if hide {
            let (rows, colsn) = self.sheet().extent();
            let total = if cols { colsn } else { rows };
            let already =
                if cols { &self.sheet().col_hidden } else { &self.sheet().row_hidden };
            let left = (0..total)
                .filter(|i| !already.contains(i) && !(lo..=hi).contains(i))
                .count();
            if total > 0 && left == 0 {
                self.status = if cols {
                    ui::t!("使っている列を全部は隠せません(戻す道が見えなくなるため)").into()
                } else {
                    ui::t!("使っている行を全部は隠せません(戻す道が見えなくなるため)").into()
                };
                return;
            }
        }
        self.checkpoint();
        let sh = self.sheet_mut();
        let set = if cols { &mut sh.col_hidden } else { &mut sh.row_hidden };
        let mut n = 0usize;
        if hide {
            for i in lo..=hi {
                n += set.insert(i) as usize;
            }
        } else {
            // 選んだ範囲の中で隠れているものを戻す(Excel の作法 —
            // 隠れた行を挟む形で選んでから「再表示」)
            let back: Vec<u32> = (lo..=hi).filter(|i| set.contains(i)).collect();
            n = back.len();
            for i in back {
                set.remove(&i);
            }
        }
        if n == 0 {
            self.undo_stack.pop(); // 何も変わっていないので控えも戻す
            self.status = match (hide, cols) {
                (true, true) => ui::t!("その列はもう隠れています").into(),
                (true, false) => ui::t!("その行はもう隠れています").into(),
                (false, true) => {
                    ui::t!("選んだ中に隠れた列はありません(隠れた分を挟むように選ぶ)").into()
                }
                (false, false) => {
                    ui::t!("選んだ中に隠れた行はありません(隠れた分を挟むように選ぶ)").into()
                }
            };
            return;
        }
        self.dirty = true;
        self.status = match (hide, cols) {
            (true, true) => {
                ui::tf!("{} 列を隠しました(見出しを跨いで選び「再表示」で戻せます)", n).into()
            }
            (true, false) => {
                ui::tf!("{} 行を隠しました(見出しを跨いで選び「再表示」で戻せます)", n).into()
            }
            (false, true) => ui::tf!("{} 列を戻しました", n).into(),
            (false, false) => ui::tf!("{} 行を戻しました", n).into(),
        };
    }
}

impl Calc {
    /// 変更履歴(校閲の記録)の入切。**writer と同じ型** — 操作を1つずつ
    /// 拾うのではなく、**記録開始時点の写しと突き合わせて差分を刻む**。
    /// 編集中は普通に打てて(モデルに手を入れない)、止めたときに1度だけ数える。
    /// 記録は xl/joChanges.xml で往復する — Excel は読まない(正直な劣化)
    pub(crate) fn track_changes(&mut self) {
        match self.track_from.take() {
            None => {
                // 開始: いまの「打った姿」を写す(式は式のまま)
                let snap = self
                    .book
                    .sheets
                    .iter()
                    .map(|s| {
                        let cells = s
                            .cells
                            .iter()
                            .map(|(p, c)| (*p, c.editable().to_string()))
                            .collect();
                        (s.name.clone(), cells)
                    })
                    .collect();
                self.track_from = Some(snap);
                self.status = ui::t!(
                    "変更履歴を記録します(もう一度押すと、開始時点との差を刻んで一覧を出します)"
                )
                .into();
            }
            Some(snap) => {
                let (who, when) = (crate::io::lock_identity(), crate::util::now_stamp());
                let mut add: Vec<sheet::model::ChangeRec> = Vec::new();
                for s in &self.book.sheets {
                    let before: &std::collections::BTreeMap<Pos, String> = snap
                        .iter()
                        .find(|(n, _)| *n == s.name)
                        .map(|(_, m)| m)
                        .unwrap_or(&EMPTY_SNAP);
                    // いま在るセル: 変わった / 増えた
                    for (p, c) in &s.cells {
                        let now = c.editable().to_string();
                        let was = before.get(p).cloned().unwrap_or_default();
                        if was != now {
                            add.push(sheet::model::ChangeRec {
                                who: who.clone(),
                                when: when.clone(),
                                sheet: s.name.clone(),
                                at: *p,
                                before: was,
                                after: now,
                            });
                        }
                    }
                    // 消えたセル
                    for (p, was) in before {
                        if !s.cells.contains_key(p) && !was.is_empty() {
                            add.push(sheet::model::ChangeRec {
                                who: who.clone(),
                                when: when.clone(),
                                sheet: s.name.clone(),
                                at: *p,
                                before: was.clone(),
                                after: String::new(),
                            });
                        }
                    }
                }
                let n = add.len();
                if n > 0 {
                    self.checkpoint();
                    self.book.changes.extend(add);
                    self.dirty = true;
                }
                self.status = if n == 0 {
                    ui::t!("記録を止めました(変わった所はありません)").into()
                } else {
                    ui::tf!("記録を止めました — {} 件を刻みました(一覧は同じボタンをもう一度)", n)
                        .into()
                };
                if n == 0 {
                    self.show_changes();
                }
            }
        }
    }

    /// 刻んだ変更履歴の一覧(新しい順)。記録していないときはこちらが出る
    pub(crate) fn show_changes(&mut self) {
        if self.book.changes.is_empty() {
            self.status =
                ui::t!("変更履歴はまだありません(共同編集タブの「変更履歴」で記録を始める)").into();
            return;
        }
        let at = self.pop_anchor();
        // 日時・シート!番地・値・名乗りは**中身**。訳すのは「(空)」「(消した)」の
        // 2語だけ。鍵は前のまま — picks.rs は3語目の「シート!A1」で跳ぶ
        let items: Vec<(String, String)> = self
            .book
            .changes
            .iter()
            .rev()
            .take(200)
            .map(|c| {
                let (arrow_key, arrow) = match (c.before.is_empty(), c.after.is_empty()) {
                    (true, _) => (
                        format!("(空) → {}", c.after),
                        ui::tf!("(空) → {}", c.after),
                    ),
                    (_, true) => (
                        format!("{} → (消した)", c.before),
                        ui::tf!("{} → (消した)", c.before),
                    ),
                    _ => (
                        format!("{} → {}", c.before, c.after),
                        format!("{} → {}", c.before, c.after),
                    ),
                };
                let head = format!("{} {}!{}", c.when, c.sheet, c.at.a1());
                (
                    format!("{head} {arrow_key} [{}]", c.who),
                    format!("{head} {arrow} [{}]", c.who),
                )
            })
            .collect();
        self.pick_note = Some(
            ui::tf!(
                "変更履歴 {} 件(新しい順。選んでもその場所へ跳ぶだけ — 戻す機能ではありません)",
                self.book.changes.len()
            )
            .into(),
        );
        self.pick_kind = "changes-pick";
        self.pick = Some((items, at));
    }

    /// **中身に合わせて列幅・行高を決める。**(リボンの「自動調整」と
    /// 橋(Python の autofit)の共有 — 文字の測りはアプリが持っているので
    /// ここに置く。2026-08-13 に rpc と分け合うため run_cmd から切り出した)
    ///
    /// 返りは合わせた本数。undo の節目は**呼ぶ側**が置く
    /// (リボンは1手、手続きの中はまとめて1手)。
    pub(crate) fn autofit_at(&mut self, a: Pos, b: Pos, col: bool) -> usize {

            let (rows, cols) = self.sheet().extent();
            self.checkpoint();
            let mut n = 0;
            if col {
                for c in a.col..=b.col {
                    let mut need: f32 = 0.0;
                    for r in 0..rows {
                        let Some(cell) = self.sheet().get(Pos::new(r, c)) else { continue };
                        if cell.fmt.wrap {
                            continue; // 折り返すセルは幅を決める根拠にしない
                        }
                        let t = cell.value.display();
                        if t.is_empty() {
                            continue;
                        }
                        // 幅を測るのも画面と同じ換算で(cell_font_px の1本)。
                        // ここだけ古い係数だと、列幅の自動調整が字より狭くなる
                        let size = crate::util::cell_font_px(cell.fmt.size_c, 1.0);
                        need = need.max(text_px(&t, size));
                    }
                    if need <= 0.0 {
                        continue;
                    }
                    // px → xlsx の字数。**上限を置く**(1セルの長文で
                    // 画面いっぱいの列にならないように。本家も 255 字)
                    let chars = (need / PX_PER_CHW).clamp(1.0, 255.0);
                    self.sheet_mut().col_width.insert(c, chars);
                    n += 1;
                }
            } else {
                let named = self.book.named_styles.clone();
                for r in a.row..=b.row {
                    let mut want: f32 = 15.0; // xlsx の既定(pt)
                    for c in 0..cols.max(1) {
                        let p = Pos::new(r, c);
                        let Some(cell) = self.sheet().get(p) else { continue };
                        let t = cell.value.display();
                        if t.is_empty() {
                            continue;
                        }
                        let md = sheet::cellmark::parse(&t);
                        let scale = match &md {
                            Some(l) if cell.fmt.wrap => {
                                sheet::cellmark::wanted_height_pt(l, 15.0, &named) / 15.0
                            }
                            Some(l) => l
                                .iter()
                                .map(|x| sheet::cellmark::line_scale(x, &named))
                                .fold(1.0, f32::max),
                            None => 1.0,
                        };
                        let lines = if cell.fmt.wrap {
                            let size = crate::util::cell_font_px(cell.fmt.size_c, 1.0);
                            let w = self.col_px(c).max(8.0);
                            (text_px(&t, size) / w).ceil().max(1.0)
                        } else {
                            1.0
                        };
                        want = want.max(15.0 * scale * lines);
                    }
                    self.sheet_mut().row_height.insert(r, want);
                    n += 1;
                }
            }
        n
    }

}

/// 記録開始時点の写しが無いシート用の空(borrow のため const で置く)
static EMPTY_SNAP: std::sync::LazyLock<std::collections::BTreeMap<Pos, String>> =
    std::sync::LazyLock::new(Default::default);

impl Calc {
    /// **ファイルのページに並ぶ物**(統合の段8 の1)。
    ///
    /// 17 個は writer と同じ id・同じ意味で、`CSV に書き出す`・`マクロ` が
    /// 表の画面だけの物です。**次の段で officework がこの表を読んで描きます。**
    pub fn file_menu(&self) -> Vec<ui::filemenu::Item> {
        use ui::filemenu::Item as I;
        vec![
            I::new("f-back", ui::t!("‹ 戻る")),
            I::new("f-new", ui::t!("新規作成")).gap(),
            I::new("f-tpl", ui::t!("テンプレートから作成")).grey(),
            I::new("f-open", ui::t!("開く")),
            I::new("f-recent", ui::t!("最近開いた")).on(self.file_view == 1),
            I::new("f-find", ui::t!("フォルダから探す")).on(self.file_view == 3),
            I::new("f-save", ui::t!("保存")).gap(),
            I::new("f-saveas", ui::t!("名前を付けて保存")),
            I::new("f-print", ui::t!("印刷")),
            I::new("f-csv", ui::t!("CSV に書き出す")),
            I::new("f-html", ui::t!("Web に書き出す")),
            I::new("f-protect", ui::t!("保護する")).on(self.file_view == 5),
            I::new("f-macro", ui::t!("マクロ")),
            I::new("f-info", ui::t!("詳細情報")).gap().on(self.file_view == 0),
            I::new("f-place", ui::t!("ファイルの場所を開く")),
            I::new("f-quit", ui::t!("終了")).gap(),
            I::new("f-opts", ui::t!("詳細設定")).tail().on(self.file_view == 2),
            I::new("f-help", ui::t!("ヘルプ")).grey().tail(),
            I::new("f-req", ui::t!("機能のリクエスト")).grey().tail(),
        ]
    }

    /// **ファイルのページの項目を捌く**(統合の段8 の1)。
    /// 前は画面の中のその場の閉包でした。1つの `match` に集めたので、
    /// **officework から id を渡して呼べます**(次の段)。
    pub fn file_menu_click(&mut self, id: &str, cx: &mut Context<Self>) {
        // **共通の腕は ui::filemenu が先に取ります**(段8 の3)。
        // 同じ id の腕をここに残すと向こうが先に取るので、残した腕は死にます
        if ui::filemenu::run(self, id) || ui::filemenu::run_cx(self, id, cx) {
            cx.notify();
            return;
        }
        match id {
            "f-print" => self.run_cmd("pdf", cx),
            "f-csv" => self.export_csv_dialog(cx),
            "f-html" => self.export_html_dialog(cx),
            "f-macro" => {
                if let Some(i) = ribbon::CALC.iter().position(|t| t.name == "マクロ") {
                    self.prev_tab = i;
                    self.tab = i;
                }
                self.run_cmd("py-list", cx);
            }
            _ => {}
        }
    }
}
