//! main.rs からの純移動(2026-08-06 の分割)。挙動は変えない。

use crate::*;

// 排他ロック・署名の鍵・16進は ops(calc と writer で1本。2026-08-12 段A)。
// 訳の要る文言だけ、ここで包む
// 16進と .sig の道の組み立ては、署名の中身ごと ops へ移りました
// (2026-08-21)。ここから使う物だけ残します
pub(crate) use ops::{lock_identity, lock_path_for};

/// 先客のロックを読む(あれば名乗りを返す)。自分自身のロックは先客と見ない。
pub(crate) fn foreign_lock(p: &std::path::Path) -> Option<String> {
    ops::foreign_lock(p, ui::t!("誰か"))
}

/// 鍵が用意できなかった理由を、その言語の文で言う(本体は ops)。
///
/// **鍵を読む所そのものは ops の1本**です(2026-08-21 に署名の中身も
/// そちらへ移しました)。ここに残るのは訳の要る文言だけで、置き場を
/// アプリ側にするのは、訳の走査が `calc/src` `writer/src` `ui/src` しか
/// 見ないからです。
pub(crate) fn key_err_msg(e: ops::KeyErr) -> String {
    match e {
        ops::KeyErr::Corrupt => ui::t!("鍵ファイルが壊れています(~/.config/officework/sign.key)").to_string(),
        ops::KeyErr::NoRandom(e) => ui::tf!("乱数が取れません: {}", e).to_string(),
        ops::KeyErr::CantStore(e) => ui::tf!("鍵が置けません: {}", e).to_string(),
    }
}

impl Calc {
    /// ブックの道を差し替える。**`book.path` も一緒に動かす** —
    /// `CELL("filename")` が `径路[ファイル名]シート名` を返すのに要る。
    /// 別々に持つと片方だけ古くなり、式が前のファイル名を答える
    pub(crate) fn set_path(&mut self, p: Option<PathBuf>) {
        self.book.path =
            p.as_ref().map(|x| x.display().to_string()).unwrap_or_default();
        self.path = p;
    }

    /// 自分のロックを外す(閉じる・別のファイルへ移るとき)。
    pub(crate) fn release_lock(&mut self) {
        if let Some(lp) = self.my_lock.take() {
            let _ = std::fs::remove_file(lp);
        }
    }

    /// このファイルのロックを見て、先客が居れば警告、居なければ自分が取る。
    pub(crate) fn acquire_lock(&mut self, p: &std::path::Path) {
        self.release_lock();
        match foreign_lock(p) {
            Some(who) => {
                self.locked_by = Some(who);
                // ロックは取らない(先客の邪魔をしない)
            }
            None => {
                self.locked_by = None;
                let lp = lock_path_for(p);
                // LibreOffice と同じ気持ちの中身(名乗りだけ。厳密な書式は要らない)
                if std::fs::write(&lp, format!("{},;", lock_identity())).is_ok() {
                    self.my_lock = Some(lp);
                }
            }
        }
    }

    /// **フォルダから探す**(2026-08-17 発注者。SFIND の写真)。
    /// 素の字は face が読み、**.xlsx は calc がセルの字を渡す**。
    /// 選んでも開かず、下の「読み込み」で初めて開く
    pub(crate) fn find_in_folder(&mut self) {
        let Some(dir) = self.find_dir() else {
            self.status = ui::t!("探す場所を選んでください").into();
            return;
        };
        let term = self.fd_term.text().to_string();
        if term.trim().is_empty() {
            self.status = ui::t!("探す字が空です").into();
            return;
        }
        self.fd_at = None;
        self.fd_peek.clear();
        // xlsx は**シートごとに1行=1行**にして渡す(行番号がセルの行に近い)
        let extract = |p: &std::path::Path| -> Option<String> {
            let e = p.extension().and_then(|x| x.to_str())?.to_ascii_lowercase();
            if e != "xlsx" {
                return None;
            }
            let f = std::fs::File::open(p).ok()?;
            let (book, _) = sheet::xlsx::read(std::io::BufReader::new(f)).ok()?;
            let mut out = String::new();
            for sh in &book.sheets {
                out.push_str(&format!("[{}]\n", sh.name));
                let (rows, cols) = sh.extent();
                for r in 0..rows {
                    let mut line = String::new();
                    for c in 0..cols {
                        let v = sh
                            .get(sheet::Pos::new(r, c))
                            .map(|x| x.value.display())
                            .unwrap_or_default();
                        if !line.is_empty() {
                            line.push('\t');
                        }
                        line.push_str(&v);
                    }
                    out.push_str(line.trim_end());
                    out.push('\n');
                }
            }
            Some(out)
        };
        let q = ui::search::Query {
            term,
            glob: self.fd_glob.text().to_string(),
            case: false,
            max_files: 4000,
            max_hits: 3000,
            extract: &extract,
        };
        let (hits, tally) = ui::search::walk(&dir, &q);
        self.fd_hits = hits;
        self.fd_tally = tally;
        // 報せは ui::search の1本(writer と calc で同じ文)
        self.status = ui::search::tally_message(&tally).into();
    }

    /// 当たりを1つ選ぶ。**開かない** — 下に前後を見せるだけ
    pub(crate) fn find_peek(&mut self, fi: usize, hi: usize) {
        let Some(f) = self.fd_hits.get(fi) else { return };
        let Some(h) = f.hits.get(hi) else { return };
        self.fd_at = Some((fi, hi));
        let body = std::fs::read_to_string(&f.path).ok();
        self.fd_peek = match body {
            Some(b) => {
                let lines: Vec<&str> = b.split('\n').collect();
                let i = (h.line as usize).saturating_sub(1);
                let from = i.saturating_sub(6);
                let to = (i + 7).min(lines.len());
                lines[from..to]
                    .iter()
                    .enumerate()
                    .map(|(k, l)| format!("{:05} {l}", from + k + 1))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            // xlsx は素の字で読めない — 当たりの行をそのまま見せる
            None => format!("{:05} {}", h.line, h.text),
        };
        self.status = ui::tf!(
            "{} の {} 行目(下の「読み込み」で開きます)",
            f.path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            h.line.to_string()
        )
        .into();
    }

    /// 下の「読み込み」。選んでいる当たりのブックを開く
    pub(crate) fn find_load(&mut self, cx: &mut Context<Self>) {
        let Some((fi, hi)) = self.fd_at else {
            self.status = ui::t!("当たりを選んでから読み込んでください").into();
            return;
        };
        let Some(f) = self.fd_hits.get(fi).cloned() else { return };
        let _ = hi;
        // **calc が開けるのは表だけ。** 素の字が当たっても開けないので、
        // 壊れた言い分を出さずにそう言う(writer で開いてください)
        let ok = f
            .path
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("xlsx") || e.eq_ignore_ascii_case("adoc"));
        if !ok {
            self.status = ui::tf!(
                "{} は calc では開けません(表ではない — writer で開いてください)",
                f.path.file_name().unwrap_or_default().to_string_lossy().to_string()
            )
            .into();
            return;
        }
        if self.dirty {
            self.status = ui::t!("いまの文書に未保存の変更があります(保存するか、捨ててから)").into();
            return;
        }
        self.open(f.path.clone());
        self.tab = self.prev_tab.max(1);
        cx.notify();
    }

    /// **探す場所。** 選んでいなければ(1)前に選んだ場所(settings.toml)
    /// (2)いま開いているブックの隣、の順(writer と同じ決め)
    pub(crate) fn find_dir(&self) -> Option<PathBuf> {
        if let Some(d) = &self.fd_dir {
            return Some(d.clone());
        }
        if let Some(s) = ui::settings::get("find_dir") {
            let p = PathBuf::from(s);
            if p.is_dir() {
                return Some(p);
            }
        }
        self.path.as_ref().and_then(|p| p.parent()).map(|d| d.to_path_buf())
    }

    /// 探す場所を選ぶ(**窓は別のスレッド**)
    pub(crate) fn find_dir_dialog(&mut self, cx: &mut Context<Self>) {
        let start = self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let ask = cx.background_executor().spawn(async move {
            let mut d = rfd::FileDialog::new();
            if let Some(s) = start {
                d = d.set_directory(s);
            }
            d.pick_folder()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = r {
                    this.status = ui::tf!("場所: {}", p.display().to_string()).into();
                    ui::settings::set("find_dir", &p.display().to_string());
                    this.fd_dir = Some(p);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open(&mut self, p: PathBuf) {
        let bytes = match std::fs::read(&p) {
            Ok(b) => b,
            Err(e) => {
                self.status = ui::tf!("開けません: {}", e).into();
                return;
            }
        };
        // **`.adoc` は字のファイル**なので、zip の道は通らない
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("adoc")) {
            self.open_adoc(p, bytes);
            return;
        }
        if ooxml::crypt::is_encrypted(&bytes) {
            // パネルでパスワードを聞き、Enter が続きをやる
            self.pw_pending = Some(p);
            self.pw_show = false;
            self.prompt = Some(("pw-open", Editor::new("")));
            self.status =
                ui::t!("このブックは暗号化されています。パスワードを打って Enter").into();
            return;
        }
        self.open_plain(p, bytes);
    }

    /// 平文(zip)の xlsx を読み込む。open とパスワードのパネルの共通の続き。
    pub(crate) fn open_plain(&mut self, p: PathBuf, bytes: Vec<u8>) {
        // 前のブックのパスワードを引きずらない(暗号化して開いた時だけ
        // パネルの続きが後から入れ直す)
        self.encrypt_pw = None;
        // 読めなかったときに拾い直すので、中身は控えておきます
        let bytes2 = bytes.clone();
        match sheet::xlsx::read(std::io::Cursor::new(bytes)) {
            Ok((mut book, rep)) => {
                sheet::recalc_all(&mut book);
                let notes = rep
                    .unsupported
                    .iter()
                    .map(|(n, c)| SharedString::from(format!("{n} × {c}")))
                    .collect();
                let status = ui::tf!(
                    "{} シート / {} セル — {}",
                    rep.sheets,
                    rep.cells,
                    p.file_name().unwrap_or_default().to_string_lossy()
                );
                self.adopt_book(p, book, notes, status);
            }
            // **読めなかった。**「開けません」で終わらせず、拾う道と
            // 控えを並べます(2026-08-09 発注者確定「拾う。ただし必ず言い、
            // 上書きは禁じる」)
            Err(e) => self.offer_repair(p, bytes2, &e),
        }
    }

    /// **壊れたブックの逃げ道を並べる**(開いて修復。2026-08-22)。
    ///
    /// 控えがあれば**先にそちらを勧めます** — 9世代の控えは、拾い集めた
    /// 穴あきより確実です。拾うのはその後の手段です。
    pub(crate) fn offer_repair(&mut self, p: PathBuf, bytes: Vec<u8>, why: &str) {
        let 控え = ops::history::list(Some(&p));
        let mut items: Vec<(String, String)> = 控え
            .iter()
            .map(|(name, _)| (name.clone(), ui::tf!("控えから開く: {}", name.clone()).to_string()))
            .collect();
        items.extend(crate::util::menu(&[ui::item!("→ 壊れたまま拾って開く(読み取り専用)")]));
        self.repair_pend = Some((p, bytes));
        self.pick_note = Some(if 控え.is_empty() {
            ui::t!("控えはありません。拾えた部品だけで開きます(上書きはできません)").into()
        } else {
            ui::t!("控えの方が確実です。拾うのはその後の手段です").into()
        });
        self.pick_kind = "repair";
        self.pick = Some((items, (60.0, 120.0)));
        self.status = ui::tf!("このファイルは読めません: {}", why).into();
    }

    /// **`.adoc` のブックを読み込む**(2026-08-19)。
    ///
    /// 値は持たないので、読んだ所で計算し直します(式が正本)。
    /// 読めなかった物は帳簿に出します。
    pub(crate) fn open_adoc(&mut self, p: PathBuf, bytes: Vec<u8>) {
        self.encrypt_pw = None;
        let src = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                self.status = ui::t!("開けません: 文字の並びが UTF-8 ではありません").into();
                return;
            }
        };
        match sheet::adoc::parse(&src) {
            Ok((mut book, report)) => {
                let mut notes: Vec<SharedString> =
                    report.iter().map(|r| SharedString::from(r.clone())).collect();
                // **見た目はテンプレートが決めます**(E群。SEKKEI 4段目)。
                // `.adoc` のブックは意味だけを持ちます — 列の幅・行の高さ・
                // 用紙の設定はここで当てます。
                //
                // *同じフォルダに `.tmpl.adoc` が1枚のときだけ*当てます。
                // 何枚もあるときは選ぶのが人の仕事なので、`find_for` が
                // None を返します(黙って1枚目を選ばない)。
                //
                // **当てたことは言います。** 開いただけで列の幅が変わるので、
                // 黙っていると「なぜこの幅なのか」が分かりません
                if let Some(tp) = sheet::booktmpl::find_for(&p) {
                    match std::fs::read_to_string(&tp)
                        .map_err(|e| e.to_string())
                        .and_then(|t| sheet::booktmpl::parse(&t))
                    {
                        Ok(theme) => {
                            sheet::booktmpl::apply(&theme, &mut book);
                            notes.push(SharedString::from(ui::tf!(
                                "見た目のテンプレートを当てました: {}",
                                tp.file_name().unwrap_or_default().to_string_lossy()
                            )));
                        }
                        // **読めなくてもブックは開きます。** 見た目が当たらない
                        // だけで、中身は読めています
                        Err(e) => notes.push(SharedString::from(ui::tf!(
                            "見た目のテンプレートが読めません({}): {}",
                            tp.file_name().unwrap_or_default().to_string_lossy(),
                            e
                        ))),
                    }
                }
                let status = ui::tf!(
                    "{} シート — {}",
                    book.sheets.len(),
                    p.file_name().unwrap_or_default().to_string_lossy()
                );
                self.adopt_book(p, book, notes, status);
            }
            Err(e) => self.status = ui::tf!("開けません: {}", e).into(),
        }
    }

    /// **見た目をテンプレートへ出す**(E群。2026-08-20)。
    ///
    /// 出したときだけ、その旨の1行を返します。
    ///
    /// *すでにテンプレートが1枚あるときは何もしません。* 配られた物は
    /// 書き替えない決めです(2026-08-18 発注者)。その代わり、見た目が
    /// そちらの持ち物であることを言います。
    ///
    /// *見た目が何も無いときも作りません。* 空のテンプレートを置いても
    /// フォルダが散らかるだけです。
    fn 見た目をテンプレートへ(&self, book: &std::path::Path) -> Option<String> {
        let theme = sheet::booktmpl::from_book(&self.book);
        if theme.is_empty() {
            return None;
        }
        if let Some(tp) = sheet::booktmpl::find_for(book) {
            return Some(
                ui::tf!(
                    "列の幅と用紙は「{}」の持ち物です(このファイルには書きません)",
                    tp.file_name().unwrap_or_default().to_string_lossy()
                )
                .to_string(),
            );
        }
        let tp = sheet::booktmpl::default_path(book);
        let src = sheet::booktmpl::write(&theme);
        match kumihan::atomic::save(&tp, |mut f| {
            use std::io::Write as _;
            f.write_all(src.as_bytes()).map_err(|e| e.to_string())
        }) {
            Ok(_) => Some(
                ui::tf!(
                    "列の幅と用紙を「{}」に書き出しました(次に開くとき当たります)",
                    tp.file_name().unwrap_or_default().to_string_lossy()
                )
                .to_string(),
            ),
            Err(e) => Some(ui::tf!("見た目のテンプレートが書けません: {}", e).to_string()),
        }
    }

    /// 読み終えたブックを画面に据える。**xlsx と adoc の共通の続き** —
    /// 片方だけ直して食い違うのを防ぐため、1本にしてあります。
    /// **拾い集めたブックを受け取る**(開いて修復)。
    ///
    /// 普通に開いたときと違うのは2つだけです — 上書きを断る旗を立てることと、
    /// **画面の下の帯に「拾い集めたもの」と出し続ける**ことです。状態行は
    /// 次の操作で流れるので、そこだけでは足りません。
    pub(crate) fn adopt_salvaged(
        &mut self,
        p: PathBuf,
        book: sheet::Book,
        notes: Vec<SharedString>,
        status: String,
    ) {
        self.adopt_book(p, book, notes, status);
        self.salvaged = true;
        // 拾った物は元のファイルの写しではありません。**錠は取りません** —
        // 読むだけなので、他の人の作業を止める理由がありません
        self.release_lock();
    }

    fn adopt_book(&mut self, p: PathBuf, book: sheet::Book, notes: Vec<SharedString>, status: String) {
        {
            {
                // **旗はここで下ろします。** 別のブックを開いたら、拾い集めた
                // ブックではありません(adopt_salvaged は後から立て直します)
                self.salvaged = false;
                self.repair_pend = None;
                self.notes = notes;
                self.status = status.into();
                self.book = book;
                // 計算方法はファイルの指定に従う(開いたときは上の一度きりの
                // 計算で値を見せ、以後の編集では勝手に回さない)
                self.auto_calc = !self.book.calc_manual;
                self.active = 0;
                self.cursor = Pos::new(0, 0);
                self.view = Pos::new(0, 0);
                self.split = None;
                self.anchor = None;
                self.frozen = None;
                self.auto_filter = None;
                self.filter_panel = None;
                // ファイルの固定枠を画面へ(sheet_ui もここで作り直す)
                self.freeze_from_book();
                self.undo_stack.clear();
                self.redo_stack.clear();
                self.clip_range = None;
                self.acquire_lock(&p);
                if let Some(who) = self.locked_by.clone() {
                    self.status = ui::tf!(
                        "{} — **{} が開いています**。上書き保存はできません(名前を付けて保存へ)",
                        self.status,
                        who
                    )
                    .into();
                }
                Self::note_recent(&p);
                self.set_path(Some(p));
                self.sync_input();
            }
        }
    }

    /// 上書きの前に、いまの中身を控える(最大9世代)。**中身は `ops::history`**
    /// — writer と calc で同じ物を使います
    pub(crate) fn keep_version(&self, p: &std::path::Path) {
        ops::history::keep(p);
    }

    /// 控えの一覧(新しい順)。(表示名, パス)
    pub(crate) fn versions(&self) -> Vec<(String, PathBuf)> {
        ops::history::list(self.path.as_deref())
    }

    /// 控えを開く。いまのファイルは動かさず、**名無しの複製**として読む
    /// (保存すると名前を聞く。元へ戻したいなら同じ名前で保存する —
    /// 黙って元のファイルを書き戻したりしない)。
    pub(crate) fn open_version(&mut self, q: &std::path::Path) {
        let raw = match std::fs::read(q) {
            Ok(b) => b,
            Err(e) => {
                self.status = ui::tf!("控えが読めません: {}", e).into();
                return;
            }
        };
        let raw = if ooxml::crypt::is_encrypted(&raw) {
            match self.encrypt_pw.as_ref().map(|pw| ooxml::crypt::decrypt(&raw, pw)) {
                Some(Ok(b)) => b,
                _ => {
                    self.status =
                        ui::t!("控えは暗号化されています(いまのパスワードでは解けません)").into();
                    return;
                }
            }
        } else {
            raw
        };
        match sheet::xlsx::read(std::io::Cursor::new(raw)) {
            Ok((mut book, _rep)) => {
                sheet::recalc_all(&mut book);
                self.release_lock();
                self.locked_by = None;
                self.book = book;
                // 計算方法はファイルの指定に従う(開いたときは上の一度きりの
                // 計算で値を見せ、以後の編集では勝手に回さない)
                self.auto_calc = !self.book.calc_manual;
                self.active = 0;
                self.cursor = Pos::new(0, 0);
                self.view = Pos::new(0, 0);
                self.split = None;
                self.anchor = None;
                self.frozen = None;
                self.auto_filter = None;
                self.filter_panel = None;
                // ファイルの固定枠を画面へ(sheet_ui もここで作り直す)
                self.freeze_from_book();
                self.undo_stack.clear();
                self.redo_stack.clear();
                self.clip_range = None;
                self.set_path(None);
                self.dirty = true;
                self.sync_input();
                self.status = ui::t!("控えを開きました(名無しの複製。保存で名前を聞きます。元へ戻すなら同じ名前で保存)").into();
            }
            Err(e) => self.status = ui::tf!("控えが読めません: {}", e).into(),
        }
    }

    /// 原本の中身(暗号化されていれば解いた平文)。部品の持ち越しに使う
    pub(crate) fn original_plain(&self) -> Option<Vec<u8>> {
        let bytes = std::fs::read(self.path.as_ref()?).ok()?;
        if ooxml::crypt::is_encrypted(&bytes) {
            let pw = self.encrypt_pw.as_ref()?;
            ooxml::crypt::decrypt(&bytes, pw).ok()
        } else {
            Some(bytes)
        }
    }

    /// 選択の生きた値(Excel の下端と同じ 合計・平均・個数)。
    /// 2セル以上を選んでいて、数のセルがあるときだけ出す。
    pub(crate) fn sel_stats(&self) -> Option<String> {
        self.anchor?;
        let (a, b) = self.sel_rect();
        let cells = (b.row - a.row + 1) as u64 * (b.col - a.col + 1) as u64;
        // 全選択のような巨大な矩形は数えない(描画のたびに走るので)
        if !(2..=200_000).contains(&cells) {
            return None;
        }
        let mut n = 0u64;
        let mut sum = 0.0f64;
        for r in a.row..=b.row {
            // 絞り込みで隠れた行は数えない(Excel と同じ — 見えている行の値)
            if !self.filter_keeps(r) {
                continue;
            }
            for c in a.col..=b.col {
                if let Some(Value::Number(v)) =
                    self.sheet().get(Pos::new(r, c)).map(|x| &x.value)
                {
                    n += 1;
                    sum += *v;
                }
            }
        }
        if n == 0 {
            return None;
        }
        let avg = (sum / n as f64 * 100.0).round() / 100.0;
        Some(format!(
            "合計 {} / 平均 {} / 個数 {n}",
            Value::Number(sum).display(),
            Value::Number(avg).display()
        ))
    }

    /// チャット(申し送り帳)の置き場。ブックの隣の 名前.xlsx.chat.txt
    pub(crate) fn chat_path(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| {
            let mut os = p.as_os_str().to_owned();
            os.push(".chat.txt");
            PathBuf::from(os)
        })
    }

    /// **控えの置き場と道は `ops` に1本**(2026-08-21)。文章にも要る
    /// ので出しました。ここは呼び出し側を変えないための包みです
    pub(crate) fn recover_path_for(orig: Option<&std::path::Path>) -> PathBuf {
        ops::recover_path_for(orig, "xlsx", "未保存のブック")
    }

    /// 自動復旧の控えを書く。**中身を写してから別スレッドで書く** —
    /// 大きな帳票で画面が止まらないように。成否は状態行に出さない
    /// (数分ごとに出ては邪魔なので、しくじったときだけ言う)
    pub(crate) fn write_recover(&mut self, cx: &mut Context<Self>) {
        let dst = Self::recover_path_for(self.path.as_deref());
        // 控えにも固定枠を載せる(復旧したときに画面が変わらないように)
        self.freeze_into_book();
        let book = self.book.clone();
        let orig = self.path.clone();
        let task = cx.background_executor().spawn(async move {
            if let Some(d) = dst.parent() {
                std::fs::create_dir_all(d).ok()?;
            }
            let mut buf = std::io::Cursor::new(Vec::new());
            sheet::xlsx::write(&book, &mut buf).ok()?;
            std::fs::write(&dst, buf.into_inner()).ok()?;
            // 元の道を添える(復旧のときに「どのファイルの控えか」を言う)
            if let Some(o) = &orig {
                std::fs::write(dst.with_extension("path"), o.to_string_lossy().as_bytes()).ok()?;
            }
            Some(())
        });
        cx.spawn(async move |this, cx| {
            let ok = task.await.is_some();
            let _ = this.update(cx, |c, _| {
                c.recover_at = std::time::Instant::now();
                if !ok {
                    // **黙って諦めない。** 控えが取れていないことは言う
                    c.status = ui::t!("自動復旧の控えが書けません(保存先の権限を確かめてください)")
                        .into();
                }
            });
        })
        .detach();
    }

    /// 無事に保存できたら控えは要らない(消し忘れると次の起動で
    /// 「落ちた後です」と嘘を言う)
    pub(crate) fn drop_recover(&self) {
        ops::drop_recover(self.path.as_deref(), "xlsx", "未保存のブック");
    }

    /// 起動のときに残っている控え(前回落ちた跡)。(見える名前, 控えの道)
    pub(crate) fn stale_recovers() -> Vec<(String, PathBuf)> {
        ops::stale_recovers("xlsx")
    }

    /// 最近開いた・保存したブックの控え(writer と同じ作法)
    /// 最近使ったファイルは **face::recent の1つの一覧**(統合の段8)。
    /// 文章と表で分けません — 使う人から見ればファイルはファイルです
    pub(crate) fn note_recent(p: &std::path::Path) {
        ui::recent::note(p);
    }

    pub(crate) fn recent_list() -> Vec<PathBuf> {
        ui::recent::list()
    }

    /// 新しいブック。未保存の変更があるときは作らない(黙って捨てない)。
    pub(crate) fn new_book(&mut self) -> bool {
        if self.dirty {
            self.status =
                ui::t!("未保存の変更があります。先に保存してください(Ctrl+S)").into();
            return false;
        }
        self.release_lock();
        self.locked_by = None;
        self.set_path(None);
        self.encrypt_pw = None;
        self.notes = Vec::new();
        self.book = Book::new();
        self.active = 0;
        self.cursor = Pos::new(0, 0);
        self.view = Pos::new(0, 0);
        self.split = None;
        self.anchor = None;
        self.frozen = None;
        self.auto_filter = None;
                self.filter_panel = None;
        self.slicers.clear();
        self.slicer_sel = 0;
        self.slicer_cfg = false;
        self.slicer_drag = None;
        self.sheet_ui.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.dirty = false;
        self.sync_input();
        self.status = ui::t!("新しいブックです").into();
        true
    }

    /// 名前を付けて保存(いつでもダイアログ。別のスレッド — rfd は同期)
    pub(crate) fn save_as(&mut self, cx: &mut Context<Self>) {
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                // **ブックの正本(AsciiDoc)を先頭に。** 窓が既定で選ぶのが
                // ここなので、並びがそのまま既定の形になる
                .add_filter("officework のブック", &["adoc"])
                .add_filter("Excelブック", &["xlsx"])
                // 型紙(XLTX)。中身は xlsx と同じで、開くと「新規」になる
                .add_filter("Excel の型紙", &["xltx"])
                .add_filter("CSV(いまのシートの値だけ)", &["csv"])
                .save_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = r {
                    let 打った名 = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    // **拡張子を打たなかったら正本(`.sheet.adoc`)。**
                    // 前は `.xlsx` に落ちていて、名前の決め(2026-08-18)と
                    // 食い違っていた
                    let 字で書く = p.extension().is_none()
                        || p.extension().is_some_and(|e| e.eq_ignore_ascii_case("adoc"));
                    if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv")) {
                        this.write_csv(&p);
                    } else if 字で書く {
                        // **表の名前に揃える。** `売上台帳` や `売上台帳.adoc` の
                        // まま書くと、一覧が「文書」と読んでしまう
                        let p = face::folder::as_sheet_adoc(&p);
                        let 直した名 =
                            p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                        this.save_to(p);
                        // **黙って名前を変えない。** 変えたときは状態行で言う
                        if 直した名 != 打った名 {
                            this.status =
                                ui::tf!("{} で保存しました(表は二重の拡張子で名前を付けます)", 直した名)
                                    .into();
                        }
                    } else {
                        this.save_to(p);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// いまのシートを CSV に書き出す(値だけ。UTF-8 BOM+CRLF — Excel が
    /// 文字化けせずに開ける形)。**self.path は動かさない** — CSV は式も
    /// 書式も他のシートも持てないので、「保存先」にはしない。
    pub(crate) fn export_csv_dialog(&mut self, cx: &mut Context<Self>) {
        self.commit();
        let name = format!("{}.csv", self.book.sheets[self.active].name);
        let ask = cx.background_executor().spawn(async move {
            rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .set_file_name(&name)
                .save_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = r {
                    this.write_csv(&p);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// CSV の形の並び `(鍵, 見出し, (文字コード, 区切り))`。
    /// **引き当ては鍵**(日本語のまま。`csv_kind` に持ち続ける字)、画面は見出し。
    /// **Shift_JIS を出せることが要**(日本の会計ソフトはまだ CP932)
    #[allow(clippy::type_complexity)]
    pub(crate) fn csv_kinds() -> Vec<(&'static str, &'static str, (&'static str, char))> {
        vec![
            row(ui::item!("UTF-8(BOM付き)・カンマ"), ("utf8bom", ',')),
            row(ui::item!("Shift_JIS(CP932)・カンマ"), ("sjis", ',')),
            row(ui::item!("UTF-8(BOMなし)・カンマ"), ("utf8", ',')),
            row(ui::item!("UTF-8(BOM付き)・タブ"), ("utf8bom", '\t')),
            row(ui::item!("Shift_JIS(CP932)・タブ"), ("sjis", '\t')),
            row(ui::item!("UTF-8(BOM付き)・セミコロン"), ("utf8bom", ';')),
        ]
    }

    /// いまの CSV の形の**見出し**(画面に出す字)。鍵が読めなければ鍵のまま
    pub(crate) fn csv_kind_label(&self) -> String {
        Self::csv_kinds()
            .iter()
            .find(|(k, _, _)| *k == self.csv_kind)
            .map(|(_, l, _)| (*l).to_string())
            .unwrap_or_else(|| self.csv_kind.to_string())
    }

    pub(crate) fn write_csv(&mut self, p: &std::path::Path) {
        let (enc, delim) = Self::csv_kinds()
            .iter()
            .find(|(k, _, _)| *k == self.csv_kind)
            .map(|(_, _, (e, d))| (*e, *d))
            .unwrap_or(("utf8bom", ','));
        let kind_label = self.csv_kind_label();
        let s = &self.book.sheets[self.active];
        let (rows, cols) = s.extent();
        let mut out = String::new();
        if enc == "utf8bom" {
            out.push('\u{feff}'); // BOM — Excel の既定の読みに合わせる
        }
        for r in 0..rows {
            let mut line: Vec<String> = Vec::new();
            for c in 0..cols.max(1) {
                let v = s
                    .get(sheet::Pos::new(r, c))
                    .map(|x| x.value.display())
                    .unwrap_or_default();
                if v.contains(delim) || v.contains('"') || v.contains('\n') || v.contains('\r') {
                    line.push(format!("\"{}\"", v.replace('"', "\"\"")));
                } else {
                    line.push(v);
                }
            }
            out.push_str(&line.join(&delim.to_string()));
            out.push_str("\r\n");
        }
        // Shift_JIS に無い字は「?」に化ける。**黙って化けさせない** —
        // 何文字落ちたかを数えて言う(帳票の名前が化けるのは事故)
        let (bytes, lost) = if enc == "sjis" {
            let (cow, _, had_err) = encoding_rs::SHIFT_JIS.encode(&out);
            let n = if had_err {
                out.chars().filter(|ch| {
                    let mut b = [0u8; 4];
                    let one = ch.encode_utf8(&mut b);
                    encoding_rs::SHIFT_JIS.encode(one).2
                }).count()
            } else {
                0
            };
            (cow.into_owned(), n)
        } else {
            (out.into_bytes(), 0)
        };
        match std::fs::write(p, bytes) {
            Ok(()) => {
                // 何が入らないかを黙らない(CSV は値だけの形式)
                self.status = ui::tf!(
                    "CSV に書き出しました: {}({} — いまのシートの値だけ。式・書式・他のシートは入りません){}",
                    p.display(),
                    kind_label,
                    if lost > 0 {
                        format!("。**{lost} 文字が Shift_JIS に無く「?」になりました**")
                    } else {
                        String::new()
                    }
                )
                .into();
            }
            Err(e) => {
                self.status = ui::tf!("CSV に書き出せませんでした: {}", e).into();
            }
        }
    }

    /// **いまのシートを Web の頁(HTML)に書き出す**(発注者 2026-08-15
    /// 「calc に web 書き出しを作ると楽になるでしょう」)。
    ///
    /// 台帳を正本にして頁を作る仕事は Python の台本でやってきたが、
    /// **1枚の表を1枚の頁にするだけなら、アプリから直に出せたほうが早い** —
    /// Python を持っていない人にも届く。
    ///
    /// 決め:
    ///
    /// - **JavaScript を使わない。** 表と字だけ。電波の細い所でも古い機械でも開く
    /// - **表示形式を通す**(`format_value`)。`0001` は `0001` のまま、
    ///   `¥#,##0` は `¥360` で出る。画面と同じ字が頁に出るのが筋
    /// - **1行目は見出し**(`<th>`)にする。表の頭は見出しである方が多い
    /// - **太字と揃えは持っていく**(それ以外の書式は落とす)
    /// - **式は結果を出す。** 頁を見る人に式は要らない
    /// - **結合は扱わない。** 落とすのではなく**そう言う** — 結合のあるシートは
    ///   状態行で件数を告げる(黙って崩さない)
    pub(crate) fn write_html(&mut self, p: &std::path::Path) {
        use std::fmt::Write as _;
        let s = &self.book.sheets[self.active];
        let (rows, cols) = s.extent();
        let cols = cols.max(1);
        let d1904 = self.book.date1904;
        let esc = |t: &str| {
            t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        };
        let 題 = s.name.clone();
        let mut out = String::new();
        let _ = write!(
            out,
            concat!(
                "<!doctype html>\n<html lang=\"ja\"><head><meta charset=\"utf-8\">\n",
                "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n",
                "<title>{}</title>\n<style>\n",
                "body{{font-family:sans-serif;max-width:60em;margin:0 auto;padding:1em;",
                "line-height:1.7;color:#222}}\n",
                "table{{border-collapse:collapse;width:100%}}\n",
                "th,td{{border:1px solid #ccc;padding:.4em .6em}}\n",
                "th{{background:#eff2f4;text-align:left}}\n",
                ".r{{text-align:right}}.c{{text-align:center}}.b{{font-weight:bold}}\n",
                "</style></head><body>\n<h1>{}</h1>\n<table>\n"
            ),
            esc(&題),
            esc(&題)
        );
        let mut 結合 = 0usize;
        for r in 0..rows {
            out.push_str("<tr>");
            for c in 0..cols {
                let pos = sheet::Pos::new(r, c);
                let cell = s.get(pos);
                let 字 = cell
                    .map(|x| {
                        sheet::model::format_value(
                            &x.value,
                            x.fmt.number_format.as_deref(),
                            d1904,
                        )
                    })
                    .unwrap_or_default();
                let mut 印 = String::new();
                if let Some(x) = cell {
                    if x.fmt.bold {
                        印.push('b');
                    }
                    match x.fmt.align {
                        sheet::model::HAlign::Right => 印.push('r'),
                        sheet::model::HAlign::Center => 印.push('c'),
                        _ => {}
                    }
                }
                let 組 = if 印.is_empty() {
                    String::new()
                } else {
                    format!(" class=\"{}\"", 印.chars().map(|ch| ch.to_string())
                        .collect::<Vec<_>>().join(" "))
                };
                let 名 = if r == 0 { "th" } else { "td" };
                let _ = write!(out, "<{名}{組}>{}</{名}>", esc(&字));
            }
            out.push_str("</tr>\n");
        }
        結合 += s.merges.len();
        out.push_str(concat!(
            "</table>\n<p style=\"color:#666;font-size:.9em\">",
            "この頁は表計算の台帳から作っています。</p>\n</body></html>\n"
        ));

        match std::fs::write(p, out.as_bytes()) {
            Ok(()) => {
                // 入らない物を黙らない
                self.status = ui::tf!(
                    "Web に書き出しました: {}(いまのシートだけ。式は結果、JavaScript なし){}",
                    p.display(),
                    if 結合 > 0 {
                        format!("。**結合 {結合} 箇所は頁では効きません**")
                    } else {
                        String::new()
                    }
                )
                .into();
            }
            Err(e) => {
                self.status = ui::tf!("Web に書き出せませんでした: {}", e).into();
            }
        }
    }

    /// Web に書き出す(場所を訊く)。CSV と同じ流儀 — **self.path は動かさない**
    pub(crate) fn export_html_dialog(&mut self, cx: &mut Context<Self>) {
        self.commit();
        let name = format!("{}.html", self.book.sheets[self.active].name);
        let ask = cx.background_executor().spawn(async move {
            rfd::FileDialog::new()
                .add_filter("Web の頁", &["html"])
                .set_file_name(&name)
                .save_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(mut p) = r {
                    if p.extension().is_none() {
                        p.set_extension("html");
                    }
                    this.write_html(&p);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn a_save(&mut self, _: &ui::Save, _: &mut Window, cx: &mut Context<Self>) {
        // .py の編集面が開いていれば、Ctrl+S はそちらの保存(ブックではない)
        if self.py_edit.is_some() {
            self.save_py_edit();
            cx.notify();
            return;
        }
        self.save(false, cx); cx.notify();
    }
    pub(crate) fn a_open(&mut self, _: &ui::Open, _: &mut Window, cx: &mut Context<Self>) {
        // **埋め込みなら officework に出してもらいます**(統合の段3)。
        // 開く物の種類で画面が決まるので、選ぶ窓は持ち主が出すのが筋です
        if self.embedded {
            self.open_dialog_request = true;
        } else {
            self.open_dialog(cx);
        }
        cx.notify();
    }

    /// 開くファイルを選ぶ。**ダイアログは別のスレッド** — rfd は同期で、
    /// メインスレッドで開くと画面ごと固まる(終了確認と同じ作法)。
    pub(crate) fn open_dialog(&mut self, cx: &mut Context<Self>) {
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .add_filter("ブック", &["xlsx", "adoc"])
                .add_filter("Excelブック", &["xlsx"])
                .add_filter("officework のブック", &["adoc"])
                .pick_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = r {
                    this.open(p);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 終了の要求。書きかけが無ければ即終了、あれば確認を**別のスレッド**で出す。
    /// 確認のダイアログでメインスレッドを塞がない — 塞ぐと画面ごと固まり、
    /// GNOME に「応答なし」と判定される(踏んで直した)。
    /// 「はい」でも保存できなかった(保存の窓を閉じた等)なら終了しない —
    /// 書きかけを黙って捨てない。
    pub(crate) fn request_quit(&mut self, cx: &mut Context<Self>) {
        self.commit();
        // 確認を出すのは**未保存の変更があるとき**。名前の無い新規でも、
        // 何か打ってあれば出す — 打った物を黙って捨てない(発注者 2026-08-06。
        // 2026-08-03 の「実ファイルに限る」を改訂: 新規が見本入りだった頃は
        // 「試し打ち」扱いでよかったが、空白の新規は実の仕事が始まる場所)。
        // 本当に空のままの新規は、従来どおり黙って閉じる(煩くしない)
        let empty_new = self.path.is_none()
            && self.book.sheets.iter().all(|s| s.cells.is_empty());
        if !self.dirty || empty_new {
            self.release_lock();
            cx.quit();
            return;
        }
        // 確認は**窓の中のパネル**で出す。rfd の OS ダイアログは親窓を持てず
        // **スクリーンの中央**に出て、窓から離れすぎる(発注者 2026-08-06)
        self.quit_ask = true;
        cx.notify();
    }

    pub(crate) fn a_quit(&mut self, _: &ui::Quit, _: &mut Window, cx: &mut Context<Self>) {
        self.request_quit(cx);
    }

    /// PDF に書き出す。保存先の選択は**別のスレッド**(rfd は同期)。
    pub(crate) fn save_pdf(&mut self, cx: &mut Context<Self>) {
        self.commit();
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name("帳票.pdf")
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

    /// 画像ファイルを選んで、いまのセルに浮かべる(選択は別のスレッド)。
    pub(crate) fn insert_image_dialog(&mut self, cx: &mut Context<Self>) {
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .add_filter("画像", &["png", "jpg", "jpeg", "bmp", "gif"])
                .pick_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = r {
                    match std::fs::read(&p) {
                        Ok(data) => match image_px(&data) {
                            Some((w, h)) => {
                                this.checkpoint();
                                let at = this.cursor;
                                this.sheet_mut().images_new.push(sheet::model::SheetImage {
                                    at,
            dx_px: 0.0,
            dy_px: 0.0,
                                    width_px: w as f32,
                                    height_px: h as f32,
                                    data,
                                });
                                this.dirty = true;
                                this.status = ui::tf!(
                                    "画像を {} に置きました(保存で xlsx に入ります)",
                                    at.a1()
                                )
                                .into();
                            }
                            None => {
                                this.status =
                                    ui::t!("この画像は読めません(PNG か JPEG を選んでください)").into();
                            }
                        },
                        Err(e) => this.status = ui::tf!("読めません: {}", e).into(),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// **いまのシートの紙**(大きさ・向き)と、効かせたものの言い分。
    /// PDF と画面の紙の切れ目が**同じ紙で数える**ように一か所に置く
    pub(crate) fn paper_of_sheet(&self) -> (paper::Paper, Vec<String>) {
        let sh = &self.book.sheets[self.active];
        let mut paper = paper::Paper::default();
        let mut desc: Vec<String> = Vec::new();
        if let Some(code) = sh.paper_size {
            match paper_mm(code) {
                Some((w, h, name)) => {
                    paper.width_mm = w;
                    paper.height_mm = h;
                    if code != 9 {
                        desc.push(name.into());
                    }
                }
                None => desc.push(format!("用紙コード{code}は未対応・A4で出します")),
            }
        }
        if sh.landscape {
            std::mem::swap(&mut paper.width_mm, &mut paper.height_mm);
            desc.push(ui::t!("横向き").into());
        }
        (paper, desc)
    }

    /// 画面に見せる**紙の切れ目**(行, 列)。刷る側と同じ規則で数える
    pub(crate) fn page_breaks_now(&self) -> (Vec<u32>, Vec<u32>) {
        let (paper, _) = self.paper_of_sheet();
        let sh = &self.book.sheets[self.active];
        let setup = paper::grid::PrintSetup {
            areas: sh.print_areas.clone(),
            margins_mm: sh.margins_mm,
            date1904: self.book.date1904,
        };
        paper::grid::page_starts(sh, paper, &setup)
    }

    /// **ブック全体を1つの PDF に。** シートを順に束ね、頁番号(&P)と
    /// 総頁(&N)は**ブック通し**で振る(paper::grid::book_to_pdf)。
    /// 隠したシートは刷らない(画面と同じです)。返り値は報告用の文言です。
    ///
    /// いま呼んでいるのは rpc.rs だけで、そちらは unix でしか組みません。
    /// そのため Windows では「使われていない」と警告が出ます。この関数自体は
    /// どの OS でも動くので、消さずに警告だけ止めます。
    #[cfg_attr(not(unix), allow(dead_code))]
    pub(crate) fn write_book_pdf(&mut self, p: &std::path::Path) -> Result<String, String> {
        let (fam, exact) = kumihan::font::for_document(None).map_err(|e| e.to_string())?;
        let data = kumihan::font::load(fam).map_err(|e| e.to_string())?;
        let prev = self.active;
        let mut jobs: Vec<(&sheet::Sheet, paper::Paper, paper::grid::PrintSetup)> = Vec::new();
        // 紙と余白は**シートごと**に効く(1冊に縦と横が混ざってよい)。
        // paper_of_sheet はいま出ているシートを見るので、順に差し替えて集める
        let mut papers: Vec<paper::Paper> = Vec::new();
        for i in 0..self.book.sheets.len() {
            self.active = i;
            papers.push(self.paper_of_sheet().0);
        }
        self.active = prev;
        for (i, sh) in self.book.sheets.iter().enumerate() {
            if sh.hidden {
                continue;
            }
            jobs.push((
                sh,
                papers[i],
                paper::grid::PrintSetup {
                    areas: sh.print_areas.clone(),
                    margins_mm: sh.margins_mm,
                    date1904: self.book.date1904,
                },
            ));
        }
        if jobs.is_empty() {
            return Err(ui::t!("刷るシートがありません(全部隠れています)").to_string());
        }
        let n_sheets = jobs.len();
        let mut buf = Vec::new();
        let clipped = paper::grid::book_to_pdf(&jobs, &data, &mut buf)?;
        std::fs::write(p, buf).map_err(|e| e.to_string())?;
        Ok(format!(
            "PDF にしました — {}({} シート){}{}",
            p.file_name().unwrap_or_default().to_string_lossy(),
            n_sheets,
            if exact { "" } else { " ※代替フォント" },
            if clipped > 0 {
                format!("({clipped} 列は1列で紙より広く、切れています)")
            } else {
                String::new()
            }
        ))
    }

    pub(crate) fn write_pdf(&mut self, p: &std::path::Path) {
        let (fam, exact) = match kumihan::font::for_document(None) {
            Ok(x) => x,
            Err(e) => {
                self.status = ui::tf!("PDF にできません: {}", e).into();
                return;
            }
        };
        let data = match kumihan::font::load(fam) {
            Ok(d) => d,
            Err(e) => {
                self.status = ui::tf!("PDF にできません: {}", e).into();
                return;
            }
        };
        // 帳票の印刷設定(pageSetup / pageMargins / Print_Area)に従う。
        // 効かせたものは status に言う(黙って既定で出さない)
        let sh = &self.book.sheets[self.active];
        let mut paper = paper::Paper::default();
        let mut desc: Vec<String> = Vec::new();
        if let Some(code) = sh.paper_size {
            match paper_mm(code) {
                Some((w, h, name)) => {
                    paper.width_mm = w;
                    paper.height_mm = h;
                    if code != 9 {
                        desc.push(name.into());
                    }
                }
                None => desc.push(format!("用紙コード{code}は未対応・A4で出します")),
            }
        }
        if sh.landscape {
            std::mem::swap(&mut paper.width_mm, &mut paper.height_mm);
            desc.push(ui::t!("横向き").into());
        }
        let areas = sh.print_areas.clone();
        let setup = paper::grid::PrintSetup {
            areas: areas.clone(),
            margins_mm: sh.margins_mm,
            date1904: self.book.date1904,
        };
        match areas.len() {
            0 => {}
            1 => desc.push(format!("印刷範囲 {}:{}", areas[0].0.a1(), areas[0].1.a1())),
            n => desc.push(format!(
                "印刷範囲 {} 域(それぞれ別の紙に刷ります): {}",
                n,
                areas
                    .iter()
                    .map(|(a, b)| format!("{}:{}", a.a1(), b.a1()))
                    .collect::<Vec<_>>()
                    .join("、")
            )),
        }
        let mut clipped = 0u32;
        let r = kumihan::atomic::save(p, |f| {
            paper::grid::sheet_to_pdf(
                &self.book.sheets[self.active],
                &data,
                paper,
                &setup,
                std::io::BufWriter::new(f),
            )
            .map(|n| clipped = n)
        });
        self.status = match r {
            // 紙に入り切らなかった列は黙らない
            Ok(_) => format!(
                "PDF にしました — {}{}{}{}",
                p.file_name().unwrap_or_default().to_string_lossy(),
                if desc.is_empty() {
                    String::new()
                } else {
                    format!("({})", desc.join("・"))
                },
                if exact { "" } else { " ※代替フォント" },
                if clipped > 0 {
                    format!("({clipped} 列は1列で紙より広く、切れています — 幅を詰めるか用紙を大きく)")
                } else {
                    String::new()
                }
            )
            .into(),
            Err(e) => format!("PDF にできません: {e}").into(),
        };
    }

    /// 保存。名前が無ければ選ばせる(**ダイアログは別のスレッド**)。
    /// `then_quit` なら保存が済んだときだけ終了する — 書きかけを黙って捨てない。
    pub(crate) fn save(&mut self, then_quit: bool, cx: &mut Context<Self>) {
        self.commit();
        // **拾い集めたブックで元のファイルを上書きしません**(2026-08-09
        // 発注者確定)。いちばん怖い事故は、穴が空いたのに気づかないまま
        // 元の壊れたファイルを上書きすることです。そうなるともう戻せません。
        // 名前を付けて保存はできます — そちらは別のファイルです
        if self.salvaged {
            self.status = ui::t!(
                "拾い集めたブックなので上書きしません。名前を付けて保存してください(元のファイルは触りません)"
            )
            .into();
            return;
        }
        if let Some(p) = self.path.clone() {
            if self.locked_by.is_some() {
                // 先客の作業を後勝ちで潰さない。別の名前でなら保存できる
                self.status = ui::tf!("{} が開いているため上書きしません。名前を付けて保存してください", self.locked_by.as_deref().unwrap_or("誰か"))
                .into();
            } else {
                self.save_to(p);
                if then_quit && !self.dirty {
                    self.release_lock();
                    cx.quit();
                }
                return;
            }
        }
        let ask = cx.background_executor().spawn(async {
            rfd::FileDialog::new()
                .add_filter("Excelブック", &["xlsx"])
                .save_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Some(p) => {
                        this.save_to(p);
                        if then_quit && !this.dirty {
                            this.release_lock();
                            cx.quit();
                        }
                    }
                    None => this.status = ui::t!("保存をやめました(名前が決まっていません)").into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 決まった場所へ書く。成功すると dirty が消える。
    pub(crate) fn save_to(&mut self, p: PathBuf) {
        // 画面の固定枠をモデルへ。**書く前に必ず** — これが無いと、
        // calc で固定した枠がファイルに載らない
        self.freeze_into_book();
        // 原本の部品(図形・テーマ・印刷設定)を持ち越す。読み終えてから書く。
        // 暗号化されていた原本は解いた平文を渡す
        // **`.adoc` を開いていたら原本は渡さない** — 字のファイルは xlsx の
        // 部品を持っていないので、zip として読ませると保存が落ちます
        let 元は字 = self
            .path
            .as_ref()
            .is_some_and(|q| q.extension().is_some_and(|e| e.eq_ignore_ascii_case("adoc")));
        // **拾い集めたブックでは原本を渡しません。** 原本は壊れた zip なので、
        // そこから部品を持ち越そうとすると保存ごと落ちます(2026-08-22)
        let original: Option<std::io::Cursor<Vec<u8>>> = if 元は字 || self.salvaged {
            None
        } else {
            self.original_plain().map(std::io::Cursor::new)
        };
        let 字で書く = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("adoc"));
        // 上書きの前に、直前の中身をバージョン履歴に控える
        if p.exists() {
            self.keep_version(&p);
        }
        let saved = if 字で書く {
            if self.encrypt_pw.is_some() {
                // **暗号を黙って外さない。** AsciiDoc は字のままのファイル
                // なので暗号化して書けない。前はここで平文のまま書いていて、
                // パスワードで守ったつもりのブックが誰でも読める字になった
                // (2026-08-19 の見直しで気づいた)
                Err(ui::t!("AsciiDoc は字のままのファイルなので、暗号化したまま保存できません(暗号化を外すか、xlsx で保存してください)").to_string())
            } else {
                // **ブックの正本を `.adoc` で書く**(2026-08-19)。値は書かず、
                // 式のまま出します。載らない物は下で帳簿に出します
                let src = sheet::adoc::write(&self.book);
                kumihan::atomic::save(&p, |mut f| {
                    use std::io::Write as _;
                    f.write_all(src.as_bytes()).map_err(|e| e.to_string())
                })
            }
        } else if let Some(pw) = self.encrypt_pw.clone() {
            // 暗号化は zip 丸ごとが単位 — 一度メモリへ書いてから包む。
            // Agile 方式(AES-256。Excel 2013+ の既定)で書く — 本物と相互
            // 検証済み。読みは Standard(2007)も Agile も両方できる
            let mut plain = Vec::new();
            sheet::xlsx::write_with(&self.book, original, std::io::Cursor::new(&mut plain))
                .and_then(|_| ooxml::crypt::encrypt_agile(&plain, &pw))
                .and_then(|enc| {
                    kumihan::atomic::save(&p, |mut f| {
                        use std::io::Write as _;
                        f.write_all(&enc).map_err(|e| e.to_string())
                    })
                })
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xltx")) {
            // 型紙(XLTX)。中身は xlsx と同じで宣言だけ違う — 一度memory へ
            // 書いてから仕立て直す。**仕立てに失敗したら xlsx のまま出さない**
            let mut plain = Vec::new();
            sheet::xlsx::write_with(&self.book, original, std::io::Cursor::new(&mut plain))
                .and_then(|_| sheet::xlsx::to_template(&plain))
                .and_then(|t| {
                    kumihan::atomic::save(&p, |mut f| {
                        use std::io::Write as _;
                        f.write_all(&t).map_err(|e| e.to_string())
                    })
                })
        } else {
            kumihan::atomic::save(&p, |f| {
                sheet::xlsx::write_with(&self.book, original, std::io::BufWriter::new(f))
            })
        };
        match saved {
            Ok(_) => {
                // **書けたら旗を下ろします。** 新しいファイルは穴あきの
                // 元ファイルではないので、これ以降は上書きできます
                if self.salvaged {
                    self.salvaged = false;
                    self.notes.clear();
                }
                // 文に差し込む添え書きも画面の文言 — 訳さないと日本語だけ残る
                let enc_note = if self.encrypt_pw.is_some() {
                    ui::t!("(暗号化)")
                } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xltx")) {
                    ui::t!("(型紙 — 開くと新しいブックになります)")
                } else if 字で書く {
                    ui::t!("(式のまま。見た目は載りません)")
                } else {
                    ""
                };
                // **載らなかった物を黙って落とさない。** 帳簿に出します
                if 字で書く {
                    self.notes =
                        sheet::adoc::write_report(&self.book).into_iter().map(SharedString::from).collect();
                    // **見た目の行き先を作ります**(E群)。`.adoc` のブックは
                    // 意味だけを持つので、列の幅・行の高さ・用紙はここで
                    // テンプレートへ出します。出さないと、保存して開き直す
                    // たびに幅が既定へ戻ります。
                    //
                    // **すでにある物は書き替えません**(2026-08-18 発注者
                    // 「テンプレートの持ち主は指示する人」)。1枚あるなら、
                    // 見た目はそちらの持ち物です
                    if let Some(m) = self.見た目をテンプレートへ(&p) {
                        self.notes.push(SharedString::from(m));
                    }
                }
                self.status = ui::tf!("保存しました — {}{}", p.file_name().unwrap_or_default().to_string_lossy(), enc_note)
                .into();
                self.acquire_lock(&p);
                Self::note_recent(&p);
                // **無事に保存できたら自動復旧の控えは捨てる。** 残すと
                // 次の起動で「前回落ちました」と嘘を言う。道が変わる
                // (名前を付けて保存)ときは古い道の分も捨てる
                self.drop_recover();
                self.set_path(Some(p));
                self.drop_recover();
                self.dirty = false;
                // 挿した絵はもう原本(いま書いたファイル)にある。次の保存で
                // 二重に書かないよう「読んだ側」へ持ち場を移す
                for sh in &mut self.book.sheets {
                    let moved: Vec<_> = sh.images_new.drain(..).collect();
                    sh.images.extend(moved);
                    let moved: Vec<_> = sh.shapes_new.drain(..).collect();
                    sh.shapes.extend(moved);
                }
                self.shape_sel = None;
                self.point_edit = None;
            }
            Err(e) => self.status = ui::tf!("保存できません: {}", e).into(),
        }
    }
}
