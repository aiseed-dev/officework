//! **文字の編集と紙面。** 選択・段落書式・目次・相互参照・印刷。

use crate::*;

impl Writer {
    /// 文字位置 → 紙の上の座標(キャレットを出すため)
    /// 語の単位でカーソルを動かす(Ctrl+←→)。
    pub(crate) fn word_move(&mut self, forward: bool, extend: bool) {
        let t = self.ed.text().to_string();
        let np = word_boundary(&t, self.ed.cursor(), forward);
        self.ed.move_to(np, extend);
        self.follow_caret();
    }

    /// カーソルの下の語を選ぶ(二度クリック)。
    pub(crate) fn select_word(&mut self) {
        let t = self.ed.text().to_string();
        if t.is_empty() {
            return;
        }
        let pos = self.ed.cursor().min(t.len());
        let chars: Vec<(usize, char)> = t.char_indices().collect();
        // カーソルの字(末尾なら手前の字)から、同じ種類の連なりを広げる
        let ci = chars.iter().position(|(i, _)| *i >= pos).unwrap_or(chars.len());
        let k = ci.min(chars.len() - 1);
        let cl = char_class(chars[k].1);
        let mut s = k;
        while s > 0 && char_class(chars[s - 1].1) == cl {
            s -= 1;
        }
        let mut e = k + 1;
        while e < chars.len() && char_class(chars[e].1) == cl {
            e += 1;
        }
        let sb = chars[s].0;
        let eb = chars.get(e).map(|(i, _)| *i).unwrap_or(t.len());
        self.ed.move_to(sb, false);
        self.ed.move_to(eb, true);
    }

    /// いまの(見た目の)行を選ぶ(三度クリック)。
    pub(crate) fn select_line(&mut self) {
        let pos = self.ed.cursor();
        let want = match self.target {
            Target::Body => None,
            Target::Cell { table, row, col } => Some((table, row, col)),
        };
        let mut hit: Option<(usize, usize)> = None;
        for l in self.page.lines.iter().filter(|l| match want {
            None => l.from_body,
            Some(id) => l.cell == Some(id),
        }) {
            if l.byte0 <= pos {
                hit = Some((l.byte0, l.byte_end()));
            }
        }
        if let Some((s, e)) = hit {
            self.ed.move_to(s, false);
            self.ed.move_to(e, true);
        }
    }

    /// 1画面ぶん(PageUp/PageDown)。見た目の行を数えて動く。
    pub(crate) fn page_move(&mut self, down: bool) {
        let pxmm = PX_PER_MM * self.zoom;
        let step = ((self.view_h_px / (LINE_MM * pxmm)) as i32 - 2).max(1);
        for _ in 0..step {
            self.move_line(down, false);
        }
    }

    /// カーソルを1行、上(または下)へ。**見た目の行**単位 — 折り返した長い
    /// 段落の中でも1段ずつ動く。横の位置(x)はなるべく保つ。
    /// 一番上で↑なら文頭、一番下で↓なら文末へ(行の端で止まって動かないより良い)。
    pub(crate) fn move_line(&mut self, down: bool, extend: bool) {
        let pos = self.ed.cursor();
        let want = match self.target {
            Target::Body => None,
            Target::Cell { table, row, col } => Some((table, row, col)),
        };
        let lines: Vec<&kumihan::Line> = self
            .page
            .lines
            .iter()
            .filter(|l| match want {
                None => l.from_body,
                Some(id) => l.cell == Some(id),
            })
            .collect();
        if lines.is_empty() {
            return;
        }
        // いまの行 = 頭がカーソル以前にある最後の行
        let cur = lines.iter().rposition(|l| l.byte0 <= pos).unwrap_or(0);
        let target = if down {
            if cur + 1 >= lines.len() {
                let end = self.ed.text().len();
                self.ed.move_to(end, extend);
                self.follow_caret();
                return;
            }
            cur + 1
        } else {
            if cur == 0 {
                self.ed.move_to(0, extend);
                self.follow_caret();
                return;
            }
            cur - 1
        };
        // いまの x(紙の座標)を保ったまま、隣の行で一番近い字の境へ
        let (x_now, _, _) = self.caret_xy();
        let ln = lines[target];
        let base = ln.cells.iter().map(|c| c.off).min().unwrap_or(0);
        let mut byte = ln.byte_end();
        for c in &ln.cells {
            let cx = self.pg.left_mm + c.x_mm;
            if x_now < cx + c.w_mm / 2.0 {
                byte = ln.byte0 + (c.off - base);
                break;
            }
        }
        self.ed.move_to(byte.min(self.ed.text().len()), extend);
        self.follow_caret();
    }

    /// カーソルの紙面上の位置と、そこの文字の大きさ(pt)。
    /// キャレットは**その場の文字の大きさで**描く — 見出しの中で
    /// 小さいままだと、どこに立っているのか分からない。
    pub(crate) fn caret_xy(&self) -> (f32, f32, f32) {
        let cur = self.ed.cursor();
        // 行の頭のバイト位置(byte0)は組版が持っている。
        // 行の文字数で数え直すと、折り返しで落ちた空白や空行でずれる。
        // 折り返し・段落の境目では**後ろの行**に立てる(Enter の直後は次の行)
        let want = match self.target {
            Target::Body => None,
            Target::Cell { table, row, col } => Some((table, row, col)),
        };
        let mut hit: Option<(f32, f32, f32)> = None;
        for (li, line) in self.page.lines.iter().enumerate().filter(|(_, l)| match want {
            None => l.from_body,
            Some(id) => l.cell == Some(id),
        }) {
            if cur < line.byte0 {
                continue;
            }
            if cur > line.byte_end() + 1 {
                continue;
            }
            let within = cur.saturating_sub(line.byte0);
            let base = line.cells.iter().map(|c| c.off).min().unwrap_or(0);
            let at = line.cells.iter().find(|c| c.off - base >= within);
            let x = at
                .map(|c| c.x_mm)
                .or_else(|| line.cells.last().map(|c| c.x_mm + c.w_mm))
                .unwrap_or(0.0);
            let pt = at
                .or_else(|| line.cells.last())
                .map(|c| c.size_pt)
                .unwrap_or(SIZE_PT);
            hit = if self.page.vertical {
                // 縦書き: x は列、y は上からの距離(スクロール追従がこれを見る)
                let col = self.page.vert_x.get(li).copied().unwrap_or(0.0);
                Some((col, line.y_mm + x, pt))
            } else {
                Some((self.pg.left_mm + x, line.y_mm, pt))
            };
        }
        hit.unwrap_or((
            self.pg.left_mm,
            self.page.lines.last().map(|l| l.y_mm).unwrap_or(self.pg.top_mm),
            SIZE_PT,
        ))
    }

    /// レビュー > 校正。**英語は辞書、日本語はモデル。**
    ///
    /// 英語の綴り誤りは辞書に無い語になるので辞書で捕まる(GPU も要らない)。
    /// 日本語の誤変換は辞書に有る語になるので、辞書では原理的に捕まらない。
    ///
    /// 検査できなかった部分があれば必ずそう出す — **黙って「指摘なし」にしない**
    /// (利用者は「誤りが無い」と受け取ってしまう)。
    pub(crate) fn run_proof(&mut self) {
        let r = self.checker.check(self.ed.text());
        self.proof_msg = r.summary().into();
        self.proof = r.findings;
    }

    /// 編集中のセルの段落へ書式を掛ける(セルは短いので丸ごと掛ける)。
    pub(crate) fn each_cell_para(&mut self, f: impl Fn(&mut kumihan::Paragraph)) {
        self.checkpoint(false); // 表のセルの段落
        let Target::Cell { table, row, col } = self.target else { return };
        self.flush_target();
        if let Some(kumihan::Block::Table(tb)) = self
            .doc
            .blocks
            .iter_mut()
            .filter(|b| matches!(b, kumihan::Block::Table(_)))
            .nth(table)
        {
            if let Some(cell) = tb.rows.get_mut(row).and_then(|r| r.get_mut(col)) {
                for p in &mut cell.paragraphs {
                    f(p);
                }
            }
        }
    }

    /// 選択している段落の文字書式を入切する。
    ///
    /// **編集先が本文かセルかで掛け先が違う。** セル編集中に本文へ掛けると、
    /// set_body_text がセルの文章で本文を上書きしてしまう。
    pub(crate) fn toggle(&mut self, f: impl Fn(&mut kumihan::CharFormat)) {
        // 書式は**文書を直に変える**ので、平文の取り消しでは戻らない。
        // 変える前に控える(太字・斜体・下線・取り消し線・上下付きが同じ道)
        self.checkpoint(false);
        match self.target {
            Target::Body => {
                let sel = self.ed.selection();
                self.doc.set_body_text(self.ed.text(), SIZE_PT);
                self.doc.apply_char_format(sel, f);
            }
            Target::Cell { .. } => self.each_cell_para(|p| {
                for r in &mut p.runs {
                    f(&mut r.fmt);
                }
            }),
        }
        self.dirty = true;
        self.relayout_keep();
    }

    /// 選んでいる段落の性質を変える。
    pub(crate) fn para(&mut self, f: impl Fn(&mut kumihan::Paragraph) + Copy) {
        self.checkpoint(false); // 段落の性質(揃え・箇条書き・字下げ・行間)
        if self.protected() {
            self.status =
                ui::t!("読み取り専用で保護されています(保護タブの「保護」で解除できます)").into();
            return;
        }
        match self.target {
            Target::Body => {
                let sel = self.ed.selection();
                self.doc.set_body_text(self.ed.text(), SIZE_PT);
                self.doc.apply_para(sel, f);
            }
            Target::Cell { .. } => self.each_cell_para(f),
        }
        self.dirty = true;
        self.relayout_keep();
    }

    pub(crate) fn size(&mut self, f: impl Fn(f32) -> f32 + Copy) {
        self.checkpoint(false); // 文字の大きさ
        match self.target {
            Target::Body => {
                let sel = self.ed.selection();
                self.doc.set_body_text(self.ed.text(), SIZE_PT);
                self.doc.apply_size(sel, f);
            }
            Target::Cell { .. } => self.each_cell_para(|p| {
                for r in &mut p.runs {
                    r.size_pt = f(r.size_pt).clamp(4.0, 400.0);
                }
            }),
        }
        self.dirty = true;
        self.relayout_keep();
    }

    /// PDF として保存。保存先の選択は**別のスレッド**(rfd は同期)。
    pub(crate) fn save_pdf(&mut self, cx: &mut Context<Self>) {
        // 見開きは画面だけの見え方。紙は1ページずつなので組み直してから写す
        if self.multipage {
            let keep = self.multipage;
            self.multipage = false;
            self.relayout_keep();
            self.multipage = keep;
        }
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name(ui::t!("文書.pdf"))
                .save_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = r {
                    this.write_pdf(&p);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// **画面に出しているのと同じ紙面を写す**ので、画面と紙が食い違わない。
    pub(crate) fn write_pdf(&mut self, p: &std::path::Path) {
        let m = Metrics::new(&self.font_bytes).expect("フォント");
        let (hdr, ftr, pg) = (self.doc.header.clone(), self.doc.footer.clone(), self.pg);
        let total = self.total_pages();
        // ページの色と透かしは紙にも(画面と紙の一致)
        let dress = paper::PageDress {
            bg: self.doc.page_color.as_deref().map(|c| (hex(c, 0), hex(c, 1), hex(c, 2))),
            watermark: self.doc.watermark.clone(),
            ink: self.doc.ink.clone(),
        };
        let r = kumihan::atomic::save(p, |f| {
            paper::to_pdf_with(
                &self.page,
                &self.font_bytes,
                paper::Paper {
                    width_mm: pg.w_mm,
                    height_mm: pg.h_mm,
                    margin_mm: pg.left_mm,
                },
                &dress,
                // ヘッダー・フッター。ページ番号はここで各頁の数字になる
                |k| {
                    let mut v = kumihan::layout_hf(&hdr, &m, &pg, LINE_MM, k, total, false);
                    v.extend(kumihan::layout_hf(&ftr, &m, &pg, LINE_MM, k, total, true));
                    v
                },
                std::io::BufWriter::new(f),
            )
        });
        self.status = match r {
            Ok(_) => ui::tf!("PDF にしました — {}", p.file_name().unwrap_or_default().to_string_lossy()).into(),
            Err(e) => ui::tf!("PDF にできません: {}", e).into(),
        };
    }

    /// 用紙の設定を変える。**文書に書き戻す**(sect_raw を作り替える)ので
    /// 保存で残る。画面と紙は同じ寸法で追随する。
    pub(crate) fn set_page(&mut self, f: impl Fn(&mut kumihan::PageSetup)) {
        self.checkpoint(false); // 用紙の設定
        f(&mut self.pg);
        self.doc.page = Some(self.pg);
        let tw = |mm: f32| -> i64 { (mm * 20.0 * 72.0 / 25.4).round() as i64 };
        let landscape = self.pg.w_mm > self.pg.h_mm;
        // 原文があっても、寸法だけはこちらが決めた値で作り替える。
        // ヘッダーの参照などは残したいので、pgSz/pgMar 以外は原文から引き継ぐ
        let rest = self
            .doc
            .sect_raw
            .as_deref()
            .map(|s| {
                let mut out = String::new();
                let mut skip = false;
                for part in s.split_inclusive('>') {
                    let t = part.trim_start();
                    if t.starts_with("<w:sectPr") || t.starts_with("</w:sectPr") {
                        continue;
                    }
                    if t.starts_with("<w:pgSz") || t.starts_with("<w:pgMar")
                        || t.starts_with("<w:cols")
                    {
                        skip = !part.trim_end().ends_with("/>");
                        continue;
                    }
                    if skip {
                        if t.starts_with("</w:pgSz") || t.starts_with("</w:pgMar")
                            || t.starts_with("</w:cols")
                        {
                            skip = false;
                        }
                        continue;
                    }
                    out.push_str(part);
                }
                out
            })
            .unwrap_or_default();
        // 段組みは Word の既定の間(425twip)で書く
        let cols = if self.pg.cols() > 1 {
            format!("<w:cols w:num=\"{}\" w:space=\"425\"/>", self.pg.cols())
        } else {
            String::new()
        };
        self.doc.sect_raw = Some(format!(
            "<w:sectPr><w:pgSz w:w=\"{}\" w:h=\"{}\"{}/>\
             <w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\"/>{cols}{rest}</w:sectPr>",
            tw(self.pg.w_mm),
            tw(self.pg.h_mm),
            if landscape { " w:orient=\"landscape\"" } else { "" },
            tw(self.pg.top_mm),
            tw(self.pg.right_mm),
            tw(self.pg.bottom_mm),
            tw(self.pg.left_mm),
        ));
        self.dirty = true;
        self.relayout_keep();
        self.status = ui::tf!("用紙 {:.0}×{:.0}mm / 余白 {:.0}mm{}", self.pg.w_mm, self.pg.h_mm, self.pg.left_mm, if self.pg.cols() > 1 { format!(" / {}段組み", self.pg.cols()) } else { String::new() })
        .into();
    }

    pub(crate) fn set_align(&mut self, a: Align) {
        self.checkpoint(false); // 揃え
        match self.target {
            Target::Body => {
                let sel = self.ed.selection();
                self.doc.set_body_text(self.ed.text(), SIZE_PT);
                self.doc.apply_align(sel, a);
            }
            Target::Cell { .. } => self.each_cell_para(|p| p.align = a),
        }
        self.dirty = true;
        self.relayout_keep();
    }

    /// カーソルの段落(通し番号)と、その頭のバイト位置。
    pub(crate) fn cursor_para(&self) -> (usize, usize) {
        let cur = self.ed.cursor();
        let (mut pi, mut b0) = (0usize, 0usize);
        let mut at = 0usize;
        for (i, p) in self.doc.paragraphs().enumerate() {
            let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
            if at <= cur {
                pi = i;
                b0 = at;
            }
            at += len + 1;
        }
        (pi, b0)
    }

    /// 相互参照の値。文字ならしおりの段落の本文、ページなら紙と同じ折り方の番号。
    pub(crate) fn ref_value(&self, name: &str, page: bool) -> Option<String> {
        let mut at = 0usize;
        for p in self.doc.paragraphs() {
            let t: String = p.runs.iter().map(|r| r.text.as_str()).collect();
            if p.bookmarks.iter().any(|b| b == name) {
                if !page {
                    return Some(t.trim().to_string());
                }
                let (pages, _) = paper::paginate(&self.page, paper::Paper {
                    width_mm: self.pg.w_mm,
                    height_mm: self.pg.h_mm,
                    margin_mm: self.pg.left_mm,
                });
                let mut hit = 1usize;
                for (l, pg2) in self.page.lines.iter().zip(&pages) {
                    if l.from_body && l.byte0 <= at {
                        hit = *pg2;
                    }
                }
                return Some(hit.to_string());
            }
            at += t.len() + 1;
        }
        None
    }

    /// 相互参照を挿す。値を普通の字として打ってから、その範囲を参照にする。
    pub(crate) fn insert_ref(&mut self, name: &str, page: bool) {
        self.checkpoint(false); // 相互参照
        self.switch_target(Target::Body);
        let Some(val) = self.ref_value(name, page) else {
            self.status = ui::tf!("しおり「{}」が見つかりません", name).into();
            return;
        };
        let start = self.ed.selection().start;
        handler::replace(self, None, &val);
        self.doc.apply_field(
            start..start + val.len(),
            Some(kumihan::RefField { name: name.to_string(), page }),
        );
        self.relayout_keep();
        self.status = ui::tf!("「{}」への参照を挿しました({}。参照は編集で中を触ると普通の字に戻ります)", name, if page { ui::t!("ページ番号") } else { ui::t!("しおりの文字") })
        .into();
    }

    /// 参照を計算し直す。run の text を直に書き換えるので、編集の平文も作り直す
    /// (**undo の控えはここで失われる** — そう言う)。
    pub(crate) fn refresh_refs(&mut self) {
        self.switch_target(Target::Body);
        self.flush_target();
        let vals: std::collections::BTreeMap<(String, bool), String> = self
            .doc
            .paragraphs()
            .flat_map(|p| p.runs.iter())
            .filter_map(|r| r.fmt.field.clone())
            .map(|f| {
                let v = self.ref_value(&f.name, f.page).unwrap_or_else(|| "?".into());
                ((f.name, f.page), v)
            })
            .collect();
        let n = self
            .doc
            .refresh_fields(|name, page| vals.get(&(name.to_string(), page)).cloned());
        if n > 0 {
            let cur = self.ed.cursor();
            self.ed = Editor::new(&self.doc.body_text());
            let len = self.ed.text().len();
            self.ed.move_to(cur.min(len), false);
            self.dirty = true;
            self.relayout_keep();
            self.status =
                ui::tf!("参照を {} 箇所更新しました(この操作は戻せません)", n).into();
        } else {
            self.status = ui::t!("参照は最新です").into();
        }
    }

    /// しおりを追加する(カーソルの段落へ)。
    pub(crate) fn bm_add(&mut self) {
        self.checkpoint(false); // しおり
        let name = self.bm_ed.text().trim().to_string();
        if name.is_empty() {
            self.status = ui::t!("しおりの名前を打ってから追加してください").into();
            return;
        }
        if self.doc.paragraphs().any(|p| p.bookmarks.contains(&name)) {
            self.status = ui::tf!("しおり「{}」は既にあります", name).into();
            return;
        }
        self.switch_target(Target::Body);
        let (pi, _) = self.cursor_para();
        let mut i = 0usize;
        for b in &mut self.doc.blocks {
            if let kumihan::Block::Para(p) = b {
                if i == pi {
                    p.bookmarks.push(name.clone());
                    break;
                }
                i += 1;
            }
        }
        self.bm_ed = Editor::new("");
        self.dirty = true;
        self.status = ui::tf!("しおり「{}」を付けました(保存で docx に入ります)", name).into();
    }

    /// 段落のスタイル。0 = 標準、1〜3 = 見出し。
    /// スタイル定義(styles.xml)を持たないので、見た目は直接書式で付ける。
    pub(crate) fn set_para_style(&mut self, n: u8) {
        self.checkpoint(false); // 段落の様式
        let (pt, bold) = match n {
            1 => (16.0, true),
            2 => (13.0, true),
            3 => (11.5, true),
            _ => (SIZE_PT, false),
        };
        self.para(move |p| {
            p.style = if n == 0 {
                kumihan::ParaStyle::Body
            } else {
                kumihan::ParaStyle::Heading(n)
            };
        });
        self.size(move |_| pt);
        self.toggle(move |f| f.bold = bold);
        self.status = match n {
            0 => ui::t!("標準の段落にしました").into(),
            n => ui::tf!("見出し{} にしました(参考資料 > 目次 の材料になります)", n).into(),
        };
    }

    /// 目次を作る・挿し直す。見出し(ホーム > 段落のスタイル)が材料。
    /// ページ番号は紙(PDF)と同じ折り方(paper::paginate)から出すので、
    /// 印刷した紙とずれない。目次の行は ParaStyle::Toc の印を持ち、
    /// 「目次の更新」はその連続を丸ごと置き換える。
    pub(crate) fn make_toc(&mut self) {
        self.checkpoint(false); // 目次
        self.switch_target(Target::Body);
        self.flush_target();
        // 見出しを集める(本文のバイト位置つき)
        let mut heads: Vec<(u8, String, usize)> = Vec::new();
        let mut at = 0usize;
        for p in self.doc.paragraphs() {
            let text: String = p.runs.iter().map(|r| r.text.as_str()).collect();
            if let kumihan::ParaStyle::Heading(n) = p.style {
                heads.push((n, text.clone(), at));
            }
            at += text.len() + 1;
        }
        if heads.is_empty() {
            self.status =
                ui::t!("見出しがありません(ホーム > 段落のスタイルで見出しを付けてください)").into();
            return;
        }
        // 行 → ページ番号(紙と同じ折り方)
        let (pages, _) = paper::paginate(&self.page, paper::Paper {
            width_mm: self.pg.w_mm,
            height_mm: self.pg.h_mm,
            margin_mm: self.pg.left_mm,
        });
        let page_of = |byte: usize| -> usize {
            let mut hit = 1usize;
            for (l, pg) in self.page.lines.iter().zip(&pages) {
                if l.from_body && l.byte0 <= byte {
                    hit = *pg;
                }
            }
            hit
        };
        // 目次の行。レベルぶん字下げし、点線(…)を実フォントの字幅で詰めて
        // 番号を右端に着地させる(揃えの機構は使わず、文字で作る —
        // 静的な本文なので、開いた Word でもそのままの見た目になる)
        let m = Metrics::new(&self.font_bytes).expect("フォント");
        let measure = self.pg.measure_mm();
        let w_of = |s: &str| -> f32 { s.chars().map(|c| m.advance_mm(c, SIZE_PT)).sum() };
        let (dot_w, sp_w) = (m.advance_mm('…', SIZE_PT), m.advance_mm('　', SIZE_PT));
        let lines: Vec<(u8, String)> = heads
            .iter()
            .map(|(n, t, b)| {
                let head = format!("{}{}", "　".repeat((*n - 1) as usize), t);
                let num = page_of(*b).to_string();
                // 前後に全角1つずつの空きを置き、残りを … で埋める。
                // 1mm の安全代 — 端数で行長を超えると折り返して目次が崩れる
                let avail = measure - w_of(&head) - w_of(&num) - 2.0 * sp_w - 1.0;
                let dots = (avail / dot_w).floor().max(0.0) as usize;
                (*n, ui::tf!("{}　{}　{}", head, "…".repeat(dots), num))
            })
            .collect();

        let toc_paras: Vec<kumihan::Paragraph> = lines
            .iter()
            .map(|(n, t)| kumihan::Paragraph {
                style: kumihan::ParaStyle::Toc(*n),
                line_spacing: 1.0,
                runs: vec![kumihan::Run {
                    text: t.clone(),
                    size_pt: SIZE_PT,
                    font: None,
                    fmt: Default::default(),
                }],
                ..Default::default()
            })
            .collect();
        let replaced =
            self.splice_marked(|st| matches!(st, kumihan::ParaStyle::Toc(_)), toc_paras);
        self.status = if replaced {
            ui::tf!("目次を更新しました({} 項目)", lines.len()).into()
        } else {
            ui::tf!("目次を入れました({} 項目。見出しを変えたら「目次の更新」)", lines.len())
                .into()
        };
    }

    /// 印の付いた段落の連続を、新しい段落の列で置き換える(無ければ
    /// カーソルの段落の前に挿す)。**編集(undo の1手)と blocks を
    /// 同じ形に揃える** — 揃えないと set_body_text の性質の持ち越し
    /// (段落番号ベース)がずれる。返り値: 置き換えたか。
    pub(crate) fn splice_marked(
        &mut self,
        is_mark: impl Fn(kumihan::ParaStyle) -> bool,
        paras: Vec<kumihan::Paragraph>,
    ) -> bool {
        let text: String = paras
            .iter()
            .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        let blocks: Vec<kumihan::Block> =
            paras.into_iter().map(kumihan::Block::Para).collect();
        let mut para_meta: Vec<(usize, usize, bool)> = Vec::new();
        let mut at = 0usize;
        for p in self.doc.paragraphs() {
            let len: usize = p.runs.iter().map(|r| r.text.len()).sum();
            para_meta.push((at, len, is_mark(p.style)));
            at += len + 1;
        }
        let para_block_idx: Vec<usize> = self
            .doc
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| matches!(b, kumihan::Block::Para(_)))
            .map(|(i, _)| i)
            .collect();
        let old = para_meta.iter().position(|(_, _, t)| *t).map(|st| {
            let mut e = st;
            while e + 1 < para_meta.len() && para_meta[e + 1].2 {
                e += 1;
            }
            (st, e)
        });
        let replaced = match old {
            Some((st, e)) => {
                let (b0, _, _) = para_meta[st];
                let (b1, l1, _) = para_meta[e];
                self.ed.move_to(b0, false);
                self.ed.move_to(b1 + l1, true);
                self.ed.insert(&text);
                self.doc.blocks.splice(para_block_idx[st]..=para_block_idx[e], blocks);
                true
            }
            None => {
                let cur = self.ed.cursor();
                let pi = para_meta.iter().rposition(|(b0, _, _)| *b0 <= cur).unwrap_or(0);
                let (b0, _, _) = para_meta[pi];
                self.ed.move_to(b0, false);
                self.ed.move_to(b0, true);
                self.ed.insert(&format!("{text}\n"));
                let bi = para_block_idx[pi];
                self.doc.blocks.splice(bi..bi, blocks);
                false
            }
        };
        self.dirty = true;
        self.relayout();
        self.follow_caret();
        replaced
    }

    /// 図表目次。図表番号(「図 n」で始まる段落)を集めて一覧にする。
    /// 行は ParaStyle::Tof の印を持ち、「図表目次の更新」で丸ごと作り直す。
    pub(crate) fn make_tof(&mut self) {
        self.checkpoint(false); // 図表目次
        self.switch_target(Target::Body);
        self.flush_target();
        let mut items: Vec<(String, usize)> = Vec::new();
        let mut at = 0usize;
        // 探す頭は貼る雛形と同じところから(caption_head の註)
        let head = caption_head();
        for p in self.doc.paragraphs() {
            let t: String = p.runs.iter().map(|r| r.text.as_str()).collect();
            let tt = t.trim();
            if p.style != kumihan::ParaStyle::Tof {
                if let Some(rest) = tt.strip_prefix(head) {
                    if rest.split_whitespace().next().is_some_and(|w| w.parse::<usize>().is_ok()) {
                        items.push((tt.to_string(), at));
                    }
                }
            }
            at += t.len() + 1;
        }
        if items.is_empty() {
            self.status =
                ui::t!("図表番号がありません(参考資料 > 図表番号で付けてください)").into();
            return;
        }
        let (pages, _) = paper::paginate(&self.page, paper::Paper {
            width_mm: self.pg.w_mm,
            height_mm: self.pg.h_mm,
            margin_mm: self.pg.left_mm,
        });
        let page_of = |byte: usize| -> usize {
            let mut hit = 1usize;
            for (l, pg) in self.page.lines.iter().zip(&pages) {
                if l.from_body && l.byte0 <= byte {
                    hit = *pg;
                }
            }
            hit
        };
        let m = Metrics::new(&self.font_bytes).expect("フォント");
        let measure = self.pg.measure_mm();
        let w_of = |s: &str| -> f32 { s.chars().map(|c| m.advance_mm(c, SIZE_PT)).sum() };
        let (dot_w, sp_w) = (m.advance_mm('…', SIZE_PT), m.advance_mm('　', SIZE_PT));
        let paras: Vec<kumihan::Paragraph> = items
            .iter()
            .map(|(t, b)| {
                let num = page_of(*b).to_string();
                let avail = measure - w_of(t) - w_of(&num) - 2.0 * sp_w - 1.0;
                let dots = (avail / dot_w).floor().max(0.0) as usize;
                kumihan::Paragraph {
                    style: kumihan::ParaStyle::Tof,
                    line_spacing: 1.0,
                    runs: vec![kumihan::Run {
                        text: ui::tf!("{}　{}　{}", t, "…".repeat(dots), num),
                        size_pt: SIZE_PT,
                        font: None,
                        fmt: Default::default(),
                    }],
                    ..Default::default()
                }
            })
            .collect();
        let n = paras.len();
        let replaced = self.splice_marked(|st| st == kumihan::ParaStyle::Tof, paras);
        self.status = if replaced {
            ui::tf!("図表目次を更新しました({} 項目)", n).into()
        } else {
            ui::tf!("図表目次を入れました({} 項目)", n).into()
        };
    }

    /// 書式を触ったあとの組み直し。**本文を戻さない**
    /// (戻すと今つけた書式が消える)。
    pub(crate) fn relayout_keep(&mut self) {
        let m = Metrics::new(&self.font_bytes).expect("フォント");
        let y0 = self.pg.top_mm + 4.0;
        if self.doc.vertical {
            // 縦書き: 行長 = 紙の縦の使い幅で組み、右からの列へ写す(K4)
            let measure =
                (self.pg.h_mm - self.pg.top_mm - self.pg.bottom_mm - 8.0).max(20.0);
            self.page = layout(
                &self.doc,
                &m,
                &Frame { measure_mm: measure, line_height_mm: LINE_MM, y0_mm: y0 },
            );
            kumihan::fold_vertical(&mut self.page, &self.pg, y0, LINE_MM);
        } else {
            self.page = layout(
                &self.doc,
                &m,
                &Frame { measure_mm: self.pg.column_measure_mm(), line_height_mm: LINE_MM, y0_mm: y0 },
            );
            kumihan::fold_columns(&mut self.page, &self.pg, y0);
        }
        self.refresh_hf();
    }

    /// クリックした画素位置(編集領域からの相対)にカーソルを置く。
    /// 文書の下端(紙の座標 mm)。1ページに満たなくても紙1枚ぶんは白い
    /// 紙の見た目の幅(mm。見開きなら2枚ぶん)
    pub(crate) fn paper_w_mm(&self) -> f32 {
        if self.multipage && !self.page.vertical {
            self.pg.w_mm * 2.0 + PAGE_GAP_MM
        } else {
            self.pg.w_mm
        }
    }

    pub(crate) fn content_mm(&self) -> f32 {
        if self.multipage && !self.page.vertical {
            // 2枚ずつの段。ページ数の半分(切り上げ)ぶんの高さ
            let pages = self.page_offsets.len().max(1);
            return (pages.div_ceil(2)) as f32 * self.pg.h_mm;
        }
        if self.page.vertical {
            // 縦書きは物理ページに畳んであるので、ページ数で決まる
            let pages = self.page.lines.iter()
                .map(|l| (l.y_mm / self.pg.h_mm) as usize)
                .max()
                .unwrap_or(0);
            return ((pages + 1) as f32) * self.pg.h_mm;
        }
        self.page.lines.last().map(|l| l.y_mm + 30.0).unwrap_or(0.0).max(self.pg.h_mm)
    }

    /// 縦にスクロールする(画素)。紙の頭より上・末尾より下へは行かない。
    pub(crate) fn scroll_px(&mut self, dy_px: f32) {
        let pxmm = PX_PER_MM * self.zoom;
        let view_mm = (self.view_h_px / pxmm).max(20.0);
        let max = (self.content_mm() + 20.0 - view_mm).max(0.0);
        self.scroll_mm = (self.scroll_mm + dy_px / pxmm).clamp(0.0, max);
    }

    /// キャレットが窓から出ていたら、見える所まで紙を送る。
    pub(crate) fn follow_caret(&mut self) {
        let pxmm = PX_PER_MM * self.zoom;
        let (_, cy, _) = self.caret_xy();
        let view_mm = (self.view_h_px / pxmm).max(20.0);
        if cy > self.scroll_mm + view_mm - 15.0 {
            self.scroll_mm = cy - (view_mm - 15.0);
        }
        if cy < self.scroll_mm + 5.0 {
            self.scroll_mm = (cy - 5.0).max(0.0);
        }
    }
}
